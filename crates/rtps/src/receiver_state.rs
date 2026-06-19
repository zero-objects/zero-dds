// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Receiver state (DDSI-RTPS 2.5 §8.3.4 + §8.3.7.4).
//!
//! On receipt of an RTPS message the receiver keeps a state
//! with:
//!
//! ```text
//!   sourceVersion        — ProtocolVersion aus RTPS-Header
//!   sourceVendorId       — VendorId aus RTPS-Header
//!   sourceGuidPrefix     — GuidPrefix of the sender
//!   destGuidPrefix       — GuidPrefix of the receiver itself
//!   unicastReplyLocators
//!   multicastReplyLocators
//!   haveTimestamp        — true if InfoTimestamp/HE.W was seen
//!   timestamp            — last seen sender wallclock
//!   messageLength        — if declared by the HE L flag
//!   messageChecksum      — if declared by the HE C field
//!   parameters           — if declared by the HE P field
//!   clockSkewDetected    — heuristic: |timestamp - now| over threshold
//! ```
//!
//! Update triggers:
//!
//! - **InfoSource** (§8.3.8.9.4): sets
//!   `sourceVersion`, `sourceVendorId`, `sourceGuidPrefix` to the values
//!   given in the InfoSource submessage; `haveTimestamp = false`
//!   and the reply-locator lists are reset to `LOCATOR_INVALID`.
//! - **InfoTimestamp** (§8.3.8.5.4): sets `haveTimestamp = true`
//!   or = false for `I-Flag = 1`, plus `timestamp = …`.
//! - **HeaderExtension** (§8.3.7.4): combines several effects — the L
//!   flag updates `messageLength`; the W flag sets
//!   `haveTimestamp = true` + `timestamp`; the C flag updates
//!   `messageChecksum`; the P flag updates `parameters`.
//!
//! The receiver state is short-lived per RTPS message: before each
//! `decode_datagram` it is initialized to the default value plus `destGuidPrefix`.

extern crate alloc;
use alloc::vec::Vec;

use crate::header::RtpsHeader;
use crate::header_extension::{ChecksumValue, HeTimestamp, HeaderExtension};
use crate::parameter_list::ParameterList;
use crate::wire_types::{GuidPrefix, Locator, ProtocolVersion, VendorId};

/// Receiver state per spec table §8.3.4 and update rules
/// in §8.3.7.4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiverState {
    /// ProtocolVersion from the RTPS header (or overwritten by InfoSource).
    pub source_version: ProtocolVersion,
    /// VendorId from the RTPS header (or overwritten by InfoSource).
    pub source_vendor_id: VendorId,
    /// GuidPrefix of the sender (RTPS header or InfoSource).
    pub source_guid_prefix: GuidPrefix,
    /// GuidPrefix of the receiver (configuration value, fixed).
    pub dest_guid_prefix: GuidPrefix,
    /// `true` if the receiver has a sender timestamp.
    pub have_timestamp: bool,
    /// Last sender timestamp (valid if `have_timestamp`).
    pub timestamp: HeTimestamp,
    /// Set by HE.L — expected remaining length of the RTPS message.
    pub message_length: Option<u32>,
    /// Set by HE.C — expected checksum of the RTPS message.
    pub message_checksum: ChecksumValue,
    /// Set by HE.P — ParameterList from the HE.
    pub parameters: Option<ParameterList>,
    /// Reply locator lists (default `LOCATOR_INVALID` lists, overridable
    /// by InfoReply).
    pub unicast_reply_locator_list: Vec<Locator>,
    /// Reply locator lists (default `LOCATOR_INVALID` lists, overridable
    /// by InfoReply).
    pub multicast_reply_locator_list: Vec<Locator>,
    /// Heuristic flag: `|timestamp - now| > threshold`. Set by
    /// `note_clock_skew`; the decode module provides only the
    /// input data.
    pub clock_skew_detected: bool,
}

impl ReceiverState {
    /// Initial state before receiving a message: all fields at
    /// spec defaults, `dest_guid_prefix` taken from the receiver config.
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

    /// Initializes from an `RtpsHeader` (Spec §8.3.4.1).
    pub fn init_from_header(&mut self, header: &RtpsHeader) {
        self.source_version = header.protocol_version;
        self.source_vendor_id = header.vendor_id;
        self.source_guid_prefix = header.guid_prefix;
        // Reset reply locator lists + haveTimestamp:
        self.unicast_reply_locator_list.clear();
        self.multicast_reply_locator_list.clear();
        self.have_timestamp = false;
    }

    /// Update from an InfoSource submessage (§8.3.8.9.4).
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

    /// Update from InfoTimestamp (§8.3.8.5.4). `invalidate = true` (i.e.
    /// the I flag in the submessage) clears the timestamp.
    pub fn apply_info_timestamp(&mut self, ts: HeTimestamp, invalidate: bool) {
        if invalidate {
            self.have_timestamp = false;
        } else {
            self.have_timestamp = true;
            self.timestamp = ts;
        }
    }

    /// Update from InfoReply (§8.3.8.10.4): sets the two reply
    /// locator lists.
    pub fn apply_info_reply(&mut self, unicast: Vec<Locator>, multicast: Option<Vec<Locator>>) {
        self.unicast_reply_locator_list = unicast;
        if let Some(m) = multicast {
            self.multicast_reply_locator_list = m;
        }
    }

    /// Update from HeaderExtension (§8.3.7.4). Updates `messageLength`,
    /// `timestamp`, `messageChecksum` and `parameters` depending on the
    /// set flags.
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

    /// Sets the `clock_skew_detected` flag if the given
    /// `now` seconds value deviates from the sender timestamp by more than
    /// `threshold_seconds`. No-op if `!have_timestamp`.
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
