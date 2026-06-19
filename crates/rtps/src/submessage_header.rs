// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Submessage-Header (DDSI-RTPS 2.5 §8.3.4).
//!
//! Every submessage in an RTPS datagram begins with a 4-byte
//! header:
//!
//! ```text
//!   0                   1                   2                   3
//!   0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
//!  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//!  | submessageId  |     flags     |       octetsToNextHeader      |
//!  +---------------+---------------+---------------+---------------+
//! ```
//!
//! `flags` carries at least the **E flag** (bit 0) for the endianness
//! of the submessage body (1 = LE, 0 = BE). `octetsToNextHeader` is
//! the body length in bytes; `0` marks a last-submessage special
//! behavior (see Spec §8.3.4.2).

use crate::error::WireError;

/// Submessage IDs supported in phase 0. Values from
/// Spec-Tabelle 8.13.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum SubmessageId {
    Pad = 0x01,
    AckNack = 0x06,
    Heartbeat = 0x07,
    Gap = 0x08,
    InfoTs = 0x09,
    InfoSrc = 0x0A,
    InfoReplyIp4 = 0x0C,
    InfoDst = 0x0E,
    InfoReply = 0x0F,
    NackFrag = 0x12,
    HeartbeatFrag = 0x13,
    Data = 0x15,
    DataFrag = 0x16,
}

impl SubmessageId {
    /// Raw wire value.
    #[must_use]
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Converts a byte. Unknown IDs are allowed — the spec
    /// requires readers to **skip** unknown submessages (via
    /// `octetsToNextHeader`). For that we use the UnknownSubmessageId
    /// variant of the error only on explicit validation.
    ///
    /// # Errors
    /// `UnknownSubmessageId`.
    pub fn from_u8(byte: u8) -> Result<Self, WireError> {
        match byte {
            0x01 => Ok(Self::Pad),
            0x06 => Ok(Self::AckNack),
            0x07 => Ok(Self::Heartbeat),
            0x08 => Ok(Self::Gap),
            0x09 => Ok(Self::InfoTs),
            0x0A => Ok(Self::InfoSrc),
            0x0C => Ok(Self::InfoReplyIp4),
            0x0E => Ok(Self::InfoDst),
            0x0F => Ok(Self::InfoReply),
            0x12 => Ok(Self::NackFrag),
            0x13 => Ok(Self::HeartbeatFrag),
            0x15 => Ok(Self::Data),
            0x16 => Ok(Self::DataFrag),
            other => Err(WireError::UnknownSubmessageId { id: other }),
        }
    }
}

/// E-Flag-Bit-Position im Flag-Byte.
pub const FLAG_E_LITTLE_ENDIAN: u8 = 0x01;

/// Submessage-Header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubmessageHeader {
    /// ID of the submessage class.
    pub submessage_id: SubmessageId,
    /// Flag byte (bit 0 = E = little-endian body; further bits
    /// submessage-specific).
    pub flags: u8,
    /// Body length in bytes. `0` has special meaning (see
    /// Spec §8.3.4.2): the reader reads "to the end of the datagram".
    pub octets_to_next_header: u16,
}

impl SubmessageHeader {
    /// Wire-Size: 4 Bytes.
    pub const WIRE_SIZE: usize = 4;

    /// `true` if the E flag is set (LE body).
    #[must_use]
    pub fn is_little_endian(self) -> bool {
        (self.flags & FLAG_E_LITTLE_ENDIAN) != 0
    }

    /// LE encoder. `octets_to_next_header` is written with the endianness
    /// given by `is_little_endian()` — the
    /// sub-header itself uses the same endianness as its body
    /// (Spec §8.3.4.1).
    #[must_use]
    pub fn to_bytes(self) -> [u8; 4] {
        let mut out = [0u8; 4];
        out[0] = self.submessage_id.as_u8();
        out[1] = self.flags;
        let len_bytes = if self.is_little_endian() {
            self.octets_to_next_header.to_le_bytes()
        } else {
            self.octets_to_next_header.to_be_bytes()
        };
        out[2..].copy_from_slice(&len_bytes);
        out
    }

    /// Decodes a 4-byte slice.
    ///
    /// # Errors
    /// `UnexpectedEof`, `UnknownSubmessageId`.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() < Self::WIRE_SIZE {
            return Err(WireError::UnexpectedEof {
                needed: Self::WIRE_SIZE,
                offset: 0,
            });
        }
        let id = SubmessageId::from_u8(bytes[0])?;
        let flags = bytes[1];
        let mut len_bytes = [0u8; 2];
        len_bytes.copy_from_slice(&bytes[2..4]);
        let octets_to_next_header = if (flags & FLAG_E_LITTLE_ENDIAN) != 0 {
            u16::from_le_bytes(len_bytes)
        } else {
            u16::from_be_bytes(len_bytes)
        };
        Ok(Self {
            submessage_id: id,
            flags,
            octets_to_next_header,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn submessage_id_data_is_0x15() {
        assert_eq!(SubmessageId::Data.as_u8(), 0x15);
    }

    #[test]
    fn submessage_id_heartbeat_is_0x07() {
        assert_eq!(SubmessageId::Heartbeat.as_u8(), 0x07);
    }

    #[test]
    fn submessage_id_acknack_is_0x06() {
        assert_eq!(SubmessageId::AckNack.as_u8(), 0x06);
    }

    #[test]
    fn submessage_id_gap_is_0x08() {
        assert_eq!(SubmessageId::Gap.as_u8(), 0x08);
    }

    #[test]
    fn submessage_id_roundtrip_for_known_ids() {
        for id in [
            SubmessageId::Pad,
            SubmessageId::AckNack,
            SubmessageId::Heartbeat,
            SubmessageId::Gap,
            SubmessageId::InfoTs,
            SubmessageId::InfoSrc,
            SubmessageId::InfoReplyIp4,
            SubmessageId::InfoDst,
            SubmessageId::InfoReply,
            SubmessageId::NackFrag,
            SubmessageId::HeartbeatFrag,
            SubmessageId::Data,
            SubmessageId::DataFrag,
        ] {
            assert_eq!(SubmessageId::from_u8(id.as_u8()).unwrap(), id);
        }
    }

    #[test]
    fn submessage_id_rejects_unknown_byte() {
        let res = SubmessageId::from_u8(0xFE);
        assert!(matches!(
            res,
            Err(WireError::UnknownSubmessageId { id: 0xFE })
        ));
    }

    #[test]
    fn submessage_header_layout_le() {
        let h = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags: FLAG_E_LITTLE_ENDIAN,
            octets_to_next_header: 0x0102,
        };
        let bytes = h.to_bytes();
        assert_eq!(bytes[0], 0x15); // Data
        assert_eq!(bytes[1], 0x01); // E-flag
        // octets_to_next_header LE: 0x0102 → [0x02, 0x01]
        assert_eq!(bytes[2], 0x02);
        assert_eq!(bytes[3], 0x01);
    }

    #[test]
    fn submessage_header_layout_be() {
        let h = SubmessageHeader {
            submessage_id: SubmessageId::Heartbeat,
            flags: 0, // E-flag not set → BE body
            octets_to_next_header: 0x0102,
        };
        let bytes = h.to_bytes();
        assert_eq!(bytes[0], 0x07);
        assert_eq!(bytes[1], 0);
        // BE: 0x0102 → [0x01, 0x02]
        assert_eq!(bytes[2], 0x01);
        assert_eq!(bytes[3], 0x02);
    }

    #[test]
    fn submessage_header_is_little_endian_flag() {
        let le = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags: FLAG_E_LITTLE_ENDIAN,
            octets_to_next_header: 0,
        };
        assert!(le.is_little_endian());
        let be = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags: 0,
            octets_to_next_header: 0,
        };
        assert!(!be.is_little_endian());
    }

    #[test]
    fn submessage_header_roundtrip_le() {
        let h = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags: FLAG_E_LITTLE_ENDIAN,
            octets_to_next_header: 0xABCD,
        };
        let bytes = h.to_bytes();
        assert_eq!(SubmessageHeader::from_bytes(&bytes).unwrap(), h);
    }

    #[test]
    fn submessage_header_roundtrip_be() {
        let h = SubmessageHeader {
            submessage_id: SubmessageId::Heartbeat,
            flags: 0,
            octets_to_next_header: 0x1234,
        };
        let bytes = h.to_bytes();
        assert_eq!(SubmessageHeader::from_bytes(&bytes).unwrap(), h);
    }

    #[test]
    fn submessage_header_decode_rejects_truncated() {
        let bytes = [0x15u8, 0x01]; // nur 2 Byte
        let res = SubmessageHeader::from_bytes(&bytes);
        assert!(matches!(
            res,
            Err(WireError::UnexpectedEof { needed: 4, .. })
        ));
    }

    #[test]
    fn submessage_header_decode_rejects_unknown_id() {
        let bytes = [0xFEu8, 0x01, 0, 0];
        let res = SubmessageHeader::from_bytes(&bytes);
        assert!(matches!(
            res,
            Err(WireError::UnknownSubmessageId { id: 0xFE })
        ));
    }
}
