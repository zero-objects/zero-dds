// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The whole interop corpus, as TypeScript, through `tsc --strict`.
//!
//! The counterpart of the `every_corpus_case_that_renders_also_compiles`
//! that `swift_end_to_end.rs`, `go_end_to_end.rs` and eight more carry.
//! The TypeScript column had none: `docs/OPEN-ITEMS.md` records that the
//! six oldest columns lost theirs when `zerodds-idl-<lang>` was removed.
//!
//! `typescript_descriptors.rs` type-checks fourteen hand-written shapes
//! against DDS-TS 1.0 Annex B; this type-checks the corpus. `--strict`
//! and not a bare `tsc`, because §2.1 item 4 of that PSM asks for output
//! that compiles under `--strict`, and a descriptor whose `fields` array
//! does not satisfy `DdsMemberDescriptor` is a defect no substring check
//! finds.
//!
//! ## One invocation, not one per case
//!
//! The nine sibling gates run a compiler once per corpus case, because
//! two generated files in one compilation would declare types of one
//! name. That is not so here: every generated file carries an `import`
//! and `export`s, so each is a *module* and its declarations are scoped
//! to it. `tsc` names the file in every diagnostic, so a failure still
//! names the case, and the whole corpus type-checks in about three
//! seconds instead of the fifteen minutes 418 invocations of `tsc`
//! would cost.
//!
//! The runtime is imported by relative path rather than by package name,
//! for the reason `typescript_reader_generates.rs` gives: a package name
//! would need an install step, and this checks the tree as it stands.
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

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_lang_rules::rule::typescript;
use zerodds_lang_rules::{ModelGraph, apply};

#[path = "support/resolved.rs"]
mod resolved;
mod typescript_toolchain;

use typescript_toolchain::{compiler_or_note, repository_root};

/// Where the generated files import the runtime from.
///
/// Relative to `runtime/typescript/dist-generated/corpus-*/`, which is
/// where they are written. `dist-generated` is ignored by git.
const RUNTIME_IMPORT: &str = "../../src/index.js";

/// IDL in, one TypeScript module out — naming the layer that refused
/// instead of panicking.
///
/// The prelude, the data types with their descriptor side-tables and the
/// Operations Profile binding: the three
/// `zerodds_lang_rules::generate::declarations` assembles for
/// `Language::TypeScript`, with the runtime import pointed at the tree
/// rather than at the package name.
fn try_generate(idl: &str, origin: &str) -> Result<String, String> {
    let ast = resolved::ast(idl, origin)?;
    let model = ModelGraph::of(&ast);
    let applied =
        apply(&model, &typescript::catalogue()).map_err(|error| format!("apply: {error}"))?;
    let mut decls = zerodds_lang_rules::adapter::typescript::prelude();
    decls.extend(
        zerodds_lang_rules::adapter::typescript::ts_items_with_descriptors(&ast, &applied)
            .map_err(|error| format!("materialise: {error}"))?,
    );
    decls.extend(
        zerodds_lang_rules::emit::interface_typescript::ts_interface_items(&ast, &applied)
            .map_err(|error| format!("materialise: {error}"))?,
    );
    Ok(zerodds_lang_render::typescript::file(
        &zerodds_lang_cst::typescript::File {
            doc: zerodds_lang_cst::common::Doc::none(),
            imports: vec![zerodds_lang_rules::adapter::typescript::descriptor_import(
                RUNTIME_IMPORT,
            )],
            decls,
        },
    ))
}

// -- the corpus ----------------------------------------------------------

/// Every corpus case, by name.
fn corpus() -> Vec<(String, PathBuf)> {
    let root = repository_root().join("tests/interop-corpus");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&root).expect("the corpus directory") {
        let entry = entry.expect("a corpus entry");
        let idl = entry.path().join("type.idl");
        if idl.is_file() {
            out.push((entry.file_name().to_string_lossy().into_owned(), idl));
        }
    }
    out.sort();
    out
}

/// How many corpus cases have to reach `tsc` for this gate to mean
/// anything. 417 cases render today.
const MUST_RENDER: usize = 350;

/// The names the TypeScript column itself refuses, each with the reason.
///
/// A list and not a filter: a case on it that starts rendering fails
/// too — an exemption nothing checks is an exemption that outlives its
/// cause.
const REFUSED: &[(&str, &str)] = &[(
    "resolve-module-named-as-type",
    "a nested declaration named `common`, which would capture the first \
     segment of every qualified reference written inside it — TypeScript \
     has no root operator to write past it with",
)];

// -- the compiler --------------------------------------------------------

/// Where the generated files go: beside the runtime, so the relative
/// import resolves, and under a directory git ignores.
fn scratch(leaf: &str) -> PathBuf {
    repository_root()
        .join("runtime/typescript/dist-generated")
        .join(leaf)
}

/// `tsc --strict` over the files in `directory`, answering its output.
fn typecheck(tsc: &Path, directory: &Path) -> String {
    let runtime = repository_root().join("runtime/typescript");
    let mut files: Vec<PathBuf> = std::fs::read_dir(directory)
        .expect("the generated directory")
        .map(|entry| entry.expect("a generated file").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "ts"))
        .collect();
    files.sort();
    let output = Command::new(tsc)
        .current_dir(&runtime)
        .args([
            "--ignoreConfig",
            "--noEmit",
            "--strict",
            "--target",
            "ES2022",
            "--module",
            "NodeNext",
            "--moduleResolution",
            "NodeNext",
            "--skipLibCheck",
        ])
        .args(&files)
        .output()
        .expect("the TypeScript compiler runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
#[ignore = "one `tsc` start; run with --include-ignored"]
fn the_compiler_check_catches_a_deliberate_error() {
    // Without this, a green result below could come from a `tsc` that
    // accepts anything — or from a harness that never ran one.
    let Some(tsc) = compiler_or_note("the TypeScript column") else {
        return;
    };
    let directory = scratch("corpus-deliberate");
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("the scratch directory");
    std::fs::write(
        directory.join("good.ts"),
        "export const good: number = 1;\n",
    )
    .expect("the good file");
    std::fs::write(
        directory.join("bad.ts"),
        "export const bad: NoSuchTypeExists = 1;\n",
    )
    .expect("the bad file");
    let report = typecheck(&tsc, &directory);
    assert!(report.contains("NoSuchTypeExists"), "{report}");
    // And the good one is not reported, so the check is not simply
    // always red — and the diagnostic names the file it came from,
    // which is what makes one invocation over the whole corpus name the
    // case.
    assert!(report.contains("bad.ts"), "{report}");
    assert!(!report.contains("good.ts"), "{report}");
    let _ = std::fs::remove_dir_all(&directory);
}

/// The whole corpus through `tsc --strict`.
///
/// `#[ignore]`, because it renders 418 files and starts a TypeScript
/// compiler over them, which costs seconds rather than the milliseconds
/// a `cargo test` run budgets. It is what proves the column rather than
/// samples it, so it is run with `--include-ignored` and its result is
/// what the `idl-ts-corpus` job records.
#[test]
#[ignore = "renders the whole corpus and starts `tsc` over it; run with --include-ignored"]
fn every_corpus_case_that_renders_also_compiles() {
    let Some(tsc) = compiler_or_note("the TypeScript column") else {
        return;
    };
    let directory = scratch("corpus-all");
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("the scratch directory");

    let mut failures = Vec::new();
    let mut unexpectedly_fine = Vec::new();
    let mut frontend = Vec::new();
    let mut checked = 0_usize;
    for (name, path) in corpus() {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&source, &path.display().to_string()) {
            continue;
        }
        let refused = REFUSED.iter().find(|(case, _)| *case == name);
        match (try_generate(&source, &path.display().to_string()), refused) {
            (Ok(text), None) => {
                checked += 1;
                std::fs::write(directory.join(format!("{name}.ts")), text)
                    .expect("the generated file");
            }
            (Ok(_), Some((case, why))) => {
                unexpectedly_fine.push(format!("{case}: renders now, but is listed as {why}"));
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
            }
            (Err(_), Some(_)) => {}
        }
    }
    if !frontend.is_empty() {
        eprintln!(
            "{} corpus case(s) the frontend refuses, so TypeScript never saw them:\n\
             {frontend:#?}",
            frontend.len()
        );
    }
    assert!(failures.is_empty(), "{failures:#?}");
    assert!(unexpectedly_fine.is_empty(), "{unexpectedly_fine:#?}");
    assert!(
        checked >= MUST_RENDER,
        "only {checked} corpus cases reached `tsc`; the gate says nothing below {MUST_RENDER}"
    );

    let report = typecheck(&tsc, &directory);
    assert!(
        report.trim().is_empty(),
        "{checked} corpus cases were type-checked and `tsc --strict` answered:\n{report}"
    );
    let _ = std::fs::remove_dir_all(&directory);
}
