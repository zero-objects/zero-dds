// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-Security §9.3.3 participant GUID adjustment.
//!
//! Binds the participant GUID prefix cryptographically to the identity
//! (certificate), so that nobody spoofs foreign GUIDs. The algorithm is
//! byte-identical to OpenDDS `make_adjusted_guid` / cyclone's `q_security`
//! — cyclone/FastDDS check in the handshake (`begin_handshake_reply`) that the
//! `c.pdata` GUID is consistently derived from the identity, and reject a
//! random GUID with "c.pdata contains incorrect participant guid".

use crate::identity::PkiError;
use alloc::format;

/// Computes the adjusted participant GUID prefix (§9.3.3):
///
/// - `prefix[0..6]` = `offset_1bit(SHA256(DER(subject_name)), i)`, then
///   `prefix[0] |= 0x80` (marks the GUID as adjusted).
/// - `prefix[6..12]` = `SHA256(candidate_guid)[0..6]`.
///
/// `candidate_guid` is the 16-byte candidate GUID (random prefix +
/// participant EntityId `0x000001C1`); the 12-byte adjusted prefix
/// is returned. The EntityId stays unchanged (the caller keeps it).
///
/// # Errors
/// `PkiError::InvalidPem` on cert parse / subject encode errors.
pub fn adjust_participant_guid_prefix(
    candidate_guid: &[u8; 16],
    cert_der: &[u8],
) -> Result<[u8; 12], PkiError> {
    use ring::digest::{SHA256, digest};
    use x509_cert::Certificate;
    use x509_cert::der::{Decode, Encode};

    let cert = Certificate::from_der(cert_der)
        .map_err(|e| PkiError::InvalidPem(format!("adjust_guid: cert der: {e}")))?;
    // SHA256 over the DER encoding of the X.509 subject name — exactly
    // OpenDDS `subject_name_digest` (X509_NAME_digest(name, sha256)).
    let subject_der = cert
        .tbs_certificate
        .subject
        .to_der()
        .map_err(|e| PkiError::InvalidPem(format!("adjust_guid: subject der: {e}")))?;
    let h1 = digest(&SHA256, &subject_der);
    let h1 = h1.as_ref();
    // SHA256 over the 16-byte candidate GUID (OpenDDS hashes `&src,
    // sizeof(GUID_t)`).
    let h2 = digest(&SHA256, candidate_guid);
    let h2 = h2.as_ref();

    let mut prefix = [0u8; 12];
    for (i, p) in prefix.iter_mut().take(6).enumerate() {
        *p = offset_1bit(h1, i);
    }
    prefix[0] |= 0x80;
    prefix[6..12].copy_from_slice(&h2[0..6]);
    Ok(prefix)
}

/// 1-bit right shift of the byte array (OpenDDS `offset_1bit`):
/// `(a[i] >> 1) | (i==0 ? 0 : (a[i-1] & 1 ? 0x80 : 0))`.
fn offset_1bit(a: &[u8], i: usize) -> u8 {
    (a[i] >> 1)
        | if i == 0 {
            0
        } else if a[i - 1] & 1 != 0 {
            0x80
        } else {
            0
        }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn offset_1bit_is_a_one_bit_right_shift() {
        // 0b1000_0001, 0b0000_0010 → right shift by 1 bit over the stream:
        // out[0] = 1000_0001 >> 1 = 0100_0000 (no predecessor).
        // out[1] = 0000_0010 >> 1 | (a[0]&1 ? 0x80 : 0) = 0000_0001 | 0x80 = 1000_0001.
        let a = [0b1000_0001u8, 0b0000_0010u8];
        assert_eq!(offset_1bit(&a, 0), 0b0100_0000);
        assert_eq!(offset_1bit(&a, 1), 0b1000_0001);
    }

    #[test]
    fn adjusted_prefix_sets_high_bit_and_is_deterministic_per_identity() {
        use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
        let mk_cert = |cn: &str| -> alloc::vec::Vec<u8> {
            let mut p = CertificateParams::new(alloc::vec::Vec::new()).unwrap();
            let mut dn = DistinguishedName::new();
            dn.push(DnType::CommonName, cn);
            p.distinguished_name = dn;
            let k = KeyPair::generate().unwrap();
            let c = p.self_signed(&k).unwrap();
            c.der().to_vec()
        };
        let cert_a = mk_cert("CN=alice");
        let cert_b = mk_cert("CN=bob");
        let candidate = [
            0x01, 0x10, 0xff, 0xb5, 0xb6, 0xb2, 0xd6, 0x02, 0x9d, 0x3b, 0xc9, 0x06, 0x00, 0x00,
            0x01, 0xc1,
        ];

        let pa = adjust_participant_guid_prefix(&candidate, &cert_a).unwrap();
        // High bit of byte 0 set (adjusted marker).
        assert_eq!(pa[0] & 0x80, 0x80);
        // Deterministic for the same identity + candidate GUID.
        assert_eq!(
            pa,
            adjust_participant_guid_prefix(&candidate, &cert_a).unwrap()
        );
        // Different identity → different prefix (subject hash feeds in).
        let pb = adjust_participant_guid_prefix(&candidate, &cert_b).unwrap();
        assert_ne!(pa, pb);
        // Different candidate GUID → different trailing 6 bytes (h2), leading 6
        // (subject hash) stay the same.
        let mut candidate2 = candidate;
        candidate2[3] ^= 0xff;
        let pa2 = adjust_participant_guid_prefix(&candidate2, &cert_a).unwrap();
        assert_eq!(
            pa[..6],
            pa2[..6],
            "leading 6 bytes = subject hash (identity-bound)"
        );
        assert_ne!(
            pa[6..],
            pa2[6..],
            "trailing 6 bytes = hash of the candidate GUID"
        );
    }
}
