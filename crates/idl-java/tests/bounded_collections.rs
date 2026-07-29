// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bounded-collection enforcement (DDS-XTypes §7.4.3) in the generated Java
//! TypeSupport encode: a `sequence<T, N>` / `string<N>` value longer than its
//! declared bound must be rejected on encode (strict vendors reject on the wire).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_java::{JavaGenOptions, generate_java_files};

fn all_source(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_java_files(&ast, &JavaGenOptions::default())
        .expect("gen")
        .iter()
        .map(|f| f.source.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn bounded_sequence_throws_on_over_bound() {
    let src = all_source("@final struct Cap { sequence<octet, 4> data; };");
    // Temp identifiers are depth-suffixed (`__seq0`) so nested sequences do not
    // collide; the over-bound check is still emitted against the member's list.
    assert!(
        src.contains(".size() > 4") && src.contains("bounded sequence length"),
        "bounded sequence<octet, 4> must throw on over-bound encode:\n{src}"
    );
}

#[test]
fn unbounded_sequence_has_no_check() {
    let src = all_source("@final struct Free { sequence<octet> data; };");
    assert!(
        !src.contains(".size() > "),
        "unbounded sequence must NOT get a bound check:\n{src}"
    );
}

#[test]
fn bounded_string_byte_length_check() {
    let src = all_source("@final struct Named { string<16> name; };");
    assert!(
        src.contains("getBytes") && src.contains("> 16") && src.contains("bounded string length"),
        "bounded string<16> must throw on UTF-8 over-bound encode:\n{src}"
    );
}

// B1 follow-up (#22 decode-side parity): idl-java had an encode-side
// bound-check emitter but no decode-side counterpart — a bounded field
// decoded a well-formed but oversized wire value without complaint. Every
// Java decode path (inline member, @mutable branch, @optional value, array
// element via emit_array_fill) funnels through the single shared
// `emit_read_into`, so the fix there covers all of them for free.

#[test]
fn decode_rejects_over_bound_string_final() {
    let src = all_source("@final struct Named { string<16> name; };");
    assert!(
        src.contains("getBytes(java.nio.charset.StandardCharsets.UTF_8).length > 16")
            && src.contains("decoded string length exceeds its IDL bound (16)"),
        "decode of bounded string<16> must throw on over-bound decode:\n{src}"
    );
}

// B1 blocker fix (deep review of #22 decode-bounds-cross-backend): the narrow
// `string<N>` case above had a decode-side check, but the `s.wide` match arm
// in `emit_read_into` had no check at all — a bounded `wstring<N>` decode was
// silently unenforced even though its encode side rejects the same
// over-bound value. This is the exact gap the deep review flagged.
#[test]
fn decode_rejects_over_bound_wstring_final() {
    let src = all_source("@final struct Named { wstring<16> name; };");
    assert!(
        src.contains("readUInt32()")
            && src.contains("__wn0 > 16")
            && src.contains("decoded wstring length exceeds its IDL bound (16)"),
        "decode of bounded wstring<16> must throw on over-bound decode:\n{src}"
    );
}

#[test]
fn decode_rejects_over_bound_sequence_final() {
    let src = all_source("@final struct Cap { sequence<octet, 4> data; };");
    assert!(
        src.contains("readSequenceCount()")
            && src.contains("decoded sequence length exceeds its IDL bound (4)"),
        "decode of bounded sequence<octet,4> must throw on over-bound decode:\n{src}"
    );
}

#[test]
fn decode_rejects_over_bound_map() {
    let src = all_source("@final struct M { map<string, long, 2> vals; };");
    assert!(
        src.contains("decoded map length exceeds its IDL bound (2)"),
        "decode of bounded map<string,long,2> must throw on over-bound decode:\n{src}"
    );
}

#[test]
fn decode_rejects_over_bound_string_mutable_member() {
    let src = all_source("@mutable struct Named { string<8> name; };");
    assert!(
        src.contains("decoded string length exceeds its IDL bound (8)"),
        "@mutable member decode (emit_member_decode_mutable_branch -> \
         emit_declarator_decode -> emit_read_into) must carry the check:\n{src}"
    );
}

#[test]
fn decode_rejects_over_bound_string_optional_member() {
    let src = all_source("@final struct Named { @optional string<8> name; };");
    assert!(
        src.contains("decoded string length exceeds its IDL bound (8)"),
        "@optional member decode (emit_optional_value_decode -> emit_read_into) \
         must carry the check:\n{src}"
    );
}

#[test]
fn decode_rejects_over_bound_string_array_element() {
    // Java's array decode (emit_array_fill) calls emit_read_into at the leaf
    // for every dimension — unlike idl-rust, there is no separate
    // manual-array-decode path, so array-of-bounded-element is covered for
    // free.
    let src = all_source("@final struct A { string<4> names[3]; };");
    assert!(
        src.contains("decoded string length exceeds its IDL bound (4)"),
        "array-of-bounded-string element decode must carry the check:\n{src}"
    );
}

#[test]
fn unbounded_decode_no_bound_check() {
    let src = all_source("@final struct Free { string name; sequence<long> vals; };");
    assert!(
        !src.contains("exceeds its IDL bound"),
        "unbounded string/sequence must NOT get a decode-side bound check:\n{src}"
    );
}
