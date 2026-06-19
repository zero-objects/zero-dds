// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! File-adapter tests: contract enforcement on disk + the PERSISTENT
//! restart-survival guarantee.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, SystemTime};

use zerodds_durability_store::{Contract, DurabilitySample, DurabilityStore, Selector};
use zerodds_durability_store_file::FileStore;
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
        payload: format!("payload-{inst}-{seq}").into_bytes(),
        created_at: SystemTime::UNIX_EPOCH + Duration::from_secs(seq),
    }
}

/// Unique temp dir per test (no cross-test contamination).
fn tmp(tag: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    p.push(format!(
        "zerodds-filestore-test-{tag}-{nanos}-{:?}",
        std::thread::current().id()
    ));
    p
}

#[test]
fn persistent_survives_reopen() {
    let root = tmp("reopen");
    {
        let store = FileStore::open(&root, keep_all()).unwrap();
        store.set_contract("Sensor", keep_all()).unwrap();
        for seq in 0..5 {
            store.store(sample("Sensor", 1, seq)).unwrap();
        }
    } // store dropped — simulates process exit
    // Fresh store on the same root = a restart. Full history survives.
    let reopened = FileStore::open(&root, keep_all()).unwrap();
    let all = reopened.replay_for_topic("Sensor").unwrap();
    assert_eq!(all.len(), 5);
    assert_eq!(all[0].sequence, 0);
    assert_eq!(all[4].sequence, 4);
    // created_at survived the round-trip via the per-sample header.
    assert_eq!(
        all[3].created_at,
        SystemTime::UNIX_EPOCH + Duration::from_secs(3)
    );
    // payload integrity.
    assert_eq!(all[2].payload, b"payload-1-2");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn keep_last_drops_oldest_on_disk() {
    let root = tmp("keeplast");
    let store = FileStore::open(&root, keep_all()).unwrap();
    let c = Contract {
        history_kind: HistoryKind::KeepLast,
        history_depth: 3,
        ..keep_all()
    };
    store.set_contract("T", c).unwrap();
    for seq in 0..8 {
        store.store(sample("T", 1, seq)).unwrap();
    }
    let seqs: Vec<u64> = store
        .replay_for_topic("T")
        .unwrap()
        .iter()
        .map(|s| s.sequence)
        .collect();
    assert_eq!(seqs, vec![5, 6, 7]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn topic_names_are_lossless() {
    // Topics with special chars must not collide (hex-encoded dirs).
    let root = tmp("lossless");
    let store = FileStore::open(&root, keep_all()).unwrap();
    store.store(sample("a/b:c", 1, 0)).unwrap();
    store.store(sample("a_b_c", 1, 0)).unwrap();
    assert_eq!(store.replay_for_topic("a/b:c").unwrap().len(), 1);
    assert_eq!(store.replay_for_topic("a_b_c").unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn unregister_cleanup_purges_instance_dir() {
    let root = tmp("cleanup");
    let store = FileStore::open(&root, keep_all()).unwrap();
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
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn query_selector_and_pagination() {
    let root = tmp("query");
    let store = FileStore::open(&root, keep_all()).unwrap();
    for seq in 0..10 {
        store.store(sample("T", 1, seq)).unwrap();
    }
    let sel = Selector {
        seq_from: Some(4),
        limit: Some(3),
        ..Selector::default()
    };
    let page = store.query("T", &sel).unwrap();
    let seqs: Vec<u64> = page.samples.iter().map(|s| s.sequence).collect();
    assert_eq!(seqs, vec![4, 5, 6]);
    assert!(page.next.is_some());
    let _ = std::fs::remove_dir_all(&root);
}
