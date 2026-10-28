// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! RTI Connext's PL_CDR1 parameter headers in a sample whose type can
//! exceed what a short header states, against RTI's own octets.
//!
//! Every row is what `tools/wireprobe/wireprobe-rti` produced on the
//! Linux test host from rtiddsgen's type support for the IDL and values
//! beside it (RTI Connext 7.7.0, `PRESTypePlugin_interpretedSerialize`),
//! XCDR1. The pairs either side of a step are the last bound with every
//! header short and the first with `PID_EXTENDED` on the members whose
//! type is not a primitive or an enumeration.
//!
//! Each row is written under `Dialect::Rti` and compared octet for
//! octet, and RTI's octets are read under the same dialect and written
//! again: the reader does not depend on which form the writer chose,
//! and what it reads is the sample.
#![allow(clippy::expect_used, clippy::panic)]

use zerodds_ast_layout::{Dialect, Encoding, Endianness};
use zerodds_wire_probe::decode::decode;
use zerodds_wire_probe::encode::{encode, hex, model_from_idl_in_dialect};
use zerodds_wire_probe::value::ProbeRequest;

struct Row {
    name: &'static str,
    idl: String,
    type_name: &'static str,
    values: &'static str,
    endianness: Endianness,
    rti: &'static str,
}

fn mutable_three(n: u32) -> String {
    format!(
        "module m {{ @mutable struct T {{ long id; sequence<long, 100> small; sequence<octet, {n}> mid; }}; }};"
    )
}

fn mutable_two(n: u32) -> String {
    format!("module m {{ @mutable struct T {{ long id; sequence<octet, {n}> mid; }}; }};")
}

fn with_double(n: u32) -> String {
    format!("module m {{ @mutable struct T {{ long id; double d; sequence<octet, {n}> mid; }}; }};")
}

fn with_string(n: u32) -> String {
    format!("module m {{ @mutable struct T {{ long id; string s; sequence<octet, {n}> mid; }}; }};")
}

fn with_unbounded_sequence(n: u32) -> String {
    format!(
        "module m {{ @mutable struct T {{ long id; sequence<long> small; sequence<octet, {n}> mid; }}; }};"
    )
}

fn appendable(n: u32) -> String {
    format!(
        "module m {{ @appendable struct T {{ long id; @optional sequence<long, 100> small_opt; sequence<octet, {n}> big; }}; }};"
    )
}

const KINDS: &str = "module m {
  enum Color { RED, GREEN };
  typedef long LongAlias;
  @mutable struct T {
    boolean b; octet o; char c; short s; unsigned short us; long l; unsigned long ul;
    long long ll; unsigned long long ull; float f; double d; Color e; LongAlias al;
    sequence<octet, 65532> big;
  };
};";

const KINDS_VALUES: &str = r#"{"b":true,"o":1,"c":"x","s":2,"us":3,"l":4,"ul":5,"ll":6,"ull":7,"f":1.5,"d":2.5,"e":"GREEN","al":8,"big":[9]}"#;

const UNION: &str = "module m {
  @mutable union U switch (long) { case 1: long a; case 2: sequence<long, 100> s; case 3: sequence<octet, 65532> big; };
  @mutable struct T { long id; U choice; };
};";

const WIDE: &str = "module m {
  typedef sequence<string> Names;
  @mutable struct Wide {
    long id;
    sequence<long, 100> small;
    sequence<octet, 65531> edge_in;
    sequence<octet, 65532> edge_out;
    sequence<long, 20000> big;
    sequence<Names> nested;
    @optional sequence<long, 20000> big_opt;
    @optional sequence<long, 100> small_opt;
  };
};";

const WIDE_VALUES: &str = r#"{"id": 1, "small": [2], "edge_in": [3], "edge_out": [4], "big": [5], "nested": [["a"]], "big_opt": [6], "small_opt": [7]}"#;

#[allow(clippy::too_many_lines)]
fn rows() -> Vec<Row> {
    let le = Endianness::Little;
    let row = |name, idl: String, type_name, values, endianness, rti| Row {
        name,
        idl,
        type_name,
        values,
        endianness,
        rti,
    };
    vec![
        row(
            "three members, last short",
            mutable_three(65068),
            "m::T",
            r#"{"id":1,"small":[2],"mid":[7]}"#,
            le,
            "0000040001000000010008000100000002000000020008000100000007000000027f0000",
        ),
        row(
            "three members, first extended",
            mutable_three(65069),
            "m::T",
            r#"{"id":1,"small":[2],"mid":[7]}"#,
            le,
            "0000040001000000017f080001000000080000000100000002000000017f080002000000080000000100000007000000027f0000",
        ),
        row(
            "two members, last short",
            mutable_two(65484),
            "m::T",
            r#"{"id":1,"mid":[7]}"#,
            le,
            "0000040001000000010008000100000007000000027f0000",
        ),
        row(
            "two members, first extended",
            mutable_two(65485),
            "m::T",
            r#"{"id":1,"mid":[7]}"#,
            le,
            "0000040001000000017f080001000000080000000100000007000000027f0000",
        ),
        row(
            "a double, last short",
            with_double(65464),
            "m::T",
            r#"{"id":1,"d":1.5,"mid":[7]}"#,
            le,
            "000004000100000001000800000000000000f83f020008000100000007000000027f0000",
        ),
        row(
            "a double, first extended",
            with_double(65465),
            "m::T",
            r#"{"id":1,"d":1.5,"mid":[7]}"#,
            le,
            "000004000100000001000800000000000000f83f017f080002000000080000000100000007000000027f0000",
        ),
        row(
            "an unbounded string, last short",
            with_string(65212),
            "m::T",
            r#"{"id":1,"s":"ab","mid":[7]}"#,
            le,
            "0000040001000000010008000300000061620000020008000100000007000000027f0000",
        ),
        row(
            "an unbounded string, first extended",
            with_string(65213),
            "m::T",
            r#"{"id":1,"s":"ab","mid":[7]}"#,
            le,
            "0000040001000000017f080001000000080000000300000061620000017f080002000000080000000100000007000000027f0000",
        ),
        row(
            "an unbounded sequence, last short",
            with_unbounded_sequence(65068),
            "m::T",
            r#"{"id":1,"small":[2],"mid":[7]}"#,
            le,
            "0000040001000000010008000100000002000000020008000100000007000000027f0000",
        ),
        row(
            "an unbounded sequence, first extended",
            with_unbounded_sequence(65069),
            "m::T",
            r#"{"id":1,"small":[2],"mid":[7]}"#,
            le,
            "0000040001000000017f080001000000080000000100000002000000017f080002000000080000000100000007000000027f0000",
        ),
        row(
            "@optional in @appendable, last short",
            appendable(65107),
            "m::T",
            r#"{"id":1,"small_opt":[2],"big":[9]}"#,
            le,
            "010000000100080001000000020000000100000009",
        ),
        row(
            "@optional in @appendable, first extended",
            appendable(65108),
            "m::T",
            r#"{"id":1,"small_opt":[2],"big":[9]}"#,
            le,
            "01000000017f0800010000000800000001000000020000000100000009",
        ),
        row(
            "primitives, an enum and a typedef keep the short header",
            KINDS.to_owned(),
            "m::T",
            KINDS_VALUES,
            le,
            "0000040001000000010004000100000002000400780000000300040002000000040004000300000005000400040000000600040005000000070008000600000000000000080008000700000000000000090004000000c03f0a00080000000000000004400b000400010000000c00040008000000017f08000d000000080000000100000009000000027f0000",
        ),
        row(
            "primitives, an enum and a typedef keep the short header, big endian",
            KINDS.to_owned(),
            "m::T",
            KINDS_VALUES,
            Endianness::Big,
            "0000000401000000000100040100000000020004780000000003000400020000000400040003000000050004000000040006000400000005000700080000000000000006000800080000000000000007000900043fc00000000a00084004000000000000000b000400000001000c0004000000087f0100080000000d0000000800000001090000007f020000",
        ),
        row(
            "wchar, int8 and uint8 keep the short header",
            "module m { @mutable struct T { wchar w; int8 i8; uint8 u8; sequence<octet, 65532> big; }; };".to_owned(),
            "m::T",
            r#"{"w":"y","i8":-1,"u8":2,"big":[9]}"#,
            le,
            "000004007900000001000400ff0000000200040002000000017f080003000000080000000100000009000000027f0000",
        ),
        row(
            "a union past the step",
            "module m { @mutable union U switch (long) { case 1: long a; case 2: sequence<long, 100> s; case 3: sequence<octet, 65532> big; }; };".to_owned(),
            "m::U",
            r#"{"_d":2,"s":[5]}"#,
            le,
            "0000040002000000017f080002000000080000000100000005000000027f0000",
        ),
        row(
            "a union within it",
            "module m { @mutable union U switch (long) { case 1: long a; case 2: sequence<long, 100> s; }; };".to_owned(),
            "m::U",
            r#"{"_d":2,"s":[5]}"#,
            le,
            "0000040002000000020008000100000005000000027f0000",
        ),
        row(
            "a union as a member",
            UNION.to_owned(),
            "m::T",
            r#"{"id":1,"choice":{"_d":2,"s":[4]}}"#,
            le,
            "0000040001000000017f080001000000200000000000040002000000017f080002000000080000000100000004000000027f0000027f0000",
        ),
        row(
            "a type small on its own, framed by its sample",
            "module m { @mutable struct Inner { long a; sequence<long, 100> s; }; @mutable struct T { long id; Inner part; sequence<octet, 65532> big; }; };".to_owned(),
            "m::T",
            r#"{"id":1,"part":{"a":2,"s":[3]},"big":[4]}"#,
            le,
            "0000040001000000017f080001000000200000000000040002000000017f080001000000080000000100000003000000027f0000017f080002000000080000000100000004000000027f0000",
        ),
        row(
            "sequences of every size",
            WIDE.to_owned(),
            "m::Wide",
            WIDE_VALUES,
            le,
            "0000040001000000017f080001000000080000000100000002000000017f080002000000080000000100000003000000017f080003000000080000000100000004000000017f080004000000080000000100000005000000017f0800050000001000000001000000010000000200000061000000017f080006000000080000000100000006000000017f080007000000080000000100000007000000027f0000",
        ),
        row(
            "sequences of every size, big endian",
            WIDE.to_owned(),
            "m::Wide",
            WIDE_VALUES,
            Endianness::Big,
            "00000004000000017f010008000000010000000800000001000000027f010008000000020000000800000001030000007f010008000000030000000800000001040000007f010008000000040000000800000001000000057f0100080000000500000010000000010000000100000002610000007f010008000000060000000800000001000000067f010008000000070000000800000001000000077f020000",
        ),
    ]
}

fn request(row: &Row) -> ProbeRequest {
    ProbeRequest::in_dialect(
        row.type_name,
        Encoding::PlainCdr1,
        row.endianness,
        Dialect::Rti,
    )
}

fn octets(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("hex"))
        .collect()
}

#[test]
fn rti_frames_a_sample_by_its_type_and_the_dialect_writes_the_same() {
    let mut differing = Vec::new();
    for row in rows() {
        let ast = model_from_idl_in_dialect(&row.idl, "type.idl", Dialect::Rti)
            .unwrap_or_else(|e| panic!("{}: {e:?}", row.name));
        let values: serde_json::Value = serde_json::from_str(row.values).expect("values");
        let written =
            encode(&ast, &request(&row), &values).unwrap_or_else(|e| panic!("{}: {e:?}", row.name));
        if hex(&written) != row.rti {
            differing.push(format!(
                "{}\n  rti  {}\n  ours {}",
                row.name,
                row.rti,
                hex(&written)
            ));
        }
    }
    assert!(differing.is_empty(), "{}", differing.join("\n"));
}

#[test]
fn rti_s_octets_read_back_to_the_same_octets() {
    for row in rows() {
        let ast = model_from_idl_in_dialect(&row.idl, "type.idl", Dialect::Rti)
            .unwrap_or_else(|e| panic!("{}: {e:?}", row.name));
        let read = decode(&ast, &request(&row), &octets(row.rti))
            .unwrap_or_else(|e| panic!("{}: reading RTI's octets: {e:?}", row.name));
        let again =
            encode(&ast, &request(&row), &read).unwrap_or_else(|e| panic!("{}: {e:?}", row.name));
        assert_eq!(hex(&again), row.rti, "{}", row.name);
    }
}
