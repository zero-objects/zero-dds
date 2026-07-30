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
//! Before 2026-05-19 we only had unit tests of the SameHostTracker and
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
use zerodds_qos::{
    DeadlineQosPolicy, DurabilityKind, LifespanQosPolicy, LivelinessQosPolicy, OwnershipKind,
};
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

/// Baseline: 2 DcpsRuntime in the same process, UDP path. Must **always**
/// work — without this test we cannot distinguish between "DCPS broken"
/// and "same-host-shm broken".
#[test]
fn e2e_two_runtimes_udp_roundtrip() {
    let domain = 117;
    let prefix_a = GuidPrefix::from_bytes([0xA1; 12]);
    let prefix_b = {
        let mut p = [0xB2; 12];
        // host-id bytes [0..4] equal → is_same_host = true
        p[..4].copy_from_slice(&prefix_a.to_bytes()[..4]);
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

/// Wave 4 — same-host SHM path. Active only with the
/// `same-host-shm` feature. The test should produce the **same**
/// sample roundtrip as the UDP test, plus activate the SHM path.
///
/// Bug-1 (2026-05-19): this test uncovers the problem that the
/// sample roundtrip does not go through when the feature is active.
#[cfg(feature = "same-host-shm")]
#[test]
fn e2e_two_runtimes_shm_roundtrip() {
    // Clean up existing SHM files from the previous run.
    let _ = std::fs::remove_dir_all("/tmp/zerodds-shm");

    let domain = 118;
    let prefix_a = GuidPrefix::from_bytes([0xA3; 12]);
    let prefix_b = {
        let mut p = [0xB4; 12];
        p[..4].copy_from_slice(&prefix_a.to_bytes()[..4]);
        GuidPrefix::from_bytes(p)
    };
    let rt_a = Arc::new(
        DcpsRuntime::start(domain, prefix_a, RuntimeConfig::default()).expect("rt_a start"),
    );
    let rt_b = Arc::new(
        DcpsRuntime::start(domain, prefix_b, RuntimeConfig::default()).expect("rt_b start"),
    );
    assert!(wait_for_peers(&rt_a, 1, Duration::from_secs(10)));
    assert!(wait_for_peers(&rt_b, 1, Duration::from_secs(10)));

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

    // Single write — no retry, no sleep. The SHM bind is already done
    // via the idempotent open_or_create in the SEDP hook.
    let payload = b"shm-roundtrip-9876543210";
    rt_a.write_user_sample(writer_eid, payload.to_vec())
        .unwrap();
    let sample = rx
        .recv_timeout(Duration::from_secs(2))
        .expect("SHM path: a single write must suffice (no retry needed)");
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

/// SPDP restart test: start and drop the first DcpsRuntime, then the
/// second one. If DcpsRuntime::shutdown does not clean up the multicast
/// memberships properly, the second runtime cannot receive SPDP
/// multicast (see Bug-2 from 2026-05-19).
#[test]
fn spdp_works_after_runtime_drop() {
    let domain = 119;
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
        p[..4].copy_from_slice(&prefix.to_bytes()[..4]);
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
/// (4-byte prefix) → NO SHM bind, the UDP path stays active.
///
/// Background: GuidPrefix.host_id() are the first 4 bytes. ZeroDDS
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
    use zerodds_dcps::same_host::SameHostState;

    let _ = std::fs::remove_dir_all("/tmp/zerodds-shm");

    let domain = 120;
    // host_id A: FNV1a("zerodds-host") — looks like ZeroDDS style
    let prefix_a = GuidPrefix::from_bytes([
        0x5A, 0xE7, 0x0D, 0xD5, // host_id A
        0xAA, 0xBB, 0xCC, 0xDD, // pid bytes
        0x11, 0x22, 0x33, 0x44, // counter+random
    ]);
    // host_id B: simulates Cyclone (IP-based, different value) — KEY:
    // these 4 bytes DIFFER from host_id A.
    let prefix_b = GuidPrefix::from_bytes([
        0xC1, 0xC1, 0x0E, 0x00, // host_id B — different!
        0x55, 0x66, 0x77, 0x88, // pid bytes
        0x99, 0xAA, 0xBB, 0xCC, // counter+random
    ]);
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

    // Core check: the same_host state must have NO Bound entry for
    // this pair. This guarantees that UDP remains the only
    // path — no SHM samples into wrong segments.
    let snapshot_a = rt_a.same_host.snapshot();
    for (_, _, state) in &snapshot_a {
        assert!(
            !matches!(state, SameHostState::Bound { .. }),
            "rt_a must have no bound state with a cross-host-id peer"
        );
    }
    let snapshot_b = rt_b.same_host.snapshot();
    for (_, _, state) in &snapshot_b {
        assert!(
            !matches!(state, SameHostState::Bound { .. }),
            "rt_b must have no bound state with a cross-host-id peer"
        );
    }

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
