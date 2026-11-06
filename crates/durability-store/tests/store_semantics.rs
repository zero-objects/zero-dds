// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The store semantics of `InMemoryStore` and `TieredStore` against the
//! clauses they implement: the retention of DDS 1.4 §2.2.3.5
//! DURABILITY_SERVICE (history kind and depth, `max_instances`,
//! `max_samples_per_instance`, `service_cleanup_delay`) and the invariants
//! of ADR 0009 (`docs/adr/0009-durability-service.md`: the cold store holds
//! the whole contracted history, the hot budget bounds only the cache,
//! reads are paginated).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, SystemTime};

use zerodds_durability_store::{
    Contract, DurabilitySample, DurabilityStore, InMemoryStore, Selector, StoreError, TieredStore,
    TopicMeta,
};
use zerodds_qos::policies::history::HistoryKind;
use zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED;

fn contract(kind: HistoryKind, depth: i32) -> Contract {
    Contract {
        history_kind: kind,
        history_depth: depth,
        max_samples: LENGTH_UNLIMITED,
        max_instances: LENGTH_UNLIMITED,
        max_samples_per_instance: LENGTH_UNLIMITED,
        cleanup_delay: Duration::ZERO,
    }
}

fn sample(topic: &str, inst: u8, seq: u64, bytes: usize) -> DurabilitySample {
    DurabilitySample {
        topic: topic.to_string(),
        instance_key: [inst; 16],
        sequence: seq,
        payload: vec![inst; bytes],
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

/// A stored sample comes back with what a replay needs besides its body:
/// the writer and sequence number it came from (to tell the writer's
/// history from its live stream) and the representation identifier of its
/// header (DDSI-RTPS 2.5 §10.5), here `PL_CDR2_BE`.
#[test]
fn a_sample_comes_back_with_its_source_and_its_representation_identifier() {
    let store = InMemoryStore::default();
    store.set_contract("T", Contract::default()).unwrap();
    let mut s = sample("T", 7, 42, 8);
    s.representation_identifier = [0x00, 0x0a];
    store.store(s.clone()).unwrap();
    assert_eq!(store.replay_for_topic("T").unwrap(), vec![s]);
}

#[test]
fn dcps_2_2_3_5_keep_last_keeps_the_newest_depth_samples_of_each_instance() {
    let store = InMemoryStore::default();
    store
        .set_contract("T", contract(HistoryKind::KeepLast, 3))
        .unwrap();
    for seq in 0..10 {
        store.store(sample("T", 1, seq, 8)).unwrap();
        store.store(sample("T", 2, seq, 8)).unwrap();
    }
    assert_eq!(sequences(&store, "T"), vec![7, 8, 9, 7, 8, 9]);
}

/// Under KEEP_ALL a sample past `max_samples_per_instance` is refused
/// (DDS 1.4 §2.2.3.5 with §2.2.3.19: OUT_OF_RESOURCES); the same sample sent
/// again is not a new one.
#[test]
fn dcps_2_2_3_5_keep_all_refuses_a_sample_past_max_samples_per_instance() {
    let store = InMemoryStore::default();
    let mut c = contract(HistoryKind::KeepAll, 0);
    c.max_samples_per_instance = 2;
    store.set_contract("T", c).unwrap();
    store.store(sample("T", 1, 0, 8)).unwrap();
    store.store(sample("T", 1, 1, 8)).unwrap();
    assert!(matches!(
        store.store(sample("T", 1, 2, 8)),
        Err(StoreError::OutOfResources(_))
    ));
    store.store(sample("T", 1, 0, 8)).unwrap();
    assert_eq!(sequences(&store, "T"), vec![0, 1]);
}

#[test]
fn dcps_2_2_3_5_a_new_instance_past_max_instances_is_refused() {
    let store = InMemoryStore::default();
    let mut c = contract(HistoryKind::KeepAll, 0);
    c.max_instances = 2;
    store.set_contract("T", c).unwrap();
    store.store(sample("T", 1, 0, 8)).unwrap();
    store.store(sample("T", 2, 0, 8)).unwrap();
    assert!(matches!(
        store.store(sample("T", 3, 0, 8)),
        Err(StoreError::OutOfResources(_))
    ));
    // An instance the store holds still takes samples.
    store.store(sample("T", 1, 1, 8)).unwrap();
}

/// `service_cleanup_delay`: the samples of an instance without writers go
/// once the delay has passed, not before.
#[test]
fn dcps_2_2_3_5_an_unregistered_instance_is_purged_after_the_cleanup_delay() {
    let store = InMemoryStore::default();
    let mut c = contract(HistoryKind::KeepAll, 0);
    c.cleanup_delay = Duration::from_secs(10);
    store.set_contract("T", c).unwrap();
    store.store(sample("T", 1, 0, 8)).unwrap();
    store.store(sample("T", 2, 0, 8)).unwrap();
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

/// ADR 0009, invariant 3: a read is paginated. Pages of three walk the ten
/// samples of two instances in `(instance, sequence)` order, and a selector
/// narrows by instance and sequence range.
#[test]
fn adr_0009_3_a_query_pages_through_the_history_in_order() {
    let store = InMemoryStore::default();
    store
        .set_contract("T", contract(HistoryKind::KeepAll, 0))
        .unwrap();
    for inst in 1..=2u8 {
        for seq in 0..5 {
            store.store(sample("T", inst, seq, 8)).unwrap();
        }
    }
    let mut got = Vec::new();
    let mut sel = Selector {
        limit: Some(3),
        ..Selector::default()
    };
    loop {
        let page = store.query("T", &sel).unwrap();
        assert!(page.samples.len() <= 3);
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
    let narrowed = store
        .query(
            "T",
            &Selector {
                instance_key: Some([2u8; 16]),
                seq_from: Some(1),
                seq_to: Some(3),
                ..Selector::default()
            },
        )
        .unwrap();
    let seqs: Vec<u64> = narrowed.samples.iter().map(|s| s.sequence).collect();
    assert_eq!(seqs, vec![1, 2, 3]);
}

/// ADR 0009, invariants 1 and 2: the hot budget bounds the cache in memory,
/// not the history a late joiner gets; the cold store holds every sample the
/// contract keeps, and the topic metadata.
#[test]
fn adr_0009_2_the_hot_budget_bounds_the_cache_and_not_the_history() {
    let tiered = TieredStore::new(InMemoryStore::default(), 50);
    tiered
        .set_contract("T", contract(HistoryKind::KeepAll, 0))
        .unwrap();
    // 100 samples of 10 bytes, far over the 50-byte budget.
    for seq in 0..100 {
        tiered.store(sample("T", 1, seq, 10)).unwrap();
    }
    assert!(tiered.hot_bytes("T").unwrap() <= 60);
    let recent = tiered.recent("T", &Selector::default()).unwrap();
    assert!(recent.len() < 100 && !recent.is_empty());
    assert_eq!(recent.last().unwrap().sequence, 99);
    assert_eq!(sequences(&tiered, "T"), (0..100).collect::<Vec<u64>>());
    let meta = TopicMeta {
        type_name: "T".to_string(),
        keyed: false,
        contract: Contract::keep_all(),
    };
    tiered.set_topic_meta("T", meta.clone()).unwrap();
    assert_eq!(tiered.topic_meta("T").unwrap(), Some(meta));
}

/// The topic's metadata comes back as set (none before), and its contract is
/// the one the store keeps to, not an earlier plain `set_contract`.
#[test]
fn the_topic_meta_comes_back_and_its_contract_governs_the_store() {
    let store = InMemoryStore::default();
    assert_eq!(store.topic_meta("T").unwrap(), None);
    store
        .set_contract("T", contract(HistoryKind::KeepLast, 100))
        .unwrap();
    let meta = TopicMeta {
        type_name: "demo::Command".to_string(),
        keyed: true,
        contract: contract(HistoryKind::KeepLast, 2),
    };
    store.set_topic_meta("T", meta.clone()).unwrap();
    assert_eq!(store.topic_meta("T").unwrap(), Some(meta));
    assert_eq!(store.topic_meta("Other").unwrap(), None);
    for seq in 0..5 {
        store.store(sample("T", 1, seq, 8)).unwrap();
    }
    assert_eq!(sequences(&store, "T"), vec![3, 4]);
}
