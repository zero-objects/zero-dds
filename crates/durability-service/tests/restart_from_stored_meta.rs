// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! e2e: a restarted service serves a late joiner from persisted [`TopicMeta`]
//! alone — no explicit `serve`/`serve_typed` call with a known type/contract
//! (which `restart.rs` already covers), and no application writer present.
//! This is the in-process reproduction of the durability-restart bug fix:
//! `zerodds-durability-svc`'s `main` used to resolve an explicitly-configured
//! topic's type only via `serve_discovered`, which waits for a writer's SEDP
//! announcement — a wait that never ends on a restart with nothing left to
//! write. `serve_if_stored` closes that gap by reading the type/keyed/
//! contract the FIRST run already resolved and persisted, and serving
//! immediately.
//!
//! Own test binary = isolated process (see `common/mod.rs`).

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
fn restart_serves_late_joiner_from_stored_meta_alone() {
    // `lease_domain()` (not `unique_domain()`): this test wants the domain
    // released when it drops, and never touches 0–100 — those are the
    // measurement host's (see `zerodds_test_domains::RESERVED_DOMAINS`).
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id();
    let topic = "RestartFromStoredMeta";
    let db = tmp_db("restart-from-stored-meta");

    // Phase 1: a service serves the topic (persisting its TopicMeta as a
    // side effect of `serve`), ingests 4 samples, then "crashes" (shutdown
    // without any special persistence step — the metadata was already
    // written when `serve` was first called, not at shutdown).
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
            for i in 0..4u8 {
                writer.write(&RawBytes::new(vec![i, 7, 7])).unwrap();
            }
            assert_eq!(
                wait_for_stored(store.as_ref(), topic, 4, Duration::from_secs(15)),
                4,
                "daemon should have persisted all 4 samples before the crash"
            );
        }
        service.shutdown();
    } // everything gone; samples + TopicMeta live only in the sqlite file

    std::thread::sleep(Duration::from_millis(300));

    // Phase 2: a FRESH service on the SAME db — a restart. No `serve`/
    // `serve_typed` call with a known type/contract: only `serve_if_stored`,
    // exactly what `zerodds-durability-svc`'s `main` now does for a
    // `durability.topics` entry with no `type`/`contract` override.
    let store2: Arc<dyn DurabilityStore> = Arc::new(SqliteStore::open(&db, keep_all()).unwrap());
    let service2 = DurabilityService::start(domain, Arc::clone(&store2)).unwrap();
    let served = service2.serve_if_stored(topic, None).unwrap();
    assert!(
        served,
        "serve_if_stored must find the TopicMeta the first run persisted"
    );
    std::thread::sleep(Duration::from_millis(300));

    // A late joiner created only AFTER the restart, with no application
    // writer anywhere in this phase, must still receive all 4 samples.
    let got = late_joiner_collect(domain, topic, 4, Duration::from_secs(15));
    let mut firsts: Vec<u8> = got.iter().filter_map(|p| p.first().copied()).collect();
    firsts.sort_unstable();
    firsts.dedup();
    assert_eq!(
        firsts,
        vec![0, 1, 2, 3],
        "after restart, a late joiner should receive all 4 distinct samples \
         from a service served ONLY via serve_if_stored (got {got:?})"
    );

    service2.shutdown();
    let _ = std::fs::remove_file(&db);
    let _ = std::fs::remove_file(format!("{}-wal", db.display()));
    let _ = std::fs::remove_file(format!("{}-shm", db.display()));
}
