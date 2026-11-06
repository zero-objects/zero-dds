// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The committed fixtures for the IDL 4.2 constructs section D of
//! `docs/spec-coverage/idl-4.2.gaps.md` records as emitted by both code
//! generators although no `.idl` in the tree carried them.
//!
//! Each file is the minimal IDL the gap scan's own probe holds for the
//! construct (`tools/idl-4.2-gap-scan/cases.tsv`, the `witness` column).
//! The scan counts a construct as present in the tree by a text search
//! alone; a hit says an IDL carries it, not that it reaches a generator.
//! This test closes that second half: `check` and the two backends the
//! page measures, over every fixture the list names.
//!
//! The list is written out rather than read off the directory so a
//! fixture that stops existing fails here — the failure the coverage
//! page's D section would otherwise only notice at the next scan.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixtures, one per section-D construct, named by the probe they
/// stand for. Adding a construct to the list means adding a file and a
/// witness to `tools/idl-4.2-gap-scan/cases.tsv`.
const FIXTURES: &[&str] = &[
    "bb1-006-const-fixed",
    "bb1-006-const-wchar",
    "bb1-006-const-wstring",
    "bb1-009-xor-expr",
    "bb1-011-shift-right",
    "bb1-015-unary-tilde",
    "bb1-017-lit-fixed",
    "bb1-017-lit-wstring",
    "bb1-017-lit-string-concat",
    "bb1-064-typedef-constr",
    "bb5-110-value-forward",
    "bb13-212-int64",
    "lex-7.2.6.2-char-escape-octal",
    "lex-7.2.6.2-char-escape-hex",
    "lex-7.2.6.2-wchar-escape-unicode",
    "lex-7.2.6.4-float-leading-dot",
    "lex-7.2.6.4-float-trailing-dot",
    "lex-7.2.6.5-fixed-literal",
    "lex-7.3-define-function",
    "lex-7.3-ifndef",
    "lex-7.3-if-else",
    "lex-7.3-elif",
    "lex-7.3-defined-operator",
    "lex-7.3-undef",
    "lex-7.3-token-paste",
    "lex-7.3-line-directive",
];

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("idl-4.2-gaps")
}

#[test]
fn idl_7_2_6_2_1_every_idl_4_2_section_d_fixture_passes_check_and_both_backends() {
    let dir = fixtures();
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("idl-4.2-gaps");
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("the output directory");

    for stem in FIXTURES {
        let path = dir.join(format!("{stem}.idl"));
        let check = Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"))
            .arg("check")
            .arg(&path)
            .output()
            .expect("idlc check runs");
        assert!(
            check.status.success(),
            "{stem}: check failed:\n{}",
            String::from_utf8_lossy(&check.stderr)
        );

        for lang in ["rust", "c"] {
            let generate = Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"))
                .args(["generate", "--lang", lang, "--out"])
                .arg(&out)
                .arg(&path)
                .output()
                .expect("idlc generate runs");
            assert!(
                generate.status.success(),
                "{stem}: generate --lang {lang} failed:\n{}",
                String::from_utf8_lossy(&generate.stderr)
            );
        }
    }
}

#[test]
fn idl_7_4_8_3_the_fixture_directory_holds_exactly_the_named_fixtures() {
    let mut present: Vec<String> = std::fs::read_dir(fixtures())
        .expect("the fixture directory")
        .map(|entry| entry.expect("a fixture entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "idl"))
        .map(|path| {
            path.file_stem()
                .expect("a file stem")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    present.sort();
    let mut named: Vec<String> = FIXTURES.iter().map(|stem| (*stem).to_string()).collect();
    named.sort();
    assert_eq!(
        present, named,
        "the fixture directory and the list disagree"
    );
}

/// OMG IDL 4.2 Annex A: every probe of the gap scan
/// (`tools/idl-4.2-gap-scan/cases.tsv`), which covers each of the 227
/// numbered productions and the semantic rules beside them, gets the answer
/// its `expect` column takes from the specification — `accept` without an
/// error, `reject` with one.
///
/// Read as `check --strict` reads it, OMG IDL 4.2 and nothing else, except
/// for the `dialect` probes: those stand for the vendor constructs a default
/// run admits, and are read as `check` reads them.
#[test]
fn idl_annex_a_every_probe_of_the_gap_scan_gets_the_specifications_answer() {
    let cases =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/idl-4.2-gap-scan/cases.tsv");
    let text = std::fs::read_to_string(&cases).expect("cases.tsv");
    let mut productions = std::collections::BTreeSet::new();
    let mut wrong = Vec::new();
    let mut probes = 0;
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let [id, axis, refs, expect, _witness, idl] = fields[..] else {
            panic!("a line of cases.tsv has {} fields: {line}", fields.len());
        };
        probes += 1;
        if axis == "grammar" {
            for reference in refs.split(',') {
                if let Ok(number) = reference.trim().parse::<u16>() {
                    productions.insert(number);
                }
            }
        }
        let source = idl.replace("<NL>", "\n");
        let dialect = if axis == "dialect" {
            zerodds_idl_parse::Dialect::ALL_VENDORS
        } else {
            zerodds_idl_parse::Dialect::STRICT
        };
        // `scan.sh` writes `other.idl` beside the probe for the two
        // `#include` probes; this is the same file.
        let loader = zerodds_idl_parse::MemoryLoader::new()
            .with("other.idl", "struct Included { long m; };\n");
        let composed = zerodds_idl_compose::compose_source_with(
            &format!("{id}.idl"),
            &source,
            &loader,
            zerodds_idl_compose::ComposeOptions {
                dialect,
                ..zerodds_idl_compose::ComposeOptions::default()
            },
        );
        let errors: Vec<_> = composed
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.is_error())
            .collect();
        let accepted = errors.is_empty();
        let agrees = match expect {
            "accept" => accepted,
            "reject" => !accepted,
            other => panic!("{id}: expect `{other}` is neither accept nor reject"),
        };
        if !agrees {
            wrong.push(format!("{id} ({expect}): {errors:?}"));
        }
    }
    let missing: Vec<u16> = (1..=227).filter(|n| !productions.contains(n)).collect();
    assert!(
        missing.is_empty(),
        "productions with no grammar probe: {missing:?}"
    );
    assert!(probes > 300, "{probes} probes");
    assert!(
        wrong.is_empty(),
        "{} probe(s):\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}
