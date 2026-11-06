// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-corba-dds-bridge` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire`: `Endianness` throughout
//! `src/bridge_daemon.rs` and `src/csiv2_wire.rs` (the byte order of a
//! request, chosen for replies, IOR encapsulations and DDS samples).
//! `src/bridge_daemon.rs` writes the `TAG_SSL_SEC_TRANS` component of its
//! CSIv2 mechanism with `Writer::new(Endianness::Little)`: the byte-order
//! octet `1`, then three `write_u16` (supports, requires, port), and
//! `finish`; it decodes a `LOCATION_FORWARD` reply body with
//! `Reader::new(body, endianness)` handed to `Ior::decode`.
//! `src/naming_client.rs` reads the repository id of an exception reply
//! with `Reader::new(&reply.body, endianness)` and `read_string`.
//!
//! Oracle: CORBA 3.3 part 2 §9.3.1.1 (alignment to the size, counted from
//! the start of the encapsulation or body), §9.3.2.7 (string), §9.3.3
//! (encapsulation: byte-order octet 0 big-endian, 1 little-endian),
//! §9.3.5.5 (an exception reply body starts with the repository id),
//! §7.6.2 (IOR: `string type_id; sequence<TaggedProfile> profiles;`), and
//! the CSIv2 association options of §10 (`NoProtection` 1, `Integrity` 2,
//! `Confidentiality` 4).
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, Writer};

#[test]
fn corba_9_3_3_ssl_sec_trans_component_is_little_endian_shorts_from_offset_two() {
    let mut w = Writer::new(Endianness::Little);
    w.write_u8(1);
    w.write_u16(0x0001 | 0x0002 | 0x0004);
    w.write_u16(0);
    w.write_u16(6834);
    assert_eq!(w.finish(), [0x01, 0x00, 0x07, 0x00, 0x00, 0x00, 0xb2, 0x1a]);
}

#[test]
fn corba_9_3_2_7_a_reply_body_is_read_in_the_reply_byte_order_from_its_first_octet() {
    // A forwarded IOR's head and an exception's repository id, in both
    // byte orders.
    for (endianness, body) in [
        (
            Endianness::Big,
            [
                0x00, 0x00, 0x00, 0x02, b'X', 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
            ],
        ),
        (
            Endianness::Little,
            [
                0x02, 0x00, 0x00, 0x00, b'X', 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
            ],
        ),
    ] {
        let mut r = Reader::new(&body, endianness);
        assert_eq!(r.read_string().unwrap(), "X");
        assert_eq!(r.read_u32().unwrap(), 1, "profile count after padding");
        assert_eq!(r.remaining(), 0);
    }
}

#[test]
fn an_unreadable_repository_id_is_an_error_from_the_reader() {
    let mut r = Reader::new(&[0x00, 0x00, 0x00, 0x05, b'I', b'D'], Endianness::Big);
    assert!(r.read_string().is_err());
}
