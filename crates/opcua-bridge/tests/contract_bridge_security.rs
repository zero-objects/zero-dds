// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-opcua-bridge` with `zerodds-bridge-security`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-bridge-security`: `build_ctx` and the ACL types `Acl`, `AclEntry`, `AclOp` (`src/daemon/security.rs`).
//!
//! Oracles: the bridge specification §7.2 and §7.3 (the auth modes; without an ACL, `none` allows everything and the other modes deny); the bridge specification §7.3 (read and write per topic, deny by default).
#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::HashMap;

use zerodds_bridge_security::{
    Acl, AclEntry, AclOp, AuthMode, AuthSubject, SecurityConfig, SecurityError, build_ctx,
};

#[test]
fn opcuabridge_7_2_build_ctx_reads_the_auth_mode_and_the_acl_default() {
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
fn opcuabridge_7_3_the_acl_grants_read_and_write_per_topic_and_denies_the_rest() {
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
