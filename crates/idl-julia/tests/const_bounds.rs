// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Named `const` and arithmetic const-expr resolution in collection/array
//! bounds and union labels (§7.4.1.4.4), parity with idl-rust's
//! `eval_const_i128` / idl-zig's `eval_const_int`. Before this the Julia backend
//! only accepted an integer literal or a unary-signed literal: a named bound
//! (`sequence<octet, MAX>`) degraded silently to unbounded, and `char[LEN]`
//! raised `Unsupported`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_julia::{JuliaGenOptions, generate_julia_module};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_julia_module(&ast, &JuliaGenOptions::default()).expect("gen")
}

/// A named `const` in a `sequence<T, N>` bound resolves to its integer value,
/// so the encode/decode bound check compares against the real bound (not `0`,
/// the former silent degrade).
#[test]
fn named_const_sequence_bound_resolves() {
    let out = emit("const long MAX = 4;\n@final struct S { sequence<octet, MAX> data; };");
    assert!(
        out.contains("length(v.data) > 4"),
        "named sequence bound must resolve to 4, got:\n{out}"
    );
    assert!(
        out.contains("decoded sequence length exceeds its IDL bound (4)"),
        "decode-side bound must resolve to 4, got:\n{out}"
    );
}

/// A named `const` in a fixed-array dimension resolves — `char[LEN]` no longer
/// raises `Unsupported` and emits a `1:LEN` row-major loop.
#[test]
fn named_const_array_dimension_resolves() {
    let out = emit("const long LEN = 3;\n@final struct S { char buf[LEN]; };");
    assert!(out.contains("buf::Vector{Char}"), "{out}");
    assert!(
        out.contains("for zdi0 in 1:3"),
        "char[LEN] must resolve LEN=3 into a 1:3 loop, got:\n{out}"
    );
}

/// An arithmetic const-expr bound folds (`MAX * 2` → 8).
#[test]
fn arithmetic_const_expr_bound_folds() {
    let out = emit("const long MAX = 4;\n@final struct S { sequence<octet, MAX * 2> data; };");
    assert!(
        out.contains("length(v.data) > 8"),
        "arithmetic bound MAX*2 must fold to 8, got:\n{out}"
    );
}

/// A named `const` in a bounded `string<N>` / `map<K,V,N>` resolves too.
#[test]
fn named_const_string_and_map_bounds_resolve() {
    let out = emit(
        "const long CAP = 16;\n\
         @final struct S { string<CAP> name; map<long, long, CAP> table; };",
    );
    assert!(
        out.contains("sizeof(v.name)") && out.contains("> 16"),
        "string<CAP> bound must resolve to 16, got:\n{out}"
    );
    assert!(
        out.contains("length(v.table) > 16"),
        "map<..,CAP> bound must resolve to 16, got:\n{out}"
    );
}

/// A union `case` label that is an arithmetic const-expr over a named `const`
/// folds to its integer discriminant (`K + 1` → 2).
#[test]
fn arithmetic_const_union_label_folds() {
    let out = emit(
        "const long K = 1;\n\
         @final union U switch (long) { case K + 1: long x; default: long y; };",
    );
    assert!(
        out.contains("v.disc == 2"),
        "union label K+1 must fold to 2, got:\n{out}"
    );
}

/// A chained const reference resolves transitively (`A` → `B` → literal).
#[test]
fn chained_const_reference_resolves() {
    let out = emit(
        "const long B = 5;\nconst long A = B;\n\
         @final struct S { sequence<octet, A> data; };",
    );
    assert!(
        out.contains("length(v.data) > 5"),
        "chained const A->B->5 must resolve to 5, got:\n{out}"
    );
}
