// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-corba-interop` with `zerodds-lang-rules`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-lang-rules` (`build.rs`, a build
//! dependency): `apply` with `rule::rust::catalogue()`,
//! `adapter::cst::{rust_items, nest_by_module}` and `encode::rust_codec`
//! under `LayoutKey::in_dialect(.., Encoding::PlainCdr1, ..)` for the data
//! types of the interop IDL, which the GIOP stubs and skeletons of
//! `zerodds-corba-rust` marshal through; the binaries and tests include the
//! result as `corba_gen`. omniORB, TAO and JacORB read what these codecs
//! write, so the contract is their CDR octets.
//!
//! Oracle: CORBA 3.3 part 2 §9.3 (CDR: the members of a struct in order,
//! each aligned to its size; an enum is an `unsigned long` holding the
//! ordinal of its enumerator, §9.3.2.6) with the IDL (`struct Point { long
//! x; long y; }`, `enum Color { RED, GREEN, BLUE }`).
#![allow(clippy::unwrap_used, clippy::panic, dead_code, unused_imports)]

use std::sync::Arc;

include!(concat!(env!("OUT_DIR"), "/corba_gen.rs"));

use corba_gen::{Color, Point};
use zerodds_wire::{Endianness, Reader, Writer};

#[test]
fn corba_3_3_part2_9_3_a_struct_is_its_members_in_order() {
    let mut w = Writer::new(Endianness::Little);
    Point { x: 1, y: -2 }.encode(&mut w).unwrap();
    let bytes = w.finish();
    assert_eq!(bytes, [1, 0, 0, 0, 0xfe, 0xff, 0xff, 0xff]);
    let mut r = Reader::new(&bytes, Endianness::Little);
    let back = Point::decode(&mut r).unwrap();
    assert_eq!((back.x, back.y), (1, -2));
}

#[test]
fn corba_3_3_part2_9_3_2_6_an_enumerator_is_the_unsigned_long_of_its_ordinal() {
    let mut w = Writer::new(Endianness::Big);
    Color::BLUE.encode(&mut w).unwrap();
    assert_eq!(w.finish(), [0, 0, 0, 2]);
}
