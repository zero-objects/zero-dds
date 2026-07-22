// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Lakehouse-adapter tests: contract enforcement, pagination, restart, and the
//! Parquet lake export.
//!
//! The whole crate is gated behind the optional `duckdb` feature
//! (`#![cfg(feature = "duckdb")]` in lib.rs); mirror that here so a default
//! `cargo test --workspace` (duckdb OFF) compiles this to an empty test target
//! instead of failing on the absent `LakehouseStore`.

#![cfg(feature = "duckdb")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, SystemTime};

use zerodds_durability_store::{Contract, DurabilitySample, DurabilityStore, Selector};
use zerodds_durability_store_lakehouse::LakehouseStore;
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
        representation: 1,
        big_endian: false,
        created_at: SystemTime::UNIX_EPOCH + Duration::from_secs(seq),
        source_guid: [0u8; 16],
        source_sequence: -1,
    }
}

fn tmp(tag: &str, ext: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    p.push(format!("zerodds-lakehouse-{tag}-{nanos}.{ext}"));
    p
}

#[test]
fn keep_last_trims_to_depth() {
    let store = LakehouseStore::open_in_memory(keep_all()).unwrap();
    let c = Contract {
        history_kind: HistoryKind::KeepLast,
        history_depth: 3,
        ..keep_all()
    };
    store.set_contract("T", c).unwrap();
    for seq in 0..10 {
        store.store(sample("T", 1, seq)).unwrap();
    }
    let seqs: Vec<u64> = store
        .replay_for_topic("T")
        .unwrap()
        .iter()
        .map(|s| s.sequence)
        .collect();
    assert_eq!(seqs, vec![7, 8, 9]);
}

#[test]
fn caps_and_idempotent() {
    let store = LakehouseStore::open_in_memory(keep_all()).unwrap();
    let c = Contract {
        max_samples_per_instance: 2,
        ..keep_all()
    };
    store.set_contract("T", c).unwrap();
    assert!(store.store(sample("T", 1, 0)).is_ok());
    assert!(store.store(sample("T", 1, 1)).is_ok());
    assert!(matches!(
        store.store(sample("T", 1, 2)).unwrap_err(),
        zerodds_durability_store::StoreError::OutOfResources(_)
    ));
    // Re-send is idempotent (PK upsert).
    store.store(sample("T", 1, 0)).unwrap();
    assert_eq!(store.stats("T").unwrap().samples, 2);
}

#[test]
fn pagination_and_selector() {
    let store = LakehouseStore::open_in_memory(keep_all()).unwrap();
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
        let n = page.samples.len();
        got.extend(page.samples.iter().map(|s| (s.instance_key[0], s.sequence)));
        match page.next {
            Some(c) if n > 0 => sel = sel.after_cursor(c),
            _ => break,
        }
    }
    assert_eq!(got.len(), 10);
    assert!(got.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn persistent_survives_reopen() {
    let db = tmp("reopen", "duckdb");
    {
        let store = LakehouseStore::open(&db, keep_all()).unwrap();
        store.set_contract("S", keep_all()).unwrap();
        for seq in 0..6 {
            store.store(sample("S", 1, seq)).unwrap();
        }
    } // dropped → DuckDB file persists
    let reopened = LakehouseStore::open(&db, keep_all()).unwrap();
    let all = reopened.replay_for_topic("S").unwrap();
    assert_eq!(all.len(), 6);
    assert_eq!(all[5].sequence, 5);
    assert_eq!(all[2].payload, b"p-1-2");
    let _ = std::fs::remove_file(&db);
}

#[test]
fn parquet_lake_export() {
    let store = LakehouseStore::open_in_memory(keep_all()).unwrap();
    store.set_contract("T", keep_all()).unwrap();
    for seq in 0..5 {
        store.store(sample("T", 7, seq)).unwrap();
    }
    let parquet = tmp("export", "parquet");
    store.export_parquet("T", &parquet).unwrap();
    // The Parquet file exists and is non-empty (schema-on-read lake artifact).
    let meta = std::fs::metadata(&parquet).unwrap();
    assert!(meta.len() > 0, "parquet export should be non-empty");
    let _ = std::fs::remove_file(&parquet);
}
