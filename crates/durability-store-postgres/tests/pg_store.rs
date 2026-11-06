// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The PostgreSQL adapter against the retention of DDS 1.4 §2.2.3.5, the
//! PERSISTENT guarantee of §2.2.3.4 (the data outlive the connection that
//! stored them) and the invariants of ADR 0009, each test on a PostgreSQL
//! server of its own (`support::Server`).
//!
//! Ignored in the crate tier, which has no PostgreSQL: the integration
//! pillar's job `integration-services-vendor` runs them.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::time::{Duration, SystemTime};

use zerodds_durability_store::{
    Contract, DurabilitySample, DurabilityStore, Selector, StoreError, TieredStore, TopicMeta,
};
use zerodds_durability_store_postgres::PostgresStore;
use zerodds_qos::policies::history::HistoryKind;
use zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED;

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

fn sample(topic: &str, inst: u8, seq: u64) -> DurabilitySample {
    DurabilitySample {
        topic: topic.to_string(),
        instance_key: [inst; 16],
        sequence: seq,
        payload: format!("p-{inst}-{seq}").into_bytes(),
        representation_identifier: [0x00, 0x07],
        created_at: SystemTime::UNIX_EPOCH + Duration::from_secs(seq),
        source_guid: [inst; 16],
        source_sequence: seq as i64,
    }
}

fn sequences(store: &dyn DurabilityStore, topic: &str) -> Vec<u64> {
    store
        .replay_for_topic(topic)
        .unwrap()
        .iter()
        .map(|s| s.sequence)
        .collect()
}

/// What one connection stored, a second connection reads back: every field
/// of every sample, among them a `D_CDR2_BE` identifier, in
/// `(instance, sequence)` order, and the topic metadata.
#[test]
#[ignore = "integration pillar, job integration-services-vendor: a PostgreSQL server of its own"]
fn dcps_2_2_3_4_persistent_samples_and_topic_meta_outlive_the_connection() {
    let server = support::Server::start();
    let mut written = Vec::new();
    for seq in 0..3 {
        written.push(sample("T", 1, seq));
        written.push(sample("T", 2, seq));
    }
    written[4].representation_identifier = [0x00, 0x08];
    let meta = TopicMeta {
        type_name: "demo::Command".to_string(),
        keyed: true,
        contract: keep_all(),
    };
    {
        let store = PostgresStore::connect(&server.url(), keep_all()).unwrap();
        assert_eq!(store.topic_meta("T").unwrap(), None);
        store.set_topic_meta("T", meta.clone()).unwrap();
        for s in &written {
            store.store(s.clone()).unwrap();
        }
    }
    written.sort_by_key(|s| (s.instance_key, s.sequence));
    let reconnected = PostgresStore::connect(&server.url(), keep_all()).unwrap();
    assert_eq!(reconnected.replay_for_topic("T").unwrap(), written);
    assert_eq!(reconnected.topic_meta("T").unwrap(), Some(meta));
}

/// Under KEEP_ALL a sample past `max_samples` is refused (DDS 1.4 §2.2.3.5
/// with §2.2.3.19: OUT_OF_RESOURCES); the same sample sent again is not a
/// new one.
#[test]
#[ignore = "integration pillar, job integration-services-vendor: a PostgreSQL server of its own"]
fn dcps_2_2_3_5_keep_all_refuses_a_sample_past_max_samples() {
    let server = support::Server::start();
    let store = PostgresStore::connect(&server.url(), keep_all()).unwrap();
    let mut c = keep_all();
    c.max_samples = 3;
    store.set_contract("T", c).unwrap();
    for seq in 0..3 {
        store.store(sample("T", 1, seq)).unwrap();
    }
    assert!(matches!(
        store.store(sample("T", 1, 99)),
        Err(StoreError::OutOfResources("max_samples"))
    ));
    store.store(sample("T", 1, 0)).unwrap();
    store.store(sample("T", 1, 0)).unwrap();
    assert_eq!(sequences(&store, "T"), vec![0, 1, 2]);
}

#[test]
#[ignore = "integration pillar, job integration-services-vendor: a PostgreSQL server of its own"]
fn dcps_2_2_3_5_keep_last_keeps_the_newest_depth_samples_of_each_instance() {
    let server = support::Server::start();
    let store = PostgresStore::connect(&server.url(), keep_all()).unwrap();
    let mut c = keep_all();
    c.history_kind = HistoryKind::KeepLast;
    c.history_depth = 2;
    store.set_contract("T", c).unwrap();
    for seq in 0..5 {
        store.store(sample("T", 1, seq)).unwrap();
        store.store(sample("T", 2, seq)).unwrap();
    }
    assert_eq!(sequences(&store, "T"), vec![3, 4, 3, 4]);
}

/// `service_cleanup_delay`: the samples of an instance without writers go
/// once the delay has passed, not before; the delay of the topic metadata
/// governs over an earlier plain `set_contract`.
#[test]
#[ignore = "integration pillar, job integration-services-vendor: a PostgreSQL server of its own"]
fn dcps_2_2_3_5_an_unregistered_instance_is_purged_after_the_cleanup_delay() {
    let server = support::Server::start();
    let store = PostgresStore::connect(&server.url(), keep_all()).unwrap();
    store.set_contract("T", keep_all()).unwrap();
    store
        .set_topic_meta(
            "T",
            TopicMeta {
                type_name: "T".to_string(),
                keyed: true,
                contract: Contract {
                    cleanup_delay: Duration::from_secs(10),
                    ..keep_all()
                },
            },
        )
        .unwrap();
    store.store(sample("T", 1, 0)).unwrap();
    store.store(sample("T", 2, 0)).unwrap();
    let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    store.unregister("T", &[1u8; 16], t0).unwrap();
    assert_eq!(store.cleanup(t0 + Duration::from_secs(5)).unwrap(), 0);
    assert_eq!(store.cleanup(t0 + Duration::from_secs(11)).unwrap(), 1);
    let left: Vec<u8> = store
        .replay_for_topic("T")
        .unwrap()
        .iter()
        .map(|s| s.instance_key[0])
        .collect();
    assert_eq!(left, vec![2]);
}

/// ADR 0009, invariant 3: a read is paginated, in `(instance, sequence)`
/// order.
#[test]
#[ignore = "integration pillar, job integration-services-vendor: a PostgreSQL server of its own"]
fn adr_0009_3_a_query_pages_through_the_history_in_order() {
    let server = support::Server::start();
    let store = PostgresStore::connect(&server.url(), keep_all()).unwrap();
    store.set_contract("T", keep_all()).unwrap();
    for inst in 1..=2u8 {
        for seq in 0..5 {
            store.store(sample("T", inst, seq)).unwrap();
        }
    }
    let mut got = Vec::new();
    let mut sel = Selector {
        limit: Some(4),
        ..Selector::default()
    };
    loop {
        let page = store.query("T", &sel).unwrap();
        assert!(page.samples.len() <= 4);
        got.extend(page.samples.iter().map(|s| (s.instance_key[0], s.sequence)));
        match page.next {
            Some(c) if !page.samples.is_empty() => sel = sel.after_cursor(c),
            _ => break,
        }
    }
    let want: Vec<(u8, u64)> = (1..=2u8)
        .flat_map(|i| (0..5).map(move |s| (i, s)))
        .collect();
    assert_eq!(got, want);
}

/// ADR 0009, invariant 2, the PostgreSQL leg of the fleet: ten thousand
/// instances through a RAM hot cache of 512 KiB over the database; the cache
/// stays in its budget, the database holds every sample.
#[test]
#[ignore = "integration pillar, job integration-services-vendor: a PostgreSQL server of its own"]
fn adr_0009_2_ten_thousand_instances_stay_in_the_hot_budget_and_whole_in_postgresql() {
    const SENSORS: usize = 10_000;
    let server = support::Server::start();
    let cold = PostgresStore::connect(&server.url(), keep_all()).unwrap();
    let store = TieredStore::new(cold, 512 * 1024);
    store.set_contract("T", keep_all()).unwrap();
    for sensor in 0..SENSORS {
        let mut k = [0u8; 16];
        k[..8].copy_from_slice(&(sensor as u64).to_le_bytes());
        store
            .store(DurabilitySample {
                topic: "T".to_string(),
                instance_key: k,
                sequence: 0,
                payload: format!("sensor-{sensor}").into_bytes(),
                representation_identifier: [0x00, 0x07],
                created_at: SystemTime::UNIX_EPOCH,
                source_guid: k,
                source_sequence: 0,
            })
            .unwrap();
    }
    let stats = store.stats("T").unwrap();
    assert_eq!((stats.instances, stats.samples), (SENSORS, SENSORS));
    assert!(store.hot_bytes("T").unwrap() <= 512 * 1024 + 256);
}
