// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-endpoint-golden` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire` (`src/main.rs`): every golden is
//! written with `Writer::with_max_align(endian, 4)` (XCDR2) and `finish`.
//! `encode_final`, `encode_typedef`, `encode_array`, `encode_union`,
//! `encode_map`, `encode_wide` and `encode_longdouble` use `write_u8`,
//! `write_u16`, `write_u32`, `write_u64`, `write_string` and `write_bytes`;
//! `encode_nested` uses `write_dheader`/`finish_dheader`; `encode_mutable`
//! uses `write_emheader(id, 4, false)`/`finish_emheader` (LC 4 with a
//! NEXTINT, short length codes off); `encode_keyhash` uses `zero_padded` on
//! a big-endian holder and `encode_keyhash_md5` uses `md5`. The binary has
//! no library, so the tests call `zerodds-wire` with the same arguments.
//!
//! Oracle: DDS-XTypes 1.3 §7.4.1.1.1 Table 31, §7.4.1.1.2, §7.4.2
//! (PLAIN_CDR2 aligns 8-octet types to 4), §7.4.3.4.1 (DHEADER counts the
//! octets after it), §7.4.3.4.2 (EMHEADER1 = (M_FLAG << 31) + (LC << 28) +
//! id; LC 4: NEXTINT holds the member's length), §7.6.8 (KeyHash: holder
//! zero-padded to 16 when it fits, MD5 otherwise), RFC 1321 §A.5 (MD5 test
//! suite). The MD5 of the 20-octet holder below was computed with Python's
//! `hashlib`, not with this repository.
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Writer};

fn xcdr2(endian: Endianness) -> Writer {
    Writer::with_max_align(endian, 4)
}

fn sensor_reading(endian: Endianness) -> Vec<u8> {
    let mut w = xcdr2(endian);
    w.write_u32(0xA1B2_C3D4);
    w.write_u16(0x1234);
    w.write_u8(0x5A);
    w.write_u32(3.5f32.to_bits());
    w.write_u64(0x0102_0304_0506_0708);
    w.write_string("bay-12");
    w.write_u32(4);
    w.write_bytes(&[0xDE, 0xAD, 0xBE, 0xEF]);
    w.finish()
}

#[test]
fn xtypes_7_4_2_plain_cdr2_puts_the_uint64_on_a_four_octet_boundary() {
    assert_eq!(
        sensor_reading(Endianness::Little),
        [
            0xd4, 0xc3, 0xb2, 0xa1, // uint32 at 0
            0x34, 0x12, // uint16 at 4
            0x5a, 0x00, // octet at 6, one octet of padding
            0x00, 0x00, 0x60, 0x40, // float 3.5 at 8
            0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, // uint64 at 12, not 16
            0x07, 0x00, 0x00, 0x00, b'b', b'a', b'y', b'-', b'1', b'2', 0x00, // string at 20
            0x00, // padding to 32
            0x04, 0x00, 0x00, 0x00, 0xde, 0xad, 0xbe, 0xef, // sequence<octet> at 32
        ]
    );
    assert_eq!(
        sensor_reading(Endianness::Big),
        [
            0xa1, 0xb2, 0xc3, 0xd4, 0x12, 0x34, 0x5a, 0x00, 0x40, 0x60, 0x00, 0x00, 0x01, 0x02,
            0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x00, 0x00, 0x00, 0x07, b'b', b'a', b'y', b'-',
            b'1', b'2', 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0xde, 0xad, 0xbe, 0xef,
        ]
    );
}

fn inner(w: &mut Writer, a: u16, b: u32) {
    let dh = w.write_dheader();
    w.write_u16(a);
    w.write_u32(b);
    w.finish_dheader(dh);
}

#[test]
fn xtypes_7_4_3_4_1_dheader_counts_the_octets_after_it_at_every_level() {
    let mut w = xcdr2(Endianness::Little);
    let outer = w.write_dheader();
    w.write_u32(0xCAFE_BABE);
    inner(&mut w, 0x1111, 0x2222_3333);
    let many = w.write_dheader();
    w.write_u32(2);
    inner(&mut w, 0xAAAA, 0xBBBB_CCCC);
    inner(&mut w, 0xDDDD, 0xEEEE_FFFF);
    w.finish_dheader(many);
    w.write_string("nested");
    w.finish_dheader(outer);
    assert!(w.deferred_error().is_none());
    assert_eq!(
        w.finish(),
        [
            0x3b, 0x00, 0x00, 0x00, // outer DHEADER: 59
            0xbe, 0xba, 0xfe, 0xca, // id
            0x08, 0x00, 0x00, 0x00, 0x11, 0x11, 0x00, 0x00, 0x33, 0x33, 0x22, 0x22, // one
            0x1c, 0x00, 0x00, 0x00, // sequence DHEADER: 28
            0x02, 0x00, 0x00, 0x00, // count
            0x08, 0x00, 0x00, 0x00, 0xaa, 0xaa, 0x00, 0x00, 0xcc, 0xcc, 0xbb, 0xbb, // many[0]
            0x08, 0x00, 0x00, 0x00, 0xdd, 0xdd, 0x00, 0x00, 0xff, 0xff, 0xee, 0xee, // many[1]
            0x07, 0x00, 0x00, 0x00, b'n', b'e', b's', b't', b'e', b'd', 0x00, // s
        ]
    );
}

#[test]
fn xtypes_7_4_3_4_2_lc4_emheader_carries_a_nextint_with_the_member_length() {
    let mut w = xcdr2(Endianness::Little);
    let dh = w.write_dheader();
    let m10 = w.write_emheader(10, 4, false).unwrap();
    w.write_u32(0xDEAD_BEEF);
    w.finish_emheader(m10).unwrap();
    let m20 = w.write_emheader(20, 4, false).unwrap();
    w.write_string("mut");
    w.finish_emheader(m20).unwrap();
    let m30 = w.write_emheader(30, 4, false).unwrap();
    w.write_u16(0x0777);
    w.finish_emheader(m30).unwrap();
    w.finish_dheader(dh);
    assert_eq!(
        w.finish(),
        [
            0x26, 0x00, 0x00, 0x00, // DHEADER: 38
            0x0a, 0x00, 0x00, 0x40, 0x04, 0x00, 0x00, 0x00, 0xef, 0xbe, 0xad, 0xde, // @id(10)
            0x14, 0x00, 0x00, 0x40, 0x08, 0x00, 0x00, 0x00, // @id(20), NEXTINT 8
            0x04, 0x00, 0x00, 0x00, b'm', b'u', b't', 0x00, // "mut"
            0x1e, 0x00, 0x00, 0x40, 0x02, 0x00, 0x00, 0x00, 0x77, 0x07, // @id(30), NEXTINT 2
        ]
    );
}

#[test]
fn xtypes_7_6_8_keyhash_of_a_holder_of_at_most_16_octets_is_zero_padded() {
    let mut w = xcdr2(Endianness::Big);
    w.write_u32(0x0102_0304);
    w.write_u16(0x0506);
    assert_eq!(
        w.zero_padded(),
        [1, 2, 3, 4, 5, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
}

#[test]
fn xtypes_7_6_8_keyhash_of_a_longer_holder_is_the_md5_of_its_octets() {
    // RFC 1321 §A.5: MD5("abc").
    let mut abc = xcdr2(Endianness::Big);
    abc.write_bytes(b"abc");
    assert_eq!(
        abc.md5(),
        [
            0x90, 0x01, 0x50, 0x98, 0x3c, 0xd2, 0x4f, 0xb0, 0xd6, 0x96, 0x3f, 0x7d, 0x28, 0xe1,
            0x7f, 0x72,
        ]
    );

    // The golden's five `@key long`s, 20 octets big-endian.
    let mut w = xcdr2(Endianness::Big);
    for v in [
        0x0102_0304u32,
        0x0506_0708,
        0x090A_0B0C,
        0x0D0E_0F10,
        0x1112_1314,
    ] {
        w.write_u32(v);
    }
    assert_eq!(w.as_bytes(), (1..=20).collect::<Vec<u8>>());
    assert_eq!(
        w.md5(),
        [
            0x4d, 0x55, 0x55, 0xe0, 0x67, 0xdd, 0x97, 0xd0, 0x8f, 0xef, 0x90, 0x95, 0x9b, 0x15,
            0x10, 0xcb,
        ]
    );
}
