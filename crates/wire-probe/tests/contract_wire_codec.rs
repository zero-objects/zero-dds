// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-wire-probe` with `zerodds-wire-codec`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire-codec` (`src/encode.rs`,
//! `src/decode.rs`, `src/canonical.rs`, `src/contract.rs`, `src/default.rs`,
//! `src/value.rs`, `src/lib.rs`, and `ProbeError` in the `dump_rust` and
//! `fill_rust` emitters): it is the codec pinned to `serde_json::Value`
//! (the `serde-json` feature, on by default). It forwards `encode::encode`,
//! `encode_recording`, `encode_with_padding`, `decode::decode`,
//! `canonical::{canonicalise, text, difference}`, the `contract::*` value
//! rules and `default::{member, of_type}`, and re-exports `ProbeError`,
//! `value::*` (`ProbeRequest`, `Emit`), `canonical::Difference`,
//! `source::Fields`, `contract::Consumed` and `json::EXACT_JSON_INTEGER`.
//!
//! Oracles: the value format of `crates/wire-probe/src/lib.rs` and
//! `tests/interop-corpus/README.md` (enum by name, `char` as a
//! one-character string, sequence as array, union as `_d` and the selected
//! member, a 64-bit integer as a string, an absent `@optional` member left
//! out and `null` refused); IEEE 754 binary64 (integers exact up to 2^53);
//! DDS-XTypes 1.3 §7.4.3.5.2 rule (20) (the `is_present` octet of an
//! `@optional` member of a final type) with the octets Cyclone DDS, RTI
//! Connext and Fast DDS wrote for `opt_final_absent` / `opt_final_present`
//! (`crates/ast-layout/tests/fixtures/vendor_bytes.md`); §7.4.3.5.3 (XCDR2:
//! MAXALIGN 4, a sequence of a primitive is its length and the elements, a
//! final union its discriminator and the selected member); §7.2.2.4.4.4.7
//! Table 9 (defaults).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use core::any::TypeId;

use serde_json::{Value, json};
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::{Id, ScopedName};
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_codec::value::ProbeRequest;
use zerodds_wire_probe::encode::model_from_idl;

fn type_id(ast: &Ast, name: &str) -> Id<TypeNode> {
    ast.find_by_scoped_name(&ScopedName::absolute(vec![name.to_string()]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .expect("the type is in the model")
}

fn xcdr2(name: &str) -> ProbeRequest {
    ProbeRequest::new(name, Encoding::PlainCdr2, Endianness::Little)
}

#[test]
fn the_reexports_are_the_codecs_own() {
    assert_eq!(
        TypeId::of::<zerodds_wire_probe::ProbeError>(),
        TypeId::of::<zerodds_wire_codec::ProbeError>()
    );
    assert_eq!(
        TypeId::of::<zerodds_wire_probe::value::ProbeRequest>(),
        TypeId::of::<ProbeRequest>()
    );
    assert_eq!(
        TypeId::of::<zerodds_wire_probe::Difference>(),
        TypeId::of::<zerodds_wire_codec::Difference>()
    );
    assert_eq!(
        TypeId::of::<zerodds_wire_probe::contract::Fields<'static, Value>>(),
        TypeId::of::<zerodds_wire_codec::Fields<'static, Value>>()
    );
    // IEEE 754 binary64 carries every integer up to 2^53 exactly.
    assert_eq!(
        zerodds_wire_probe::EXACT_JSON_INTEGER,
        9_007_199_254_740_992
    );
    assert_eq!(
        zerodds_wire_probe::EXACT_JSON_INTEGER,
        zerodds_wire_codec::EXACT_JSON_INTEGER
    );
}

#[test]
fn xtypes_7_4_3_5_3_each_json_form_of_the_value_file_is_its_xcdr2_value() {
    let idl = "enum Colour { RED, GREEN };\n\
               @final union Pick switch (long) { case 1: long a; case 2: string b; };\n\
               @final struct V { boolean f; Colour c; char ch; sequence<short> q; \
               long long w; Pick u; };";
    let ast = model_from_idl(idl, "forms.idl").unwrap();
    let value = json!({
        "f": true,
        "c": "GREEN",
        "ch": "x",
        "q": [1, -1],
        "w": "-3",
        "u": { "_d": 2, "b": "y" }
    });
    #[rustfmt::skip]
    let expected = [
        0x01, 0x00, 0x00, 0x00,                         // boolean, padding
        0x01, 0x00, 0x00, 0x00,                         // GREEN, a 32-bit enumerator
        b'x', 0x00, 0x00, 0x00,                         // char, padding
        0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0xff, 0xff, // sequence<short>: 2, 1, -1
        0xfd, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, // long long at four (MAXALIGN 4)
        0x02, 0x00, 0x00, 0x00,                         // discriminator 2
        0x02, 0x00, 0x00, 0x00, b'y', 0x00,             // string "y"
    ];
    let request = xcdr2("V");
    let bytes = zerodds_wire_codec::encode::encode(&ast, &request, &value).unwrap();
    assert_eq!(bytes, expected);
    let read: Value = zerodds_wire_codec::decode::decode(&ast, &request, &bytes).unwrap();
    assert_eq!(
        zerodds_wire_codec::canonical::difference(&value, &read, "V"),
        None
    );
    // The canonical text sorts the keys and writes the 64-bit integer as
    // a string.
    assert_eq!(
        zerodds_wire_codec::canonical::text(&read),
        r#"{"c":"GREEN","ch":"x","f":true,"q":[1,-1],"u":{"_d":2,"b":"y"},"w":"-3"}"#
    );
}

#[test]
fn xtypes_7_4_3_5_2_an_absent_optional_member_is_a_key_left_out() {
    let idl = "@final struct Sample { long a; @optional octet b; long c; };";
    let ast = model_from_idl(idl, "optional.idl").unwrap();
    let request = xcdr2("Sample");
    // The three vendors agree on these octets for the absent member.
    let absent = [
        0x79, 0x29, 0xed, 0xff, 0x00, 0x00, 0x00, 0x00, 0xfe, 0xff, 0xff, 0xff,
    ];
    let bytes =
        zerodds_wire_codec::encode::encode(&ast, &request, &json!({"a": -1_234_567, "c": -2}))
            .unwrap();
    assert_eq!(bytes, absent);
    let read: Value = zerodds_wire_codec::decode::decode(&ast, &request, &absent).unwrap();
    assert_eq!(read, json!({"a": -1_234_567, "c": -2}));
    // Present: `01`, the octet; the two octets before `c` are padding
    // whose content no clause fixes (Cyclone 0000, RTI 0400, Fast DDS 0100).
    for vendor in [[0x00, 0x00], [0x04, 0x00], [0x01, 0x00]] {
        let mut present = vec![0x79, 0x29, 0xed, 0xff, 0x01, 0xab];
        present.extend_from_slice(&vendor);
        present.extend_from_slice(&[0xfe, 0xff, 0xff, 0xff]);
        let read: Value = zerodds_wire_codec::decode::decode(&ast, &request, &present).unwrap();
        assert_eq!(read, json!({"a": -1_234_567, "b": 171, "c": -2}));
    }
    // `null` is not absence.
    assert!(
        zerodds_wire_codec::encode::encode(
            &ast,
            &request,
            &json!({"a": -1_234_567, "b": null, "c": -2})
        )
        .is_err()
    );
}

#[test]
fn a_64_bit_integer_past_2_53_is_refused_as_a_json_number() {
    let idl = "@final struct W { long long x; unsigned long long y; };";
    let ast = model_from_idl(idl, "wide.idl").unwrap();
    let request = xcdr2("W");
    let as_strings = json!({"x": "-3", "y": "18446744073709551615"});
    #[rustfmt::skip]
    let expected = [
        0xfd, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    ];
    let bytes = zerodds_wire_codec::encode::encode(&ast, &request, &as_strings).unwrap();
    assert_eq!(bytes, expected);
    let read: Value = zerodds_wire_codec::decode::decode(&ast, &request, &bytes).unwrap();
    assert_eq!(read, as_strings, "the way out is always a string");
    // 2^53 + 1 does not survive a double.
    assert!(
        zerodds_wire_codec::encode::encode(
            &ast,
            &request,
            &json!({"x": 9_007_199_254_740_993_i64, "y": "0"})
        )
        .is_err()
    );
}

#[test]
fn xtypes_7_2_2_4_4_4_7_the_default_value_in_its_json_form() {
    let idl = "@final struct D { long a; long long b; string s; boolean f; sequence<long> q; };";
    let ast = model_from_idl(idl, "default.idl").unwrap();
    let value: Value = zerodds_wire_codec::default::of_type(&ast, type_id(&ast, "D"), "D").unwrap();
    assert_eq!(
        value,
        json!({"a": 0, "b": "0", "s": "", "f": false, "q": []})
    );
}

#[test]
fn the_first_difference_names_its_path() {
    let found = zerodds_wire_codec::canonical::difference(
        &json!({"a": 1, "q": [1, 2]}),
        &json!({"a": 1, "q": [1, 3]}),
        "S",
    )
    .expect("the values differ");
    assert_eq!(found.path, "S.q[1]");
    assert_eq!(found.expected.as_deref(), Some("2"));
    assert_eq!(found.found.as_deref(), Some("3"));
    let missing =
        zerodds_wire_codec::canonical::difference(&json!({"a": 1, "b": 2}), &json!({"a": 1}), "S")
            .expect("a key is missing");
    assert_eq!(missing.path, "S.b");
    assert_eq!(missing.found, None);
}

#[test]
fn the_value_rules_refuse_the_wrong_json_kind() {
    use zerodds_wire_codec::contract;
    assert_eq!(contract::integer(&json!(5), "p").unwrap(), 5);
    assert_eq!(contract::integer(&json!("-7"), "p").unwrap(), -7);
    assert!(contract::integer(&json!("x"), "p").is_err());
    assert!(contract::text(&json!(1), "p").is_err());
    assert!(contract::boolean(&json!(0), "p").is_err());
    assert!(contract::reject_null(&json!(null), "p").is_err());
    assert_eq!(contract::character(&json!("x"), "p").unwrap(), 'x');
    assert!(contract::character(&json!("xy"), "p").is_err());
}
