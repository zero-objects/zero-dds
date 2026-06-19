// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-Security 1.2 §10.5.2 — session-key derivation + AAD format (C3.7).
//!
//! Spec §10.5.2 Tab.74 defines the session-key derivation via HMAC-
//! SHA256 over a tuple (master_salt || tag-string || session_id):
//!
//! ```text
//! session_key                    = HMAC-SHA256(masterSenderKey,
//!                                              masterSalt || "SessionKey" || session_id)
//! session_receiver_specific_key  = HMAC-SHA256(masterReceiverSpecificKey,
//!                                              masterSalt || "SessionReceiverKey"
//!                                              || session_id)
//! ```
//!
//! Spec §8.1 Tab.78 defines the AAD in the AES-GCM crypto header:
//!
//! ```text
//! AAD = octet array =
//!   transformation_kind (4 byte BE)         # CRYPTO_TRANSFORMATION_KIND_*
//!   transformation_key_id (4 byte BE)       # CryptoTransformKeyId (sender_key_id)
//!   session_id (4 byte BE)                  # CryptoTransformKeyId
//!   ...                                      # optional payload (extensions, body-only)
//! ```
//!
//! Concretely we build the 16-byte header (transformation_kind +
//! transformation_key_id + session_id + 4 reserved padding bytes) as
//! in Cyclone/FastDDS. Extensions (e.g. the RTPS header for
//! `rtps_protection_kind`) are appended as an additional trailer.
//!
//! # Scope C3.7
//!
//! Only helper functions + tests — the hot path
//! `AesGcmCryptoPlugin::encrypt_submessage` is switched to
//! these functions in a C3.7 follow-up, once the wire migration is coordinated
//! (currently the hot path still uses the ZeroDDS-specific HKDF
//! pipeline).

extern crate alloc;

use alloc::vec::Vec;

use ring::hmac;

/// Spec tag string for the session key (§10.5.2 Tab.74).
pub const SESSION_KEY_TAG: &[u8] = b"SessionKey";

/// Spec tag string for the session receiver-specific key (§10.5.2 Tab.74).
pub const SESSION_RECEIVER_KEY_TAG: &[u8] = b"SessionReceiverKey";

/// Length of the AES-GCM AAD header (spec §8.1 Tab.78): 16 bytes.
pub const AAD_HEADER_LEN: usize = 16;

/// Spec §10.5.2 Tab.74 — session-key derivation.
///
/// `master_key` is the `master_sender_key` from the KeyMaterial token
/// (16 bytes for AES-128, 32 bytes for AES-256). `master_salt` is
/// the 32-byte salt from the same token. `session_id` changes per
/// session (4 bytes).
///
/// Always returns 32 bytes (HMAC-SHA256 output); the caller truncates
/// to the suite-specific key length.
#[must_use]
pub fn derive_session_key(master_key: &[u8], master_salt: &[u8], session_id: &[u8; 4]) -> [u8; 32] {
    derive_with_tag(master_key, master_salt, SESSION_KEY_TAG, session_id)
}

/// Spec §10.5.2 Tab.74 — session receiver-specific key derivation.
///
/// Computed for per-receiver-specific MACs in DataReader-specific
/// slots (instead of a global sender key).
#[must_use]
pub fn derive_session_hmac_key(
    master_receiver_specific_key: &[u8],
    master_salt: &[u8],
    session_id: &[u8; 4],
) -> [u8; 32] {
    derive_with_tag(
        master_receiver_specific_key,
        master_salt,
        SESSION_RECEIVER_KEY_TAG,
        session_id,
    )
}

/// Session key **byte-identical to cyclone** (`crypto_calculate_key_impl`,
/// crypto_utils.c): `HMAC-SHA256(master_key, "SessionKey" || master_salt[..key_bytes]
/// || session_id_BE)`. The tag prefix comes **first** (unlike
/// [`derive_session_key`], which internally had `master_salt` first in ZeroDDS) and
/// `master_salt` is shortened to the suite key length — both needed for
/// cross-vendor submessage protection (§9.5.3.3). `session_id` is already the
/// 4-byte BE array from the CryptoHeader.
#[must_use]
pub fn derive_session_key_cyclone(
    master_key: &[u8],
    master_salt: &[u8],
    session_id: &[u8; 4],
    key_bytes: usize,
) -> [u8; 32] {
    let hmac_key = hmac::Key::new(hmac::HMAC_SHA256, master_key);
    let mut ctx = hmac::Context::with_key(&hmac_key);
    ctx.update(SESSION_KEY_TAG);
    let salt_len = key_bytes.min(master_salt.len());
    ctx.update(&master_salt[..salt_len]);
    ctx.update(session_id);
    let mut out = [0u8; 32];
    out.copy_from_slice(ctx.sign().as_ref());
    out
}

fn derive_with_tag(key: &[u8], master_salt: &[u8], tag: &[u8], session_id: &[u8; 4]) -> [u8; 32] {
    let hmac_key = hmac::Key::new(hmac::HMAC_SHA256, key);
    let mut ctx = hmac::Context::with_key(&hmac_key);
    ctx.update(master_salt);
    ctx.update(tag);
    ctx.update(session_id);
    let tag = ctx.sign();
    let mut out = [0u8; 32];
    out.copy_from_slice(tag.as_ref());
    out
}

/// Spec §8.1 Tab.78 — AAD header for AES-GCM submessage encryption.
///
/// Layout (16 bytes BE):
///
/// ```text
/// 0..4   transformation_kind  (e.g. [0,0,0,0x02] for AES128_GCM)
/// 4..8   transformation_key_id (sender_key_id)
/// 8..12  session_id
/// 12..16 reserved (spec: 4 bytes for plugin-specific extensions,
///        we set 0)
/// ```
///
/// `extension` is appended at the end — e.g. the RTPS header for
/// `rtps_protection_kind != NONE` (spec §7.4.6.6).
#[must_use]
pub fn compute_aad(
    transformation_kind: [u8; 4],
    transformation_key_id: [u8; 4],
    session_id: [u8; 4],
    extension: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(AAD_HEADER_LEN + extension.len());
    out.extend_from_slice(&transformation_kind);
    out.extend_from_slice(&transformation_key_id);
    out.extend_from_slice(&session_id);
    out.extend_from_slice(&[0u8; 4]); // reserved padding
    out.extend_from_slice(extension);
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn session_key_is_deterministic() {
        let mk = [0xAA; 32];
        let salt = [0xBB; 32];
        let sid = [0x01, 0x02, 0x03, 0x04];
        let k1 = derive_session_key(&mk, &salt, &sid);
        let k2 = derive_session_key(&mk, &salt, &sid);
        assert_eq!(k1, k2);
    }

    #[test]
    fn session_key_changes_with_session_id() {
        let mk = [0xAA; 32];
        let salt = [0xBB; 32];
        let k1 = derive_session_key(&mk, &salt, &[1, 2, 3, 4]);
        let k2 = derive_session_key(&mk, &salt, &[1, 2, 3, 5]);
        assert_ne!(k1, k2);
    }

    #[test]
    fn session_key_changes_with_master_salt() {
        let mk = [0xAA; 32];
        let sid = [0x01, 0x02, 0x03, 0x04];
        let k1 = derive_session_key(&mk, &[0xBB; 32], &sid);
        let k2 = derive_session_key(&mk, &[0xCC; 32], &sid);
        assert_ne!(k1, k2);
    }

    #[test]
    fn session_key_changes_with_master_key() {
        let salt = [0xBB; 32];
        let sid = [0x01, 0x02, 0x03, 0x04];
        let k1 = derive_session_key(&[0xAA; 32], &salt, &sid);
        let k2 = derive_session_key(&[0xCC; 32], &salt, &sid);
        assert_ne!(k1, k2);
    }

    #[test]
    fn sender_key_and_receiver_key_use_different_tags() {
        // Spec §10.5.2 Tab.74: sender and receiver-specific have
        // different tag strings ("SessionKey" vs "SessionReceiverKey")
        // → with the same master_key + salt + sid the outputs must
        // differ.
        let mk = [0xAA; 32];
        let salt = [0xBB; 32];
        let sid = [0x01, 0x02, 0x03, 0x04];
        let sender = derive_session_key(&mk, &salt, &sid);
        let receiver = derive_session_hmac_key(&mk, &salt, &sid);
        assert_ne!(sender, receiver);
    }

    #[test]
    fn session_key_aes128_gcm_truncated_to_16_byte() {
        // Suite::Aes128Gcm has key_len=16 — the caller truncates the
        // 32-byte HMAC output to the first 16 bytes.
        let mk = [0xAA; 16];
        let salt = [0xBB; 32];
        let sid = [0; 4];
        let full = derive_session_key(&mk, &salt, &sid);
        let aes128_key = &full[..16];
        assert_eq!(aes128_key.len(), 16);
        // Consistency check: a second call returns identical first 16.
        let full2 = derive_session_key(&mk, &salt, &sid);
        assert_eq!(&full2[..16], aes128_key);
    }

    #[test]
    fn aad_layout_is_16_byte_with_padding() {
        let aad = compute_aad([0, 0, 0, 0x02], [0, 0, 0, 0x07], [0, 0, 0, 0x42], &[]);
        assert_eq!(aad.len(), AAD_HEADER_LEN);
        assert_eq!(&aad[0..4], &[0, 0, 0, 0x02]);
        assert_eq!(&aad[4..8], &[0, 0, 0, 0x07]);
        assert_eq!(&aad[8..12], &[0, 0, 0, 0x42]);
        assert_eq!(&aad[12..16], &[0, 0, 0, 0]); // reserved padding
    }

    #[test]
    fn aad_with_extension_appends_after_header() {
        let ext = b"rtps-header-bytes";
        let aad = compute_aad([0; 4], [0; 4], [0; 4], ext);
        assert_eq!(aad.len(), AAD_HEADER_LEN + ext.len());
        assert_eq!(&aad[AAD_HEADER_LEN..], ext);
    }

    #[test]
    fn aad_distinct_for_different_session_ids() {
        let a = compute_aad([0; 4], [0; 4], [1, 2, 3, 4], &[]);
        let b = compute_aad([0; 4], [0; 4], [1, 2, 3, 5], &[]);
        assert_ne!(a, b);
    }

    #[test]
    fn aad_distinct_for_different_kinds() {
        // AES128_GCM vs AES256_GCM — different transformation_kind
        // must yield different AAD bytes (otherwise a replier could
        // decode an AES-128 token as an AES-256 token).
        let a = compute_aad([0, 0, 0, 0x02], [0; 4], [0; 4], &[]);
        let b = compute_aad([0, 0, 0, 0x04], [0; 4], [0; 4], &[]);
        assert_ne!(a, b);
    }

    #[test]
    fn rfc_4231_hmac_sha256_known_vector_via_derive() {
        // RFC 4231 Test Case 1:
        //   key = 0x0b * 20
        //   data = "Hi There"
        //   HMAC-SHA-256 = b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7
        // We test indirectly via derive_with_tag by constructing master_salt =
        // "Hi T" + b"" (tag) + "here" (4-byte session_id slice) —
        // i.e. master_salt || tag || session_id == "Hi There".
        let key = [0x0b; 20];
        let master_salt = b"Hi T";
        let tag: &[u8] = b"";
        let sid = [b'h', b'e', b'r', b'e'];
        let out = derive_with_tag(&key, master_salt, tag, &sid);
        let expected: [u8; 32] = [
            0xb0, 0x34, 0x4c, 0x61, 0xd8, 0xdb, 0x38, 0x53, 0x5c, 0xa8, 0xaf, 0xce, 0xaf, 0x0b,
            0xf1, 0x2b, 0x88, 0x1d, 0xc2, 0x00, 0xc9, 0x83, 0x3d, 0xa7, 0x26, 0xe9, 0x37, 0x6c,
            0x2e, 0x32, 0xcf, 0xf7,
        ];
        assert_eq!(out, expected);
    }
}
