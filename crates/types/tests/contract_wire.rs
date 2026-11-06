// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-types` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire`, from its non-test sources (the
//! TypeObject codecs in `type_object/`, `type_identifier/`,
//! `type_information.rs`, `type_lookup.rs`, `type_object_v1/codec.rs`,
//! `framing.rs`, `hash.rs`, `qos.rs` and others):
//! - `Writer::with_max_align` / `Reader::with_max_align` with the caps four
//!   (XCDR2) and eight (XCDR1), `max_align()` and `endianness()` to tell the
//!   encoding version of a stream, `position`, `remaining`, `finish`.
//! - The primitives `write_/read_` `u8`, `i8`, `bool`, `i16`, `u16`, `i32`,
//!   `u32`, `i64`, `u64`, `f32`, `f64`, `f128`, `char` and `wchar`
//!   (`WcharPosition::Member`), `write_bytes` / `read_bytes`.
//! - `write_string` / `read_string`; `write_wstring_bounded` / `read_wstring`
//!   with `WstringLengthPolicy::SPECIFICATION`, `WstringUnitOrder::Stream`.
//! - XCDR2 framing: `write_dheader` / `finish_dheader` / `read_dheader`,
//!   `write_emheader` (length codes 4 and 5) / `finish_emheader` /
//!   `read_emheader` (`member_id`, `extent`).
//! - XCDR1 framing: `write_pid`, `write_pid_extended`, `finish_pid`,
//!   `write_sentinel`, `read_pid` / `read_pid_as`, `skip_parameter`, the
//!   `PidRead` flags `end`, `ignore`, `impl_extension`, `reserved`,
//!   `must_understand`; `ParameterRole::Member`.
//! - `md5` for the NameHash and the EquivalenceHash;
//!   `Writer::with_minimal_member_trailing_octet` and its getter.
//! - `WireError` in every codec signature.
//!
//! Oracles: RFC 1321 Appendix A.5 (MD5 test suite); CORBA 3.3 part 2
//! §9.3.1.1 and §9.3.1.2 (alignment, byte order), §9.3.2.7 (GIOP 1.2
//! `wstring`: an octet count, no terminator); IEEE 754-2008 binary128 and
//! CORBA Figure 9.2 (`long double` reversed as a whole); DDS-XTypes 1.3
//! §7.4.1.1.1 Table 31 (sizes, `char8` one octet, `char16` two),
//! §7.4.1.1.2 (string length counts the NUL; `wstring` length is twice the
//! characters), §7.4.1.2 Table 34 (PID_EXTENDED 0x3F01, PID_LIST_END 0x3F02,
//! PID_IGNORE 0x3F03, bit 14 must-understand, bit 15 FLAG_IMPL_EXTENSION;
//! the marker and the sentinel with the must-understand bit as Cyclone DDS
//! and RTI Connext write them), §7.4.2 (MAXALIGN 4 under version 2),
//! §7.4.3.4.1 (DHEADER) and §7.4.3.4.2 (EMHEADER: M_FLAG bit 31, LC bits
//! 28..30, member id the low 28 bits; the extent of each length code).
#![allow(clippy::unwrap_used)]

use zerodds_float128::Float128;
use zerodds_wire::{
    Endianness, ParameterRole, Reader, WcharPosition, WirePosition, Writer, WstringLengthPolicy,
    WstringUnitOrder, md5,
};

fn xcdr2(endianness: Endianness) -> Writer {
    Writer::with_max_align(endianness, 4)
}

fn xcdr1(endianness: Endianness) -> Writer {
    Writer::with_max_align(endianness, 8)
}

#[test]
fn rfc_1321_md5_and_the_name_hash() {
    // RFC 1321 A.5.
    assert_eq!(
        md5(b""),
        [
            0xd4, 0x1d, 0x8c, 0xd9, 0x8f, 0x00, 0xb2, 0x04, 0xe9, 0x80, 0x09, 0x98, 0xec, 0xf8,
            0x42, 0x7e,
        ]
    );
    assert_eq!(
        md5(b"abc"),
        [
            0x90, 0x01, 0x50, 0x98, 0x3c, 0xd2, 0x4f, 0xb0, 0xd6, 0x96, 0x3f, 0x7d, 0x28, 0xe1,
            0x7f, 0x72,
        ]
    );
    // XTypes 1.3 §7.3.4.4: a NameHash is the first four octets of MD5 of
    // the name; MD5("id") computed with Python's hashlib.
    assert_eq!(md5(b"id")[..4], [0xb8, 0x0b, 0xb7, 0x74]);
}

#[test]
fn xtypes_7_4_1_1_1_primitives_in_both_byte_orders() {
    let mut w = xcdr2(Endianness::Little);
    w.write_u8(0xa1);
    w.write_i8(-2);
    w.write_bool(true);
    w.write_i16(-2);
    w.write_u16(0x0102);
    w.write_i32(-2);
    w.write_u32(0x0304_0506);
    w.write_i64(-2);
    w.write_u64(0x0708_090a_0b0c_0d0e);
    w.write_f32(1.0);
    w.write_f64(1.0);
    #[rustfmt::skip]
    let little = [
        0xa1, 0xfe, 0x01, 0x00,             // u8, i8, bool, padding to two
        0xfe, 0xff, 0x02, 0x01,             // i16, u16
        0xfe, 0xff, 0xff, 0xff,             // i32
        0x06, 0x05, 0x04, 0x03,             // u32
        0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, // i64
        0x0e, 0x0d, 0x0c, 0x0b, 0x0a, 0x09, 0x08, 0x07, // u64
        0x00, 0x00, 0x80, 0x3f,             // f32 1.0
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0x3f, // f64 1.0
    ];
    assert_eq!(w.finish(), little);

    #[rustfmt::skip]
    let big = [
        0xa1, 0xfe, 0x01, 0x00,
        0xff, 0xfe, 0x01, 0x02,
        0xff, 0xff, 0xff, 0xfe,
        0x03, 0x04, 0x05, 0x06,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe,
        0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
        0x3f, 0x80, 0x00, 0x00,
        0x3f, 0xf0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    for (bytes, endianness) in [(&little, Endianness::Little), (&big, Endianness::Big)] {
        let mut r = Reader::with_max_align(bytes, endianness, 4);
        assert_eq!(r.read_u8().unwrap(), 0xa1);
        assert_eq!(r.read_i8().unwrap(), -2);
        assert!(r.read_bool().unwrap());
        assert_eq!(r.read_i16().unwrap(), -2);
        assert_eq!(r.read_u16().unwrap(), 0x0102);
        assert_eq!(r.read_i32().unwrap(), -2);
        assert_eq!(r.read_u32().unwrap(), 0x0304_0506);
        assert_eq!(r.read_i64().unwrap(), -2);
        assert_eq!(r.read_u64().unwrap(), 0x0708_090a_0b0c_0d0e);
        assert_eq!(r.read_f32().unwrap().to_bits(), 1.0f32.to_bits());
        assert_eq!(r.read_f64().unwrap().to_bits(), 1.0f64.to_bits());
        assert_eq!(r.remaining(), 0);
        assert!(r.read_u8().is_err(), "past the end is an error");
    }
    let mut w = xcdr2(Endianness::Big);
    w.write_u8(0xa1);
    w.write_i8(-2);
    w.write_bool(true);
    w.write_i16(-2);
    w.write_u16(0x0102);
    w.write_i32(-2);
    w.write_u32(0x0304_0506);
    w.write_i64(-2);
    w.write_u64(0x0708_090a_0b0c_0d0e);
    w.write_f32(1.0);
    w.write_f64(1.0);
    assert_eq!(w.finish(), big);

    // A boolean is 0 or 1.
    let mut r = Reader::with_max_align(&[0x02], Endianness::Little, 4);
    assert!(r.read_bool().is_err());
}

#[test]
fn xtypes_7_4_2_the_alignment_cap_is_four_for_version_2_and_eight_for_version_1() {
    for (cap, padding) in [(4usize, 0usize), (8, 4)] {
        let mut w = Writer::with_max_align(Endianness::Little, cap);
        assert_eq!(w.max_align(), cap);
        assert_eq!(w.endianness(), Endianness::Little);
        w.write_u32(1);
        w.write_u64(2);
        assert_eq!(w.position(), 4 + padding + 8, "cap {cap}");
        let bytes = w.finish();
        let mut r = Reader::with_max_align(&bytes, Endianness::Little, cap);
        assert_eq!(r.max_align(), cap);
        assert_eq!(r.endianness(), Endianness::Little);
        assert_eq!(r.read_u32().unwrap(), 1);
        assert_eq!(r.read_u64().unwrap(), 2);
        assert_eq!(r.position(), bytes.len());
    }
}

#[test]
fn xtypes_7_4_1_1_1_long_double_is_binary128_reversed_as_a_whole() {
    // 1.0: sign 0, exponent 0x3fff, fraction 0.
    let one = Float128::from_bits(0x3fff_u128 << 112);
    let mut w = xcdr2(Endianness::Big);
    w.write_u32(0);
    w.write_f128(one);
    let mut expected = vec![0x00, 0x00, 0x00, 0x00, 0x3f, 0xff];
    expected.extend_from_slice(&[0x00; 14]);
    assert_eq!(w.finish(), expected, "aligned to four under version 2");

    let mut w = xcdr1(Endianness::Little);
    w.write_u32(0);
    w.write_f128(one);
    let mut expected = vec![0x00; 8]; // the u32 and four octets of padding
    expected.extend_from_slice(&[0x00; 14]);
    expected.extend_from_slice(&[0xff, 0x3f]);
    let bytes = w.finish();
    assert_eq!(bytes, expected, "aligned to eight under version 1");
    let mut r = Reader::with_max_align(&bytes, Endianness::Little, 8);
    assert_eq!(r.read_u32().unwrap(), 0);
    assert_eq!(r.read_f128().unwrap().to_bits(), one.to_bits());
}

#[test]
fn xtypes_7_4_1_1_2_string_length_counts_the_terminating_nul() {
    let mut w = xcdr2(Endianness::Little);
    w.write_string("Robot");
    let bytes = w.finish();
    assert_eq!(
        bytes,
        [0x06, 0x00, 0x00, 0x00, b'R', b'o', b'b', b'o', b't', 0x00]
    );
    let mut r = Reader::with_max_align(&bytes, Endianness::Little, 4);
    assert_eq!(r.read_string().unwrap(), "Robot");
    // A last octet that is not NUL is refused.
    let bad = [0x02, 0x00, 0x00, 0x00, b'a', b'b'];
    assert!(
        Reader::with_max_align(&bad, Endianness::Little, 4)
            .read_string()
            .is_err()
    );
    // A length the stream does not hold is refused.
    let long = [0xff, 0x00, 0x00, 0x00, b'a', 0x00];
    assert!(
        Reader::with_max_align(&long, Endianness::Little, 4)
            .read_string()
            .is_err()
    );
}

#[test]
fn corba_9_3_2_7_wstring_is_an_octet_count_and_utf16_units_without_terminator() {
    // "Aé": U+0041, U+00E9, two units of two octets.
    let mut w = xcdr2(Endianness::Little);
    w.write_wstring_bounded(
        "A\u{e9}",
        256,
        WstringLengthPolicy::SPECIFICATION,
        WstringUnitOrder::Stream,
        WirePosition::Member,
    )
    .unwrap();
    let bytes = w.finish();
    assert_eq!(bytes, [0x04, 0x00, 0x00, 0x00, 0x41, 0x00, 0xe9, 0x00]);
    let mut r = Reader::with_max_align(&bytes, Endianness::Little, 4);
    assert_eq!(
        r.read_wstring(
            WstringLengthPolicy::SPECIFICATION,
            WstringUnitOrder::Stream,
            WirePosition::Member,
        )
        .unwrap(),
        "A\u{e9}"
    );
    // Over its bound: refused, not truncated.
    let mut w = xcdr2(Endianness::Little);
    assert!(
        w.write_wstring_bounded(
            "abc",
            2,
            WstringLengthPolicy::SPECIFICATION,
            WstringUnitOrder::Stream,
            WirePosition::Member,
        )
        .is_err()
    );
}

#[test]
fn xtypes_7_4_1_1_1_char8_is_one_octet_and_char16_two() {
    let mut w = xcdr2(Endianness::Little);
    w.write_char('A').unwrap();
    w.write_wchar('\u{e9}', WcharPosition::Member).unwrap();
    let bytes = w.finish();
    assert_eq!(bytes, [0x41, 0x00, 0xe9, 0x00], "char16 aligned to two");
    let mut r = Reader::with_max_align(&bytes, Endianness::Little, 4);
    assert_eq!(r.read_char().unwrap(), 'A');
    assert_eq!(r.read_wchar(WcharPosition::Member).unwrap(), '\u{e9}');
    // No octet stands for U+0100, no code unit for a code point past the BMP.
    assert!(xcdr2(Endianness::Little).write_char('\u{100}').is_err());
    assert!(
        xcdr2(Endianness::Little)
            .write_wchar('\u{1f600}', WcharPosition::Member)
            .is_err()
    );
}

#[test]
fn octets_go_through_unchanged_and_a_short_read_is_an_error() {
    let mut w = xcdr2(Endianness::Big);
    w.write_bytes(&[1, 2, 3]);
    assert_eq!(w.as_bytes(), [1, 2, 3]);
    let bytes = w.finish();
    let mut r = Reader::with_max_align(&bytes, Endianness::Big, 4);
    assert_eq!(r.read_bytes(2).unwrap(), [1, 2]);
    assert_eq!(r.remaining(), 1);
    assert!(r.read_bytes(2).is_err());
}

#[test]
fn xtypes_7_4_3_4_1_dheader_is_the_length_of_what_follows() {
    for (endianness, header) in [
        (Endianness::Little, [0x06, 0x00, 0x00, 0x00]),
        (Endianness::Big, [0x00, 0x00, 0x00, 0x06]),
    ] {
        let mut w = xcdr2(endianness);
        let h = w.write_dheader();
        w.write_u32(1);
        w.write_u16(2);
        w.finish_dheader(h);
        assert!(w.deferred_error().is_none());
        let bytes = w.finish();
        assert_eq!(bytes[..4], header);
        assert_eq!(bytes.len(), 10);
        let mut r = Reader::with_max_align(&bytes, endianness, 4);
        assert_eq!(r.read_dheader().unwrap(), 6);
    }
    // A DHEADER claiming more than the stream holds.
    let mut r = Reader::with_max_align(&[0x64, 0x00, 0x00, 0x00, 0x01], Endianness::Little, 4);
    assert!(r.read_dheader().is_err());
}

#[test]
fn xtypes_7_4_3_4_2_emheader_is_flag_length_code_and_member_id() {
    // LC 4: the NEXTINT follows and holds the member's length.
    let mut w = xcdr2(Endianness::Little);
    let h = w.write_emheader(0x0c53_6065, 4, false).unwrap();
    w.write_u32(1);
    w.write_u16(2);
    w.finish_emheader(h).unwrap();
    assert_eq!(
        w.finish(),
        [
            0x65, 0x60, 0x53, 0x4c, // 0x4c536065: M = 0, LC = 4
            0x06, 0x00, 0x00, 0x00, // NEXTINT
            0x01, 0x00, 0x00, 0x00, 0x02, 0x00,
        ]
    );

    // LC 5 with M_FLAG: no NEXTINT of its own, the member's first word is it.
    let mut w = xcdr2(Endianness::Little);
    let h = w.write_emheader(1, 5, true).unwrap();
    let d = w.write_dheader();
    w.write_u8(9);
    w.finish_dheader(d);
    w.finish_emheader(h).unwrap();
    assert_eq!(
        w.finish(),
        [0x01, 0x00, 0x00, 0xd0, 0x01, 0x00, 0x00, 0x00, 0x09]
    );

    // LC 2 declares four octets; a member of two misdeclares it.
    let mut w = xcdr2(Endianness::Little);
    let h = w.write_emheader(7, 2, false).unwrap();
    w.write_u16(1);
    assert!(w.finish_emheader(h).is_err());

    // The member id has 28 bits.
    assert!(
        xcdr2(Endianness::Little)
            .write_emheader(0x1000_0000, 4, false)
            .is_err()
    );
}

#[test]
fn xtypes_7_4_3_4_2_the_extent_of_every_length_code() {
    let cases: [(&[u8], u8, usize); 7] = [
        (&[0x03, 0, 0, 0x00, 0xaa], 0, 1),
        (&[0x03, 0, 0, 0x10, 0xaa, 0xaa], 1, 2),
        (&[0x03, 0, 0, 0x20, 1, 2, 3, 4], 2, 4),
        (&[0x03, 0, 0, 0x30, 1, 2, 3, 4, 5, 6, 7, 8], 3, 8),
        // LC 5: 4 + NEXTINT; LC 6: 4 + 4 * NEXTINT; LC 7: 4 + 8 * NEXTINT.
        (&[0x03, 0, 0, 0x50, 0x02, 0, 0, 0, 0xaa, 0xaa], 5, 6),
        (
            &[0x03, 0, 0, 0x60, 0x02, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8],
            6,
            12,
        ),
        (
            &[0x03, 0, 0, 0x70, 0x01, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8],
            7,
            12,
        ),
    ];
    for (bytes, code, extent) in cases {
        let mut r = Reader::with_max_align(bytes, Endianness::Little, 4);
        let h = r.read_emheader().unwrap();
        assert_eq!((h.member_id, h.length_code, h.extent), (3, code, extent));
        assert!(!h.must_understand);
        // The member starts here: for LC 5 to 7 at the NEXTINT itself.
        assert_eq!(r.position(), 4, "LC {code}");
        assert_eq!(r.remaining(), extent, "LC {code}");
    }
    // LC 4 consumes its NEXTINT; M_FLAG is bit 31.
    let bytes = [0x03, 0, 0, 0xc0, 0x02, 0, 0, 0, 0xaa, 0xaa];
    let mut r = Reader::with_max_align(&bytes, Endianness::Little, 4);
    let h = r.read_emheader().unwrap();
    assert_eq!((h.member_id, h.length_code, h.extent), (3, 4, 2));
    assert!(h.must_understand);
    assert_eq!(r.position(), 8);
    // A NEXTINT past the end of the stream.
    let bytes = [0x03, 0, 0, 0x40, 0xff, 0xff, 0xff, 0xff];
    assert!(
        Reader::with_max_align(&bytes, Endianness::Little, 4)
            .read_emheader()
            .is_err()
    );
}

#[test]
fn xtypes_7_4_1_2_parameter_list_headers_and_the_sentinel() {
    let mut w = xcdr1(Endianness::Little);
    let h = w.write_pid(5, ParameterRole::Member, false).unwrap();
    w.write_u32(0x0102_0304);
    w.finish_pid(h);
    let h = w.write_pid(6, ParameterRole::Member, true).unwrap();
    w.write_u16(7);
    w.finish_pid(h);
    let h = w
        .write_pid_extended(0x0c53_6065, ParameterRole::Member, false)
        .unwrap();
    w.write_u32(9);
    w.finish_pid(h);
    w.write_sentinel();
    let bytes = w.finish();
    #[rustfmt::skip]
    let expected = [
        0x05, 0x00, 0x04, 0x00, 0x04, 0x03, 0x02, 0x01, // id 5, length 4
        0x06, 0x40, 0x02, 0x00, 0x07, 0x00, 0x00, 0x00, // id 6 | must-understand, length 2, padding
        0x01, 0x7f, 0x08, 0x00,                         // PID_EXTENDED | must-understand, 8
        0x65, 0x60, 0x53, 0x0c, 0x04, 0x00, 0x00, 0x00, // member id, length 4
        0x09, 0x00, 0x00, 0x00,
        0x02, 0x7f, 0x00, 0x00,                         // PID_LIST_END | must-understand
    ];
    assert_eq!(bytes, expected);

    let mut r = Reader::with_max_align(&bytes, Endianness::Little, 8);
    let p = r.read_pid().unwrap();
    assert_eq!(
        (p.pid, p.length, p.end, p.must_understand),
        (5, 4, false, false)
    );
    assert_eq!(r.read_u32().unwrap(), 0x0102_0304);
    r.finish_pid(p).unwrap();
    let p = r.read_pid_as(ParameterRole::Member).unwrap();
    assert_eq!((p.pid, p.length, p.must_understand), (6, 2, true));
    assert_eq!(r.read_u16().unwrap(), 7);
    r.finish_pid(p).unwrap();
    let p = r.read_pid().unwrap();
    assert_eq!((p.pid, p.length, p.end), (0x0c53_6065, 4, false));
    assert!(!p.reserved, "the extended header has no reserved range");
    r.skip_parameter(p).unwrap();
    let p = r.read_pid().unwrap();
    assert!(p.end);
    assert_eq!(r.remaining(), 0);

    // A short header does not carry an id above 0x3F00.
    assert!(
        xcdr1(Endianness::Little)
            .write_pid(0x3f01, ParameterRole::Member, false)
            .is_err()
    );
}

#[test]
fn xtypes_7_4_1_2_ignore_and_impl_extension_parameters_are_no_members() {
    #[rustfmt::skip]
    let bytes = [
        0x03, 0x3f, 0x00, 0x00,                         // PID_IGNORE
        0x05, 0x80, 0x04, 0x00, 0xaa, 0xaa, 0xaa, 0xaa, // id 5 | FLAG_IMPL_EXTENSION
        0x02, 0x7f, 0x00, 0x00,
    ];
    let mut r = Reader::with_max_align(&bytes, Endianness::Little, 8);
    let p = r.read_pid().unwrap();
    assert!(p.ignore);
    assert!(!p.is_user_member());
    r.skip_parameter(p).unwrap();
    let p = r.read_pid().unwrap();
    assert_eq!(p.pid, 5);
    assert!(p.impl_extension);
    assert!(!p.is_user_member());
    r.skip_parameter(p).unwrap();
    assert!(r.read_pid().unwrap().end);
}

#[test]
fn the_minimal_member_trailing_octet_setting_reads_back() {
    // type_object/ reads the camp off the writer it is handed.
    assert!(!xcdr2(Endianness::Little).minimal_member_trailing_octet());
    assert!(
        xcdr2(Endianness::Little)
            .with_minimal_member_trailing_octet(true)
            .minimal_member_trailing_octet()
    );
}
