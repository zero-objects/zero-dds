// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! e2e: the daemon ingests a writer whose application type-name is NOT the
//! native `zerodds::RawBytes` and replays it to a late-joiner on the same
//! foreign type-name. A foreign-vendor (Cyclone/FastDDS/OpenDDS) `TRANSIENT`
//! writer presents on the wire exactly as a runtime-level writer with a custom
//! type-name does under `use_xtypes=no` (match keyed on topic + type-name), so
//! this in-process scenario proves the cross-vendor INGEST capability without a
//! foreign binary. The real-vendor wire confirmation runs on codepit
//! (internal/interop/durability-cross-vendor-ingest-followup.md).
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
use zerodds_qos::{
    DeadlineQosPolicy, LifespanQosPolicy, LivelinessKind, LivelinessQosPolicy, OwnershipKind,
};

const FOREIGN_TYPE: &str = "Foreign::Sensor";

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

// A "foreign vendor" writer: TRANSIENT_LOCAL so a late-matching ingest reader
// still backfills, type-name != `zerodds::RawBytes`.
fn foreign_writer_cfg(topic: &str) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic.to_string(),
        type_name: FOREIGN_TYPE.to_string(),
        reliable: true,
        durability: DurabilityKind::TransientLocal,
        deadline: DeadlineQosPolicy::default(),
        lifespan: LifespanQosPolicy::default(),
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        type_identifier: zerodds_types::TypeIdentifier::None,
        data_representation_offer: None,
    }
}

// Late-joiner reader on the foreign type-name; TRANSIENT_LOCAL so it pulls the
// daemon's replay history.
fn foreign_reader_cfg(topic: &str) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic.to_string(),
        type_name: FOREIGN_TYPE.to_string(),
        reliable: true,
        durability: DurabilityKind::TransientLocal,
        deadline: DeadlineQosPolicy::default(),
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ownership: OwnershipKind::Shared,
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        type_identifier: zerodds_types::TypeIdentifier::None,
        type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        data_representation_offer: None,
    }
}

#[test]
fn daemon_ingests_foreign_type_name_and_replays() {
    let domain = 73;
    let topic = "ForeignVendorTopic";
    let store: Arc<dyn DurabilityStore> =
        Arc::new(SqliteStore::open_in_memory(keep_all()).unwrap());
    let service = DurabilityService::start(domain, Arc::clone(&store)).unwrap();

    // The daemon serves the topic under the FOREIGN type-name (as auto-discovery
    // would from a discovered DCPSPublication). The typed RawBytes path
    // (`serve`) is locked to `zerodds::RawBytes` and would never match this.
    service
        .serve_typed(topic, FOREIGN_TYPE, false, keep_all())
        .unwrap();

    let payloads: Vec<Vec<u8>> = (0..3u8).map(|i| vec![i, i + 10, i + 20]).collect();

    {
        let factory = DomainParticipantFactory::instance();
        let app = factory
            .create_participant(domain, DomainParticipantQos::default())
            .unwrap();
        let rt = app.runtime().expect("participant runtime").clone();
        let weid = rt
            .register_user_writer_kind(foreign_writer_cfg(topic), false)
            .unwrap();

        // Wait until the daemon's ingest reader has matched our foreign writer,
        // so its VOLATILE reader does not miss the writes (mirrors the typed
        // `wait_for_matched_subscription`).
        let deadline = Instant::now() + Duration::from_secs(15);
        while rt.user_writer_matched_count(weid) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            rt.user_writer_matched_count(weid) >= 1,
            "daemon ingest reader should match the foreign-typed writer"
        );

        for p in &payloads {
            rt.write_user_sample_borrowed(weid, p).unwrap();
        }

        // The daemon must have ingested all three under the foreign type-name.
        let got = wait_for_stored(store.as_ref(), topic, 3, Duration::from_secs(15));
        assert_eq!(got, 3, "daemon should ingest 3 foreign-typed samples");
    } // foreign participant dropped → original writer gone

    std::thread::sleep(Duration::from_millis(300));

    // Late-joiner on the SAME foreign type-name pulls the daemon's replay.
    let got = late_joiner_collect_foreign(domain, topic, 3, Duration::from_secs(15));
    assert_eq!(
        got.len(),
        3,
        "late-joiner should receive 3 replayed foreign-typed samples"
    );
    let mut firsts: Vec<u8> = got.iter().map(|p| p[0]).collect();
    firsts.sort_unstable();
    assert_eq!(firsts, vec![0, 1, 2], "payloads must round-trip intact");

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
            Ok(UserSample::Lifecycle { .. }) => {}
            Err(_) => {}
        }
    }
    out
}
