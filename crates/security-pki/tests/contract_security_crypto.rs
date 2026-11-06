// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-security-pki` with `zerodds-security-crypto`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-security-crypto`, from its non-test
//! sources: `rsa_pss::verify_rsassa_pss_sha256(public_key, message,
//! signature)` alone (`plugin.rs`, `verify_rsa_pss_of_any_salt`). The key
//! it passes is the `subjectPublicKey` bit string of the peer's
//! certificate (the PKCS #1 `RSAPublicKey`), read with `x509-cert`; an
//! `Err` becomes `AuthenticationFailed`, the salt length is dropped.
//!
//! What it relies on: the handshake signature of a peer that signs with
//! RSASSA-PSS verifies whatever salt length the peer chose (DDS-Security 1.2
//! §8.2 Table 25: "The validation of the signature shall detect the salt
//! length from the signature"), and a signature over other content or with
//! another key does not.
//!
//! Oracles: the signatures OpenSSL 4.0.3 made over `msg.bin` with salt
//! lengths 0, 8, 20, 32, 64 and 222, in
//! `crates/security-crypto/tests/fixtures/rsa_pss/` (`README.txt` gives the
//! commands; `openssl pkeyutl -verify` with `rsa_pss_saltlen:N` accepts each
//! one), and `cert2048.der`, the self-signed certificate of that key, from
//! which the key is taken as this crate takes it (RFC 5280 §4.1.2.7,
//! RFC 8017 A.1.1).
#![allow(clippy::unwrap_used)]

use x509_cert::Certificate;
use x509_cert::der::Decode;
use zerodds_security::error::SecurityErrorKind;
use zerodds_security_crypto::rsa_pss::verify_rsassa_pss_sha256;

const CERT_2048: &[u8] =
    include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/cert2048.der");
const PUB_2048: &[u8] = include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/pub2048.der");
const MSG: &[u8] = include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/msg.bin");

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

/// The `subjectPublicKey` of the certificate, as `plugin.rs` reads it.
fn certificate_key() -> Vec<u8> {
    let cert = Certificate::from_der(CERT_2048).unwrap();
    cert.tbs_certificate
        .subject_public_key_info
        .subject_public_key
        .raw_bytes()
        .to_vec()
}

#[test]
fn security_8_2_the_key_of_the_certificate_verifies_every_salt_length() {
    let key = certificate_key();
    // The bit string of an RSA certificate is the PKCS #1 RSAPublicKey,
    // the same octets as `pub2048.der`.
    assert_eq!(key, PUB_2048);
    for (sig, salt) in SIG_2048 {
        assert_eq!(verify_rsassa_pss_sha256(&key, MSG, sig).unwrap(), salt);
    }
}

#[test]
fn security_8_2_another_message_or_signature_is_refused() {
    let key = certificate_key();
    let (sig, _) = SIG_2048[2];
    let mut message = MSG.to_vec();
    message.push(0x00);
    let mut changed = sig.to_vec();
    changed[255] ^= 0x80;
    for (m, s) in [(&message[..], sig), (MSG, &changed[..]), (MSG, &sig[..128])] {
        assert_eq!(
            verify_rsassa_pss_sha256(&key, m, s).unwrap_err().kind,
            SecurityErrorKind::CryptoFailed
        );
    }
    // The whole SubjectPublicKeyInfo is not the key the function takes.
    let cert = Certificate::from_der(CERT_2048).unwrap();
    let spki =
        x509_cert::der::Encode::to_der(&cert.tbs_certificate.subject_public_key_info).unwrap();
    assert!(verify_rsassa_pss_sha256(&spki, MSG, sig).is_err());
}
