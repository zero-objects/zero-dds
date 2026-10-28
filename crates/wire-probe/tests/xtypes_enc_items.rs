// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Items of DDS-XTypes 1.3 §7.2 and §7.4 that name a byte string or a
//! construction rule, checked against the clause and not against the
//! output of an earlier run.
//!
//! Every expectation below is worked out from the clause quoted at the
//! test. None of them is a vendor recording; where a recorded vendor row
//! differs, the test says which dialect reproduces it.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::json;
use zerodds_ast_layout::{Dialect, Encoding, Endianness};
use zerodds_wire_probe::canonical::text;
use zerodds_wire_probe::decode::{decode, unprobe};
use zerodds_wire_probe::encode::{ProbeError, encode, hex, model_from_idl, probe};
use zerodds_wire_probe::value::{Emit, EncapsulationIds, ProbeRequest, encapsulation_form};

/// The document of `{"m": value}`: a 64-bit integer is a string there, as
/// the codec's JSON does not hold it exactly otherwise.
fn json_integer(bound: u32, value: u64) -> String {
    if bound == 64 {
        format!(r#"{{"m":"{value}"}}"#)
    } else {
        format!(r#"{{"m":{value}}}"#)
    }
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("hex"))
        .collect()
}

fn bytes_in(
    dialect: Dialect,
    idl: &str,
    type_name: &str,
    values: &serde_json::Value,
    encoding: Encoding,
) -> String {
    let ast = model_from_idl(idl, "probe.idl").expect("the model");
    let request = ProbeRequest::in_dialect(type_name, encoding, Endianness::Little, dialect);
    hex(&encode(&ast, &request, values).expect("the probe"))
}

// -- §7.4.3.5.2: an absent optional member, under PL_CDR -----------------

/// §7.4.3.5.2: "PL_CDR and PL_CDR2 serialize optional members as it would
/// with regular members except that if the optional member is not
/// present, then the corresponding member header and serialized member are
/// omitted from the serialized stream."
///
/// `a` is member id 0 and `b` is member id 1. Under PL_CDR (§7.4.1.2) a
/// parameter is its id, its length and the value; the list ends with
/// `PID_LIST_END` (`0x3F02` with `FLAG_MUST_UNDERSTAND`, Table 34).
#[test]
fn an_absent_optional_member_has_no_parameter_under_pl_cdr() {
    let idl = "@mutable struct MutOpt { long a; @optional long b; };";
    let absent = probe(
        idl,
        "MutOpt",
        &json!({ "a": 7 }),
        Encoding::PlCdr1,
        Endianness::Little,
    )
    .expect("the probe");
    assert_eq!(
        hex(&absent),
        "0000040007000000027f0000",
        "a parameter was written for the absent member"
    );
    let present = probe(
        idl,
        "MutOpt",
        &json!({ "a": 7, "b": 9 }),
        Encoding::PlCdr1,
        Endianness::Little,
    )
    .expect("the probe");
    assert_eq!(hex(&present), "00000400070000000100040009000000027f0000");
    // And it is read back as absent, and as present.
    assert_eq!(
        text(
            &unprobe(idl, "MutOpt", &absent, Encoding::PlCdr1, Endianness::Little)
                .expect("it reads")
        ),
        r#"{"a":7}"#
    );
    assert_eq!(
        text(
            &unprobe(
                idl,
                "MutOpt",
                &present,
                Encoding::PlCdr1,
                Endianness::Little
            )
            .expect("it reads")
        ),
        r#"{"a":7,"b":9}"#
    );
}

// -- §7.2.2.4.4.4.6 and §7.4.3.4.2: the discriminator of a @mutable union --

/// §7.2.2.4.4.4.6: "In a union type, the discriminator member shall always
/// have the 'must understand' attribute set to true." The discriminator of
/// a `@mutable` union has a member header (§7.4.3.5.3 rules (27) and
/// (28)), and the flag of that header is the attribute: `M_FLAG`, bit 31
/// of the EMHEADER (§7.4.3.4.2), and `FLAG_MUST_UNDERSTAND`, bit 14 of the
/// parameter id (§7.4.1.2.1).
///
/// The discriminator is member id 0 and a `long`, so its EMHEADER is
/// `M_FLAG | LC2 | 0` = `0xA0000000`; the selected member `whole` is id 1
/// at LC2 without the flag. Under version 1 the discriminator's parameter
/// id is `0x4000`, `whole`'s is `0x0001`, and the list ends with `0x7F02`.
#[test]
fn the_discriminator_of_a_mutable_union_is_must_understand() {
    let idl = "@mutable union Choice switch (long) {
                   case 1: long whole;
                   default: double frac;
               };
               @final struct Sample { Choice picked; long tail; };";
    let value = json!({ "picked": { "_d": 1, "whole": 7 }, "tail": -2 });
    let ast = model_from_idl(idl, "mut_union.idl").expect("the model");

    let xcdr2 = "10000000000000a0010000000100002007000000feffffff";
    let xcdr1 = "00400400010000000100040007000000027f0000feffffff";
    for (encoding, expected) in [(Encoding::PlainCdr2, xcdr2), (Encoding::PlainCdr1, xcdr1)] {
        let request = ProbeRequest::new("Sample", encoding, Endianness::Little);
        let written = encode(&ast, &request, &value).expect("it serialises");
        assert_eq!(hex(&written), expected, "{encoding:?}");
        assert_eq!(
            text(&decode(&ast, &request, &written).expect("it reads back")),
            r#"{"picked":{"_d":1,"whole":7},"tail":-2}"#,
            "{encoding:?}"
        );
    }

    // RTI and OpenDDS were measured writing the bit clear
    // (`vendor-corpus.tsv`, `union-mutable`); their dialects keep that
    // and the reader of the specification dialect takes it.
    let rti2 = "1000000000000020010000000100002007000000feffffff";
    let rti1 = "00000400010000000100040007000000027f0000feffffff";
    for (encoding, expected) in [(Encoding::PlainCdr2, rti2), (Encoding::PlainCdr1, rti1)] {
        assert_eq!(
            bytes_in(Dialect::Rti, idl, "Sample", &value, encoding),
            expected,
            "{encoding:?}"
        );
        let request = ProbeRequest::new("Sample", encoding, Endianness::Little);
        assert_eq!(
            text(&decode(&ast, &request, &unhex(expected)).expect("a clear bit is read")),
            r#"{"picked":{"_d":1,"whole":7},"tail":-2}"#,
            "{encoding:?}"
        );
    }
}

// -- §7.2.2.4.7: KeyHolder of a union that is the type of a key member ----

/// §7.2.2.4.7: "If T is a union and the discriminator is not marked as
/// key, then KeyHolder(T) is the same type T." A key member whose type is
/// such a union therefore contributes the whole union to the holder, the
/// discriminator and the selected member (§7.6.8 step 3: PLAIN_CDR2, big
/// endian). The member that is not a key does not appear.
///
/// Holder for `u = {_d: 1, a: 7}`: `00000001 00000007`; for
/// `u = {_d: 2, b: 5}`: `00000002 0005`. Both are at most sixteen octets,
/// so the KeyHash is the holder zero-padded (§7.6.8 step 5).
#[test]
fn a_union_as_the_type_of_a_key_member_is_in_the_holder_whole() {
    let idl = "@final union Alt switch (long) { case 1: long a; case 2: short b; };
               @final struct S { @key Alt u; long other; };";
    let ast = model_from_idl(idl, "probe.idl").expect("the model");
    let request =
        ProbeRequest::new("S", Encoding::PlainCdr2, Endianness::Little).emitting(Emit::KeyHash);
    let hash = |value: serde_json::Value| hex(&encode(&ast, &request, &value).expect("a hash"));
    assert_eq!(
        hash(json!({ "u": { "_d": 1, "a": 7 }, "other": 3 })),
        "00000001000000070000000000000000"
    );
    assert_eq!(
        hash(json!({ "u": { "_d": 2, "b": 5 }, "other": 3 })),
        "00000002000500000000000000000000"
    );
    // The member that is not a key does not move the hash.
    assert_eq!(
        hash(json!({ "u": { "_d": 1, "a": 7 }, "other": 99 })),
        "00000001000000070000000000000000"
    );
}

// -- Table 15: the integer a bitmask constructs ----------------------------

/// Table 15: a `UINT8`..`UINT64` type is assignable from a `BITMASK_TYPE`
/// whose bound lies in 1..8, 9..16, 17..32, 33..64, and the object is built
/// as "for each bitflag that is set in the bitmask construct the integer
/// value (1 << position) using the position of that bitflag. Add all those
/// integer values to obtain the resulting object O1 of type T1".
///
/// Table 33 serializes a bitmask as the unsigned integer of its bound, so
/// the construction is reading the bytes of the bitmask as that integer.
#[test]
fn an_unsigned_integer_is_built_from_the_flags_of_a_bitmask() {
    let cases = [
        (
            8,
            "uint8",
            "@bit_bound(8) bitmask Mask { A, @position(3) B, @position(7) C };",
            137u64,
        ),
        (
            16,
            "uint16",
            "@bit_bound(16) bitmask Mask { A, @position(9) B, @position(15) C };",
            33_281,
        ),
        (
            32,
            "uint32",
            "@bit_bound(32) bitmask Mask { A, @position(17) B, @position(31) C };",
            2_147_614_721,
        ),
        (
            64,
            "uint64",
            "@bit_bound(64) bitmask Mask { A, @position(40) B, @position(63) C };",
            9_223_373_136_366_403_585,
        ),
    ];
    for (bound, integer, declaration, expected) in cases {
        let writer = format!("{declaration} @final struct W {{ Mask m; }};");
        let reader = format!("@final struct R {{ {integer} m; }};");
        for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
            for endianness in [Endianness::Little, Endianness::Big] {
                let written = probe(
                    &writer,
                    "W",
                    &json!({ "m": ["A", "B", "C"] }),
                    encoding,
                    endianness,
                )
                .expect("the bitmask writes");
                let read = unprobe(&reader, "R", &written, encoding, endianness)
                    .expect("the integer reads the bitmask's octets");
                assert_eq!(
                    text(&read),
                    json_integer(bound, expected),
                    "bit_bound {bound}, {encoding:?}, {endianness:?}"
                );
            }
        }
        // The set of flags in one place is the sum of their `1 << position`.
        let written = probe(
            &writer,
            "W",
            &json!({ "m": ["B"] }),
            Encoding::PlainCdr2,
            Endianness::Little,
        )
        .expect("the bitmask writes");
        let read = unprobe(
            &reader,
            "R",
            &written,
            Encoding::PlainCdr2,
            Endianness::Little,
        )
        .expect("it reads");
        let position = match bound {
            8 => 3,
            16 => 9,
            32 => 17,
            _ => 40,
        };
        assert_eq!(text(&read), json_integer(bound, 1u64 << position));
    }
}

// -- §7.2.4.4.5.1: a string longer than the bound constructs nothing -------

/// §7.2.4.4.5.1: "any T2 string object containing more characters than the
/// bound of the T1 string type cannot construct any object of type T1 in
/// order to prevent data misinterpretations resulting from truncations."
/// Table 16: O2 can construct T1 iff `O2.length <= T1.length`.
#[test]
fn a_string_longer_than_the_bound_of_the_reader_constructs_no_object() {
    let writer = "@final struct W { string<10> s; };";
    let reader = "@final struct R { string<5> s; };";
    let long = probe(
        writer,
        "W",
        &json!({ "s": "catsup" }),
        Encoding::PlainCdr2,
        Endianness::Little,
    )
    .expect("the string writes");
    assert!(
        matches!(
            unprobe(reader, "R", &long, Encoding::PlainCdr2, Endianness::Little),
            Err(ProbeError::Discarded { .. })
        ),
        "six characters constructed a string<5>"
    );
    for fits in ["cat", "catfd", ""] {
        let short = probe(
            writer,
            "W",
            &json!({ "s": fits }),
            Encoding::PlainCdr2,
            Endianness::Little,
        )
        .expect("the string writes");
        assert_eq!(
            text(
                &unprobe(reader, "R", &short, Encoding::PlainCdr2, Endianness::Little)
                    .expect("it constructs")
            ),
            format!(r#"{{"s":"{fits}"}}"#)
        );
    }
}

// -- §7.4.3.4 Table 39 and §7.4.3.2.3: the encapsulation header ----------

/// §7.4.3.4, Table 39 (`ENC_HEADER`): `{0x00, 0x10}` PLAIN_CDR2 big endian,
/// `{0x00, 0x11}` PLAIN_CDR2 little endian, `{0x00, 0x12}` and
/// `{0x00, 0x13}` PL_CDR2, `{0x00, 0x14}` and `{0x00, 0x15}` DELIMIT_CDR,
/// `{0x01, 0x00}` XML; version 1 keeps `{0x00, 0x00}` to `{0x00, 0x03}`.
/// §7.4.3.2.3 and rule (1): these two octets, then the options, then the
/// object.
#[test]
fn the_header_names_the_identifiers_of_table_39_when_asked() {
    let rows = [
        (Encoding::PlainCdr2, Endianness::Big, [0x00, 0x10]),
        (Encoding::PlainCdr2, Endianness::Little, [0x00, 0x11]),
        (Encoding::PlCdr2, Endianness::Big, [0x00, 0x12]),
        (Encoding::PlCdr2, Endianness::Little, [0x00, 0x13]),
        (Encoding::DelimitedCdr, Endianness::Big, [0x00, 0x14]),
        (Encoding::DelimitedCdr, Endianness::Little, [0x00, 0x15]),
        (Encoding::PlainCdr1, Endianness::Big, [0x00, 0x00]),
        (Encoding::PlainCdr1, Endianness::Little, [0x00, 0x01]),
        (Encoding::PlCdr1, Endianness::Big, [0x00, 0x02]),
        (Encoding::PlCdr1, Endianness::Little, [0x00, 0x03]),
    ];
    let idl = "@appendable struct S { long a; };";
    let ast = model_from_idl(idl, "probe.idl").expect("the model");
    let value = json!({ "a": 7 });
    for (encoding, endianness, identifier) in rows {
        let request = ProbeRequest::new("S", encoding, endianness)
            .emitting(Emit::PayloadWithEncapsulation)
            .with_encapsulation_ids(EncapsulationIds::Table39);
        let written = encode(&ast, &request, &value).expect("it serialises");
        assert_eq!(
            &written[..2],
            &identifier,
            "{encoding:?} {endianness:?}: {}",
            hex(&written)
        );
        assert_eq!(&written[2..4], &[0, 0], "options: no padding after a long");
        assert_eq!(
            encapsulation_form(identifier),
            Some((encoding, endianness)),
            "{identifier:02x?}"
        );
        assert_eq!(
            text(&decode(&ast, &request, &written).expect("it reads back")),
            r#"{"a":7}"#
        );
    }
    // The XML identifier of the table.
    assert_eq!(EncapsulationIds::Table39.xml_identifier(), [0x01, 0x00]);
    assert_eq!(EncapsulationIds::Table60.xml_identifier(), [0x00, 0x04]);
}

/// Table 60 stays the default, because Cyclone DDS, RTI Connext and
/// Fast DDS write its identifiers, and a reader takes both tables.
#[test]
fn the_default_header_is_table_60_and_both_tables_are_read() {
    let idl = "@appendable struct S { long a; };";
    let ast = model_from_idl(idl, "probe.idl").expect("the model");
    let value = json!({ "a": 7 });
    let rows = [
        (
            Encoding::PlainCdr2,
            Endianness::Big,
            [0x00, 0x06],
            [0x00, 0x10],
        ),
        (
            Encoding::PlainCdr2,
            Endianness::Little,
            [0x00, 0x07],
            [0x00, 0x11],
        ),
        (
            Encoding::DelimitedCdr,
            Endianness::Big,
            [0x00, 0x08],
            [0x00, 0x14],
        ),
        (
            Encoding::DelimitedCdr,
            Endianness::Little,
            [0x00, 0x09],
            [0x00, 0x15],
        ),
        (
            Encoding::PlCdr2,
            Endianness::Big,
            [0x00, 0x0a],
            [0x00, 0x12],
        ),
        (
            Encoding::PlCdr2,
            Endianness::Little,
            [0x00, 0x0b],
            [0x00, 0x13],
        ),
    ];
    for (encoding, endianness, table60, table39) in rows {
        let request =
            ProbeRequest::new("S", encoding, endianness).emitting(Emit::PayloadWithEncapsulation);
        let written = encode(&ast, &request, &value).expect("it serialises");
        assert_eq!(&written[..2], &table60, "{encoding:?} {endianness:?}");
        assert_eq!(encapsulation_form(table60), Some((encoding, endianness)));
        assert_eq!(encapsulation_form(table39), Some((encoding, endianness)));
    }
    assert_eq!(encapsulation_form([0x01, 0x00]), None, "XML is no CDR form");
    assert_eq!(encapsulation_form([0x00, 0x04]), None, "XML is no CDR form");
    assert_eq!(encapsulation_form([0x00, 0x05]), None);
    assert_eq!(
        EncapsulationIds::parse("table-39"),
        Some(EncapsulationIds::Table39)
    );
    assert_eq!(EncapsulationIds::parse("Table39"), None);
}
