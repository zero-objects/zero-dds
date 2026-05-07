// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! L2 Codegen-Conformance — prueft dass `idl-cpp::generate_c_header` fuer
//! die V-1..V-12 IDL-Snippets aus `zerodds-xcdr2-bindings-conformance-1.0`
//! die erwarteten C99-Strukturen + TypeSupport-Tabellen emittiert.
//!
//! Inhaltliche L1-Wire-Pruefung passiert in
//! `xcdr2_wire_vectors.rs`. Hier wird nur die Codegen-Surface
//! verifiziert: Type-Name, Extensibility-Flag, Member-Mapping, KeyHash-
//! Emission.

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
use zerodds_idl_cpp::{CGenOptions, generate_c_header};

fn gen_c(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_c_header(&ast, &CGenOptions::default()).expect("c-gen")
}

#[test]
fn v1_codegen() {
    let h = gen_c("@final struct Empty {};");
    assert!(h.contains("typedef struct Empty_s"));
    assert!(h.contains("Empty_typesupport"));
    assert!(h.contains(".extensibility = 0"));
}

#[test]
fn v2_codegen() {
    let h = gen_c("@final struct Point { long x; long y; };");
    assert!(h.contains("int32_t x;"));
    assert!(h.contains("int32_t y;"));
    assert!(h.contains("\"Point\""));
}

#[test]
fn v3_codegen_all_primitives() {
    let h = gen_c(
        "@final struct All { boolean b; octet o; short s; unsigned short us; \
         long l; unsigned long ul; long long ll; unsigned long long ull; \
         float f; double d; };",
    );
    assert!(h.contains("uint8_t b;"));
    assert!(h.contains("uint8_t o;"));
    assert!(h.contains("int16_t s;"));
    assert!(h.contains("uint16_t us;"));
    assert!(h.contains("int32_t l;"));
    assert!(h.contains("uint32_t ul;"));
    assert!(h.contains("int64_t ll;"));
    assert!(h.contains("uint64_t ull;"));
    assert!(h.contains("float f;"));
    assert!(h.contains("double d;"));
}

#[test]
fn v4_codegen_string() {
    let h = gen_c("@final struct Greeting { string text; };");
    assert!(h.contains("char* text;"));
}

#[test]
fn v5_codegen_sequence_int32() {
    let h = gen_c("@final struct Bag { sequence<long> ids; };");
    assert!(h.contains("uint32_t len; int32_t* elems;"));
}

#[test]
fn v6_codegen_sequence_string() {
    let h = gen_c("@final struct Tags { sequence<string> tags; };");
    assert!(h.contains("uint32_t len; char** elems;"));
}

#[test]
fn v7_codegen_nested_modules() {
    let h = gen_c("module Outer { module Inner { @final struct S { long x; }; }; };");
    assert!(h.contains("typedef struct Outer_Inner_S_s"));
    assert!(h.contains("\"Outer::Inner::S\""));
}

#[test]
fn v8_codegen_keyed() {
    let h = gen_c("@final struct Sensor { @key long id; double value; };");
    assert!(h.contains(".is_keyed = 1"));
    assert!(h.contains("Sensor_key_hash"));
}

#[test]
fn v9_codegen_appendable() {
    let h = gen_c("@appendable struct V { long a; long b; };");
    assert!(h.contains(".extensibility = 1"));
}

#[test]
fn v10_codegen_mutable_with_id() {
    let h = gen_c("@mutable struct M { @id(1) long a; @id(2) string b; };");
    assert!(h.contains(".extensibility = 2"));
    // EMHEADER mit LC=4, id=1 → 0x40000001; id=2 → 0x40000002.
    assert!(h.contains("0x40000001"));
    assert!(h.contains("0x40000002"));
}

#[test]
fn v11_codegen_optional_field_present_via_mutable_emheader() {
    // Optional in @mutable wird via EMHEADER-Anwesenheit signalisiert;
    // unsere c_mode-Implementation emittiert die EMHEADER unconditionally
    // (rc1-scope: kein @optional-Flag-Handling). Test prueft zumindest
    // die Mutable-Surface.
    let h = gen_c("@mutable struct O { @id(1) long maybe; };");
    assert!(h.contains(".extensibility = 2"));
    assert!(h.contains("0x40000001"));
}

#[test]
fn v12_codegen_no_explicit_sentinel_emission() {
    // Im Encoder-Body darf KEIN PID_LIST_END-Sentinel-Schreiben stehen.
    let h = gen_c("@mutable struct M { @id(1) long a; };");
    assert!(!h.contains("PID_LIST_END"));
    assert!(!h.contains("0x3F02"));
}
