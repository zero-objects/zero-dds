// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-webdds-bridge` with `zerodds-wire-codec`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire-codec`:
//! - `src/gateway.rs`: `ProbeRequest::in_dialect` with the type's qualified
//!   name and the participant's dialect; a write always asks
//!   `Encoding::PlainCdr2` little endian, a read `PlainCdr1` or `PlainCdr2`
//!   by the sample's representation and its byte order, whatever the type's
//!   extensibility. Then `encode` of a `DynamicData` built from the XML body
//!   and `decode::<DynamicData>` of a received body.
//! - `src/dynamic_rpc.rs`: `encode_continuing` and
//!   `decode_continuing::<DynamicData>` for a value behind the first
//!   `start` octets of a DDS-RPC sample.
//!
//! So the crate relies on the codec taking only the XCDR version from the
//! encoding and the framing (DHEADER, EMHEADER) from the type, and on a
//! continuation aligning to the sample and returning the octets from
//! `start` on, padding included.
//!
//! The model is built from IDL with `zerodds-wire-probe` (a dependency of
//! this crate), as its own tests do.
//!
//! Oracle, octets written out by hand: DDS-XTypes 1.3 §7.4.1.1.1 (alignment
//! to the size under XCDR1, byte order), §7.4.2 (MAXALIGN 4 under XCDR2;
//! DELIMITED_CDR for APPENDABLE, PL_CDR2 for MUTABLE, an XCDR1 APPENDABLE
//! type not delimited), §7.4.3.4.2 (EMHEADER), DDS-RPC 1.0 §7.5.1.1.6 (the
//! `Call` behind a `RequestHeader`, aligned to the sample).
#![allow(clippy::unwrap_used)]

use zerodds_ast::PrimitiveKind;
use zerodds_ast_layout::Dialect;
use zerodds_ast_layout::key::{Encoding, Endianness};
use zerodds_types::dynamic::DynamicData;
use zerodds_wire_codec::{ProbeRequest, decode, decode_continuing, encode, encode_continuing};

const IDL: &str = "\
module to {
  @final struct Plain { long a; short b; };
  @appendable struct Appendable { long a; short b; };
  @mutable struct Mutable { @id(10) long a; @id(20) short b; };
  @final struct Reply { double d; };
};";

fn model() -> zerodds_ast::Ast {
    zerodds_wire_probe::encode::model_from_idl(IDL, "contract.idl").unwrap()
}

fn request(type_name: &str, encoding: Encoding, endianness: Endianness) -> ProbeRequest {
    ProbeRequest::in_dialect(type_name, encoding, endianness, Dialect::default())
}

/// `{ a = 7, b = -2 }`, as `dynxml::from_xml_document` builds it.
fn seven_minus_two() -> DynamicData {
    DynamicData::Aggregate(vec![
        (
            "a".to_string(),
            DynamicData::integer(PrimitiveKind::Int32, 7),
        ),
        (
            "b".to_string(),
            DynamicData::integer(PrimitiveKind::Int16, -2),
        ),
    ])
}

fn is_seven_minus_two(data: &DynamicData) -> bool {
    matches!(
        data.member("a"),
        Some(DynamicData::Integer { value: 7, .. })
    ) && matches!(
        data.member("b"),
        Some(DynamicData::Integer { value: -2, .. })
    )
}

#[test]
fn xtypes_7_4_2_a_write_under_plain_cdr2_frames_the_type_by_its_extensibility() {
    let ast = model();
    let cases: [(&str, &[u8]); 2] = [
        ("to::Plain", &[0x07, 0x00, 0x00, 0x00, 0xfe, 0xff]),
        (
            // The gateway asks PlainCdr2; the type is appendable, so a
            // DHEADER of 6 comes first.
            "to::Appendable",
            &[0x06, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xfe, 0xff],
        ),
    ];
    for (type_name, octets) in cases {
        let request = request(type_name, Encoding::PlainCdr2, Endianness::Little);
        assert_eq!(
            encode(&ast, &request, &seven_minus_two()).unwrap(),
            octets,
            "{type_name}"
        );
        let back: DynamicData = decode(&ast, &request, octets).unwrap();
        assert!(is_seven_minus_two(&back), "{type_name}: {back:?}");
    }
}

#[test]
fn xtypes_7_4_3_4_2_a_mutable_body_read_under_plain_cdr2_is_pl_cdr2() {
    let ast = model();
    let request = request("to::Mutable", Encoding::PlainCdr2, Endianness::Little);
    let octets = [
        0x0e, 0x00, 0x00, 0x00, // DHEADER: 14
        0x0a, 0x00, 0x00, 0x20, 0x07, 0x00, 0x00, 0x00, // LC 2, id 10: a
        0x14, 0x00, 0x00, 0x10, 0xfe, 0xff, // LC 1, id 20: b
    ];
    let data: DynamicData = decode(&ast, &request, &octets).unwrap();
    assert!(is_seven_minus_two(&data), "{data:?}");
}

#[test]
fn xtypes_7_4_1_1_1_a_read_follows_the_samples_representation_and_byte_order() {
    let ast = model();
    // XCDR2 big endian (`D_CDR2_BE`).
    let be: DynamicData = decode(
        &ast,
        &request("to::Appendable", Encoding::PlainCdr2, Endianness::Big),
        &[0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x07, 0xff, 0xfe],
    )
    .unwrap();
    assert!(is_seven_minus_two(&be), "{be:?}");
    // XCDR1 (`CDR_LE`): an appendable type is not delimited.
    let xcdr1: DynamicData = decode(
        &ast,
        &request("to::Appendable", Encoding::PlainCdr1, Endianness::Little),
        &[0x07, 0x00, 0x00, 0x00, 0xfe, 0xff],
    )
    .unwrap();
    assert!(is_seven_minus_two(&xcdr1), "{xcdr1:?}");
    // A body cut short is an error, not a value.
    let cut: Result<DynamicData, _> = decode(
        &ast,
        &request("to::Plain", Encoding::PlainCdr2, Endianness::Little),
        &[0x07, 0x00, 0x00, 0x00, 0xfe],
    );
    assert!(cut.is_err());
}

#[test]
fn rpc_7_5_1_1_6_a_value_behind_a_header_aligns_to_the_sample() {
    let ast = model();
    let value = DynamicData::Aggregate(vec![(
        "d".to_string(),
        DynamicData::float(PrimitiveKind::Float64, 1.5),
    )]);
    let one_and_a_half = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf8, 0x3f];
    // Behind 28 octets under XCDR1 the double aligns to 32: four octets of
    // padding come back with it.
    let xcdr1 = request("to::Reply", Encoding::PlainCdr1, Endianness::Little);
    let bytes = encode_continuing(&ast, &xcdr1, &value, 28).unwrap();
    let mut expected = vec![0x00; 4];
    expected.extend_from_slice(&one_and_a_half);
    assert_eq!(bytes, expected);
    let mut sample = vec![0xaa; 28];
    sample.extend_from_slice(&bytes);
    let back: DynamicData = decode_continuing(&ast, &xcdr1, &sample, 28).unwrap();
    assert!(
        matches!(back.member("d"), Some(DynamicData::Float { value, .. }) if *value == 1.5),
        "{back:?}"
    );
    // Under XCDR2 the cap is 4 and 28 is aligned: no padding.
    let xcdr2 = request("to::Reply", Encoding::PlainCdr2, Endianness::Little);
    assert_eq!(
        encode_continuing(&ast, &xcdr2, &value, 28).unwrap(),
        one_and_a_half
    );
}
