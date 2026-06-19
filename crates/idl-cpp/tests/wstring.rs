// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! idl-cpp XCDR2 `wstring` encode/decode (conformance §9.1: UTF-16 wire,
//! byte-identical to the Rust `WString` encoder). Previously the cpp codegen
//! skipped `wstring` ("not supported"); this verifies it now emits the
//! UTF-16 encode + decode + bound enforcement.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CppGenOptions, generate_cpp_header};

fn gen_cpp(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen")
}

#[test]
fn wstring_member_emits_utf16_encode_and_decode() {
    let cpp = gen_cpp("@final struct W { wstring label; };");
    assert!(
        cpp.contains("write_wstring_origin") || cpp.contains("write_wstring_be"),
        "wstring member must emit a UTF-16 encode:\n{cpp}"
    );
    assert!(
        cpp.contains("read_wstring_origin"),
        "wstring member must emit a UTF-16 decode:\n{cpp}"
    );
    // The wstring member must no longer be skipped as unsupported.
    assert!(
        !cpp.contains("member 'label' not supported"),
        "wstring member must NOT be skipped:\n{cpp}"
    );
}

#[test]
fn bounded_wstring_throws_on_over_bound() {
    let cpp = gen_cpp("@final struct W { wstring<8> label; };");
    assert!(
        cpp.contains(".size() > 8") && cpp.contains("bounded wstring length"),
        "bounded wstring<8> must throw on over-bound encode:\n{cpp}"
    );
    assert!(
        cpp.contains("#include <stdexcept>"),
        "a bounded collection must pull in <stdexcept>:\n{cpp}"
    );
}

#[test]
fn narrow_string_still_uses_utf8_path() {
    // Regression: narrow string must keep its UTF-8 write_string path.
    let cpp = gen_cpp("@final struct S { string name; };");
    assert!(
        cpp.contains("write_string_origin") || cpp.contains("write_string_be"),
        "narrow string must still use the UTF-8 string path:\n{cpp}"
    );
    assert!(
        !cpp.contains("write_wstring"),
        "a narrow-string-only struct must not emit wstring helpers:\n{cpp}"
    );
}
