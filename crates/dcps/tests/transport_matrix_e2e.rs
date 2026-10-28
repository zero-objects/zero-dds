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
        ..Default::default()
    }
}

fn make_reader_config(topic: &str) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic.into(),
        type_name: "RawBytes".into(),
        ..Default::default()
    }
}

/// Distinct host_id (prefix octets 2..6) → no same-host SHM bind,
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
        common::unique_domain(),
        "MatrixUdpV4",
        b"udpv4-roundtrip-0123456789",
    );
}

#[test]
fn e2e_roundtrip_over_tcpv4() {
    roundtrip_over(
        UserTransportKind::TcpV4,
        common::unique_domain(),
        "MatrixTcpV4",
        b"tcpv4-roundtrip-0123456789",
    );
}

/// Two writers in runtimes of their own reach one reader over TCPv4. The
/// reader's listener read one connection until it closed and left the
/// second unread, and the second writer's send waited for ever on its
/// handshake.
#[test]
fn e2e_two_tcpv4_writers_reach_one_reader() {
    let domain = common::unique_domain();
    let (prefix_r, prefix_w1) = distinct_host_prefixes();
    let prefix_w2 = GuidPrefix::from_bytes([
        0x7B, 0x3E, 0x51, 0x09, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80,
    ]);
    let cfg = || RuntimeConfig {
        user_transport: Some(UserTransportKind::TcpV4),
        ..RuntimeConfig::default()
    };
    let rt_r = Arc::new(DcpsRuntime::start(domain, prefix_r, cfg()).expect("reader start"));
    let rt_w1 = Arc::new(DcpsRuntime::start(domain, prefix_w1, cfg()).expect("w1 start"));
    let rt_w2 = Arc::new(DcpsRuntime::start(domain, prefix_w2, cfg()).expect("w2 start"));
    assert!(wait_for_peers(&rt_r, 2, Duration::from_secs(10)));

    let topic = "MatrixTcpV4TwoWriters";
    let (reader_eid, rx) = rt_r
        .register_user_reader(make_reader_config(topic))
        .unwrap();
    let w1 = rt_w1
        .register_user_writer(make_writer_config(topic))
        .unwrap();
    let w2 = rt_w2
        .register_user_writer(make_writer_config(topic))
        .unwrap();
    assert!(wait_for_matched(
        &rt_r,
        reader_eid,
        2,
        Duration::from_secs(10)
    ));
    assert!(wait_for_writer_matched(
        &rt_w1,
        w1,
        1,
        Duration::from_secs(10)
    ));
    assert!(wait_for_writer_matched(
        &rt_w2,
        w2,
        1,
        Duration::from_secs(10)
    ));

    rt_w1.write_user_sample(w1, b"from-w1".to_vec()).unwrap();
    rt_w2.write_user_sample(w2, b"from-w2".to_vec()).unwrap();
    let mut got = Vec::new();
    while got.len() < 2 {
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(zerodds_dcps::runtime::UserSample::Alive { payload, .. }) => {
                got.push(payload.as_slice().to_vec());
            }
            Ok(_) => {}
            Err(_) => panic!("the reader got {} of 2 samples", got.len()),
        }
    }
    got.sort();
    assert_eq!(got, [b"from-w1".to_vec(), b"from-w2".to_vec()]);

    for rt in [&rt_w1, &rt_w2, &rt_r] {
        rt.shutdown();
    }
}

/// `shutdown` of a TCPv4 participant returns while its peer is alive and
/// connected. Its TCP accept worker read the peer's connection without a
/// timeout, and the join waited until the peer went away.
#[test]
fn e2e_tcpv4_shutdown_returns_while_the_peer_is_connected() {
    let domain = common::unique_domain();
    let (prefix_a, prefix_b) = distinct_host_prefixes();
    let cfg = || RuntimeConfig {
        user_transport: Some(UserTransportKind::TcpV4),
        ..RuntimeConfig::default()
    };
    let rt_a = Arc::new(DcpsRuntime::start(domain, prefix_a, cfg()).expect("rt_a start"));
    let rt_b = Arc::new(DcpsRuntime::start(domain, prefix_b, cfg()).expect("rt_b start"));
    assert!(wait_for_peers(&rt_a, 1, Duration::from_secs(10)));

    let topic = "MatrixTcpV4Shutdown";
    let writer_eid = rt_a
        .register_user_writer(make_writer_config(topic))
        .unwrap();
    let (reader_eid, rx) = rt_b
        .register_user_reader(make_reader_config(topic))
        .unwrap();
    assert!(wait_for_matched(
        &rt_b,
        reader_eid,
        1,
        Duration::from_secs(10)
    ));
    assert!(wait_for_writer_matched(
        &rt_a,
        writer_eid,
        1,
        Duration::from_secs(10)
    ));
    rt_a.write_user_sample(writer_eid, b"connect".to_vec())
        .unwrap();
    rx.recv_timeout(Duration::from_secs(5))
        .expect("a sample over TCPv4");

    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let rt = Arc::clone(&rt_a);
    std::thread::spawn(move || {
        let start = Instant::now();
        rt.shutdown();
        let _ = done_tx.send(start.elapsed());
    });
    let took = done_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("shutdown returns while the peer is connected");
    assert!(took < Duration::from_secs(2), "shutdown took {took:?}");
    rt_b.shutdown();
}

#[test]
fn e2e_roundtrip_over_udpv6() {
    if !ipv6_loopback_available() {
        eprintln!("skip e2e_roundtrip_over_udpv6: no IPv6 loopback in this environment");
        return;
    }
    roundtrip_over(
        UserTransportKind::UdpV6,
        common::unique_domain(),
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
        common::unique_domain(),
        "MatrixTcpV6",
        b"tcpv6-roundtrip-0123456789",
    );
}

#[cfg(feature = "same-host-uds")]
#[test]
fn e2e_roundtrip_over_uds() {
    roundtrip_over(
        UserTransportKind::Uds,
        common::unique_domain(),
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
        common::unique_domain(),
        "MatrixShm",
        b"shm-roundtrip-0123456789",
    );
}

/// Leases a DDS domain no other process on this host holds — the
/// domain number is the UDP port number, so it is a host-wide resource.
#[path = "common/mod.rs"]
mod common;
