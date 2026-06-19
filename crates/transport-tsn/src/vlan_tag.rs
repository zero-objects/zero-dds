// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IEEE 802.1Q VLAN Tag — Spec §7.2.3 Tab 7.21.

/// Spec — IEEE 802.1Q TPID = `0x8100`.
pub const TPID_8021Q: u16 = 0x8100;
/// Spec — IEEE 802.1ad (QinQ) TPID = `0x88a8`.
pub const TPID_8021AD: u16 = 0x88A8;

/// Spec §7.2.3 Tab 7.21 — IEEE 802.1Q VLAN tag (4-byte wire form).
///
/// Wire layout:
/// ```text
///   0                   1                   2                   3
///   0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
///  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
///  |              TPID             | PCP |D|         VID           |
///  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// ```
/// * TPID — `0x8100` (802.1Q) or `0x88a8` (802.1ad / QinQ).
/// * PCP (Priority Code Point) — 3 Bits (0..=7).
/// * DEI (Drop Eligible Indicator) — 1 Bit.
/// * VID (VLAN Identifier) — 12 Bits (1..=4094 valid; 0 + 4095
///   reserved).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ieee802VlanTag {
    /// TPID (Tag Protocol Identifier).
    pub tpid: u16,
    /// PCP (Priority Code Point) — 0..=7.
    pub pcp: u8,
    /// DEI (Drop Eligible Indicator).
    pub dei: bool,
    /// VID (VLAN Identifier) — 0..=4095 (1..=4094 valid).
    pub vid: u16,
}

impl Ieee802VlanTag {
    /// Constructor with validation.
    ///
    /// # Errors
    /// `Err(VlanError::PcpOutOfRange)` if `pcp > 7`.
    /// `Err(VlanError::VidOutOfRange)` if `vid > 4095`.
    pub fn new(tpid: u16, pcp: u8, dei: bool, vid: u16) -> Result<Self, VlanError> {
        if pcp > 7 {
            return Err(VlanError::PcpOutOfRange(pcp));
        }
        if vid > 4095 {
            return Err(VlanError::VidOutOfRange(vid));
        }
        Ok(Self {
            tpid,
            pcp,
            dei,
            vid,
        })
    }

    /// Encodes to 4-byte wire form (BE).
    #[must_use]
    pub fn to_wire(self) -> [u8; 4] {
        let mut out = [0u8; 4];
        out[0..2].copy_from_slice(&self.tpid.to_be_bytes());
        let tci_high =
            ((self.pcp & 0x07) << 5) | (u8::from(self.dei) << 4) | ((self.vid >> 8) as u8 & 0x0F);
        let tci_low = (self.vid & 0xFF) as u8;
        out[2] = tci_high;
        out[3] = tci_low;
        out
    }

    /// Decodes from 4-byte wire form.
    ///
    /// # Errors
    /// `Truncated` if `bytes.len() < 4`.
    pub fn from_wire(bytes: &[u8]) -> Result<Self, VlanError> {
        if bytes.len() < 4 {
            return Err(VlanError::Truncated);
        }
        let tpid = u16::from_be_bytes([bytes[0], bytes[1]]);
        let tci_high = bytes[2];
        let tci_low = bytes[3];
        let pcp = (tci_high >> 5) & 0x07;
        let dei = (tci_high & 0x10) != 0;
        let vid = (u16::from(tci_high & 0x0F) << 8) | u16::from(tci_low);
        Ok(Self {
            tpid,
            pcp,
            dei,
            vid,
        })
    }

    /// Spec — VID 0 or 4095 are reserved.
    #[must_use]
    pub const fn is_reserved_vid(self) -> bool {
        self.vid == 0 || self.vid == 4095
    }
}

/// VLAN validation error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VlanError {
    /// PCP > 7 (3-bit limit).
    PcpOutOfRange(u8),
    /// VID > 4095 (12-bit limit).
    VidOutOfRange(u16),
    /// Wire bytes shorter than 4.
    Truncated,
}

impl core::fmt::Display for VlanError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::PcpOutOfRange(p) => write!(f, "PCP {p} > 7"),
            Self::VidOutOfRange(v) => write!(f, "VID {v} > 4095"),
            Self::Truncated => f.write_str("VLAN tag < 4 bytes"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for VlanError {}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn well_known_tpid_constants() {
        // IEEE 802.1Q + 802.1ad.
        assert_eq!(TPID_8021Q, 0x8100);
        assert_eq!(TPID_8021AD, 0x88A8);
    }

    #[test]
    fn pcp_values_0_through_7_accepted() {
        for p in 0..=7u8 {
            assert!(Ieee802VlanTag::new(TPID_8021Q, p, false, 100).is_ok());
        }
    }

    #[test]
    fn pcp_above_7_rejected() {
        assert_eq!(
            Ieee802VlanTag::new(TPID_8021Q, 8, false, 100),
            Err(VlanError::PcpOutOfRange(8))
        );
    }

    #[test]
    fn vid_above_4095_rejected() {
        assert_eq!(
            Ieee802VlanTag::new(TPID_8021Q, 0, false, 4096),
            Err(VlanError::VidOutOfRange(4096))
        );
    }

    #[test]
    fn wire_round_trip_preserves_all_fields() {
        for (pcp, dei, vid) in [
            (0u8, false, 1u16),
            (7, true, 4094),
            (3, false, 100),
            (5, true, 200),
        ] {
            let t = Ieee802VlanTag::new(TPID_8021Q, pcp, dei, vid).expect("ok");
            let bytes = t.to_wire();
            let parsed = Ieee802VlanTag::from_wire(&bytes).expect("decode");
            assert_eq!(parsed, t);
        }
    }

    #[test]
    fn wire_layout_matches_spec_bit_packing() {
        // PCP=3 (binary 011), DEI=1, VID=100 (0x064).
        // TCI-High = 011|1|0000 = 0111_0000 = 0x70.
        // TCI-Low  = 0x64.
        let t = Ieee802VlanTag::new(TPID_8021Q, 3, true, 100).expect("ok");
        let bytes = t.to_wire();
        assert_eq!(bytes[0], 0x81);
        assert_eq!(bytes[1], 0x00);
        assert_eq!(bytes[2], 0x70);
        assert_eq!(bytes[3], 0x64);
    }

    #[test]
    fn from_wire_too_short_fails() {
        assert_eq!(
            Ieee802VlanTag::from_wire(&[0x81, 0x00]),
            Err(VlanError::Truncated)
        );
    }

    #[test]
    fn reserved_vids_detected() {
        let t0 = Ieee802VlanTag::new(TPID_8021Q, 0, false, 0).expect("ok");
        let t4095 = Ieee802VlanTag::new(TPID_8021Q, 0, false, 4095).expect("ok");
        let t100 = Ieee802VlanTag::new(TPID_8021Q, 0, false, 100).expect("ok");
        assert!(t0.is_reserved_vid());
        assert!(t4095.is_reserved_vid());
        assert!(!t100.is_reserved_vid());
    }
}
