// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Operations Profile of DDS-TS 1.0 chapter 11, type-checked.
//!
//! `tests/interface_bindings.rs` holds the *text* the emitter writes;
//! this holds it against `tsc --strict`. The two are not the same
//! check: a `Promise<{ result: number; remainder: number }>` that is
//! spelled right and does not satisfy `ServiceDescriptor<TClient,
//! THandler>` compiles nowhere, and a text assertion cannot tell.
//!
//! The descriptor types come from `runtime/typescript/src/operations.ts`
//! — the module Ch2 §Operations Profile requires — so this is also the
//! check that the emitter and that file agree.
//!
//! The IDL below declares no `raises`. That is deliberate and not a
//! narrower test: §11.4 puts the exception *descriptors* of chapter 8
//! into the `raises` array, and those are the struct column's side
//! tables. Compiling them here would measure that column and not this
//! one; `tests/interface_bindings.rs::typescript_writes_the_service_descriptor_of_eleven_five`
//! holds the `raises` array itself.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::PathBuf;
use std::process::Command;

use zerodds_ast::Ast;
use zerodds_lang_cst::Language;
use zerodds_lang_rules::emit::interface_typescript;
use zerodds_lang_rules::{ModelGraph, apply, rule};

mod typescript_toolchain;

use typescript_toolchain::{compiler_or_note, repository_root};

const SOURCE: &str = "
    module robot {
      interface Base { long ping(); };
      interface Cursor : Base {
        readonly attribute long total;
        attribute string label;
        long advance(in long step, inout long position, out boolean wrapped);
        void reset();
      };
    };
";

#[test]
fn the_generated_operations_profile_type_checks() {
    let Some(tsc) = compiler_or_note("the Operations Profile") else {
        return;
    };
    let parsed = zerodds_idl_parse::parse(SOURCE);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "cursor.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");

    let graph = ModelGraph::of(&ast);
    let applied = apply(&graph, &rule::catalogue(Language::TypeScript)).expect("apply");
    let body = interface_typescript::ts_interface_items(&ast, &applied)
        .unwrap_or_else(|error| panic!("{error}"))
        .iter()
        .map(zerodds_lang_render::typescript::decl)
        .collect::<Vec<_>>()
        .join("\n");

    let root = repository_root();
    let at = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("dds-ts-operations");
    std::fs::create_dir_all(&at).expect("the build directory");
    let source = at.join("generated.ts");
    let runtime = root.join("runtime/typescript/src/operations.ts");
    std::fs::write(
        &source,
        format!(
            "import type {{ ServiceDescriptor }} from \"{}\";\n\n{body}\n",
            runtime.with_extension("js").display()
        ),
    )
    .expect("the generated file");

    // The same settings the runtime is written for: a type that only
    // holds under looser ones is a type a consumer cannot use.
    let output = Command::new(&tsc)
        .current_dir(root.join("runtime/typescript"))
        .args([
            // The runtime's own `tsconfig.json` lists `src/**/*` and
            // would refuse to load beside a named file; the settings it
            // holds are spelled out below instead.
            "--ignoreConfig",
            "--noEmit",
            "--strict",
            "--exactOptionalPropertyTypes",
            "--noUncheckedIndexedAccess",
            "--target",
            "ES2022",
            "--module",
            "NodeNext",
            "--moduleResolution",
            "NodeNext",
        ])
        .arg(&source)
        .output()
        .expect("the TypeScript compiler runs");
    assert!(
        output.status.success(),
        "the generated Operations Profile does not type-check:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
