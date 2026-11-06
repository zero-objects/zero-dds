// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-corba-csiv2` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire`: `src/sas.rs` encodes a
//! `SASContextBody` as an encapsulation with `Writer::new(endianness)`:
//! the byte-order octet, `write_i16` (the message type), then `write_u64`
//! (`client_context_id`), `write_u32`, `write_u8` (booleans) and
//! `sequence<octet>` as `write_u32` + `write_bytes`; it decodes with
//! `Reader::new`, `read_u8`, `read_i16`, `read_u64`, `read_u32`,
//! `read_bytes` and checks counts against `remaining`. `src/gssup.rs`
//! does the same for the GSSUP `InitialContextToken` (three
//! `sequence<octet>`). `src/mech_list.rs` puts `Writer`, `Reader` and
//! `WireError` into the public `encode`/`decode` of the
//! `CompoundSecMechList` structures (`write_u16`/`read_u16` for the
//! association options, `write_u32`/`read_u32`, `write_u8`/`read_u8`),
//! and builds `WireError::OutOfRange` and `WireError::ImpossibleLength`
//! itself with `Writer::position`/`Reader::position`.
//!
//! Oracle: CORBA 3.3 part 2 §9.3.1.1 (each primitive aligned to its size,
//! counted from the start of the encapsulation, eight for `long long`),
//! §9.3.1.5 (boolean), §9.3.2.5 (sequence: unsigned long count, then the
//! elements), §9.3.3 (encapsulation: byte-order octet first), with the
//! CSIv2 types of CORBA 3.3 part 2 chapter 10 (`SASContextBody` with
//! `MsgType` `short` discriminator, `MTMessageInContext` = 5;
//! `GSSUP::InitialContextToken` of three octet sequences).
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, WireError, Writer};

/// `MessageInContext { client_context_id: 0x0102030405060708,
/// discard_context: TRUE }`, big-endian encapsulation.
const MESSAGE_IN_CONTEXT_BE: [u8; 17] = [
    0x00, // byte order
    0x00, // padding
    0x00, 0x05, // MTMessageInContext at 2
    0x00, 0x00, 0x00, 0x00, // padding to 8
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, // client_context_id at 8
    0x01, // discard_context
];

#[test]
fn corba_9_3_1_1_sas_body_aligns_the_short_to_two_and_the_long_long_to_eight() {
    let mut w = Writer::new(Endianness::Big);
    w.write_u8(0);
    w.write_i16(5);
    w.write_u64(0x0102_0304_0506_0708);
    w.write_u8(1);
    assert_eq!(w.finish(), MESSAGE_IN_CONTEXT_BE);

    let mut r = Reader::new(&MESSAGE_IN_CONTEXT_BE, Endianness::Big);
    assert_eq!(r.read_u8().unwrap(), 0);
    assert_eq!(r.read_i16().unwrap(), 5);
    assert_eq!(r.read_u64().unwrap(), 0x0102_0304_0506_0708);
    assert_eq!(r.read_u8().unwrap(), 1);
    assert_eq!(r.remaining(), 0);
}

#[test]
fn corba_9_3_2_5_gssup_token_is_three_octet_sequences_without_terminators() {
    let expected = [
        0x01, 0x00, 0x00, 0x00, // byte order, padding
        0x01, 0x00, 0x00, 0x00, b'u', // username
        0x00, 0x00, 0x00, // padding
        0x01, 0x00, 0x00, 0x00, b'p', // password
        0x00, 0x00, 0x00, // padding
        0x00, 0x00, 0x00, 0x00, // empty target_name
    ];
    let mut w = Writer::new(Endianness::Little);
    w.write_u8(1);
    for field in [&b"u"[..], &b"p"[..], &[][..]] {
        w.write_u32(u32::try_from(field.len()).unwrap());
        w.write_bytes(field);
    }
    assert_eq!(w.finish(), expected);

    let mut r = Reader::new(&expected, Endianness::Little);
    r.read_u8().unwrap();
    let mut fields = Vec::new();
    for _ in 0..3 {
        let n = r.read_u32().unwrap() as usize;
        fields.push(r.read_bytes(n).unwrap().to_vec());
    }
    assert_eq!(fields, [b"u".to_vec(), b"p".to_vec(), Vec::new()]);
}

#[test]
fn corba_9_3_1_1_association_options_then_a_long_and_an_octet_sequence() {
    // `AS_ContextSec`-like: two unsigned shorts, then a sequence count that
    // is already on a four-octet boundary; `CompoundSecMechList`: a
    // boolean, then a count padded to four.
    let mut w = Writer::new(Endianness::Big);
    w.write_u16(0x0040);
    w.write_u16(0x0040);
    w.write_u32(2);
    w.write_bytes(&[0xde, 0xad]);
    w.write_u8(1);
    assert_eq!(w.position(), 11);
    w.write_u32(0);
    assert_eq!(
        w.finish(),
        [
            0x00, 0x40, 0x00, 0x40, 0x00, 0x00, 0x00, 0x02, 0xde, 0xad, 0x01, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ]
    );
}

#[test]
fn a_cut_sas_body_and_the_errors_built_by_hand() {
    let mut r = Reader::new(&MESSAGE_IN_CONTEXT_BE[..12], Endianness::Big);
    r.read_u8().unwrap();
    r.read_i16().unwrap();
    assert!(matches!(
        r.read_u64(),
        Err(WireError::Truncated {
            at: 8,
            needed: 8,
            available: 4
        })
    ));
    assert_eq!(r.position(), 8);

    // `mech_list.rs` builds both of these itself.
    let too_long = WireError::OutOfRange {
        at: 3,
        expected: String::from("a length a u32 can carry"),
    };
    let impossible = WireError::ImpossibleLength {
        at: 4,
        claimed: 9,
        available: 0,
    };
    assert_ne!(too_long, impossible);
    assert!(!impossible.to_string().is_empty());
}
