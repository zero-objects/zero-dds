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

/// AMQP behavior for a topic mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmqpBehavior {
    /// Settlement.
    pub settle_mode: SettleMode,
    /// Durable-Queue (Broker side).
    pub durable: bool,
    /// Message TTL in milliseconds (None = no ttl).
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
    /// Defaults from `WriterQos::default()` (Reliable, Volatile).
    #[must_use]
    pub fn default_for_topic() -> Self {
        let w = WriterQos::default();
        let r = ReaderQos::default();
        dds_qos_to_amqp_behavior(&w, &r)
    }
}

/// Main mapping function.
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
    // Rounded to the nearest millisecond: 300 ms is 299.999… ms in
    // 2^-32 s and was cut to 299.
    let total_ms = u64::try_from(d.period.to_millis()).unwrap_or(u64::MAX);
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

    /// Spec `dds-amqp-1.0` §7.4.3 (HISTORY), §7.4.5 (LATENCY_BUDGET),
    /// §7.4.6 (OWNERSHIP) and §7.4.9 (all other policies): these are
    /// DDS-side and transparent to the AMQP boundary. "Transparent" is
    /// measurable: changing them must leave the AMQP behaviour — settle
    /// mode, durability and TTL, the whole of `AmqpBehavior` — exactly
    /// as it was. Only RELIABILITY, DURABILITY and DEADLINE cross.
    #[test]
    fn the_dds_side_policies_leave_no_trace_on_the_amqp_boundary() {
        use zerodds_qos::{
            DestinationOrderKind, DestinationOrderQosPolicy, HistoryKind, HistoryQosPolicy,
            LatencyBudgetQosPolicy, LivelinessKind, LivelinessQosPolicy, OwnershipKind,
            OwnershipQosPolicy, ResourceLimitsQosPolicy, TimeBasedFilterQosPolicy,
            TransportPriorityQosPolicy,
        };

        let base = dds_qos_to_amqp_behavior(&WriterQos::default(), &ReaderQos::default());

        let mut w = WriterQos::default();
        let mut r = ReaderQos::default();
        // §7.4.3 HISTORY
        w.history = HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 999,
        };
        r.history = w.history;
        // §7.4.5 LATENCY_BUDGET
        w.latency_budget = LatencyBudgetQosPolicy {
            duration: Duration::from_millis(250),
        };
        r.latency_budget = w.latency_budget;
        // §7.4.6 OWNERSHIP
        w.ownership = OwnershipQosPolicy {
            kind: OwnershipKind::Exclusive,
        };
        r.ownership = w.ownership;
        // §7.4.9 the rest of the list
        r.time_based_filter = TimeBasedFilterQosPolicy {
            minimum_separation: Duration::from_millis(50),
        };
        w.transport_priority = TransportPriorityQosPolicy { value: 7 };
        w.liveliness = LivelinessQosPolicy {
            kind: LivelinessKind::ManualByTopic,
            lease_duration: Duration::from_millis(1000),
        };
        r.liveliness = w.liveliness;
        w.destination_order = DestinationOrderQosPolicy {
            kind: DestinationOrderKind::BySourceTimestamp,
        };
        r.destination_order = w.destination_order;
        w.resource_limits = ResourceLimitsQosPolicy {
            max_samples: 5,
            max_instances: 5,
            max_samples_per_instance: 5,
        };
        r.resource_limits = w.resource_limits;

        assert_eq!(
            dds_qos_to_amqp_behavior(&w, &r),
            base,
            "no DDS-side policy may reach the AMQP boundary"
        );

        // The three that do cross, to show the comparison is not vacuous.
        let mut crossing = WriterQos::default();
        crossing.durability.kind = DurabilityKind::TransientLocal;
        assert_ne!(
            dds_qos_to_amqp_behavior(&crossing, &ReaderQos::default()),
            base
        );
    }

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
