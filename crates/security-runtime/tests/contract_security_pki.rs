// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-security-runtime` with `zerodds-security-pki`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-security-pki`, from its non-test
//! sources:
//! - `DelegationChain` with `encode` and `decode` (`caps.rs`,
//!   `caps_wire.rs`: the chain travels base64-encoded in a property),
//!   `DelegationLink::new` and `sign`, `DelegationError`,
//!   `SignatureAlgorithm` (`gateway_bridge.rs`, `peer_class.rs`).
//! - `first_cert_der`, `crls_der`, `subject_attributes` (re-exported by
//!   `lib.rs`), `encrypted_key::{PasswordReading,
//!   decrypt_private_key_pem_as}`, `PERMISSIONS_CREDENTIAL_CERT`,
//!   `IdentityConfig`, `IdentityHandle`, `PkiAuthenticationPlugin`,
//!   `PkiError` (`profile.rs`).
//! - `CertPolicy`, `KxSuite`, `property_enables_deviation`,
//!   `PROP_ACCEPT_X509V1_IDENTITY`, `AlgorithmNames` (`parse`, `V1_2`),
//!   `OcspPolicy::default`, `OcspUnreachable::{Fail, Accept}`
//!   (`qos_properties.rs`), and the re-exports `AlgorithmNames`,
//!   `CertAcceptance`, `CertPolicy`, `KxSuite`,
//!   `LOG_CATEGORY_X509V1_DEVIATION`, `SPEC_CLAUSE_IDENTITY_CERTIFICATE`
//!   of `lib.rs`.
//! - With the feature `pkcs11` only: `cert_key_algo`, `CertKeyAlgo`,
//!   `IdentitySigner` (`pkcs11.rs`, `profile.rs`); not exercised here, the
//!   merge lane builds the default features. The handshake of
//!   `PkiAuthenticationPlugin` is left to the integration pillar.
//!
//! Oracles: RFC 8032 §7.1 TEST 1 (the Ed25519 key pair; Ed25519 signatures
//! are deterministic, §5.1.6) with the signature over the documented
//! layout of a delegation link computed with Python `cryptography` 43.0.3,
//! and the PKCS #8 v2 form of the key (RFC 5958, RFC 8410 §7; `openssl pkey
//! -inform DER` of OpenSSL 4.0.3 reads it and prints the RFC's public key);
//! the wire layout in the rustdoc of `DelegationChain::encode` and
//! `DelegationLink::encode`; RFC 7468 (the DER of a PEM block is the base64
//! of its body); a CRL made with `openssl ca -gencrl` (OpenSSL 4.0.3); the
//! private keys of `crates/security-pki/tests/fixtures/encrypted_keys/`
//! (`openssl ec -passin pass:OpenSesame` and `openssl pkey` give the keys of
//! `ec.pem` and `ec_pkcs8_plain.pem`; `openssl enc -d -aes-128-cbc -K
//! 000102…0f` opens `ec_trad_spec_key.pem` to the key of `ec.pem`) with
//! DDS-Security 1.2 §10.3.1 Table 50 (`dds.sec.auth.password` is the base64
//! of the AES-128 key); §10.4.2.1 Table 64 (`dds.perm.cert`); and the
//! rustdoc of `property_enables_deviation`, `CertPolicy`, `OcspPolicy` and
//! `AlgorithmNames::parse`.
#![allow(clippy::unwrap_used)]

use base64::Engine as _;
use zerodds_security_pki::encrypted_key::{PasswordReading, decrypt_private_key_pem_as};
use zerodds_security_pki::{
    AlgorithmNames, CertPolicy, DelegationChain, DelegationError, DelegationLink, KxSuite,
    OcspPolicy, OcspUnreachable, PERMISSIONS_CREDENTIAL_CERT, PROP_ACCEPT_X509V1_IDENTITY,
    SPEC_CLAUSE_IDENTITY_CERTIFICATE, SignatureAlgorithm, crls_der, first_cert_der,
    private_key_pkcs8_der, property_enables_deviation, subject_attributes,
};

/// RFC 8032 §7.1 TEST 1 as PKCS #8 v2 (RFC 5958 `OneAsymmetricKey` with
/// the public key in `[1]`, RFC 8410 §7): the SECRET KEY, then the PUBLIC
/// KEY.
#[rustfmt::skip]
const RFC8032_TEST1_PKCS8: [u8; 83] = [
    0x30, 0x51, 0x02, 0x01, 0x01, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70,
    0x04, 0x22, 0x04, 0x20,
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
    0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
    0x81, 0x21, 0x00,
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

/// The PUBLIC KEY of RFC 8032 §7.1 TEST 1.
fn rfc8032_public() -> &'static [u8] {
    &RFC8032_TEST1_PKCS8[51..]
}

/// The signature of RFC 8032 TEST 1 over the layout of `link()`, computed
/// with Python `cryptography` 43.0.3.
const LINK_SIGNATURE: [u8; 64] = [
    0x92, 0xae, 0x12, 0xee, 0x74, 0xd5, 0xf2, 0x63, 0xb3, 0x2a, 0x8c, 0x82, 0xed, 0x62, 0x35, 0xdf,
    0xd1, 0x14, 0x95, 0x48, 0xfb, 0xc6, 0xd9, 0xfc, 0x80, 0x37, 0x0e, 0x8c, 0x19, 0xcb, 0x25, 0xeb,
    0xa2, 0x11, 0x82, 0x97, 0x1f, 0xee, 0x8c, 0xf3, 0x57, 0x21, 0xc1, 0x2d, 0x49, 0xef, 0xdf, 0x7c,
    0xca, 0xea, 0xbb, 0xd1, 0x7f, 0x42, 0x54, 0x42, 0xa4, 0xeb, 0xf7, 0xee, 0xe9, 0x19, 0x45, 0x0b,
];

/// The signing input of `link()` after the layout of the rustdoc.
#[rustfmt::skip]
const LINK_LAYOUT: [u8; 87] = [
    b'Z', b'E', b'R', b'O', b'D', b'D', b'S', b'D', 0x01,
    0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
    0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22,
    0x00, 0x00, 0x00, 0x00, 0x65, 0x53, 0xf1, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x6b, 0x49, 0xd2, 0x00,
    0x04,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x08, b's', b'e', b'n', b's', b'o', b'r', b'/', b'*',
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x05, b'f', b'l', b'e', b'e', b't',
];

/// The link `gateway_bridge.rs` issues: from the gateway `0x11..` to the
/// edge `0x22..`.
fn link() -> DelegationLink {
    DelegationLink::new(
        [0x11; 16],
        [0x22; 16],
        vec!["sensor/*".to_string()],
        vec!["fleet".to_string()],
        1_700_000_000,
        1_800_000_000,
        SignatureAlgorithm::Ed25519,
    )
    .unwrap()
}

#[test]
fn rfc_8032_the_gateway_signs_a_link_as_ed25519_prescribes() {
    let mut link = link();
    assert_eq!(link.signing_bytes(), LINK_LAYOUT);
    link.sign(&RFC8032_TEST1_PKCS8).unwrap();
    // §5.1.6: the same key and message give the same signature.
    assert_eq!(link.signature, LINK_SIGNATURE);
    link.verify(rfc8032_public()).unwrap();
    // A key that is no PKCS #8 is refused.
    let mut unsigned = self::link();
    assert!(matches!(
        unsigned.sign(&[0x30, 0x00]),
        Err(DelegationError::SignFailed(_))
    ));
}

#[test]
fn a_delegation_chain_travels_in_the_documented_layout() {
    let mut link = link();
    link.signature = LINK_SIGNATURE.to_vec();
    let chain = DelegationChain::new([0x11; 16], vec![link]).unwrap();

    // version, origin_guid, n_links, then each link: its signing bytes,
    // the u16 big-endian signature length and the signature.
    let mut expected = vec![0x01];
    expected.extend_from_slice(&[0x11; 16]);
    expected.push(0x01);
    expected.extend_from_slice(&LINK_LAYOUT);
    expected.extend_from_slice(&[0x00, 0x40]);
    expected.extend_from_slice(&LINK_SIGNATURE);
    assert_eq!(chain.encode(), expected);
    assert_eq!(DelegationChain::decode(&expected).unwrap(), chain);

    // What `caps_wire.rs` reads back from a peer can be anything.
    let mut other_version = expected.clone();
    other_version[0] = 0x02;
    assert_eq!(
        DelegationChain::decode(&other_version),
        Err(DelegationError::UnsupportedVersion(2))
    );
    let mut too_many = expected.clone();
    too_many[17] = 9;
    assert!(matches!(
        DelegationChain::decode(&too_many),
        Err(DelegationError::TooManyPatterns { kind: "chain", .. })
    ));
    assert!(matches!(
        DelegationChain::decode(&expected[..expected.len() - 1]),
        Err(DelegationError::Malformed(_))
    ));
}

const PERMISSIONS_CA_PEM: &str =
    include_str!("../../security-permissions/tests/fixtures/permissions_ca_cert.pem");

/// The base64 body of the first PEM block of `pem` with `label`.
fn pem_body(pem: &str, label: &str) -> Vec<u8> {
    let begin = format!("-----BEGIN {label}-----");
    let end = format!("-----END {label}-----");
    let start = pem.find(&begin).unwrap() + begin.len();
    let stop = pem[start..].find(&end).unwrap() + start;
    let text: String = pem[start..stop].split_whitespace().collect();
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .unwrap()
}

#[test]
fn rfc_7468_the_certificate_and_the_crls_of_a_pem_file() {
    let der = first_cert_der(PERMISSIONS_CA_PEM.as_bytes()).unwrap();
    assert_eq!(der, pem_body(PERMISSIONS_CA_PEM, "CERTIFICATE"));
    // `lib.rs` re-exports the subject reader the grants are matched with
    // (`openssl x509 -nameopt oneline`: CN, O, C in certificate order).
    let names: Vec<String> = subject_attributes(&der)
        .unwrap()
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(names, ["CN", "O", "C"]);

    // An empty CRL of `openssl ca -gencrl`, twice in one file.
    let crl = "-----BEGIN X509 CRL-----\n\
               MIGoMFACAQEwCgYIKoZIzj0EAwIwETEPMA0GA1UEAwwGY3JsLWNhFw0yNjExMDEx\n\
               NTI5NDFaFw0zNjEwMjkxNTI5NDFaoA4wDDAKBgNVHRQEAwIBATAKBggqhkjOPQQD\n\
               AgNIADBFAiBtWEcnmffzxYdF8Wi4mn4Imcm+RN2StBQbiLaM/xUqkgIhALC1jmr0\n\
               xHD1manHed3h4zN9NArk62mIddMEr8+gAfV7\n\
               -----END X509 CRL-----\n";
    let crl_der = pem_body(crl, "X509 CRL");
    assert_eq!(crl_der.len(), 0xa8 + 3);
    let two = format!("{crl}{crl}");
    assert_eq!(
        crls_der(two.as_bytes()).unwrap(),
        [crl_der.clone(), crl_der.clone()]
    );
    // Rustdoc: one DER list is taken as it is; a PEM text without a CRL is
    // refused.
    assert_eq!(crls_der(&crl_der).unwrap(), core::slice::from_ref(&crl_der));
    assert!(crls_der(PERMISSIONS_CA_PEM.as_bytes()).is_err());
}

macro_rules! key_fixture {
    ($name:literal) => {
        include_bytes!(concat!(
            "../../security-pki/tests/fixtures/encrypted_keys/",
            $name
        ))
    };
}

#[test]
fn security_10_3_1_the_password_of_an_encrypted_identity_key() {
    let ec = private_key_pkcs8_der(key_fixture!("ec.pem")).unwrap();
    // Table 50: the password is the base64 of the AES-128 key; that is the
    // default reading.
    assert_eq!(PasswordReading::default(), PasswordReading::Clause);
    assert_eq!(
        decrypt_private_key_pem_as(
            key_fixture!("ec_trad_spec_key.pem"),
            "AAECAwQFBgcICQoLDA0ODw==",
            PasswordReading::Clause
        )
        .unwrap(),
        ec
    );
    // An OpenSSL passphrase opens a key only with the vendors' reading.
    for (encrypted, plain) in [
        (&key_fixture!("ec_trad_aes128.pem")[..], &ec),
        (
            &key_fixture!("ec_pkcs8_pbes2.pem")[..],
            &private_key_pkcs8_der(key_fixture!("ec_pkcs8_plain.pem")).unwrap(),
        ),
    ] {
        assert!(
            decrypt_private_key_pem_as(encrypted, "OpenSesame", PasswordReading::Clause).is_err()
        );
        assert_eq!(
            &decrypt_private_key_pem_as(
                encrypted,
                "OpenSesame",
                PasswordReading::ClauseThenPassphrase
            )
            .unwrap(),
            plain
        );
        assert!(
            decrypt_private_key_pem_as(encrypted, "wrong", PasswordReading::ClauseThenPassphrase)
                .is_err()
        );
    }
}

#[test]
fn the_settings_the_runtime_reads_from_the_participant_properties() {
    // Table 64.
    assert_eq!(PERMISSIONS_CREDENTIAL_CERT, "dds.perm.cert");
    // The X.509 v1 deviation: a vendor property, off by default, on only
    // for an explicit affirmative.
    assert!(PROP_ACCEPT_X509V1_IDENTITY.starts_with("org.zerodds."));
    assert_eq!(
        SPEC_CLAUSE_IDENTITY_CERTIFICATE,
        "DDS-Security 1.2 §10.3.1.3"
    );
    assert!(!CertPolicy::default().accept_x509v1_identity_certificate);
    for on in ["true", "1", "yes", "on", " TRUE ", "Yes"] {
        assert!(property_enables_deviation(on), "{on:?}");
    }
    for off in ["false", "0", "no", "off", "", "ture", "y"] {
        assert!(!property_enables_deviation(off), "{off:?}");
    }
    // OCSP: an unreachable responder refuses by default.
    assert_eq!(OcspPolicy::default().unreachable, OcspUnreachable::Fail);
    assert_ne!(OcspUnreachable::Accept, OcspUnreachable::Fail);
    // `org.zerodds.sec.algorithm_names`.
    assert_eq!(AlgorithmNames::parse("1.2").unwrap(), AlgorithmNames::V1_2);
    assert!(AlgorithmNames::parse("1.3").is_err());
    // The key agreement suites of `dds.sec.auth.key_establishment_algorithm`
    // (§8.3 Table 26).
    assert_eq!(KxSuite::EcdhP256.crypto_algorithm_id(), Some(0x21));
}
