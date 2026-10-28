// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The whole interop corpus through a C compiler.
//!
//! The six oldest columns got their corpus compile gate back after the
//! `idl-<lang>` crates were folded into `lang-rules`; the C column never
//! had one, and nothing measured its generated source over the corpus.
//! This is that gate: every case renders through
//! `generate::source(…, Language::C, …)` — the declarations and the
//! codec — behind the preamble `zerodds-idlc` writes for C, and goes
//! through the compiler with the flags `tools/wireprobe/wireprobe-zerodds-c`
//! compiles the measured C column with, minus the link step.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

#[path = "support/corpus_frontend.rs"]
mod corpus_frontend;

use std::process::Command;

use zerodds_ast::Ast;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::Language;

#[path = "support/corpus.rs"]
mod corpus_harness;

/// IDL in, one C header out — naming the layer that refused instead of
/// panicking.
fn try_generate(idl: &str, origin: &str) -> Result<String, String> {
    let parsed = zerodds_idl_parse::parse(idl);
    if !parsed.diagnostics().is_empty() {
        return Err(format!("parse: {:?}", parsed.diagnostics()));
    }
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), origin);
    if !lowered.diagnostics.is_empty() {
        return Err(format!("lower: {:?}", lowered.diagnostics));
    }
    let mut ast: Ast = lowered.ast;
    let report = ast
        .resolve_all_by_name()
        .map_err(|error| format!("resolve: {error:?}"))?;
    if !report.is_clean() {
        return Err(format!("resolve: broken references {:?}", report.broken));
    }
    let body = zerodds_lang_rules::generate::source(
        &ast,
        Language::C,
        LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
    )
    .map_err(|error| format!("generate: {error:?}"))?;
    // `tools/idlc/src/generate.rs::preamble` for C.
    Ok(format!(
        "#ifndef ZD_GEN_H\n#define ZD_GEN_H\n#include \"zerodds_wire.h\"\n\
         #define ZD_ENDIANNESS ZDW_LITTLE\n{body}\n#endif\n"
    ))
}

// -- the corpus ----------------------------------------------------------

/// How many corpus cases have to reach the compiler for this gate to
/// mean anything: a generator that refused everything would otherwise
/// pass it.
const MUST_RENDER: usize = 350;

/// The names the C column itself refuses, each with the reason. A list
/// and not a filter, as in the C++ gate: a case here that starts
/// rendering fails.
///
/// ISO C has no structure without a member: the grammar of C99 §6.7.2.1
/// asks for at least one struct-declaration, and C23 §6.7.2.1p8 leaves a
/// structure with no named member undefined. The column answers
/// `NoLanguageForm` for one (`ffbe848f6d3`) instead of emitting a member
/// the IDL does not have.
const REFUSED: &[(&str, &str)] = &[
    ("empty-struct", "an aggregate with no members"),
    ("empty-struct-as-base", "an aggregate with no members"),
    ("empty-struct-in-sequence", "an aggregate with no members"),
];

/// The cases whose generated C a C compiler rejects, each with the
/// message it answers. A list and not a filter: a case here that starts
/// compiling fails, so an entry cannot outlive its defect.
const DOES_NOT_COMPILE: &[(&str, &str)] = &[];

// -- the compiler --------------------------------------------------------

/// The C compiler: `CC` first, otherwise the first of the usual
/// spellings that answers `--version`.
fn compiler() -> Option<String> {
    if let Ok(chosen) = std::env::var("CC") {
        return Some(chosen);
    }
    ["cc", "gcc", "clang"]
        .iter()
        .find(|candidate| {
            Command::new(candidate)
                .arg("--version")
                .output()
                .is_ok_and(|out| out.status.success())
        })
        .map(|found| (*found).to_string())
}

/// One generated header through the compiler, as a consumer includes it.
fn c_compiles(cc: &str, source: &str, tag: &str) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("zd-c-corpus-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let header = dir.join("gen.h");
    let unit = dir.join("tu.c");
    std::fs::write(&header, source).map_err(|error| error.to_string())?;
    std::fs::write(&unit, "#include \"gen.h\"\n").map_err(|error| error.to_string())?;
    let runtime = corpus_harness::repository_root().join("runtime/c/include");
    let output = Command::new(cc)
        .args([
            "-std=c99",
            "-fsyntax-only",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Wconversion",
            "-Wsign-conversion",
            "-Werror",
        ])
        .arg(format!("-I{}", dir.display()))
        .arg(format!("-I{}", runtime.display()))
        .arg(&unit)
        .output()
        .map_err(|error| error.to_string())?;
    let result = if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    };
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn the_compiler_check_catches_a_deliberate_error() {
    // Without this, a green result below could come from a compiler that
    // accepts anything, or from a harness that never ran one.
    let Some(cc) = compiler() else {
        eprintln!("SKIPPED: no C compiler on the path.");
        return;
    };
    let why = c_compiles(&cc, "struct S { NoSuchTypeExists x; };\n", "deliberate")
        .expect_err("the compiler accepted a bad header");
    assert!(why.contains("NoSuchTypeExists"), "{why}");
    c_compiles(&cc, "struct S { int x; };\n", "control")
        .expect("the compiler rejected a good header");
}

/// The whole corpus through the C compiler, one invocation per case.
/// `#[ignore]` by cost, like the six sibling gates; the CI job runs it
/// with `--include-ignored`.
#[test]
#[ignore = "one C compiler invocation per corpus case; run with --include-ignored"]
fn every_corpus_case_that_renders_also_compiles() {
    let Some(cc) = compiler() else {
        eprintln!("SKIPPED: no C compiler on the path.");
        return;
    };
    let mut failures = Vec::new();
    let mut unexpectedly_fine = Vec::new();
    let mut frontend = Vec::new();
    let mut checked = 0_usize;
    for (name, path) in corpus_harness::corpus() {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&source, &path.display().to_string()) {
            continue;
        }
        let refused = REFUSED.iter().find(|(case, _)| *case == name);
        let text = match (try_generate(&source, &path.display().to_string()), refused) {
            (Ok(text), None) => text,
            (Ok(_), Some((case, why))) => {
                unexpectedly_fine.push(format!("{case}: renders now, but is listed as {why}"));
                continue;
            }
            (Err(why), None) => {
                // `parse` and `lower` are the frontend's and the same for
                // every column; anything later is this column's.
                if why.starts_with("parse:") || why.starts_with("lower:") {
                    frontend.push(format!("{name}: {why}"));
                } else {
                    failures.push(format!("{name}: {why}"));
                }
                continue;
            }
            (Err(why), Some((case, reason))) => {
                // Refused, and for the reason listed: a refusal for
                // another reason is a failure of its own.
                if !why.contains(reason) {
                    failures.push(format!("{case}: refused, but not as {reason}: {why}"));
                }
                continue;
            }
        };
        checked += 1;
        let known = DOES_NOT_COMPILE.iter().find(|(case, _)| *case == name);
        match (c_compiles(&cc, &text, &name), known) {
            (Ok(()), None) | (Err(_), Some(_)) => {}
            (Ok(()), Some((case, why))) => {
                unexpectedly_fine.push(format!("{case}: compiles now, but is listed as {why}"));
            }
            (Err(why), None) => failures.push(format!("{name}: {why}")),
        }
    }
    eprintln!(
        "{checked} corpus case(s) reached C; {} failed unexpectedly",
        failures.len()
    );
    if !frontend.is_empty() {
        eprintln!(
            "{} corpus case(s) the frontend refuses, so C never saw them:\n{frontend:#?}",
            frontend.len()
        );
    }
    assert!(
        failures.is_empty(),
        "{} of {checked} did not compile:\n{failures:#?}",
        failures.len()
    );
    assert!(unexpectedly_fine.is_empty(), "{unexpectedly_fine:#?}");
    assert!(
        checked >= MUST_RENDER,
        "only {checked} corpus cases reached the compiler; the gate says nothing \
         below {MUST_RENDER}"
    );
}
