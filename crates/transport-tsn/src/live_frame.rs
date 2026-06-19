// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Platform-independent frame helpers for the live AF_PACKET transport.
//!
//! This logic (VLAN selection, Ethernet frame building incl. min-frame
//! padding, MAC parsing from the sysfs format) needs no syscalls and is
//! therefore testable on every platform — in contrast to [`crate::socket`],
//! which requires `AF_PACKET`/`SOCK_RAW` and thus Linux. `socket.rs`
//! calls these helpers exclusively, so the frame logic has exactly
//! one tested source.

use alloc::vec::Vec;

use crate::ethernet_psm::{ETHERTYPE_RTPS, EthernetFrameHeader};
use crate::mac::MacAddress;
use crate::vlan_tag::{Ieee802VlanTag, TPID_8021Q};

/// IEEE 802.3 minimum frame size without FCS.
pub const ETH_MIN_FRAME: usize = 60;

/// Error determining the outbound VLAN tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VlanError {
    /// `pcp` > 7 or `vid` > 4095.
    OutOfRange,
}

/// Error parsing a MAC from the sysfs `address` format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacParseError {
    /// Fewer than six octets.
    TooFewOctets,
    /// An octet is not valid hex.
    NotHex,
}

/// Selects the VLAN tag for an outbound frame:
///
/// * `target_vid != 0` (from the destination locator) overrides `default_vid`.
/// * effective `vid == 0` → untagged (`None`).
/// * otherwise an `Ieee802VlanTag` with the configured `pcp`.
///
/// # Errors
/// [`VlanError::OutOfRange`] if `pcp`/`vid` are outside the valid
/// range.
pub fn outbound_vlan(
    default_vid: u16,
    pcp: u8,
    target_vid: u16,
) -> Result<Option<Ieee802VlanTag>, VlanError> {
    let vid = if target_vid != 0 {
        target_vid
    } else {
        default_vid
    };
    if vid == 0 {
        return Ok(None);
    }
    Ieee802VlanTag::new(TPID_8021Q, pcp, false, vid)
        .map(Some)
        .map_err(|_| VlanError::OutOfRange)
}

/// Builds a complete Ethernet frame: header (with an optional
/// VLAN tag) + payload, padded to [`ETH_MIN_FRAME`] with null bytes
/// (tolerable as a PAD submessage).
#[must_use]
pub fn build_frame(
    local_mac: [u8; 6],
    dst_mac: [u8; 6],
    vlan: Option<Ieee802VlanTag>,
    data: &[u8],
) -> Vec<u8> {
    let hdr = EthernetFrameHeader {
        destination: MacAddress(dst_mac),
        source: MacAddress(local_mac),
        vlan_tag: vlan,
        ethertype: ETHERTYPE_RTPS,
    };
    let mut frame = hdr.to_wire();
    frame.extend_from_slice(data);
    if frame.len() < ETH_MIN_FRAME {
        frame.resize(ETH_MIN_FRAME, 0);
    }
    frame
}

/// Parses a MAC from the Linux sysfs `address` format
/// (`aa:bb:cc:dd:ee:ff`, lowercase, a trailing newline
/// is allowed).
///
/// # Errors
/// [`MacParseError`] on too few octets or non-hex.
pub fn parse_mac_sysfs(text: &str) -> Result<[u8; 6], MacParseError> {
    let mut mac = [0u8; 6];
    let mut parts = text.trim().split(':');
    for slot in &mut mac {
        let p = parts.next().ok_or(MacParseError::TooFewOctets)?;
        *slot = u8::from_str_radix(p, 16).map_err(|_| MacParseError::NotHex)?;
    }
    Ok(mac)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn target_vid_overrides_default() {
        let vt = outbound_vlan(10, 5, 200).expect("ok").expect("tagged");
        assert_eq!(vt.vid, 200);
        assert_eq!(vt.pcp, 5);
    }

    #[test]
    fn zero_target_falls_back_to_default_vid() {
        let vt = outbound_vlan(10, 3, 0).expect("ok").expect("tagged");
        assert_eq!(vt.vid, 10);
        assert_eq!(vt.pcp, 3);
    }

    #[test]
    fn zero_default_and_target_is_untagged() {
        assert_eq!(outbound_vlan(0, 0, 0).expect("ok"), None);
    }

    #[test]
    fn vid_out_of_range_is_error() {
        assert_eq!(outbound_vlan(0, 0, 5000), Err(VlanError::OutOfRange));
    }

    #[test]
    fn pcp_out_of_range_is_error() {
        assert_eq!(outbound_vlan(10, 9, 0), Err(VlanError::OutOfRange));
    }

    #[test]
    fn build_frame_untagged_has_14_byte_header() {
        let frame = build_frame([0xAA; 6], [0xBB; 6], None, &[1, 2, 3, 4]);
        // 14-byte header + 4-byte payload = 18, padded to 60.
        assert_eq!(frame.len(), ETH_MIN_FRAME);
        assert_eq!(&frame[0..6], &[0xBB; 6]); // destination
        assert_eq!(&frame[6..12], &[0xAA; 6]); // source
        assert_eq!(&frame[12..14], &ETHERTYPE_RTPS.to_be_bytes()); // ethertype
        assert_eq!(&frame[14..18], &[1, 2, 3, 4]); // payload
        assert!(frame[18..].iter().all(|&b| b == 0)); // padding
    }

    #[test]
    fn build_frame_tagged_has_18_byte_header() {
        let vlan = Ieee802VlanTag::new(TPID_8021Q, 6, false, 42).expect("vlan");
        let frame = build_frame([0xAA; 6], [0xBB; 6], Some(vlan), &[9; 100]);
        // 18-byte header + 100-byte payload = 118 (> 60, no padding).
        assert_eq!(frame.len(), 18 + 100);
        assert_eq!(&frame[0..6], &[0xBB; 6]);
        assert_eq!(&frame[6..12], &[0xAA; 6]);
        // VLAN TPID at offset 12.
        assert_eq!(&frame[12..14], &TPID_8021Q.to_be_bytes());
    }

    #[test]
    fn build_frame_large_payload_not_padded() {
        let frame = build_frame([0; 6], [0; 6], None, &[7; 80]);
        assert_eq!(frame.len(), 14 + 80);
    }

    #[test]
    fn parse_mac_sysfs_standard_format() {
        assert_eq!(
            parse_mac_sysfs("aa:bb:cc:dd:ee:ff").expect("ok"),
            [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]
        );
    }

    #[test]
    fn parse_mac_sysfs_tolerates_trailing_newline() {
        assert_eq!(
            parse_mac_sysfs("00:11:22:33:44:55\n").expect("ok"),
            [0x00, 0x11, 0x22, 0x33, 0x44, 0x55]
        );
    }

    #[test]
    fn parse_mac_sysfs_too_few_octets() {
        assert_eq!(
            parse_mac_sysfs("aa:bb:cc"),
            Err(MacParseError::TooFewOctets)
        );
    }

    #[test]
    fn parse_mac_sysfs_non_hex() {
        assert_eq!(
            parse_mac_sysfs("aa:bb:cc:dd:ee:zz"),
            Err(MacParseError::NotHex)
        );
    }
}
