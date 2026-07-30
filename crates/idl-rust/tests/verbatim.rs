// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `@verbatim` codegen-hook tests for idl-rust (XTypes 1.3 §7.2.2.4.8 +
//! IDL 4.2 §8.3.5.1). Item 14 of feature-completeness-F1: idl-rust parsed the
//! `VerbatimSpec` but never emitted it — the annotation content was dropped.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};

fn emit(idl: &str) -> String {
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    generate_rust_module(&ast, &RustGenOptions::default()).expect("gen")
}

#[test]
fn verbatim_before_declaration_appears_before_struct() {
    let rust = emit(
        r#"
        @verbatim(language="rust", placement=BEFORE_DECLARATION, text="// pre-decl marker")
        struct PlainStruct { long x; };
    "#,
    );
    assert!(
        rust.contains("// pre-decl marker"),
        "@verbatim BEFORE_DECLARATION missing:\n{rust}"
    );
    let pos_marker = rust.find("// pre-decl marker").unwrap_or(usize::MAX);
    let pos_struct = rust.find("pub struct PlainStruct").unwrap_or(usize::MAX);
    assert!(
        pos_marker < pos_struct,
        "BEFORE_DECLARATION must precede the struct:\n{rust}"
    );
}

#[test]
fn verbatim_after_declaration_appears_after_struct() {
    let rust = emit(
        r#"
        @verbatim(language="rust", placement=AFTER_DECLARATION, text="// trailer marker")
        struct S { long x; };
    "#,
    );
    let pos_marker = rust.find("// trailer marker").unwrap_or(usize::MAX);
    let pos_struct = rust.find("pub struct S").unwrap_or(usize::MAX);
    assert!(
        pos_marker != usize::MAX && pos_marker > pos_struct,
        "AFTER_DECLARATION must follow the struct:\n{rust}"
    );
}

#[test]
fn verbatim_default_placement_is_after_declaration() {
    // No explicit placement → spec default AFTER_DECLARATION (§8.3.5.1).
    let rust = emit(
        r#"
        @verbatim(language="rust", text="// default-placement marker")
        struct S { long x; };
    "#,
    );
    let pos_marker = rust
        .find("// default-placement marker")
        .unwrap_or(usize::MAX);
    let pos_struct = rust.find("pub struct S").unwrap_or(usize::MAX);
    assert!(
        pos_marker != usize::MAX && pos_marker > pos_struct,
        "default placement must be AFTER_DECLARATION:\n{rust}"
    );
}

#[test]
fn verbatim_begin_and_end_declaration_inside_struct_body() {
    let rust = emit(
        r#"
        @verbatim(language="rust", placement=BEGIN_DECLARATION, text="// begin body")
        @verbatim(language="rust", placement=END_DECLARATION, text="// end body")
        struct S { long x; };
    "#,
    );
    let pos_open = rust.find("pub struct S {").unwrap_or(usize::MAX);
    let pos_begin = rust.find("// begin body").unwrap_or(usize::MAX);
    let pos_field = rust.find("pub x:").unwrap_or(usize::MAX);
    let pos_end = rust.find("// end body").unwrap_or(usize::MAX);
    assert!(
        pos_open < pos_begin && pos_begin < pos_field && pos_field < pos_end,
        "BEGIN/END_DECLARATION must bracket the fields inside the body:\n{rust}"
    );
}

#[test]
fn verbatim_begin_file_and_end_file() {
    let rust = emit(
        r#"
        @verbatim(language="rust", placement=BEGIN_FILE, text="// top of file")
        @verbatim(language="rust", placement=END_FILE, text="// bottom of file")
        struct S { long x; };
    "#,
    );
    let pos_top = rust.find("// top of file").unwrap_or(usize::MAX);
    let pos_struct = rust.find("pub struct S").unwrap_or(usize::MAX);
    let pos_bottom = rust.find("// bottom of file").unwrap_or(usize::MAX);
    assert!(
        pos_top < pos_struct && pos_struct < pos_bottom,
        "BEGIN_FILE before all types, END_FILE after:\n{rust}"
    );
}

#[test]
fn verbatim_wildcard_language_applies() {
    let rust = emit(
        r#"
        @verbatim(language="*", placement=BEFORE_DECLARATION, text="// universal pre")
        struct S { long x; };
    "#,
    );
    assert!(
        rust.contains("// universal pre"),
        "wildcard must match:\n{rust}"
    );
}

#[test]
fn verbatim_other_language_skipped() {
    let rust = emit(
        r#"
        @verbatim(language="java", placement=BEFORE_DECLARATION, text="// not for rust")
        struct S { long x; };
    "#,
    );
    assert!(
        !rust.contains("// not for rust"),
        "a foreign language tag must not emit:\n{rust}"
    );
}

#[test]
fn verbatim_applies_to_enum_and_union() {
    let rust = emit(
        r#"
        @verbatim(language="rust", placement=BEFORE_DECLARATION, text="// enum pre")
        enum Color { RED, GREEN };
        @verbatim(language="rust", placement=BEFORE_DECLARATION, text="// union pre")
        union U switch (long) { case 0: long a; };
    "#,
    );
    assert!(
        rust.contains("// enum pre"),
        "enum verbatim missing:\n{rust}"
    );
    assert!(
        rust.contains("// union pre"),
        "union verbatim missing:\n{rust}"
    );
}
