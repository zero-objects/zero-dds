// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! RTPS header AAD for SRTPS wrapping — DDS-Security 1.2 §7.4.6.6 + §8.1.
//!
//! When `rtps_protection_kind != NONE`, the full
//! RTPS header (20 bytes) MUST be added to the AAD (authenticated
//! additional data). This protects the header against tampering
//! — an attacker cannot change the sender GuidPrefix
//! without making the GCM tag invalid.

use alloc::vec::Vec;

/// Wire size of an RTPS header (Spec §8.3.5.1: 4 magic + 2 vendor +
/// 2 version + 12 GuidPrefix = 20 Bytes).
pub const RTPS_HEADER_LEN: usize = 20;

/// Builds the AAD slot for an `SRTPS_PREFIX`-wrapped datagram
/// protection. Spec §7.4.6.6:
///
/// ```text
///   AAD = transformation_kind ||
///         transformation_key_id ||
///         session_id ||
///         reserved-4 ||
///         RTPS-Header[0..20]
/// ```
///
/// `transformation_*` and `session_id` come from the `CryptoHeader`
/// of the SRTPS_PREFIX submessage; the caller provides the RTPS header
/// as a 20-byte slice.
///
/// # Errors
/// A static string if `rtps_header_bytes.len() < 20`.
pub fn build_rtps_header_aad(
    transformation_kind: [u8; 4],
    transformation_key_id: [u8; 4],
    session_id: [u8; 4],
    rtps_header_bytes: &[u8],
) -> Result<Vec<u8>, &'static str> {
    if rtps_header_bytes.len() < RTPS_HEADER_LEN {
        return Err("rtps header < 20 bytes");
    }
    let mut out = Vec::with_capacity(16 + RTPS_HEADER_LEN);
    out.extend_from_slice(&transformation_kind);
    out.extend_from_slice(&transformation_key_id);
    out.extend_from_slice(&session_id);
    out.extend_from_slice(&[0u8; 4]); // reserved
    out.extend_from_slice(&rtps_header_bytes[..RTPS_HEADER_LEN]);
    Ok(out)
}

/// Spec §7.4.7.8/9: the AAD for SubmessageProtection is the
/// `SEC_PREFIX` submessage header (before the CryptoHeader) plus the
/// crypto-header bytes themselves.
#[must_use]
pub fn build_submessage_aad(
    transformation_kind: [u8; 4],
    transformation_key_id: [u8; 4],
    session_id: [u8; 4],
    sec_prefix_header_bytes: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + sec_prefix_header_bytes.len());
    out.extend_from_slice(&transformation_kind);
    out.extend_from_slice(&transformation_key_id);
    out.extend_from_slice(&session_id);
    out.extend_from_slice(&[0u8; 4]); // reserved
    out.extend_from_slice(sec_prefix_header_bytes);
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn rtps_header_aad_round_trip() {
        let kind = [0, 0, 0, 0x02];
        let key_id = [1, 2, 3, 4];
        let sid = [10, 20, 30, 40];
        let aad = build_rtps_header_aad(kind, key_id, sid, &[0xCAu8; 20]).unwrap();
        assert_eq!(aad.len(), 16 + 20);
        assert_eq!(&aad[0..4], &kind);
        assert_eq!(&aad[4..8], &key_id);
        assert_eq!(&aad[8..12], &sid);
        assert_eq!(&aad[12..16], &[0, 0, 0, 0]);
        assert_eq!(&aad[16..36], &[0xCA; 20]);
    }

    #[test]
    fn rtps_header_aad_short_buffer_rejected() {
        assert!(build_rtps_header_aad([0; 4], [0; 4], [0; 4], &[0; 10]).is_err());
    }

    #[test]
    fn submessage_aad_includes_prefix_header() {
        let aad = build_submessage_aad([0, 0, 0, 0x04], [1; 4], [2; 4], &[0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(aad.len(), 16 + 4);
        assert_eq!(&aad[16..20], &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn rtps_header_len_matches_spec() {
        // Spec §8.3.5.1: 4 magic + 2 vendor + 2 version + 12 GuidPrefix.
        assert_eq!(RTPS_HEADER_LEN, 20);
    }

    #[test]
    fn aad_changes_with_kind() {
        let aad1 = build_rtps_header_aad([0, 0, 0, 2], [0; 4], [0; 4], &[0; 20]).unwrap();
        let aad2 = build_rtps_header_aad([0, 0, 0, 4], [0; 4], [0; 4], &[0; 20]).unwrap();
        assert_ne!(aad1, aad2);
    }

    #[test]
    fn aad_changes_with_session_id() {
        let aad1 = build_rtps_header_aad([0; 4], [0; 4], [1; 4], &[0; 20]).unwrap();
        let aad2 = build_rtps_header_aad([0; 4], [0; 4], [2; 4], &[0; 20]).unwrap();
        assert_ne!(aad1, aad2);
    }

    #[test]
    fn aad_changes_with_rtps_header_content() {
        let aad1 = build_rtps_header_aad([0; 4], [0; 4], [0; 4], &[0xAA; 20]).unwrap();
        let aad2 = build_rtps_header_aad([0; 4], [0; 4], [0; 4], &[0xBB; 20]).unwrap();
        assert_ne!(aad1, aad2);
    }
}
