// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! An enumeration as a member and as a sequence element, through the
//! generated codec.
//!
//! The member is written as the enumeration's holder, a 32-bit integer
//! without `@bit_bound` (XTypes 1.3 §7.4.3.5.3, rule (5)); the field the
//! data column declares for it is the generated enum, so the codec goes
//! through the enum's own `encode`/`decode` and not through a bare
//! `write_u32` on the field.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_wire_conformance::generated::{Painted, Shade};
use zerodds_wire_conformance::{Endianness, Reader, Writer};

#[test]
fn an_enumeration_member_is_its_holder_and_round_trips() {
    let sample = Painted {
        s: Shade::LIGHT,
        trail: vec![Shade::BRIGHT, Shade::DARK],
        tail: 7,
    };
    let mut w = Writer::little_endian();
    sample.encode(&mut w).expect("encode");
    let bytes = w.finish();
    assert_eq!(&bytes[0..4], &[1, 0, 0, 0], "LIGHT is not its holder value");

    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = Painted::decode(&mut r).expect("decode");
    assert_eq!(read, sample);
    assert_eq!(r.remaining(), 0, "the decoder left bytes unread");
}
