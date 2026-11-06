// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Kotlin column over a bounded collection whose elements are
//! references, read from the octets another implementation wrote.
//!
//! # What this measures
//!
//! DDS-XTypes 1.3 §7.3.1.2.1.12.3 gives a bounded collection whose
//! stream carries more elements than the bound a TRIM behaviour, and
//! `emit::decode` renders it through [`CodecTarget::keep_first`]. The
//! Kotlin backend spelled that `v = v.copyOf(bound)`, and
//! `Array<T>.copyOf(newSize)` is declared `Array<T?>` in the Kotlin
//! standard library — the argument may exceed the receiver and the
//! places it would add hold `null`. Assigning it back to the `Array<T>`
//! the walk declared is
//! `assignment type mismatch: actual type is 'Array<String?>', but
//! 'Array<String>' was expected`, which `kotlinc` reports as an **error**
//! and not a warning.
//!
//! So the defect was not a wrong octet: it was a file that did not
//! compile, and every sample of every case whose type set reaches one of
//! these three shapes answered `exit 7` with no values at all. Measured
//! on 2026-09-04 across the recorded matrix, that was 224 cells of the
//! `zerodds-kotlin` column, in exactly the six cases these three shapes
//! appear in (`sequence-string`, `sequence-wstring` and `map-bounded`,
//! each with its `-unannotated` twin).
//!
//! # The three cases, and why these three
//!
//! One per **kind of element storage** a Kotlin array can have, because
//! the standard library's answer differs by kind and only one of the
//! three ever compiled:
//!
//! - **`sequence-string`** — `sequence<string, 8>`, a boxed
//!   `Array<String>`;
//! - **`sequence-wstring`** — `sequence<wstring, 8>`, the same shape
//!   through the other character width, which the column holds in a
//!   `String` as well;
//! - **`map-bounded`** — `map<long, long, 4>`, whose entries are a
//!   generated holder class, so an `Array<ZdEntry_long_long>` even where
//!   both halves are primitive. A bound on a map reaches the seam for
//!   every key and value type there is.
//!
//! A bounded sequence of a *primitive* is deliberately not here: it is
//! an `IntArray`, `IntArray.copyOf` answers an `IntArray`, and that
//! shape compiled throughout. It would measure nothing.
//!
//! # Why the read direction and against the expected-bytes table
//!
//! The write direction never reached the decoder and so never reached
//! the defect — the matrix records `wrote NB` beside a `.jsn.err` for
//! exactly these cells. Reading is where the generated decoder is
//! compiled, and the values that come out are compared against the
//! corpus sample, so a column that compiles but mis-reads is a failure
//! here too and not a pass.
//!
//! The octets are a vendor's, from `tests/interop-corpus/expected-bytes.tsv`
//! (decision 3 of `internal/release/rc8-test-suite.md`), read under that
//! vendor's dialect: Cyclone's for the two sequences, Fast DDS's for the
//! map. No column of this project is read. One test per case, each under
//! the 25 s of a test with `kotlinc`; they run in the Kotlin codegen jobs
//! (`ci/integration_scenarios.py`, `CODEGEN["kotlin"]`).
//!
//! # Cost
//!
//! Three `kotlinc` invocations, about twenty-five seconds together on a
//! 2026 laptop. That is why the list is three and not the corpus:
//! `matrix/gen-zerodds-kotlin.sh` is the full sweep.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::{Path, PathBuf};
use std::process::Command;
use zerodds_wire_probe::canonical::{canonicalise, difference, text};
use zerodds_wire_probe::encode::model_from_idl;
use zerodds_wire_probe::expected::{row_of, table};

/// The encoding the octets are read in.
const ENCODING: &str = "plain-cdr2";

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Whether a Kotlin toolchain is here at all.
///
/// A loud skip and not a silent pass: a run with no `kotlinc` measures
/// nothing, and a gate that said nothing about it would read the same as
/// a gate that passed.
fn kotlin_is_here() -> bool {
    match Command::new("kotlinc").arg("-version").output() {
        Ok(output) if output.status.success() => true,
        Ok(_) | Err(_) => {
            println!("SKIPPED: `kotlinc` did not answer; the Kotlin column is unmeasured here");
            false
        }
    }
}

/// The Kotlin probe reads the table's `xcdr2` little-endian octets of
/// `case`, under the dialect of their first writer, back to the sample.
fn reads_the_octets_of_the_table(case: &str, purpose: &str) {
    if !kotlin_is_here() {
        return;
    }
    if let Err(failure) = read(case) {
        panic!("the Kotlin column does not read {case} ({purpose}): {failure}");
    }
}

fn read(case: &str) -> Result<(), String> {
    let root = repository_root();
    let probe = root.join("tools/wireprobe/wireprobe-zerodds-kotlin");
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("kotlin-bounded-collections");
    std::fs::create_dir_all(&scratch).expect("a directory for the values");
    let table = table(&root).expect("tests/interop-corpus/expected-bytes.tsv");
    let directory = root.join("tests/interop-corpus").join(case);
    let (row, dialect) =
        row_of(&table, case, "xcdr2", "le").ok_or("the table has no xcdr2 le row")?;
    let octets_file = scratch.join(format!("{case}.{dialect}.bin"));
    let octets: Vec<u8> = (0..row.hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&row.hex[i..i + 2], 16).expect("hex"))
        .collect();
    std::fs::write(&octets_file, octets).expect("the vendor's octets");
    // The corpus names its root in `type`; reading it off the IDL takes the
    // last declaration, which is the wrong root wherever a case declares
    // two.
    let type_name = std::fs::read_to_string(directory.join("type"))
        .expect("the corpus case names its root")
        .trim()
        .to_string();
    let values = scratch.join(format!("{case}.jsn"));

    let output = Command::new(&probe)
        .current_dir(&root)
        .args(["--idl", &directory.join("type.idl").to_string_lossy()])
        .args(["--type", &type_name])
        .args(["--in", &octets_file.to_string_lossy()])
        .args([
            "--encoding",
            ENCODING,
            "--endian",
            "le",
            "--dialect",
            dialect,
        ])
        .args(["--values-out", &values.to_string_lossy()])
        .output()
        .map_err(|error| format!("the probe did not start ({error})"))?;
    if !output.status.success() {
        // The exit code is the verdict — `docs/specs/zerodds-verdicts-1.0.md`.
        // Exit 7 is this generator's own defect, which is what a file that
        // does not compile is, so it is reported beside the message rather
        // than folded into "refused".
        return Err(format!(
            "exit {} — {}",
            output
                .status
                .code()
                .map_or_else(|| "by a signal".to_string(), |code| code.to_string()),
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .take(2)
                .collect::<Vec<_>>()
                .join(" / ")
        ));
    }

    // Both sides through `canonical.rs`, and not a text comparison: the
    // spelling of a float and the escape of a character are the printer's,
    // and the one form two implementations' values can be compared in is
    // the canonical one.
    let source = std::fs::read_to_string(directory.join("type.idl")).expect("the IDL");
    let ast = model_from_idl(&source, &directory.join("type.idl").to_string_lossy())
        .map_err(|error| format!("the model does not build ({error:?})"))?;
    let short_name = type_name.rsplit("::").next().ok_or("an empty type name")?;
    let root_type = ast
        .types()
        .find(|(_, node)| node.name.as_str() == short_name)
        .map(|(id, _)| id)
        .ok_or_else(|| format!("`{type_name}` is not a type in this model"))?;
    let produced: serde_json::Value = std::fs::read_to_string(&values)
        .map_err(|error| error.to_string())
        .and_then(|text| serde_json::from_str(&text).map_err(|error| error.to_string()))
        .map_err(|error| format!("the values are not JSON ({error})"))?;
    let expected: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(directory.join("values.json")).expect("the sample"),
    )
    .expect("the corpus sample is JSON");
    let produced = canonicalise(&ast, root_type, &produced, "read")
        .map_err(|error| format!("the values are not canonical ({error:?})"))?;
    let expected = canonicalise(&ast, root_type, &expected, "sample")
        .map_err(|error| format!("the sample is not canonical ({error:?})"))?;
    if let Some(where_) = difference(&expected, &produced, case) {
        return Err(format!(
            "{where_:?}\n    sample {}\n    read   {}",
            text(&expected),
            text(&produced)
        ));
    }
    Ok(())
}

#[test]
fn xtypes_7_3_1_2_1_12_3_a_bounded_sequence_of_a_boxed_string_is_read() {
    reads_the_octets_of_the_table("sequence-string", "a bounded sequence of a boxed String");
}

#[test]
fn xtypes_7_3_1_2_1_12_3_a_bounded_sequence_of_a_wide_string_is_read() {
    reads_the_octets_of_the_table(
        "sequence-wstring",
        "the same through the wide character width",
    );
}

#[test]
fn xtypes_7_3_1_2_1_12_3_a_bounded_map_of_holder_entries_is_read() {
    reads_the_octets_of_the_table(
        "map-bounded",
        "a bounded map, whose entries are a holder class",
    );
}
