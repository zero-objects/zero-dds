// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bounded-collection enforcement (DDS-XTypes §7.4.3) in the generated Julia
//! encode/decode: a `sequence<T, N>` / `string<N>` / `wstring<N>` /
//! `map<K,V,N>` value longer than its declared bound must be rejected on
//! BOTH sides (strict vendors reject on the wire; decode must not accept an
//! oversized well-formed payload either — untrusted-input DoS vector).
//!
//! B1 follow-up: idl-julia previously had NO enforcement on either side.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_julia::{JuliaGenOptions, generate_julia_module};

fn gen_julia(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_julia_module(&ast, &JuliaGenOptions::default()).expect("gen")
}

#[test]
fn encode_rejects_over_bound_string() {
    let j = gen_julia("@final struct Named { string<16> name; };");
    assert!(
        j.contains("sizeof(v.name) > 16")
            && j.contains("bounded string length exceeds its IDL bound"),
        "bounded string<16> must throw on over-bound encode:\n{j}"
    );
}

#[test]
fn encode_rejects_over_bound_wstring() {
    let j = gen_julia("@final struct Named { wstring<16> name; };");
    assert!(
        j.contains("bounded wstring length exceeds its IDL bound"),
        "bounded wstring<16> must throw on over-bound encode:\n{j}"
    );
}

#[test]
fn encode_rejects_over_bound_sequence() {
    let j = gen_julia("@final struct Cap { sequence<octet, 4> data; };");
    assert!(
        j.contains("length(v.data) > 4")
            && j.contains("bounded sequence length exceeds its IDL bound"),
        "bounded sequence<octet,4> must throw on over-bound encode:\n{j}"
    );
}

#[test]
fn encode_rejects_over_bound_map() {
    let j = gen_julia("@final struct M { map<long, long, 2> vals; };");
    assert!(
        j.contains("bounded map length exceeds its IDL bound"),
        "bounded map<long,long,2> must throw on over-bound encode:\n{j}"
    );
}

#[test]
fn encode_unbounded_no_check() {
    let j = gen_julia("@final struct Free { string name; sequence<octet> data; };");
    assert!(
        !j.contains("exceeds its IDL bound"),
        "unbounded string/sequence must NOT get a bound check:\n{j}"
    );
}

#[test]
fn decode_rejects_over_bound_string() {
    let j = gen_julia("@final struct Named { string<16> name; };");
    assert!(
        j.contains("decoded string length exceeds its IDL bound (16)"),
        "decode of bounded string<16> must throw on over-bound decode:\n{j}"
    );
}

#[test]
fn decode_rejects_over_bound_wstring() {
    let j = gen_julia("@final struct Named { wstring<16> name; };");
    assert!(
        j.contains("decoded wstring length exceeds its IDL bound (16)"),
        "decode of bounded wstring<16> must throw on over-bound decode:\n{j}"
    );
}

#[test]
fn decode_rejects_over_bound_sequence_octet() {
    let j = gen_julia("@final struct Cap { sequence<octet, 4> data; };");
    assert!(
        j.contains("decoded sequence length exceeds its IDL bound (4)"),
        "decode of bounded sequence<octet,4> must throw on over-bound decode:\n{j}"
    );
}

#[test]
fn decode_rejects_over_bound_sequence_of_struct() {
    let j = gen_julia(
        "@final struct Pt { long x; long y; }; @final struct Cap { sequence<Pt, 3> pts; };",
    );
    assert!(
        j.contains("decoded sequence length exceeds its IDL bound (3)"),
        "decode of bounded sequence<Pt,3> must throw on over-bound decode:\n{j}"
    );
    // Moderate fix (deep review of #22 decode-bounds-cross-backend): the
    // bound check must run BEFORE the `Vector{Pt}(undef, zdN)` allocation and
    // the per-element decode loop, not after — otherwise an attacker-supplied
    // huge wire count `zdN` drives an oversized allocation/decode loop before
    // the bound is ever checked, defeating the point of the bound.
    let check_pos = j
        .find("decoded sequence length exceeds its IDL bound (3)")
        .expect("check present");
    let alloc_pos = j
        .find("Vector{Pt}(undef, zdN)")
        .expect("allocation present");
    assert!(
        check_pos < alloc_pos,
        "bound check must run BEFORE the Vector allocation/decode loop, not after:\n{j}"
    );
}

#[test]
fn decode_rejects_over_bound_map() {
    let j = gen_julia("@final struct M { map<long, long, 2> vals; };");
    assert!(
        j.contains("decoded map length exceeds its IDL bound (2)"),
        "decode of bounded map<long,long,2> must throw on over-bound decode:\n{j}"
    );
}

#[test]
fn decode_rejects_over_bound_string_in_union() {
    // Union cases route through the same map_type/map_get pair as struct
    // members (verified by reading the emitter), so the check must appear
    // for a union case too.
    let j = gen_julia("union U switch (long) { case 1: string<8> s; };");
    assert!(
        j.contains("decoded string length exceeds its IDL bound (8)"),
        "union case decode must carry the bound check:\n{j}"
    );
}

#[test]
fn decode_unbounded_no_check() {
    let j = gen_julia("@final struct Free { string name; sequence<octet> data; };");
    assert!(
        !j.contains("exceeds its IDL bound"),
        "unbounded string/sequence must NOT get a decode-side bound check:\n{j}"
    );
}
