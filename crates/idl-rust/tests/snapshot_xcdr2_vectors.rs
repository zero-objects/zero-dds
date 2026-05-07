//! Snapshot-Tests fuer XCDR2 wire-vector IDL-Files (V-1..V-12).
//!
//! Wir vergleichen den `idl-rust`-Output gegen abgespeicherte Snapshots,
//! damit Aenderungen an der Codegen-Form auffallen. Spec:
//! `zerodds-xcdr2-bindings-conformance-1.0` §6 + `zerodds-xcdr2-csharp-1.0`.

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
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};

fn gen_default(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_rust_module(&ast, &RustGenOptions::default()).expect("gen")
}

#[test]
fn snapshot_v1_empty_final() {
    insta::assert_snapshot!(gen_default("@final struct Empty {};"));
}

#[test]
fn snapshot_v2_plain_primitives_final() {
    insta::assert_snapshot!(gen_default("@final struct Point { long x; long y; };"));
}

#[test]
fn snapshot_v3_mixed_primitives_final() {
    insta::assert_snapshot!(gen_default(
        "@final struct All {\n\
         boolean b; octet o; short s; unsigned short us;\n\
         long l; unsigned long ul; long long ll; unsigned long long ull;\n\
         float f; double d; };"
    ));
}

#[test]
fn snapshot_v4_string_final() {
    insta::assert_snapshot!(gen_default("@final struct Greeting { string text; };"));
}

#[test]
fn snapshot_v5_seq_int32_final() {
    insta::assert_snapshot!(gen_default("@final struct Bag { sequence<long> ids; };"));
}

#[test]
fn snapshot_v6_seq_string_final() {
    insta::assert_snapshot!(gen_default(
        "@final struct Tags { sequence<string> tags; };"
    ));
}

#[test]
fn snapshot_v7_nested_modules_final() {
    insta::assert_snapshot!(gen_default(
        "module Outer { module Inner { @final struct S { long x; }; }; };"
    ));
}

#[test]
fn snapshot_v8_keyed_final() {
    insta::assert_snapshot!(gen_default(
        "@final struct Sensor { @key long id; double value; };"
    ));
}

#[test]
fn snapshot_v9_appendable() {
    insta::assert_snapshot!(gen_default("@appendable struct V { long a; long b; };"));
}

#[test]
fn snapshot_v10_mutable() {
    insta::assert_snapshot!(gen_default(
        "@mutable struct M { @id(1) long a; @id(2) string b; };"
    ));
}

#[test]
fn snapshot_v11_optional_member_mutable() {
    insta::assert_snapshot!(gen_default(
        "@mutable struct O { @id(1) @optional long maybe; };"
    ));
}
