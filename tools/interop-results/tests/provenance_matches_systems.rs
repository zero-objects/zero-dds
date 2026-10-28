// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The versions `systems.rs` claims, against what the host reported.
//!
//! `SYSTEMS.md` prints a product version beside every column, and those
//! strings are literals in `systems.rs`. Nothing connected them to the
//! toolchain that produced a row: upgrade a vendor, re-record, and the
//! table keeps naming the old version while the octets move under it.
//!
//! `tools/interop-results/toolchain-provenance.sh` asks each toolchain
//! on the host that has it and writes `tests/interop-results/PROVENANCE.tsv`.
//! This test is what makes that file bind: a version in `systems.rs`
//! that the recorded provenance does not support is a failure here.
//!
//! ## What it does not do
//!
//! It does not require the two strings to be equal. `systems.rs` says
//! "eProsima Fast DDS 3.6.1 / Fast CDR 2.3.5" and the host reports
//! "fastddsgen version 4.3.0 / libfastdds 3.6.1.0" — different
//! granularity, same product. The test requires that **every version
//! number in the claim appears in the probed string**, which catches a
//! claim of 3.6.1 against an installed 3.7.0 and tolerates the two
//! naming different parts of one toolchain.
//!
//! ## When a column has no probe
//!
//! Recorded and reported, not skipped. A column in the ledger whose
//! toolchain is absent from the measurement host is a column nobody can
//! re-derive, and that is a fact about the comparison rather than an
//! accident of the run.
//!
//! Such a column has to be named in
//! `tests/interop-results/PROVENANCE-NOTES.md`, with what is missing
//! and what it would take to restore. Absent from the host *and* absent
//! from that file is a failure — which is what keeps "known absence"
//! from becoming "forgotten".

#![allow(clippy::expect_used, clippy::panic, missing_docs)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the repository root, two levels above tools/interop-results")
        .to_path_buf()
}

/// `system -> (probed, version)` from the recorded provenance.
fn provenance() -> BTreeMap<String, (bool, String)> {
    let path = repository_root()
        .join("tests")
        .join("interop-results")
        .join("PROVENANCE.tsv");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let mut out = BTreeMap::new();
    for line in text.lines() {
        if line.starts_with('#') || line.starts_with("system\t") {
            continue;
        }
        let mut parts = line.split('\t');
        let (Some(system), Some(probed), Some(version)) =
            (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        out.insert(system.to_string(), (probed == "yes", version.to_string()));
    }
    out
}

/// Every dotted version number in a string: `3.6.1`, `11.0.1`, `6.5.24`.
fn version_numbers(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        if character.is_ascii_digit() || character == '.' {
            current.push(character);
        } else {
            if current.contains('.') && current.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                out.push(current.trim_end_matches('.').to_string());
            }
            current.clear();
        }
    }
    if current.contains('.') {
        out.push(current.trim_end_matches('.').to_string());
    }
    out
}

/// The claim in `systems.rs` is supported by what the host reported.
#[test]
fn every_claimed_version_is_one_the_host_reported() {
    let recorded = provenance();
    let notes_path = repository_root()
        .join("tests")
        .join("interop-results")
        .join("PROVENANCE-NOTES.md");
    let notes = std::fs::read_to_string(&notes_path)
        .unwrap_or_else(|error| panic!("{}: {error}", notes_path.display()));
    let mut trouble = Vec::new();

    for system in zerodds_interop_results::systems::SYSTEMS {
        // The three own columns are this tree, not an installed vendor.
        if system.version.starts_with("this tree") {
            continue;
        }
        // ROS columns carry the distribution, which the probe reports as
        // `ROS_DISTRO` under one key for both.
        let key = if system.id.starts_with("ros-") {
            "ros"
        } else {
            system.id
        };
        let Some((probed, reported)) = recorded.get(key) else {
            trouble.push(format!(
                "{}: claims `{}` and PROVENANCE.tsv has no row at all",
                system.id, system.version
            ));
            continue;
        };
        if !probed {
            if !notes.contains(&format!("`{}`", system.id)) {
                trouble.push(format!(
                    "{}: claims `{}`, the toolchain is absent from the measurement host, \
                     and PROVENANCE-NOTES.md does not name it",
                    system.id, system.version
                ));
            }
            continue;
        }
        for number in version_numbers(system.version) {
            if !reported.contains(&number) {
                trouble.push(format!(
                    "{}: claims `{}`, the host reported `{}` — {number} is not in it",
                    system.id, system.version, reported
                ));
            }
        }
    }

    assert!(
        trouble.is_empty(),
        "the versions SYSTEMS.md prints are not the ones the host has:\n  {}\n\n\
         Re-run `tools/interop-results/toolchain-provenance.sh` on the measurement \
         host and update `systems.rs` to what it reports; a toolchain that is not \
         there belongs in tests/interop-results/PROVENANCE-NOTES.md with what it \
         would take to restore it.",
        trouble.join("\n  ")
    );
}
