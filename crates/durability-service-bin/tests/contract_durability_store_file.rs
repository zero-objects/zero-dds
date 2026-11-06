// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The contract of `zerodds-durability-service-bin` with
//! `zerodds-durability-store-file` (internal/release/rc8-test-suite.md,
//! "Crate tier"). What the daemon uses (`build_store` in `src/main.rs`):
//! `FileStore::open(dir, contract)` for `store: file`, behind `Arc<dyn
//! DurabilityStore>`, and the error's text when it cannot open. What it
//! relies on: the directory is created when it is missing, the contract it
//! hands over governs a topic that has none of its own, the directory keeps
//! what it holds for the next open, and a path that cannot be a directory
//! is an error (`zerodds-durability-svc.yaml(5)`: `path` is "a directory
//! (required)").

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use std::time::SystemTime;

use zerodds_durability_store::{Contract, DurabilitySample, DurabilityStore};
use zerodds_durability_store_file::FileStore;
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
fn the_daemons_contract_governs_a_topic_and_the_directory_keeps_it() {
    let dir =
        std::env::temp_dir().join(format!("zerodds-svc-contract-{}/store", std::process::id()));
    {
        let store = FileStore::open(&dir, keep_last_2()).unwrap();
        for seq in 0..3 {
            store.store(sample(seq)).unwrap();
        }
    }
    let store = FileStore::open(&dir, keep_last_2()).unwrap();
    assert_eq!(sequences(&store), vec![1, 2]);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn a_path_that_cannot_be_a_directory_is_an_error_with_a_text() {
    let file =
        std::env::temp_dir().join(format!("zerodds-svc-contract-{}.file", std::process::id()));
    std::fs::write(&file, b"x").unwrap();
    let err = FileStore::open(file.join("store"), Contract::keep_all())
        .err()
        .expect("an error");
    assert!(!err.to_string().is_empty());
    let _ = std::fs::remove_file(&file);
}
