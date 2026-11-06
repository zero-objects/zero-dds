// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Java column's reader over the octets another implementation wrote.
//!
//! `java_column_gate.rs` holds the Java column's octets against
//! `tests/interop-corpus/expected-bytes.tsv`, which measures the write
//! direction. This measures the other one: the Java probe reads the
//! table's XCDR2 little-endian octets of a case, under the dialect of
//! their first writer, and the values that come out have to be the case's
//! sample. No column of this project is read (decision 3 of
//! `internal/release/rc8-test-suite.md`); it replaces a self round trip,
//! which passed a reader wrong in the same way as its own writer.
//!
//! It catches a member dropped, a length misread, a union's discriminator
//! selecting the wrong case, an `@optional` that comes back present, a
//! Table 9 default put where a value belonged. One test per case, each
//! under the 25 s of a test with a JDK; they run in the Java codegen jobs
//! (`ci/integration_scenarios.py`, `CODEGEN["java"]`).
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
//! The rest are there because a reader measured only on the absent forms
//! would say nothing about the present ones. `primitive-longdouble` and
//! `fixed-basic` have no XCDR2 row in the table and are not here.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_wire_probe::canonical::{canonicalise, difference, text};
use zerodds_wire_probe::encode::model_from_idl;
use zerodds_wire_probe::expected::{row_of, table};

/// The encoding the octets are read in: XCDR2's plain form, the corpus's
/// own default; `wireprobe-java-corpus` covers the other framings.
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

/// The Java probe reads the table's octets of `case` back to the sample.
fn reads_the_octets_of_the_table(case: &str, purpose: &str) {
    if !java_is_here() {
        return;
    }
    if let Err(failure) = read(case) {
        panic!("the Java column does not read {case} ({purpose}): {failure}");
    }
}

fn read(case: &str) -> Result<(), String> {
    let root = repository_root();
    let probe = root.join("tools/wireprobe/wireprobe-zerodds-java");
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("java-reads-the-table");
    std::fs::create_dir_all(&scratch).expect("a directory for the octets");
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
        .output()
        .map_err(|error| format!("the probe did not start ({error})"))?;
    if !output.status.success() {
        return Err(format!(
            "the read under {dialect} refused: {}",
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .next()
                .unwrap_or_default()
        ));
    }

    // Both sides through `canonical.rs`, and not a text comparison: the
    // spelling of a float and the escape of a character are the printer's,
    // and the one form two implementations' values can be compared in is
    // the canonical one.
    let ast = model_from_idl(
        &std::fs::read_to_string(directory.join("type.idl")).expect("the IDL"),
        &directory.join("type.idl").to_string_lossy(),
    )
    .map_err(|error| format!("the model does not build ({error:?})"))?;
    let short_name = type_name.rsplit("::").next().ok_or("an empty type name")?;
    let root_type = ast
        .types()
        .find(|(_, node)| node.name.as_str() == short_name)
        .map(|(id, _)| id)
        .ok_or_else(|| format!("`{type_name}` is not a type in this model"))?;
    let produced: serde_json::Value = serde_json::from_slice(&output.stdout)
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
            "{where_:?} under {dialect}\n    sample {}\n    read   {}",
            text(&expected),
            text(&produced)
        ));
    }
    Ok(())
}

#[test]
fn struct_appendable() {
    reads_the_octets_of_the_table(
        "struct-appendable",
        "a delimiter that may end before the members do",
    );
}

#[test]
fn struct_mutable() {
    reads_the_octets_of_the_table(
        "struct-mutable",
        "a member that may not be in the stream at all",
    );
}

#[test]
fn annotation_optional_mixed() {
    reads_the_octets_of_the_table(
        "annotation-optional-mixed",
        "@optional members, present and absent",
    );
}

#[test]
fn annotation_optional_absent() {
    reads_the_octets_of_the_table("annotation-optional-absent", "an @optional member, absent");
}

#[test]
fn string_bounded_utf8() {
    reads_the_octets_of_the_table("string-bounded-utf8", "a bound the value may not fit");
}

#[test]
fn union_appendable() {
    reads_the_octets_of_the_table("union-appendable", "a branch that is not selected");
}

#[test]
fn union_mutable() {
    reads_the_octets_of_the_table("union-mutable", "the same, framed by parameter headers");
}

#[test]
fn primitive_mixed() {
    reads_the_octets_of_the_table(
        "primitive-mixed",
        "fourteen scalars, and the alignment between them",
    );
}

#[test]
fn sequence_primitives_nonempty() {
    reads_the_octets_of_the_table(
        "sequence-primitives-nonempty",
        "a count, a loop and an element codec",
    );
}

#[test]
fn sequence_bounded_full() {
    reads_the_octets_of_the_table("sequence-bounded-full", "a sequence at its bound");
}

#[test]
fn array_1d_enum() {
    reads_the_octets_of_the_table(
        "array-1d-enum",
        "an enumeration read back to its own constant",
    );
}

#[test]
fn array_2d_primitives() {
    reads_the_octets_of_the_table("array-2d-primitives", "two dimensions, in one flat run");
}

#[test]
fn bitmask_bitbound_32() {
    reads_the_octets_of_the_table("bitmask-bitbound-32", "a holder wider than an int");
}

#[test]
fn bitset_basic() {
    reads_the_octets_of_the_table("bitset-basic", "fields packed into one anchor");
}

#[test]
fn map_key_long() {
    reads_the_octets_of_the_table("map-key-long", "an entry order the sample decides");
}

#[test]
fn map_value_struct() {
    reads_the_octets_of_the_table("map-value-struct", "an aggregate inside an entry");
}

#[test]
fn wstring_bounded_utf8() {
    reads_the_octets_of_the_table("wstring-bounded-utf8", "UTF-16 code units, counted");
}

#[test]
fn enum_sparse_values() {
    reads_the_octets_of_the_table("enum-sparse-values", "values that are not ordinals");
}

#[test]
fn inherit_two_levels() {
    reads_the_octets_of_the_table(
        "inherit-two-levels",
        "a base's members, in front of the derived type's",
    );
}

#[test]
fn evolution_mutable_optional() {
    reads_the_octets_of_the_table(
        "evolution-mutable-optional",
        "an @optional member of a @mutable type",
    );
}

#[test]
fn optional_nested_struct() {
    reads_the_octets_of_the_table("optional-nested-struct", "an absent aggregate");
}

#[test]
fn string_bounded_empty() {
    reads_the_octets_of_the_table("string-bounded-empty", "a bound and nothing in it");
}
