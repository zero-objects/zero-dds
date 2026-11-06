// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-corba-interop` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire`: `src/lib.rs` encodes and
//! decodes the Echo bodies with `Writer::new(Endianness::Big)` +
//! `write_string` and `Reader::new(body, Big)` + `read_string` (the public
//! `WIRE` constant is an `Endianness`), and a system exception body with
//! `write_string` + two `write_u32`. `src/runtime.rs` builds tagged
//! component encapsulations with `Writer::new(Big)`: the byte-order octet,
//! then `write_u32` (code sets), `write_u16` (the SSL component's three
//! shorts at offsets 2, 4 and 6) or `write_string` (Java codebase), and
//! reads them back with `Reader::new(data, endianness)`, `read_u8`,
//! `read_u16` and `read_string`; it hands a `Reader::new` over a
//! `TargetAddress`'s profile or IOR bytes to `zerodds-corba-ior`. The code
//! `build.rs` generates (included only by the binaries in `src/bin/`) uses
//! `Reader`, `Writer` and `WireError` for classic-CDR codecs and cannot be
//! linked into this test.
//!
//! Oracle: CORBA 3.3 part 2 §9.3.1.1 (alignment to the size of the
//! primitive, counted from the start of the encapsulation), §9.3.2.7
//! (string), §9.3.3 (encapsulation: byte-order octet first), §9.3.5.5
//! (system exception body: repository id, `unsigned long minor`,
//! `CompletionStatus`).
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, WireError, Writer};

#[test]
fn corba_9_3_2_7_the_echo_body_is_one_string() {
    let mut w = Writer::new(Endianness::Big);
    w.write_string("hi");
    let body = w.finish();
    assert_eq!(body, [0x00, 0x00, 0x00, 0x03, b'h', b'i', 0x00]);
    assert_eq!(
        Reader::new(&body, Endianness::Big).read_string().unwrap(),
        "hi"
    );

    // A body that is no string is an error from the reader.
    assert!(matches!(
        Reader::new(&[0x00, 0x00, 0x00, 0x00], Endianness::Big).read_string(),
        Err(WireError::ImpossibleLength { claimed: 0, .. })
    ));
}

#[test]
fn corba_9_3_3_code_sets_component_puts_the_first_long_at_offset_four() {
    let mut w = Writer::new(Endianness::Big);
    w.write_u8(0);
    for code_set in [0x0501_0001, 1, 0x0001_0001, 0x0001_0109, 1, 0x0001_0100] {
        w.write_u32(code_set);
    }
    assert_eq!(
        w.finish(),
        [
            0x00, 0x00, 0x00, 0x00, // byte order, padding
            0x05, 0x01, 0x00, 0x01, // native char set UTF-8
            0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, // one conversion set
            0x00, 0x01, 0x01, 0x09, // native wchar set UTF-16
            0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x01, 0x00, // one conversion set
        ]
    );
}

#[test]
fn corba_9_3_1_1_ssl_component_shorts_sit_at_offsets_two_four_and_six() {
    let mut w = Writer::new(Endianness::Big);
    w.write_u8(0);
    w.write_u16(0x0066);
    w.write_u16(0x0006);
    w.write_u16(2810);
    let data = w.finish();
    assert_eq!(data, [0x00, 0x00, 0x00, 0x66, 0x00, 0x06, 0x0a, 0xfa]);

    let mut r = Reader::new(&data, Endianness::Big);
    r.read_u8().unwrap();
    assert_eq!(r.read_u16().unwrap(), 0x0066);
    assert_eq!(r.read_u16().unwrap(), 0x0006);
    assert_eq!(r.read_u16().unwrap(), 2810);
}

#[test]
fn corba_9_3_5_5_system_exception_body() {
    let id = "IDL:omg.org/CORBA/OBJECT_NOT_EXIST:1.0";
    let mut w = Writer::new(Endianness::Big);
    w.write_string(id);
    w.write_u32(0);
    w.write_u32(1);
    let mut expected = vec![0, 0, 0, 39];
    expected.extend_from_slice(id.as_bytes());
    expected.push(0); // ends at 43
    expected.push(0); // padding to 44
    expected.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    assert_eq!(w.finish(), expected);
}
