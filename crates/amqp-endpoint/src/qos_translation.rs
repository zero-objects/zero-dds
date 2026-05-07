// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! §6 — DDS-QoS → AMQP-Behavior-Translation.
//!
//! Mapping:
//!
//! * `Reliability::Reliable`           → settled-mode `Mixed/Unsettled`.
//! * `Reliability::BestEffort`         → settled-mode `PreSettled`.
//! * `Durability::Volatile`            → durable=false.
//! * `Durability::TransientLocal+`     → durable=true (Broker queue).
//! * `Deadline::period`                → message-ttl (millis).

use zerodds_qos::{DurabilityKind, ReaderQos, ReliabilityKind, WriterQos};

/// AMQP-Settlement-Mode (Spec §2.6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettleMode {
    /// PreSettled — sender settled at send.
    PreSettled,
    /// Mixed/Unsettled — settlement after receiver disposition.
    Unsettled,
}

/// AMQP-Behavior fuer ein Topic-Mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmqpBehavior {
    /// Settlement.
    pub settle_mode: SettleMode,
    /// Durable-Queue (Broker side).
    pub durable: bool,
    /// Message-TTL in Millisekunden (None = no ttl).
    pub ttl_ms: Option<u32>,
}

impl Default for AmqpBehavior {
    fn default() -> Self {
        Self {
            settle_mode: SettleMode::Unsettled,
            durable: false,
            ttl_ms: None,
        }
    }
}

impl AmqpBehavior {
    /// Defaults aus `WriterQos::default()` (Reliable, Volatile).
    #[must_use]
    pub fn default_for_topic() -> Self {
        let w = WriterQos::default();
        let r = ReaderQos::default();
        dds_qos_to_amqp_behavior(&w, &r)
    }
}

/// Mapping-Hauptfunktion.
#[must_use]
pub fn dds_qos_to_amqp_behavior(writer: &WriterQos, reader: &ReaderQos) -> AmqpBehavior {
    let settle_mode = match (writer.reliability.kind, reader.reliability.kind) {
        (ReliabilityKind::Reliable, _) | (_, ReliabilityKind::Reliable) => SettleMode::Unsettled,
        _ => SettleMode::PreSettled,
    };
    let durable = matches!(
        writer.durability.kind,
        DurabilityKind::TransientLocal | DurabilityKind::Transient | DurabilityKind::Persistent,
    );
    let ttl_ms = ttl_for(&writer.deadline);
    AmqpBehavior {
        settle_mode,
        durable,
        ttl_ms,
    }
}

fn ttl_for(d: &zerodds_qos::DeadlineQosPolicy) -> Option<u32> {
    if d.period == zerodds_qos::Duration::INFINITE || d.period == zerodds_qos::Duration::ZERO {
        return None;
    }
    let frac_ms = ((d.period.fraction as u64) * 1000) >> 32;
    let total_ms = (d.period.seconds.max(0) as u64) * 1000 + frac_ms;
    if total_ms == 0 {
        None
    } else if total_ms > u32::MAX as u64 {
        Some(u32::MAX)
    } else {
        Some(total_ms as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zerodds_qos::{Duration, ReliabilityQosPolicy};

    #[test]
    fn reliable_yields_unsettled() {
        let w = WriterQos::default();
        let r = ReaderQos::default();
        let b = dds_qos_to_amqp_behavior(&w, &r);
        assert_eq!(b.settle_mode, SettleMode::Unsettled);
    }

    #[test]
    fn best_effort_yields_presettled() {
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
        let b = dds_qos_to_amqp_behavior(&w, &r);
        assert_eq!(b.settle_mode, SettleMode::PreSettled);
    }

    #[test]
    fn transient_local_yields_durable() {
        let mut w = WriterQos::default();
        w.durability.kind = DurabilityKind::TransientLocal;
        let r = ReaderQos::default();
        let b = dds_qos_to_amqp_behavior(&w, &r);
        assert!(b.durable);
    }

    #[test]
    fn volatile_yields_not_durable() {
        let w = WriterQos::default();
        let r = ReaderQos::default();
        let b = dds_qos_to_amqp_behavior(&w, &r);
        assert!(!b.durable);
    }

    #[test]
    fn deadline_set_yields_ttl() {
        let mut w = WriterQos::default();
        w.deadline.period = Duration::from_millis(2500);
        let r = ReaderQos::default();
        let b = dds_qos_to_amqp_behavior(&w, &r);
        assert_eq!(b.ttl_ms, Some(2500));
    }

    #[test]
    fn deadline_infinite_yields_no_ttl() {
        let w = WriterQos::default();
        let r = ReaderQos::default();
        let b = dds_qos_to_amqp_behavior(&w, &r);
        assert_eq!(b.ttl_ms, None);
    }

    #[test]
    fn default_for_topic_is_unsettled_no_durable() {
        let b = AmqpBehavior::default_for_topic();
        assert_eq!(b.settle_mode, SettleMode::Unsettled);
        assert!(!b.durable);
    }
}
