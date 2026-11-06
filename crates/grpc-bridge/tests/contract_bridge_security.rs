// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-grpc-bridge` with `zerodds-bridge-security`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-bridge-security`: `authenticate`, `LiveSecurity`, `RotatingTlsConfig`, `serve_tls_handshake` and `tls_provider` (`src/bin/zerodds-grpc-bridged.rs`, `src/bridge_security.rs`).
//!
//! Oracles: RFC 6750 §2.1 (`Authorization: Bearer <token>`) with RFC 7235 §2.1 (the scheme is case-insensitive); the bridge specification §7.3 (read and write per topic, deny by default); RFC 8446 §9.1 (TLS_AES_128_GCM_SHA256 must be implemented); RFC 7468 §5, with a certificate and key made by `openssl req`.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::HashMap;
use std::path::PathBuf;

use zerodds_bridge_security::{
    Acl, AclEntry, AclOp, AuthError, AuthMode, AuthSubject, RotatingTlsConfig, TlsConfigError,
    authenticate, load_server_config, rustls, tls_provider,
};

/// `openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -subj
/// "/O=Example/CN=alice"` (OpenSSL 4.0.3).
const CERT_PEM: &str = "-----BEGIN CERTIFICATE-----\nMIIBeTCCASCgAwIBAgIUNYvSwdb4nEI1QfcU6A3VHMe8RIQwCgYIKoZIzj0EAwIw\nIjEQMA4GA1UECgwHRXhhbXBsZTEOMAwGA1UEAwwFYWxpY2UwIBcNMjYxMTAxMjMx\nNzQxWhgPMjEyNjEwMDgyMzE3NDFaMCIxEDAOBgNVBAoMB0V4YW1wbGUxDjAMBgNV\nBAMMBWFsaWNlMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEXjZywaxMDiJUet1T\n1J4T3v2SVAyDDMEna5jzLGm96fNXCSH3lrCHtDTSKDrKZfDOOVktt9WV5VLQjMnw\nuTDqlKMyMDAwHQYDVR0OBBYEFHlA+hOEKTwWrLHBF/JFiCzLdVGcMA8GA1UdEwEB\n/wQFMAMBAf8wCgYIKoZIzj0EAwIDRwAwRAIgBJDU1K8HqMWikQpg39J+5+yeo1DN\n2+5ADkJJvNABPKgCIERq4rHsCOs0FM9GaH9NUc8tob9jmSbp1qce2uqsJWhv\n-----END CERTIFICATE-----\n";

/// The key of [`CERT_PEM`].
const KEY_PEM: &str = "-----BEGIN PRIVATE KEY-----\nMIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQg7FPbKMz31lhIvf+O\nbfBzKB4DqZAF63PYgc8VbqS1FamhRANCAAReNnLBrEwOIlR63VPUnhPe/ZJUDIMM\nwSdrmPMsab3p81cJIfeWsIe0NNIoOspl8M45WS231ZXlUtCMyfC5MOqU\n-----END PRIVATE KEY-----\n";

#[test]
fn grpcbridge_7_2_a_bearer_token_names_its_subject() {
    let mut tokens = HashMap::new();
    tokens.insert("tk-1".to_string(), AuthSubject::new("alice"));
    let mode = AuthMode::Bearer { tokens };
    assert_eq!(
        authenticate(&mode, Some("Bearer tk-1"), None, None)
            .unwrap()
            .name,
        "alice"
    );
    assert_eq!(
        authenticate(&mode, Some("bearer tk-1"), None, None)
            .unwrap()
            .name,
        "alice"
    );
    assert!(matches!(
        authenticate(&mode, None, None, None),
        Err(AuthError::MissingCredentials)
    ));
    assert!(matches!(
        authenticate(&mode, Some("Basic dGstMQ=="), None, None),
        Err(AuthError::MalformedCredentials(_))
    ));
    assert!(matches!(
        authenticate(&mode, Some("Bearer tk-2"), None, None),
        Err(AuthError::Rejected(_))
    ));
}

#[test]
fn grpcbridge_7_3_the_acl_grants_read_and_write_per_topic_and_denies_the_rest() {
    let mut acl = Acl::deny_all();
    acl.set(
        "Trade",
        AclEntry {
            read: vec!["alice".to_string()],
            write: vec!["bob".to_string()],
        },
    );
    let (alice, bob) = (AuthSubject::new("alice"), AuthSubject::new("bob"));
    assert!(acl.check(&alice, AclOp::Read, "Trade"));
    assert!(!acl.check(&alice, AclOp::Write, "Trade"));
    assert!(acl.check(&bob, AclOp::Write, "Trade"));
    assert!(!acl.check(&bob, AclOp::Read, "Trade"));
    assert!(
        !acl.check(&alice, AclOp::Read, "Quote"),
        "a topic without an entry"
    );
    assert!(Acl::allow_all().check(&AuthSubject::anonymous(), AclOp::Write, "Quote"));
}

#[test]
fn grpcbridge_7_1_the_tls_provider_implements_tls_aes_128_gcm_sha256() {
    let provider = tls_provider();
    assert!(
        provider
            .cipher_suites
            .iter()
            .any(|s| s.suite() == rustls::CipherSuite::TLS13_AES_128_GCM_SHA256)
    );
}

/// A directory of this test with the fixture certificate and key.
fn pem_files(name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "zerodds-contract-bridge-security-{}-{name}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let (cert, key) = (dir.join("cert.pem"), dir.join("key.pem"));
    std::fs::write(&cert, CERT_PEM).unwrap();
    std::fs::write(&key, KEY_PEM).unwrap();
    (dir, cert, key)
}

#[test]
fn grpcbridge_7_1_a_server_config_loads_from_a_certificate_and_its_key() {
    let (dir, cert, key) = pem_files("rotating");
    let loaded = RotatingTlsConfig::load(cert.clone(), key.clone(), None).map(|r| r.current());
    assert!(loaded.is_ok(), "{:?}", loaded.err());
    // A file without a `CERTIFICATE` block is no certificate.
    let not_a_cert = dir.join("not-a-cert.pem");
    std::fs::write(&not_a_cert, KEY_PEM).unwrap();
    assert!(matches!(
        load_server_config(&not_a_cert, &key),
        Err(TlsConfigError::NoCertificateInPem)
    ));
    assert!(matches!(
        load_server_config(&dir.join("missing.pem"), &key),
        Err(TlsConfigError::CertFileRead(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}
