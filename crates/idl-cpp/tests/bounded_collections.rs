// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bounded-collection enforcement (DDS-XTypes §7.4.3) in the generated C++
//! XCDR2 encode: a `sequence<T, N>` / `string<N>` value longer than its declared
//! bound is rejected on encode (throws — strict vendors reject on the wire).
//! `<stdexcept>` is pulled in only when a bounded collection is present.

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
