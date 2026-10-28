// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The corpus cases that carry two IDL files, as a test.
//!
//! Every other corpus case is one type: a value goes in, bytes come out,
//! and the same type reads them back. That measures a round trip and
//! nothing about type evolution, which is the one thing `@mutable` and
//! `@appendable` exist for — a new subscriber meeting an old publisher.
//!
//! An evolution case is two. `old.idl` with `old.json` is the writer;
//! `type.idl` is the reader; and what the reader hands back is recorded
//! in the case's `## Evolution` table, in every encoding and both byte
//! orders. XTypes 1.3 §7.4.1.2.3 and §7.2.4.4.8, Table 19.
//!
//! A refusal is a recorded outcome like any other. `evolution-final-tail`
//! is a pair Table 19 forbids — two final types must have the same set
//! of member IDs — and the row that says so is the assertion.
//!
//! ## Three columns, one record
//!
//! The table in each README was the Rust column's answer and nothing
//! else's. It is all three columns' now: `the_c_and_python_columns_read
//! _the_old_bytes_too` runs the same fourteen pairs through the C and
//! Python generators and holds them to the same rows. Three columns
//! against one record is what makes the record a claim about the format
//! rather than about one implementation of it.
//!
//! This is also the one cross-column read that is not a longer road to a
//! round trip. Over a single model the three columns write identical
//! bytes — `three_column_gate.rs` compares all 47075 payload slots — so
//! column X's bytes into column Y's reader is Y's own reader over the
//! same input. Here the two ends do not share a model, so the reader is
//! handed bytes it would never have produced.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::unwrap_used
)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_probe::canonical::text;
use zerodds_wire_probe::decode::decode;
use zerodds_wire_probe::encode::{encode, model_from_idl};
use zerodds_wire_probe::value::ProbeRequest;

/// How many cases carry two IDL files.
///
/// Asserted rather than counted, for the reason `interop_corpus.rs`
/// gives: a walk that silently found nothing passes with an empty loop.
const EVOLUTION_CASES: usize = 15;

const MODES: [(&str, Encoding, Endianness); 4] = [
    (
        "--encoding xcdr2 --endian le",
        Encoding::PlainCdr2,
        Endianness::Little,
    ),
    (
        "--encoding xcdr2 --endian be",
        Encoding::PlainCdr2,
        Endianness::Big,
    ),
    (
        "--encoding xcdr1 --endian le",
        Encoding::PlainCdr1,
        Endianness::Little,
    ),
    (
        "--encoding xcdr1 --endian be",
        Encoding::PlainCdr1,
        Endianness::Big,
    ),
];

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/interop-corpus")
}

/// The `## Evolution` rows of one README, keyed by the flag string.
fn evolution_rows(readme: &str, name: &str) -> BTreeMap<String, String> {
    let mut rows = BTreeMap::new();
    let Some((_, section)) = readme.split_once("## Evolution") else {
        panic!("{name}/README.md: a case with an `old.idl` and no `## Evolution` section");
    };
    for line in section.lines() {
        let Some(rest) = line.strip_prefix("| `--") else {
            continue;
        };
        let Some((flag, cell)) = rest.split_once("` | ") else {
            panic!("{name}/README.md: `{line}` is not an evolution row");
        };
        let cell = cell
            .strip_suffix(" |")
            .unwrap_or_else(|| panic!("{name}/README.md: `{line}` does not end the row"));
        let previous = rows.insert(format!("--{flag}"), cell.to_owned());
        assert!(
            previous.is_none(),
            "{name}/README.md: two evolution rows for `--{flag}`"
        );
    }
    rows
}

#[test]
fn every_evolution_case_reads_what_its_readme_records() {
    let root = corpus_root();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap_or_else(|error| panic!("{}: {error}", root.display()))
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.join("old.idl").is_file())
        .collect();
    entries.sort();

    let mut trouble = String::new();
    for dir in &entries {
        let name = dir
            .file_name()
            .expect("a case directory has a name")
            .to_string_lossy()
            .into_owned();
        let read = |file: &str| {
            std::fs::read_to_string(dir.join(file))
                .unwrap_or_else(|error| panic!("{name}/{file}: {error}"))
        };
        let type_name = read("type").trim().to_owned();
        let recorded = evolution_rows(&read("README.md"), &name);
        let old = model_from_idl(&read("old.idl"), &format!("{name}/old.idl"))
            .unwrap_or_else(|error| panic!("{name}/old.idl: {error}"));
        let new = model_from_idl(&read("type.idl"), &format!("{name}/type.idl"))
            .unwrap_or_else(|error| panic!("{name}/type.idl: {error}"));
        let sample: serde_json::Value = serde_json::from_str(&read("old.json"))
            .unwrap_or_else(|error| panic!("{name}/old.json: {error}"));

        for (flag, encoding, endianness) in &MODES {
            let where_ = format!("{name} {flag}");
            let Some(cell) = recorded.get(*flag) else {
                writeln!(trouble, "{where_}: no evolution row for this mode")
                    .expect("a String never fails to write");
                continue;
            };
            let request = ProbeRequest::new(&type_name, *encoding, *endianness);
            let bytes = match encode(&old, &request, &sample) {
                Ok(bytes) => bytes,
                Err(error) => {
                    check(&mut trouble, &where_, cell, &format!("**no** — {error}"));
                    continue;
                }
            };
            let produced = match decode(&new, &request, &bytes) {
                Ok(value) => format!("`{}`", text(&value)),
                Err(error) => format!("**no** — {error}"),
            };
            check(&mut trouble, &where_, cell, &produced);
        }
    }

    assert_eq!(
        entries.len(),
        EVOLUTION_CASES,
        "{} case(s) carry an `old.idl` and this test expects {EVOLUTION_CASES}",
        entries.len()
    );
    assert!(trouble.is_empty(), "\n{trouble}");
}

fn check(trouble: &mut String, where_: &str, recorded: &str, produced: &str) {
    if recorded != produced {
        writeln!(
            trouble,
            "{where_}:\n  the README records {recorded}\n  the probe read    {produced}"
        )
        .expect("a String never fails to write");
    }
}

/// Table 9, one row at a time, against the type each row is about.
///
/// The corpus case proves the values come back through a real stream;
/// this proves the table itself, including the rows no vendor was asked
/// about because no vendor's IDL has the construct.
#[test]
fn table_nine_gives_every_kind_the_value_the_clause_names() {
    let ast = model_from_idl(
        "enum Colour { @value(7) AMBER, @value(0) RED };
         @bit_bound(8) bitmask Flags { @position(0) LOW, @position(7) HIGH };
         bitset Bits { bitfield<3> a; bitfield<5> b; };
         @final union Choice switch (long) { case 1: long whole; default: double frac; };
         @final union Unselected switch (long) { case 1: long whole; };
         typedef long Counter;
         @final struct Inner { long n; string t; };
         @final struct Every {
             octet by; boolean bo; char c8; wchar c16; short i16; long long i64;
             unsigned long long u64; float f32; double f64; string s; wstring w;
             Colour e; Flags f; Bits t; Choice u; Unselected v; Counter al;
             sequence<long> sq; long arr[2][3]; map<long, long> m;
             fixed<5,2> fx; Inner nested;
         };",
        "table9.idl",
    )
    .expect("the model");
    let type_ = ast
        .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
            "Every".to_string(),
        ]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .expect("Every");

    let value = zerodds_wire_probe::default::of_type(&ast, type_, "Every").expect("a default");
    assert_eq!(
        text(&value),
        concat!(
            r#"{"al":0,"arr":[[0,0,0],[0,0,0]],"bo":false,"by":0,"c16":"\u0000","#,
            r#""c8":"\u0000","e":"AMBER","f":[],"f32":0.0,"f64":0.0,"fx":"0.00","#,
            r#""i16":0,"i64":"0","m":[],"nested":{"n":0,"t":""},"s":"","sq":[],"#,
            r#""t":{"a":0,"b":0},"u":{"_d":0,"frac":0.0},"u64":"0","v":{"_d":0},"w":""}"#
        )
    );

    // The default is a value of the type it is a default for, and the
    // canonical form is idempotent over it: a default that did not
    // canonicalise to itself would compare unequal to the very sample it
    // stands in for.
    let again = zerodds_wire_probe::canonicalise(&ast, type_, &value, "Every").expect("canonical");
    assert_eq!(text(&again), text(&value));
}

/// An explicit `@default` is the member's default, not Table 9's.
///
/// §7.2.2.4.4.4.9: "If the definition of the aggregated type does not
/// provide an explicit default value for a member then the default value
/// for that member is implicit." RTI and Fast DDS both honour it;
/// Cyclone's `idlc` does not implement the annotation at all.
#[test]
fn an_explicit_default_beats_table_nine() {
    let ast = model_from_idl(
        "enum Colour { RED, GREEN, BLUE };
         @final struct S {
             @default(42) long a;
             @default(\"hi\") string b;
             @default(TRUE) boolean c;
             @default(1.5) double d;
             @default(BLUE) Colour e;
         };",
        "default.idl",
    )
    .expect("the model");
    let type_ = ast
        .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
            "S".to_string(),
        ]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .expect("S");
    let value = zerodds_wire_probe::default::of_type(&ast, type_, "S").expect("a default");
    assert_eq!(
        text(&value),
        r#"{"a":42,"b":"hi","c":true,"d":1.5,"e":"BLUE"}"#
    );
}

// -- the other two columns ------------------------------------------------

/// The Python column, old writer to new reader: two probes, one pipe.
///
/// The same shape as [`c_evolution`], over `wireprobe-zerodds-python-walk`
/// instead of `wireprobe-zerodds-c`: the write probe is generated from
/// `old.idl` and the read probe from `type.idl`, so the bytes the reader
/// is handed are bytes its own model would never have produced. Direct
/// invocation and not a batch runner — fifty-six interpreter start-ups
/// over fourteen cases and four modes is not the eleven-thousand-case
/// corpus `wireprobe-python-matrix` exists to spend once per shard
/// instead of once per cell.
fn python_evolution(
    root: &Path,
    dir: &Path,
    type_name: &str,
    flag: &str,
    directory: &Path,
) -> Result<String, String> {
    let probe = root
        .join("tools")
        .join("wireprobe")
        .join("wireprobe-zerodds-python-walk");
    let (encoding, endian) = match flag {
        "--encoding xcdr2 --endian le" => ("xcdr2", "le"),
        "--encoding xcdr2 --endian be" => ("xcdr2", "be"),
        "--encoding xcdr1 --endian le" => ("xcdr1", "le"),
        "--encoding xcdr1 --endian be" => ("xcdr1", "be"),
        other => panic!("`{other}` is not one of the four modes"),
    };
    let bytes = directory.join(format!("old-python-{encoding}-{endian}.bin"));
    let written = std::process::Command::new(&probe)
        .args(["--idl".as_ref(), dir.join("old.idl").as_os_str()])
        .args(["--type", type_name])
        .args(["--values".as_ref(), dir.join("old.json").as_os_str()])
        .args(["--encoding", encoding, "--endian", endian])
        .args(["--out".as_ref(), bytes.as_os_str()])
        .output()
        .map_err(|error| format!("the Python probe could not be started: {error}"))?;
    if !written.status.success() {
        return Err(format!(
            "the old model would not write: {}",
            first_line(&written.stderr)
        ));
    }
    let values = directory.join(format!("new-python-{encoding}-{endian}.json"));
    let read = std::process::Command::new(&probe)
        .args(["--idl".as_ref(), dir.join("type.idl").as_os_str()])
        .args(["--type", type_name])
        .args(["--in".as_ref(), bytes.as_os_str()])
        .args(["--encoding", encoding, "--endian", endian])
        .args(["--values-out".as_ref(), values.as_os_str()])
        .output()
        .map_err(|error| format!("the Python probe could not be started: {error}"))?;
    if !read.status.success() {
        return Ok(format!("**no** — {}", first_line(&read.stderr)));
    }
    let json = std::fs::read_to_string(&values).map_err(|error| format!("{error}"))?;
    let found: serde_json::Value =
        serde_json::from_str(&json).map_err(|error| format!("the probe's JSON: {error}"))?;
    // Canonicalised here and not in Python, for the same reason
    // `c_evolution` canonicalises the C probe's `--values-out` in Rust:
    // the README records the canonical form of
    // `crates/wire-probe/src/canonical.rs`, and a second canonicaliser
    // in a foreign column would be a second chance to disagree about a
    // spelling rather than about a value.
    let new = model_from_idl(
        &std::fs::read_to_string(dir.join("type.idl")).map_err(|error| format!("{error}"))?,
        "type.idl",
    )
    .map_err(|error| format!("{error}"))?;
    let type_ = new
        .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(
            type_name
                .split("::")
                .filter(|segment| !segment.is_empty())
                .map(std::string::ToString::to_string)
                .collect(),
        ))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .ok_or_else(|| format!("`{type_name}` is not in the new model"))?;
    let value = zerodds_wire_probe::canonicalise(&new, type_, &found, "sample")
        .map_err(|error| format!("what came back is not a value of this type: {error}"))?;
    Ok(format!("`{}`", text(&value)))
}

/// One mode of one case through the C column: two probes, one pipe.
///
/// The write probe is generated from `old.idl` and the read probe from
/// `type.idl`, so the bytes the reader is handed are bytes its own model
/// would never have produced. That is what makes this the one
/// cross-column read that is not a longer road to a round trip: for a
/// single model the three columns write identical bytes and a column
/// reading another's is the same decoder over the same input, and here
/// the two ends do not share a model at all.
fn c_evolution(
    root: &Path,
    dir: &Path,
    type_name: &str,
    flag: &str,
    directory: &Path,
) -> Result<String, String> {
    let probe = root
        .join("tools")
        .join("wireprobe")
        .join("wireprobe-zerodds-c");
    let (encoding, endian) = match flag {
        "--encoding xcdr2 --endian le" => ("xcdr2", "le"),
        "--encoding xcdr2 --endian be" => ("xcdr2", "be"),
        "--encoding xcdr1 --endian le" => ("xcdr1", "le"),
        "--encoding xcdr1 --endian be" => ("xcdr1", "be"),
        other => panic!("`{other}` is not one of the four modes"),
    };
    let bytes = directory.join(format!("old-{encoding}-{endian}.bin"));
    let written = std::process::Command::new(&probe)
        .args(["--idl".as_ref(), dir.join("old.idl").as_os_str()])
        .args(["--type", type_name])
        .args(["--values".as_ref(), dir.join("old.json").as_os_str()])
        .args(["--encoding", encoding, "--endian", endian])
        .args(["--out".as_ref(), bytes.as_os_str()])
        .output()
        .map_err(|error| format!("the C probe could not be started: {error}"))?;
    if !written.status.success() {
        return Err(format!(
            "the old model would not write: {}",
            first_line(&written.stderr)
        ));
    }
    let values = directory.join(format!("new-{encoding}-{endian}.json"));
    let read = std::process::Command::new(&probe)
        .args(["--idl".as_ref(), dir.join("type.idl").as_os_str()])
        .args(["--type", type_name])
        .args(["--in".as_ref(), bytes.as_os_str()])
        .args(["--encoding", encoding, "--endian", endian])
        .args(["--values-out".as_ref(), values.as_os_str()])
        .output()
        .map_err(|error| format!("the C probe could not be started: {error}"))?;
    if !read.status.success() {
        return Ok(format!("**no** — {}", first_line(&read.stderr)));
    }
    let json = std::fs::read_to_string(&values).map_err(|error| format!("{error}"))?;
    let found: serde_json::Value =
        serde_json::from_str(&json).map_err(|error| format!("the probe's JSON: {error}"))?;
    // Canonicalised here and not in C: the README records the canonical
    // form of `crates/wire-probe/src/canonical.rs`, and a second
    // canonicaliser in the C column would be a second chance to disagree
    // about a spelling rather than about a value.
    let new = model_from_idl(
        &std::fs::read_to_string(dir.join("type.idl")).map_err(|error| format!("{error}"))?,
        "type.idl",
    )
    .map_err(|error| format!("{error}"))?;
    let type_ = new
        .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(
            type_name
                .split("::")
                .filter(|segment| !segment.is_empty())
                .map(std::string::ToString::to_string)
                .collect(),
        ))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .ok_or_else(|| format!("`{type_name}` is not in the new model"))?;
    let value = zerodds_wire_probe::canonicalise(&new, type_, &found, "sample")
        .map_err(|error| format!("what came back is not a value of this type: {error}"))?;
    Ok(format!("`{}`", text(&value)))
}

fn first_line(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr)
        .lines()
        .next()
        .unwrap_or("the probe said nothing")
        .trim()
        .to_string()
}

/// The C and Python columns over the same fourteen pairs.
///
/// `every_evolution_case_reads_what_its_readme_records` is the Rust
/// column against the same table. Three columns against one record is
/// what makes the record a claim about the *format* rather than about
/// one implementation of it.
///
/// The C half runs in the full tier only, for the reason
/// `three_column_gate.rs` gives: it is fifty-six generated translation
/// units compiled under `-Werror`, which is a minute that
/// `cargo test --workspace` should not spend on every run. What the fast
/// tier therefore leaves unmeasured is said here and in the case
/// READMEs rather than left to be discovered.
#[test]
fn the_c_and_python_columns_read_the_old_bytes_too() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the repository root")
        .to_path_buf();
    let full = matches!(std::env::var("ZERODDS_GATE_TIER").as_deref(), Ok("full"));
    let directory = root.join("target").join("evolution-columns");
    std::fs::create_dir_all(&directory).expect("a scratch directory");

    let corpus = corpus_root();
    let mut cases: Vec<PathBuf> = std::fs::read_dir(&corpus)
        .unwrap_or_else(|error| panic!("{}: {error}", corpus.display()))
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.join("old.idl").is_file())
        .collect();
    cases.sort();
    assert_eq!(cases.len(), EVOLUTION_CASES);

    let mut trouble = String::new();
    let (mut c_slots, mut python_slots) = (0usize, 0usize);
    for dir in &cases {
        let name = dir
            .file_name()
            .expect("a case directory has a name")
            .to_string_lossy()
            .into_owned();
        let readme = std::fs::read_to_string(dir.join("README.md"))
            .unwrap_or_else(|error| panic!("{name}/README.md: {error}"));
        let recorded = evolution_rows(&readme, &name);
        let type_name = std::fs::read_to_string(dir.join("type"))
            .unwrap_or_else(|error| panic!("{name}/type: {error}"))
            .trim()
            .to_owned();

        for (flag, _, _) in &MODES {
            let cell = recorded
                .get(*flag)
                .unwrap_or_else(|| panic!("{name}: no evolution row for `{flag}`"));

            python_slots += 1;
            match python_evolution(&root, dir, &type_name, flag, &directory) {
                // A refusal is compared by the fact of it and not by its
                // sentence: the Python column writes its own English and
                // the README carries the Rust column's.
                Ok(produced)
                    if produced == *cell
                        || (produced.starts_with("**no**") && cell.starts_with("**no**")) => {}
                Ok(produced) => {
                    let _ = writeln!(
                        trouble,
                        "{name} {flag}: python\n  the README records {cell}\n  \
                         the Python column read {produced}"
                    );
                }
                Err(error) => {
                    let _ = writeln!(trouble, "{name} {flag}: python — {error}");
                }
            }

            if !full {
                continue;
            }
            c_slots += 1;
            match c_evolution(&root, dir, &type_name, flag, &directory) {
                // A refusal is compared by the fact of it and not by its
                // sentence: the C column writes its own English and the
                // README carries the Rust column's.
                Ok(produced)
                    if produced == *cell
                        || (produced.starts_with("**no**") && cell.starts_with("**no**")) => {}
                Ok(produced) => {
                    let _ = writeln!(
                        trouble,
                        "{name} {flag}: c\n  the README records {cell}\n  the C column read {produced}"
                    );
                }
                Err(error) => {
                    let _ = writeln!(trouble, "{name} {flag}: c — {error}");
                }
            }
        }
    }

    println!("evolution: {python_slots} Python slot(s), {c_slots} C slot(s)");
    if !full {
        println!(
            "the C column is measured by `ZERODDS_GATE_TIER=full`; it compiles \
             {} translation unit(s) under -Werror",
            cases.len() * MODES.len() * 2
        );
    }
    assert!(trouble.is_empty(), "\n{trouble}");
}
