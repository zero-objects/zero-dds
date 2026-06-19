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
//!     GUID_t   participantGuidPrefix; // 12 byte (GuidPrefix only!)
//!     octet    kind[4];               // 4 byte u32 (BE/LE per CDR)
//!     sequence<octet> data;           // 4 byte length + N byte
//! };
//! ```
//!
//! Spec pitfall: despite the field name `participantGuidPrefix`, in
//! practice Cyclone DDS and Fast-DDS send a full 16-byte GUID (prefix +
//! ENTITYID_PARTICIPANT). We therefore write 16 bytes and parse
//! tolerantly: 16 bytes → full GUID, 12 bytes → prefix-only.
//!
//! The `data` sequence is semantically a `vec<octet>` with a leading
//! 32-bit length. Spec §9.6.3.1 defines it as an opaque token; ZeroDDS
//! uses it for MANUAL_BY_TOPIC to transport the topic token (topic hash
//! fingerprint).
//!
//! # CDR encoding
//!
//! Encoded as XCDR1 plain (encapsulation 0x0000 BE / 0x0001 LE) or
//! XCDR2 plain (0x0006 BE / 0x0007 LE). Cyclone sends the topic by
//! default as XCDR1 plain. We accept all four encapsulation kinds and
//! write LE by default.
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

/// Spec-defined `kind` code: AUTOMATIC_LIVELINESS_UPDATE (§9.6.3.1).
/// Sent by the builtin WLP writer when LIVELINESS=AUTOMATIC.
pub const PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE: u32 = 0x0000_0000;

/// Spec-defined `kind` code: MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE
/// (§9.6.3.1). Triggered by `assert_liveliness()` on the
/// `DomainParticipant`.
pub const PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE: u32 = 0x0000_0001;

/// Vendor-specific kind range: top bit set
/// (`0x80000000..=0xFFFFFFFF`). Spec §9.6.3.1 reserves this for
/// vendor-own kinds (e.g. ZeroDDS MANUAL_BY_TOPIC with a topic token
/// in the `data` sequence).
pub const PARTICIPANT_MESSAGE_DATA_KIND_VENDOR_BASE: u32 = 0x8000_0000;

/// ZeroDDS vendor kind: MANUAL_BY_TOPIC. Triggered by
/// `DataWriter::assert_liveliness()`; the `data` sequence carries a
/// topic token (typically a 4-byte hash). Cyclone peers ignore the
/// vendor kind (spec §9.6.3.1: "If kind has its MSB set,
/// implementations not understanding the kind shall ignore the
/// message").
pub const PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC: u32 = 0x8000_0001;

/// `ParticipantMessageData` (DDSI-RTPS 2.5 §9.6.3.1) — payload of the
/// `DCPSParticipantMessage` topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantMessageData {
    /// 16-byte GUID of the sender (prefix + EntityId::PARTICIPANT).
    /// Even though the spec says "prefix", Cyclone and Fast-DDS write
    /// the full GUID here; we follow.
    pub participant_guid: [u8; 16],
    /// Liveliness kind (see `PARTICIPANT_MESSAGE_DATA_KIND_*`).
    pub kind: u32,
    /// Opaque token. On MANUAL_BY_TOPIC a topic hash, otherwise empty.
    pub data: Vec<u8>,
}

impl ParticipantMessageData {
    /// Constructor for an AUTOMATIC heartbeat (data empty).
    #[must_use]
    pub fn automatic(prefix: GuidPrefix) -> Self {
        Self {
            participant_guid: full_guid_bytes(prefix),
            kind: PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE,
            data: Vec::new(),
        }
    }

    /// Constructor for a MANUAL_BY_PARTICIPANT heartbeat (data empty).
    #[must_use]
    pub fn manual_by_participant(prefix: GuidPrefix) -> Self {
        Self {
            participant_guid: full_guid_bytes(prefix),
            kind: PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE,
            data: Vec::new(),
        }
    }

    /// Constructor for ZeroDDS MANUAL_BY_TOPIC (data = topic token).
    #[must_use]
    pub fn manual_by_topic(prefix: GuidPrefix, topic_token: Vec<u8>) -> Self {
        Self {
            participant_guid: full_guid_bytes(prefix),
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
        let mut out = Vec::with_capacity(4 + 16 + 4 + 4 + self.data.len());
        // Encapsulation-Header
        if little_endian {
            out.extend_from_slice(&ENCAPSULATION_CDR_LE);
        } else {
            out.extend_from_slice(&ENCAPSULATION_CDR_BE);
        }
        out.extend_from_slice(&[0, 0]); // options
        // Body Start (CDR-Offset 0)
        // GUID — 16 bytes raw, no endian swap (bytes are opaque).
        out.extend_from_slice(&self.participant_guid);
        // kind — 4 byte u32 BE/LE
        let kind_bytes = if little_endian {
            self.kind.to_le_bytes()
        } else {
            self.kind.to_be_bytes()
        };
        out.extend_from_slice(&kind_bytes);
        // data: u32 length + N byte
        let len_bytes = if little_endian {
            data_len_u32.to_le_bytes()
        } else {
            data_len_u32.to_be_bytes()
        };
        out.extend_from_slice(&len_bytes);
        out.extend_from_slice(&self.data);
        Ok(out)
    }

    /// Decodes from CDR bytes (with an encapsulation header).
    ///
    /// Accepts `0x0000`/`0x0001` (XCDR1 plain) and `0x0006`/`0x0007`
    /// (XCDR2 plain). Other encapsulation kinds → error.
    ///
    /// Tolerant of the 12-byte prefix-only encoding (pads with 0 to 16
    /// bytes).
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
        // We accept a 16-byte GUID (spec de-facto) OR 12-byte
        // prefix-only (an alternative reading of the spec wording
        // "GUID_t guidPrefix" — strict 12-byte encoders exist).
        // Heuristic: 16 bytes is the default, 12 bytes is only valid if
        // the following fields (kind + data-len) parse correctly with 12
        // bytes.
        let (guid_bytes, after_guid_offset) = parse_guid(body)?;
        if body.len() < after_guid_offset + 4 {
            return Err(WireError::UnexpectedEof {
                needed: after_guid_offset + 4,
                offset: 4,
            });
        }
        let kind_slice = &body[after_guid_offset..after_guid_offset + 4];
        let mut kind_arr = [0u8; 4];
        kind_arr.copy_from_slice(kind_slice);
        let kind = if little_endian {
            u32::from_le_bytes(kind_arr)
        } else {
            u32::from_be_bytes(kind_arr)
        };
        let len_offset = after_guid_offset + 4;
        if body.len() < len_offset + 4 {
            return Err(WireError::UnexpectedEof {
                needed: len_offset + 4,
                offset: 4,
            });
        }
        let mut len_arr = [0u8; 4];
        len_arr.copy_from_slice(&body[len_offset..len_offset + 4]);
        let data_len = if little_endian {
            u32::from_le_bytes(len_arr)
        } else {
            u32::from_be_bytes(len_arr)
        } as usize;
        if data_len > MAX_DATA_LEN {
            return Err(WireError::ValueOutOfRange {
                message: "ParticipantMessageData.data exceeds MAX_DATA_LEN",
            });
        }
        let data_offset = len_offset + 4;
        if body.len() < data_offset + data_len {
            return Err(WireError::UnexpectedEof {
                needed: data_offset + data_len,
                offset: 4,
            });
        }
        let data = body[data_offset..data_offset + data_len].to_vec();
        Ok(Self {
            participant_guid: guid_bytes,
            kind,
            data,
        })
    }

    /// Returns the GuidPrefix (first 12 bytes).
    #[must_use]
    pub fn prefix(&self) -> GuidPrefix {
        let mut p = [0u8; 12];
        p.copy_from_slice(&self.participant_guid[..12]);
        GuidPrefix::from_bytes(p)
    }

    /// `true` if the `kind` value is vendor-specific (MSB set).
    #[must_use]
    pub fn is_vendor_kind(&self) -> bool {
        self.kind >= PARTICIPANT_MESSAGE_DATA_KIND_VENDOR_BASE
    }
}

fn full_guid_bytes(prefix: GuidPrefix) -> [u8; 16] {
    let mut g = [0u8; 16];
    g[..12].copy_from_slice(&prefix.to_bytes());
    // EntityId::PARTICIPANT = [0, 0, 1, 0xC1]
    g[12] = 0;
    g[13] = 0;
    g[14] = 1;
    g[15] = 0xC1;
    g
}

/// Parses the GUID body. Tries 16 bytes first (Cyclone/Fast-DDS
/// default), falls back to 12 bytes (strict spec reading).
fn parse_guid(body: &[u8]) -> Result<([u8; 16], usize), WireError> {
    // 16-byte variant: needs at least 16 + 4 (kind) + 4 (data-len).
    if body.len() >= 24 {
        let mut g = [0u8; 16];
        g.copy_from_slice(&body[..16]);
        return Ok((g, 16));
    }
    // 12-byte variant: needs at least 12 + 4 + 4.
    if body.len() >= 20 {
        let mut g = [0u8; 16];
        g[..12].copy_from_slice(&body[..12]);
        // EntityId-Default: PARTICIPANT
        g[14] = 1;
        g[15] = 0xC1;
        return Ok((g, 12));
    }
    Err(WireError::UnexpectedEof {
        needed: 24,
        offset: 4,
    })
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

    #[test]
    fn participant_message_data_kind_constants_match_spec() {
        // §9.6.3.1: AUTOMATIC = 0x00000000, MANUAL_BY_PARTICIPANT = 0x00000001.
        // Vendor range: MSB set, i.e. >= 0x80000000.
        assert_eq!(
            PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE,
            0x0000_0000
        );
        assert_eq!(
            PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE,
            0x0000_0001
        );
        assert_eq!(PARTICIPANT_MESSAGE_DATA_KIND_VENDOR_BASE, 0x8000_0000);
        // The ZeroDDS vendor kind must be in the vendor range (MSB set).
        // `assert_eq!` instead of `assert!` because clippy
        // `assertions_on_constants` would otherwise complain; here we
        // compare against a constant expected value.
        assert_eq!(
            PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC
                & PARTICIPANT_MESSAGE_DATA_KIND_VENDOR_BASE,
            PARTICIPANT_MESSAGE_DATA_KIND_VENDOR_BASE
        );
    }

    #[test]
    fn participant_message_data_roundtrip_le() {
        let m = ParticipantMessageData::manual_by_participant(sample_prefix());
        let bytes = m.to_cdr(true).unwrap();
        // Encapsulation-Header LE
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
        // The ZeroDDS default for user topics is XCDR2-LE (0x0007). If a
        // peer sends the WLP topic with XCDR2, we must be able to decode
        // it (Cyclone does this with the spec-2.5 default rep).
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
        bytes.extend_from_slice(&[0u8; 24]);
        let res = ParticipantMessageData::from_cdr(&bytes);
        assert!(matches!(
            res,
            Err(WireError::UnsupportedEncapsulation { kind: [0x99, 0x99] })
        ));
    }

    #[test]
    fn participant_message_data_rejects_overlong_data() {
        // Build manually: encapsulation + 16 byte guid + 4 byte kind +
        // 4 byte length=MAX+1.
        let mut bytes = vec![0x00, 0x01, 0x00, 0x00];
        bytes.extend_from_slice(&[0u8; 16]);
        bytes.extend_from_slice(&0u32.to_le_bytes()); // kind
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
        // Encapsulation valid, body only 8 byte (less than 12-byte prefix variant).
        let bytes = vec![0x00, 0x01, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0, 0];
        let res = ParticipantMessageData::from_cdr(&bytes);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn participant_message_data_truncated_data_section() {
        // length=8 but only 4 byte follow.
        let mut bytes = vec![0x00, 0x01, 0x00, 0x00];
        bytes.extend_from_slice(&[0u8; 16]);
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(&[1, 2, 3, 4]);
        let res = ParticipantMessageData::from_cdr(&bytes);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn participant_message_data_le_be_bytes_differ_for_kind() {
        // Sanity: BE and LE encoding differ via kind.
        let mut m = ParticipantMessageData::automatic(sample_prefix());
        m.kind = 0x0102_0304;
        let le = m.to_cdr(true).unwrap();
        let be = m.to_cdr(false).unwrap();
        assert_ne!(le, be);
        // Both must yield the same value again.
        assert_eq!(ParticipantMessageData::from_cdr(&le).unwrap(), m);
        assert_eq!(ParticipantMessageData::from_cdr(&be).unwrap(), m);
    }

    #[test]
    fn participant_message_data_accepts_12_byte_prefix_only_encoding() {
        // A legacy/strict encoder writes only 12 bytes (prefix). We
        // must be able to decode it + fill in the EntityId default
        // (PARTICIPANT = 0xC1).
        let mut bytes = vec![0x00, 0x01, 0x00, 0x00];
        let prefix = sample_prefix().to_bytes();
        bytes.extend_from_slice(&prefix); // 12 byte
        bytes.extend_from_slice(&0u32.to_le_bytes()); // kind
        bytes.extend_from_slice(&0u32.to_le_bytes()); // data len = 0
        let decoded = ParticipantMessageData::from_cdr(&bytes).unwrap();
        assert_eq!(decoded.prefix(), sample_prefix());
        // EntityId-Default-Auffuellung
        assert_eq!(&decoded.participant_guid[12..], &[0, 0, 1, 0xC1]);
    }
}
