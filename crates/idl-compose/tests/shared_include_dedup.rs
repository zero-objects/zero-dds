// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Multi-file composition with a **shared** `#include` (compose-naming spec
//! §5). `a.idl` and `b.idl` both `#include "common.idl"`, so single-file
//! composition gives each output a full copy of `module common`. Emitted per
//! input and `include!`d flat into one namespace, the second `pub mod common`
//! is an E0428 ("defined multiple times"). The GH #25 regression test uses
//! disjoint IDLs (`module common` + `module metrics`) and never sees this.
//!
//! [`compose_project`] must mark the redundant copy so the caller strips it,
//! leaving one `module common` across the project; the deduplicated output
//! must then compile without E0428, and a contradictory shared definition must
//! be rejected rather than silently resolved.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::fs;
use std::path::Path;
use std::process::Command;

use zerodds_idl_compose::{
    ComposeError, ComposeOptions, compose_project, strip_top_level_modules_rust,
};
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};

const COMMON: &str = "module common { struct Shared { long x; }; };";
const A_IDL: &str = "#include \"common.idl\"\n\
     module app_a { struct UsesA { common::Shared s; long a; }; };";
const B_IDL: &str = "#include \"common.idl\"\n\
     module app_b { struct UsesB { common::Shared s; long b; }; };";

/// Writes `common.idl`, `a.idl`, `b.idl` into a fresh tempdir sharing the
/// include, and returns the dir plus the two input paths.
fn project(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    fs::write(dir.join("common.idl"), COMMON).expect("write common.idl");
    let a = dir.join("a.idl");
    let b = dir.join("b.idl");
    fs::write(&a, A_IDL).expect("write a.idl");
    fs::write(&b, B_IDL).expect("write b.idl");
    (a, b)
}

/// Emits the Rust for one project input the way a build integration would:
/// full module (so `super::common::Shared` resolves) with the shared modules
/// stripped from a non-owning input.
fn emit(comp: &zerodds_idl_compose::ProjectComposition) -> String {
    let code =
        generate_rust_module(&comp.output.ast, &RustGenOptions::default()).expect("generate rust");
    strip_top_level_modules_rust(&code, &comp.shared_modules)
}

#[test]
fn shared_include_module_deduplicated_once() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (a, b) = project(dir.path());

    let comps = compose_project(&[&a, &b], &ComposeOptions::default()).expect("compose_project");
    assert_eq!(comps.len(), 2);

    // The first input owns the shared module; the second reports it as shared.
    assert!(
        comps[0].shared_modules.is_empty(),
        "owning input must keep the shared module, got {:?}",
        comps[0].shared_modules
    );
    assert_eq!(
        comps[1].shared_modules,
        vec!["common".to_string()],
        "second input must flag `common` as a shared-include duplicate"
    );

    // Exactly one `pub mod common` across the emitted project.
    let combined = format!("{}\n{}", emit(&comps[0]), emit(&comps[1]));
    let count = combined.matches("pub mod common {").count();
    assert_eq!(
        count, 1,
        "expected exactly one `pub mod common`, found {count}"
    );

    // The owning input still carries the shared type's TypeObject; the
    // non-owning input does not re-serialise it.
    assert!(
        comps[0]
            .output
            .type_objects
            .iter()
            .any(|t| t.fqn == "common::Shared"),
        "owner must keep common::Shared TypeObject"
    );
    assert!(
        !comps[1]
            .output
            .type_objects
            .iter()
            .any(|t| t.fqn == "common::Shared"),
        "non-owner must not re-emit common::Shared TypeObject"
    );
}

#[test]
fn deduplicated_project_compiles_without_e0428() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (a, b) = project(dir.path());
    let comps = compose_project(&[&a, &b], &ComposeOptions::default()).expect("compose_project");

    // Write the deduplicated per-input modules and a flat include site (one
    // parent namespace) — the exact shape that triggers E0428 without dedup.
    let a_rs = dir.path().join("a_gen.rs");
    let b_rs = dir.path().join("b_gen.rs");
    fs::write(&a_rs, emit(&comps[0])).expect("write a_gen.rs");
    fs::write(&b_rs, emit(&comps[1])).expect("write b_gen.rs");
    let combined = dir.path().join("combined.rs");
    fs::write(
        &combined,
        format!(
            "#[allow(unused)]\nmod project {{\n    include!(r\"{}\");\n    include!(r\"{}\");\n}}\n",
            a_rs.display(),
            b_rs.display()
        ),
    )
    .expect("write combined.rs");

    // E0428 is a name-resolution error rustc reports independently of the
    // (absent here) zerodds_cdr/dcps/types crates, so grepping its diagnostics
    // isolates the duplicate-module class from the expected E0433s.
    let errors = rustc_error_codes(&combined);
    assert!(
        !errors.contains("E0428"),
        "deduplicated project must not raise E0428; rustc codes: {errors:?}"
    );

    // Positive control: without stripping, the same flat include *does* raise
    // E0428 — proving the assertion above is load-bearing.
    let a_full = dir.path().join("a_full.rs");
    let b_full = dir.path().join("b_full.rs");
    fs::write(
        &a_full,
        generate_rust_module(&comps[0].output.ast, &RustGenOptions::default()).unwrap(),
    )
    .unwrap();
    fs::write(
        &b_full,
        generate_rust_module(&comps[1].output.ast, &RustGenOptions::default()).unwrap(),
    )
    .unwrap();
    let combined_full = dir.path().join("combined_full.rs");
    fs::write(
        &combined_full,
        format!(
            "#[allow(unused)]\nmod project {{\n    include!(r\"{}\");\n    include!(r\"{}\");\n}}\n",
            a_full.display(),
            b_full.display()
        ),
    )
    .unwrap();
    let control = rustc_error_codes(&combined_full);
    assert!(
        control.contains("E0428"),
        "control (no dedup) must raise E0428, got {control:?}"
    );
}

#[test]
fn conflicting_shared_module_is_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Two different include files each define `module common` with a different
    // body; the two inputs pull one each. Same name, contradictory body.
    fs::write(
        dir.path().join("ca.idl"),
        "module common { struct Shared { long x; }; };",
    )
    .expect("write ca.idl");
    fs::write(
        dir.path().join("cb.idl"),
        "module common { struct Shared { short y; }; };",
    )
    .expect("write cb.idl");
    let a = dir.path().join("a.idl");
    let b = dir.path().join("b.idl");
    fs::write(
        &a,
        "#include \"ca.idl\"\nmodule app_a { struct U { common::Shared s; }; };",
    )
    .expect("write a.idl");
    fs::write(
        &b,
        "#include \"cb.idl\"\nmodule app_b { struct V { common::Shared s; }; };",
    )
    .expect("write b.idl");

    match compose_project(&[&a, &b], &ComposeOptions::default()) {
        Err(ComposeError::Conflict(msg)) => {
            assert!(
                msg.contains("common"),
                "conflict message must name the module: {msg}"
            );
        }
        Err(other) => panic!("expected ComposeError::Conflict, got {other:?}"),
        Ok(_) => panic!("contradictory shared `module common` must be rejected"),
    }
}

/// Compiles `path` with `rustc` and returns the set of `E####` diagnostic codes
/// it emitted (empty on success). Used to assert on E0428 specifically while
/// tolerating the E0433s from the deliberately absent runtime crates.
fn rustc_error_codes(path: &Path) -> std::collections::BTreeSet<String> {
    let out = Command::new("rustc")
        .args(["--edition", "2021", "--crate-type", "lib", "-o"])
        .arg(if cfg!(windows) { "NUL" } else { "/dev/null" })
        .arg(path)
        .output()
        .expect("run rustc");
    let stderr = String::from_utf8_lossy(&out.stderr);
    let mut codes = std::collections::BTreeSet::new();
    let mut rest = stderr.as_ref();
    while let Some(pos) = rest.find("error[E") {
        let tail = &rest[pos + "error[".len()..];
        let code: String = tail.chars().take_while(|c| *c != ']').collect();
        let len = code.len();
        codes.insert(code);
        rest = &tail[len..];
    }
    codes
}
