// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The lakehouse adapter against the retention of DDS 1.4 §2.2.3.5, the
//! PERSISTENT guarantee of §2.2.3.4, the paginated read of ADR 0009 and its
//! Parquet export, read back by DuckDB's Parquet reader.
//!
//! The whole crate is gated behind the optional `duckdb` feature
//! (`#![cfg(feature = "duckdb")]` in lib.rs); mirror that here so a default
//! `cargo test --workspace` (duckdb OFF) compiles this to an empty test target
//! instead of failing on the absent `LakehouseStore`.

#![cfg(feature = "duckdb")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, SystemTime};

use zerodds_durability_store::{Contract, DurabilitySample, DurabilityStore, Selector, TopicMeta};
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
        representation_identifier: [0x00, 0x07],
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
fn dcps_2_2_3_5_keep_last_keeps_the_newest_depth_samples() {
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

/// Under KEEP_ALL a sample past `max_samples_per_instance` is refused
/// (DDS 1.4 §2.2.3.5 with §2.2.3.19: OUT_OF_RESOURCES); the same sample sent
/// again is not a new one.
#[test]
fn dcps_2_2_3_5_keep_all_refuses_a_sample_past_max_samples_per_instance() {
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

/// ADR 0009, invariant 3: a read is paginated, in `(instance, sequence)`
/// order.
#[test]
fn adr_0009_3_a_query_pages_through_the_history_in_order() {
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

/// What a store held comes back from the DuckDB file after the process that
/// wrote it is gone: every field of every sample, among them a `PL_CDR2_LE`
/// identifier, and the topic metadata.
#[test]
fn dcps_2_2_3_4_persistent_samples_and_topic_meta_outlive_the_store() {
    let db = tmp("reopen", "duckdb");
    let mut written: Vec<DurabilitySample> = (0..6).map(|seq| sample("S", 1, seq)).collect();
    written[2].representation_identifier = [0x00, 0x0b];
    let meta = TopicMeta {
        type_name: "demo::Command".to_string(),
        keyed: true,
        contract: Contract {
            history_kind: HistoryKind::KeepLast,
            history_depth: 6,
            ..keep_all()
        },
    };
    {
        let store = LakehouseStore::open(&db, keep_all()).unwrap();
        assert_eq!(store.topic_meta("S").unwrap(), None);
        store.set_topic_meta("S", meta.clone()).unwrap();
        for s in &written {
            store.store(s.clone()).unwrap();
        }
    } // dropped → DuckDB file persists
    let reopened = LakehouseStore::open(&db, keep_all()).unwrap();
    assert_eq!(reopened.replay_for_topic("S").unwrap(), written);
    assert_eq!(reopened.topic_meta("S").unwrap(), Some(meta));
    let _ = std::fs::remove_file(&db);
}

/// Also exercises `cleanup`'s path that reads the contract while already
/// holding the connection lock — the deadlock this fix's `contract_for_conn`
/// split closed (`contract_for` used to re-lock `self.conn`, which is not
/// reentrant, inside `cleanup`'s row-filtering closure).
#[test]
fn dcps_2_2_3_5_the_cleanup_delay_of_the_topic_meta_purges_an_unregistered_instance() {
    let store = LakehouseStore::open_in_memory(keep_all()).unwrap();
    store
        .set_topic_meta(
            "T",
            TopicMeta {
                type_name: "T".to_string(),
                keyed: false,
                contract: Contract {
                    cleanup_delay: Duration::ZERO,
                    ..keep_all()
                },
            },
        )
        .unwrap();
    store.store(sample("T", 1, 0)).unwrap();
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    store.unregister("T", &[1u8; 16], now).unwrap();
    assert_eq!(store.cleanup(now).unwrap(), 1);
    assert_eq!(store.stats("T").unwrap().samples, 0);
}

/// The lake export (ADR 0009: the adapter's own read interface) is a Parquet
/// file that DuckDB's Parquet reader reads back as the topic's samples, in
/// `(instance, sequence)` order.
#[test]
fn adr_0009_the_parquet_export_reads_back_as_the_topics_samples() {
    let store = LakehouseStore::open_in_memory(keep_all()).unwrap();
    store.set_contract("T", keep_all()).unwrap();
    for seq in 0..5 {
        store.store(sample("T", 7, seq)).unwrap();
    }
    store.store(sample("Other", 7, 9)).unwrap();
    let parquet = tmp("export", "parquet");
    store.export_parquet("T", &parquet).unwrap();
    let reader = duckdb::Connection::open_in_memory().unwrap();
    let path = parquet.display().to_string().replace('\'', "''");
    let mut statement = reader
        .prepare(&format!(
            "SELECT topic, sequence, payload FROM read_parquet('{path}')"
        ))
        .unwrap();
    let rows: Vec<(String, i64, Vec<u8>)> = statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let want: Vec<(String, i64, Vec<u8>)> = (0..5)
        .map(|seq| ("T".to_string(), seq, format!("p-7-{seq}").into_bytes()))
        .collect();
    assert_eq!(rows, want);
    let _ = std::fs::remove_file(&parquet);
}
