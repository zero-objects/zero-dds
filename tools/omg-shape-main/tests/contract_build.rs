// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-omg-shape-main` with `zerodds-build`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-build`: its build script compiles
//! `interop/shapes/omg-dds-rtps.idl` with `Config::new()`, the include
//! directory, `default_extensibility(DefaultExt::Appendable)` and
//! `dds_type_impls(true)`, and `src/shape_type.rs` includes the generated
//! module and writes and reads `ShapeType` through the `DdsType` impl it
//! holds. The contract includes the same module, which the package's build
//! script wrote into `OUT_DIR`, and holds it to what the tool relies on.
//!
//! Oracle: the suite's IDL (`@extensibility(APPENDABLE)`, `@key string<128>
//! color`, the type name `ShapeType` the other suite executables announce)
//! and DDS-XTypes 1.3 §7.4.3.4.2 (an APPENDABLE aggregate under XCDR2 is
//! DELIMITED_CDR: a DHEADER with the length of what follows), §7.4.3.5.3
//! (a string is its length with the NUL, the octets, the NUL) and §7.4.3.5.4
//! (a sequence of a primitive carries no DHEADER), every value aligned to
//! its size up to four (§7.4.1.1.1, MAXALIGN 4 under XCDR2).
#![allow(clippy::unwrap_used, clippy::panic, dead_code)]

mod generated {
    include!(concat!(env!("OUT_DIR"), "/omg-dds-rtps.rs"));
}

use generated::ShapeType;
use zerodds_dcps::DdsType;

fn blue() -> ShapeType {
    ShapeType {
        color: "BLUE".into(),
        x: 10,
        y: 20,
        shapesize: 30,
        additional_payload_size: Vec::new(),
    }
}

/// `BLUE`, 10, 20, 30, an empty filler: DHEADER 28, the string (5 with the
/// NUL, padded to 16), three longs, the sequence's length 0.
const BLUE_XCDR2_LE: [u8; 32] = [
    0x1c, 0, 0, 0, 5, 0, 0, 0, b'B', b'L', b'U', b'E', 0, 0, 0, 0, 10, 0, 0, 0, 20, 0, 0, 0, 30, 0,
    0, 0, 0, 0, 0, 0,
];

#[test]
fn xtypes_7_4_3_4_2_the_suites_shape_type_is_delimited_and_written_as_the_clause_lays_it_out() {
    let mut out = Vec::new();
    DdsType::encode(&blue(), &mut out).unwrap();
    assert_eq!(out, BLUE_XCDR2_LE);
}

#[test]
fn xtypes_7_4_3_4_2_the_octets_of_the_clause_read_back_as_the_sample() {
    let back = <ShapeType as DdsType>::decode(&BLUE_XCDR2_LE).unwrap();
    assert_eq!(back.color, "BLUE");
    assert_eq!((back.x, back.y, back.shapesize), (10, 20, 30));
    assert!(back.additional_payload_size.is_empty());
}

#[test]
fn xtypes_7_4_3_4_2_the_big_endian_pair_writes_the_same_layout_in_its_own_byte_order() {
    let mut out = Vec::new();
    blue().encode_be(&mut out).unwrap();
    assert_eq!(
        out,
        [
            0, 0, 0, 0x1c, 0, 0, 0, 5, b'B', b'L', b'U', b'E', 0, 0, 0, 0, 0, 0, 0, 10, 0, 0, 0,
            20, 0, 0, 0, 30, 0, 0, 0, 0
        ]
    );
    assert_eq!(ShapeType::decode_be(&out).unwrap().x, 10);
}

#[test]
fn the_type_is_announced_by_the_name_and_with_the_key_and_extensibility_of_the_suites_idl() {
    assert_eq!(<ShapeType as DdsType>::TYPE_NAME, "ShapeType");
    assert_eq!(
        <ShapeType as DdsType>::EXTENSIBILITY,
        zerodds_dcps::dds_type::Extensibility::Appendable
    );
    assert!(<ShapeType as DdsType>::has_key());
    assert!(
        <ShapeType as DdsType>::type_object().is_some(),
        "the generated impl carries the TypeObject (DDS-XTypes 1.3 §7.6.3.2)"
    );
}

#[test]
fn xtypes_7_4_3_5_4_a_filler_payload_is_a_length_and_the_octets() {
    let mut shape = blue();
    shape.additional_payload_size = vec![7, 8, 9];
    let mut out = Vec::new();
    DdsType::encode(&shape, &mut out).unwrap();
    assert_eq!(&out[..4], &[0x1f, 0, 0, 0], "DHEADER: 28 + 3");
    assert_eq!(&out[28..], &[3, 0, 0, 0, 7, 8, 9]);
}
