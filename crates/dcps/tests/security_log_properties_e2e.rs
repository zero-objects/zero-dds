// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! E2E: DDS-Security spec-style logger wireup. A participant carries
//! `dds.sec.log.*` properties on its QoS; the runtime materializes the fan-out
//! logger from them and the wired logger actually receives `log()` calls. This
//! is the path the security-plugin-chain docs show.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#![cfg(feature = "security")]

use zerodds_dcps::qos::DomainParticipantQos;
use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_qos::PropertyQosPolicy;
use zerodds_security_runtime::LogLevel;

#[test]
fn property_driven_fanout_logger_is_built_and_writes() {
    let dir = std::env::temp_dir().join(format!("zerodds-prop-log-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("audit.ndjson");

    let property = PropertyQosPolicy::new()
        .with("dds.sec.log.plugin", "stderr,jsonl")
        .with("org.zerodds.sec.log.level", "Notice")
        .with("org.zerodds.sec.log.jsonl.path", path.to_str().unwrap());

    let cfg = RuntimeConfig::default()
        .with_security_log_properties(&property)
        .expect("valid log properties");
    assert!(
        cfg.security_logger.is_some(),
        "dds.sec.log.* must materialize a security_logger"
    );

    cfg.security_logger.as_ref().unwrap().log(
        LogLevel::Error,
        [0u8; 16],
        "access_control",
        "permission denied",
    );

    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.contains("access_control"), "got: {contents}");
    assert!(contents.contains("permission denied"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn absent_log_properties_leave_logger_unset() {
    let cfg = RuntimeConfig::default()
        .with_security_log_properties(&PropertyQosPolicy::new())
        .unwrap();
    assert!(cfg.security_logger.is_none());
}

#[test]
fn misconfigured_jsonl_is_a_clean_error() {
    // jsonl sink selected but no path property → Err, not panic.
    let property = PropertyQosPolicy::new().with("dds.sec.log.plugin", "jsonl");
    assert!(
        RuntimeConfig::default()
            .with_security_log_properties(&property)
            .is_err()
    );
}

#[test]
fn live_participant_accepts_log_properties_on_qos() {
    // The full auto-wireup path: properties on the participant QoS flow through
    // create_participant → new_with_runtime → with_security_log_properties.
    let dir = std::env::temp_dir().join(format!("zerodds-prop-live-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("live-audit.ndjson");

    let qos = DomainParticipantQos {
        property: PropertyQosPolicy::new()
            .with("dds.sec.log.plugin", "jsonl")
            .with("org.zerodds.sec.log.level", "Informational")
            .with("org.zerodds.sec.log.jsonl.path", path.to_str().unwrap()),
        ..Default::default()
    };

    let factory = zerodds_dcps::DomainParticipantFactory::instance();
    let domain = common::unique_domain();
    let participant = factory
        .create_participant(domain, qos)
        .expect("participant with dds.sec.log.* QoS creates cleanly");
    assert_eq!(participant.domain_id(), domain);
    let _ = std::fs::remove_dir_all(&dir);
}

/// DDS-Security 1.2 §10.6.1: with `distribute` off the builtin
/// `DDS:Logging:DDS_LogTopic` sink creates no DataWriter, so it needs no
/// publisher and logs only to its `log_file`.
#[test]
fn builtin_sink_with_distribute_off_is_file_only() {
    let dir = std::env::temp_dir().join(format!("zerodds-builtin-off-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("security.log");

    let property = PropertyQosPolicy::new()
        .with("dds.sec.log.plugin", "builtin.DDS_LogTopic")
        .with(
            "org.zerodds.sec.log.builtin.DDS_LogTopic.logging_level",
            "WARNING_LEVEL",
        )
        .with(
            "dds.sec.log.builtin.DDS_LogTopic.log_file",
            path.to_str().unwrap(),
        )
        .with("dds.sec.log.builtin.DDS_LogTopic.distribute", "false");

    let cfg = RuntimeConfig::default()
        .with_security_log_properties(&property)
        .expect("a file-only builtin sink needs no DataWriter");
    cfg.security_logger.as_ref().unwrap().log(
        LogLevel::Error,
        [0xAA; 16],
        "auth.handshake.failed",
        "bad certificate",
    );

    let contents = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 1, "only Warning-and-above: {contents}");
    assert!(
        lines[0].contains("auth.handshake.failed"),
        "got: {contents}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Leases a DDS domain no other process on this host holds — the
/// domain number is the UDP port number, so it is a host-wide resource.
#[path = "common/mod.rs"]
mod common;
