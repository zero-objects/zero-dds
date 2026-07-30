// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Const-expression bound resolution (IDL 4.2 §7.4.1.4.4.5): a fixed-array
//! dimension, `sequence`/`string`/`wstring`/`map` bound, or `fixed<P,S>`
//! argument written as a named `const`, an enum literal, an arithmetic
//! expression, or a non-decimal literal (hex/octal/binary/suffixed) must fold
//! to its integer value — matching `idl-rust` (`eval_const_i128`) and
//! `idl-python` (`eval_const_int`). Before this, `array_size` resolved only a
//! bare integer literal or a unary-signed one, so every named/computed bound
//! aborted codegen with `Unsupported` (an array dimension) or silently dropped
//! its bound check (a collection).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_d::{DGenOptions, generate_d_module};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_d_module(&ast, &DGenOptions::default()).expect("gen")
}

#[test]
fn named_const_array_dimension_resolves_ok() {
    let d = emit("const long N = 4;\n@final struct S { long v[N]; };");
    assert!(
        d.contains("int[4] v;"),
        "named-const array dimension must fold to int[4]:\n{d}"
    );
}

#[test]
fn multidim_named_const_array_resolves_ok() {
    let d = emit("const long R = 2;\nconst long C = 3;\n@final struct S { long m[R][C]; };");
    assert!(
        d.contains("int[2][3] m;"),
        "each named-const dimension must fold independently:\n{d}"
    );
}

#[test]
fn binary_expr_array_dimension_resolves_ok() {
    let d = emit("const long BASE = 4;\n@final struct S { long v[BASE * 2]; };");
    assert!(
        d.contains("int[8] v;"),
        "arithmetic array dimension must fold to int[8]:\n{d}"
    );
}

#[test]
fn octal_array_dimension_resolves_ok() {
    // OMG IDL octal: a leading `0` → base 8, so `010` is 8, not 10.
    let d = emit("@final struct S { long v[010]; };");
    assert!(
        d.contains("int[8] v;"),
        "octal `010` array dimension must fold to 8:\n{d}"
    );
}

#[test]
fn named_const_sequence_bound_enforced_ok() {
    let d = emit("const long MAX = 8;\n@final struct S { sequence<long, MAX> xs; };");
    assert!(
        d.contains("exceeds its IDL bound (8)"),
        "named-const sequence bound must emit the bound check with value 8:\n{d}"
    );
}

#[test]
fn binary_expr_sequence_bound_enforced_ok() {
    let d = emit("const long BASE = 4;\n@final struct S { sequence<long, BASE + 4> xs; };");
    assert!(
        d.contains("exceeds its IDL bound (8)"),
        "arithmetic sequence bound (4+4) must fold to 8:\n{d}"
    );
}

#[test]
fn named_const_string_bound_enforced_ok() {
    let d = emit("const long L = 16;\n@final struct S { string<L> name; };");
    assert!(
        d.contains(".length > 16") && d.contains("exceeds its IDL bound (16)"),
        "named-const string bound must fold to 16 on both sides:\n{d}"
    );
    assert!(
        d.contains("decoded string length exceeds its IDL bound (16)"),
        "decode side must enforce the folded bound too:\n{d}"
    );
}

#[test]
fn named_const_map_bound_enforced_ok() {
    let d = emit("const long CAP = 4;\n@final struct S { map<long, long, CAP> m; };");
    assert!(
        d.contains("exceeds its IDL bound (4)"),
        "named-const map bound must fold to 4:\n{d}"
    );
}

#[test]
fn named_const_fixed_digits_resolves_ok() {
    // fixed<P,S>: wire = (P+2)/2 packed-BCD octets. P=5 → getBytesN(3).
    let d = emit(
        "const long PREC = 5;\nconst long SCALE = 2;\n@final struct S { fixed<PREC, SCALE> amount; };",
    );
    assert!(
        d.contains("getBytesN(3)"),
        "named-const fixed<P,S> must resolve P=5 → (5+2)/2 = 3 BCD octets:\n{d}"
    );
}

#[test]
fn const_referencing_earlier_const_folds_ok() {
    // §7.4.1.4.4: a const may reference an earlier one; the chain must fold.
    let d = emit("const long A = 3;\nconst long B = A + 1;\n@final struct S { long v[B]; };");
    assert!(
        d.contains("int[4] v;"),
        "const-referencing-const chain must fold to int[4]:\n{d}"
    );
}

#[test]
fn hex_literal_bound_resolves_ok() {
    let d = emit("@final struct S { long v[0x10]; };");
    assert!(
        d.contains("int[16] v;"),
        "hex `0x10` array dimension must fold to 16:\n{d}"
    );
}
