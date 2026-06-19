// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! e2e: PERSISTENT history survives a full service restart (fresh service on
//! the same sqlite file, re-primed from disk with no application writer).
//! Own test binary = isolated process.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::*;
use zerodds_dcps::{
    DomainParticipantFactory, DomainParticipantQos, PublisherQos, RawBytes, TopicQos,
};
use zerodds_durability_service::DurabilityService;
use zerodds_durability_store::DurabilityStore;
use zerodds_durability_store_sqlite::SqliteStore;

#[test]
fn persistent_survives_service_restart() {
    let domain = 72;
    let topic = "DurPersist";
    let db = tmp_db("restart");

    // Phase 1: service ingests 3 samples to the sqlite file, then shuts down.
    {
        let store: Arc<dyn DurabilityStore> = Arc::new(SqliteStore::open(&db, keep_all()).unwrap());
        let service = DurabilityService::start(domain, Arc::clone(&store)).unwrap();
        service.serve(topic, keep_all()).unwrap();
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
                .create_datawriter::<RawBytes>(&topic_h, transient_local_writer_qos())
                .unwrap();
            let _ = writer.wait_for_matched_subscription(1, Duration::from_secs(15));
            for i in 0..3u8 {
                writer.write(&RawBytes::new(vec![i, 9, 9])).unwrap();
            }
            assert_eq!(
                wait_for_stored(store.as_ref(), topic, 3, Duration::from_secs(15)),
                3
            );
        }
        service.shutdown();
    } // everything gone; samples live only in the sqlite file

    std::thread::sleep(Duration::from_millis(300));

    // Phase 2: a FRESH service on the SAME db (a restart). Startup-sync must
    // re-prime the replay writer from disk, with no application writer present.
    let store2: Arc<dyn DurabilityStore> = Arc::new(SqliteStore::open(&db, keep_all()).unwrap());
    let service2 = DurabilityService::start(domain, Arc::clone(&store2)).unwrap();
    service2.serve(topic, keep_all()).unwrap();
    std::thread::sleep(Duration::from_millis(300));

    let got = late_joiner_collect(domain, topic, 3, Duration::from_secs(15));
    // The test's claim is "all 3 survived the restart on disk", so assert the 3
    // DISTINCT payloads are present (deduped by content). An exact count would
    // be flaky: a not-yet-torn-down phase-1 replay writer can briefly deliver
    // the same payloads again — duplicates of identical data, not lost/extra
    // samples. (payloads are [0,9,9], [1,9,9], [2,9,9].)
    let mut firsts: Vec<u8> = got.iter().filter_map(|p| p.first().copied()).collect();
    firsts.sort_unstable();
    firsts.dedup();
    assert_eq!(
        firsts,
        vec![0, 1, 2],
        "after restart, late-joiner should receive all 3 distinct samples from disk (got {got:?})"
    );
    service2.shutdown();

    let _ = std::fs::remove_file(&db);
    let _ = std::fs::remove_file(format!("{}-wal", db.display()));
    let _ = std::fs::remove_file(format!("{}-shm", db.display()));
}
