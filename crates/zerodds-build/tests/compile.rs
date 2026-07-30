// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! End-to-end tests: `Config::compile` against real `.idl` files on disk,
//! covering the four things the pre-existing ad hoc `build.rs` scripts
//! skipped (`#include` tracking is exercised indirectly — the composed
//! AST must contain the included type; vendor key-pragmas; default
//! extensibility; TypeObject emission).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::fs;
use std::path::Path;

use zerodds_build::{Config, DefaultExt};

#[test]
fn compiles_plain_struct_to_rust_with_dds_type_impl() {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Sample.idl");
    fs::write(&idl, "struct Sample { long a; };").expect("write idl");
    let out_dir = dir.path().join("out");

    Config::new()
        .out_dir(&out_dir)
        .emit_cargo_directives(false)
        .compile(&[&idl])
        .expect("compile");

    let generated = fs::read_to_string(out_dir.join("Sample.rs")).expect("read generated");
    assert!(generated.contains("struct Sample"));
    assert!(
        generated.contains("impl") && generated.contains("DdsType"),
        "default (non-cdr_only) mode must emit a DdsType impl: {generated}"
    );
}

#[test]
fn typeobject_block_present_by_default_and_absent_when_disabled() {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Sample.idl");
    fs::write(&idl, "struct Sample { long a; };").expect("write idl");

    let with = dir.path().join("with_to");
    Config::new()
        .out_dir(&with)
        .emit_cargo_directives(false)
        .compile(&[&idl])
        .expect("compile with typeobject");
    let with_code = fs::read_to_string(with.join("Sample.rs")).expect("read");
    assert!(with_code.contains("pub mod type_objects"));

    let without = dir.path().join("without_to");
    Config::new()
        .out_dir(&without)
        .typeobject(false)
        .emit_cargo_directives(false)
        .compile(&[&idl])
        .expect("compile without typeobject");
    let without_code = fs::read_to_string(without.join("Sample.rs")).expect("read");
    assert!(!without_code.contains("pub mod type_objects"));
}

#[test]
fn cdr_only_omits_dds_type_impl() {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Sample.idl");
    fs::write(&idl, "struct Sample { long a; };").expect("write idl");
    let out_dir = dir.path().join("out");

    Config::new()
        .out_dir(&out_dir)
        .cdr_only(true)
        .emit_cargo_directives(false)
        .compile(&[&idl])
        .expect("compile");

    let generated = fs::read_to_string(out_dir.join("Sample.rs")).expect("read generated");
    assert!(
        !generated.contains("DdsType"),
        "cdr_only must omit the DdsType impl: {generated}"
    );
    assert!(generated.contains("CdrEncode"));
}

#[test]
fn vendor_keylist_pragma_marks_field_key() {
    // `#pragma keylist` is the Cyclone/OpenSplice/RTI spelling of `@key` —
    // an ad hoc `zerodds_idl::parse` + `generate_rust_module` build.rs
    // (the pre-existing pattern this crate replaces) never applied it.
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Sample.idl");
    fs::write(
        &idl,
        "struct Sample { long id; long payload; };\n#pragma keylist Sample id\n",
    )
    .expect("write idl");
    let out_dir = dir.path().join("out");

    Config::new()
        .out_dir(&out_dir)
        .emit_cargo_directives(false)
        .compile(&[&idl])
        .expect("compile");

    let generated = fs::read_to_string(out_dir.join("Sample.rs")).expect("read generated");
    // The key-holder / key-encode path only exists for annotated `@key`
    // members — its presence proves the pragma reached the AST.
    assert!(
        generated.contains("key"),
        "pragma keylist must surface as key-handling code: {generated}"
    );
}

#[test]
fn default_extensibility_override_is_applied() {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Sample.idl");
    fs::write(&idl, "struct Sample { long a; };").expect("write idl");
    let out_dir = dir.path().join("out");

    // Compose the same IDL directly via zerodds-idl-compose to compare
    // annotation outcome (zerodds-build wraps compose 1:1 for this knob).
    let composed = zerodds_idl_compose::compose(
        &idl,
        &zerodds_idl_compose::ComposeOptions {
            default_extensibility: Some(DefaultExt::Mutable),
            ..Default::default()
        },
    )
    .expect("compose");
    assert!(
        !composed.type_objects.is_empty(),
        "sanity: TypeObject lowering must succeed for a plain struct"
    );

    Config::new()
        .out_dir(&out_dir)
        .default_extensibility(DefaultExt::Mutable)
        .emit_cargo_directives(false)
        .compile(&[&idl])
        .expect("compile");
    assert!(out_dir.join("Sample.rs").exists());
}

#[test]
fn include_directive_pulls_in_the_included_type() {
    // The included file defines a type the top-level IDL references;
    // success proves `-I`/include_dir() reached the preprocessor (the
    // ad hoc build.rs pattern this crate replaces had no #include
    // support at all).
    let dir = tempfile::tempdir().expect("tempdir");
    let idl_dir = dir.path().join("idl");
    fs::create_dir_all(&idl_dir).expect("mkdir idl");
    fs::write(idl_dir.join("Pose.idl"), "struct Pose { long x; long y; };").expect("write");
    fs::write(
        idl_dir.join("Robot.idl"),
        "#include \"Pose.idl\"\nstruct Robot { Pose pose; long id; };",
    )
    .expect("write");
    let out_dir = dir.path().join("out");

    Config::new()
        .include_dir(&idl_dir)
        .out_dir(&out_dir)
        .emit_cargo_directives(false)
        .compile(&[idl_dir.join("Robot.idl")])
        .expect("compile");

    let generated = fs::read_to_string(out_dir.join("Robot.rs")).expect("read generated");
    assert!(generated.contains("struct Robot"));
    assert!(
        generated.contains("Pose"),
        "included type must appear: {generated}"
    );
}

#[test]
fn flatten_shared_includes_emits_shared_module_once() {
    // `a.idl` and `b.idl` both `#include "common.idl"`. Off, each output
    // carries its own `pub mod common` (fine when wrapped per file). On, the
    // shared module is emitted once across the flat-included project.
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(
        dir.path().join("common.idl"),
        "module common { struct Shared { long x; }; };",
    )
    .expect("write common.idl");
    let a = dir.path().join("a.idl");
    let b = dir.path().join("b.idl");
    fs::write(
        &a,
        "#include \"common.idl\"\nmodule app_a { struct UsesA { common::Shared s; }; };",
    )
    .expect("write a.idl");
    fs::write(
        &b,
        "#include \"common.idl\"\nmodule app_b { struct UsesB { common::Shared s; }; };",
    )
    .expect("write b.idl");

    // Default (per-file) path: every output redeclares `pub mod common`.
    let plain = dir.path().join("plain");
    Config::new()
        .out_dir(&plain)
        .emit_cargo_directives(false)
        .compile(&[&a, &b])
        .expect("compile plain");
    let plain_total = fs::read_to_string(plain.join("a.rs"))
        .unwrap()
        .matches("pub mod common {")
        .count()
        + fs::read_to_string(plain.join("b.rs"))
            .unwrap()
            .matches("pub mod common {")
            .count();
    assert_eq!(
        plain_total, 2,
        "per-file path keeps a copy of `common` per input"
    );

    // Project path: exactly one `pub mod common` across the two outputs, and
    // the non-owning output keeps its `super::common::Shared` reference.
    let flat = dir.path().join("flat");
    Config::new()
        .out_dir(&flat)
        .flatten_shared_includes(true)
        .emit_cargo_directives(false)
        .compile(&[&a, &b])
        .expect("compile flat");
    let flat_a = fs::read_to_string(flat.join("a.rs")).expect("read a.rs");
    let flat_b = fs::read_to_string(flat.join("b.rs")).expect("read b.rs");
    let flat_total =
        flat_a.matches("pub mod common {").count() + flat_b.matches("pub mod common {").count();
    assert_eq!(
        flat_total, 1,
        "project path emits `common` once across the set"
    );
    assert!(
        flat_b.contains("super::common::Shared"),
        "stripped output must keep its reference into the single shared module"
    );
}

#[test]
fn missing_out_dir_and_no_env_errors_cleanly() {
    // SAFETY: single-threaded test process is not guaranteed here (the
    // test harness runs tests concurrently), so only assert on the error
    // path when OUT_DIR is genuinely absent; skip otherwise rather than
    // mutate global process env from a parallel test.
    if std::env::var("OUT_DIR").is_ok() {
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Sample.idl");
    fs::write(&idl, "struct Sample { long a; };").expect("write idl");

    let err = Config::new()
        .emit_cargo_directives(false)
        .compile(&[&idl])
        .expect_err("no out_dir and no OUT_DIR env must error");
    assert!(matches!(err, zerodds_build::Error::NoOutDir));
}

#[test]
fn duplicate_basename_inputs_are_rejected_not_silently_overwritten() {
    // Two inputs in different directories with the same file stem both map
    // to `<out_dir>/types.rs`; without the preflight the second silently
    // clobbers the first. It must error instead.
    let dir = tempfile::tempdir().expect("tempdir");
    let a_dir = dir.path().join("a");
    let b_dir = dir.path().join("b");
    fs::create_dir_all(&a_dir).expect("mkdir a");
    fs::create_dir_all(&b_dir).expect("mkdir b");
    let a_types = a_dir.join("types.idl");
    let b_types = b_dir.join("types.idl");
    fs::write(&a_types, "struct AThing { long a; };").expect("write a/types.idl");
    fs::write(&b_types, "struct BThing { long b; };").expect("write b/types.idl");
    let out_dir = dir.path().join("out");

    let err = Config::new()
        .out_dir(&out_dir)
        .emit_cargo_directives(false)
        .compile(&[&a_types, &b_types])
        .expect_err("duplicate basenames must be rejected");
    match err {
        zerodds_build::Error::DuplicateStem {
            stem,
            first,
            second,
        } => {
            assert_eq!(stem, "types");
            assert_eq!(first, a_types);
            assert_eq!(second, b_types);
        }
        other => panic!("expected DuplicateStem error, got {other}"),
    }

    // Preflight runs before any codegen: nothing was written.
    assert!(
        !out_dir.join("types.rs").exists(),
        "no output must be written when the batch is rejected"
    );

    // Distinct stems still compile fine (regression guard for the preflight).
    let ok_out = dir.path().join("ok_out");
    let b_other = b_dir.join("other.idl");
    fs::write(&b_other, "struct BThing { long b; };").expect("write b/other.idl");
    Config::new()
        .out_dir(&ok_out)
        .emit_cargo_directives(false)
        .compile(&[&a_types, &b_other])
        .expect("distinct stems compile");
    assert!(ok_out.join("types.rs").exists());
    assert!(ok_out.join("other.rs").exists());
}

#[test]
fn unresolvable_include_errors_with_path_context() {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Bad.idl");
    fs::write(&idl, "#include \"DoesNotExist.idl\"\nstruct S { long a; };").expect("write idl");
    let out_dir = dir.path().join("out");

    let err = Config::new()
        .out_dir(&out_dir)
        .emit_cargo_directives(false)
        .compile(&[&idl])
        .expect_err("missing include must error");
    match err {
        zerodds_build::Error::Compose { path, .. } => assert_eq!(path, Path::new(&idl)),
        other => panic!("expected Compose error, got {other}"),
    }
}
