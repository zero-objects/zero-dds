// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Java column, held against the octets the Rust column recorded.
//!
//! `tools/wireprobe/wireprobe-java-corpus` runs all 374 recorded cases
//! and takes about eight minutes; this runs the handful that separate
//! the things a column can get wrong, and runs on every `cargo test`.
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
//! - **`primitive-longdouble`**, six binary128 values of which five no
//!   `double` can hold.
//!
//! # Why it compares against the Rust column and not against a constant
//!
//! A hard-coded expectation here would be a third opinion about the
//! wire. `tests/interop-results/<case>/zerodds-rust/` is what the Rust
//! column produced and what every other column in the ledger is held
//! against; a Java run that differs from it is a disagreement between
//! two implementations of DDS-XTypes 1.3 §7.4, and that is the finding.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::PathBuf;
use std::process::Command;

/// The cases, and the configuration each is measured in.
///
/// `plain-cdr1` alongside `plain-cdr2` only where the two differ in a
/// way this file is checking; everywhere else one configuration is the
/// measurement and a second would be the same defect twice.
const CASES: [(&str, &str); 8] = [
    ("primitive-mixed", "plain-cdr2"),
    ("primitive-mixed", "plain-cdr1"),
    ("primitive-mixed-descending-alignment", "plain-cdr1"),
    ("sequence-primitives-nonempty", "plain-cdr2"),
    ("bitmask-bitbound-32", "plain-cdr2"),
    ("array-1d-enum", "plain-cdr2"),
    ("union-discriminator-boolean", "plain-cdr2"),
    ("primitive-longdouble", "plain-cdr2"),
];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Whether a Java toolchain is here at all.
///
/// A loud skip and not a silent pass: a run with no `javac` measures
/// nothing, and a gate that said nothing about it would read the same
/// as a gate that passed.
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

#[test]
fn the_java_column_writes_what_the_rust_column_recorded() {
    if !java_is_here() {
        return;
    }
    let root = repository_root();
    let probe = root.join("tools/wireprobe/wireprobe-zerodds-java");
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
            // A case the Rust column does not record is not a Java
            // failure, and saying so beats asserting against nothing.
            Err(error) => {
                failures.push(format!("{case} {encoding}: no recorded octets ({error})"));
                continue;
            }
        };
        // The corpus names its root in `type`; reading it off the IDL
        // takes the last declaration, which is the wrong root wherever
        // a case declares two.
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
                "{case} {encoding}:\n    java {produced}\n    rust {want}"
            ));
            continue;
        }
        checked += 1;
    }

    assert!(
        failures.is_empty(),
        "the Java column disagrees with the recorded octets:\n  {}",
        failures.join("\n  ")
    );
    assert_eq!(
        checked,
        CASES.len(),
        "every case has to be measured, not skipped into a pass"
    );
}
