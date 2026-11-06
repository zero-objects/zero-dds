// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A Rust type standing for an IDL primitive or `string` (GitHub #33):
//! the octets it writes, against XTypes 1.3 §7.4.1, and what it refuses.
//!
//! The oracle is the clause's layout of a `string` — a four-octet length
//! that counts the terminator, the text, a NUL, the length aligned to
//! four — and the invariants of the Rust types themselves (`NonZero` holds
//! no zero). A round trip through this crate alone is not asserted: the
//! writer and the reader would agree about a mistake.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use core::fmt;
use core::num::NonZeroU32;
use zerodds_wire::repr::{from_default, from_wire, from_wire_with, idl_primitive, idl_string};
use zerodds_wire::{
    DisplayFromStr, Endianness, IdlOut, Reader, WireError, WireRepr, WireReprWith, Writer,
};

/// A value whose text is produced by several `write_str` calls.
struct Pieces(&'static [&'static str]);

impl fmt::Display for Pieces {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for piece in self.0 {
            f.write_str(piece)?;
        }
        Ok(())
    }
}

/// XTypes 1.3 §7.4.1.1.2: the length counts the terminator and is aligned
/// to four from the encapsulation. A text a formatter wrote in three
/// pieces straight into the buffer is the same octets: padding behind the
/// octet already there, a length of six, five octets and a NUL.
#[test]
fn xtypes_7_4_1_1_2_a_displayed_string_is_the_octets_of_its_text() {
    let mut w = Writer::little_endian();
    w.write_u8(0xAA);
    IdlOut::string(&mut w)
        .display(&Pieces(&["he", "l", "lo"]))
        .expect("an unbounded string");
    assert_eq!(
        w.as_bytes(),
        &[0xAA, 0, 0, 0, 6, 0, 0, 0, b'h', b'e', b'l', b'l', b'o', 0]
    );

    let mut big = Writer::big_endian();
    IdlOut::bounded_string(&mut big, 5)
        .display(&"hello")
        .expect("five octets fit string<5>");
    assert_eq!(
        big.as_bytes(),
        &[0, 0, 0, 6, b'h', b'e', b'l', b'l', b'o', 0]
    );
}

/// `string<N>` bounds the text in octets (XTypes 1.3 Table 17,
/// `STRING_TYPE`: "O2.length <= T1.length"), and a NUL inside the text is
/// where every C peer's `char *` ends. A displayed text that breaks
/// either is refused after it was formatted, and the buffer is what it
/// was before the call: the alignment padding goes with it.
#[test]
fn xtypes_7_4_1_1_2_a_displayed_string_over_its_bound_or_with_a_nul_is_refused_whole() {
    let mut w = Writer::little_endian();
    w.write_u8(0x11);
    let refused = IdlOut::bounded_string(&mut w, 4).display(&Pieces(&["hel", "lo"]));
    assert!(
        matches!(refused, Err(WireError::OutOfRange { .. })),
        "{refused:?}"
    );
    assert_eq!(
        w.as_bytes(),
        &[0x11],
        "nothing of the refused value remains"
    );

    let refused = IdlOut::string(&mut w).display(&"a\0b");
    assert!(
        matches!(&refused, Err(WireError::OutOfRange { at: 9, .. })),
        "the NUL is the ninth octet: {refused:?}"
    );
    assert_eq!(w.as_bytes(), &[0x11]);

    let refused = IdlOut::string(&mut w).str("a\0b");
    assert!(
        matches!(refused, Err(WireError::OutOfRange { .. })),
        "{refused:?}"
    );
    assert_eq!(w.as_bytes(), &[0x11]);

    // The value itself fits once the bound does.
    IdlOut::bounded_string(&mut w, 5)
        .display(&Pieces(&["hel", "lo"]))
        .expect("five octets");
    assert_eq!(&w.as_bytes()[4..8], &[6, 0, 0, 0]);
}

/// XTypes 1.3 §7.4.1.1.2: the text is the run of octets between the
/// length and the terminator. `read_str` hands out exactly that run of the
/// sample's own bytes, and the reader stands behind the NUL.
#[test]
fn xtypes_7_4_1_1_2_read_str_lends_the_text_out_of_the_sample() {
    let bytes = [0xAA, 0, 0, 0, 3, 0, 0, 0, b'h', b'i', 0, 0x07];
    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(r.read_u8().expect("octet"), 0xAA);
    let text = r.read_str().expect("a string");
    assert_eq!(text, "hi");
    assert!(
        core::ptr::eq(text.as_ptr(), bytes[8..].as_ptr()),
        "the text is the sample's octets, not a copy"
    );
    assert_eq!(r.position(), 11);
    assert_eq!(r.read_u8().expect("the octet after the NUL"), 0x07);
}

/// `NonZeroU32` holds no zero. A zero on the wire is a well-formed IDL
/// `unsigned long` that the Rust type does not accept: refused with a
/// reason, at the offset the generated decoder hands over; and as the
/// default of a member the sample left out, refused saying so.
#[test]
fn issue_33_a_zero_for_a_non_zero_type_is_refused_with_its_reason() {
    let refused = from_wire::<NonZeroU32>(&0, 24).expect_err("zero is no NonZeroU32");
    match &refused {
        WireError::OutOfRange { at, expected } => {
            assert_eq!(*at, 24);
            assert!(expected.contains("other than zero"), "{expected}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        from_wire::<NonZeroU32>(&7, 24).expect("seven"),
        NonZeroU32::new(7).expect("seven")
    );
    let defaulted = from_default::<NonZeroU32>(&0).expect_err("Table 9's zero");
    assert!(
        defaulted.to_string().contains("did not carry"),
        "{defaulted}"
    );

    // Written as the IDL type: the four octets of the integer (§7.4.1.1.1).
    let mut w = Writer::big_endian();
    NonZeroU32::new(0x0102_0304)
        .expect("non-zero")
        .to_wire(IdlOut::uint32(&mut w))
        .expect("written");
    assert_eq!(w.as_bytes(), &[1, 2, 3, 4]);
    assert_eq!(
        idl_primitive::<NonZeroU32, u32>(&NonZeroU32::new(9).expect("nine")).expect("nine"),
        9
    );
}

/// A `Display + FromStr` type through [`DisplayFromStr`]: written as the
/// text `Display` gives, read with `FromStr`, and a text the parser
/// refuses is refused with the parser's message.
#[test]
fn issue_33_display_from_str_writes_the_text_and_refuses_what_from_str_refuses() {
    let mut w = Writer::little_endian();
    WireReprWith::<core::net::Ipv4Addr>::to_wire(
        &DisplayFromStr,
        &core::net::Ipv4Addr::new(10, 0, 0, 1),
        IdlOut::string(&mut w),
    )
    .expect("written");
    assert_eq!(
        w.as_bytes(),
        &[
            9, 0, 0, 0, b'1', b'0', b'.', b'0', b'.', b'0', b'.', b'1', 0
        ]
    );
    let back: core::net::Ipv4Addr =
        from_wire_with(&DisplayFromStr, "10.0.0.1", 0).expect("an address");
    assert_eq!(back, core::net::Ipv4Addr::new(10, 0, 0, 1));
    let refused: Result<core::net::Ipv4Addr, _> = from_wire_with(&DisplayFromStr, "10.0.0", 40);
    assert!(
        matches!(&refused, Err(WireError::OutOfRange { at: 40, expected }) if expected.contains("FromStr")),
        "{refused:?}"
    );
    assert_eq!(idl_string(&String::from("ab")).expect("text"), "ab");
}
