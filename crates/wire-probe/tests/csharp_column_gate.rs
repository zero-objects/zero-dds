// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C# column, held against the octets the Rust column recorded.
//!
//! `tools/interop-results/csharp-record.sh` runs the whole corpus and
//! takes about half an hour; this runs the handful that separate the
//! things a column can get wrong, and runs on every `cargo test`. The
//! full sweep is what a change to `runtime/csharp` or to `emit/csharp.rs`
//! should be measured with; this is what keeps the column from rotting
//! between those runs.
//!
//! # What each case is here for
//!
//! Not a sample of the corpus — a set chosen so that a defect has
//! somewhere to show, and the same set the Java gate uses so the two
//! columns' failures are comparable line for line:
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
//! - **`primitive-longdouble`**, six binary128 values of which five no
//!   `double` can hold — and none of which a `decimal` can.
//! - **`fixed-scales`**, whose `fixed<31,10>` value has thirty
//!   significant digits: the measurement that says a `decimal` is not a
//!   holder for an IDL `fixed`.
//!
//! # Why it compares against the Rust column and not against a constant
//!
//! A hard-coded expectation here would be a third opinion about the
//! wire. `tests/interop-results/<case>/zerodds-rust/` is what the Rust
//! column produced and what every other column in the ledger is held
//! against; a C# run that differs from it is a disagreement between two
//! implementations of DDS-XTypes 1.3 §7.4, and that is the finding.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::PathBuf;
use std::process::Command;

/// The cases, and the configuration each is measured in.
const CASES: [(&str, &str); 9] = [
    ("primitive-mixed", "plain-cdr2"),
    ("primitive-mixed", "plain-cdr1"),
    ("primitive-mixed-descending-alignment", "plain-cdr1"),
    ("sequence-primitives-nonempty", "plain-cdr2"),
    ("bitmask-bitbound-32", "plain-cdr2"),
    ("array-1d-enum", "plain-cdr2"),
    ("union-discriminator-boolean", "plain-cdr2"),
    ("primitive-longdouble", "plain-cdr2"),
    ("fixed-scales", "plain-cdr2"),
];

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

#[test]
fn the_csharp_column_writes_what_the_rust_column_recorded() {
    if !dotnet_is_here() {
        return;
    }
    let root = repository_root();
    let probe = root.join("tools/wireprobe/wireprobe-zerodds-csharp");
    let mut checked = 0;
    let mut failures = Vec::new();

    for (case, encoding) in CASES {
        let directory = root.join("tests/interop-corpus").join(case);
        let recorded = root
            .join("tests/interop-results")
            .join(case)
            .join("zerodds-rust")
            .join(format!("{encoding}.le.bin"));
        let expected = match std::fs::read(&recorded) {
            Ok(bytes) => bytes,
            // A case the Rust column does not record is not a C#
            // failure, and saying so beats asserting against nothing.
            Err(error) => {
                failures.push(format!("{case} {encoding}: no recorded octets ({error})"));
                continue;
            }
        };
        // The corpus names its root in `type`; reading it off the IDL
        // takes the last declaration, which is the wrong root wherever a
        // case declares two.
        let type_name = std::fs::read_to_string(directory.join("type"))
            .expect("the corpus case names its root")
            .trim()
            .to_string();

        let run = Command::new(&probe)
            .current_dir(&root)
            .args(["--idl", &directory.join("type.idl").to_string_lossy()])
            .args(["--type", &type_name])
            .args(["--values", &directory.join("values.json").to_string_lossy()])
            .args(["--encoding", encoding, "--endian", "le"])
            .output();
        let output = match run {
            Ok(output) => output,
            Err(error) => {
                failures.push(format!(
                    "{case} {encoding}: the probe did not start ({error})"
                ));
                continue;
            }
        };
        if !output.status.success() {
            failures.push(format!(
                "{case} {encoding}: {}",
                String::from_utf8_lossy(&output.stderr)
                    .lines()
                    .next()
                    .unwrap_or("refused")
            ));
            continue;
        }
        let produced = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let want: String = expected.iter().map(|byte| format!("{byte:02x}")).collect();
        if produced != want {
            failures.push(format!(
                "{case} {encoding}:\n    c#   {produced}\n    rust {want}"
            ));
            continue;
        }
        checked += 1;
    }

    assert!(
        failures.is_empty(),
        "the C# column disagrees with the recorded octets:\n  {}",
        failures.join("\n  ")
    );
    assert_eq!(
        checked,
        CASES.len(),
        "every case has to be measured, not skipped into a pass"
    );
}
