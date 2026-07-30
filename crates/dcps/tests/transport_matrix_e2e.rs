// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! Transport-matrix e2e: ZeroDDS-self DCPS pub/sub roundtrip over every
//! wire transport supported by the `user_transport` selector.
//!
//! Discovery (SPDP/SEDP) stays UDPv4 multicast (RTPS 2.5 §9.6.1.4.1);
//! what is tested is that the USER traffic runs over the chosen
//! transport. The runtimes are deliberately given different host_id
//! prefixes so that the same-host SHM path does NOT bind and the sample
//! is guaranteed to travel over the configured `user_transport`.
//!
//! # Platform notes
//!
//! Multicast loopback on macOS is unreliable (SPDP discovery does not
//! see itself). Tests are therefore `#[cfg(target_os = "linux")]`.

#![cfg(all(target_os = "linux", feature = "std"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr
)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserTransportKind, UserWriterConfig,
};
use zerodds_qos::{
    DeadlineQosPolicy, DurabilityKind, LifespanQosPolicy, LivelinessQosPolicy, OwnershipKind,
};
use zerodds_rtps::wire_types::{EntityId, GuidPrefix};

fn wait_for_peers(rt: &Arc<DcpsRuntime>, n: usize, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if rt.discovered_participants().len() >= n {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn wait_for_matched(rt: &Arc<DcpsRuntime>, eid: EntityId, n: usize, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if rt.user_reader_matched_count(eid) >= n {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn wait_for_writer_matched(
    rt: &Arc<DcpsRuntime>,
    eid: EntityId,
    n: usize,
    timeout: Duration,
) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if rt.user_writer_matched_count(eid) >= n {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn make_writer_config(topic: &str) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic.into(),
        type_name: "RawBytes".into(),
        reliable: true,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        latency_budget: Default::default(),
        destination_order: Default::default(),
        lifespan: LifespanQosPolicy::default(),
        liveliness: LivelinessQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        presentation: Default::default(),
        partition: vec![],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_identifier: zerodds_types::TypeIdentifier::None,
        data_representation_offer: None,
    }
}

fn make_reader_config(topic: &str) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic.into(),
        type_name: "RawBytes".into(),
        reliable: true,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        latency_budget: Default::default(),
        destination_order: Default::default(),
        liveliness: LivelinessQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        presentation: Default::default(),
        partition: vec![],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_identifier: zerodds_types::TypeIdentifier::None,
        type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        data_representation_offer: None,
    }
}

/// Distinct host_id (first 4 prefix bytes) → no same-host SHM bind,
/// user traffic is guaranteed to travel over the `user_transport`.
fn distinct_host_prefixes() -> (GuidPrefix, GuidPrefix) {
    let a = GuidPrefix::from_bytes([
        0x5A, 0xE7, 0x0D, 0xD5, 0xAA, 0xBB, 0xCC, 0xDD, 0x11, 0x22, 0x33, 0x44,
    ]);
    let b = GuidPrefix::from_bytes([
        0xC1, 0xC1, 0x0E, 0x00, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC,
    ]);
    assert!(!a.is_same_host(b), "prefixes must have distinct host_id");
    (a, b)
}

/// `true` if IPv6 loopback is available. Some CI containers have IPv6
/// disabled — there the v6 tests skip instead of failing incorrectly on
/// bind_v6.
fn ipv6_loopback_available() -> bool {
    std::net::UdpSocket::bind((std::net::Ipv6Addr::LOCALHOST, 0)).is_ok()
}

/// Generic ZeroDDS-self roundtrip over the given transport.
fn roundtrip_over(transport: UserTransportKind, domain: i32, topic: &str, payload: &[u8]) {
    let (prefix_a, prefix_b) = distinct_host_prefixes();
    let cfg = || RuntimeConfig {
        user_transport: Some(transport),
        ..RuntimeConfig::default()
    };
    let rt_a = Arc::new(DcpsRuntime::start(domain, prefix_a, cfg()).expect("rt_a start"));
    let rt_b = Arc::new(DcpsRuntime::start(domain, prefix_b, cfg()).expect("rt_b start"));

    assert!(
        wait_for_peers(&rt_a, 1, Duration::from_secs(10)),
        "rt_a did not see rt_b via SPDP ({transport:?})"
    );
    assert!(
        wait_for_peers(&rt_b, 1, Duration::from_secs(10)),
        "rt_b did not see rt_a via SPDP ({transport:?})"
    );

    let writer_eid = rt_a
        .register_user_writer(make_writer_config(topic))
        .unwrap();
    let (reader_eid, rx) = rt_b
        .register_user_reader(make_reader_config(topic))
        .unwrap();

    assert!(
        wait_for_matched(&rt_b, reader_eid, 1, Duration::from_secs(10)),
        "reader did not see writer via SEDP ({transport:?})"
    );
    assert!(
        wait_for_writer_matched(&rt_a, writer_eid, 1, Duration::from_secs(10)),
        "writer did not see reader via SEDP ({transport:?})"
    );

    rt_a.write_user_sample(writer_eid, payload.to_vec())
        .unwrap();
    let sample = rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or_else(|_| panic!("no sample received over {transport:?}"));
    match sample {
        zerodds_dcps::runtime::UserSample::Alive { payload: bytes, .. } => {
            assert_eq!(
                bytes.as_slice(),
                payload,
                "Payload-Mismatch ({transport:?})"
            );
        }
        other => panic!(
            "expected Alive sample over {transport:?}, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
}

#[test]
fn e2e_roundtrip_over_udpv4() {
    roundtrip_over(
        UserTransportKind::UdpV4,
        130,
        "MatrixUdpV4",
        b"udpv4-roundtrip-0123456789",
    );
}

#[test]
fn e2e_roundtrip_over_tcpv4() {
    roundtrip_over(
        UserTransportKind::TcpV4,
        131,
        "MatrixTcpV4",
        b"tcpv4-roundtrip-0123456789",
    );
}

#[test]
fn e2e_roundtrip_over_udpv6() {
    if !ipv6_loopback_available() {
        eprintln!("skip e2e_roundtrip_over_udpv6: no IPv6 loopback in this environment");
        return;
    }
    roundtrip_over(
        UserTransportKind::UdpV6,
        132,
        "MatrixUdpV6",
        b"udpv6-roundtrip-0123456789",
    );
}

#[test]
fn e2e_roundtrip_over_tcpv6() {
    if !ipv6_loopback_available() {
        eprintln!("skip e2e_roundtrip_over_tcpv6: no IPv6 loopback in this environment");
        return;
    }
    roundtrip_over(
        UserTransportKind::TcpV6,
        133,
        "MatrixTcpV6",
        b"tcpv6-roundtrip-0123456789",
    );
}

#[cfg(feature = "same-host-uds")]
#[test]
fn e2e_roundtrip_over_uds() {
    roundtrip_over(
        UserTransportKind::Uds,
        134,
        "MatrixUds",
        b"uds-roundtrip-0123456789",
    );
}

#[cfg(feature = "same-host-shm")]
#[test]
fn e2e_roundtrip_over_shm() {
    let _ = std::fs::remove_dir_all("/tmp/zerodds-shm");
    roundtrip_over(
        UserTransportKind::Shm,
        135,
        "MatrixShm",
        b"shm-roundtrip-0123456789",
    );
}
