// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! RTPS-Header-AAD fuer SRTPS-Wrapping — DDS-Security 1.2 §7.4.6.6 + §8.1.
//!
//! Wenn `rtps_protection_kind != NONE`, MUSS der vollstaendige
//! RTPS-Header (20 Bytes) zur AAD (Authenticated-Additional-Data)
//! hinzugefuegt werden. Damit ist der Header gegen Tampering
//! geschuetzt — ein Angreifer kann nicht den Sender-GuidPrefix
//! aendern, ohne dass der GCM-Tag invalid wird.

use alloc::vec::Vec;

/// Wire-Size eines RTPS-Headers (Spec §8.3.5.1: 4 magic + 2 vendor +
/// 2 version + 12 GuidPrefix = 20 Bytes).
pub const RTPS_HEADER_LEN: usize = 20;

/// Baut den AAD-Slot fuer einen `SRTPS_PREFIX`-gewrapten Datagram-
/// Schutz. Spec §7.4.6.6:
///
/// ```text
///   AAD = transformation_kind ||
///         transformation_key_id ||
///         session_id ||
///         reserved-4 ||
///         RTPS-Header[0..20]
/// ```
///
/// `transformation_*` und `session_id` kommen aus dem `CryptoHeader`
/// der SRTPS_PREFIX-Submessage; den RTPS-Header liefert der Caller
/// als 20-Byte-Slice.
///
/// # Errors
/// Static-String wenn `rtps_header_bytes.len() < 20`.
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

/// Spec §7.4.7.8/9: AAD fuer SubmessageProtection ist der
/// `SEC_PREFIX`-Submessage-Header (vor dem CryptoHeader) plus die
/// Crypto-Header-Bytes selbst.
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
