// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `WriterProxy` — Reader-seitiger Zustand ueber **einen** Remote-Writer.
//!
//! DDSI-RTPS 2.5 §8.4.6.5 (Stateful Reader behavior). Der Reader fuehrt
//! pro matched Writer einen `WriterProxy`, in dem er die Range
//! `[first_available_sn, last_available_sn]` aus HEARTBEATs mitverfolgt,
//! bereits empfangene SNs markiert und fehlende als **missing** erkennt.
//! Die missing-Menge speist den AckNack-Bitmap.
//!
//! ein Reader hat aktuell einen Writer (Single-Writer-
//! Annahme).

extern crate alloc;
use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::wire_types::{Guid, Locator, SequenceNumber};

/// Reader-seitiger State fuer einen Remote-Writer.
#[derive(Debug, Clone)]
pub struct WriterProxy {
    /// GUID des Remote-Writer-Endpoints.
    pub remote_writer_guid: Guid,
    /// Unicast-Locators des Writers (z.B. fuer gerichtete Re-Sends).
    pub unicast_locators: Vec<Locator>,
    /// Multicast-Locators.
    pub multicast_locators: Vec<Locator>,
    /// Reliable-Kind.
    pub is_reliable: bool,
    /// Kleinste SN, die der Writer **noch** im Cache haelt (aus HEARTBEAT.first_sn).
    first_available_sn: SequenceNumber,
    /// Groesste SN, die der Writer annonciert hat (aus HEARTBEAT.last_sn).
    last_available_sn: SequenceNumber,
    /// Hoechste SN, die dieser Reader tatsaechlich **empfangen** hat.
    highest_received_sn: SequenceNumber,
    /// Bereits empfangene SNs (fuer Dup-Rejection + in-order Delivery).
    received: BTreeSet<SequenceNumber>,
    /// Von GAP-Submessages als irrelevant markierte SNs.
    irrelevant: BTreeSet<SequenceNumber>,
}

impl WriterProxy {
    /// Erzeugt einen frischen Proxy.
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

    /// Verarbeitet einen HEARTBEAT.
    ///
    /// Gemaess §8.4.15: `first_sn` ist die kleinste SN, die der Writer
    /// re-liefern kann; `last_sn` die groesste annoncierte.
    pub fn update_from_heartbeat(&mut self, first_sn: SequenceNumber, last_sn: SequenceNumber) {
        // Monoton wachsende Bounds.
        if first_sn > self.first_available_sn {
            self.first_available_sn = first_sn;
            // SNs, die **vor** first_sn liegen, sind verloren und koennen
            // nicht mehr angefragt werden — aus received/irrelevant werfen
            // wir sie; sie werden auch nicht mehr missing sein.
            let split = self.received.split_off(&first_sn);
            self.received = split;
            let split = self.irrelevant.split_off(&first_sn);
            self.irrelevant = split;
        }
        if last_sn > self.last_available_sn {
            self.last_available_sn = last_sn;
        }
    }

    /// Markiert eine SN als empfangen.
    pub fn received_change_set(&mut self, sn: SequenceNumber) {
        if sn < self.first_available_sn {
            // Liegt vor dem annoncierten Range — ignorieren.
            return;
        }
        self.received.insert(sn);
        if sn > self.highest_received_sn {
            self.highest_received_sn = sn;
        }
    }

    /// Markiert eine SN als irrelevant (per GAP).
    pub fn irrelevant_change_set(&mut self, sn: SequenceNumber) {
        if sn < self.first_available_sn {
            return;
        }
        self.irrelevant.insert(sn);
    }

    /// True wenn SN bereits empfangen oder als irrelevant markiert.
    #[must_use]
    pub fn is_known(&self, sn: SequenceNumber) -> bool {
        self.received.contains(&sn) || self.irrelevant.contains(&sn)
    }

    /// Liefert alle **fehlenden** SNs (weder empfangen noch irrelevant) im
    /// Bereich `[first_available_sn, last_available_sn]`.
    ///
    /// Vektor ist nach SN aufsteigend sortiert. Begrenzt auf `max_count`
    /// Eintraege — der erwartete RTPS-Bitmap-Window ist 256 SNs.
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

    /// True wenn fehlende SNs vorhanden sind.
    #[must_use]
    pub fn has_missing_changes(&self) -> bool {
        !self.missing_changes(1).is_empty()
    }

    /// Getter: kleinste annoncierte SN.
    #[must_use]
    pub fn first_available_sn(&self) -> SequenceNumber {
        self.first_available_sn
    }

    /// Getter: groesste annoncierte SN.
    #[must_use]
    pub fn last_available_sn(&self) -> SequenceNumber {
        self.last_available_sn
    }

    /// Getter: hoechste empfangene SN.
    #[must_use]
    pub fn highest_received_sn(&self) -> SequenceNumber {
        self.highest_received_sn
    }

    /// Passender AckNack-Base: kleinste noch nicht acked SN.
    ///
    /// Convention: alle SN < `acknack_base` sind acked. Wir liefern
    /// die kleinste noch-nicht-empfangene-oder-irrelevante SN in `[first, last+1]`.
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
        // Noch nichts empfangen → alles missing
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
        // Writer rotiert Cache → first jetzt bei 5
        p.update_from_heartbeat(sn(5), sn(10));
        assert_eq!(p.first_available_sn(), sn(5));
        // sn(3) aus received entfernt, sn(7) bleibt
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
