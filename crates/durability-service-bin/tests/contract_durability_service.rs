// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The contract of `zerodds-durability-service-bin` with
//! `zerodds-durability-service` (internal/release/rc8-test-suite.md, "Crate
//! tier"). What the daemon uses (`run` in `src/main.rs`):
//! `DurabilityService::start_with_config` with the runtime configuration
//! its command line loaded, `enable_auto_discovery` for `auto`,
//! `serve_typed` for a topic with `type`, `serve_if_stored` and then
//! `serve_discovered` for a topic without, and `shutdown`. What it relies
//! on, from `zerodds-durability-svc.yaml(5)` ("type", "WHICH CONTRACT
//! WINS"): a topic served once is resumed from the store by the next run
//! without a writer; a pinned contract wins over the stored one; a topic the
//! store does not know is not resumed; `serve_discovered` on a topic already
//! served is no error.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use std::sync::Arc;

use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_durability_service::DurabilityService;
use zerodds_durability_store::{Contract, DurabilityStore};
use zerodds_durability_store_sqlite::SqliteStore;

#[test]
fn a_topic_served_once_is_resumed_by_the_next_run_with_the_pinned_contract() {
    let domain = zerodds_test_domains::unique_domain();
    let store: Arc<dyn DurabilityStore> =
        Arc::new(SqliteStore::open_in_memory(Contract::keep_all()).unwrap());
    {
        let first = DurabilityService::start_with_config(
            domain,
            Arc::clone(&store),
            RuntimeConfig::default(),
        )
        .unwrap();
        first
            .serve_typed("Commands", "demo::Command", true, Contract::keep_all())
            .unwrap();
        first.shutdown();
    }
    let next = Arc::new(
        DurabilityService::start_with_config(domain, Arc::clone(&store), RuntimeConfig::default())
            .unwrap(),
    );
    next.enable_auto_discovery(Contract::keep_all()).unwrap();
    let pinned = Contract {
        history_depth: 42,
        ..Contract::keep_all()
    };
    assert!(!next.serve_if_stored("Unknown", None).unwrap());
    assert!(next.serve_if_stored("Commands", Some(pinned)).unwrap());
    next.serve_discovered("Commands", pinned).unwrap();
    assert!(
        next.served_topics()
            .unwrap()
            .iter()
            .any(|t| t == "Commands")
    );
    assert_eq!(next.contract_of("Commands"), Some(pinned));
    next.shutdown();
}
