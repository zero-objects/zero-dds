//! Snapshot-Tests fuer den Java Codegen-Output.
//!
//! Vergleicht die emittierten `.java`-Files gegen Snapshot-Files.
//! Aenderungen via `cargo insta review`.

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
use zerodds_idl_java::{JavaGenOptions, generate_java_files, generate_java_files_with_amqp};

fn gen_default(src: &str) -> String {
    // Snapshots fokussieren auf POJO-Output. TypeSupport-Snapshots
    // leben in separaten Tests (siehe `snapshot_typesupport_*`).
    let opts = JavaGenOptions {
        emit_typesupport: false,
        ..Default::default()
    };
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    let files = generate_java_files(&ast, &opts).expect("gen");
    let mut combined = String::new();
    for f in &files {
        combined.push_str(&format!(
            "// === FILE: {}/{}.java ===\n",
            f.package_path, f.class_name
        ));
        combined.push_str(&f.source);
        combined.push('\n');
    }
    combined
}

fn gen_with_amqp(src: &str) -> String {
    let opts = JavaGenOptions {
        emit_typesupport: false,
        ..Default::default()
    };
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    let files = generate_java_files_with_amqp(&ast, &opts).expect("gen");
    let mut combined = String::new();
    for f in &files {
        combined.push_str(&format!(
            "// === FILE: {}/{}.java ===\n",
            f.package_path, f.class_name
        ));
        combined.push_str(&f.source);
        combined.push('\n');
    }
    combined
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

#[test]
fn snapshot_inheritance() {
    insta::assert_snapshot!(gen_default(
        "struct Base { long base_field; }; struct Child : Base { long child_field; };"
    ));
}

#[test]
fn snapshot_amqp_helpers_struct() {
    insta::assert_snapshot!(gen_with_amqp("struct Sensor { long id; double temp; };"));
}

#[test]
fn snapshot_amqp_helpers_union() {
    insta::assert_snapshot!(gen_with_amqp(
        "union U switch (long) { case 1: long a; case 2: double b; };"
    ));
}

fn gen_typesupport(src: &str) -> String {
    // Nur die TypeSupport-Files (zerodds-xcdr2-java-1.0 §4).
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    let files = generate_java_files(&ast, &JavaGenOptions::default()).expect("gen");
    let mut combined = String::new();
    for f in &files {
        if !f.class_name.ends_with("TypeSupport") {
            continue;
        }
        combined.push_str(&format!(
            "// === FILE: {}/{}.java ===\n",
            f.package_path, f.class_name
        ));
        combined.push_str(&f.source);
        combined.push('\n');
    }
    combined
}

#[test]
fn snapshot_typesupport_final_struct() {
    // V-2 aus zerodds-xcdr2-bindings-conformance-1.0 §6.
    insta::assert_snapshot!(gen_typesupport("@final struct Point { long x; long y; };"));
}

#[test]
fn snapshot_typesupport_keyed_struct() {
    // V-8 aus Conformance-Spec.
    insta::assert_snapshot!(gen_typesupport(
        "@final struct Sensor { @key long id; double value; };"
    ));
}

#[test]
fn snapshot_typesupport_mutable_struct() {
    // V-10 aus Conformance-Spec.
    insta::assert_snapshot!(gen_typesupport(
        "@mutable struct M { @id(1) long a; @id(2) string b; };"
    ));
}
