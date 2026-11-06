// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-corba-giop` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire`: `Endianness` in its public API
//! (`Flags::from_endianness`/`endianness`, `encode_message`,
//! `decode_message_ctx`, the encapsulation helpers) and `WireError` inside
//! `GiopError::Wire` (`src/error.rs`). `src/codec.rs` encodes a message
//! with `Writer::new(endianness).recording_padding()`, twelve `write_u8(0)`
//! for the header, the body, `padding_spans` and `finish`, and decodes with
//! `Reader::new(&bytes[..end], endianness)` and `skip_to(HEADER_SIZE)`, so
//! that the body aligns from the start of the message. `src/header.rs`
//! writes and reads the header with `write_bytes`/`read_bytes`,
//! `write_u8`/`read_u8` and `write_u32`/`read_u32`, the size through a
//! second `Reader` in the flagged byte order. The message bodies
//! (`request.rs`, `reply.rs`, `target_address.rs`, `service_context.rs`,
//! `locate_*.rs`, `fragment.rs`, `cancel_request.rs`) use
//! `write_u32`/`read_u32`, `write_u16`/`read_u16`, `write_u8`/`read_u8`,
//! `write_string`/`read_string`, `write_bytes`/`read_bytes`, `align_to(2)`
//! and `align_to(8)` and `remaining`. `code_set.rs` builds a CDR
//! encapsulation with its own `Writer`/`Reader`; `object_ops.rs` writes a
//! `boolean`, a `string` and a nil object reference.
//!
//! Oracle: CORBA 3.3 part 2 §9.3.1.1 (primitives aligned to their size,
//! counted from the start of the GIOP message), §9.3.1.5 (boolean: 1 and
//! 0), §9.3.2.7 (string: unsigned long length including the NUL), §9.3.3
//! (an encapsulation starts with its byte-order octet and aligns from it),
//! §9.4.1 (message header: magic "GIOP", version, flags with the byte
//! order in bit 0, message type, `message_size` in that byte order),
//! §9.4.2 (GIOP 1.2 Request header; the body 8-aligned when present).
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, WireError, Writer};

#[test]
fn corba_9_4_1_message_header_and_the_size_in_the_flagged_byte_order() {
    for (endianness, flags, size) in [
        (Endianness::Big, 0x00, [0x00, 0x00, 0x00, 0x10]),
        (Endianness::Little, 0x01, [0x10, 0x00, 0x00, 0x00]),
    ] {
        let mut w = Writer::new(endianness);
        w.write_bytes(b"GIOP");
        w.write_u8(1);
        w.write_u8(2);
        w.write_u8(flags);
        w.write_u8(0);
        w.write_u32(0x10);
        let bytes = w.finish();
        let mut expected = b"GIOP".to_vec();
        expected.extend_from_slice(&[1, 2, flags, 0]);
        expected.extend_from_slice(&size);
        assert_eq!(bytes, expected);

        let mut head = Reader::new(&bytes[..8], Endianness::Big);
        assert_eq!(head.read_bytes(4).unwrap(), b"GIOP");
        assert_eq!(head.read_u8().unwrap(), 1);
        assert_eq!(head.read_u8().unwrap(), 2);
        assert_eq!(head.read_u8().unwrap(), flags);
        assert_eq!(head.read_u8().unwrap(), 0);
        let mut size_reader = Reader::new(&bytes[8..12], endianness);
        assert_eq!(size_reader.read_u32().unwrap(), 0x10);
    }
}

/// A GIOP 1.2 Request, big-endian: request id 5, response flags 3,
/// `KeyAddr` with the key `0a 0b 0c`, operation "op", no service context,
/// body `ab cd`.
const REQUEST_1_2: [u8; 50] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // header, patched later
    0x00, 0x00, 0x00, 0x05, // request_id at 12
    0x03, 0x00, 0x00, 0x00, // response_flags, reserved[3]
    0x00, 0x00, // AddressingDisposition KeyAddr at 20
    0x00, 0x00, // padding
    0x00, 0x00, 0x00, 0x03, 0x0a, 0x0b, 0x0c, // object_key at 24
    0x00, // padding
    0x00, 0x00, 0x00, 0x03, b'o', b'p', 0x00, // operation at 32
    0x00, // padding
    0x00, 0x00, 0x00, 0x00, // service_context count at 40
    0x00, 0x00, 0x00, 0x00, // padding to 48
    0xab, 0xcd, // body
];

#[test]
fn corba_9_4_2_giop_1_2_request_aligns_from_the_start_of_the_message() {
    let mut w = Writer::new(Endianness::Big).recording_padding();
    for _ in 0..12 {
        w.write_u8(0);
    }
    w.write_u32(5);
    w.write_u8(0x03);
    w.write_u8(0);
    w.write_u8(0);
    w.write_u8(0);
    w.align_to(2);
    w.write_u16(0);
    w.write_u32(3);
    w.write_bytes(&[0x0a, 0x0b, 0x0c]);
    w.write_string("op");
    w.write_u32(0);
    w.align_to(8);
    w.write_bytes(&[0xab, 0xcd]);
    assert_eq!(w.padding_spans(), [(22, 2), (31, 1), (39, 1), (44, 4)]);
    assert_eq!(w.finish(), REQUEST_1_2);

    let mut r = Reader::new(&REQUEST_1_2, Endianness::Big);
    r.skip_to(12).unwrap();
    assert_eq!(r.read_u32().unwrap(), 5);
    assert_eq!(r.read_u8().unwrap(), 0x03);
    for _ in 0..3 {
        assert_eq!(r.read_u8().unwrap(), 0);
    }
    r.align_to(2).unwrap();
    assert_eq!(r.read_u16().unwrap(), 0);
    let key_len = r.read_u32().unwrap() as usize;
    assert_eq!(r.read_bytes(key_len).unwrap(), [0x0a, 0x0b, 0x0c]);
    assert_eq!(r.read_string().unwrap(), "op");
    assert_eq!(r.read_u32().unwrap(), 0);
    assert!(r.remaining() > 0);
    r.align_to(8).unwrap();
    assert_eq!(r.read_bytes(r.remaining()).unwrap(), [0xab, 0xcd]);
}

#[test]
fn corba_9_3_3_an_encapsulation_aligns_from_its_byte_order_octet() {
    // `CodeSetContext::encode_encapsulation`: byte order, char_data,
    // wchar_data (ISO 8859-1 0x00010001, UTF-16 0x00010109).
    let mut w = Writer::new(Endianness::Little);
    w.write_u8(1);
    w.write_u32(0x0001_0001);
    w.write_u32(0x0001_0109);
    let encapsulation = w.finish();
    assert_eq!(
        encapsulation,
        [
            0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x09, 0x01, 0x01, 0x00
        ]
    );

    let mut r = Reader::new(&encapsulation, Endianness::Little);
    assert_eq!(r.read_u8().unwrap(), 1);
    assert_eq!(r.read_u32().unwrap(), 0x0001_0001);
    assert_eq!(r.read_u32().unwrap(), 0x0001_0109);
}

#[test]
fn corba_9_3_1_5_boolean_and_the_nil_object_reference() {
    let mut w = Writer::new(Endianness::Big);
    w.write_bool(true);
    w.write_bool(false);
    assert_eq!(w.finish(), [1, 0]);

    // `object_ops::encode_nil_object_reference`: an IOR with an empty
    // type_id and no profiles.
    let mut w = Writer::new(Endianness::Big);
    w.write_string("");
    w.write_u32(0);
    assert_eq!(w.finish(), [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn a_message_shorter_than_its_fields_is_a_wire_error() {
    let mut r = Reader::new(&[0x00, 0x00, 0x00, 0x08, 0x01], Endianness::Big);
    let n = r.read_u32().unwrap() as usize;
    assert!(matches!(
        r.read_bytes(n),
        Err(WireError::Truncated {
            at: 4,
            needed: 8,
            available: 1
        })
    ));

    let mut r = Reader::new(&[0u8; 8], Endianness::Big);
    assert!(matches!(r.skip_to(12), Err(WireError::Truncated { .. })));

    let mut r = Reader::new(&[0x00, 0x00, 0x00, 0x09, b'o'], Endianness::Big);
    assert!(matches!(
        r.read_string(),
        Err(WireError::ImpossibleLength { at: 0, .. })
    ));
}
