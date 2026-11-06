// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-corba-rust` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate's runtime uses of `zerodds-wire` (`src/runtime.rs`,
//! `src/value_wire.rs`); the generators in the other files only emit text
//! that names `Writer::new`, `Reader::new`, `finish`, `write_u32`,
//! `read_u32`, `remaining`, `position` and `WireError`.
//! - `reply_writer(endianness)` is
//!   `Writer::new(endianness).with_wstring_unit_order(ByteOrderMark)`, the
//!   omniORB `wstring` form that `zerodds_corba_rt::WString::encode` reads
//!   back from the writer.
//! - `ValueWriter` (valuetypes): `align_to(4)` and `position()` for the
//!   tag position and the indirection offset (`prior - position()`),
//!   `write_u32` for the value tag, the repository-id count and the end
//!   tag, `write_string` for the repository ids, and
//!   `write_dheader`/`finish_dheader` as the chunk-size placeholder and its
//!   patch. `ValueReader` and `ValueStreamReader`: `align_to(4)`,
//!   `read_u32`, `read_string`, `read_bytes`, `position`, `remaining`, and
//!   the `WireError::ImpossibleLength`/`refused` errors it builds.
//! - `Endianness`, `Writer`, `Reader` and `WireError` are in the public
//!   signatures of the servant, connection and value traits.
//!
//! Oracle: CORBA 3.3 part 2 §9.3.1.1 (alignment), §9.3.2.7 (string),
//! §9.3.4 (value types: the value tag 0x7fffff00 with bit 3 for chunking
//! and bits 1-2 = 6 for a repository-id list, a chunk as a positive
//! `long` size followed by that many octets, the end tag a negative
//! `long`, an indirection as 0xffffffff followed by a negative `long`
//! offset from that offset field to the referenced value), §9.3.1.6
//! (`wstring` byte-order mark).
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, Writer, WstringUnitOrder};

#[test]
fn corba_9_3_1_6_the_reply_writer_carries_the_byte_order_mark_setting() {
    for endianness in Endianness::ALL {
        let w = Writer::new(endianness).with_wstring_unit_order(WstringUnitOrder::ByteOrderMark);
        assert_eq!(w.wstring_unit_order(), WstringUnitOrder::ByteOrderMark);
        assert_eq!(w.endianness(), endianness);
        assert_eq!(w.max_align(), 8);
        assert!(w.as_bytes().is_empty());
    }
}

/// A chunked value with the repository-id list `["IDL:V:1.0"]` whose state
/// is `long 5; octet 1`, big-endian.
const CHUNKED_VALUE_BE: [u8; 40] = [
    0x7f, 0xff, 0xff, 0x0e, // value tag: chunked, repository-id list
    0x00, 0x00, 0x00, 0x01, // one repository id
    0x00, 0x00, 0x00, 0x0a, b'I', b'D', b'L', b':', b'V', b':', b'1', b'.', b'0', 0x00, 0x00,
    0x00, // padding
    0x00, 0x00, 0x00, 0x05, // chunk size at 24
    0x00, 0x00, 0x00, 0x05, 0x01, // the state
    0x00, 0x00, 0x00, // padding
    0xff, 0xff, 0xff, 0xff, // end tag -1
];

#[test]
fn corba_9_3_4_a_chunk_is_its_octet_count_then_the_octets() {
    let mut w = Writer::new(Endianness::Big);
    w.align_to(4);
    assert_eq!(w.position(), 0);
    w.write_u32(0x7fff_ff0e);
    w.write_u32(1);
    w.write_string("IDL:V:1.0");
    let chunk = w.write_dheader();
    w.write_i32(5);
    w.write_u8(1);
    w.finish_dheader(chunk);
    w.align_to(4);
    w.write_u32((-1_i32) as u32);
    assert!(w.deferred_error().is_none());
    assert_eq!(w.finish(), CHUNKED_VALUE_BE);

    let mut r = Reader::new(&CHUNKED_VALUE_BE, Endianness::Big);
    r.align_to(4).unwrap();
    assert_eq!(r.read_u32().unwrap(), 0x7fff_ff0e);
    let n = r.read_u32().unwrap() as usize;
    assert!(n <= r.remaining() / 4);
    assert_eq!(r.read_string().unwrap(), "IDL:V:1.0");
    r.align_to(4).unwrap();
    let chunk_size = r.read_u32().unwrap() as i32;
    assert_eq!(chunk_size, 5);
    let before = r.position();
    assert_eq!(r.read_i32().unwrap(), 5);
    assert_eq!(r.read_u8().unwrap(), 1);
    assert_eq!(r.position() - before, 5);
    r.align_to(4).unwrap();
    assert_eq!(r.read_u32().unwrap() as i32, -1);
    assert_eq!(r.remaining(), 0);
}

#[test]
fn corba_9_3_4_an_indirection_offset_counts_from_its_own_field() {
    // A value tag at 0, then a second reference to the same value: the
    // indirection marker at 8, the offset field at 12, the offset -12.
    let mut w = Writer::new(Endianness::Big);
    w.align_to(4);
    let tag_pos = w.position();
    w.write_u32(0x7fff_ff02);
    w.write_u8(0);
    w.align_to(4);
    w.write_u32(0xffff_ffff);
    let field = w.position();
    assert_eq!(field, 12);
    let offset = i32::try_from(tag_pos as i64 - field as i64).unwrap();
    w.write_u32(offset as u32);
    assert_eq!(
        w.finish(),
        [
            0x7f, 0xff, 0xff, 0x02, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xf4,
        ]
    );
}
