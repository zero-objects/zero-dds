// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The file adapter against the retention of DDS 1.4 §2.2.3.5, the
//! PERSISTENT guarantee of §2.2.3.4 (the data outlive the process that
//! stored them) and the paginated read of ADR 0009.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, SystemTime};

use zerodds_durability_store::{Contract, DurabilitySample, DurabilityStore, Selector, TopicMeta};
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
        representation_identifier: [0x00, 0x07],
        created_at: SystemTime::UNIX_EPOCH + Duration::from_secs(seq),
        source_guid: [inst; 16],
        source_sequence: seq as i64,
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

/// What a store held comes back from the directory after the process that
/// wrote it is gone: every field of every sample, among them a big-endian
/// XCDR1 parameter-list identifier (`PL_CDR_BE`), and the topic metadata.
#[test]
fn dcps_2_2_3_4_persistent_samples_and_topic_meta_outlive_the_store() {
    let root = tmp("reopen");
    let mut written: Vec<DurabilitySample> = (0..5).map(|seq| sample("Sensor", 1, seq)).collect();
    written[3].representation_identifier = [0x00, 0x02];
    let meta = TopicMeta {
        type_name: "demo::Command".to_string(),
        keyed: true,
        contract: Contract {
            history_kind: HistoryKind::KeepLast,
            history_depth: 5,
            ..keep_all()
        },
    };
    {
        let store = FileStore::open(&root, keep_all()).unwrap();
        assert_eq!(store.topic_meta("Sensor").unwrap(), None);
        store.set_topic_meta("Sensor", meta.clone()).unwrap();
        for s in &written {
            store.store(s.clone()).unwrap();
        }
    } // store dropped — simulates process exit
    let reopened = FileStore::open(&root, keep_all()).unwrap();
    assert_eq!(reopened.replay_for_topic("Sensor").unwrap(), written);
    assert_eq!(reopened.topic_meta("Sensor").unwrap(), Some(meta));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn dcps_2_2_3_5_keep_last_keeps_the_newest_depth_samples_on_disk() {
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

/// The topic and instance names are hex-encoded into the directory names
/// (module docs): two topics a sanitizer would map to one directory stay
/// apart.
#[test]
fn two_topic_names_a_sanitizer_would_join_stay_apart() {
    let root = tmp("lossless");
    let store = FileStore::open(&root, keep_all()).unwrap();
    store.store(sample("a/b:c", 1, 0)).unwrap();
    store.store(sample("a_b_c", 1, 0)).unwrap();
    assert_eq!(store.replay_for_topic("a/b:c").unwrap().len(), 1);
    assert_eq!(store.replay_for_topic("a_b_c").unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn dcps_2_2_3_5_an_unregistered_instance_is_purged_after_the_cleanup_delay() {
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

/// A topic directory from before the `.meta` sidecar existed opens, and has
/// no metadata (module docs, `META_FILE`).
#[test]
fn a_topic_directory_without_its_sidecar_opens_without_meta() {
    let root = tmp("old-format");
    {
        let store = FileStore::open(&root, keep_all()).unwrap();
        store.store(sample("OldTopic", 1, 0)).unwrap();
    }
    let reopened = FileStore::open(&root, keep_all()).unwrap();
    assert_eq!(reopened.topic_meta("OldTopic").unwrap(), None);
    assert_eq!(reopened.replay_for_topic("OldTopic").unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_topic_meta_contract_governs_retention_over_set_contract() {
    let root = tmp("meta-governs");
    let store = FileStore::open(&root, keep_all()).unwrap();
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
    let _ = std::fs::remove_dir_all(&root);
}

/// ADR 0009, invariant 3: a read is paginated from the selector.
#[test]
fn adr_0009_3_a_query_pages_from_the_selector() {
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
