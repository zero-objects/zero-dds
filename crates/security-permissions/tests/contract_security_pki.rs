// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-security-permissions` with `zerodds-security-pki`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-security-pki`, from its non-test
//! sources:
//! - `DelegationChain` and `SignatureAlgorithm` (`delegation_check.rs`,
//!   `governance.rs`): `validate_chain` reads `origin_guid`, `links`,
//!   `depth()` and `edge_guid()`, and of each `DelegationLink`
//!   `delegator_guid`, `delegatee_guid`, `not_before`, `not_after`,
//!   `algorithm`, the pattern lists and `verify(public_key)`; the profile
//!   keeps the algorithms by `SignatureAlgorithm::wire_id`, and the
//!   governance parses the four variants by name.
//! - `first_cert_der` and `subject::subject_attributes` (`plugin.rs`,
//!   `certificate_subject`): the subject of a `c.id` certificate as
//!   `(attribute, value)` pairs for the grants.
//! - `certificate_algorithm_name(oid, legacy)` and
//!   `format_subject_name(name, form)` (`plugin.rs`, `describe_ca`): the
//!   `dds.perm_ca.algo` and `dds.perm_ca.sn` of the PermissionsToken.
//! - `AlgorithmNames` (`default()`, `V1_1`; `plugin.rs`).
//! - `PskHandshakeAuthenticationPlugin::new`, `register_psk` and
//!   `validate_local_psk_identity` (`psk_access.rs`, `PskProfile`).
//!
//! Oracles: the subject of `tests/fixtures/permissions_ca_cert.pem` as
//! OpenSSL 4.0.3 prints it (`openssl x509 -noout -subject` with `-nameopt
//! RFC2253`, `-nameopt compat` and `-nameopt oneline`), RFC 4514; the
//! signature algorithm OIDs of RFC 5758 §3.2 and RFC 4055 §5 with the
//! names of DDS-Security 1.2 §8.2 Table 25 (and §10.4.2.2 Table 65, which
//! names Table 25 for `dds.perm_ca.algo`); RFC 8032 §7.1 TEST 1 (the
//! Ed25519 key pair) with a signature over the documented layout of a
//! delegation link computed outside this tree (Python `cryptography`
//! 43.0.3 on OpenSSL 3.3.2); the layout in the module doc of `delegation.rs` and
//! `docs/architecture/09_delegation.md` §5; RFC 8032 §5.1.6 (64-octet
//! signatures), SEC 1 §4.1.3 with the fixed `r || s` encoding (64 and 96
//! octets), RFC 8017 (256 octets for 2048 bits); and the rustdoc of
//! `DelegationLink::new`, `DelegationChain::new`, `first_cert_der`,
//! `AlgorithmNames::parse` and `PskHandshakeAuthenticationPlugin`.
#![allow(clippy::unwrap_used)]

use x509_cert::Certificate;
use x509_cert::der::Decode;
use zerodds_security::SubjectNameForm;
use zerodds_security::authentication::IdentityHandle;
use zerodds_security::error::SecurityErrorKind;
use zerodds_security_pki::{
    AlgorithmNames, DelegationChain, DelegationError, DelegationLink, MAX_CHAIN_DEPTH_HARD_CAP,
    PkiError, PskHandshakeAuthenticationPlugin, SignatureAlgorithm, certificate_algorithm_name,
    first_cert_der, format_subject_name, subject::subject_attributes,
};

const PERMISSIONS_CA_PEM: &[u8] = include_bytes!("fixtures/permissions_ca_cert.pem");

/// RFC 8032 §7.1 TEST 1, PUBLIC KEY.
const RFC8032_TEST1_PUBLIC: [u8; 32] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

/// Ed25519 signature with the SECRET KEY of RFC 8032 §7.1 TEST 1 over
/// `LINK_LAYOUT`, computed with Python `cryptography` 43.0.3 (OpenSSL 3.3.2).
const LINK_SIGNATURE: [u8; 64] = [
    0x92, 0xae, 0x12, 0xee, 0x74, 0xd5, 0xf2, 0x63, 0xb3, 0x2a, 0x8c, 0x82, 0xed, 0x62, 0x35, 0xdf,
    0xd1, 0x14, 0x95, 0x48, 0xfb, 0xc6, 0xd9, 0xfc, 0x80, 0x37, 0x0e, 0x8c, 0x19, 0xcb, 0x25, 0xeb,
    0xa2, 0x11, 0x82, 0x97, 0x1f, 0xee, 0x8c, 0xf3, 0x57, 0x21, 0xc1, 0x2d, 0x49, 0xef, 0xdf, 0x7c,
    0xca, 0xea, 0xbb, 0xd1, 0x7f, 0x42, 0x54, 0x42, 0xa4, 0xeb, 0xf7, 0xee, 0xe9, 0x19, 0x45, 0x0b,
];

/// The signing input of the link of `link()`, written from the layout in
/// the module doc of `delegation.rs`.
#[rustfmt::skip]
const LINK_LAYOUT: [u8; 87] = [
    b'Z', b'E', b'R', b'O', b'D', b'D', b'S', b'D', // magic
    0x01,                                           // version
    0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, // delegator
    0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
    0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, // delegatee
    0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22,
    0x00, 0x00, 0x00, 0x00, 0x65, 0x53, 0xf1, 0x00, // not_before 1_700_000_000, i64 BE
    0x00, 0x00, 0x00, 0x00, 0x6b, 0x49, 0xd2, 0x00, // not_after 1_800_000_000, i64 BE
    0x04,                                           // algorithm: Ed25519
    0x00, 0x00, 0x00, 0x01,                         // one topic pattern
    0x00, 0x00, 0x00, 0x08, b's', b'e', b'n', b's', b'o', b'r', b'/', b'*',
    0x00, 0x00, 0x00, 0x01,                         // one partition pattern
    0x00, 0x00, 0x00, 0x05, b'f', b'l', b'e', b'e', b't',
];

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
fn delegation_algorithms_keep_their_wire_ids_and_signature_lengths() {
    // `delegation_check.rs` keeps the allowed algorithms by wire id.
    let all = [
        (SignatureAlgorithm::EcdsaP256, 1, 64),
        (SignatureAlgorithm::EcdsaP384, 2, 96),
        (SignatureAlgorithm::RsaPss2048, 3, 256),
        (SignatureAlgorithm::Ed25519, 4, 64),
    ];
    for (algorithm, id, len) in all {
        assert_eq!(algorithm.wire_id(), id);
        assert_eq!(SignatureAlgorithm::from_wire_id(id), Some(algorithm));
        assert_eq!(algorithm.expected_signature_len(), Some(len));
    }
    assert_eq!(SignatureAlgorithm::from_wire_id(0), None);
    assert_eq!(SignatureAlgorithm::from_wire_id(5), None);
}

#[test]
fn a_delegation_link_signs_the_documented_layout() {
    let link = link();
    assert_eq!(link.signing_bytes(), LINK_LAYOUT);
    // The fields `validate_chain` reads.
    assert_eq!(
        (link.delegator_guid, link.delegatee_guid),
        ([0x11; 16], [0x22; 16])
    );
    assert_eq!(
        (link.not_before, link.not_after),
        (1_700_000_000, 1_800_000_000)
    );
    assert_eq!(link.allowed_topic_patterns, ["sensor/*"]);
    assert_eq!(link.allowed_partition_patterns, ["fleet"]);
    assert!(link.signature.is_empty());
}

#[test]
fn rfc_8032_an_ed25519_link_signed_outside_this_tree_verifies() {
    let mut link = link();
    link.signature = LINK_SIGNATURE.to_vec();
    // Rustdoc of `verify`: Ed25519 takes the 32-octet raw public key.
    link.verify(&RFC8032_TEST1_PUBLIC).unwrap();

    // Another time window is another statement.
    let mut later = link.clone();
    later.not_after += 1;
    assert!(matches!(
        later.verify(&RFC8032_TEST1_PUBLIC),
        Err(DelegationError::VerifyFailed(_))
    ));
    // Another key.
    let mut other_key = RFC8032_TEST1_PUBLIC;
    other_key[0] ^= 0x01;
    assert!(link.verify(&other_key).is_err());
    // A signature of the wrong length, and none.
    let mut short = link.clone();
    short.signature.pop();
    assert!(matches!(
        short.verify(&RFC8032_TEST1_PUBLIC),
        Err(DelegationError::VerifyFailed(_))
    ));
    let mut unsigned = link.clone();
    unsigned.signature.clear();
    assert!(matches!(
        unsigned.verify(&RFC8032_TEST1_PUBLIC),
        Err(DelegationError::SignFailed(_))
    ));
}

#[test]
fn a_delegation_chain_names_origin_edge_and_depth() {
    let first = link();
    let mut second = link();
    second.delegator_guid = [0x22; 16];
    second.delegatee_guid = [0x33; 16];
    let chain = DelegationChain::new([0x11; 16], vec![first, second]).unwrap();
    assert_eq!(chain.origin_guid, [0x11; 16]);
    assert_eq!(chain.depth(), 2);
    assert_eq!(chain.edge_guid(), Some([0x33; 16]));
    assert_eq!(chain.links[0].delegatee_guid, chain.links[1].delegator_guid);
    assert_eq!(
        DelegationChain::new([0; 16], Vec::new())
            .unwrap()
            .edge_guid(),
        None
    );

    // Rustdoc of `DelegationChain::new`: more links than the hard cap are
    // refused with the kind "chain".
    let too_deep = vec![link(); MAX_CHAIN_DEPTH_HARD_CAP + 1];
    assert!(matches!(
        DelegationChain::new([0x11; 16], too_deep),
        Err(DelegationError::TooManyPatterns { kind: "chain", .. })
    ));
    // Rustdoc of `DelegationLink::new`: not_before after not_after.
    assert_eq!(
        DelegationLink::new(
            [1; 16],
            [2; 16],
            Vec::new(),
            Vec::new(),
            2,
            1,
            SignatureAlgorithm::EcdsaP256
        ),
        Err(DelegationError::InvalidTimeWindow)
    );
}

#[test]
fn the_subject_of_a_certificate_is_read_in_certificate_order() {
    // `openssl x509 -noout -subject -nameopt oneline` of the fixture:
    // "CN = ZeroDDS Permissions CA, O = ZeroDDS, C = DE".
    let der = first_cert_der(PERMISSIONS_CA_PEM).unwrap();
    assert_eq!(
        subject_attributes(&der).unwrap(),
        [
            ("CN".to_string(), "ZeroDDS Permissions CA".to_string()),
            ("O".to_string(), "ZeroDDS".to_string()),
            ("C".to_string(), "DE".to_string()),
        ]
    );
    // Rustdoc of `first_cert_der`: no certificate, `NoCertInPem`.
    assert!(matches!(
        first_cert_der(b"no certificate here"),
        Err(PkiError::NoCertInPem)
    ));
    assert!(subject_attributes(b"\x30\x00").is_err());
}

#[test]
fn the_three_forms_of_the_subject_name_are_those_openssl_prints() {
    let der = first_cert_der(PERMISSIONS_CA_PEM).unwrap();
    let cert = Certificate::from_der(&der).unwrap();
    let subject = &cert.tbs_certificate.subject;
    // `-nameopt RFC2253` (RFC 4514 for these values).
    assert_eq!(
        format_subject_name(subject, SubjectNameForm::Rfc4514),
        "C=DE,O=ZeroDDS,CN=ZeroDDS Permissions CA"
    );
    // `-nameopt compat`: `X509_NAME_oneline`.
    assert_eq!(
        format_subject_name(subject, SubjectNameForm::X509NameOneline),
        "/CN=ZeroDDS Permissions CA/O=ZeroDDS/C=DE"
    );
    // `-nameopt oneline`: `XN_FLAG_ONELINE`.
    assert_eq!(
        format_subject_name(subject, SubjectNameForm::XnFlagOneline),
        "CN = ZeroDDS Permissions CA, O = ZeroDDS, C = DE"
    );
}

#[test]
fn security_8_2_the_table_25_names_of_certificate_signature_algorithms() {
    // RFC 5758 §3.2 and RFC 4055 §5 give the OIDs; Table 25 the names.
    let names = [
        ("1.2.840.10045.4.3.2", "ECDSA+P256+SHA256"),
        ("1.2.840.10045.4.3.3", "ECDSA+P384+SHA384"),
        ("1.2.840.113549.1.1.11", "RSASSA-PKCS1-V1_5+2048+SHA256"),
        ("1.2.840.113549.1.1.10", "RSASSA-PSS-MGF1SHA256+2048+SHA256"),
    ];
    for (oid, name) in names {
        assert_eq!(certificate_algorithm_name(oid, false), name, "{oid}");
    }
    // ecdsa-with-SHA512 has no name in Table 25: the rustdoc gives the
    // empty string, and `describe_ca` leaves both properties out.
    assert_eq!(certificate_algorithm_name("1.2.840.10045.4.3.4", false), "");
    // With `legacy` the names of DDS-Security 1.1 (rustdoc).
    assert_eq!(
        certificate_algorithm_name("1.2.840.10045.4.3.2", true),
        "EC-prime256v1"
    );
    assert_eq!(
        certificate_algorithm_name("1.2.840.113549.1.1.11", true),
        "RSA-2048"
    );

    // The Permissions CA of the fixture is signed with ecdsa-with-SHA256
    // (`openssl x509 -text`), so its token names ECDSA+P256+SHA256.
    let der = first_cert_der(PERMISSIONS_CA_PEM).unwrap();
    let cert = Certificate::from_der(&der).unwrap();
    let oid = cert.signature_algorithm.oid.to_string();
    assert_eq!(certificate_algorithm_name(&oid, false), "ECDSA+P256+SHA256");
}

#[test]
fn the_algorithm_names_setting() {
    // Rustdoc: the names of Tables 25 and 26 are the default; `1.1`,
    // `1.2` and `auto` are the settings, anything else is refused.
    assert_eq!(AlgorithmNames::default(), AlgorithmNames::V1_2);
    assert_eq!(AlgorithmNames::parse("1.1").unwrap(), AlgorithmNames::V1_1);
    assert_eq!(AlgorithmNames::parse("1.2").unwrap(), AlgorithmNames::V1_2);
    assert_eq!(AlgorithmNames::parse("auto").unwrap(), AlgorithmNames::Auto);
    assert_eq!(
        AlgorithmNames::parse("2.0").unwrap_err().kind,
        SecurityErrorKind::InvalidConfiguration
    );
}

#[test]
fn the_psk_handshake_plugin_of_the_psk_profile() {
    // `PskProfile::with_pre_shared_key` registers the key and validates
    // the local identity.
    let mut auth = PskHandshakeAuthenticationPlugin::new();
    auth.register_psk("alice".to_string(), vec![0x42; 32])
        .unwrap();
    let handle = auth.validate_local_psk_identity("alice").unwrap();
    assert_ne!(handle, IdentityHandle::NIL);
    // Rustdoc: `BadArgument` for an empty id or key and an unknown id.
    for err in [
        auth.register_psk(String::new(), vec![1]).unwrap_err(),
        auth.register_psk("bob".to_string(), Vec::new())
            .unwrap_err(),
        auth.validate_local_psk_identity("carol").unwrap_err(),
    ] {
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }
}
