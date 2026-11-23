// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Settling deliveries (spec §4.4, §4.5): the outcome the daemon gives a
//! message it received, and what it does with the outcome the broker gives
//! a message it sent (AMQP 1.0 Part 2 §2.6.12, §2.8.3; Part 3 §3.4).

use std::string::{String, ToString};
use std::sync::Arc;
use std::vec::Vec;

use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_dcps::runtime::DcpsRuntime;
use zerodds_rtps::wire_types::EntityId;

use super::acl::Live;
use super::config::{DaemonConfig, TopicMapping};
use super::dds_pump::{drain_all, round_trip_out, topic_metrics};
use super::link_state::{InLink, LinkState, OutLink, top_up};
use super::log::{Field, emit, enabled, log};
use super::message::{
    CONTENT_TYPE_XCDR1, CONTENT_TYPE_XCDR2, decode_message, redelivered, split_encapsulation,
    wire_extensibility_of,
};
use super::trace::{self, unix_ns};
use crate::broker::{AmqpError, BrokerWriter, Outcome};
use crate::coexistence::{CoexistenceConfig, InboundDecision, inspect_inbound};
use crate::daemon_runtime::BridgeMetrics;
use crate::properties::DdsOperation;

/// How often a released or modified delivery is sent again.
pub(super) const MAX_ATTEMPTS: u32 = 5;

/// A delivery settled with `outcome`, in either direction (spec §8.2).
pub(super) fn count_outcome(metrics: &BridgeMetrics, outcome: &Outcome) {
    let Some(sm) = &metrics.spec else {
        return;
    };
    match outcome {
        Outcome::Accepted => sm.dispositions_accepted.inc(),
        Outcome::Rejected(e) => sm.rejected(e.as_ref().map(|e| e.condition.as_str())),
        Outcome::Released | Outcome::Modified { .. } => sm.dispositions_released.inc(),
        Outcome::None => {}
    }
}

/// The name of an outcome in the log line and the span (§8.1, §8.3);
/// `settled` for a delivery settled without one.
pub(super) fn outcome_name(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Accepted => "accepted",
        Outcome::Rejected(_) => "rejected",
        Outcome::Released => "released",
        Outcome::Modified { .. } => "modified",
        Outcome::None => "settled",
    }
}

/// The error condition of a rejection.
fn condition_of(outcome: &Outcome) -> Option<String> {
    match outcome {
        Outcome::Rejected(Some(e)) => Some(e.condition.clone()),
        _ => None,
    }
}

/// A transfer on receiving link `l` (§4.4), read from the connection at
/// `read_ns`: written to DDS, unless a reload withdrew the right to write
/// the topic, and settled with the outcome. The `transfer_in` line (§8.1)
/// and the spans of §8.3 follow the disposition.
#[allow(clippy::too_many_arguments)]
pub(super) fn on_transfer(
    cfg: &DaemonConfig,
    live: &Live,
    runtime: &Arc<DcpsRuntime>,
    writer: &mut BrokerWriter,
    l: &mut InLink,
    delivery_id: u32,
    settled: bool,
    message: &[u8],
    read_ns: u64,
    coexistence: &CoexistenceConfig,
    metrics: &BridgeMetrics,
) -> Result<(), String> {
    let t = &cfg.topics[l.topic];
    l.delivery_count = l.delivery_count.wrapping_add(1);
    l.outstanding = l.outstanding.saturating_sub(1);
    metrics.bytes_in_total.add(message.len() as u64);
    if let Some(tm) = topic_metrics(metrics, l.topic) {
        tm.transfers_in.inc();
        tm.bytes_in.add(message.len() as u64);
    }
    let mut write = None;
    let outcome = if live.allows(&t.dds_name, true) {
        deliver(
            runtime,
            l.writer,
            t,
            message,
            coexistence,
            metrics,
            &mut write,
        )
    } else {
        // A right withdrawn by a reload.
        log("acl_denied", &[("dds_topic", &t.dds_name), ("op", "write")]);
        Outcome::Rejected(Some(AmqpError {
            condition: "amqp:unauthorized-access".into(),
            description: Some(format!("acl: no write on {}", t.dds_name)),
        }))
    };
    let result = settle_inbound(writer, l, t, delivery_id, settled, &outcome, metrics);
    let now = unix_ns();
    if enabled("transfer_in") {
        emit(
            "transfer_in",
            &[
                ("link_name", Field::Text(&l.spec.name)),
                ("dds_topic", Field::Text(&t.dds_name)),
                ("amqp_address", Field::Text(&l.spec.address)),
                ("delivery_id", Field::Number(u64::from(delivery_id))),
                ("bytes", Field::Number(message.len() as u64)),
                ("outcome", Field::Text(outcome_name(&outcome))),
                (
                    "latency_us",
                    Field::Number(now.saturating_sub(read_ns) / 1000),
                ),
            ],
        );
    }
    if trace::tracing() {
        trace::record(&trace::Transfer {
            direction: trace::Direction::In,
            start_ns: read_ns,
            end_ns: now,
            dds: write,
            dds_topic: &t.dds_name,
            amqp_address: &l.spec.address,
            link_name: &l.spec.name,
            delivery_id,
            // A message the broker sent settled gets no disposition; its
            // outcome is still what the bridge did with it.
            outcome: outcome_name(&outcome),
            error: condition_of(&outcome),
        });
    }
    result
}

/// Sends the outcome of delivery `delivery_id` of receiving link `l` and
/// grants credit again. A delivery the broker sent settled gets no
/// disposition; one that was not written is logged. In `rcv-settle-mode`
/// second (Part 2 §2.8.3) the outcome goes out unsettled and the delivery
/// takes up credit until the broker settles it.
pub(super) fn settle_inbound(
    writer: &mut BrokerWriter,
    l: &mut InLink,
    t: &TopicMapping,
    delivery_id: u32,
    settled: bool,
    outcome: &Outcome,
    metrics: &BridgeMetrics,
) -> Result<(), String> {
    if let (Some(tm), Outcome::Accepted) = (topic_metrics(metrics, l.topic), outcome) {
        tm.dds_published.inc();
    }
    if !settled {
        count_outcome(metrics, outcome);
        writer
            .disposition_settled(l.channel, delivery_id, delivery_id, !l.second, outcome)
            .map_err(|e| format!("disposition: {e}"))?;
        if l.second {
            l.awaiting.insert(delivery_id);
        }
    } else if !matches!(outcome, Outcome::Accepted) {
        log("settled_message_not_written", &[("dds_topic", &t.dds_name)]);
    }
    top_up(writer, l, t.credit, metrics).map(|_| ())
}

/// A disposition from the broker for deliveries `range` (first, last) on
/// the session `channel`.
///
/// As the sender (`role_receiver` false) the broker settles deliveries of
/// a receiving link in mode second; they no longer take up credit. As the
/// receiver it gives the outcome of messages the daemon sent (§4.5, AMQP
/// 1.0 Part 3 §3.4):
///
/// * `accepted` and `rejected` end the delivery;
/// * `released` sends it again with `first-acquirer` false;
/// * `modified` sends it again like `released`, its delivery-count one
///   more when `delivery-failed`, its message-annotations combined with
///   `annotations`; with `undeliverable-here` it is not sent again on this
///   link (§3.4.5), which is the only one of its topic: it is discarded,
///   counted as an error and logged;
/// * a message given back [`MAX_ATTEMPTS`] times is discarded, counted and
///   logged.
///
/// A state that is not an outcome (`received`) leaves an unsettled delivery
/// as it is. An outcome the broker did not settle is settled by the daemon
/// (Part 2 §2.8.3).
#[allow(clippy::too_many_arguments)]
pub(super) fn on_disposition(
    cfg: &DaemonConfig,
    writer: &mut BrokerWriter,
    outs: &mut [OutLink],
    ins: &mut [InLink],
    channel: u16,
    role_receiver: bool,
    (first, last): (u32, u32),
    settled: bool,
    outcome: &Outcome,
    annotations: &[(AmqpExtValue, AmqpExtValue)],
    metrics: &BridgeMetrics,
) -> Result<(), String> {
    if !role_receiver {
        if settled && let Some(l) = ins.iter_mut().find(|l| l.channel == channel) {
            let ids: Vec<u32> = l.awaiting.range(first..=last).copied().collect();
            for id in ids {
                l.awaiting.remove(&id);
            }
            if l.state == LinkState::Up {
                let credit = cfg.topics[l.topic].credit;
                top_up(writer, l, credit, metrics)?;
            }
        }
        return Ok(());
    }
    if !settled && matches!(outcome, Outcome::None) {
        return Ok(());
    }
    if let Some(l) = outs.iter_mut().find(|l| l.channel == channel) {
        let ids: Vec<u32> = l.unsettled.range(first..=last).map(|(id, _)| *id).collect();
        let name = l.spec.name.clone();
        if !settled && !ids.is_empty() {
            writer
                .settle(channel, first, last)
                .map_err(|e| format!("settle: {e}"))?;
        }
        // Sent again first, in the order they were sent.
        let mut again = Vec::new();
        for id in ids {
            let Some(mut p) = l.unsettled.remove(&id) else {
                continue;
            };
            count_outcome(metrics, outcome);
            round_trip_out(l, id, &p, outcome_name(outcome), condition_of(outcome));
            match outcome {
                Outcome::Accepted | Outcome::None => {}
                Outcome::Rejected(e) => {
                    metrics.errors_total.inc();
                    let reason = e
                        .as_ref()
                        .map_or_else(|| "no reason".to_string(), ToString::to_string);
                    log(
                        "rejected_by_broker",
                        &[("link_name", &name), ("err", &reason)],
                    );
                }
                Outcome::Modified {
                    undeliverable_here: true,
                    ..
                } => {
                    metrics.errors_total.inc();
                    log("undeliverable_here", &[("link_name", &name)]);
                }
                Outcome::Released | Outcome::Modified { .. } => {
                    p.attempts += 1;
                    if p.attempts >= MAX_ATTEMPTS {
                        metrics.errors_total.inc();
                        log("released_too_often", &[("link_name", &name)]);
                        continue;
                    }
                    let (failed, add) = match outcome {
                        Outcome::Modified {
                            delivery_failed, ..
                        } => (*delivery_failed, annotations),
                        _ => (false, &[][..]),
                    };
                    match redelivered(&p.message, failed, add) {
                        Ok(m) => {
                            p.message = m;
                            again.push(p);
                        }
                        Err(e) => {
                            metrics.errors_total.inc();
                            log("redelivery_failed", &[("link_name", &name), ("err", &e)]);
                        }
                    }
                }
            }
        }
        for p in again.into_iter().rev() {
            l.queue.push_front(p);
        }
    }
    drain_all(writer, outs, metrics)
}

/// Writes one message as a DDS sample; the outcome settles it (§4.4).
/// `write` gets the start and end of the DDS write, when there was one
/// (§8.3 `dds.write`).
pub(super) fn deliver(
    runtime: &Arc<DcpsRuntime>,
    eid: EntityId,
    t: &TopicMapping,
    message: &[u8],
    coexistence: &CoexistenceConfig,
    metrics: &BridgeMetrics,
    write: &mut Option<(u64, u64)>,
) -> Outcome {
    let rejected = |condition: &str, description: String| {
        metrics.errors_total.inc();
        log(
            "rejected",
            &[("dds_topic", &t.dds_name), ("err", &description)],
        );
        Outcome::Rejected(Some(AmqpError {
            condition: condition.into(),
            description: Some(description),
        }))
    };
    let msg = match decode_message(message) {
        Ok(m) => m,
        Err(e) => return rejected("amqp:decode-error", e),
    };
    match inspect_inbound(coexistence, &msg.application_properties) {
        InboundDecision::Forward => {}
        // Sent by this bridge (or past the hop cap): consumed, not written,
        // and said so (DDS-AMQP 1.0 §8.4).
        InboundDecision::DropLoop => {
            log("dropped_own_message", &[("dds_topic", &t.dds_name)]);
            return Outcome::Accepted;
        }
        InboundDecision::DropHopCap => {
            metrics.errors_total.inc();
            log("dropped_hop_cap", &[("dds_topic", &t.dds_name)]);
            return Outcome::Accepted;
        }
    }
    match msg.content_type.as_deref() {
        None | Some(CONTENT_TYPE_XCDR2) | Some(CONTENT_TYPE_XCDR1) => {}
        Some(other) => {
            return rejected(
                "amqp:decode-error",
                format!("content-type {other}; the bridge writes {CONTENT_TYPE_XCDR2} bodies"),
            );
        }
    }
    // §6: a message that names its partitions is written only into a
    // topic whose partitions match one of them (DDS 1.4 §2.2.3.13); the
    // bridge's writer has the topic's partitions and no other. A message
    // without `dds:partition` takes the topic's.
    if let Some(p) = &msg.partitions
        && !zerodds_qos::partitions_compatible(p, &t.partition)
    {
        metrics.errors_total.inc();
        let description = format!(
            "dds:partition {p:?} matches none of the topic's partitions {:?}",
            t.partition
        );
        log(
            "partition_mismatch",
            &[("dds_topic", &t.dds_name), ("err", &description)],
        );
        return Outcome::Rejected(Some(AmqpError {
            condition: "amqp:precondition-failed".into(),
            description: Some(description),
        }));
    }
    let key = if t.keyed { msg.instance_handle } else { None };
    let write_start = unix_ns();
    let result = match msg.operation {
        DdsOperation::Dispose | DdsOperation::Unregister => {
            let Some(key) = key else {
                return rejected(
                    "amqp:decode-error",
                    "dispose/unregister without dds:instance-handle".into(),
                );
            };
            let bits = if msg.operation == DdsOperation::Dispose {
                zerodds_rtps::inline_qos::status_info::DISPOSED
            } else {
                zerodds_rtps::inline_qos::status_info::UNREGISTERED
            };
            // The DDS-AMQP 1.0 mapping's dispose/unregister message
            // carries only `dds:instance-handle` (16 byte,
            // `msg.instance_handle` above), so no key holder is passed.
            // With the topic's `type_file` the writer takes the key it
            // recorded — from a sample this endpoint wrote to DDS or one it
            // carried to AMQP — or reads it from a key hash that spells it
            // out, and the marker carries the serializedKey. Without a
            // type the marker has the hash alone and the writer reports
            // that once.
            runtime.write_user_lifecycle(eid, key, None, bits, None)
        }
        DdsOperation::Register => return Outcome::Accepted,
        DdsOperation::Write => {
            let (rep, be, payload) = match split_encapsulation(&msg.body) {
                Ok(v) => v,
                Err(e) => return rejected("amqp:decode-error", e),
            };
            use zerodds_rtps::publication_data::data_representation as dr;
            let offer = if rep == 0 { dr::XCDR } else { dr::XCDR2 };
            let _ = runtime.set_user_writer_data_rep_override(eid, Some(vec![offer]));
            let _ = runtime.set_user_writer_byte_order_override(eid, be);
            // The format of the body as its header named it: the writer's
            // header has to say the same (DDSI-RTPS 2.5 §10.5).
            let _ = runtime.set_user_writer_wire_extensibility(
                eid,
                wire_extensibility_of([msg.body[0], msg.body[1]]),
            );
            match key {
                Some(k) => runtime.write_user_sample_keyed(eid, payload, k, None),
                None => runtime.write_user_sample_borrowed(eid, payload),
            }
        }
    };
    *write = Some((write_start, unix_ns()));
    match result {
        Ok(()) => Outcome::Accepted,
        Err(e) => {
            metrics.errors_total.inc();
            log(
                "dds_write_failed",
                &[("dds_topic", &t.dds_name), ("err", &format!("{e:?}"))],
            );
            Outcome::Released
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::bridge_daemon::link_state::Pending;
    use crate::bridge_daemon::test_support::{in_link, metrics, out_link, topic, two_topics};
    use crate::broker::tests::bare_writer_and_written;
    use crate::terminus::decode_outcome;
    use zerodds_amqp_bridge::extended_types::AmqpExtValue;
    use zerodds_amqp_bridge::performatives::descriptor;
    use zerodds_amqp_bridge::sections::MessageSection;

    fn ids(l: &OutLink) -> Vec<u32> {
        l.unsettled.keys().copied().collect()
    }

    /// A message as the daemon sends one: a header (durable false,
    /// priority 4, first-acquirer true) and a data section of `byte`.
    fn sample(byte: u8) -> Pending {
        let mut message = MessageSection::Header(AmqpExtValue::List(vec![
            AmqpExtValue::Boolean(false),
            AmqpExtValue::Ubyte(4),
            AmqpExtValue::Null,
            AmqpExtValue::Boolean(true),
        ]))
        .encode()
        .unwrap();
        message.extend_from_slice(&MessageSection::Data(vec![byte]).encode().unwrap());
        Pending {
            message,
            attempts: 0,
            taken_ns: 0,
        }
    }

    /// The sections of a queued message.
    fn sections(p: &Pending) -> Vec<MessageSection> {
        let mut rest = &p.message[..];
        let mut out = Vec::new();
        while !rest.is_empty() {
            let (s, n) = MessageSection::decode(rest).unwrap();
            out.push(s);
            rest = &rest[n..];
        }
        out
    }

    /// The data byte of a queued message.
    fn byte_of(p: &Pending) -> u8 {
        sections(p)
            .into_iter()
            .find_map(|s| match s {
                MessageSection::Data(d) => Some(d[0]),
                _ => None,
            })
            .unwrap()
    }

    /// The header fields of a queued message.
    fn header_of(p: &Pending) -> Vec<AmqpExtValue> {
        match sections(p).remove(0) {
            MessageSection::Header(AmqpExtValue::List(f)) => f,
            other => panic!("{other:?}"),
        }
    }

    fn queued(l: &OutLink) -> Vec<(u8, u32)> {
        l.queue.iter().map(|p| (byte_of(p), p.attempts)).collect()
    }

    fn settle(
        outs: &mut [OutLink],
        ins: &mut [InLink],
        role_receiver: bool,
        range: (u32, u32),
        settled: bool,
        outcome: Outcome,
    ) -> (crate::broker::tests::Written, BridgeMetrics) {
        settle_with(outs, ins, role_receiver, range, settled, outcome, &[])
    }

    fn settle_with(
        outs: &mut [OutLink],
        ins: &mut [InLink],
        role_receiver: bool,
        range: (u32, u32),
        settled: bool,
        outcome: Outcome,
        annotations: &[(AmqpExtValue, AmqpExtValue)],
    ) -> (crate::broker::tests::Written, BridgeMetrics) {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let (mut w, written) = bare_writer_and_written();
        on_disposition(
            &cfg,
            &mut w,
            outs,
            ins,
            outs.first().map_or(1, |l| l.channel),
            role_receiver,
            range,
            settled,
            &outcome,
            annotations,
            &m,
        )
        .unwrap();
        (written, m)
    }

    fn unsettled(ids: &[u32]) -> Vec<OutLink> {
        let mut l = out_link(&two_topics(), 0, 0);
        for id in ids {
            l.unsettled.insert(*id, sample(u8::try_from(*id).unwrap()));
        }
        vec![l]
    }

    /// §4.5, Part 2 §2.7.6: `accepted` for first..=last ends exactly the
    /// deliveries of that range.
    #[test]
    fn amqpbridge_12_1_disposition_accepted_ends_the_deliveries_of_the_range() {
        let mut outs = unsettled(&[3, 4, 5, 6]);
        let (mut written, m) = settle(&mut outs, &mut [], true, (4, 5), true, Outcome::Accepted);
        assert_eq!(ids(&outs[0]), [3, 6]);
        assert!(outs[0].queue.is_empty());
        assert_eq!(m.spec.as_ref().unwrap().dispositions_accepted.get(), 2);
        assert_eq!(m.errors_total.get(), 0);
        assert!(
            written.nothing_more(),
            "settled by the broker, nothing to answer"
        );
    }

    /// §4.5: `rejected` ends the delivery, counts an error and is not sent
    /// again.
    #[test]
    fn amqpbridge_12_1_disposition_rejected_ends_the_delivery_and_counts_an_error() {
        let mut outs = unsettled(&[7]);
        let rejected = Outcome::Rejected(Some(AmqpError {
            condition: "amqp:decode-error".into(),
            description: None,
        }));
        let (_, m) = settle(&mut outs, &mut [], true, (7, 7), true, rejected);
        assert!(outs[0].unsettled.is_empty());
        assert!(
            outs[0].queue.is_empty(),
            "a rejected message is not sent again"
        );
        assert_eq!(m.errors_total.get(), 1);
    }

    /// §4.5: `released` and `modified` send the message again, first in the
    /// queue, until it has been tried [`MAX_ATTEMPTS`] times.
    #[test]
    fn amqpbridge_12_1_disposition_released_and_modified_send_again_up_to_max_attempts() {
        let mut outs = unsettled(&[1, 2]);
        outs[0].unsettled.get_mut(&2).unwrap().attempts = MAX_ATTEMPTS - 1;
        outs[0].queue.push_back(sample(9));
        let (_, m) = settle(&mut outs, &mut [], true, (1, 2), true, Outcome::Released);
        assert_eq!(
            queued(&outs[0]),
            [(1, 1), (9, 0)],
            "2 was tried often enough"
        );
        assert_eq!(m.errors_total.get(), 1);
        assert_eq!(m.spec.as_ref().unwrap().dispositions_released.get(), 2);
        outs[0].queue.clear();
        outs[0].unsettled.insert(4, sample(4));
        let modified = Outcome::Modified {
            delivery_failed: true,
            undeliverable_here: false,
        };
        settle(&mut outs, &mut [], true, (4, 4), true, modified);
        assert_eq!(queued(&outs[0]), [(4, 1)]);
    }

    /// §4.5, Part 3 §3.4.4, §3.2.1: several released deliveries go out
    /// again in the order they were sent, each with `first-acquirer` false
    /// and its delivery-count unchanged.
    #[test]
    fn amqpbridge_4_5_released_deliveries_go_out_again_in_order_not_first_acquired() {
        let mut outs = unsettled(&[1, 2, 3]);
        settle(&mut outs, &mut [], true, (1, 3), true, Outcome::Released);
        assert_eq!(queued(&outs[0]), [(1, 1), (2, 1), (3, 1)]);
        for p in &outs[0].queue {
            let h = header_of(p);
            assert_eq!(h[3], AmqpExtValue::Boolean(false), "first-acquirer");
            assert!(
                matches!(h.get(4), None | Some(AmqpExtValue::Null)),
                "delivery-count stays 0: {h:?}"
            );
            assert_eq!(h[1], AmqpExtValue::Ubyte(4), "priority kept");
        }
    }

    /// §4.5, Part 3 §3.4.5: `modified` with delivery-failed raises the
    /// delivery-count by one each time; without it the count stays.
    #[test]
    fn amqpbridge_4_5_modified_delivery_failed_raises_the_delivery_count() {
        let mut outs = unsettled(&[1]);
        let failed = Outcome::Modified {
            delivery_failed: true,
            undeliverable_here: false,
        };
        settle(&mut outs, &mut [], true, (1, 1), true, failed.clone());
        let p = outs[0].queue.pop_front().unwrap();
        assert_eq!(header_of(&p)[4], AmqpExtValue::Uint(1));
        outs[0].unsettled.insert(2, p);
        settle(&mut outs, &mut [], true, (2, 2), true, failed);
        let p = outs[0].queue.pop_front().unwrap();
        assert_eq!(header_of(&p)[4], AmqpExtValue::Uint(2), "once more");
        assert_eq!(p.attempts, 2);
        outs[0].unsettled.insert(3, p);
        let not_failed = Outcome::Modified {
            delivery_failed: false,
            undeliverable_here: false,
        };
        settle(&mut outs, &mut [], true, (3, 3), true, not_failed);
        let p = outs[0].queue.pop_front().unwrap();
        assert_eq!(header_of(&p)[4], AmqpExtValue::Uint(2), "not failed: kept");
        assert_eq!(byte_of(&p), 1, "the same message");
    }

    /// §4.5, Part 3 §3.4.5: `modified` with undeliverable-here is not sent
    /// again on the link; it is counted as an error and as released.
    #[test]
    fn amqpbridge_4_5_modified_undeliverable_here_is_not_sent_again() {
        let mut outs = unsettled(&[1, 2]);
        let outcome = Outcome::Modified {
            delivery_failed: true,
            undeliverable_here: true,
        };
        let (_, m) = settle(&mut outs, &mut [], true, (1, 1), true, outcome);
        assert!(outs[0].queue.is_empty(), "not sent again");
        assert_eq!(ids(&outs[0]), [2]);
        assert_eq!(m.errors_total.get(), 1);
        assert_eq!(m.spec.as_ref().unwrap().dispositions_released.get(), 1);
    }

    /// §4.5, Part 3 §3.4.5: the message-annotations of `modified` are
    /// combined with the message's: added as a section after the header
    /// when it has none, an entry of the same key replaced otherwise.
    #[test]
    fn amqpbridge_4_5_modified_annotations_are_combined_with_the_message() {
        let sym = |s: &str| AmqpExtValue::Symbol(s.into());
        let annotations_of = |p: &Pending| {
            sections(p)
                .into_iter()
                .find_map(|s| match s {
                    MessageSection::MessageAnnotations(AmqpExtValue::Map(m)) => Some(m),
                    _ => None,
                })
                .unwrap()
        };
        let modified = Outcome::Modified {
            delivery_failed: false,
            undeliverable_here: false,
        };
        let mut outs = unsettled(&[1]);
        settle_with(
            &mut outs,
            &mut [],
            true,
            (1, 1),
            true,
            modified.clone(),
            &[(sym("x-opt-a"), AmqpExtValue::Uint(1))],
        );
        let p = outs[0].queue.pop_front().unwrap();
        let kinds: Vec<&str> = sections(&p)
            .iter()
            .map(|s| match s {
                MessageSection::Header(_) => "header",
                MessageSection::MessageAnnotations(_) => "message-annotations",
                MessageSection::Data(_) => "data",
                _ => "other",
            })
            .collect();
        assert_eq!(kinds, ["header", "message-annotations", "data"]);
        assert_eq!(
            annotations_of(&p),
            [(sym("x-opt-a"), AmqpExtValue::Uint(1))]
        );
        outs[0].unsettled.insert(2, p);
        settle_with(
            &mut outs,
            &mut [],
            true,
            (2, 2),
            true,
            modified,
            &[
                (sym("x-opt-a"), AmqpExtValue::Uint(7)),
                (sym("x-opt-b"), AmqpExtValue::Str("b".into())),
            ],
        );
        let p = outs[0].queue.pop_front().unwrap();
        assert_eq!(
            annotations_of(&p),
            [
                (sym("x-opt-a"), AmqpExtValue::Uint(7)),
                (sym("x-opt-b"), AmqpExtValue::Str("b".into()))
            ],
            "a replaced, b added, one section"
        );
        assert_eq!(byte_of(&p), 1);
    }

    /// A range without an unsettled delivery, or a session without a
    /// sending link, changes nothing and sends nothing.
    #[test]
    fn amqpbridge_12_1_disposition_an_unknown_delivery_changes_nothing() {
        let mut outs = unsettled(&[3]);
        let (mut written, m) = settle(&mut outs, &mut [], true, (10, 12), false, Outcome::Accepted);
        assert_eq!(ids(&outs[0]), [3]);
        assert!(written.nothing_more(), "nothing to settle");
        assert_eq!(m.spec.as_ref().unwrap().dispositions_accepted.get(), 0);
        outs[0].channel = 7;
        let cfg = two_topics();
        let (mut w, mut written) = bare_writer_and_written();
        on_disposition(
            &cfg,
            &mut w,
            &mut outs,
            &mut [],
            0,
            true,
            (3, 3),
            true,
            &Outcome::Accepted,
            &[],
            &m,
        )
        .unwrap();
        assert_eq!(ids(&outs[0]), [3], "another session's delivery 3");
        assert!(written.nothing_more());
    }

    /// Part 2 §2.8.3: an outcome the broker sends unsettled (its receiver
    /// settle mode is second) is settled by the daemon; a state that is no
    /// outcome (`received`) leaves the delivery as it is.
    #[test]
    fn amqpbridge_12_1_disposition_an_unsettled_outcome_is_settled_by_the_daemon() {
        let mut outs = unsettled(&[4]);
        let (mut written, _) = settle(&mut outs, &mut [], true, (4, 4), false, Outcome::Accepted);
        let (channel, code, fields) = written.next_on();
        assert_eq!((channel, code), (0, descriptor::DISPOSITION));
        assert_eq!(fields[0], AmqpExtValue::Boolean(false), "role sender");
        assert_eq!(fields[3], AmqpExtValue::Boolean(true), "settled");
        assert!(outs[0].unsettled.is_empty());
        let mut outs = unsettled(&[5]);
        let (mut written, _) = settle(&mut outs, &mut [], true, (5, 5), false, Outcome::None);
        assert_eq!(ids(&outs[0]), [5]);
        assert!(written.nothing_more());
    }

    /// Part 2 §2.8.3, §4.4: in mode second the broker's settlement frees
    /// the credit its deliveries held, and the credit is granted again.
    #[test]
    fn amqpbridge_12_1_disposition_the_brokers_settlement_frees_credit_in_mode_second() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let (mut w, mut written) = bare_writer_and_written();
        let mut ins = vec![in_link(&cfg, 1, 1, true)];
        ins[0].awaiting.extend([2, 3, 4]);
        ins[0].outstanding = 0;
        w.begin(1).unwrap();
        written.next_on();
        on_disposition(
            &cfg,
            &mut w,
            &mut [],
            &mut ins,
            1,
            false,
            (2, 3),
            false,
            &Outcome::Accepted,
            &[],
            &m,
        )
        .unwrap();
        assert_eq!(ins[0].awaiting.len(), 3, "not settled yet");
        assert!(written.nothing_more());
        on_disposition(
            &cfg,
            &mut w,
            &mut [],
            &mut ins,
            1,
            false,
            (2, 3),
            true,
            &Outcome::Accepted,
            &[],
            &m,
        )
        .unwrap();
        assert_eq!(ins[0].awaiting.iter().copied().collect::<Vec<_>>(), [4]);
        let (_, code, fields) = written.next_on();
        assert_eq!(code, descriptor::FLOW);
        assert_eq!(
            fields[6],
            AmqpExtValue::Uint(7),
            "8 less the one still awaiting"
        );
    }

    /// §4.4: the outcome of a received message goes out settled; in mode
    /// second unsettled, and the delivery waits for the broker. A message
    /// the broker sent settled gets no disposition.
    #[test]
    fn amqpbridge_12_1_disposition_an_inbound_outcome_goes_out_settled_or_in_mode_second() {
        let cfg = two_topics();
        let m = metrics(&cfg);
        let t = &cfg.topics[1];
        let (mut w, mut written) = bare_writer_and_written();
        let mut first = in_link(&cfg, 1, 1, false);
        settle_inbound(&mut w, &mut first, t, 11, false, &Outcome::Accepted, &m).unwrap();
        let (channel, code, fields) = written.next_on();
        assert_eq!((channel, code), (1, descriptor::DISPOSITION));
        assert_eq!(fields[0], AmqpExtValue::Boolean(true), "role receiver");
        assert_eq!(
            (&fields[1], &fields[2]),
            (&AmqpExtValue::Uint(11), &AmqpExtValue::Uint(11))
        );
        assert_eq!(fields[3], AmqpExtValue::Boolean(true));
        assert_eq!(decode_outcome(fields.get(4)), Outcome::Accepted);
        assert_eq!(m.spec.as_ref().unwrap().topics[1].dds_published.get(), 1);
        let mut second = in_link(&cfg, 1, 1, true);
        settle_inbound(&mut w, &mut second, t, 12, false, &Outcome::Released, &m).unwrap();
        let fields = written.next_on().2;
        assert_eq!(
            fields[3],
            AmqpExtValue::Boolean(false),
            "mode second: unsettled"
        );
        assert_eq!(decode_outcome(fields.get(4)), Outcome::Released);
        assert!(second.awaiting.contains(&12));
        settle_inbound(&mut w, &mut first, t, 13, true, &Outcome::Released, &m).unwrap();
        assert!(
            written.nothing_more(),
            "settled by the broker: no disposition"
        );
    }
    use crate::bridge_daemon::{OutSample, encode_message};
    use zerodds_dcps::runtime::{RuntimeConfig, UserWriterConfig};

    /// §4.4: a message the DDS writer cannot take is `released`, not
    /// rejected: here a TRANSIENT_LOCAL KEEP_ALL writer at its RESOURCE_LIMITS
    /// (`max_samples` 1), which keeps what it wrote (DDS 1.4 §2.2.3.19,
    /// OUT_OF_RESOURCES). The broker may deliver the message again later.
    #[test]
    fn amqpbridge_4_4_a_failed_dds_write_releases_the_delivery() {
        let lease = zerodds_test_vendor_reader::lease_domain();
        let runtime = DcpsRuntime::start(
            lease.id(),
            zerodds_dcps::participant::bridge_guid_prefix("amqpbridge_4_4_released"),
            RuntimeConfig::default(),
        )
        .unwrap();
        let _stop = crate::bridge_daemon::ShutdownOnDrop(Arc::clone(&runtime));
        let mut t = topic();
        t.keyed = false;
        t.durability = zerodds_qos::DurabilityKind::TransientLocal;
        let eid = runtime
            .register_user_writer_kind(
                UserWriterConfig {
                    topic_name: "amqpbridge_4_4_released".into(),
                    type_name: "T".into(),
                    reliable: true,
                    durability: t.durability,
                    ..Default::default()
                },
                false,
            )
            .unwrap();
        runtime
            .set_user_writer_history_depth(eid, usize::MAX)
            .unwrap();
        runtime
            .set_user_writer_resource_limits(
                eid,
                zerodds_qos::ResourceLimitsQosPolicy {
                    max_samples: 1,
                    ..Default::default()
                },
                zerodds_qos::Duration::ZERO,
            )
            .unwrap();
        let ours = CoexistenceConfig::new("11111111-2222-4333-8444-555555555555".into());
        let theirs = CoexistenceConfig::new("99999999-2222-4333-8444-555555555555".into());
        let message = encode_message(
            &t,
            0,
            &theirs,
            &OutSample {
                payload: &[1, 0, 0, 0],
                representation_identifier: [0x00, 0x07],
                key_hash: None,
                writer_guid: [3; 16],
                sequence: 1,
                source_timestamp_ns: None,
                operation: DdsOperation::Write,
                lifespan_ms: None,
                partitions: &[],
            },
        )
        .unwrap();
        let metrics = BridgeMetrics::register(&zerodds_monitor::Registry::new());
        assert_eq!(
            deliver(&runtime, eid, &t, &message, &ours, &metrics, &mut None),
            Outcome::Accepted
        );
        assert_eq!(
            deliver(&runtime, eid, &t, &message, &ours, &metrics, &mut None),
            Outcome::Released,
            "the writer is full"
        );
        // A body that is no DDS sample stays rejected.
        assert!(matches!(
            deliver(
                &runtime,
                eid,
                &t,
                b"\x00\x53\x75\xa0\x01x",
                &ours,
                &metrics,
                &mut None
            ),
            Outcome::Rejected(_)
        ));
    }
}
