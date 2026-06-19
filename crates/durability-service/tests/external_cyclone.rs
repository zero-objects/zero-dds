// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-vendor ingest probe: run alongside an external foreign-vendor
//! `TRANSIENT`/`TRANSIENT_LOCAL` writer (a CycloneDDS publisher) on the same
//! domain and verify the ZeroDDS durability service ingests its samples.
//!
//! `#[ignore]` — needs the external writer; run with `--ignored`. Build + run
//! the Cyclone side from `tests/cyclone-harness/` (see its README). Env knobs:
//! `ZD_DOMAIN` / `ZD_TOPIC` / `ZD_TYPE` (defaults match the harness).
//!
//! Verified live on codepit (CycloneDDS 0.11): a final `dur::Sample`
//! `TRANSIENT_LOCAL` writer → `matched=1`, sample ingested. This surfaced the
//! data-representation interop fix (the ingest reader must accept XCDR1 as well
//! as XCDR2 — a foreign FINAL-type writer offers only XCDR1).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// Diagnostics are printed for the harness operator (run with --nocapture).
#![allow(clippy::print_stdout)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_durability_service::DurabilityService;
use zerodds_durability_store::{Contract, DurabilityStore};
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

#[test]
#[ignore = "needs an external foreign-vendor writer; see tests/cyclone-harness/"]
fn external_cyclone_ingest() {
    let domain: i32 = std::env::var("ZD_DOMAIN")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let topic = std::env::var("ZD_TOPIC").unwrap_or_else(|_| "DurVendorX".to_string());
    let type_name = std::env::var("ZD_TYPE").unwrap_or_else(|_| "dur::Sample".to_string());

    let store: Arc<dyn DurabilityStore> =
        Arc::new(SqliteStore::open_in_memory(keep_all()).unwrap());
    let service = DurabilityService::start(domain, Arc::clone(&store)).unwrap();
    service
        .serve_typed(&topic, &type_name, false, keep_all())
        .unwrap();
    println!("ZD_SERVING topic={topic} type={type_name} domain={domain}");

    // ZD_HOLD_SECS keeps the service up for the full window (so an external
    // late-joiner subscriber can match the replay writer + receive history) —
    // used by the representation-fidelity end-to-end (cyclone pub → service →
    // cyclone sub). Default: poll until the first ingest, then exit.
    let hold: Option<u64> = std::env::var("ZD_HOLD_SECS")
        .ok()
        .and_then(|v| v.parse().ok());
    let window = hold.unwrap_or(40);
    let deadline = Instant::now() + Duration::from_secs(window);
    let mut got = 0;
    while Instant::now() < deadline {
        got = store.stats(&topic).map(|s| s.samples).unwrap_or(0);
        if got > 0 && hold.is_none() {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    println!("ZD_INGESTED={got}");
    service.shutdown();
    assert!(
        got > 0,
        "expected to ingest >=1 sample from the external foreign-vendor writer"
    );
}
