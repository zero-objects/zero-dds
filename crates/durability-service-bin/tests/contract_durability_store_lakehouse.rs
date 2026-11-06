// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The contract of `zerodds-durability-service-bin` with
//! `zerodds-durability-store-lakehouse` (internal/release/rc8-test-suite.md,
//! "Crate tier"), with the daemon's feature `lakehouse`. What the daemon
//! uses (`build_store` in `src/main.rs`): `LakehouseStore::open(path,
//! contract)` for `store: lakehouse` with a `path`, `open_in_memory(contract)`
//! without one, behind `Arc<dyn DurabilityStore>`. What it relies on: the
//! contract it hands over governs a topic that has none of its own, and a
//! file keeps what it holds for the next open.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use std::time::SystemTime;

use zerodds_durability_store::{Contract, DurabilitySample, DurabilityStore};
use zerodds_durability_store_lakehouse::LakehouseStore;
use zerodds_qos::policies::history::HistoryKind;

fn sample(seq: u64) -> DurabilitySample {
    DurabilitySample {
        topic: "T".into(),
        instance_key: [1; 16],
        sequence: seq,
        payload: vec![seq as u8],
        representation_identifier: [0x00, 0x07],
        created_at: SystemTime::UNIX_EPOCH,
        source_guid: [0; 16],
        source_sequence: -1,
    }
}

/// KEEP_LAST(2), the contract the daemon hands its store for the topics
/// without one of their own.
fn keep_last_2() -> Contract {
    Contract {
        history_kind: HistoryKind::KeepLast,
        history_depth: 2,
        ..Contract::keep_all()
    }
}

fn sequences(store: &dyn DurabilityStore) -> Vec<u64> {
    store
        .replay_for_topic("T")
        .unwrap()
        .iter()
        .map(|s| s.sequence)
        .collect()
}

#[test]
fn the_daemons_contract_governs_a_topic_and_the_file_keeps_it() {
    let path = std::env::temp_dir().join(format!(
        "zerodds-svc-contract-{}.duckdb",
        std::process::id()
    ));
    {
        let store = LakehouseStore::open(&path, keep_last_2()).unwrap();
        for seq in 0..3 {
            store.store(sample(seq)).unwrap();
        }
    }
    let store = LakehouseStore::open(&path, keep_last_2()).unwrap();
    assert_eq!(sequences(&store), vec![1, 2]);
    let memory = LakehouseStore::open_in_memory(keep_last_2()).unwrap();
    memory.store(sample(0)).unwrap();
    assert_eq!(sequences(&memory), vec![0]);
    drop(store);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}.wal", path.display()));
}
