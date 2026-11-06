// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-security-permissions` with `zerodds-security-crypto`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-security-crypto`, from its non-test
//! sources:
//! - `rsa_pss::verify_rsassa_pss_sha256(public_key_der, message, signature)`:
//!   `cms.rs` verifies an RSASSA-PSS signature of the signed permissions and
//!   governance documents with it and takes `is_ok()` as the verdict.
//! - `PskDerivedCryptoPlugin::with_suite(Suite::Aes128Gcm)` and
//!   `register_psk(1, key)`: the crypto plugin of the `PskProfile` bundle
//!   (`psk_access.rs`), handed to the caller as a public field.
//!
//! What it relies on: a signature of any salt length verifies, as
//! DDS-Security 1.2 §8.2 Table 25 demands ("The validation of the signature
//! shall detect the salt length from the signature"), and anything else is
//! refused; `Suite::Aes128Gcm` is the AES-128-GCM suite; an empty
//! pre-shared key is refused.
//!
//! Oracles: signatures made by OpenSSL 4.0.3 with salt lengths 0, 8, 20,
//! 32, 64 and 222 (2048-bit key) and 0, 32 and 94 (3072-bit key), in
//! `crates/security-crypto/tests/fixtures/rsa_pss/` (`README.txt` gives the
//! commands; `openssl pkeyutl -verify` with `rsa_pss_saltlen:N` accepts each
//! of them); RFC 8017 §8.1.2 (a signature has the length of the modulus);
//! DDS-Security 1.2 §8.1 (`CID_AES128_GCM` = 0x02) and §10.5.2.2 Table 77
//! (an AES128 variant has a 16-octet master_sender_key); the rustdoc of
//! `PskDerivedCryptoPlugin::register_psk` (`BadArgument` if the key is
//! empty).
#![allow(clippy::unwrap_used)]

use zerodds_security::error::SecurityErrorKind;
use zerodds_security_crypto::rsa_pss::verify_rsassa_pss_sha256;
use zerodds_security_crypto::{PskDerivedCryptoPlugin, Suite};

const MSG: &[u8] = include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/msg.bin");
const PUB_2048: &[u8] = include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/pub2048.der");
const PUB_3072: &[u8] = include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/pub3072.der");

/// The 2048-bit signatures with their salt lengths.
const SIG_2048: [(&[u8], usize); 6] = [
    (
        include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/sig2048_0.bin"),
        0,
    ),
    (
        include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/sig2048_8.bin"),
        8,
    ),
    (
        include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/sig2048_20.bin"),
        20,
    ),
    (
        include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/sig2048_32.bin"),
        32,
    ),
    (
        include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/sig2048_64.bin"),
        64,
    ),
    (
        include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/sig2048_222.bin"),
        222,
    ),
];

/// The 3072-bit signatures with their salt lengths.
const SIG_3072: [(&[u8], usize); 3] = [
    (
        include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/sig3072_0.bin"),
        0,
    ),
    (
        include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/sig3072_32.bin"),
        32,
    ),
    (
        include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/sig3072_94.bin"),
        94,
    ),
];

#[test]
fn security_8_2_rsassa_pss_signatures_of_openssl_verify_with_their_salt_length() {
    for (sig, salt) in SIG_2048 {
        assert_eq!(sig.len(), 256);
        assert_eq!(verify_rsassa_pss_sha256(PUB_2048, MSG, sig).unwrap(), salt);
    }
    for (sig, salt) in SIG_3072 {
        assert_eq!(sig.len(), 384);
        assert_eq!(verify_rsassa_pss_sha256(PUB_3072, MSG, sig).unwrap(), salt);
    }
}

#[test]
fn security_8_2_anything_but_the_signed_message_and_its_key_is_refused() {
    let (sig, _) = SIG_2048[3];
    let refused = |r: zerodds_security::error::SecurityResult<usize>| matches!(r, Err(e) if e.kind == SecurityErrorKind::CryptoFailed);

    // Another message.
    let mut message = MSG.to_vec();
    message[0] ^= 0x01;
    assert!(refused(verify_rsassa_pss_sha256(PUB_2048, &message, sig)));
    // A changed signature.
    let mut changed = sig.to_vec();
    changed[100] ^= 0x01;
    assert!(refused(verify_rsassa_pss_sha256(PUB_2048, MSG, &changed)));
    // RFC 8017 §8.1.2: a signature of another length than the modulus.
    assert!(refused(verify_rsassa_pss_sha256(PUB_2048, MSG, &sig[1..])));
    // The signature of the 2048-bit key under the 3072-bit key.
    assert!(refused(verify_rsassa_pss_sha256(PUB_3072, MSG, sig)));
    // No RSAPublicKey.
    assert!(refused(verify_rsassa_pss_sha256(&[0x30, 0x00], MSG, sig)));
}

#[test]
fn security_10_5_2_2_the_suite_of_the_psk_profile_is_aes128_gcm() {
    // §8.1: CID_AES128_GCM = 0x02; Table 77: 16 octets of key.
    assert_eq!(Suite::Aes128Gcm.key_len(), 16);
    assert_eq!(Suite::Aes128Gcm.transform_kind(), [0, 0, 0, 0x02]);
    assert!(Suite::Aes128Gcm.is_aead());
}

#[test]
fn the_psk_derived_plugin_takes_a_non_empty_pre_shared_key() {
    let mut crypto = PskDerivedCryptoPlugin::with_suite(Suite::Aes128Gcm);
    assert_eq!(crypto.suite(), Suite::Aes128Gcm);
    crypto.register_psk(1, vec![0x42; 32]).unwrap();
    // Rustdoc: "`BadArgument` if the key is empty".
    let err = crypto.register_psk(2, Vec::new()).unwrap_err();
    assert_eq!(err.kind, SecurityErrorKind::BadArgument);
}
