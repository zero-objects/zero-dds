// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bounded-Sequence/-String-Bound-Enforcement (DDS-XTypes §7.4.3): the
//! generated `CdrEncode` must reject a `sequence<T, N>` / `string<N>` value
//! longer than its declared bound `N` (strict vendors like OpenDDS reject on
//! the wire). The Rust field type is an unbounded `Vec<T>` / `String`, so the
//! codegen enforces the bound at encode time — recursively (nested sequences),
//! over array elements, and for narrow strings (byte length).
//! See docs/interop/bounded-sequence-enforcement-followup.md.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};

fn gen_rust(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_rust_module(&ast, &RustGenOptions::default()).expect("gen")
}

#[test]
fn bounded_sequence_emits_encode_bound_check() {
    let out = gen_rust("@final struct Cap { sequence<octet, 4> data; };");
    assert!(
        out.contains("data.len() > 4"),
        "bounded sequence<octet, 4> must emit a len-bound check in encode:\n{out}"
    );
    assert!(
        out.contains("ValueOutOfRange"),
        "the bound violation must surface as EncodeError::ValueOutOfRange:\n{out}"
    );
}

#[test]
fn unbounded_sequence_has_no_bound_check() {
    let out = gen_rust("@final struct Free { sequence<octet> data; };");
    assert!(
        !out.contains("data.len() >"),
        "an UNbounded sequence must NOT get a bound check:\n{out}"
    );
}

#[test]
fn bound_value_is_the_declared_bound() {
    // A different bound must produce a different literal — the check is the
    // declared N, not a hard-coded constant.
    let out = gen_rust("@final struct Big { sequence<long, 8192> ids; };");
    assert!(
        out.contains("ids.len() > 8192"),
        "the emitted bound must equal the declared bound (8192):\n{out}"
    );
}

#[test]
fn nested_bounded_sequence_checks_both_levels() {
    // `sequence<sequence<octet, 4>, 8>` — both the outer bound (8) and the
    // inner element bound (4) must be enforced, the inner via a per-element loop.
    let out = gen_rust("@final struct Grid { sequence<sequence<octet, 4>, 8> rows; };");
    assert!(
        out.contains("rows.len() > 8"),
        "outer sequence bound (8) must be enforced:\n{out}"
    );
    assert!(
        out.contains(".iter()") && out.contains(".len() > 4"),
        "inner bounded sequence (4) must be enforced per element via a loop:\n{out}"
    );
}

#[test]
fn bounded_string_emits_byte_length_check() {
    // narrow `string<N>` — CDR encodes byte-wise, so the bound guards the
    // UTF-8 byte length.
    let out = gen_rust("@final struct Named { string<16> name; };");
    assert!(
        out.contains("name.len() > 16") && out.contains("bounded string length"),
        "bounded string<16> must emit a byte-length bound check:\n{out}"
    );
}

#[test]
fn unbounded_string_has_no_bound_check() {
    let out = gen_rust("@final struct FreeText { string note; };");
    assert!(
        !out.contains("note.len() >"),
        "an UNbounded string must NOT get a bound check:\n{out}"
    );
}

#[test]
fn array_of_bounded_sequence_checks_element_not_array_length() {
    // `sequence<octet, 4> rows[3]` — the field is `[Vec<u8>; 3]`. The bound (4)
    // applies to each inner Vec, NOT to the array length. The naive
    // `self.rows.len() > 4` would wrongly test the array length (3) against the
    // sequence bound — this verifies we loop over array elements instead.
    let out = gen_rust("@final struct Mat { sequence<octet, 4> rows[3]; };");
    assert!(
        !out.contains("self.rows.len() > 4"),
        "must NOT check the ARRAY length against the sequence bound:\n{out}"
    );
    assert!(
        out.contains("self.rows.iter()") && out.contains(".len() > 4"),
        "must loop over array elements and check each inner sequence:\n{out}"
    );
}

#[test]
fn bounded_wstring_emits_utf16_unit_check() {
    // wide `wstring<N>`: bound is in wide characters = UTF-16 code units.
    let out = gen_rust("@final struct WName { wstring<8> label; };");
    assert!(
        out.contains("label.as_str().encode_utf16().count() > 8")
            && out.contains("bounded wstring length"),
        "bounded wstring<8> must emit a UTF-16-unit bound check:\n{out}"
    );
}

#[test]
fn bounded_map_checks_size_and_values() {
    // `map<long, sequence<octet,4>, 16>` — the map bound (16) AND each value's
    // inner sequence bound (4) must be enforced (values via a `.values()` loop).
    let out = gen_rust("@final struct Grid { map<long, sequence<octet, 4>, 16> cells; };");
    assert!(
        out.contains("cells.len() > 16") && out.contains("bounded map length"),
        "map bound (16) must be enforced:\n{out}"
    );
    assert!(
        out.contains(".values()") && out.contains(".len() > 4"),
        "each bounded map value must be enforced via a loop:\n{out}"
    );
}

#[test]
fn union_arm_bounded_sequence_is_enforced() {
    // A bounded sequence as a union arm body must be enforced on encode of the
    // active member (`__v` is the matched arm value).
    let out = gen_rust(
        "union Pick switch(long) { case 1: sequence<octet, 4> data; case 2: long other; };",
    );
    assert!(
        out.contains("__v.len() > 4") && out.contains("ValueOutOfRange"),
        "a bounded-sequence union arm must emit a bound check on encode:\n{out}"
    );
}
