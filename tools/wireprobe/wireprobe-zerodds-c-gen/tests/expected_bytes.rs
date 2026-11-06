// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C column against `tests/interop-corpus/expected-bytes.tsv`
//! (decision 3 of `internal/release/rc8-test-suite.md`: each producer
//! checks its own octets against the table in its own tier).
//!
//! The table holds the octets other implementations wrote; no column of
//! this project is read. `wireprobe-c-matrix` answers one configuration in
//! one encoding, `plain-cdr1` or `plain-cdr2` for the table's `xcdr1` and
//! `xcdr2`, and `zerodds_wire_probe::expected::check` holds the payloads
//! to the rows of the configuration's dialect in that encoding and the
//! column's own read of them to the sample. One module per configuration
//! and one test per encoding the table has rows of for it (the CORBA ORBs
//! and the ROS 2 middlewares `xcdr1` only), so the list of tests says
//! which dialect broke. The two encodings compile disjoint units; as one
//! test, a configuration with an empty cache took up to 16.4 s on the CI runner host
//! beside a cold build of `zerodds-dcps` in the other slot, and `rti` ran
//! past its 25 s (pipeline 9143, job 230932).
//!
//! The driver compiles one translation unit per case, configuration and
//! encoding against `runtime/c`, a group of them per compiler call and one
//! program per group, and runs them; compiled probes are kept in
//! `CARGO_TARGET_TMPDIR`, so a configuration after the first compiles only
//! the units it does not share with an earlier one.
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
    // `tools/wireprobe/wireprobe-zerodds-c-gen` is three below the root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("the repository root")
        .to_path_buf()
}

/// The driver's answer for one configuration in one encoding,
/// `case → slot → answer`.
fn column(configuration: &str, encoding: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "expected-bytes-{configuration}-{encoding}-{}.json",
        std::process::id()
    ));
    let status = Command::new(env!("CARGO_BIN_EXE_wireprobe-c-matrix"))
        .args(["--config", configuration])
        .args(["--encoding", encoding])
        .arg("--cache")
        .arg(Path::new(env!("CARGO_TARGET_TMPDIR")).join("wireprobe-c-cache"))
        .arg("--out")
        .arg(&out)
        .status()
        .expect("wireprobe-c-matrix starts");
    assert!(
        status.success(),
        "wireprobe-c-matrix --config {configuration} --encoding {encoding} failed; its output is above"
    );
    let text = std::fs::read_to_string(&out).expect("the driver's JSON");
    let _ = std::fs::remove_file(&out);
    serde_json::from_str(&text).expect("the driver's JSON parses")
}

/// `xcdr` is the table's name of the encoding, `xcdr1` or `xcdr2`.
fn matches_the_table(configuration: &str, xcdr: &str) {
    let encoding = match xcdr {
        "xcdr1" => "plain-cdr1",
        "xcdr2" => "plain-cdr2",
        other => panic!("no encoding of the table is called `{other}`"),
    };
    let root = repository_root();
    let mut table = table(&root).expect("tests/interop-corpus/expected-bytes.tsv");
    table.rows.retain(|row| row.encoding == xcdr);
    // A declared cell is `case/sample encoding endian`.
    let declared: BTreeMap<_, _> = declared(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/expected_bytes_gaps.tsv"),
    )
    .expect("the gaps file")
    .into_iter()
    .filter(|((_, cell), _)| cell.split(' ').nth(1) == Some(xcdr))
    .collect();
    let settings = CONFIGURATIONS
        .iter()
        .find(|entry| entry.name == configuration)
        .expect("a configuration of the matrix");
    let verdict = check(
        &root,
        &table,
        settings,
        &column(configuration, encoding),
        &declared,
    );
    assert!(
        verdict.checked > 0,
        "no row of the table names `{configuration}` in {xcdr}"
    );
    let report = verdict.report("C", configuration, &table);
    assert!(report.is_empty(), "{report}");
    // The cells no reader can return the sample from are Cyclone's
    // (crates/wire-probe/tests/expected_bytes.rs,
    // `the_unreadable_cells_are_cyclones_24`).
    if configuration != "cyclone" {
        assert_eq!(verdict.nothing_to_read, 0, "cells without a branch to read");
    }
}

mod specification {
    #[test]
    fn xcdr1() {
        super::matches_the_table("specification", "xcdr1");
    }

    #[test]
    fn xcdr2() {
        super::matches_the_table("specification", "xcdr2");
    }
}

mod cyclone {
    #[test]
    fn xcdr1() {
        super::matches_the_table("cyclone", "xcdr1");
    }

    #[test]
    fn xcdr2() {
        super::matches_the_table("cyclone", "xcdr2");
    }
}

mod rti {
    #[test]
    fn xcdr1() {
        super::matches_the_table("rti", "xcdr1");
    }

    #[test]
    fn xcdr2() {
        super::matches_the_table("rti", "xcdr2");
    }
}

mod fastdds {
    #[test]
    fn xcdr1() {
        super::matches_the_table("fastdds", "xcdr1");
    }

    #[test]
    fn xcdr2() {
        super::matches_the_table("fastdds", "xcdr2");
    }
}

mod omniorb {
    #[test]
    fn xcdr1() {
        super::matches_the_table("omniorb", "xcdr1");
    }
}

mod tao {
    #[test]
    fn xcdr1() {
        super::matches_the_table("tao", "xcdr1");
    }
}

mod jacorb {
    #[test]
    fn xcdr1() {
        super::matches_the_table("jacorb", "xcdr1");
    }
}

mod opendds {
    #[test]
    fn xcdr1() {
        super::matches_the_table("opendds", "xcdr1");
    }

    #[test]
    fn xcdr2() {
        super::matches_the_table("opendds", "xcdr2");
    }
}

mod ros_fastrtps {
    #[test]
    fn xcdr1() {
        super::matches_the_table("ros-fastrtps", "xcdr1");
    }
}

mod ros_cyclone {
    #[test]
    fn xcdr1() {
        super::matches_the_table("ros-cyclone", "xcdr1");
    }
}
