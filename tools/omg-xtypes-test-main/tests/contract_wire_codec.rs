// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-omg-xtypes-test-main` with `zerodds-wire-codec`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire-codec` (`src/carrier.rs`,
//! `src/data.rs`):
//! - `ProbeRequest::new(type, encoding, endianness).emitting(Emit::Payload)`
//!   with the encoding `-x` and the extensibility choose (`PlainCdr1`,
//!   `PlCdr1`, `PlainCdr2`, `DelimitedCdr`, `PlCdr2`), then
//!   `encode::<serde_json::Value>` for a published sample and
//!   `decode::<serde_json::Value>` for a received one.
//! - `ProbeError::Discarded` from `decode`: the sample is dropped
//!   (`DecodeError::Discarded`), every other error keeps the octets.
//! - `Emit::KeyHash` with `Endianness::Big`: the sixteen octets of the
//!   KeyHash, copied verbatim into the key holder.
//! - `default_of_type::<serde_json::Value>`: the type's default value, which
//!   the loaded expectation is laid over before comparing.
//!
//! The model is composed with `zerodds-idl-compose`, as the run composes an
//! IDL type file.
//!
//! Oracle, octets and digests written out by hand: DDS-XTypes 1.3 §7.4.1.1.1,
//! §7.4.2 (PLAIN_CDR2, DELIMITED_CDR, PL_CDR2), §7.4.3.4.2 (EMHEADER), §7.6.8
//! and DDSI-RTPS 2.5 §9.6.4.8 (the key holder is PLAIN_CDR2 big endian; at
//! most 16 octets of maximum size are zero-padded, more are MD5-hashed; the
//! MD5 computed outside this tree), §7.2.4.1 and §7.3.1.2.1.12 (a value the
//! reader's type cannot hold is discarded when `@try_construct` is absent),
//! §7.2.4.4.4.4 and Table 9 (a member the writer did not send takes its
//! default: 0 for an integer, "" for a string).
#![allow(clippy::unwrap_used, clippy::panic)]

use serde_json::{Value, json};
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_codec::{Emit, ProbeError, ProbeRequest, decode, default_of_type, encode};

const IDL: &str = "\
module to {
  @final struct Plain { long a; short b; };
  @appendable struct Appendable { long a; short b; };
  @mutable struct Mutable { @id(10) long a; @id(20) short b; };
  @final struct Bounded { string<4> s; };
  @final struct KeyedLong { @key long id; long value; };
  @final struct KeyedText { @key string color; long x; };
  @appendable struct ShortWriter { long x1; };
  @appendable struct LongReader { long x1; long x2; string x3; };
};";

fn model() -> zerodds_ast::Ast {
    let composed = zerodds_idl_compose::compose_source(
        "contract.idl",
        IDL,
        &zerodds_idl_compose::PathLoader::bare(),
    );
    assert!(!composed.failed(), "the IDL composes without an error");
    composed.ast
}

fn payload(type_name: &str, encoding: Encoding, endianness: Endianness) -> ProbeRequest {
    ProbeRequest::new(type_name, encoding, endianness).emitting(Emit::Payload)
}

#[test]
fn xtypes_7_4_2_a_published_value_is_the_octets_of_its_encoding() {
    let ast = model();
    let value = json!({ "a": 7, "b": -2 });
    let cases: [(&str, Encoding, Endianness, &[u8]); 4] = [
        (
            "to::Plain",
            Encoding::PlainCdr2,
            Endianness::Little,
            &[0x07, 0x00, 0x00, 0x00, 0xfe, 0xff],
        ),
        (
            "to::Plain",
            Encoding::PlainCdr1,
            Endianness::Big,
            &[0x00, 0x00, 0x00, 0x07, 0xff, 0xfe],
        ),
        (
            "to::Appendable",
            Encoding::DelimitedCdr,
            Endianness::Little,
            &[0x06, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xfe, 0xff],
        ),
        (
            // XCDR1 does not delimit an appendable type.
            "to::Appendable",
            Encoding::PlainCdr1,
            Endianness::Little,
            &[0x07, 0x00, 0x00, 0x00, 0xfe, 0xff],
        ),
    ];
    for (type_name, encoding, endianness, octets) in cases {
        let request = payload(type_name, encoding, endianness);
        assert_eq!(
            encode(&ast, &request, &value).unwrap(),
            octets,
            "{type_name} {encoding:?} {endianness:?}"
        );
        let back: Value = decode(&ast, &request, octets).unwrap();
        assert_eq!(back, value, "{type_name} {encoding:?} {endianness:?}");
    }
}

#[test]
fn xtypes_7_4_3_4_2_a_received_mutable_value_is_read_by_member_id() {
    let ast = model();
    let request = payload("to::Mutable", Encoding::PlCdr2, Endianness::Little);
    let octets = [
        0x0e, 0x00, 0x00, 0x00, // DHEADER: 14
        0x0a, 0x00, 0x00, 0x20, 0x07, 0x00, 0x00, 0x00, // LC 2, id 10: a
        0x14, 0x00, 0x00, 0x10, 0xfe, 0xff, // LC 1, id 20: b
    ];
    let value: Value = decode(&ast, &request, &octets).unwrap();
    assert_eq!(value, json!({ "a": 7, "b": -2 }));
}

#[test]
fn xtypes_7_2_4_1_a_string_past_the_readers_bound_is_discarded() {
    let ast = model();
    let request = payload("to::Bounded", Encoding::PlainCdr2, Endianness::Little);
    // "abcdef": six octets for a `string<4>`.
    let octets = [
        0x07, 0x00, 0x00, 0x00, b'a', b'b', b'c', b'd', b'e', b'f', 0x00,
    ];
    let result: Result<Value, ProbeError> = decode(&ast, &request, &octets);
    assert!(
        matches!(result, Err(ProbeError::Discarded { .. })),
        "{result:?}"
    );
    // Four fit.
    let fits = [0x05, 0x00, 0x00, 0x00, b'a', b'b', b'c', b'd', 0x00];
    let value: Value = decode(&ast, &request, &fits).unwrap();
    assert_eq!(value, json!({ "s": "abcd" }));
    // A stream cut short is an error of another kind: the octets are kept
    // and the comparison fails, the sample is not dropped as discarded.
    let cut: Result<Value, ProbeError> = decode(&ast, &request, &fits[..6]);
    assert!(
        matches!(cut, Err(ref e) if !matches!(e, ProbeError::Discarded { .. })),
        "{cut:?}"
    );
}

#[test]
fn rtps_9_6_4_8_the_key_hash_of_a_key_of_at_most_sixteen_octets_is_the_holder_zero_padded() {
    let ast = model();
    let request = ProbeRequest::new("to::KeyedLong", Encoding::PlainCdr2, Endianness::Big)
        .emitting(Emit::KeyHash);
    let key_hash = encode(&ast, &request, &json!({ "id": 5, "value": 9 })).unwrap();
    assert_eq!(
        key_hash,
        [0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        "the holder is `id` big endian"
    );
}

#[test]
fn rtps_9_6_4_8_the_key_hash_of_an_unbounded_key_is_the_md5_of_the_holder() {
    let ast = model();
    let request = ProbeRequest::new("to::KeyedText", Encoding::PlainCdr2, Endianness::Big)
        .emitting(Emit::KeyHash);
    let key_hash = encode(&ast, &request, &json!({ "color": "BLUE", "x": 7 })).unwrap();
    // MD5(00000005 424c554500) = cac217c318363f8ef1160eeedef9e886.
    assert_eq!(
        key_hash,
        [
            0xca, 0xc2, 0x17, 0xc3, 0x18, 0x36, 0x3f, 0x8e, 0xf1, 0x16, 0x0e, 0xee, 0xde, 0xf9,
            0xe8, 0x86,
        ]
    );
}

#[test]
fn xtypes_7_2_4_4_4_4_a_member_the_writer_did_not_send_takes_its_default() {
    let ast = model();
    // The writer's `ShortWriter { x1 = 1 }`, read with the longer type.
    let request = payload("to::LongReader", Encoding::DelimitedCdr, Endianness::Little);
    let octets = [0x04, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
    let value: Value = decode(&ast, &request, &octets).unwrap();
    assert_eq!(value, json!({ "x1": 1, "x2": 0, "x3": "" }));

    // The default value the expectation is laid over.
    let id = ast
        .types()
        .find(|(_, node)| node.name == "LongReader")
        .map(|(id, _)| id)
        .unwrap();
    let default: Value = default_of_type(&ast, id, "to::LongReader").unwrap();
    assert_eq!(default, json!({ "x1": 0, "x2": 0, "x3": "" }));
}
