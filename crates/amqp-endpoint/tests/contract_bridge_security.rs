// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-amqp-endpoint` with `zerodds-bridge-security`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-bridge-security`: `authenticate` with the bearer and SASL PLAIN modes (`src/bridge_security.rs`), `tls_provider` and `connection::build_client_tls_connector` (`src/broker.rs`).
//!
//! Oracles: RFC 6750 §2.1 (`Authorization: Bearer <token>`) with RFC 7235 §2.1 (the scheme is case-insensitive); RFC 4616 §2 (`[authzid] NUL authcid NUL passwd`); RFC 8446 §9.1 (TLS_AES_128_GCM_SHA256 must be implemented).
#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::HashMap;

use zerodds_bridge_security::{
    AuthError, AuthMode, AuthSubject, authenticate, rustls, tls_provider,
};

#[test]
fn amqpbridge_7_2_a_bearer_token_names_its_subject() {
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
fn amqpbridge_7_2_a_plain_message_is_authzid_nul_authcid_nul_passwd() {
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
fn amqpbridge_7_1_the_tls_provider_implements_tls_aes_128_gcm_sha256() {
    let provider = tls_provider();
    assert!(
        provider
            .cipher_suites
            .iter()
            .any(|s| s.suite() == rustls::CipherSuite::TLS13_AES_128_GCM_SHA256)
    );
}
