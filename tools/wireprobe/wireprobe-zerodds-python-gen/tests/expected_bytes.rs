// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Python column against `tests/interop-corpus/expected-bytes.tsv`
//! (decision 3 of `internal/release/rc8-test-suite.md`: each producer
//! checks its own octets against the table in its own tier).
//!
//! The table holds the octets other implementations wrote; no column of
//! this project is read. `wireprobe-python-matrix` answers one configuration in the
//! encodings `plain-cdr1` and `plain-cdr2`, the table's `xcdr1` and
//! `xcdr2`, and `zerodds_wire_probe::expected::check` holds the payloads
//! to the rows of the configuration's dialect and the column's own read
//! of them to the sample. One test per configuration, so the list of
//! tests says which dialect broke.
//!
//! The driver generates one module per case, configuration, encoding and
//! byte order and runs them in `python3` workers.
//!
//! A cell this column does not reproduce is declared in
//! `tests/fixtures/expected_bytes_gaps.tsv` with its `OPEN-ITEMS.md`
//! entry, or `-` for a refusal `docs/language-limits.md` documents; a
//! declared cell that passes fails the test as well.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_wire_probe::expected::{check, declared, table};
use zerodds_wire_probe::matrix::CONFIGURATIONS;

fn repository_root() -> PathBuf {
    // `tools/wireprobe/wireprobe-zerodds-python-gen` is three below the root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("the repository root")
        .to_path_buf()
}

/// The driver's answer for one configuration, `case → slot → answer`.
fn column(configuration: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "expected-bytes-{configuration}-{}.json",
        std::process::id()
    ));
    let status = Command::new(env!("CARGO_BIN_EXE_wireprobe-python-matrix"))
        .args(["--config", configuration])
        .args(["--encoding", "plain-cdr1", "--encoding", "plain-cdr2"])
        .arg("--out")
        .arg(&out)
        .status()
        .expect("wireprobe-python-matrix starts");
    assert!(
        status.success(),
        "wireprobe-python-matrix --config {configuration} failed; its output is above"
    );
    let text = std::fs::read_to_string(&out).expect("the driver's JSON");
    let _ = std::fs::remove_file(&out);
    serde_json::from_str(&text).expect("the driver's JSON parses")
}

fn matches_the_table(configuration: &str) {
    let root = repository_root();
    let table = table(&root).expect("tests/interop-corpus/expected-bytes.tsv");
    let declared = declared(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/expected_bytes_gaps.tsv"),
    )
    .expect("the gaps file");
    let settings = CONFIGURATIONS
        .iter()
        .find(|entry| entry.name == configuration)
        .expect("a configuration of the matrix");
    let verdict = check(&root, &table, settings, &column(configuration), &declared);
    assert!(
        verdict.checked > 0,
        "no row of the table names `{configuration}`"
    );
    let report = verdict.report("Python", configuration, &table);
    assert!(report.is_empty(), "{report}");
    // The cells no reader can return the sample from are Cyclone's
    // (crates/wire-probe/tests/expected_bytes.rs,
    // `the_unreadable_cells_are_cyclones_24`).
    if configuration != "cyclone" {
        assert_eq!(verdict.nothing_to_read, 0, "cells without a branch to read");
    }
}

#[test]
fn specification() {
    matches_the_table("specification");
}

#[test]
fn cyclone() {
    matches_the_table("cyclone");
}

#[test]
fn rti() {
    matches_the_table("rti");
}

#[test]
fn fastdds() {
    matches_the_table("fastdds");
}

#[test]
fn omniorb() {
    matches_the_table("omniorb");
}

#[test]
fn tao() {
    matches_the_table("tao");
}

#[test]
fn jacorb() {
    matches_the_table("jacorb");
}

#[test]
fn opendds() {
    matches_the_table("opendds");
}

#[test]
fn ros_fastrtps() {
    matches_the_table("ros-fastrtps");
}

#[test]
fn ros_cyclone() {
    matches_the_table("ros-cyclone");
}
