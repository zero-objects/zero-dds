// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C# column, held against the octets other implementations wrote.
//!
//! `tools/interop-results/csharp-record.sh` runs the whole corpus and
//! takes about half an hour; this runs the handful that separate the
//! things a column can get wrong, and runs in the C# codegen jobs. The
//! full sweep is what a change to `runtime/csharp` or to `emit/csharp.rs`
//! should be measured with; this is what keeps the column from rotting
//! between those runs.
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
//!   loop and the element codec meet. A C# array's `Length` is an `int`
//!   and the count on the wire is a `uint`, and this is the case that
//!   says whether the crossing was written.
//! - **`bitmask-bitbound-32`**, which is where a `[Flags]` enum meets a
//!   runtime taking a `ulong`: C# has no implicit conversion from an
//!   enum to an integer, so a missing cast is a compile error and a
//!   wrong one is a different number.
//! - **`array-1d-enum`**, which is the same question for a real `enum`
//!   holder — the Java column maps an enumeration to `int` constants and
//!   this one does not.
//! - **`union-discriminator-boolean`**, whose labels are where a `bool`
//!   discriminator has no C# literal: the IDL evaluator gives 0 and 1,
//!   and `(bool)1` is not a cast C# has.
//! - **`fixed-scales`**, whose `fixed<31,10>` value has thirty
//!   significant digits: the measurement that says a `decimal` is not a
//!   holder for an IDL `fixed`.
//!
//! # What it compares against
//!
//! The octets of `tests/interop-corpus/expected-bytes.tsv` (decision 3 of
//! `internal/release/rc8-test-suite.md`): what another implementation wrote
//! for the case, the probe run under that vendor's dialect. No column of
//! this project is read. `primitive-longdouble` has no row there (no
//! measured vendor writes a binary128) and `fixed-scales` one, omniORB's
//! XCDR1; the long double goes, the `fixed` is measured in XCDR1.
//!
//! One test per case, each under the 25 s of a test with the toolchain;
//! they need `dotnet` and run in the C# codegen jobs
//! (`ci/integration_scenarios.py`, `CODEGEN["csharp"]`).
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::PathBuf;
use std::process::Command;

use zerodds_wire_probe::expected::{row_of, table};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Whether a .NET toolchain is here at all.
///
/// A loud skip and not a silent pass: a run with no `dotnet` measures
/// nothing, and a gate that said nothing about it would read the same as
/// a gate that passed.
fn dotnet_is_here() -> bool {
    match Command::new("dotnet").arg("--version").output() {
        Ok(output) if output.status.success() => true,
        Ok(_) | Err(_) => {
            println!("SKIPPED: `dotnet` did not answer; the C# column is unmeasured here");
            false
        }
    }
}

/// The C# probe writes `case` in `encoding` (`plain-cdr1` or
/// `plain-cdr2`, little-endian) as the first dialect of the table's row
/// writes it.
fn writes_the_octets_of_the_table(case: &str, encoding: &str) {
    if !dotnet_is_here() {
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
    let output = Command::new(root.join("tools/wireprobe/wireprobe-zerodds-csharp"))
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
        .expect("the C# probe starts");
    assert!(
        output.status.success(),
        "{case} {encoding} under {dialect}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let produced = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(
        produced, row.hex,
        "{case} {encoding}: the C# column against {dialect}'s octets"
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

#[test]
fn a_fixed_of_thirty_one_digits_as_omniorb_writes_it() {
    writes_the_octets_of_the_table("fixed-scales", "plain-cdr1");
}
