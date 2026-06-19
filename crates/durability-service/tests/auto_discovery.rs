// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! e2e: the service auto-serves a topic whose writer declares TRANSIENT
//! durability — no explicit `serve()`, no application-side code. Own test
//! binary = isolated process.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::*;
use zerodds_dcps::{
    DomainParticipantFactory, DomainParticipantQos, PublisherQos, RawBytes, TopicQos,
};
use zerodds_durability_service::DurabilityService;
use zerodds_durability_store::DurabilityStore;
use zerodds_durability_store_sqlite::SqliteStore;

#[test]
fn auto_discovery_serves_transient_topic() {
    let domain = 73;
    let topic = "AutoSensor";
    let store: Arc<dyn DurabilityStore> =
        Arc::new(SqliteStore::open_in_memory(keep_all()).unwrap());
    let service = Arc::new(DurabilityService::start(domain, Arc::clone(&store)).unwrap());
    // No explicit serve(): the service auto-serves topics whose writers declare
    // TRANSIENT/PERSISTENT durability.
    service.enable_auto_discovery(keep_all()).unwrap();

    {
        let factory = DomainParticipantFactory::instance();
        let app = factory
            .create_participant(domain, DomainParticipantQos::default())
            .unwrap();
        let publisher = app.create_publisher(PublisherQos::default());
        let topic_h = app
            .create_topic::<RawBytes>(topic, TopicQos::default())
            .unwrap();
        let writer = publisher
            .create_datawriter::<RawBytes>(&topic_h, transient_service_writer_qos())
            .unwrap();
        // Deterministic proof #1: the service auto-discovered + is serving it.
        let deadline = Instant::now() + Duration::from_secs(15);
        while !service
            .served_topics()
            .map(|v| v.iter().any(|t| t == topic))
            .unwrap_or(false)
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(
            service.served_topics().unwrap().iter().any(|t| t == topic),
            "service should have auto-discovered the TRANSIENT topic"
        );
        let _ = writer.wait_for_matched_subscription(1, Duration::from_secs(15));
        for i in 0..3u8 {
            writer.write(&RawBytes::new(vec![i, i, i])).unwrap();
        }
        // Deterministic proof #2: clean ingest of exactly 3.
        assert_eq!(
            wait_for_stored(store.as_ref(), topic, 3, Duration::from_secs(15)),
            3
        );
    } // app writer dropped

    std::thread::sleep(Duration::from_millis(300));
    // The daemon serves the auto-discovered topic to a late-joiner. `>= 3`
    // (not `== 3`): a TRANSIENT app writer carries its own writer-embedded
    // backend, so during the brief overlap the late-joiner may see a sample
    // from both logical sources — an inherent durability-service property, not
    // an auto-discovery defect. Writer-dead exactness is covered by the
    // `writer_dies` test.
    let got = late_joiner_collect(domain, topic, 3, Duration::from_secs(15));
    assert!(
        got.len() >= 3,
        "late-joiner should receive at least 3 via auto-discovery, got {}",
        got.len()
    );

    service.shutdown();
}
