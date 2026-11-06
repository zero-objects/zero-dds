// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-bench-suite` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire`: the code `build.rs` generates
//! with `zerodds-build` from `idl/roundtrip.idl` and `src/bin/roundtrip_typed.rs`
//! includes — `@final struct RoundtripBench::Roundtrip { unsigned long
//! sequence_id; unsigned long long t_send_ns; sequence<octet, 65536>
//! payload; }` with its `DdsType` surface — writes through
//! `Writer::continuing(Endianness::Little | Big, 4, prefix)`, `write_u32`,
//! `write_u64`, the sequence length and its octets, `check_deferred` and
//! `finish`, and reads through `Reader::with_max_align(bytes, …, 4)`,
//! `skip_to(offset)` and the matching reads. (The direct
//! `zerodds_wire::{Endianness, Reader, Writer}` import of that file is in its
//! `#[cfg(test)]` module.)
//!
//! Oracle: DDS-XTypes 1.3 §7.4.2 (PLAIN_CDR2, MAXALIGN 4: the `unsigned long
//! long` behind the `unsigned long` at offset 4, not 8), CORBA 3.3 part 2
//! §9.3.1.2 (byte order) and §9.3.2.5 (a sequence is its length and the
//! elements).
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, Writer};

/// The sample of `roundtrip_typed.rs`'s own test.
fn write(endianness: Endianness, prefix: Vec<u8>) -> Vec<u8> {
    let mut w = Writer::continuing(endianness, 4, prefix);
    w.write_u32(0x1111_1111);
    w.write_u64(0x2222_2222_2222_2222);
    w.write_u32(4);
    w.write_bytes(&[0xab; 4]);
    w.check_deferred().unwrap();
    w.finish()
}

#[test]
fn xtypes_7_4_2_the_roundtrip_body_is_twenty_octets_under_maxalign_4() {
    #[rustfmt::skip]
    let little = [
        0x11, 0x11, 0x11, 0x11,                         // sequence_id @0
        0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, // t_send_ns @4
        0x04, 0x00, 0x00, 0x00,                         // length @12
        0xab, 0xab, 0xab, 0xab,                         // payload @16
    ];
    assert_eq!(write(Endianness::Little, Vec::new()), little);
    let mut big = little;
    big[12..16].copy_from_slice(&[0x00, 0x00, 0x00, 0x04]);
    assert_eq!(write(Endianness::Big, Vec::new()), big);
}

#[test]
fn a_continuing_writer_keeps_the_prefix_and_counts_alignment_from_its_start() {
    // A four-octet prefix (an encapsulation header) in front: the body
    // follows unchanged, every member still four-aligned.
    let bytes = write(Endianness::Little, vec![0x00, 0x07, 0x00, 0x00]);
    assert_eq!(bytes.len(), 24);
    assert_eq!(bytes[..4], [0x00, 0x07, 0x00, 0x00]);
    assert_eq!(bytes[4..8], [0x11; 4]);
    assert_eq!(bytes[8..16], [0x22; 8]);

    let mut r = Reader::with_max_align(&bytes, Endianness::Little, 4);
    r.skip_to(4).unwrap();
    assert_eq!(r.read_u32().unwrap(), 0x1111_1111);
    assert_eq!(r.read_u64().unwrap(), 0x2222_2222_2222_2222);
    assert_eq!(r.read_u32().unwrap(), 4);
    assert_eq!(r.read_bytes(4).unwrap(), [0xab; 4]);
    assert_eq!(r.remaining(), 0);
}
