// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Verifikation der Default-Werte gegen DDS 1.4 §2.2.3.
//!
//! Jede Policy hat einen Spec-Default; dieser Test-Satz stellt sicher,
//! dass unsere `Default::default()`-Impls den Spec-Defaults entsprechen.
//! Fehlerhafte Defaults sind eine haeufige Interop-Quelle (siehe Memory:
//! "Spec-Treue schlaegt Diff-Groesse" — Core-Changes willkommen, wenn sie
//! das Datenmodell spec-treu machen).
//!
//! Die Tests hier sind Reine-Verifikation und wollen keine Logik; sie
//! dokumentieren die Spec-Paragraphen im Klartext.

#[cfg(test)]
#[allow(clippy::bool_assert_comparison)]
mod tests {
    use crate::duration::Duration;
    use crate::policies::resource_limits::LENGTH_UNLIMITED;
    use crate::policies::{
        DeadlineQosPolicy, DestinationOrderKind, DestinationOrderQosPolicy, DurabilityKind,
        DurabilityQosPolicy, DurabilityServiceQosPolicy, EntityFactoryQosPolicy,
        GroupDataQosPolicy, HistoryKind, HistoryQosPolicy, LatencyBudgetQosPolicy,
        LifespanQosPolicy, LivelinessKind, LivelinessQosPolicy, OwnershipKind, OwnershipQosPolicy,
        OwnershipStrengthQosPolicy, PartitionQosPolicy, PresentationAccessScope,
        PresentationQosPolicy, ReaderDataLifecycleQosPolicy, ReliabilityKind, ReliabilityQosPolicy,
        ResourceLimitsQosPolicy, TimeBasedFilterQosPolicy, TopicDataQosPolicy,
        TransportPriorityQosPolicy, UserDataQosPolicy, WriterDataLifecycleQosPolicy,
    };

    // §2.2.3.1-3 UserData/TopicData/GroupData: leere bytes.
    #[test]
    fn user_data_default_is_empty() {
        assert!(UserDataQosPolicy::default().value.is_empty());
    }
    #[test]
    fn topic_data_default_is_empty() {
        assert!(TopicDataQosPolicy::default().value.is_empty());
    }
    #[test]
    fn group_data_default_is_empty() {
        assert!(GroupDataQosPolicy::default().value.is_empty());
    }

    // §2.2.3.4 Durability: VOLATILE.
    #[test]
    fn durability_default_volatile() {
        assert_eq!(
            DurabilityQosPolicy::default().kind,
            DurabilityKind::Volatile
        );
    }

    // §2.2.3.5 DurabilityService: cleanup_delay=0, KeepLast(1), unlimited.
    #[test]
    fn durability_service_defaults_match_spec() {
        let d = DurabilityServiceQosPolicy::default();
        assert_eq!(d.service_cleanup_delay, Duration::ZERO);
        assert_eq!(d.history_kind, HistoryKind::KeepLast);
        assert_eq!(d.history_depth, 1);
        assert_eq!(d.max_samples, LENGTH_UNLIMITED);
        assert_eq!(d.max_instances, LENGTH_UNLIMITED);
        assert_eq!(d.max_samples_per_instance, LENGTH_UNLIMITED);
    }

    // §2.2.3.6 Presentation: Instance, coherent=false, ordered=false.
    #[test]
    fn presentation_default_instance_no_flags() {
        let d = PresentationQosPolicy::default();
        assert_eq!(d.access_scope, PresentationAccessScope::Instance);
        assert_eq!(d.coherent_access, false);
        assert_eq!(d.ordered_access, false);
    }

    // §2.2.3.7 Deadline: INFINITE.
    #[test]
    fn deadline_default_infinite() {
        assert!(DeadlineQosPolicy::default().period.is_infinite());
    }

    // §2.2.3.10 LatencyBudget: ZERO.
    #[test]
    fn latency_budget_default_zero() {
        assert_eq!(LatencyBudgetQosPolicy::default().duration, Duration::ZERO);
    }

    // §2.2.3.11 Liveliness: Automatic, lease=INFINITE.
    #[test]
    fn liveliness_default_automatic_infinite() {
        let d = LivelinessQosPolicy::default();
        assert_eq!(d.kind, LivelinessKind::Automatic);
        assert!(d.lease_duration.is_infinite());
    }

    // §2.2.3.12 TimeBasedFilter: ZERO.
    #[test]
    fn time_based_filter_default_zero() {
        assert_eq!(
            TimeBasedFilterQosPolicy::default().minimum_separation,
            Duration::ZERO
        );
    }

    // §2.2.3.13 Partition: empty.
    #[test]
    fn partition_default_empty() {
        assert!(PartitionQosPolicy::default().names.is_empty());
    }

    // §2.2.3.14 Reliability: BestEffort (Reader-default). Writer setzt
    // ::Reliable explizit.
    #[test]
    fn reliability_default_best_effort_100ms() {
        let d = ReliabilityQosPolicy::default();
        assert_eq!(d.kind, ReliabilityKind::BestEffort);
        assert_eq!(d.max_blocking_time, Duration::from_millis(100));
    }

    // §2.2.3.15 TransportPriority: 0.
    #[test]
    fn transport_priority_default_zero() {
        assert_eq!(TransportPriorityQosPolicy::default().value, 0);
    }

    // §2.2.3.16 Lifespan: INFINITE.
    #[test]
    fn lifespan_default_infinite() {
        assert!(LifespanQosPolicy::default().duration.is_infinite());
    }

    // §2.2.3.17 History: KeepLast, depth=1.
    #[test]
    fn history_default_keep_last_1() {
        let d = HistoryQosPolicy::default();
        assert_eq!(d.kind, HistoryKind::KeepLast);
        assert_eq!(d.depth, 1);
    }

    // §2.2.3.18 DestinationOrder: ByReceptionTimestamp.
    #[test]
    fn destination_order_default_by_reception() {
        assert_eq!(
            DestinationOrderQosPolicy::default().kind,
            DestinationOrderKind::ByReceptionTimestamp
        );
    }

    // §2.2.3.19 ResourceLimits: unlimited.
    #[test]
    fn resource_limits_default_unlimited() {
        let d = ResourceLimitsQosPolicy::default();
        assert_eq!(d.max_samples, LENGTH_UNLIMITED);
        assert_eq!(d.max_instances, LENGTH_UNLIMITED);
        assert_eq!(d.max_samples_per_instance, LENGTH_UNLIMITED);
    }

    // §2.2.3.20 ReaderDataLifecycle: beide INFINITE.
    #[test]
    fn reader_data_lifecycle_default_infinite() {
        let d = ReaderDataLifecycleQosPolicy::default();
        assert!(d.autopurge_nowriter_samples_delay.is_infinite());
        assert!(d.autopurge_disposed_samples_delay.is_infinite());
    }

    // §2.2.3.21 WriterDataLifecycle: autodispose=true.
    #[test]
    fn writer_data_lifecycle_default_autodispose() {
        assert!(WriterDataLifecycleQosPolicy::default().autodispose_unregistered_instances);
    }

    // §2.2.3.23 Ownership: Shared.
    #[test]
    fn ownership_default_shared() {
        assert_eq!(OwnershipQosPolicy::default().kind, OwnershipKind::Shared);
    }

    // §2.2.3.24 OwnershipStrength: 0.
    #[test]
    fn ownership_strength_default_zero() {
        assert_eq!(OwnershipStrengthQosPolicy::default().value, 0);
    }

    // §2.2.3.27 EntityFactory: autoenable=true.
    #[test]
    fn entity_factory_default_autoenable() {
        assert!(EntityFactoryQosPolicy::default().autoenable_created_entities);
    }
}
