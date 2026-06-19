// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! E2E test for `zerodds-corba-bridged` §7 security wireup.
//!
//! Covers:
//! * §7.1 TLS — `build_corba_tls_config` (SSLIOP — no ALPN token).
//! * §7.2 Auth — bearer token via GIOP service context (`context_id=256`).
//! * §7.2 Reject — malformed service context.
//! * §7.2 mTLS — subject pass-through via the TLS layer.
//! * §7.3 Topic ACL — DDS topic bridge mapping.

#![cfg(feature = "std")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::path::PathBuf;
use std::sync::Arc;

use zerodds_corba_dds_bridge::bridge_security::{
    AclEntry, AclOp, AuthError, AuthMode, AuthSubject, SecurityConfig, authenticate_corba,
    authorize, build_ctx,
};
use zerodds_corba_dds_bridge::daemon_runtime::build_corba_tls_config;

fn write_temp(name: &str, body: &[u8]) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "zd-corba-sec-e2e-{}-{}-{}",
        name,
        std::process::id(),
        seq
    ));
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    p
}

fn gen_self_signed() -> (String, String) {
    let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    (ck.cert.pem(), ck.key_pair.serialize_pem())
}

#[test]
fn build_corba_tls_config_loads_self_signed() {
    let (cert, key) = gen_self_signed();
    let c = write_temp("corba_cert.pem", cert.as_bytes());
    let k = write_temp("corba_key.pem", key.as_bytes());
    let cfg: Arc<rustls::ServerConfig> = build_corba_tls_config(c, k, None).expect("build cfg");
    // SSLIOP: no ALPN token expected.
    assert!(cfg.alpn_protocols.is_empty());
}

#[test]
fn build_corba_tls_config_with_mtls_succeeds() {
    let (cert, key) = gen_self_signed();
    let c = write_temp("corba_cert2.pem", cert.as_bytes());
    let k = write_temp("corba_key2.pem", key.as_bytes());
    let ca = write_temp("corba_ca.pem", cert.as_bytes());
    let cfg = build_corba_tls_config(c, k, Some(ca)).expect("mtls cfg");
    assert!(cfg.alpn_protocols.is_empty());
}

#[test]
fn build_corba_tls_config_rejects_missing_cert() {
    let bad = PathBuf::from("/tmp/zerodds-corba-nonexistent-cert.pem");
    let bad_key = PathBuf::from("/tmp/zerodds-corba-nonexistent-key.pem");
    let err = build_corba_tls_config(bad, bad_key, None).unwrap_err();
    let lower = err.to_lowercase();
    assert!(lower.contains("tls") || lower.contains("read") || lower.contains("file"));
}

#[test]
fn auth_bearer_via_service_context_accepts_known_token() {
    let mut sec = SecurityConfig::default();
    sec.auth_mode = "bearer".into();
    sec.bearer_tokens.insert("tk-corba".into(), "alice".into());
    let ctx = build_ctx(&sec).unwrap();
    // Vendor schema: `\0bearer\0<token>`.
    let blob = b"\0bearer\0tk-corba";
    let s = authenticate_corba(&ctx.auth, Some(blob), None).unwrap();
    assert_eq!(s.name, "alice");
}

#[test]
fn auth_bearer_via_service_context_rejects_unknown_token() {
    let mut sec = SecurityConfig::default();
    sec.auth_mode = "bearer".into();
    sec.bearer_tokens.insert("tk-corba".into(), "alice".into());
    let ctx = build_ctx(&sec).unwrap();
    let blob = b"\0bearer\0wrong-token";
    let err = authenticate_corba(&ctx.auth, Some(blob), None).unwrap_err();
    assert!(matches!(err, AuthError::Rejected(_)));
}

#[test]
fn auth_malformed_service_context_rejected() {
    let auth = AuthMode::Bearer {
        tokens: std::collections::HashMap::new(),
    };
    let blob = b"\0wrongscheme\0xx";
    let err = authenticate_corba(&auth, Some(blob), None).unwrap_err();
    assert!(matches!(err, AuthError::MalformedCredentials(_)));
}

#[test]
fn auth_mtls_passes_subject_through_tls_layer() {
    let auth = AuthMode::Mtls;
    let s = authenticate_corba(&auth, None, Some(AuthSubject::new("CN=client.example"))).unwrap();
    assert_eq!(s.name, "CN=client.example");
}

#[test]
fn acl_dds_topic_bridge_mapping_allows_listed_subject() {
    let mut sec = SecurityConfig::default();
    sec.topic_acl.insert(
        "DdsTopic::Trade".into(),
        AclEntry {
            read: vec!["alice".into()],
            write: vec!["alice".into()],
        },
    );
    let ctx = build_ctx(&sec).unwrap();
    let alice = AuthSubject::new("alice");
    let bob = AuthSubject::new("bob");
    assert!(authorize(&ctx.acl, &alice, AclOp::Read, "DdsTopic::Trade"));
    assert!(authorize(&ctx.acl, &alice, AclOp::Write, "DdsTopic::Trade"));
    assert!(!authorize(&ctx.acl, &bob, AclOp::Read, "DdsTopic::Trade"));
}

#[test]
fn auth_none_yields_anonymous_for_unauthenticated_giop() {
    let s = authenticate_corba(&AuthMode::None, None, None).unwrap();
    assert_eq!(s.name, "anonymous");
}
