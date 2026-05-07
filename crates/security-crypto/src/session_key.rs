// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-Security 1.2 §10.5.2 — Session-Key-Derivation + AAD-Format (C3.7).
//!
//! Spec §10.5.2 Tab.74 definiert die Session-Key-Derivation per HMAC-
//! SHA256 ueber ein Tupel (master_salt || tag-string || session_id):
//!
//! ```text
//! session_key                    = HMAC-SHA256(masterSenderKey,
//!                                              masterSalt || "SessionKey" || session_id)
//! session_receiver_specific_key  = HMAC-SHA256(masterReceiverSpecificKey,
//!                                              masterSalt || "SessionReceiverKey"
//!                                              || session_id)
//! ```
//!
//! Spec §8.1 Tab.78 definiert AAD im AES-GCM-Crypto-Header:
//!
//! ```text
//! AAD = octet array =
//!   transformation_kind (4 byte BE)         # CRYPTO_TRANSFORMATION_KIND_*
//!   transformation_key_id (4 byte BE)       # CryptoTransformKeyId (sender_key_id)
//!   session_id (4 byte BE)                  # CryptoTransformKeyId
//!   ...                                      # optional payload (extensions, body-only)
//! ```
//!
//! Konkret bauen wir den 16-byte Header (transformation_kind +
//! transformation_key_id + session_id + 4 reserved padding bytes) wie
//! in Cyclone/FastDDS. Erweiterungen (z.B. RTPS-Header beim
//! `rtps_protection_kind`) werden als zusaetzlicher Trailer angehaengt.
//!
//! # Scope C3.7
//!
//! Nur Helper-Funktionen + Tests — der Hot-Path
//! `AesGcmCryptoPlugin::encrypt_submessage` wird in C3.7-Followup auf
//! diese Funktionen umgestellt, sobald die Wire-Migration koordiniert
//! ist (zZt nutzt der Hot-Path noch die ZeroDDS-spezifische HKDF-
//! Pipeline).

extern crate alloc;

use alloc::vec::Vec;

use ring::hmac;

/// Spec-Tag-String fuer Session-Key (§10.5.2 Tab.74).
pub const SESSION_KEY_TAG: &[u8] = b"SessionKey";

/// Spec-Tag-String fuer Session-Receiver-Specific-Key (§10.5.2 Tab.74).
pub const SESSION_RECEIVER_KEY_TAG: &[u8] = b"SessionReceiverKey";

/// Laenge des AES-GCM-AAD-Headers (Spec §8.1 Tab.78): 16 byte.
pub const AAD_HEADER_LEN: usize = 16;

/// Spec §10.5.2 Tab.74 — Session-Key-Derivation.
///
/// `master_key` ist der `master_sender_key` aus dem KeyMaterial-Token
/// (16 byte fuer AES-128, 32 byte fuer AES-256). `master_salt` ist
/// der 32-byte salt aus dem gleichen Token. `session_id` wechselt pro
/// Session (4 byte).
///
/// Liefert immer 32 byte (HMAC-SHA256 output); der Caller schneidet
/// auf die suite-spezifische Key-Laenge ab.
#[must_use]
pub fn derive_session_key(master_key: &[u8], master_salt: &[u8], session_id: &[u8; 4]) -> [u8; 32] {
    derive_with_tag(master_key, master_salt, SESSION_KEY_TAG, session_id)
}

/// Spec §10.5.2 Tab.74 — Session-Receiver-Specific-Key-Derivation.
///
/// Wird fuer per-Receiver-spezifische MACs in DataReader-spezifischen
/// Slots berechnet (statt eines globalen Sender-Keys).
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

/// Spec §8.1 Tab.78 — AAD-Header fuer AES-GCM-Submessage-Verschluesselung.
///
/// Layout (16 byte BE):
///
/// ```text
/// 0..4   transformation_kind  (z.B. [0,0,0,0x02] fuer AES128_GCM)
/// 4..8   transformation_key_id (sender_key_id)
/// 8..12  session_id
/// 12..16 reserved (Spec: 4 byte fuer plug-spezifische Erweiterungen,
///        wir setzen 0)
/// ```
///
/// `extension` wird hinten angehaengt — z.B. der RTPS-Header beim
/// `rtps_protection_kind != NONE` (Spec §7.4.6.6).
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
        // Spec §10.5.2 Tab.74: Sender und Receiver-Specific haben
        // verschiedene Tag-Strings ("SessionKey" vs "SessionReceiverKey")
        // → mit selbem master_key + salt + sid muessen die Outputs
        // unterschiedlich sein.
        let mk = [0xAA; 32];
        let salt = [0xBB; 32];
        let sid = [0x01, 0x02, 0x03, 0x04];
        let sender = derive_session_key(&mk, &salt, &sid);
        let receiver = derive_session_hmac_key(&mk, &salt, &sid);
        assert_ne!(sender, receiver);
    }

    #[test]
    fn session_key_aes128_gcm_truncated_to_16_byte() {
        // Suite::Aes128Gcm hat key_len=16 — der Caller schneidet das
        // 32-byte HMAC-Output auf die ersten 16 byte ab.
        let mk = [0xAA; 16];
        let salt = [0xBB; 32];
        let sid = [0; 4];
        let full = derive_session_key(&mk, &salt, &sid);
        let aes128_key = &full[..16];
        assert_eq!(aes128_key.len(), 16);
        // Konsistenzcheck: zweiter Aufruf liefert identische ersten 16.
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
        // AES128_GCM vs AES256_GCM — verschiedene transformation_kind
        // muessen verschiedene AAD-Bytes ergeben (sonst kann ein Replier
        // ein AES-128-Token als AES-256-Token dekoden).
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
        // Wir testen indirekt ueber derive_with_tag, indem wir master_salt =
        // "Hi T" + b"" (tag) + "here" (4 byte session_id-Pfeil) konstruieren —
        // d.h. master_salt || tag || session_id == "Hi There".
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
