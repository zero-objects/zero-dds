// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! sqlite-adapter tests: contract enforcement, pagination, ACID restart.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, SystemTime};

use zerodds_durability_store::{Contract, DurabilitySample, DurabilityStore, Selector};
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
        representation: 1,
        big_endian: false,
        created_at: SystemTime::UNIX_EPOCH + Duration::from_secs(seq),
    }
}

fn tmp_db(tag: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
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
fn keep_last_trims_to_depth() {
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

#[test]
fn keep_all_caps_reject() {
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

#[test]
fn idempotent_resend() {
    let store = SqliteStore::open_in_memory(keep_all()).unwrap();
    store.set_contract("T", keep_all()).unwrap();
    store.store(sample("T", 1, 5)).unwrap();
    store.store(sample("T", 1, 5)).unwrap(); // re-send same seq
    assert_eq!(store.stats("T").unwrap().samples, 1);
}

#[test]
fn pagination_and_selector() {
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

#[test]
fn acid_survives_reopen() {
    let path = tmp_db("reopen");
    {
        let store = SqliteStore::open(&path, keep_all()).unwrap();
        store.set_contract("Sensor", keep_all()).unwrap();
        for seq in 0..7 {
            store.store(sample("Sensor", 1, seq)).unwrap();
        }
    } // connection closed → WAL checkpoint on drop
    let reopened = SqliteStore::open(&path, keep_all()).unwrap();
    let all = reopened.replay_for_topic("Sensor").unwrap();
    assert_eq!(all.len(), 7);
    assert_eq!(all[6].sequence, 6);
    assert_eq!(all[3].payload, b"p-1-3");
    assert_eq!(
        all[3].created_at,
        SystemTime::UNIX_EPOCH + Duration::from_secs(3)
    );
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn corrupt_db_surfaces_gracefully() {
    // P4 scenario 5 (disk corruption): a corrupt store must yield a graceful
    // error, never a panic / UB.
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
fn cleanup_after_delay() {
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
