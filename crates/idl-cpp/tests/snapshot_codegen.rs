//! Snapshot-Tests fuer den C++ Codegen-Output.
//!
//! Statt brittler `assert!(contains(...))`-Patterns vergleichen wir
//! den vollstaendigen emittierten Header gegen ein Snapshot-File.
//! Aenderungen werden via `cargo insta review` interaktiv akzeptiert
//! oder verworfen. Faengt unbeabsichtigte Code-Drift im Codegen.

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
use zerodds_idl_cpp::{CppGenOptions, generate_cpp_header, generate_cpp_header_with_amqp};

fn gen_default(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen")
}

fn gen_with_amqp(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_cpp_header_with_amqp(&ast, &CppGenOptions::default()).expect("gen")
}

#[test]
fn snapshot_simple_struct() {
    let cpp = gen_default("struct Point { long x; long y; };");
    insta::assert_snapshot!(cpp);
}

#[test]
fn snapshot_struct_with_string_and_sequence() {
    let cpp =
        gen_default("struct Bag { string name; sequence<long> ids; sequence<string, 16> tags; };");
    insta::assert_snapshot!(cpp);
}

#[test]
fn snapshot_module_nesting() {
    let cpp = gen_default("module Outer { module Inner { struct S { long x; }; }; };");
    insta::assert_snapshot!(cpp);
}

#[test]
fn snapshot_enum_and_typedef() {
    let cpp = gen_default("enum Color { RED, GREEN, BLUE }; typedef Color Hue;");
    insta::assert_snapshot!(cpp);
}

#[test]
fn snapshot_union() {
    let cpp = gen_default(
        "union U switch (long) { case 1: long a; case 2: double b; default: octet c; };",
    );
    insta::assert_snapshot!(cpp);
}

#[test]
fn snapshot_keyed_struct() {
    let cpp = gen_default("struct Sensor { @key long id; double value; };");
    insta::assert_snapshot!(cpp);
}

#[test]
fn snapshot_optional_member() {
    let cpp = gen_default("struct S { @optional long maybe; };");
    insta::assert_snapshot!(cpp);
}

#[test]
fn snapshot_inheritance() {
    let cpp =
        gen_default("struct Base { long base_field; }; struct Child : Base { long child_field; };");
    insta::assert_snapshot!(cpp);
}

#[test]
fn snapshot_amqp_helpers_struct() {
    let cpp = gen_with_amqp("struct Sensor { long id; double temp; };");
    insta::assert_snapshot!(cpp);
}

#[test]
fn snapshot_amqp_helpers_union() {
    let cpp = gen_with_amqp("union U switch (long) { case 1: long a; case 2: double b; };");
    insta::assert_snapshot!(cpp);
}
