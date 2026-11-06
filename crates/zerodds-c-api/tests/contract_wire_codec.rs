// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-c-api` with `zerodds-wire-codec`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire-codec` (`src/dyntype_ffi.rs`,
//! the C ABI of `dds::core::xtypes`): `ProbeRequest::new(type_name,
//! Encoding::PlainCdr2 or PlainCdr1, Endianness::Big or Little)` for every
//! type whatever its extensibility, `encode(&ast, &request,
//! &serde_json::Value)` for a JSON value, `decode::<serde_json::Value>(&ast,
//! &request, bytes)` back to JSON, and
//! `default_of_type::<serde_json::Value>(&ast, id, name)` for the default
//! value. The octets are the body of a sample without the encapsulation
//! header; any error becomes `ZERODDS_BAD_PARAMETER` or `ZERODDS_ERROR`.
//!
//! Oracle: DDS-XTypes 1.3 §7.4.1.1.1 Table 31, §7.4.1.1.2, §7.4.2 (an
//! encoding version 2 `@final` struct is PLAIN_CDR2 with 8-octet members
//! aligned to 4; an `@appendable` one is DELIMITED_CDR, a DHEADER first),
//! §7.4.3.2.2 Table 37 (MAXALIGN 8 for version 1), §7.4.3.1 Table 35
//! (version 1 has no DHEADER), §7.2.2.4.4.4.7 Table 9 (defaults: integers
//! and floats 0, string "").
#![allow(clippy::unwrap_used, clippy::panic)]

use serde_json::json;
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_codec::ProbeRequest;

const IDL: &str = "\
module m {
  @final struct S { long id; double v; string text; };
  @appendable struct A { long id; };
};";

fn model() -> zerodds_ast::Ast {
    let parsed = zerodds_idl_parse::parse(IDL);
    assert!(!parsed.has_errors());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "contract.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().unwrap();
    ast
}

/// `S { id: 1, v: 0.5, text: "x" }`, XCDR2 little-endian.
const S_XCDR2_LE: [u8; 18] = [
    0x01, 0x00, 0x00, 0x00, // id
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xe0, 0x3f, // v at 4
    0x02, 0x00, 0x00, 0x00, b'x', 0x00, // text
];

/// The same value, XCDR1 big-endian: `v` at 8.
const S_XCDR1_BE: [u8; 22] = [
    0x00, 0x00, 0x00, 0x01, // id
    0x00, 0x00, 0x00, 0x00, // padding
    0x3f, 0xe0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // v at 8
    0x00, 0x00, 0x00, 0x02, b'x', 0x00, // text
];

#[test]
fn xtypes_7_4_2_a_json_value_becomes_the_plain_cdr2_body_and_back() {
    let ast = model();
    let request = ProbeRequest::new("m::S", Encoding::PlainCdr2, Endianness::Little);
    let value = json!({ "id": 1, "v": 0.5, "text": "x" });
    assert_eq!(
        zerodds_wire_codec::encode(&ast, &request, &value).unwrap(),
        S_XCDR2_LE
    );
    let back: serde_json::Value = zerodds_wire_codec::decode(&ast, &request, &S_XCDR2_LE).unwrap();
    assert_eq!(back, value);
}

#[test]
fn xtypes_7_4_3_2_2_the_xcdr1_request_aligns_the_double_to_eight() {
    let ast = model();
    let request = ProbeRequest::new("m::S", Encoding::PlainCdr1, Endianness::Big);
    let value = json!({ "id": 1, "v": 0.5, "text": "x" });
    assert_eq!(
        zerodds_wire_codec::encode(&ast, &request, &value).unwrap(),
        S_XCDR1_BE
    );
    let back: serde_json::Value = zerodds_wire_codec::decode(&ast, &request, &S_XCDR1_BE).unwrap();
    assert_eq!(back, value);
}

#[test]
fn xtypes_7_4_2_plain_cdr2_in_the_request_still_delimits_an_appendable_type() {
    // `dyntype_ffi.rs` passes `PlainCdr2` for every type; the type's own
    // extensibility has to add the DHEADER.
    let ast = model();
    let value = json!({ "id": 7 });
    let request = ProbeRequest::new("m::A", Encoding::PlainCdr2, Endianness::Little);
    assert_eq!(
        zerodds_wire_codec::encode(&ast, &request, &value).unwrap(),
        [0x04, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00]
    );
    let request = ProbeRequest::new("m::A", Encoding::PlainCdr1, Endianness::Little);
    assert_eq!(
        zerodds_wire_codec::encode(&ast, &request, &value).unwrap(),
        [0x07, 0x00, 0x00, 0x00],
        "no DHEADER in version 1"
    );
}

#[test]
fn xtypes_7_2_2_4_4_4_7_the_default_value_is_table_9() {
    let ast = model();
    let id = ast
        .types()
        .find(|(_, node)| node.name == "S")
        .map(|(id, _)| id)
        .unwrap();
    let value: serde_json::Value = zerodds_wire_codec::default_of_type(&ast, id, "m::S").unwrap();
    assert_eq!(value["id"].as_i64(), Some(0));
    assert_eq!(value["v"].as_f64(), Some(0.0));
    assert_eq!(value["text"], "");
}

#[test]
fn a_value_that_does_not_fit_the_type_or_the_octets_is_an_error() {
    let ast = model();
    let request = ProbeRequest::new("m::S", Encoding::PlainCdr2, Endianness::Little);
    assert!(zerodds_wire_codec::encode(&ast, &request, &json!({ "id": 1 })).is_err());
    assert!(
        zerodds_wire_codec::decode::<serde_json::Value>(&ast, &request, &S_XCDR2_LE[..10]).is_err()
    );
}
