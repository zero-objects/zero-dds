// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Loads back the shipped `packaging/linux/configs/opcua-bridged.yaml.example`
//! — it must parse under the daemon's own strict key check, the same
//! contract `zerodds-coap-bridge`'s own `example_yaml_loadback.rs` checks.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_opcua_bridge::daemon::config::DaemonConfig;

#[test]
fn shipped_example_config_loads_back() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packaging/linux/configs/opcua-bridged.yaml.example");
    let cfg =
        DaemonConfig::load_from_file(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    assert_eq!(cfg.domain, 0);
    assert_eq!(cfg.idl.len(), 1);
    assert!(cfg.gateway_config.is_some());
    assert!(cfg.server.enabled);
    assert_eq!(cfg.server.bind, "0.0.0.0:4840");
    assert!(!cfg.client.enabled);
    assert_eq!(cfg.client.servers.len(), 1);
    assert_eq!(cfg.client.servers[0].nodes[0].dds_topic, "Commands");
    assert!(!cfg.pubsub.enabled);
    assert_eq!(
        cfg.pubsub.connections[0].publishers[0].dds_topic,
        "Telemetry"
    );
    assert_eq!(
        cfg.pubsub.connections[0].subscribers[0].fields,
        vec!["value".to_string()]
    );
    assert!(!cfg.acl_default_deny);
    assert_eq!(cfg.acl_rules[0].0, "opc-client-1");
}
