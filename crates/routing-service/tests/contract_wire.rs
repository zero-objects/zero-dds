// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-routing-service` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire` (`src/transform/codec.rs`, the
//! codec of a filter or transform route over a flat `@final`/`@appendable`
//! struct): `Writer::with_max_align` and `Reader::with_max_align` with
//! `Endianness::Little` and a cap of 4 (XCDR2) or 8 (XCDR1);
//! `Writer::write_dheader`/`finish_dheader` for an appendable body under
//! XCDR2, while the decoder reads the DHEADER with `Reader::read_u32` and
//! steps over it; `align_to` before every member, then `write_bool`,
//! `write_u8`, `write_i8`, `write_u16`, `write_i16`, `write_u32`, `write_i32`,
//! `write_u64`, `write_i64`, `write_f32`, `write_f64`, `write_string` and the
//! `read_*` counterparts (a boolean read with `read_u8`); `Writer::finish`.
//! Every `read_*` error becomes a decode error of the route.
//!
//! Oracle: DDS-XTypes 1.3 §7.4.1.1.1 Table 31 (alignment to the primitive's
//! size, two's complement, IEEE 754), §7.4.1.1.2 (a string's length counts
//! its NUL), §7.4.2 (PLAIN_CDR2 aligns 8-octet primitives to 4;
//! DELIMITED_CDR puts a UINT32 DHEADER in front), §7.4.3.4.1 (the DHEADER
//! holds the length of what follows it). The octets are written out by hand
//! from those clauses.
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, Writer};

/// One sample of `struct { boolean b; short s; long long l; string str;
/// double d; }`, as the route codec holds it.
const B: bool = true;
const S: i16 = -2;
const L: i64 = 0x0102_0304_0506_0708;
const STR: &str = "hi";
const D: f64 = 1.5;

/// XCDR2 alignment of each member as the codec asks for it: the size,
/// capped at 4.
const XCDR2_ALIGN: [usize; 5] = [1, 2, 4, 4, 4];
/// XCDR1 alignment: the natural size (a string's length is a UInt32).
const XCDR1_ALIGN: [usize; 5] = [1, 2, 8, 4, 8];

fn write(max_align: usize, appendable: bool, align: [usize; 5]) -> Vec<u8> {
    let mut w = Writer::with_max_align(Endianness::Little, max_align);
    let header = appendable.then(|| w.write_dheader());
    w.align_to(align[0]);
    w.write_bool(B);
    w.align_to(align[1]);
    w.write_i16(S);
    w.align_to(align[2]);
    w.write_i64(L);
    w.align_to(align[3]);
    w.write_string(STR);
    w.align_to(align[4]);
    w.write_f64(D);
    if let Some(header) = header {
        w.finish_dheader(header);
    }
    w.finish()
}

fn read(
    bytes: &[u8],
    endianness: Endianness,
    max_align: usize,
    appendable: bool,
    align: [usize; 5],
) -> Result<(bool, i16, i64, String, f64), zerodds_wire::WireError> {
    let mut r = Reader::with_max_align(bytes, endianness, max_align);
    if appendable {
        r.read_u32()?;
    }
    r.align_to(align[0])?;
    let b = r.read_u8()? != 0;
    r.align_to(align[1])?;
    let s = r.read_i16()?;
    r.align_to(align[2])?;
    let l = r.read_i64()?;
    r.align_to(align[3])?;
    let text = r.read_string()?;
    r.align_to(align[4])?;
    let d = r.read_f64()?;
    Ok((b, s, l, text, d))
}

/// DELIMITED_CDR, little endian: the DHEADER (28), then the members with
/// the 8-octet ones at offsets aligned to 4.
const APPENDABLE_XCDR2: [u8; 32] = [
    0x1c, 0x00, 0x00, 0x00, // DHEADER: 28 octets follow
    0x01, // b
    0x00, // pad to 2
    0xfe, 0xff, // s = -2
    0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, // l at offset 8
    0x03, 0x00, 0x00, 0x00, b'h', b'i', 0x00, // str: length 3 counts the NUL
    0x00, // pad to 4
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf8, 0x3f, // d = 1.5 at offset 24
];

/// PLAIN_CDR (XCDR1), little endian, of the same sample in a `@final`
/// struct: no DHEADER, the 8-octet members at offsets aligned to 8.
const FINAL_XCDR1: [u8; 32] = [
    0x01, // b
    0x00, // pad to 2
    0xfe, 0xff, // s
    0x00, 0x00, 0x00, 0x00, // pad to 8
    0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, // l at offset 8
    0x03, 0x00, 0x00, 0x00, b'h', b'i', 0x00, // str
    0x00, // pad to 8
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf8, 0x3f, // d at offset 24
];

#[test]
fn xtypes_7_4_2_an_appendable_body_under_xcdr2_is_a_dheader_then_plain_cdr2() {
    assert_eq!(write(4, true, XCDR2_ALIGN), APPENDABLE_XCDR2);
    assert_eq!(
        read(&APPENDABLE_XCDR2, Endianness::Little, 4, true, XCDR2_ALIGN).unwrap(),
        (B, S, L, STR.to_string(), D)
    );
}

#[test]
fn xtypes_7_4_2_a_final_body_under_xcdr2_has_no_dheader() {
    // The same members without the four DHEADER octets; every alignment is
    // at most 4, so the offsets move by four and the padding stays.
    let body = write(4, false, XCDR2_ALIGN);
    assert_eq!(body, APPENDABLE_XCDR2[4..]);
    assert_eq!(
        read(&body, Endianness::Little, 4, false, XCDR2_ALIGN).unwrap(),
        (B, S, L, STR.to_string(), D)
    );
}

#[test]
fn xtypes_7_4_1_1_1_xcdr1_aligns_eight_octet_members_to_eight() {
    assert_eq!(write(8, false, XCDR1_ALIGN), FINAL_XCDR1);
    assert_eq!(
        read(&FINAL_XCDR1, Endianness::Little, 8, false, XCDR1_ALIGN).unwrap(),
        (B, S, L, STR.to_string(), D)
    );
}

#[test]
fn a_body_cut_short_anywhere_is_a_read_error_and_never_a_value() {
    for cut in 0..APPENDABLE_XCDR2.len() {
        assert!(
            read(
                &APPENDABLE_XCDR2[..cut],
                Endianness::Little,
                4,
                true,
                XCDR2_ALIGN
            )
            .is_err(),
            "xcdr2 body cut at {cut}"
        );
    }
    for cut in 0..FINAL_XCDR1.len() {
        assert!(
            read(
                &FINAL_XCDR1[..cut],
                Endianness::Little,
                8,
                false,
                XCDR1_ALIGN
            )
            .is_err(),
            "xcdr1 body cut at {cut}"
        );
    }
}

#[test]
fn xtypes_7_4_1_1_1_a_big_endian_body_reads_with_endianness_big() {
    // The codec reads every body little endian. A writer whose
    // encapsulation is D_CDR2_BE sends the same sample as below; read as
    // little endian it yields other values, read as big endian the right
    // ones. The reader honours the order it is given.
    let big: [u8; 32] = [
        0x00, 0x00, 0x00, 0x1c, // DHEADER, big endian
        0x01, 0x00, 0xff, 0xfe, // b, pad, s
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, // l
        0x00, 0x00, 0x00, 0x03, b'h', b'i', 0x00, 0x00, // str, pad
        0x3f, 0xf8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // d
    ];
    assert_eq!(
        read(&big, Endianness::Big, 4, true, XCDR2_ALIGN).unwrap(),
        (B, S, L, STR.to_string(), D)
    );
    // Little endian, the string length reads as 0x0300_0000 and the
    // read is refused rather than producing a sample.
    assert!(read(&big, Endianness::Little, 4, true, XCDR2_ALIGN).is_err());
}
