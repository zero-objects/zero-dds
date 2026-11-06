// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The contract of `zerodds-durability-service-bin` with
//! `zerodds-durability-store-postgres` (internal/release/rc8-test-suite.md,
//! "Crate tier"), with the daemon's feature `postgres`. What the daemon uses
//! (`build_store` in `src/main.rs`): `PostgresStore::connect(path, contract)`
//! with `path` a libpq connection string, and the error's text when it
//! cannot connect, which it reports before it exits. A connection to a
//! server needs one; the adapter's own tests run against a server in the
//! integration pillar. Here: a connection string whose socket directory
//! holds no server is an error with a text, not a panic and not a store.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_durability_store::Contract;
use zerodds_durability_store_postgres::PostgresStore;

#[test]
fn a_connection_string_without_a_server_is_an_error_with_a_text() {
    let err = PostgresStore::connect(
        "host=/nonexistent-zerodds-dir port=5432 user=zerodds dbname=postgres connect_timeout=2",
        Contract::keep_all(),
    )
    .err()
    .expect("an error");
    assert!(!err.to_string().is_empty());
}
