// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-wire-codec` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire` (`src/encode.rs`, `src/decode.rs`,
//! `src/value.rs`, `src/canonical.rs`): the plan walk writes and reads every
//! value through `Writer` and `Reader` — the primitives, `string`, `char`,
//! `wchar`, `f128`, `write_fixed` / `read_fixed` with `FixedDigits` and
//! `parse_fixed` for the canonical form; the fixed-order units
//! `write_/read_u16_little`, `_u16_big`, `_u32_little`, `_u32_big`; the
//! framing calls (DHEADER, EMHEADER, PID, sentinel, `skip_to`); the
//! dialect settings `with_wchar_width`, `with_wchar_framing`,
//! `with_wstring_unit_order`, `with_pl_cdr1_framing` (`PlCdr1Framing`,
//! `PidFlagPolicy`), `with_pl_cdr1_reading`, `with_short_length_codes`;
//! `begin_/finish_packed_element`; `Writer::continuing`,
//! `Writer::little_endian`, `with_max_align`; `recording_padding` and
//! `padding_spans`; `WireError::OutOfRange { expected, .. }`, which
//! `encode.rs::refused` takes apart; `reader::PidRead` held in an enum.
//!
//! Oracles: CORBA 3.3 part 2 §9.3.1.1 (alignment), §9.3.1.6 (GIOP 1.2
//! `wchar`: an octet count, then the octets), §9.3.2.8 (`fixed` as packed
//! BCD, a leading zero nibble for an even digit count, sign 0xC or 0xD
//! last); DDS-XTypes 1.3 §7.4.1.1.1 Table 31, §7.4.1.2 (parameter list
//! headers), §7.4.3.4.2 (length codes 0 to 3 are 1, 2, 4, 8 octets).
//! Vendor measurements documented in `zerodds-wire`: the PL_CDR1 flags
//! Cyclone DDS and RTI Connext write (`7f01`, `7f02`), Fast DDS (`3f01`,
//! `3f02`, bare ids), OpenDDS (`ff01`, `c0xx` on every member, `3f02`);
//! Cyclone and RTI rounding a `@mutable` member's length to four; Fast
//! DDS shortening an LC4 member of 8 octets to LC 3 (`02000030`); RTI's
//! four-octet `wchar` under XCDR1; omniORB's big-endian scalar `wchar`.
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_wire::{
    Endianness, FixedDigits, ParameterRole, PidFlagPolicy, PlCdr1Framing, PlCdr1Reading, Reader,
    WcharFraming, WcharPosition, WcharWidth, WireError, Writer, WstringUnitOrder, parse_fixed,
};

#[test]
fn xtypes_7_4_1_1_1_the_primitives_the_walk_writes() {
    let mut w = Writer::with_max_align(Endianness::Big, 8);
    w.write_bool(false);
    w.write_i8(-1);
    w.write_i16(-2);
    w.write_i32(-3);
    w.write_i64(-4);
    w.write_f32(-1.0);
    w.write_f64(-1.0);
    w.write_string("ab");
    #[rustfmt::skip]
    let expected = [
        0x00, 0xff, 0xff, 0xfe,                         // bool, i8, i16
        0xff, 0xff, 0xff, 0xfd,                         // i32
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfc, // i64 at eight
        0xbf, 0x80, 0x00, 0x00,                         // f32 -1.0
        0x00, 0x00, 0x00, 0x00,                         // padding to eight
        0xbf, 0xf0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // f64 -1.0
        0x00, 0x00, 0x00, 0x03, b'a', b'b', 0x00,       // string
    ];
    let bytes = w.finish();
    assert_eq!(bytes, expected);
    let mut r = Reader::with_max_align(&bytes, Endianness::Big, 8);
    assert!(!r.read_bool().unwrap());
    assert_eq!(r.read_i8().unwrap(), -1);
    assert_eq!(r.read_i16().unwrap(), -2);
    assert_eq!(r.read_i32().unwrap(), -3);
    assert_eq!(r.read_i64().unwrap(), -4);
    assert_eq!(r.read_f32().unwrap().to_bits(), (-1.0f32).to_bits());
    assert_eq!(r.read_f64().unwrap().to_bits(), (-1.0f64).to_bits());
    assert_eq!(r.read_string().unwrap(), "ab");
    assert_eq!(Writer::little_endian().endianness(), Endianness::Little);
}

#[test]
fn corba_9_3_2_8_fixed_is_packed_bcd_with_the_sign_last() {
    for (text, digits, scale, octets) in [
        // Five digits and the sign: six nibbles.
        ("123.45", 5, 2, &[0x12, 0x34, 0x5c][..]),
        // Four digits: a leading zero nibble.
        ("1.5", 4, 2, &[0x00, 0x15, 0x0c][..]),
        ("-1.5", 3, 1, &[0x01, 0x5d][..]),
        // Zero is positive.
        ("0", 1, 0, &[0x0c][..]),
    ] {
        let mut w = Writer::little_endian();
        w.write_fixed(text, digits, scale, FixedDigits::Declared)
            .unwrap();
        let bytes = w.finish();
        assert_eq!(bytes, octets, "{text} as fixed<{digits},{scale}>");
        let mut r = Reader::new(&bytes, Endianness::Little);
        let read = r.read_fixed(digits, scale, FixedDigits::Declared).unwrap();
        assert_eq!(
            parse_fixed(&read, digits, scale, 0).unwrap().negative,
            text.starts_with('-')
        );
        assert_eq!(r.remaining(), 0);
    }
    // A value its declaration does not hold is refused, not truncated.
    assert!(
        Writer::little_endian()
            .write_fixed("1234.5", 5, 2, FixedDigits::Declared)
            .is_err()
    );
    let parts = parse_fixed("-12.5", 5, 2, 0).unwrap();
    assert!(parts.negative);
    assert_eq!((parts.whole, parts.fraction), ("12", "5"));
    assert!(parse_fixed("1.2.3", 5, 2, 0).is_err());
}

#[test]
fn fixed_order_units_ignore_the_stream_order() {
    let mut w = Writer::with_max_align(Endianness::Big, 4);
    w.write_u16_little(0x0102);
    w.write_u16_big(0x0304);
    w.write_u32_little(0x0506_0708);
    w.write_u32_big(0x090a_0b0c);
    let bytes = w.finish();
    assert_eq!(
        bytes,
        [
            0x02, 0x01, 0x03, 0x04, 0x08, 0x07, 0x06, 0x05, 0x09, 0x0a, 0x0b, 0x0c
        ]
    );
    let mut r = Reader::with_max_align(&bytes, Endianness::Little, 4);
    assert_eq!(r.read_u16_little().unwrap(), 0x0102);
    assert_eq!(r.read_u16_big().unwrap(), 0x0304);
    assert_eq!(r.read_u32_little().unwrap(), 0x0506_0708);
    assert_eq!(r.read_u32_big().unwrap(), 0x090a_0b0c);
}

#[test]
fn corba_9_3_1_6_wchar_widths_and_framings() {
    // Table 31: two octets, aligned to two.
    let mut w = Writer::little_endian();
    w.write_u8(0);
    w.write_wchar('\u{e4}', WcharPosition::Member).unwrap();
    assert_eq!(w.finish(), [0x00, 0x00, 0xe4, 0x00]);

    // RTI under XCDR1: four octets, aligned to four.
    let mut w = Writer::little_endian().with_wchar_width(WcharWidth::Four);
    w.write_u8(0);
    w.write_wchar('\u{e4}', WcharPosition::Member).unwrap();
    let bytes = w.finish();
    assert_eq!(bytes, [0x00, 0x00, 0x00, 0x00, 0xe4, 0x00, 0x00, 0x00]);
    let mut r = Reader::new(&bytes, Endianness::Little).with_wchar_width(WcharWidth::Four);
    r.read_u8().unwrap();
    assert_eq!(r.read_wchar(WcharPosition::Member).unwrap(), '\u{e4}');

    // GIOP 1.2: the octet count, then the octets, aligned to one; in the
    // stream's order, or big-endian where the unit order is not the
    // stream's (omniORB, JacORB).
    let giop = Writer::little_endian().with_wchar_framing(WcharFraming::GiopLengthPrefixed);
    let mut w = giop.clone();
    w.write_wchar('\u{e4}', WcharPosition::Member).unwrap();
    let bytes = w.finish();
    assert_eq!(bytes, [0x02, 0xe4, 0x00]);
    let mut r = Reader::new(&bytes, Endianness::Little)
        .with_wchar_framing(WcharFraming::GiopLengthPrefixed);
    assert_eq!(r.read_wchar(WcharPosition::Member).unwrap(), '\u{e4}');
    let mut w = giop.with_wstring_unit_order(WstringUnitOrder::BigEndianAlways);
    w.write_wchar('\u{e4}', WcharPosition::Member).unwrap();
    assert_eq!(w.finish(), [0x02, 0x00, 0xe4]);

    // TAO: the count on a member only, a bare unit in a collection.
    let mut w =
        Writer::little_endian().with_wchar_framing(WcharFraming::GiopLengthPrefixedMemberOnly);
    w.write_wchar('\u{e4}', WcharPosition::Element).unwrap();
    assert_eq!(w.finish(), [0xe4, 0x00]);
}

#[test]
fn packed_elements_align_from_their_own_first_octet() {
    let mut w = Writer::with_max_align(Endianness::Little, 4);
    w.write_u8(1);
    let saved = w.begin_packed_element();
    w.write_u8(2);
    w.write_u32(3); // aligned from the element's first octet (offset 1)
    w.finish_packed_element(saved);
    w.write_u32(4); // aligned from the stream again
    #[rustfmt::skip]
    let expected = [
        0x01,
        0x02, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00,
        0x04, 0x00, 0x00, 0x00,
    ];
    let bytes = w.finish();
    assert_eq!(bytes, expected);
    let mut r = Reader::with_max_align(&bytes, Endianness::Little, 4);
    assert_eq!(r.read_u8().unwrap(), 1);
    let saved = r.begin_packed_element();
    assert_eq!(r.read_u8().unwrap(), 2);
    assert_eq!(r.read_u32().unwrap(), 3);
    r.finish_packed_element(saved);
    assert_eq!(r.read_u32().unwrap(), 4);
}

#[test]
fn a_continuing_writer_counts_alignment_from_the_start_of_the_prefix() {
    let mut w = Writer::continuing(Endianness::Little, 8, vec![0xaa; 5]);
    w.write_u64(1);
    let bytes = w.finish();
    assert_eq!(bytes.len(), 16, "three octets of padding to offset 8");
    assert_eq!(bytes[..5], [0xaa; 5]);
    assert_eq!(bytes[5..8], [0; 3]);
    assert_eq!(bytes[8..], 1u64.to_le_bytes());
}

#[test]
fn xtypes_7_4_3_4_2_a_short_length_code_replaces_the_nextint_where_it_fits() {
    // Fast DDS: sequence<short> of two elements, eight octets, `02000030`.
    let mut w = Writer::with_max_align(Endianness::Little, 4).with_short_length_codes(true);
    let h = w.write_emheader(2, 4, false).unwrap();
    w.write_u32(2);
    w.write_i16(1);
    w.write_i16(2);
    w.finish_emheader(h).unwrap();
    assert_eq!(
        w.finish(),
        [
            0x02, 0x00, 0x00, 0x30, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x02, 0x00
        ]
    );
    // Three elements are ten octets: LC 4 stays, with its NEXTINT.
    let mut w = Writer::with_max_align(Endianness::Little, 4).with_short_length_codes(true);
    let h = w.write_emheader(2, 4, false).unwrap();
    w.write_u32(3);
    w.write_i16(1);
    w.write_i16(2);
    w.write_i16(3);
    w.finish_emheader(h).unwrap();
    let bytes = w.finish();
    assert_eq!(bytes[..8], [0x02, 0x00, 0x00, 0x40, 0x0a, 0x00, 0x00, 0x00]);
    assert_eq!(bytes.len(), 18);
    // Inside a DHEADER the length is counted after the rewrite.
    let mut w = Writer::with_max_align(Endianness::Little, 4).with_short_length_codes(true);
    let d = w.write_dheader();
    let h = w.write_emheader(1, 4, false).unwrap();
    w.write_u32(7);
    w.finish_emheader(h).unwrap();
    w.finish_dheader(d);
    assert_eq!(
        w.finish(),
        [
            0x08, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x20, 0x07, 0x00, 0x00, 0x00
        ]
    );
}

fn framing(pid_flags: PidFlagPolicy) -> PlCdr1Framing {
    PlCdr1Framing {
        pid_flags,
        ..PlCdr1Framing::DEFAULT
    }
}

/// One `@mutable` member `@id(1) long`, one with id 0x3F05 through the
/// extended header, and the sentinel.
fn list(framing: PlCdr1Framing) -> Vec<u8> {
    let mut w = Writer::with_max_align(Endianness::Little, 8).with_pl_cdr1_framing(framing);
    let h = w.write_pid(1, ParameterRole::Member, false).unwrap();
    w.write_i32(5);
    w.finish_pid(h);
    let h = w
        .write_pid_extended(0x3f05, ParameterRole::Member, false)
        .unwrap();
    w.write_i32(6);
    w.finish_pid(h);
    w.write_sentinel();
    w.finish()
}

#[test]
fn xtypes_7_4_1_2_the_three_measured_parameter_list_flag_conventions() {
    #[rustfmt::skip]
    let tail = |marker: [u8; 2], sentinel: [u8; 2]| -> Vec<u8> {
        let mut v = vec![
            marker[0], marker[1], 0x08, 0x00, 0x05, 0x3f, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00,
            0x06, 0x00, 0x00, 0x00,
        ];
        v.extend_from_slice(&[sentinel[0], sentinel[1], 0x00, 0x00]);
        v
    };
    // Cyclone DDS, RTI Connext: bare member, 7f01, 7f02.
    let mut cyclone = vec![0x01, 0x00, 0x04, 0x00, 0x05, 0x00, 0x00, 0x00];
    cyclone.extend(tail([0x01, 0x7f], [0x02, 0x7f]));
    assert_eq!(list(PlCdr1Framing::DEFAULT), cyclone);
    // Fast DDS: bare everywhere.
    let mut fast = vec![0x01, 0x00, 0x04, 0x00, 0x05, 0x00, 0x00, 0x00];
    fast.extend(tail([0x01, 0x3f], [0x02, 0x3f]));
    assert_eq!(list(framing(PidFlagPolicy::Bare)), fast);
    // OpenDDS: impl-extension and must-understand on every member, ff01,
    // a bare sentinel.
    let mut open = vec![0x01, 0xc0, 0x04, 0x00, 0x05, 0x00, 0x00, 0x00];
    open.extend(tail([0x01, 0xff], [0x02, 0x3f]));
    assert_eq!(list(framing(PidFlagPolicy::ImplExtensionOnMembers)), open);

    // A strict reader of one convention reports another; a tolerant one
    // reads it.
    let read_all = |reader: &mut Reader<'_>| -> Result<Vec<u32>, WireError> {
        let mut ids = Vec::new();
        loop {
            let p = reader.read_pid()?;
            if p.end {
                return Ok(ids);
            }
            ids.push(p.pid);
            reader.skip_parameter(p)?;
        }
    };
    let mut strict = Reader::with_max_align(&fast, Endianness::Little, 8);
    assert!(matches!(
        read_all(&mut strict),
        Err(WireError::WrongFraming { .. })
    ));
    let mut tolerant = Reader::with_max_align(&fast, Endianness::Little, 8)
        .with_pl_cdr1_reading(PlCdr1Reading::Tolerant);
    assert_eq!(read_all(&mut tolerant).unwrap(), [1, 0x3f05]);
    let mut matching = Reader::with_max_align(&fast, Endianness::Little, 8)
        .with_pl_cdr1_framing(framing(PidFlagPolicy::Bare));
    assert_eq!(read_all(&mut matching).unwrap(), [1, 0x3f05]);
}

#[test]
fn xtypes_7_4_1_2_a_padded_declared_length_rounds_a_member_to_four() {
    // Cyclone and RTI declare `short c` of a @mutable type as 4.
    let padded = PlCdr1Framing {
        pad_declared_length: true,
        ..PlCdr1Framing::DEFAULT
    };
    let mut w = Writer::with_max_align(Endianness::Little, 8).with_pl_cdr1_framing(padded);
    let h = w.write_pid(3, ParameterRole::Member, false).unwrap();
    w.write_i16(-1);
    w.finish_pid(h);
    assert_eq!(w.finish(), [0x03, 0x00, 0x04, 0x00, 0xff, 0xff, 0x00, 0x00]);
    // The specification's framing declares the value's own two.
    let mut w = Writer::with_max_align(Endianness::Little, 8);
    let h = w.write_pid(3, ParameterRole::Member, false).unwrap();
    w.write_i16(-1);
    w.finish_pid(h);
    assert_eq!(w.finish(), [0x03, 0x00, 0x02, 0x00, 0xff, 0xff, 0x00, 0x00]);
}

#[test]
fn the_padding_record_names_every_alignment_span() {
    let mut w = Writer::with_max_align(Endianness::Little, 8).recording_padding();
    assert!(w.is_recording_padding());
    w.write_u8(1);
    w.write_u32(2);
    w.write_u8(3);
    w.write_u64(4);
    assert_eq!(w.padding_spans(), [(1, 3), (9, 7)]);
    assert_eq!(w.endianness(), Endianness::Little);
    assert_eq!(w.max_align(), 8);
    // Without the request: no record.
    let mut w = Writer::with_max_align(Endianness::Little, 8);
    w.write_u8(1);
    w.write_u32(2);
    assert!(w.padding_spans().is_empty());
}

#[test]
fn a_refused_write_is_out_of_range_with_the_expected_text() {
    let mut w = Writer::little_endian();
    match w.write_char('\u{100}') {
        Err(WireError::OutOfRange { at, expected }) => {
            assert_eq!(at, 0);
            assert!(!expected.is_empty());
        }
        other => panic!("expected OutOfRange, got {other:?}"),
    }
}

#[test]
fn skip_to_moves_forward_and_refuses_backward_or_past_the_end() {
    let bytes = [0u8; 8];
    let mut r = Reader::new(&bytes, Endianness::Little);
    r.skip_to(4).unwrap();
    assert_eq!(r.position(), 4);
    assert!(r.skip_to(2).is_err());
    assert!(r.skip_to(9).is_err());
    assert_eq!(r.position(), 4);
}
