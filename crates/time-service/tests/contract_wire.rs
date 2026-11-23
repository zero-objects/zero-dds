// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-time-service` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire` (feature `serve`):
//! `src/servant.rs` decodes each request body with a
//! `Reader::new(body, endianness)` and encodes each reply body with a
//! `Writer::new(endianness)`, `finish` at the end. A `TimeBase::UtcT` is
//! `write_u64` (time), `write_u32` (inacclo), `write_u16` (inacchi),
//! `write_i16` (tdf), read back with `read_u64`, `read_u32`, `read_u16`,
//! `read_i16`; an `IntervalT` two `write_u64`/`read_u64`; the arguments of
//! `new_universal_time` `read_u64`, `read_u64`, `read_i16`; the enums
//! `ComparisonType`, `TimeComparison` and `OverlapType` `write_u32`/
//! `read_u32`; a system exception body its repository id (`write_string`),
//! the minor code and `COMPLETED_NO` (`write_u32`); the user exception
//! `TimeUnavailable` its repository id alone. For a GIOP 1.0 or 1.1 body
//! that does not start on an 8-octet boundary of its message,
//! `TimeServant::dispatch_at` puts `offset % 8` octets in front of the
//! request body and skips them with `read_bytes`, and writes as many
//! `write_u8(0)` in front of the reply body and drops them after `finish`:
//! it relies on a reader and a writer aligning from the start of their own
//! octets, to at most 8 octets. Every `Err` of a read is the system
//! exception `MARSHAL`. `Endianness` is in the public signatures
//! (`TimeServant::dispatch`, `system_exception_body`, `src/server.rs`).
//!
//! Oracle: CORBA 3.3 Part 2 §9.3.1.1 (a primitive is aligned to its size,
//! relative to the start of the octet stream, which for GIOP is the start
//! of the message), §9.3.2.7 (string: length including the NUL), §9.3.5.5
//! (exception: repository id, then the members; `COMPLETED_NO` = 1), with
//! OMG Time Service 1.1 §1.3.2.4 (`struct UtcT { TimeT time; unsigned long
//! inacclo; unsigned short inacchi; TdfT tdf; }`, 16 octets).
#![allow(clippy::unwrap_used, missing_docs)]

use zerodds_wire::{Endianness, Reader, WireError, Writer};

/// `UtcT { time 0x0102030405060708, inacclo 0x090A0B0C, inacchi 0x0D0E,
/// tdf -60 }`, big-endian.
const UTC_BE: [u8; 16] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, // time
    0x09, 0x0A, 0x0B, 0x0C, // inacclo
    0x0D, 0x0E, // inacchi
    0xFF, 0xC4, // tdf = -60
];

#[test]
fn time_1_3_2_4_utc_t_is_time_inacclo_inacchi_tdf_in_16_octets() {
    for e in [Endianness::Big, Endianness::Little] {
        let mut w = Writer::new(e);
        w.write_u64(0x0102_0304_0506_0708);
        w.write_u32(0x090A_0B0C);
        w.write_u16(0x0D0E);
        w.write_i16(-60);
        let bytes = w.finish();
        let expected: Vec<u8> = match e {
            Endianness::Big => UTC_BE.to_vec(),
            Endianness::Little => {
                // Each member in its own byte order, no padding.
                let mut le = Vec::new();
                le.extend(UTC_BE[0..8].iter().rev());
                le.extend(UTC_BE[8..12].iter().rev());
                le.extend(UTC_BE[12..14].iter().rev());
                le.extend(UTC_BE[14..16].iter().rev());
                le
            }
        };
        assert_eq!(bytes, expected, "{e:?}");

        let mut r = Reader::new(&bytes, e);
        assert_eq!(r.read_u64().unwrap(), 0x0102_0304_0506_0708);
        assert_eq!(r.read_u32().unwrap(), 0x090A_0B0C);
        assert_eq!(r.read_u16().unwrap(), 0x0D0E);
        assert_eq!(r.read_i16().unwrap(), -60);
        assert_eq!(r.remaining(), 0);
    }
}

#[test]
fn corba_9_3_1_1_new_universal_time_arguments_pad_after_the_tdf_only() {
    // `new_universal_time(TimeT time, InaccuracyT inaccuracy, TdfT tdf)`:
    // two `unsigned long long` back to back, then a `short` at 16.
    let mut w = Writer::new(Endianness::Little);
    w.write_u64(7);
    w.write_u64(5);
    w.write_i16(120);
    let bytes = w.finish();
    assert_eq!(bytes.len(), 18);

    // An `unsigned long long` after a `short` is aligned to 8: six octets
    // of padding.
    let mut w = Writer::new(Endianness::Little);
    w.write_i16(1);
    w.write_u64(2);
    assert_eq!(w.position(), 16);
    let bytes = w.finish();
    assert_eq!(&bytes[2..8], &[0; 6]);
    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(r.read_i16().unwrap(), 1);
    assert_eq!(r.read_u64().unwrap(), 2);
    assert_eq!(r.position(), 16);
}

#[test]
fn corba_9_3_1_1_octets_in_front_shift_the_alignment_as_the_message_does() {
    // A GIOP 1.0 request body at offset 60 of its message: its first
    // `unsigned long long` sits at message offset 64, after 4 octets of
    // padding. `dispatch_at` puts 60 % 8 = 4 octets in front of the body
    // and skips them; the reader then aligns as the message does.
    let mut body = vec![0xEE; 4]; // the padding the sender wrote
    body.extend_from_slice(&42u64.to_be_bytes());

    let mut placed = vec![0u8; 4];
    placed.extend_from_slice(&body);
    let mut r = Reader::new(&placed, Endianness::Big);
    assert_eq!(r.read_bytes(4).unwrap(), &[0, 0, 0, 0]);
    assert_eq!(r.read_u64().unwrap(), 42);
    assert_eq!(r.remaining(), 0);

    // Read from the start of the body alone, the reader aligns from the
    // body: it takes the padding as part of the value.
    let mut unaligned = Reader::new(&body, Endianness::Big);
    assert_ne!(unaligned.read_u64().unwrap(), 42);

    // The reply side: a body at offset 20 of its message (20 % 8 = 4).
    let mut w = Writer::new(Endianness::Big);
    for _ in 0..4 {
        w.write_u8(0);
    }
    w.write_u64(42);
    let mut out = w.finish();
    out.drain(..4);
    let mut expected = vec![0u8; 4];
    expected.extend_from_slice(&42u64.to_be_bytes());
    assert_eq!(out, expected);
}

#[test]
fn corba_9_3_5_5_system_exception_body_is_repository_id_minor_and_completion() {
    let id = "IDL:omg.org/CORBA/BAD_PARAM:1.0";
    let mut w = Writer::new(Endianness::Big);
    w.write_string(id);
    w.write_u32(0);
    w.write_u32(1);
    let bytes = w.finish();

    let mut expected = vec![0, 0, 0, 32];
    expected.extend_from_slice(id.as_bytes());
    expected.push(0); // ends at 36, no padding
    expected.extend_from_slice(&[0, 0, 0, 0]); // minor
    expected.extend_from_slice(&[0, 0, 0, 1]); // COMPLETED_NO
    assert_eq!(bytes, expected);

    let mut r = Reader::new(&bytes, Endianness::Big);
    assert_eq!(r.read_string().unwrap(), id);
    assert_eq!(r.read_u32().unwrap(), 0);
    assert_eq!(r.read_u32().unwrap(), 1);
}

#[test]
fn a_cut_utc_t_is_a_wire_error() {
    // The servant answers MARSHAL on any `Err`: a UtcT one octet short
    // must not read as a value.
    let mut r = Reader::new(&UTC_BE[..15], Endianness::Big);
    r.read_u64().unwrap();
    r.read_u32().unwrap();
    r.read_u16().unwrap();
    assert!(matches!(r.read_i16(), Err(WireError::Truncated { .. })));

    let mut r = Reader::new(&[0u8; 3], Endianness::Big);
    assert!(matches!(r.read_bytes(4), Err(WireError::Truncated { .. })));
}
