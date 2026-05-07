// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Request/Offered QoS-Compatibility (DDS 1.4 §2.2.3).
//!
//! Jede Policy, die sowohl auf DataWriter- als auch auf DataReader-Seite
//! gesetzt wird, hat im DDS-Sinne eine *Compatibility-Rule*: der Writer
//! "offers" einen Wert, der Reader "requests" einen Wert, und das
//! Matching-Verfahren vergleicht beide. Scheitern solcher Checks triggert
//! `OFFERED_INCOMPATIBLE_QOS`/`REQUESTED_INCOMPATIBLE_QOS`-Listener-Events.

use alloc::vec::Vec;

use crate::policies::{
    DeadlineQosPolicy, DestinationOrderQosPolicy, DurabilityQosPolicy, LatencyBudgetQosPolicy,
    LivelinessQosPolicy, OwnershipQosPolicy, PartitionQosPolicy, PresentationQosPolicy, ReaderQos,
    ReliabilityQosPolicy, WriterQos,
};

/// Einzelner Grund, warum Writer-QoS und Reader-QoS nicht matchen.
///
/// Die Varianten entsprechen der `QosPolicyId_t`-Enumeration aus DDS 1.4
/// §2.2.3, beschraenkt auf Policies mit Request/Offered-Semantik.
///
/// `Ord`/`PartialOrd` nach Deklarations-Reihenfolge — erlaubt stabile
/// Reports in [`CompatibilityResult::from_reasons`].
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
    /// `LivelinessQosPolicy` — offered Kind < requested Kind oder
    /// lease_duration offered > requested.
    Liveliness,
    /// `DestinationOrderQosPolicy` — offered < requested.
    DestinationOrder,
    /// `PresentationQosPolicy` — Scope zu schwach oder coherent/ordered
    /// nicht abgedeckt.
    Presentation,
    /// `OwnershipQosPolicy` — Shared/Exclusive muessen matchen.
    Ownership,
    /// `PartitionQosPolicy` — keine Partition-Ueberschneidung.
    Partition,
}

/// Ergebnis eines Compatibility-Checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompatibilityResult {
    /// QoS-Sets sind kompatibel.
    Compatible,
    /// QoS-Sets sind nicht kompatibel mit Reason-Liste.
    Incompatible(Vec<IncompatibleReason>),
}

impl CompatibilityResult {
    /// `true` wenn die QoS-Sets kompatibel sind.
    #[must_use]
    pub fn is_compatible(&self) -> bool {
        matches!(self, Self::Compatible)
    }

    /// Aus einer Reason-Liste bauen. Dedupliziert und sortiert nach
    /// kanonischer Discriminator-Reihenfolge — so sind Log-Ausgaben
    /// stabil und Tests koennen per `==` vergleichen statt `.contains`.
    /// Leere Liste ⇒ `Compatible`.
    #[must_use]
    pub fn from_reasons(mut reasons: Vec<IncompatibleReason>) -> Self {
        if reasons.is_empty() {
            return Self::Compatible;
        }
        // Stable sort + dedup. Ord-Impl kommt vom derive; Varianten-
        // Reihenfolge ist die kanonische Sortier-Reihenfolge.
        reasons.sort();
        reasons.dedup();
        Self::Incompatible(reasons)
    }
}

// ============================================================================
// Pro-Policy-Checks. Signatur: (offered, requested) -> bool (true = ok).
// ============================================================================

impl DurabilityQosPolicy {
    /// §2.2.3 Table: `offered.kind >= requested.kind`.
    #[must_use]
    pub fn is_compatible_with(self, requested: Self) -> bool {
        self.kind >= requested.kind
    }
}

impl ReliabilityQosPolicy {
    /// §2.2.3 Table: `offered.kind >= requested.kind`. Kind-Ordering
    /// `BestEffort < Reliable`.
    #[must_use]
    pub fn is_compatible_with(self, requested: Self) -> bool {
        self.kind >= requested.kind
    }
}

impl DeadlineQosPolicy {
    /// §2.2.3.7.4: `offered.period <= requested.period` (Writer kann
    /// mindestens so haeufig liefern wie Reader verlangt).
    #[must_use]
    pub fn is_compatible_with(self, requested: Self) -> bool {
        self.period <= requested.period
    }
}

impl LatencyBudgetQosPolicy {
    /// §2.2.3.10.4: `offered.duration <= requested.duration` (Writer
    /// verspricht mindestens so schnell wie Reader tolerieren kann).
    #[must_use]
    pub fn is_compatible_with(self, requested: Self) -> bool {
        self.duration <= requested.duration
    }
}

impl LivelinessQosPolicy {
    /// §2.2.3.11.4:
    /// - `offered.kind >= requested.kind`, UND
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
    /// §2.2.3.23: `offered.kind == requested.kind`. Kein Ordering.
    #[must_use]
    pub fn is_compatible_with(self, requested: Self) -> bool {
        self.kind == requested.kind
    }
}

impl PresentationQosPolicy {
    /// §2.2.3.6.6:
    /// - `offered.access_scope >= requested.access_scope`, UND
    /// - `offered.coherent_access >= requested.coherent_access`, UND
    /// - `offered.ordered_access >= requested.ordered_access`.
    ///
    /// Fuer bool gilt `true >= false`.
    #[must_use]
    pub fn is_compatible_with(self, requested: Self) -> bool {
        self.access_scope >= requested.access_scope
            && (self.coherent_access || !requested.coherent_access)
            && (self.ordered_access || !requested.ordered_access)
    }
}

// ============================================================================
// Aggregat: alle Request/Offered-Policies in einem Aufruf
// ============================================================================

/// Vollstaendiger DataWriter↔DataReader Compatibility-Check.
///
/// WP 2.8 (C2.8) — kombiniert alle 9 Pro-Policy-Checks zu einem
/// einzigen Aufruf. Caller (DCPS-Match-Pfad in publisher.rs /
/// subscriber.rs) ruft diesen, bevor er das Pairing erlaubt; bei
/// Inkompatibilitaet werden die Listener-Statuses
/// `OFFERED_INCOMPATIBLE_QOS` (auf Writer-Seite) bzw.
/// `REQUESTED_INCOMPATIBLE_QOS` (auf Reader-Seite) gefeuert.
///
/// Spec-Referenzen: DDS 1.4 §2.2.3 Compatibility-Tabellen,
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
    /// §2.2.3.13.6: Es muss mindestens einen gemeinsamen Partition-Namen
    /// geben. Matching ist **fnmatch-Glob-basiert** (`*`, `?`, `[...]`):
    /// offered-Pattern kann requested-Namen matchen oder umgekehrt.
    ///
    /// Leer/Leer matcht (Default-Partition). Leer vs. nicht-leer
    /// matcht **nicht** (spec-konform: Default-Partition ist ein
    /// separater Namensraum).
    #[must_use]
    pub fn is_compatible_with(&self, requested: &Self) -> bool {
        if self.names.is_empty() && requested.names.is_empty() {
            return true;
        }
        if self.names.is_empty() || requested.names.is_empty() {
            return false;
        }
        // fnmatch ist symmetrisch relevant: entweder offered-Pattern
        // matched requested-Text oder umgekehrt.
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
        // Umgekehrt nicht.
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

        // Reader verlangt coherent, Writer bietet nicht ⇒ fail.
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
    // WP 2.8: compute_compatibility (Aggregat aller 9 Policy-Checks)
    // ========================================================================

    #[test]
    fn compute_compatibility_default_writer_reader_is_compatible() {
        let w = crate::policies::WriterQos::default();
        let r = crate::policies::ReaderQos::default();
        let result = compute_compatibility(&w, &r);
        // Default-Writer ist Reliable, Default-Reader BestEffort → Reliable >= BestEffort
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
