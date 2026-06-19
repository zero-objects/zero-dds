// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! e2e: the standalone daemon serves history to a late-joiner AFTER the
//! original writer is gone. Own test binary = isolated process.

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
fn late_joiner_gets_history_after_writer_dies() {
    let domain = 71;
    let topic = "DurSensor";
    let store: Arc<dyn DurabilityStore> =
        Arc::new(SqliteStore::open_in_memory(keep_all()).unwrap());
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
            writer.write(&RawBytes::new(vec![i, i, i])).unwrap();
        }
        assert_eq!(
            wait_for_stored(store.as_ref(), topic, 3, Duration::from_secs(15)),
            3,
            "daemon should have ingested 3 samples"
        );
    } // app participant dropped → original writer is gone

    std::thread::sleep(Duration::from_millis(300));

    let got = late_joiner_collect(domain, topic, 3, Duration::from_secs(15));
    // The daemon's guarantee is "the 3 distinct samples survive the original
    // writer's death and are served". A not-yet-torn-down original writer can
    // briefly re-deliver the same transient_local history (duplicates of
    // identical data, not extra/lost samples), so assert the 3 DISTINCT
    // payloads — an exact count would be flaky (mirrors the restart.rs note).
    let mut firsts: Vec<u8> = got.iter().filter_map(|p| p.first().copied()).collect();
    firsts.sort_unstable();
    firsts.dedup();
    assert_eq!(
        firsts,
        vec![0, 1, 2],
        "late-joiner should receive all 3 distinct history samples from the daemon (got {got:?})"
    );

    service.shutdown();
}
