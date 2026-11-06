// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The contract of `zerodds-durability-service-bin` with
//! `zerodds-durability-store` (internal/release/rc8-test-suite.md, "Crate
//! tier"). What the daemon uses: `Contract::keep_all()` as its own default
//! and as the base of a configured `contract`, whose fields it sets one by
//! one (`read_contract` in `src/config.rs`), and the `DurabilityStore` trait
//! object it builds a store as. What it relies on, from
//! `zerodds-durability-svc.yaml(5)`: the daemon's default is KEEP_ALL, every
//! cap unlimited (-1, DDS 1.4 §2.2.3.5 `LENGTH_UNLIMITED`), no cleanup delay,
//! and a field left out of a configured contract keeps that value.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use std::sync::Arc;
use std::time::Duration;

use zerodds_durability_store::{Contract, DurabilityStore, InMemoryStore};
use zerodds_qos::policies::history::HistoryKind;

#[test]
fn the_daemons_default_contract_is_keep_all_unlimited_without_delay() {
    let all = Contract::keep_all();
    assert_eq!(all.history_kind, HistoryKind::KeepAll);
    assert_eq!(
        (
            all.max_samples,
            all.max_instances,
            all.max_samples_per_instance
        ),
        (-1, -1, -1)
    );
    assert_eq!(all.cleanup_delay, Duration::ZERO);
    // A configured contract: the fields set, the others keep_all's.
    let configured = Contract {
        history_kind: HistoryKind::KeepLast,
        history_depth: 5,
        max_instances: 1000,
        cleanup_delay: Duration::from_secs(300),
        ..all
    };
    assert_eq!(configured.max_samples, -1);
    assert_eq!(configured.effective_depth(), 5);
    let store: Arc<dyn DurabilityStore> = Arc::new(InMemoryStore::default());
    store.set_contract("T", configured).unwrap();
    assert_eq!(store.stats("T").unwrap().samples, 0);
}
