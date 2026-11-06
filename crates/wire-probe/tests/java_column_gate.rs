// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Java column, held against the octets other implementations wrote.
//!
//! `tools/wireprobe/wireprobe-java-corpus` runs all 374 recorded cases
//! and takes about eight minutes; this runs the handful that separate
//! the things a column can get wrong, and runs in the Java codegen jobs.
//! The full sweep is what a change to `runtime/java` or to
//! `emit/java.rs` should be measured with; this is what keeps the column
//! from rotting between those runs.
//!
//! # What each case is here for
//!
//! Not a sample of the corpus — a set chosen so that a defect has
//! somewhere to show:
//!
//! - **`primitive-mixed`** in both encodings. XCDR1's MAXALIGN is eight
//!   and XCDR2's is four, so the same fourteen members are 56 octets in
//!   one and 48 in the other; a column that missed the cap matches
//!   neither.
//! - **`sequence-primitives-nonempty`**, which is where the count, the
//!   loop and the element codec meet. It was blocked for a whole sweep
//!   by one field name: the walk asked C's sequence struct for
//!   `_length` and Java's array has none.
//! - **`bitmask-bitbound-32`**, which found `writeBitmask` taking its
//!   arguments in the opposite order from `zdw_write_bitmask`.
//! - **`array-1d-enum`**, which found the same in `writeEnum` — and
//!   found it at *run* time, because a swapped `int` and `long` are
//!   still a legal call.
//! - **`union-discriminator-boolean`**, whose labels are where a
//!   `boolean` discriminator has no Java literal: the IDL evaluator
//!   gives 0 and 1, and `(boolean) 1` is not a cast Java has.
//!
//! # What it compares against
//!
//! The octets of `tests/interop-corpus/expected-bytes.tsv` (decision 3 of
//! `internal/release/rc8-test-suite.md`): what another implementation wrote
//! for the case, the probe run under that vendor's dialect. No column of
//! this project is read. `primitive-longdouble` has no row there (no
//! measured vendor writes a binary128) and is not here.
//!
//! One test per case, each under the 25 s of a test with the toolchain;
//! they need a JDK and run in the Java codegen jobs
//! (`ci/integration_scenarios.py`, `CODEGEN["java"]`).
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::PathBuf;
use std::process::Command;

use zerodds_wire_probe::expected::{row_of, table};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Whether a Java toolchain is here at all.
///
/// A loud skip and not a silent pass: a run with no `javac` measures
/// nothing, and a gate that said nothing about it would read the same as
/// a gate that passed.
fn java_is_here() -> bool {
    for tool in ["javac", "java"] {
        match Command::new(tool).arg("-version").output() {
            Ok(output) if output.status.success() => {}
            Ok(_) | Err(_) => {
                println!("SKIPPED: `{tool}` did not answer; the Java column is unmeasured here");
                return false;
            }
        }
    }
    true
}

/// The Java probe writes `case` in `encoding` (`plain-cdr1` or
/// `plain-cdr2`, little-endian) as the first dialect of the table's row
/// writes it.
fn writes_the_octets_of_the_table(case: &str, encoding: &str) {
    if !java_is_here() {
        return;
    }
    let root = repository_root();
    let table = table(&root).expect("tests/interop-corpus/expected-bytes.tsv");
    let xcdr = if encoding == "plain-cdr1" {
        "xcdr1"
    } else {
        "xcdr2"
    };
    let (row, dialect) = row_of(&table, case, xcdr, "le")
        .unwrap_or_else(|| panic!("{case}: the table has no {xcdr} le row"));
    let directory = root.join("tests/interop-corpus").join(case);
    // The corpus names its root in `type`; reading it off the IDL takes
    // the last declaration, which is the wrong root wherever a case
    // declares two.
    let type_name = std::fs::read_to_string(directory.join("type"))
        .expect("the corpus case names its root")
        .trim()
        .to_string();
    let output = Command::new(root.join("tools/wireprobe/wireprobe-zerodds-java"))
        .current_dir(&root)
        .args(["--idl", &directory.join("type.idl").to_string_lossy()])
        .args(["--type", &type_name])
        .args(["--values", &directory.join("values.json").to_string_lossy()])
        .args([
            "--encoding",
            encoding,
            "--endian",
            "le",
            "--dialect",
            dialect,
        ])
        .output()
        .expect("the Java probe starts");
    assert!(
        output.status.success(),
        "{case} {encoding} under {dialect}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let produced = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(
        produced, row.hex,
        "{case} {encoding}: the Java column against {dialect}'s octets"
    );
}

#[test]
fn xtypes_7_4_primitive_mixed_xcdr2() {
    writes_the_octets_of_the_table("primitive-mixed", "plain-cdr2");
}

#[test]
fn xtypes_7_4_primitive_mixed_xcdr1_aligns_eight_octets() {
    writes_the_octets_of_the_table("primitive-mixed", "plain-cdr1");
}

#[test]
fn xtypes_7_4_primitive_mixed_descending_alignment_xcdr1() {
    writes_the_octets_of_the_table("primitive-mixed-descending-alignment", "plain-cdr1");
}

#[test]
fn xtypes_7_4_a_sequence_is_its_count_and_its_elements() {
    writes_the_octets_of_the_table("sequence-primitives-nonempty", "plain-cdr2");
}

#[test]
fn xtypes_7_4_a_bitmask_of_bit_bound_32() {
    writes_the_octets_of_the_table("bitmask-bitbound-32", "plain-cdr2");
}

#[test]
fn xtypes_7_4_an_array_of_an_enumeration() {
    writes_the_octets_of_the_table("array-1d-enum", "plain-cdr2");
}

#[test]
fn xtypes_7_4_a_union_with_a_boolean_discriminator() {
    writes_the_octets_of_the_table("union-discriminator-boolean", "plain-cdr2");
}
