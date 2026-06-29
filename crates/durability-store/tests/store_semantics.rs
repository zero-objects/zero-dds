// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Trait-semantics tests for the durability store core: contract enforcement,
//! paginated query, unregister/cleanup, and the tiered hot/cold invariant.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, SystemTime};

use zerodds_durability_store::{
    Contract, DurabilitySample, DurabilityStore, InMemoryStore, Selector, TieredStore,
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
        representation: 1,
        big_endian: false,
        created_at: SystemTime::UNIX_EPOCH + Duration::from_secs(seq),
    }
}

#[test]
fn keep_last_caps_history_per_instance() {
    let store = InMemoryStore::default();
    store
        .set_contract("T", contract(HistoryKind::KeepLast, 3))
        .unwrap();
    for seq in 0..10 {
        store.store(sample("T", 1, seq, 8)).unwrap();
    }
    let all = store.replay_for_topic("T").unwrap();
    // Only the newest 3 sequences survive.
    let seqs: Vec<u64> = all.iter().map(|s| s.sequence).collect();
    assert_eq!(seqs, vec![7, 8, 9]);
}

#[test]
fn keep_all_rejects_past_per_instance_cap() {
    let store = InMemoryStore::default();
    let mut c = contract(HistoryKind::KeepAll, 0);
    c.max_samples_per_instance = 2;
    store.set_contract("T", c).unwrap();
    assert!(store.store(sample("T", 1, 0, 8)).is_ok());
    assert!(store.store(sample("T", 1, 1, 8)).is_ok());
    let err = store.store(sample("T", 1, 2, 8)).unwrap_err();
    assert!(matches!(
        err,
        zerodds_durability_store::StoreError::OutOfResources(_)
    ));
    // A re-send of an EXISTING sequence at the cap is idempotent, not rejected.
    store.store(sample("T", 1, 0, 8)).unwrap();
    assert_eq!(store.stats("T").unwrap().samples, 2);
}

#[test]
fn max_instances_cap_enforced() {
    let store = InMemoryStore::default();
    let mut c = contract(HistoryKind::KeepAll, 0);
    c.max_instances = 2;
    store.set_contract("T", c).unwrap();
    assert!(store.store(sample("T", 1, 0, 8)).is_ok());
    assert!(store.store(sample("T", 2, 0, 8)).is_ok());
    let err = store.store(sample("T", 3, 0, 8)).unwrap_err();
    assert!(matches!(
        err,
        zerodds_durability_store::StoreError::OutOfResources(_)
    ));
    // An existing instance still accepts more.
    assert!(store.store(sample("T", 1, 1, 8)).is_ok());
}

#[test]
fn query_paginates_in_order() {
    let store = InMemoryStore::default();
    store
        .set_contract("T", contract(HistoryKind::KeepAll, 0))
        .unwrap();
    for inst in 1..=2u8 {
        for seq in 0..5 {
            store.store(sample("T", inst, seq, 8)).unwrap();
        }
    }
    // Page through with a small limit.
    let mut got = Vec::new();
    let mut sel = Selector {
        limit: Some(3),
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
    // 10 total, ordered (instance, seq).
    assert_eq!(got.len(), 10);
    assert_eq!(got[0], (1, 0));
    assert_eq!(got[9], (2, 4));
    assert!(got.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn query_selector_filters() {
    let store = InMemoryStore::default();
    store
        .set_contract("T", contract(HistoryKind::KeepAll, 0))
        .unwrap();
    for seq in 0..10 {
        store.store(sample("T", 1, seq, 8)).unwrap();
    }
    let sel = Selector {
        seq_from: Some(3),
        seq_to: Some(6),
        ..Selector::default()
    };
    let page = store.query("T", &sel).unwrap();
    let seqs: Vec<u64> = page.samples.iter().map(|s| s.sequence).collect();
    assert_eq!(seqs, vec![3, 4, 5, 6]);
}

#[test]
fn unregister_then_cleanup_purges_after_delay() {
    let store = InMemoryStore::default();
    let mut c = contract(HistoryKind::KeepAll, 0);
    c.cleanup_delay = Duration::from_secs(10);
    store.set_contract("T", c).unwrap();
    store.store(sample("T", 1, 0, 8)).unwrap();
    let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    store.unregister("T", &[1u8; 16], t0).unwrap();
    // Before the delay: not purged.
    assert_eq!(store.cleanup(t0 + Duration::from_secs(5)).unwrap(), 0);
    assert_eq!(store.stats("T").unwrap().samples, 1);
    // After the delay: purged.
    assert_eq!(store.cleanup(t0 + Duration::from_secs(11)).unwrap(), 1);
    assert_eq!(store.stats("T").unwrap().samples, 0);
}

#[test]
fn stats_counts_samples_instances_bytes() {
    let store = InMemoryStore::default();
    store
        .set_contract("T", contract(HistoryKind::KeepAll, 0))
        .unwrap();
    store.store(sample("T", 1, 0, 10)).unwrap();
    store.store(sample("T", 1, 1, 10)).unwrap();
    store.store(sample("T", 2, 0, 10)).unwrap();
    let s = store.stats("T").unwrap();
    assert_eq!(s.samples, 3);
    assert_eq!(s.instances, 2);
    assert_eq!(s.bytes, 30);
}

#[test]
fn tiered_hot_budget_bounds_cache_but_not_contract() {
    // THE core invariant (ADR 0009 inv. 2): a tiny hot budget must NOT shrink
    // the contracted history a late-joiner receives. Cold holds everything.
    let cold = InMemoryStore::default();
    let tiered = TieredStore::new(cold, 50); // 50 bytes hot budget
    tiered
        .set_contract("T", contract(HistoryKind::KeepAll, 0))
        .unwrap();
    // 100 samples × 10 bytes = 1000 bytes total, far over the 50-byte budget.
    for seq in 0..100 {
        tiered.store(sample("T", 1, seq, 10)).unwrap();
    }
    // Hot cache stayed within budget (~50 bytes = ~5 samples).
    let hot = tiered.hot_bytes("T").unwrap();
    assert!(hot <= 60, "hot bytes {hot} exceeded budget");
    // recent() returns only the hot subset (the newest few).
    let recent = tiered.recent("T", &Selector::default()).unwrap();
    assert!(recent.len() < 100 && !recent.is_empty());
    assert_eq!(recent.last().unwrap().sequence, 99); // newest is hot
    // query() (cold-authoritative) returns the FULL contracted history.
    let all = tiered.replay_for_topic("T").unwrap();
    assert_eq!(all.len(), 100);
    assert_eq!(all.first().unwrap().sequence, 0);
    assert_eq!(all.last().unwrap().sequence, 99);
}

#[test]
fn tiered_write_through_survives_hot_eviction() {
    let cold = InMemoryStore::default();
    let tiered = TieredStore::new(cold, 10); // 1 sample fits hot
    tiered
        .set_contract("T", contract(HistoryKind::KeepLast, 1000))
        .unwrap();
    for seq in 0..20 {
        tiered.store(sample("T", 1, seq, 10)).unwrap();
    }
    // Even with a 1-sample hot budget, all 20 are in cold.
    assert_eq!(tiered.stats("T").unwrap().samples, 20);
    assert_eq!(tiered.replay_for_topic("T").unwrap().len(), 20);
}
