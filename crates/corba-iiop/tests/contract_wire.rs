// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-corba-iiop` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire`: `src/profile_body.rs` encodes
//! the IIOP `ProfileBody` with `write_u8`, `write_string`, `write_u16`,
//! `write_u32` and `write_bytes` into the public `encode(&mut Writer)`, and
//! as an encapsulation with `Writer::new(endianness)` whose first octet is
//! the byte order; it decodes with `Reader::new(bytes, endianness)`,
//! `read_u8`, `read_string`, `read_u16`, `read_u32` and `read_bytes`, and
//! wraps a `WireError` in `CdrError::Wire`. `src/bidir.rs` writes and reads
//! the listen points of a `BiDirIIOPServiceContext` with
//! `write_u32`/`write_string`/`write_u16` and their reads.
//! `src/framing.rs` writes headers with `Writer::new` and a fragment's
//! `request_id` with `write_u32` and `finish`. `Endianness` is in the
//! public signatures of `framing.rs` and `connection.rs`.
//!
//! Oracle: CORBA 3.3 part 2 §9.3.1.1 (alignment to the size of the
//! primitive), §9.3.2.7 (string), §9.3.3 (an encapsulation: byte-order
//! octet, then the contents aligned from that octet), §9.7.2 (IIOP 1.1+
//! `ProfileBody`: version, host, port, object_key, components), §9.8
//! (`BiDirIIOPServiceContext`: `sequence<ListenPoint>`, each a string and
//! an unsigned short).
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, WireError, Writer};

fn profile_body(endianness: Endianness, order_octet: u8) -> Vec<u8> {
    let mut w = Writer::new(endianness);
    w.write_u8(order_octet);
    w.write_u8(1);
    w.write_u8(2);
    w.write_string("h");
    w.write_u16(2809);
    w.write_u32(2);
    w.write_bytes(&[0x4b, 0x31]);
    w.write_u32(1);
    w.write_u32(1);
    w.write_u32(3);
    w.write_bytes(&[0xaa, 0xbb, 0xcc]);
    w.finish()
}

const PROFILE_BE: [u8; 35] = [
    0x00, // byte order
    0x01, 0x02, // IIOP 1.2
    0x00, // padding
    0x00, 0x00, 0x00, 0x02, b'h', 0x00, // host at 4
    0x0a, 0xf9, // port 2809 at 10
    0x00, 0x00, 0x00, 0x02, 0x4b, 0x31, // object_key at 12
    0x00, 0x00, // padding
    0x00, 0x00, 0x00, 0x01, // one component at 20
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x03, 0xaa, 0xbb, 0xcc, // tag 1, data
];

#[test]
fn corba_9_7_2_profile_body_encapsulation_aligns_from_the_byte_order_octet() {
    assert_eq!(profile_body(Endianness::Big, 0), PROFILE_BE);
    assert_eq!(
        profile_body(Endianness::Little, 1),
        [
            0x01, 0x01, 0x02, 0x00, 0x02, 0x00, 0x00, 0x00, b'h', 0x00, 0xf9, 0x0a, 0x02, 0x00,
            0x00, 0x00, 0x4b, 0x31, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
            0x03, 0x00, 0x00, 0x00, 0xaa, 0xbb, 0xcc,
        ]
    );

    let mut r = Reader::new(&PROFILE_BE, Endianness::Big);
    assert_eq!(r.read_u8().unwrap(), 0);
    assert_eq!((r.read_u8().unwrap(), r.read_u8().unwrap()), (1, 2));
    assert_eq!(r.read_string().unwrap(), "h");
    assert_eq!(r.read_u16().unwrap(), 2809);
    let key_len = r.read_u32().unwrap() as usize;
    assert_eq!(r.read_bytes(key_len).unwrap(), [0x4b, 0x31]);
    assert_eq!(r.read_u32().unwrap(), 1);
    assert_eq!(r.read_u32().unwrap(), 1);
    let data_len = r.read_u32().unwrap() as usize;
    assert_eq!(r.read_bytes(data_len).unwrap(), [0xaa, 0xbb, 0xcc]);
    assert_eq!(r.remaining(), 0);
}

#[test]
fn corba_9_8_bidir_listen_points_are_a_string_and_an_unsigned_short_each() {
    let expected = [
        0x00, 0x00, 0x00, 0x02, // two listen points
        0x00, 0x00, 0x00, 0x03, b'a', b'b', 0x00, // host "ab" at 4
        0x00, // padding
        0x00, 0x01, // port 1 at 12
        0x00, 0x00, // padding
        0x00, 0x00, 0x00, 0x02, b'c', 0x00, // host "c" at 16
        0x00, 0x02, // port 2 at 22
    ];
    let mut w = Writer::new(Endianness::Big);
    w.write_u32(2);
    w.write_string("ab");
    w.write_u16(1);
    w.write_string("c");
    w.write_u16(2);
    assert_eq!(w.finish(), expected);

    let mut r = Reader::new(&expected, Endianness::Big);
    assert_eq!(r.read_u32().unwrap(), 2);
    assert_eq!(r.read_string().unwrap(), "ab");
    assert_eq!(r.read_u16().unwrap(), 1);
    assert_eq!(r.read_string().unwrap(), "c");
    assert_eq!(r.read_u16().unwrap(), 2);
    assert_eq!(r.remaining(), 0);
}

#[test]
fn a_profile_cut_inside_the_object_key_is_a_wire_error() {
    let mut r = Reader::new(&PROFILE_BE[..17], Endianness::Big);
    r.read_u8().unwrap();
    r.read_u8().unwrap();
    r.read_u8().unwrap();
    r.read_string().unwrap();
    r.read_u16().unwrap();
    let key_len = r.read_u32().unwrap() as usize;
    assert!(matches!(
        r.read_bytes(key_len),
        Err(WireError::Truncated {
            at: 16,
            needed: 2,
            available: 1
        })
    ));
}
