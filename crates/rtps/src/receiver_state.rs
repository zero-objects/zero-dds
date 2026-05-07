// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Receiver-State (DDSI-RTPS 2.5 §8.3.4 + §8.3.7.4).
//!
//! Beim Empfang einer RTPS-Message haelt der Receiver einen Zustand
//! mit:
//!
//! ```text
//!   sourceVersion        — ProtocolVersion aus RTPS-Header
//!   sourceVendorId       — VendorId aus RTPS-Header
//!   sourceGuidPrefix     — GuidPrefix des Senders
//!   destGuidPrefix       — GuidPrefix des Receivers selbst
//!   unicastReplyLocators
//!   multicastReplyLocators
//!   haveTimestamp        — true wenn InfoTimestamp/HE.W gesehen
//!   timestamp            — letzte gesehene Sender-Wallclock
//!   messageLength        — falls vom HE-L-Flag deklariert
//!   messageChecksum      — falls vom HE-C-Feld deklariert
//!   parameters           — falls vom HE-P-Feld deklariert
//!   clockSkewDetected    — Heuristik: |timestamp - now| ueber Schwelle
//! ```
//!
//! Update-Trigger:
//!
//! - **InfoSource** (§8.3.8.9.4): setzt
//!   `sourceVersion`, `sourceVendorId`, `sourceGuidPrefix` auf die in
//!   der InfoSource-Submessage gegebenen Werte; `haveTimestamp = false`
//!   und Reply-Locator-Listen werden auf `LOCATOR_INVALID` resettet.
//! - **InfoTimestamp** (§8.3.8.5.4): setzt `haveTimestamp = true`
//!   bzw. = false bei `I-Flag = 1`, plus `timestamp = …`.
//! - **HeaderExtension** (§8.3.7.4): kombiniert mehrere Wirkungen — L-
//!   Flag aktualisiert `messageLength`; W-Flag setzt
//!   `haveTimestamp = true` + `timestamp`; C-Flag aktualisiert
//!   `messageChecksum`; P-Flag aktualisiert `parameters`.
//!
//! Der Receiver-State ist pro RTPS-Message kurzlebig: Vor jedem
//! `decode_datagram` wird er auf den Defaultwert plus `destGuidPrefix`
//! initialisiert.

extern crate alloc;
use alloc::vec::Vec;

use crate::header::RtpsHeader;
use crate::header_extension::{ChecksumValue, HeTimestamp, HeaderExtension};
use crate::parameter_list::ParameterList;
use crate::wire_types::{GuidPrefix, Locator, ProtocolVersion, VendorId};

/// Receiver-State entsprechend Spec-Tabelle §8.3.4 und Update-Regeln
/// in §8.3.7.4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiverState {
    /// ProtocolVersion aus RTPS-Header (oder von InfoSource ueberschrieben).
    pub source_version: ProtocolVersion,
    /// VendorId aus RTPS-Header (oder von InfoSource ueberschrieben).
    pub source_vendor_id: VendorId,
    /// GuidPrefix des Senders (RTPS-Header oder InfoSource).
    pub source_guid_prefix: GuidPrefix,
    /// GuidPrefix des Receivers (Konfigurations-Wert, fix).
    pub dest_guid_prefix: GuidPrefix,
    /// `true` wenn der Receiver einen Sender-Timestamp hat.
    pub have_timestamp: bool,
    /// Letzter Sender-Timestamp (gueltig wenn `have_timestamp`).
    pub timestamp: HeTimestamp,
    /// Gesetzt durch HE.L — Soll-Restlaenge der RTPS-Message.
    pub message_length: Option<u32>,
    /// Gesetzt durch HE.C — Soll-Checksum der RTPS-Message.
    pub message_checksum: ChecksumValue,
    /// Gesetzt durch HE.P — ParameterList aus dem HE.
    pub parameters: Option<ParameterList>,
    /// Reply-Locator-Listen (Default `LOCATOR_INVALID`-Listen, von
    /// InfoReply ueberschreibbar).
    pub unicast_reply_locator_list: Vec<Locator>,
    /// Reply-Locator-Listen (Default `LOCATOR_INVALID`-Listen, von
    /// InfoReply ueberschreibbar).
    pub multicast_reply_locator_list: Vec<Locator>,
    /// Heuristik-Flag: `|timestamp - now| > Schwelle`. Wird von
    /// `note_clock_skew` gesetzt; das Decode-Modul liefert nur die
    /// Eingangsdaten.
    pub clock_skew_detected: bool,
}

impl ReceiverState {
    /// Initial-Zustand vor Empfang einer Message: alle Felder auf
    /// Spec-Defaults, `dest_guid_prefix` aus dem Receiver-Konfig
    /// uebernommen.
    #[must_use]
    pub fn new(dest_guid_prefix: GuidPrefix) -> Self {
        Self {
            source_version: ProtocolVersion::V2_5,
            source_vendor_id: VendorId([0, 0]),
            source_guid_prefix: GuidPrefix::from_bytes([0; 12]),
            dest_guid_prefix,
            have_timestamp: false,
            timestamp: HeTimestamp::default(),
            message_length: None,
            message_checksum: ChecksumValue::None,
            parameters: None,
            unicast_reply_locator_list: Vec::new(),
            multicast_reply_locator_list: Vec::new(),
            clock_skew_detected: false,
        }
    }

    /// Initialisiert aus einem `RtpsHeader` (Spec §8.3.4.1).
    pub fn init_from_header(&mut self, header: &RtpsHeader) {
        self.source_version = header.protocol_version;
        self.source_vendor_id = header.vendor_id;
        self.source_guid_prefix = header.guid_prefix;
        // Reply-Locator-Listen + haveTimestamp resetten:
        self.unicast_reply_locator_list.clear();
        self.multicast_reply_locator_list.clear();
        self.have_timestamp = false;
    }

    /// Update aus einer InfoSource-Submessage (§8.3.8.9.4).
    ///
    /// > "An InfoSource Submessage MUST set the receiver's source
    /// >  GuidPrefix, source ProtocolVersion, source VendorId, and MUST
    /// >  reset haveTimestamp = false and the reply-locator-lists to
    /// >  LOCATOR_INVALID."
    pub fn apply_info_source(
        &mut self,
        version: ProtocolVersion,
        vendor_id: VendorId,
        guid_prefix: GuidPrefix,
    ) {
        self.source_version = version;
        self.source_vendor_id = vendor_id;
        self.source_guid_prefix = guid_prefix;
        self.have_timestamp = false;
        self.unicast_reply_locator_list.clear();
        self.multicast_reply_locator_list.clear();
    }

    /// Update aus InfoTimestamp (§8.3.8.5.4). `invalidate = true` (also
    /// das I-Flag in der Submessage) loescht den Timestamp.
    pub fn apply_info_timestamp(&mut self, ts: HeTimestamp, invalidate: bool) {
        if invalidate {
            self.have_timestamp = false;
        } else {
            self.have_timestamp = true;
            self.timestamp = ts;
        }
    }

    /// Update aus InfoReply (§8.3.8.10.4): setzt die beiden Reply-
    /// Locator-Listen.
    pub fn apply_info_reply(&mut self, unicast: Vec<Locator>, multicast: Option<Vec<Locator>>) {
        self.unicast_reply_locator_list = unicast;
        if let Some(m) = multicast {
            self.multicast_reply_locator_list = m;
        }
    }

    /// Update aus HeaderExtension (§8.3.7.4). Aktualisiert je nach
    /// gesetzten Flags `messageLength`, `timestamp`, `messageChecksum`
    /// und `parameters`.
    pub fn apply_header_extension(&mut self, he: &HeaderExtension) {
        if let Some(len) = he.message_length {
            self.message_length = Some(len);
        }
        if let Some(ts) = he.timestamp {
            self.have_timestamp = true;
            self.timestamp = ts;
        }
        if !matches!(he.checksum, ChecksumValue::None) {
            self.message_checksum = he.checksum.clone();
        }
        if let Some(pl) = &he.parameters {
            self.parameters = Some(pl.clone());
        }
    }

    /// Setzt das `clock_skew_detected`-Flag, wenn der gegebene
    /// `now`-Sekunden-Wert mehr als `threshold_seconds` vom Sender-
    /// Timestamp abweicht. No-op wenn `!have_timestamp`.
    pub fn note_clock_skew(&mut self, now_seconds: i32, threshold_seconds: u32) {
        if !self.have_timestamp {
            return;
        }
        let diff = (now_seconds as i64).saturating_sub(self.timestamp.seconds as i64);
        if diff.unsigned_abs() > u64::from(threshold_seconds) {
            self.clock_skew_detected = true;
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use crate::header_extension::ChecksumValue;
    use alloc::vec;

    fn dummy_prefix(byte: u8) -> GuidPrefix {
        GuidPrefix::from_bytes([byte; 12])
    }

    #[test]
    fn new_state_has_default_fields() {
        let st = ReceiverState::new(dummy_prefix(7));
        assert!(!st.have_timestamp);
        assert_eq!(st.dest_guid_prefix, dummy_prefix(7));
        assert!(matches!(st.message_checksum, ChecksumValue::None));
        assert!(st.message_length.is_none());
        assert!(!st.clock_skew_detected);
    }

    #[test]
    fn init_from_header_overrides_source_fields() {
        let mut st = ReceiverState::new(dummy_prefix(0));
        let h = RtpsHeader::new(VendorId::ZERODDS, dummy_prefix(0xAB));
        st.init_from_header(&h);
        assert_eq!(st.source_vendor_id, VendorId::ZERODDS);
        assert_eq!(st.source_guid_prefix, dummy_prefix(0xAB));
    }

    #[test]
    fn apply_info_source_resets_reply_locators_and_timestamp() {
        let mut st = ReceiverState::new(dummy_prefix(0));
        st.have_timestamp = true;
        st.unicast_reply_locator_list.push(Locator::INVALID);
        st.apply_info_source(
            ProtocolVersion { major: 2, minor: 5 },
            VendorId([0x42, 0x42]),
            dummy_prefix(0x99),
        );
        assert_eq!(st.source_version, ProtocolVersion { major: 2, minor: 5 });
        assert_eq!(st.source_vendor_id, VendorId([0x42, 0x42]));
        assert_eq!(st.source_guid_prefix, dummy_prefix(0x99));
        assert!(!st.have_timestamp);
        assert!(st.unicast_reply_locator_list.is_empty());
    }

    #[test]
    fn apply_info_timestamp_sets_value() {
        let mut st = ReceiverState::new(dummy_prefix(0));
        st.apply_info_timestamp(
            HeTimestamp {
                seconds: 100,
                fraction: 200,
            },
            false,
        );
        assert!(st.have_timestamp);
        assert_eq!(st.timestamp.seconds, 100);
        assert_eq!(st.timestamp.fraction, 200);
    }

    #[test]
    fn apply_info_timestamp_with_invalidate_clears() {
        let mut st = ReceiverState::new(dummy_prefix(0));
        st.have_timestamp = true;
        st.apply_info_timestamp(HeTimestamp::default(), true);
        assert!(!st.have_timestamp);
    }

    #[test]
    fn apply_info_reply_sets_locators() {
        let mut st = ReceiverState::new(dummy_prefix(0));
        let uni = vec![Locator::INVALID];
        let multi = vec![Locator::INVALID, Locator::INVALID];
        st.apply_info_reply(uni.clone(), Some(multi.clone()));
        assert_eq!(st.unicast_reply_locator_list, uni);
        assert_eq!(st.multicast_reply_locator_list, multi);
    }

    #[test]
    fn apply_header_extension_updates_fields() {
        let mut st = ReceiverState::new(dummy_prefix(0));
        let he = HeaderExtension {
            little_endian: true,
            message_length: Some(99),
            timestamp: Some(HeTimestamp {
                seconds: 1,
                fraction: 2,
            }),
            checksum: ChecksumValue::Crc32c(0xCAFE),
            ..HeaderExtension::default()
        };
        st.apply_header_extension(&he);
        assert_eq!(st.message_length, Some(99));
        assert!(st.have_timestamp);
        assert_eq!(st.timestamp.seconds, 1);
        assert!(matches!(st.message_checksum, ChecksumValue::Crc32c(0xCAFE)));
    }

    #[test]
    fn apply_header_extension_with_parameters_sets_pl() {
        let mut st = ReceiverState::new(dummy_prefix(0));
        let pl = ParameterList::new();
        let he = HeaderExtension {
            little_endian: true,
            parameters: Some(pl.clone()),
            ..HeaderExtension::default()
        };
        st.apply_header_extension(&he);
        assert_eq!(st.parameters, Some(pl));
    }

    #[test]
    fn note_clock_skew_skipped_without_timestamp() {
        let mut st = ReceiverState::new(dummy_prefix(0));
        st.note_clock_skew(1_000_000, 5);
        assert!(!st.clock_skew_detected);
    }

    #[test]
    fn note_clock_skew_within_threshold_does_not_flag() {
        let mut st = ReceiverState::new(dummy_prefix(0));
        st.have_timestamp = true;
        st.timestamp = HeTimestamp {
            seconds: 100,
            fraction: 0,
        };
        st.note_clock_skew(102, 5); // diff 2s, threshold 5s
        assert!(!st.clock_skew_detected);
    }

    #[test]
    fn note_clock_skew_above_threshold_flags() {
        let mut st = ReceiverState::new(dummy_prefix(0));
        st.have_timestamp = true;
        st.timestamp = HeTimestamp {
            seconds: 100,
            fraction: 0,
        };
        st.note_clock_skew(200, 5); // diff 100s, threshold 5s
        assert!(st.clock_skew_detected);
    }
}
