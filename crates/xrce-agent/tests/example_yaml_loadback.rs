// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Loadback test for `packaging/linux/configs/xrce-agentd.yaml.example`,
//! mirroring `crates/coap-bridge/tests/example_yaml_loadback.rs`: the
//! shipped example YAML is embedded at build time via `include_str!` and
//! run through the real parser, so drift between the packaged example and
//! `DaemonConfig::from_config`'s accepted keys shows up as a test failure
//! rather than a silent WARN at someone's daemon startup.

#![cfg(feature = "daemon")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_xrce_agent::daemon::config::{DaemonConfig, ListenerConfig};

/// The shipped example YAML — embedded at compile time against the real
/// file, so drift becomes visible immediately as a test failure.
const EXAMPLE_YAML: &str =
    include_str!("../../../packaging/linux/configs/xrce-agentd.yaml.example");

#[test]
fn example_yaml_parses_without_error() {
    let res = DaemonConfig::load_from_str(EXAMPLE_YAML);
    assert!(
        res.is_ok(),
        "xrce-agentd.yaml.example does not parse: {:?}",
        res.err()
    );
}

#[test]
fn example_yaml_key_fields_match_intent() {
    let cfg = DaemonConfig::load_from_str(EXAMPLE_YAML)
        .expect("example yaml must parse — see example_yaml_parses_without_error");

    assert_eq!(cfg.log_level, "info");
    assert_eq!(cfg.vendor_id, [0, 0]);
    assert_eq!(cfg.session_timeout_secs, 60);

    assert_eq!(cfg.max_clients, 1024);
    assert_eq!(cfg.max_objects_per_client, 256);
    assert_eq!(cfg.max_pending_samples, 256);

    assert!(cfg.discovery_enabled);
    assert_eq!(cfg.discovery_port, 7400);

    assert_eq!(
        cfg.listeners,
        vec![ListenerConfig::Udp {
            bind: "0.0.0.0:2018".to_string()
        }]
    );

    assert!(cfg.type_files.is_empty());
    assert!(cfg.xml_profile_files.is_empty());
    assert!(cfg.qos_library_files.is_empty());
}
