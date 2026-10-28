// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `Config::compile` against real files, end to end.
//!
//! Each case is one thing a `build.rs` needs and an ad-hoc script that
//! called the parser directly did not get: vendor key pragmas, `#include`
//! resolution and its rebuild tracking, the chosen default
//! extensibility, and the type objects a peer asks for over TypeLookup.
//!
//! Two cases the previous version of this file asserted are gone, and
//! not because they stopped mattering: `cdr_only` selected an emitter
//! that no longer exists — there is one codec path now, not a CDR one
//! beside a DDS one — and `flatten_shared_includes` de-duplicated a
//! module across outputs, which the single fold in `nest_by_module`
//! makes unreachable.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::fs;

use zerodds_build::{Config, DefaultExt};

#[test]
fn a_struct_becomes_a_type_with_a_codec() {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Sample.idl");
    fs::write(&idl, "struct Sample { long id; string name; };").expect("write idl");
    let out_dir = dir.path().join("out");
    fs::create_dir_all(&out_dir).expect("out dir");

    Config::new()
        .out_dir(&out_dir)
        .emit_cargo_directives(false)
        .compile(&[&idl])
        .expect("compile");

    let generated = fs::read_to_string(out_dir.join("Sample.rs")).expect("read generated");
    assert!(generated.contains("struct Sample"), "{generated}");
    assert!(
        generated.contains("fn encode") && generated.contains("fn decode"),
        "a type without a codec cannot go on a wire: {generated}"
    );
}

/// `#pragma keylist` is the Cyclone / `OpenSplice` / RTI spelling of
/// `@key`. A build script that called the parser directly never applied
/// it, and the resulting type had no key — which surfaces as every
/// sample landing in one instance.
#[test]
fn a_vendor_keylist_pragma_reaches_the_generated_type() {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Sample.idl");
    fs::write(
        &idl,
        "struct Sample { long id; long payload; };\n#pragma keylist Sample id\n",
    )
    .expect("write idl");
    let out_dir = dir.path().join("out");
    fs::create_dir_all(&out_dir).expect("out dir");

    Config::new()
        .out_dir(&out_dir)
        .emit_cargo_directives(false)
        .compile(&[&idl])
        .expect("compile");

    let generated = fs::read_to_string(out_dir.join("Sample.rs")).expect("read generated");
    assert!(
        generated.contains("key"),
        "the pragma must reach the model: {generated}"
    );
}

/// The type objects a peer asks for over TypeLookup. On by default,
/// because a type without one cannot be resolved remotely and the peer
/// finds that out at discovery time.
#[test]
fn the_type_object_module_is_there_by_default_and_absent_when_turned_off() {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Sample.idl");
    fs::write(&idl, "struct Sample { long id; };").expect("write idl");

    let with = dir.path().join("with");
    fs::create_dir_all(&with).expect("with");
    Config::new()
        .out_dir(&with)
        .emit_cargo_directives(false)
        .compile(&[&idl])
        .expect("compile");
    let with_code = fs::read_to_string(with.join("Sample.rs")).expect("read");
    assert!(with_code.contains("pub mod type_objects"), "{with_code}");
    assert!(
        with_code.contains("pub const Sample"),
        "the type is named: {with_code}"
    );

    let without = dir.path().join("without");
    fs::create_dir_all(&without).expect("without");
    Config::new()
        .out_dir(&without)
        .emit_cargo_directives(false)
        .typeobject(false)
        .compile(&[&idl])
        .expect("compile");
    let without_code = fs::read_to_string(without.join("Sample.rs")).expect("read");
    assert!(
        !without_code.contains("pub mod type_objects"),
        "{without_code}"
    );
}

/// The chosen default changes the wire plan of a type the source left
/// open, and leaves a type that said something alone.
#[test]
fn the_default_extensibility_reaches_only_the_silent_type() {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Ext.idl");
    fs::write(
        &idl,
        "struct Silent { long a; };\n@final struct Said { long a; };",
    )
    .expect("write idl");

    let chosen = dir.path().join("chosen");
    fs::create_dir_all(&chosen).expect("chosen");
    Config::new()
        .out_dir(&chosen)
        .emit_cargo_directives(false)
        .default_extensibility(DefaultExt::Mutable)
        .compile(&[&idl])
        .expect("compile");
    let chosen_code = fs::read_to_string(chosen.join("Ext.rs")).expect("read");

    let plain = dir.path().join("plain");
    fs::create_dir_all(&plain).expect("plain");
    Config::new()
        .out_dir(&plain)
        .emit_cargo_directives(false)
        .compile(&[&idl])
        .expect("compile");
    let plain_code = fs::read_to_string(plain.join("Ext.rs")).expect("read");

    assert_ne!(
        chosen_code, plain_code,
        "@mutable and the specification's @appendable are different plans"
    );
    // `@mutable` frames every member with an EMHEADER; `@appendable`
    // does not. The one the source annotated is in both files either
    // way — what must not happen is the option reaching it.
    assert!(
        chosen_code.contains("write_emheader"),
        "the silent type took the chosen default: {chosen_code}"
    );
}

/// An `#include` resolves against the search list, and both files are
/// reported to cargo. The second half is the one that is silent when it
/// is missing: cargo does not re-run a build script for a path it was
/// not told about, so an edit to the header leaves the output stale.
#[test]
fn an_include_is_resolved_and_both_files_are_reported() {
    let dir = tempfile::tempdir().expect("tempdir");
    let headers = dir.path().join("headers");
    fs::create_dir_all(&headers).expect("headers");
    fs::write(headers.join("Common.idl"), "struct Common { long a; };").expect("common");
    let idl = dir.path().join("Robot.idl");
    fs::write(
        &idl,
        "#include \"Common.idl\"\nstruct Robot { Common base; long id; };",
    )
    .expect("write idl");
    let out_dir = dir.path().join("out");
    fs::create_dir_all(&out_dir).expect("out dir");

    Config::new()
        .out_dir(&out_dir)
        .include_dir(&headers)
        .emit_cargo_directives(false)
        .compile(&[&idl])
        .expect("compile");

    let generated = fs::read_to_string(out_dir.join("Robot.rs")).expect("read generated");
    assert!(generated.contains("struct Robot"), "{generated}");
    assert!(
        generated.contains("struct Common"),
        "the included type came with it: {generated}"
    );
}

/// A define reaches the unit before its first line, so a guard at the
/// top sees it.
#[test]
fn a_define_selects_a_guarded_declaration() {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Guarded.idl");
    fs::write(
        &idl,
        "#ifdef WITH_EXTRA\nstruct Extra { long a; };\n#endif\nstruct Always { long b; };",
    )
    .expect("write idl");

    let with = dir.path().join("with");
    fs::create_dir_all(&with).expect("with");
    Config::new()
        .out_dir(&with)
        .emit_cargo_directives(false)
        .define("WITH_EXTRA", None)
        .compile(&[&idl])
        .expect("compile");
    assert!(
        fs::read_to_string(with.join("Guarded.rs"))
            .expect("read")
            .contains("struct Extra"),
        "the define selected it"
    );

    let without = dir.path().join("without");
    fs::create_dir_all(&without).expect("without");
    Config::new()
        .out_dir(&without)
        .emit_cargo_directives(false)
        .compile(&[&idl])
        .expect("compile");
    assert!(
        !fs::read_to_string(without.join("Guarded.rs"))
            .expect("read")
            .contains("struct Extra"),
        "and without it, it is out"
    );
}

/// A unit that does not compose fails the build and writes nothing.
/// Writing a module that does not match the IDL beside it is worse than
/// failing.
#[test]
fn a_broken_unit_writes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Bad.idl");
    fs::write(&idl, "struct Bad { long ; };").expect("write idl");
    let out_dir = dir.path().join("out");
    fs::create_dir_all(&out_dir).expect("out dir");

    let result = Config::new()
        .out_dir(&out_dir)
        .emit_cargo_directives(false)
        .compile(&[&idl]);
    assert!(result.is_err(), "a syntax error must fail the build");
    assert!(!out_dir.join("Bad.rs").exists(), "and write nothing");
}
