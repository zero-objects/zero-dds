// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Integration test for the `pkcs11:` identity material
//! (DDS-Security 1.2 §9.3.1 Table 44, RFC 7512) against SoftHSM.
//!
//! `tools/pkcs11/softhsm-fixture.sh` builds a throwaway SoftHSM token
//! with a P-256 identity, its certificate signed by a test CA, the CA
//! certificate and the matching private key, and prints the `pkcs11:`
//! URIs. This test reads the certificate out of the token and signs a
//! handshake-shaped message (§9.3.2.5.2.2 `reply_signing_bytes`) through
//! the token, then verifies the signature with the certificate's public
//! key — the same check a peer runs.
//!
//! The test is skipped, with a printed reason, unless
//! `ZERODDS_TEST_PKCS11_MODULE` names a PKCS#11 module. The reviewer's
//! Linux host has SoftHSM 2.6.1 at
//! `/usr/lib/softhsm/libsofthsm2.so` and OpenSC installed.

#![cfg(feature = "pkcs11")]
#![allow(
    missing_docs,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr
)]

use std::path::PathBuf;
use std::process::Command;

use zerodds_security_pki::{
    IdentitySigner, cert_key_algo, final_signing_bytes, verify_der_signature,
};
use zerodds_security_runtime::{
    Material, Pkcs11ObjectType, Pkcs11Signer, Pkcs11Uri, read_certificate_der,
};

/// The one test: everything SoftHSM is exercised here, so the token is
/// built once.
#[test]
fn a_token_key_signs_a_handshake_message_the_certificate_verifies() {
    let Some(module) = std::env::var_os("ZERODDS_TEST_PKCS11_MODULE") else {
        eprintln!(
            "skipping the pkcs11 SoftHSM test: ZERODDS_TEST_PKCS11_MODULE is not set (the Linux \
             test host has SoftHSM at /usr/lib/softhsm/libsofthsm2.so)"
        );
        return;
    };

    let (ca_uri, cert_uri, key_uri, conf) = run_fixture(&module);

    // SoftHSM reads its token directory from SOFTHSM2_CONF at
    // C_Initialize. The fixture set it for its own child processes; this
    // process has to set it before the first cryptoki call.
    //
    // SAFETY: this test binary contains a single test, so no other thread
    // reads the environment while it is changed.
    unsafe {
        std::env::set_var("SOFTHSM2_CONF", &conf);
    }

    // The certificate is read from the token's CKA_VALUE (DER), not from
    // a file.
    let cert_der = Material::Pkcs11(cert_uri)
        .read()
        .expect("read identity cert from token");
    let ca_der = read_certificate_der(&ca_uri).expect("read identity CA from token");
    assert!(!cert_der.is_empty(), "the token certificate is empty");
    assert!(!ca_der.is_empty(), "the token CA certificate is empty");
    assert_eq!(
        cert_key_algo(&cert_der),
        zerodds_security_pki::CertKeyAlgo::EcdsaP256Sha256,
        "the fixture identity is a P-256 key",
    );

    // The private key stays on the token; the signer holds the session.
    let signer =
        Pkcs11Signer::open(&key_uri, cert_key_algo(&cert_der)).expect("open the token signer");
    let message = final_signing_bytes(
        &[0x11; 32],
        &[0x22; 32],
        &[0x33; 32],
        &[0x44; 32],
        &[0x55; 32],
        &[0x66; 32],
    );
    let signature = signer.sign(&message).expect("the token signs");

    // The peer's check: the certificate's public key verifies the
    // signature over the handshake bytes.
    verify_der_signature(&cert_der, &message, &signature)
        .expect("the certificate public key verifies the token signature");

    let mut tampered = message.clone();
    tampered[0] ^= 0xff;
    assert!(
        verify_der_signature(&cert_der, &tampered, &signature).is_err(),
        "a signature must not verify a changed message",
    );
}

/// Runs the fixture and returns the three URIs and the SoftHSM config
/// path it printed.
fn run_fixture(module: &std::ffi::OsStr) -> (Pkcs11Uri, Pkcs11Uri, Pkcs11Uri, String) {
    let script =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/pkcs11/softhsm-fixture.sh");
    let output = Command::new("bash")
        .arg(&script)
        .env("PKCS11_MODULE", module)
        .output()
        .expect("run tools/pkcs11/softhsm-fixture.sh");
    assert!(
        output.status.success(),
        "softhsm-fixture.sh failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8(output.stdout).expect("fixture stdout is UTF-8");

    let mut ca_uri = None;
    let mut cert_uri = None;
    let mut key_uri = None;
    let mut conf = None;
    for line in stdout.lines() {
        if line.starts_with("pkcs11:") {
            let uri = zerodds_security_runtime::pkcs11::parse(line).expect("fixture URI parses");
            match (uri.object.as_deref(), uri.object_type) {
                (Some("identity_ca"), _) => ca_uri = Some(uri),
                (Some("identity"), Some(Pkcs11ObjectType::Cert)) => cert_uri = Some(uri),
                (Some("identity"), Some(Pkcs11ObjectType::Private)) => key_uri = Some(uri),
                (object, kind) => panic!("unexpected fixture object {object:?} of type {kind:?}"),
            }
        } else if let Some(value) = line.strip_prefix("SOFTHSM2_CONF=") {
            conf = Some(value.to_string());
        }
    }
    (
        ca_uri.expect("the fixture prints the identity_ca URI"),
        cert_uri.expect("the fixture prints the identity certificate URI"),
        key_uri.expect("the fixture prints the private key URI"),
        conf.expect("the fixture prints SOFTHSM2_CONF"),
    )
}
