// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Request/offered QoS compatibility (DDS 1.4 §2.2.3).
//!
//! Each policy that is set on both the DataWriter and DataReader side
//! has, in the DDS sense, a *compatibility rule*: the writer
//! "offers" a value, the reader "requests" a value, and the
//! matching procedure compares the two. Failure of such checks triggers
//! `OFFERED_INCOMPATIBLE_QOS`/`REQUESTED_INCOMPATIBLE_QOS` listener events.

use alloc::vec::Vec;

use crate::policies::{
    DeadlineQosPolicy, DestinationOrderQosPolicy, DurabilityQosPolicy, LatencyBudgetQosPolicy,
    LivelinessQosPolicy, OwnershipQosPolicy, PartitionQosPolicy, PresentationQosPolicy, ReaderQos,
    ReliabilityQosPolicy, WriterQos,
};

/// A single reason why writer QoS and reader QoS do not match.
///
/// The variants correspond to the `QosPolicyId_t` enumeration from DDS 1.4
/// §2.2.3, restricted to policies with request/offered semantics.
///
/// `Ord`/`PartialOrd` by declaration order — enables stable
/// reports in [`CompatibilityResult::from_reasons`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum IncompatibleReason {
    /// `DurabilityQosPolicy` — offered < requested.
    Durability,
    /// `ReliabilityQosPolicy` — BestEffort offered, Reliable requested.
    Reliability,
    /// `DeadlineQosPolicy` — offered > requested.
    Deadline,
    /// `LatencyBudgetQosPolicy` — offered > requested.
    LatencyBudget,
    /// `LivelinessQosPolicy` — offered kind < requested kind, or
    /// offered lease_duration > requested.
    Liveliness,
    /// `DestinationOrderQosPolicy` — offered < requested.
    DestinationOrder,
    /// `PresentationQosPolicy` — scope too weak or coherent/ordered
    /// not covered.
    Presentation,
    /// `OwnershipQosPolicy` — Shared/Exclusive must match.
    Ownership,
    /// `PartitionQosPolicy` — no partition overlap.
    Partition,
}

/// Result of a compatibility check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompatibilityResult {
    /// The QoS sets are compatible.
    Compatible,
    /// The QoS sets are incompatible, with a reason list.
    Incompatible(Vec<IncompatibleReason>),
}

impl CompatibilityResult {
    /// `true` if the QoS sets are compatible.
    #[must_use]
    pub fn is_compatible(&self) -> bool {
        matches!(self, Self::Compatible)
    }

    /// Build from a reason list. Deduplicates and sorts by
    /// canonical discriminator order — so log output is
    /// stable and tests can compare with `==` instead of `.contains`.
    /// Empty list ⇒ `Compatible`.
    #[must_use]
    pub fn from_reasons(mut reasons: Vec<IncompatibleReason>) -> Self {
        if reasons.is_empty() {
            return Self::Compatible;
        }
        // Stable sort + dedup. The Ord impl comes from the derive; the
        // variant order is the canonical sort order.
        reasons.sort();
        reasons.dedup();
        Self::Incompatible(reasons)
    }
}

// ============================================================================
// Per-policy checks. Signature: (offered, requested) -> bool (true = ok).
// ============================================================================

impl DurabilityQosPolicy {
    /// §2.2.3 Table: `offered.kind >= requested.kind`.
    #[must_use]
    pub fn is_compatible_with(self, requested: Self) -> bool {
        self.kind >= requested.kind
    }
}

impl ReliabilityQosPolicy {
    /// §2.2.3 Table: `offered.kind >= requested.kind`. Kind ordering
    /// `BestEffort < Reliable`.
    #[must_use]
    pub fn is_compatible_with(self, requested: Self) -> bool {
        self.kind >= requested.kind
    }
}

impl DeadlineQosPolicy {
    /// §2.2.3.7.4: `offered.period <= requested.period` (the writer can
    /// deliver at least as often as the reader requires).
    #[must_use]
    pub fn is_compatible_with(self, requested: Self) -> bool {
        self.period <= requested.period
    }
}

impl LatencyBudgetQosPolicy {
    /// §2.2.3.10.4: `offered.duration <= requested.duration` (the writer
    /// promises at least as fast as the reader can tolerate).
    #[must_use]
    pub fn is_compatible_with(self, requested: Self) -> bool {
        self.duration <= requested.duration
    }
}

impl LivelinessQosPolicy {
    /// §2.2.3.11.4:
    /// - `offered.kind >= requested.kind`, AND
    /// - `offered.lease_duration <= requested.lease_duration`.
    #[must_use]
    pub fn is_compatible_with(self, requested: Self) -> bool {
        self.kind >= requested.kind && self.lease_duration <= requested.lease_duration
    }
}

impl DestinationOrderQosPolicy {
    /// §2.2.3.18.3: `offered.kind >= requested.kind`.
    #[must_use]
    pub fn is_compatible_with(self, requested: Self) -> bool {
        self.kind >= requested.kind
    }
}

impl OwnershipQosPolicy {
    /// §2.2.3.23: `offered.kind == requested.kind`. No ordering.
    #[must_use]
    pub fn is_compatible_with(self, requested: Self) -> bool {
        self.kind == requested.kind
    }
}

impl PresentationQosPolicy {
    /// §2.2.3.6.6:
    /// - `offered.access_scope >= requested.access_scope`, AND
    /// - `offered.coherent_access >= requested.coherent_access`, AND
    /// - `offered.ordered_access >= requested.ordered_access`.
    ///
    /// For bool, `true >= false` holds.
    #[must_use]
    pub fn is_compatible_with(self, requested: Self) -> bool {
        self.access_scope >= requested.access_scope
            && (self.coherent_access || !requested.coherent_access)
            && (self.ordered_access || !requested.ordered_access)
    }
}

// ============================================================================
// Aggregate: all request/offered policies in a single call
// ============================================================================

/// Full DataWriter↔DataReader compatibility check.
///
/// WP 2.8 (C2.8) — combines all 9 per-policy checks into a
/// single call. The caller (DCPS match path in publisher.rs /
/// subscriber.rs) calls this before allowing the pairing; on
/// incompatibility the listener statuses
/// `OFFERED_INCOMPATIBLE_QOS` (on the writer side) and
/// `REQUESTED_INCOMPATIBLE_QOS` (on the reader side) are fired.
///
/// Spec references: DDS 1.4 §2.2.3 compatibility tables,
/// §2.2.4.1 OFFERED_INCOMPATIBLE_QOS_STATUS / REQUESTED_INCOMPATIBLE_QOS_STATUS.
#[must_use]
pub fn compute_compatibility(offered: &WriterQos, requested: &ReaderQos) -> CompatibilityResult {
    let mut reasons = Vec::new();
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

impl PartitionQosPolicy {
    /// §2.2.3.13.6: there must be at least one common partition name.
    /// Matching is **fnmatch-glob-based** (`*`, `?`, `[...]`):
    /// an offered pattern can match a requested name or vice versa.
    ///
    /// Empty/empty matches (default partition). Empty vs non-empty
    /// does **not** match (spec-conformant: the default partition is a
    /// separate namespace).
    #[must_use]
    pub fn is_compatible_with(&self, requested: &Self) -> bool {
        if self.names.is_empty() && requested.names.is_empty() {
            return true;
        }
        if self.names.is_empty() || requested.names.is_empty() {
            return false;
        }
        // fnmatch is symmetrically relevant: either the offered pattern
        // matches the requested text or vice versa.
        self.names.iter().any(|o| {
            requested.names.iter().any(|rq| {
                super::policies::partition::fnmatch(o, rq)
                    || super::policies::partition::fnmatch(rq, o)
            })
        })
    }
}

#[cfg(test)]
#[allow(clippy::bool_assert_comparison, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::duration::Duration;
    use crate::policies::{
        DestinationOrderKind, DurabilityKind, LivelinessKind, OwnershipKind,
        PresentationAccessScope, ReliabilityKind,
    };

    #[test]
    fn empty_reasons_is_compatible() {
        let r = CompatibilityResult::from_reasons(Vec::new());
        assert_eq!(r, CompatibilityResult::Compatible);
        assert!(r.is_compatible());
    }

    #[test]
    fn non_empty_reasons_is_incompatible() {
        let r = CompatibilityResult::from_reasons(alloc::vec![IncompatibleReason::Durability]);
        assert!(!r.is_compatible());
    }

    #[test]
    fn durability_offered_ge_requested() {
        let offered = DurabilityQosPolicy {
            kind: DurabilityKind::Transient,
        };
        let req = DurabilityQosPolicy {
            kind: DurabilityKind::TransientLocal,
        };
        assert!(offered.is_compatible_with(req));
        // Not the other way around.
        assert!(!req.is_compatible_with(offered));
    }

    #[test]
    fn reliability_reliable_offered_besteffort_requested_ok() {
        let offered = ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: Duration::ZERO,
        };
        let req = ReliabilityQosPolicy {
            kind: ReliabilityKind::BestEffort,
            max_blocking_time: Duration::ZERO,
        };
        assert!(offered.is_compatible_with(req));
        assert!(!req.is_compatible_with(offered));
    }

    #[test]
    fn deadline_offered_le_requested() {
        let offered = DeadlineQosPolicy {
            period: Duration::from_secs(1),
        };
        let req = DeadlineQosPolicy {
            period: Duration::from_secs(5),
        };
        assert!(offered.is_compatible_with(req));
        assert!(!req.is_compatible_with(offered));
    }

    #[test]
    fn latency_budget_offered_le_requested() {
        let offered = LatencyBudgetQosPolicy {
            duration: Duration::from_millis(10),
        };
        let req = LatencyBudgetQosPolicy {
            duration: Duration::from_millis(100),
        };
        assert!(offered.is_compatible_with(req));
        assert!(!req.is_compatible_with(offered));
    }

    #[test]
    fn liveliness_kind_and_lease_checked() {
        let offered = LivelinessQosPolicy {
            kind: LivelinessKind::ManualByTopic,
            lease_duration: Duration::from_secs(1),
        };
        let req = LivelinessQosPolicy {
            kind: LivelinessKind::ManualByParticipant,
            lease_duration: Duration::from_secs(5),
        };
        assert!(offered.is_compatible_with(req));

        // Lease zu lang ⇒ fail.
        let req_strict = LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            lease_duration: Duration::ZERO,
        };
        assert!(!offered.is_compatible_with(req_strict));
    }

    #[test]
    fn destination_order_offered_ge_requested() {
        let offered = DestinationOrderQosPolicy {
            kind: DestinationOrderKind::BySourceTimestamp,
        };
        let req = DestinationOrderQosPolicy {
            kind: DestinationOrderKind::ByReceptionTimestamp,
        };
        assert!(offered.is_compatible_with(req));
        assert!(!req.is_compatible_with(offered));
    }

    #[test]
    fn ownership_must_match_exactly() {
        let a = OwnershipQosPolicy {
            kind: OwnershipKind::Shared,
        };
        let b = OwnershipQosPolicy {
            kind: OwnershipKind::Exclusive,
        };
        assert!(a.is_compatible_with(a));
        assert!(!a.is_compatible_with(b));
    }

    #[test]
    fn presentation_scope_and_flags() {
        let offered = PresentationQosPolicy {
            access_scope: PresentationAccessScope::Group,
            coherent_access: true,
            ordered_access: true,
        };
        let req = PresentationQosPolicy {
            access_scope: PresentationAccessScope::Topic,
            coherent_access: false,
            ordered_access: true,
        };
        assert!(offered.is_compatible_with(req));

        // Reader requests coherent, writer does not offer ⇒ fail.
        let offered_weak = PresentationQosPolicy {
            access_scope: PresentationAccessScope::Group,
            coherent_access: false,
            ordered_access: true,
        };
        let req_coherent = PresentationQosPolicy {
            access_scope: PresentationAccessScope::Instance,
            coherent_access: true,
            ordered_access: false,
        };
        assert!(!offered_weak.is_compatible_with(req_coherent));
    }

    #[test]
    fn partition_exact_match() {
        use alloc::string::String;
        let offered = PartitionQosPolicy {
            names: alloc::vec![String::from("a"), String::from("b")],
        };
        let req = PartitionQosPolicy {
            names: alloc::vec![String::from("c"), String::from("b")],
        };
        assert!(offered.is_compatible_with(&req));

        let req_disjoint = PartitionQosPolicy {
            names: alloc::vec![String::from("c"), String::from("d")],
        };
        assert!(!offered.is_compatible_with(&req_disjoint));
    }

    #[test]
    fn partition_empty_match_empty() {
        let a = PartitionQosPolicy::default();
        let b = PartitionQosPolicy::default();
        assert!(a.is_compatible_with(&b));
    }

    // ========================================================================
    // WP 2.8: compute_compatibility (aggregate of all 9 policy checks)
    // ========================================================================

    #[test]
    fn compute_compatibility_default_writer_reader_is_compatible() {
        let w = crate::policies::WriterQos::default();
        let r = crate::policies::ReaderQos::default();
        let result = compute_compatibility(&w, &r);
        // Default writer is Reliable, default reader BestEffort → Reliable >= BestEffort
        assert!(result.is_compatible(), "got {result:?}");
    }

    #[test]
    fn compute_compatibility_reports_durability_mismatch() {
        let mut w = crate::policies::WriterQos::default();
        let mut r = crate::policies::ReaderQos::default();
        w.durability = DurabilityQosPolicy {
            kind: DurabilityKind::Volatile,
        };
        r.durability = DurabilityQosPolicy {
            kind: DurabilityKind::Transient,
        };
        let result = compute_compatibility(&w, &r);
        assert!(!result.is_compatible());
        if let CompatibilityResult::Incompatible(reasons) = result {
            assert!(reasons.contains(&IncompatibleReason::Durability));
        }
    }

    #[test]
    fn compute_compatibility_reports_multiple_mismatches() {
        let mut w = crate::policies::WriterQos::default();
        let mut r = crate::policies::ReaderQos::default();
        // Reliability: Writer BestEffort, Reader Reliable → fail
        w.reliability.kind = ReliabilityKind::BestEffort;
        r.reliability.kind = ReliabilityKind::Reliable;
        // Durability: Writer Volatile, Reader Transient → fail
        w.durability = DurabilityQosPolicy {
            kind: DurabilityKind::Volatile,
        };
        r.durability = DurabilityQosPolicy {
            kind: DurabilityKind::Transient,
        };
        let result = compute_compatibility(&w, &r);
        if let CompatibilityResult::Incompatible(reasons) = result {
            assert!(reasons.contains(&IncompatibleReason::Reliability));
            assert!(reasons.contains(&IncompatibleReason::Durability));
            assert!(reasons.len() >= 2);
        } else {
            panic!("expected incompatible");
        }
    }

    #[test]
    fn compute_compatibility_partition_disjoint_fails() {
        use alloc::string::String;
        let mut w = crate::policies::WriterQos::default();
        let mut r = crate::policies::ReaderQos::default();
        w.partition = PartitionQosPolicy {
            names: alloc::vec![String::from("alpha")],
        };
        r.partition = PartitionQosPolicy {
            names: alloc::vec![String::from("beta")],
        };
        let result = compute_compatibility(&w, &r);
        if let CompatibilityResult::Incompatible(reasons) = result {
            assert!(reasons.contains(&IncompatibleReason::Partition));
        } else {
            panic!("expected partition mismatch");
        }
    }
}
