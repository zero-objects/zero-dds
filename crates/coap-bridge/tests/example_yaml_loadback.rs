// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Loadback test for `packaging/linux/configs/coap-bridged.yaml.example`.
//!
//! Background: the shipped example yaml declared `participant:` / `coap:` /
//! `routes:` / `logging:` / `observability:`. Only `coap:` was a key the
//! parser knows; everything else fell into `_ => {}`. The file parsed
//! without error, but the declared resource map never reached the daemon.
//!
//! This test prevents a recurrence: the shipped example YAML is embedded at
//! build time via `include_str!` and run through `DaemonConfig::load_from_str`.
//!
//! Spec reference: `docs/specs/zerodds-coap-bridge-1.0.md` §3.

#![cfg(feature = "daemon")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_coap_bridge::daemon::config::DaemonConfig;

/// The shipped example YAML — embedded at compile time against the real
/// file, so drift becomes visible immediately as a test failure.
const EXAMPLE_YAML: &str =
    include_str!("../../../packaging/linux/configs/coap-bridged.yaml.example");

#[test]
fn example_yaml_parses_without_error() {
    let res = DaemonConfig::load_from_str(EXAMPLE_YAML);
    assert!(
        res.is_ok(),
        "coap-bridged.yaml.example does not parse: {:?}",
        res.err()
    );
}

#[test]
fn example_yaml_key_fields_match_intent() {
    let cfg = DaemonConfig::load_from_str(EXAMPLE_YAML)
        .expect("example yaml must parse — see example_yaml_parses_without_error");

    assert_eq!(cfg.domain, 0);
    assert_eq!(cfg.log_level, "info");

    // coap — `observe_max_age_secs` deviates from the default (60), so the
    // assertion proves the block reached the daemon.
    assert_eq!(cfg.bind, "0.0.0.0:5683");
    assert_eq!(cfg.max_message_size, 1152);
    assert_eq!(cfg.ack_timeout_ms, 2000);
    assert_eq!(cfg.max_retransmit, 4);
    assert_eq!(cfg.observe_max_age_secs, 90);
    assert!(!cfg.dtls_enabled);

    // oscore — off; with `enabled: true` and no master secret the parser
    // would refuse the file (fail-closed, Spec §7.2).
    assert!(!cfg.oscore.enabled);

    assert_eq!(cfg.auth_mode, "none");

    // metrics — endpoint off, but `metrics.listen` (Spec §3) must arrive;
    // the default is the empty string.
    assert!(!cfg.metrics_enabled);
    assert_eq!(cfg.metrics_addr, "127.0.0.1:9093");
}

#[test]
fn example_yaml_topics_reach_the_daemon() {
    let cfg = DaemonConfig::load_from_str(EXAMPLE_YAML)
        .expect("example yaml must parse — see example_yaml_parses_without_error");

    // The regression the old `routes:` schema caused: an empty topic list.
    assert_eq!(
        cfg.topics.len(),
        1,
        "example should show exactly one demo resource"
    );
    let t = &cfg.topics[0];
    assert_eq!(t.dds_name, "DeviceShadow");
    assert_eq!(t.dds_type, "demo::Iot::Shadow");
    assert_eq!(t.coap_uri_path, "shadow/device");
    assert_eq!(t.direction, "bidir");
    assert_eq!(t.reliability, "reliable");
    assert_eq!(t.durability, "volatile");
    assert_eq!(t.history_depth, 5);
}

#[test]
fn example_yaml_acl_reaches_the_daemon() {
    let cfg = DaemonConfig::load_from_str(EXAMPLE_YAML)
        .expect("example yaml must parse — see example_yaml_parses_without_error");

    // Spec §3/§7.3: subject-keyed rules, `allow_post` = write,
    // `allow_observe` = read, inverted into the resource-keyed map.
    assert!(cfg.acl_default_deny);
    let (read, write) = cfg
        .topic_acl
        .get("shadow/device")
        .expect("acl.rules must land in topic_acl");
    assert_eq!(read, &vec!["device-001".to_string()]);
    assert_eq!(write, &vec!["device-001".to_string()]);
}

#[test]
fn example_yaml_has_no_unknown_top_level_keys() {
    const KNOWN: &[&str] = &[
        "domain",
        "log_level",
        "coap",
        "oscore",
        "auth",
        "acl",
        "metrics",
        "topics",
    ];
    for key in top_level_keys(EXAMPLE_YAML) {
        assert!(
            KNOWN.contains(&key.as_str()),
            "coap-bridged.yaml.example declares top-level key {key:?}, \
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
