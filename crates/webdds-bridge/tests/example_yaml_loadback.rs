// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Loadback test for `packaging/linux/configs/webdds-bridged.yaml.example`.
//!
//! The shipped example YAML is embedded at build time via
//! `include_str!` and run through `DaemonConfig::load_from_str`. Doc
//! drift between the example and the parser's actual schema surfaces
//! here, not only in the field — the same discipline every other
//! `zerodds-*-bridged` daemon's `example_yaml_loadback.rs` follows.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_webdds_bridge::daemon::config::DaemonConfig;

/// The shipped example YAML.
const EXAMPLE_YAML: &str =
    include_str!("../../../packaging/linux/configs/webdds-bridged.yaml.example");

#[test]
fn example_yaml_parses_without_error() {
    let res = DaemonConfig::load_from_str(EXAMPLE_YAML);
    assert!(
        res.is_ok(),
        "webdds-bridged.yaml.example does not parse: {:?}",
        res.err()
    );
}

#[test]
fn example_yaml_key_fields_match_intent() {
    let cfg =
        DaemonConfig::load_from_str(EXAMPLE_YAML).expect("example yaml must parse — see above");

    assert_eq!(cfg.listen, "0.0.0.0:8080");
    assert_eq!(cfg.domain, 0);
    assert_eq!(cfg.log_level, "info");
    assert_eq!(cfg.max_body_bytes, 8_388_608);

    assert!(!cfg.tls_enabled);
    assert_eq!(cfg.auth_mode, "none");
    assert!(cfg.auth_bearer_token.is_none());

    assert!(!cfg.metrics_enabled);
    assert_eq!(cfg.metrics_addr, "127.0.0.1:9095");

    assert_eq!(
        cfg.app_acl.get("app1"),
        Some(&(vec!["operator".to_string()], vec!["operator".to_string()]))
    );

    assert_eq!(cfg.types.len(), 1);
    assert_eq!(cfg.types[0].type_name, "Demo::Sensor");
    assert_eq!(
        cfg.types[0].idl_file,
        Some("/etc/zerodds/idl/sensor.idl".to_string())
    );
    assert_eq!(cfg.types[0].xml_file, None);

    assert_eq!(
        cfg.interfaces,
        vec!["/etc/zerodds/idl/calculator.idl".to_string()]
    );
}
