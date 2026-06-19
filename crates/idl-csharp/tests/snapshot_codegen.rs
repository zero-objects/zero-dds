//! Snapshot tests for the C# codegen output.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_csharp::{CsGenOptions, generate_csharp};

fn gen_default(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_csharp(&ast, &CsGenOptions::default()).expect("gen")
}

#[test]
fn snapshot_simple_struct() {
    insta::assert_snapshot!(gen_default("struct Point { long x; long y; };"));
}

#[test]
fn snapshot_struct_with_string_and_sequence() {
    insta::assert_snapshot!(gen_default(
        "struct Bag { string name; sequence<long> ids; };"
    ));
}

#[test]
fn snapshot_module_nesting() {
    insta::assert_snapshot!(gen_default(
        "module Outer { module Inner { struct S { long x; }; }; };"
    ));
}

#[test]
fn snapshot_enum() {
    insta::assert_snapshot!(gen_default("enum Color { RED, GREEN, BLUE };"));
}

#[test]
fn snapshot_union() {
    insta::assert_snapshot!(gen_default(
        "union U switch (long) { case 1: long a; case 2: double b; default: octet c; };"
    ));
}
