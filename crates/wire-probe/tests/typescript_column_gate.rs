// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The TypeScript column, held against the octets the Rust column
//! recorded.
//!
//! `tools/wireprobe/wireprobe-typescript-corpus` runs all 374 recorded
//! cases and takes about six minutes; this runs the handful that
//! separate the things a column can get wrong, and runs on every
//! `cargo test`. The full sweep is what a change to
//! `runtime/typescript` or to `emit/typescript.rs` should be measured
//! with; this is what keeps the column from rotting between those runs.
//!
//! # What each case is here for
//!
//! Not a sample of the corpus — a set chosen so that a defect has
//! somewhere to show, and every one of them was a defect on this
//! column's first sweep:
//!
//! - **`primitive-mixed`** in both encodings. XCDR1's MAXALIGN is eight
//!   and XCDR2's is four, so the same fourteen members are 56 octets in
//!   one and 48 in the other; a column that missed the cap matches
//!   neither.
//! - **`array-2d-primitives`**, where the codec's parameter has to be
//!   `boolean[][]`. It was `boolean[]` for a whole sweep, because
//!   `value_type` wrapped one pair of brackets whatever the dimension
//!   count, and the inner `v[i0][i1]` then indexed the global `Boolean`.
//! - **`bitmask-bitbound-32`** and **`bitset-basic`**, the two casts
//!   `as` cannot do: `v as bigint` from a `number` is
//!   `TS2352 … neither type sufficiently overlaps with the other`, and
//!   `BigInt(v)` is the conversion.
//! - **`array-1d-enum`**, which found `writeEnum` taking its arguments
//!   in the opposite order from `zdw_write_enum` — at run time, because
//!   two `number`s are a legal call either way round. The value is a
//!   `bigint` now and the swap does not compile.
//! - **`array-1d-union`**, where a `default:` arm carries the
//!   discriminator's whole type, so narrowing on `_d` reaches no arm and
//!   the case member is a property TypeScript says does not exist.
//! - **`union-default-unselected`**, whose sample is `_d: 99` against
//!   `case 1:` and `case 2:` — §7.2.2.4.4.4's "no selected member",
//!   which had no arm in the materialised type at all.
//! - **`empty-struct`**, one line of IDL inside a `module`, which is
//!   where a codec at the file root writes a bare `S` for a declaration
//!   that lives in `namespace m`.
//! - **`primitive-longdouble`**, six binary128 values of which five no
//!   `number` can hold, parsed from their decimals inside
//!   `runtime/typescript` and nowhere else.
//! - **`struct-mutable`** under `pl-cdr1`, which is the only
//!   configuration that reaches the parameter-list framing: the
//!   sentinel's flags, the member's, and the padding of a declared
//!   length.
//!
//! # Why it compares against the Rust column and not against a constant
//!
//! A hard-coded expectation here would be a third opinion about the
//! wire. `tests/interop-results/<case>/zerodds-rust/` is what the Rust
//! column produced and what every other column in the ledger is held
//! against; a TypeScript run that differs from it is a disagreement
//! between two implementations of DDS-XTypes 1.3 §7.4, and that is the
//! finding.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// The cases, and the configuration each is measured in.
///
/// A second configuration only where the two differ in a way this file
/// is checking; everywhere else one configuration is the measurement and
/// a second would be the same defect twice.
const CASES: [(&str, &str); 11] = [
    ("primitive-mixed", "plain-cdr2"),
    ("primitive-mixed", "plain-cdr1"),
    ("array-2d-primitives", "plain-cdr2"),
    ("bitmask-bitbound-32", "plain-cdr2"),
    ("bitset-basic", "plain-cdr2"),
    ("array-1d-enum", "plain-cdr2"),
    ("array-1d-union", "plain-cdr2"),
    ("union-default-unselected", "plain-cdr2"),
    ("empty-struct", "plain-cdr2"),
    ("primitive-longdouble", "plain-cdr2"),
    ("struct-mutable", "pl-cdr1"),
];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Whether a TypeScript toolchain is here at all.
///
/// A loud skip and not a silent pass: a run with no `node` measures
/// nothing, and a gate that said nothing about it would read the same as
/// a gate that passed.
///
/// Both halves are checked. `node` alone runs the compiled program and
/// `tsc` is what turns the generated file into one; a host with the
/// first and not the second reports "the generated TypeScript did not
/// compile" for every case, which looks like a finding and is not.
///
/// The `tsc` half asks an installed compiler and never `npx tsc`: with
/// no local TypeScript, `npx` resolves the name from the npm registry
/// and runs whatever answers to it — on the CI image that was
/// `tsc@2.0.4`, an unrelated package, and this probe read its refusal as
/// "no toolchain" and skipped a column the image was supposed to
/// measure. `ZERODDS_TSC` names one explicitly, after `ZERODDS_CSC` in
/// `crates/lang-rules/tests/csharp_corpus_compiles.rs`.
fn typescript_is_here() -> bool {
    let missing = |what: &str| {
        println!("SKIPPED: {what} did not answer; the TypeScript column is unmeasured here");
        // `ci/Dockerfile.rust` carries node and typescript, so a missing
        // toolchain inside a CI job is a stale or broken image. Skipping
        // there would leave this gate green and unmeasured.
        assert!(
            std::env::var_os("CI").is_none(),
            "{what} did not answer in a CI job — ci/Dockerfile.rust installs node and \
             typescript; rebuild the ci-rust image (the `ci-image` job) rather than \
             leaving the TypeScript column unmeasured."
        );
        false
    };
    match Command::new("node").arg("--version").output() {
        Ok(output) if output.status.success() => {}
        Ok(_) | Err(_) => return missing("`node`"),
    }
    if answers(&tsc_candidate()) {
        true
    } else {
        missing("`tsc`")
    }
}

/// Whether that program is a compiler that runs.
fn answers(program: &Path) -> bool {
    Command::new(program)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// The compiler the probe will use: `ZERODDS_TSC`, the runtime's own
/// `node_modules/.bin`, or `tsc` on the path — the order
/// `tools/wireprobe/wireprobe-zerodds-typescript` picks in.
fn tsc_candidate() -> PathBuf {
    if let Some(named) = std::env::var_os("ZERODDS_TSC") {
        return PathBuf::from(named);
    }
    let local = repository_root().join("runtime/typescript/node_modules/.bin/tsc");
    if local.is_file() {
        return local;
    }
    PathBuf::from("tsc")
}

#[test]
fn the_typescript_column_writes_what_the_rust_column_recorded() {
    if !typescript_is_here() {
        return;
    }
    let root = repository_root();
    let probe = root.join("tools/wireprobe/wireprobe-zerodds-typescript");
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
            // A case the Rust column does not record is not a
            // TypeScript failure, and saying so beats asserting against
            // nothing.
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
            // The exit code is the verdict —
            // `docs/specs/zerodds-verdicts-1.0.md` — so it is reported
            // beside the message rather than folded into "refused".
            failures.push(format!(
                "{case} {encoding}: exit {} — {}",
                output.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&output.stderr)
                    .lines()
                    .next()
                    .unwrap_or("no message")
            ));
            continue;
        }
        let produced = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let want: String = expected.iter().map(|byte| format!("{byte:02x}")).collect();
        if produced != want {
            failures.push(format!(
                "{case} {encoding}:\n    typescript {produced}\n    rust       {want}"
            ));
            continue;
        }
        checked += 1;
    }

    assert!(
        failures.is_empty(),
        "the TypeScript column disagrees with the recorded octets:\n  {}",
        failures.join("\n  ")
    );
    assert_eq!(
        checked,
        CASES.len(),
        "every case has to be measured, not skipped into a pass"
    );
}
