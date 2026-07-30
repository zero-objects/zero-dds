// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A3 — participant scaling past the 32-localhost landmine.
//!
//! ROS-2 painpoint (ros.discourse #49976): Fast DDS' undocumented
//! `MaxAutoParticipantIndex = 32` localhost limit silently fails the 33rd
//! participant on a host. ZeroDDS has no such magic cap — it probes the
//! deterministic SPDP unicast ports for participant indices `0..120`
//! (RTPS §9.6.1.4.1: `7400 + 250*domain + 10 + 2*pid`) and, past that, degrades
//! gracefully to an ephemeral port (still multicast-discovered) instead of
//! failing silently.
//!
//! This test brings up **40 participants** (> 32) in one process on one host,
//! same domain, and proves (a) every runtime starts (no silent bind failure),
//! (b) discovery scales past 32, and (c) a participant at a high index still
//! exchanges user data.
//!
//! macOS: ignored — multicast loopback is unreliable there (same as the other
//! same-host SPDP e2e tests); runs on Linux / the CI bench.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig, UserWriterConfig};
use zerodds_qos::{
    DeadlineQosPolicy, DurabilityKind, LifespanQosPolicy, LivelinessQosPolicy, OwnershipKind,
};
use zerodds_rtps::wire_types::GuidPrefix;

const N: usize = 40; // > 32 (the Fast DDS landmine)

fn writer_cfg(topic: &str) -> UserWriterConfig {
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

fn reader_cfg(topic: &str) -> UserReaderConfig {
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
        group_data: vec![],
        topic_data: vec![],
        type_identifier: zerodds_types::TypeIdentifier::None,
        type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        data_representation_offer: None,
    }
}

#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn forty_participants_on_one_host_no_landmine() {
    let domain = 91;
    // 40 same-host participants: identical host-id bytes [0..4], unique tail.
    let host = [0x5Au8; 4];
    let mut rts: Vec<Arc<DcpsRuntime>> = Vec::with_capacity(N);
    for i in 0..N {
        let mut p = [0u8; 12];
        p[..4].copy_from_slice(&host);
        p[4] = 0xC0;
        p[10] = (i as u8).wrapping_add(1);
        p[11] = ((i >> 8) as u8).wrapping_add(1);
        let rt = DcpsRuntime::start(domain, GuidPrefix::from_bytes(p), RuntimeConfig::default())
            .unwrap_or_else(|e| {
                panic!("participant {i} failed to start (silent landmine?): {e:?}")
            });
        rts.push(rt);
    }
    assert_eq!(rts.len(), N, "all {N} participants must start");

    // (b) Discovery scales past 32: the first runtime must see > 32 peers.
    let probe = &rts[0];
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut seen = 0;
    while Instant::now() < deadline {
        seen = probe.discovered_participants().len();
        if seen >= N - 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        seen > 32,
        "discovery must scale past the 32 landmine; participant 0 saw only {seen} peers"
    );

    // (c) A high-index participant still exchanges user data: writer on rt[0],
    // reader on rt[N-1].
    let topic = "A3Scaling";
    let w = rts[0].register_user_writer(writer_cfg(topic)).unwrap();
    let (r, rx) = rts[N - 1].register_user_reader(reader_cfg(topic)).unwrap();

    let mdl = Instant::now() + Duration::from_secs(15);
    while Instant::now() < mdl && rts[N - 1].user_reader_matched_count(r) < 1 {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        rts[N - 1].user_reader_matched_count(r) >= 1,
        "reader on the {N}th participant never matched the writer on participant 0"
    );
    // Give the symmetric SEDP match a beat, then publish.
    while Instant::now() < mdl && rts[0].user_writer_matched_count(w) < 1 {
        std::thread::sleep(Duration::from_millis(20));
    }
    let payload = vec![0xA3u8, 0x40, 0x00, 0xFF];
    rts[0].write_user_sample(w, payload).expect("write");

    let mut got = false;
    let rdl = Instant::now() + Duration::from_secs(5);
    while Instant::now() < rdl {
        if rx.try_recv().is_ok() {
            got = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        got,
        "user data did not reach the {N}th participant (comms broken past index 32)"
    );
}
