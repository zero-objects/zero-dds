// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! zerodds-flatdata-1.0 "Ziele" / "Nicht-Ziele" — shape of the default
//! build.
//!
//! Two clauses constrain what a plain `cargo build -p zerodds-flatdata`
//! pulls in: the goal "Pure-Rust — kein C-Plugin, kein Iceoryx-Cargo-Dep
//! im Default-Build" and the non-goal "Iceoryx-Wire-Compat — nur via
//! Optional-Feature `--features iceoryx2-bridge`". Both are properties of
//! the manifest, so the manifest is what is asserted.

#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

const MANIFEST: &str = include_str!("../Cargo.toml");

/// The `[features] default = [...]` list, verbatim.
fn default_features() -> String {
    MANIFEST
        .lines()
        .find(|l| l.trim_start().starts_with("default ="))
        .expect("[features] default entry")
        .to_string()
}

/// Goal "Pure-Rust": iceoryx2 must not be reachable from the default
/// feature set, neither directly nor through another default feature.
#[test]
fn default_features_do_not_pull_in_iceoryx2() {
    let defaults = default_features();
    assert!(
        !defaults.contains("iceoryx"),
        "default feature set must not name iceoryx: {defaults}"
    );
    // The dependency itself has to stay optional, otherwise it is linked
    // regardless of the feature list.
    assert!(
        MANIFEST.contains("iceoryx2 = { version = \"0.8\", optional = true }"),
        "iceoryx2 must stay an optional dependency"
    );
}

/// Non-goal "Iceoryx-Wire-Compat": the bridge exists, but only behind its
/// own opt-in feature.
#[test]
fn iceoryx_bridge_is_an_opt_in_feature() {
    assert!(
        MANIFEST.contains("iceoryx2-bridge = ["),
        "the iceoryx2-bridge feature must exist as an opt-in"
    );
    assert!(
        MANIFEST.contains("iceoryx2-bridge = [\"std\", \"dep:iceoryx2\"]"),
        "the bridge feature must be what enables the iceoryx2 dependency"
    );
}

/// Goal "Same-Host-Pfad im Default-Build": the POSIX shm/mmap backend is
/// on by default — that is the path §4.1 describes.
#[test]
fn posix_mmap_backend_is_on_by_default() {
    let defaults = default_features();
    assert!(
        defaults.contains("posix-mmap"),
        "the POSIX shm backend must be a default feature: {defaults}"
    );
}
