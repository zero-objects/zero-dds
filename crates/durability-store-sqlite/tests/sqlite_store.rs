// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The sqlite adapter against the retention of DDS 1.4 §2.2.3.5, the
//! PERSISTENT guarantee of §2.2.3.4 (the data outlive the process that
//! stored them) and the paginated read of ADR 0009.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, SystemTime};

use zerodds_durability_store::{Contract, DurabilitySample, DurabilityStore, Selector, TopicMeta};
use zerodds_durability_store_sqlite::SqliteStore;
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

/// A fresh database path; on Linux in `/dev/shm`, so that what is measured
/// is the store and not the disk of the host.
fn tmp_db(tag: &str) -> std::path::PathBuf {
    let shm = std::path::Path::new("/dev/shm");
    let mut p = if cfg!(target_os = "linux") && shm.is_dir() {
        shm.to_path_buf()
    } else {
        std::env::temp_dir()
    };
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    p.push(format!(
        "zerodds-sqlite-{tag}-{nanos}-{:?}.db",
        std::thread::current().id()
    ));
    p
}

#[test]
fn dcps_2_2_3_5_keep_last_keeps_the_newest_depth_samples() {
    let store = SqliteStore::open_in_memory(keep_all()).unwrap();
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
    let store = SqliteStore::open_in_memory(keep_all()).unwrap();
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
    // Re-send of an existing sequence at cap is idempotent, not rejected.
    store.store(sample("T", 1, 0)).unwrap();
    assert_eq!(store.stats("T").unwrap().samples, 2);
}

/// ADR 0009, invariant 3: a read is paginated, in `(instance, sequence)`
/// order, and a selector narrows it.
#[test]
fn adr_0009_3_a_query_pages_through_the_history_in_order() {
    let store = SqliteStore::open_in_memory(keep_all()).unwrap();
    store.set_contract("T", keep_all()).unwrap();
    for inst in 1..=2u8 {
        for seq in 0..5 {
            store.store(sample("T", inst, seq)).unwrap();
        }
    }
    // Page size 4 across 10 samples → 3 pages.
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
    // Instance filter.
    let only2 = store
        .query(
            "T",
            &Selector {
                instance_key: Some([2u8; 16]),
                ..Selector::default()
            },
        )
        .unwrap();
    assert_eq!(only2.samples.len(), 5);
}

/// What a store held comes back from the database file after the process
/// that wrote it is gone: every field of every sample, among them the
/// writer and sequence number it came from and a `D_CDR2_BE` identifier,
/// and the topic metadata.
#[test]
fn dcps_2_2_3_4_persistent_samples_and_topic_meta_outlive_the_store() {
    let path = tmp_db("reopen");
    let mut written: Vec<DurabilitySample> = (0..7).map(|seq| sample("Sensor", 1, seq)).collect();
    written[3].representation_identifier = [0x00, 0x08];
    let meta = TopicMeta {
        type_name: "demo::Command".to_string(),
        keyed: true,
        contract: Contract {
            history_kind: HistoryKind::KeepLast,
            history_depth: 7,
            ..keep_all()
        },
    };
    {
        let store = SqliteStore::open(&path, keep_all()).unwrap();
        assert_eq!(store.topic_meta("Sensor").unwrap(), None);
        store.set_topic_meta("Sensor", meta.clone()).unwrap();
        for s in &written {
            store.store(s.clone()).unwrap();
        }
    } // connection closed → WAL checkpoint on drop
    let reopened = SqliteStore::open(&path, keep_all()).unwrap();
    assert_eq!(reopened.replay_for_topic("Sensor").unwrap(), written);
    assert_eq!(reopened.topic_meta("Sensor").unwrap(), Some(meta));
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

/// A database whose header is destroyed is an error on open or read, not a
/// panic and not an empty store.
#[test]
fn a_corrupt_database_is_an_error() {
    let path = tmp_db("corrupt");
    {
        let store = SqliteStore::open(&path, keep_all()).unwrap();
        store.store(sample("T", 1, 0)).unwrap();
    }
    // Drop the WAL/SHM so the main file is authoritative, then smash the
    // SQLite header magic ("SQLite format 3\0").
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    let mut bytes = std::fs::read(&path).unwrap();
    for b in bytes.iter_mut().take(16) {
        *b = 0;
    }
    std::fs::write(&path, &bytes).unwrap();

    // Open + a read must return Err (or Ok-then-Err), but must NOT panic.
    let result =
        SqliteStore::open(&path, keep_all()).and_then(|s| s.replay_for_topic("T").map(|v| v.len()));
    assert!(
        result.is_err(),
        "corrupt sqlite must surface as a graceful error, got {result:?}"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn the_topic_meta_contract_governs_retention_over_set_contract() {
    let store = SqliteStore::open_in_memory(keep_all()).unwrap();
    store
        .set_contract(
            "T",
            Contract {
                history_kind: HistoryKind::KeepLast,
                history_depth: 100,
                ..keep_all()
            },
        )
        .unwrap();
    store
        .set_topic_meta(
            "T",
            TopicMeta {
                type_name: "T".to_string(),
                keyed: false,
                contract: Contract {
                    history_kind: HistoryKind::KeepLast,
                    history_depth: 2,
                    ..keep_all()
                },
            },
        )
        .unwrap();
    for seq in 0..5 {
        store.store(sample("T", 1, seq)).unwrap();
    }
    let seqs: Vec<u64> = store
        .replay_for_topic("T")
        .unwrap()
        .iter()
        .map(|s| s.sequence)
        .collect();
    assert_eq!(seqs, vec![3, 4]);
}

/// A database written before the `topic_meta` table existed (only `samples`
/// and `unregistered`) still opens — the schema change is additive
/// (`CREATE TABLE IF NOT EXISTS`), never a silent-empty or refused open —
/// and its sample, stored with the column defaults of then
/// (`representation` 1 for XCDR2, little endian), reads as CDR2_LE.
#[test]
fn a_database_from_before_the_topic_meta_table_opens() {
    let path = tmp_db("old-schema");
    {
        // Hand-build the pre-topic_meta schema directly (no `topic_meta`
        // table at all), insert a sample the way the old code would have.
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE samples (
                topic TEXT NOT NULL, instance BLOB NOT NULL, sequence INTEGER NOT NULL,
                created_nanos INTEGER NOT NULL, payload BLOB NOT NULL,
                representation INTEGER NOT NULL DEFAULT 1, big_endian INTEGER NOT NULL DEFAULT 0,
                source_guid BLOB NOT NULL DEFAULT x'00000000000000000000000000000000',
                source_sequence INTEGER NOT NULL DEFAULT -1,
                PRIMARY KEY (topic, instance, sequence)
            ) WITHOUT ROWID;
             CREATE TABLE unregistered (
                topic TEXT NOT NULL, instance BLOB NOT NULL, at_nanos INTEGER NOT NULL,
                PRIMARY KEY (topic, instance)
             );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO samples(topic, instance, sequence, created_nanos, payload) \
             VALUES ('T', x'00000000000000000000000000000000', 0, 0, x'2a')",
            [],
        )
        .unwrap();
    }
    // Opening through the current adapter must not fail, and the pre-existing
    // sample must still be readable; `topic_meta` for it is `None`, not an
    // error (never silently start empty, but a genuinely absent registration
    // is not corruption either).
    let store = SqliteStore::open(&path, keep_all()).unwrap();
    assert_eq!(store.topic_meta("T").unwrap(), None);
    let all = store.replay_for_topic("T").unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].payload, vec![0x2a]);
    assert_eq!(all[0].representation_identifier, [0x00, 0x07]);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn dcps_2_2_3_5_an_unregistered_instance_is_purged_after_the_cleanup_delay() {
    let store = SqliteStore::open_in_memory(keep_all()).unwrap();
    let c = Contract {
        cleanup_delay: Duration::from_secs(10),
        ..keep_all()
    };
    store.set_contract("T", c).unwrap();
    store.store(sample("T", 1, 0)).unwrap();
    let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    store.unregister("T", &[1u8; 16], t0).unwrap();
    assert_eq!(store.cleanup(t0 + Duration::from_secs(5)).unwrap(), 0);
    assert_eq!(store.cleanup(t0 + Duration::from_secs(11)).unwrap(), 1);
    assert_eq!(store.stats("T").unwrap().samples, 0);
}
