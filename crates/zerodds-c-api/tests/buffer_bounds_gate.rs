// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Gate: every exported `#[unsafe(no_mangle)] pub unsafe extern "C" fn` in
//! `src/` that writes a caller buffer bounded by a capacity parameter must
//! have a boundary test. The scan runs over the source text, so a new
//! capacity-buffer call is caught the moment it is added, and the test below
//! fails naming it.
//!
//! The capacity parameter set is the one the C API uses for a caller buffer's
//! capacity: `cap` (the seven §2.8 list calls, `zerodds_reader_take_into`),
//! `capacity` (`*_unicast_locators`), `out_cap` (`zerodds_xcdr2_encode`) and
//! `inout_len` (the two `*_get_key_value` byte buffers). A bare `len` is an
//! input-buffer length in this API (payloads, TypeObjects), not an output
//! buffer's capacity, so it is not treated as one.
//!
//! `BOUNDARY_TESTS` names, for every such function, the test that covers the
//! empty result, the exactly-full buffer and the one-entry/byte-too-small
//! buffer. The gate also fails on a stale entry (a listed function the scan
//! no longer finds) and on a named test that does not exist.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Parameter names the C API uses for the capacity of a caller buffer.
const CAPACITY_PARAMS: &[&str] = &["cap", "capacity", "out_cap", "buf_len", "inout_len"];

/// Every detected capacity-buffer call and the test covering its three
/// boundary cases. A rename of either side fails the gate.
const BOUNDARY_TESTS: &[(&str, &str)] = &[
    (
        "zerodds_dp_get_discovered_participants",
        "dp_get_discovered_participants_bounds",
    ),
    (
        "zerodds_dp_get_discovered_topics",
        "dp_get_discovered_topics_bounds",
    ),
    (
        "zerodds_dw_get_matched_subscriptions",
        "dw_get_matched_subscriptions_bounds",
    ),
    (
        "zerodds_dr_get_matched_publications",
        "dr_get_matched_publications_bounds",
    ),
    ("zerodds_sub_get_datareaders", "sub_get_datareaders_bounds"),
    (
        "zerodds_waitset_get_conditions",
        "waitset_get_conditions_reports_the_total",
    ),
    ("zerodds_waitset_wait", "waitset_wait_reports_the_total"),
    (
        "zerodds_dw_get_key_value",
        "dw_get_key_value_reports_the_needed_length",
    ),
    (
        "zerodds_dr_get_key_value",
        "dr_get_key_value_reports_the_needed_length",
    ),
    ("zerodds_reader_take_into", "reader_take_into_bounds"),
    (
        "zerodds_writer_unicast_locators",
        "writer_unicast_locators_bounds",
    ),
    (
        "zerodds_reader_unicast_locators",
        "reader_unicast_locators_bounds",
    ),
    ("zerodds_xcdr2_encode", "xcdr2_encode_bounds"),
    (
        "zerodds_writer_matched_rpc_readers",
        "writer_matched_rpc_readers_bounds",
    ),
    (
        "zerodds_reader_matched_rpc_writers",
        "reader_matched_rpc_writers_bounds",
    ),
];

/// Collects every `.rs` file under `dir`, recursively.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read directory") {
        let entry = entry.expect("directory entry");
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// The exported function names declared in `source` that take a capacity
/// parameter. Only `#[unsafe(no_mangle)] pub ... extern "C" fn` declarations
/// count.
fn capacity_functions(source: &str) -> Vec<String> {
    let marker = "#[unsafe(no_mangle)]";
    let mut out = Vec::new();
    let mut rest = source;
    while let Some(pos) = rest.find(marker) {
        rest = &rest[pos + marker.len()..];
        let Some(fn_pos) = rest.find("fn ") else {
            break;
        };
        let declaration = &rest[..fn_pos];
        if !declaration.contains("extern \"C\"") || !declaration.contains("pub ") {
            // A private helper or a non-C item between the attribute and `fn`.
            rest = &rest[fn_pos + 3..];
            continue;
        }
        let after = &rest[fn_pos + 3..];
        let Some(open) = after.find('(') else {
            break;
        };
        let name = after[..open].trim().to_string();
        let bytes = after.as_bytes();
        let mut depth = 0usize;
        let mut end = None;
        for (index, byte) in bytes.iter().enumerate().skip(open) {
            match byte {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(index);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end else {
            break;
        };
        if params_have_capacity(&after[open + 1..end]) {
            out.push(name);
        }
        rest = &after[end..];
    }
    out
}

/// Whether a parameter list declares a parameter whose name is in
/// [`CAPACITY_PARAMS`]. The name is the identifier immediately before a `:`
/// that is not part of a `::` path.
fn params_have_capacity(params: &str) -> bool {
    let bytes = params.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] != b':' || bytes.get(i + 1) == Some(&b':') || (i > 0 && bytes[i - 1] == b':') {
            continue;
        }
        let mut end = i;
        while end > 0 && bytes[end - 1].is_ascii_whitespace() {
            end -= 1;
        }
        let mut start = end;
        while start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
            start -= 1;
        }
        if CAPACITY_PARAMS.contains(&&params[start..end]) {
            return true;
        }
    }
    false
}

#[test]
fn every_capacity_buffer_call_has_a_boundary_test() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    let mut source_files = Vec::new();
    rust_files(&root.join("src"), &mut source_files);
    let mut detected: BTreeMap<String, PathBuf> = BTreeMap::new();
    for file in &source_files {
        let text = fs::read_to_string(file).expect("read source file");
        for name in capacity_functions(&text) {
            detected.insert(name, file.clone());
        }
    }

    let listed: BTreeMap<&str, &str> = BOUNDARY_TESTS.iter().copied().collect();

    // Every detected call is covered.
    for (name, file) in &detected {
        if !listed.contains_key(name.as_str()) {
            panic!(
                "{name} in {} writes a caller buffer bounded by a capacity \
                 parameter but is not in BOUNDARY_TESTS; add its empty / \
                 exactly-full / one-too-small boundary test",
                file.display()
            );
        }
    }

    // The list does not name a function the scan no longer finds.
    for name in listed.keys() {
        if !detected.contains_key(*name) {
            panic!(
                "BOUNDARY_TESTS names {name}, which the scan no longer detects \
                 as a capacity-parameter call; remove the stale entry"
            );
        }
    }

    // Every named test exists in `src` or `tests`.
    let mut test_files = Vec::new();
    rust_files(&root.join("tests"), &mut test_files);
    test_files.extend(source_files);
    let mut test_text = String::new();
    for file in &test_files {
        test_text.push_str(&fs::read_to_string(file).expect("read test file"));
        test_text.push('\n');
    }
    for (name, test) in BOUNDARY_TESTS {
        let needle = format!("fn {test}(");
        if !test_text.contains(&needle) {
            panic!(
                "BOUNDARY_TESTS maps {name} to {test}, but no `{needle}` \
                 declaration exists in src/ or tests/"
            );
        }
    }
}
