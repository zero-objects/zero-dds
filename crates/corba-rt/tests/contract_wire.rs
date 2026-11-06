// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-corba-rt` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire`: `Writer`, `Reader` and
//! `WireError` in the public `encode`/`decode` of `CorbaAny`, `WString`,
//! `TypeCode` and `ObjectReference`, and `Endianness` in the
//! `RTCorbaPriority` context helpers.
//! - `src/any.rs`: an `any` value with `write_bool`/`read_bool`,
//!   `write_u8`, `write_i16`/`read_i16`, `write_u16`, `write_i32`,
//!   `write_u32`, `write_i64`/`read_i64`, `write_u64`, `write_f32`/
//!   `read_f32`, `write_f64`/`read_f64`, `write_string`/`read_string`; a
//!   `wstring` as `write_u32` (octets) plus the code units through
//!   `write_bytes` in the order `Writer::wstring_unit_order` and
//!   `Writer::endianness` give, read with `read_u32`, `remaining`,
//!   `read_bytes` and `Reader::wstring_unit_order`/`endianness`.
//! - `src/type_code.rs`: a complex TypeCode's parameters as an
//!   encapsulation built by a fresh `Writer::new(endianness)` (byte-order
//!   octet first) and written as `write_u32` + `write_bytes`; read with
//!   `read_u32`, `position`, `read_bytes` and a fresh `Reader::new` over
//!   the encapsulation; `align_to(4)` before a TCKind.
//! - `src/propagation.rs`: the `RTCorbaPriority` context, a byte-order
//!   octet and `write_i16`/`read_i16`; it builds `WireError::Truncated`.
//! - `src/object.rs`: an IOR with `write_string`, `write_u32`,
//!   `write_bytes` and the reads.
//!
//! Oracle: CORBA 3.3 part 2 §9.3.1.1 to §9.3.1.5 (sizes, alignment and
//! byte order of the primitives; IEEE 754 for float and double),
//! §9.3.1.6 (a `wstring` without a byte-order mark is in the stream's
//! byte order), §9.3.2.7 (GIOP 1.2 `wstring`: length in octets, no
//! terminator), §9.3.3 (encapsulation), §9.3.5.1 (TypeCode: the TCKind,
//! and for `tk_sequence` = 19 an encapsulation of the element TypeCode and
//! the bound).
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, WireError, Writer, WstringUnitOrder};

/// `boolean TRUE, short -2, long 5, long long -3, float 1.5,
/// double -2.0`, big-endian, each on its own boundary.
const PRIMITIVES_BE: [u8; 32] = [
    0x01, // boolean
    0x00, // padding
    0xff, 0xfe, // short at 2
    0x00, 0x00, 0x00, 0x05, // long at 4
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfd, // long long at 8
    0x3f, 0xc0, 0x00, 0x00, // float at 16
    0x00, 0x00, 0x00, 0x00, // padding
    0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // double at 24
];

#[test]
fn corba_9_3_1_1_any_values_take_their_size_alignment_and_byte_order() {
    let mut w = Writer::new(Endianness::Big);
    w.write_bool(true);
    w.write_i16(-2);
    w.write_i32(5);
    w.write_i64(-3);
    w.write_f32(1.5);
    w.write_f64(-2.0);
    assert_eq!(w.finish(), PRIMITIVES_BE);

    let mut r = Reader::new(&PRIMITIVES_BE, Endianness::Big);
    assert!(r.read_bool().unwrap());
    assert_eq!(r.read_i16().unwrap(), -2);
    assert_eq!(r.read_i32().unwrap(), 5);
    assert_eq!(r.read_i64().unwrap(), -3);
    assert_eq!(r.read_f32().unwrap().to_bits(), 1.5_f32.to_bits());
    assert_eq!(r.read_f64().unwrap().to_bits(), (-2.0_f64).to_bits());
    assert_eq!(r.remaining(), 0);
}

#[test]
fn corba_9_3_1_6_an_unmarked_wstring_takes_the_stream_byte_order_by_default() {
    let w = Writer::new(Endianness::Big);
    assert_eq!(w.wstring_unit_order(), WstringUnitOrder::Stream);
    assert_eq!(w.endianness(), Endianness::Big);
    let r = Reader::new(&[], Endianness::Little);
    assert_eq!(r.wstring_unit_order(), WstringUnitOrder::Stream);
    assert_eq!(r.endianness(), Endianness::Little);

    // The order a caller configures is the one `WString::encode` reads.
    let w =
        Writer::new(Endianness::Little).with_wstring_unit_order(WstringUnitOrder::ByteOrderMark);
    assert_eq!(w.wstring_unit_order(), WstringUnitOrder::ByteOrderMark);
    let r = Reader::new(&[], Endianness::Big)
        .with_wstring_unit_order(WstringUnitOrder::BigEndianAlways);
    assert_eq!(r.wstring_unit_order(), WstringUnitOrder::BigEndianAlways);
}

#[test]
fn corba_9_3_2_7_giop_1_2_wstring_is_an_octet_count_and_the_code_units() {
    // "hi" big-endian: four octets, no terminator.
    let mut w = Writer::new(Endianness::Big);
    w.write_u8(7);
    w.write_u32(4);
    w.write_bytes(&[0x00, b'h', 0x00, b'i']);
    let bytes = w.finish();
    assert_eq!(bytes, [7, 0, 0, 0, 0, 0, 0, 4, 0x00, b'h', 0x00, b'i']);

    let mut r = Reader::new(&bytes, Endianness::Big);
    r.read_u8().unwrap();
    let octets = r.read_u32().unwrap() as usize;
    assert!(octets <= r.remaining());
    assert_eq!(r.read_bytes(octets).unwrap(), [0x00, b'h', 0x00, b'i']);
}

#[test]
fn corba_9_3_5_1_sequence_typecode_parameters_are_an_encapsulation() {
    // tk_sequence (19) of tk_long (3), unbounded.
    let mut encap = Writer::new(Endianness::Big);
    encap.write_u8(0);
    encap.write_u32(3);
    encap.write_u32(0);
    let encap = encap.finish();
    let mut w = Writer::new(Endianness::Big);
    w.write_u8(0xee);
    w.write_u32(19);
    w.write_u32(u32::try_from(encap.len()).unwrap());
    w.write_bytes(&encap);
    let bytes = w.finish();
    assert_eq!(
        bytes,
        [
            0xee, 0x00, 0x00, 0x00, // something before, padding
            0x00, 0x00, 0x00, 0x13, // tk_sequence
            0x00, 0x00, 0x00, 0x0c, // encapsulation length
            0x00, 0x00, 0x00, 0x00, // byte order, padding
            0x00, 0x00, 0x00, 0x03, // tk_long
            0x00, 0x00, 0x00, 0x00, // bound
        ]
    );

    let mut r = Reader::new(&bytes, Endianness::Big);
    r.read_u8().unwrap();
    r.align_to(4).unwrap();
    assert_eq!(r.position(), 4);
    assert_eq!(r.read_u32().unwrap(), 19);
    let len = r.read_u32().unwrap() as usize;
    assert_eq!(r.position(), 12, "byte 0 of the encapsulation");
    let inner = r.read_bytes(len).unwrap();
    let mut e = Reader::new(inner, Endianness::Big);
    assert_eq!(e.read_u8().unwrap(), 0);
    assert_eq!(e.read_u32().unwrap(), 3);
    assert_eq!(e.read_u32().unwrap(), 0);
    assert_eq!(e.remaining(), 0);
}

#[test]
fn corba_9_3_3_priority_context_is_the_order_octet_and_a_short() {
    let mut w = Writer::new(Endianness::Big);
    w.write_u8(0);
    w.write_i16(5);
    assert_eq!(w.finish(), [0x00, 0x00, 0x00, 0x05]);
    let mut w = Writer::new(Endianness::Little);
    w.write_u8(1);
    w.write_i16(5);
    let data = w.finish();
    assert_eq!(data, [0x01, 0x00, 0x05, 0x00]);

    let mut r = Reader::new(&data, Endianness::Little);
    r.read_u8().unwrap();
    assert_eq!(r.read_i16().unwrap(), 5);
    let mut short = Reader::new(&data[..3], Endianness::Little);
    short.read_u8().unwrap();
    assert!(matches!(
        short.read_i16(),
        Err(WireError::Truncated { at: 2, .. })
    ));
}
