// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! RTPS-Header (DDSI-RTPS 2.5 §8.3.3).
//!
//! The RTPS header forms the outer envelope of an RTPS datagram. It
//! is 20 bytes long (4 magic + 2 version + 2 vendor + 12 prefix), fixed
//! layout, and not endianness-tagged (all fields are byte
//! arrays).
//!
//! ```text
//!   0                   1                   2                   3
//!   0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
//!  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//!  |   'R' = 0x52  |   'T' = 0x54  |   'P' = 0x50  |   'S' = 0x53  |
//!  +---------------+---------------+---------------+---------------+
//!  | major version | minor version |   vendor.major| vendor.minor  |
//!  +---------------+---------------+---------------+---------------+
//!  |                                                               |
//!  +                                                               +
//!  |                          guidPrefix                           |
//!  +                                                               +
//!  |                                                               |
//!  +---------------+---------------+---------------+---------------+
//! ```

use crate::error::WireError;
use crate::wire_types::{GuidPrefix, ProtocolVersion, VendorId};

/// RTPS-Magic-Bytes "RTPS".
pub const RTPS_MAGIC: [u8; 4] = [b'R', b'T', b'P', b'S'];

/// RTPS-Header (20 Byte fix).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RtpsHeader {
    /// Protokoll-Version (default: 2.5).
    pub protocol_version: ProtocolVersion,
    /// Vendor-Identifier.
    pub vendor_id: VendorId,
    /// Participant-Prefix.
    pub guid_prefix: GuidPrefix,
}

impl RtpsHeader {
    /// Wire-Size: 20 Bytes (4 magic + 2 version + 2 vendor + 12 prefix).
    pub const WIRE_SIZE: usize = 20;

    /// Constructor with defaults for the version (2.5).
    #[must_use]
    pub fn new(vendor_id: VendorId, guid_prefix: GuidPrefix) -> Self {
        Self {
            protocol_version: ProtocolVersion::V2_5,
            vendor_id,
            guid_prefix,
        }
    }

    /// Encodes the header into a 20-byte array.
    #[must_use]
    pub fn to_bytes(self) -> [u8; 20] {
        let mut out = [0u8; 20];
        out[..4].copy_from_slice(&RTPS_MAGIC);
        out[4..6].copy_from_slice(&self.protocol_version.to_bytes());
        out[6..8].copy_from_slice(&self.vendor_id.to_bytes());
        out[8..20].copy_from_slice(&self.guid_prefix.to_bytes());
        out
    }

    /// Decodes a 20-byte slice. Checks the magic bytes.
    ///
    /// # Errors
    /// `InvalidMagic` on a wrong prefix; `UnexpectedEof` on too-short
    /// Input.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() < Self::WIRE_SIZE {
            return Err(WireError::UnexpectedEof {
                needed: Self::WIRE_SIZE,
                offset: 0,
            });
        }
        let mut magic = [0u8; 4];
        magic.copy_from_slice(&bytes[..4]);
        if magic != RTPS_MAGIC {
            return Err(WireError::InvalidMagic { found: magic });
        }
        let mut pv = [0u8; 2];
        pv.copy_from_slice(&bytes[4..6]);
        let mut vid = [0u8; 2];
        vid.copy_from_slice(&bytes[6..8]);
        let mut gp = [0u8; 12];
        gp.copy_from_slice(&bytes[8..20]);
        Ok(Self {
            protocol_version: ProtocolVersion::from_bytes(pv),
            vendor_id: VendorId::from_bytes(vid),
            guid_prefix: GuidPrefix::from_bytes(gp),
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn header_layout_first_four_bytes_are_rtps_magic() {
        let h = RtpsHeader::new(VendorId::ZERODDS, GuidPrefix::UNKNOWN);
        let bytes = h.to_bytes();
        assert_eq!(&bytes[..4], &RTPS_MAGIC);
        assert_eq!(&bytes[..4], b"RTPS");
    }

    #[test]
    fn header_protocol_version_at_bytes_4_5() {
        let h = RtpsHeader::new(VendorId::ZERODDS, GuidPrefix::UNKNOWN);
        let bytes = h.to_bytes();
        assert_eq!(&bytes[4..6], &[2, 5]); // 2.5
    }

    #[test]
    fn header_vendor_id_at_bytes_6_7() {
        let h = RtpsHeader::new(VendorId::ZERODDS, GuidPrefix::UNKNOWN);
        let bytes = h.to_bytes();
        assert_eq!(&bytes[6..8], &[0x01, 0xF0]);
    }

    #[test]
    fn header_guid_prefix_at_bytes_8_to_19() {
        let prefix = GuidPrefix::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        let h = RtpsHeader::new(VendorId::ZERODDS, prefix);
        let bytes = h.to_bytes();
        assert_eq!(&bytes[8..20], &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    }

    #[test]
    fn header_total_size_is_20_bytes() {
        let h = RtpsHeader::new(VendorId::ZERODDS, GuidPrefix::UNKNOWN);
        assert_eq!(h.to_bytes().len(), 20);
        assert_eq!(RtpsHeader::WIRE_SIZE, 20);
    }

    #[test]
    fn header_roundtrip() {
        let h = RtpsHeader::new(VendorId([0xAB, 0xCD]), GuidPrefix::from_bytes([42; 12]));
        let bytes = h.to_bytes();
        let decoded = RtpsHeader::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, h);
    }

    #[test]
    fn header_decode_rejects_invalid_magic() {
        let mut bytes = [0u8; 20];
        bytes[..4].copy_from_slice(b"XXXX");
        let res = RtpsHeader::from_bytes(&bytes);
        assert!(matches!(
            res,
            Err(WireError::InvalidMagic { found }) if &found == b"XXXX"
        ));
    }

    #[test]
    fn header_decode_rejects_truncated_input() {
        let bytes = [b'R', b'T', b'P', b'S', 2, 5, 0, 0]; // nur 8 Byte
        let res = RtpsHeader::from_bytes(&bytes);
        assert!(matches!(
            res,
            Err(WireError::UnexpectedEof { needed: 20, .. })
        ));
    }

    #[test]
    fn header_decode_accepts_extra_trailing_bytes() {
        // The decoder consumes only 20 bytes; trailing bytes (e.g. the first
        // submessage) survive the input.
        let h = RtpsHeader::new(VendorId::ZERODDS, GuidPrefix::UNKNOWN);
        let mut bytes = [0u8; 36];
        bytes[..20].copy_from_slice(&h.to_bytes());
        bytes[20..].copy_from_slice(&[0xAB; 16]);
        let decoded = RtpsHeader::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, h);
    }
}
