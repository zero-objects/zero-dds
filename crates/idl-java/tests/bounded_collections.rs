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
