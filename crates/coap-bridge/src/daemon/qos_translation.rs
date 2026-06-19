// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! §6 — DDS QoS → CoAP behavior translation.
//!
//! Mapping per Spec `zerodds-coap-bridge-1.0.md` §6:
//!
//! * `Reliability::Reliable`        → CON (Confirmable; ACK + retransmit).
//! * `Reliability::BestEffort`      → NON (Non-confirmable).
//! * `Durability::Volatile`         → no observe replay cache.
//! * `Durability::TransientLocal+`  → observe replay cache active.
//! * `Deadline::period`             → CoAP option `Max-Age` (RFC 7252 §5.10.5).

use zerodds_qos::{DurabilityKind, HistoryKind, ReaderQos, ReliabilityKind, WriterQos};

/// CoAP message type (RFC 7252 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoapMessageType {
    /// Confirmable (CON) — peer acks each message.
    Confirmable,
    /// Non-confirmable (NON) — fire-and-forget.
    NonConfirmable,
}

/// Behavior for a topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoapBehavior {
    /// Default type for outgoing notify frames.
    pub message_type: CoapMessageType,
    /// `Max-Age` option in seconds (None = default 60s).
    pub max_age_secs: Option<u32>,
    /// Replay cache depth for newly subscribing observers (0 = none).
    pub replay_depth: u32,
}

impl Default for CoapBehavior {
    fn default() -> Self {
        Self {
            message_type: CoapMessageType::Confirmable,
            max_age_secs: Some(60),
            replay_depth: 0,
        }
    }
}

impl CoapBehavior {
    /// Derive spec-compliant defaults from `WriterQos::default()`.
    #[must_use]
    pub fn default_for_topic() -> Self {
        let w = WriterQos::default();
        let r = ReaderQos::default();
        dds_qos_to_coap_behavior(&w, &r)
    }
}

/// Main function.
#[must_use]
pub fn dds_qos_to_coap_behavior(writer: &WriterQos, reader: &ReaderQos) -> CoapBehavior {
    let message_type = match (writer.reliability.kind, reader.reliability.kind) {
        (ReliabilityKind::Reliable, _) | (_, ReliabilityKind::Reliable) => {
            CoapMessageType::Confirmable
        }
        _ => CoapMessageType::NonConfirmable,
    };
    let replay_depth = match writer.durability.kind {
        DurabilityKind::Volatile => 0,
        DurabilityKind::TransientLocal => match writer.history.kind {
            HistoryKind::KeepLast => writer.history.depth.max(1) as u32,
            HistoryKind::KeepAll => 1024,
        },
        DurabilityKind::Transient | DurabilityKind::Persistent => 1024,
    };
    let max_age_secs = max_age_for(&writer.deadline);
    CoapBehavior {
        message_type,
        max_age_secs,
        replay_depth,
    }
}

fn max_age_for(d: &zerodds_qos::DeadlineQosPolicy) -> Option<u32> {
    if d.period == zerodds_qos::Duration::INFINITE || d.period == zerodds_qos::Duration::ZERO {
        return None;
    }
    if d.period.seconds <= 0 {
        return Some(1);
    }
    Some(d.period.seconds as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zerodds_qos::{Duration, ReliabilityQosPolicy};

    #[test]
    fn reliable_yields_con() {
        let w = WriterQos::default();
        let r = ReaderQos::default();
        let b = dds_qos_to_coap_behavior(&w, &r);
        assert_eq!(b.message_type, CoapMessageType::Confirmable);
    }

    #[test]
    fn best_effort_yields_non() {
        let mut w = WriterQos::default();
        w.reliability = ReliabilityQosPolicy {
            kind: ReliabilityKind::BestEffort,
            ..w.reliability
        };
        let mut r = ReaderQos::default();
        r.reliability = ReliabilityQosPolicy {
            kind: ReliabilityKind::BestEffort,
            ..r.reliability
        };
        let b = dds_qos_to_coap_behavior(&w, &r);
        assert_eq!(b.message_type, CoapMessageType::NonConfirmable);
    }

    #[test]
    fn transient_local_keep_last_replay_depth() {
        let mut w = WriterQos::default();
        w.durability.kind = DurabilityKind::TransientLocal;
        w.history.kind = HistoryKind::KeepLast;
        w.history.depth = 5;
        let r = ReaderQos::default();
        let b = dds_qos_to_coap_behavior(&w, &r);
        assert_eq!(b.replay_depth, 5);
    }

    #[test]
    fn volatile_no_replay() {
        let w = WriterQos::default();
        let r = ReaderQos::default();
        let b = dds_qos_to_coap_behavior(&w, &r);
        assert_eq!(b.replay_depth, 0);
    }

    #[test]
    fn deadline_set_yields_max_age() {
        let mut w = WriterQos::default();
        w.deadline.period = Duration::from_secs(10);
        let r = ReaderQos::default();
        let b = dds_qos_to_coap_behavior(&w, &r);
        assert_eq!(b.max_age_secs, Some(10));
    }

    #[test]
    fn deadline_infinite_no_max_age() {
        let w = WriterQos::default();
        let r = ReaderQos::default();
        let b = dds_qos_to_coap_behavior(&w, &r);
        assert_eq!(b.max_age_secs, None);
    }

    #[test]
    fn default_behavior_is_reliable_no_replay() {
        let b = CoapBehavior::default_for_topic();
        assert_eq!(b.message_type, CoapMessageType::Confirmable);
        assert_eq!(b.replay_depth, 0);
    }
}
