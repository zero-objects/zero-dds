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
