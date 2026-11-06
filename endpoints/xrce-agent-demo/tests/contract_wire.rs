// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-xrce-agent-demo` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire` (`src/main.rs`): `sample_body`
//! writes the XCDR2 little-endian SensorReading with
//! `Writer::with_max_align(Endianness::Little, 4)`, `write_u32`,
//! `write_u16`, `write_u8`, `write_u64`, `write_string`, `write_bytes` and
//! `finish`; `recv` reads the body of a WRITE_DATA back with
//! `Reader::with_max_align(.., Endianness::Little, 4)`, `read_u32`,
//! `read_u16`, `read_u8`, `read_u64`, `read_string` and `read_bytes(4)`.
//! The binary has no library, so the tests call `zerodds-wire` with the
//! same arguments. No socket is opened.
//!
//! Oracle: DDS-XTypes 1.3 §7.4.1.1.1 Table 31, §7.4.1.1.2 (string length
//! counts the NUL), §7.4.2 (PLAIN_CDR2 aligns the 8-octet `uint64` to 4),
//! IEEE 754 binary32 (3.5 = 0x4060_0000).
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, Writer};

/// The SensorReading as §7.4.2 lays it out: the `uint64` at 12, not 16.
const SENSOR_READING_LE: [u8; 40] = [
    0xd4, 0xc3, 0xb2, 0xa1, // id at 0
    0x34, 0x12, // kind at 4
    0x5a, 0x00, // flags at 6, one octet of padding
    0x00, 0x00, 0x60, 0x40, // value at 8
    0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, // stamp at 12
    0x07, 0x00, 0x00, 0x00, b'b', b'a', b'y', b'-', b'1', b'2', 0x00, // label at 20
    0x00, // padding to 32
    0x04, 0x00, 0x00, 0x00, 0xde, 0xad, 0xbe, 0xef, // raw at 32
];

#[test]
fn xtypes_7_4_2_sample_body_is_the_plain_cdr2_layout() {
    let mut w = Writer::with_max_align(Endianness::Little, 4);
    w.write_u32(0xA1B2_C3D4);
    w.write_u16(0x1234);
    w.write_u8(0x5A);
    w.write_u32(3.5f32.to_bits());
    w.write_u64(0x0102_0304_0506_0708);
    w.write_string("bay-12");
    w.write_u32(4);
    w.write_bytes(&[0xDE, 0xAD, 0xBE, 0xEF]);
    assert_eq!(w.finish(), SENSOR_READING_LE);
}

#[test]
fn xtypes_7_4_2_recv_reads_the_plain_cdr2_layout_back() {
    let mut r = Reader::with_max_align(&SENSOR_READING_LE, Endianness::Little, 4);
    assert_eq!(r.read_u32().unwrap(), 0xA1B2_C3D4);
    assert_eq!(r.read_u16().unwrap(), 0x1234);
    assert_eq!(r.read_u8().unwrap(), 0x5A);
    assert_eq!(r.read_u32().unwrap(), 3.5f32.to_bits());
    assert_eq!(r.read_u64().unwrap(), 0x0102_0304_0506_0708);
    assert_eq!(r.read_string().unwrap(), "bay-12");
    assert_eq!(r.read_u32().unwrap(), 4);
    assert_eq!(r.read_bytes(4).unwrap(), [0xDE, 0xAD, 0xBE, 0xEF]);
    assert_eq!(r.remaining(), 0);
}
