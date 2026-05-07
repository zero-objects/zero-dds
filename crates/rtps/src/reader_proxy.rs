// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `ReaderProxy` — Writer-seitiger Zustand ueber **einen** Remote-Reader.
//!
//! DDSI-RTPS 2.5 §8.4.4.11 (Stateful Writer behavior). Der Writer fuehrt
//! pro matched Reader einen `ReaderProxy`, in dem er mitverfolgt, welche
//! Sequence-Numbers der Reader bereits acked hat und welche er explizit
//! re-requested hat.
//!
//! ein Writer hat aktuell nur einen Reader (Single-Reader-
//! Annahme). Trotzdem ist die Datenstruktur so geschnitten, dass spaeter
//! `Vec<ReaderProxy>` moeglich ist.

extern crate alloc;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use crate::wire_types::{FragmentNumber, Guid, Locator, SequenceNumber};

/// Writer-seitiger State fuer einen Remote-Reader.
#[derive(Debug, Clone)]
pub struct ReaderProxy {
    /// GUID des Remote-Reader-Endpoints.
    pub remote_reader_guid: Guid,
    /// Unicast-Empfangs-Locator(s) des Readers.
    pub unicast_locators: Vec<Locator>,
    /// Multicast-Empfangs-Locator(s).
    pub multicast_locators: Vec<Locator>,
    /// Reliable-Kind (immer true in WP 1.1).
    pub is_reliable: bool,
    /// Hoechste SN, die der Reader **bereits acked** hat
    /// (aus AckNack.reader_sn_state.bitmap_base - 1).
    highest_acked_sn: SequenceNumber,
    /// Hoechste SN, die der Writer an diesen Reader **bereits gesendet** hat.
    highest_sent_sn: SequenceNumber,
    /// Set von Requested SNs aus AckNack.bitmap, fuer Re-Send vorgemerkt.
    requested_changes: BTreeSet<SequenceNumber>,
    /// Pro Sample-SN: Set fehlender FragmentNumbers, die der Reader via
    /// NACK_FRAG angefragt hat. Fuer Fragment-granulare Re-Sends.
    requested_fragments: BTreeMap<SequenceNumber, BTreeSet<FragmentNumber>>,
    /// Spec §8.4.15.6 Inactive-Reader-Reclaim: letzte beobachtete
    /// Reader-Aktivitaet (eingehender ACKNACK / NACK_FRAG). Der Writer
    /// ruft `note_activity(now)` aus dem ACKNACK-Pfad. Wenn
    /// `now - last_activity > inactive_threshold`, kann der Writer den
    /// Proxy via `is_inactive` als reclaim-Kandidat erkennen, um
    /// Strict-Reliability nicht den Cache OOM laufen zu lassen.
    last_activity: core::time::Duration,
    /// XTypes 1.3 §7.6.3.1 — pro-Reader ausgehandeltes Wire-Format.
    /// Default `XCDR2` (=2). Bei Match wird das Field via
    /// `data_representation::negotiate(writer_offered, reader_accepted, mode)`
    /// gesetzt, sonst bleibt es Default. Encap-Header bei sample-write
    /// nutzt `data_representation::encap_for_final_le(this)`.
    negotiated_data_representation: i16,
}

impl ReaderProxy {
    /// Erzeugt einen frischen Proxy.
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
            // Pre-existing state: nichts acked, nichts gesendet. SN beginnt
            // bei 1; "0 acked" heisst "nichts acked" (§8.7.4).
            highest_acked_sn: SequenceNumber(0),
            highest_sent_sn: SequenceNumber(0),
            requested_changes: BTreeSet::new(),
            requested_fragments: BTreeMap::new(),
            last_activity: core::time::Duration::ZERO,
            // Default: XCDR2 (modern). SEDP-Match-Pfad ueberschreibt
            // mit dem ausgehandelten Wert.
            negotiated_data_representation: crate::publication_data::data_representation::XCDR2,
        }
    }

    /// Setzt das ausgehandelte Wire-Format fuer diesen Reader.
    /// Wird vom DCPS-SEDP-Match-Pfad nach `negotiate(...)` aufgerufen.
    pub fn set_negotiated_data_representation(&mut self, id: i16) {
        self.negotiated_data_representation = id;
    }

    /// Liefert das ausgehandelte Wire-Format.
    #[must_use]
    pub fn negotiated_data_representation(&self) -> i16 {
        self.negotiated_data_representation
    }

    /// Spec §8.4.15.6 — markiert eingehende Reader-Aktivitaet (jeder
    /// ACKNACK / NACK_FRAG ruft das aus dem Receiver-Pfad).
    pub fn note_activity(&mut self, now: core::time::Duration) {
        self.last_activity = now;
    }

    /// Spec §8.4.15.6 — `true` wenn der Reader laenger als `threshold`
    /// keine Aktivitaet gezeigt hat. Caller (z.B. ReliableWriter)
    /// nutzt das, um den Proxy aus der `matched_readers`-Liste zu
    /// reclaimen, sodass Strict-Reliability nicht den Cache OOM
    /// laufen laesst.
    #[must_use]
    pub fn is_inactive(&self, now: core::time::Duration, threshold: core::time::Duration) -> bool {
        now.checked_sub(self.last_activity)
            .is_some_and(|elapsed| elapsed > threshold)
    }

    /// Liefert den Last-Activity-Zeitpunkt (Diagnose).
    #[must_use]
    pub fn last_activity(&self) -> core::time::Duration {
        self.last_activity
    }

    /// Markiert Samples bis zu und einschliesslich `sn` als "nicht
    /// mehr relevant" fuer diesen Proxy — sowohl gesendet als auch
    /// acked. Wird z.B. bei Volatile-Durability aufgerufen, wenn ein
    /// neuer Reader-Proxy hinzukommt: der soll keine Historic-Samples
    /// bekommen, also springen wir direkt auf den aktuellen Cache-
    /// Stand.
    ///
    /// Spec-Bezug: OMG DDS 1.4 §2.2.3.4 DurabilityQosPolicy Volatile:
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

    /// Aktualisiert auf ACKNACK-Base — Reader hat alle SNs < `base` acked.
    /// `base` entspricht `reader_sn_state.bitmap_base`.
    pub fn acked_changes_set(&mut self, base: SequenceNumber) {
        let new_acked = SequenceNumber(base.0 - 1);
        if new_acked > self.highest_acked_sn {
            self.highest_acked_sn = new_acked;
        }
        // Bereits acked SNs aus requested entfernen.
        self.requested_changes
            .retain(|sn| *sn > self.highest_acked_sn);
        // Analog fuer fragment-granulare Requests.
        self.requested_fragments
            .retain(|sn, _| *sn > self.highest_acked_sn);
    }

    /// Merkt sich die im ACKNACK-Bitmap angefragten SNs fuer Re-Send.
    pub fn requested_changes_set(&mut self, sns: impl IntoIterator<Item = SequenceNumber>) {
        for sn in sns {
            if sn > self.highest_acked_sn {
                self.requested_changes.insert(sn);
            }
        }
    }

    /// Zieht die kleinste offene Requested-SN und entfernt sie.
    pub fn next_requested_change(&mut self) -> Option<SequenceNumber> {
        let sn = *self.requested_changes.iter().next()?;
        self.requested_changes.remove(&sn);
        Some(sn)
    }

    /// Liefert die naechste noch nicht gesendete SN, falls im Cache vorhanden.
    ///
    /// `cache_max` ist die groesste SN, die aktuell im Writer-Cache liegt.
    pub fn next_unsent_change(&mut self, cache_max: SequenceNumber) -> Option<SequenceNumber> {
        if self.highest_sent_sn < cache_max {
            let next = SequenceNumber(self.highest_sent_sn.0 + 1);
            self.highest_sent_sn = next;
            Some(next)
        } else {
            None
        }
    }

    /// True wenn zwischen `highest_acked` und `cache_max` noch unbestaetigte
    /// Samples liegen.
    #[must_use]
    pub fn unacked_changes(&self, cache_max: SequenceNumber) -> bool {
        cache_max > self.highest_acked_sn
    }

    /// Getter fuer `highest_acked_sn`.
    #[must_use]
    pub fn highest_acked_sn(&self) -> SequenceNumber {
        self.highest_acked_sn
    }

    /// Getter fuer `highest_sent_sn`.
    #[must_use]
    pub fn highest_sent_sn(&self) -> SequenceNumber {
        self.highest_sent_sn
    }

    /// Anzahl vorgemerkter Resend-Requests.
    #[must_use]
    pub fn pending_requested_count(&self) -> usize {
        self.requested_changes.len()
    }

    /// Merkt sich Fragment-granulare Resend-Requests aus einem NACK_FRAG.
    /// SN-Werte ≤ `highest_acked_sn` werden ignoriert.
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

    /// Zieht das kleinste offene (SN, FragmentNumber)-Paar und entfernt es.
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

    /// Anzahl vorgemerkter Fragment-Resends (Summe ueber alle SNs).
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
        // Rueckwaerts-Acks werden ignoriert
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
        // Nur SN > 4 ueberleben
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
        // sn(3) und sn(5) sind jetzt obsolet
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
        // sn(3) ist obsolet
        assert_eq!(p.pending_requested_fragment_count(), 1);
        assert_eq!(p.next_requested_fragment(), Some((sn(7), frag(2))));
    }

    #[test]
    fn requested_fragments_ignore_unknown_sentinel() {
        let mut p = proxy();
        p.requested_fragments_set(sn(1), [FragmentNumber::UNKNOWN, frag(1)]);
        assert_eq!(p.pending_requested_fragment_count(), 1);
    }

    // ---- Spec §8.4.15.6 Inactive-Reader-Reclaim ----

    #[test]
    fn proxy_is_inactive_initially_when_threshold_is_short() {
        // Initial last_activity = ZERO. Wenn der Writer mit
        // now=10s + threshold=1s prueft, ist der Proxy inactive.
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
        // Innerhalb des Schwellwerts ist der Proxy aktiv.
        assert!(!p.is_inactive(
            core::time::Duration::from_secs(6),
            core::time::Duration::from_secs(2)
        ));
    }

    #[test]
    fn proxy_becomes_inactive_after_threshold_elapses() {
        let mut p = proxy();
        p.note_activity(core::time::Duration::from_secs(5));
        // 10 Sekunden spaeter, threshold 2s → inaktiv.
        assert!(p.is_inactive(
            core::time::Duration::from_secs(15),
            core::time::Duration::from_secs(2)
        ));
    }

    #[test]
    fn proxy_inactivity_not_reported_when_now_before_last_activity() {
        // Edge case: now < last_activity (Clock-Skew o.ae.) → kein
        // Inactive-Report.
        let mut p = proxy();
        p.note_activity(core::time::Duration::from_secs(100));
        assert!(!p.is_inactive(
            core::time::Duration::from_secs(50),
            core::time::Duration::from_secs(1)
        ));
    }
}
