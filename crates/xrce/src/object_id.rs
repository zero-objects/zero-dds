// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `ObjectId` (Spec §7.2.1).
//!
//! Wire-Layout: `octet[2]`, also 16 Bit.
//!
//! ```text
//!  bit:  15                          4   3       0
//!       +---------------------------+----+--------+
//!       |        raw_id (12)        |    kind (4) |
//!       +---------------------------+-------------+
//! ```
//!
//! Die Spec definiert:
//! - Lower 4 Bits = `ObjectKind` (siehe `crate::object_kind`).
//! - Upper 12 Bits = anwendungs-/agent-vergebene `raw_id`.
//!
//! Zusaetzlich tracken wir eine **Kind-Mask** (Bit 15), die zwischen
//! "well-known builtin Object" (Bit 15 = 0) und "client-vergeben" (Bit
//! 15 = 1) unterscheidet — das ist die in C6.2.B-Aufgabe geforderte
//! 15-Bit-raw / 1-Bit-mask-Sicht. Auf Wire ist das einfach das
//! oberste Bit im 12-Bit-raw-Feld, semantisch aber wichtig fuer
//! Object-Lookup-Routing.
//!
//! Reservierte Werte:
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
/// `OBJECTID_AGENT` (Spec §7.5.2.1) — Singleton auf der Agent-Seite.
pub const OBJECTID_AGENT: ObjectId = ObjectId(0xFFFD);
/// `OBJECTID_CLIENT` (Spec §7.5.2.1) — Singleton auf der Client-Seite.
pub const OBJECTID_CLIENT: ObjectId = ObjectId(0xFFFE);

/// Bit-Position der Kind-Mask im 16-Bit-Wort. Setzt sich auf das
/// hoechste Bit des 12-Bit-`raw_id`-Felds.
pub const KIND_MASK_BIT: u16 = 15;

/// Maximale `raw_id` ohne Kind-Mask (12 Bit, also `0..=0xFFF`).
pub const RAW_ID_MAX: u16 = 0x0FFF;

impl ObjectId {
    /// Konstruiere aus rohem 16-Bit-Wort.
    #[must_use]
    pub const fn from_raw(value: u16) -> Self {
        Self(value)
    }

    /// Konstruiere aus 12-Bit-`raw_id` und 4-Bit-`kind`.
    ///
    /// # Errors
    /// `ValueOutOfRange`, wenn `raw_id > 0xFFF`.
    pub fn new(raw_id: u16, kind: ObjectKind) -> Result<Self, XrceError> {
        if raw_id > RAW_ID_MAX {
            return Err(XrceError::ValueOutOfRange {
                message: "ObjectId raw_id exceeds 12 bits",
            });
        }
        Ok(Self((raw_id << 4) | u16::from(kind.to_u8())))
    }

    /// Konstruiere mit explizit gesetzter Kind-Mask (Bit 15).
    ///
    /// `raw_id` darf nur 11 Bit nutzen (`0..=0x7FF`), weil Bit 11 (=Bit
    /// 15 im Wort) fuer die Kind-Mask reserviert ist.
    ///
    /// # Errors
    /// `ValueOutOfRange`, wenn `raw_id > 0x7FF`.
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

    /// Roher 16-Bit-Wert.
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// `true`, falls dies `OBJECTID_INVALID` ist.
    #[must_use]
    pub fn is_invalid(self) -> bool {
        self == OBJECTID_INVALID
    }

    /// 4-Bit-Kind aus den unteren 4 Bits.
    ///
    /// # Errors
    /// `ValueOutOfRange`, wenn der Kind-Code nicht in der Spec ist.
    pub fn kind(self) -> Result<ObjectKind, XrceError> {
        ObjectKind::from_u8((self.0 & 0x000F) as u8)
    }

    /// 12-Bit-`raw_id` aus den oberen 12 Bits.
    #[must_use]
    pub fn raw_id_12(self) -> u16 {
        (self.0 >> 4) & 0x0FFF
    }

    /// 11-Bit-`raw_id` (oberes Bit ist `kind_mask`).
    #[must_use]
    pub fn raw_id_11(self) -> u16 {
        (self.0 >> 4) & 0x07FF
    }

    /// `true`, wenn die Kind-Mask (Bit 15) gesetzt ist — das markiert das
    /// Object als "client-owned" (vom Client zugeteilte ID, im Gegensatz
    /// zu builtin/agent-zugewiesenen IDs).
    #[must_use]
    pub fn kind_mask(self) -> bool {
        (self.0 & (1u16 << KIND_MASK_BIT)) != 0
    }

    /// Wire-Encoding: `octet[2]`, Big-Endian (entspricht dem normalen
    /// XCDR2-Layout fuer `octet[N]` als opake Byte-Sequenz, kein
    /// Endianness-Swap; das obere Byte zuerst).
    #[must_use]
    pub fn to_bytes(self) -> [u8; 2] {
        self.0.to_be_bytes()
    }

    /// Wire-Decoding aus 2-Byte-Slice (Big-Endian).
    ///
    /// # Errors
    /// `UnexpectedEof`, wenn `bytes.len() < 2`.
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
        // 0x07 ist nicht in der Spec
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
        // raw_id_11 ist gleich, kind_mask unterscheidet
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
