// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-mqtt-bridge` with `zerodds-bridge-security`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-bridge-security`: `build_ctx`, `authenticate` with the bearer and SASL PLAIN modes, `LiveSecurity` and `tls_provider` (`src/daemon/security.rs`).
//!
//! Oracles: the bridge specification §7.2 and §7.3 (the auth modes; without an ACL, `none` allows everything and the other modes deny); RFC 6750 §2.1 (`Authorization: Bearer <token>`) with RFC 7235 §2.1 (the scheme is case-insensitive); RFC 4616 §2 (`[authzid] NUL authcid NUL passwd`); the bridge specification §7.3 (read and write per topic, deny by default); RFC 8446 §9.1 (TLS_AES_128_GCM_SHA256 must be implemented).
#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::HashMap;

use zerodds_bridge_security::{
    Acl, AclEntry, AclOp, AuthError, AuthMode, AuthSubject, SecurityConfig, SecurityError,
    authenticate, build_ctx, rustls, tls_provider,
};

#[test]
fn mqttbridge_7_2_build_ctx_reads_the_auth_mode_and_the_acl_default() {
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
fn mqttbridge_7_2_a_bearer_token_names_its_subject() {
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
fn mqttbridge_7_2_a_plain_message_is_authzid_nul_authcid_nul_passwd() {
    let mut users = HashMap::new();
    users.insert("alice".to_string(), "wonderland".to_string());
    let mode = AuthMode::SaslPlain { users };
    assert_eq!(
        authenticate(&mode, None, Some(b"\x00alice\x00wonderland"), None)
            .unwrap()
            .name,
        "alice"
    );
    assert!(matches!(
        authenticate(&mode, None, Some(b"\x00alice\x00rabbit"), None),
        Err(AuthError::Rejected(_))
    ));
    assert!(authenticate(&mode, None, Some(b"alice"), None).is_err());
}

#[test]
fn mqttbridge_7_3_the_acl_grants_read_and_write_per_topic_and_denies_the_rest() {
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
fn mqttbridge_7_1_the_tls_provider_implements_tls_aes_128_gcm_sha256() {
    let provider = tls_provider();
    assert!(
        provider
            .cipher_suites
            .iter()
            .any(|s| s.suite() == rustls::CipherSuite::TLS13_AES_128_GCM_SHA256)
    );
}
