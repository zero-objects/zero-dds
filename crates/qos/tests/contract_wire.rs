// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-qos` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire`: every policy's public
//! `encode_into(&mut Writer)` / `decode_from(&mut Reader) -> Result<_,
//! WireError>` (`src/duration.rs`, `src/wire_helpers.rs` and the files in
//! `src/policies/`). The calls: `write_u32`/`read_u32` for kinds, counts,
//! `Duration_t` and the integer limits (signed values cast through `u32`),
//! `write_u8`/`read_u8` for the boolean plus three padding octets,
//! `write_string`/`read_string` for partition names and data tags,
//! `write_array` with `write_u8` and `read_array(Reader::read_u8, len)` for
//! the opaque `sequence<octet>` of User/Topic/Group data, `Writer::position`,
//! `Reader::position` and `Reader::remaining` for error offsets and the
//! count caps, `WireError::refused` for an unknown enumerator or a cap, and
//! a hand-built `WireError::ImpossibleLength { at, claimed, available }`.
//!
//! Oracle: DDSI-RTPS 2.5 §9.3.2.1 (`Duration_t { long seconds; unsigned
//! long fraction; }`), §9.6.3.2 (a parameter value is CDR), DDS-XTypes 1.3
//! §7.4.1.1.1 Table 31 (UInt32 four octets, aligned to four, in the
//! stream's byte order), §7.4.1.1.2 (a string's length counts the NUL).
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, WireError, Writer};

#[test]
fn rtps_9_3_2_1_duration_t_is_long_seconds_and_unsigned_long_fraction() {
    // `Duration::encode_into` writes `seconds as u32` and the fraction.
    let (seconds, fraction) = (-1_i32, 0x8000_0000_u32);
    for (endianness, expected) in [
        (
            Endianness::Little,
            [0xff_u8, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x80],
        ),
        (
            Endianness::Big,
            [0xff_u8, 0xff, 0xff, 0xff, 0x80, 0x00, 0x00, 0x00],
        ),
    ] {
        let mut w = Writer::new(endianness);
        w.write_u32(seconds as u32);
        w.write_u32(fraction);
        let bytes = w.finish();
        assert_eq!(bytes, expected);

        let mut r = Reader::new(&bytes, endianness);
        let back = (r.read_u32().unwrap() as i32, r.read_u32().unwrap());
        assert_eq!(back, (seconds, fraction));
    }
}

#[test]
fn xtypes_7_4_1_1_2_partition_names_each_start_on_a_four_octet_boundary() {
    let mut w = Writer::new(Endianness::Little);
    w.write_u32(2);
    w.write_string("a");
    w.write_string("bc");
    let bytes = w.finish();
    assert_eq!(
        bytes,
        [
            0x02, 0x00, 0x00, 0x00, // count
            0x02, 0x00, 0x00, 0x00, b'a', 0x00, // "a"
            0x00, 0x00, // padding
            0x03, 0x00, 0x00, 0x00, b'b', b'c', 0x00, // "bc"
        ]
    );

    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(r.read_u32().unwrap(), 2);
    assert_eq!(r.remaining(), 15);
    assert_eq!(r.read_string().unwrap(), "a");
    assert_eq!(r.position(), 10);
    assert_eq!(r.read_string().unwrap(), "bc");
    assert_eq!(r.remaining(), 0);
}

#[test]
fn rtps_9_6_3_2_opaque_data_is_a_length_and_the_octets_unpadded() {
    let mut w = Writer::new(Endianness::Big);
    w.write_u32(3);
    w.write_array(&[0x01_u8, 0x02, 0x03], |w, byte| {
        w.write_u8(*byte);
        Ok(())
    })
    .unwrap();
    assert_eq!(w.position(), 7);
    let bytes = w.finish();
    assert_eq!(bytes, [0x00, 0x00, 0x00, 0x03, 0x01, 0x02, 0x03]);

    let mut r = Reader::new(&bytes, Endianness::Big);
    let len = r.read_u32().unwrap() as usize;
    assert_eq!(r.read_array(Reader::read_u8, len).unwrap(), [1, 2, 3]);
}

#[test]
fn a_boolean_with_three_padding_octets_is_four_single_octets() {
    // `write_bool_padded`: four `write_u8`, no alignment between them.
    let mut w = Writer::new(Endianness::Little);
    w.write_u8(1);
    w.write_u8(0);
    w.write_u8(0);
    w.write_u8(0);
    w.write_u32(7);
    assert_eq!(w.finish(), [1, 0, 0, 0, 7, 0, 0, 0]);
}

#[test]
fn short_and_inconsistent_streams_are_errors_at_the_offset_read() {
    let mut r = Reader::new(&[0x01, 0x00], Endianness::Little);
    assert_eq!(
        r.read_u32(),
        Err(WireError::Truncated {
            at: 0,
            needed: 4,
            available: 2
        })
    );

    let mut r = Reader::new(&[0x02, 0x00, 0x00, 0x00, b'a'], Endianness::Little);
    assert!(matches!(
        r.read_string(),
        Err(WireError::ImpossibleLength { .. } | WireError::Truncated { .. })
    ));

    let mut r = Reader::new(&[0x02, 0x00, 0x00, 0x00, 0xff, 0x00], Endianness::Little);
    assert_eq!(r.read_string(), Err(WireError::NotUtf8 { at: 0 }));
}

#[test]
fn the_error_constructors_qos_builds_by_hand() {
    assert_eq!(
        WireError::refused(8, "a ReliabilityKind enumerator, not 9"),
        WireError::OutOfRange {
            at: 8,
            expected: String::from("a ReliabilityKind enumerator, not 9"),
        }
    );
    let impossible = WireError::ImpossibleLength {
        at: 0,
        claimed: 1000,
        available: 4,
    };
    assert!(!impossible.is_discarded());
    assert!(!impossible.to_string().is_empty());
}
