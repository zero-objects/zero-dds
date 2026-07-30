// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bounded-collection enforcement (DDS-XTypes §7.4.3) in the generated C++
//! XCDR2 encode: a `sequence<T, N>` / `string<N>` value longer than its declared
//! bound is rejected on encode (throws — strict vendors reject on the wire).
//! `<stdexcept>` is pulled in only when a bounded collection is present.
//!
//! B1 follow-up (#22 decode-side parity): the encode-side checks above have
//! no decode-side counterpart — a bounded field decoded a well-formed but
//! oversized wire value without complaint (untrusted-input DoS vector,
//! XTypes 1.3 §7.4.3 requires enforcement on BOTH sides). The tests below
//! prove the mirrored decode-side check is emitted for every representation
//! path (`@final`/`@appendable` via `emit_value_read`, `@mutable` via
//! `emit_mutable_member_decode_case`, PL_CDR1 union members via
//! `emit_pl_cdr1_member_decode_case`, which itself calls `emit_value_read`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CppGenOptions, generate_cpp_header};

fn gen_cpp(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen")
}

#[test]
fn bounded_sequence_throws_on_over_bound() {
    let cpp = gen_cpp("@final struct Cap { sequence<octet, 4> data; };");
    assert!(
        cpp.contains(".size() > 4") && cpp.contains("bounded sequence length"),
        "bounded sequence<octet, 4> must throw on over-bound encode:\n{cpp}"
    );
    assert!(
        cpp.contains("#include <stdexcept>"),
        "a bounded collection must pull in <stdexcept>:\n{cpp}"
    );
}

#[test]
fn unbounded_sequence_no_check_and_no_stdexcept() {
    let cpp = gen_cpp("@final struct Free { sequence<octet> data; };");
    assert!(
        !cpp.contains(".size() > "),
        "unbounded sequence must NOT get a bound check:\n{cpp}"
    );
    assert!(
        !cpp.contains("#include <stdexcept>"),
        "no bounded collection → no <stdexcept> (byte-identical header):\n{cpp}"
    );
}

#[test]
fn bounded_string_byte_length_check() {
    let cpp = gen_cpp("@final struct Named { string<16> name; };");
    assert!(
        cpp.contains(".size() > 16") && cpp.contains("bounded string length"),
        "bounded string<16> must throw on byte-length over-bound:\n{cpp}"
    );
}

#[test]
fn decode_rejects_over_bound_string_final() {
    let cpp = gen_cpp("@final struct Named { string<16> name; };");
    assert!(
        cpp.contains("if (zd_bc")
            && cpp.contains("decoded string length exceeds its IDL bound (16)"),
        "decode of bounded string<16> must throw on over-bound decode:\n{cpp}"
    );
}

#[test]
fn decode_rejects_over_bound_wstring_final() {
    let cpp = gen_cpp("@final struct Named { wstring<16> name; };");
    assert!(
        cpp.contains("decoded wstring length exceeds its IDL bound (16)"),
        "decode of bounded wstring<16> must throw on over-bound decode:\n{cpp}"
    );
}

#[test]
fn decode_rejects_over_bound_sequence_final() {
    let cpp = gen_cpp("@final struct Cap { sequence<octet, 4> data; };");
    assert!(
        cpp.contains("if (zd_cnt > 4)")
            && cpp.contains("decoded sequence length exceeds its IDL bound (4)"),
        "decode of bounded sequence<octet,4> must throw on over-bound decode:\n{cpp}"
    );
}

#[test]
fn decode_rejects_over_bound_sequence_of_struct() {
    // Non-octet-element sequence path (general decode loop, not the
    // sequence<octet> fast path) must also carry the bound check.
    let cpp = gen_cpp(
        "@final struct Pt { long x; long y; }; @final struct Cap { sequence<Pt, 3> pts; };",
    );
    assert!(
        cpp.contains("if (zd_cnt > 3)")
            && cpp.contains("decoded sequence length exceeds its IDL bound (3)"),
        "decode of bounded sequence<Pt,3> must throw on over-bound decode:\n{cpp}"
    );
}

#[test]
fn decode_rejects_over_bound_map() {
    let cpp = gen_cpp("@final struct M { map<string, long, 2> vals; };");
    assert!(
        cpp.contains("if (zd_mcnt > 2)")
            && cpp.contains("decoded map length exceeds its IDL bound (2)"),
        "decode of bounded map<string,long,2> must throw on over-bound decode:\n{cpp}"
    );
}

#[test]
fn decode_rejects_over_bound_string_mutable() {
    let cpp = gen_cpp("@mutable struct Named { string<8> name; };");
    assert!(
        cpp.contains("zd_bcs") && cpp.contains("decoded string length exceeds its IDL bound (8)"),
        "@mutable decode path must carry the same bound check as @final:\n{cpp}"
    );
}

#[test]
fn decode_rejects_over_bound_sequence_mutable() {
    let cpp = gen_cpp("@mutable struct Cap { sequence<octet, 4> data; };");
    let checks = cpp
        .matches("decoded sequence length exceeds its IDL bound (4)")
        .count();
    assert!(
        checks >= 1,
        "@mutable decode path must carry the sequence bound check:\n{cpp}"
    );
}

#[test]
fn decode_rejects_over_bound_string_pl_cdr1_union() {
    // PL_CDR1 union member decode (emit_pl_cdr1_member_decode_case) routes
    // through the shared emit_value_read — proves it inherits the check for
    // free, same as the Rust "single shared emit path" design.
    let cpp = gen_cpp("union U switch (long) { case 1: string<8> s; };");
    assert!(
        cpp.contains("decoded string length exceeds its IDL bound (8)"),
        "PL_CDR1 union member decode must carry the bound check:\n{cpp}"
    );
}

#[test]
fn unbounded_decode_no_bound_check() {
    let cpp = gen_cpp("@final struct Free { string name; sequence<long> vals; };");
    assert!(
        !cpp.contains("exceeds its IDL bound"),
        "unbounded string/sequence must NOT get a decode-side bound check:\n{cpp}"
    );
}

/// T2 (typesystem oracle F2): a `@bit_bound(N)` enum narrows its XCDR2 wire
/// holder to a signed int8/int16 (N≤8 / N≤16); default stays int32_t
/// (XTypes 1.3 §7.4.5.1). Cyclone honours this — the prior fixed-int32 path did
/// not, breaking cross-vendor interop on `@bit_bound` enums.
#[test]
fn bit_bound_enum_narrows_cpp_wire_width() {
    let cpp = gen_cpp(
        "@bit_bound(8) enum Tiny { T_A, T_B };\n\
         @bit_bound(16) enum Mid { M_A, M_B };\n\
         enum Wide { W_A, W_B };\n\
         @final struct H { Tiny t; Mid m; Wide w; };",
    );
    assert!(
        cpp.contains("write_be<int8_t>(zd_out, static_cast<int8_t>")
            || cpp.contains("write_le_origin<int8_t>"),
        "@bit_bound(8) enum must encode as int8_t:\n{cpp}"
    );
    assert!(
        cpp.contains("read_le_origin<int8_t>"),
        "@bit_bound(8) enum must decode from int8_t:\n{cpp}"
    );
    assert!(
        cpp.contains("write_le_origin<int16_t>") || cpp.contains("write_be<int16_t>"),
        "@bit_bound(16) enum must encode as int16_t:\n{cpp}"
    );
    // The unannotated enum keeps the full 32-bit holder.
    assert!(
        cpp.contains("write_le_origin<int32_t>") || cpp.contains("write_be<int32_t>"),
        "default enum must stay int32_t:\n{cpp}"
    );
}
