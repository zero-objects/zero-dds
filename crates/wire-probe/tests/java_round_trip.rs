// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Java column, out and back again.
//!
//! `java_column_gate.rs` holds the Java column's octets against the ones
//! the Rust column recorded, which measures the write direction. This
//! measures the other one: the same column writes a sample and reads it
//! back, and the values that come out have to be the values that went
//! in.
//!
//! ## What a self round trip can and cannot catch
//!
//! It cannot catch a reader that is wrong in the same way its own writer
//! is wrong — that is exactly what a self round trip passes. The gate
//! against the recorded octets is what catches that half, and the two
//! together are what a column needs: the octets say the writer agrees
//! with another implementation, and this says the reader agrees with the
//! writer.
//!
//! It catches everything else: a member dropped, a length misread, a
//! union's discriminator selecting the wrong case, an `@optional` that
//! comes back present, a Table 9 default put where a value belonged.
//!
//! ## The cases
//!
//! Not a sample of the corpus — a set chosen so that each of the five
//! forms in which a value may be **absent** has somewhere to show, since
//! those are the ones that need DDS-XTypes 1.3 Table 9 and the ones the
//! Java column could not read at all before `default_java.rs`:
//!
//! - an `@appendable` aggregate, whose delimiter may end early;
//! - a `@mutable` one, where a member may simply not be in the stream;
//! - an `@optional` member, absent outright;
//! - a bounded `string`, where the value may not fit;
//! - a `union`, where every branch but one is unselected.
//!
//! The rest are there because a round trip that only exercised the
//! absent forms would say nothing about the present ones.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::PathBuf;
use std::process::Command;
use zerodds_wire_probe::canonical::{canonicalise, difference, text};
use zerodds_wire_probe::encode::model_from_idl;

/// The cases, and what each is here for.
///
/// The first five are the forms Table 9 answers for; the rest are the
/// shapes a reader can lose a value in.
const CASES: [(&str, &str); 24] = [
    (
        "struct-appendable",
        "a delimiter that may end before the members do",
    ),
    (
        "struct-mutable",
        "a member that may not be in the stream at all",
    ),
    (
        "annotation-optional-mixed",
        "@optional members, present and absent",
    ),
    ("annotation-optional-absent", "an @optional member, absent"),
    ("string-bounded-utf8", "a bound the value may not fit"),
    ("union-appendable", "a branch that is not selected"),
    ("union-mutable", "the same, framed by parameter headers"),
    (
        "primitive-mixed",
        "fourteen scalars, and the alignment between them",
    ),
    (
        "primitive-longdouble",
        "binary128, which no double can hold",
    ),
    (
        "sequence-primitives-nonempty",
        "a count, a loop and an element codec",
    ),
    ("sequence-bounded-full", "a sequence at its bound"),
    (
        "array-1d-enum",
        "an enumeration read back to its own constant",
    ),
    ("array-2d-primitives", "two dimensions, in one flat run"),
    ("bitmask-bitbound-32", "a holder wider than an int"),
    ("bitset-basic", "fields packed into one anchor"),
    ("map-key-long", "an entry order the sample decides"),
    ("map-value-struct", "an aggregate inside an entry"),
    ("fixed-basic", "a decimal no double carries"),
    ("wstring-bounded-utf8", "UTF-16 code units, counted"),
    ("enum-sparse-values", "values that are not ordinals"),
    (
        "inherit-two-levels",
        "a base's members, in front of the derived type's",
    ),
    (
        "evolution-mutable-optional",
        "an @optional member of a @mutable type",
    ),
    ("optional-nested-struct", "an absent aggregate"),
    ("string-bounded-empty", "a bound and nothing in it"),
];

/// The encoding the round trip is made in.
///
/// One and not all five: what this measures is the reader against the
/// writer, and a second encoding would measure the same pair through a
/// second framing — which is what `wireprobe-java-corpus` is for. XCDR2's
/// plain form is the corpus's own default.
const ENCODING: &str = "plain-cdr2";

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

#[test]
fn the_java_column_reads_back_what_it_wrote() {
    if !java_is_here() {
        return;
    }
    let root = repository_root();
    let probe = root.join("tools/wireprobe/wireprobe-zerodds-java");
    let scratch = std::env::temp_dir().join("zd-java-round-trip");
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
        // compared in is the canonical one. No second implementation of
        // it here — the C column's harness does the same.
        let ast = match model_from_idl(
            &std::fs::read_to_string(directory.join("type.idl")).expect("the IDL"),
            &directory.join("type.idl").to_string_lossy(),
        ) {
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

        let produced: serde_json::Value =
            match serde_json::from_slice(&output.stdout).map_err(|error| error.to_string()) {
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
        "the Java column does not read back what it wrote:\n  {}",
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
