// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! End-to-end regression for the **type-following** monitor path
//! (`zerodds-spy` / `zerodds-record`).
//!
//! The reported bug: a monitor that announces `type_name = "zerodds::RawBytes"`
//! never matches a typed writer (e.g. `cuas::Track`), because DDS matches
//! reader↔writer by type-name equality on BOTH ends (SEDP). The fix is
//! type-following: discover the writer's real `type_name` and announce THAT.
//!
//! This proves, across two real DcpsRuntime on loopback SEDP:
//!   1. discovery surfaces the writer as `(topic, "cuas::Track")`
//!      — the data source `TypeFollower` polls;
//!   2. a reader announcing `cuas::Track` matches + receives the samples
//!      (the fix);
//!   3. a reader announcing `zerodds::RawBytes` does NOT match
//!      (the original bug — guarded so it can't silently come back).
//!
//! Linux-only: macOS multicast loopback is unreliable for SPDP (see
//! `same_host_e2e.rs`). Run on codepit.

#![cfg(all(target_os = "linux", feature = "std"))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_qos::{
    DeadlineQosPolicy, DurabilityKind, LifespanQosPolicy, LivelinessQosPolicy, OwnershipKind,
};
use zerodds_rtps::wire_types::{EntityId, GuidPrefix};

const TOPIC: &str = "Track";
const TYPED: &str = "cuas::Track";
const RAWBYTES: &str = "zerodds::RawBytes";

fn writer_config(topic: &str, type_name: &str) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic.into(),
        type_name: type_name.into(),
        reliable: true,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        lifespan: LifespanQosPolicy::default(),
        liveliness: LivelinessQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        partition: vec![],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_identifier: zerodds_types::TypeIdentifier::None,
        data_representation_offer: None,
    }
}

/// Mirrors `zerodds_cli_common::raw_reader_config_typed` (kept inline to avoid a
/// dev-dependency cycle: cli-common depends on dcps).
fn reader_config(topic: &str, type_name: &str) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic.into(),
        type_name: type_name.into(),
        reliable: true,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        liveliness: LivelinessQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        partition: vec![],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_identifier: zerodds_types::TypeIdentifier::None,
        type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        data_representation_offer: None,
    }
}

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

fn wait_for_reader_matched(
    rt: &Arc<DcpsRuntime>,
    eid: EntityId,
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

fn wait_for_discovered_type(
    rt: &Arc<DcpsRuntime>,
    topic: &str,
    type_name: &str,
    timeout: Duration,
) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if rt
            .discovered_publication_topics()
            .iter()
            .any(|(t, ty)| t == topic && ty == type_name)
        {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn two_runtimes(domain: i32) -> (Arc<DcpsRuntime>, Arc<DcpsRuntime>) {
    let prefix_a = GuidPrefix::from_bytes([0xA1; 12]);
    let prefix_b = {
        let mut p = [0xB2; 12];
        p[..4].copy_from_slice(&prefix_a.to_bytes()[..4]); // same host-id
        GuidPrefix::from_bytes(p)
    };
    let rt_a = DcpsRuntime::start(domain, prefix_a, RuntimeConfig::default()).expect("rt_a");
    let rt_b = DcpsRuntime::start(domain, prefix_b, RuntimeConfig::default()).expect("rt_b");
    assert!(
        wait_for_peers(&rt_a, 1, Duration::from_secs(10)),
        "rt_a !see rt_b"
    );
    assert!(
        wait_for_peers(&rt_b, 1, Duration::from_secs(10)),
        "rt_b !see rt_a"
    );
    (rt_a, rt_b)
}

/// The fix: a reader announcing the writer's real type follows it end-to-end.
#[test]
fn type_following_reader_matches_typed_writer() {
    let (rt_a, rt_b) = two_runtimes(141);

    let writer_eid = rt_a
        .register_user_writer(writer_config(TOPIC, TYPED))
        .unwrap();

    // (1) discovery surfaces the writer's REAL type — the TypeFollower data source.
    assert!(
        wait_for_discovered_type(&rt_b, TOPIC, TYPED, Duration::from_secs(10)),
        "rt_b discovery never surfaced ({TOPIC}, {TYPED}) — TypeFollower would see nothing"
    );

    // (2) a reader announcing the discovered type matches + receives.
    let (reader_eid, rx) = rt_b
        .register_user_reader(reader_config(TOPIC, TYPED))
        .unwrap();
    assert!(
        wait_for_reader_matched(&rt_b, reader_eid, 1, Duration::from_secs(10)),
        "type-following reader did not match the typed writer via SEDP"
    );

    // Publish a few samples; the follower reader must receive them as opaque bytes.
    let mut received = 0usize;
    for i in 0..5u8 {
        let payload = vec![i; 16 + i as usize];
        rt_a.write_user_sample(writer_eid, payload.clone()).unwrap();
        // Reliable; retry-friendly recv window.
        if let Ok(UserSample::Alive { payload: got, .. }) = rx.recv_timeout(Duration::from_secs(2))
        {
            assert_eq!(
                got.as_slice(),
                payload.as_slice(),
                "opaque payload mismatch"
            );
            received += 1;
        }
    }
    assert_eq!(
        received, 5,
        "type-following reader did not receive all samples"
    );
}

/// The original bug, guarded: a RawBytes reader must NOT match a typed writer.
#[test]
fn rawbytes_reader_does_not_match_typed_writer() {
    let (rt_a, rt_b) = two_runtimes(142);

    let _writer_eid = rt_a
        .register_user_writer(writer_config(TOPIC, TYPED))
        .unwrap();
    assert!(
        wait_for_discovered_type(&rt_b, TOPIC, TYPED, Duration::from_secs(10)),
        "precondition: typed writer must be discovered"
    );

    // A reader announcing RawBytes (the old spy behaviour) must never match the
    // cuas::Track writer — different type_name on both ends.
    let (reader_eid, _rx) = rt_b
        .register_user_reader(reader_config(TOPIC, RAWBYTES))
        .unwrap();
    let matched = wait_for_reader_matched(&rt_b, reader_eid, 1, Duration::from_secs(2));
    assert!(
        !matched,
        "REGRESSION: a RawBytes reader matched a cuas::Track writer — type-name gate is broken"
    );
}
