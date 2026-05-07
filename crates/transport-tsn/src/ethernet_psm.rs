// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDSI-RTPS Ethernet PSM — Spec Annex A.
//!
//! Spec Annex A definiert ein PSM, in dem RTPS-Messages direkt im
//! Ethernet-Frame-Payload gesendet werden — ohne IP/UDP-Stack. Das
//! ist sinnvoll fuer hard-real-time Use-Cases, wo der IP-Stack zu
//! viel Latenz oder Jitter hinzufuegt.

use crate::mac::MacAddress;
use crate::vlan_tag::Ieee802VlanTag;

/// Spec Annex A — RTPS-EtherType (vendor-spezifisch; Spec sagt
/// implementer-defined, IANA-Reserved-Bereich).
///
/// Wir nutzen den durch RTI/Cyclone-DDS verbreiteten Wert `0x88B5`
/// (IEEE Std 802 Local Experimental EtherType).
pub const ETHERTYPE_RTPS: u16 = 0x88B5;

/// Spec Annex A — Ethernet-Frame-Header fuer RTPS-direct-on-Ethernet.
///
/// Wire-Layout:
/// ```text
///  +---------+---------+--------+--------+---------+
///  | Dst MAC | Src MAC | VLAN?  | Type   | Payload |
///  | 6 byte  | 6 byte  | 4 byte | 2 byte | n bytes |
///  +---------+---------+--------+--------+---------+
/// ```
/// VLAN-Tag (4 bytes) ist optional — wenn vorhanden, dann mit
/// EtherType=0x8100/0x88a8 als TPID, sonst direkt Type-Field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EthernetFrameHeader {
    /// Destination MAC.
    pub destination: MacAddress,
    /// Source MAC.
    pub source: MacAddress,
    /// Optionaler VLAN-Tag.
    pub vlan_tag: Option<Ieee802VlanTag>,
    /// EtherType (typisch `ETHERTYPE_RTPS` = 0x88B5 fuer DDS-TSN).
    pub ethertype: u16,
}

impl EthernetFrameHeader {
    /// Header-Laenge in Bytes (14 ohne VLAN, 18 mit VLAN).
    #[must_use]
    pub fn header_len(self) -> usize {
        if self.vlan_tag.is_some() { 18 } else { 14 }
    }

    /// Encodet den Header zu Wire-Bytes.
    #[must_use]
    pub fn to_wire(self) -> alloc::vec::Vec<u8> {
        let mut out = alloc::vec::Vec::with_capacity(self.header_len());
        out.extend_from_slice(&self.destination.0);
        out.extend_from_slice(&self.source.0);
        if let Some(vt) = self.vlan_tag {
            out.extend_from_slice(&vt.to_wire());
        }
        out.extend_from_slice(&self.ethertype.to_be_bytes());
        out
    }

    /// Decodet den Header von Wire-Bytes. Liefert `(header,
    /// consumed_bytes)`.
    ///
    /// # Errors
    /// `EthError::Truncated` wenn weniger als 14 Bytes vorhanden.
    pub fn from_wire(bytes: &[u8]) -> Result<(Self, usize), EthError> {
        if bytes.len() < 14 {
            return Err(EthError::Truncated);
        }
        let mut dst = [0u8; 6];
        let mut src = [0u8; 6];
        dst.copy_from_slice(&bytes[0..6]);
        src.copy_from_slice(&bytes[6..12]);
        let possible_tpid = u16::from_be_bytes([bytes[12], bytes[13]]);
        if (possible_tpid == crate::vlan_tag::TPID_8021Q
            || possible_tpid == crate::vlan_tag::TPID_8021AD)
            && bytes.len() >= 18
        {
            let vt =
                Ieee802VlanTag::from_wire(&bytes[12..16]).map_err(|_| EthError::InvalidVlan)?;
            let ethertype = u16::from_be_bytes([bytes[16], bytes[17]]);
            Ok((
                Self {
                    destination: MacAddress(dst),
                    source: MacAddress(src),
                    vlan_tag: Some(vt),
                    ethertype,
                },
                18,
            ))
        } else {
            Ok((
                Self {
                    destination: MacAddress(dst),
                    source: MacAddress(src),
                    vlan_tag: None,
                    ethertype: possible_tpid,
                },
                14,
            ))
        }
    }
}

/// Ethernet-PSM-Fehler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EthError {
    /// Header < 14 Bytes (oder < 18 mit VLAN-Tag).
    Truncated,
    /// VLAN-Tag konnte nicht geparst werden.
    InvalidVlan,
}

impl core::fmt::Display for EthError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated => f.write_str("ethernet header truncated"),
            Self::InvalidVlan => f.write_str("invalid VLAN tag"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EthError {}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::vlan_tag::TPID_8021Q;

    #[test]
    fn rtps_ethertype_constant() {
        assert_eq!(ETHERTYPE_RTPS, 0x88B5);
    }

    #[test]
    fn header_without_vlan_is_14_bytes() {
        let h = EthernetFrameHeader {
            destination: MacAddress::BROADCAST,
            source: MacAddress::new([1; 6]),
            vlan_tag: None,
            ethertype: ETHERTYPE_RTPS,
        };
        assert_eq!(h.header_len(), 14);
        assert_eq!(h.to_wire().len(), 14);
    }

    #[test]
    fn header_with_vlan_is_18_bytes() {
        let h = EthernetFrameHeader {
            destination: MacAddress::BROADCAST,
            source: MacAddress::new([1; 6]),
            vlan_tag: Some(Ieee802VlanTag::new(TPID_8021Q, 5, false, 100).expect("ok")),
            ethertype: ETHERTYPE_RTPS,
        };
        assert_eq!(h.header_len(), 18);
        assert_eq!(h.to_wire().len(), 18);
    }

    #[test]
    fn round_trip_without_vlan() {
        let h = EthernetFrameHeader {
            destination: MacAddress::new([0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02]),
            source: MacAddress::new([0xCA, 0xFE, 0xBA, 0xBE, 0x03, 0x04]),
            vlan_tag: None,
            ethertype: ETHERTYPE_RTPS,
        };
        let bytes = h.to_wire();
        let (parsed, consumed) = EthernetFrameHeader::from_wire(&bytes).expect("decode");
        assert_eq!(parsed, h);
        assert_eq!(consumed, 14);
    }

    #[test]
    fn round_trip_with_vlan() {
        let h = EthernetFrameHeader {
            destination: MacAddress::new([0x01, 0x80, 0xC2, 0x00, 0x00, 0x0E]),
            source: MacAddress::new([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]),
            vlan_tag: Some(Ieee802VlanTag::new(TPID_8021Q, 7, true, 200).expect("ok")),
            ethertype: ETHERTYPE_RTPS,
        };
        let bytes = h.to_wire();
        let (parsed, consumed) = EthernetFrameHeader::from_wire(&bytes).expect("decode");
        assert_eq!(parsed, h);
        assert_eq!(consumed, 18);
    }

    #[test]
    fn truncated_input_is_error() {
        assert_eq!(
            EthernetFrameHeader::from_wire(&[0; 13]),
            Err(EthError::Truncated)
        );
    }

    #[test]
    fn ipv4_ethertype_parsed_without_vlan_branch() {
        // Wenn die Bytes 12..14 nicht 0x8100/0x88a8 sind, kein VLAN-Tag.
        let mut bytes = [0u8; 14];
        // Type = 0x0800 (IPv4).
        bytes[12] = 0x08;
        bytes[13] = 0x00;
        let (h, consumed) = EthernetFrameHeader::from_wire(&bytes).expect("decode");
        assert_eq!(consumed, 14);
        assert!(h.vlan_tag.is_none());
        assert_eq!(h.ethertype, 0x0800);
    }
}
