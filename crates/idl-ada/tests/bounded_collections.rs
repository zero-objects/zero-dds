// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bounded-collection enforcement (DDS-XTypes §7.4.3) in the generated Ada
//! `Marshal_Into` (encode) and `Read_<Name>`/`Unmarshal` (decode): a
//! `string<N>` / `wstring<N>` / `sequence<octet,N>` / `sequence<Struct,N>` /
//! `map<K,V,N>` value longer than its declared bound is rejected with
//! `Constraint_Error` on BOTH sides — before this fix idl-ada had NO bound
//! enforcement at all, neither side (B1 report). XTypes 1.3 §7.4.3 requires
//! the IDL bound enforced on encode AND decode.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_ada::{AdaGenOptions, generate_ada_module};

fn gen_ada(src: &str) -> zerodds_idl_ada::AdaModule {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_ada_module(&ast, &AdaGenOptions::default()).expect("gen")
}

#[test]
fn bounded_string_encode_and_decode_checks() {
    let m = gen_ada("@final struct Named { string<16> name; };");
    assert!(
        m.body.contains("Length (V.name) > 16")
            && m.body
                .contains("bounded string length exceeds its IDL bound (16)"),
        "encode of bounded string<16> must throw on over-bound encode:\n{}",
        m.body
    );
    assert!(
        m.body
            .contains("decoded string length exceeds its IDL bound (16)"),
        "decode of bounded string<16> must throw on over-bound decode:\n{}",
        m.body
    );
}

#[test]
fn bounded_wstring_encode_and_decode_checks() {
    let m = gen_ada("@final struct Named { wstring<16> name; };");
    assert!(
        m.body
            .contains("bounded wstring length exceeds its IDL bound (16)"),
        "encode of bounded wstring<16> must throw on over-bound encode:\n{}",
        m.body
    );
    assert!(
        m.body
            .contains("decoded wstring length exceeds its IDL bound (16)"),
        "decode of bounded wstring<16> must throw on over-bound decode:\n{}",
        m.body
    );
}

#[test]
fn bounded_octet_sequence_encode_and_decode_checks() {
    let m = gen_ada("@final struct Cap { sequence<octet, 4> data; };");
    assert!(
        m.body.contains("Length (V.data) > 4")
            && m.body
                .contains("bounded sequence length exceeds its IDL bound (4)"),
        "encode of bounded sequence<octet,4> must throw on over-bound encode:\n{}",
        m.body
    );
    assert!(
        m.body
            .contains("decoded sequence length exceeds its IDL bound (4)"),
        "decode of bounded sequence<octet,4> must throw on over-bound decode:\n{}",
        m.body
    );
}

#[test]
fn bounded_struct_sequence_encode_and_decode_checks() {
    let m = gen_ada(
        "@final struct Pt { long x; long y; }; @final struct Cap { sequence<Pt, 3> pts; };",
    );
    assert!(
        m.body.contains("Natural (V.pts.Length) > 3")
            && m.body
                .contains("bounded sequence length exceeds its IDL bound (3)"),
        "encode of bounded sequence<Pt,3> must throw on over-bound encode:\n{}",
        m.body
    );
    assert!(
        m.body.contains("Zn > 3")
            && m.body
                .contains("decoded sequence length exceeds its IDL bound (3)"),
        "decode of bounded sequence<Pt,3> must throw on over-bound decode:\n{}",
        m.body
    );
}

#[test]
fn bounded_map_encode_and_decode_checks() {
    let m = gen_ada("@final struct M { map<string, long, 2> vals; };");
    assert!(
        m.body
            .contains("bounded map length exceeds its IDL bound (2)"),
        "encode of bounded map<string,long,2> must throw on over-bound encode:\n{}",
        m.body
    );
    assert!(
        m.body.contains("Zn > 2")
            && m.body
                .contains("decoded map length exceeds its IDL bound (2)"),
        "decode of bounded map<string,long,2> must throw on over-bound decode:\n{}",
        m.body
    );
}

#[test]
fn unbounded_string_and_sequence_have_no_check() {
    let m = gen_ada("@final struct Free { string name; sequence<octet> data; };");
    assert!(
        !m.body.contains("exceeds its IDL bound"),
        "unbounded string/sequence must NOT get a bound check:\n{}",
        m.body
    );
}

#[test]
fn bounded_string_in_mutable_struct_checks_both_sides() {
    // @mutable routes members through the same Marshal_Into / Read_<Name>
    // emit_marshal body — one shared function, same as the csharp/java
    // "single shared decode path" design — so it inherits the check for
    // free. Prove it explicitly for the mutable representation.
    let m = gen_ada("@mutable struct Named { string<8> name; };");
    assert!(
        m.body
            .contains("bounded string length exceeds its IDL bound (8)"),
        "@mutable encode must carry the bound check:\n{}",
        m.body
    );
    assert!(
        m.body
            .contains("decoded string length exceeds its IDL bound (8)"),
        "@mutable decode must carry the bound check:\n{}",
        m.body
    );
}
