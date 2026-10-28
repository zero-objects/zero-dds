// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The probe against bytes three foreign implementations produced.
//!
//! Each expectation is a hex string copied out of
//! `crates/ast-layout/tests/fixtures/vendor_bytes.md`, which records
//! what Cyclone DDS, RTI Connext 7.7 and Fast DDS wrote for that IDL
//! and that sample. Nothing in this project produced any of them.
//!
//! "Wrote" means "handed back from an in-memory serialisation call", not
//! "put on a socket". The two are known to differ for the encapsulation
//! options field (`docs/wire-feature-inventory.md` B5) and for the
//! content of alignment padding (`tools/wire-capture`). Nothing asserted
//! here is either of those; the tests that are, say so.
//!
//! Where the three disagree, the case says which policy reproduces
//! which — that is the point of the policy existing.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    // The sample value is 3.14159, not pi — it is what the vendor runs
    // were given, and rounding it to the constant changes the bytes.
    clippy::approx_constant
)]

use serde_json::json;
use zerodds_ast_layout::{CollectionDelimiterPolicy, Dialect, Encoding, Endianness};
use zerodds_wire_probe::encode;
use zerodds_wire_probe::encode::{encode as encode_request, hex, model_from_idl, probe};
use zerodds_wire_probe::value::{Emit, ProbeRequest};

fn le1() -> Encoding {
    Encoding::PlainCdr1
}

/// The bytes the probe produces in the default dialect, as hex.
fn bytes(idl: &str, type_name: &str, values: &serde_json::Value, encoding: Encoding) -> String {
    hex(&probe(idl, type_name, values, encoding, Endianness::Little).expect("the probe"))
}

/// The bytes the probe produces in a named dialect, as hex.
///
/// The cases below that use it are the ones where the implementations
/// part: one model, one sample, one flag, each vendor's own bytes.
fn bytes_in(
    dialect: Dialect,
    idl: &str,
    type_name: &str,
    values: &serde_json::Value,
    encoding: Encoding,
) -> String {
    let ast = model_from_idl(idl, "probe.idl").expect("the model");
    let request = ProbeRequest::in_dialect(type_name, encoding, Endianness::Little, dialect);
    hex(&encode_request(&ast, &request, values).expect("the probe"))
}

// -- all three agree -------------------------------------------------

#[test]
fn point_matches_cyclone_rti_and_fastdds() {
    let idl = "@final struct Point { long x; double y; };";
    let values = json!({ "x": 1, "y": 2.0 });
    assert_eq!(
        bytes(idl, "Point", &values, Encoding::PlainCdr2),
        "010000000000000000000040"
    );
    assert_eq!(
        bytes(idl, "Point", &values, le1()),
        "01000000000000000000000000000040"
    );
}

#[test]
fn the_mixed_primitive_struct_matches_all_three() {
    let idl = "\
@final struct Mixed {
    boolean b; octet o; short s; unsigned short us;
    long l; unsigned long ul; long long ll; unsigned long long ull;
    float f; double d;
};";
    let values = json!({
        "b": true, "o": 171, "s": -12345, "us": 54321,
        "l": -1234567, "ul": 2345678,
        "ll": "-987654321", "ull": "123456789",
        "f": 2.5, "d": 3.14159
    });
    assert_eq!(
        bytes(idl, "Mixed", &values, Encoding::PlainCdr2),
        "01abc7cf31d400007929edffceca23004f9721c5ffffffff15cd5b0700000000\
         000020406e861bf0f9210940"
            .replace(char::is_whitespace, "")
    );
    assert_eq!(
        bytes(idl, "Mixed", &values, le1()),
        "01abc7cf31d400007929edffceca23004f9721c5ffffffff15cd5b0700000000\
         00002040000000006e861bf0f9210940"
            .replace(char::is_whitespace, "")
    );
}

#[test]
fn a_structure_carries_no_trailing_padding() {
    let idl = "@final struct Odd { long a; short b; };";
    let values = json!({ "a": 1, "b": 2 });
    assert_eq!(
        bytes(idl, "Odd", &values, Encoding::PlainCdr2),
        "010000000200"
    );
    assert_eq!(bytes(idl, "Odd", &values, le1()), "010000000200");
}

#[test]
fn an_array_of_primitives_matches_all_three() {
    let idl = "@final struct Arr { long a[3]; double d[2]; };";
    let values = json!({ "a": [1, 2, 3], "d": [1.5, 2.5] });
    assert_eq!(
        bytes(idl, "Arr", &values, Encoding::PlainCdr2),
        "010000000200000003000000000000000000f83f0000000000000440"
    );
    assert_eq!(
        bytes(idl, "Arr", &values, le1()),
        "01000000020000000300000000000000000000000000f83f0000000000000440"
    );
}

#[test]
fn a_string_member_matches_all_three() {
    let idl = "@final struct Str { string s; long n; };";
    let values = json!({ "s": "abc", "n": 7 });
    assert_eq!(
        bytes(idl, "Str", &values, Encoding::PlainCdr2),
        "040000006162630007000000"
    );
}

#[test]
fn a_sequence_of_a_primitive_matches_all_three() {
    let idl = "@final struct SeqL { sequence<long> ids; };";
    let values = json!({ "ids": [1, 2, 3] });
    assert_eq!(
        bytes(idl, "SeqL", &values, Encoding::PlainCdr2),
        "03000000010000000200000003000000"
    );
}

#[test]
fn an_enumeration_matches_all_three() {
    let idl = "enum Colour { RED, GREEN, BLUE };\n@final struct Enu { Colour c; long x; };";
    let values = json!({ "c": "BLUE", "x": 9 });
    assert_eq!(
        bytes(idl, "Enu", &values, Encoding::PlainCdr2),
        "0200000009000000"
    );
}

#[test]
fn a_union_in_a_struct_matches_all_three() {
    let idl = "\
@final union Choice switch (long) {
    case 1: long a;
    default: double b;
};
@final struct Uni { Choice c; };";
    let values = json!({ "c": { "_d": 1, "a": 42 } });
    assert_eq!(
        bytes(idl, "Uni", &values, Encoding::PlainCdr2),
        "010000002a000000"
    );
}

#[test]
fn the_three_extensibilities_match_all_three() {
    let members = "long a; double b; short c;";
    let values = json!({ "a": 7, "b": 1.5, "c": -3 });

    let final_ = format!("@final struct Ext {{ {members} }};");
    assert_eq!(
        bytes(&final_, "Ext", &values, Encoding::PlainCdr2),
        "07000000000000000000f83ffdff"
    );
    assert_eq!(
        bytes(&final_, "Ext", &values, le1()),
        "0700000000000000000000000000f83ffdff"
    );

    // `@appendable` is a delimiter and nothing else under XCDR2, and
    // nothing at all under XCDR1.
    let appendable = format!("@appendable struct Ext {{ {members} }};");
    assert_eq!(
        bytes(&appendable, "Ext", &values, Encoding::DelimitedCdr),
        "0e00000007000000000000000000f83ffdff"
    );
    assert_eq!(
        bytes(&appendable, "Ext", &values, le1()),
        "0700000000000000000000000000f83ffdff"
    );

    // `@mutable` under XCDR2: all three wrote the same thirty bytes.
    let mutable = format!("@mutable struct Ext {{ {members} }};");
    assert_eq!(
        bytes(&mutable, "Ext", &values, Encoding::PlCdr2),
        "1a000000000000200700000001000030000000000000f83f02000010fdff"
    );
}

/// `@mutable` under XCDR1, where the three do not agree on anything but
/// the values.
///
/// Three streams, copied out of `vendor_bytes.md`'s `ext_mutable` rows.
/// They differ in the header form, in what the length field counts and
/// in the must-understand bit of the marker and the sentinel — three
/// axes that move together, which is why one policy holds all three.
#[test]
fn the_pl_cdr1_parameter_list_reproduces_each_vendor_under_its_dialect() {
    let idl = "@mutable struct Ext { long a; double b; short c; };";
    let values = json!({ "a": 7, "b": 1.5, "c": -3 });

    assert_eq!(
        bytes_in(Dialect::Cyclone, idl, "Ext", &values, Encoding::PlCdr1),
        "017f08000000000004000000 07000000\
         017f08000100000008000000 000000000000f83f\
         017f08000200000004000000 fdff0000\
         027f0000"
            .replace(char::is_whitespace, ""),
        "Cyclone: PID_EXTENDED throughout, must-understand set, 56 bytes"
    );
    assert_eq!(
        bytes_in(Dialect::Rti, idl, "Ext", &values, Encoding::PlCdr1),
        "00000400 07000000\
         01000800 000000000000f83f\
         02000400 fdff0000\
         027f0000"
            .replace(char::is_whitespace, ""),
        "RTI: the short form, `short c` declared as four, 32 bytes"
    );
    assert_eq!(
        bytes_in(Dialect::FastDds, idl, "Ext", &values, Encoding::PlCdr1),
        "00000400 07000000\
         01000800 000000000000f83f\
         02000200 fdff0000\
         023f0000"
            .replace(char::is_whitespace, ""),
        "Fast DDS: the short form, `short c` declared as two, and a \
         sentinel without the must-understand bit"
    );
    // XTypes 1.3 §7.4.1.2: the length is the exact length of the member,
    // so `short c` declares two; the sentinel carries the must-understand
    // bit (Table 34). The multiple-of-four length of DDSI-RTPS 2.5
    // §9.4.2.11 is the named setting `short-rounded-length`, and with it
    // the clause's bytes are RTI's.
    assert_eq!(
        bytes_in(
            Dialect::Specification,
            idl,
            "Ext",
            &values,
            Encoding::PlCdr1
        ),
        "00000400 07000000\
         01000800 000000000000f83f\
         02000200 fdff0000\
         027f0000"
            .replace(char::is_whitespace, ""),
        "the specification dialect declares the exact length"
    );
    {
        let ast = model_from_idl(idl, "probe.idl").expect("the model");
        let mut request = ProbeRequest::new("Ext", Encoding::PlCdr1, Endianness::Little);
        request.pl_cdr1_headers = zerodds_ast_layout::PlCdr1HeaderPolicy::ShortRoundedLength;
        assert_eq!(
            hex(&encode_request(&ast, &request, &values).expect("the probe")),
            bytes_in(Dialect::Rti, idl, "Ext", &values, Encoding::PlCdr1),
            "the ParameterList format's multiple-of-four length is RTI's shape"
        );
    }
    assert_eq!(
        bytes(idl, "Ext", &values, Encoding::PlCdr1),
        bytes_in(
            Dialect::Specification,
            idl,
            "Ext",
            &values,
            Encoding::PlCdr1
        ),
        "and the default dialect is the clause's — it was Cyclone's, whose 56-byte \
         extended-header row is the first assertion above"
    );
}

/// XTypes 1.3 §7.4.1.1.5.1: "The members defined by the base type, if
/// any, shall be serialized before the members of their derived types.
/// The representation shall be exactly as if all of the members had been
/// defined, in the same order, in the most-derived type."
///
/// The failure this pins is a silent one: with the base's members absent
/// from the plan, the probe wrote the derived members alone and returned
/// success.
#[test]
fn an_inherited_member_is_written_ahead_of_the_derived_ones() {
    let base = "struct Base { long base_a; short base_b; };";
    let derived = "struct Sample : Base { double derived_a; octet derived_b; };";
    let values = json!({
        "base_a": -1234567, "base_b": -2,
        "derived_a": 3.141592653589793, "derived_b": 171
    });

    // One flat run, the base first and no frame of its own.
    let final_ = format!("@final {base} @final {derived}");
    assert_eq!(
        bytes(&final_, "Sample", &values, Encoding::PlainCdr2),
        "7929edfffeff0000182d4454fb210940ab"
    );
    assert_eq!(
        bytes(&final_, "Sample", &values, le1()),
        "7929edfffeff0000182d4454fb210940ab"
    );

    // One delimiter around the pair, not one each.
    let appendable = format!("@appendable {base} @appendable {derived}");
    assert_eq!(
        bytes(&appendable, "Sample", &values, Encoding::DelimitedCdr),
        "110000007929edfffeff0000182d4454fb210940ab"
    );
}

/// §7.3.1.2.1.1: "In the case an aggregated type inherits from another
/// aggregated type the first member of the derived type gets the member
/// ID of the last member in the base type plus one."
///
/// The EMHEADER ids are the only place that rule is visible on the wire,
/// which is why the `@mutable` pair is the one measured.
#[test]
fn member_ids_continue_across_the_inheritance_boundary() {
    let idl = "\
@mutable struct Base { long base_a; short base_b; };
@mutable struct Sample : Base { double derived_a; octet derived_b; };";
    let values = json!({
        "base_a": -1234567, "base_b": -2,
        "derived_a": 3.141592653589793, "derived_b": 171
    });
    // ids 0, 1, 2, 3 — the derived members start at 2, not back at 0.
    assert_eq!(
        bytes(idl, "Sample", &values, Encoding::PlCdr2),
        "21000000000000207929edff01000010feff000002000030182d4454fb2109\
         4003000000ab"
            .replace(char::is_whitespace, "")
    );
}

/// XTypes 1.3 §7.2.2.4.5: a derived structure that states no
/// extensibility has its base's.
///
/// Measured with the byte gate, case `struct_inherit_mut_silent`: Cyclone
/// 11.0.1 and RTI 7.7.0 write this `Sample` under XCDR2 as the 37 bytes of
/// the `@mutable` pair above (RTI differing only in the two padding octets
/// after `feff`, see `vendor_bytes.md`). ZeroDDS took the rule's appendable
/// default for the derived structure instead and wrote a DHEADER with no
/// EMHEADERs.
#[test]
fn a_derived_structure_that_states_no_extensibility_is_framed_as_its_base() {
    let idl = "\
@mutable struct Base { long base_a; short base_b; };
struct Sample : Base { double derived_a; octet derived_b; };";
    let values = json!({
        "base_a": -1234567, "base_b": -2,
        "derived_a": 3.141592653589793, "derived_b": 171
    });
    assert_eq!(
        bytes(idl, "Sample", &values, Encoding::PlainCdr2),
        "21000000000000207929edff01000010feff000002000030182d4454fb2109\
         4003000000ab"
            .replace(char::is_whitespace, "")
    );
}

/// A typedef of an array is the array. The frontend used to drop the
/// declarator's dimensions, which made `Matrix` an alias of `long` and
/// the member one four-byte value where the vendors write twenty-four.
#[test]
fn a_typedef_of_an_array_matches_cyclone_and_rti() {
    let idl = "\
typedef long Matrix[2][3];
@final struct Sample { Matrix grid; long tail; };";
    let values = json!({
        "grid": [[1, 2, 3], [-1, 2147483647, -1234567]],
        "tail": -1
    });
    let expected = "010000000200000003000000ffffffffffffff7f7929edffffffffff";
    assert_eq!(bytes(idl, "Sample", &values, Encoding::PlainCdr2), expected);
    assert_eq!(bytes(idl, "Sample", &values, le1()), expected);
}

/// §7.4.3.5 gives ENUM_TYPE a signed holder, so `@value(-1)` is the
/// two's complement and not a zero. RTI Connext 7.7 wrote these eight
/// bytes; Cyclone's `idlc` refuses `@value` with a negative argument, so
/// RTI is the only oracle this case has.
///
/// The failure this pins was the quietest of the three:
/// `u32::try_from(-1).unwrap_or(0)` wrote another enumerator's value and
/// the probe reported success.
#[test]
fn a_negative_enumerator_is_the_twos_complement_rti_wrote() {
    let idl = "\
enum Signed { @value(-1) MINUS, @value(0) ZERO, @value(2147483647) MAX };
@final struct Sample { Signed first; Signed second; };";
    let values = json!({ "first": "MINUS", "second": "MAX" });
    assert_eq!(
        bytes(idl, "Sample", &values, Encoding::PlainCdr2),
        "ffffffffffffff7f"
    );
    assert_eq!(bytes(idl, "Sample", &values, le1()), "ffffffffffffff7f");
}

/// The other end of the same rule: Cyclone reads `@bit_bound(8)` as a
/// width and writes `@value(200)` as `c8`. RTI refuses any enum bound
/// but 32, so Cyclone is the only oracle here.
#[test]
fn a_narrow_bound_holds_the_full_width_cyclone_wrote() {
    let idl = "\
@bit_bound(8) enum Small { @value(3) RED, @value(9) GREEN, @value(200) BLUE };
@final struct Sample { Small first; Small second; octet tail; };";
    let values = json!({ "first": "BLUE", "second": "GREEN", "tail": 171 });
    assert_eq!(bytes(idl, "Sample", &values, Encoding::PlainCdr2), "c809ab");
    assert_eq!(bytes(idl, "Sample", &values, le1()), "c809ab");
}

/// Past the width neither implementation can express the value, so the
/// probe says so rather than writing a truncated one.
#[test]
fn an_enumerator_wider_than_its_bound_is_refused_and_not_truncated() {
    let idl = "\
@bit_bound(8) enum Small { @value(256) TOO_WIDE, @value(0) ZERO };
@final struct Sample { Small only; };";
    let ast = model_from_idl(idl, "probe.idl").expect("the model");
    let request = ProbeRequest::new("Sample", Encoding::PlainCdr2, Endianness::Little);
    let error = encode(&ast, &request, &json!({ "only": "TOO_WIDE" })).expect_err("a refusal");
    assert!(
        matches!(&error, zerodds_wire_probe::ProbeError::Mismatch { found, .. }
            if found.contains("TOO_WIDE") && found.contains("256")),
        "{error}"
    );
}

/// A union over an enumeration, which used not to lower at all: the
/// case label was taken for a constant, looked up from the root and
/// reported `NotFound`.
///
/// `Choice` is unannotated, so this is also the default-extensibility
/// split below — RTI reads it as appendable and writes the `0c000000`
/// delimiter, Cyclone reads it as final and writes sixteen bytes. Both
/// are measured, and the dialect is what picks between them.
#[test]
fn a_union_over_an_enumeration_splits_cyclone_from_rti() {
    let idl = "\
enum Colour { RED, GREEN, BLUE };
union Choice switch (Colour) {
    case RED: long whole;
    case GREEN:
    case BLUE: double fraction;
};
@final struct Sample { Choice picked; long tail; };";
    let values = json!({
        "picked": { "_d": "GREEN", "fraction": 3.141592653589793 },
        "tail": -1
    });
    assert_eq!(
        bytes_in(Dialect::Rti, idl, "Sample", &values, Encoding::DelimitedCdr),
        "0c00000001000000182d4454fb210940ffffffff",
        "RTI Connext 7.7: twenty bytes behind the union's delimiter"
    );
    assert_eq!(
        bytes_in(
            Dialect::Cyclone,
            idl,
            "Sample",
            &values,
            Encoding::DelimitedCdr
        ),
        "01000000182d4454fb210940ffffffff",
        "Cyclone DDS: sixteen, no delimiter"
    );
    assert_eq!(
        bytes(idl, "Sample", &values, Encoding::DelimitedCdr),
        bytes_in(Dialect::Rti, idl, "Sample", &values, Encoding::DelimitedCdr),
        "and the default dialect reads §7.3.1.2.1.8 the way RTI does — it read it \
         Cyclone's way while `Dialect::Cyclone` was the default"
    );
    // XCDR1 has no delimiter, and there the three agree: Cyclone and
    // RTI both wrote these twenty bytes.
    for dialect in Dialect::ALL {
        assert_eq!(
            bytes_in(dialect, idl, "Sample", &values, le1()),
            "0100000000000000182d4454fb210940ffffffff"
        );
    }
}

#[test]
fn an_absent_optional_member_is_not_written() {
    let idl = "@mutable struct MutOpt { long a; @optional long b; };";
    let values = json!({ "a": 7 });
    assert_eq!(
        bytes(idl, "MutOpt", &values, Encoding::PlCdr2),
        "080000000000002007000000"
    );
}

#[test]
fn the_reported_robot_type_writes_cyclone_s_bytes_and_rti_s_on_request() {
    // GitHub issues #41 and #42. Unannotated throughout, which is the
    // whole disagreement: Cyclone read it as final and wrote 48 bytes,
    // RTI read it as appendable and wrote 60.
    let idl = "\
module common {
    module util {
        struct Location { double x; double y; double z; };
        struct Orientation { double x; double y; double z; };
    };
    struct Robot {
        ::common::util::Location location;
        ::common::util::Orientation orientation;
    };
};";
    let values = json!({
        "location": { "x": 1.0, "y": 2.0, "z": 3.0 },
        "orientation": { "x": 4.0, "y": 5.0, "z": 6.0 }
    });
    let cyclone = "000000000000f03f0000000000000040000000000000084000000000000010\
         40000000000000144000000000000018 40"
        .replace(char::is_whitespace, "");
    let rti = "3800000018000000000000000000f03f00000000000000400000000000000840\
         18000000000000000000104000000000000014400000000000001840"
        .replace(char::is_whitespace, "");

    assert_eq!(
        bytes_in(
            Dialect::Cyclone,
            idl,
            "common::Robot",
            &values,
            Encoding::DelimitedCdr
        ),
        cyclone,
        "Cyclone's 48-byte reading, which is what `--dialect cyclone` is for"
    );
    assert_eq!(
        bytes(idl, "common::Robot", &values, Encoding::DelimitedCdr),
        rti,
        "and the default dialect is the clause's, which is the 60-byte reading — the \
         reversal of issues #41 and #42: what a caller who names nothing gets is now \
         §7.3.1.2.1.8's `@appendable` and not Cyclone's `@final`"
    );
    assert_eq!(
        bytes_in(
            Dialect::Rti,
            idl,
            "common::Robot",
            &values,
            Encoding::DelimitedCdr
        ),
        rti
    );
    assert_eq!(
        bytes_in(
            Dialect::Specification,
            idl,
            "common::Robot",
            &values,
            Encoding::DelimitedCdr
        ),
        rti,
        "§7.3.1.2.1.8's appendable is the reading RTI took"
    );

    // Under XCDR1 there is no delimiter, which is why the same pair of
    // peers interoperates there: 48 bytes, as all of them wrote.
    for dialect in Dialect::ALL {
        assert_eq!(
            bytes_in(dialect, idl, "common::Robot", &values, le1()),
            cyclone
        );
    }
}

// -- where the vendors disagree ---------------------------------------

#[test]
fn a_collection_of_structs_reproduces_each_vendor_under_its_policy() {
    let idl = "\
@final struct Cell { long a; long b; };
@final struct SeqS { sequence<Cell> cells; };";
    let values = json!({ "cells": [ { "a": 1, "b": 2 }, { "a": 3, "b": 4 } ] });
    let ast = model_from_idl(idl, "probe.idl").expect("the model");

    let spec = ProbeRequest::new("SeqS", Encoding::PlainCdr2, Endianness::Little);
    assert_eq!(
        hex(&encode(&ast, &spec, &values).expect("bytes")),
        "140000000200000001000000020000000300000004000000",
        "Cyclone and Fast DDS"
    );

    let rti = ProbeRequest {
        collection_delimiters: CollectionDelimiterPolicy::Never,
        ..spec.clone()
    };
    assert_eq!(
        hex(&encode(&ast, &rti, &values).expect("bytes")),
        "0200000001000000020000000300000004000000",
        "RTI Connext 7.7"
    );
}

#[test]
fn a_collection_of_enumerations_splits_the_vendors_three_ways() {
    let idl = "\
enum Colour { RED, GREEN, BLUE };
@final struct SeqE { sequence<Colour> cs; };";
    let values = json!({ "cs": ["RED", "BLUE"] });
    let ast = model_from_idl(idl, "probe.idl").expect("the model");

    let spec = ProbeRequest::new("SeqE", Encoding::PlainCdr2, Endianness::Little);
    assert_eq!(
        hex(&encode(&ast, &spec, &values).expect("bytes")),
        "0c000000020000000000000002000000",
        "Cyclone"
    );

    for policy in [
        CollectionDelimiterPolicy::EnumIsPrimitive,
        CollectionDelimiterPolicy::Never,
    ] {
        let request = ProbeRequest {
            collection_delimiters: policy,
            ..spec.clone()
        };
        assert_eq!(
            hex(&encode(&ast, &request, &values).expect("bytes")),
            "020000000000000002000000",
            "Fast DDS and RTI, under {}",
            policy.as_str()
        );
    }
}

#[test]
fn an_array_of_structs_reproduces_each_vendor_under_its_policy() {
    let idl = "\
@final struct Cell { long a; long b; };
@final struct ArrN { Cell items[2]; };";
    let values = json!({ "items": [ { "a": 1, "b": 2 }, { "a": 3, "b": 4 } ] });
    let ast = model_from_idl(idl, "probe.idl").expect("the model");

    let spec = ProbeRequest::new("ArrN", Encoding::PlainCdr2, Endianness::Little);
    assert_eq!(
        hex(&encode(&ast, &spec, &values).expect("bytes")),
        "1000000001000000020000000300000004000000",
        "Cyclone and Fast DDS"
    );

    let rti = ProbeRequest {
        collection_delimiters: CollectionDelimiterPolicy::Never,
        ..spec
    };
    assert_eq!(
        hex(&encode(&ast, &rti, &values).expect("bytes")),
        "01000000020000000300000004000000",
        "RTI Connext 7.7"
    );
}

#[test]
fn array_elements_pad_between_themselves_but_not_after_the_last() {
    let idl = "\
@final struct Odd { long a; short b; };
@final struct ArrOdd { Odd items[2]; octet tail; };";
    let values = json!({
        "items": [ { "a": 1, "b": 2 }, { "a": 3, "b": 4 } ],
        "tail": 119
    });
    assert_eq!(
        bytes(idl, "ArrOdd", &values, le1()),
        "010000000200000003000000040077"
    );
}

// -- the value format itself ------------------------------------------

#[test]
fn a_misspelled_member_is_an_error_and_not_a_default() {
    let idl = "@final struct Point { long x; double y; };";
    let ast = model_from_idl(idl, "probe.idl").expect("the model");
    let request = ProbeRequest::new("Point", Encoding::PlainCdr2, Endianness::Little);
    let error = encode(&ast, &request, &json!({ "x": 1, "z": 2.0 })).expect_err("a typo");
    assert!(
        matches!(&error, zerodds_wire_probe::ProbeError::Unknown { path, .. } if path == "Point.z"),
        "{error}"
    );
}

#[test]
fn a_missing_member_that_is_not_optional_is_an_error() {
    let idl = "@final struct Point { long x; double y; };";
    let ast = model_from_idl(idl, "probe.idl").expect("the model");
    let request = ProbeRequest::new("Point", Encoding::PlainCdr2, Endianness::Little);
    let error = encode(&ast, &request, &json!({ "x": 1 })).expect_err("a gap");
    assert!(
        matches!(&error, zerodds_wire_probe::ProbeError::Missing { path } if path == "Point.y"),
        "{error}"
    );
}

#[test]
fn null_is_refused_so_absent_and_null_cannot_be_confused() {
    let idl = "@mutable struct MutOpt { long a; @optional long b; };";
    let ast = model_from_idl(idl, "probe.idl").expect("the model");
    let request = ProbeRequest::new("MutOpt", Encoding::PlCdr2, Endianness::Little);
    let error =
        encode(&ast, &request, &json!({ "a": 7, "b": null })).expect_err("null is not absent");
    assert!(
        matches!(&error, zerodds_wire_probe::ProbeError::Mismatch { path, .. } if path == "MutOpt.b"),
        "{error}"
    );
}

#[test]
fn a_sixty_four_bit_integer_survives_as_a_string() {
    // 2^53 + 1 does not survive a JSON number, which is a double.
    let idl = "@final struct Big { unsigned long long v; };";
    let values = json!({ "v": "9007199254740993" });
    assert_eq!(
        bytes(idl, "Big", &values, Encoding::PlainCdr2),
        "0100000000002000"
    );
}

#[test]
fn the_key_hash_of_a_small_key_is_the_zero_padded_holder() {
    // XTypes 1.3 §7.6.8 example 1: `@key long id` with id = 5, holder
    // big-endian and under sixteen bytes, so zero-padded rather than
    // hashed.
    let idl = "@final struct Keyed { @key long id; long x; };";
    let ast = model_from_idl(idl, "probe.idl").expect("the model");
    let request =
        ProbeRequest::new("Keyed", Encoding::PlainCdr2, Endianness::Little).emitting(Emit::KeyHash);
    let hash = encode(&ast, &request, &json!({ "id": 5, "x": 6 })).expect("a key hash");
    assert_eq!(hex(&hash), "00000005000000000000000000000000");
}

/// Member id 0x3F00 is the last one the short PL_CDR1 header carries.
///
/// XTypes 1.3 §7.4.1.2.2 gives the range as "from 0 to 16,128 (0x3F00)
/// inclusive". Case `pid_3f00`, 2026-08-10, `--dialect rti` and
/// `--dialect fastdds`: both vendors write the four-octet header for
/// both ids. Cyclone writes the extended form for every member whatever
/// its id, so it is the same 36 octets it writes for anything.
#[test]
fn the_last_short_pl_cdr1_id_is_the_one_two_vendors_write_short() {
    let idl = "@mutable struct PidEdge { @id(16127) long a; @id(16128) long b; };";
    let values = json!({ "a": 7, "b": 9 });
    assert_eq!(
        bytes_in(Dialect::Rti, idl, "PidEdge", &values, Encoding::PlCdr1),
        "ff3e040007000000003f040009000000027f0000"
    );
    assert_eq!(
        bytes_in(Dialect::FastDds, idl, "PidEdge", &values, Encoding::PlCdr1),
        "ff3e040007000000003f040009000000023f0000"
    );
    assert_eq!(
        bytes_in(Dialect::Cyclone, idl, "PidEdge", &values, Encoding::PlCdr1),
        "017f0800ff3e00000400000007000000017f0800003f00000400000009000000027f0000"
    );
    // XCDR2 puts the id in 28 bits and has no such edge; all three agree.
    for dialect in [Dialect::Cyclone, Dialect::Rti, Dialect::FastDds] {
        assert_eq!(
            bytes_in(dialect, idl, "PidEdge", &values, Encoding::PlCdr2),
            "10000000ff3e002007000000003f002009000000",
            "{dialect:?}"
        );
    }
}

/// A `@mutable` type inside a `@mutable` type, with the same member id
/// at both levels — §7.4.1.2.4's own example.
///
/// Case `nested_mutable`, 2026-08-10. Ids are relative to the innermost
/// type and `PID_LIST_END` ends the innermost list: Cyclone's XCDR1 form
/// carries two sentinels, one per level. `fastddsgen` refuses a
/// `@mutable` member of `@mutable` type, so Fast DDS has no answer here
/// and only its sentinel differs from RTI's.
#[test]
fn a_nested_parameter_list_numbers_and_terminates_per_level() {
    let idl = "\
@mutable struct Inner { @id(5) long a; };
@mutable struct Outer { @id(5) Inner nested; };";
    let values = json!({ "nested": { "a": 7 } });
    assert_eq!(
        bytes_in(Dialect::Cyclone, idl, "Outer", &values, Encoding::PlCdr1),
        "017f08000500000014000000017f0800050000000400000007000000027f0000027f0000"
    );
    assert_eq!(
        bytes_in(Dialect::Rti, idl, "Outer", &values, Encoding::PlCdr1),
        "05000c000500040007000000027f0000027f0000"
    );
    assert_eq!(
        bytes_in(Dialect::Cyclone, idl, "Outer", &values, Encoding::PlCdr2),
        "14000000050000400c000000080000000500002007000000"
    );
    // RTI reuses the inner DHEADER as the NEXTINT — LC5, `LengthCodePolicy`
    // — so its XCDR2 form is four octets shorter than Cyclone's.
    assert_eq!(
        bytes_in(Dialect::Rti, idl, "Outer", &values, Encoding::PlCdr2),
        "1000000005000050080000000500002007000000"
    );
}

/// A typedef of the element type changes nothing inside a `@mutable`
/// member's sequence.
///
/// Case `mut_seq_typedef`, 2026-08-10. `sequence<long>` and
/// `sequence<L>` where `typedef long L` take the same length code and
/// the same bytes in all three implementations — which is the `Alias`
/// arm of `sequence_length_code`, and it had never been executed.
#[test]
fn an_aliased_sequence_element_takes_the_same_length_code_as_the_type_it_aliases() {
    let idl = "\
typedef long L;
@mutable struct MT { @id(1) sequence<long> plain; @id(2) sequence<L> aliased; };";
    let values = json!({ "plain": [1, 2], "aliased": [3, 4] });
    // Both members take LC6 — `0x6…` — and the two halves are the same
    // shape with different values.
    for dialect in [Dialect::Cyclone, Dialect::Rti, Dialect::FastDds] {
        assert_eq!(
            bytes_in(dialect, idl, "MT", &values, Encoding::PlCdr2),
            "200000000100006002000000010000000200000002000060020000000300000004000000",
            "{dialect:?}"
        );
    }
    assert_eq!(
        bytes_in(Dialect::Rti, idl, "MT", &values, Encoding::PlCdr1),
        "01000c0002000000010000000200000002000c00020000000300000004000000027f0000"
    );
}

/// A `boolean` and a `char` discriminator, which §7.4.3.5 rule (26)
/// admits and nothing had measured.
///
/// Cases `union_disc_bool`, `union_disc_bool_default` and
/// `union_disc_char`, 2026-08-10. Both are one octet followed by the
/// selected member's own alignment padding. Cyclone and RTI produce all
/// three; `fastddsgen` refuses a `boolean` switch outright — *"boolean
/// switch is malformed"* — so Fast DDS answers only the `char` case.
#[test]
fn a_boolean_and_a_char_discriminator_are_one_octet_and_then_the_member() {
    let boolean = "\
@final union UB switch (boolean) { case TRUE: long a; default: short b; };
@final struct SB { UB u; long tail; };";
    assert_eq!(
        bytes(
            boolean,
            "SB",
            &json!({ "u": { "_d": true, "a": 7 }, "tail": -1 }),
            Encoding::PlainCdr2
        ),
        "0100000007000000ffffffff"
    );
    assert_eq!(
        bytes(
            boolean,
            "SB",
            &json!({ "u": { "_d": false, "b": -3 }, "tail": -1 }),
            Encoding::PlainCdr2
        ),
        "0000fdffffffffff"
    );

    let character = "\
@final union UC switch (char) { case 'a': long x; default: short y; };
@final struct SC { UC u; long tail; };";
    assert_eq!(
        bytes(
            character,
            "SC",
            &json!({ "u": { "_d": "a", "x": 7 }, "tail": -1 }),
            Encoding::PlainCdr2
        ),
        "6100000007000000ffffffff"
    );
    // XCDR1 is the same bytes: a `@final` union frames nothing in
    // either version.
    assert_eq!(
        bytes(
            character,
            "SC",
            &json!({ "u": { "_d": "a", "x": 7 }, "tail": -1 }),
            le1()
        ),
        "6100000007000000ffffffff"
    );
}

/// The KeyHash of a holder whose declaration order is the reverse of
/// its member-id order, from three implementations.
///
/// XTypes 1.3 §7.6.8 orders the holder's members by member id.
/// Measured 2026-08-10 through `n3-keyholder/` on the Linux test
/// host: Cyclone DDS 11.0.1 through
/// `dds_stream_extract_keyBE_from_key(…, DDS_CDR_KEY_SERIALIZATION_KEYHASH, …)`,
/// RTI Connext 7.7.0 through
/// `PRESTypePlugin_interpretedSerializeKeyForKeyhash` under `CDR_BE`,
/// `PL_CDR_BE`, `CDR2_BE`, `D_CDR2_BE` and `PL_CDR2_BE`, and Fast DDS
/// through the generated `PubSubType::compute_key`.
///
/// Three of the four combinations order by member id. RTI under
/// encoding version 1 keeps declaration order — the third axis on which
/// its XCDR1 key holder differs from everyone else's, beside
/// `KeyHolderExtensibility` and `KeyHolderNesting`.
#[test]
fn a_key_holders_member_order_is_the_one_each_vendor_hashes() {
    let idl = "\
@final struct KeyOrder { @id(5) @key long b; @id(2) @key long a; };
@final struct KeyMixed { @id(5) @key octet b; @id(2) @key long a; };";
    let values = json!({ "b": 2, "a": 1 });
    let hash = |dialect: Dialect, type_name: &str, encoding: Encoding| {
        let ast = model_from_idl(idl, "probe.idl").expect("the model");
        let request = ProbeRequest::in_dialect(type_name, encoding, Endianness::Little, dialect)
            .emitting(Emit::KeyHash);
        hex(&encode_request(&ast, &request, &values).expect("a key hash"))
    };

    // Member-id order: `a` first, then `b`.
    for (dialect, encoding) in [
        (Dialect::Cyclone, le1()),
        (Dialect::Cyclone, Encoding::PlainCdr2),
        (Dialect::Rti, Encoding::PlainCdr2),
        (Dialect::FastDds, le1()),
        (Dialect::FastDds, Encoding::PlainCdr2),
        (Dialect::Specification, le1()),
        (Dialect::Specification, Encoding::PlainCdr2),
    ] {
        assert_eq!(
            hash(dialect, "KeyOrder", encoding),
            "00000001000000020000000000000000",
            "{dialect:?} {encoding:?}"
        );
        // The mixed-width holder is what tells an ordering from a
        // coincidence: five octets and no padding this way round.
        assert_eq!(
            hash(dialect, "KeyMixed", encoding),
            "00000001020000000000000000000000",
            "{dialect:?} {encoding:?}"
        );
    }

    // RTI under XCDR1: declaration order, `b` first — and for
    // `KeyMixed` that puts the `octet` at offset 0 with three octets of
    // padding behind it, so the holder is eight and not five.
    assert_eq!(
        hash(Dialect::Rti, "KeyOrder", le1()),
        "00000002000000010000000000000000"
    );
    assert_eq!(
        hash(Dialect::Rti, "KeyMixed", le1()),
        "02000000000000010000000000000000"
    );
}

/// A key holder past sixteen octets is the MD5 of the holder.
///
/// DDSI-RTPS 2.5 §9.6.3.8. The `Md5` arm of `key_hash_form` had no test
/// that executed it and no foreign measurement; this is both.
/// `n3-keyholder/`, 2026-08-10 — Cyclone, RTI under all five
/// encapsulations and Fast DDS all three produce the same sixteen
/// octets for a twenty-octet holder.
#[test]
fn a_key_holder_past_sixteen_octets_is_md5_hashed() {
    let idl = "@final struct KeyBig \
{ @key long a; @key long b; @key long c; @key long d; @key long e; };";
    let values = json!({ "a": 1, "b": 2, "c": 3, "d": 4, "e": 5 });
    for dialect in [
        Dialect::Cyclone,
        Dialect::Rti,
        Dialect::FastDds,
        Dialect::Specification,
    ] {
        for encoding in [le1(), Encoding::PlainCdr2] {
            let ast = model_from_idl(idl, "probe.idl").expect("the model");
            let request = ProbeRequest::in_dialect("KeyBig", encoding, Endianness::Little, dialect)
                .emitting(Emit::KeyHash);
            assert_eq!(
                hex(&encode_request(&ast, &request, &values).expect("a key hash")),
                "4321f7288e521aa62aee2745f3f8d92b",
                "{dialect:?} {encoding:?}"
            );
        }
    }
}

#[test]
fn the_encapsulation_options_carry_the_padding_count() {
    // XTypes 1.3 §7.4.1.1 pads the serialized data to the next
    // four-byte boundary with zeros, and §7.6.3.1.2 puts the count in
    // the low two bits of the second options byte: those bits "shall be
    // interpreted by the receiver to determine where the serialized
    // data exactly ended".
    //
    // Measured on the loopback wire on 2026-08-09 with `tshark`.
    // `@final struct One { octet a; }` with `a = 0xAB`, all three:
    //
    //     cyclone   CDR_LE  options 0x0003  serializedData ab000000
    //     rti       CDR_LE  options 0x0003  serializedData ab000000
    //     fastdds   CDR_LE  options 0x0003  serializedData ab000000
    //
    // and §7.6.3.1.2's own `TypeB` through Cyclone: options 0x0001,
    // serializedData 23006200.
    let encapsulated = |idl: &str, type_name: &str, values: &serde_json::Value, encoding| {
        let ast = model_from_idl(idl, "probe.idl").expect("the model");
        let request = ProbeRequest::new(type_name, encoding, Endianness::Little)
            .emitting(Emit::PayloadWithEncapsulation);
        hex(&encode(&ast, &request, values).expect("the probe"))
    };

    assert_eq!(
        encapsulated(
            "@final struct One { octet a; };",
            "One",
            &json!({ "a": 171 }),
            le1()
        ),
        "00010003ab000000"
    );
    assert_eq!(
        encapsulated(
            "@final struct TypeB { short a; char b; };",
            "TypeB",
            &json!({ "a": 35, "b": "b" }),
            le1()
        ),
        "0001000123006200"
    );
    // A payload already on the boundary pads by nothing and says zero,
    // which is why leaving the field at zero looked right for so long.
    assert_eq!(
        encapsulated(
            "@final struct Point { long x; double y; };",
            "Point",
            &json!({ "x": 1, "y": 2.0 }),
            Encoding::PlainCdr2
        ),
        "00070000010000000000000000000040"
    );
}

#[test]
fn a_reader_that_is_told_the_padding_count_stops_at_the_data() {
    // The other half of §7.6.3.1.2, and the half that says "shall".
    // A reader that ignored the count would hand the decoder three
    // bytes of zero as if they were the stream.
    let idl = "@final struct One { octet a; };";
    let ast = model_from_idl(idl, "probe.idl").expect("the model");
    let request = ProbeRequest::new("One", Encoding::PlainCdr1, Endianness::Little)
        .emitting(Emit::PayloadWithEncapsulation);

    let value = zerodds_wire_probe::decode::decode(
        &ast,
        &request,
        &[0x00, 0x01, 0x00, 0x03, 0xAB, 0x00, 0x00, 0x00],
    )
    .expect("the sample reads back");
    assert_eq!(value, json!({ "a": 171 }));

    // A count longer than what follows the header is a malformed
    // stream, not a shorter sample.
    let error = zerodds_wire_probe::decode::decode(&ast, &request, &[0x00, 0x01, 0x00, 0x03, 0xAB])
        .expect_err("four padding bytes cannot fit in one");
    assert!(format!("{error}").contains("padding"), "{error}");

    // The padding is zero (§7.4.1.1). A non-zero byte there is either a
    // writer that miscounted or a reader at the wrong offset, and
    // cutting it off in silence would hide both.
    let error = zerodds_wire_probe::decode::decode(
        &ast,
        &request,
        &[0x00, 0x01, 0x00, 0x03, 0xAB, 0x00, 0x07, 0x00],
    )
    .expect_err("padding that is not zero");
    assert!(format!("{error}").contains("zero byte"), "{error}");
}

// -- length codes, where three implementations take three positions ----

/// `mut_lengthcodes`: ten members, one per length code, three dialects.
///
/// Run 2026-08-09 through `wire-oracle/gate`. The EMHEADERs are
/// where the three part, and they part twice: RTI reuses a nested
/// aggregate's own DHEADER as the NEXTINT where Cyclone writes LC4, and
/// Fast DDS does that *and* takes a short code whenever the member's
/// serialized size turns out to be 1, 2, 4 or 8 bytes.
///
/// Before `LengthCodePolicy` had four variants every dialect answered
/// `Compact`, and `Compact` itself gave `long ap[2]` and `Cell nested`
/// a short code — which is Fast DDS's answer written under Cyclone's
/// name. So all three presets were wrong here, each in its own way.
const LENGTH_CODE_IDL: &str = "\
@final struct Cell { long a; long b; };
@mutable struct LenCodes {
    string s;
    sequence<octet> o1;
    sequence<short> s2;
    sequence<long> l4;
    sequence<double> d8;
    sequence<Cell> sc;
    Cell arr[2];
    long ap[2];
    Cell nested;
    long tail;
};";

fn length_code_values() -> serde_json::Value {
    json!({
        "s": "abc", "o1": [1, 2, 3], "s2": [-1, 2], "l4": [1, 2], "d8": [1.5],
        "sc": [{ "a": 1, "b": 2 }],
        "arr": [{ "a": 3, "b": 4 }, { "a": 5, "b": 6 }],
        "ap": [7, 8], "nested": { "a": 9, "b": 10 }, "tail": 11
    })
}

#[test]
fn every_length_code_matches_the_dialect_s_own_vendor() {
    let values = length_code_values();
    for (dialect, expected) in [
        (
            Dialect::Cyclone,
            "9c000000\
             00000050 04000000 61626300\
             01000050 03000000 010203 00\
             02000040 08000000 02000000 ffff0200\
             03000060 02000000 01000000 02000000\
             04000070 01000000 000000000000f83f\
             05000050 0c000000 01000000 01000000 02000000\
             06000050 10000000 03000000 04000000 05000000 06000000\
             07000040 08000000 07000000 08000000\
             08000040 08000000 09000000 0a000000\
             09000020 0b000000",
        ),
        (
            Dialect::Rti,
            "9c000000\
             00000050 04000000 61626300\
             01000050 03000000 010203 00\
             02000040 08000000 02000000 ffff0200\
             03000060 02000000 01000000 02000000\
             04000070 01000000 000000000000f83f\
             05000040 0c000000 01000000 01000000 02000000\
             06000040 10000000 03000000 04000000 05000000 06000000\
             07000040 08000000 07000000 08000000\
             08000040 08000000 09000000 0a000000\
             09000020 0b000000",
        ),
        (
            Dialect::FastDds,
            "90000000\
             00000050 04000000 61626300\
             01000050 03000000 010203 00\
             02000030 02000000 ffff0200\
             03000060 02000000 01000000 02000000\
             04000070 01000000 000000000000f83f\
             05000050 0c000000 01000000 01000000 02000000\
             06000050 10000000 03000000 04000000 05000000 06000000\
             07000030 07000000 08000000\
             08000030 09000000 0a000000\
             09000020 0b000000",
        ),
    ] {
        let want: String = expected.chars().filter(|c| !c.is_whitespace()).collect();
        assert_eq!(
            bytes_in(
                dialect,
                LENGTH_CODE_IDL,
                "LenCodes",
                &values,
                Encoding::PlCdr2
            ),
            want,
            "{dialect:?}"
        );
    }
}

/// `mut_lc_nested`: whether a nested aggregate's own DHEADER is reused.
///
/// The same run. `ACell` is `@appendable` and `MCell` is `@mutable`, so
/// both begin with a DHEADER that is already the byte length LC5 would
/// state. RTI and Fast DDS reuse it; Cyclone writes LC4 with a FIXME
/// beside it in `dds_cdrstream.c`.
#[test]
fn a_nested_delimiter_is_reused_by_two_of_the_three() {
    let idl = "\
@final struct Cell { long a; long b; };
@appendable struct ACell { long a; long b; };
@mutable struct MCell { long a; long b; };
enum Colour { RED, GREEN, BLUE };
@mutable struct LenNest {
    ACell app;
    MCell mnest;
    Colour ce[2];
    Cell arr1[1];
    long tail;
};";
    let values = json!({
        "app": { "a": 1, "b": 2 }, "mnest": { "a": 3, "b": 4 },
        "ce": ["GREEN", "BLUE"], "arr1": [{ "a": 5, "b": 6 }], "tail": 7
    });
    for (dialect, expected) in [
        (
            Dialect::Cyclone,
            "58000000\
             00000040 0c000000 08000000 01000000 02000000\
             01000040 14000000 10000000 00000020 03000000 01000020 04000000\
             02000050 08000000 01000000 02000000\
             03000050 08000000 05000000 06000000\
             04000020 07000000",
        ),
        (
            Dialect::Rti,
            "50000000\
             00000050 08000000 01000000 02000000\
             01000050 10000000 00000020 03000000 01000020 04000000\
             02000040 08000000 01000000 02000000\
             03000040 08000000 05000000 06000000\
             04000020 07000000",
        ),
        (
            Dialect::FastDds,
            "4c000000\
             00000050 08000000 01000000 02000000\
             01000050 10000000 00000020 03000000 01000020 04000000\
             02000030 01000000 02000000\
             03000050 08000000 05000000 06000000\
             04000020 07000000",
        ),
    ] {
        let want: String = expected.chars().filter(|c| !c.is_whitespace()).collect();
        assert_eq!(
            bytes_in(dialect, idl, "LenNest", &values, Encoding::PlCdr2),
            want,
            "{dialect:?}"
        );
    }
}

/// The half of Fast DDS's rule a plan cannot make.
///
/// `mut_lc_enum`, the same run. `sequence<Flags>` of two elements is
/// eight bytes and Fast DDS states it in the code, `01000030`. The count
/// is in the value, so the plan writes LC4 and `Writer::finish_emheader`
/// removes the NEXTINT once the length is known. A third element would
/// be ten bytes and stay LC4.
#[test]
fn a_sequence_that_happens_to_fit_takes_a_short_code_from_fast_dds() {
    let idl = "\
enum Colour { RED, GREEN, BLUE };
@bit_bound(16) bitmask Flags { @position(0) a, @position(3) b };
@mutable struct LenEnum { sequence<Colour> cs; sequence<Flags> fs; long tail; };";
    let two = json!({ "cs": ["GREEN", "BLUE"], "fs": [["a"], ["b"]], "tail": 7 });
    for (dialect, expected) in [
        (
            Dialect::Cyclone,
            "2c000000\
             00000050 0c000000 02000000 01000000 02000000\
             01000050 08000000 02000000 0100 0800\
             02000020 07000000",
        ),
        (
            Dialect::FastDds,
            "24000000\
             00000060 02000000 01000000 02000000\
             01000030 02000000 0100 0800\
             02000020 07000000",
        ),
    ] {
        let want: String = expected.chars().filter(|c| !c.is_whitespace()).collect();
        assert_eq!(
            bytes_in(dialect, idl, "LenEnum", &two, Encoding::PlCdr2),
            want,
            "{dialect:?}"
        );
    }

    // Three elements: ten bytes, no short code, LC4 with a NEXTINT.
    let three = json!({ "cs": ["GREEN"], "fs": [["a"], ["b"], ["a", "b"]], "tail": 7 });
    let bytes = bytes_in(Dialect::FastDds, idl, "LenEnum", &three, Encoding::PlCdr2);
    assert!(
        bytes.contains("010000400a000000"),
        "the ten-byte sequence did not stay at LC4: {bytes}"
    );
}

/// `M_FLAG` in the XCDR2 EMHEADER, which Fast DDS never sets.
///
/// `mut_mustunderstand`, both encoding versions. The flag is in three
/// different places — bit 14 of the short parameter id, `FLAG_2` of the
/// extended `eMemberHeader`, bit 31 of the EMHEADER — and Fast DDS omits
/// it from all three. `Dialect::FastDds` used to reproduce the XCDR1
/// half and set the bit under XCDR2, because the setting hung off
/// `PlCdr1HeaderPolicy` and XCDR2 never asked it.
#[test]
fn the_must_understand_flag_is_the_one_each_vendor_writes_in_both_versions() {
    let idl = "@mutable struct M { @id(1) @must_understand long a; @id(2) long b; };";
    let values = json!({ "a": 7, "b": -2 });
    for (dialect, xcdr2, xcdr1) in [
        (
            Dialect::Cyclone,
            "10000000010000a00700000002000020feffffff",
            "017f0800010000400400000007000000017f08000200000004000000feffffff027f0000",
        ),
        (
            Dialect::Rti,
            "10000000010000a00700000002000020feffffff",
            "014004000700000002000400feffffff027f0000",
        ),
        (
            Dialect::FastDds,
            "10000000010000200700000002000020feffffff",
            "010004000700000002000400feffffff023f0000",
        ),
    ] {
        assert_eq!(
            bytes_in(dialect, idl, "M", &values, Encoding::PlCdr2),
            xcdr2,
            "{dialect:?} XCDR2"
        );
        assert_eq!(
            bytes_in(dialect, idl, "M", &values, Encoding::PlCdr1),
            xcdr1,
            "{dialect:?} XCDR1"
        );
    }
}

// -- the PL_CDR1 header form of an unbounded member ---------------------

/// `mut_string`: the short header RTI and Fast DDS write for a `string`.
///
/// Run 2026-08-09 through `byte-gate/gate`,
/// `@mutable struct M { @id(1) string s; @id(2) long n; };` with
/// `s = "abc"`, `n = 7`, XCDR1 little endian. RTI and Fast DDS spend 24
/// bytes; Cyclone spends 40 because it writes `PID_EXTENDED` for every
/// member whatever the length.
///
/// This project spent 40 under all three dialects: the plan gave every
/// unbounded member the extended form, on the reasoning that it cannot
/// promise the value will fit sixteen bits. It cannot; the writer can,
/// after the value, and `Writer::finish_pid` does.
#[test]
fn an_unbounded_member_takes_the_header_form_its_vendor_writes() {
    let idl = "@mutable struct M { @id(1) string s; @id(2) long n; };";
    let values = json!({ "s": "abc", "n": 7 });
    for (dialect, expected) in [
        (
            Dialect::Cyclone,
            "017f0800 01000000 08000000 04000000 61626300 \
             017f0800 02000000 04000000 07000000 027f0000",
        ),
        (
            Dialect::Rti,
            "01000800 04000000 61626300 02000400 07000000 027f0000",
        ),
        (
            Dialect::FastDds,
            "01000800 04000000 61626300 02000400 07000000 023f0000",
        ),
    ] {
        let want: String = expected.chars().filter(|c| !c.is_whitespace()).collect();
        assert_eq!(
            bytes_in(dialect, idl, "M", &values, Encoding::PlCdr1),
            want,
            "{dialect:?}"
        );
    }
}

/// The fallback, end to end: a member that really does overrun.
///
/// The plan writes the short form and the writer rewrites it, so the
/// bytes are the extended form and the sample reads back. A `@mutable`
/// type whose member is over 64 KiB is the one case where RTI and Fast
/// DDS write twelve bytes of header too — §7.4.1.2.3 leaves them no
/// choice — and it is the case a plan-time answer could not tell from
/// the one above.
#[test]
fn a_member_that_overruns_the_short_header_still_goes_out_and_reads_back() {
    let idl = "@mutable struct Big { @id(1) sequence<octet> v; @id(2) long n; };";
    let ast = model_from_idl(idl, "probe.idl").expect("the model");
    let values = json!({ "v": vec![0xABu8; 0x1_0000], "n": 7 });
    // The marker's flags are `PidFlagPolicy`'s, asked at
    // `WirePosition::ExtendedMarker`, so the two dialects differ in that
    // one octet: RTI is `SentinelMustUnderstand` and writes `7f01`,
    // which is what its recorded `annotation-hashid-implicit` row
    // carries; Fast DDS is `Bare` and writes `3f01`, which is what its
    // row carries. Nothing measured covers a *promoted* marker
    // specifically — no recorded case has a member past sixteen bits —
    // and this is the axis's answer applied at the second site rather
    // than a second measurement.
    for (dialect, marker) in [(Dialect::Rti, "017f0800"), (Dialect::FastDds, "013f0800")] {
        let request =
            ProbeRequest::in_dialect("Big", Encoding::PlCdr1, Endianness::Little, dialect);
        let bytes = encode_request(&ast, &request, &values).expect("the probe");
        assert_eq!(
            &hex(&bytes)[..24],
            format!("{marker}0100000004000100"),
            "{dialect:?}: not the extended header"
        );
        let back = zerodds_wire_probe::decode::decode(&ast, &request, &bytes)
            .unwrap_or_else(|error| panic!("{dialect:?}: {error}"));
        assert_eq!(back["n"], json!(7), "{dialect:?}");
        assert_eq!(
            back["v"].as_array().map(Vec::len),
            Some(0x1_0000),
            "{dialect:?}"
        );
    }
}
