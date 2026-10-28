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

// zerodds-lint: allow no_dyn_in_safe
// (`ChangeFilter` holds the per-reader DDS_FILTER of DDSI-RTPS 2.5 §8.4.7.1 as a
// closure chosen by the caller; its type cannot be named in the proxy.)

extern crate alloc;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use crate::wire_types::{EntityId, FragmentNumber, Guid, Locator, SequenceNumber};

/// `DDS_FILTER(reader_proxy, change)` (DDSI-RTPS 2.5 §8.4.7.1): says
/// whether a change passes the filters of the matched reader (a
/// content-filtered topic, a time-based filter) and is sent to it, or does
/// not and is announced by a GAP with `filteredCount` 1 instead.
#[derive(Clone)]
pub struct ChangeFilter(
    alloc::sync::Arc<dyn Fn(&crate::history_cache::CacheChange) -> bool + Send + Sync>,
);

impl ChangeFilter {
    /// A filter that passes the changes `f` returns `true` for.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&crate::history_cache::CacheChange) -> bool + Send + Sync + 'static,
    {
        Self(alloc::sync::Arc::new(f))
    }

    /// Whether `change` passes.
    #[must_use]
    pub fn passes(&self, change: &crate::history_cache::CacheChange) -> bool {
        (self.0)(change)
    }
}

impl core::fmt::Debug for ChangeFilter {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("ChangeFilter(..)")
    }
}

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
    /// `remoteGroupEntityId` (DDSI-RTPS 2.5 §8.4.7.5, Table 8.61): the group
    /// (Subscriber) the matched reader belongs to, as its SEDP announcement
    /// names it in `PID_GROUP_GUID`; `ENTITYID_UNKNOWN` where it names none.
    pub remote_group_entity_id: EntityId,
    /// `DDS_FILTER` of this reader; `None` is the no-op the spec allows
    /// (§8.2.10.1: "DDS_FILTER may be a noop"): every change passes.
    filter: Option<ChangeFilter>,
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
    /// Where the HEARTBEAT/ACKNACK exchange with this reader stands, for a
    /// writer that has nothing to announce yet. See
    /// [`Self::handshake_owed`].
    handshake: Handshake,
    /// When the last HEARTBEAT went to this reader. See
    /// [`Self::acknack_received`].
    heartbeat_at: Option<core::time::Duration>,
    /// DDSI-RTPS 2.5 §8.4.7.5 `expectsInlineQos`. See
    /// [`Self::expects_inline_qos`].
    expects_inline_qos: bool,
    /// When the oldest repair request still open arrived, for
    /// `nackResponseDelay` (§8.4.7.1). See [`Self::repair_due`].
    nack_since: Option<core::time::Duration>,
    /// Whether the reader's last ACKNACK named as many missing changes as
    /// one `SequenceNumberSet` holds. See [`Self::note_window`].
    window_full: bool,
    /// Whether an ACKNACK with the FinalFlag clear waits for its HEARTBEAT.
    /// See [`Self::heartbeat_asked`].
    heartbeat_asked: bool,
    /// Largest RTPS message the path to this reader carries. See
    /// [`Self::message_limit`].
    message_limit: Option<usize>,
}

/// The first HEARTBEAT/ACKNACK exchange between a writer and a reliable
/// reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Handshake {
    /// No HEARTBEAT has gone to the reader yet.
    Owed,
    /// A HEARTBEAT went out; the reader has not answered one.
    Sent,
    /// The reader answered a HEARTBEAT.
    Done,
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
            remote_group_entity_id: EntityId::UNKNOWN,
            filter: None,
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
            handshake: Handshake::Owed,
            heartbeat_at: None,
            expects_inline_qos: false,
            nack_since: None,
            window_full: false,
            heartbeat_asked: false,
            message_limit: None,
        }
    }

    /// Sets the `DDS_FILTER` of this reader.
    #[must_use]
    pub fn with_filter(mut self, filter: ChangeFilter) -> Self {
        self.filter = Some(filter);
        self
    }

    /// The `DDS_FILTER` of this reader, if it has one.
    #[must_use]
    pub fn filter(&self) -> Option<&ChangeFilter> {
        self.filter.as_ref()
    }

    /// Sets `remoteGroupEntityId` (Table 8.61).
    #[must_use]
    pub fn with_remote_group_entity_id(mut self, group: EntityId) -> Self {
        self.remote_group_entity_id = group;
        self
    }

    /// The largest RTPS message, in octets, that the path to this reader
    /// carries, when it is larger than the writer's own message size.
    /// DDSI-RTPS 2.5 §8.4.14.1 fixes the fragment size of a writer for all
    /// its readers and takes it from the transport with the smallest
    /// maximum message size; it also asks that data is fragmented only
    /// where required ("If multiple transports are available to the Writer
    /// and some transports do not require fragmentation, a regular Data
    /// Submessage must be sent on those transports instead"). A reader on
    /// the same host is reached through the loopback interface, which
    /// carries a sample of that size as one DATA; the fragments of every
    /// other reader keep the writer's fragment size.
    /// `None` is the writer's own message size.
    #[must_use]
    pub fn message_limit(&self) -> Option<usize> {
        self.message_limit
    }

    /// Sets [`Self::message_limit`].
    pub fn set_message_limit(&mut self, limit: Option<usize>) {
        self.message_limit = limit;
    }

    /// Records whether the reader's last ACKNACK named a full
    /// `SequenceNumberSet` (`RTPS_BITMAP_MAX_BITS` missing changes): it may
    /// be missing more than one set can name, and only a HEARTBEAT lets it
    /// ask for the rest (DDSI-RTPS 2.5 §8.4.2.3, a reader sends an ACKNACK
    /// only in response to one).
    pub fn note_window(&mut self, full: bool) {
        self.window_full = full;
    }

    /// Takes the mark [`Self::note_window`] left: `true` once after a full
    /// window.
    pub fn take_full_window(&mut self) -> bool {
        core::mem::take(&mut self.window_full)
    }

    /// Records that a NACK from this reader arrived at `now`, when one is
    /// open: the repair it asked for waits from here
    /// (`nackResponseDelay`, DDSI-RTPS 2.5 §8.4.7.1). An earlier open
    /// request keeps its own, earlier time.
    pub fn note_nack(&mut self, now: core::time::Duration) {
        if self.has_requests() {
            self.nack_since.get_or_insert(now);
        }
    }

    /// Whether the repair this reader asked for may go out at `now`: a
    /// request is open and has waited `delay` (DDSI-RTPS 2.5 §8.4.7.1
    /// `nackResponseDelay`). A request no NACK time was noted for counts
    /// from the first call that sees it. Nothing open resets the clock.
    pub fn repair_due(&mut self, now: core::time::Duration, delay: core::time::Duration) -> bool {
        if !self.has_requests() {
            self.nack_since = None;
            return false;
        }
        let since = *self.nack_since.get_or_insert(now);
        now.saturating_sub(since) >= delay
    }

    /// Clears the NACK time once nothing is open, so the next request
    /// waits from its own arrival.
    pub fn repairs_sent(&mut self) {
        if !self.has_requests() {
            self.nack_since = None;
        }
    }

    fn has_requests(&self) -> bool {
        !self.requested_changes.is_empty() || !self.requested_fragments.is_empty()
    }

    /// Whether the reader expects the writer's QoS in-line with every DATA
    /// (DDSI-RTPS 2.5 §8.4.7.5): what its `PID_EXPECTS_INLINE_QOS` said,
    /// or its participant's (§8.5.3.2). `false` by default (Table 9.19).
    #[must_use]
    pub fn expects_inline_qos(&self) -> bool {
        self.expects_inline_qos
    }

    /// Records whether the reader expects in-line QoS. See
    /// [`Self::expects_inline_qos`].
    pub fn set_expects_inline_qos(&mut self, expects: bool) {
        self.expects_inline_qos = expects;
    }

    /// Whether a writer with nothing written still owes this reader a
    /// HEARTBEAT: until the reader has answered one. OpenDDS 3.34 counts a
    /// reliable writer as matched only after that exchange (measured on
    /// Linux test host: its reader stayed unmatched for 25 s and sent ACKNACKs a
    /// writer without data never answered).
    #[must_use]
    pub fn handshake_owed(&self) -> bool {
        self.is_reliable && self.handshake != Handshake::Done
    }

    /// Records that this reader sent an ACKNACK with the FinalFlag clear:
    /// DDSI-RTPS 2.5 §9.4.5.3, "F=0 means the writer must respond to the
    /// AckNack message with a Heartbeat message." [`Self::heartbeat_sent`]
    /// answers it.
    pub fn heartbeat_asked(&mut self) {
        self.heartbeat_asked = true;
    }

    /// Whether an ACKNACK with the FinalFlag clear waits for its HEARTBEAT
    /// at `now`. One HEARTBEAT that went to this reader less than `spacing`
    /// ago answers it: a peer that clears F in every ACKNACK, also one that
    /// acknowledges everything, would otherwise draw a HEARTBEAT per round
    /// trip.
    #[must_use]
    pub fn heartbeat_is_asked(
        &self,
        now: core::time::Duration,
        spacing: core::time::Duration,
    ) -> bool {
        self.heartbeat_asked
            && self
                .heartbeat_at
                .is_none_or(|at| now.saturating_sub(at) >= spacing)
    }

    /// Records that a HEARTBEAT went to this reader at `at`.
    pub fn heartbeat_sent(&mut self, at: core::time::Duration) {
        self.heartbeat_asked = false;
        self.heartbeat_at = Some(at);
        if self.handshake == Handshake::Owed {
            self.handshake = Handshake::Sent;
        }
    }

    /// Records an ACKNACK from this reader at `now`. It answers a HEARTBEAT
    /// only once one was sent; a preemptive ACKNACK before that does not.
    ///
    /// Once the handshake is done, an ACKNACK to a writer with nothing
    /// written that arrives at least `period` after the last HEARTBEAT
    /// answers none: the reader is asking on its own, as an OpenDDS reader
    /// does every 1, 2, 4 … seconds until a HEARTBEAT reaches it. It
    /// reopens the handshake. Measured on the Linux test host against OpenDDS 3.35
    /// (`writer_dispose_sample_vendors`, 2 of 30 runs unmatched for 25 s):
    /// the one HEARTBEAT went out 1.5 ms after the reader announced
    /// itself, before it held the writer, and its first preemptive
    /// ACKNACK 20 ms later closed the handshake here; the six that
    /// followed were never answered. One answer arriving twice — one
    /// HEARTBEAT reaching two of the reader's locators — comes within the
    /// period and reopens nothing.
    pub fn acknack_received(
        &mut self,
        now: core::time::Duration,
        writer_has_nothing: bool,
        period: core::time::Duration,
    ) {
        match self.handshake {
            Handshake::Sent => self.handshake = Handshake::Done,
            Handshake::Done
                if writer_has_nothing
                    && self
                        .heartbeat_at
                        .is_some_and(|at| now.saturating_sub(at) >= period) =>
            {
                self.handshake = Handshake::Owed;
            }
            Handshake::Owed | Handshake::Done => {}
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
        // What the reader acknowledges it holds, it was sent. A proxy that
        // joined behind the cache and was brought up by repair is in step
        // again, so the writer's next change goes to it directly instead of
        // waiting for the next HEARTBEAT and ACKNACK.
        if self.highest_acked_sn > self.highest_sent_sn {
            self.highest_sent_sn = self.highest_acked_sn;
        }
        // Remove already-acked SNs from requested.
        self.requested_changes
            .retain(|sn| *sn > self.highest_acked_sn);
        // Analogously for fragment-granular requests.
        self.requested_fragments
            .retain(|sn, _| *sn > self.highest_acked_sn);
    }

    /// Remembers the SNs requested in the ACKNACK bitmap for re-send.
    ///
    /// A number this reader acknowledged before is kept as well: DDSI-RTPS
    /// 2.5 §8.4.2.3, "if a Writer receives a negative acknowledgement to a
    /// previously positively acknowledged sample, and the Writer can still
    /// service the request, the Writer should send the sample." A reader
    /// that came back under the same GUID asks from 1 again; a change the
    /// writer no longer holds goes to it as a GAP. The caller passes no
    /// number below the ACKNACK's own base: that one is acknowledged.
    pub fn requested_changes_set(&mut self, sns: impl IntoIterator<Item = SequenceNumber>) {
        for sn in sns {
            if sn.0 > 0 {
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

    /// Records that `sn` went out to this proxy as a repair (DATA or GAP).
    /// The send cursor moves when `sn` is the change it expects next, so a
    /// run of repairs in order brings a lagging proxy into step.
    pub fn repaired(&mut self, sn: SequenceNumber) {
        if sn.0 == self.highest_sent_sn.0 + 1 {
            self.highest_sent_sn = sn;
        }
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

    /// Pulls and removes up to `max` of the open fragments of `sn` that
    /// follow `after` without a gap, the neighbours that one DATA_FRAG with
    /// `fragmentsInSubmessage` = 1 + the count returned can carry along.
    /// Returns `(sn, first, count)`, `None` where `after + 1` is not open.
    pub fn next_requested_fragment_run_after(
        &mut self,
        sn: SequenceNumber,
        after: FragmentNumber,
        max: u16,
    ) -> Option<(SequenceNumber, FragmentNumber, u16)> {
        let mut count = 0u16;
        while count < max {
            let next = FragmentNumber(after.0.saturating_add(u32::from(count) + 1));
            let Some(set) = self.requested_fragments.get_mut(&sn) else {
                break;
            };
            if !set.remove(&next) {
                break;
            }
            if set.is_empty() {
                self.requested_fragments.remove(&sn);
            }
            count += 1;
        }
        (count > 0).then_some((sn, FragmentNumber(after.0.saturating_add(1)), count))
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

    /// §8.4.2.3: a negative acknowledgement of a change acknowledged
    /// before is kept for re-send; the ACKNACK that acknowledges a requested
    /// change removes the request.
    #[test]
    fn a_request_for_an_acknowledged_change_is_kept() {
        let mut p = proxy();
        p.acked_changes_set(sn(5)); // → highest_acked = 4
        p.requested_changes_set([sn(2), sn(4), sn(6), sn(8)]);
        assert_eq!(p.pending_requested_count(), 4);
        p.acked_changes_set(sn(7)); // acknowledges up to 6
        assert_eq!(p.next_requested_change(), Some(sn(8)));
        assert_eq!(p.next_requested_change(), None);
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

    #[test]
    fn the_remote_group_entity_id_is_kept() {
        let group = EntityId::user_reader_with_key([0, 0, 7]);
        assert_eq!(proxy().remote_group_entity_id, EntityId::UNKNOWN);
        assert_eq!(
            proxy()
                .with_remote_group_entity_id(group)
                .remote_group_entity_id,
            group
        );
    }

    #[test]
    fn next_requested_fragment_run_after_pulls_neighbours_of_one_sn() {
        let mut p = proxy();
        p.requested_fragments_set(sn(5), [frag(1), frag(2), frag(3), frag(5), frag(6)]);
        p.requested_fragments_set(sn(6), [frag(4)]);
        assert_eq!(p.next_requested_fragment(), Some((sn(5), frag(1))));
        assert_eq!(
            p.next_requested_fragment_run_after(sn(5), frag(1), 1),
            Some((sn(5), frag(2), 1))
        );
        assert_eq!(
            p.next_requested_fragment_run_after(sn(5), frag(2), 9),
            Some((sn(5), frag(3), 1)),
            "4 is not open"
        );
        assert_eq!(p.next_requested_fragment_run_after(sn(5), frag(3), 9), None);
        assert_eq!(p.next_requested_fragment(), Some((sn(5), frag(5))));
        assert_eq!(
            p.next_requested_fragment_run_after(sn(5), frag(5), 9),
            Some((sn(5), frag(6), 1))
        );
        assert_eq!(p.next_requested_fragment(), Some((sn(6), frag(4))));
        assert_eq!(p.next_requested_fragment(), None);
    }
}
