// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! End-to-end tests for wave 4 (Spec `zerodds-zero-copy-1.0` §6):
//! two DcpsRuntime in the same process, same domain, writer sends a
//! sample, reader receives it. Validates both the UDP baseline path
//! and the feature-gated SHM path.
//!
//! # Why these tests exist
//!
//! Before 2026-05-19 we only had unit tests of the same-host tracker and
//! direct open_owner/open_consumer roundtrips in same_host_shm.rs.
//! None of those tests triggered the DCPS hot path with two
//! DcpsRuntime + SEDP match + write_user_sample. Bug-1 ("sample
//! disappears despite a successful SEDP match") could therefore never
//! be uncovered by the existing tests.
//!
//! # Platform notes
//!
//! Multicast loopback on macOS is unreliable (SPDP discovery
//! does not see itself). The tests are therefore `#[cfg(target_os = "linux")]`.

#![cfg(all(target_os = "linux", feature = "std"))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_dcps::dds_type::RawBytes;
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig, UserWriterConfig};
use zerodds_rtps::wire_types::GuidPrefix;

/// Waits until the local reader has `n` matched writers.
fn wait_for_matched(
    rt: &Arc<DcpsRuntime>,
    eid: zerodds_rtps::wire_types::EntityId,
    n: usize,
    timeout: Duration,
) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if rt.user_reader_matched_count(eid) >= n {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

/// Waits until the local writer has `n` matched readers.
fn wait_for_writer_matched(
    rt: &Arc<DcpsRuntime>,
    eid: zerodds_rtps::wire_types::EntityId,
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

/// Waits until SPDP discovery sees `n` peers.
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

/// Baseline: 2 DcpsRuntime in the same process, UDP path. Must **always**
/// work — without this test we cannot distinguish between "DCPS broken"
/// and "same-host-shm broken".
#[test]
fn e2e_two_runtimes_udp_roundtrip() {
    let domain = common::unique_domain();
    let prefix_a = GuidPrefix::from_bytes([0xA1; 12]);
    let prefix_b = {
        let mut p = [0xB2; 12];
        // Vendor id and host id, octets 0..6, equal → is_same_host = true
        // (DDSI-RTPS 2.5 §9.3.1.5 puts the vendor id first).
        p[..6].copy_from_slice(&prefix_a.to_bytes()[..6]);
        GuidPrefix::from_bytes(p)
    };
    let rt_a = Arc::new(
        DcpsRuntime::start(domain, prefix_a, RuntimeConfig::default()).expect("rt_a start"),
    );
    let rt_b = Arc::new(
        DcpsRuntime::start(domain, prefix_b, RuntimeConfig::default()).expect("rt_b start"),
    );
    assert!(
        wait_for_peers(&rt_a, 1, Duration::from_secs(10)),
        "rt_a did not see rt_b via SPDP"
    );
    assert!(
        wait_for_peers(&rt_b, 1, Duration::from_secs(10)),
        "rt_b did not see rt_a via SPDP"
    );

    let writer_eid = rt_a
        .register_user_writer(make_writer_config("E2EUdp"))
        .unwrap();
    let (reader_eid, rx) = rt_b
        .register_user_reader(make_reader_config("E2EUdp"))
        .unwrap();
    assert!(
        wait_for_matched(&rt_b, reader_eid, 1, Duration::from_secs(10)),
        "reader did not see writer via SEDP"
    );

    // Wait symmetrically: rt_a must also see rt_b's reader.
    assert!(
        wait_for_writer_matched(&rt_a, writer_eid, 1, Duration::from_secs(10)),
        "writer did not see reader via SEDP (rt_a side asymmetric match)"
    );

    // Single write — the symmetric wait guarantees that both reader_proxy
    // and writer_proxy are in sync. unknown_src_count must stay 0.
    let drops_before = rt_b.user_reader_unknown_src_count(reader_eid);
    let payload = b"udp-roundtrip-1234567890";
    rt_a.write_user_sample(writer_eid, payload.to_vec())
        .unwrap();
    let sample = rx
        .recv_timeout(Duration::from_secs(2))
        .expect("UDP path: a single write must suffice (no retry needed)");
    let drops_after = rt_b.user_reader_unknown_src_count(reader_eid);
    assert_eq!(
        drops_after - drops_before,
        0,
        "unknown_src_count must stay 0 with symmetric wait"
    );
    match sample {
        zerodds_dcps::runtime::UserSample::Alive { payload: bytes, .. } => {
            assert_eq!(bytes.as_slice(), payload);
        }
        other => panic!(
            "expected Alive sample, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
}

/// The prefix of a second participant on the host of `a`: the host-id
/// octets `[2..6]` are what `is_same_host` compares, behind the vendor id.
#[cfg(feature = "same-host-shm")]
fn same_host_as(a: GuidPrefix, fill: u8) -> GuidPrefix {
    let mut p = [fill; 12];
    // The vendor id and the host id, octets 0..6.
    p[..6].copy_from_slice(&a.to_bytes()[..6]);
    GuidPrefix::from_bytes(p)
}

/// Waits until `rt` holds a channel to `peer` whose ring the peer has
/// opened, and returns it.
#[cfg(feature = "same-host-shm")]
fn wait_for_channel(
    rt: &DcpsRuntime,
    peer: GuidPrefix,
    timeout: Duration,
) -> Option<Arc<zerodds_dcps::same_host_channel::PeerChannel>> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some((_, channel)) = rt
            .same_host_channels
            .snapshot()
            .into_iter()
            .find(|(p, c)| *p == peer && c.outbound.peer_attached())
        {
            return Some(channel);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

#[cfg(feature = "same-host-shm")]
fn alive_payload(sample: zerodds_dcps::runtime::UserSample) -> Vec<u8> {
    match sample {
        zerodds_dcps::runtime::UserSample::Alive { payload, .. } => payload.as_slice().to_vec(),
        other => panic!(
            "expected Alive sample, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
}

/// Spec `zerodds-zero-copy-1.0` §6: two same-host participants open one
/// channel each way when SPDP reports the other, and everything unicast
/// between them goes through it: the sample, and the discovery and
/// reliability traffic around it (SEDP, HEARTBEAT, ACKNACK) as metatraffic
/// and user frames. Before, only user DATA went over SHM, through one
/// segment per writer/reader pair.
#[cfg(feature = "same-host-shm")]
#[test]
fn e2e_two_runtimes_shm_roundtrip() {
    use zerodds_dcps::same_host_channel::Port;
    let domain = common::unique_domain();
    let prefix_a = GuidPrefix::from_bytes([0xA3; 12]);
    let prefix_b = same_host_as(prefix_a, 0xB4);
    let rt_a = DcpsRuntime::start(domain, prefix_a, RuntimeConfig::default()).expect("rt_a start");
    let rt_b = DcpsRuntime::start(domain, prefix_b, RuntimeConfig::default()).expect("rt_b start");
    assert!(wait_for_channel(&rt_a, prefix_b, Duration::from_secs(10)).is_some());
    assert!(wait_for_channel(&rt_b, prefix_a, Duration::from_secs(10)).is_some());

    let writer_eid = rt_a
        .register_user_writer(make_writer_config("E2EShm"))
        .unwrap();
    let (reader_eid, rx) = rt_b
        .register_user_reader(make_reader_config("E2EShm"))
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

    let payload = b"shm-roundtrip-9876543210";
    rt_a.write_user_sample(writer_eid, payload.to_vec())
        .unwrap();
    let sample = rx
        .recv_timeout(Duration::from_secs(2))
        .expect("SHM path: a single write must suffice (no retry needed)");
    assert_eq!(alive_payload(sample), payload);
    assert!(
        rt_a.same_host_channels.frames_sent(Port::User) > 0,
        "the sample went through the channel"
    );
    for rt in [&rt_a, &rt_b] {
        assert!(
            rt.same_host_channels.frames_sent(Port::Metatraffic) > 0,
            "SEDP went through the channel"
        );
        assert_eq!(rt.same_host_channels.unknown_ports(), 0);
    }
    rt_a.shutdown();
    rt_b.shutdown();
}

/// `transport.same_host_shm` off: no channel between two same-host
/// participants, and the sample arrives over UDP — the setting a transport
/// comparison and a host shared with another implementation need.
#[cfg(feature = "same-host-shm")]
#[test]
fn e2e_same_host_shm_off_takes_udp() {
    let domain = common::unique_domain();
    let prefix_a = GuidPrefix::from_bytes([0xA5; 12]);
    let prefix_b = same_host_as(prefix_a, 0xB6);
    let off = || RuntimeConfig {
        same_host_shm: false,
        ..RuntimeConfig::default()
    };
    let rt_a = DcpsRuntime::start(domain, prefix_a, off()).expect("rt_a start");
    let rt_b = DcpsRuntime::start(domain, prefix_b, off()).expect("rt_b start");
    assert!(wait_for_peers(&rt_a, 1, Duration::from_secs(10)));
    assert!(wait_for_peers(&rt_b, 1, Duration::from_secs(10)));
    let writer_eid = rt_a
        .register_user_writer(make_writer_config("E2EShmOff"))
        .unwrap();
    let (reader_eid, rx) = rt_b
        .register_user_reader(make_reader_config("E2EShmOff"))
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
    let payload = b"udp-with-shm-off";
    rt_a.write_user_sample(writer_eid, payload.to_vec())
        .unwrap();
    let sample = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the sample arrives over UDP");
    assert_eq!(alive_payload(sample), payload);
    assert!(rt_a.same_host_channels.is_empty());
    assert!(rt_b.same_host_channels.is_empty());
    rt_a.shutdown();
    rt_b.shutdown();
}

/// A datagram the ring cannot take, here one larger than the writer's
/// `shm_max_datagram`, reaches a best-effort reader over UDP.
#[cfg(feature = "same-host-shm")]
#[test]
fn e2e_a_datagram_the_channel_cannot_take_goes_over_udp() {
    let domain = common::unique_domain();
    let prefix_a = GuidPrefix::from_bytes([0xE7; 12]);
    let prefix_b = same_host_as(prefix_a, 0xE8);
    let small_rings = RuntimeConfig {
        shm_max_datagram: 1024,
        ..RuntimeConfig::default()
    };
    let rt_a = DcpsRuntime::start(domain, prefix_a, small_rings).expect("rt_a start");
    let rt_b = DcpsRuntime::start(domain, prefix_b, RuntimeConfig::default()).expect("rt_b start");
    assert!(wait_for_channel(&rt_a, prefix_b, Duration::from_secs(10)).is_some());
    let writer_eid = rt_a
        .register_user_writer(UserWriterConfig {
            reliable: false,
            ..make_writer_config("E2EShmFallback")
        })
        .unwrap();
    let (reader_eid, rx) = rt_b
        .register_user_reader(UserReaderConfig {
            reliable: false,
            ..make_reader_config("E2EShmFallback")
        })
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
    let before = rt_a.same_host_shm_udp_fallbacks();
    let big = vec![0x5a; 4000];
    rt_a.write_user_sample(writer_eid, big.clone()).unwrap();
    let sample = rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the datagram the ring refused arrives over UDP");
    assert_eq!(alive_payload(sample), big);
    assert!(rt_a.same_host_shm_udp_fallbacks() > before);
    rt_a.shutdown();
    rt_b.shutdown();
}

/// A participant that shuts down closes its rings; its peer sees the
/// consumer go and sends to it over UDP again, and `shutdown` leaves no
/// channel and no mapping behind.
#[cfg(feature = "same-host-shm")]
#[test]
fn e2e_shutdown_closes_the_channels_on_both_sides() {
    let domain = common::unique_domain();
    let prefix_a = GuidPrefix::from_bytes([0xE5; 12]);
    let prefix_b = same_host_as(prefix_a, 0xE6);
    let rt_a = DcpsRuntime::start(domain, prefix_a, RuntimeConfig::default()).expect("rt_a start");
    let rt_b = DcpsRuntime::start(domain, prefix_b, RuntimeConfig::default()).expect("rt_b start");
    let to_b = wait_for_channel(&rt_a, prefix_b, Duration::from_secs(10)).expect("a → b");
    let to_a = wait_for_channel(&rt_b, prefix_a, Duration::from_secs(10)).expect("b → a");
    let b_rings = (
        Arc::downgrade(&to_a.outbound),
        Arc::downgrade(&to_a.inbound),
    );
    drop(to_a);
    rt_b.shutdown();
    assert!(rt_b.same_host_channels.is_empty());
    assert!(
        b_rings.0.upgrade().is_none() && b_rings.1.upgrade().is_none(),
        "b's rings are unmapped"
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while to_b.outbound.peer_attached() {
        assert!(Instant::now() < deadline, "a still sees b's consumer");
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(to_b);
    rt_a.shutdown();
    assert!(rt_a.same_host_channels.is_empty());
}

/// With the same-host channels carrying the unicast traffic the sockets
/// are idle, and a shutdown waited out the receive threads' 1 s read
/// timeout (a multi-process test chain: 749–1 003 ms). It wakes them now and returns in a
/// fraction of that.
#[cfg(feature = "same-host-shm")]
#[test]
fn e2e_shutdown_with_idle_sockets_returns_quickly() {
    let domain = common::unique_domain();
    let prefix_a = GuidPrefix::from_bytes([0xE7; 12]);
    let prefix_b = same_host_as(prefix_a, 0xE8);
    let rt_a = DcpsRuntime::start(domain, prefix_a, RuntimeConfig::default()).expect("rt_a start");
    let rt_b = DcpsRuntime::start(domain, prefix_b, RuntimeConfig::default()).expect("rt_b start");
    drop(wait_for_channel(&rt_a, prefix_b, Duration::from_secs(10)).expect("a → b"));
    drop(wait_for_channel(&rt_b, prefix_a, Duration::from_secs(10)).expect("b → a"));
    // Past the first receive timeouts, so each thread waits somewhere in
    // its next second.
    std::thread::sleep(Duration::from_millis(1500));
    let started = Instant::now();
    rt_b.shutdown();
    let took = started.elapsed();
    rt_a.shutdown();
    assert!(
        took < Duration::from_millis(300),
        "shutdown took {took:?} with idle sockets"
    );
}

/// SPDP restart test: start and drop the first DcpsRuntime, then the
/// second one. If DcpsRuntime::shutdown does not clean up the multicast
/// memberships properly, the second runtime cannot receive SPDP
/// multicast (see Bug-2 from 2026-05-19).
#[test]
fn spdp_works_after_runtime_drop() {
    let domain = common::unique_domain();
    let prefix = GuidPrefix::from_bytes([0xC5; 12]);
    // First runtime — start and drop immediately.
    {
        let rt = Arc::new(
            DcpsRuntime::start(domain, prefix, RuntimeConfig::default()).expect("rt1 start"),
        );
        // Wait a bit so the SPDP beacon went out at least once.
        std::thread::sleep(Duration::from_millis(500));
        drop(rt);
    }
    // Short cooldown phase so the drop-cleanup paths take effect.
    std::thread::sleep(Duration::from_millis(500));

    // Second runtime with a peer partner → SPDP must work.
    let rt_a =
        Arc::new(DcpsRuntime::start(domain, prefix, RuntimeConfig::default()).expect("rt_a start"));
    let prefix_b = {
        let mut p = [0xD6; 12];
        p[..6].copy_from_slice(&prefix.to_bytes()[..6]);
        GuidPrefix::from_bytes(p)
    };
    let rt_b = Arc::new(
        DcpsRuntime::start(domain, prefix_b, RuntimeConfig::default()).expect("rt_b start"),
    );
    assert!(
        wait_for_peers(&rt_a, 1, Duration::from_secs(15)),
        "rt_a did not see rt_b after a prior runtime drop — SPDP-Membership-Leak"
    );
    assert!(
        wait_for_peers(&rt_b, 1, Duration::from_secs(15)),
        "rt_b did not see rt_a after a prior runtime drop — SPDP-Membership-Leak"
    );
}

/// Wave-4 same-host detection — negative test: different host_id
/// (four prefix octets) → NO SHM bind, the UDP path stays active.
///
/// Background: GuidPrefix.host_id() are octets 2..6, behind the vendor
/// id DDSI-RTPS 2.5 §9.3.1.5 puts first. ZeroDDS
/// generates host_id as FNV1a("hostname"); Cyclone DDS uses an
/// IP-/MAC-based value. On the same host these collide
/// **practically never** (1/2^32 collision probability).
///
/// This test simulates the cross-vendor scenario by starting two
/// participants with explicitly different host_id prefixes
/// (like ZeroDDS ↔ Cyclone). Expectation:
/// * `is_same_host` returns false
/// * the SHM hook is not triggered
/// * the sample roundtrip works via UDP as usual
///
/// Limitation: this is a simulation. A real Cyclone interop test
/// needs an installed Cyclone — see Task #60 / cyclone-interop
/// followup.
#[cfg(feature = "same-host-shm")]
#[test]
fn e2e_cross_vendor_different_host_id_no_shm_bind() {
    let domain = common::unique_domain();
    // host_id A: FNV1a("zerodds-host") — a ZeroDDS prefix.
    let prefix_a = GuidPrefix::zerodds(
        [0x5A, 0xE7, 0x0D, 0xD5],             // host_id A
        [0xAA, 0xBB, 0xCC, 0xDD, 0x11, 0x22], // pid + counter
    );
    // host_id B: simulates Cyclone (IP-based, different value) — KEY:
    // the four host octets DIFFER from host_id A.
    let prefix_b = GuidPrefix::zerodds(
        [0xC1, 0xC1, 0x0E, 0x00],             // host_id B — different!
        [0x55, 0x66, 0x77, 0x88, 0x99, 0xAA], // pid + counter
    );
    assert!(
        !prefix_a.is_same_host(prefix_b),
        "test precondition: prefixes must have different host_id"
    );

    let rt_a = Arc::new(
        DcpsRuntime::start(domain, prefix_a, RuntimeConfig::default()).expect("rt_a start"),
    );
    let rt_b = Arc::new(
        DcpsRuntime::start(domain, prefix_b, RuntimeConfig::default()).expect("rt_b start"),
    );
    assert!(wait_for_peers(&rt_a, 1, Duration::from_secs(10)));
    assert!(wait_for_peers(&rt_b, 1, Duration::from_secs(10)));

    let writer_eid = rt_a
        .register_user_writer(make_writer_config("CrossVendor"))
        .unwrap();
    let (reader_eid, rx) = rt_b
        .register_user_reader(make_reader_config("CrossVendor"))
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

    // Core check: no channel to a participant with another host id, so
    // UDP stays the only path.
    assert!(rt_a.same_host_channels.is_empty());
    assert!(rt_b.same_host_channels.is_empty());

    // The UDP path must still work.
    let payload = b"cross-vendor-udp-only";
    rt_a.write_user_sample(writer_eid, payload.to_vec())
        .unwrap();
    let sample = rx
        .recv_timeout(Duration::from_secs(2))
        .expect("UDP path must keep working with different host_ids");
    match sample {
        zerodds_dcps::runtime::UserSample::Alive { payload: bytes, .. } => {
            assert_eq!(bytes.as_slice(), payload);
        }
        other => panic!(
            "expected Alive sample, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
}

// Unused-import suppression for non-shm builds.
#[allow(dead_code)]
fn _silence(_: RawBytes) {}

/// Leases a DDS domain no other process on this host holds — the
/// domain number is the UDP port number, so it is a host-wide resource.
#[path = "common/mod.rs"]
mod common;
