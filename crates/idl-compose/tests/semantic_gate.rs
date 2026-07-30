// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The composition pipeline must reject semantically invalid IDL — duplicate
//! members and unresolved type names — with a `ComposeError::Semantic`,
//! independent of TypeObject emission (`emit_typeobject == false` must not
//! bypass the gate). Valid IDL must still compose cleanly.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::fs;

use zerodds_idl_compose::{ComposeError, ComposeOptions, compose};

fn write_idl(src: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("in.idl");
    fs::write(&path, src).expect("write idl");
    (dir, path)
}

fn assert_semantic_reject(result: Result<zerodds_idl_compose::ComposeOutput, ComposeError>) {
    match result {
        Err(ComposeError::Semantic(_)) => {}
        Err(other) => panic!("expected ComposeError::Semantic, got {other:?}"),
        Ok(_) => panic!("expected the gate to reject, but compose succeeded"),
    }
}

#[test]
fn duplicate_member_is_rejected() {
    let (_dir, path) = write_idl("struct S { long value; long value; };");
    assert_semantic_reject(compose(&path, &ComposeOptions::default()));
}

#[test]
fn unknown_type_is_rejected() {
    let (_dir, path) = write_idl("struct S { DoesNotExist value; };");
    assert_semantic_reject(compose(&path, &ComposeOptions::default()));
}

#[test]
fn gate_runs_without_typeobject_emission() {
    let (_dir, path) = write_idl("struct S { DoesNotExist value; };");
    let opts = ComposeOptions {
        emit_typeobject: false,
        ..ComposeOptions::default()
    };
    assert_semantic_reject(compose(&path, &opts));
}

#[test]
fn valid_idl_composes() {
    let (_dir, path) = write_idl("struct Inner { long a; }; struct Outer { Inner nested; };");
    let out = compose(&path, &ComposeOptions::default()).expect("valid IDL must compose");
    assert!(!out.ast.definitions.is_empty());
}

// ---------------------------------------------------------------------------
// Adversarial corpus additions (Schuld 6): the gate previously covered only
// the data-type surface. These three classes were slipping through.
// ---------------------------------------------------------------------------

// Class (a): valuetype / interface unresolved references.

#[test]
fn interface_op_return_of_undeclared_type_is_rejected() {
    let (_dir, path) = write_idl("interface I { DoesNotExist get(); };");
    assert_semantic_reject(compose(&path, &ComposeOptions::default()));
}

#[test]
fn interface_op_param_of_undeclared_type_is_rejected() {
    let (_dir, path) = write_idl("interface I { void set(in DoesNotExist v); };");
    assert_semantic_reject(compose(&path, &ComposeOptions::default()));
}

#[test]
fn interface_base_of_undeclared_type_is_rejected() {
    let (_dir, path) = write_idl("interface I : DoesNotExist { };");
    assert_semantic_reject(compose(&path, &ComposeOptions::default()));
}

#[test]
fn interface_with_declared_refs_composes() {
    // Counter-corpus: an interface whose refs all resolve must still compose.
    let (_dir, path) = write_idl(
        "struct Payload { long a; };\n\
         interface Base { };\n\
         interface I : Base { Payload fetch(in Payload p); };",
    );
    let out = compose(&path, &ComposeOptions::default()).expect("declared refs must compose");
    assert!(!out.ast.definitions.is_empty());
}

// Class (b): bitfield / bitmask @position collisions.

#[test]
fn bitset_colliding_position_is_rejected() {
    let (_dir, path) =
        write_idl("bitset BS { @position(0) bitfield<4> a; @position(2) bitfield<4> b; };");
    assert_semantic_reject(compose(&path, &ComposeOptions::default()));
}

#[test]
fn bitmask_duplicate_position_is_rejected() {
    let (_dir, path) = write_idl("bitmask Flags { @position(2) F0, @position(2) F1 };");
    assert_semantic_reject(compose(&path, &ComposeOptions::default()));
}

#[test]
fn well_packed_bitset_composes() {
    // Counter-corpus: sequential, non-overlapping bitfields must still compose.
    let (_dir, path) = write_idl(
        "@bit_bound(16) bitset TypeFlag {\n\
            bitfield<1> is_final;\n\
            bitfield<1> is_appendable;\n\
            bitfield<11>;\n\
         };",
    );
    let out = compose(&path, &ComposeOptions::default()).expect("packed bitset must compose");
    assert!(!out.ast.definitions.is_empty());
}

// Class (c): @default type conversion.

#[test]
fn default_string_on_integer_member_is_rejected() {
    let (_dir, path) = write_idl("struct S { @default(\"oops\") long x; };");
    assert_semantic_reject(compose(&path, &ComposeOptions::default()));
}

#[test]
fn default_out_of_range_on_short_is_rejected() {
    let (_dir, path) = write_idl("struct S { @default(70000) short x; };");
    assert_semantic_reject(compose(&path, &ComposeOptions::default()));
}

#[test]
fn matching_default_composes() {
    // Counter-corpus: a type-compatible @default must still compose.
    let (_dir, path) = write_idl("struct S { @default(7) long x; @default(\"hi\") string s; };");
    let out = compose(&path, &ComposeOptions::default()).expect("matching default must compose");
    assert!(!out.ast.definitions.is_empty());
}
