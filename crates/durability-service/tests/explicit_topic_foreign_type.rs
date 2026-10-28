// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! e2e: the explicit `--topic NAME` path (`serve_discovered`) resolves a
//! writer's real application type name and keyedness from discovery, instead
//! of locking the topic to `zerodds::RawBytes`. Previously an explicitly
//! named topic was always registered as a native `Topic<RawBytes>`, which
//! never matches a writer of any other type name under `use_xtypes=no`
//! (match keyed on topic_name + type_name) — every foreign-vendor writer, and
//! every ZeroDDS writer of an actual application type.
//!
//! Own test binary = isolated process (process-global DDS state).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{UserReaderConfig, UserSample, UserWriterConfig};
use zerodds_dcps::{DomainParticipantFactory, DomainParticipantQos};
use zerodds_durability_service::DurabilityService;
use zerodds_durability_store::{Contract, DurabilityStore};
use zerodds_durability_store_sqlite::SqliteStore;
use zerodds_qos::policies::durability::DurabilityKind;
use zerodds_qos::policies::history::HistoryKind;
use zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED;
use zerodds_qos::{LivelinessKind, LivelinessQosPolicy};

const FOREIGN_TYPE: &str = "Explicit::Foreign::Type";

fn keep_all() -> Contract {
    Contract {
        history_kind: HistoryKind::KeepAll,
        history_depth: 0,
        max_samples: LENGTH_UNLIMITED,
        max_instances: LENGTH_UNLIMITED,
        max_samples_per_instance: LENGTH_UNLIMITED,
        cleanup_delay: Duration::ZERO,
    }
}

fn foreign_writer_cfg(topic: &str) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic.to_string(),
        type_name: FOREIGN_TYPE.to_string(),
        durability: DurabilityKind::TransientLocal,
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn foreign_reader_cfg(topic: &str) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic.to_string(),
        type_name: FOREIGN_TYPE.to_string(),
        durability: DurabilityKind::TransientLocal,
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn explicit_topic_resolves_foreign_type_via_discovery() {
    let domain = zerodds_test_domains::unique_domain();
    let topic = "ExplicitForeignTopic";
    let store: Arc<dyn DurabilityStore> =
        Arc::new(SqliteStore::open_in_memory(keep_all()).unwrap());
    let service = Arc::new(DurabilityService::start(domain, Arc::clone(&store)).unwrap());

    // This is what `--topic ExplicitForeignTopic` (no `type`/`keyed` config
    // override) resolves to in the daemon binary: the operator names the
    // topic, the type comes from whichever writer of it shows up.
    service.serve_discovered(topic, keep_all()).unwrap();

    // Nothing served yet — no writer has appeared.
    assert!(service.served_topics().unwrap().iter().all(|t| t != topic));

    let payloads: Vec<Vec<u8>> = (0..3u8).map(|i| vec![i, i + 100]).collect();
    {
        let factory = DomainParticipantFactory::instance();
        let app = factory
            .create_participant(domain, DomainParticipantQos::default())
            .unwrap();
        let rt = app.runtime().expect("participant runtime").clone();
        let weid = rt
            .register_user_writer_kind(foreign_writer_cfg(topic), false)
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(15);
        while rt.user_writer_matched_count(weid) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            rt.user_writer_matched_count(weid) >= 1,
            "the daemon should have resolved the foreign type and matched this writer"
        );

        for p in &payloads {
            rt.write_user_sample_borrowed(weid, p).unwrap();
        }
        let got = wait_for_stored(store.as_ref(), topic, 3, Duration::from_secs(15));
        assert_eq!(got, 3, "daemon should ingest 3 samples of the foreign type");
    } // foreign participant dropped -> original writer gone

    std::thread::sleep(Duration::from_millis(300));

    let got = late_joiner_collect_foreign(domain, topic, 3, Duration::from_secs(15));
    assert_eq!(
        got.len(),
        3,
        "late-joiner should receive 3 replayed samples of the resolved foreign type"
    );

    service.shutdown();
}

fn wait_for_stored(store: &dyn DurabilityStore, topic: &str, n: usize, timeout: Duration) -> usize {
    let deadline = Instant::now() + timeout;
    loop {
        let got = store.stats(topic).map(|s| s.samples).unwrap_or(0);
        if got >= n || Instant::now() >= deadline {
            return got;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn late_joiner_collect_foreign(
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
        .register_user_reader_kind(foreign_reader_cfg(topic), false)
        .unwrap();

    let mut out = Vec::new();
    let deadline = Instant::now() + timeout;
    while out.len() < expect && Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(UserSample::Alive { payload, .. }) => out.push(payload.to_vec()),
            Ok(UserSample::Lifecycle { .. }) | Ok(UserSample::CoherentSetEnd { .. }) => {}
            Err(_) => {}
        }
    }
    out
}
