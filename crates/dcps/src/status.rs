// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Communication status structures (DDS DCPS 1.4 §2.2.4.1, Table 2.10).
//!
//! The spec defines **13 standard communication statuses**, which are
//! combined into a bitmask in `StatusMask`. Each status has an
//! associated data structure that `get_<status>()` returns on the
//! respective entity. The bitmask constants (linking status ↔ bit) live
//! in [`crate::psm_constants::status`].
//!
//! ## Classification of the 13 statuses (spec §2.2.4.1):
//!
//! - **PLAIN** statuses (only `total_count` + `total_count_change`):
//!   - `INCONSISTENT_TOPIC`         (Topic)
//!   - `SAMPLE_LOST`                (DataReader)
//!   - `LIVELINESS_LOST`            (DataWriter)
//!   - `OFFERED_DEADLINE_MISSED`    (DataWriter)
//!   - `REQUESTED_DEADLINE_MISSED`  (DataReader)
//!
//! - **STATEFUL** statuses (with `last_*` + detail fields):
//!   - `SAMPLE_REJECTED`            (DataReader)
//!   - `LIVELINESS_CHANGED`         (DataReader)
//!   - `PUBLICATION_MATCHED`        (DataWriter)
//!   - `SUBSCRIPTION_MATCHED`       (DataReader)
//!   - `OFFERED_INCOMPATIBLE_QOS`   (DataWriter)
//!   - `REQUESTED_INCOMPATIBLE_QOS` (DataReader)
//!
//! - **SIGNAL** statuses (pure bits, no data structure — we mirror them
//!   as marker structs for completeness of the table and for a uniform
//!   `get_*` API):
//!   - `DATA_AVAILABLE`             (DataReader)
//!   - `DATA_ON_READERS`            (Subscriber)
//!
//! `total_count_change` is typed as `i32`: the spec says "incremental
//! count since the last time the listener was called or the status was
//! read", which can go negative when the reader regains liveliness
//! (LIVELINESS_CHANGED). We keep `i32` for all `*_count_change` fields to
//! stay spec-compliant.

extern crate alloc;

use alloc::vec::Vec;

use crate::instance_handle::InstanceHandle;

// ============================================================================
// Plain counter statuses
// ============================================================================

/// `INCONSISTENT_TOPIC_STATUS` — Spec §2.2.4.1 Tab. 2.10 + §2.2.2.3.2.
///
/// "Another topic exists with the same name but different
/// characteristics." Maintained at the `Topic` level.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InconsistentTopicStatus {
    /// Total cumulative count of inconsistencies detected.
    pub total_count: i32,
    /// Increment since last read.
    pub total_count_change: i32,
}

/// `SAMPLE_LOST_STATUS` — Spec §2.2.4.1 + §2.2.2.5.6.
///
/// "All samples that have been lost (never received) by the
/// DataReader."
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SampleLostStatus {
    /// Total cumulative count of all lost samples.
    pub total_count: i32,
    /// Increment since last read.
    pub total_count_change: i32,
}

/// `LIVELINESS_LOST_STATUS` — Spec §2.2.4.1 + §2.2.2.4.2.
///
/// Counter of how often the DataWriter has been declared "not alive"
/// under the LIVELINESS QoS contract (writer side).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LivelinessLostStatus {
    /// Total cumulative count of times the writer was declared
    /// not-alive.
    pub total_count: i32,
    /// Increment since last read.
    pub total_count_change: i32,
}

/// `OFFERED_DEADLINE_MISSED_STATUS` — Spec §2.2.4.1 + §2.2.2.4.2.
///
/// Counter of how often the writer failed to honor its offered DEADLINE
/// promise. Also maintains the `last_instance_handle` against which the
/// violation was counted.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OfferedDeadlineMissedStatus {
    /// Total cumulative count of offered-deadline misses.
    pub total_count: i32,
    /// Increment since last read.
    pub total_count_change: i32,
    /// Handle of the last instance for which the deadline was missed.
    pub last_instance_handle: InstanceHandle,
}

/// `REQUESTED_DEADLINE_MISSED_STATUS` — Spec §2.2.4.1 + §2.2.2.5.6.
///
/// Reader side. Counter of how often the reader did not receive a sample
/// within the requested DEADLINE.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestedDeadlineMissedStatus {
    /// Total cumulative count of requested-deadline misses.
    pub total_count: i32,
    /// Increment since last read.
    pub total_count_change: i32,
    /// Handle of the last instance for which the deadline was missed.
    pub last_instance_handle: InstanceHandle,
}

// ============================================================================
// Stateful statuses (with `last_*` + detail fields)
// ============================================================================

/// `SampleRejectedStatusKind` — reason why the last sample was rejected.
/// Spec §2.2.4.1 Table 2.10 (kind enum under `SAMPLE_REJECTED`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SampleRejectedStatusKind {
    /// No sample was rejected (default).
    #[default]
    NotRejected,
    /// Reader resource limit `max_instances` exceeded.
    RejectedByInstancesLimit,
    /// Reader resource limit `max_samples` exceeded.
    RejectedBySamplesLimit,
    /// Reader resource limit `max_samples_per_instance` exceeded.
    RejectedBySamplesPerInstanceLimit,
}

/// `SAMPLE_REJECTED_STATUS` — Spec §2.2.4.1 + §2.2.2.5.6.
///
/// Raised when the reader has dropped a sample due to a RESOURCE_LIMITS
/// violation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SampleRejectedStatus {
    /// Total cumulative count of rejected samples.
    pub total_count: i32,
    /// Increment since last read.
    pub total_count_change: i32,
    /// Reason for the most recent rejection.
    pub last_reason: SampleRejectedStatusKind,
    /// Handle of the instance that was the target of the most recent
    /// rejection.
    pub last_instance_handle: InstanceHandle,
}

/// `LIVELINESS_CHANGED_STATUS` — Spec §2.2.4.1 + §2.2.2.5.6.
///
/// Reader-Seite: "Reports the status of the liveliness of one or more
/// `DataWriter` objects that are matched with the `DataReader`."
///
/// Unlike the plain counter statuses, `*_count_change` here may go
/// **negative**, for example when a writer previously declared "alive"
/// now counts as "not_alive" (moved from `alive_count` to
/// `not_alive_count`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LivelinessChangedStatus {
    /// Number of currently-alive matched writers.
    pub alive_count: i32,
    /// Number of currently-not-alive matched writers.
    pub not_alive_count: i32,
    /// Change in `alive_count` since the last read.
    pub alive_count_change: i32,
    /// Change in `not_alive_count` since the last read.
    pub not_alive_count_change: i32,
    /// Handle of the last writer that triggered the change.
    pub last_publication_handle: InstanceHandle,
}

/// `PUBLICATION_MATCHED_STATUS` — Spec §2.2.4.1 + §2.2.2.4.2.
///
/// Writer-Seite: "Reports the discovery of a new compatible
/// DataReader / the loss of one."
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PublicationMatchedStatus {
    /// Total cumulative count of compatible DataReaders that have been
    /// discovered so far (monotonically increasing).
    pub total_count: i32,
    /// Change in `total_count` since the last read.
    pub total_count_change: i32,
    /// Currently matched DataReaders (can drop when a reader leaves).
    pub current_count: i32,
    /// Change in `current_count` since the last read (may go negative).
    pub current_count_change: i32,
    /// Handle of the last DataReader that matched the writer.
    pub last_subscription_handle: InstanceHandle,
}

/// `SUBSCRIPTION_MATCHED_STATUS` — Spec §2.2.4.1 + §2.2.2.5.6.
///
/// Reader side: mirrors PublicationMatched. Fields run in parallel.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionMatchedStatus {
    /// Total cumulative count of compatible DataWriters discovered.
    pub total_count: i32,
    /// Change in `total_count` since last read.
    pub total_count_change: i32,
    /// Currently matched DataWriters.
    pub current_count: i32,
    /// Change in `current_count` since last read.
    pub current_count_change: i32,
    /// Handle of the last DataWriter that matched.
    pub last_publication_handle: InstanceHandle,
}

/// `QosPolicyCount` — sub-element of `*IncompatibleQosStatus`.
///
/// Per QoS policy id, a counter of how often exactly that policy caused
/// an incompatibility. Spec §2.2.4.1 Table 2.10.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QosPolicyCount {
    /// Policy id (see [`crate::psm_constants::qos_policy_id`]).
    pub policy_id: u32,
    /// How often this policy was incompatible.
    pub count: i32,
}

impl QosPolicyCount {
    /// Constructor.
    #[must_use]
    pub const fn new(policy_id: u32, count: i32) -> Self {
        Self { policy_id, count }
    }
}

/// `OFFERED_INCOMPATIBLE_QOS_STATUS` — Spec §2.2.4.1 + §2.2.2.4.2.
///
/// Writer side: a reader was found whose `requested QoS` does not match
/// the writer's `offered QoS`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OfferedIncompatibleQosStatus {
    /// Total cumulative count of incompatible-QoS detections.
    pub total_count: i32,
    /// Change in `total_count` since last read.
    pub total_count_change: i32,
    /// Policy-Id of the *most recent* policy that caused the
    /// incompatibility.
    pub last_policy_id: u32,
    /// Per-policy counters.
    pub policies: Vec<QosPolicyCount>,
}

/// `REQUESTED_INCOMPATIBLE_QOS_STATUS` — Spec §2.2.4.1 + §2.2.2.5.6.
///
/// Reader side: a writer was found whose `offered QoS` does not match
/// the reader's `requested QoS`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RequestedIncompatibleQosStatus {
    /// Total cumulative count of incompatible-QoS detections.
    pub total_count: i32,
    /// Change in `total_count` since last read.
    pub total_count_change: i32,
    /// Policy-Id of the *most recent* policy that caused the
    /// incompatibility.
    pub last_policy_id: u32,
    /// Per-policy counters.
    pub policies: Vec<QosPolicyCount>,
}

// ============================================================================
// Signal statuses (marker structs)
// ============================================================================

/// `DATA_AVAILABLE_STATUS` — Spec §2.2.4.1.
///
/// Pure signal: "new data has arrived in the DataReader". There is no
/// spec data structure for it — we mirror the status as an empty marker
/// struct so that a uniform `get_data_available_status()` path exists.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DataAvailableStatus;

/// `DATA_ON_READERS_STATUS` — Spec §2.2.4.1.
///
/// Pure signal: "new data has arrived in **any** DataReader of the
/// Subscriber". Like [`DataAvailableStatus`], without a data structure.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DataOnReadersStatus;

// ============================================================================
// Reading a status
// ============================================================================

/// What reading a status, or handing it to a listener, does to it. DDS 1.4
/// §2.2.4.1 counts the change fields "since the last time the listener was
/// called or the status was read": a read takes the changes it reports off
/// the status, and what changed after it was taken stays.
pub trait StatusRecord: Clone {
    /// Takes the change fields of `reported` off `self`.
    fn acknowledge(&mut self, reported: &Self);

    /// Whether a change field is not zero: a change nobody read or was
    /// handed yet.
    fn has_changes(&self) -> bool;
}

impl StatusRecord for InconsistentTopicStatus {
    fn acknowledge(&mut self, reported: &Self) {
        self.total_count_change -= reported.total_count_change;
    }

    fn has_changes(&self) -> bool {
        self.total_count_change != 0
    }
}

impl StatusRecord for SampleLostStatus {
    fn acknowledge(&mut self, reported: &Self) {
        self.total_count_change -= reported.total_count_change;
    }

    fn has_changes(&self) -> bool {
        self.total_count_change != 0
    }
}

impl StatusRecord for LivelinessLostStatus {
    fn acknowledge(&mut self, reported: &Self) {
        self.total_count_change -= reported.total_count_change;
    }

    fn has_changes(&self) -> bool {
        self.total_count_change != 0
    }
}

impl StatusRecord for OfferedDeadlineMissedStatus {
    fn acknowledge(&mut self, reported: &Self) {
        self.total_count_change -= reported.total_count_change;
    }

    fn has_changes(&self) -> bool {
        self.total_count_change != 0
    }
}

impl StatusRecord for RequestedDeadlineMissedStatus {
    fn acknowledge(&mut self, reported: &Self) {
        self.total_count_change -= reported.total_count_change;
    }

    fn has_changes(&self) -> bool {
        self.total_count_change != 0
    }
}

impl StatusRecord for SampleRejectedStatus {
    fn acknowledge(&mut self, reported: &Self) {
        self.total_count_change -= reported.total_count_change;
    }

    fn has_changes(&self) -> bool {
        self.total_count_change != 0
    }
}

impl StatusRecord for LivelinessChangedStatus {
    fn acknowledge(&mut self, reported: &Self) {
        self.alive_count_change -= reported.alive_count_change;
        self.not_alive_count_change -= reported.not_alive_count_change;
    }

    fn has_changes(&self) -> bool {
        self.alive_count_change != 0 || self.not_alive_count_change != 0
    }
}

impl StatusRecord for PublicationMatchedStatus {
    fn acknowledge(&mut self, reported: &Self) {
        self.total_count_change -= reported.total_count_change;
        self.current_count_change -= reported.current_count_change;
    }

    fn has_changes(&self) -> bool {
        self.total_count_change != 0 || self.current_count_change != 0
    }
}

impl StatusRecord for SubscriptionMatchedStatus {
    fn acknowledge(&mut self, reported: &Self) {
        self.total_count_change -= reported.total_count_change;
        self.current_count_change -= reported.current_count_change;
    }

    fn has_changes(&self) -> bool {
        self.total_count_change != 0 || self.current_count_change != 0
    }
}

impl StatusRecord for OfferedIncompatibleQosStatus {
    fn acknowledge(&mut self, reported: &Self) {
        self.total_count_change -= reported.total_count_change;
    }

    fn has_changes(&self) -> bool {
        self.total_count_change != 0
    }
}

impl StatusRecord for RequestedIncompatibleQosStatus {
    fn acknowledge(&mut self, reported: &Self) {
        self.total_count_change -= reported.total_count_change;
    }

    fn has_changes(&self) -> bool {
        self.total_count_change != 0
    }
}

/// A status as an entity keeps it: the record, its change fields counting
/// since it was last read or handed to a listener (§2.2.4.1).
///
/// The StatusChangedFlag is read off the change fields, not kept beside
/// them. A flag of its own was reset by an acknowledgement of what a
/// listener had been handed even when the status had changed again since
/// the listener's look, and that change then reached no listener. Every
/// change of the statuses kept here moves a change field — a match counts
/// in `total_count_change`, an unmatch in `current_count_change` — so a
/// record whose change fields are all zero has nothing unread.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatusLedger<S> {
    record: S,
    /// How many times a `get_*_status` read the record; see
    /// [`Self::acknowledge`].
    reads: u64,
}

impl<S: StatusRecord> StatusLedger<S> {
    /// The record as it stands, change fields included.
    #[must_use]
    pub fn record(&self) -> S {
        self.record.clone()
    }

    /// The StatusChangedFlag: the status changed since it was last read
    /// or handed to a listener.
    #[must_use]
    pub fn unread(&self) -> bool {
        self.record.has_changes()
    }

    /// Changes the record. The change fields it moves are its flag.
    pub fn update(&mut self, change: impl FnOnce(&mut S)) {
        change(&mut self.record);
    }

    /// The StatusChangedFlag, the record and how many times a
    /// `get_*_status` read it, taken together for a listener's look. The
    /// count goes back with [`Self::acknowledge`].
    #[must_use]
    pub fn observe(&self) -> (bool, S, u64) {
        (self.unread(), self.record(), self.reads)
    }

    /// The record, resetting its change fields and its flag: what
    /// `get_*_status` does.
    pub fn take(&mut self) -> S {
        let record = self.record.clone();
        self.record.acknowledge(&record);
        self.reads = self.reads.wrapping_add(1);
        record
    }

    /// `reported` was handed to a listener by a look that found the record
    /// read `reads` times: its changes are no longer pending. A change
    /// after the look stays. A `get_*_status` since the look has reset the
    /// reported changes already; taking them off a second time left a
    /// negative change, which reads as a new one and called the listener
    /// again.
    pub fn acknowledge(&mut self, reported: &S, reads: u64) {
        if self.reads == reads {
            self.record.acknowledge(reported);
        }
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Adds a `policy_id` counter into a `policies` vec. If the entry
/// exists, `count` is incremented; otherwise a new one is appended. Used
/// by the runtime layer when distributing an IncompatibleQos event.
pub fn bump_policy_count(policies: &mut Vec<QosPolicyCount>, policy_id: u32) {
    if let Some(slot) = policies.iter_mut().find(|p| p.policy_id == policy_id) {
        slot.count = slot.count.saturating_add(1);
        return;
    }
    policies.push(QosPolicyCount::new(policy_id, 1));
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::psm_constants::qos_policy_id;

    #[test]
    fn inconsistent_topic_default_is_zero() {
        let s = InconsistentTopicStatus::default();
        assert_eq!(s.total_count, 0);
        assert_eq!(s.total_count_change, 0);
    }

    #[test]
    fn sample_lost_clone_roundtrip() {
        let s = SampleLostStatus {
            total_count: 5,
            total_count_change: 2,
        };
        let c = s;
        assert_eq!(s, c);
    }

    #[test]
    fn sample_rejected_status_default_kind_is_not_rejected() {
        let s = SampleRejectedStatus::default();
        assert_eq!(s.last_reason, SampleRejectedStatusKind::NotRejected);
        assert!(s.last_instance_handle.is_nil());
    }

    #[test]
    fn sample_rejected_kind_variants_are_distinct() {
        // Completeness of the spec enum variants.
        let kinds = [
            SampleRejectedStatusKind::NotRejected,
            SampleRejectedStatusKind::RejectedByInstancesLimit,
            SampleRejectedStatusKind::RejectedBySamplesLimit,
            SampleRejectedStatusKind::RejectedBySamplesPerInstanceLimit,
        ];
        for (i, a) in kinds.iter().enumerate() {
            for b in &kinds[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn liveliness_lost_default() {
        let s = LivelinessLostStatus::default();
        assert_eq!(s.total_count, 0);
    }

    #[test]
    fn liveliness_changed_negative_count_change_allowed() {
        // Spec §2.2.4.1: alive_count_change can be negative when a writer
        // transitions from "alive" to "not_alive".
        let s = LivelinessChangedStatus {
            alive_count: 0,
            not_alive_count: 1,
            alive_count_change: -1,
            not_alive_count_change: 1,
            last_publication_handle: InstanceHandle::from_raw(42),
        };
        assert_eq!(s.alive_count_change, -1);
        assert_eq!(s.not_alive_count_change, 1);
        assert_eq!(s.last_publication_handle.as_raw(), 42);
    }

    #[test]
    fn publication_matched_with_handle_roundtrip() {
        let h = InstanceHandle::from_raw(99);
        let s = PublicationMatchedStatus {
            total_count: 3,
            total_count_change: 1,
            current_count: 2,
            current_count_change: 1,
            last_subscription_handle: h,
        };
        let c = s;
        assert_eq!(c.last_subscription_handle, h);
        assert_eq!(c.current_count, 2);
    }

    #[test]
    fn subscription_matched_with_handle_roundtrip() {
        let h = InstanceHandle::from_raw(7);
        let s = SubscriptionMatchedStatus {
            total_count: 5,
            total_count_change: 0,
            current_count: 5,
            current_count_change: 0,
            last_publication_handle: h,
        };
        assert_eq!(s.last_publication_handle, h);
    }

    #[test]
    fn offered_deadline_missed_default_handle_is_nil() {
        let s = OfferedDeadlineMissedStatus::default();
        assert!(s.last_instance_handle.is_nil());
    }

    #[test]
    fn requested_deadline_missed_default_handle_is_nil() {
        let s = RequestedDeadlineMissedStatus::default();
        assert!(s.last_instance_handle.is_nil());
    }

    #[test]
    fn offered_incompatible_qos_clone_roundtrip() {
        let s = OfferedIncompatibleQosStatus {
            total_count: 2,
            total_count_change: 1,
            last_policy_id: qos_policy_id::DURABILITY,
            policies: alloc::vec![QosPolicyCount::new(qos_policy_id::DURABILITY, 2)],
        };
        let c = s.clone();
        assert_eq!(c, s);
        assert_eq!(c.policies.len(), 1);
        assert_eq!(c.policies[0].policy_id, qos_policy_id::DURABILITY);
    }

    #[test]
    fn requested_incompatible_qos_clone_roundtrip() {
        let s = RequestedIncompatibleQosStatus {
            total_count: 1,
            total_count_change: 1,
            last_policy_id: qos_policy_id::RELIABILITY,
            policies: alloc::vec![QosPolicyCount::new(qos_policy_id::RELIABILITY, 1)],
        };
        let c = s.clone();
        assert_eq!(c, s);
    }

    #[test]
    fn bump_policy_count_inserts_then_increments() {
        let mut v = alloc::vec::Vec::<QosPolicyCount>::new();
        bump_policy_count(&mut v, qos_policy_id::DURABILITY);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].count, 1);
        bump_policy_count(&mut v, qos_policy_id::DURABILITY);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].count, 2);
        bump_policy_count(&mut v, qos_policy_id::RELIABILITY);
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn data_available_and_data_on_readers_are_zero_sized_markers() {
        // Spec §2.2.4.1: pure signals — no fields.
        // We check the marker semantics via Default+Eq.
        let a1 = DataAvailableStatus;
        let a2 = DataAvailableStatus;
        assert_eq!(a1, a2);
        let r1 = DataOnReadersStatus;
        let r2 = DataOnReadersStatus;
        assert_eq!(r1, r2);
        // Marker structs have size 0 (compact).
        assert_eq!(core::mem::size_of::<DataAvailableStatus>(), 0);
        assert_eq!(core::mem::size_of::<DataOnReadersStatus>(), 0);
    }

    fn lost(ledger: &mut StatusLedger<SampleLostStatus>, n: i32) {
        ledger.update(|s| {
            s.total_count += n;
            s.total_count_change += n;
        });
    }

    /// DDS 1.4 §2.2.4.1: a read takes the changes it reports; the total
    /// stays, and a change after the read is the next read's.
    #[test]
    fn a_read_takes_the_changes_and_keeps_the_total() {
        let mut ledger = StatusLedger::<SampleLostStatus>::default();
        lost(&mut ledger, 2);
        let first = ledger.take();
        assert_eq!((first.total_count, first.total_count_change), (2, 2));
        let second = ledger.take();
        assert_eq!((second.total_count, second.total_count_change), (2, 0));
        lost(&mut ledger, 1);
        assert!(ledger.unread());
        assert_eq!(ledger.take().total_count_change, 1);
        assert!(!ledger.unread());
    }

    /// A `get_*_status` between a listener's look and its acknowledgement
    /// has reset what the look reported; the acknowledgement takes nothing
    /// off after it. Subtracting again left `total_count_change` at -1,
    /// which is unread and handed the listener a second, negative change.
    #[test]
    fn a_read_between_the_look_and_the_acknowledgement_is_not_taken_off_twice() {
        let mut ledger = StatusLedger::<SampleLostStatus>::default();
        lost(&mut ledger, 1);
        let (unread, handed, reads) = ledger.observe();
        assert!(unread);
        assert_eq!(ledger.take().total_count_change, 1);
        ledger.acknowledge(&handed, reads);
        assert!(!ledger.unread());
        assert_eq!(ledger.record().total_count_change, 0);
        // A change after the read still reaches the next look.
        lost(&mut ledger, 1);
        let (unread, handed, reads) = ledger.observe();
        assert!(unread);
        ledger.acknowledge(&handed, reads);
        assert!(!ledger.unread());
    }

    /// A change between a listener's look and its acknowledgement stays
    /// unread: the acknowledgement takes off what the listener was handed
    /// and nothing more.
    #[test]
    fn a_change_after_the_look_survives_the_acknowledgement() {
        let mut ledger = StatusLedger::<SampleLostStatus>::default();
        lost(&mut ledger, 1);
        let (_, handed, reads) = ledger.observe();
        lost(&mut ledger, 1);
        ledger.acknowledge(&handed, reads);
        assert!(ledger.unread());
        assert_eq!(ledger.take().total_count_change, 1);
        assert!(!ledger.unread());
    }

    /// What a listener was handed is acknowledged; what changed after it
    /// was taken stays for the next read.
    #[test]
    fn an_acknowledgement_keeps_what_came_after_the_report() {
        let mut ledger = StatusLedger::<PublicationMatchedStatus>::default();
        ledger.update(|s| {
            s.total_count = 1;
            s.total_count_change = 1;
            s.current_count = 1;
            s.current_count_change = 1;
        });
        let (_, handed, reads) = ledger.observe();
        ledger.update(|s| {
            s.current_count = 0;
            s.current_count_change -= 1;
        });
        ledger.acknowledge(&handed, reads);
        let rest = ledger.take();
        assert_eq!(
            (
                rest.total_count,
                rest.total_count_change,
                rest.current_count,
                rest.current_count_change
            ),
            (1, 0, 0, -1)
        );
    }

    #[test]
    fn liveliness_changed_acknowledges_both_changes() {
        let mut reported = LivelinessChangedStatus {
            alive_count: 1,
            not_alive_count: 1,
            alive_count_change: 1,
            not_alive_count_change: 1,
            last_publication_handle: InstanceHandle::from_raw(7),
        };
        let mut now = reported;
        now.alive_count_change = 2;
        now.acknowledge(&reported);
        assert_eq!((now.alive_count_change, now.not_alive_count_change), (1, 0));
        reported.acknowledge(&reported.clone());
        assert_eq!(reported.alive_count_change, 0);
    }

    /// DDS 1.4 §2.2.4.1 `RequestedDeadlineMissedStatus`: the
    /// `total_count_change` is what was missed since the last read; a read
    /// reports it and sets it back, the total stays.
    #[test]
    fn requested_deadline_missed_total_count_change_is_read_and_reset() {
        let mut ledger = StatusLedger::<RequestedDeadlineMissedStatus>::default();
        ledger.update(|s| {
            s.total_count += 2;
            s.total_count_change += 2;
            s.last_instance_handle = InstanceHandle::from_raw(5);
        });
        assert!(ledger.unread());
        let first = ledger.take();
        assert_eq!((first.total_count, first.total_count_change), (2, 2));
        assert_eq!(first.last_instance_handle.as_raw(), 5);
        let second = ledger.take();
        assert_eq!((second.total_count, second.total_count_change), (2, 0));
        assert!(!ledger.unread());
        ledger.update(|s| {
            s.total_count += 1;
            s.total_count_change += 1;
        });
        assert!(ledger.unread());
        assert_eq!(ledger.take().total_count_change, 1);
    }

    /// §2.2.4.1 `LivelinessLostStatus.total_count_change`, as above.
    #[test]
    fn liveliness_lost_total_count_change_is_read_and_reset() {
        let mut ledger = StatusLedger::<LivelinessLostStatus>::default();
        ledger.update(|s| {
            s.total_count += 1;
            s.total_count_change += 1;
        });
        assert!(ledger.unread());
        let first = ledger.take();
        assert_eq!((first.total_count, first.total_count_change), (1, 1));
        let second = ledger.take();
        assert_eq!((second.total_count, second.total_count_change), (1, 0));
        assert!(!ledger.unread());
        ledger.update(|s| {
            s.total_count += 3;
            s.total_count_change += 3;
        });
        assert_eq!(ledger.take().total_count_change, 3);
    }

    /// §2.2.4.1 `OfferedDeadlineMissedStatus.total_count_change`, as above.
    #[test]
    fn offered_deadline_missed_total_count_change_is_read_and_reset() {
        let mut ledger = StatusLedger::<OfferedDeadlineMissedStatus>::default();
        ledger.update(|s| {
            s.total_count += 4;
            s.total_count_change += 4;
            s.last_instance_handle = InstanceHandle::from_raw(9);
        });
        let first = ledger.take();
        assert_eq!((first.total_count, first.total_count_change), (4, 4));
        assert_eq!(first.last_instance_handle.as_raw(), 9);
        let second = ledger.take();
        assert_eq!((second.total_count, second.total_count_change), (4, 0));
        assert!(!ledger.unread());
    }

    /// §2.2.4.1 `SubscriptionMatchedStatus.current_count_change`: a match
    /// adds one, an unmatch takes one off, and both changes count since the
    /// last read: a match and its unmatch before a read leave a change of
    /// zero and a total of one; an unmatch after a read is a change of -1.
    #[test]
    fn subscription_matched_current_count_change_is_read_and_reset() {
        let mut ledger = StatusLedger::<SubscriptionMatchedStatus>::default();
        let matched = |s: &mut SubscriptionMatchedStatus| {
            s.total_count += 1;
            s.total_count_change += 1;
            s.current_count += 1;
            s.current_count_change += 1;
            s.last_publication_handle = InstanceHandle::from_raw(3);
        };
        let unmatched = |s: &mut SubscriptionMatchedStatus| {
            s.current_count -= 1;
            s.current_count_change -= 1;
        };
        ledger.update(matched);
        let first = ledger.take();
        assert_eq!(
            (
                first.total_count,
                first.total_count_change,
                first.current_count,
                first.current_count_change
            ),
            (1, 1, 1, 1)
        );
        ledger.update(unmatched);
        assert!(ledger.unread());
        let second = ledger.take();
        assert_eq!(
            (
                second.total_count,
                second.total_count_change,
                second.current_count,
                second.current_count_change
            ),
            (1, 0, 0, -1)
        );
        assert!(!ledger.unread());
        let third = ledger.take();
        assert_eq!((third.current_count, third.current_count_change), (0, 0));
        ledger.update(matched);
        ledger.update(unmatched);
        let fourth = ledger.take();
        assert_eq!(
            (
                fourth.total_count,
                fourth.total_count_change,
                fourth.current_count,
                fourth.current_count_change
            ),
            (2, 1, 0, 0)
        );
    }
}
