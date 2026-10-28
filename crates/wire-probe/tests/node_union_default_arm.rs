// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Node column over a union whose discriminator selects the
//! `default:` arm, out and back again.
//!
//! # What this measures, and what a weaker test missed
//!
//! `wireprobe-zerodds-node-gen/src/dump.rs` printed a decoded union
//! through a JavaScript `switch (v._d)` whose arms it built from the
//! case labels. A `CaseLabel::Default` names no value, so the case
//! carrying it collected no `case` label at all: its assignment was
//! emitted anyway, as unreachable text behind the previous arm's
//! `break`, and a hard-coded `default: break;` closed the switch. The
//! generated **decoder** had read the branch into the sample correctly;
//! the **printer** dropped it and reported nothing.
//!
//! That is why the cells stood as `-done.md` and not as a refusal.
//! `zerodds-node/omniorb-xcdr1-be/array-1d-union` wrote 28 octets that
//! carry `c0 04 00 00 00 00 00 00` — `-2.5` big endian — read them back
//! without an error, and printed `{"_d":9}`. Measured on 2026-09-04
//! against the recorded matrix: 441 of the 444 cells of the
//! `omniorb-xcdr1-be` column were re-read with the fix in place and
//! thirteen cases changed, every one of them a sample that selects a
//! `default:` arm. Across all forty option sets that is 520 cells that
//! carried a value with the branch missing. Only 48 of them had a peer
//! cell in the same constellation to be compared against, and there the
//! peer printed the branch; the other 472 had nothing to be measured
//! against and the drop was invisible there as well. No other
//! `zerodds-*` column drops one: all twenty sibling generators build the
//! same if-chain this one now does.
//!
//! **So the assertion here is on the value and not on the exit code.** A
//! test that only checked that the read succeeded is exactly the test
//! that let this stand: the read did succeed.
//!
//! # The cases
//!
//! Every case in `tests/interop-corpus` whose sample selects a
//! `default:` arm, plus the two controls that say the chain did not
//! simply start printing every branch. `matrix/cases` derives an
//! `-explicit-extensibility` twin of four of them, which the recorded
//! matrix measures and which is the same shape twice here:
//!
//! - **`union-default-unselected`**, whose `_d: 99` matches no label and
//!   whose union has no `default:` — the value is `_d` alone, and a
//!   printer that fell through to some arm would add a key here.
//! - **`union-multiple-labels`**, where one branch carries several
//!   labels and the discriminator picks one of them, so a chain built
//!   from an `||` of the labels is measured.
//!
//! `recursive-union-branch` is in the list for its size: its `default:`
//! arm is `sequence<Node>` and the drop took the whole subtree, not one
//! scalar. `array-of-union` is here for its discriminator: a `boolean`,
//! whose two values are `TRUE` as a label and `FALSE` through the
//! `default:` arm, so the label a chain has to spell is `true` and not a
//! number.
//!
//! # Why a round trip and not the recorded octets
//!
//! `crates/wire-probe/tests/` already holds column gates that compare a
//! column's octets against the Rust column's; the write direction was
//! never wrong here. What was wrong is the path from octets back to
//! values, and the shortest measurement of it is the column's own
//! sample, written by the column and read by the column, compared
//! against what went in. A reader wrong in the same way its writer is
//! wrong would pass this — that half is `matrix/gen-zerodds-node.sh`'s,
//! which holds the octets against nine vendors.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::PathBuf;
use std::process::Command;
use zerodds_wire_probe::canonical::{canonicalise, difference, text};
use zerodds_wire_probe::encode::model_from_idl;

/// The cases, and what each is here for.
const CASES: [(&str, &str); 11] = [
    ("union-with-default", "the bare shape: `default: double`"),
    (
        "union-with-default-unannotated",
        "the same with no extensibility annotation",
    ),
    ("array-1d-union", "the default arm inside an array element"),
    (
        "array-1d-union-unannotated",
        "the same with no extensibility annotation",
    ),
    (
        "sequence-union",
        "the default arm inside a sequence element",
    ),
    (
        "sequence-union-unannotated",
        "the same with no extensibility annotation",
    ),
    (
        "union-discriminator-typedef-enum",
        "a default arm under an enumerated discriminator",
    ),
    (
        "array-of-union",
        "a boolean discriminator, whose `default:` is its `FALSE`",
    ),
    (
        "recursive-union-branch",
        "a default arm holding a sequence that closes the recursion",
    ),
    (
        "union-default-unselected",
        "the control: no default arm and no label matches",
    ),
    (
        "union-multiple-labels",
        "the control: one branch behind several labels",
    ),
];

/// The encoding the round trip is made in.
///
/// One and not all five: what this measures is the printer against the
/// decoder, and a second framing would measure the same pair twice.
/// XCDR2's plain form is the corpus's own default.
const ENCODING: &str = "plain-cdr2";

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Whether a JavaScript runtime is here at all.
///
/// A loud skip and not a silent pass: a run with no `node` measures
/// nothing, and a gate that said nothing about it would read the same as
/// a gate that passed.
fn node_is_here() -> bool {
    match Command::new("node").arg("--version").output() {
        Ok(output) if output.status.success() => true,
        Ok(_) | Err(_) => {
            println!("SKIPPED: `node` did not answer; the Node column is unmeasured here");
            false
        }
    }
}

#[test]
fn the_node_column_prints_the_branch_a_default_arm_selected() {
    if !node_is_here() {
        return;
    }
    let root = repository_root();
    let probe = root.join("tools/wireprobe/wireprobe-zerodds-node");
    let scratch = std::env::temp_dir().join("zd-node-union-default-arm");
    std::fs::create_dir_all(&scratch).expect("a directory for the payloads");

    let mut checked = 0;
    let mut failures = Vec::new();

    for (case, purpose) in CASES {
        let directory = root.join("tests/interop-corpus").join(case);
        // The corpus names its root in `type`; reading it off the IDL
        // takes the last declaration, which is the wrong root wherever a
        // case declares two.
        let type_name = std::fs::read_to_string(directory.join("type"))
            .expect("the corpus case names its root")
            .trim()
            .to_string();
        let payload = scratch.join(format!("{case}.bin"));
        let values = scratch.join(format!("{case}.jsn"));

        let written = Command::new(&probe)
            .current_dir(&root)
            .args(["--idl", &directory.join("type.idl").to_string_lossy()])
            .args(["--type", &type_name])
            .args(["--values", &directory.join("values.json").to_string_lossy()])
            .args(["--encoding", ENCODING, "--endian", "le"])
            .args(["--out", &payload.to_string_lossy()])
            .output();
        match written {
            Ok(output) if output.status.success() => {}
            Ok(output) => {
                failures.push(format!(
                    "{case} ({purpose}): the write direction refused: {}",
                    first_line(&output.stderr)
                ));
                continue;
            }
            Err(error) => {
                failures.push(format!("{case}: the probe did not start ({error})"));
                continue;
            }
        }

        let read = Command::new(&probe)
            .current_dir(&root)
            .args(["--idl", &directory.join("type.idl").to_string_lossy()])
            .args(["--type", &type_name])
            .args(["--in", &payload.to_string_lossy()])
            .args(["--encoding", ENCODING, "--endian", "le"])
            .args(["--values-out", &values.to_string_lossy()])
            .output();
        let output = match read {
            Ok(output) => output,
            Err(error) => {
                failures.push(format!("{case}: the probe did not start ({error})"));
                continue;
            }
        };
        if !output.status.success() {
            failures.push(format!(
                "{case} ({purpose}): the read direction refused: {}",
                first_line(&output.stderr)
            ));
            continue;
        }

        // Both sides through `canonical.rs`, and not a text comparison:
        // the spelling of a float and the escape of a character are the
        // printer's, and the one form two implementations' values can be
        // compared in is the canonical one.
        let source = std::fs::read_to_string(directory.join("type.idl")).expect("the IDL");
        let ast = match model_from_idl(&source, &directory.join("type.idl").to_string_lossy()) {
            Ok(ast) => ast,
            Err(error) => {
                failures.push(format!("{case}: the model does not build ({error:?})"));
                continue;
            }
        };
        let Some(root_type) = ast
            .types()
            .find(|(_, node)| node.name.as_str() == type_name.rsplit("::").next().unwrap_or(""))
            .map(|(id, _)| id)
        else {
            failures.push(format!("{case}: `{type_name}` is not a type in this model"));
            continue;
        };

        let produced: serde_json::Value = match std::fs::read_to_string(&values)
            .map_err(|error| error.to_string())
            .and_then(|text| serde_json::from_str(&text).map_err(|error| error.to_string()))
        {
            Ok(value) => value,
            Err(error) => {
                failures.push(format!("{case}: the values are not JSON ({error})"));
                continue;
            }
        };
        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(directory.join("values.json")).expect("the sample"),
        )
        .expect("the corpus sample is JSON");

        let (produced, expected) = match (
            canonicalise(&ast, root_type, &produced, "read"),
            canonicalise(&ast, root_type, &expected, "sample"),
        ) {
            (Ok(produced), Ok(expected)) => (produced, expected),
            (Err(error), _) | (_, Err(error)) => {
                failures.push(format!("{case}: the values are not canonical ({error:?})"));
                continue;
            }
        };
        if let Some(where_) = difference(&expected, &produced, case) {
            failures.push(format!(
                "{case} ({purpose}): {where_:?}\n    wrote {}\n    read  {}",
                text(&expected),
                text(&produced)
            ));
            continue;
        }
        checked += 1;
    }

    assert!(
        failures.is_empty(),
        "the Node column does not read back what it wrote:\n  {}",
        failures.join("\n  ")
    );
    assert_eq!(
        checked,
        CASES.len(),
        "every case has to be measured, not skipped into a pass"
    );
}

fn first_line(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr)
        .lines()
        .next()
        .unwrap_or("refused")
        .to_string()
}
