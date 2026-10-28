// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Every policy axis, spelled as a flag, through every one of the five
//! renderers.
//!
//! ## What this is for
//!
//! Moving the command line into the stem widened it: four of the five
//! columns took `--dialect` and nothing else, and now all five take
//! every axis a `LayoutKey` has. That is sound only because no axis is
//! answered by a renderer — each lands in the key or in `CodecOptions`,
//! which the shared walk and every harness read — and "sound only
//! because" is exactly the shape of claim that has to be measured.
//!
//! A flag a column accepts and then ignores is the silent-drop shape
//! this suite exists to find: the caller selects a policy, the column
//! writes the octets of the policy it was not given, and the ledger
//! records a divergence that is the harness's own. So for each column
//! and each axis, this requires **some** corpus case whose generated
//! source moves when that axis alone moves.
//!
//! ## The claim, exactly
//!
//! For a column *C* and an axis *a* with command-line values *v₁* and
//! *v₂*: there is a case in the corpus for which *C* generates
//! different source under `--a v₁` than under `--a v₂`, everything else
//! held at the default.
//!
//! The converse — an axis that changes the source of every case — is
//! not asserted and would be wrong: `--fixed-digits` touches a case
//! with a `fixed<P,S>` in it and nothing else.
//!
//! The values are written as the strings a caller types, and go in
//! through [`zerodds_wireprobe_gen::parse`], so this also holds the
//! usage text to the parsers: a value the usage lists and the parser
//! does not take is refused identically at both settings, no case moves,
//! and the axis is reported as unreached.
//!
//! `wireprobe-zerodds-c-gen/tests/axes_reach_the_generator.rs` makes the
//! stronger claim for the C column, against the octets the Rust column
//! actually produces. This is the weaker claim for all five, and it is
//! the one that catches an axis that was parsed and dropped.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::unwrap_used
)]

use std::path::{Path, PathBuf};
use zerodds_wireprobe_gen::{Column, Session, parse};

/// One corpus case.
struct Case {
    name: String,
    directory: PathBuf,
    type_name: String,
}

fn corpus() -> Vec<Case> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("tests")
        .join("interop-corpus");
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&root)
        .expect("the corpus is where the tree says it is")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    entries.sort();
    let mut cases = Vec::new();
    for directory in entries {
        if !directory.join("type.idl").is_file()
            || !directory.join("values.json").is_file()
            || !directory.join("type").is_file()
        {
            continue;
        }
        cases.push(Case {
            name: directory
                .file_name()
                .expect("a corpus directory has a name")
                .to_string_lossy()
                .into_owned(),
            type_name: std::fs::read_to_string(directory.join("type"))
                .expect("the type file reads")
                .trim()
                .to_string(),
            directory,
        });
    }
    assert!(!cases.is_empty(), "the corpus is empty");
    cases
}

/// The source a column generates for one command line, or the reason it
/// did not.
///
/// A refusal is part of the answer and not a skip: two settings that
/// refuse for different reasons have said different things, and a
/// setting that refuses where the other emits has certainly said
/// something different.
fn source_of<C: Column>(case: &Case, encoding: &str, flag: &str, value: &str) -> String {
    let mut argv = vec![
        C::BINARY.to_string(),
        "--idl".to_string(),
        case.directory.join("type.idl").display().to_string(),
        "--type".to_string(),
        case.type_name.clone(),
        "--values".to_string(),
        case.directory.join("values.json").display().to_string(),
        "--encoding".to_string(),
        encoding.to_string(),
        // Never written to: nothing below calls `run`.
        "--out-dir".to_string(),
        String::new(),
    ];
    if !flag.is_empty() {
        argv.push(flag.to_string());
        argv.push(value.to_string());
    }
    let args = match parse::<C>(argv) {
        Ok(args) => args,
        Err(refusal) => return format!("!{}: {}", refusal.code(), refusal.message()),
    };
    match Session::open(&args) {
        Ok(session) => match C::render(&session) {
            Ok(files) => files
                .iter()
                .map(|file| format!("=== {}\n{}", file.name, file.text))
                .collect::<Vec<_>>()
                .join("\n"),
            Err(refusal) => format!("!{}: {}", refusal.code(), refusal.message()),
        },
        Err(refusal) => format!("!{}: {}", refusal.code(), refusal.message()),
    }
}

/// One axis: the flag, the encoding it has a question to answer under,
/// and two of the values the usage text lists for it.
///
/// PL_CDR1 is where a parameter header lives, so the three axes about
/// one have no question under PL_CDR2 and are asked under XCDR1.
/// PL_CDR2 reaches the rest: it has member headers, delimiters and
/// length codes at once.
///
/// `--dialect` is not here: it is not one axis but a preset over all of
/// them, and every column took it before this change.
const AXES: &[(&str, &str, &str, &str)] = &[
    ("--length-codes", "pl-cdr2", "compact", "universal-nextint"),
    (
        "--collection-delimiters",
        "pl-cdr2",
        "specification",
        "never",
    ),
    (
        "--wstring-length",
        "pl-cdr2",
        "specification",
        "character-count",
    ),
    (
        "--wchar-framing",
        "pl-cdr2",
        "fixed",
        "giop-length-prefixed",
    ),
    ("--wchar-width", "pl-cdr2", "2", "4"),
    ("--map-delimiter", "pl-cdr2", "either-half", "value-only"),
    ("--wstring-units", "pl-cdr2", "stream", "big-endian-always"),
    ("--pid-flags", "pl-cdr1", "sentinel-must-understand", "bare"),
    ("--enum-holder", "pl-cdr2", "declared", "bit-bound-dropped"),
    (
        "--union-labels",
        "pl-cdr2",
        "sign-extended",
        "negative-unmatched-below-32",
    ),
    ("--map-entries", "pl-cdr2", "declaration", "ascending-key"),
    ("--enumerator-value", "pl-cdr2", "declared", "ordinal"),
    (
        "--union-framing",
        "pl-cdr1",
        "specification",
        "branches-from-zero",
    ),
    (
        "--short-pid-range",
        "pl-cdr1",
        "up-to-reserved",
        "all-fourteen-bits",
    ),
    ("--fixed-digits", "pl-cdr2", "declared", "value-precision"),
    (
        "--map-length",
        "pl-cdr2",
        "sequence-equivalent",
        "rule-15-omits",
    ),
];

fn axis_reaches<C: Column>() {
    let cases = corpus();
    let mut missed = Vec::new();
    for (flag, encoding, low, high) in AXES {
        let moved = cases.iter().any(|case| {
            source_of::<C>(case, encoding, flag, low) != source_of::<C>(case, encoding, flag, high)
        });
        if !moved {
            missed.push(*flag);
        }
    }
    assert!(
        missed.is_empty(),
        "{}: no corpus case moves under {missed:?} — the column takes the flag and \
         nothing downstream of it changes, which is a policy selected and not applied",
        C::BINARY
    );
}

#[test]
fn every_axis_reaches_the_c_column() {
    axis_reaches::<zerodds_wireprobe_c_gen::Renderer>();
}

#[test]
fn every_axis_reaches_the_cpp_column() {
    axis_reaches::<zerodds_wireprobe_cpp_gen::Renderer>();
}

#[test]
fn every_axis_reaches_the_csharp_column() {
    axis_reaches::<zerodds_wireprobe_csharp_gen::Renderer>();
}

#[test]
fn every_axis_reaches_the_java_column() {
    axis_reaches::<zerodds_wireprobe_java_gen::Renderer>();
}

#[test]
fn every_axis_reaches_the_typescript_column() {
    axis_reaches::<zerodds_wireprobe_typescript_gen::Renderer>();
}

/// The comparison is not one that has only ever passed.
///
/// Two settings that certainly differ have to come out different; a
/// `source_of` that returned a constant would pass every check above and
/// fail this one.
#[test]
fn the_comparison_bites() {
    let cases = corpus();
    let case = cases
        .iter()
        .find(|case| case.name == "primitive-octet")
        .expect("`primitive-octet` is in the corpus");
    type C = zerodds_wireprobe_c_gen::Renderer;
    assert_ne!(
        source_of::<C>(case, "plain-cdr1", "", ""),
        source_of::<C>(case, "delimited-cdr", "", ""),
        "PLAIN_CDR1 and DELIMITED_CDR generate the same C, so the comparison is blind"
    );
    // Byte order and not `--dialect`: `primitive-octet` is two octets
    // in a `@final` struct and no dialect moves it, which is a true
    // answer and a useless probe. The byte order is baked into the
    // single-form generator as `ZDW_LITTLE` or `ZDW_BIG`, so it moves
    // this case and every other.
    assert_ne!(
        source_of::<C>(case, "pl-cdr2", "--endian", "le"),
        source_of::<C>(case, "pl-cdr2", "--endian", "be"),
        "the two byte orders generate the same C, so the comparison is blind"
    );
}

/// Every value the usage text lists is a value the parser takes.
///
/// The two are written in different places — the usage text in
/// `zerodds_wireprobe_gen::usage`, the values in
/// `zerodds_ast_layout`'s `parse` functions — and a value listed and
/// not taken is a flag a caller cannot use. It is checked here rather
/// than reasoned about, because the axis checks above would report such
/// a value as "the axis does not reach the generator" and send a reader
/// looking in the wrong place.
#[test]
fn the_usage_text_lists_values_the_parser_takes() {
    let cases = corpus();
    let case = &cases[0];
    type C = zerodds_wireprobe_c_gen::Renderer;
    let mut refused = Vec::new();
    for (flag, encoding, low, high) in AXES {
        for value in [low, high] {
            let answer = source_of::<C>(case, encoding, flag, value);
            if answer.starts_with("!1: `") {
                refused.push(format!("{flag} {value}: {answer}"));
            }
        }
    }
    assert!(
        refused.is_empty(),
        "the usage text lists values the parser refuses: {refused:#?}"
    );
}
