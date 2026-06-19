// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `ReaderProxy` — writer-side state over **one** remote reader.
//!
//! DDSI-RTPS 2.5 §8.4.4.11 (stateful writer behavior). The writer keeps
//! a `ReaderProxy` per matched reader, in which it tracks which
//! sequence numbers the reader has already acked and which it has
//! explicitly re-requested.
//!
//! A writer currently has only one reader (single-reader
//! assumption). Still, the data structure is cut so that `Vec<ReaderProxy>`
//! is possible later.

extern crate alloc;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use crate::wire_types::{FragmentNumber, Guid, Locator, SequenceNumber};

/// Writer-side state for one remote reader.
#[derive(Debug, Clone)]
pub struct ReaderProxy {
    /// GUID of the remote reader endpoint.
    pub remote_reader_guid: Guid,
    /// Unicast receive locator(s) of the reader.
    pub unicast_locators: Vec<Locator>,
    /// Multicast receive locator(s).
    pub multicast_locators: Vec<Locator>,
    /// Reliable kind (always true in WP 1.1).
    pub is_reliable: bool,
    /// Highest SN the reader has **already acked**
    /// (from AckNack.reader_sn_state.bitmap_base - 1).
    highest_acked_sn: SequenceNumber,
    /// Highest SN the writer has **already sent** to this reader.
    highest_sent_sn: SequenceNumber,
    /// Set of requested SNs from AckNack.bitmap, queued for re-send.
    requested_changes: BTreeSet<SequenceNumber>,
    /// Per sample SN: set of missing FragmentNumbers the reader requested via
    /// NACK_FRAG. For fragment-granular re-sends.
    requested_fragments: BTreeMap<SequenceNumber, BTreeSet<FragmentNumber>>,
    /// Spec §8.4.15.6 inactive-reader reclaim: last observed
    /// reader activity (incoming ACKNACK / NACK_FRAG). The writer
    /// calls `note_activity(now)` from the ACKNACK path. If
    /// `now - last_activity > inactive_threshold`, the writer can recognize the
    /// proxy as a reclaim candidate via `is_inactive`, so that
    /// strict reliability does not run the cache OOM.
    last_activity: core::time::Duration,
    /// XTypes 1.3 §7.6.3.1 — per-reader negotiated wire format.
    /// Default `XCDR2` (=2). On match the field is set via
    /// `data_representation::negotiate(writer_offered, reader_accepted, mode)`,
    /// otherwise it stays default. The encap header on sample-write
    /// uses `data_representation::encap_for_final_le(this)`.
    negotiated_data_representation: i16,
}

impl ReaderProxy {
    /// Creates a fresh proxy.
    #[must_use]
    pub fn new(
        remote_reader_guid: Guid,
        unicast_locators: Vec<Locator>,
        multicast_locators: Vec<Locator>,
        is_reliable: bool,
    ) -> Self {
        Self {
            remote_reader_guid,
            unicast_locators,
            multicast_locators,
            is_reliable,
            // Pre-existing state: nothing acked, nothing sent. SN starts
            // at 1; "0 acked" means "nothing acked" (§8.7.4).
            highest_acked_sn: SequenceNumber(0),
            highest_sent_sn: SequenceNumber(0),
            requested_changes: BTreeSet::new(),
            requested_fragments: BTreeMap::new(),
            last_activity: core::time::Duration::ZERO,
            // Default: XCDR2 (modern). The SEDP match path overwrites
            // with the negotiated value.
            negotiated_data_representation: crate::publication_data::data_representation::XCDR2,
        }
    }

    /// Sets the negotiated wire format for this reader.
    /// Called by the DCPS-SEDP match path after `negotiate(...)`.
    pub fn set_negotiated_data_representation(&mut self, id: i16) {
        self.negotiated_data_representation = id;
    }

    /// Returns the negotiated wire format.
    #[must_use]
    pub fn negotiated_data_representation(&self) -> i16 {
        self.negotiated_data_representation
    }

    /// Spec §8.4.15.6 — marks incoming reader activity (every
    /// ACKNACK / NACK_FRAG calls this from the receiver path).
    pub fn note_activity(&mut self, now: core::time::Duration) {
        self.last_activity = now;
    }

    /// Spec §8.4.15.6 — `true` if the reader has shown no activity for
    /// longer than `threshold`. The caller (e.g. ReliableWriter)
    /// uses this to reclaim the proxy from the `matched_readers` list,
    /// so that strict reliability does not run the cache OOM.
    #[must_use]
    pub fn is_inactive(&self, now: core::time::Duration, threshold: core::time::Duration) -> bool {
        now.checked_sub(self.last_activity)
            .is_some_and(|elapsed| elapsed > threshold)
    }

    /// Returns the last-activity timestamp (diagnosis).
    #[must_use]
    pub fn last_activity(&self) -> core::time::Duration {
        self.last_activity
    }

    /// Marks samples up to and including `sn` as "no longer
    /// relevant" for this proxy — both sent and
    /// acked. Called e.g. for volatile durability when a
    /// new reader proxy is added: it should not get historic samples,
    /// so we jump directly to the current cache
    /// state.
    ///
    /// Spec reference: OMG DDS 1.4 §2.2.3.4 DurabilityQosPolicy Volatile:
    /// "The Service will not attempt to retain old data beyond what is
    /// currently held by the DataWriter for live Readers".
    pub fn skip_samples_up_to(&mut self, sn: SequenceNumber) {
        if sn > self.highest_sent_sn {
            self.highest_sent_sn = sn;
        }
        if sn > self.highest_acked_sn {
            self.highest_acked_sn = sn;
        }
    }

    /// Updates to the ACKNACK base — the reader has acked all SNs < `base`.
    /// `base` corresponds to `reader_sn_state.bitmap_base`.
    pub fn acked_changes_set(&mut self, base: SequenceNumber) {
        let new_acked = SequenceNumber(base.0 - 1);
        if new_acked > self.highest_acked_sn {
            self.highest_acked_sn = new_acked;
        }
        // Remove already-acked SNs from requested.
        self.requested_changes
            .retain(|sn| *sn > self.highest_acked_sn);
        // Analogously for fragment-granular requests.
        self.requested_fragments
            .retain(|sn, _| *sn > self.highest_acked_sn);
    }

    /// Remembers the SNs requested in the ACKNACK bitmap for re-send.
    pub fn requested_changes_set(&mut self, sns: impl IntoIterator<Item = SequenceNumber>) {
        for sn in sns {
            if sn > self.highest_acked_sn {
                self.requested_changes.insert(sn);
            }
        }
    }

    /// Pulls the smallest open requested SN and removes it.
    pub fn next_requested_change(&mut self) -> Option<SequenceNumber> {
        let sn = *self.requested_changes.iter().next()?;
        self.requested_changes.remove(&sn);
        Some(sn)
    }

    /// Returns the next not-yet-sent SN, if present in the cache.
    ///
    /// `cache_max` is the largest SN currently in the writer cache.
    pub fn next_unsent_change(&mut self, cache_max: SequenceNumber) -> Option<SequenceNumber> {
        if self.highest_sent_sn < cache_max {
            let next = SequenceNumber(self.highest_sent_sn.0 + 1);
            self.highest_sent_sn = next;
            Some(next)
        } else {
            None
        }
    }

    /// True if there are still unacknowledged samples between `highest_acked`
    /// and `cache_max`.
    #[must_use]
    pub fn unacked_changes(&self, cache_max: SequenceNumber) -> bool {
        cache_max > self.highest_acked_sn
    }

    /// Getter for `highest_acked_sn`.
    #[must_use]
    pub fn highest_acked_sn(&self) -> SequenceNumber {
        self.highest_acked_sn
    }

    /// Getter for `highest_sent_sn`.
    #[must_use]
    pub fn highest_sent_sn(&self) -> SequenceNumber {
        self.highest_sent_sn
    }

    /// Number of queued resend requests.
    #[must_use]
    pub fn pending_requested_count(&self) -> usize {
        self.requested_changes.len()
    }

    /// Remembers fragment-granular resend requests from a NACK_FRAG.
    /// SN values ≤ `highest_acked_sn` are ignored.
    pub fn requested_fragments_set(
        &mut self,
        sn: SequenceNumber,
        fragments: impl IntoIterator<Item = FragmentNumber>,
    ) {
        if sn <= self.highest_acked_sn {
            return;
        }
        let entry = self.requested_fragments.entry(sn).or_default();
        for f in fragments {
            if f != FragmentNumber::UNKNOWN {
                entry.insert(f);
            }
        }
        if entry.is_empty() {
            self.requested_fragments.remove(&sn);
        }
    }

    /// Pulls the smallest open (SN, FragmentNumber) pair and removes it.
    pub fn next_requested_fragment(&mut self) -> Option<(SequenceNumber, FragmentNumber)> {
        let sn = *self.requested_fragments.keys().next()?;
        let frag = {
            let set = self.requested_fragments.get_mut(&sn)?;
            let f = *set.iter().next()?;
            set.remove(&f);
            f
        };
        if self
            .requested_fragments
            .get(&sn)
            .is_some_and(alloc::collections::BTreeSet::is_empty)
        {
            self.requested_fragments.remove(&sn);
        }
        Some((sn, frag))
    }

    /// Number of queued fragment resends (sum over all SNs).
    #[must_use]
    pub fn pending_requested_fragment_count(&self) -> usize {
        self.requested_fragments.values().map(BTreeSet::len).sum()
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

    fn proxy() -> ReaderProxy {
        let guid = Guid::new(
            GuidPrefix::from_bytes([1; 12]),
            EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0]),
        );
        ReaderProxy::new(guid, alloc::vec![], alloc::vec![], true)
    }

    #[test]
    fn fresh_proxy_has_zero_state() {
        let p = proxy();
        assert_eq!(p.highest_acked_sn(), sn(0));
        assert_eq!(p.highest_sent_sn(), sn(0));
        assert_eq!(p.pending_requested_count(), 0);
    }

    #[test]
    fn acked_changes_set_monotonic() {
        let mut p = proxy();
        p.acked_changes_set(sn(5));
        assert_eq!(p.highest_acked_sn(), sn(4));
        // Backwards acks are ignored
        p.acked_changes_set(sn(3));
        assert_eq!(p.highest_acked_sn(), sn(4));
        p.acked_changes_set(sn(10));
        assert_eq!(p.highest_acked_sn(), sn(9));
    }

    #[test]
    fn requested_changes_set_above_ack_only() {
        let mut p = proxy();
        p.acked_changes_set(sn(5)); // → highest_acked = 4
        p.requested_changes_set([sn(2), sn(4), sn(6), sn(8)]);
        // Only SN > 4 survive
        assert_eq!(p.pending_requested_count(), 2);
    }

    #[test]
    fn next_requested_change_pulls_smallest_first() {
        let mut p = proxy();
        p.requested_changes_set([sn(8), sn(3), sn(5)]);
        assert_eq!(p.next_requested_change(), Some(sn(3)));
        assert_eq!(p.next_requested_change(), Some(sn(5)));
        assert_eq!(p.next_requested_change(), Some(sn(8)));
        assert_eq!(p.next_requested_change(), None);
    }

    #[test]
    fn next_unsent_change_walks_sequentially() {
        let mut p = proxy();
        let cache_max = sn(3);
        assert_eq!(p.next_unsent_change(cache_max), Some(sn(1)));
        assert_eq!(p.next_unsent_change(cache_max), Some(sn(2)));
        assert_eq!(p.next_unsent_change(cache_max), Some(sn(3)));
        assert_eq!(p.next_unsent_change(cache_max), None);
    }

    #[test]
    fn next_unsent_change_picks_up_after_cache_grows() {
        let mut p = proxy();
        assert_eq!(p.next_unsent_change(sn(2)), Some(sn(1)));
        assert_eq!(p.next_unsent_change(sn(2)), Some(sn(2)));
        assert_eq!(p.next_unsent_change(sn(2)), None);
        assert_eq!(p.next_unsent_change(sn(5)), Some(sn(3)));
    }

    #[test]
    fn unacked_changes_detects_gap() {
        let mut p = proxy();
        assert!(!p.unacked_changes(sn(0)));
        assert!(p.unacked_changes(sn(5)));
        p.acked_changes_set(sn(6)); // → highest_acked = 5
        assert!(!p.unacked_changes(sn(5)));
        assert!(p.unacked_changes(sn(7)));
    }

    #[test]
    fn acking_also_prunes_requested_changes() {
        let mut p = proxy();
        p.requested_changes_set([sn(3), sn(5), sn(7)]);
        assert_eq!(p.pending_requested_count(), 3);
        p.acked_changes_set(sn(6)); // → highest_acked = 5
        // sn(3) and sn(5) are now obsolete
        assert_eq!(p.pending_requested_count(), 1);
        assert_eq!(p.next_requested_change(), Some(sn(7)));
    }

    fn frag(n: u32) -> FragmentNumber {
        FragmentNumber(n)
    }

    #[test]
    fn requested_fragments_set_above_ack_only() {
        let mut p = proxy();
        p.acked_changes_set(sn(3)); // → highest_acked = 2
        p.requested_fragments_set(sn(2), [frag(1), frag(2)]);
        p.requested_fragments_set(sn(5), [frag(1), frag(3)]);
        assert_eq!(p.pending_requested_fragment_count(), 2);
    }

    #[test]
    fn next_requested_fragment_pulls_smallest_sn_first() {
        let mut p = proxy();
        p.requested_fragments_set(sn(5), [frag(3), frag(1)]);
        p.requested_fragments_set(sn(2), [frag(2)]);
        assert_eq!(p.next_requested_fragment(), Some((sn(2), frag(2))));
        assert_eq!(p.next_requested_fragment(), Some((sn(5), frag(1))));
        assert_eq!(p.next_requested_fragment(), Some((sn(5), frag(3))));
        assert_eq!(p.next_requested_fragment(), None);
    }

    #[test]
    fn acking_also_prunes_requested_fragments() {
        let mut p = proxy();
        p.requested_fragments_set(sn(3), [frag(1)]);
        p.requested_fragments_set(sn(7), [frag(2)]);
        assert_eq!(p.pending_requested_fragment_count(), 2);
        p.acked_changes_set(sn(5)); // → highest_acked = 4
        // sn(3) is obsolete
        assert_eq!(p.pending_requested_fragment_count(), 1);
        assert_eq!(p.next_requested_fragment(), Some((sn(7), frag(2))));
    }

    #[test]
    fn requested_fragments_ignore_unknown_sentinel() {
        let mut p = proxy();
        p.requested_fragments_set(sn(1), [FragmentNumber::UNKNOWN, frag(1)]);
        assert_eq!(p.pending_requested_fragment_count(), 1);
    }

    // ---- Spec §8.4.15.6 inactive-reader reclaim ----

    #[test]
    fn proxy_is_inactive_initially_when_threshold_is_short() {
        // Initial last_activity = ZERO. If the writer checks with
        // now=10s + threshold=1s, the proxy is inactive.
        let p = proxy();
        assert!(p.is_inactive(
            core::time::Duration::from_secs(10),
            core::time::Duration::from_secs(1)
        ));
    }

    #[test]
    fn proxy_is_active_after_note_activity() {
        let mut p = proxy();
        p.note_activity(core::time::Duration::from_secs(5));
        assert_eq!(p.last_activity(), core::time::Duration::from_secs(5));
        // Within the threshold the proxy is active.
        assert!(!p.is_inactive(
            core::time::Duration::from_secs(6),
            core::time::Duration::from_secs(2)
        ));
    }

    #[test]
    fn proxy_becomes_inactive_after_threshold_elapses() {
        let mut p = proxy();
        p.note_activity(core::time::Duration::from_secs(5));
        // 10 seconds later, threshold 2s → inactive.
        assert!(p.is_inactive(
            core::time::Duration::from_secs(15),
            core::time::Duration::from_secs(2)
        ));
    }

    #[test]
    fn proxy_inactivity_not_reported_when_now_before_last_activity() {
        // Edge case: now < last_activity (clock skew or similar) → no
        // inactive report.
        let mut p = proxy();
        p.note_activity(core::time::Duration::from_secs(100));
        assert!(!p.is_inactive(
            core::time::Duration::from_secs(50),
            core::time::Duration::from_secs(1)
        ));
    }
}
