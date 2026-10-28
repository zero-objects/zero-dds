// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `char` is eight bits and `wchar` is sixteen, and a code point that
//! does not fit either is refused.
//!
//! `Writer::write_char` used to be `value as u32 as u8`. Nothing in the
//! corpus reached it — every `char` sample is ASCII — so the sweep, which
//! read code, saw a cast with a comment saying the caller must not do
//! this, and the sweep's own definition needs a value to see the rest:
//! what came out was a different character and nothing said so.
//!
//! Both directions are here. The reader was checked for the same shape
//! and has it in neither: `read_char` maps a byte onto U+0000..U+00FF,
//! which is total, and `read_wchar_member` refuses a surrogate by name.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_wire::{
    Endianness, Reader, WcharFraming, WcharPosition, WcharWidth, WireError, Writer,
};

/// DDS-XTypes 1.3 §7.2.2.2.1.2.2: "This specification does not define an
/// encoding for the CHAR_8_TYPE. The only constraint is that it shall be
/// representable using 8 bits."
#[test]
fn a_char_above_u00ff_is_refused_and_not_narrowed() {
    let mut w = Writer::little_endian();
    // U+0100 narrows to 0x00 and U+0161 to 0x61, which is `a`. Both are
    // bytes a reader accepts, so the loss has no shape a length check or
    // a round trip through this crate can find.
    match w.write_char('\u{100}') {
        Err(WireError::OutOfRange { at, expected }) => {
            assert_eq!(at, 0);
            assert!(expected.contains("U+0100"), "{expected}");
        }
        other => panic!("U+0100 was accepted into one byte: {other:?}"),
    }
    assert!(
        w.as_bytes().is_empty(),
        "a byte went out for a refused value: {:?}",
        w.as_bytes()
    );
}

/// The one that would have been read back as a different character.
#[test]
fn the_narrowed_byte_is_not_written_where_it_would_read_back_as_another_char() {
    let mut w = Writer::little_endian();
    // U+0161 is `š`; its low byte is 0x61, which is `a`.
    assert!(w.write_char('\u{161}').is_err());
    w.write_char('z').expect("`z` is one byte");
    let bytes = w.finish();
    assert_eq!(bytes, [b'z'], "the refused character left a byte behind");

    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(r.read_char().expect("char"), 'z');
}

/// The whole of U+0000..U+00FF is one byte and goes out unchanged.
#[test]
fn every_code_point_a_char_can_hold_round_trips() {
    for point in 0u32..=0xFF {
        let value = char::from_u32(point).expect("Latin-1 is a subset of Unicode");
        let mut w = Writer::little_endian();
        w.write_char(value).expect("one byte");
        let bytes = w.finish();
        assert_eq!(bytes.len(), 1, "U+{point:04X} took more than a byte");
        let mut r = Reader::new(&bytes, Endianness::Little);
        assert_eq!(r.read_char().expect("char"), value, "U+{point:04X}");
    }
}

/// DDS-XTypes 1.3 §7.2.2.2.1.2.5: "The CHAR_16_TYPE shall be restricted
/// to representing Unicode codepoints in the Basic Multilingual Plane.
/// That is Unicode codepoints from 0x0000 to U+FFFF."
#[test]
fn a_wchar_above_the_bmp_is_refused_and_not_narrowed() {
    let mut w = Writer::little_endian();
    // U+1F600 narrows to 0xF600, which is a private-use character a
    // reader accepts without complaint.
    match w.write_wchar_member('\u{1F600}') {
        Err(WireError::OutOfRange { at, expected }) => {
            assert_eq!(at, 0);
            assert!(expected.contains("U+1F600"), "{expected}");
        }
        other => panic!("U+1F600 was accepted into two bytes: {other:?}"),
    }
    assert!(
        w.as_bytes().is_empty(),
        "bytes went out for a refused value: {:?}",
        w.as_bytes()
    );
}

/// The top of the BMP is not above it.
#[test]
fn the_last_bmp_code_point_is_written() {
    let mut w = Writer::big_endian();
    w.write_wchar_member('\u{FFFD}').expect("the BMP");
    assert_eq!(w.finish(), [0xFF, 0xFD]);
}

/// The refusal names the offset the value would have gone to, not the
/// offset after the padding it never asked for.
#[test]
fn the_refusal_names_where_the_value_would_have_gone() {
    let mut w = Writer::little_endian();
    w.write_u8(1);
    match w.write_wchar_member('\u{10000}') {
        Err(WireError::OutOfRange { at, .. }) => assert_eq!(at, 1),
        other => panic!("{other:?}"),
    }
    assert_eq!(w.finish(), [1], "the alignment padding was written anyway");
}

/// The reader has neither shape.
///
/// `read_char` is total over its byte, and `read_wchar_member` refuses the one
/// value in its range that is not a Unicode scalar. Neither returns a
/// value that disagrees with the bytes it read.
#[test]
fn the_reader_refuses_a_surrogate_rather_than_returning_something_else() {
    let bytes = [0x00, 0xD8];
    let mut r = Reader::new(&bytes, Endianness::Little);
    match r.read_wchar_member() {
        Err(WireError::OutOfRange { at, .. }) => assert_eq!(at, 0),
        other => panic!("a lone surrogate came back as a character: {other:?}"),
    }
}

// -- the four-byte `wchar` ------------------------------------------------

/// RTI Connext writes a four-byte `wchar` under XCDR1.
///
/// Measured 2026-08-09 through `byte-gate/gate`, case
/// `wchar_member`, `@final struct WCh { wchar c; octet tail; }` with
/// `c = U+0061` and `tail = 0xAB`:
///
/// | | XCDR1 LE | XCDR1 BE | XCDR2 LE | XCDR2 BE |
/// |---|---|---|---|---|
/// | Cyclone | `6100 ab` | `0061 ab` | `6100 ab` | `0061 ab` |
/// | RTI 7.7 | `61000000 ab` | `00000061 ab` | `6100 ab` | `0061 ab` |
/// | Fast DDS | `6100 ab` | `0061 ab` | `6100 ab` | `0061 ab` |
///
/// A member read at the wrong width does not only return the wrong
/// character: every member after it lands two bytes out of place.
#[test]
fn the_four_byte_wchar_is_the_one_rti_writes_under_xcdr1() {
    for (endianness, expected) in [
        (Endianness::Little, [0x61, 0x00, 0x00, 0x00, 0xAB]),
        (Endianness::Big, [0x00, 0x00, 0x00, 0x61, 0xAB]),
    ] {
        let mut w = Writer::with_max_align(endianness, 8).with_wchar_width(WcharWidth::Four);
        w.write_wchar_member('a').expect("the BMP");
        w.write_u8(0xAB);
        assert_eq!(w.finish(), expected, "{endianness:?}");
    }
    for (endianness, expected) in [
        (Endianness::Little, [0x61, 0x00, 0xAB]),
        (Endianness::Big, [0x00, 0x61, 0xAB]),
    ] {
        let mut w = Writer::with_max_align(endianness, 8);
        w.write_wchar_member('a').expect("the BMP");
        w.write_u8(0xAB);
        assert_eq!(w.finish(), expected, "{endianness:?}");
    }
}

/// The four-byte form aligns to four, which moves what follows.
#[test]
fn the_four_byte_wchar_aligns_to_four() {
    let mut w = Writer::with_max_align(Endianness::Little, 8).with_wchar_width(WcharWidth::Four);
    w.write_u8(1);
    w.write_wchar_member('a').expect("the BMP");
    // One byte, three of padding to the four-byte boundary, then the
    // character. The two-byte form pads one and starts at two.
    assert_eq!(w.finish(), [1, 0, 0, 0, 0x61, 0x00, 0x00, 0x00]);
}

/// Reading it back is the same setting, and the wrong one is an error.
#[test]
fn a_four_byte_wchar_read_two_bytes_wide_does_not_return_a_wrong_sample() {
    let bytes = [0x61, 0x00, 0x00, 0x00, 0xAB];
    let mut wide =
        Reader::with_max_align(&bytes, Endianness::Little, 8).with_wchar_width(WcharWidth::Four);
    assert_eq!(wide.read_wchar_member().expect("a wchar"), 'a');
    assert_eq!(wide.read_u8().expect("the tail"), 0xAB);

    // Two bytes wide the character still reads as `a` — the low half is
    // the same — and then the tail is the padding rather than 0xAB. The
    // width is not recoverable from the value, only from what follows.
    let mut narrow = Reader::with_max_align(&bytes, Endianness::Little, 8);
    assert_eq!(narrow.read_wchar_member().expect("a wchar"), 'a');
    assert_eq!(narrow.read_u8().expect("a byte"), 0x00);
}

/// A four-byte character that uses its high half is refused.
///
/// RTI zero-extends one UTF-16 code unit. Narrowing anything else would
/// hand the caller a character the peer did not send.
#[test]
fn a_four_byte_wchar_holding_more_than_a_code_unit_is_refused() {
    let bytes = [0x1E, 0xD1, 0x01, 0x00];
    let mut r =
        Reader::with_max_align(&bytes, Endianness::Little, 8).with_wchar_width(WcharWidth::Four);
    match r.read_wchar_member() {
        Err(WireError::OutOfRange { at, expected }) => {
            assert_eq!(at, 0);
            assert!(expected.contains("one UTF-16 code unit"), "{expected}");
        }
        other => panic!("a two-unit value came back as a character: {other:?}"),
    }
}

/// The BMP restriction is a property of the type, not of the width.
#[test]
fn the_four_byte_form_still_refuses_a_code_point_above_the_bmp() {
    let mut w = Writer::little_endian().with_wchar_width(WcharWidth::Four);
    assert!(w.write_wchar_member('\u{1F600}').is_err());
    assert!(w.as_bytes().is_empty());
}

/// TAO frames a `wchar` member and a `wchar` collection element
/// differently, and the framing is per position rather than per type.
///
/// Measured from `tests/interop-results/`, `plain-cdr1` little endian.
/// `primitive-wchar` is `04 e4000000  04 34120000` from `tao_idl`'s
/// output: the GIOP 1.2 octet count before each code unit. The three
/// `wchar` elements of `array-1d-primitives` are
/// `00 e4000000 34120000 5a000000` — one pad octet to the four-octet
/// alignment and then three bare units, with no count anywhere.
///
/// omniORB and JacORB keep the count in both positions —
/// `02 00e4 02 1234 02 005a` for the same array — which is why the two
/// forms are separate variants and not one policy with a tolerance.
///
/// This is the test that goes red if `WcharPosition` is taken back out:
/// without it a preset carrying TAO's name reproduces TAO's members and
/// not TAO's arrays, which is the whole of what `--dialect tao` used to
/// leave over.
#[test]
fn taos_wchar_carries_its_count_as_a_member_and_not_as_an_element() {
    let member = |framing| {
        let mut w = Writer::little_endian()
            .with_wchar_width(WcharWidth::Four)
            .with_wchar_framing(framing);
        w.write_wchar('\u{e4}', WcharPosition::Member)
            .expect("the BMP");
        w.finish()
    };
    let element = |framing| {
        let mut w = Writer::little_endian()
            .with_wchar_width(WcharWidth::Four)
            .with_wchar_framing(framing);
        w.write_wchar('\u{e4}', WcharPosition::Element)
            .expect("the BMP");
        w.finish()
    };

    // TAO: the count as a member, a bare unit as an element.
    assert_eq!(
        member(WcharFraming::GiopLengthPrefixedMemberOnly),
        vec![0x04, 0xE4, 0x00, 0x00, 0x00]
    );
    assert_eq!(
        element(WcharFraming::GiopLengthPrefixedMemberOnly),
        vec![0xE4, 0x00, 0x00, 0x00]
    );

    // omniORB and JacORB: the count in both positions.
    assert_eq!(
        member(WcharFraming::GiopLengthPrefixed),
        element(WcharFraming::GiopLengthPrefixed)
    );
    // Every DDS implementation: the count in neither.
    assert_eq!(member(WcharFraming::Fixed), element(WcharFraming::Fixed));
}

/// The reading side takes the same position, and a reader given the
/// wrong one refuses rather than returning a character nobody sent.
#[test]
fn an_element_read_as_a_member_is_refused_rather_than_returned() {
    let bytes = [0xE4, 0x00, 0x00, 0x00];
    let reader = || {
        Reader::with_max_align(&bytes, Endianness::Little, 8)
            .with_wchar_width(WcharWidth::Four)
            .with_wchar_framing(WcharFraming::GiopLengthPrefixedMemberOnly)
    };
    assert_eq!(
        reader()
            .read_wchar(WcharPosition::Element)
            .expect("a wchar"),
        '\u{e4}'
    );
    // As a member the leading 0xE4 reads as a count of 228 octets,
    // which is not the configured width. The count is checked and not
    // believed, so this is a refusal and not 228 octets of whatever
    // followed.
    assert!(reader().read_wchar(WcharPosition::Member).is_err());
}
