// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A collection inside a collection, written and read back.
//!
//! # What this is here for
//!
//! `resolve-array-of-typedef-array` is `typedef long Row[3];
//! @appendable struct S { Row grid[2]; };`. It read back as
//! `[[1,5,0],[1,5,0]]` from a `[[1,2,3],[4,5,6]]` that had been written
//! and re-read by the same column, in **C and C++ alike**, and it was
//! filed as a defect in the shared decoder walk on exactly that
//! evidence: two independent columns agreeing on one wrong value.
//!
//! They were not independent. The decoders were right — each collection
//! gets its own generated function, so its loop index is alone in its
//! scope. The **printer** that turns a decoded sample back into JSON
//! inlines the whole walk into one function, and it named its loop
//! index after the nesting level *within one array declaration*. An
//! array whose element is another array opened two loops both called
//! `zd_a0`, so it printed `grid[i][i]` and read one element past the
//! array. The C and C++ printers are the same code in two spellings,
//! which is why they agreed.
//!
//! A test instrument that reports a correct decode as a wrong one is
//! worse than one that reports nothing: it sends the next reader into
//! the walk that was never broken. This file is the gate that would
//! have separated the two.
//!
//! # Why it compares against the corpus sample
//!
//! Not against the column's own reading of its own bytes, which passes
//! whenever the writer and the reader are wrong in the same way, and
//! not against another column's reading, which passes whenever the two
//! printers share a defect — the exact thing that happened here.
//! `values.json` is what the case says the values are.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::PathBuf;
use std::process::Command;

use zerodds_wire_probe::canonical::{canonicalise, difference, text};
use zerodds_wire_probe::encode::model_from_idl;

/// The cases, and what nesting each one puts inside another.
///
/// Chosen so that a printer or a decoder that loses an index has
/// somewhere to show it — every one of these opens a loop inside a
/// loop, which is the shape the defect needed:
const CASES: [(&str, &str); 8] = [
    (
        "resolve-array-of-typedef-array",
        "an array whose element is a typedef'd array — the case that was misread",
    ),
    ("array-2d-primitives", "two dimensions in one declaration"),
    (
        "array-3d-primitives",
        "three, so a middle index has a neighbour on both sides",
    ),
    (
        "array-1d-sequence",
        "an array of sequences: a constant bound outside a counted one",
    ),
    ("sequence-of-array", "the same the other way round"),
    ("sequence-nested", "a sequence of sequences, both counted"),
    ("array-2d-struct", "two dimensions over an aggregate"),
    ("map-nested", "a map whose value is a map"),
];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

#[test]
fn the_c_column_reads_back_a_collection_inside_a_collection() {
    let root = repository_root();
    let probe = root.join("tools/wireprobe/wireprobe-zerodds-c");
    let mut checked = 0;
    let mut failures = Vec::new();

    for (case, why) in CASES {
        let directory = root.join("tests/interop-corpus").join(case);
        let recorded = root
            .join("tests/interop-results")
            .join(case)
            .join("zerodds-rust")
            .join("plain-cdr2.le.bin");
        if !recorded.is_file() {
            failures.push(format!("{case} ({why}): no recorded octets to read"));
            continue;
        }
        let type_name = std::fs::read_to_string(directory.join("type"))
            .expect("the corpus case names its root")
            .trim()
            .to_string();
        // `target` of the source tree does not exist where cargo builds elsewhere
        // (`CARGO_TARGET_DIR`); the probe writes its values there.
        std::fs::create_dir_all(root.join("target")).expect("the output directory is made");
        let read_back = root
            .join("target")
            .join(format!("zd_round_trip_{case}.json"));

        let run = Command::new(&probe)
            .current_dir(&root)
            .args(["--idl", &directory.join("type.idl").to_string_lossy()])
            .args(["--type", &type_name])
            .args(["--in", &recorded.to_string_lossy()])
            .args(["--values-out", &read_back.to_string_lossy()])
            .output();
        let output = match run {
            Ok(output) => output,
            Err(error) => {
                failures.push(format!("{case} ({why}): the probe did not start ({error})"));
                continue;
            }
        };
        if !output.status.success() {
            failures.push(format!(
                "{case} ({why}): {}",
                String::from_utf8_lossy(&output.stderr)
                    .lines()
                    .next()
                    .unwrap_or("refused")
            ));
            continue;
        }
        let produced_text = std::fs::read_to_string(&read_back).expect("the values the probe read");
        let expected_text = std::fs::read_to_string(directory.join("values.json"))
            .expect("the corpus case's own sample");
        let _ = std::fs::remove_file(&read_back);

        // Both sides through `canonical.rs`, and not a text comparison:
        // whether a whole number prints as `1` or `1.0` is the
        // printer's, and the one form two implementations' values can
        // be compared in is the canonical one.
        let source = std::fs::read_to_string(directory.join("type.idl")).expect("the IDL");
        let Ok(ast) = model_from_idl(&source, &directory.join("type.idl").to_string_lossy()) else {
            failures.push(format!("{case} ({why}): the model does not build"));
            continue;
        };
        let Some(root) = ast
            .types()
            .find(|(_, node)| node.name.as_str() == type_name.rsplit("::").next().unwrap_or(""))
            .map(|(id, _)| id)
        else {
            failures.push(format!("{case} ({why}): `{type_name}` is not a type here"));
            continue;
        };
        let produced: serde_json::Value =
            serde_json::from_str(&produced_text).expect("the probe prints JSON");
        let expected: serde_json::Value =
            serde_json::from_str(&expected_text).expect("the corpus sample is JSON");
        let (Ok(produced), Ok(expected)) = (
            canonicalise(&ast, root, &produced, "read"),
            canonicalise(&ast, root, &expected, "sample"),
        ) else {
            failures.push(format!("{case} ({why}): the values are not canonical"));
            continue;
        };
        if let Some(found) = difference(&expected, &produced, "") {
            failures.push(format!(
                "{case} ({why}): {found:?}\n    read {}\n    said {}",
                text(&produced),
                text(&expected)
            ));
            continue;
        }
        checked += 1;
    }

    assert!(
        failures.is_empty(),
        "a collection inside a collection did not survive the round trip:\n  {}",
        failures.join("\n  ")
    );
    assert_eq!(
        checked,
        CASES.len(),
        "every case has to be measured, not skipped into a pass"
    );
}
