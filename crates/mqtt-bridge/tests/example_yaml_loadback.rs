// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Loadback test for `packaging/linux/configs/mqtt-bridged.yaml.example`.
//!
//! Background: the shipped example yaml declared `participant:` / `mqtt:` /
//! `routes:` / `logging:` / `observability:`. Only `mqtt:` was a key the
//! parser knows; everything else fell into `_ => {}`. The file parsed
//! without error, but the declared topic map never reached the daemon —
//! `cfg.topics` stayed empty and the bridge bridged nothing.
//!
//! This test prevents a recurrence: the shipped example YAML is embedded at
//! build time via `include_str!` and run through `DaemonConfig::load_from_str`.
//!
//! Spec reference: `docs/specs/zerodds-mqtt-bridge-1.0.md` §3.

#![cfg(feature = "daemon")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_mqtt_bridge::daemon::config::DaemonConfig;

/// The shipped example YAML — embedded at compile time against the real
/// file, so drift becomes visible immediately as a test failure.
const EXAMPLE_YAML: &str =
    include_str!("../../../packaging/linux/configs/mqtt-bridged.yaml.example");

#[test]
fn example_yaml_parses_without_error() {
    let res = DaemonConfig::load_from_str(EXAMPLE_YAML);
    assert!(
        res.is_ok(),
        "mqtt-bridged.yaml.example does not parse: {:?}",
        res.err()
    );
}

#[test]
fn example_yaml_key_fields_match_intent() {
    let cfg = DaemonConfig::load_from_str(EXAMPLE_YAML)
        .expect("example yaml must parse — see example_yaml_parses_without_error");

    assert_eq!(cfg.domain, 0);
    assert_eq!(cfg.log_level, "info");

    // mqtt — `broker_url`, `keep_alive_secs` and `clean_start` all deviate
    // from `default_for_dev()` (mqtt://127.0.0.1:1883 / 60 / true), so the
    // assertions prove the block reached the daemon.
    assert_eq!(cfg.broker_url, "mqtt://localhost:1883");
    assert_eq!(cfg.client_id, "zerodds-mqtt-bridge");
    assert_eq!(cfg.keep_alive_secs, 120);
    assert!(!cfg.clean_start);

    // mqtt.tls — Spec §3 spells the client credentials `cert_file`/`key_file`.
    assert!(!cfg.broker_tls_enabled);
    assert_eq!(cfg.broker_tls_ca_file, "/etc/zerodds/certs/mqtt-ca.pem");
    assert_eq!(
        cfg.broker_tls_client_cert_file,
        "/etc/zerodds/certs/mqtt-bridged.crt"
    );
    assert_eq!(
        cfg.broker_tls_client_key_file,
        "/etc/zerodds/certs/mqtt-bridged.key"
    );

    assert_eq!(cfg.auth_mode, "none");

    // metrics — endpoint off, but `metrics.listen` (Spec §3) must arrive;
    // the default is the empty string.
    assert!(!cfg.metrics_enabled);
    assert_eq!(cfg.metrics_addr, "127.0.0.1:9092");
}

#[test]
fn example_yaml_topics_reach_the_daemon() {
    let cfg = DaemonConfig::load_from_str(EXAMPLE_YAML)
        .expect("example yaml must parse — see example_yaml_parses_without_error");

    // The regression the old `routes:` schema caused: an empty topic list.
    assert_eq!(
        cfg.topics.len(),
        1,
        "example should show exactly one demo topic"
    );
    let t = &cfg.topics[0];
    assert_eq!(t.dds_name, "Telemetry");
    assert_eq!(t.dds_type, "demo::Sensor::Reading");
    assert_eq!(t.mqtt_topic, "factory/+/telemetry");
    assert_eq!(t.direction, "out");
    assert_eq!(t.mqtt_qos, 1);
    assert!(!t.retain);
    assert_eq!(t.reliability, "reliable");
    assert_eq!(t.durability, "volatile");
    assert_eq!(t.history_depth, 10);
}

#[test]
fn example_yaml_acl_reaches_the_daemon() {
    let cfg = DaemonConfig::load_from_str(EXAMPLE_YAML)
        .expect("example yaml must parse — see example_yaml_parses_without_error");

    // Spec §3/§7.3: subject-keyed rules, inverted into the topic-keyed map.
    // `default_deny: false` deviates from the fail-closed default.
    assert!(!cfg.acl_default_deny);
    let (read, write) = cfg
        .topic_acl
        .get("factory/+/telemetry")
        .expect("acl.rules must land in topic_acl");
    assert_eq!(read, &vec!["service-account".to_string()]);
    assert_eq!(write, &vec!["service-account".to_string()]);
}

#[test]
fn example_yaml_has_no_unknown_top_level_keys() {
    const KNOWN: &[&str] = &[
        "domain",
        "log_level",
        "mqtt",
        "auth",
        "acl",
        "metrics",
        "topics",
    ];
    for key in top_level_keys(EXAMPLE_YAML) {
        assert!(
            KNOWN.contains(&key.as_str()),
            "mqtt-bridged.yaml.example declares top-level key {key:?}, \
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
