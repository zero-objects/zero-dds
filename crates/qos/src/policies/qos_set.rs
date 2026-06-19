// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Writer/Reader QoS aggregates and an aggregate compatibility check.

use super::data_lifecycle::{ReaderDataLifecycleQosPolicy, WriterDataLifecycleQosPolicy};
use super::deadline::DeadlineQosPolicy;
use super::destination_order::DestinationOrderQosPolicy;
use super::durability::DurabilityQosPolicy;
use super::durability_service::DurabilityServiceQosPolicy;
use super::generic_data::{GroupDataQosPolicy, TopicDataQosPolicy, UserDataQosPolicy};
use super::history::HistoryQosPolicy;
use super::latency_budget::LatencyBudgetQosPolicy;
use super::lifespan::LifespanQosPolicy;
use super::liveliness::LivelinessQosPolicy;
use super::ownership::OwnershipQosPolicy;
use super::ownership_strength::OwnershipStrengthQosPolicy;
use super::partition::PartitionQosPolicy;
use super::presentation::PresentationQosPolicy;
use super::reliability::ReliabilityQosPolicy;
use super::resource_limits::ResourceLimitsQosPolicy;
use super::time_based_filter::TimeBasedFilterQosPolicy;
use super::transport_priority::TransportPriorityQosPolicy;

/// Sum of all DataWriter QoS policies.
///
/// See DDS 1.4 §2.2.2.4.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriterQos {
    /// DurabilityQosPolicy.
    pub durability: DurabilityQosPolicy,
    /// DurabilityServiceQosPolicy.
    pub durability_service: DurabilityServiceQosPolicy,
    /// DeadlineQosPolicy.
    pub deadline: DeadlineQosPolicy,
    /// LatencyBudgetQosPolicy.
    pub latency_budget: LatencyBudgetQosPolicy,
    /// LivelinessQosPolicy.
    pub liveliness: LivelinessQosPolicy,
    /// ReliabilityQosPolicy.
    pub reliability: ReliabilityQosPolicy,
    /// DestinationOrderQosPolicy.
    pub destination_order: DestinationOrderQosPolicy,
    /// HistoryQosPolicy.
    pub history: HistoryQosPolicy,
    /// ResourceLimitsQosPolicy.
    pub resource_limits: ResourceLimitsQosPolicy,
    /// TransportPriorityQosPolicy.
    pub transport_priority: TransportPriorityQosPolicy,
    /// LifespanQosPolicy.
    pub lifespan: LifespanQosPolicy,
    /// OwnershipQosPolicy.
    pub ownership: OwnershipQosPolicy,
    /// OwnershipStrengthQosPolicy.
    pub ownership_strength: OwnershipStrengthQosPolicy,
    /// PresentationQosPolicy. Spec-wise at the publisher level; here on
    /// the WriterQos for convenience (the WP 2.x DCPS API will split
    /// this correctly by publisher/subscriber).
    pub presentation: PresentationQosPolicy,
    /// PartitionQosPolicy.
    pub partition: PartitionQosPolicy,
    /// WriterDataLifecycle.
    pub writer_data_lifecycle: WriterDataLifecycleQosPolicy,
    /// UserData.
    pub user_data: UserDataQosPolicy,
    /// TopicData.
    pub topic_data: TopicDataQosPolicy,
    /// GroupData.
    pub group_data: GroupDataQosPolicy,
}

impl Default for WriterQos {
    /// Writer-Defaults gem. DDS 1.4 §2.2.3.14.3 (Reliability):
    /// Writer side defaults to **Reliable** (reader stays BestEffort).
    /// All other policies use their policy-specific defaults.
    fn default() -> Self {
        Self {
            durability: DurabilityQosPolicy::default(),
            durability_service: DurabilityServiceQosPolicy::default(),
            deadline: DeadlineQosPolicy::default(),
            latency_budget: LatencyBudgetQosPolicy::default(),
            liveliness: LivelinessQosPolicy::default(),
            reliability: ReliabilityQosPolicy {
                kind: super::reliability::ReliabilityKind::Reliable,
                max_blocking_time: crate::duration::Duration::from_millis(100),
            },
            destination_order: DestinationOrderQosPolicy::default(),
            history: HistoryQosPolicy::default(),
            resource_limits: ResourceLimitsQosPolicy::default(),
            transport_priority: TransportPriorityQosPolicy::default(),
            lifespan: LifespanQosPolicy::default(),
            ownership: OwnershipQosPolicy::default(),
            ownership_strength: OwnershipStrengthQosPolicy::default(),
            presentation: PresentationQosPolicy::default(),
            partition: PartitionQosPolicy::default(),
            writer_data_lifecycle: WriterDataLifecycleQosPolicy::default(),
            user_data: UserDataQosPolicy::default(),
            topic_data: TopicDataQosPolicy::default(),
            group_data: GroupDataQosPolicy::default(),
        }
    }
}

/// Sum of all DataReader QoS policies.
///
/// See DDS 1.4 §2.2.2.5.2.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReaderQos {
    /// DurabilityQosPolicy.
    pub durability: DurabilityQosPolicy,
    /// DeadlineQosPolicy.
    pub deadline: DeadlineQosPolicy,
    /// LatencyBudgetQosPolicy.
    pub latency_budget: LatencyBudgetQosPolicy,
    /// LivelinessQosPolicy.
    pub liveliness: LivelinessQosPolicy,
    /// ReliabilityQosPolicy.
    pub reliability: ReliabilityQosPolicy,
    /// DestinationOrderQosPolicy.
    pub destination_order: DestinationOrderQosPolicy,
    /// HistoryQosPolicy.
    pub history: HistoryQosPolicy,
    /// ResourceLimitsQosPolicy.
    pub resource_limits: ResourceLimitsQosPolicy,
    /// OwnershipQosPolicy.
    pub ownership: OwnershipQosPolicy,
    /// TimeBasedFilterQosPolicy.
    pub time_based_filter: TimeBasedFilterQosPolicy,
    /// PresentationQosPolicy. Spec-wise at the subscriber level; here on
    /// the ReaderQos for convenience (refactoring in WP 2.x DCPS).
    pub presentation: PresentationQosPolicy,
    /// PartitionQosPolicy.
    pub partition: PartitionQosPolicy,
    /// ReaderDataLifecycle.
    pub reader_data_lifecycle: ReaderDataLifecycleQosPolicy,
    /// UserData.
    pub user_data: UserDataQosPolicy,
    /// TopicData.
    pub topic_data: TopicDataQosPolicy,
    /// GroupData.
    pub group_data: GroupDataQosPolicy,
}

// ============================================================================
// Intra-QoS-Consistency-Check
// ============================================================================

/// Intra-QoS consistency error: policies that, taken on their own, must
/// not contradict each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum InconsistentReason {
    /// `HistoryQosPolicy` with Kind=KeepLast and depth <= 0.
    HistoryDepth,
    /// `ResourceLimitsQosPolicy`: max_samples_per_instance > max_samples.
    ResourceLimits,
    /// `TimeBasedFilterQosPolicy.minimum_separation` > `DeadlineQosPolicy.period`
    /// (DDS 1.4 §2.2.3.12.3).
    FilterVsDeadline,
    /// `HistoryQosPolicy` Kind=KeepLast, but `history.depth > resource_limits.max_samples_per_instance`
    /// (DDS 1.4 §2.2.3.19.3 — KeepLast history must not exceed the resource limit).
    HistoryDepthExceedsResourceLimit,
    /// `OwnershipQosPolicy.kind == Exclusive` but `OwnershipStrengthQosPolicy.value`
    /// not set (default 0 is allowed, but only meaningful if explicitly configured).
    /// A marker for tooling, **not** a hard error.
    ExclusiveOwnershipWithoutStrength,
}

impl WriterQos {
    /// Checks local consistency per DDS 1.4 §2.2.3.
    ///
    /// # Errors
    /// `InconsistentReason` on violation.
    pub fn check_consistency(&self) -> Result<(), InconsistentReason> {
        if matches!(self.history.kind, super::history::HistoryKind::KeepLast)
            && self.history.depth <= 0
        {
            return Err(InconsistentReason::HistoryDepth);
        }
        if !self.resource_limits.is_consistent() {
            return Err(InconsistentReason::ResourceLimits);
        }
        // §2.2.3.19.3: KeepLast.depth must not exceed
        // max_samples_per_instance (LENGTH_UNLIMITED=-1 is always ok).
        if matches!(self.history.kind, super::history::HistoryKind::KeepLast)
            && self.resource_limits.max_samples_per_instance >= 0
            && self.history.depth > self.resource_limits.max_samples_per_instance
        {
            return Err(InconsistentReason::HistoryDepthExceedsResourceLimit);
        }
        Ok(())
    }
}

impl ReaderQos {
    /// Checks local consistency per DDS 1.4 §2.2.3.
    ///
    /// # Errors
    /// `InconsistentReason` on violation.
    pub fn check_consistency(&self) -> Result<(), InconsistentReason> {
        if matches!(self.history.kind, super::history::HistoryKind::KeepLast)
            && self.history.depth <= 0
        {
            return Err(InconsistentReason::HistoryDepth);
        }
        if !self.resource_limits.is_consistent() {
            return Err(InconsistentReason::ResourceLimits);
        }
        // §2.2.3.12.3: TimeBasedFilter.minimum_separation must be <=
        // Deadline.period.
        if self.time_based_filter.minimum_separation > self.deadline.period {
            return Err(InconsistentReason::FilterVsDeadline);
        }
        // §2.2.3.19.3: KeepLast.depth must not exceed
        // max_samples_per_instance.
        if matches!(self.history.kind, super::history::HistoryKind::KeepLast)
            && self.resource_limits.max_samples_per_instance >= 0
            && self.history.depth > self.resource_limits.max_samples_per_instance
        {
            return Err(InconsistentReason::HistoryDepthExceedsResourceLimit);
        }
        Ok(())
    }
}

// ============================================================================
// Aggregate Compatibility-Check
// ============================================================================

use alloc::vec::Vec;

use crate::compatibility::{CompatibilityResult, IncompatibleReason};

/// Aggregate Request/Offered-Compatibility-Check.
///
/// Applies all policy-specific `is_compatible_with` rules and collects
/// the incompatibility reasons. Corresponds to DDS 1.4 §2.2.3 table
/// "QoS compatibility".
#[must_use]
pub fn check_compatibility(offered: &WriterQos, requested: &ReaderQos) -> CompatibilityResult {
    let mut reasons: Vec<IncompatibleReason> = Vec::new();

    if !offered.durability.is_compatible_with(requested.durability) {
        reasons.push(IncompatibleReason::Durability);
    }
    if !offered
        .reliability
        .is_compatible_with(requested.reliability)
    {
        reasons.push(IncompatibleReason::Reliability);
    }
    if !offered.deadline.is_compatible_with(requested.deadline) {
        reasons.push(IncompatibleReason::Deadline);
    }
    if !offered
        .latency_budget
        .is_compatible_with(requested.latency_budget)
    {
        reasons.push(IncompatibleReason::LatencyBudget);
    }
    if !offered.liveliness.is_compatible_with(requested.liveliness) {
        reasons.push(IncompatibleReason::Liveliness);
    }
    if !offered
        .destination_order
        .is_compatible_with(requested.destination_order)
    {
        reasons.push(IncompatibleReason::DestinationOrder);
    }
    if !offered
        .presentation
        .is_compatible_with(requested.presentation)
    {
        reasons.push(IncompatibleReason::Presentation);
    }
    if !offered.ownership.is_compatible_with(requested.ownership) {
        reasons.push(IncompatibleReason::Ownership);
    }
    if !offered.partition.is_compatible_with(&requested.partition) {
        reasons.push(IncompatibleReason::Partition);
    }

    CompatibilityResult::from_reasons(reasons)
}

#[cfg(test)]
#[allow(clippy::panic, clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::duration::Duration;
    use crate::policies::{DurabilityKind, ReliabilityKind};

    #[test]
    fn defaults_are_compatible() {
        let writer = WriterQos::default();
        let reader = ReaderQos::default();
        assert!(check_compatibility(&writer, &reader).is_compatible());
    }

    #[test]
    fn reliable_writer_besteffort_reader_is_compatible() {
        let mut writer = WriterQos::default();
        writer.reliability.kind = ReliabilityKind::Reliable;
        let reader = ReaderQos::default();
        assert!(check_compatibility(&writer, &reader).is_compatible());
    }

    #[test]
    fn besteffort_writer_reliable_reader_is_incompatible() {
        let mut writer = WriterQos::default();
        writer.reliability.kind = ReliabilityKind::BestEffort;
        let mut reader = ReaderQos::default();
        reader.reliability.kind = ReliabilityKind::Reliable;
        let res = check_compatibility(&writer, &reader);
        assert!(!res.is_compatible());
        match res {
            CompatibilityResult::Incompatible(reasons) => {
                assert!(reasons.contains(&IncompatibleReason::Reliability));
            }
            CompatibilityResult::Compatible => panic!("expected incompatible"),
        }
    }

    #[test]
    fn volatile_writer_transient_local_reader_fails_durability() {
        let writer = WriterQos::default();
        let mut reader = ReaderQos::default();
        reader.durability.kind = DurabilityKind::TransientLocal;
        let res = check_compatibility(&writer, &reader);
        assert!(!res.is_compatible());
        match res {
            CompatibilityResult::Incompatible(reasons) => {
                assert!(reasons.contains(&IncompatibleReason::Durability));
            }
            CompatibilityResult::Compatible => panic!("expected incompatible"),
        }
    }

    #[test]
    fn multiple_incompatibilities_are_all_reported() {
        let mut writer = WriterQos::default();
        writer.reliability.kind = ReliabilityKind::BestEffort;
        let mut reader = ReaderQos::default();
        reader.durability.kind = DurabilityKind::Transient;
        reader.reliability.kind = ReliabilityKind::Reliable;
        reader.deadline.period = Duration::ZERO;
        let res = check_compatibility(&writer, &reader);
        match res {
            CompatibilityResult::Incompatible(reasons) => {
                assert!(reasons.contains(&IncompatibleReason::Durability));
                assert!(reasons.contains(&IncompatibleReason::Reliability));
                assert!(reasons.contains(&IncompatibleReason::Deadline));
            }
            CompatibilityResult::Compatible => panic!("expected incompatible"),
        }
    }

    // ============================================================
    // Intra-QoS-Consistency — WriterQos::check_consistency
    // ============================================================

    use super::super::history::HistoryKind;
    use super::super::resource_limits::ResourceLimitsQosPolicy;

    /// The WriterQos default must be consistent (regression guard).
    #[test]
    fn writer_default_is_consistent() {
        assert!(WriterQos::default().check_consistency().is_ok());
    }

    /// HistoryDepth <= 0 with KeepLast violates §2.2.3.17.3.
    #[test]
    fn writer_keep_last_zero_depth_inconsistent() {
        let mut w = WriterQos::default();
        w.history.kind = HistoryKind::KeepLast;
        w.history.depth = 0;
        assert_eq!(w.check_consistency(), Err(InconsistentReason::HistoryDepth));
        w.history.depth = -1;
        assert_eq!(w.check_consistency(), Err(InconsistentReason::HistoryDepth));
    }

    /// KeepAll with depth=0 is permitted (depth only relevant for KeepLast).
    #[test]
    fn writer_keep_all_depth_zero_is_ok() {
        let mut w = WriterQos::default();
        w.history.kind = HistoryKind::KeepAll;
        w.history.depth = 0;
        assert!(w.check_consistency().is_ok());
    }

    /// ResourceLimits inconsistent (per_instance > total).
    #[test]
    fn writer_resource_limits_inconsistent() {
        let mut w = WriterQos::default();
        w.resource_limits = ResourceLimitsQosPolicy {
            max_samples: 10,
            max_instances: 5,
            max_samples_per_instance: 100,
        };
        assert_eq!(
            w.check_consistency(),
            Err(InconsistentReason::ResourceLimits)
        );
    }

    // ============================================================
    // Intra-QoS-Consistency — ReaderQos::check_consistency
    // ============================================================

    /// The ReaderQos default is consistent.
    #[test]
    fn reader_default_is_consistent() {
        assert!(ReaderQos::default().check_consistency().is_ok());
    }

    /// Reader HistoryDepth rule identical to writer.
    #[test]
    fn reader_keep_last_negative_depth_inconsistent() {
        let mut r = ReaderQos::default();
        r.history.kind = HistoryKind::KeepLast;
        r.history.depth = -7;
        assert_eq!(r.check_consistency(), Err(InconsistentReason::HistoryDepth));
    }

    /// Reader ResourceLimits rule.
    #[test]
    fn reader_resource_limits_inconsistent() {
        let mut r = ReaderQos::default();
        r.resource_limits = ResourceLimitsQosPolicy {
            max_samples: 2,
            max_instances: 1,
            max_samples_per_instance: 50,
        };
        assert_eq!(
            r.check_consistency(),
            Err(InconsistentReason::ResourceLimits)
        );
    }

    /// §2.2.3.12.3: TimeBasedFilter.minimum_separation > Deadline.period
    /// → FilterVsDeadline.
    #[test]
    fn reader_filter_gt_deadline_inconsistent() {
        let mut r = ReaderQos::default();
        r.deadline.period = Duration::from_millis(100);
        r.time_based_filter.minimum_separation = Duration::from_millis(500);
        assert_eq!(
            r.check_consistency(),
            Err(InconsistentReason::FilterVsDeadline)
        );
    }

    /// FilterVsDeadline boundary: equal size → ok (`>` comparison).
    #[test]
    fn reader_filter_eq_deadline_is_ok() {
        let mut r = ReaderQos::default();
        r.deadline.period = Duration::from_millis(250);
        r.time_based_filter.minimum_separation = Duration::from_millis(250);
        assert!(r.check_consistency().is_ok());
    }

    // ============================================================
    // Aggregate compatibility — additional reason combinations
    // ============================================================

    /// LatencyBudget offered.duration > requested.duration → inkompatibel.
    #[test]
    fn latency_budget_mismatch_reported() {
        let mut writer = WriterQos::default();
        writer.latency_budget.duration = Duration::from_millis(100);
        let mut reader = ReaderQos::default();
        reader.latency_budget.duration = Duration::from_millis(10);
        let res = check_compatibility(&writer, &reader);
        match res {
            CompatibilityResult::Incompatible(reasons) => {
                assert!(reasons.contains(&IncompatibleReason::LatencyBudget));
            }
            CompatibilityResult::Compatible => panic!("expected LatencyBudget mismatch"),
        }
    }

    /// Liveliness offered.lease_duration > requested.lease_duration →
    /// incompatible (offered must react faster or equally).
    #[test]
    fn liveliness_lease_mismatch_reported() {
        use super::super::liveliness::LivelinessKind;
        let mut writer = WriterQos::default();
        writer.liveliness.kind = LivelinessKind::Automatic;
        writer.liveliness.lease_duration = Duration::from_secs(10);
        let mut reader = ReaderQos::default();
        reader.liveliness.kind = LivelinessKind::Automatic;
        reader.liveliness.lease_duration = Duration::from_secs(1);
        let res = check_compatibility(&writer, &reader);
        match res {
            CompatibilityResult::Incompatible(reasons) => {
                assert!(reasons.contains(&IncompatibleReason::Liveliness));
            }
            CompatibilityResult::Compatible => panic!("expected Liveliness mismatch"),
        }
    }

    /// DestinationOrder offered < requested (Reception < Source) →
    /// inkompatibel.
    #[test]
    fn destination_order_mismatch_reported() {
        use super::super::destination_order::DestinationOrderKind;
        let writer = WriterQos::default();
        let mut reader = ReaderQos::default();
        reader.destination_order.kind = DestinationOrderKind::BySourceTimestamp;
        let res = check_compatibility(&writer, &reader);
        match res {
            CompatibilityResult::Incompatible(reasons) => {
                assert!(reasons.contains(&IncompatibleReason::DestinationOrder));
            }
            CompatibilityResult::Compatible => panic!("expected DestinationOrder mismatch"),
        }
    }

    /// Ownership mismatch (SHARED vs EXCLUSIVE) → inkompatibel.
    #[test]
    fn ownership_mismatch_reported() {
        use super::super::ownership::OwnershipKind;
        let writer = WriterQos::default();
        let mut reader = ReaderQos::default();
        reader.ownership.kind = OwnershipKind::Exclusive;
        let res = check_compatibility(&writer, &reader);
        match res {
            CompatibilityResult::Incompatible(reasons) => {
                assert!(reasons.contains(&IncompatibleReason::Ownership));
            }
            CompatibilityResult::Compatible => panic!("expected Ownership mismatch"),
        }
    }

    /// Partition mismatch: writer `alpha`, reader `beta`.
    #[test]
    fn partition_mismatch_reported() {
        use alloc::string::ToString;
        let mut writer = WriterQos::default();
        writer.partition.names.push("alpha".to_string());
        let mut reader = ReaderQos::default();
        reader.partition.names.push("beta".to_string());
        let res = check_compatibility(&writer, &reader);
        match res {
            CompatibilityResult::Incompatible(reasons) => {
                assert!(reasons.contains(&IncompatibleReason::Partition));
            }
            CompatibilityResult::Compatible => panic!("expected Partition mismatch"),
        }
    }

    /// Presentation mismatch: Writer INSTANCE-access, Reader TOPIC-access
    /// (spec §2.2.3.6: requested access_scope > offered → inkompatibel).
    #[test]
    fn presentation_mismatch_reported() {
        use super::super::presentation::PresentationAccessScope;
        let writer = WriterQos::default();
        let mut reader = ReaderQos::default();
        reader.presentation.access_scope = PresentationAccessScope::Topic;
        reader.presentation.coherent_access = true;
        let res = check_compatibility(&writer, &reader);
        match res {
            CompatibilityResult::Incompatible(reasons) => {
                assert!(reasons.contains(&IncompatibleReason::Presentation));
            }
            CompatibilityResult::Compatible => panic!("expected Presentation mismatch"),
        }
    }

    /// Debug/Clone for the aggregate (trivial, but coverage: struct builder).
    #[test]
    fn writer_qos_clone_and_eq() {
        let a = WriterQos::default();
        let b = a.clone();
        assert_eq!(a, b);
    }
}
