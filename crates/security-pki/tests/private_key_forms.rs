// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! The three private-key forms a `dds.sec.auth.private_key` file can
//! carry all reach the signing backend as PKCS#8.
//!
//! Every vendor key of the OMG `dds-security` interoperability suite is
//! PKCS#1 (`BEGIN RSA PRIVATE KEY`), and a key made with
//! `openssl ecparam -genkey` is SEC1 (`BEGIN EC PRIVATE KEY`). Measured
//! before this existed: the suite's own reference client for this
//! project refused both with "no certificate in PEM" and no participant
//! came up.
//!
//! The fixtures are throwaway keys generated for this test with
//! `openssl genrsa -traditional 2048` and `openssl ecparam -name
//! prime256v1 -genkey`; the PKCS#8 twin of the EC key is
//! `openssl pkcs8 -topk8 -nocrypt` over the SEC1 file.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_security_pki::{PkiError, private_key_pkcs8_der};

const RSA_PKCS1: &str = r#"-----BEGIN RSA PRIVATE KEY-----
MIIEogIBAAKCAQEA3RRYBdQMYyXuMHM9PoQAbtxuMyzOqeeJOYgVsI5DE/FRb0L6
Oj2A+SMSuFb4Mh3dpl+NZF6HU6Cor4j5qn3UkFYbBMfdL3pHOh5NQTha8EARFmHr
yL+9b7DrFl3J2ZCzd4BLxemqDGAUkWH3cIOgi0/SMnv+81v5IGp50AhSRy26clfJ
3YPd2GciWMcL7jUlQARqt9sl1CqBBiAvTPeOeE0WZxVIQol3ak8CULKKMtG+4jD6
Ga8jG9z59s+bbcZ9IFW9Tc95htlN1bLuthypu7pSZOYVTOtQA+tU0sXpfHTB+1Q4
mezrBvzoEXprmHhOqsEB3ultoO33Srmv64LfpQIDAQABAoIBACO0HWMTk8yPW6BH
fCSUfN1qjAGHwvh9hBPefmI9F2gpGNW3I2I0fJe/qk5K/VpaqGkuFU+YVJKU56kx
neCC4clNnp9vB7GbNdWU/zUBPO5/6kKLHeEV1iorkEvUabzROaqa9aHI0sWc3rV7
9NIe79aP1YqBq/Lxb5Jkn7eRIHiOBSFBfPxCbScU+jSpeCRtd6vAN0wPQkpIjkZq
eh1zsx7m7sbgWPTuyh7WeVlW7yBCMJdh3YI69d5iAhmNnA/XfEWGwg7Q6lET6/P/
Athcp9WX4tekn5URTW13IldWk1iFZF2JFogJXlu2JDJCNBgQlf0DSzaGAYIAi1ae
oaBBWiECgYEA9vlJxZEjhBEj90QPNZUo8HR/fNFBLNJQPgs9kNcdWP1xdtKr31+F
oU8lRYL9EtsQNE8c1uo1OloHaMjVTUYigJGSf+y8EsopPlAf58sljCBOhGev+MxD
sSgxkmhtVh4PqIauCsvn4+d22jzToGbctGq2ooiTyYFnautKwFe80B8CgYEA5SjJ
LGhBKcEE1IwEu+OmtpqRC8oxIsQOr8BuHo8PaE6wuPsuZXg1lrIUD6A6xrJ5u33t
g0F11+Pkpf3NqiD5zQdu4kyvuLuRwMT4cufhoegNwNevdE98BdR0r0vAOf5muQWk
Vsyq5VTSGgLvyG4PXMZ94t9/DJzdG2Okh4BaB7sCgYBVhIqD67bVky3V/+4YSrbr
VBuU98xOneCFK1U9Iw+ZceDpGRfEeAoIiiRIxoulDM+2nTJKRLz7oJLyOKCLal6m
EEIfI7UX+IKBfjMeCxzMQZ+Of2zShMBnxpgrrxc0+6HGZCQ9vs9j/hEMjoVlBFkl
69GjxBEQdEuAHUs8AOVzVwKBgEFnl6tuAhM5vPgxEnTR79Y0XWuhBM8oaAyvfn6I
VFxwAS56um5Jlfnxz5n6IGxvqiDiOzT4LPs2Vx8tRlLlYIatNW+vdYPjPSFo1oRt
PE/qnGpK5Q/sU7iSP898blvfCVCLlLEVbz1PLTwxVGC/UPEo0es74NYnu0/C68Wk
041bAoGAeRoBVH56+lbzYfDegMj3FSOpCvHTxlelX8hEXPE4S0Q0XRFW9HUJLYJF
DstAEckYi9Z0KVj9dDeWIHFMNjhwenq+l7+xcJcgWTZ2zqBQNSeguu18WuEKMKMo
UEcVJ+daCDZx/OijGGAHVsOEb7Rd76WcwaZZ8OyMYMHk8MVxuKQ=
-----END RSA PRIVATE KEY-----
"#;

const EC_SEC1: &str = r#"-----BEGIN EC PRIVATE KEY-----
MHcCAQEEIC6BTcPkZsUIoynVJbr6oDeXVvbTca5OeinGkBaXJiRBoAoGCCqGSM49
AwEHoUQDQgAEFOChvqMOdre4m7elaRKzVPdF/KzWkCV3XVTzFcJtZZDZthOgg3CP
LpIL15obZX0wVGN4SLDCNEw2v3kzXWauzA==
-----END EC PRIVATE KEY-----
"#;

const EC_PKCS8: &str = r#"-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgLoFNw+RmxQijKdUl
uvqgN5dW9tNxrk56KcaQFpcmJEGhRANCAAQU4KG+ow52t7ibt6VpErNU90X8rNaQ
JXddVPMVwm1lkNm2E6CDcI8ukgvXmhtlfTBUY3hIsMI0TDa/eTNdZq7M
-----END PRIVATE KEY-----
"#;

fn pkcs8_parses(der: &[u8]) -> bool {
    ring::signature::RsaKeyPair::from_pkcs8(der).is_ok()
        || ring::signature::EcdsaKeyPair::from_pkcs8(
            &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
            der,
            &ring::rand::SystemRandom::new(),
        )
        .is_ok()
}

#[test]
fn a_pkcs1_rsa_key_reaches_the_backend_as_pkcs8() {
    let der = private_key_pkcs8_der(RSA_PKCS1.as_bytes()).expect("PKCS#1 wraps");
    assert!(
        ring::signature::RsaKeyPair::from_pkcs8(&der).is_ok(),
        "the wrapped RSA key is not a PKCS#8 key the backend takes"
    );
}

#[test]
fn a_sec1_ec_key_reaches_the_backend_as_the_same_pkcs8_openssl_writes() {
    let from_sec1 = private_key_pkcs8_der(EC_SEC1.as_bytes()).expect("SEC1 wraps");
    let from_pkcs8 = private_key_pkcs8_der(EC_PKCS8.as_bytes()).expect("PKCS#8 passes through");
    assert!(
        pkcs8_parses(&from_sec1),
        "the wrapped EC key is not a PKCS#8 key the backend takes"
    );
    assert!(pkcs8_parses(&from_pkcs8));
    // Same key, same curve: the backend derives the same public key.
    let a = ring::signature::EcdsaKeyPair::from_pkcs8(
        &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
        &from_sec1,
        &ring::rand::SystemRandom::new(),
    )
    .expect("sec1-derived");
    let b = ring::signature::EcdsaKeyPair::from_pkcs8(
        &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
        &from_pkcs8,
        &ring::rand::SystemRandom::new(),
    )
    .expect("pkcs8-derived");
    use ring::signature::KeyPair as _;
    assert_eq!(a.public_key().as_ref(), b.public_key().as_ref());
}

#[test]
fn a_file_without_a_key_block_says_so() {
    let err =
        private_key_pkcs8_der(b"-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n")
            .expect_err("no key");
    assert!(matches!(err, PkiError::NoPrivateKeyInPem), "{err}");
}
