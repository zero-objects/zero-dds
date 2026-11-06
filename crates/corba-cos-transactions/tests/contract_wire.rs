// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-corba-cos-transactions` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire`: `src/otid.rs` and
//! `src/propagation.rs` put `Writer`, `Reader` and `WireError` into the
//! public `encode`/`decode` of `Otid` and `PropagationContext`, and
//! `Endianness` into `to_service_context_data`. The encapsulation is
//! `Writer::new(endianness)` with the byte-order octet first; the body is
//! `write_u32` (timeout, counts, the `tk_null` TypeCode), `write_i32`
//! (`formatID`, `bqual_length`), `write_u32` + `write_bytes` (`tid`), and
//! an object reference either as a nil IOR (`write_string("")`,
//! `write_u32(0)`) or as the raw IOR bytes with `write_bytes`. Decoding
//! uses `Reader::new`, `read_u8`, `read_u32`, `read_i32`, `read_u16`,
//! `read_string`, `read_bytes`, and re-serializes an IOR with
//! `Writer::new(reader.endianness())`. The crate builds
//! `WireError::Truncated` and `WireError::OutOfRange` itself.
//!
//! Oracle: CORBA 3.3 part 2 §9.3.1.1 (each primitive aligned to its size,
//! counted from the start of the encapsulation), §9.3.2.5 (sequence),
//! §9.3.2.7 (string), §9.3.3 (encapsulation), §7.6.2 (an object reference
//! is an IOR, `string type_id` first, so it starts on a four-octet
//! boundary; the nil reference has an empty type id and no profiles), with
//! the `CosTransactions::PropagationContext` and `otid_t` of the OMG
//! Transaction Service.
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, WireError, Writer};

/// `PropagationContext { timeout: 30, current: { nil, nil, otid { formatID
/// 0x1234, bqual_length 0, tid [aa bb] } }, parents: [], impl data tk_null }`
/// as a big-endian encapsulation.
const CONTEXT_BE: [u8; 56] = [
    0x00, 0x00, 0x00, 0x00, // byte order, padding
    0x00, 0x00, 0x00, 0x1e, // timeout at 4
    0x00, 0x00, 0x00, 0x01, 0x00, // coordinator type_id ""
    0x00, 0x00, 0x00, // padding
    0x00, 0x00, 0x00, 0x00, // coordinator: no profiles
    0x00, 0x00, 0x00, 0x01, 0x00, // terminator type_id ""
    0x00, 0x00, 0x00, // padding
    0x00, 0x00, 0x00, 0x00, // terminator: no profiles
    0x00, 0x00, 0x12, 0x34, // formatID at 32
    0x00, 0x00, 0x00, 0x00, // bqual_length
    0x00, 0x00, 0x00, 0x02, 0xaa, 0xbb, // tid
    0x00, 0x00, // padding
    0x00, 0x00, 0x00, 0x00, // no parents at 48
    0x00, 0x00, 0x00, 0x00, // implementation_specific_data: tk_null
];

#[test]
fn corba_9_3_3_propagation_context_with_nil_references_in_an_encapsulation() {
    let mut w = Writer::new(Endianness::Big);
    w.write_u8(0);
    w.write_u32(30);
    for _ in 0..2 {
        w.write_string("");
        w.write_u32(0);
    }
    w.write_i32(0x1234);
    w.write_i32(0);
    w.write_u32(2);
    w.write_bytes(&[0xaa, 0xbb]);
    w.write_u32(0);
    w.write_u32(0);
    assert_eq!(w.finish(), CONTEXT_BE);

    let mut r = Reader::new(&CONTEXT_BE, Endianness::Big);
    assert_eq!(r.endianness(), Endianness::Big);
    assert_eq!(r.read_u8().unwrap(), 0);
    assert_eq!(r.read_u32().unwrap(), 30);
    for _ in 0..2 {
        assert_eq!(r.read_string().unwrap(), "");
        assert_eq!(r.read_u32().unwrap(), 0);
    }
    assert_eq!(r.read_i32().unwrap(), 0x1234);
    assert_eq!(r.read_i32().unwrap(), 0);
    let n = r.read_u32().unwrap() as usize;
    assert_eq!(r.read_bytes(n).unwrap(), [0xaa, 0xbb]);
    assert_eq!(r.read_u32().unwrap(), 0);
    assert_eq!(r.read_u32().unwrap(), 0);
    assert_eq!(r.remaining(), 0);
}

#[test]
fn corba_9_3_1_1_raw_ior_bytes_are_not_aligned_by_write_bytes() {
    // An IOR built by its own writer from offset 0: type_id "A", one
    // profile with three octets of data. It is 23 octets long, so what
    // follows it in a stream starts at an offset that is 3 modulo 4.
    let mut ior = Writer::new(Endianness::Big);
    ior.write_string("A");
    ior.write_u32(1);
    ior.write_u32(0);
    ior.write_u32(3);
    ior.write_bytes(&[0xaa, 0xbb, 0xcc]);
    let ior = ior.finish();
    assert_eq!(ior.len(), 23);

    // `write_bytes` puts the octets where the stream is; it does not align.
    let mut w = Writer::new(Endianness::Big);
    w.write_bytes(&ior);
    w.write_bytes(&ior);
    assert_eq!(w.position(), 46);

    // A second IOR in the stream has to start on a four-octet boundary
    // (its first member is the type_id length): the writer must be
    // aligned before the raw bytes go in.
    let mut w = Writer::new(Endianness::Big);
    w.write_bytes(&ior);
    w.align_to(4);
    assert_eq!(w.position(), 24);
    w.write_bytes(&ior);
    let stream = w.finish();
    let mut r = Reader::new(&stream, Endianness::Big);
    for _ in 0..2 {
        assert_eq!(r.read_string().unwrap(), "A");
        assert_eq!(r.read_u32().unwrap(), 1);
        assert_eq!(r.read_u32().unwrap(), 0);
        let n = r.read_u32().unwrap() as usize;
        assert_eq!(r.read_bytes(n).unwrap(), [0xaa, 0xbb, 0xcc]);
    }
}

#[test]
fn a_cut_context_and_the_errors_built_by_hand() {
    let mut r = Reader::new(&CONTEXT_BE[..45], Endianness::Big);
    r.read_u8().unwrap();
    r.read_u32().unwrap();
    for _ in 0..2 {
        r.read_string().unwrap();
        r.read_u32().unwrap();
    }
    r.read_i32().unwrap();
    r.read_i32().unwrap();
    let n = r.read_u32().unwrap() as usize;
    assert!(matches!(
        r.read_bytes(n),
        Err(WireError::Truncated { at: 44, .. })
    ));

    let empty = WireError::Truncated {
        at: 0,
        needed: 1,
        available: 0,
    };
    assert!(!empty.to_string().is_empty());
}
