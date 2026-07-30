// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bounded-collection enforcement (DDS-XTypes §7.4.3) in the generated D
//! `marshalInto` (encode) / `unmarshalFrom*` (decode): a `sequence<T, N>` /
//! `string<N>` / `wstring<N>` / `map<K,V,N>` value longer than its declared
//! bound must be rejected on BOTH sides (strict vendors reject on the wire;
//! decode enforcement additionally closes an untrusted-input DoS vector — a
//! well-formed but oversized wire value previously decoded without
//! complaint). B1 follow-up (#22 decode-side parity): idl-d previously had
//! NO bound enforcement at all, on either side.
//!
//! `map_type`/`map_get` are the single choke point every representation
//! (`@final`/`@appendable`/`@mutable` struct members, union case arms) and
//! `emit_struct`/`emit_union` route through, so fixing it there covers all
//! of them for free (no separate per-representation test needed the way
//! idl-cpp's `@mutable` path did — this backend has one shared put/get pair
//! per field, reused verbatim for a `@mutable` framing and by union cases).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_d::{DGenOptions, generate_d_module};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_d_module(&ast, &DGenOptions::default()).expect("gen")
}

#[test]
fn bounded_string_encode_and_decode_checks() {
    let d = emit("@final struct Named { string<16> name; };");
    assert!(
        d.contains(".length > 16")
            && d.contains("bounded string length exceeds its IDL bound (16)"),
        "encode must throw on over-bound string<16>:\n{d}"
    );
    assert!(
        d.contains("decoded string length exceeds its IDL bound (16)"),
        "decode must throw on over-bound string<16>:\n{d}"
    );
}

#[test]
fn bounded_wstring_uses_utf16_code_unit_count() {
    let d = emit("@final struct Named { wstring<8> name; };");
    assert!(
        d.contains("codeLength!wchar")
            && d.contains("bounded wstring length exceeds its IDL bound (8)"),
        "encode must count UTF-16 code units, not D string.length:\n{d}"
    );
    assert!(
        d.contains("decoded wstring length exceeds its IDL bound (8)"),
        "decode must throw on over-bound wstring<8>:\n{d}"
    );
}

#[test]
fn bounded_octet_sequence_encode_and_decode_checks() {
    let d = emit("@final struct Cap { sequence<octet, 4> data; };");
    assert!(
        d.contains(".length > 4")
            && d.contains("bounded sequence length exceeds its IDL bound (4)"),
        "encode must throw on over-bound sequence<octet,4>:\n{d}"
    );
    assert!(
        d.contains("decoded sequence length exceeds its IDL bound (4)"),
        "decode must throw on over-bound sequence<octet,4>:\n{d}"
    );
}

#[test]
fn bounded_struct_sequence_encode_and_decode_checks() {
    // Non-octet element path (sequence<struct,N>) — a separate branch in
    // map_sequence/map_get_sequence from the octet fast path above.
    let d =
        emit("@final struct Pt { long x; long y; }; @final struct Cap { sequence<Pt, 3> pts; };");
    assert!(
        d.contains("bounded sequence length exceeds its IDL bound (3)"),
        "encode of sequence<Pt,3> must throw on over-bound:\n{d}"
    );
    assert!(
        d.contains("decoded sequence length exceeds its IDL bound (3)"),
        "decode of sequence<Pt,3> must throw on over-bound:\n{d}"
    );
}

#[test]
fn bounded_map_encode_and_decode_checks() {
    let d = emit("@final struct M { map<string, long, 2> vals; };");
    assert!(
        d.contains("bounded map length exceeds its IDL bound (2)"),
        "encode of map<string,long,2> must throw on over-bound:\n{d}"
    );
    assert!(
        d.contains("decoded map length exceeds its IDL bound (2)"),
        "decode of map<string,long,2> must throw on over-bound:\n{d}"
    );
}

#[test]
fn bounded_string_check_reaches_mutable_member() {
    // @mutable reuses the exact same FieldGen::put/get strings as
    // @final/@appendable (emit_struct builds them once per field), so the
    // check is emitted regardless of extensibility.
    let d = emit("@mutable struct Named { string<8> name; };");
    assert!(
        d.contains("bounded string length exceeds its IDL bound (8)")
            && d.contains("decoded string length exceeds its IDL bound (8)"),
        "@mutable struct must carry both encode and decode checks:\n{d}"
    );
}

#[test]
fn bounded_string_check_reaches_union_member() {
    // Union cases build their put/get via the same map_type/map_get calls
    // (emit_union), so they inherit the check for free.
    let d = emit("union U switch (long) { case 1: string<8> s; };");
    assert!(
        d.contains("bounded string length exceeds its IDL bound (8)")
            && d.contains("decoded string length exceeds its IDL bound (8)"),
        "union member must carry both encode and decode checks:\n{d}"
    );
}

#[test]
fn unbounded_no_checks_emitted() {
    let d = emit(
        "@final struct Free { string name; wstring wname; sequence<octet> data; map<string, long> vals; };",
    );
    assert!(
        !d.contains("exceeds its IDL bound"),
        "unbounded string/wstring/sequence/map must NOT get a bound check:\n{d}"
    );
}
