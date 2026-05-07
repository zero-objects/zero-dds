// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! E2E-Test fuer `zerodds-grpc-bridged` §7 Security-Wireup.
//!
//! Deckt:
//! * §7.1 TLS — `build_grpc_tls_config` mit ALPN `h2`.
//! * §7.2 Auth — Bearer-Token via HTTP/2-`authorization`-Metadata.
//! * §7.3 Topic-ACL — Path → Topic-Mapping mit Read/Write-Check.

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

use zerodds_grpc_bridge::bridge_security::{
    AclEntry, AclOp, AuthMode, AuthSubject, SecurityConfig, authenticate_grpc, authorize,
    authorize_grpc, build_ctx,
};
use zerodds_grpc_bridge::daemon_runtime::build_grpc_tls_config;

fn write_temp(name: &str, body: &[u8]) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "zd-grpc-sec-e2e-{}-{}-{}",
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
fn build_grpc_tls_config_sets_alpn_h2() {
    let (cert, key) = gen_self_signed();
    let c = write_temp("grpc_cert.pem", cert.as_bytes());
    let k = write_temp("grpc_key.pem", key.as_bytes());
    let cfg: Arc<rustls::ServerConfig> = build_grpc_tls_config(c, k, None).expect("build cfg");
    // ALPN-Liste muss exakt [b"h2"] sein.
    assert_eq!(cfg.alpn_protocols.len(), 1);
    assert_eq!(cfg.alpn_protocols[0], b"h2");
}

#[test]
fn build_grpc_tls_config_with_client_ca_for_mtls() {
    let (cert, key) = gen_self_signed();
    let c = write_temp("grpc_cert2.pem", cert.as_bytes());
    let k = write_temp("grpc_key2.pem", key.as_bytes());
    let ca = write_temp("grpc_ca.pem", cert.as_bytes());
    let cfg = build_grpc_tls_config(c, k, Some(ca)).expect("mtls cfg");
    assert_eq!(cfg.alpn_protocols, vec![b"h2".to_vec()]);
}

#[test]
fn build_grpc_tls_config_rejects_missing_cert() {
    let bad = PathBuf::from("/tmp/zerodds-nonexistent-cert.pem");
    let bad_key = PathBuf::from("/tmp/zerodds-nonexistent-key.pem");
    let err = build_grpc_tls_config(bad, bad_key, None).unwrap_err();
    assert!(err.to_lowercase().contains("open") || err.to_lowercase().contains("no such"));
}

#[test]
fn auth_bearer_in_metadata_succeeds() {
    let mut sec = SecurityConfig::default();
    sec.auth_mode = "bearer".into();
    sec.bearer_tokens.insert("tk-grpc".into(), "alice".into());
    let ctx = build_ctx(&sec).unwrap();
    let md = vec![
        (":method".to_string(), "POST".to_string()),
        (":path".to_string(), "/com.zerodds/Trade".to_string()),
        ("authorization".to_string(), "Bearer tk-grpc".to_string()),
    ];
    let s = authenticate_grpc(&ctx.auth, &md, None).unwrap();
    assert_eq!(s.name, "alice");
}

#[test]
fn auth_missing_bearer_metadata_rejected() {
    let mut sec = SecurityConfig::default();
    sec.auth_mode = "bearer".into();
    sec.bearer_tokens.insert("tk".into(), "alice".into());
    let ctx = build_ctx(&sec).unwrap();
    let md: Vec<(String, String)> = vec![];
    let err = authenticate_grpc(&ctx.auth, &md, None).unwrap_err();
    assert!(matches!(
        err,
        zerodds_grpc_bridge::bridge_security::AuthError::MissingCredentials
    ));
}

#[test]
fn acl_grpc_path_to_topic_mapping_allows_alice_on_trade() {
    let mut sec = SecurityConfig::default();
    sec.topic_acl.insert(
        "Trade".into(),
        AclEntry {
            read: vec!["alice".into()],
            write: vec!["alice".into()],
        },
    );
    let ctx = build_ctx(&sec).unwrap();
    let alice = AuthSubject::new("alice");
    assert!(authorize_grpc(
        &ctx.acl,
        &alice,
        AclOp::Read,
        "/com.zerodds.svc/Trade"
    ));
    assert!(authorize_grpc(
        &ctx.acl,
        &alice,
        AclOp::Write,
        "/com.zerodds.svc/Trade"
    ));
}

#[test]
fn acl_deny_for_non_listed_subject_yields_false() {
    let mut sec = SecurityConfig::default();
    sec.topic_acl.insert(
        "Restricted".into(),
        AclEntry {
            read: vec!["admin".into()],
            write: vec!["admin".into()],
        },
    );
    let ctx = build_ctx(&sec).unwrap();
    let bob = AuthSubject::new("bob");
    assert!(!authorize_grpc(
        &ctx.acl,
        &bob,
        AclOp::Read,
        "/svc/Restricted"
    ));
    assert!(!authorize(&ctx.acl, &bob, AclOp::Write, "Restricted"));
}

#[test]
fn auth_none_yields_anonymous_for_open_grpc_endpoint() {
    let auth = AuthMode::None;
    let s = authenticate_grpc(&auth, &[], None).unwrap();
    assert_eq!(s.name, "anonymous");
}
