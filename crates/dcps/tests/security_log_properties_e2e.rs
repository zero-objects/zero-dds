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
        .with("dds.sec.log.level", "Notice")
        .with("dds.sec.log.jsonl.path", path.to_str().unwrap());

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
            .with("dds.sec.log.level", "Informational")
            .with("dds.sec.log.jsonl.path", path.to_str().unwrap()),
        ..Default::default()
    };

    let factory = zerodds_dcps::DomainParticipantFactory::instance();
    let participant = factory
        .create_participant(0, qos)
        .expect("participant with dds.sec.log.* QoS creates cleanly");
    assert_eq!(participant.domain_id(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}
