// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The state of the daemon's links (spec §4.2, §9.3): one session per link,
//! begun, attached, ended and begun again after the back-off; the credit
//! the broker grants a sending link and the credit the daemon grants on a
//! receiving one (AMQP 1.0 Part 2 §2.5, §2.6).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::string::{String, ToString};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::vec::Vec;

use zerodds_rtps::wire_types::EntityId;

use super::acl::Live;
use super::config::{DaemonConfig, History, KEEP_ALL_QUEUE_CAP, Reconnect, TopicMapping};
use super::dds_pump::drain_all;
use super::log::log;
use crate::broker::{BrokerWriter, INITIAL_DELIVERY_COUNT, Inbound, LinkSpec, sender_credit};
use crate::daemon_runtime::BridgeMetrics;

/// A message waiting for credit, or unsettled at the broker.
#[derive(Clone)]
pub(super) struct Pending {
    pub(super) message: Vec<u8>,
    pub(super) attempts: u32,
    /// When the DDS sample was taken, in nanoseconds since the Unix epoch
    /// (§8.1 `latency_us`, §8.3); 0 for a message of no sample (the
    /// catalog).
    pub(super) taken_ns: u64,
}

/// Where a link's session stands. Every link has a session of its own, so
/// a broker that ends one (RabbitMQ does for a queue that does not exist)
/// leaves the others running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LinkState {
    /// No session; begun again at `retry_at`, or at once when `None`.
    Down,
    /// Begin sent, waiting for the broker's.
    Beginning,
    /// Attach sent, waiting for the broker's.
    Attaching,
    /// Attached.
    Up,
    /// End sent, waiting for the broker's.
    Ending,
}

/// A sending link: one per `out`/`bidir` topic, and the catalog's.
pub(super) struct OutLink {
    /// The topic it carries; `None` for the catalog link.
    pub(super) topic: Option<usize>,
    /// The DDS name of that topic, empty for the catalog link (§8.1, §8.3).
    pub(super) dds_topic: String,
    /// The channel of the link's session.
    pub(super) channel: u16,
    pub(super) state: LinkState,
    pub(super) spec: LinkSpec,
    /// Send settled (AT_MOST_ONCE).
    pub(super) settled: bool,
    pub(super) remote_handle: Option<u32>,
    /// Credit the broker granted and deliveries have not used.
    pub(super) credit: u32,
    /// Deliveries sent since the attach, from its initial-delivery-count
    /// (Part 2 §2.6.7).
    pub(super) delivery_count: u32,
    /// The broker's drain flag (Part 2 §2.6.7): credit the link cannot use
    /// is consumed by advancing `delivery_count` ([`super::dds_pump::drain`]).
    pub(super) drain: bool,
    /// The broker asked for the link's flow state (Part 2 §2.7.4 `echo`);
    /// the next [`super::dds_pump::drain`] sends it.
    pub(super) echo: bool,
    /// The stop of §9.2 is seen: no message of the queue goes out any more.
    pub(super) stopping: bool,
    pub(super) queue: VecDeque<Pending>,
    /// Unsettled deliveries by delivery id.
    pub(super) unsettled: BTreeMap<u32, Pending>,
    pub(super) cap: usize,
    /// The next delivery tag; unique per link (Part 2 §2.6.12).
    pub(super) next_tag: u64,
    /// When a link the broker detached is attached again.
    pub(super) retry_at: Option<Instant>,
    pub(super) retry_delay: Duration,
    /// The topic's DDS reader, whose missed deadlines the pump watches
    /// (§6); `None` for the catalog link.
    pub(super) reader: Option<EntityId>,
    /// The reader's REQUESTED_DEADLINE_MISSED total last seen.
    pub(super) deadline_missed: u64,
    /// A total not yet carried by a message: the next message of the topic
    /// takes it as its `x-opt-zerodds-deadline-missed` annotation.
    pub(super) deadline_note: Option<u64>,
    /// The PARTITION of each writer the reader matched (`dds:partition`).
    pub(super) writer_partitions: BTreeMap<[u8; 16], Vec<String>>,
    /// The writers of a keyed topic whose samples came without a KeyHash
    /// the reader could give, each logged once (`key_hash_unavailable`).
    pub(super) key_hash_unknown: BTreeSet<[u8; 16]>,
    /// The participant's ACL as the last reload left it: a message of the
    /// topic goes out only while the topic may be read (§7.3, §9.2).
    pub(super) acl: Arc<Live>,
}

/// A receiving link: one per `in`/`bidir` topic.
pub(super) struct InLink {
    pub(super) topic: usize,
    /// The channel of the link's session.
    pub(super) channel: u16,
    pub(super) state: LinkState,
    pub(super) spec: LinkSpec,
    pub(super) remote_handle: Option<u32>,
    pub(super) writer: EntityId,
    pub(super) delivery_count: u32,
    /// Credit still granted to the broker.
    pub(super) outstanding: u32,
    /// `rcv-settle-mode` second (Part 2 §2.8.3): the bridge sends its
    /// outcome unsettled and settles once the broker has.
    pub(super) second: bool,
    /// Deliveries with an outcome sent, waiting for the broker's settlement
    /// (mode second). They take up credit: at most `receiver.credit`.
    pub(super) awaiting: std::collections::BTreeSet<u32>,
    pub(super) retry_at: Option<Instant>,
    pub(super) retry_delay: Duration,
}

impl OutLink {
    /// The sending link of topic `topic` (DDS → AMQP) with handle and
    /// session channel `channel`, down. Its queue holds the topic's HISTORY
    /// depth (§6), [`KEEP_ALL_QUEUE_CAP`] for KEEP_ALL. `acl` is the
    /// participant's live ACL.
    pub(super) fn for_topic(
        cfg: &DaemonConfig,
        topic: usize,
        channel: u16,
        acl: &Arc<Live>,
    ) -> Self {
        let t = &cfg.topics[topic];
        Self {
            topic: Some(topic),
            dds_topic: t.dds_name.clone(),
            channel,
            state: LinkState::Down,
            spec: link_spec(cfg, t, u32::from(channel), true),
            settled: t.settled,
            remote_handle: None,
            credit: 0,
            delivery_count: INITIAL_DELIVERY_COUNT,
            drain: false,
            echo: false,
            stopping: false,
            queue: VecDeque::new(),
            unsettled: BTreeMap::new(),
            cap: match t.history {
                History::KeepLast(d) => d,
                History::KeepAll => KEEP_ALL_QUEUE_CAP,
            },
            next_tag: 0,
            retry_at: None,
            retry_delay: Duration::from_millis(cfg.reconnect.initial_delay_ms),
            reader: None,
            deadline_missed: 0,
            deadline_note: None,
            writer_partitions: BTreeMap::new(),
            key_hash_unknown: BTreeSet::new(),
            acl: Arc::clone(acl),
        }
    }
}

impl InLink {
    /// The receiving link of topic `topic` (AMQP → DDS) into the DDS writer
    /// `writer`, with handle and session channel `channel`, down; settle
    /// mode second when the topic's `rcv_settle_mode` is 1 (Part 2 §2.8.3).
    pub(super) fn for_topic(
        cfg: &DaemonConfig,
        topic: usize,
        channel: u16,
        writer: EntityId,
    ) -> Self {
        let t = &cfg.topics[topic];
        Self {
            topic,
            channel,
            state: LinkState::Down,
            spec: link_spec(cfg, t, u32::from(channel), false),
            remote_handle: None,
            writer,
            delivery_count: 0,
            outstanding: 0,
            second: t.rcv_settle_mode == 1,
            awaiting: std::collections::BTreeSet::new(),
            retry_at: None,
            retry_delay: Duration::from_millis(cfg.reconnect.initial_delay_ms),
        }
    }
}

pub(super) fn link_spec(
    cfg: &DaemonConfig,
    t: &TopicMapping,
    handle: u32,
    sender: bool,
) -> LinkSpec {
    LinkSpec {
        name: format!(
            "{}/{}/{}",
            cfg.container_id,
            t.dds_name,
            if sender { "sender" } else { "receiver" }
        ),
        handle,
        sender,
        address: t.amqp_address.clone(),
        snd_settle_mode: t.snd_settle_mode,
        rcv_settle_mode: t.rcv_settle_mode,
        durable: t.terminus_durable(sender),
        properties: vec![
            ("zerodds_topic".into(), t.dds_name.clone()),
            ("zerodds_type".into(), t.dds_type.clone()),
        ],
        desired_capabilities: vec!["dds.cdr2".into()],
        coordinator: None,
    }
}

/// The back-off after `d` (§9.3): `d` times the factor, between the
/// initial and the maximum delay.
pub(super) fn next_delay(d: Duration, rc: &Reconnect) -> Duration {
    let next = (d.as_millis() as f64 * rc.factor) as u64;
    Duration::from_millis(next.min(rc.max_delay_ms).max(rc.initial_delay_ms))
}

/// Begins the session of every link that has none.
pub(super) fn begin_all(
    writer: &mut BrokerWriter,
    outs: &mut [OutLink],
    ins: &mut [InLink],
) -> Result<(), String> {
    for l in outs.iter_mut().filter(|l| l.state == LinkState::Down) {
        writer
            .begin(l.channel)
            .map_err(|e| format!("begin {}: {e}", l.spec.name))?;
        l.state = LinkState::Beginning;
        l.retry_at = None;
    }
    for l in ins.iter_mut().filter(|l| l.state == LinkState::Down) {
        writer
            .begin(l.channel)
            .map_err(|e| format!("begin {}: {e}", l.spec.name))?;
        l.state = LinkState::Beginning;
        l.retry_at = None;
    }
    Ok(())
}

/// Begins again the session of every link whose back-off has run out.
pub(super) fn retry_due(
    writer: &mut BrokerWriter,
    outs: &mut [OutLink],
    ins: &mut [InLink],
    now: Instant,
) -> Result<(), String> {
    for l in outs.iter_mut() {
        if l.state == LinkState::Down && l.retry_at.is_some_and(|t| now >= t) {
            l.retry_at = None;
            writer
                .begin(l.channel)
                .map_err(|e| format!("begin {}: {e}", l.spec.name))?;
            l.state = LinkState::Beginning;
        }
    }
    for l in ins.iter_mut() {
        if l.state == LinkState::Down && l.retry_at.is_some_and(|t| now >= t) {
            l.retry_at = None;
            writer
                .begin(l.channel)
                .map_err(|e| format!("begin {}: {e}", l.spec.name))?;
            l.state = LinkState::Beginning;
        }
    }
    Ok(())
}

/// The connection is gone (§9.3): every link is down, its session begun
/// on the next connection at once; unsettled deliveries go out again first,
/// in the order they were sent.
pub(super) fn connection_lost(outs: &mut [OutLink], ins: &mut [InLink]) {
    for l in outs.iter_mut() {
        let unsettled: Vec<Pending> = std::mem::take(&mut l.unsettled).into_values().collect();
        for p in unsettled.into_iter().rev() {
            l.queue.push_front(p);
        }
        l.remote_handle = None;
        l.credit = 0;
        l.retry_at = None;
        l.state = LinkState::Down;
    }
    for l in ins.iter_mut() {
        l.remote_handle = None;
        l.outstanding = 0;
        l.awaiting.clear();
        l.retry_at = None;
        l.state = LinkState::Down;
    }
}

/// A link whose session ended: its unsettled deliveries wait in the queue,
/// the session is begun again after the back-off.
pub(super) fn out_link_down(cfg: &DaemonConfig, l: &mut OutLink, reason: &str) {
    let unsettled: Vec<Pending> = std::mem::take(&mut l.unsettled).into_values().collect();
    for p in unsettled.into_iter().rev() {
        l.queue.push_front(p);
    }
    l.remote_handle = None;
    l.credit = 0;
    l.state = LinkState::Down;
    l.retry_at = Some(Instant::now() + l.retry_delay);
    log(
        "link_down",
        &[
            ("link_name", &l.spec.name),
            ("dds_topic", &l.dds_topic),
            ("amqp_address", &l.spec.address),
            ("err", reason),
            ("retry_ms", &l.retry_delay.as_millis().to_string()),
        ],
    );
    l.retry_delay = next_delay(l.retry_delay, &cfg.reconnect);
}

/// Grants the broker `credit` again on a receiving link once half of it is
/// used (§4.4); deliveries waiting for the broker's settlement (mode
/// second) count against it. Returns whether it sent a flow.
pub(super) fn top_up(
    writer: &mut BrokerWriter,
    l: &mut InLink,
    credit: u32,
    metrics: &BridgeMetrics,
) -> Result<bool, String> {
    let room = credit.saturating_sub(u32::try_from(l.awaiting.len()).unwrap_or(u32::MAX));
    if l.outstanding <= credit / 2 && room > l.outstanding {
        l.outstanding = room;
        writer
            .flow(l.channel, l.spec.handle, l.delivery_count, room)
            .map_err(|e| format!("flow: {e}"))?;
        if let Some(sm) = &metrics.spec {
            sm.credit_topup.inc();
        }
        return Ok(true);
    }
    Ok(false)
}

/// The broker's flow state as the sender of receiving link `l` (Part 2
/// §2.6.7, §2.7.4). Its delivery-count is the one the daemon counts from:
/// a count that moved without deliveries (a link the broker drained)
/// consumed credit, which is granted again once half is used (§4.4); the
/// limit the daemon granted stays. With `echo` the broker asks for the
/// link's state: the flow that grants credit carries it, else a flow with
/// the delivery-count and the credit still granted. The broker's `drain`
/// is its own mode as the sender (§2.7.4); the daemon never asks for one.
fn on_sender_flow(
    cfg: &DaemonConfig,
    writer: &mut BrokerWriter,
    l: &mut InLink,
    delivery_count: Option<u32>,
    echo: bool,
    metrics: &BridgeMetrics,
) -> Result<(), String> {
    if let Some(count) = delivery_count {
        let limit = l.delivery_count.wrapping_add(l.outstanding);
        let left = limit.wrapping_sub(count);
        l.delivery_count = count;
        l.outstanding = if left < 0x8000_0000 { left } else { 0 };
    }
    let granted = top_up(writer, l, cfg.topics[l.topic].credit, metrics)?;
    if echo && !granted {
        writer
            .flow(l.channel, l.spec.handle, l.delivery_count, l.outstanding)
            .map_err(|e| format!("flow: {e}"))?;
    }
    Ok(())
}

pub(super) fn in_link_down(cfg: &DaemonConfig, l: &mut InLink, reason: &str) {
    l.remote_handle = None;
    l.outstanding = 0;
    l.awaiting.clear();
    l.state = LinkState::Down;
    l.retry_at = Some(Instant::now() + l.retry_delay);
    log(
        "link_down",
        &[
            ("link_name", &l.spec.name),
            ("dds_topic", &cfg.topics[l.topic].dds_name),
            ("amqp_address", &l.spec.address),
            ("err", reason),
            ("retry_ms", &l.retry_delay.as_millis().to_string()),
        ],
    );
    l.retry_delay = next_delay(l.retry_delay, &cfg.reconnect);
}

/// A frame of a session or link (Part 2 §2.7): the broker's begin is
/// answered with the attach, its attach brings the link up (a receiving
/// link grants its credit), a flow gives a sending link credit and its
/// drain flag and moves the delivery-count of a receiving link (Part 2
/// §2.6.7), a flow with `echo` is answered with the state asked for (Part
/// 2 §2.7.4), a detach is answered and the session ended, an end takes the
/// link down until the back-off has run out, a close ends the connection.
pub(super) fn on_link_frame(
    cfg: &DaemonConfig,
    writer: &mut BrokerWriter,
    inbound: Inbound,
    outs: &mut [OutLink],
    ins: &mut [InLink],
    metrics: &BridgeMetrics,
) -> Result<(), String> {
    let initial_retry = Duration::from_millis(cfg.reconnect.initial_delay_ms);
    match inbound {
        Inbound::Heartbeat => {}
        Inbound::Begin {
            channel,
            incoming_window,
            ..
        } => {
            writer.session_begun(channel, incoming_window);
            if let Some(l) = outs.iter_mut().find(|l| l.channel == channel) {
                writer
                    .attach(channel, &l.spec)
                    .map_err(|e| format!("attach {}: {e}", l.spec.name))?;
                // The attach states the initial-delivery-count (Part 2
                // §2.7.3): the link counts from it again, without the
                // broker's drain flag of the last attach.
                l.delivery_count = INITIAL_DELIVERY_COUNT;
                l.drain = false;
                l.echo = false;
                l.state = LinkState::Attaching;
            } else if let Some(l) = ins.iter_mut().find(|l| l.channel == channel) {
                writer
                    .attach(channel, &l.spec)
                    .map_err(|e| format!("attach {}: {e}", l.spec.name))?;
                l.state = LinkState::Attaching;
            } else {
                return Err(format!(
                    "the broker began a session on channel {channel} nobody asked for"
                ));
            }
        }
        Inbound::Attach {
            channel,
            name,
            remote_handle,
            has_node,
            initial_delivery_count,
            ..
        } => {
            if let Some(l) = outs
                .iter_mut()
                .find(|l| l.channel == channel && l.spec.name == name)
            {
                // Without a node the detach with the reason follows under
                // this handle; no credit comes, nothing is sent.
                l.remote_handle = Some(remote_handle);
                if has_node {
                    l.state = LinkState::Up;
                    l.retry_delay = initial_retry;
                    log(
                        "attached",
                        &[
                            ("link_name", &name),
                            ("dds_topic", &l.dds_topic),
                            ("amqp_address", &l.spec.address),
                        ],
                    );
                } else {
                    log(
                        "attach_refused",
                        &[
                            ("link_name", &name),
                            ("dds_topic", &l.dds_topic),
                            ("amqp_address", &l.spec.address),
                        ],
                    );
                }
            } else if let Some(l) = ins
                .iter_mut()
                .find(|l| l.channel == channel && l.spec.name == name)
            {
                l.remote_handle = Some(remote_handle);
                if has_node {
                    let credit = cfg.topics[l.topic].credit;
                    l.state = LinkState::Up;
                    l.retry_delay = initial_retry;
                    l.delivery_count = initial_delivery_count.unwrap_or(0);
                    l.outstanding = credit;
                    writer
                        .flow(channel, l.spec.handle, l.delivery_count, credit)
                        .map_err(|e| format!("flow: {e}"))?;
                    if let Some(sm) = &metrics.spec {
                        sm.credit_topup.inc();
                    }
                    log(
                        "attached",
                        &[
                            ("link_name", &name),
                            ("dds_topic", &cfg.topics[l.topic].dds_name),
                            ("amqp_address", &l.spec.address),
                        ],
                    );
                } else {
                    log(
                        "attach_refused",
                        &[
                            ("link_name", &name),
                            ("dds_topic", &cfg.topics[l.topic].dds_name),
                            ("amqp_address", &l.spec.address),
                        ],
                    );
                }
            } else {
                return Err(format!("the broker attached an unknown link {name}"));
            }
        }
        Inbound::Flow {
            channel,
            next_incoming_id,
            incoming_window,
            remote_handle,
            delivery_count,
            link_credit,
            echo,
            drain,
        } => {
            writer.apply_session_flow(channel, next_incoming_id, incoming_window);
            match remote_handle {
                // Part 2 §2.7.4: "If set to true, and the handle field is
                // not set, then the sender only requires session endpoint
                // state to be echoed".
                None if echo => writer
                    .session_flow(channel)
                    .map_err(|e| format!("flow: {e}"))?,
                None => {}
                Some(h) => {
                    if let Some(l) = outs
                        .iter_mut()
                        .find(|l| l.channel == channel && l.remote_handle == Some(h))
                    {
                        // Part 2 §2.6.7: the broker is the receiver; its
                        // credit, drain flag and echo request are answered
                        // by `drain` below.
                        if let Some(credit) = link_credit {
                            l.credit = sender_credit(
                                delivery_count,
                                credit,
                                l.delivery_count,
                                INITIAL_DELIVERY_COUNT,
                            );
                        }
                        if l.state == LinkState::Up {
                            l.drain = drain;
                            l.echo |= echo;
                        }
                    } else if let Some(l) = ins.iter_mut().find(|l| {
                        l.channel == channel
                            && l.remote_handle == Some(h)
                            && l.state == LinkState::Up
                    }) {
                        on_sender_flow(cfg, writer, l, delivery_count, echo, metrics)?;
                    }
                }
            }
            drain_all(writer, outs, metrics)?;
        }
        Inbound::Transfer { .. } | Inbound::Disposition { .. } => {
            return Err(
                "a transfer or a disposition settles a delivery; it is not a link frame".into(),
            );
        }
        Inbound::Detach {
            channel,
            remote_handle,
            error,
            ..
        } => {
            // The broker gave up one link. Answer the detach (Part 2
            // §2.6.5) and end its session; the session is begun again once
            // the broker's end arrives, after the back-off.
            let reason = error.map_or_else(|| "no reason".to_string(), |e| e.to_string());
            // The link's handle and what the `link_detached` line names
            // (§8.1).
            let (handle, name, topic, address) =
                if let Some(l) = outs.iter_mut().find(|l| l.channel == channel) {
                    l.state = LinkState::Ending;
                    (
                        l.spec.handle,
                        l.spec.name.clone(),
                        l.dds_topic.clone(),
                        l.spec.address.clone(),
                    )
                } else if let Some(l) = ins.iter_mut().find(|l| l.channel == channel) {
                    l.state = LinkState::Ending;
                    (
                        l.spec.handle,
                        l.spec.name.clone(),
                        cfg.topics[l.topic].dds_name.clone(),
                        l.spec.address.clone(),
                    )
                } else {
                    log(
                        "detach_of_unknown_link",
                        &[("handle", &remote_handle.to_string()), ("err", &reason)],
                    );
                    return Ok(());
                };
            log(
                "link_detached",
                &[
                    ("link_name", &name),
                    ("dds_topic", &topic),
                    ("amqp_address", &address),
                    ("channel", &channel.to_string()),
                    ("err", &reason),
                ],
            );
            writer
                .detach(channel, handle, None)
                .map_err(|e| format!("detach: {e}"))?;
            writer.end(channel, None).map_err(|e| format!("end: {e}"))?;
        }
        Inbound::End { channel, error } => {
            let reason = error.map_or_else(|| "no reason".to_string(), |e| e.to_string());
            if let Some(l) = outs.iter_mut().find(|l| l.channel == channel) {
                if l.state != LinkState::Ending {
                    // The broker ended it: answer with ours.
                    writer.end(channel, None).map_err(|e| format!("end: {e}"))?;
                }
                out_link_down(cfg, l, &reason);
            } else if let Some(l) = ins.iter_mut().find(|l| l.channel == channel) {
                if l.state != LinkState::Ending {
                    writer.end(channel, None).map_err(|e| format!("end: {e}"))?;
                }
                in_link_down(cfg, l, &reason);
            } else {
                log(
                    "end_of_unknown_session",
                    &[("channel", &channel.to_string()), ("err", &reason)],
                );
            }
        }
        Inbound::Close(e) => {
            return Err(format!(
                "the broker closed the connection: {}",
                e.map_or_else(|| "no reason".into(), |e| e.to_string())
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::bridge_daemon::test_support::{
        in_link, load, metrics, out_link, pending, two_topics,
    };
    use crate::broker::AmqpError;
    use crate::broker::tests::bare_writer_and_written;
    use zerodds_amqp_bridge::extended_types::AmqpExtValue;
    use zerodds_amqp_bridge::performatives::descriptor;

    fn uint(v: &AmqpExtValue) -> u32 {
        match v {
            AmqpExtValue::Uint(n) => *n,
            AmqpExtValue::Ulong(n) => u32::try_from(*n).unwrap(),
            AmqpExtValue::Ubyte(n) => u32::from(*n),
            AmqpExtValue::Ushort(n) => u32::from(*n),
            other => panic!("not an unsigned integer: {other:?}"),
        }
    }

    fn queued(l: &OutLink) -> Vec<u8> {
        l.queue.iter().map(|p| p.message[0]).collect()
    }

    /// §4.2: every link has a session of its own; the links that are down
    /// are begun, each on its channel, the others are left alone.
    #[test]
    fn amqpbridge_12_1_link_state_begin_all_begins_every_link_that_is_down() {
        let cfg = two_topics();
        let (mut w, mut written) = bare_writer_and_written();
        let mut outs = vec![out_link(&cfg, 0, 0), out_link(&cfg, 0, 2)];
        let mut ins = vec![in_link(&cfg, 1, 1, false)];
        outs[0].state = LinkState::Down;
        outs[0].retry_at = Some(Instant::now());
        ins[0].state = LinkState::Down;
        begin_all(&mut w, &mut outs, &mut ins).unwrap();
        assert_eq!(
            (outs[0].state, outs[1].state, ins[0].state),
            (LinkState::Beginning, LinkState::Up, LinkState::Beginning)
        );
        assert_eq!(outs[0].retry_at, None);
        assert_eq!(written.next_on().0, 0);
        let (channel, code, _) = written.next_on();
        assert_eq!((channel, code), (1, descriptor::BEGIN));
        assert!(
            written.nothing_more(),
            "the link that is up has its session"
        );
    }

    /// Part 2 §2.7.2, §2.7.3: the broker's begin is answered with the attach
    /// of the session's link; a begin nobody asked for ends the connection.
    #[test]
    fn amqpbridge_12_1_link_state_the_brokers_begin_is_answered_with_the_attach() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let (mut w, mut written) = bare_writer_and_written();
        let mut outs = vec![out_link(&cfg, 0, 0)];
        let mut ins = vec![in_link(&cfg, 1, 1, false)];
        outs[0].state = LinkState::Beginning;
        w.begin(0).unwrap();
        written.next_on();
        let begin = Inbound::Begin {
            channel: 0,
            next_outgoing_id: 0,
            incoming_window: 50,
        };
        on_link_frame(&cfg, &mut w, begin, &mut outs, &mut ins, &m).unwrap();
        assert_eq!(outs[0].state, LinkState::Attaching);
        assert_eq!(w.remote_incoming_window(0), 50);
        let (channel, code, fields) = written.next_on();
        assert_eq!((channel, code), (0, descriptor::ATTACH));
        assert_eq!(fields[0], AmqpExtValue::Str("cid/Out/sender".into()));
        let stray = Inbound::Begin {
            channel: 9,
            next_outgoing_id: 0,
            incoming_window: 1,
        };
        let e = on_link_frame(&cfg, &mut w, stray, &mut outs, &mut ins, &m).unwrap_err();
        assert!(e.contains("nobody asked for"), "{e}");
    }

    /// Part 2 §2.6.3, §2.6.7: the broker's attach with a node brings the
    /// link up and resets its back-off; a receiving link grants its credit
    /// from the broker's initial delivery count.
    #[test]
    fn amqpbridge_12_1_link_state_an_attach_brings_the_link_up_and_a_receiver_grants_credit() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let (mut w, mut written) = bare_writer_and_written();
        let mut outs = vec![out_link(&cfg, 0, 0)];
        let mut ins = vec![in_link(&cfg, 1, 1, false)];
        outs[0].state = LinkState::Attaching;
        outs[0].retry_delay = Duration::from_millis(800);
        ins[0].state = LinkState::Attaching;
        ins[0].outstanding = 0;
        w.begin(1).unwrap();
        written.next_on();
        let attach = |name: &str, handle: u32, count: Option<u32>| Inbound::Attach {
            channel: if name.ends_with("sender") { 0 } else { 1 },
            name: name.into(),
            remote_handle: handle,
            has_node: true,
            initial_delivery_count: count,
            coordinator: None,
        };
        on_link_frame(
            &cfg,
            &mut w,
            attach("cid/Out/sender", 7, None),
            &mut outs,
            &mut ins,
            &m,
        )
        .unwrap();
        assert_eq!(outs[0].state, LinkState::Up);
        assert_eq!(outs[0].remote_handle, Some(7));
        assert_eq!(outs[0].retry_delay, Duration::from_millis(100));
        assert!(
            written.nothing_more(),
            "a sender waits for the broker's credit"
        );
        on_link_frame(
            &cfg,
            &mut w,
            attach("cid/In/receiver", 8, Some(5)),
            &mut outs,
            &mut ins,
            &m,
        )
        .unwrap();
        assert_eq!(ins[0].state, LinkState::Up);
        assert_eq!((ins[0].delivery_count, ins[0].outstanding), (5, 8));
        let (channel, code, fields) = written.next_on();
        assert_eq!((channel, code), (1, descriptor::FLOW));
        assert_eq!(
            (uint(&fields[4]), uint(&fields[5]), uint(&fields[6])),
            (1, 5, 8),
            "handle, delivery-count, link-credit"
        );
        assert_eq!(m.spec.as_ref().unwrap().credit_topup.get(), 1);
        let e = on_link_frame(
            &cfg,
            &mut w,
            attach("cid/X/sender", 9, None),
            &mut outs,
            &mut ins,
            &m,
        )
        .unwrap_err();
        assert!(e.contains("unknown link"), "{e}");
    }

    /// Part 2 §2.6.3: an attach without a node is the broker's refusal; the
    /// link stays down for messages, whatever credit follows.
    #[test]
    fn amqpbridge_12_1_link_state_an_attach_without_a_node_sends_nothing() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let (mut w, mut written) = bare_writer_and_written();
        let mut outs = vec![out_link(&cfg, 0, 0)];
        let mut ins = vec![];
        outs[0].state = LinkState::Attaching;
        outs[0].queue.push_back(pending(1));
        w.begin(0).unwrap();
        w.session_begun(0, 100);
        written.next_on();
        let refused = Inbound::Attach {
            channel: 0,
            name: "cid/Out/sender".into(),
            remote_handle: 3,
            has_node: false,
            initial_delivery_count: None,
            coordinator: None,
        };
        on_link_frame(&cfg, &mut w, refused, &mut outs, &mut ins, &m).unwrap();
        assert_eq!(outs[0].state, LinkState::Attaching);
        assert_eq!(outs[0].remote_handle, Some(3));
        let flow = Inbound::Flow {
            channel: 0,
            next_incoming_id: Some(0),
            incoming_window: 100,
            remote_handle: Some(3),
            delivery_count: Some(0),
            link_credit: Some(10),
            echo: false,
            drain: false,
        };
        on_link_frame(&cfg, &mut w, flow, &mut outs, &mut ins, &m).unwrap();
        assert!(written.nothing_more());
        assert_eq!(queued(&outs[0]), [1]);
    }

    /// Part 2 §2.6.7: link-credit is the receiver's delivery-count plus its
    /// link-credit minus the sender's delivery-count; a flow without a
    /// delivery-count (the broker has not seen the attach) counts from the
    /// initial-delivery-count of the attach, 0.
    #[test]
    fn amqpbridge_12_1_link_state_credit_follows_part_2_2_6_7() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let (mut w, mut written) = bare_writer_and_written();
        let mut outs = vec![out_link(&cfg, 0, 0)];
        let mut ins = vec![];
        outs[0].delivery_count = 5;
        for b in [1, 2, 3] {
            outs[0].queue.push_back(pending(b));
        }
        w.begin(0).unwrap();
        written.next_on();
        let flow = |count: Option<u32>, credit: u32| Inbound::Flow {
            channel: 0,
            next_incoming_id: Some(0),
            incoming_window: 100,
            remote_handle: Some(40),
            delivery_count: count,
            link_credit: Some(credit),
            echo: false,
            drain: false,
        };
        on_link_frame(&cfg, &mut w, flow(Some(3), 4), &mut outs, &mut ins, &m).unwrap();
        // 3 + 4 - 5 = 2 messages go out.
        for _ in 0..2 {
            assert_eq!(written.next_on().1, descriptor::TRANSFER);
        }
        assert!(written.nothing_more());
        assert_eq!((outs[0].credit, outs[0].delivery_count), (0, 7));
        assert_eq!(outs[0].unsettled.len(), 2, "sent unsettled");
        assert_eq!(queued(&outs[0]), [3]);
        // 0 + 8 - 7 = 1.
        on_link_frame(&cfg, &mut w, flow(None, 8), &mut outs, &mut ins, &m).unwrap();
        assert_eq!(written.next_on().1, descriptor::TRANSFER);
        assert!(written.nothing_more());
        assert_eq!((outs[0].credit, outs[0].delivery_count), (0, 8));
        assert!(outs[0].queue.is_empty());
    }

    /// Part 2 §2.6.5: the broker's detach is answered, and the link's
    /// session ended; a detach on a channel without a link changes nothing.
    #[test]
    fn amqpbridge_12_1_link_state_a_detach_is_answered_and_the_session_ended() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let (mut w, mut written) = bare_writer_and_written();
        let mut outs = vec![out_link(&cfg, 0, 0)];
        let mut ins = vec![];
        w.begin(0).unwrap();
        written.next_on();
        let detach = |channel: u16| Inbound::Detach {
            channel,
            remote_handle: 40,
            closed: true,
            error: Some(AmqpError {
                condition: "amqp:not-found".into(),
                description: None,
            }),
        };
        on_link_frame(&cfg, &mut w, detach(0), &mut outs, &mut ins, &m).unwrap();
        assert_eq!(outs[0].state, LinkState::Ending);
        let (channel, code, fields) = written.next_on();
        assert_eq!((channel, code), (0, descriptor::DETACH));
        assert_eq!(uint(&fields[0]), 0, "the link's own handle");
        assert_eq!(written.next_on().1, descriptor::END);
        on_link_frame(&cfg, &mut w, detach(5), &mut outs, &mut ins, &m).unwrap();
        assert!(written.nothing_more());
    }

    /// §9.3: an end takes the link down; the broker's own end is answered,
    /// the answer to ours is not; unsettled deliveries wait at the front of
    /// the queue; the session is begun again after the back-off, which grows.
    #[test]
    fn amqpbridge_12_1_link_state_an_end_takes_the_link_down_until_the_back_off() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let (mut w, mut written) = bare_writer_and_written();
        let mut outs = vec![out_link(&cfg, 0, 0)];
        let mut ins = vec![in_link(&cfg, 1, 1, false)];
        outs[0].unsettled.insert(3, pending(3));
        outs[0].unsettled.insert(4, pending(4));
        outs[0].queue.push_back(pending(9));
        outs[0].credit = 5;
        w.begin(0).unwrap();
        w.begin(1).unwrap();
        written.next_on();
        written.next_on();
        let before = Instant::now();
        let end = |channel: u16| Inbound::End {
            channel,
            error: None,
        };
        on_link_frame(&cfg, &mut w, end(0), &mut outs, &mut ins, &m).unwrap();
        assert_eq!(
            written.next_on().1,
            descriptor::END,
            "the broker's end is answered"
        );
        let l = &outs[0];
        assert_eq!(
            (l.state, l.credit, l.remote_handle),
            (LinkState::Down, 0, None)
        );
        assert_eq!(queued(l), [3, 4, 9]);
        assert!(
            l.retry_at
                .is_some_and(|t| t >= before + Duration::from_millis(100))
        );
        assert_eq!(l.retry_delay, Duration::from_millis(200));
        ins[0].state = LinkState::Ending;
        ins[0].awaiting.insert(2);
        on_link_frame(&cfg, &mut w, end(1), &mut outs, &mut ins, &m).unwrap();
        assert!(
            written.nothing_more(),
            "the answer to our end is not answered"
        );
        assert_eq!((ins[0].state, ins[0].outstanding), (LinkState::Down, 0));
        assert!(ins[0].awaiting.is_empty());
    }

    /// §9.3: a link whose back-off has run out is begun again; one still
    /// waiting is not.
    #[test]
    fn amqpbridge_12_1_link_state_retry_due_begins_again_after_the_back_off() {
        let cfg = two_topics();
        let (mut w, mut written) = bare_writer_and_written();
        let mut outs = vec![out_link(&cfg, 0, 0)];
        let mut ins = vec![in_link(&cfg, 1, 1, false)];
        let now = Instant::now();
        outs[0].state = LinkState::Down;
        outs[0].retry_at = Some(now);
        ins[0].state = LinkState::Down;
        ins[0].retry_at = Some(now + Duration::from_secs(3600));
        retry_due(&mut w, &mut outs, &mut ins, now).unwrap();
        assert_eq!(
            (outs[0].state, outs[0].retry_at),
            (LinkState::Beginning, None)
        );
        assert_eq!(ins[0].state, LinkState::Down);
        let (channel, code, _) = written.next_on();
        assert_eq!((channel, code), (0, descriptor::BEGIN));
        assert!(written.nothing_more());
    }

    /// §9.3: after a lost connection every link begins at once on the next
    /// one, unsettled deliveries first and in the order they were sent.
    #[test]
    fn amqpbridge_12_1_link_state_a_lost_connection_puts_unsettled_deliveries_first() {
        let cfg = two_topics();
        let mut outs = vec![out_link(&cfg, 0, 0)];
        let mut ins = vec![in_link(&cfg, 1, 1, true)];
        outs[0].unsettled.insert(5, pending(5));
        outs[0].unsettled.insert(2, pending(2));
        outs[0].queue.push_back(pending(7));
        outs[0].credit = 3;
        outs[0].retry_at = Some(Instant::now());
        ins[0].awaiting.insert(1);
        connection_lost(&mut outs, &mut ins);
        assert_eq!(queued(&outs[0]), [2, 5, 7]);
        assert!(outs[0].unsettled.is_empty());
        assert_eq!(
            (outs[0].state, outs[0].credit, outs[0].retry_at),
            (LinkState::Down, 0, None)
        );
        assert_eq!((ins[0].state, ins[0].outstanding), (LinkState::Down, 0));
        assert!(ins[0].awaiting.is_empty() && ins[0].remote_handle.is_none());
    }

    /// §4.4: a receiving link grants its credit again once half of it is
    /// used; deliveries waiting for the broker's settlement count against it.
    #[test]
    fn amqpbridge_12_1_link_state_credit_is_granted_again_at_half() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let (mut w, mut written) = bare_writer_and_written();
        let mut l = in_link(&cfg, 1, 1, true);
        w.begin(1).unwrap();
        written.next_on();
        l.outstanding = 5;
        assert!(!top_up(&mut w, &mut l, 8, &m).unwrap());
        assert!(written.nothing_more(), "more than half is still granted");
        l.outstanding = 4;
        assert!(top_up(&mut w, &mut l, 8, &m).unwrap());
        assert_eq!(uint(&written.next_on().2[6]), 8);
        assert_eq!(l.outstanding, 8);
        l.outstanding = 2;
        l.awaiting.extend([1, 2, 3]);
        assert!(top_up(&mut w, &mut l, 8, &m).unwrap());
        assert_eq!(uint(&written.next_on().2[6]), 5, "8 less the 3 awaiting");
        l.outstanding = 0;
        l.awaiting.extend(4..=8);
        assert!(!top_up(&mut w, &mut l, 8, &m).unwrap());
        assert!(
            written.nothing_more(),
            "every credit is held by an awaiting delivery"
        );
        assert_eq!(m.spec.as_ref().unwrap().credit_topup.get(), 2);
    }

    /// §6: a sending link's queue holds the topic's HISTORY depth, the
    /// KEEP_ALL bound for KEEP_ALL; a receiving link settles second when
    /// the topic asks for it. The links `run_with_reload` builds.
    #[test]
    fn amqpbridge_6_a_link_takes_its_queue_and_settle_mode_from_the_topic() {
        let cfg = crate::bridge_daemon::test_support::load(
            "topics:\n  - dds_name: A\n    amqp_address: /queues/a\n    direction: out\n    qos:\n      history:\n        kind: keep_last\n        depth: 7\n  - dds_name: B\n    amqp_address: /queues/b\n    qos:\n      history:\n        kind: keep_all\n    sender:\n      rcv_settle_mode: 1\n",
        )
        .unwrap();
        let acl = Arc::new(Live::new(&cfg, None, Vec::new()));
        let a = OutLink::for_topic(&cfg, 0, 0, &acl);
        assert_eq!(a.cap, 7);
        assert_eq!(a.state, LinkState::Down);
        assert_eq!((a.channel, a.spec.handle, a.topic), (0, 0, Some(0)));
        let b = OutLink::for_topic(&cfg, 1, 1, &acl);
        assert_eq!(b.cap, KEEP_ALL_QUEUE_CAP);
        let writer = EntityId::user_writer_no_key([0, 0, 2]);
        let r = InLink::for_topic(&cfg, 1, 2, writer);
        assert!(r.second);
        assert_eq!((r.channel, r.spec.handle, r.writer), (2, 2, writer));
        assert!(!r.spec.sender);
        assert!(!InLink::for_topic(&cfg, 0, 3, writer).second);
    }

    /// §4.2: a link is named `<container-id>/<dds-topic>/<role>`, its node
    /// is the topic's address, its settle modes and durability are those of
    /// the configuration, its properties name topic and type, and it
    /// desires `dds.cdr2`.
    #[test]
    fn amqpbridge_4_2_a_link_is_named_container_topic_role() {
        let c = load(
            "amqp:\n  container_id: cid\ntopics:\n  - dds_name: Chat::Message\n    dds_type: Chat::Msg\n    amqp_address: /queues/chat\n    sender:\n      durable: 2\n      snd_settle_mode: 2\n      rcv_settle_mode: 1\n",
        )
        .unwrap();
        let t = &c.topics[0];
        let s = link_spec(&c, t, 4, true);
        assert_eq!(s.name, "cid/Chat::Message/sender");
        assert_eq!((s.handle, s.sender), (4, true));
        assert_eq!(s.address, "/queues/chat");
        assert_eq!((s.snd_settle_mode, s.rcv_settle_mode, s.durable), (2, 1, 2));
        assert_eq!(
            s.properties,
            vec![
                ("zerodds_topic".to_string(), "Chat::Message".to_string()),
                ("zerodds_type".to_string(), "Chat::Msg".to_string())
            ]
        );
        assert_eq!(s.desired_capabilities, vec!["dds.cdr2".to_string()]);
        assert!(s.coordinator.is_none());
        let r = link_spec(&c, t, 5, false);
        assert_eq!(r.name, "cid/Chat::Message/receiver");
        assert!(!r.sender);
        assert_eq!(r.address, "/queues/chat");
        assert_eq!(r.durable, 2, "sender.durable is both links'");
    }

    /// §9.3: the delay starts at `initial_delay_ms`, grows by `factor` per
    /// failed attempt and stops at `max_delay_ms`; with factor 1.0 it stays.
    #[test]
    fn amqpbridge_9_3_the_delay_grows_by_the_factor_up_to_the_maximum() {
        let rc = Reconnect {
            initial_delay_ms: 500,
            max_delay_ms: 30_000,
            factor: 2.0,
        };
        let mut d = Duration::from_millis(rc.initial_delay_ms);
        let mut seen = vec![d.as_millis()];
        for _ in 0..8 {
            d = next_delay(d, &rc);
            seen.push(d.as_millis());
        }
        assert_eq!(
            seen,
            [500, 1000, 2000, 4000, 8000, 16_000, 30_000, 30_000, 30_000]
        );
        let flat = Reconnect { factor: 1.0, ..rc };
        assert_eq!(
            next_delay(Duration::from_millis(500), &flat).as_millis(),
            500
        );
        let odd = Reconnect {
            initial_delay_ms: 300,
            max_delay_ms: 1000,
            factor: 1.5,
        };
        assert_eq!(
            next_delay(Duration::from_millis(300), &odd).as_millis(),
            450
        );
        assert_eq!(
            next_delay(Duration::from_millis(900), &odd).as_millis(),
            1000
        );
    }

    /// A flow of the broker on session `channel`, for the link of
    /// `remote_handle` when one is given.
    fn broker_flow(
        channel: u16,
        remote_handle: Option<u32>,
        delivery_count: Option<u32>,
        link_credit: Option<u32>,
        drain: bool,
        echo: bool,
    ) -> Inbound {
        Inbound::Flow {
            channel,
            next_incoming_id: Some(0),
            incoming_window: 100,
            remote_handle,
            delivery_count,
            link_credit,
            echo,
            drain,
        }
    }

    /// Part 2 §2.6.7: a sending link asked to drain sends what is queued
    /// while the credit lasts; with nothing left it advances its
    /// delivery-count by the credit left, which consumes the credit, and
    /// sends its flow state: credit 0, `available` 0, `drain` set.
    #[test]
    fn amqp10_2_6_7_a_sending_link_asked_to_drain_consumes_the_credit_it_cannot_use() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let (mut w, mut written) = bare_writer_and_written();
        let mut outs = vec![out_link(&cfg, 0, 0)];
        let mut ins = vec![];
        outs[0].queue.push_back(pending(1));
        outs[0].queue.push_back(pending(2));
        w.begin(0).unwrap();
        written.next_on();
        let drain = broker_flow(0, Some(40), Some(0), Some(5), true, false);
        on_link_frame(&cfg, &mut w, drain, &mut outs, &mut ins, &m).unwrap();
        for _ in 0..2 {
            assert_eq!(written.next_on().1, descriptor::TRANSFER);
        }
        let (channel, code, f) = written
            .try_next_on()
            .expect("the flow state of the drained link");
        assert_eq!((channel, code), (0, descriptor::FLOW));
        assert_eq!(
            (uint(&f[4]), uint(&f[5]), uint(&f[6]), uint(&f[7])),
            (0, 5, 0, 0),
            "handle, delivery-count, link-credit, available"
        );
        assert_eq!(f[8], AmqpExtValue::Boolean(true), "drain");
        assert!(written.nothing_more());
        assert_eq!((outs[0].credit, outs[0].delivery_count), (0, 5));
        // Nothing queued: the whole credit is consumed at once.
        let drain = broker_flow(0, Some(40), Some(5), Some(3), true, false);
        on_link_frame(&cfg, &mut w, drain, &mut outs, &mut ins, &m).unwrap();
        let (_, code, f) = written.try_next_on().expect("the flow state");
        assert_eq!(code, descriptor::FLOW);
        assert_eq!((uint(&f[5]), uint(&f[6])), (8, 0));
        assert_eq!((outs[0].credit, outs[0].delivery_count), (0, 8));
    }

    /// §9.2, Part 2 §2.6.7: once the stop is seen, the broker's credit
    /// sends no queued message; a link asked to drain then has no message
    /// available and consumes its credit, `available` 0; the message stays
    /// queued for the count at the end of the drain.
    #[test]
    fn amqpbridge_9_2_a_link_asked_to_drain_after_the_stop_consumes_its_credit() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let (mut w, mut written) = bare_writer_and_written();
        let mut outs = vec![out_link(&cfg, 0, 0)];
        let mut ins = vec![];
        outs[0].stopping = true;
        outs[0].queue.push_back(pending(1));
        w.begin(0).unwrap();
        written.next_on();
        let credit = broker_flow(0, Some(40), Some(0), Some(3), false, false);
        on_link_frame(&cfg, &mut w, credit, &mut outs, &mut ins, &m).unwrap();
        assert!(
            written.nothing_more(),
            "a queued message sent after the stop"
        );
        assert_eq!(outs[0].credit, 3);
        let drain = broker_flow(0, Some(40), Some(0), Some(3), true, false);
        on_link_frame(&cfg, &mut w, drain, &mut outs, &mut ins, &m).unwrap();
        let (_, code, f) = written.try_next_on().expect("the drained flow");
        assert_eq!(code, descriptor::FLOW);
        assert_eq!(
            (uint(&f[5]), uint(&f[6]), uint(&f[7])),
            (3, 0, 0),
            "delivery-count, link-credit, available"
        );
        assert_eq!(f[8], AmqpExtValue::Boolean(true), "drain");
        assert!(written.nothing_more());
        assert_eq!(queued(&outs[0]), [1]);
    }

    /// Part 2 §2.7.4: a flow with `echo` for a sending link is answered
    /// with the link's flow state, `available` the messages queued for
    /// credit; a flow with `echo` and no handle with the session's state.
    #[test]
    fn amqp10_2_7_4_a_flow_with_echo_is_answered_with_the_state_of_the_sending_link() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let (mut w, mut written) = bare_writer_and_written();
        let mut outs = vec![out_link(&cfg, 0, 0)];
        let mut ins = vec![];
        for b in [1, 2, 3] {
            outs[0].queue.push_back(pending(b));
        }
        w.begin(0).unwrap();
        written.next_on();
        let echo = broker_flow(0, Some(40), Some(0), Some(1), false, true);
        on_link_frame(&cfg, &mut w, echo, &mut outs, &mut ins, &m).unwrap();
        assert_eq!(written.next_on().1, descriptor::TRANSFER);
        let (channel, code, f) = written.try_next_on().expect("the echoed flow");
        assert_eq!((channel, code), (0, descriptor::FLOW));
        assert_eq!(
            (uint(&f[4]), uint(&f[5]), uint(&f[6]), uint(&f[7])),
            (0, 1, 0, 2),
            "handle, delivery-count, link-credit, available"
        );
        assert_eq!(f[8], AmqpExtValue::Boolean(false), "drain");
        assert!(
            f.get(9).is_none_or(|e| *e != AmqpExtValue::Boolean(true)),
            "an answer does not ask for an echo itself"
        );
        assert!(written.nothing_more());
        let session = broker_flow(0, None, None, None, false, true);
        on_link_frame(&cfg, &mut w, session, &mut outs, &mut ins, &m).unwrap();
        let (channel, code, f) = written.try_next_on().expect("the session's flow");
        assert_eq!((channel, code), (0, descriptor::FLOW));
        assert_eq!(f.len(), 4, "session fields only: {f:?}");
        assert!(written.nothing_more());
    }

    /// Part 2 §2.6.7: a flow the broker sent before it saw the deliveries
    /// in flight grants no credit when its delivery-count plus its credit
    /// lies behind the link's delivery-count (RFC 1982 serial numbers).
    #[test]
    fn amqp10_2_6_7_a_flow_behind_the_deliveries_sent_grants_no_credit() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let (mut w, mut written) = bare_writer_and_written();
        let mut outs = vec![out_link(&cfg, 0, 0)];
        let mut ins = vec![];
        outs[0].delivery_count = 2;
        outs[0].queue.push_back(pending(1));
        w.begin(0).unwrap();
        written.next_on();
        let behind = broker_flow(0, Some(40), Some(0), Some(1), false, false);
        on_link_frame(&cfg, &mut w, behind, &mut outs, &mut ins, &m).unwrap();
        assert!(written.nothing_more(), "sent without credit");
        assert_eq!(outs[0].credit, 0);
        assert_eq!(queued(&outs[0]), [1]);
    }

    /// Part 2 §2.6.7, §2.7.3: the attach of a sending link states
    /// initial-delivery-count 0, so a link attached again counts from 0;
    /// the broker's credit then admits as many deliveries as it says.
    #[test]
    fn amqp10_2_6_7_a_link_attached_again_counts_from_its_initial_delivery_count() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let (mut w, mut written) = bare_writer_and_written();
        let mut outs = vec![out_link(&cfg, 0, 0)];
        let mut ins = vec![];
        outs[0].delivery_count = 150;
        outs[0].state = LinkState::Beginning;
        outs[0].remote_handle = None;
        outs[0].queue.push_back(pending(1));
        outs[0].queue.push_back(pending(2));
        w.begin(0).unwrap();
        written.next_on();
        let begin = Inbound::Begin {
            channel: 0,
            next_outgoing_id: 0,
            incoming_window: 100,
        };
        on_link_frame(&cfg, &mut w, begin, &mut outs, &mut ins, &m).unwrap();
        let (_, code, f) = written.next_on();
        assert_eq!(code, descriptor::ATTACH);
        assert_eq!(uint(&f[9]), 0, "initial-delivery-count");
        let attach = Inbound::Attach {
            channel: 0,
            name: "cid/Out/sender".into(),
            remote_handle: 40,
            has_node: true,
            initial_delivery_count: None,
            coordinator: None,
        };
        on_link_frame(&cfg, &mut w, attach, &mut outs, &mut ins, &m).unwrap();
        let credit = broker_flow(0, Some(40), Some(0), Some(1), false, false);
        on_link_frame(&cfg, &mut w, credit, &mut outs, &mut ins, &m).unwrap();
        assert_eq!(written.next_on().1, descriptor::TRANSFER);
        assert!(written.nothing_more(), "one credit, one delivery");
        assert_eq!((outs[0].credit, outs[0].delivery_count), (0, 1));
        assert_eq!(queued(&outs[0]), [2]);
    }

    /// Part 2 §2.7.4: a flow with `echo` for a receiving link is answered
    /// with its flow state: the delivery-count it counted and the credit
    /// still granted.
    #[test]
    fn amqp10_2_7_4_a_flow_with_echo_is_answered_with_the_state_of_the_receiving_link() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let (mut w, mut written) = bare_writer_and_written();
        let mut outs = vec![];
        let mut ins = vec![in_link(&cfg, 1, 1, false)];
        ins[0].delivery_count = 3;
        ins[0].outstanding = 5;
        w.begin(1).unwrap();
        written.next_on();
        let echo = broker_flow(1, Some(41), Some(3), Some(5), false, true);
        on_link_frame(&cfg, &mut w, echo, &mut outs, &mut ins, &m).unwrap();
        let (channel, code, f) = written.try_next_on().expect("the echoed flow");
        assert_eq!((channel, code), (1, descriptor::FLOW));
        assert_eq!(
            (uint(&f[4]), uint(&f[5]), uint(&f[6])),
            (1, 3, 5),
            "handle, delivery-count, link-credit"
        );
        assert!(written.nothing_more());
        assert_eq!(
            m.spec.as_ref().unwrap().credit_topup.get(),
            0,
            "an echo grants no credit"
        );
    }

    /// Part 2 §2.6.7: the broker's flow as the sender gives its
    /// delivery-count; a count that moved without deliveries (a drained
    /// link) consumed credit, and the receiving link grants it again once
    /// half is used (§4.4).
    #[test]
    fn amqp10_2_6_7_a_receiving_link_takes_the_delivery_count_of_the_brokers_flow() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let (mut w, mut written) = bare_writer_and_written();
        let mut outs = vec![];
        let mut ins = vec![in_link(&cfg, 1, 1, false)];
        w.begin(1).unwrap();
        written.next_on();
        let moved = broker_flow(1, Some(41), Some(3), Some(5), false, false);
        on_link_frame(&cfg, &mut w, moved, &mut outs, &mut ins, &m).unwrap();
        assert!(written.nothing_more(), "more than half is still granted");
        assert_eq!((ins[0].delivery_count, ins[0].outstanding), (3, 5));
        let drained = broker_flow(1, Some(41), Some(8), Some(0), true, false);
        on_link_frame(&cfg, &mut w, drained, &mut outs, &mut ins, &m).unwrap();
        let (channel, code, f) = written.try_next_on().expect("the credit granted again");
        assert_eq!((channel, code), (1, descriptor::FLOW));
        assert_eq!(
            (uint(&f[4]), uint(&f[5]), uint(&f[6])),
            (1, 8, 8),
            "handle, delivery-count, link-credit"
        );
        assert_eq!((ins[0].delivery_count, ins[0].outstanding), (8, 8));
    }
}
