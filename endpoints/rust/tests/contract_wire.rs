// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-endpoint-rust` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire` (`src/lib.rs`): `Reading::marshal`
//! builds the `@final` body `uint32 id; float value; string label;` with
//! `Writer::with_max_align(endian, 4)`, `write_u32`, `write_f32`,
//! `write_string` and `finish`; `Reading::decode` reads it back with
//! `Reader::with_max_align(body, Endianness::Little, 4)`, `read_u32`,
//! `read_f32`, `read_string`. The crate claims the body is byte-identical
//! to the other endpoint SDKs.
//!
//! Oracle: DDS-XTypes 1.3 §7.4.1.1.1 Table 31 (UInt32 and Float32, four
//! octets aligned to four), §7.4.1.1.2 (a string is a UInt32 length that
//! counts the NUL, the octets, the NUL), IEEE 754 binary32
//! (1.5 = 0x3FC0_0000).
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, Writer};

fn marshal(id: u32, value: f32, label: &str, endian: Endianness) -> Vec<u8> {
    let mut w = Writer::with_max_align(endian, 4);
    w.write_u32(id);
    w.write_f32(value);
    w.write_string(label);
    w.finish()
}

#[test]
fn xtypes_7_4_1_1_2_reading_body_octets_in_both_byte_orders() {
    assert_eq!(
        marshal(7, 1.5, "ab", Endianness::Little),
        [
            0x07, 0x00, 0x00, 0x00, // id
            0x00, 0x00, 0xc0, 0x3f, // value
            0x03, 0x00, 0x00, 0x00, b'a', b'b', 0x00, // label
        ]
    );
    assert_eq!(
        marshal(7, 1.5, "ab", Endianness::Big),
        [
            0x00, 0x00, 0x00, 0x07, 0x3f, 0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, b'a', b'b',
            0x00,
        ]
    );
    // An empty label is a length of one and the NUL.
    assert_eq!(
        marshal(0, 0.0, "", Endianness::Little),
        [0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0]
    );
}

#[test]
fn xtypes_7_4_1_1_2_reading_body_reads_back_from_the_clause_octets() {
    let body = [
        0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0xc0, 0x3f, 0x03, 0x00, 0x00, 0x00, b'a', b'b', 0x00,
    ];
    let mut r = Reader::with_max_align(&body, Endianness::Little, 4);
    assert_eq!(r.read_u32().unwrap(), 7);
    assert_eq!(r.read_f32().unwrap().to_bits(), 1.5_f32.to_bits());
    assert_eq!(r.read_string().unwrap(), "ab");
    assert_eq!(r.remaining(), 0);
}
