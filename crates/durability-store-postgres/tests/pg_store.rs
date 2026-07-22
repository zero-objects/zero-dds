// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! PostgreSQL-adapter tests: contract enforcement, pagination, idempotent
//! re-send, unregister/cleanup, and a 10k-sensor cold-tier load run.
//!
//! These need a live PostgreSQL. Set `ZERODDS_PG_TEST_URL` to a libpq
//! connection string (e.g. `postgres://zerodds@localhost/zerodds`) to run them;
//! without it every test logs a skip and passes, so a PG-less CI is green.
//! A throwaway database is expected — the tests create tables and use unique
//! topic names per run to stay isolated on a shared server.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

use std::time::{Duration, SystemTime};

use zerodds_durability_store::{
    Contract, DurabilitySample, DurabilityStore, Selector, TieredStore,
};
use zerodds_durability_store_postgres::PostgresStore;
use zerodds_qos::policies::history::HistoryKind;
use zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED;

/// Returns the store + a run-unique topic prefix, or `None` when no test DB is
/// configured (caller then skips).
fn store_or_skip(tag: &str) -> Option<(PostgresStore, String)> {
    let Ok(url) = std::env::var("ZERODDS_PG_TEST_URL") else {
        eprintln!("skip {tag}: ZERODDS_PG_TEST_URL not set");
        return None;
    };
    let store = PostgresStore::connect(&url, Contract::keep_all()).expect("connect");
    // Unique topic per run so parallel tests / repeated runs never collide.
    let topic = format!("t/{}/{tag}/{}", std::process::id(), nanos());
    Some((store, topic))
}

fn nanos() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

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
        representation: 1,
        big_endian: false,
        created_at: SystemTime::UNIX_EPOCH + Duration::from_secs(seq),
        source_guid: [inst; 16],
        source_sequence: seq as i64,
    }
}

#[test]
fn roundtrip_and_ordering() {
    let Some((store, topic)) = store_or_skip("roundtrip") else {
        return;
    };
    store.set_contract(&topic, keep_all()).unwrap();
    // Two instances, interleaved sequences.
    for seq in 0..3 {
        store.store(sample(&topic, 1, seq)).unwrap();
        store.store(sample(&topic, 2, seq)).unwrap();
    }
    let page = store.query(&topic, &Selector::default()).unwrap();
    assert_eq!(page.samples.len(), 6);
    // Ordered by (instance_key, sequence).
    let keys: Vec<(u8, u64)> = page
        .samples
        .iter()
        .map(|s| (s.instance_key[0], s.sequence))
        .collect();
    assert_eq!(keys, vec![(1, 0), (1, 1), (1, 2), (2, 0), (2, 1), (2, 2)]);
    // Payload + metadata survive the wire.
    assert_eq!(page.samples[0].payload, b"p-1-0");
    assert_eq!(page.samples[0].source_sequence, 0);
}

#[test]
fn idempotent_resend() {
    let Some((store, topic)) = store_or_skip("resend") else {
        return;
    };
    store.set_contract(&topic, keep_all()).unwrap();
    store.store(sample(&topic, 7, 42)).unwrap();
    // Same identity, twice more — must not grow the store.
    store.store(sample(&topic, 7, 42)).unwrap();
    store.store(sample(&topic, 7, 42)).unwrap();
    assert_eq!(store.stats(&topic).unwrap().samples, 1);
}

#[test]
fn pagination_cursor() {
    let Some((store, topic)) = store_or_skip("paging") else {
        return;
    };
    store.set_contract(&topic, keep_all()).unwrap();
    for seq in 0..10 {
        store.store(sample(&topic, 1, seq)).unwrap();
    }
    let mut sel = Selector {
        limit: Some(4),
        ..Selector::default()
    };
    let mut seen = 0;
    loop {
        let page = store.query(&topic, &sel).unwrap();
        seen += page.samples.len();
        match page.next {
            Some(c) if !page.samples.is_empty() => sel = sel.after_cursor(c),
            _ => break,
        }
    }
    assert_eq!(seen, 10);
}

#[test]
fn keep_all_max_samples_cap() {
    let Some((store, topic)) = store_or_skip("cap") else {
        return;
    };
    let mut c = keep_all();
    c.max_samples = 3;
    store.set_contract(&topic, c).unwrap();
    for seq in 0..3 {
        store.store(sample(&topic, 1, seq)).unwrap();
    }
    // The 4th (new identity) breaches the cap.
    let err = store.store(sample(&topic, 1, 99)).unwrap_err();
    assert!(matches!(
        err,
        zerodds_durability_store::StoreError::OutOfResources("max_samples")
    ));
    // A re-send of an existing sample is still accepted (grows nothing).
    store.store(sample(&topic, 1, 0)).unwrap();
    assert_eq!(store.stats(&topic).unwrap().samples, 3);
}

#[test]
fn keep_last_trims_per_instance() {
    let Some((store, topic)) = store_or_skip("keeplast") else {
        return;
    };
    let mut c = keep_all();
    c.history_kind = HistoryKind::KeepLast;
    c.history_depth = 2;
    store.set_contract(&topic, c).unwrap();
    for seq in 0..5 {
        store.store(sample(&topic, 1, seq)).unwrap();
    }
    let page = store.query(&topic, &Selector::default()).unwrap();
    let seqs: Vec<u64> = page.samples.iter().map(|s| s.sequence).collect();
    assert_eq!(seqs, vec![3, 4], "only the newest 2 survive");
}

#[test]
fn unregister_then_cleanup_purges() {
    let Some((store, topic)) = store_or_skip("cleanup") else {
        return;
    };
    store.set_contract(&topic, keep_all()).unwrap();
    store.store(sample(&topic, 1, 0)).unwrap();
    store.store(sample(&topic, 2, 0)).unwrap();
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    store.unregister(&topic, &[1u8; 16], now).unwrap();
    // cleanup_delay is zero → instance 1's samples purge now.
    let removed = store.cleanup(now).unwrap();
    assert_eq!(removed, 1);
    assert_eq!(store.stats(&topic).unwrap().instances, 1);
}

#[test]
fn stats_counts() {
    let Some((store, topic)) = store_or_skip("stats") else {
        return;
    };
    store.set_contract(&topic, keep_all()).unwrap();
    store.store(sample(&topic, 1, 0)).unwrap();
    store.store(sample(&topic, 1, 1)).unwrap();
    store.store(sample(&topic, 2, 0)).unwrap();
    let s = store.stats(&topic).unwrap();
    assert_eq!(s.samples, 3);
    assert_eq!(s.instances, 2);
    assert!(s.bytes > 0);
}

/// The PostgreSQL leg of the O11 fleet load: 10k sensors through a RAM hot
/// cache over the shared cold database.
#[test]
fn fleet_of_10k_sensors_postgres() {
    let Some((cold, topic)) = store_or_skip("load10k") else {
        return;
    };
    const SENSORS: usize = 10_000;
    let store = TieredStore::new(cold, 512 * 1024);
    store.set_contract(&topic, keep_all()).unwrap();
    for sensor in 0..SENSORS {
        let mut k = [0u8; 16];
        k[..8].copy_from_slice(&(sensor as u64).to_le_bytes());
        store
            .store(DurabilitySample {
                topic: topic.clone(),
                instance_key: k,
                sequence: 0,
                payload: format!("sensor-{sensor}").into_bytes(),
                representation: 1,
                big_endian: false,
                created_at: SystemTime::UNIX_EPOCH,
                source_guid: k,
                source_sequence: 0,
            })
            .unwrap();
    }
    let stats = store.stats(&topic).unwrap();
    assert_eq!(stats.instances, SENSORS);
    assert_eq!(stats.samples, SENSORS);
    assert!(store.hot_bytes(&topic).unwrap() <= 512 * 1024 + 256);
}
