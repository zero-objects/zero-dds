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
