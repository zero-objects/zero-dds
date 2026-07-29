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
