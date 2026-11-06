// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `wireprobe-c-matrix` measures the configurations of
//! `zerodds_wire_probe::matrix::CONFIGURATIONS`, in their order.
//!
//! The Python column's driver imports that list; the C driver still holds
//! a literal of its own (its module doc says why), and this test keeps the
//! two one list: a configuration added to the shared list and not to the
//! C driver would go unmeasured in the C column while
//! `tests/expected_bytes.rs` had no test for it to fail.
#![allow(clippy::expect_used, clippy::panic)]

use std::path::Path;

use zerodds_wire_probe::matrix::CONFIGURATIONS;

/// The configuration names in `wireprobe-c-matrix.rs`'s `CONFIGURATIONS`,
/// in order.
fn c_configurations(text: &str) -> Vec<String> {
    let start = text
        .find("const CONFIGURATIONS: [Configuration;")
        .expect("wireprobe-c-matrix.rs has a CONFIGURATIONS list");
    let block = &text[start..];
    let end = block.find("\n];\n").expect("the list ends");
    block[..end]
        .lines()
        .filter_map(|line| line.trim().strip_prefix("name: \""))
        .map(|rest| rest.trim_end_matches("\",").to_string())
        .collect()
}

#[test]
fn the_c_column_covers_the_same_configurations() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bin/wireprobe-c-matrix.rs"),
    )
    .expect("the C matrix driver");
    let shared: Vec<String> = CONFIGURATIONS
        .iter()
        .map(|entry| entry.name.to_string())
        .collect();
    // Read out of the literal and compared whole, rather than each name
    // searched for: a search passes on a name in a comment and says
    // nothing about a configuration the C column has and the list not.
    assert_eq!(
        c_configurations(&source),
        shared,
        "wireprobe-c-matrix.rs drives a different set of configurations"
    );
}
