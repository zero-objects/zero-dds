// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Receiver state (DDSI-RTPS 2.5 §8.3.4 + §8.3.7.4).
//!
//! On receipt of an RTPS message the receiver keeps a state
//! with:
//!
//! ```text
//!   sourceVersion        — ProtocolVersion from the RTPS header
//!   sourceVendorId       — VendorId from the RTPS header
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
//!   and the reply-locator lists are reset to `{ LOCATOR_INVALID }`.
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
    /// Last sender timestamp (valid if `have_timestamp`); `TIME_INVALID`
    /// before an InfoTimestamp (Table 8.17).
    pub timestamp: HeTimestamp,
    /// `rtpsReceptionTimestamp` (Table 8.17): the time the message was
    /// received, `None` is TIME_INVALID. Set by [`Self::note_reception`].
    pub rtps_reception_timestamp: Option<HeTimestamp>,
    /// Set by HE.T — `rtpsSendTimestamp`, the time the message was sent
    /// (DDSI-RTPS 2.5 §8.3.7.4); `None` is TIME_INVALID. Not the source
    /// timestamp of the changes in the message: only an InfoTimestamp sets
    /// `timestamp` (§8.3.8.11).
    pub rtps_send_timestamp: Option<HeTimestamp>,
    /// Set by HE.U — `uExtension4` (§8.3.7.4).
    pub uextension4: Option<[u8; 4]>,
    /// Set by HE.W — `wExtension8` (§8.3.7.4).
    pub wextension8: Option<[u8; 8]>,
    /// Set by HE.L — expected remaining length of the RTPS message.
    pub message_length: Option<u32>,
    /// Set by HE.C — expected checksum of the RTPS message.
    pub message_checksum: ChecksumValue,
    /// Set by HE.P — ParameterList from the HE.
    pub parameters: ParameterList,
    /// Reply locator lists (default `LOCATOR_INVALID` lists, overridable
    /// by InfoReply).
    pub unicast_reply_locator_list: Vec<Locator>,
    /// Reply locator lists (default `LOCATOR_INVALID` lists, overridable
    /// by InfoReply).
    pub multicast_reply_locator_list: Vec<Locator>,
    /// Heuristic flag: `|rtpsSendTimestamp - now| > threshold`
    /// (§8.3.7.4 `CLOCK_SKEW_DETECTED`). Set by `note_clock_skew`; the
    /// decode module has no clock and provides only the input data.
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
            timestamp: HeTimestamp::TIME_INVALID,
            rtps_reception_timestamp: None,
            rtps_send_timestamp: None,
            uextension4: None,
            wextension8: None,
            message_length: None,
            message_checksum: ChecksumValue::None,
            parameters: ParameterList::new(),
            unicast_reply_locator_list: alloc::vec![Locator::INVALID],
            multicast_reply_locator_list: alloc::vec![Locator::INVALID],
            clock_skew_detected: false,
        }
    }

    /// Sets both reply-locator lists to their initial values of Table 8.17
    /// for a message received by the transport of `kind`: the unicast list
    /// holds one locator of that kind with the source address of the
    /// message (`None`: `LOCATOR_ADDRESS_INVALID`, for a transport that
    /// does not give one) and `LOCATOR_PORT_INVALID`; the multicast list
    /// one locator of that kind with `LOCATOR_ADDRESS_INVALID` and
    /// `LOCATOR_PORT_INVALID`.
    pub fn set_initial_reply_locators(
        &mut self,
        kind: crate::wire_types::LocatorKind,
        source_address: Option<[u8; 16]>,
    ) {
        self.unicast_reply_locator_list = alloc::vec![Locator {
            kind,
            port: Locator::PORT_INVALID,
            address: source_address.unwrap_or(Locator::ADDRESS_INVALID),
        }];
        self.multicast_reply_locator_list = alloc::vec![Locator {
            kind,
            port: Locator::PORT_INVALID,
            address: Locator::ADDRESS_INVALID,
        }];
    }

    /// Records the time of reception (`rtpsReceptionTimestamp`) and, with
    /// a threshold, runs the clock-skew heuristic of
    /// [`Self::note_clock_skew`] against the `rtpsSendTimestamp` of the
    /// message. The spec gives no threshold; `None` leaves
    /// `clockSkewDetected` at FALSE.
    pub fn note_reception(&mut self, now: HeTimestamp, skew_threshold_seconds: Option<u32>) {
        self.rtps_reception_timestamp = Some(now);
        if let Some(threshold) = skew_threshold_seconds {
            self.note_clock_skew(now.seconds, threshold);
        }
    }

    /// Initializes from an `RtpsHeader` (Spec §8.3.4.1).
    pub fn init_from_header(&mut self, header: &RtpsHeader) {
        self.source_version = header.protocol_version;
        self.source_vendor_id = header.vendor_id;
        self.source_guid_prefix = header.guid_prefix;
        // Reset reply locator lists + haveTimestamp (Table 8.17). The
        // transport kind and source address are not in the header; the
        // caller that has them sets them with `set_initial_reply_locators`.
        self.unicast_reply_locator_list = alloc::vec![Locator::INVALID];
        self.multicast_reply_locator_list = alloc::vec![Locator::INVALID];
        self.have_timestamp = false;
        self.timestamp = HeTimestamp::TIME_INVALID;
        self.rtps_reception_timestamp = None;
        // What a HeaderExtension set belongs to its message: each message
        // starts from the initial values of Table 8.17.
        self.rtps_send_timestamp = None;
        self.uextension4 = None;
        self.wextension8 = None;
        self.message_length = None;
        self.message_checksum = ChecksumValue::None;
        self.parameters = ParameterList::new();
        self.clock_skew_detected = false;
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
        self.timestamp = HeTimestamp::TIME_INVALID;
        self.unicast_reply_locator_list = alloc::vec![Locator::INVALID];
        self.multicast_reply_locator_list = alloc::vec![Locator::INVALID];
    }

    /// Update from InfoTimestamp (§8.3.8.5.4). `invalidate = true` (i.e.
    /// the I flag in the submessage) clears the timestamp.
    pub fn apply_info_timestamp(&mut self, ts: HeTimestamp, invalidate: bool) {
        if invalidate {
            self.have_timestamp = false;
            self.timestamp = HeTimestamp::TIME_INVALID;
        } else {
            self.have_timestamp = true;
            self.timestamp = ts;
            // §8.3.8.11: "if ( Receiver.clockSkewDetected ) {
            // Receiver.timestamp += Receiver.rtpsReceptionTime -
            // Receiver.rtpsSendTimestamp }".
            if self.clock_skew_detected {
                if let (Some(received), Some(sent)) =
                    (self.rtps_reception_timestamp, self.rtps_send_timestamp)
                {
                    let ticks =
                        |t: HeTimestamp| (i128::from(t.seconds) << 32) | i128::from(t.fraction);
                    let corrected = ticks(ts) + ticks(received) - ticks(sent);
                    self.timestamp = HeTimestamp {
                        seconds: u32::try_from(corrected >> 32).unwrap_or(if corrected < 0 {
                            0
                        } else {
                            u32::MAX
                        }),
                        fraction: u32::try_from(corrected & 0xFFFF_FFFF).unwrap_or(0),
                    };
                }
            }
        }
    }

    /// Update from InfoReply (§8.3.8.10.4): sets the two reply
    /// locator lists.
    pub fn apply_info_reply(&mut self, unicast: Vec<Locator>, multicast: Option<Vec<Locator>>) {
        self.unicast_reply_locator_list = unicast;
        // Without the M flag the multicast list is empty (§8.3.8.9).
        self.multicast_reply_locator_list = multicast.unwrap_or_default();
    }

    /// Update from HeaderExtension (§8.3.7.4): `messageLength`,
    /// `rtpsSendTimestamp`, `uExtension4`, `wExtension8`,
    /// `messageChecksum` and `parameters`, each where its flag is set.
    /// `haveTimestamp` and `timestamp` stay: "Receiver.rtpsSendTimestamp =
    /// HeaderExtension.rtpsSendTimestamp".
    pub fn apply_header_extension(&mut self, he: &HeaderExtension) {
        if let Some(len) = he.message_length {
            self.message_length = Some(len);
        }
        if let Some(ts) = he.timestamp {
            self.rtps_send_timestamp = Some(ts);
        }
        if let Some(u) = he.uextension4 {
            self.uextension4 = Some(u);
        }
        if let Some(w) = he.wextension8 {
            self.wextension8 = Some(w);
        }
        if !matches!(he.checksum, ChecksumValue::None) {
            self.message_checksum = he.checksum.clone();
        }
        if let Some(pl) = &he.parameters {
            self.parameters = pl.clone();
        }
    }

    /// Sets the `clock_skew_detected` flag if the given `now` seconds value
    /// deviates from `rtpsSendTimestamp` by more than `threshold_seconds`
    /// (§8.3.7.4). No-op without one.
    pub fn note_clock_skew(&mut self, now_seconds: u32, threshold_seconds: u32) {
        let Some(sent) = self.rtps_send_timestamp else {
            return;
        };
        let diff = i64::from(now_seconds).saturating_sub(i64::from(sent.seconds));
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
        // Table 8.17: rtpsSendTimestamp TIME_INVALID, no extension fields.
        assert!(st.rtps_send_timestamp.is_none());
        assert!(st.uextension4.is_none() && st.wextension8.is_none());
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
        assert_eq!(st.timestamp, HeTimestamp::TIME_INVALID);
        // §8.3.8.9.4: both lists are `{ LOCATOR_INVALID }` again.
        assert_eq!(st.unicast_reply_locator_list, vec![Locator::INVALID]);
        assert_eq!(st.multicast_reply_locator_list, vec![Locator::INVALID]);
    }

    /// Table 8.17: `sourceVersion` PROTOCOLVERSION, `sourceVendorId`
    /// VENDORID_UNKNOWN, `sourceGuidPrefix` GUIDPREFIX_UNKNOWN, `parameters`
    /// an empty list.
    #[test]
    fn the_initial_source_values_and_parameters_are_those_of_table_8_17() {
        let st = ReceiverState::new(dummy_prefix(5));
        assert_eq!(st.source_version, ProtocolVersion::V2_5);
        assert_eq!(st.source_vendor_id, VendorId([0, 0]));
        assert_eq!(st.source_guid_prefix, GuidPrefix::UNKNOWN);
        assert!(st.parameters.parameters.is_empty());
    }

    /// §8.3.6.4: the Header sets `sourceVersion`, `sourceVendorId` and
    /// `sourceGuidPrefix`, and `haveTimestamp = false`; what a message's
    /// HeaderExtension set before is gone with the next message.
    #[test]
    fn the_header_sets_the_source_and_clears_have_timestamp() {
        let mut st = ReceiverState::new(dummy_prefix(0));
        st.have_timestamp = true;
        st.parameters.push(crate::parameter_list::Parameter::new(
            0x8001,
            vec![1, 2, 3, 4],
        ));
        let mut h = RtpsHeader::new(VendorId([0x01, 0x10]), dummy_prefix(0xCD));
        h.protocol_version = ProtocolVersion { major: 2, minor: 3 };
        st.init_from_header(&h);
        assert_eq!(st.source_version, ProtocolVersion { major: 2, minor: 3 });
        assert_eq!(st.source_vendor_id, VendorId([0x01, 0x10]));
        assert_eq!(st.source_guid_prefix, dummy_prefix(0xCD));
        assert!(!st.have_timestamp);
        assert!(st.parameters.parameters.is_empty());
    }

    /// Table 8.17 initial values.
    #[test]
    fn the_initial_state_is_the_one_of_table_8_17() {
        use crate::wire_types::LocatorKind;
        let mut st = ReceiverState::new(dummy_prefix(1));
        assert_eq!(st.timestamp, HeTimestamp::TIME_INVALID);
        assert_eq!(st.timestamp.seconds, 0xFFFF_FFFF);
        assert_eq!(st.timestamp.fraction, 0xFFFF_FFFF);
        assert!(st.rtps_reception_timestamp.is_none());
        assert_eq!(st.unicast_reply_locator_list, vec![Locator::INVALID]);
        st.set_initial_reply_locators(
            LocatorKind::UdpV4,
            Some([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 0, 0, 7]),
        );
        let u = &st.unicast_reply_locator_list;
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].kind, LocatorKind::UdpV4);
        assert_eq!(u[0].port, Locator::PORT_INVALID);
        assert_eq!(u[0].address[12..], [10, 0, 0, 7]);
        let m = &st.multicast_reply_locator_list;
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].kind, LocatorKind::UdpV4);
        assert_eq!(m[0].address, Locator::ADDRESS_INVALID);
        assert_eq!(m[0].port, Locator::PORT_INVALID);
        // Without a source address (a transport that gives none).
        st.set_initial_reply_locators(LocatorKind::UdpV4, None);
        assert_eq!(
            st.unicast_reply_locator_list[0].address,
            Locator::ADDRESS_INVALID
        );
    }

    /// §8.3.8.9: an InfoReply without the M flag leaves the multicast
    /// list empty, whatever it held before.
    #[test]
    fn an_info_reply_without_the_multicast_flag_empties_the_multicast_list() {
        let mut st = ReceiverState::new(dummy_prefix(0));
        st.apply_info_reply(
            vec![Locator::udp_v4([10, 0, 0, 1], 7411)],
            Some(vec![Locator::udp_v4([239, 255, 0, 1], 7400)]),
        );
        st.apply_info_reply(vec![Locator::udp_v4([10, 0, 0, 2], 7411)], None);
        assert!(st.multicast_reply_locator_list.is_empty());
        assert_eq!(st.unicast_reply_locator_list.len(), 1);
    }

    /// §8.3.8.11: with clockSkewDetected the InfoTimestamp is moved by the
    /// difference between reception and send time of the message.
    #[test]
    fn an_info_timestamp_is_corrected_by_the_clock_skew() {
        let mut st = ReceiverState::new(dummy_prefix(0));
        st.rtps_send_timestamp = Some(HeTimestamp {
            seconds: 1000,
            fraction: 0,
        });
        st.note_reception(
            HeTimestamp {
                seconds: 1100,
                fraction: 1 << 31,
            },
            Some(30),
        );
        assert!(st.clock_skew_detected);
        st.apply_info_timestamp(
            HeTimestamp {
                seconds: 990,
                fraction: 1 << 31,
            },
            false,
        );
        // 990.5 + (1100.5 - 1000) = 1091
        assert_eq!(
            st.timestamp,
            HeTimestamp {
                seconds: 1091,
                fraction: 0
            }
        );
        // Without the skew the value is taken as it is.
        let mut st = ReceiverState::new(dummy_prefix(0));
        st.apply_info_timestamp(
            HeTimestamp {
                seconds: 5,
                fraction: 7,
            },
            false,
        );
        assert_eq!(
            st.timestamp,
            HeTimestamp {
                seconds: 5,
                fraction: 7
            }
        );
    }

    /// Table 8.17: rtpsReceptionTimestamp, and the clock-skew heuristic
    /// that runs on it.
    #[test]
    fn the_reception_time_is_kept_and_checked_against_the_send_time() {
        let mut st = ReceiverState::new(dummy_prefix(0));
        st.rtps_send_timestamp = Some(HeTimestamp {
            seconds: 1000,
            fraction: 0,
        });
        let now = HeTimestamp {
            seconds: 1010,
            fraction: 5,
        };
        st.note_reception(now, Some(30));
        assert_eq!(st.rtps_reception_timestamp, Some(now));
        assert!(!st.clock_skew_detected);
        st.note_reception(
            HeTimestamp {
                seconds: 1100,
                fraction: 0,
            },
            Some(30),
        );
        assert!(st.clock_skew_detected);
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
            uextension4: Some([1, 2, 3, 4]),
            wextension8: Some([5; 8]),
            ..HeaderExtension::default()
        };
        st.apply_header_extension(&he);
        assert_eq!(st.uextension4, Some([1, 2, 3, 4]));
        assert_eq!(st.wextension8, Some([5; 8]));
        assert_eq!(st.message_length, Some(99));
        assert_eq!(st.rtps_send_timestamp.map(|t| t.seconds), Some(1));
        assert!(
            !st.have_timestamp,
            "§8.3.7.4: the send timestamp is no source timestamp"
        );
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
        assert_eq!(st.parameters, pl);
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
        st.rtps_send_timestamp = Some(HeTimestamp {
            seconds: 100,
            fraction: 0,
        });
        st.note_clock_skew(102, 5); // diff 2s, threshold 5s
        assert!(!st.clock_skew_detected);
    }

    #[test]
    fn note_clock_skew_above_threshold_flags() {
        let mut st = ReceiverState::new(dummy_prefix(0));
        st.rtps_send_timestamp = Some(HeTimestamp {
            seconds: 100,
            fraction: 0,
        });
        st.note_clock_skew(200, 5); // diff 100s, threshold 5s
        assert!(st.clock_skew_detected);
    }
}
