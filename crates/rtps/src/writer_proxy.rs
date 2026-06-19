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
use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::wire_types::{Guid, Locator, SequenceNumber};

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
    /// Smallest SN the writer **still** holds in the cache (from HEARTBEAT.first_sn).
    first_available_sn: SequenceNumber,
    /// Largest SN the writer announced (from HEARTBEAT.last_sn).
    last_available_sn: SequenceNumber,
    /// Highest SN this reader has actually **received**.
    highest_received_sn: SequenceNumber,
    /// Already-received SNs (for dup rejection + in-order delivery).
    received: BTreeSet<SequenceNumber>,
    /// SNs marked irrelevant by GAP submessages.
    irrelevant: BTreeSet<SequenceNumber>,
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
            received: BTreeSet::new(),
            irrelevant: BTreeSet::new(),
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
    /// can re-deliver; `last_sn` the largest announced.
    pub fn update_from_heartbeat(&mut self, first_sn: SequenceNumber, last_sn: SequenceNumber) {
        // Monotonically growing bounds.
        if first_sn > self.first_available_sn {
            self.first_available_sn = first_sn;
            // SNs **before** first_sn are lost and can no longer be
            // requested — we drop them from received/irrelevant; they
            // will also no longer be missing.
            let split = self.received.split_off(&first_sn);
            self.received = split;
            let split = self.irrelevant.split_off(&first_sn);
            self.irrelevant = split;
        }
        if last_sn > self.last_available_sn {
            self.last_available_sn = last_sn;
        }
    }

    /// Marks an SN as received.
    pub fn received_change_set(&mut self, sn: SequenceNumber) {
        if sn < self.first_available_sn {
            // Lies before the announced range — ignore.
            return;
        }
        self.received.insert(sn);
        if sn > self.highest_received_sn {
            self.highest_received_sn = sn;
        }
    }

    /// Marks an SN as irrelevant (via GAP).
    pub fn irrelevant_change_set(&mut self, sn: SequenceNumber) {
        if sn < self.first_available_sn {
            return;
        }
        self.irrelevant.insert(sn);
    }

    /// True if the SN is already received or marked irrelevant.
    #[must_use]
    pub fn is_known(&self, sn: SequenceNumber) -> bool {
        self.received.contains(&sn) || self.irrelevant.contains(&sn)
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
        let mut sn = self.first_available_sn;
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
        let mut sn = self.first_available_sn;
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
}
