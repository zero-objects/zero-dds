// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! e2e: a KEYED TRANSIENT topic served through `serve_typed` — two instances,
//! `KEEP_LAST(2)` per instance, one instance disposed. Proves the fixes for
//! the "always unkeyed" defect: `keyed` is derived from the discovered
//! writer's RTPS entityKind (not hardcoded `false`), samples are stored and
//! replayed **per instance** (not one shared fake instance), and a
//! dispose/unregister marker reaches the store (`unregister`) and the replay
//! writer (`write_user_lifecycle`) instead of being dropped.
//!
//! Own test binary = isolated process (process-global DDS state).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{UserReaderConfig, UserSample, UserWriterConfig};
use zerodds_dcps::{DomainParticipantFactory, DomainParticipantQos};
use zerodds_durability_service::DurabilityService;
use zerodds_durability_store::{Contract, DurabilityStore, Selector};
use zerodds_durability_store_sqlite::SqliteStore;
use zerodds_qos::policies::durability::DurabilityKind;
use zerodds_qos::policies::history::HistoryKind;
use zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED;
use zerodds_qos::{LivelinessKind, LivelinessQosPolicy};

const KEYED_TYPE: &str = "Keyed::Reading";

fn keep_last_2() -> Contract {
    Contract {
        history_kind: HistoryKind::KeepLast,
        history_depth: 2,
        max_samples: LENGTH_UNLIMITED,
        max_instances: LENGTH_UNLIMITED,
        max_samples_per_instance: LENGTH_UNLIMITED,
        cleanup_delay: Duration::ZERO,
    }
}

fn keyed_writer_cfg(topic: &str) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic.to_string(),
        type_name: KEYED_TYPE.to_string(),
        durability: DurabilityKind::TransientLocal,
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn keyed_reader_cfg(topic: &str) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic.to_string(),
        type_name: KEYED_TYPE.to_string(),
        durability: DurabilityKind::TransientLocal,
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn instance_a() -> [u8; 16] {
    let mut k = [0u8; 16];
    k[0] = 0xAA;
    k
}

fn instance_b() -> [u8; 16] {
    let mut k = [0u8; 16];
    k[0] = 0xBB;
    k
}

#[test]
fn keyed_topic_keeps_last_2_per_instance_and_disposes() {
    let domain = zerodds_test_domains::unique_domain();
    let topic = "KeyedReadings";
    let store: Arc<dyn DurabilityStore> =
        Arc::new(SqliteStore::open_in_memory(keep_last_2()).unwrap());
    let service = DurabilityService::start(domain, Arc::clone(&store)).unwrap();

    // `keyed = true`: served explicitly, as `enable_auto_discovery` would
    // resolve it from the writer's entityKind (§9.3.1.2).
    service
        .serve_typed(topic, KEYED_TYPE, true, keep_last_2())
        .unwrap();

    {
        let factory = DomainParticipantFactory::instance();
        let app = factory
            .create_participant(domain, DomainParticipantQos::default())
            .unwrap();
        let rt = app.runtime().expect("participant runtime").clone();
        let weid = rt
            .register_user_writer_kind(keyed_writer_cfg(topic), true)
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(15);
        while rt.user_writer_matched_count(weid) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            rt.user_writer_matched_count(weid) >= 1,
            "daemon ingest reader should match the keyed writer"
        );

        // Instance A: 3 samples — KEEP_LAST(2) must retain only the last 2.
        for i in 0..3u8 {
            rt.write_user_sample_keyed(weid, &[b'A', i], instance_a(), None)
                .unwrap();
        }
        // Instance B: 1 sample, then dispose it.
        rt.write_user_sample_keyed(weid, &[b'B', 0], instance_b(), None)
            .unwrap();
        rt.write_user_lifecycle(
            weid,
            instance_b(),
            None,
            zerodds_rtps::inline_qos::status_info::DISPOSED,
            None,
        )
        .unwrap();

        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let n = store.stats(topic).map(|s| s.samples).unwrap_or(0);
            // 2 retained of instance A (KEEP_LAST(2) dropped the oldest) + 1
            // of instance B = 3, once ingest has caught up.
            if n >= 3 || Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    } // app participant dropped -> original writer gone

    std::thread::sleep(Duration::from_millis(300));

    // The store itself: exactly 2 samples of instance A survive (KEEP_LAST(2)
    // per instance, not a single shared "instance 0" — the pre-fix behavior).
    let a_samples = store
        .query(
            topic,
            &Selector {
                instance_key: Some(instance_a()),
                ..Selector::default()
            },
        )
        .unwrap()
        .samples;
    assert_eq!(
        a_samples.len(),
        2,
        "KEEP_LAST(2) must retain exactly 2 samples of instance A, got {a_samples:?}"
    );
    let firsts: Vec<u8> = a_samples.iter().map(|s| s.payload[1]).collect();
    assert_eq!(
        firsts,
        vec![1, 2],
        "the oldest of the 3 (index 0) must be the one KEEP_LAST(2) dropped"
    );

    // Dispose reaches the store (previously dropped unconditionally): with
    // `cleanup_delay: ZERO`, instance B is purge-eligible the instant
    // `cleanup` runs. Calling it directly (not waiting for the daemon's own
    // 30 s sweep) proves `unregister` was actually called for it.
    let purged = store.cleanup(std::time::SystemTime::now()).unwrap();
    assert_eq!(
        purged, 1,
        "disposing instance B must make it purge-eligible"
    );
    let b_samples = store
        .query(
            topic,
            &Selector {
                instance_key: Some(instance_b()),
                ..Selector::default()
            },
        )
        .unwrap()
        .samples;
    assert!(
        b_samples.is_empty(),
        "instance B's sample must be purged after cleanup, got {b_samples:?}"
    );

    // A late joiner sees instance A's retained history on the keyed topic.
    let got = late_joiner_collect_keyed(domain, topic, 2, Duration::from_secs(15));
    let mut got_indices: Vec<u8> = got.iter().filter(|p| p[0] == b'A').map(|p| p[1]).collect();
    got_indices.sort_unstable();
    assert_eq!(
        got_indices,
        vec![1, 2],
        "late joiner should replay exactly the 2 KEEP_LAST(2)-retained samples of instance A, once each"
    );

    service.shutdown();
}

fn late_joiner_collect_keyed(
    domain: i32,
    topic: &str,
    expect: usize,
    timeout: Duration,
) -> Vec<Vec<u8>> {
    let factory = DomainParticipantFactory::instance();
    let participant = factory
        .create_participant(domain, DomainParticipantQos::default())
        .unwrap();
    let rt = participant.runtime().expect("participant runtime").clone();
    let (_reid, rx) = rt
        .register_user_reader_kind(keyed_reader_cfg(topic), true)
        .unwrap();

    // Collects until `expect` arrived AND a short quiet period follows (not
    // just "stop at the first `expect`"): the writer's history-cache trim
    // can still be draining its outbound batch when the reader matches, so
    // an early stop could record pre-trim samples and miss the ones the
    // trim was still delivering.
    let mut out = Vec::new();
    let deadline = Instant::now() + timeout;
    let mut quiet_since: Option<Instant> = None;
    let quiet_window = Duration::from_millis(500);
    loop {
        if Instant::now() >= deadline {
            break;
        }
        if out.len() >= expect {
            match quiet_since {
                Some(since) if since.elapsed() >= quiet_window => break,
                Some(_) => {}
                None => quiet_since = Some(Instant::now()),
            }
        }
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(UserSample::Alive { payload, .. }) => {
                out.push(payload.to_vec());
                quiet_since = None;
            }
            Ok(UserSample::Lifecycle { .. }) | Ok(UserSample::CoherentSetEnd { .. }) => {}
            Err(_) => {}
        }
    }
    out
}
