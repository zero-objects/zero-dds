// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `ObjectId` (Spec §7.2.1).
//!
//! Wire layout: `octet[2]`, i.e. 16 bits.
//!
//! ```text
//!  bit:  15                          4   3       0
//!       +---------------------------+----+--------+
//!       |        raw_id (12)        |    kind (4) |
//!       +---------------------------+-------------+
//! ```
//!
//! The spec defines:
//! - Lower 4 bits = `ObjectKind` (see `crate::object_kind`).
//! - Upper 12 bits = application-/agent-assigned `raw_id`.
//!
//! Additionally we track a **kind mask** (bit 15) that distinguishes
//! between a "well-known builtin object" (bit 15 = 0) and "client-assigned"
//! (bit 15 = 1) — this is the 15-bit-raw / 1-bit-mask view required
//! in the C6.2.B task. On the wire this is simply the
//! top bit in the 12-bit raw field, but semantically important for
//! object lookup routing.
//!
//! Reserved values:
//! - `OBJECTID_INVALID = 0xFFFF` (Spec §7.2.1).
//! - `OBJECTID_AGENT   = 0xFFFD` (kind=0xD `OBJK_AGENT`, raw=0xFFF).
//! - `OBJECTID_CLIENT  = 0xFFFE` (kind=0xE `OBJK_CLIENT`, raw=0xFFF).

use crate::error::XrceError;
use crate::object_kind::ObjectKind;

/// `ObjectId` — 16 Bit, kind in lower 4, raw in upper 12.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ObjectId(pub u16);

/// `OBJECTID_INVALID` (Spec §7.2.1).
pub const OBJECTID_INVALID: ObjectId = ObjectId(0xFFFF);
/// `OBJECTID_AGENT` (Spec §7.5.2.1) — singleton on the agent side.
pub const OBJECTID_AGENT: ObjectId = ObjectId(0xFFFD);
/// `OBJECTID_CLIENT` (Spec §7.5.2.1) — singleton on the client side.
pub const OBJECTID_CLIENT: ObjectId = ObjectId(0xFFFE);

/// Bit position of the kind mask in the 16-bit word. Maps to the
/// highest bit of the 12-bit `raw_id` field.
pub const KIND_MASK_BIT: u16 = 15;

/// Maximum `raw_id` without the kind mask (12 bits, i.e. `0..=0xFFF`).
pub const RAW_ID_MAX: u16 = 0x0FFF;

impl ObjectId {
    /// Constructs from a raw 16-bit word.
    #[must_use]
    pub const fn from_raw(value: u16) -> Self {
        Self(value)
    }

    /// Constructs from a 12-bit `raw_id` and a 4-bit `kind`.
    ///
    /// # Errors
    /// `ValueOutOfRange` if `raw_id > 0xFFF`.
    pub fn new(raw_id: u16, kind: ObjectKind) -> Result<Self, XrceError> {
        if raw_id > RAW_ID_MAX {
            return Err(XrceError::ValueOutOfRange {
                message: "ObjectId raw_id exceeds 12 bits",
            });
        }
        Ok(Self((raw_id << 4) | u16::from(kind.to_u8())))
    }

    /// Constructs with the kind mask (bit 15) explicitly set.
    ///
    /// `raw_id` may use only 11 bits (`0..=0x7FF`), because bit 11 (=bit
    /// 15 in the word) is reserved for the kind mask.
    ///
    /// # Errors
    /// `ValueOutOfRange` if `raw_id > 0x7FF`.
    pub fn new_with_mask(
        raw_id: u16,
        kind: ObjectKind,
        client_owned: bool,
    ) -> Result<Self, XrceError> {
        if raw_id > 0x07FF {
            return Err(XrceError::ValueOutOfRange {
                message: "ObjectId raw_id with kind_mask exceeds 11 bits",
            });
        }
        let mut word = (raw_id << 4) | u16::from(kind.to_u8());
        if client_owned {
            word |= 1u16 << KIND_MASK_BIT;
        }
        Ok(Self(word))
    }

    /// Raw 16-bit value.
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// `true` if this is `OBJECTID_INVALID`.
    #[must_use]
    pub fn is_invalid(self) -> bool {
        self == OBJECTID_INVALID
    }

    /// 4-bit kind from the lower 4 bits.
    ///
    /// # Errors
    /// `ValueOutOfRange` if the kind code is not in the spec.
    pub fn kind(self) -> Result<ObjectKind, XrceError> {
        ObjectKind::from_u8((self.0 & 0x000F) as u8)
    }

    /// 12-bit `raw_id` from the upper 12 bits.
    #[must_use]
    pub fn raw_id_12(self) -> u16 {
        (self.0 >> 4) & 0x0FFF
    }

    /// 11-bit `raw_id` (the top bit is `kind_mask`).
    #[must_use]
    pub fn raw_id_11(self) -> u16 {
        (self.0 >> 4) & 0x07FF
    }

    /// `true` when the kind mask (bit 15) is set — this marks the
    /// object as "client-owned" (an ID assigned by the client, as opposed
    /// to builtin/agent-assigned IDs).
    #[must_use]
    pub fn kind_mask(self) -> bool {
        (self.0 & (1u16 << KIND_MASK_BIT)) != 0
    }

    /// Wire encoding: `octet[2]`, big-endian (corresponds to the normal
    /// XCDR2 layout for `octet[N]` as an opaque byte sequence, no
    /// endianness swap; the upper byte first).
    #[must_use]
    pub fn to_bytes(self) -> [u8; 2] {
        self.0.to_be_bytes()
    }

    /// Wire decoding from a 2-byte slice (big-endian).
    ///
    /// # Errors
    /// `UnexpectedEof` if `bytes.len() < 2`.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, XrceError> {
        if bytes.len() < 2 {
            return Err(XrceError::UnexpectedEof {
                needed: 2,
                offset: bytes.len(),
            });
        }
        let mut buf = [0u8; 2];
        buf.copy_from_slice(&bytes[..2]);
        Ok(Self(u16::from_be_bytes(buf)))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn new_packs_kind_into_lower_4_bits() {
        let id = ObjectId::new(0x123, ObjectKind::DataWriter).unwrap();
        // raw_id = 0x123 → upper 12 bits; kind = 0x05
        assert_eq!(id.raw(), (0x123 << 4) | 0x05);
        assert_eq!(id.kind().unwrap(), ObjectKind::DataWriter);
        assert_eq!(id.raw_id_12(), 0x123);
    }

    #[test]
    fn new_rejects_raw_id_overflow() {
        let res = ObjectId::new(0x1000, ObjectKind::Topic);
        assert!(res.is_err());
    }

    #[test]
    fn agent_singleton_has_kind_agent() {
        assert_eq!(OBJECTID_AGENT.kind().unwrap(), ObjectKind::Agent);
    }

    #[test]
    fn client_singleton_has_kind_client() {
        assert_eq!(OBJECTID_CLIENT.kind().unwrap(), ObjectKind::Client);
    }

    #[test]
    fn invalid_object_id_is_all_ones() {
        assert!(OBJECTID_INVALID.is_invalid());
        assert_eq!(OBJECTID_INVALID.raw(), 0xFFFF);
    }

    #[test]
    fn invalid_kind_lookup_fails() {
        // 0x07 is not in the spec
        let id = ObjectId::from_raw(0xABC7);
        assert!(id.kind().is_err());
    }

    #[test]
    fn bytes_are_big_endian() {
        let id = ObjectId::from_raw(0x1234);
        let b = id.to_bytes();
        assert_eq!(b, [0x12, 0x34]);
        let id2 = ObjectId::from_bytes(&b).unwrap();
        assert_eq!(id2, id);
    }

    #[test]
    fn from_bytes_truncated_returns_eof() {
        let res = ObjectId::from_bytes(&[0xAB]);
        assert!(matches!(
            res,
            Err(XrceError::UnexpectedEof { needed: 2, .. })
        ));
    }

    #[test]
    fn kind_mask_top_bit_distinguishes_client_vs_builtin() {
        let builtin = ObjectId::new_with_mask(0x100, ObjectKind::DataWriter, false).unwrap();
        let client = ObjectId::new_with_mask(0x100, ObjectKind::DataWriter, true).unwrap();
        assert!(!builtin.kind_mask());
        assert!(client.kind_mask());
        // raw_id_11 is equal, kind_mask differs
        assert_eq!(builtin.raw_id_11(), 0x100);
        assert_eq!(client.raw_id_11(), 0x100);
    }

    #[test]
    fn kind_mask_overflow_rejected() {
        let res = ObjectId::new_with_mask(0x800, ObjectKind::Topic, false);
        assert!(res.is_err());
    }

    #[test]
    fn ordering_is_lexicographic_on_raw() {
        let a = ObjectId::from_raw(0x0010);
        let b = ObjectId::from_raw(0x0011);
        let c = ObjectId::from_raw(0x1000);
        assert!(a < b);
        assert!(b < c);
    }

    #[test]
    fn default_is_zero() {
        assert_eq!(ObjectId::default().raw(), 0);
    }
}
