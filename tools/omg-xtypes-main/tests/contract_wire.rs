// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-omg-xtypes-main` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire`: the codec of the 25 generated
//! shape types (`src/shape_type.rs`, `include!` of the `zerodds-build`
//! output). Their `DdsType` methods open `Writer::appending` /
//! `Reader::with_max_align` with `Endianness::Little` or `Endianness::Big`
//! and the cap of the XCDR version (4 for XCDR2, 8 for XCDR1), and the
//! inherent codec calls `write_dheader`/`finish_dheader`,
//! `write_string_bounded`, `write_i32`, `write_f32` and, for `@mutable`,
//! the EMHEADER calls; the reader `read_dheader`, `skip_to`, `read_string`,
//! `read_i32`, `read_f32`, `read_emheader`; `check_deferred` and `finish`
//! close the writer. The contract drives those calls through the generated
//! `Shape1Final`, `Shape1Extensible`, `Shape1Mutable` and `Shape2Extensible`
//! with the values `ShapeVariant::fill_data("BLUE", 7)` gives (`x = 7`,
//! `y = 14`, `shapesize = 30`, `angle = 35`).
//!
//! Oracle, octets written out by hand: DDS-XTypes 1.3 §7.4.1.1.1 (alignment,
//! byte order), §7.4.1.1.2 (a string's length counts its NUL), §7.4.2
//! (PLAIN_CDR2 for FINAL, DELIMITED_CDR for APPENDABLE, PL_CDR2 for MUTABLE;
//! an XCDR1 APPENDABLE type has no DHEADER), §7.4.3.4.1 (the DHEADER lets a
//! reader skip what follows the members it knows), §7.4.3.4.2 (EMHEADER: M
//! flag bit 31, length code bits 28..30, member id bits 0..27).
#![allow(clippy::unwrap_used)]

use zerodds_dcps::DdsType;
use zerodds_omg_xtypes_main::shape_type::ShapeVariant;
use zerodds_omg_xtypes_main::shape_type::generated::{
    Shape1Extensible, Shape1Final, Shape1Mutable, Shape2Extensible,
};

/// `color = "BLUE", x = 7, y = 14, shapesize = 30`, PLAIN_CDR2 little endian.
const BODY_LE: [u8; 24] = [
    0x05, 0x00, 0x00, 0x00, b'B', b'L', b'U', b'E', 0x00, // length 5 counts the NUL
    0x00, 0x00, 0x00, // pad to 4
    0x07, 0x00, 0x00, 0x00, // x
    0x0e, 0x00, 0x00, 0x00, // y
    0x1e, 0x00, 0x00, 0x00, // shapesize
];
/// The same, big endian.
const BODY_BE: [u8; 24] = [
    0x00, 0x00, 0x00, 0x05, b'B', b'L', b'U', b'E', 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07,
    0x00, 0x00, 0x00, 0x0e, 0x00, 0x00, 0x00, 0x1e,
];

fn with_dheader(dheader: [u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = dheader.to_vec();
    out.extend_from_slice(body);
    out
}

#[test]
fn xtypes_7_4_2_a_final_shape_is_plain_cdr2_in_both_byte_orders() {
    let sample = <Shape1Final as ShapeVariant>::fill_data("BLUE", 7);
    let mut le = Vec::new();
    DdsType::encode(&sample, &mut le).unwrap();
    assert_eq!(le, BODY_LE);
    let mut be = Vec::new();
    sample.encode_be(&mut be).unwrap();
    assert_eq!(be, BODY_BE);
    assert_eq!(<Shape1Final as DdsType>::decode(&BODY_LE).unwrap(), sample);
    assert_eq!(Shape1Final::decode_be(&BODY_BE).unwrap(), sample);
}

#[test]
fn xtypes_7_4_2_an_appendable_shape_is_a_dheader_then_the_members() {
    let sample = <Shape1Extensible as ShapeVariant>::fill_data("BLUE", 7);
    let mut le = Vec::new();
    DdsType::encode(&sample, &mut le).unwrap();
    assert_eq!(le, with_dheader([0x18, 0, 0, 0], &BODY_LE), "DHEADER 24");
    let mut be = Vec::new();
    sample.encode_be(&mut be).unwrap();
    assert_eq!(be, with_dheader([0, 0, 0, 0x18], &BODY_BE));
    assert_eq!(<Shape1Extensible as DdsType>::decode(&le).unwrap(), sample);
    assert_eq!(Shape1Extensible::decode_be(&be).unwrap(), sample);
    // Under XCDR1 an appendable type is not delimited.
    let mut xcdr1 = Vec::new();
    DdsType::encode_xcdr1(&sample, &mut xcdr1).unwrap();
    assert_eq!(xcdr1, BODY_LE);
    assert_eq!(
        <Shape1Extensible as DdsType>::decode_xcdr1(&BODY_LE).unwrap(),
        sample
    );
}

#[test]
fn xtypes_7_4_3_4_1_a_reader_skips_what_a_longer_writer_appended() {
    // `Shape2Extensible` appends `float angle`; a `Shape1Extensible`
    // reader takes the four members it knows and steps to the DHEADER's end.
    let longer = <Shape2Extensible as ShapeVariant>::fill_data("BLUE", 7);
    let mut bytes = Vec::new();
    DdsType::encode(&longer, &mut bytes).unwrap();
    let mut expected = with_dheader([0x1c, 0, 0, 0], &BODY_LE);
    expected.extend_from_slice(&35.0f32.to_le_bytes());
    assert_eq!(bytes, expected, "DHEADER 28, angle = 35.0 last");
    assert_eq!(
        <Shape1Extensible as DdsType>::decode(&bytes).unwrap(),
        <Shape1Extensible as ShapeVariant>::fill_data("BLUE", 7)
    );
}

#[test]
fn xtypes_7_4_3_4_2_a_mutable_shape_is_read_by_member_id() {
    let bytes: [u8; 48] = [
        0x2c, 0x00, 0x00, 0x00, // DHEADER: 44
        0x00, 0x00, 0x00, 0xc0, // EMHEADER: M (key), LC 4, id 0 (`color`)
        0x09, 0x00, 0x00, 0x00, // NEXTINT: 9
        0x05, 0x00, 0x00, 0x00, b'B', b'L', b'U', b'E', 0x00, //
        0x00, 0x00, 0x00, // pad to the next EMHEADER
        0x01, 0x00, 0x00, 0x20, 0x07, 0x00, 0x00, 0x00, // LC 2, id 1: x
        0x02, 0x00, 0x00, 0x20, 0x0e, 0x00, 0x00, 0x00, // LC 2, id 2: y
        0x03, 0x00, 0x00, 0x20, 0x1e, 0x00, 0x00, 0x00, // LC 2, id 3: shapesize
    ];
    assert_eq!(
        <Shape1Mutable as DdsType>::decode(&bytes).unwrap(),
        <Shape1Mutable as ShapeVariant>::fill_data("BLUE", 7)
    );
}

#[test]
fn a_color_past_its_bound_is_refused_and_leaves_the_buffer_as_it_was() {
    // `string<STR_LEN_MAX>` with `STR_LEN_MAX = 32`.
    let sample = <Shape1Final as ShapeVariant>::fill_data(&"C".repeat(33), 7);
    let mut out = vec![0xAA, 0xBB];
    assert!(DdsType::encode(&sample, &mut out).is_err());
    assert_eq!(out, [0xAA, 0xBB]);
    // Thirty-two octets fit.
    let fits = <Shape1Final as ShapeVariant>::fill_data(&"C".repeat(32), 7);
    let mut out = Vec::new();
    DdsType::encode(&fits, &mut out).unwrap();
    assert_eq!(out[..4], [33, 0, 0, 0], "length 33 counts the NUL");
}

#[test]
fn a_sample_cut_short_is_a_decode_error() {
    for cut in 0..BODY_LE.len() {
        assert!(
            <Shape1Final as DdsType>::decode(&BODY_LE[..cut]).is_err(),
            "cut at {cut}"
        );
    }
    // A DHEADER claiming more than the sample holds.
    let lying = with_dheader([0x40, 0, 0, 0], &BODY_LE);
    assert!(<Shape1Extensible as DdsType>::decode(&lying).is_err());
}
