// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! E2E test for the `zerodds-grpc-bridged` §7 security wireup.
//!
//! Covers:
//! * §7.1 TLS — `build_grpc_tls_config` with ALPN `h2`.
//! * §7.2 Auth — bearer token via the HTTP/2 `authorization` metadata.
//! * §7.3 Topic ACL — path → topic mapping with a read/write check.

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
use zerodds_grpc_bridge::daemon_runtime::{
    GrpcTlsOptions, TlsMinVersion, build_grpc_tls_config, build_grpc_tls_config_opts,
    provider_cipher_suites, select_tls_suites,
};

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
fn grpcbridge_7_1_build_grpc_tls_config_sets_alpn_h2() {
    let (cert, key) = gen_self_signed();
    let c = write_temp("grpc_cert.pem", cert.as_bytes());
    let k = write_temp("grpc_key.pem", key.as_bytes());
    let cfg: Arc<rustls::ServerConfig> = build_grpc_tls_config(c, k, None).expect("build cfg");
    // The ALPN list must be exactly [b"h2"].
    assert_eq!(cfg.alpn_protocols.len(), 1);
    assert_eq!(cfg.alpn_protocols[0], b"h2");
}

#[test]
fn grpcbridge_7_1_build_grpc_tls_config_with_client_ca_for_mtls() {
    let (cert, key) = gen_self_signed();
    let c = write_temp("grpc_cert2.pem", cert.as_bytes());
    let k = write_temp("grpc_key2.pem", key.as_bytes());
    let ca = write_temp("grpc_ca.pem", cert.as_bytes());
    let cfg = build_grpc_tls_config(c, k, Some(ca)).expect("mtls cfg");
    assert_eq!(cfg.alpn_protocols, vec![b"h2".to_vec()]);
}

#[test]
fn grpcbridge_7_1_build_grpc_tls_config_rejects_missing_cert() {
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

/// §7.1 `grpc.tls.cipher_suites`: the suites of the server configuration
/// are the named ones, in the order named; the server's order decides.
#[test]
fn grpcbridge_7_1_the_configured_cipher_suites_are_the_ones_in_the_config() {
    let (cert, key) = gen_self_signed();
    let c = write_temp("suites_cert.pem", cert.as_bytes());
    let k = write_temp("suites_key.pem", key.as_bytes());
    let opts = GrpcTlsOptions {
        min_version: TlsMinVersion::Tls12,
        cipher_suites: vec![
            "TLS_CHACHA20_POLY1305_SHA256".into(),
            "TLS_AES_128_GCM_SHA256".into(),
        ],
    };
    let cfg = build_grpc_tls_config_opts(c, k, None, false, &opts).expect("build");
    let names: Vec<String> = cfg
        .crypto_provider()
        .cipher_suites
        .iter()
        .map(|s| format!("{:?}", s.suite()))
        .collect();
    assert_eq!(
        names,
        ["TLS13_CHACHA20_POLY1305_SHA256", "TLS13_AES_128_GCM_SHA256"]
    );
    assert!(cfg.ignore_client_order);
    // Without a list: every suite of the provider, the client's order.
    let (cert, key) = gen_self_signed();
    let c = write_temp("suites_cert2.pem", cert.as_bytes());
    let k = write_temp("suites_key2.pem", key.as_bytes());
    let all = build_grpc_tls_config_opts(c, k, None, false, &GrpcTlsOptions::default()).unwrap();
    assert_eq!(
        all.crypto_provider().cipher_suites.len(),
        provider_cipher_suites().len()
    );
    assert!(!all.ignore_client_order);
    assert!(provider_cipher_suites().contains(&"TLS_AES_128_GCM_SHA256".to_string()));
    assert!(
        provider_cipher_suites().contains(&"TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256".to_string())
    );
}

/// §7.1: a suite the build does not offer, a suite named twice, and
/// `min_version: 1.3` with TLS 1.2 suites only are refused.
#[test]
fn grpcbridge_7_1_an_unknown_suite_does_not_load() {
    let opts = |min, names: &[&str]| GrpcTlsOptions {
        min_version: min,
        cipher_suites: names.iter().map(|n| n.to_string()).collect(),
    };
    let e =
        select_tls_suites(&opts(TlsMinVersion::Tls12, &["TLS_RSA_WITH_RC4_128_SHA"])).unwrap_err();
    assert!(e.contains("not offered"), "{e}");
    let e = select_tls_suites(&opts(
        TlsMinVersion::Tls12,
        &["TLS_AES_128_GCM_SHA256", "TLS13_AES_128_GCM_SHA256"],
    ))
    .unwrap_err();
    assert!(e.contains("twice"), "{e}");
    let e = select_tls_suites(&opts(
        TlsMinVersion::Tls13,
        &["TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256"],
    ))
    .unwrap_err();
    assert!(e.contains("1.3"), "{e}");
    let (_, versions) = select_tls_suites(&opts(TlsMinVersion::Tls13, &[])).unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].version, rustls::ProtocolVersion::TLSv1_3);
    let (_, versions) = select_tls_suites(&GrpcTlsOptions::default()).unwrap();
    assert_eq!(versions.len(), 2);
}
