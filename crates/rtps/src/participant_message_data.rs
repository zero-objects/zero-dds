// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `ParticipantMessageData` Wire-Encoding (DDSI-RTPS 2.5 §9.6.3.1).
//!
//! Payload structure of the writer liveliness protocol (WLP, §8.4.13).
//! Published periodically by the `BUILTIN_PARTICIPANT_MESSAGE_WRITER`
//! as a DATA submessage on the `DCPSParticipantMessage` topic. Readers
//! use the reception as an implicit `assert_liveliness()` and thereby
//! drive the lease tracking per peer participant.
//!
//! # Wire-Layout (§9.6.3.1)
//!
//! ```text
//! struct ParticipantMessageData {
//!     GuidPrefix_t participantGuidPrefix; // 12 octets
//!     OctetArray4  kind;                  // 4 octets, no byte order
//!     OctetSeq     data;                  // unsigned long length + octets
//! };
//! ```
//!
//! `kind` is four octets, not an integer: `{0,0,0,1}` is
//! AUTOMATIC_LIVELINESS_UPDATE, `{0,0,0,2}` MANUAL_LIVELINESS_UPDATE, and
//! "kind.value\[0\] & 0x80 == 1" marks a vendor-specific kind. This module
//! carries it as the `u32` the four octets spell big-endian, so the
//! vendor bit is the top bit whatever the encapsulation.
//!
//! Cyclone DDS 11 serializes the prefix and the kind as one 16-octet
//! block (`ddsi_participant_message_data_ops = { XG, XO }`,
//! `ddsi_pmd.c:34`); Fast DDS 3.6 writes its instance handle, the prefix
//! and `{0,0,0,kind}`, then a zero length (`WLP.cpp:882-894`). Both are
//! this layout; the 16 octets are not a participant GUID.
//!
//! The `data` sequence is an opaque token. ZeroDDS uses it for
//! MANUAL_BY_TOPIC to transport the topic token (topic hash fingerprint).
//!
//! # CDR encoding
//!
//! Encoded as XCDR1 plain (encapsulation 0x0000 BE / 0x0001 LE) or
//! XCDR2 plain (0x0006 BE / 0x0007 LE). We accept all four encapsulation
//! kinds and write LE by default; only `data.length` follows the byte
//! order.
//!
//! # DoS caps
//!
//! `data.len()` is capped at [`MAX_DATA_LEN`] = 4096 bytes. The encoder
//! does not truncate — the caller must cap before the call — the
//! decoder rejects over-long data with [`WireError::ValueOutOfRange`].

extern crate alloc;
use alloc::vec::Vec;

use crate::error::WireError;
use crate::wire_types::GuidPrefix;

/// CDR-Encapsulation-Header: XCDR1 Plain Big-Endian (`0x0000`).
pub const ENCAPSULATION_CDR_BE: [u8; 2] = [0x00, 0x00];
/// CDR-Encapsulation-Header: XCDR1 Plain Little-Endian (`0x0001`).
pub const ENCAPSULATION_CDR_LE: [u8; 2] = [0x00, 0x01];
/// CDR-Encapsulation-Header: XCDR2 Plain Big-Endian (`0x0006`).
pub const ENCAPSULATION_CDR2_BE: [u8; 2] = [0x00, 0x06];
/// CDR-Encapsulation-Header: XCDR2 Plain Little-Endian (`0x0007`).
pub const ENCAPSULATION_CDR2_LE: [u8; 2] = [0x00, 0x07];

/// DoS cap for the `data` sequence (topic token / vendor opaque).
/// Deliberately chosen small — WLP heartbeats should be lightweight, a
/// peer that sends more is either buggy or malicious.
pub const MAX_DATA_LEN: usize = 4096;

/// `kind` `{0x00, 0x00, 0x00, 0x00}`: PARTICIPANT_MESSAGE_DATA_KIND_UNKNOWN
/// (§9.6.3.1).
pub const PARTICIPANT_MESSAGE_DATA_KIND_UNKNOWN: u32 = 0x0000_0000;

/// `kind` `{0x00, 0x00, 0x00, 0x01}`:
/// PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE (§9.6.3.1).
/// Sent by the builtin WLP writer when LIVELINESS=AUTOMATIC.
pub const PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE: u32 = 0x0000_0001;

/// `kind` `{0x00, 0x00, 0x00, 0x02}`:
/// PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_LIVELINESS_UPDATE (§9.6.3.1).
/// Triggered by `assert_liveliness()` on the `DomainParticipant`.
pub const PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE: u32 = 0x0000_0002;

/// Vendor-specific kinds: "kind.value\[0\] & 0x80 == 1 // vendor specific
/// kind" (§9.6.3.1), the top bit of the big-endian `u32`.
pub const PARTICIPANT_MESSAGE_DATA_KIND_VENDOR_BASE: u32 = 0x8000_0000;

/// ZeroDDS vendor kind `{0x80, 0x00, 0x00, 0x01}`: MANUAL_BY_TOPIC.
/// Triggered by `DataWriter::assert_liveliness()`; the `data` sequence
/// carries a topic token (typically a 4-byte hash). A peer that does not
/// know the kind renews the participant's liveliness at most.
pub const PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC: u32 = 0x8000_0001;

/// `ParticipantMessageData` (DDSI-RTPS 2.5 §9.6.3.1) — payload of the
/// `DCPSParticipantMessage` topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantMessageData {
    /// `participantGuidPrefix`: the sender's GuidPrefix.
    pub participant_guid_prefix: GuidPrefix,
    /// `kind`, the four octets as a big-endian `u32` (see
    /// `PARTICIPANT_MESSAGE_DATA_KIND_*`).
    pub kind: u32,
    /// Opaque token. On MANUAL_BY_TOPIC a topic hash, otherwise empty.
    pub data: Vec<u8>,
}

impl ParticipantMessageData {
    /// Constructor for an AUTOMATIC heartbeat (data empty).
    #[must_use]
    pub fn automatic(prefix: GuidPrefix) -> Self {
        Self {
            participant_guid_prefix: prefix,
            kind: PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE,
            data: Vec::new(),
        }
    }

    /// Constructor for a MANUAL_BY_PARTICIPANT heartbeat (data empty).
    #[must_use]
    pub fn manual_by_participant(prefix: GuidPrefix) -> Self {
        Self {
            participant_guid_prefix: prefix,
            kind: PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE,
            data: Vec::new(),
        }
    }

    /// Constructor for ZeroDDS MANUAL_BY_TOPIC (data = topic token).
    #[must_use]
    pub fn manual_by_topic(prefix: GuidPrefix, topic_token: Vec<u8>) -> Self {
        Self {
            participant_guid_prefix: prefix,
            kind: PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC,
            data: topic_token,
        }
    }

    /// Encodes to CDR bytes (with a 4-byte encapsulation header).
    /// `little_endian = true` → `ENCAPSULATION_CDR_LE`, otherwise BE.
    ///
    /// # Errors
    /// `WireError::ValueOutOfRange` if `data.len() > MAX_DATA_LEN`.
    pub fn to_cdr(&self, little_endian: bool) -> Result<Vec<u8>, WireError> {
        if self.data.len() > MAX_DATA_LEN {
            return Err(WireError::ValueOutOfRange {
                message: "ParticipantMessageData.data exceeds MAX_DATA_LEN",
            });
        }
        let data_len_u32 =
            u32::try_from(self.data.len()).map_err(|_| WireError::ValueOutOfRange {
                message: "ParticipantMessageData.data length exceeds u32",
            })?;
        let mut out = Vec::with_capacity(4 + 12 + 4 + 4 + self.data.len());
        if little_endian {
            out.extend_from_slice(&ENCAPSULATION_CDR_LE);
        } else {
            out.extend_from_slice(&ENCAPSULATION_CDR_BE);
        }
        out.extend_from_slice(&[0, 0]); // options
        out.extend_from_slice(&self.participant_guid_prefix.to_bytes());
        // `kind` is an octet array: the same four octets in either order.
        out.extend_from_slice(&self.kind.to_be_bytes());
        out.extend_from_slice(&if little_endian {
            data_len_u32.to_le_bytes()
        } else {
            data_len_u32.to_be_bytes()
        });
        out.extend_from_slice(&self.data);
        Ok(out)
    }

    /// Decodes from CDR bytes (with an encapsulation header).
    ///
    /// Accepts `0x0000`/`0x0001` (XCDR1 plain) and `0x0006`/`0x0007`
    /// (XCDR2 plain). Other encapsulation kinds → error. Octets after the
    /// `data` sequence (Fast DDS pads four) are padding.
    ///
    /// # Errors
    /// - `UnsupportedEncapsulation` on non-CDR encapsulation
    /// - `UnexpectedEof` if the bytes are too short for header / body
    /// - `ValueOutOfRange` if `data.len > MAX_DATA_LEN`
    pub fn from_cdr(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() < 4 {
            return Err(WireError::UnexpectedEof {
                needed: 4,
                offset: 0,
            });
        }
        let little_endian = match (bytes[0], bytes[1]) {
            (0x00, 0x00) | (0x00, 0x06) => false,
            (0x00, 0x01) | (0x00, 0x07) => true,
            (a, b) => {
                return Err(WireError::UnsupportedEncapsulation { kind: [a, b] });
            }
        };
        // Body starts at offset 4 (after options).
        let body = &bytes[4..];
        if body.len() < 20 {
            return Err(WireError::UnexpectedEof {
                needed: 20,
                offset: 4,
            });
        }
        let mut prefix = [0u8; 12];
        prefix.copy_from_slice(&body[..12]);
        let kind = u32::from_be_bytes([body[12], body[13], body[14], body[15]]);
        let len = [body[16], body[17], body[18], body[19]];
        let data_len = if little_endian {
            u32::from_le_bytes(len)
        } else {
            u32::from_be_bytes(len)
        } as usize;
        if data_len > MAX_DATA_LEN {
            return Err(WireError::ValueOutOfRange {
                message: "ParticipantMessageData.data exceeds MAX_DATA_LEN",
            });
        }
        if body.len() < 20 + data_len {
            return Err(WireError::UnexpectedEof {
                needed: 20 + data_len,
                offset: 4,
            });
        }
        Ok(Self {
            participant_guid_prefix: GuidPrefix::from_bytes(prefix),
            kind,
            data: body[20..20 + data_len].to_vec(),
        })
    }

    /// Returns the GuidPrefix.
    #[must_use]
    pub fn prefix(&self) -> GuidPrefix {
        self.participant_guid_prefix
    }

    /// `true` if the `kind` value is vendor-specific (`kind.value[0] &
    /// 0x80`).
    #[must_use]
    pub fn is_vendor_kind(&self) -> bool {
        self.kind & PARTICIPANT_MESSAGE_DATA_KIND_VENDOR_BASE != 0
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;
    use alloc::vec;

    fn sample_prefix() -> GuidPrefix {
        GuidPrefix::from_bytes([0xA, 0xB, 0xC, 0xD, 1, 2, 3, 4, 5, 6, 7, 8])
    }

    #[test]
    fn participant_message_data_automatic_default_data_empty() {
        let m = ParticipantMessageData::automatic(sample_prefix());
        assert_eq!(
            m.kind,
            PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE
        );
        assert!(m.data.is_empty());
        assert_eq!(m.prefix(), sample_prefix());
    }

    /// §9.6.3.1: `PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE
    /// {0x00, 0x00, 0x00, 0x01}`, `…_MANUAL_LIVELINESS_UPDATE {0x00, 0x00,
    /// 0x00, 0x02}`, "kind.value[0] & 0x80 == 1 // vendor specific kind".
    #[test]
    fn participant_message_data_kind_constants_match_spec() {
        assert_eq!(
            PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE.to_be_bytes(),
            [0, 0, 0, 1]
        );
        assert_eq!(
            PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE.to_be_bytes(),
            [0, 0, 0, 2]
        );
        assert_eq!(PARTICIPANT_MESSAGE_DATA_KIND_UNKNOWN.to_be_bytes(), [0; 4]);
        assert_eq!(
            PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC.to_be_bytes()[0] & 0x80,
            0x80
        );
    }

    /// The octets of an AUTOMATIC beat, little-endian: prefix, kind
    /// `{0,0,0,1}`, length 0 — the layout Cyclone's `{ XG, XO }` and Fast
    /// DDS's `send_liveliness_message` put on the wire.
    #[test]
    fn participant_message_data_octets_are_those_of_9_6_3_1() {
        let le = ParticipantMessageData::automatic(sample_prefix())
            .to_cdr(true)
            .unwrap();
        let mut expected = vec![0x00, 0x01, 0x00, 0x00];
        expected.extend_from_slice(&sample_prefix().to_bytes());
        expected.extend_from_slice(&[0, 0, 0, 1, 0, 0, 0, 0]);
        assert_eq!(le, expected);
        // Big-endian: only the length changes byte order, and it is 0 —
        // a manual beat with a token shows the difference.
        let be = ParticipantMessageData::manual_by_topic(sample_prefix(), vec![9])
            .to_cdr(false)
            .unwrap();
        assert_eq!(&be[16..], &[0x80, 0, 0, 1, 0, 0, 0, 1, 9]);
    }

    /// Fast DDS 3.6 pads four zero octets after the empty `data`
    /// (`serializedPayload.length = 4 + 12 + 4 + 4 + 4`).
    #[test]
    fn participant_message_data_reads_the_fast_dds_beat() {
        let mut bytes = vec![0x00, 0x01, 0x00, 0x00];
        bytes.extend_from_slice(&sample_prefix().to_bytes());
        bytes.extend_from_slice(&[0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0]);
        let m = ParticipantMessageData::from_cdr(&bytes).unwrap();
        assert_eq!(
            m,
            ParticipantMessageData::manual_by_participant(sample_prefix())
        );
    }

    #[test]
    fn participant_message_data_roundtrip_le() {
        let m = ParticipantMessageData::manual_by_participant(sample_prefix());
        let bytes = m.to_cdr(true).unwrap();
        assert_eq!(&bytes[..4], &[0x00, 0x01, 0x00, 0x00]);
        let decoded = ParticipantMessageData::from_cdr(&bytes).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn participant_message_data_roundtrip_be() {
        let m = ParticipantMessageData::automatic(sample_prefix());
        let bytes = m.to_cdr(false).unwrap();
        assert_eq!(&bytes[..4], &[0x00, 0x00, 0x00, 0x00]);
        let decoded = ParticipantMessageData::from_cdr(&bytes).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn participant_message_data_roundtrip_with_topic_token() {
        let m =
            ParticipantMessageData::manual_by_topic(sample_prefix(), vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert!(m.is_vendor_kind());
        let bytes = m.to_cdr(true).unwrap();
        let decoded = ParticipantMessageData::from_cdr(&bytes).unwrap();
        assert_eq!(decoded.data, vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(
            decoded.kind,
            PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC
        );
    }

    #[test]
    fn participant_message_data_accepts_xcdr2_le_encapsulation() {
        let m = ParticipantMessageData::automatic(sample_prefix());
        let mut bytes = m.to_cdr(true).unwrap();
        bytes[0] = 0x00;
        bytes[1] = 0x07; // XCDR2 LE
        let decoded = ParticipantMessageData::from_cdr(&bytes).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn participant_message_data_accepts_xcdr2_be_encapsulation() {
        let m = ParticipantMessageData::automatic(sample_prefix());
        let mut bytes = m.to_cdr(false).unwrap();
        bytes[0] = 0x00;
        bytes[1] = 0x06; // XCDR2 BE
        let decoded = ParticipantMessageData::from_cdr(&bytes).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn participant_message_data_rejects_unknown_encapsulation() {
        let mut bytes = vec![0x99, 0x99, 0, 0];
        bytes.extend_from_slice(&[0u8; 20]);
        let res = ParticipantMessageData::from_cdr(&bytes);
        assert!(matches!(
            res,
            Err(WireError::UnsupportedEncapsulation { kind: [0x99, 0x99] })
        ));
    }

    #[test]
    fn participant_message_data_rejects_overlong_data() {
        // Encapsulation + 12-octet prefix + kind + length = MAX + 1.
        let mut bytes = vec![0x00, 0x01, 0x00, 0x00];
        bytes.extend_from_slice(&[0u8; 12]);
        bytes.extend_from_slice(&[0, 0, 0, 1]);
        let too_big = (MAX_DATA_LEN as u32) + 1;
        bytes.extend_from_slice(&too_big.to_le_bytes());
        // data missing — that's fine, the cap check fires first.
        let res = ParticipantMessageData::from_cdr(&bytes);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn participant_message_data_encoder_caps_data_length() {
        let mut m = ParticipantMessageData::automatic(sample_prefix());
        m.data = vec![0u8; MAX_DATA_LEN + 1];
        let res = m.to_cdr(true);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn participant_message_data_too_short_encapsulation() {
        let bytes = [0x00];
        let res = ParticipantMessageData::from_cdr(&bytes);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn participant_message_data_too_short_body() {
        // Encapsulation valid, body only 8 octets.
        let bytes = vec![0x00, 0x01, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0, 0];
        let res = ParticipantMessageData::from_cdr(&bytes);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn participant_message_data_truncated_data_section() {
        // length=8 but only 4 octets follow.
        let mut bytes = vec![0x00, 0x01, 0x00, 0x00];
        bytes.extend_from_slice(&[0u8; 12]);
        bytes.extend_from_slice(&[0, 0, 0, 1]);
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(&[1, 2, 3, 4]);
        let res = ParticipantMessageData::from_cdr(&bytes);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    /// An earlier ZeroDDS wrote a 16-octet GUID and `kind` as an integer.
    /// Read by this layout, its beat is of an unknown kind — not a
    /// manual one — from the right participant.
    #[test]
    fn participant_message_data_reads_an_earlier_zerodds_beat_as_unknown_kind() {
        let mut bytes = vec![0x00, 0x01, 0x00, 0x00];
        bytes.extend_from_slice(&sample_prefix().to_bytes());
        bytes.extend_from_slice(&[0, 0, 1, 0xC1]);
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        let m = ParticipantMessageData::from_cdr(&bytes).unwrap();
        assert_eq!(m.prefix(), sample_prefix());
        assert_eq!(m.kind, 0x0000_01C1);
        assert!(!m.is_vendor_kind());
    }
}
