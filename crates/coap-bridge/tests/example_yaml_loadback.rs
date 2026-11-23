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
fn coapbridge_3_example_yaml_parses_without_error() {
    let res = DaemonConfig::load_from_str(EXAMPLE_YAML);
    assert!(
        res.is_ok(),
        "coap-bridged.yaml.example does not parse: {:?}",
        res.err()
    );
}

#[test]
fn coapbridge_3_example_yaml_key_fields_match_intent() {
    let cfg = DaemonConfig::load_from_str(EXAMPLE_YAML)
        .expect("example yaml must parse — see coapbridge_3_example_yaml_parses_without_error");

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
fn coapbridge_5_1_example_yaml_topics_reach_the_daemon() {
    let cfg = DaemonConfig::load_from_str(EXAMPLE_YAML)
        .expect("example yaml must parse — see coapbridge_3_example_yaml_parses_without_error");

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
fn coapbridge_7_3_example_yaml_acl_reaches_the_daemon() {
    let cfg = DaemonConfig::load_from_str(EXAMPLE_YAML)
        .expect("example yaml must parse — see coapbridge_3_example_yaml_parses_without_error");

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
fn coapbridge_11_example_yaml_has_no_unknown_top_level_keys() {
    const KNOWN: &[&str] = &[
        "domain",
        "log_level",
        "coap",
        "oscore",
        "proxy",
        "auth",
        "acl",
        "metrics",
        "content_format",
        "topics",
        "tenants",
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

/// Spec §3: the example of the specification itself, taken from
/// `docs/specs/zerodds-coap-bridge-1.0.md`, loads through the parser, with
/// the values it names (it used to stop at `coap.block_size`,
/// `content_format`, `metrics.path` and `history.kind`).
#[test]
fn coapbridge_3_the_example_of_the_spec_loads() {
    let spec = include_str!("../../../docs/specs/zerodds-coap-bridge-1.0.md");
    let section = &spec[spec.find("## §3 Config file format").expect("§3")..];
    let start = section.find("```yaml\n").expect("the example") + "```yaml\n".len();
    let yaml = &section[start..start + section[start..].find("```").expect("its end")];
    let cfg = DaemonConfig::load_from_str(yaml).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(cfg.block_size, 1024);
    assert_eq!(cfg.max_body_size, 65_536);
    assert_eq!(cfg.metrics_path, "/metrics");
    assert_eq!(cfg.content_formats.cdr2_be, 65001);
    assert_eq!(cfg.content_formats.cbor, 60);
    let chat = &cfg.topics[0];
    assert_eq!(chat.history_kind, "keep_last");
    assert_eq!(chat.history_depth, 10);
    assert_eq!(chat.idl_file, "/etc/zerodds/idl/chat.idl");
    assert_eq!(chat.lifespan_ms, Some(30_000));
    assert_eq!(chat.deadline_ms, Some(5_000));
    assert_eq!(chat.partition, vec!["site-a".to_string()]);
    assert_eq!(cfg.topics[1].extensibility, "final");
    assert_eq!(cfg.topics.len(), 3);
    assert_eq!(cfg.tenants.len(), 1);
    assert_eq!(cfg.tenants[0].domain, 7);
    assert_eq!(cfg.tenants[0].subjects, vec!["psk:device-001".to_string()]);
    assert_eq!(cfg.tenants[0].topics[0].coap_uri_path, "plant/status");
}
