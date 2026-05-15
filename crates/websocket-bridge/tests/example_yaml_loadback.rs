// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Loadback-Test fuer `packaging/linux/configs/ws-bridged.yaml.example`.
//!
//! Hintergrund: das ausgelieferte example-yaml hatte historisch ein
//! Schema (`participant:` / `websocket:` / `routes:` / `observability:`),
//! das der Parser stillschweigend mit `_ => {}` ignorierte; die Bridge
//! bootete mit Defaults statt der user-konfigurierten Werte (Issue #3
//! auf github.com/zero-objects/zero-dds).
//!
//! Dieser Test verhindert eine Wiederholung: die ausgelieferte example
//! YAML wird via `include_str!` zur Build-Zeit eingebettet und durch
//! `DaemonConfig::load_from_str` gefahren. Schlaegt Doku-Drift in
//! Zukunft wieder zu, faellt sie hier auf — nicht erst im Feld.
//!
//! Spec-Bezug: `docs/specs/zerodds-ws-bridge-1.0.md` §3 (Config-File-Format).

#![cfg(feature = "daemon")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_websocket_bridge::daemon::config::DaemonConfig;

/// Das ausgelieferte example-YAML — wird zur Compile-Zeit gegen den
/// realen File eingebettet, damit Drift sofort als Test-Fail sichtbar wird.
const EXAMPLE_YAML: &str = include_str!("../../../packaging/linux/configs/ws-bridged.yaml.example");

#[test]
fn example_yaml_parses_without_error() {
    let res = DaemonConfig::load_from_str(EXAMPLE_YAML);
    assert!(
        res.is_ok(),
        "ws-bridged.yaml.example parst nicht: {:?}",
        res.err()
    );
}

#[test]
fn example_yaml_key_fields_match_intent() {
    let cfg = DaemonConfig::load_from_str(EXAMPLE_YAML)
        .expect("example yaml must parse — siehe example_yaml_parses_without_error");

    // listen/domain/log_level — die Top-Level-Skalare aus dem File.
    assert_eq!(cfg.listen, "0.0.0.0:8080");
    assert_eq!(cfg.domain, 0);
    assert_eq!(cfg.log_level, "info");

    // tls — disabled by default, aber Pfade gesetzt, damit User sie sehen.
    assert!(!cfg.tls_enabled);
    assert_eq!(cfg.tls_cert_file, "/etc/zerodds/certs/ws-bridged.crt");
    assert_eq!(cfg.tls_key_file, "/etc/zerodds/certs/ws-bridged.key");

    // auth — none als Default; bearer_token ist auskommentiert.
    assert_eq!(cfg.auth_mode, "none");
    assert!(cfg.auth_bearer_token.is_none());

    // metrics — disabled by default.
    assert!(!cfg.metrics_enabled);
    assert_eq!(cfg.metrics_addr, "127.0.0.1:9091");

    // topics — genau eine Demo-Route, die der User als Vorlage anpassen kann.
    assert_eq!(
        cfg.topics.len(),
        1,
        "example soll genau einen Demo-Topic zeigen"
    );
    let t = &cfg.topics[0];
    assert_eq!(t.name, "Chat::Message");
    assert_eq!(t.type_name, "Chat::Message");
    assert_eq!(t.direction, "bidir");
    assert_eq!(t.ws_path, "/chat");
    assert_eq!(t.reliability, "reliable");
    assert_eq!(t.durability, "volatile");
    assert_eq!(t.history_depth, 10);
}
