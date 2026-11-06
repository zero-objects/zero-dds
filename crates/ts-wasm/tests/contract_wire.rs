// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-ts-wasm` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire` (`src/lib.rs`): `CdrEncoder` wraps
//! a `Writer::with_max_align(Endianness, 8 or 4)` and forwards `write_u8`, `write_u16`,
//! `write_u32`, `write_u64`, `write_string`, `write_bytes`, `align_to`,
//! `position` and `finish`. `CdrDecoder` keeps the bytes and a position and
//! re-seats a `Reader::with_max_align` per call: `skip_to(position)`, one of
//! `read_u8`, `read_u16`, `read_u32`, `read_u64`, `read_string`,
//! `read_bytes`, then `position = len - remaining()`. A `WireError` is
//! turned into a `JsError` with its `Debug` text.
//!
//! The tests call `zerodds-wire` with the arguments `src/lib.rs` passes;
//! the crate's own `CdrEncoder`/`CdrDecoder` error paths build a `JsError`,
//! which exists only on a wasm target. No wasm runtime is needed.
//!
//! Oracle: DDS-XTypes 1.3 §7.4.1.1.1 Table 31 (sizes, alignment and byte
//! order of the primitives), §7.4.1.1.2 (a string's length counts its NUL),
//! §7.4.3.2.1 Table 36 (alignment relative to the origin), §7.4.3.2.2
//! Table 37 (`MAXALIGN` is 8 for encoding version 1 and 4 for version 2,
//! `ALIGN(N)` pads with zeros to `MIN(N, maxalign)`).
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, WireError, Writer};

#[test]
fn xtypes_7_4_1_1_1_primitives_take_their_size_alignment_and_byte_order() {
    let mut little = Writer::new(Endianness::Little);
    little.write_u8(0x42);
    little.write_u16(0xbeef);
    little.write_u32(0xcafe_babe);
    little.write_u64(0x1122_3344_5566_7788);
    assert_eq!(little.position(), 16);
    assert_eq!(
        little.finish(),
        [
            0x42, 0x00, // u8, one octet of padding
            0xef, 0xbe, // u16 at 2
            0xbe, 0xba, 0xfe, 0xca, // u32 at 4
            0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, // u64 at 8
        ]
    );

    let mut big = Writer::new(Endianness::Big);
    big.write_u8(0x42);
    big.write_u16(0xbeef);
    big.write_u32(0xcafe_babe);
    big.write_u64(0x1122_3344_5566_7788);
    assert_eq!(
        big.finish(),
        [
            0x42, 0x00, 0xbe, 0xef, 0xca, 0xfe, 0xba, 0xbe, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88,
        ]
    );
}

#[test]
fn xtypes_7_4_3_2_2_with_max_align_eight_and_four_are_the_two_encoding_versions() {
    // `CdrEncoder::new(endianness, encoding)` and `CdrDecoder` take
    // `with_max_align(.., 8)` for encoding version 1 (also when none is
    // given) and `with_max_align(.., 4)` for version 2: a u64 behind a u32
    // gets four octets of padding under version 1 and none under version 2.
    for (max_align, expected) in [
        (8, &[1u8, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0][..]),
        (4, &[1u8, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0][..]),
    ] {
        let mut w = Writer::with_max_align(Endianness::Little, max_align);
        assert_eq!(w.max_align(), max_align);
        w.write_u32(1);
        w.write_u64(2);
        let bytes = w.finish();
        assert_eq!(bytes, expected, "MAXALIGN {max_align}");

        let mut r = Reader::with_max_align(&bytes, Endianness::Little, max_align);
        assert_eq!(r.max_align(), max_align);
        assert_eq!(r.read_u32().unwrap(), 1);
        assert_eq!(r.read_u64().unwrap(), 2);
        assert_eq!(r.remaining(), 0);
    }
}

#[test]
fn xtypes_7_4_1_1_2_string_length_counts_the_terminating_nul() {
    let mut w = Writer::new(Endianness::Little);
    w.write_u8(9);
    w.write_string("Hi");
    let bytes = w.finish();
    assert_eq!(bytes, [9, 0, 0, 0, 3, 0, 0, 0, b'H', b'i', 0]);

    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(r.read_u8().unwrap(), 9);
    assert_eq!(r.read_string().unwrap(), "Hi");
    assert_eq!(r.remaining(), 0);
}

#[test]
fn write_bytes_and_read_bytes_carry_octets_without_length_or_alignment() {
    let mut w = Writer::new(Endianness::Big);
    w.write_u8(1);
    w.write_bytes(&[0xde, 0xad]);
    let bytes = w.finish();
    assert_eq!(bytes, [0x01, 0xde, 0xad]);

    let mut r = Reader::new(&bytes, Endianness::Big);
    assert_eq!(r.read_u8().unwrap(), 1);
    assert_eq!(r.read_bytes(2).unwrap(), [0xde, 0xad]);
}

#[test]
fn xtypes_7_4_3_2_2_align_to_pads_zeros_up_to_min_of_n_and_maxalign() {
    let mut w = Writer::new(Endianness::Little);
    w.write_u8(7);
    w.align_to(4);
    assert_eq!(w.position(), 4);
    w.align_to(4);
    assert_eq!(w.position(), 4, "aligned already: nothing added");
    w.write_u8(8);
    w.align_to(16);
    assert_eq!(w.position(), 8, "capped at MAXALIGN 8");
    assert_eq!(w.finish(), [7, 0, 0, 0, 8, 0, 0, 0]);
}

#[test]
fn xtypes_7_4_3_2_1_a_reader_seated_with_skip_to_aligns_from_the_stream_start() {
    // `CdrDecoder::with_reader` builds a fresh reader, skips to its saved
    // position and computes the new position from `remaining()`.
    let bytes = [
        0x42, 0x00, 0xef, 0xbe, 0x03, 0x00, 0x00, 0x00, b'a', b'b', 0x00,
    ];
    let mut r = Reader::new(&bytes, Endianness::Little);
    r.skip_to(1).unwrap();
    assert_eq!(r.read_u16().unwrap(), 0xbeef, "padding at 1 skipped");
    assert_eq!(bytes.len() - r.remaining(), 4);

    let mut r = Reader::new(&bytes, Endianness::Little);
    r.skip_to(4).unwrap();
    assert_eq!(r.read_string().unwrap(), "ab");
    assert_eq!(r.remaining(), 0);
}

#[test]
fn a_short_or_inconsistent_stream_is_an_error_not_a_value() {
    let short = [0x01, 0x02];
    let mut r = Reader::new(&short, Endianness::Little);
    assert!(matches!(r.read_u32(), Err(WireError::Truncated { .. })));
    let mut r = Reader::new(&short, Endianness::Little);
    assert!(matches!(r.read_bytes(4), Err(WireError::Truncated { .. })));

    // A string length past the end of the stream.
    let lying = [0xff, 0xff, 0xff, 0x7f, b'a'];
    let mut r = Reader::new(&lying, Endianness::Little);
    assert!(matches!(
        r.read_string(),
        Err(WireError::ImpossibleLength { at: 0, .. })
    ));
}
