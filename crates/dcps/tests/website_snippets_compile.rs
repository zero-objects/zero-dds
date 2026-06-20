// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Compile + smoke test for the exact code shown in the website docs for the
//! three capability gaps that were just implemented. If a doc snippet's API
//! drifts from the codebase, this test stops compiling — the snippets are
//! guaranteed to be real.

#![cfg(feature = "security")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

/// `topics/transport-tuning` — Gap 1: preference-ordered multi-transport.
/// Recipe "TCP for WAN / firewall traversal" uses only the default TCP
/// transport (verbatim as published — a single kind, no UDP fallback).
#[test]
fn transport_tuning_snippet() {
    use zerodds_dcps::runtime::{RuntimeConfig, UserTransportKind};

    let cfg = RuntimeConfig {
        // Preference order: first kind that matches a peer locator wins.
        user_transports: vec![
            UserTransportKind::TcpV4, // TCP for WAN / firewall traversal
        ],
        ..Default::default()
    };

    assert_eq!(cfg.user_transports.len(), 1);
}

/// `topics/transport-tuning` SHM/UDS recipes (SHM fast-path, UDS sidecar,
/// SHM+UDS+UDP failover). `UserTransportKind::Shm`/`Uds` are feature-gated
/// (`same-host-shm` / `same-host-uds`) — the website snippets therefore
/// carry a Cargo feature hint, and this test compiles them only when both
/// features are on, matching that hint. The transport lists are the exact
/// published recipes (verbatim, verified against zerodds-examples/rust-transport-tuning).
#[cfg(all(feature = "same-host-shm", feature = "same-host-uds"))]
#[test]
fn transport_tuning_shm_uds_snippets() {
    use zerodds_dcps::runtime::{RuntimeConfig, UserTransportKind};

    // Recipe — SHM for same-host peers (fast path), UDP fallback.
    let shm = RuntimeConfig {
        user_transports: vec![UserTransportKind::Shm, UserTransportKind::UdpV4],
        ..Default::default()
    };
    assert_eq!(shm.user_transports.len(), 2);

    // Recipe — UDS for container sidecars.
    let uds = RuntimeConfig {
        user_transports: vec![UserTransportKind::Uds],
        ..Default::default()
    };
    assert_eq!(uds.user_transports.len(), 1);

    // Recipe — SHM + UDS + UDP failover (same-host fast path, container
    // sidecars, cross-host fallback — three kinds, verbatim).
    let multi = RuntimeConfig {
        user_transports: vec![
            UserTransportKind::Shm,
            UserTransportKind::Uds,
            UserTransportKind::UdpV4,
        ],
        ..Default::default()
    };
    assert_eq!(multi.user_transports.len(), 3);
}

/// `topics/audit-logging` — Gap 2: SecurityBundle fan-out logger.
#[test]
fn audit_logging_snippet() {
    use zerodds_security::logging::LogLevel;
    use zerodds_security_logging::{FanOutLoggingPlugin, StderrLoggingPlugin};
    use zerodds_security_runtime::SecurityBundle;

    let stderr_sink = StderrLoggingPlugin::with_level(LogLevel::Warning);
    let fanout = FanOutLoggingPlugin::new().with(stderr_sink);

    let security_bundle = SecurityBundle::builder()
        .logging_plugin(Box::new(fanout))
        .build();

    assert!(security_bundle.has_logging());

    // And it wires into a RuntimeConfig (the andocking step).
    let cfg =
        zerodds_dcps::runtime::RuntimeConfig::default().with_security_bundle(&security_bundle);
    assert!(cfg.security_logger.is_some());
}

/// `topics/security-plugin-chain` — Gap 3: property-driven logger wireup.
#[test]
fn security_plugin_chain_snippet() {
    use zerodds_dcps::qos::DomainParticipantQos;
    use zerodds_qos::PropertyQosPolicy;

    let qos = DomainParticipantQos {
        property: PropertyQosPolicy::new()
            .with("dds.sec.log.plugin", "stderr,jsonl") // fan out to both
            .with("dds.sec.log.level", "Notice") // min level
            .with("dds.sec.log.jsonl.path", "/var/log/zerodds-audit.jsonl"),
        ..Default::default()
    };

    assert_eq!(qos.property.get("dds.sec.log.plugin"), Some("stderr,jsonl"));
    assert_eq!(qos.property.get("dds.sec.log.level"), Some("Notice"));
}
