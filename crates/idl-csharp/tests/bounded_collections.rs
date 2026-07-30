// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bounded-collection enforcement (DDS-XTypes §7.4.3) in the generated C#
//! TypeSupport encode: a `sequence<T, N>` / `string<N>` value longer than its
//! declared bound must be rejected on encode (strict vendors reject on the wire).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_csharp::{CsGenOptions, generate_csharp};

fn gen_cs(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_csharp(&ast, &CsGenOptions::default()).expect("gen")
}

#[test]
fn bounded_sequence_throws_on_over_bound() {
    let cs = gen_cs("@final struct Cap { sequence<octet, 4> data; };");
    assert!(
        cs.contains("__mat0.Count > 4") && cs.contains("bounded sequence length"),
        "bounded sequence<octet, 4> must throw on over-bound encode:\n{cs}"
    );
}

#[test]
fn unbounded_sequence_has_no_check() {
    let cs = gen_cs("@final struct Free { sequence<octet> data; };");
    assert!(
        !cs.contains("__mat.Count > "),
        "unbounded sequence must NOT get a bound check:\n{cs}"
    );
}

#[test]
fn bounded_string_byte_length_check() {
    let cs = gen_cs("@final struct Named { string<16> name; };");
    assert!(
        cs.contains("GetByteCount") && cs.contains("> 16") && cs.contains("bounded string length"),
        "bounded string<16> must throw on UTF-8 over-bound encode:\n{cs}"
    );
}

// B1 follow-up (#22 decode-side parity): idl-csharp had an encode-side
// bound-check emitter but no decode-side counterpart — a bounded field
// decoded a well-formed but oversized wire value without complaint. Every
// C# decode path (Final/Appendable/Mutable-PL_CDR1/Mutable-XCDR2, union
// members) funnels through the single shared `emit_decode_assign`, so the
// fix there covers all of them for free — proven by the mutable/union
// cases below.

#[test]
fn decode_rejects_over_bound_string_final() {
    let cs = gen_cs("@final struct Named { string<16> name; };");
    assert!(
        cs.contains("ReadString()")
            && cs.contains("GetByteCount")
            && cs.contains("decoded string length exceeds its IDL bound (16)"),
        "decode of bounded string<16> must throw on over-bound decode:\n{cs}"
    );
}

// B1 blocker fix (deep review of #22 decode-bounds-cross-backend): the narrow
// `string<N>` case above had a decode-side check, but `wstring<N>` did not —
// `emit_decode_wstring` never referenced `s.bound`, so a bounded wide string
// decode was silently unenforced even though its encode side rejects the
// same over-bound value. This is the exact gap the deep review flagged.
#[test]
fn decode_rejects_over_bound_wstring_final() {
    let cs = gen_cs("@final struct Named { wstring<16> name; };");
    assert!(
        cs.contains("ReadUInt32()")
            && cs.contains(".Length > 16")
            && cs.contains("decoded wstring length exceeds its IDL bound (16)"),
        "decode of bounded wstring<16> must throw on over-bound decode:\n{cs}"
    );
}

#[test]
fn decode_rejects_over_bound_sequence_final() {
    let cs = gen_cs("@final struct Cap { sequence<octet, 4> data; };");
    assert!(
        cs.contains("ReadSequenceLength()")
            && cs.contains("decoded sequence length exceeds its IDL bound (4)"),
        "decode of bounded sequence<octet,4> must throw on over-bound decode:\n{cs}"
    );
}

#[test]
fn decode_rejects_over_bound_map() {
    let cs = gen_cs("@final struct M { map<string, long, 2> vals; };");
    assert!(
        cs.contains("decoded map length exceeds its IDL bound (2)"),
        "decode of bounded map<string,long,2> must throw on over-bound decode:\n{cs}"
    );
}

#[test]
fn decode_rejects_over_bound_string_mutable_member() {
    let cs = gen_cs("@mutable struct Named { string<8> name; };");
    assert!(
        cs.matches("decoded string length exceeds its IDL bound (8)")
            .count()
            >= 2,
        "@mutable decode (PL_CDR1 loop AND XCDR2 EMHEADER loop, both routed \
         through emit_decode_member_assign) must both carry the check:\n{cs}"
    );
}

#[test]
fn decode_rejects_over_bound_string_union_member() {
    let cs = gen_cs("union U switch (long) { case 1: string<8> s; };");
    assert!(
        cs.contains("decoded string length exceeds its IDL bound (8)"),
        "union member decode (emit_union_decode_cases -> emit_decode_assign) \
         must carry the check:\n{cs}"
    );
}

#[test]
fn unbounded_decode_no_bound_check() {
    let cs = gen_cs("@final struct Free { string name; sequence<long> vals; };");
    assert!(
        !cs.contains("exceeds its IDL bound"),
        "unbounded string/sequence must NOT get a decode-side bound check:\n{cs}"
    );
}
