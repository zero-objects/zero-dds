// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `const_expr` collection-bound / array-dimension / union-label resolution for
//! the Elixir backend (IDL 4.2 §7.4.1.4.4 — idl-rust/idl-zig parity).
//!
//! idl-elixir's bound evaluator (`array_size`) previously accepted only a bare
//! integer literal + unary sign: a NAMED array dimension (`long a[LEN]`) was a
//! hard `Unsupported` error, a NAMED / arithmetic sequence-or-map bound
//! (`sequence<octet, MAX>`, `sequence<octet, 2 * 5>`) silently degraded to
//! unbounded, and a NAMED / arithmetic union `case` label dropped the case.
//! `array_size` now delegates to a shared `eval_const_int` that resolves named
//! `const`s and enumerators and folds IDL arithmetic/bitwise operators, so all
//! three sites resolve to the SAME integer idl-rust/idl-zig produce.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_elixir::{ElixirGenOptions, generate_elixir_module};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_elixir_module(&ast, &ElixirGenOptions::default()).expect("gen")
}

/// A named `const` array dimension resolves to its integer — the fixed-array
/// decoder reduces over `1..LEN//1`. Previously `Unsupported("non-literal
/// array size")`.
#[test]
fn named_const_array_dimension_resolves() {
    let e = emit("const long LEN = 4;\n@final struct S { long arr[LEN]; };");
    assert!(
        e.contains("1..4//1"),
        "named array dimension LEN must fold to 4 (decode reduce `1..4//1`):\n{e}"
    );
}

/// A named `const` sequence bound is enforced on encode + decode at the folded
/// value. Previously the bound was dropped (unbounded).
#[test]
fn named_const_sequence_bound_resolves_and_enforced() {
    let e = emit("const long MAX = 8;\n@final struct S { sequence<octet, MAX> data; };");
    assert!(
        e.contains("length(v.data) > 8")
            && e.contains("bounded sequence length exceeds its IDL bound (8)"),
        "named sequence bound MAX must fold to 8 on encode:\n{e}"
    );
    assert!(
        e.contains("decoded sequence length exceeds its IDL bound (8)"),
        "named sequence bound MAX must fold to 8 on decode:\n{e}"
    );
}

/// An arithmetic bound (`2 * 5`) folds to 10 (const-expr operator folding).
#[test]
fn arithmetic_sequence_bound_folds() {
    let e = emit("const long CAP = 2 * 5;\n@final struct S { sequence<long, CAP> xs; };");
    assert!(
        e.contains("length(v.xs) > 10")
            && e.contains("bounded sequence length exceeds its IDL bound (10)"),
        "CAP = 2 * 5 must fold to 10:\n{e}"
    );
}

/// A named `const` string bound folds and is enforced (UTF-8 byte length).
#[test]
fn named_const_string_bound_resolves() {
    let e = emit("const long N = 16;\n@final struct S { string<N> name; };");
    assert!(
        e.contains("byte_size(v.name) > 16")
            && e.contains("bounded string length exceeds its IDL bound (16)"),
        "named string bound N must fold to 16:\n{e}"
    );
}

/// A named `const` map bound folds and is enforced on encode + decode.
#[test]
fn named_const_map_bound_resolves() {
    let e = emit("const long M = 2;\n@final struct S { map<string, long, M> vals; };");
    assert!(
        e.contains("map_size(v.vals) > 2")
            && e.contains("bounded map length exceeds its IDL bound (2)"),
        "named map bound M must fold to 2 on encode:\n{e}"
    );
    assert!(
        e.contains("decoded map length exceeds its IDL bound (2)"),
        "named map bound M must fold to 2 on decode:\n{e}"
    );
}

/// A union `case` label that names a `const` folds to the const's integer — the
/// generated `case` clause matches that literal. `1 + 1` → `2`.
#[test]
fn arithmetic_union_label_folds() {
    let e = emit(
        "const long TWO = 1 + 1;\n\
         union U switch (long) { case TWO: uint32 x; default: octet y; };",
    );
    assert!(
        e.contains("      2 ->"),
        "`case TWO` (=2) must fold to an integer `2 ->` clause:\n{e}"
    );
}

/// A union `case` label that names an enumerator folds to its discriminant.
/// `enum Color { RED, GREEN, BLUE }` → `GREEN` = 1.
#[test]
fn enumerator_union_label_folds() {
    let e = emit(
        "enum Color { RED, GREEN, BLUE };\n\
         union U switch (long) { case GREEN: uint32 x; default: octet y; };",
    );
    assert!(
        e.contains("      1 ->"),
        "`case GREEN` must fold to its discriminant 1:\n{e}"
    );
}

/// A named `const` bitmask/bitfield-adjacent path is unaffected; sanity that a
/// named `fixed<P,S>`-free const bound in a nested module still resolves
/// (registry recurses into modules).
#[test]
fn named_const_in_module_scope_resolves() {
    let e = emit(
        "module m { const long LEN = 3; };\n\
         @final struct S { long arr[m::LEN]; };",
    );
    assert!(
        e.contains("1..3//1"),
        "a module-scoped const bound `m::LEN` must fold to 3:\n{e}"
    );
}
