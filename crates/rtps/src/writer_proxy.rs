// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `WriterProxy` — reader-side state over **one** remote writer.
//!
//! DDSI-RTPS 2.5 §8.4.6.5 (stateful reader behavior). The reader keeps
//! a `WriterProxy` per matched writer, in which it tracks the range
//! `[first_available_sn, last_available_sn]` from HEARTBEATs,
//! marks already-received SNs and recognizes missing ones as **missing**.
//! The missing set feeds the AckNack bitmap.
//!
//! A reader currently has one writer (single-writer
//! assumption).

extern crate alloc;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use crate::wire_types::{EntityId, Guid, Locator, SequenceNumber};

/// Most NOT_AVAILABLE runs a proxy keeps (see
/// [`WriterProxy::change_status`]). A BEST_EFFORT writer sends no
/// HEARTBEAT, so nothing else bounds them.
const MAX_NOT_AVAILABLE_RUNS: usize = 4096;

/// Reader-side state for one remote writer.
#[derive(Debug, Clone)]
pub struct WriterProxy {
    /// GUID of the remote writer endpoint.
    pub remote_writer_guid: Guid,
    /// Unicast locators of the writer (e.g. for directed re-sends).
    pub unicast_locators: Vec<Locator>,
    /// Multicast locators.
    pub multicast_locators: Vec<Locator>,
    /// Reliable kind.
    pub is_reliable: bool,
    /// `remoteGroupEntityId` (DDSI-RTPS 2.5 §8.4.10.4, Table 8.72): the
    /// group (Publisher) the matched writer belongs to, as its SEDP
    /// announcement names it in `PID_GROUP_GUID`; `ENTITYID_UNKNOWN` where it
    /// names none.
    pub remote_group_entity_id: EntityId,
    /// Smallest SN the writer **still** holds in the cache (from HEARTBEAT.first_sn).
    first_available_sn: SequenceNumber,
    /// Largest SN the writer announced (from HEARTBEAT.last_sn).
    last_available_sn: SequenceNumber,
    /// Highest SN this reader has actually **received**.
    highest_received_sn: SequenceNumber,
    /// Every SN from `first_available_sn` up to this one is received or
    /// irrelevant; `received` and `irrelevant` hold only what lies above
    /// it. Without it both sets kept every SN ever seen: a BEST_EFFORT
    /// writer sends no HEARTBEAT, so nothing ever cut them, and a 1 kB
    /// stream on the Linux test host left 12.7 million entries after
    /// 15 s; and `acknack_base` and `missing_changes` walked them one SN at
    /// a time from `first_available_sn` up, every ACKNACK.
    known_up_to: SequenceNumber,
    /// Received SNs above `known_up_to` (for dup rejection + in-order
    /// delivery).
    received: BTreeSet<SequenceNumber>,
    /// SNs marked irrelevant by GAP submessages, above `known_up_to`.
    irrelevant: BTreeSet<SequenceNumber>,
    /// Changes a GAP newly marked NOT_AVAILABLE, per sub-status
    /// (§8.4.10.4 `not_available_change_set`): NA_FILTERED, NA_REMOVED,
    /// NA_UNSPECIFIED.
    not_available: [u64; 3],
    /// The NOT_AVAILABLE changes with their sub-status, as runs
    /// `start -> (end inclusive, kind)` of neighbouring SNs of one kind
    /// (Table 8.74 `status`, kept per change). Every SN of a run was
    /// neither received nor irrelevant before it entered one.
    not_available_runs: BTreeMap<i64, (i64, NotAvailableKind)>,
    /// Runs below this SN were dropped at [`MAX_NOT_AVAILABLE_RUNS`];
    /// [`Self::change_status`] says `None` for them.
    status_horizon: SequenceNumber,
}

/// The status of one change relative to a writer
/// (DDSI-RTPS 2.5 §8.4.6 Table 8.52, §8.4.10.5 Table 8.74).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeFromWriterStatus {
    /// The writer has not announced the change yet.
    Unknown,
    /// The writer announced the change; this reader has not got it.
    Missing,
    /// This reader got the change.
    Received,
    /// The change is not available from the writer, with the reason.
    NotAvailable(NotAvailableKind),
}

/// Sub-status of NOT_AVAILABLE (DDSI-RTPS 2.5 §8.4.7, ChangeFromWriterStatusKind).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotAvailableKind {
    /// The change is in the writer cache but was filtered for this reader.
    Filtered,
    /// The change was in the writer cache and is no longer there.
    Removed,
    /// The writer did not say which of the two.
    Unspecified,
}

impl NotAvailableKind {
    /// The sub-status for a GAP whose set has `total` sequence numbers of
    /// which `filtered_count` are filtered (§8.4.10.4: all filtered →
    /// NA_FILTERED, none → NA_REMOVED, otherwise NA_UNSPECIFIED).
    #[must_use]
    pub fn for_gap(total: u64, filtered_count: u64) -> Self {
        if filtered_count == total {
            Self::Filtered
        } else if filtered_count == 0 {
            Self::Removed
        } else {
            Self::Unspecified
        }
    }

    fn slot(self) -> usize {
        match self {
            Self::Filtered => 0,
            Self::Removed => 1,
            Self::Unspecified => 2,
        }
    }
}

impl WriterProxy {
    /// Creates a fresh proxy.
    #[must_use]
    pub fn new(
        remote_writer_guid: Guid,
        unicast_locators: Vec<Locator>,
        multicast_locators: Vec<Locator>,
        is_reliable: bool,
    ) -> Self {
        Self {
            remote_writer_guid,
            unicast_locators,
            multicast_locators,
            is_reliable,
            first_available_sn: SequenceNumber(1),
            last_available_sn: SequenceNumber(0),
            highest_received_sn: SequenceNumber(0),
            known_up_to: SequenceNumber(0),
            received: BTreeSet::new(),
            irrelevant: BTreeSet::new(),
            not_available: [0; 3],
            not_available_runs: BTreeMap::new(),
            status_horizon: SequenceNumber(0),
            remote_group_entity_id: EntityId::UNKNOWN,
        }
    }

    /// Sets `remoteGroupEntityId` (Table 8.72).
    #[must_use]
    pub fn with_remote_group_entity_id(mut self, group: EntityId) -> Self {
        self.remote_group_entity_id = group;
        self
    }

    /// Records `sn` as NOT_AVAILABLE with `kind`, joined to a neighbouring
    /// run of the same kind.
    fn record_not_available(&mut self, from: i64, to: i64, kind: NotAvailableKind) {
        if to < from {
            return;
        }
        let (mut start, mut end) = (from, to);
        if let Some((&s, &(e, k))) = self.not_available_runs.range(..=start).next_back() {
            if k == kind && e + 1 >= start {
                start = s;
                end = end.max(e);
                self.not_available_runs.remove(&s);
            }
        }
        if let Some((&s, &(e, k))) = self.not_available_runs.range(end + 1..).next() {
            if k == kind && s == end + 1 {
                end = e;
                self.not_available_runs.remove(&s);
            }
        }
        self.not_available_runs.insert(start, (end, kind));
        while self.not_available_runs.len() > MAX_NOT_AVAILABLE_RUNS {
            if let Some((&s, &(e, _))) = self.not_available_runs.iter().next() {
                self.not_available_runs.remove(&s);
                if SequenceNumber(e + 1) > self.status_horizon {
                    self.status_horizon = SequenceNumber(e + 1);
                }
            }
        }
    }

    /// The status of the change `sn` (Table 8.74 `status`).
    ///
    /// `None` for an SN below the point from which the proxy no longer
    /// keeps the NOT_AVAILABLE runs (it keeps `MAX_NOT_AVAILABLE_RUNS` of
    /// them), and for an SN that is not positive.
    #[must_use]
    pub fn change_status(&self, sn: SequenceNumber) -> Option<ChangeFromWriterStatus> {
        if sn.0 <= 0 || sn < self.status_horizon {
            return None;
        }
        if let Some((_, &(end, kind))) = self.not_available_runs.range(..=sn.0).next_back() {
            if sn.0 <= end {
                return Some(ChangeFromWriterStatus::NotAvailable(kind));
            }
        }
        if sn < self.first_available_sn || self.is_known(sn) {
            return Some(ChangeFromWriterStatus::Received);
        }
        if sn > self.last_available_sn {
            return Some(ChangeFromWriterStatus::Unknown);
        }
        Some(ChangeFromWriterStatus::Missing)
    }

    /// Moves `known_up_to` over every SN right behind it that the sets
    /// hold, taking them out of the sets.
    fn absorb(&mut self) {
        loop {
            let next = SequenceNumber(self.known_up_to.0 + 1);
            // Out of both: an SN can be received and irrelevant at once, and
            // one left in a set below `known_up_to` would still count after
            // `unreceive_change_set` took it back.
            let received = self.received.remove(&next);
            let irrelevant = self.irrelevant.remove(&next);
            if received || irrelevant {
                self.known_up_to = next;
            } else {
                return;
            }
        }
    }

    /// Marks `sn` known: past `known_up_to` when it is the next one, into
    /// `set` when there is a gap before it.
    fn mark_known(&mut self, sn: SequenceNumber, received: bool) {
        if sn.0 == self.known_up_to.0 + 1 {
            self.known_up_to = sn;
            self.absorb();
        } else if sn > self.known_up_to {
            if received {
                self.received.insert(sn);
            } else {
                self.irrelevant.insert(sn);
            }
        }
    }

    /// Updates only the locators (re-discovery of the same writer), without
    /// touching the reliability state (SN bounds, received/irrelevant).
    /// A renewed SPDP/SEDP announce must not discard the reader progress
    /// — otherwise the reader falsely reports "nothing missing" after a
    /// HEARTBEAT and the writer never delivers the DATA.
    pub fn refresh_locators(
        &mut self,
        unicast_locators: Vec<Locator>,
        multicast_locators: Vec<Locator>,
    ) {
        self.unicast_locators = unicast_locators;
        self.multicast_locators = multicast_locators;
    }

    /// Processes a HEARTBEAT.
    ///
    /// Per §8.4.15: `first_sn` is the smallest SN the writer
    /// can re-deliver; `last_sn` the largest announced. Returns what
    /// [`Self::lost_changes_update`] returns for `first_sn`.
    pub fn update_from_heartbeat(
        &mut self,
        first_sn: SequenceNumber,
        last_sn: SequenceNumber,
    ) -> u64 {
        // Monotonically growing bounds. SNs **before** first_sn are lost
        // and can no longer be requested (§8.4.10.4.4).
        let lost = self.lost_changes_update(first_sn);
        if last_sn > self.last_available_sn {
            self.last_available_sn = last_sn;
        }
        lost
    }

    /// §8.4.10.4.4 `lost_changes_update` — everything below
    /// `first_available_seq_num` that is still UNKNOWN or MISSING can no
    /// longer be requested from the writer and becomes NOT_AVAILABLE.
    ///
    /// In this proxy the three statuses are held as sets rather than per
    /// change: raising `first_available_sn` makes every SN below it
    /// unreachable for [`Self::missing_changes`], and the now-dead
    /// entries in `received`/`irrelevant` are dropped with it.
    /// [`Self::update_from_heartbeat`] routes its `first_sn` through
    /// here.
    ///
    /// Returns how many of the SNs this made NOT_AVAILABLE were neither
    /// received nor irrelevant: changes this reader never got and now
    /// cannot get, what DDS 1.4 §2.2.4.1 SAMPLE_LOST counts. Whether a
    /// change before the first one the reader took was one it waited for
    /// is the reader's to decide.
    pub fn lost_changes_update(&mut self, first_available_seq_num: SequenceNumber) -> u64 {
        if first_available_seq_num <= self.first_available_sn {
            return 0;
        }
        let lost = self.unknown_before(first_available_seq_num);
        self.record_unknown_as_removed(first_available_seq_num);
        self.not_available[NotAvailableKind::Removed.slot()] =
            self.not_available[NotAvailableKind::Removed.slot()].saturating_add(lost);
        self.first_available_sn = first_available_seq_num;
        self.received = self.received.split_off(&first_available_seq_num);
        self.irrelevant = self.irrelevant.split_off(&first_available_seq_num);
        // Nothing below the new first SN can be asked for; the run of known
        // SNs starts again at it.
        let floor = SequenceNumber(first_available_seq_num.0 - 1);
        if self.known_up_to < floor {
            self.known_up_to = floor;
        }
        self.absorb();
        lost
    }

    /// Enters the SNs from `first_available_sn` up to `until` (exclusive)
    /// that are neither received nor irrelevant as NA_REMOVED runs.
    fn record_unknown_as_removed(&mut self, until: SequenceNumber) {
        let from = self.first_unknown_candidate();
        if until <= from {
            return;
        }
        let mut known: Vec<i64> = self
            .received
            .range(from..until)
            .chain(self.irrelevant.range(from..until))
            .map(|sn| sn.0)
            .collect();
        known.sort_unstable();
        known.dedup();
        let mut cursor = from.0;
        for k in known {
            self.record_not_available(cursor, k - 1, NotAvailableKind::Removed);
            cursor = k + 1;
        }
        self.record_not_available(cursor, until.0 - 1, NotAvailableKind::Removed);
    }

    /// The SNs from `first_available_sn` up to `until` (exclusive) that are
    /// neither received nor irrelevant.
    fn unknown_before(&self, until: SequenceNumber) -> u64 {
        let from = self.first_unknown_candidate();
        if until <= from {
            return 0;
        }
        let span = u64::try_from(until.0 - from.0).unwrap_or(u64::MAX);
        let received = self.received.range(from..until).count();
        let irrelevant = self
            .irrelevant
            .range(from..until)
            .filter(|sn| !self.received.contains(sn))
            .count();
        span.saturating_sub((received + irrelevant) as u64)
    }

    /// §8.4.10.4.2 `available_changes_max` — the largest SN the
    /// DataReader may access, i.e. the largest SN for which no smaller
    /// or equal SN is still MISSING or UNKNOWN.
    ///
    /// Zero when the very first announced SN has not arrived: the spec's
    /// `MAX` over an empty set, and the SN space starts at 1.
    #[must_use]
    pub fn available_changes_max(&self) -> SequenceNumber {
        SequenceNumber(self.acknack_base().0 - 1)
    }

    /// Marks an SN as received.
    pub fn received_change_set(&mut self, sn: SequenceNumber) {
        if sn < self.first_available_sn {
            // Lies before the announced range — ignore.
            return;
        }
        self.mark_known(sn, true);
        if sn > self.highest_received_sn {
            self.highest_received_sn = sn;
        }
    }

    /// Takes back a [`Self::received_change_set`] for `sn`, so the SN counts
    /// as MISSING again and [`Self::missing_changes`] reports it to the next
    /// ACKNACK.
    ///
    /// The reader marks an SN received before it stores the sample, and the
    /// store can still fail — the per-proxy receive cache is a KeepAll cache
    /// with a fixed capacity, and it fills whenever in-order delivery is held
    /// up by an earlier gap. Without this rollback the SN stayed "received"
    /// with no sample behind it: the reader never asked for it again, and a
    /// RELIABLE stream lost it permanently and without a trace. Rolling the
    /// mark back turns that permanent loss into a repair the writer can serve
    /// once the backlog has drained.
    ///
    /// `highest_received_sn` is deliberately left alone: it is a high-water
    /// mark for HEARTBEAT handling, and lowering it would re-open windows the
    /// proxy has already reasoned about.
    ///
    /// An SN a GAP also marked irrelevant is MISSING afterwards too: a run
    /// of known SNs no longer tells the two apart, and asking again for an
    /// irrelevant SN costs the writer one more GAP.
    pub fn unreceive_change_set(&mut self, sn: SequenceNumber) {
        if sn < self.first_available_sn {
            return;
        }
        if sn <= self.known_up_to {
            // The SNs after it stay known, now above a gap. Whether each
            // was received or irrelevant no longer matters: both are known.
            let mut after = SequenceNumber(sn.0 + 1);
            while after <= self.known_up_to {
                self.received.insert(after);
                after = SequenceNumber(after.0 + 1);
            }
            self.known_up_to = SequenceNumber(sn.0 - 1);
        } else {
            self.received.remove(&sn);
            self.irrelevant.remove(&sn);
        }
    }

    /// Marks an SN as irrelevant (via GAP).
    pub fn irrelevant_change_set(&mut self, sn: SequenceNumber) {
        if sn < self.first_available_sn {
            return;
        }
        self.mark_known(sn, false);
    }

    /// Marks an SN as NOT_AVAILABLE with the sub-status `kind`
    /// (§8.4.10.4). An SN that is already known (received, or marked by an
    /// earlier GAP) keeps its state and is not counted again.
    pub fn not_available_change_set(&mut self, sn: SequenceNumber, kind: NotAvailableKind) {
        if sn < self.first_available_sn {
            return;
        }
        if !self.is_known(sn) {
            self.not_available[kind.slot()] = self.not_available[kind.slot()].saturating_add(1);
            self.record_not_available(sn.0, sn.0, kind);
        }
        self.mark_known(sn, false);
    }

    /// Number of changes marked NOT_AVAILABLE by a GAP with the sub-status
    /// `kind`.
    #[must_use]
    pub fn not_available_count(&self, kind: NotAvailableKind) -> u64 {
        self.not_available[kind.slot()]
    }

    /// True if the SN is already received or marked irrelevant.
    #[must_use]
    pub fn is_known(&self, sn: SequenceNumber) -> bool {
        (sn >= self.first_available_sn && sn <= self.known_up_to)
            || self.received.contains(&sn)
            || self.irrelevant.contains(&sn)
    }

    /// The first SN at or after `first_available_sn` that can be missing:
    /// everything before it is known.
    fn first_unknown_candidate(&self) -> SequenceNumber {
        let after_known = SequenceNumber(self.known_up_to.0 + 1);
        if after_known > self.first_available_sn {
            after_known
        } else {
            self.first_available_sn
        }
    }

    /// Returns all **missing** SNs (neither received nor irrelevant) in the
    /// range `[first_available_sn, last_available_sn]`.
    ///
    /// The vector is sorted ascending by SN. Limited to `max_count`
    /// entries — the expected RTPS bitmap window is 256 SNs.
    #[must_use]
    pub fn missing_changes(&self, max_count: usize) -> Vec<SequenceNumber> {
        let mut out = Vec::new();
        if self.last_available_sn < self.first_available_sn {
            return out;
        }
        let mut sn = self.first_unknown_candidate();
        while sn <= self.last_available_sn && out.len() < max_count {
            if !self.is_known(sn) {
                out.push(sn);
            }
            sn = SequenceNumber(sn.0 + 1);
        }
        out
    }

    /// True if there are missing SNs.
    #[must_use]
    pub fn has_missing_changes(&self) -> bool {
        !self.missing_changes(1).is_empty()
    }

    /// Getter: smallest announced SN.
    #[must_use]
    pub fn first_available_sn(&self) -> SequenceNumber {
        self.first_available_sn
    }

    /// Getter: largest announced SN.
    #[must_use]
    pub fn last_available_sn(&self) -> SequenceNumber {
        self.last_available_sn
    }

    /// Getter: highest received SN.
    #[must_use]
    pub fn highest_received_sn(&self) -> SequenceNumber {
        self.highest_received_sn
    }

    /// Matching AckNack base: smallest not-yet-acked SN.
    ///
    /// Convention: all SN < `acknack_base` are acked. We return
    /// the smallest not-yet-received-or-irrelevant SN in `[first, last+1]`.
    #[must_use]
    pub fn acknack_base(&self) -> SequenceNumber {
        let mut sn = self.first_unknown_candidate();
        while sn <= self.last_available_sn {
            if !self.is_known(sn) {
                return sn;
            }
            sn = SequenceNumber(sn.0 + 1);
        }
        SequenceNumber(self.last_available_sn.0 + 1)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::wire_types::{EntityId, GuidPrefix};

    fn sn(n: i64) -> SequenceNumber {
        SequenceNumber(n)
    }

    fn proxy() -> WriterProxy {
        let guid = Guid::new(
            GuidPrefix::from_bytes([2; 12]),
            EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
        );
        WriterProxy::new(guid, alloc::vec![], alloc::vec![], true)
    }

    #[test]
    fn fresh_proxy_has_no_missing() {
        let p = proxy();
        assert!(!p.has_missing_changes());
        assert_eq!(p.missing_changes(10), alloc::vec![]);
        assert_eq!(p.acknack_base(), sn(1));
    }

    #[test]
    fn heartbeat_sets_available_range() {
        let mut p = proxy();
        p.update_from_heartbeat(sn(1), sn(5));
        assert_eq!(p.first_available_sn(), sn(1));
        assert_eq!(p.last_available_sn(), sn(5));
        // Nothing received yet → everything missing
        assert_eq!(
            p.missing_changes(10),
            alloc::vec![sn(1), sn(2), sn(3), sn(4), sn(5)]
        );
    }

    #[test]
    fn received_removes_from_missing() {
        let mut p = proxy();
        p.update_from_heartbeat(sn(1), sn(5));
        p.received_change_set(sn(2));
        p.received_change_set(sn(4));
        assert_eq!(p.missing_changes(10), alloc::vec![sn(1), sn(3), sn(5)]);
        assert_eq!(p.acknack_base(), sn(1));
    }

    #[test]
    fn gap_marks_irrelevant() {
        let mut p = proxy();
        p.update_from_heartbeat(sn(1), sn(5));
        p.irrelevant_change_set(sn(3));
        assert_eq!(
            p.missing_changes(10),
            alloc::vec![sn(1), sn(2), sn(4), sn(5)]
        );
    }

    #[test]
    fn acknack_base_walks_up() {
        let mut p = proxy();
        p.update_from_heartbeat(sn(1), sn(3));
        p.received_change_set(sn(1));
        p.received_change_set(sn(2));
        assert_eq!(p.acknack_base(), sn(3));
        p.received_change_set(sn(3));
        assert_eq!(p.acknack_base(), sn(4));
    }

    #[test]
    fn heartbeat_advancing_first_prunes_old_state() {
        let mut p = proxy();
        p.update_from_heartbeat(sn(1), sn(10));
        p.received_change_set(sn(3));
        p.received_change_set(sn(7));
        // Writer rotates the cache → first now at 5
        p.update_from_heartbeat(sn(5), sn(10));
        assert_eq!(p.first_available_sn(), sn(5));
        // sn(3) removed from received, sn(7) stays
        assert!(!p.is_known(sn(3)));
        assert!(p.is_known(sn(7)));
    }

    #[test]
    fn highest_received_tracks_max() {
        let mut p = proxy();
        p.update_from_heartbeat(sn(1), sn(10));
        p.received_change_set(sn(3));
        p.received_change_set(sn(7));
        p.received_change_set(sn(5));
        assert_eq!(p.highest_received_sn(), sn(7));
    }

    #[test]
    fn received_before_first_is_ignored() {
        let mut p = proxy();
        p.update_from_heartbeat(sn(5), sn(10));
        p.received_change_set(sn(2));
        assert!(!p.is_known(sn(2)));
        assert_eq!(p.highest_received_sn(), sn(0));
    }

    #[test]
    fn missing_changes_respects_max_count() {
        let mut p = proxy();
        p.update_from_heartbeat(sn(1), sn(100));
        let m = p.missing_changes(5);
        assert_eq!(m, alloc::vec![sn(1), sn(2), sn(3), sn(4), sn(5)]);
    }

    #[test]
    fn acknack_base_when_all_received_is_last_plus_one() {
        let mut p = proxy();
        p.update_from_heartbeat(sn(1), sn(3));
        p.received_change_set(sn(1));
        p.received_change_set(sn(2));
        p.received_change_set(sn(3));
        assert_eq!(p.acknack_base(), sn(4));
        assert!(!p.has_missing_changes());
    }

    /// §8.4.10.4.2: `available_changes_max` is the largest SN with no
    /// smaller-or-equal SN still MISSING or UNKNOWN. A hole at 3 caps
    /// the value at 2 even though 4 and 5 have arrived; filling the hole
    /// releases the whole run.
    #[test]
    fn available_changes_max_stops_at_the_first_hole() {
        let mut p = proxy();
        p.update_from_heartbeat(sn(1), sn(5));
        // Nothing received: the MAX is over an empty set, SN space
        // starts at 1, so nothing is accessible.
        assert_eq!(p.available_changes_max(), sn(0));

        p.received_change_set(sn(1));
        p.received_change_set(sn(2));
        p.received_change_set(sn(4));
        p.received_change_set(sn(5));
        assert_eq!(p.available_changes_max(), sn(2));

        p.received_change_set(sn(3));
        assert_eq!(p.available_changes_max(), sn(5));
    }

    /// A NOT_AVAILABLE change (GAP) counts as accessible for the
    /// purposes of §8.4.10.4.2 — the reader will never get it, so it
    /// must not block the changes behind it.
    #[test]
    fn available_changes_max_passes_over_an_irrelevant_change() {
        let mut p = proxy();
        p.update_from_heartbeat(sn(1), sn(3));
        p.received_change_set(sn(1));
        p.irrelevant_change_set(sn(2));
        p.received_change_set(sn(3));
        assert_eq!(p.available_changes_max(), sn(3));
    }

    /// §8.4.10.4.4: after `lost_changes_update(4)` the changes 1..=3 are
    /// NOT_AVAILABLE — no longer missing, no longer requestable — and
    /// `available_changes_max` starts from the new base.
    #[test]
    fn lost_changes_update_counts_what_was_neither_received_nor_irrelevant() {
        let mut p = proxy();
        p.received_change_set(sn(1));
        p.received_change_set(sn(4));
        p.irrelevant_change_set(sn(6));
        p.received_change_set(sn(6));
        // Below 8: 2, 3, 5 and 7 never came; 6 is counted once.
        assert_eq!(p.lost_changes_update(sn(8)), 4);
        assert_eq!(p.lost_changes_update(sn(8)), 0, "already NOT_AVAILABLE");
        assert_eq!(p.update_from_heartbeat(sn(10), sn(12)), 2, "8 and 9");
        p.received_change_set(sn(10));
        assert_eq!(p.lost_changes_update(sn(11)), 0);
    }

    #[test]
    fn lost_changes_update_retires_everything_below_the_new_base() {
        let mut p = proxy();
        p.update_from_heartbeat(sn(1), sn(6));
        p.received_change_set(sn(2));
        assert_eq!(
            p.missing_changes(10),
            alloc::vec![sn(1), sn(3), sn(4), sn(5), sn(6)]
        );

        p.lost_changes_update(sn(4));
        assert_eq!(p.missing_changes(10), alloc::vec![sn(4), sn(5), sn(6)]);
        assert!(!p.is_known(sn(2)), "the retired receipt is dropped");
        assert_eq!(p.first_available_sn(), sn(4));
        assert_eq!(p.available_changes_max(), sn(3));

        // Monotone: a lower base is ignored.
        p.lost_changes_update(sn(2));
        assert_eq!(p.first_available_sn(), sn(4));
    }

    /// The proxy with its run of known SNs answers as the plain sets did:
    /// one pseudo-random sequence of receptions, GAPs, take-backs and
    /// HEARTBEATs, checked step by step against a model that keeps every
    /// SN in a set.
    #[test]
    fn the_known_run_answers_as_the_sets_of_every_sn_did() {
        struct Model {
            first: i64,
            last: i64,
            known: alloc::collections::BTreeSet<i64>,
        }
        impl Model {
            fn is_known(&self, n: i64) -> bool {
                n >= self.first && self.known.contains(&n)
            }
            fn missing(&self, max: usize) -> alloc::vec::Vec<SequenceNumber> {
                (self.first..=self.last)
                    .filter(|n| !self.is_known(*n))
                    .take(max)
                    .map(sn)
                    .collect()
            }
            fn base(&self) -> SequenceNumber {
                sn((self.first..=self.last)
                    .find(|n| !self.is_known(*n))
                    .unwrap_or(self.last + 1))
            }
        }
        let mut p = proxy();
        let mut m = Model {
            first: 1,
            last: 0,
            known: alloc::collections::BTreeSet::new(),
        };
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = |bound: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % bound
        };
        for step in 0..20_000 {
            let op = next(100);
            let n = 1 + next(400) as i64;
            match op {
                0..=59 => {
                    p.received_change_set(sn(n));
                    if n >= m.first {
                        m.known.insert(n);
                    }
                }
                60..=74 => {
                    p.irrelevant_change_set(sn(n));
                    if n >= m.first {
                        m.known.insert(n);
                    }
                }
                75..=84 => {
                    p.unreceive_change_set(sn(n));
                    if n >= m.first {
                        m.known.remove(&n);
                    }
                }
                _ => {
                    let first = m.first.max(1 + next(380) as i64 - 20).max(1);
                    let last = first + next(60) as i64;
                    p.update_from_heartbeat(sn(first), sn(last));
                    if first > m.first {
                        m.first = first;
                        m.known = m.known.split_off(&first);
                    }
                    m.last = m.last.max(last);
                }
            }
            for probe in [n, n + 1, n - 1, m.first, m.last] {
                assert_eq!(
                    p.is_known(sn(probe)),
                    m.is_known(probe),
                    "step {step}, sn {probe}"
                );
            }
            assert_eq!(p.missing_changes(16), m.missing(16), "step {step}");
            assert_eq!(p.acknack_base(), m.base(), "step {step}");
        }
    }

    /// Table 8.52 / 8.74: every change has one status; NOT_AVAILABLE has the
    /// three sub-kinds, kept per change (§8.4.10.4 `not_available_change_set`).
    #[test]
    fn change_status_is_kept_per_change_with_the_sub_kind() {
        use ChangeFromWriterStatus as S;
        let mut p = proxy();
        // Nothing announced: UNKNOWN.
        assert_eq!(p.change_status(sn(1)), Some(S::Unknown));
        p.update_from_heartbeat(sn(1), sn(8));
        assert_eq!(p.change_status(sn(1)), Some(S::Missing));
        assert_eq!(p.change_status(sn(9)), Some(S::Unknown));
        p.received_change_set(sn(2));
        assert_eq!(p.change_status(sn(2)), Some(S::Received));
        p.not_available_change_set(sn(3), NotAvailableKind::Filtered);
        p.not_available_change_set(sn(4), NotAvailableKind::Removed);
        p.not_available_change_set(sn(5), NotAvailableKind::Unspecified);
        assert_eq!(
            p.change_status(sn(3)),
            Some(S::NotAvailable(NotAvailableKind::Filtered))
        );
        assert_eq!(
            p.change_status(sn(4)),
            Some(S::NotAvailable(NotAvailableKind::Removed))
        );
        assert_eq!(
            p.change_status(sn(5)),
            Some(S::NotAvailable(NotAvailableKind::Unspecified))
        );
        // The neighbours keep their own status.
        assert_eq!(p.change_status(sn(1)), Some(S::Missing));
        assert_eq!(p.change_status(sn(6)), Some(S::Missing));
        // A change that is known is not marked again.
        p.not_available_change_set(sn(2), NotAvailableKind::Removed);
        assert_eq!(p.change_status(sn(2)), Some(S::Received));
        assert_eq!(p.not_available_count(NotAvailableKind::Removed), 1);
        assert_eq!(p.change_status(sn(0)), None);
    }

    /// §8.4.10.4.4 `lost_changes_update`: what is UNKNOWN or MISSING below
    /// `first_available_seq_num` becomes NOT_AVAILABLE (NA_REMOVED); what
    /// was RECEIVED stays RECEIVED; the sub-kind counter counts them.
    #[test]
    fn lost_changes_update_sets_na_removed_per_change() {
        use ChangeFromWriterStatus as S;
        let mut p = proxy();
        p.update_from_heartbeat(sn(1), sn(10));
        p.received_change_set(sn(2));
        p.received_change_set(sn(6));
        p.not_available_change_set(sn(4), NotAvailableKind::Filtered);
        let lost = p.lost_changes_update(sn(8));
        // 1, 3, 5, 7 were MISSING.
        assert_eq!(lost, 4);
        for n in [1, 3, 5, 7] {
            assert_eq!(
                p.change_status(sn(n)),
                Some(S::NotAvailable(NotAvailableKind::Removed)),
                "sn {n}"
            );
        }
        assert_eq!(p.change_status(sn(2)), Some(S::Received));
        assert_eq!(p.change_status(sn(6)), Some(S::Received));
        assert_eq!(
            p.change_status(sn(4)),
            Some(S::NotAvailable(NotAvailableKind::Filtered))
        );
        assert_eq!(p.change_status(sn(8)), Some(S::Missing));
        assert_eq!(p.not_available_count(NotAvailableKind::Removed), 4);
        assert_eq!(p.not_available_count(NotAvailableKind::Filtered), 1);
    }

    /// A long run of NOT_AVAILABLE changes is one entry, and the number of
    /// entries is bounded; a status that fell out of the bound is `None`.
    #[test]
    fn not_available_runs_are_joined_and_bounded() {
        use ChangeFromWriterStatus as S;
        let mut p = proxy();
        for n in 1..=1000 {
            p.not_available_change_set(sn(n), NotAvailableKind::Removed);
        }
        assert_eq!(p.not_available_runs.len(), 1);
        assert_eq!(
            p.change_status(sn(500)),
            Some(S::NotAvailable(NotAvailableKind::Removed))
        );
        // Alternating kinds make one run each.
        let mut q = proxy();
        let total = (MAX_NOT_AVAILABLE_RUNS as i64) + 10;
        for n in 1..=total {
            let kind = if n % 2 == 0 {
                NotAvailableKind::Removed
            } else {
                NotAvailableKind::Filtered
            };
            q.not_available_change_set(sn(n), kind);
        }
        assert_eq!(q.not_available_runs.len(), MAX_NOT_AVAILABLE_RUNS);
        assert_eq!(q.change_status(sn(1)), None);
        assert_eq!(
            q.change_status(sn(total)),
            Some(S::NotAvailable(NotAvailableKind::Removed))
        );
    }

    #[test]
    fn the_remote_group_entity_id_is_kept() {
        let group = EntityId::user_writer_with_key([0, 0, 9]);
        assert_eq!(proxy().remote_group_entity_id, EntityId::UNKNOWN);
        let p = proxy().with_remote_group_entity_id(group);
        assert_eq!(p.remote_group_entity_id, group);
    }
}
