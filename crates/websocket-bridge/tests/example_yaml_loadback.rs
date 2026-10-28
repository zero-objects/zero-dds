// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Loadback test for `packaging/linux/configs/ws-bridged.yaml.example`.
//!
//! Background: the shipped example yaml historically had a
//! schema (`participant:` / `websocket:` / `routes:` / `observability:`)
//! that the parser silently ignored with `_ => {}`; the bridge
//! booted with defaults instead of the user-configured values (Issue #3
//! on github.com/zero-objects/zero-dds).
//!
//! This test prevents a recurrence: the shipped example
//! YAML is embedded at build time via `include_str!` and run through
//! `DaemonConfig::load_from_str`. If doc drift strikes again in
//! the future, it surfaces here — not only in the field.
//!
//! Spec reference: `docs/specs/zerodds-ws-bridge-1.0.md` §3 (config file format).

#![cfg(feature = "daemon")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_websocket_bridge::daemon::config::DaemonConfig;

/// The shipped example YAML — embedded at compile time against the
/// real file, so drift becomes visible immediately as a test failure.
const EXAMPLE_YAML: &str = include_str!("../../../packaging/linux/configs/ws-bridged.yaml.example");

#[test]
fn example_yaml_parses_without_error() {
    let res = DaemonConfig::load_from_str(EXAMPLE_YAML);
    assert!(
        res.is_ok(),
        "ws-bridged.yaml.example does not parse: {:?}",
        res.err()
    );
}

#[test]
fn example_yaml_key_fields_match_intent() {
    let cfg = DaemonConfig::load_from_str(EXAMPLE_YAML)
        .expect("example yaml must parse — see example_yaml_parses_without_error");

    // listen/domain/log_level — the top-level scalars from the file.
    // `listen` deviates from `default_for_dev()` (127.0.0.1:8080), so the
    // assertion proves the value came from the file, not from the default.
    assert_eq!(cfg.listen, "0.0.0.0:8080");
    assert_eq!(cfg.domain, 0);
    assert_eq!(cfg.log_level, "info");

    // tls — disabled by default, but paths set so users see them. Both
    // paths are empty in the default config.
    assert!(!cfg.tls_enabled);
    assert_eq!(cfg.tls_cert_file, "/etc/zerodds/certs/ws-bridged.crt");
    assert_eq!(cfg.tls_key_file, "/etc/zerodds/certs/ws-bridged.key");

    // auth — none as the default; bearer_token is commented out.
    assert_eq!(cfg.auth_mode, "none");
    assert!(cfg.auth_bearer_token.is_none());

    // metrics — the endpoint is off, but `metrics.listen` (Spec §3) has to
    // reach the daemon; the default is the empty string.
    assert!(!cfg.metrics_enabled);
    assert_eq!(cfg.metrics_addr, "127.0.0.1:9091");

    // topics — exactly one demo route that the user can adapt as a template.
    assert_eq!(
        cfg.topics.len(),
        1,
        "example should show exactly one demo topic"
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

#[test]
fn example_yaml_topic_acl_reaches_the_daemon() {
    let cfg = DaemonConfig::load_from_str(EXAMPLE_YAML)
        .expect("example yaml must parse — see example_yaml_parses_without_error");

    // Spec §3/§7.3: the ACL is declared inside the topic entry. The old
    // top-level `acl:` block was never read for the shipped example.
    let (read, write) = cfg
        .topic_acl
        .get("Chat::Message")
        .expect("topics[].acl must land in topic_acl");
    assert_eq!(read, &vec!["*".to_string()]);
    assert_eq!(
        write,
        &vec!["service-account".to_string(), "alice".to_string()]
    );
}

#[test]
fn example_yaml_has_no_unknown_top_level_keys() {
    // Every top-level key of the shipped file must be one the parser
    // handles — otherwise the daemon would boot on defaults and only a
    // stderr WARN would say so.
    const KNOWN: &[&str] = &[
        "listen",
        "domain",
        "log_level",
        "tls",
        "auth",
        "metrics",
        "topics",
    ];
    for key in top_level_keys(EXAMPLE_YAML) {
        assert!(
            KNOWN.contains(&key.as_str()),
            "ws-bridged.yaml.example declares top-level key {key:?}, \
             which DaemonConfig::load_from_str does not read"
        );
    }
}

/// Top-level (column-0) mapping keys of a YAML document, comments stripped.
fn top_level_keys(yaml: &str) -> Vec<String> {
    yaml.lines()
        .filter(|l| !l.starts_with([' ', '\t', '#', '-']) && !l.trim().is_empty())
        .filter_map(|l| l.split_once(':').map(|(k, _)| k.trim().to_string()))
        .collect()
}
