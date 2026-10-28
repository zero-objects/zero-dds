// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The bytes back into the value — at `serde_json::Value`.
//!
//! The walk is `zerodds_wire_codec::decode`'s and follows the same
//! layout plan the encoder does. What is here is the value file's end of
//! it: the entry point pinned to the representation the probe writes,
//! and the shorthand that parses the IDL first.
//!
//! [`decode`] carries a recursion marker for the reason `contract.rs`'s
//! module note gives: `dds_bounded_recursion` reads a path call by its
//! last segment, so forwarding to the function of the same name in the
//! other crate looks like recursion. Depth 1.

use serde_json::Value;
use zerodds_ast::Ast;
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_codec::encode::ProbeError;
use zerodds_wire_codec::value::ProbeRequest;

/// Read `bytes` as `request.type_name`, in canonical form. See
/// [`zerodds_wire_codec::decode::decode`].
///
/// # Errors
/// [`ProbeError`], naming the path in the value or the offset in the
/// stream.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::decode::decode`, and the lint reads a
/// path call by its last segment. See the module note.
#[inline]
pub fn decode(ast: &Ast, request: &ProbeRequest, bytes: &[u8]) -> Result<Value, ProbeError> {
    zerodds_wire_codec::decode::decode(ast, request, bytes)
}

/// Shorthand for the common case: one IDL source, one type, one payload.
///
/// # Errors
/// As [`decode`], plus the frontend's own diagnostics.
pub fn unprobe(
    idl: &str,
    type_name: &str,
    bytes: &[u8],
    encoding: Encoding,
    endianness: Endianness,
) -> Result<Value, ProbeError> {
    let ast = crate::encode::model_from_idl(idl, "probe.idl")?;
    decode(
        &ast,
        &ProbeRequest::new(type_name, encoding, endianness),
        bytes,
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::canonical::text;
    use crate::encode::{encode, model_from_idl};
    use crate::value::MapLengthPolicy;
    use serde_json::json;
    use zerodds_ast::TypeNode;
    use zerodds_ast_core::{Id, ScopedName};

    fn find_type(ast: &Ast, name: &str) -> Id<TypeNode> {
        let segments: Vec<String> = name
            .split("::")
            .filter(|segment| !segment.is_empty())
            .map(std::string::ToString::to_string)
            .collect();
        ast.find_by_scoped_name(&ScopedName::absolute(segments))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type)
            .expect("the type is in the model")
    }

    /// One value out, the bytes, and the value back — through this
    /// project alone, which is what the vendor pairs are then compared
    /// against.
    fn round_trip(idl: &str, type_name: &str, value: &Value, encoding: Encoding) -> String {
        let ast = model_from_idl(idl, "decode.idl").expect("the fixture parses");
        for endianness in [Endianness::Little, Endianness::Big] {
            let request = ProbeRequest::new(type_name, encoding, endianness);
            let bytes = encode(&ast, &request, value).expect("the sample serialises");
            let back = decode(&ast, &request, &bytes).expect("the bytes read back");
            let there =
                crate::canonical::canonicalise(&ast, find_type(&ast, type_name), value, type_name)
                    .expect("the sample is well formed");
            assert_eq!(
                text(&there),
                text(&back),
                "{type_name} under {encoding:?}/{endianness:?}"
            );
        }
        let ast = model_from_idl(idl, "decode.idl").expect("the fixture parses");
        let request = ProbeRequest::new(type_name, encoding, Endianness::Little);
        let bytes = encode(&ast, &request, value).expect("the sample serialises");
        text(&decode(&ast, &request, &bytes).expect("the bytes read back"))
    }

    #[test]
    fn every_primitive_comes_back_the_value_that_went_in() {
        let idl = "@final struct S {
            boolean flag; octet raw; int8 tiny; char letter; wchar wide;
            short si; unsigned short us; long l; unsigned long ul;
            long long ll; unsigned long long ull; float f; double d;
        };";
        let value = json!({
            "flag": true, "raw": 171, "tiny": -13, "letter": "Z", "wide": "\u{e4}",
            "si": -12345, "us": 48879, "l": -1234567, "ul": 3735928559u32,
            "ll": "-987654321987", "ull": "16045690984833335998",
            "f": -3.75, "d": -1.2345678901234567
        });
        for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
            let back = round_trip(idl, "S", &value, encoding);
            assert!(back.contains(r#""ull":"16045690984833335998""#), "{back}");
            assert!(back.contains("\"wide\":\"\u{e4}\""), "{back}");
        }
    }

    #[test]
    fn a_mutable_structure_round_trips_under_both_encodings() {
        let idl = "@mutable struct S { long a; double b; short c; };";
        let value = json!({ "a": -1234567, "b": -1.2345678901234567, "c": -2 });
        for encoding in [Encoding::PlCdr1, Encoding::PlCdr2] {
            round_trip(idl, "S", &value, encoding);
        }
    }

    #[test]
    fn an_absent_optional_stays_absent_and_a_present_one_comes_back() {
        let idl =
            "@mutable struct S { @optional long first; long middle; @optional string text; };";
        let absent = round_trip(idl, "S", &json!({ "middle": 7 }), Encoding::PlCdr2);
        assert_eq!(absent, r#"{"middle":7}"#);
        let present = round_trip(
            idl,
            "S",
            &json!({ "first": -1, "middle": 7, "text": "\u{fc}ber" }),
            Encoding::PlCdr2,
        );
        assert_eq!(
            present,
            "{\"first\":-1,\"middle\":7,\"text\":\"\u{fc}ber\"}"
        );
    }

    #[test]
    fn an_optional_in_a_final_type_is_told_apart_from_the_member_after_it() {
        // Nothing marks the gap, so the reader has to recognise the
        // EMHEADER that is not there. Both samples, so a reader that
        // always guessed "absent" fails the second.
        let idl = "@final struct S { long a; @optional long b; long c; };";
        assert_eq!(
            round_trip(
                idl,
                "S",
                &json!({ "a": -1234567, "c": -2 }),
                Encoding::PlainCdr2
            ),
            r#"{"a":-1234567,"c":-2}"#
        );
        assert_eq!(
            round_trip(
                idl,
                "S",
                &json!({ "a": -1234567, "b": 9, "c": -2 }),
                Encoding::PlainCdr2
            ),
            r#"{"a":-1234567,"b":9,"c":-2}"#
        );
    }

    #[test]
    fn a_union_comes_back_with_the_member_the_discriminator_selects() {
        let idl = "union Choice switch (long) {
            case 1: case 2: case 3: long whole;
            case 10: case 20: double fraction;
            default: short small;
        };
        @final struct S { Choice picked; long tail; };";
        assert_eq!(
            round_trip(
                idl,
                "S",
                &json!({ "picked": { "_d": 20, "fraction": -2.5 }, "tail": -1 }),
                Encoding::PlainCdr2
            ),
            r#"{"picked":{"_d":20,"fraction":-2.5},"tail":-1}"#
        );
        assert_eq!(
            round_trip(
                idl,
                "S",
                &json!({ "picked": { "_d": 99, "small": 3 }, "tail": -1 }),
                Encoding::PlainCdr2
            ),
            r#"{"picked":{"_d":99,"small":3},"tail":-1}"#
        );
    }

    #[test]
    fn a_union_with_no_default_and_no_matching_label_carries_no_member() {
        let idl = "union Choice switch (long) { case 1: long whole; case 2: double fraction; };
                   @final struct S { Choice picked; long tail; };";
        assert_eq!(
            round_trip(
                idl,
                "S",
                &json!({ "picked": { "_d": 99 }, "tail": -1 }),
                Encoding::PlainCdr2
            ),
            r#"{"picked":{"_d":99},"tail":-1}"#
        );
    }

    #[test]
    fn an_inherited_member_comes_back_under_its_own_name() {
        let idl = "@final struct Base { long base_a; short base_b; };
                   @final struct S : Base { double derived_a; octet derived_b; };";
        assert_eq!(
            round_trip(
                idl,
                "S",
                &json!({
                    "base_a": -1234567, "base_b": -2,
                    "derived_a": -1.2345678901234567, "derived_b": 171
                }),
                Encoding::PlainCdr2
            ),
            r#"{"base_a":-1234567,"base_b":-2,"derived_a":-1.2345678901234567,"derived_b":171}"#
        );
    }

    #[test]
    fn a_truncated_payload_is_an_error_and_not_a_shorter_sample() {
        let ast = model_from_idl("@final struct S { long a; long b; };", "decode.idl")
            .expect("the fixture parses");
        let request = ProbeRequest::new("S", Encoding::PlainCdr2, Endianness::Little);
        let bytes = encode(&ast, &request, &json!({ "a": 1, "b": 2 })).expect("it serialises");
        let refused = decode(&ast, &request, &bytes[..4]);
        assert!(refused.is_err(), "four bytes are not two longs");
    }

    #[test]
    fn bytes_left_over_are_an_error_and_not_ignored() {
        let ast = model_from_idl("@final struct S { long a; };", "decode.idl")
            .expect("the fixture parses");
        let request = ProbeRequest::new("S", Encoding::PlainCdr2, Endianness::Little);
        let mut bytes = encode(&ast, &request, &json!({ "a": 1 })).expect("it serialises");
        bytes.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        let refused = decode(&ast, &request, &bytes);
        assert!(refused.is_err(), "a tail this long is not padding");
    }

    #[test]
    fn a_sequence_and_an_array_of_aggregates_round_trip_with_their_delimiters() {
        let idl = "@final struct Cell { long a; long b; };
                   @appendable struct S { sequence<Cell> items; Cell grid[2]; long tail; };";
        round_trip(
            idl,
            "S",
            &json!({
                "items": [{ "a": 1, "b": 2 }, { "a": 3, "b": 4 }],
                "grid": [{ "a": 5, "b": 6 }, { "a": 7, "b": 8 }],
                "tail": -1
            }),
            Encoding::DelimitedCdr,
        );
    }

    #[test]
    fn an_enumeration_at_a_narrow_bound_comes_back_by_name() {
        let idl = "@bit_bound(8) enum Small { @value(3) RED, @value(9) GREEN, @value(200) BLUE };
                   @final struct S { Small first; Small second; octet tail; };";
        assert_eq!(
            round_trip(
                idl,
                "S",
                &json!({ "first": "BLUE", "second": "GREEN", "tail": 171 }),
                Encoding::PlainCdr2
            ),
            r#"{"first":"BLUE","second":"GREEN","tail":171}"#
        );
    }

    #[test]
    fn a_fixed_comes_back_as_the_decimal_that_went_in() {
        let idl = "@final struct S { fixed<7, 3> amount; fixed<5, 0> whole; };";
        assert_eq!(
            round_trip(
                idl,
                "S",
                &json!({ "amount": "-12.25", "whole": "42" }),
                Encoding::PlainCdr2
            ),
            r#"{"amount":"-12.250","whole":"42"}"#
        );
    }

    #[test]
    fn a_bitmask_and_a_bitset_come_back_by_name() {
        let idl = "@bit_bound(16) bitmask Flags { A, @position(5) B, @position(15) C };
                   struct Packed { @bit_bound(8) long field { unsigned short lo : 3; short : 2; unsigned short hi : 3; }; };";
        let idl = idl.split("struct Packed").next().unwrap_or(idl);
        let idl = format!("{idl}@final struct S {{ Flags marks; }};");
        assert_eq!(
            round_trip(
                &idl,
                "S",
                &json!({ "marks": ["C", "A"] }),
                Encoding::PlainCdr2
            ),
            r#"{"marks":["A","C"]}"#
        );
    }

    #[test]
    fn a_map_round_trips_under_both_length_policies() {
        let ast = model_from_idl(
            "@final struct S { map<long, double> table; };",
            "decode.idl",
        )
        .expect("the fixture parses");
        let value = json!({ "table": [[1, 2.5], [-3, 4.5]] });
        for policy in MapLengthPolicy::ALL {
            let mut request = ProbeRequest::new("S", Encoding::PlainCdr2, Endianness::Little);
            request.map_length = policy;
            let bytes = encode(&ast, &request, &value).expect("the sample serialises");
            let back = decode(&ast, &request, &bytes).expect("the bytes read back");
            // Sorted on the key's canonical text, which is the order
            // both sides of a comparison are put in: `"-3"` before
            // `"1"`. The rule only has to be the same rule everywhere,
            // not a numeric one.
            assert_eq!(
                text(&back),
                r#"{"table":[[-3,4.5],[1,2.5]]}"#,
                "under {policy:?}"
            );
        }
    }
}
