// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-c-api` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire` (`src/entities.rs`, `CdrRow`):
//! the positional decoder of a content-filtered topic without a type
//! object. It builds `Reader::with_max_align(payload, Big or Little,
//! 8 for XCDR1 or 4 for XCDR2)`, consumes a DHEADER with `read_u32` when
//! the schema says `@appendable` or `@mutable` (XCDR2 only), and reads the
//! schema's fields in order with `read_u8` (boolean), `read_u32` (int32,
//! float32 bits), `read_u64` (int64, float64 bits) and `read_string`. A
//! read error ends the lookup with `None`, so the sample is not delivered.
//!
//! Oracle: DDS-XTypes 1.3 §7.4.1.1.1 Table 31 (sizes and alignment),
//! §7.4.2 (PLAIN_CDR2 aligns 8-octet types to 4; DELIMITED_CDR is a
//! DHEADER then the members; PL_CDR2 is a DHEADER then an EMHEADER before
//! every member), §7.4.3.2.2 Table 37 (MAXALIGN 8 for version 1, 4 for
//! version 2), §7.4.3.4.2 (EMHEADER1), §7.4.1.1.2 (string).
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, WireError};

/// An `@appendable` body, XCDR2 little-endian: `boolean b = TRUE;
/// long i = -2; long long l = 3; float f = 1.5; double d = 0.5;
/// string s = "x";`.
const APPENDABLE_XCDR2_LE: [u8; 38] = [
    0x22, 0x00, 0x00, 0x00, // DHEADER: 34
    0x01, // b at 4
    0x00, 0x00, 0x00, // padding
    0xfe, 0xff, 0xff, 0xff, // i at 8
    0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // l at 12, aligned to 4
    0x00, 0x00, 0xc0, 0x3f, // f at 20
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xe0, 0x3f, // d at 24
    0x02, 0x00, 0x00, 0x00, b'x', 0x00, // s at 32
];

#[test]
fn xtypes_7_4_2_appendable_xcdr2_body_is_a_dheader_then_members_aligned_to_four() {
    let mut r = Reader::with_max_align(&APPENDABLE_XCDR2_LE, Endianness::Little, 4);
    assert_eq!(r.read_u32().unwrap(), 34, "DHEADER");
    assert_eq!(r.read_u8().unwrap(), 1);
    assert_eq!(r.read_u32().unwrap() as i32, -2);
    assert_eq!(r.read_u64().unwrap() as i64, 3);
    assert_eq!(r.read_u32().unwrap(), 1.5_f32.to_bits());
    assert_eq!(r.read_u64().unwrap(), 0.5_f64.to_bits());
    assert_eq!(r.read_string().unwrap(), "x");
    assert_eq!(r.remaining(), 0);
}

/// The same members as a `@final`/`@appendable` XCDR1 big-endian body: no
/// DHEADER, 8-octet members aligned to 8.
const XCDR1_BE: [u8; 38] = [
    0x01, // b at 0
    0x00, 0x00, 0x00, // padding
    0xff, 0xff, 0xff, 0xfe, // i at 4
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, // l at 8
    0x3f, 0xc0, 0x00, 0x00, // f at 16
    0x00, 0x00, 0x00, 0x00, // padding
    0x3f, 0xe0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // d at 24
    0x00, 0x00, 0x00, 0x02, b'x', 0x00, // s at 32
];

#[test]
fn xtypes_7_4_3_2_2_xcdr1_body_aligns_eight_octet_members_to_eight() {
    let mut r = Reader::with_max_align(&XCDR1_BE, Endianness::Big, 8);
    assert_eq!(r.read_u8().unwrap(), 1);
    assert_eq!(r.read_u32().unwrap() as i32, -2);
    assert_eq!(r.read_u64().unwrap() as i64, 3);
    assert_eq!(r.read_u32().unwrap(), 1.5_f32.to_bits());
    assert_eq!(r.read_u64().unwrap(), 0.5_f64.to_bits());
    assert_eq!(r.read_string().unwrap(), "x");
    assert_eq!(r.remaining(), 0);
}

#[test]
fn xtypes_7_4_3_4_2_a_mutable_xcdr2_body_has_an_emheader_before_each_member() {
    // `@mutable struct { long i; }` with i = 7: DHEADER, EMHEADER (LC 2,
    // id 0), the value. The word behind the DHEADER is the EMHEADER, not
    // the first member.
    let body = [
        0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x07, 0x00, 0x00, 0x00,
    ];
    let mut positional = Reader::with_max_align(&body, Endianness::Little, 4);
    positional.read_u32().unwrap();
    assert_eq!(positional.read_u32().unwrap(), 0x2000_0000);

    let mut r = Reader::with_max_align(&body, Endianness::Little, 4);
    assert_eq!(r.read_dheader().unwrap(), 8);
    let header = r.read_emheader().unwrap();
    assert_eq!((header.member_id, header.length_code), (0, 2));
    assert_eq!(r.read_u32().unwrap(), 7);
}

#[test]
fn a_body_shorter_than_the_schema_is_an_error_so_the_sample_is_not_delivered() {
    let mut r = Reader::with_max_align(&APPENDABLE_XCDR2_LE[..14], Endianness::Little, 4);
    r.read_u32().unwrap();
    r.read_u8().unwrap();
    r.read_u32().unwrap();
    assert!(matches!(
        r.read_u64(),
        Err(WireError::Truncated { at: 12, .. })
    ));
}
