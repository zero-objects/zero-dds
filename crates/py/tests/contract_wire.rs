// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-py` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire` (`src/ffi.rs`, behind the
//! `extension-module` feature): the `KeyedReading` codec of the
//! `conformance::Reading` type (`@key long id; long seq; double value;`,
//! `@final`). `Writer::with_max_align` with MAXALIGN 4 (XCDR2) and 8
//! (XCDR1), in both byte orders, then `write_i32`, `write_i32`, `write_f64`
//! and `as_bytes`; `Reader::with_max_align` with the same arguments, then
//! `read_i32`, `read_i32`, `read_f64`. `ffi.rs` states that the sample is
//! sixteen octets with `value` at offset eight under both MAXALIGNs.
//!
//! `ffi.rs` compiles only with `extension-module`, which links against the
//! Python interpreter, and its items are private. The tests therefore call
//! `zerodds-wire` directly with the arguments `ffi.rs` passes; nothing here
//! needs a Python interpreter.
//!
//! Oracle: DDS-XTypes 1.3 §7.4.1.1.1 Table 31 (sizes and alignment of
//! Int32 and Float64), §7.4.2 (PLAIN_CDR2 aligns 8-octet types to 4),
//! §7.4.3.2.2 Table 37 (MAXALIGN 8 for version 1, 4 for version 2),
//! IEEE 754 binary64 (1.5 = 0x3FF8_0000_0000_0000).
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, Writer};

/// `id = 1, seq = 2, value = 1.5`, little-endian: two Int32 and a Float64
/// at offset eight, which no MAXALIGN pads.
const READING_LE: [u8; 16] = [
    0x01, 0x00, 0x00, 0x00, // id
    0x02, 0x00, 0x00, 0x00, // seq
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf8, 0x3f, // value
];

/// The same sample big-endian.
const READING_BE: [u8; 16] = [
    0x00, 0x00, 0x00, 0x01, // id
    0x00, 0x00, 0x00, 0x02, // seq
    0x3f, 0xf8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // value
];

fn encode(endianness: Endianness, max_align: usize) -> Vec<u8> {
    let mut w = Writer::with_max_align(endianness, max_align);
    w.write_i32(1);
    w.write_i32(2);
    w.write_f64(1.5);
    w.as_bytes().to_vec()
}

fn decode(bytes: &[u8], endianness: Endianness, max_align: usize) -> (i32, i32, f64) {
    let mut r = Reader::with_max_align(bytes, endianness, max_align);
    (
        r.read_i32().unwrap(),
        r.read_i32().unwrap(),
        r.read_f64().unwrap(),
    )
}

#[test]
fn xtypes_7_4_1_1_1_keyed_reading_is_sixteen_octets_under_xcdr1_and_xcdr2() {
    for max_align in [4, 8] {
        assert_eq!(
            encode(Endianness::Little, max_align),
            READING_LE,
            "MAXALIGN {max_align}"
        );
        assert_eq!(
            encode(Endianness::Big, max_align),
            READING_BE,
            "MAXALIGN {max_align}"
        );
    }
}

#[test]
fn xtypes_7_4_1_1_1_keyed_reading_reads_back_from_the_clause_octets() {
    for max_align in [4, 8] {
        assert_eq!(
            decode(&READING_LE, Endianness::Little, max_align),
            (1, 2, 1.5)
        );
        assert_eq!(decode(&READING_BE, Endianness::Big, max_align), (1, 2, 1.5));
    }
}
