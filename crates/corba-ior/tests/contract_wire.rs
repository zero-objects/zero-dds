// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-corba-ior` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire`: `src/ior.rs` encodes an IOR
//! into the public `encode(&mut Writer)` with `write_string` (type id),
//! `write_u32` (profile count) and, per profile (`src/tagged_profile.rs`),
//! `write_u32` (tag), `write_u32` + `write_bytes` (profile data); as an
//! encapsulation with `Writer::new(endianness)`, the byte-order octet first,
//! then `finish`. Decoding mirrors it with `Reader::new(bytes, endianness)`,
//! `read_u8`, `read_string`, `read_u32` and `read_bytes`.
//! `src/components.rs` writes and reads the tagged components' own
//! encapsulations (`write_u8`, `write_u16`, `write_u32`, `write_string`,
//! `as_bytes`; the matching reads). `Endianness` is in the public
//! signatures (`to_stringified`, `encode_encapsulation`), and `WireError`
//! in `IorError::Wire`.
//!
//! Oracle: CORBA 3.3 part 2 §9.3.1.1 (alignment to the size, counted from
//! the start of the encapsulation), §9.3.2.7 (string), §9.3.3
//! (encapsulation: the byte-order octet, 0 big-endian and 1
//! little-endian, then the contents), with the IOR structure of §7.6.2
//! (`string type_id; sequence<TaggedProfile> profiles;`, a `TaggedProfile`
//! being `ProfileId tag; sequence<octet> profile_data;`).
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, WireError, Writer};

/// An IOR of type `IDL:A:1.0` with one `TAG_INTERNET_IOP` profile whose
/// data is `aa bb cc`, as a little-endian encapsulation.
const IOR_LE: [u8; 35] = [
    0x01, // byte order: little-endian
    0x00, 0x00, 0x00, // padding
    0x0a, 0x00, 0x00, 0x00, b'I', b'D', b'L', b':', b'A', b':', b'1', b'.', b'0',
    0x00, // type_id
    0x00, 0x00, // padding
    0x01, 0x00, 0x00, 0x00, // one profile at 20
    0x00, 0x00, 0x00, 0x00, // TAG_INTERNET_IOP
    0x03, 0x00, 0x00, 0x00, 0xaa, 0xbb, 0xcc, // profile_data
];

#[test]
fn corba_9_3_3_ior_encapsulation_is_the_order_octet_then_type_id_and_profiles() {
    let mut w = Writer::new(Endianness::Little);
    w.write_u8(1);
    w.write_string("IDL:A:1.0");
    w.write_u32(1);
    w.write_u32(0);
    w.write_u32(3);
    w.write_bytes(&[0xaa, 0xbb, 0xcc]);
    assert_eq!(w.finish(), IOR_LE);

    let mut r = Reader::new(&IOR_LE, Endianness::Little);
    assert_eq!(r.read_u8().unwrap(), 1);
    assert_eq!(r.read_string().unwrap(), "IDL:A:1.0");
    assert_eq!(r.read_u32().unwrap(), 1);
    assert_eq!(r.read_u32().unwrap(), 0);
    let n = r.read_u32().unwrap() as usize;
    assert_eq!(r.read_bytes(n).unwrap(), [0xaa, 0xbb, 0xcc]);
    assert_eq!(r.remaining(), 0);
}

#[test]
fn corba_9_3_1_1_component_encapsulation_aligns_shorts_and_longs_from_its_first_octet() {
    // Three unsigned shorts behind the order octet: each on an even offset.
    let mut w = Writer::new(Endianness::Big);
    w.write_u8(0);
    w.write_u16(0x0066);
    w.write_u16(0x0002);
    w.write_u16(684);
    assert_eq!(
        w.as_bytes(),
        [0x00, 0x00, 0x00, 0x66, 0x00, 0x02, 0x02, 0xac]
    );

    // A code-set component: native set, then a sequence of conversion sets.
    let mut w = Writer::new(Endianness::Little);
    w.write_u8(1);
    w.write_u32(0x0501_0001);
    w.write_u32(1);
    w.write_u32(0x0001_0109);
    let bytes = w.as_bytes().to_vec();
    assert_eq!(
        bytes,
        [
            0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x05, 0x01, 0x00, 0x00, 0x00, 0x09, 0x01,
            0x01, 0x00,
        ]
    );
    let mut r = Reader::new(&bytes, Endianness::Little);
    r.read_u8().unwrap();
    assert_eq!(r.read_u32().unwrap(), 0x0501_0001);
    assert_eq!(r.read_u32().unwrap(), 1);
    assert_eq!(r.read_u32().unwrap(), 0x0001_0109);

    // An address component: a host string and an unsigned short port.
    let mut w = Writer::new(Endianness::Big);
    w.write_u8(0);
    w.write_string("h");
    w.write_u16(2809);
    assert_eq!(
        w.as_bytes(),
        [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, b'h', 0x00, 0x0a, 0xf9
        ]
    );
}

#[test]
fn a_profile_whose_data_runs_past_the_end_is_a_wire_error() {
    let mut r = Reader::new(&IOR_LE[..33], Endianness::Little);
    r.read_u8().unwrap();
    r.read_string().unwrap();
    r.read_u32().unwrap();
    r.read_u32().unwrap();
    let n = r.read_u32().unwrap() as usize;
    assert!(matches!(
        r.read_bytes(n),
        Err(WireError::Truncated { at: 32, .. })
    ));
}
