// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The whole interop corpus, as C++, through a C++ compiler.
//!
//! The counterpart of the `every_corpus_case_that_renders_also_compiles`
//! that `swift_end_to_end.rs`, `go_end_to_end.rs` and eight more carry.
//! The C++ column had none: `docs/OPEN-ITEMS.md` records that the six
//! oldest columns lost theirs when `zerodds-idl-<lang>` was removed, and
//! that the pipeline job which would have caught it calls a package that
//! no longer exists.
//!
//! A snapshot says the emitter produced the text it produced last time.
//! It does not say the text is C++, that every name in it resolves, or
//! that a member's type is complete where the standard requires it to
//! be — `std::array<T, N>` and a by-value member both require `T`
//! complete, and an emitter that orders its declarations wrongly
//! produces a header no consumer can include.
//!
//! The header is compiled the way a consumer includes it: a translation
//! unit whose only line is `#include "zd_types.hpp"`, under the flag set
//! `tools/wireprobe/wireprobe-zerodds-cpp` compiles the measured column
//! with. `-fsyntax-only` and no link step, because what is in question is
//! the header and not a program.
//!
//! `print_stderr` is allowed here on purpose: a skipped compiler check
//! and a frontend refusal both have to be *said*, and standard error is
//! the channel a passing test does not otherwise use.
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
use zerodds_lang_rules::adapter::interface::ServiceMapping;
use zerodds_lang_rules::rule::cpp;
use zerodds_lang_rules::{ModelGraph, apply};

#[path = "support/corpus.rs"]
mod corpus_harness;

/// IDL in, one C++ header out — naming the layer that refused instead of
/// panicking.
///
/// The declarations and the RPC binding of DDS-RPC 1.0 §7.11.1 after
/// them, which is the pair `zerodds_lang_rules::generate::declarations`
/// assembles for `Language::Cpp`, wrapped in the file the C++ generator
/// writes: `#pragma once` and `adapter::cpp::includes()`.
fn try_generate(idl: &str, origin: &str) -> Result<String, String> {
    try_generate_with(idl, origin, ServiceMapping::Basic)
}

/// [`try_generate`] for the given service mapping.
fn try_generate_with(idl: &str, origin: &str, mapping: ServiceMapping) -> Result<String, String> {
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
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
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &cpp::catalogue()).map_err(|error| format!("apply: {error}"))?;
    let mut decls = zerodds_lang_rules::adapter::cpp::cpp_items(&ast, &applied)
        .map_err(|error| format!("materialise: {error}"))?;
    decls.extend(
        zerodds_lang_rules::emit::interface_cpp::cpp_interface_items_with(&ast, mapping)
            .map_err(|error| format!("materialise: {error}"))?,
    );
    Ok(zerodds_lang_render::cpp::file(
        &zerodds_lang_cst::cpp::File {
            doc: zerodds_lang_cst::common::Doc::none(),
            pragma_once: true,
            includes: zerodds_lang_rules::adapter::cpp::includes(),
            decls,
        },
    ))
}

// -- the corpus ----------------------------------------------------------
/// How many corpus cases have to reach the compiler for this gate to
/// mean anything.
///
/// What would make it vacuous is a generator that refused everything,
/// and this is the number that stops it. 418 cases render today.
const MUST_RENDER: usize = 350;

/// The names the C++ column itself refuses, each with the reason.
///
/// A list and not a filter: a case on it that starts rendering fails
/// too — an exemption nothing checks is an exemption that outlives its
/// cause.
const REFUSED: &[(&str, &str)] = &[];

/// The cases whose generated C++ a C++ compiler rejects, each with the
/// message it answers.
///
/// The same list-and-not-a-filter rule: a case here that starts
/// compiling fails, so the entry cannot outlive the defect. Every one is
/// in `docs/OPEN-ITEMS.md` with the compiler's own text.
///
/// A by-value array cycle is now rejected by the production frontend
/// before this compiler gate, through `corpus_frontend`.
///
/// `recursive-optional-external` was a second entry and is not one any
/// more. An `@external` member is a `std::shared_ptr<T>` since the C++
/// column was given the indirection XTypes 1.3 Table 21 asks for
/// (`crates/lang-rules/tests/external_is_an_indirection.rs`), and a
/// `std::shared_ptr` is complete whatever it points at. The fix landed
/// after this file did and did not reach the list here, so the entry
/// outlived its cause — which is the failure this list is shaped to
/// produce, and it produced it: g++ 14.2.0, clang 19.1.7 and Apple clang
/// 21.0.0 all accept the case today.
// Array cycles are frontend refusals, not successful compiler checks.
const DOES_NOT_COMPILE: &[(&str, &str)] = &[];

// -- the compiler --------------------------------------------------------

/// The C++ compiler to use, or `None` where there is none.
///
/// `CXX` first, so a host with several says which; otherwise the first
/// of the three usual spellings that answers `--version`.
fn compiler() -> Option<String> {
    if let Ok(chosen) = std::env::var("CXX") {
        return Some(chosen);
    }
    ["c++", "g++", "clang++"]
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
///
/// A translation unit of one `#include` and not the header on the
/// command line: a header compiled as a main file warns about its own
/// `#pragma once`, which under `-Werror` is an error about the harness
/// rather than about the generated code.
fn cpp_compiles(cxx: &str, source: &str, tag: &str) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("zd-cpp-corpus-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let header = dir.join("zd_types.hpp");
    let unit = dir.join("tu.cpp");
    std::fs::write(&header, source).map_err(|error| error.to_string())?;
    std::fs::write(&unit, "#include \"zd_types.hpp\"\n").map_err(|error| error.to_string())?;
    let runtime = corpus_harness::repository_root().join("runtime/cpp/include");
    // The flag set `tools/wireprobe/wireprobe-zerodds-cpp` compiles the
    // measured C++ column with, minus the link step.
    let output = Command::new(cxx)
        .args(["-std=c++20", "-fsyntax-only", "-Wall", "-Wextra", "-Werror"])
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
    // accepts anything — or from a harness that never ran one.
    let Some(cxx) = compiler() else {
        eprintln!("SKIPPED: no C++ compiler on the path.");
        return;
    };
    let bad = "#pragma once\nstruct S { NoSuchTypeExists x; };\n";
    let why =
        cpp_compiles(&cxx, bad, "deliberate").expect_err("the compiler accepted a bad header");
    assert!(why.contains("NoSuchTypeExists"), "{why}");
    // And a good one passes, so the check is not simply always red.
    cpp_compiles(&cxx, "#pragma once\nstruct S { int x; };\n", "control")
        .expect("the compiler rejected a good header");
}

/// The whole corpus through the C++ compiler, one invocation per case.
///
/// `#[ignore]`, because it is one invocation of a compiler per corpus
/// case and each costs about one and a half seconds — eleven minutes for
/// the corpus as it stands. It is what proves the column rather than
/// samples it, so it is run with `--include-ignored` and its result is
/// what the `idl-cpp-corpus` job records.
#[test]
#[ignore = "one C++ compiler invocation per corpus case, ~11 minutes; run with --include-ignored"]
fn every_corpus_case_that_renders_also_compiles() {
    let Some(cxx) = compiler() else {
        eprintln!("SKIPPED: no C++ compiler on the path.");
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
                // The stage that refused is what decides whose failure it
                // is. `parse` and `lower` are the frontend's and are the
                // same for every column.
                if why.starts_with("parse:") || why.starts_with("lower:") {
                    frontend.push(format!("{name}: {why}"));
                } else {
                    failures.push(format!("{name}: {why}"));
                }
                continue;
            }
            (Err(_), Some(_)) => continue,
        };
        checked += 1;
        let known = DOES_NOT_COMPILE.iter().find(|(case, _)| *case == name);
        match (cpp_compiles(&cxx, &text, &name), known) {
            (Ok(()), None) | (Err(_), Some(_)) => {}
            (Ok(()), Some((case, why))) => {
                unexpectedly_fine.push(format!("{case}: compiles now, but is listed as {why}"));
            }
            (Err(why), None) => failures.push(format!("{name}: {why}")),
        }
    }
    eprintln!(
        "{checked} corpus case(s) reached C++; {} failed unexpectedly",
        failures.len()
    );
    if !frontend.is_empty() {
        eprintln!(
            "{} corpus case(s) the frontend refuses, so C++ never saw them:\n{frontend:#?}",
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

/// The RPC corpus — interfaces, under both service mappings — through the
/// C++ compiler. The data corpus above has no `interface` in it, so
/// nothing there compiles the RPC binding.
///
/// Not `#[ignore]`: there are a handful of cases.
#[test]
fn every_rpc_corpus_case_compiles() {
    let Some(cxx) = compiler() else {
        eprintln!("SKIPPED: no C++ compiler on the path.");
        return;
    };
    let cases = corpus_harness::rpc_corpus();
    assert!(
        cases.len() >= 4,
        "the RPC corpus has Basic and Enhanced cases: {}",
        cases.len()
    );
    let mut failures = Vec::new();
    for (name, path, mapping) in cases {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        match try_generate_with(&source, &path.display().to_string(), mapping) {
            Err(why) => failures.push(format!("{name}: {why}")),
            Ok(text) => {
                if let Err(why) = cpp_compiles(&cxx, &text, &name) {
                    failures.push(format!("{name}: {why}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
