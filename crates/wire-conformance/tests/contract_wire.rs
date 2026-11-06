// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-wire-conformance` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire` (`src/lib.rs`): the module
//! `generated`, which `build.rs` writes from `conformance.idl` with the
//! generator and `include!`s, is compiled against `Writer`, `Reader`,
//! `WireError`, `ParameterRole`, `TryConstruct` and `Narrowed`; the crate
//! re-exports `Endianness`, `ParameterRole`, `Reader`, `TryConstruct`,
//! `WireError` and `Writer`. The generated `encode` / `decode` /
//! `key_hash` call the runtime's primitives, framing and bound checks;
//! the alignment cap is the one of the `Writer` or `Reader` the caller
//! hands in, except in `key_hash`, which opens its own big-endian writer
//! with the cap four.
//!
//! Oracles: DDS-XTypes 1.3 §7.4.2 (MAXALIGN 4 under version 2) with the
//! octets Cyclone DDS and RTI Connext 7.7 write for `@final struct Point {
//! long x; double y; }`, `x = 1`, `y = 2.0`
//! (`010000000000000000000040`, `crates/ast-layout/src/key.rs`);
//! §7.4.3.5.3 (DHEADER of an appendable type), §7.4.3.5.2 rule (20) with
//! the `opt_final_present` / `opt_final_absent` octets of Cyclone, RTI and
//! Fast DDS (`crates/ast-layout/tests/fixtures/vendor_bytes.md`); §7.4.1.2
//! rule (23) (PL_CDR1 parameters and `PID_LIST_END`); §7.2.4.4.8 Table 19
//! (a missing member of an appendable reader takes its default);
//! §7.2.2.7 and §7.3.1.2.1.12 (`try_construct`) against the payload
//! Cyclone DDS wrote for `@mutable struct S { long a; string<32> m; long
//! b; }` (recorded in `tests/bytes.rs`); §7.2.2.2.1.2.2 (`char` is eight
//! bits); DDSI-RTPS 2.5 §9.6.3.8 and DDS-XTypes 1.3 §7.6.8 (the KeyHash: the
//! key holder as PLAIN_CDR2 big-endian, zero-padded to 16 octets).
#![allow(clippy::unwrap_used, clippy::panic)]

use core::any::TypeId;

use zerodds_wire_conformance::generated::{
    Ext, Header, Label, Newer, Older, OptV2, Point, Sensor, TcDiscard, TcTrim, TcUseDefault,
};
use zerodds_wire_conformance::{Endianness, Reader, WireError, Writer};

fn xcdr2() -> Writer {
    Writer::with_max_align(Endianness::Little, 4)
}

fn read2(bytes: &[u8]) -> Reader<'_> {
    Reader::with_max_align(bytes, Endianness::Little, 4)
}

#[test]
fn the_reexports_are_the_wire_runtime() {
    assert_eq!(
        TypeId::of::<zerodds_wire_conformance::Writer>(),
        TypeId::of::<zerodds_wire::Writer>()
    );
    assert_eq!(
        TypeId::of::<zerodds_wire_conformance::Reader<'static>>(),
        TypeId::of::<zerodds_wire::Reader<'static>>()
    );
    assert_eq!(
        TypeId::of::<zerodds_wire_conformance::WireError>(),
        TypeId::of::<zerodds_wire::WireError>()
    );
    assert_eq!(
        TypeId::of::<zerodds_wire_conformance::Endianness>(),
        TypeId::of::<zerodds_wire::Endianness>()
    );
    assert_eq!(
        TypeId::of::<zerodds_wire_conformance::ParameterRole>(),
        TypeId::of::<zerodds_wire::ParameterRole>()
    );
    assert_eq!(
        TypeId::of::<zerodds_wire_conformance::TryConstruct>(),
        TypeId::of::<zerodds_wire::TryConstruct>()
    );
}

#[test]
fn xtypes_7_4_2_the_generated_point_under_maxalign_4_is_the_vendors_twelve_octets() {
    let mut w = xcdr2();
    Point { x: 1, y: 2.0 }.encode(&mut w).unwrap();
    let bytes = w.finish();
    assert_eq!(
        bytes,
        [
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40
        ]
    );
    let mut r = read2(&bytes);
    let read = Point::decode(&mut r).unwrap();
    assert_eq!(read.x, 1);
    assert_eq!(read.y.to_bits(), 2.0f64.to_bits());
    assert_eq!(r.remaining(), 0);
}

#[test]
fn xtypes_7_4_3_5_3_an_appendable_type_opens_with_its_dheader() {
    let mut w = xcdr2();
    Header { seq: 9 }.encode(&mut w).unwrap();
    assert_eq!(w.finish(), [0x04, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00]);
}

#[test]
fn xtypes_7_4_3_5_2_rule_20_the_optional_marker_the_three_vendors_write() {
    let mut w = xcdr2();
    OptV2 {
        a: -1_234_567,
        b: None,
        c: -2,
    }
    .encode(&mut w)
    .unwrap();
    assert_eq!(
        w.finish(),
        [
            0x79, 0x29, 0xed, 0xff, 0x00, 0x00, 0x00, 0x00, 0xfe, 0xff, 0xff, 0xff
        ]
    );
    // Present, as Cyclone, RTI and Fast DDS wrote it; the two octets
    // before `c` are padding of no fixed content.
    for padding in [[0x00, 0x00], [0x04, 0x00], [0x01, 0x00]] {
        let mut bytes = vec![0x79, 0x29, 0xed, 0xff, 0x01, 0xab];
        bytes.extend_from_slice(&padding);
        bytes.extend_from_slice(&[0xfe, 0xff, 0xff, 0xff]);
        let read = OptV2::decode(&mut read2(&bytes)).unwrap();
        assert_eq!((read.a, read.b, read.c), (-1_234_567, Some(0xab), -2));
    }
}

#[test]
fn xtypes_7_4_1_2_rule_23_a_mutable_xcdr1_type_is_a_parameter_list_and_the_sentinel() {
    let mut w = Writer::with_max_align(Endianness::Little, 8);
    Ext { d: 2.5, c: -3 }.encode(&mut w).unwrap();
    let bytes = w.finish();
    #[rustfmt::skip]
    let expected = [
        0x01, 0x00, 0x08, 0x00,                         // @id(1), length 8
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x40, // 2.5, its own alignment origin
        0x02, 0x00, 0x02, 0x00,                         // @id(2), length 2
        0xfd, 0xff, 0x00, 0x00,                         // -3, padding to four
        0x02, 0x7f, 0x00, 0x00,                         // PID_LIST_END
    ];
    assert_eq!(bytes, expected);
    let mut r = Reader::with_max_align(&bytes, Endianness::Little, 8);
    let read = Ext::decode(&mut r).unwrap();
    assert_eq!((read.d.to_bits(), read.c), (2.5f64.to_bits(), -3));
    assert_eq!(r.remaining(), 0);
}

#[test]
fn xtypes_7_2_4_4_8_an_appendable_reader_defaults_the_tail_its_writer_did_not_have() {
    let mut w = xcdr2();
    Older { a: 7, b: 1.5 }.encode(&mut w).unwrap();
    let bytes = w.finish();
    // DHEADER 12, the long, the double at four.
    assert_eq!(bytes.len(), 16);
    assert_eq!(bytes[..4], [0x0c, 0x00, 0x00, 0x00]);
    let read = Newer::decode(&mut read2(&bytes)).unwrap();
    assert_eq!(read.a, 7);
    assert_eq!(read.b.to_bits(), 1.5f64.to_bits());
    assert_eq!(read.c, 0);
    assert_eq!(read.d, "");
}

/// Cyclone DDS, XCDR2: `@mutable struct S { long a; string<32> m; long b;
/// }`, `a = 7`, `m = "Hello World!"`, `b = 9`.
#[rustfmt::skip]
const WIDE_STRING: [u8; 44] = [
    0x28, 0x00, 0x00, 0x00,                         // DHEADER 40
    0x00, 0x00, 0x00, 0x20, 0x07, 0x00, 0x00, 0x00, // LC 2, id 0: a = 7
    0x01, 0x00, 0x00, 0x50, 0x0d, 0x00, 0x00, 0x00, // LC 5, id 1: length 13
    b'H', b'e', b'l', b'l', b'o', b' ', b'W', b'o', b'r', b'l', b'd', b'!', 0x00,
    0x00, 0x00, 0x00,                               // padding
    0x02, 0x00, 0x00, 0x20, 0x09, 0x00, 0x00, 0x00, // LC 2, id 2: b = 9
];

#[test]
fn xtypes_7_2_2_7_try_construct_on_a_string_past_the_receivers_bound() {
    // No annotation: DISCARD, the sample is dropped as an absence.
    match TcDiscard::decode(&mut read2(&WIDE_STRING)) {
        Err(error) => assert!(error.is_discarded(), "{error:?}"),
        Ok(_) => panic!("a string<5> took twelve characters"),
    }
    // USE_DEFAULT: the member's default, the others as sent.
    let read = TcUseDefault::decode(&mut read2(&WIDE_STRING)).unwrap();
    assert_eq!((read.a, read.m.as_str(), read.b), (7, "", 9));
    // TRIM: the first five characters.
    let read = TcTrim::decode(&mut read2(&WIDE_STRING)).unwrap();
    assert_eq!((read.a, read.m.as_str(), read.b), (7, "Hello", 9));
}

#[test]
fn xtypes_7_2_2_2_1_2_2_a_char_above_u00ff_is_refused_not_narrowed() {
    let mut w = xcdr2();
    match (Label {
        tag: '\u{161}',
        wide: 'x',
    })
    .encode(&mut w)
    {
        Err(WireError::OutOfRange { .. }) => {}
        other => panic!("U+0161 went into one octet: {other:?}"),
    }
    assert!(w.finish().is_empty());
}

#[test]
fn rtps_9_6_3_8_the_key_hash_is_the_big_endian_holder_zero_padded() {
    let hash = Sensor { id: 1, reading: 5 }.key_hash().unwrap();
    assert_eq!(hash[..4], [0x00, 0x00, 0x00, 0x01]);
    assert_eq!(hash[4..], [0x00; 12]);
    // The member outside the key does not reach the hash.
    assert_eq!(
        Sensor { id: 1, reading: 6 }.key_hash().unwrap(),
        Sensor { id: 1, reading: 5 }.key_hash().unwrap()
    );
}
