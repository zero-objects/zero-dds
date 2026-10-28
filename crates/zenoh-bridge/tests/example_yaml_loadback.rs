// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Loadback test for `packaging/linux/configs/zenoh-bridged.yaml.example`,
//! mirroring `crates/coap-bridge/tests/example_yaml_loadback.rs`: the
//! shipped example YAML is embedded at build time and run through
//! `DaemonConfig::load_from_str`, so schema drift between the example and
//! the parser is a test failure, not a silent divergence.

#![cfg(feature = "daemon")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_zenoh_bridge::daemon::config::DaemonConfig;

/// The shipped example YAML — embedded at compile time against the real
/// file, so drift becomes visible immediately as a test failure.
const EXAMPLE_YAML: &str =
    include_str!("../../../packaging/linux/configs/zenoh-bridged.yaml.example");

#[test]
fn example_yaml_parses_without_error() {
    let res = DaemonConfig::load_from_str(EXAMPLE_YAML);
    assert!(
        res.is_ok(),
        "zenoh-bridged.yaml.example does not parse: {:?}",
        res.err()
    );
}

#[test]
fn example_yaml_key_fields_match_intent() {
    let cfg = DaemonConfig::load_from_str(EXAMPLE_YAML)
        .expect("example yaml must parse — see example_yaml_parses_without_error");

    assert_eq!(cfg.domain, 0);
    assert_eq!(cfg.log_level, "info");

    assert_eq!(cfg.zenoh.mode, "peer");
    assert!(cfg.zenoh.connect.is_empty());
    assert!(cfg.zenoh.listen.is_empty());
    assert!(cfg.zenoh.multicast_scouting);
    // Matches zenoh-plugin-dds's own default (no scope segment).
    assert_eq!(cfg.zenoh.scope, "");
    assert!(cfg.zenoh.reliable_routes_blocking);
    assert_eq!(cfg.zenoh.queries_timeout_ms, 5000);
    assert!(cfg.zenoh.config_file.is_none());

    assert!(cfg.discovery.auto_bridge);
    assert_eq!(cfg.discovery.poll_interval_ms, 1000);
    assert!(cfg.discovery.allow.is_empty());
    assert!(cfg.discovery.deny.is_empty());

    assert_eq!(cfg.limits.max_payload_bytes, 1024 * 1024);

    assert!(!cfg.metrics_enabled);
    assert_eq!(cfg.metrics_addr, "127.0.0.1:9097");
}

#[test]
fn example_yaml_topics_reach_the_daemon() {
    let cfg = DaemonConfig::load_from_str(EXAMPLE_YAML)
        .expect("example yaml must parse — see example_yaml_parses_without_error");

    assert_eq!(
        cfg.topics.len(),
        1,
        "example should show exactly one demo topic (the keyed one is commented out)"
    );
    let t = &cfg.topics[0];
    assert_eq!(t.dds_name, "Chatter");
    assert_eq!(t.dds_type, "std_msgs::String");
    assert_eq!(t.direction, "bidir");
    assert_eq!(t.partition, "");
    assert_eq!(t.key_expr, "");
    assert!(!t.keyed);
    assert_eq!(t.reliability, "reliable");
    assert_eq!(t.durability, "volatile");
    assert_eq!(t.extensibility, "final");
    assert_eq!(t.history_depth, 1);
}

#[test]
fn example_yaml_has_no_unknown_top_level_keys() {
    const KNOWN: &[&str] = &[
        "domain",
        "log_level",
        "zenoh",
        "discovery",
        "limits",
        "metrics",
        "topics",
    ];
    for key in top_level_keys(EXAMPLE_YAML) {
        assert!(
            KNOWN.contains(&key.as_str()),
            "zenoh-bridged.yaml.example declares top-level key {key:?}, \
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
