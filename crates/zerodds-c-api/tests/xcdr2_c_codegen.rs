// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! L2 codegen conformance — the C shape of the V-1..V-12 IDL snippets
//! from `zerodds-xcdr2-bindings-conformance-1.0`.
//!
//! The content-level L1 wire check happens in `xcdr2_wire_vectors.rs`.
//! Here only the declaration surface is verified: the type name, the
//! member mapping, the scope encoding and the keyword escape.
//!
//! # What produces this, and what it no longer produces
//!
//! The generator is `zerodds-idl-parse` + `zerodds-idl-lower` for the
//! model, the C rule catalogue in `zerodds-lang-rules` for the shape and
//! `zerodds-lang-render` for the text. It replaces `idl-cpp`'s
//! `generate_c_header`, and it is not a rename of it — five of the
//! mappings below are different, and each difference is stated at its
//! test:
//!
//! | | `idl-cpp` | here |
//! |---|---|---|
//! | `boolean` | `uint8_t` | `bool` |
//! | `string` | `char *` | `zd_string` |
//! | `sequence<T>` | two inline members | a named `zd_seq_*` |
//! | `Outer::Inner::S` | `Outer_sInner_sS` | `Outer_Inner_S` |
//!
//! **The DDS TypeSupport table is not among them, and its assertions are
//! not silently dropped.** `idl-cpp` also emitted a
//! `<Type>_typesupport` with `.extensibility`, `.is_keyed`, the
//! `<Type>_key_hash` function and the literal EMHEADERs; the rebuilt
//! backend arc emits the C shape of a type and no DDS binding around it,
//! so those six assertions have no producer in this tree. They are
//! carried in `docs/OPEN-ITEMS.md` under the C TypeSupport backend, with
//! the snippets and the expected values, rather than restated here
//! against something that cannot answer them.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::uninlined_format_args,
    missing_docs
)]

use zerodds_lang_cst::c;
use zerodds_lang_cst::common::{Doc, Ident};
use zerodds_lang_rules::adapter::c::c_items;
use zerodds_lang_rules::rule;
use zerodds_lang_rules::{ModelGraph, apply};

/// As [`gen_c`], and hands back the materialiser's refusal instead of
/// panicking on it.
///
/// A construct C has no form for is a refusal by name, and a test that
/// asserts the refusal needs to read it. `gen_c` unwraps, which is right
/// for the cases that have a form.
///
/// # Errors
/// Whatever the C materialiser refused, with the construct named.
pub fn render_c(src: &str) -> Result<String, zerodds_lang_rules::adapter::cst::MaterialiseError> {
    let parsed = zerodds_idl_parse::parse(src);
    assert!(!parsed.has_errors(), "parse: {:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "conformance.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("resolve");
    assert!(report.is_clean(), "unresolved: {:?}", report.broken);

    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &rule::c::catalogue()).expect("apply the C rule catalogue");
    let decls = c_items(&ast, &applied)?;

    Ok(zerodds_lang_render::c::file(&c::File {
        doc: Doc::lines(&["Generated for the XCDR2 C binding conformance test."]),
        guard: Some(Ident::new("ZD_CONFORMANCE_H")),
        includes: vec![
            c::Include::system("stdbool.h"),
            c::Include::system("stddef.h"),
            c::Include::system("stdint.h"),
        ],
        decls,
    }))
}

/// One IDL source through the front end and the C rule catalogue.
///
/// Every stage that can report is asserted rather than unwrapped: a
/// snippet that stops resolving has to say so here and not produce an
/// empty header that the `contains` assertions below would then read as
/// a mapping failure.
pub fn gen_c(src: &str) -> String {
    let parsed = zerodds_idl_parse::parse(src);
    assert!(!parsed.has_errors(), "parse: {:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "conformance.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("resolve");
    assert!(report.is_clean(), "unresolved: {:?}", report.broken);

    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &rule::c::catalogue()).expect("apply the C rule catalogue");
    let decls = c_items(&ast, &applied).expect("materialise the C declarations");

    zerodds_lang_render::c::file(&c::File {
        doc: Doc::lines(&["Generated for the XCDR2 C binding conformance test."]),
        guard: Some(Ident::new("ZD_CONFORMANCE_H")),
        includes: vec![
            c::Include::system("stdbool.h"),
            c::Include::system("stddef.h"),
            c::Include::system("stdint.h"),
        ],
        decls,
    })
}

#[test]
fn v1_codegen_empty_final() {
    // This test asserted `struct Empty {` for years and never compiled
    // it. C23 §6.7.2.1p8 leaves a structure with no named member
    // undefined; gcc and clang take it only as an extension and the
    // probes build with `-pedantic-errors`, where it is an error. What
    // the assertion held open was a header nothing could build.
    //
    // The refusal is the correct answer and is what is asserted now. C++
    // has no such rule and carries the case; the octets the Rust and
    // Python columns write for it — a DHEADER of zero under XCDR2 — are
    // in the corpus as `empty-struct`.
    let refused = render_c("@final struct Empty {};");
    let message = format!("{}", refused.expect_err("C has no empty aggregate"));
    assert!(
        message.contains("no members"),
        "the refusal names the construct: {message}"
    );
}

#[test]
fn v2_codegen_two_longs() {
    let h = gen_c("@final struct Point { long x; long y; };");
    assert!(h.contains("int32_t x;"), "{h}");
    assert!(h.contains("int32_t y;"), "{h}");
    assert!(h.contains("typedef struct Point Point;"), "{h}");
}

#[test]
fn v3_codegen_all_primitives() {
    let h = gen_c(
        "@final struct All { boolean b; octet o; short s; unsigned short us; \
         long l; unsigned long ul; long long ll; unsigned long long ull; \
         float f; double d; };",
    );
    // `boolean` is C99 `bool` and not `uint8_t`. `idl-cpp` wrote the
    // octet, which is the same one byte on every target it was built
    // for but is not what the language mapping says, and it made
    // `boolean` and `octet` indistinguishable in the emitted C.
    assert!(h.contains("bool b;"), "{h}");
    assert!(h.contains("uint8_t o;"), "{h}");
    assert!(h.contains("int16_t s;"), "{h}");
    assert!(h.contains("uint16_t us;"), "{h}");
    assert!(h.contains("int32_t l;"), "{h}");
    assert!(h.contains("uint32_t ul;"), "{h}");
    assert!(h.contains("int64_t ll;"), "{h}");
    assert!(h.contains("uint64_t ull;"), "{h}");
    assert!(h.contains("float f;"), "{h}");
    assert!(h.contains("double d;"), "{h}");
}

#[test]
fn v4_codegen_string() {
    let h = gen_c("@final struct Greeting { string text; };");
    // `zd_string` and not a bare `char *`: the prelude declares it as
    // `const char *`, which states that the write path does not own the
    // bytes and does not free them.
    assert!(h.contains("zd_string text;"), "{h}");
    assert!(h.contains("typedef const char *zd_string;"), "{h}");
}

#[test]
fn v5_codegen_sequence_int32() {
    let h = gen_c("@final struct Bag { sequence<long> ids; };");
    // A named element type per sequence instead of `idl-cpp`'s two
    // inline members: two structures with a `sequence<long>` each now
    // share one declaration, and a sequence can be passed by itself.
    assert!(h.contains("struct zd_seq_long {"), "{h}");
    assert!(h.contains("uint32_t _length;"), "{h}");
    assert!(h.contains("int32_t *_buffer;"), "{h}");
    assert!(h.contains("zd_seq_long ids;"), "{h}");
}

#[test]
fn v6_codegen_sequence_string() {
    let h = gen_c("@final struct Tags { sequence<string> labels; };");
    assert!(h.contains("struct zd_seq_zd_string {"), "{h}");
    assert!(h.contains("zd_string *_buffer;"), "{h}");
    assert!(h.contains("zd_seq_zd_string labels;"), "{h}");
}

#[test]
fn v7_codegen_nested_modules() {
    let h = gen_c("module Outer { module Inner { @final struct S { long x; }; }; };");
    // `Outer::Inner::S` -> `Outer_Inner_S`. `idl-cpp` wrote
    // `Outer_sInner_sS`: an `_s` per separator, to stay injective
    // against an identifier that itself contains an underscore. The
    // model resolves scoped names before the C name is formed, so a
    // collision is a `DuplicateName` from the front end rather than
    // something the separator has to encode around.
    assert!(
        h.contains("typedef struct Outer_Inner_S Outer_Inner_S;"),
        "{h}"
    );
    assert!(h.contains("int32_t x;"), "{h}");
}

#[test]
fn v8_codegen_keyed() {
    // The `@key` is a wire property, not a shape one: the C structure of
    // a keyed type is the C structure of the same type without the
    // annotation. What the key changes — the KeyHash and the TypeSupport
    // `.is_keyed` — is the open TypeSupport backend's, see the module doc.
    let h = gen_c("@final struct Sensor { @key long id; double value; };");
    assert!(h.contains("int32_t id;"), "{h}");
    assert!(h.contains("double value;"), "{h}");
}

#[test]
fn v9_codegen_appendable() {
    let h = gen_c("@appendable struct V { long a; long b; };");
    assert!(h.contains("typedef struct V V;"), "{h}");
    assert!(h.contains("int32_t a;"), "{h}");
    assert!(h.contains("int32_t b;"), "{h}");
}

#[test]
fn v10_codegen_mutable_with_id() {
    let h = gen_c("@mutable struct M { @id(1) long a; @id(2) string b; };");
    assert!(h.contains("int32_t a;"), "{h}");
    assert!(h.contains("zd_string b;"), "{h}");
}

#[test]
fn v11_codegen_mutable_single_member() {
    let h = gen_c("@mutable struct O { @id(1) long maybe; };");
    assert!(h.contains("int32_t maybe;"), "{h}");
}

#[test]
fn v12_codegen_keyword_identifiers() {
    // IDL identifiers that are C keywords — `for`, `register`, `static`,
    // and `int` as a type name — escape with a trailing underscore at
    // every emit site. `xcdr2_c_compile.rs` puts the same snippet
    // through `-std=c99 -Wall -Werror`, which is the proof that the
    // escape reaches all of them and not only the ones read here.
    let h = gen_c(
        "union U switch (long) { case 0: long for; case 1: long while_val; }; \
         @final struct int { long register; long static; U discriminated; };",
    );
    assert!(h.contains("int32_t for_;"), "{h}");
    assert!(h.contains("int32_t while_val;"), "{h}");
    assert!(h.contains("typedef struct int_ int_;"), "{h}");
    assert!(h.contains("int32_t register_;"), "{h}");
    assert!(h.contains("int32_t static_;"), "{h}");
}

#[test]
fn a_union_and_a_member_differing_only_in_case_is_rejected() {
    // The V-12 snippet used to read `U u;` for the union member. OMG IDL
    // 4.2 §7.2.3.1 gives a scope one namespace and compares identifiers
    // without regard to case, so `U` and `u` in one scope is not IDL —
    // the previous front end accepted it and this one names it. The
    // snippet above was corrected rather than the rule relaxed, and this
    // test is what keeps the correction from being read as a workaround
    // for a parser that got stricter by accident.
    let parsed = zerodds_idl_parse::parse(
        "union U switch (long) { case 0: long a; }; @final struct S { U u; };",
    );
    assert!(!parsed.has_errors(), "the syntax itself is well-formed");
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "case-collision.idl");
    assert!(
        lowered
            .diagnostics
            .iter()
            .any(|d| format!("{:?}", d.code).contains("DuplicateName")),
        "expected a DuplicateName diagnostic, got {:?}",
        lowered.diagnostics
    );
}
