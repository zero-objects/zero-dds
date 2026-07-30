// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `@verbatim` codegen-hook tests for idl-python (XTypes 1.3 §7.2.2.4.8 +
//! IDL 4.2 §8.3.5.1). Item 14 of feature-completeness-F1: idl-python parsed
//! the `VerbatimSpec` but never emitted it — the annotation content was dropped.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_python::{PythonGenOptions, generate_python_module};

fn emit(idl: &str) -> String {
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    generate_python_module(&ast, &PythonGenOptions::default()).expect("gen")
}

#[test]
fn verbatim_before_declaration_appears_before_class() {
    let py = emit(
        r##"
        @verbatim(language="python", placement=BEFORE_DECLARATION, text="# pre-decl marker")
        struct PlainStruct { long x; };
    "##,
    );
    assert!(
        py.contains("# pre-decl marker"),
        "@verbatim BEFORE_DECLARATION missing:\n{py}"
    );
    let pos_marker = py.find("# pre-decl marker").unwrap_or(usize::MAX);
    let pos_class = py.find("class PlainStruct").unwrap_or(usize::MAX);
    assert!(
        pos_marker < pos_class,
        "BEFORE_DECLARATION must precede the class:\n{py}"
    );
}

#[test]
fn verbatim_after_declaration_appears_after_class() {
    let py = emit(
        r##"
        @verbatim(language="python", placement=AFTER_DECLARATION, text="# trailer marker")
        struct S { long x; };
    "##,
    );
    let pos_marker = py.find("# trailer marker").unwrap_or(usize::MAX);
    let pos_class = py.find("class S").unwrap_or(usize::MAX);
    assert!(
        pos_marker != usize::MAX && pos_marker > pos_class,
        "AFTER_DECLARATION must follow the class:\n{py}"
    );
}

#[test]
fn verbatim_default_placement_is_after_declaration() {
    // No explicit placement → spec default AFTER_DECLARATION (§8.3.5.1).
    let py = emit(
        r##"
        @verbatim(language="py", text="# default-placement marker")
        struct S { long x; };
    "##,
    );
    let pos_marker = py.find("# default-placement marker").unwrap_or(usize::MAX);
    let pos_class = py.find("class S").unwrap_or(usize::MAX);
    assert!(
        pos_marker != usize::MAX && pos_marker > pos_class,
        "default placement must be AFTER_DECLARATION:\n{py}"
    );
}

#[test]
fn verbatim_begin_and_end_declaration_inside_class_body() {
    let py = emit(
        r##"
        @verbatim(language="python", placement=BEGIN_DECLARATION, text="# begin body")
        @verbatim(language="python", placement=END_DECLARATION, text="# end body")
        struct S { long x; };
    "##,
    );
    let pos_class = py.find("class S:").unwrap_or(usize::MAX);
    let pos_begin = py.find("# begin body").unwrap_or(usize::MAX);
    let pos_field = py.find("    x: Int32").unwrap_or(usize::MAX);
    let pos_end = py.find("# end body").unwrap_or(usize::MAX);
    assert!(
        pos_class < pos_begin && pos_begin < pos_field && pos_field < pos_end,
        "BEGIN/END_DECLARATION must bracket the fields inside the class body:\n{py}"
    );
    // The verbatim lines are indented into the class body.
    assert!(
        py.contains("    # begin body"),
        "begin body must be indented:\n{py}"
    );
}

#[test]
fn verbatim_begin_declaration_replaces_pass_for_empty_struct() {
    // An empty struct normally emits `pass`; a BEGIN_DECLARATION verbatim fills
    // the body, so no spurious `pass` should follow the marker as the field.
    let py = emit(
        r##"
        @verbatim(language="python", placement=BEGIN_DECLARATION, text="# body content")
        struct Empty { };
    "##,
    );
    assert!(
        py.contains("    # body content"),
        "body verbatim missing:\n{py}"
    );
    let class_body = py.split("class Empty:").nth(1).expect("class body present");
    assert!(
        !class_body.contains("    pass"),
        "verbatim body must suppress the placeholder `pass`:\n{py}"
    );
}

#[test]
fn verbatim_begin_file_and_end_file() {
    let py = emit(
        r##"
        @verbatim(language="python", placement=BEGIN_FILE, text="# top of file")
        @verbatim(language="python", placement=END_FILE, text="# bottom of file")
        struct S { long x; };
    "##,
    );
    let pos_top = py.find("# top of file").unwrap_or(usize::MAX);
    let pos_class = py.find("class S").unwrap_or(usize::MAX);
    let pos_bottom = py.find("# bottom of file").unwrap_or(usize::MAX);
    assert!(
        pos_top < pos_class && pos_class < pos_bottom,
        "BEGIN_FILE before all types, END_FILE after:\n{py}"
    );
}

#[test]
fn verbatim_wildcard_language_applies() {
    let py = emit(
        r##"
        @verbatim(language="*", placement=BEFORE_DECLARATION, text="# universal pre")
        struct S { long x; };
    "##,
    );
    assert!(py.contains("# universal pre"), "wildcard must match:\n{py}");
}

#[test]
fn verbatim_other_language_skipped() {
    let py = emit(
        r##"
        @verbatim(language="java", placement=BEFORE_DECLARATION, text="# not for python")
        struct S { long x; };
    "##,
    );
    assert!(
        !py.contains("# not for python"),
        "a foreign language tag must not emit:\n{py}"
    );
}

#[test]
fn verbatim_applies_to_enum() {
    let py = emit(
        r##"
        @verbatim(language="python", placement=BEFORE_DECLARATION, text="# enum pre")
        enum Color { RED, GREEN };
    "##,
    );
    assert!(py.contains("# enum pre"), "enum verbatim missing:\n{py}");
}
