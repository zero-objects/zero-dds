// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The pump (spec §4.3, §9.2): the loop that takes DDS samples and broker
//! frames in order, turns samples into messages, queues them per link and
//! sends them while the broker grants credit.

use std::string::{String, ToString};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use std::vec::Vec;

use zerodds_dcps::runtime::{DcpsRuntime, UserSample};

use super::acl::Live;
use super::cbs::Cbs;
use super::config::{DaemonConfig, TopicMapping};
use super::disposition::{on_disposition, on_transfer};
use super::link_state::{InLink, LinkState, OutLink, Pending, on_link_frame, retry_due};
use super::log::{Field, emit, enabled, log};
use super::message::{OutSample, annotated, encode_message};
use super::trace::{self, unix_ns};
use crate::broker::{self, BrokerWriter, Inbound};
use crate::coexistence::CoexistenceConfig;
use crate::daemon_runtime::BridgeMetrics;
use crate::properties::DdsOperation;
use zerodds_amqp_bridge::extended_types::AmqpExtValue;

/// What the pump takes, in order.
pub(super) enum Event {
    /// A DDS sample of the topic with this index, boxed: a `UserSample` is
    /// several times the size of the other variants.
    Dds(usize, Box<UserSample>),
    /// A frame of connection `generation`, read at this time (nanoseconds
    /// since the Unix epoch, §8.1 `latency_us`, §8.3).
    Amqp(u64, Inbound, u64),
    /// Connection `generation` is gone.
    Lost(u64, String),
}

/// Runs connection `generation` until it is lost (`Err` with the reason)
/// or, after a shutdown, until what is in flight is settled or
/// `cfg.drain_budget` has passed (§9.2). What the stop leaves behind is
/// counted in `errors_total` and logged: deliveries the broker did not
/// settle (`shutdown_with_unsettled`), messages still queued for credit and
/// samples the DDS reader delivered after the stop (`shutdown_with_queued`).
#[allow(clippy::too_many_arguments)]
pub(super) fn pump(
    cfg: &DaemonConfig,
    live: &Live,
    runtime: &Arc<DcpsRuntime>,
    writer: &mut BrokerWriter,
    rx: &mpsc::Receiver<Event>,
    generation: u64,
    remote: &broker::RemoteOpen,
    outs: &mut [OutLink],
    ins: &mut [InLink],
    coexistence: &CoexistenceConfig,
    own_prefix: &[u8; 12],
    metrics: &BridgeMetrics,
    shutdown: &AtomicBool,
    mut cbs: Option<&mut Cbs>,
) -> Result<(), String> {
    // Part 2 §2.4.5: a frame at least every half of the broker's idle
    // time-out; without one, wake up only to look at the shutdown flag and
    // the links waiting to be attached again.
    let beat = remote
        .idle_time_out_ms
        .map(|t| Duration::from_millis(u64::from(t / 2).max(1)));
    let wake = beat
        .unwrap_or(Duration::from_millis(250))
        .min(Duration::from_millis(250));
    let mut next_beat = beat.map(|b| Instant::now() + b);
    let mut drain_until: Option<Instant> = None;
    // Samples the DDS reader delivered after the stop: not sent (§9.2).
    let mut not_taken = 0usize;
    let mut next_deadline_look = Instant::now();
    loop {
        if let Some(sm) = &metrics.spec {
            let up = |s: &LinkState| *s == LinkState::Up;
            sm.links_sender.set(
                i64::try_from(outs.iter().filter(|l| up(&l.state)).count()).unwrap_or(i64::MAX),
            );
            sm.links_receiver.set(
                i64::try_from(ins.iter().filter(|l| up(&l.state)).count()).unwrap_or(i64::MAX),
            );
        }
        if shutdown.load(Ordering::SeqCst) {
            // §9.2: stop taking DDS samples, wait for what is in flight.
            let in_flight: usize = outs.iter().map(|l| l.unsettled.len()).sum();
            // The catalog is sent again on the next connection; it is not
            // a sample.
            let queued: usize = outs
                .iter()
                .filter(|l| l.topic.is_some())
                .map(|l| l.queue.len())
                .sum();
            let until = match drain_until {
                Some(until) => until,
                None => {
                    // The stop is seen: from here on no DDS sample is taken,
                    // and no message queued for credit is sent, whatever
                    // credit the broker grants (`drain`).
                    for l in outs.iter_mut() {
                        l.stopping = true;
                    }
                    emit(
                        "drain_started",
                        &[
                            ("in_flight", Field::Number(in_flight as u64)),
                            ("queued", Field::Number(queued as u64)),
                        ],
                    );
                    *drain_until.insert(Instant::now() + cfg.drain_budget)
                }
            };
            if in_flight == 0 || Instant::now() >= until {
                if in_flight > 0 {
                    metrics.errors_total.inc();
                    log(
                        "shutdown_with_unsettled",
                        &[("count", &in_flight.to_string())],
                    );
                }
                if queued + not_taken > 0 {
                    metrics.errors_total.inc();
                    emit(
                        "shutdown_with_queued",
                        &[
                            ("count", Field::Number(queued as u64)),
                            ("not_taken", Field::Number(not_taken as u64)),
                        ],
                    );
                }
                return Ok(());
            }
        }
        let now = Instant::now();
        if let (Some(at), Some(b)) = (next_beat, beat)
            && now >= at
        {
            writer.heartbeat().map_err(|e| format!("heartbeat: {e}"))?;
            next_beat = Some(now + b);
        }
        retry_due(writer, outs, ins, now)?;
        if let Some(c) = cbs.as_deref_mut() {
            c.renew_if_due(writer, now)?;
        }
        if now >= next_deadline_look {
            look_at_deadlines(runtime, outs, cfg, metrics);
            next_deadline_look = now + DEADLINE_LOOK;
        }
        // Wake up no later than the next heartbeat is due, so the gap
        // between two frames stays at half the broker's idle time-out.
        let until_beat = next_beat.map_or(wake, |at| at.saturating_duration_since(Instant::now()));
        let ev = match rx.recv_timeout(wake.min(until_beat)) {
            Ok(ev) => ev,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("event channel closed".into());
            }
        };
        match ev {
            Event::Lost(g, reason) if g == generation => return Err(reason),
            Event::Lost(..) => {}
            Event::Amqp(g, ..) if g != generation => {}
            Event::Amqp(_, inbound, _) if cbs.as_deref().is_some_and(|c| c.owns(&inbound)) => {
                metrics.frames_in_total.inc();
                if let Some(c) = cbs.as_deref_mut()
                    && c.on_inbound(writer, inbound)?
                {
                    // Every token accepted: the links now (§7.2).
                    super::link_state::begin_all(writer, outs, ins)?;
                }
            }
            Event::Amqp(_, inbound, read_ns) => {
                on_inbound(
                    cfg,
                    live,
                    runtime,
                    writer,
                    inbound,
                    read_ns,
                    outs,
                    ins,
                    coexistence,
                    metrics,
                )?;
            }
            Event::Dds(..) if drain_until.is_some() => not_taken += 1,
            Event::Dds(topic, sample) => {
                let taken_ns = unix_ns();
                let Some(l) = outs.iter_mut().find(|l| l.topic == Some(topic)) else {
                    continue;
                };
                let t = &cfg.topics[topic];
                // A right withdrawn by a reload: the sample stays in DDS.
                if !live.allows(&t.dds_name, false) {
                    metrics.errors_total.inc();
                    log("acl_denied", &[("dds_topic", &t.dds_name), ("op", "read")]);
                    continue;
                }
                look_at_deadline(runtime, l, t, metrics);
                let writer_guid = match &*sample {
                    UserSample::Alive { writer_guid, .. }
                    | UserSample::Lifecycle { writer_guid, .. } => Some(*writer_guid),
                    UserSample::CoherentSetEnd { .. } => None,
                };
                let partitions = writer_guid
                    .map(|g| writer_partitions(runtime, l, g))
                    .unwrap_or_default();
                let Some(messages) =
                    sample_messages(t, cfg.domain, coexistence, own_prefix, &sample, &partitions)
                else {
                    continue;
                };
                note_unknown_key_hash(l, t, &sample, metrics);
                // A dispose coming back from AMQP names this instance by its
                // key hash alone; the topic's AMQP → DDS writer records the
                // key now.
                if t.keyed
                    && let UserSample::Alive {
                        key_hash,
                        payload,
                        representation,
                        big_endian,
                        ..
                    } = &*sample
                    && let Some(back) = ins.iter().find(|l| l.topic == topic)
                {
                    let _ = runtime.remember_user_writer_instance(
                        back.writer,
                        *key_hash,
                        payload.as_ref(),
                        *representation,
                        *big_endian,
                    );
                }
                for out in messages {
                    // §6: a missed deadline not yet told rides on the next
                    // message of the topic.
                    let out = match (out, l.deadline_note.take()) {
                        (Ok(m), Some(total)) => annotated(
                            &m,
                            &[(
                                AmqpExtValue::Symbol(DEADLINE_MISSED_ANNOTATION.into()),
                                AmqpExtValue::Ulong(total),
                            )],
                        ),
                        (other, note) => {
                            l.deadline_note = note;
                            other
                        }
                    };
                    match out {
                        Ok(message) => {
                            metrics.dds_samples_in_total.inc();
                            if let Some(tm) = topic_metrics(metrics, topic) {
                                tm.dds_received.inc();
                            }
                            enqueue(l, message, taken_ns, &t.dds_name, metrics);
                            drain(writer, l, metrics)?;
                        }
                        Err(e) => {
                            metrics.errors_total.inc();
                            log("encode_error", &[("dds_topic", &t.dds_name), ("err", &e)]);
                        }
                    }
                }
            }
        }
    }
}

/// The message annotation that tells the REQUESTED_DEADLINE_MISSED total of
/// the topic's DDS reader (§6). Message annotation keys that do not begin
/// with `x-` are reserved, and a receiver may detach on an unknown one that
/// does not begin with `x-opt-` (AMQP 1.0 Part 3 §3.2.10).
pub(super) const DEADLINE_MISSED_ANNOTATION: &str = "x-opt-zerodds-deadline-missed";

/// How often the pump looks at the readers' missed deadlines.
const DEADLINE_LOOK: Duration = Duration::from_millis(50);

/// §6: a rise of a DDS reader's REQUESTED_DEADLINE_MISSED total is logged
/// (`deadline_missed`), counted in `errors_total`, and noted for the next
/// message of the topic.
fn look_at_deadlines(
    runtime: &DcpsRuntime,
    outs: &mut [OutLink],
    cfg: &DaemonConfig,
    metrics: &BridgeMetrics,
) {
    for l in outs.iter_mut() {
        if let Some(topic) = l.topic {
            look_at_deadline(runtime, l, &cfg.topics[topic], metrics);
        }
    }
}

/// [`look_at_deadlines`] for link `l` of topic `t`. The pump looks once
/// more when a sample of the topic arrives, so the message of the sample
/// after a gap carries every miss the gap caused.
fn look_at_deadline(
    runtime: &DcpsRuntime,
    l: &mut OutLink,
    t: &TopicMapping,
    metrics: &BridgeMetrics,
) {
    let Some(eid) = l.reader else {
        return;
    };
    if t.deadline_ms.is_none() {
        return;
    }
    let total = runtime.user_reader_requested_deadline_missed(eid);
    if total > l.deadline_missed {
        l.deadline_missed = total;
        l.deadline_note = Some(total);
        metrics.errors_total.inc();
        log(
            "deadline_missed",
            &[("dds_topic", &t.dds_name), ("total", &total.to_string())],
        );
    }
}

/// A sample, dispose or unregister of a keyed topic whose writer sent no
/// KeyHash and whose reader could not compute one goes out with the nil
/// instance handle (`dds:instance-handle` of zeros, no `group-id`, see
/// [`sample_messages`]): without `type_file` the reader has no type to
/// compute it from (§3). Logged (`key_hash_unavailable`) and counted in
/// `errors_total` once per writer.
pub(super) fn note_unknown_key_hash(
    l: &mut OutLink,
    t: &TopicMapping,
    sample: &UserSample,
    metrics: &BridgeMetrics,
) {
    let unknown_of = match sample {
        UserSample::Alive {
            key_hash: None,
            writer_guid,
            ..
        }
        | UserSample::Lifecycle {
            key_hash_unavailable: true,
            writer_guid,
            ..
        } => Some(writer_guid),
        _ => None,
    };
    if t.keyed
        && let Some(writer_guid) = unknown_of
        && l.key_hash_unknown.insert(*writer_guid)
    {
        metrics.errors_total.inc();
        log(
            "key_hash_unavailable",
            &[
                ("dds_topic", &t.dds_name),
                ("writer", &crate::properties::hex_lower(writer_guid)),
                (
                    "type_file",
                    if t.type_file.is_empty() {
                        "none"
                    } else {
                        &t.type_file
                    },
                ),
            ],
        );
    }
}

/// The PARTITION of the writer `writer_guid` matched with the reader of
/// link `l`, from its DCPSPublication data, kept per writer; empty while it
/// is not known.
fn writer_partitions(runtime: &DcpsRuntime, l: &mut OutLink, writer_guid: [u8; 16]) -> Vec<String> {
    if let Some(p) = l.writer_partitions.get(&writer_guid) {
        return p.clone();
    }
    let Some(eid) = l.reader else {
        return Vec::new();
    };
    let handle = zerodds_dcps::instance_handle::InstanceHandle::from_guid(
        zerodds_rtps::wire_types::Guid::from_bytes(writer_guid),
    );
    match runtime.user_reader_matched_publication_data(eid, handle) {
        Some(data) => {
            l.writer_partitions
                .insert(writer_guid, data.partition.clone());
            data.partition
        }
        None => Vec::new(),
    }
}

/// A frame of the broker: a transfer or a disposition settles a delivery
/// (`disposition`), every other frame is one of a session or link
/// (`link_state`).
#[allow(clippy::too_many_arguments)]
fn on_inbound(
    cfg: &DaemonConfig,
    live: &Live,
    runtime: &Arc<DcpsRuntime>,
    writer: &mut BrokerWriter,
    inbound: Inbound,
    read_ns: u64,
    outs: &mut [OutLink],
    ins: &mut [InLink],
    coexistence: &CoexistenceConfig,
    metrics: &BridgeMetrics,
) -> Result<(), String> {
    metrics.frames_in_total.inc();
    match inbound {
        Inbound::Transfer {
            channel,
            remote_handle,
            delivery_id,
            settled,
            message,
            ..
        } => {
            let Some(l) = ins
                .iter_mut()
                .find(|l| l.channel == channel && l.remote_handle == Some(remote_handle))
            else {
                return Err(format!(
                    "a transfer on unknown link handle {remote_handle} of session {channel}"
                ));
            };
            on_transfer(
                cfg,
                live,
                runtime,
                writer,
                l,
                delivery_id,
                settled,
                &message,
                read_ns,
                coexistence,
                metrics,
            )
        }
        Inbound::Disposition {
            channel,
            role_receiver,
            first,
            last,
            settled,
            outcome,
            annotations,
            ..
        } => on_disposition(
            cfg,
            writer,
            outs,
            ins,
            channel,
            role_receiver,
            (first, last),
            settled,
            &outcome,
            &annotations,
            metrics,
        ),
        other => on_link_frame(cfg, writer, other, outs, ins, metrics),
    }
}

/// The messages a DDS sample of topic `t` becomes (§4.3): one for a sample,
/// one per operation for a lifecycle marker ([`lifecycle_operations`]).
/// `None` for a sample this bridge wrote itself from AMQP (its writer GUID
/// has the participant's prefix `own_prefix`), which comes back on its own
/// reader, and for the end of a coherent set, which has no message.
/// `partitions` are the PARTITION of the sample's writer (`dds:partition`).
pub(super) fn sample_messages(
    t: &TopicMapping,
    domain: i32,
    coexistence: &CoexistenceConfig,
    own_prefix: &[u8; 12],
    sample: &UserSample,
    partitions: &[String],
) -> Option<Vec<Result<Vec<u8>, String>>> {
    match sample {
        UserSample::Alive {
            key_hash,
            payload,
            writer_guid,
            representation_identifier,
            source_timestamp,
            source_sequence_number,
            lifespan_nanos,
            ..
        } => {
            if writer_guid[..12] == own_prefix[..] {
                return None;
            }
            Some(vec![encode_message(
                t,
                domain,
                coexistence,
                &OutSample {
                    payload: payload.as_ref(),
                    representation_identifier: *representation_identifier,
                    key_hash: if t.keyed { *key_hash } else { None },
                    writer_guid: *writer_guid,
                    sequence: *source_sequence_number,
                    source_timestamp_ns: source_timestamp.map(he_timestamp_ns),
                    operation: DdsOperation::Write,
                    lifespan_ms: (*lifespan_nanos > 0)
                        .then(|| (*lifespan_nanos / 1_000_000).max(1)),
                    partitions,
                },
            )])
        }
        UserSample::Lifecycle {
            key_hash,
            key_hash_unavailable,
            kind,
            writer_guid,
            source_sequence_number,
            source_timestamp,
            ..
        } => {
            if writer_guid[..12] == own_prefix[..] {
                return None;
            }
            Some(
                lifecycle_operations(*kind)
                    .iter()
                    .map(|op| {
                        encode_message(
                            t,
                            domain,
                            coexistence,
                            &OutSample {
                                payload: &[],
                                // CDR2_LE: a marker carries no body.
                                representation_identifier: [0x00, 0x07],
                                // A hash the reader could not take from
                                // the wire or compute names no instance:
                                // the nil handle, as for such a sample.
                                key_hash: (t.keyed && !*key_hash_unavailable).then_some(*key_hash),
                                writer_guid: *writer_guid,
                                sequence: *source_sequence_number,
                                source_timestamp_ns: source_timestamp.map(he_timestamp_ns),
                                operation: *op,
                                lifespan_ms: None,
                                partitions,
                            },
                        )
                    })
                    .collect(),
            )
        }
        UserSample::CoherentSetEnd { .. } => None,
    }
}

/// Queues `message`, of a sample taken at `taken_ns`, on sending link `l`
/// until the broker grants credit. The queue holds at most the topic's
/// HISTORY depth (§6); a full queue drops its oldest message.
pub(super) fn enqueue(
    l: &mut OutLink,
    message: Vec<u8>,
    taken_ns: u64,
    topic: &str,
    metrics: &BridgeMetrics,
) {
    if l.queue.len() >= l.cap {
        l.queue.pop_front();
        metrics.errors_total.inc();
        log("queue_full_dropped_oldest", &[("dds_topic", topic)]);
    }
    l.queue.push_back(Pending {
        message,
        attempts: 0,
        taken_ns,
    });
}

/// An RTPS `Time_t` (seconds and 2^-32 fractions) in nanoseconds since the
/// epoch, rounded to the nearest nanosecond.
fn he_timestamp_ns(ts: zerodds_rtps::header_extension::HeTimestamp) -> u64 {
    let time = zerodds_dcps::time::he_timestamp_to_time(ts);
    u64::try_from(time.sec).unwrap_or(0) * 1_000_000_000 + u64::from(time.nanosec)
}

/// The DDS-AMQP operations a lifecycle marker becomes, in order. DDSI-RTPS
/// 2.5 §9.6.4.9 sets the D and U bits of a status info together for a
/// `dispose` and `unregister_instance` of one instance; DDS-AMQP 1.0 has one
/// operation per message (`dds:operation`), so that marker is two messages,
/// the dispose first (DDS 1.4 §2.2.2.4.2.13 `dispose`, §2.2.2.4.2.7
/// `unregister_instance`).
pub(crate) fn lifecycle_operations(
    kind: zerodds_rtps::history_cache::ChangeKind,
) -> &'static [DdsOperation] {
    use zerodds_rtps::history_cache::ChangeKind;
    match kind {
        ChangeKind::NotAliveDisposed => &[DdsOperation::Dispose],
        ChangeKind::NotAliveUnregistered => &[DdsOperation::Unregister],
        ChangeKind::NotAliveDisposedUnregistered => {
            &[DdsOperation::Dispose, DdsOperation::Unregister]
        }
        _ => &[],
    }
}

/// The families of topic `index` (spec §8.2), when registered.
pub(super) fn topic_metrics(
    metrics: &BridgeMetrics,
    index: usize,
) -> Option<&crate::daemon_runtime::TopicMetrics> {
    metrics.spec.as_ref().and_then(|s| s.topics.get(index))
}

/// Sends queued messages while the link has credit and the session room,
/// then the flow state the broker asked for ([`answer_flow`]). A topic
/// whose read right a reload withdrew sends nothing more (§7.3): the
/// messages queued for credit, and those the broker released or modified,
/// are dropped with the `acl_denied` line. Once the stop of §9.2 is seen
/// no message of the queue goes out, whatever credit the broker grants:
/// it stays queued and is counted at the end of the drain.
pub(super) fn drain(
    writer: &mut BrokerWriter,
    l: &mut OutLink,
    metrics: &BridgeMetrics,
) -> Result<(), String> {
    if l.topic.is_some() && !l.queue.is_empty() && !l.acl.allows(&l.dds_topic, false) {
        let dropped = l.queue.len() as u64;
        l.queue.clear();
        metrics.errors_total.add(dropped);
        emit(
            "acl_denied",
            &[
                ("dds_topic", Field::Text(&l.dds_topic)),
                ("op", Field::Text("read")),
                ("dropped", Field::Number(dropped)),
            ],
        );
    }
    while !l.stopping && l.credit > 0 && l.state == LinkState::Up {
        let Some(p) = l.queue.pop_front() else {
            break;
        };
        l.next_tag += 1;
        let mut tag = [0u8; 16];
        tag[..8].copy_from_slice(&l.next_tag.to_be_bytes());
        match writer.transfer(l.channel, l.spec.handle, &tag, &p.message, l.settled) {
            Ok(delivery_id) => {
                l.credit -= 1;
                l.delivery_count = l.delivery_count.wrapping_add(1);
                if l.topic.is_some() {
                    metrics.dds_samples_out_total.inc();
                }
                if let Some(tm) = l.topic.and_then(|i| topic_metrics(metrics, i)) {
                    tm.transfers_out.inc();
                    tm.bytes_out.add(p.message.len() as u64);
                }
                metrics.frames_out_total.inc();
                metrics.bytes_out_total.add(p.message.len() as u64);
                sent(l, delivery_id, &p);
                if !l.settled {
                    l.unsettled.insert(delivery_id, p);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                l.queue.push_front(p);
                break;
            }
            Err(e) => {
                l.queue.push_front(p);
                return Err(format!("transfer: {e}"));
            }
        }
    }
    answer_flow(writer, l)
}

/// The flow state the broker asked of sending link `l` (AMQP 1.0 Part 2
/// §2.6.7, §2.7.4). With `drain` and no message available, the link
/// advances its delivery-count by the credit left, which consumes the
/// credit ("If the sender's drain flag is set and there are no available
/// messages, the sender MUST advance its delivery-count until link-credit
/// is zero, and send its updated flow state to the receiver"). No message
/// is available when the queue is empty, or once the stop of §9.2 is seen,
/// after which nothing is sent; a message that waits for the session window
/// is available, and the link drains once it has gone out. With `echo`
/// the state is sent as it is; one flow answers both. `available` is the
/// number of messages queued for credit, 0 after the stop.
fn answer_flow(writer: &mut BrokerWriter, l: &mut OutLink) -> Result<(), String> {
    if l.state != LinkState::Up {
        return Ok(());
    }
    let available = if l.stopping {
        0
    } else {
        u32::try_from(l.queue.len()).unwrap_or(u32::MAX)
    };
    let drained = l.drain && l.credit > 0 && available == 0;
    if drained {
        l.delivery_count = l.delivery_count.wrapping_add(l.credit);
        l.credit = 0;
    }
    if drained || l.echo {
        l.echo = false;
        writer
            .sender_flow(
                l.channel,
                l.spec.handle,
                l.delivery_count,
                l.credit,
                available,
                l.drain,
            )
            .map_err(|e| format!("flow: {e}"))?;
    }
    Ok(())
}

/// A message of a DDS sample went out as delivery `delivery_id` on link
/// `l`: the `transfer_out` line (§8.1); a link that sends settled has no
/// disposition to wait for, so the round trip ends here (`disposition`
/// line with the outcome `settled`, §8.3 span).
fn sent(l: &OutLink, delivery_id: u32, p: &Pending) {
    if l.topic.is_none() {
        return;
    }
    if enabled("transfer_out") {
        emit(
            "transfer_out",
            &[
                ("link_name", Field::Text(&l.spec.name)),
                ("dds_topic", Field::Text(&l.dds_topic)),
                ("amqp_address", Field::Text(&l.spec.address)),
                ("delivery_id", Field::Number(u64::from(delivery_id))),
                ("bytes", Field::Number(p.message.len() as u64)),
            ],
        );
    }
    if l.settled {
        round_trip_out(l, delivery_id, p, "settled", None);
    }
}

/// The end of the round trip of delivery `delivery_id` of link `l`, the
/// message `p` (§8.1 `disposition` line with `latency_us` from the sample
/// taken, §8.3 spans).
pub(super) fn round_trip_out(
    l: &OutLink,
    delivery_id: u32,
    p: &Pending,
    outcome: &str,
    error: Option<String>,
) {
    if p.taken_ns == 0 || l.topic.is_none() {
        return;
    }
    let now = unix_ns();
    if enabled("disposition") {
        emit(
            "disposition",
            &[
                ("link_name", Field::Text(&l.spec.name)),
                ("dds_topic", Field::Text(&l.dds_topic)),
                ("amqp_address", Field::Text(&l.spec.address)),
                ("delivery_id", Field::Number(u64::from(delivery_id))),
                ("outcome", Field::Text(outcome)),
                (
                    "latency_us",
                    Field::Number(now.saturating_sub(p.taken_ns) / 1000),
                ),
            ],
        );
    }
    if trace::tracing() {
        trace::record(&trace::Transfer {
            direction: trace::Direction::Out,
            start_ns: p.taken_ns,
            end_ns: now,
            dds: Some((p.taken_ns, p.taken_ns)),
            dds_topic: &l.dds_topic,
            amqp_address: &l.spec.address,
            link_name: &l.spec.name,
            delivery_id,
            outcome,
            error,
        });
    }
}

pub(super) fn drain_all(
    writer: &mut BrokerWriter,
    outs: &mut [OutLink],
    metrics: &BridgeMetrics,
) -> Result<(), String> {
    for l in outs.iter_mut() {
        drain(writer, l, metrics)?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::bridge_daemon::decode_message;
    use crate::bridge_daemon::test_support::{metrics, out_link, pending, topic, two_topics};
    use crate::broker::tests::bare_writer_and_written;
    use zerodds_amqp_bridge::performatives::descriptor;
    use zerodds_rtps::history_cache::ChangeKind;

    const OWN: [u8; 12] = [1; 12];

    fn guid(prefix: u8) -> [u8; 16] {
        let mut g = [prefix; 16];
        g[12..].copy_from_slice(&[0, 0, 1, 2]);
        g
    }

    fn alive(writer_guid: [u8; 16], key_hash: Option<[u8; 16]>) -> UserSample {
        UserSample::Alive {
            key_hash,
            payload: vec![1, 0, 0, 0].into(),
            writer_guid,
            writer_strength: 0,
            representation: 1,
            big_endian: false,
            representation_identifier: [0x00, 0x07],
            source_timestamp: None,
            reception_timestamp: None,
            source_sequence_number: 9,
            coherent_set_sn: None,
            lifespan_nanos: 0,
            related_sample_identity: None,
            group: None,
            crypto_content: false,
        }
    }

    fn lifecycle(writer_guid: [u8; 16], kind: ChangeKind) -> UserSample {
        UserSample::Lifecycle {
            key_hash: [4; 16],
            key_hash_unavailable: false,
            kind,
            key_holder: None,
            source_timestamp: None,
            group: None,
            writer_guid,
            coherent_set_sn: None,
            source_sequence_number: 3,
        }
    }

    /// The first 16 octets of a `serializedKey` (CDR_LE header, `id` 7),
    /// which dcps hands on as the key hash of a dispose it could not
    /// compute one for.
    const FALLBACK: [u8; 16] = [0, 1, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    /// A dispose from `writer_guid` whose writer sent no PID_KEY_HASH and
    /// whose key the reader could not read (Cyclone 11.0.1 in front of a
    /// reader without `type_file`).
    fn dispose_without_key_hash(writer_guid: [u8; 16]) -> UserSample {
        UserSample::Lifecycle {
            key_hash: FALLBACK,
            key_hash_unavailable: true,
            kind: ChangeKind::NotAliveDisposed,
            key_holder: None,
            source_timestamp: None,
            group: None,
            writer_guid,
            coherent_set_sn: None,
            source_sequence_number: 4,
        }
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    fn messages(t: &TopicMapping, s: &UserSample) -> Option<Vec<Vec<u8>>> {
        let coex = CoexistenceConfig::new("11111111-2222-4333-8444-555555555555".into());
        sample_messages(t, 0, &coex, &OWN, s, &[])
            .map(|v| v.into_iter().map(Result::unwrap).collect())
    }

    /// The end of DDS-AMQP 1.0 §8.4 that DDS holds: a sample this bridge
    /// wrote from AMQP comes back on its own reader and is not sent back;
    /// another writer's sample is.
    #[test]
    fn amqpbridge_12_1_dds_pump_a_sample_of_this_bridge_is_not_sent_back() {
        let t = topic();
        assert!(messages(&t, &alive(guid(1), None)).is_none());
        assert!(messages(&t, &lifecycle(guid(1), ChangeKind::NotAliveDisposed)).is_none());
        assert_eq!(messages(&t, &alive(guid(2), None)).unwrap().len(), 1);
        assert_eq!(
            messages(&t, &lifecycle(guid(2), ChangeKind::NotAliveDisposed))
                .unwrap()
                .len(),
            1
        );
    }

    /// §4.3: a sample is one message with its body; the key hash becomes
    /// `dds:instance-handle` on a keyed topic only.
    #[test]
    fn amqpbridge_12_1_dds_pump_a_sample_becomes_one_message_with_its_key_on_a_keyed_topic() {
        let mut t = topic();
        let m = messages(&t, &alive(guid(2), Some([7; 16]))).unwrap();
        let d = decode_message(&m[0]).unwrap();
        assert_eq!(d.operation, DdsOperation::Write);
        assert_eq!(d.body, vec![0x00, 0x07, 0, 0, 1, 0, 0, 0]);
        assert_eq!(d.instance_handle, Some([7; 16]));
        t.keyed = false;
        let m = messages(&t, &alive(guid(2), Some([7; 16]))).unwrap();
        assert_eq!(decode_message(&m[0]).unwrap().instance_handle, None);
    }

    /// §3, §4.3: a sample of a keyed topic without a KeyHash (its writer
    /// left PID_KEY_HASH out, the reader has no `type_file`) goes out with
    /// the nil instance handle, and that is counted once per writer; a
    /// sample with a KeyHash, a lifecycle marker with one and a sample of
    /// an unkeyed topic are not counted.
    #[test]
    fn amqpbridge_12_1_dds_pump_a_keyed_sample_without_key_hash_is_told_once_per_writer() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let mut l = out_link(&cfg, 0, 1);
        let mut t = topic();
        note_unknown_key_hash(&mut l, &t, &alive(guid(2), None), &m);
        note_unknown_key_hash(&mut l, &t, &alive(guid(2), None), &m);
        assert_eq!(m.errors_total.get(), 1, "once for writer 2");
        note_unknown_key_hash(&mut l, &t, &alive(guid(3), None), &m);
        assert_eq!(m.errors_total.get(), 2, "once for writer 3");
        note_unknown_key_hash(&mut l, &t, &alive(guid(4), Some([7; 16])), &m);
        note_unknown_key_hash(
            &mut l,
            &t,
            &lifecycle(guid(4), ChangeKind::NotAliveDisposed),
            &m,
        );
        let d = decode_message(&messages(&t, &alive(guid(2), None)).unwrap()[0]).unwrap();
        assert_eq!(d.instance_handle, None, "the nil instance handle");
        t.keyed = false;
        note_unknown_key_hash(&mut l, &t, &alive(guid(5), None), &m);
        note_unknown_key_hash(&mut l, &t, &dispose_without_key_hash(guid(5)), &m);
        assert_eq!(
            m.errors_total.get(),
            2,
            "a KeyHash, a marker with one, an unkeyed topic: not counted"
        );
    }

    /// §3, §4.3: a dispose from a writer that sent no PID_KEY_HASH, whose
    /// key the reader could not read (no `type_file`), names no instance:
    /// it goes out with the nil instance handle and no `group-id`, not
    /// with the octets dcps fell back on, and it is counted once per
    /// writer like such a sample.
    #[test]
    fn amqpbridge_12_1_dds_pump_a_dispose_without_key_hash_goes_out_with_the_nil_handle() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let mut l = out_link(&cfg, 0, 1);
        let t = topic();
        let out = messages(&t, &dispose_without_key_hash(guid(4))).unwrap();
        assert_eq!(out.len(), 1);
        let d = decode_message(&out[0]).unwrap();
        assert_eq!(d.operation, DdsOperation::Dispose);
        assert_eq!(d.instance_handle, None, "the nil instance handle");
        assert!(
            !contains(&out[0], crate::keyhash::group_id(&FALLBACK).as_bytes()),
            "no group-id from the fallback octets"
        );
        let known = messages(&t, &lifecycle(guid(4), ChangeKind::NotAliveDisposed)).unwrap();
        assert!(
            contains(&known[0], crate::keyhash::group_id(&[4; 16]).as_bytes()),
            "a marker with a KeyHash has its group-id"
        );
        note_unknown_key_hash(&mut l, &t, &dispose_without_key_hash(guid(4)), &m);
        note_unknown_key_hash(&mut l, &t, &dispose_without_key_hash(guid(4)), &m);
        assert_eq!(m.errors_total.get(), 1, "once for writer 4");
        note_unknown_key_hash(&mut l, &t, &alive(guid(4), None), &m);
        assert_eq!(
            m.errors_total.get(),
            1,
            "a sample of the same writer: told already"
        );
    }

    /// §3, §8.1: the line of `key_hash_unavailable` is a `warn` with the
    /// topic, the writer's GUID and the `type_file` (`none` without one),
    /// written once per writer.
    #[test]
    fn amqpbridge_8_1_key_hash_unavailable_is_logged_once_per_writer_with_its_fields() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let mut l = out_link(&cfg, 0, 1);
        let mut t = topic();
        super::super::log::set_participant(None, None);
        let first = super::super::log::capture(|| {
            note_unknown_key_hash(&mut l, &t, &alive(guid(2), None), &m);
        });
        assert_eq!(first.len(), 1, "{first:?}");
        let (_, rest) = first[0].split_once(",\"level\"").unwrap();
        assert_eq!(
            rest,
            r#":"warn","event":"key_hash_unavailable","dds_topic":"T","writer":"02020202020202020202020200000102","type_file":"none"}"#
        );
        let again = super::super::log::capture(|| {
            note_unknown_key_hash(&mut l, &t, &alive(guid(2), None), &m);
            note_unknown_key_hash(&mut l, &t, &dispose_without_key_hash(guid(2)), &m);
        });
        assert!(again.is_empty(), "writer 2 is told already: {again:?}");
        t.type_file = "/etc/zerodds/t.typeobject".into();
        let marker = super::super::log::capture(|| {
            note_unknown_key_hash(&mut l, &t, &dispose_without_key_hash(guid(3)), &m);
        });
        assert_eq!(marker.len(), 1, "{marker:?}");
        assert!(
            marker[0].ends_with(
                r#""dds_topic":"T","writer":"03030303030303030303030300000102","type_file":"/etc/zerodds/t.typeobject"}"#
            ),
            "{marker:?}"
        );
    }

    /// §4.3, DDSI-RTPS 2.5 §9.6.4.9: a lifecycle marker is one message per
    /// operation, the dispose first, each naming the instance by its key.
    #[test]
    fn amqpbridge_12_1_dds_pump_a_lifecycle_marker_becomes_one_message_per_operation() {
        let t = topic();
        let m = messages(
            &t,
            &lifecycle(guid(2), ChangeKind::NotAliveDisposedUnregistered),
        )
        .unwrap();
        let decoded: Vec<_> = m.iter().map(|b| decode_message(b).unwrap()).collect();
        assert_eq!(
            decoded.iter().map(|d| d.operation).collect::<Vec<_>>(),
            [DdsOperation::Dispose, DdsOperation::Unregister]
        );
        assert!(decoded.iter().all(|d| d.instance_handle == Some([4; 16])));
        let unregistered = messages(&t, &lifecycle(guid(2), ChangeKind::NotAliveUnregistered));
        assert_eq!(unregistered.unwrap().len(), 1);
        // §4.3: an unkeyed topic has no instance handle, a marker neither.
        let mut plain = topic();
        plain.keyed = false;
        let m = messages(&plain, &lifecycle(guid(2), ChangeKind::NotAliveDisposed)).unwrap();
        assert_eq!(decode_message(&m[0]).unwrap().instance_handle, None);
    }

    /// The end of a coherent set (DDS 1.4 §2.2.2.4.1.8) has no message.
    #[test]
    fn amqpbridge_12_1_dds_pump_the_end_of_a_coherent_set_has_no_message() {
        let end = UserSample::CoherentSetEnd {
            writer_guid: guid(2),
            set_first_sn: Some(1),
            ecs_sn: Some(3),
            group: None,
            complete: Some(true),
        };
        assert!(messages(&topic(), &end).is_none());
    }

    /// §6: while the broker grants no credit the queue holds the topic's
    /// HISTORY depth; a full queue drops its oldest message and counts it.
    #[test]
    fn amqpbridge_12_1_dds_pump_a_full_queue_drops_its_oldest_message() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let mut l = out_link(&cfg, 0, 0);
        assert_eq!(l.cap, 3, "KEEP_LAST 3");
        for b in 1..=4u8 {
            enqueue(&mut l, vec![b], 0, "Out", &m);
        }
        let queued: Vec<u8> = l.queue.iter().map(|p| p.message[0]).collect();
        assert_eq!(queued, [2, 3, 4]);
        assert_eq!(m.errors_total.get(), 1);
    }

    /// Part 2 §2.6.7, §2.6.12: a link sends while it has credit, one
    /// delivery per message with a tag of its own; unsettled deliveries are
    /// kept by delivery id, settled ones are not; a link that is not up
    /// sends nothing.
    #[test]
    fn amqpbridge_12_1_dds_pump_drain_sends_while_credit_lasts() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let (mut w, mut written) = bare_writer_and_written();
        w.begin(0).unwrap();
        w.session_begun(0, 100);
        written.next_on();
        let mut l = out_link(&cfg, 0, 0);
        l.credit = 2;
        for b in [1, 2, 3] {
            l.queue.push_back(pending(b));
        }
        drain(&mut w, &mut l, &m).unwrap();
        let tags: Vec<_> = (0..2)
            .map(|_| {
                let (_, code, fields) = written.next_on();
                assert_eq!(code, descriptor::TRANSFER);
                fields[2].clone()
            })
            .collect();
        assert_ne!(tags[0], tags[1], "a tag per delivery");
        assert!(written.nothing_more());
        assert_eq!((l.credit, l.delivery_count), (0, 2));
        assert_eq!(l.unsettled.keys().copied().collect::<Vec<_>>(), [0, 1]);
        assert_eq!(l.queue.len(), 1);
        assert_eq!(m.dds_samples_out_total.get(), 2);
        assert_eq!(m.spec.as_ref().unwrap().topics[0].transfers_out.get(), 2);
        l.settled = true;
        l.credit = 1;
        drain(&mut w, &mut l, &m).unwrap();
        written.next_on();
        assert_eq!(l.unsettled.len(), 2, "a settled message is not kept");
        l.queue.push_back(pending(4));
        l.credit = 5;
        l.state = LinkState::Attaching;
        drain(&mut w, &mut l, &m).unwrap();
        assert!(written.nothing_more(), "the link is not up");
    }

    /// Part 2 §2.5.6: a full session window keeps the message at the front
    /// of the queue, its credit unused, until the broker opens the window.
    #[test]
    fn amqpbridge_12_1_dds_pump_a_full_session_window_keeps_the_message() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let (mut w, mut written) = bare_writer_and_written();
        w.begin(0).unwrap();
        w.session_begun(0, 1);
        written.next_on();
        let mut l = out_link(&cfg, 0, 0);
        l.credit = 3;
        l.queue.push_back(pending(1));
        l.queue.push_back(pending(2));
        drain(&mut w, &mut l, &m).unwrap();
        assert_eq!(written.next_on().1, descriptor::TRANSFER);
        assert!(written.nothing_more());
        assert_eq!(l.credit, 2);
        assert_eq!(l.queue.front().map(|p| p.message[0]), Some(2));
        w.apply_session_flow(0, Some(1), 5);
        drain(&mut w, &mut l, &m).unwrap();
        assert_eq!(written.next_on().1, descriptor::TRANSFER);
        assert!(l.queue.is_empty());
    }

    /// §7.3, §9.2: once a reload withdrew the topic's read right, the
    /// messages queued for credit do not go out; they are dropped and
    /// counted. The catalog link carries no topic and still sends.
    #[test]
    fn amqpbridge_7_3_dds_pump_a_withdrawn_read_right_drops_the_queue() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let (mut w, mut written) = bare_writer_and_written();
        w.begin(0).unwrap();
        w.session_begun(0, 100);
        written.next_on();
        let mut l = out_link(&cfg, 0, 0);
        l.queue.push_back(pending(1));
        l.queue.push_back(pending(2));
        *l.acl.acl.write().unwrap() = (true, Default::default());
        assert!(!l.acl.allows("Out", false));
        l.credit = 5;
        drain(&mut w, &mut l, &m).unwrap();
        assert!(written.nothing_more(), "no transfer of a denied topic");
        assert!(l.queue.is_empty());
        assert_eq!((l.credit, l.delivery_count), (5, 0));
        assert_eq!(m.errors_total.get(), 2);
        assert_eq!(m.dds_samples_out_total.get(), 0);
        l.topic = None;
        l.queue.push_back(pending(3));
        drain(&mut w, &mut l, &m).unwrap();
        assert_eq!(written.next_on().1, descriptor::TRANSFER);
    }

    /// DDSI-RTPS 2.5 §9.6.4.9: a status info with the D and U bits is a
    /// dispose and an unregister of the instance. DDS-AMQP 1.0 carries one
    /// `dds:operation` per message, so the bridge sends both, the dispose
    /// first; before, it sent the dispose alone.
    #[test]
    fn rtps_9_6_4_9_a_dispose_with_unregister_is_two_messages_dispose_first() {
        use zerodds_rtps::history_cache::ChangeKind;
        assert_eq!(
            lifecycle_operations(ChangeKind::NotAliveDisposedUnregistered),
            [DdsOperation::Dispose, DdsOperation::Unregister]
        );
        assert_eq!(
            lifecycle_operations(ChangeKind::NotAliveDisposed),
            [DdsOperation::Dispose]
        );
        assert_eq!(
            lifecycle_operations(ChangeKind::NotAliveUnregistered),
            [DdsOperation::Unregister]
        );
        assert!(lifecycle_operations(ChangeKind::Alive).is_empty());
        let t = topic();
        let coex = CoexistenceConfig::new("x".into());
        let operations: Vec<DdsOperation> =
            lifecycle_operations(ChangeKind::NotAliveDisposedUnregistered)
                .iter()
                .map(|op| {
                    let m = encode_message(
                        &t,
                        0,
                        &coex,
                        &OutSample {
                            payload: &[],
                            representation_identifier: [0x00, 0x07],
                            key_hash: Some([3; 16]),
                            writer_guid: [0; 16],
                            sequence: 0,
                            source_timestamp_ns: None,
                            operation: *op,
                            lifespan_ms: None,
                            partitions: &[],
                        },
                    )
                    .unwrap();
                    decode_message(&m).unwrap().operation
                })
                .collect();
        assert_eq!(
            operations,
            [DdsOperation::Dispose, DdsOperation::Unregister]
        );
    }
}
