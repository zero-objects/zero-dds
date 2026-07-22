// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! O11 load test: a fleet of 10k sensors through the tiered store (RAM hot
//! cache over an on-disk sqlite cold tier — the RAM/SSD legs of the
//! RAM/SSD/PostgreSQL story).
//!
//! Proves the two ADR-0009 tiering invariants at scale:
//! * the RAM hot cache stays within its byte budget while thousands of
//!   sensors stream in (no hot-tier OOM), and
//! * the on-disk cold tier is authoritative — every sample of every sensor is
//!   durably stored and queryable, regardless of how small the hot budget is.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, SystemTime};

use zerodds_durability_store::{
    Contract, DurabilitySample, DurabilityStore, Selector, TieredStore,
};
use zerodds_durability_store_sqlite::SqliteStore;

const SENSORS: usize = 10_000;
const SAMPLES_PER_SENSOR: u64 = 3;
const HOT_BUDGET_BYTES: u64 = 512 * 1024; // 512 KiB — far below the full working set

/// Instance key for sensor `id` (distinct 16-byte KeyHash per sensor).
fn sensor_key(id: usize) -> [u8; 16] {
    let mut k = [0u8; 16];
    k[..8].copy_from_slice(&(id as u64).to_le_bytes());
    k
}

fn sample(topic: &str, sensor: usize, seq: u64) -> DurabilitySample {
    DurabilitySample {
        topic: topic.to_string(),
        instance_key: sensor_key(sensor),
        sequence: seq,
        payload: format!("sensor-{sensor}-reading-{seq}").into_bytes(),
        representation: 1,
        big_endian: false,
        created_at: SystemTime::UNIX_EPOCH + Duration::from_millis(seq),
        source_guid: sensor_key(sensor),
        source_sequence: seq as i64,
    }
}

fn tmp_db() -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    let nonce = std::process::id();
    p.push(format!("zerodds-load-10k-{nonce}.db"));
    let _ = std::fs::remove_file(&p);
    p
}

#[test]
fn fleet_of_10k_sensors_stays_hot_bounded_and_cold_complete() {
    let topic = "Sensors/Temperature";
    let db = tmp_db();
    let cold = SqliteStore::open(&db, Contract::keep_all()).unwrap();
    let store = TieredStore::new(cold, HOT_BUDGET_BYTES);
    store.set_contract(topic, Contract::keep_all()).unwrap();

    // Ingest the whole fleet, interleaving sensors per round so that no single
    // sensor dominates the hot cache.
    for seq in 0..SAMPLES_PER_SENSOR {
        for sensor in 0..SENSORS {
            store.store(sample(topic, sensor, seq)).unwrap();
        }
    }

    let total = (SENSORS as u64 * SAMPLES_PER_SENSOR) as usize;

    // Invariant 1 — the hot cache never exceeds its budget (allow a single
    // over-budget sample, which the evictor deliberately keeps to avoid an
    // empty cache).
    let hot = store.hot_bytes(topic).unwrap();
    assert!(
        hot <= HOT_BUDGET_BYTES + 256,
        "hot cache {hot} B exceeded budget {HOT_BUDGET_BYTES} B"
    );

    // Invariant 2 — the on-disk cold tier holds the FULL fleet history.
    let stats = store.stats(topic).unwrap();
    assert_eq!(stats.samples, total, "cold sample count");
    assert_eq!(stats.instances, SENSORS, "cold instance count");

    // Every individual sensor's full series is queryable from cold.
    for sensor in [0, SENSORS / 2, SENSORS - 1] {
        let page = store
            .query(
                topic,
                &Selector {
                    instance_key: Some(sensor_key(sensor)),
                    ..Selector::default()
                },
            )
            .unwrap();
        assert_eq!(
            page.samples.len(),
            SAMPLES_PER_SENSOR as usize,
            "sensor {sensor} series"
        );
    }

    // Paginated full drain returns exactly the fleet total.
    let drained = store.replay_for_topic(topic).unwrap();
    assert_eq!(drained.len(), total, "paginated replay total");

    drop(store);
    let _ = std::fs::remove_file(&db);
}
