// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-coap-bridge` with `zerodds-bridge-security`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-bridge-security`: `build_ctx`, `authenticate`, `LiveSecurity` and `mtls_subject_from_der` (`src/daemon/security.rs`, the DTLS client certificate).
//!
//! Oracles: the bridge specification §7.2 and §7.3 (the auth modes; without an ACL, `none` allows everything and the other modes deny); RFC 6750 §2.1 (`Authorization: Bearer <token>`) with RFC 7235 §2.1 (the scheme is case-insensitive); the bridge specification §7.3 (read and write per topic, deny by default); RFC 4514 §2 (the subject DN, RDNs in reverse order of the encoding) and FIPS 180-4 SHA-256 of the DER, both as `openssl x509 -subject -fingerprint -sha256` prints them for the certificate below.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::HashMap;

use zerodds_bridge_security::{
    Acl, AclEntry, AclOp, AuthError, AuthMode, AuthSubject, SecurityConfig, SecurityError,
    authenticate, build_ctx, mtls_subject_from_der, rustls_pemfile,
};

/// `openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -subj
/// "/O=Example/CN=alice"` (OpenSSL 4.0.3).
const CERT_PEM: &str = "-----BEGIN CERTIFICATE-----\nMIIBeTCCASCgAwIBAgIUNYvSwdb4nEI1QfcU6A3VHMe8RIQwCgYIKoZIzj0EAwIw\nIjEQMA4GA1UECgwHRXhhbXBsZTEOMAwGA1UEAwwFYWxpY2UwIBcNMjYxMTAxMjMx\nNzQxWhgPMjEyNjEwMDgyMzE3NDFaMCIxEDAOBgNVBAoMB0V4YW1wbGUxDjAMBgNV\nBAMMBWFsaWNlMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEXjZywaxMDiJUet1T\n1J4T3v2SVAyDDMEna5jzLGm96fNXCSH3lrCHtDTSKDrKZfDOOVktt9WV5VLQjMnw\nuTDqlKMyMDAwHQYDVR0OBBYEFHlA+hOEKTwWrLHBF/JFiCzLdVGcMA8GA1UdEwEB\n/wQFMAMBAf8wCgYIKoZIzj0EAwIDRwAwRAIgBJDU1K8HqMWikQpg39J+5+yeo1DN\n2+5ADkJJvNABPKgCIERq4rHsCOs0FM9GaH9NUc8tob9jmSbp1qce2uqsJWhv\n-----END CERTIFICATE-----\n";

#[test]
fn coapbridge_7_2_build_ctx_reads_the_auth_mode_and_the_acl_default() {
    let open = build_ctx(&SecurityConfig {
        auth_mode: "none".to_string(),
        ..SecurityConfig::default()
    })
    .unwrap();
    assert!(matches!(*open.auth, AuthMode::None));
    assert!(open.tls.is_none());
    assert!(
        open.acl
            .check(&AuthSubject::anonymous(), AclOp::Write, "Any")
    );
    let mut tokens = HashMap::new();
    tokens.insert("tk".to_string(), "alice".to_string());
    let bearer = build_ctx(&SecurityConfig {
        auth_mode: "bearer".to_string(),
        bearer_tokens: tokens,
        ..SecurityConfig::default()
    })
    .unwrap();
    assert!(
        !bearer
            .acl
            .check(&AuthSubject::new("alice"), AclOp::Read, "Any")
    );
    assert!(matches!(
        build_ctx(&SecurityConfig {
            auth_mode: "kerberos".to_string(),
            ..SecurityConfig::default()
        }),
        Err(SecurityError::UnknownAuthMode(_))
    ));
}

#[test]
fn coapbridge_7_2_a_bearer_token_names_its_subject() {
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
fn coapbridge_7_3_the_acl_grants_read_and_write_per_topic_and_denies_the_rest() {
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
fn coapbridge_7_2_the_mtls_subject_is_the_dn_and_the_sha_256_fingerprint() {
    let der = rustls_pemfile::certs(&mut CERT_PEM.as_bytes())
        .next()
        .unwrap()
        .unwrap();
    let subject = mtls_subject_from_der(der.as_ref());
    assert_eq!(subject.name, "CN=alice,O=Example");
    assert_eq!(
        subject.fingerprint.as_deref(),
        Some("mtls:0e86be9f6f43e4050f8c1bf7e5c4d3d52a4433c218a9eaea4191a88f7f1f3267")
    );
}
