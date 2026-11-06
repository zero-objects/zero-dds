// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-corba-cosnaming` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire`: `src/wire.rs` encodes and
//! decodes the `CosNaming` operation bodies and exception bodies with a
//! `Writer::new(endianness)`/`Reader::new(body, endianness)` per body:
//! `Name` as `write_u32` + `write_string` pairs (`read_u32`,
//! `read_string`), enumerations (`BindingType`, `NotFoundReason`) as
//! `write_u32`, an exception as its repository id string followed by the
//! members; `Reader::remaining`/`position` and `Writer::position` for the
//! count checks and the `WireError::ImpossibleLength`/`OutOfRange` it
//! builds; `WireError` in the public `NamingWireError::Wire`.
//! `src/naming_daemon.rs` and `src/servant.rs` write system exception
//! bodies the same way. `Endianness` is in the public signatures.
//!
//! Oracle: CORBA 3.3 part 2 §9.3.1.1 (alignment to the size of the
//! primitive), §9.3.2.5 (sequence: count, then the elements), §9.3.2.6
//! (enum: unsigned long), §9.3.2.7 (string: length including the NUL),
//! §9.3.5.5 (exception: repository id string, then the members; a system
//! exception's members are `unsigned long minor` and the `CompletionStatus`
//! enum, `COMPLETED_NO` = 1), with the Naming Service 1.3 types
//! (`NameComponent { string id; string kind; }`, `Name` a sequence of
//! them).
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, WireError, Writer};

/// `Name [ {id "a", kind ""}, {id "bc", kind "k"} ]`, little-endian.
const NAME_LE: [u8; 34] = [
    0x02, 0x00, 0x00, 0x00, // two components
    0x02, 0x00, 0x00, 0x00, b'a', 0x00, // id "a"
    0x00, 0x00, // padding
    0x01, 0x00, 0x00, 0x00, 0x00, // kind ""
    0x00, 0x00, 0x00, // padding
    0x03, 0x00, 0x00, 0x00, b'b', b'c', 0x00, // id "bc"
    0x00, // padding
    0x02, 0x00, 0x00, 0x00, b'k', 0x00, // kind "k"
];

#[test]
fn corba_9_3_2_5_a_name_is_a_count_and_id_kind_string_pairs() {
    let mut w = Writer::new(Endianness::Little);
    w.write_u32(2);
    for s in ["a", "", "bc", "k"] {
        w.write_string(s);
    }
    assert_eq!(w.finish(), NAME_LE);

    let mut r = Reader::new(&NAME_LE, Endianness::Little);
    let n = r.read_u32().unwrap() as usize;
    assert!(n <= r.remaining() / 8);
    let mut parts = Vec::new();
    for _ in 0..2 * n {
        parts.push(r.read_string().unwrap());
    }
    assert_eq!(parts, ["a", "", "bc", "k"]);
    assert_eq!(r.remaining(), 0);
}

#[test]
fn corba_9_3_5_5_system_exception_body_is_repository_id_minor_and_completion() {
    let id = "IDL:omg.org/CORBA/NO_PERMISSION:1.0";
    let mut w = Writer::new(Endianness::Little);
    w.write_string(id);
    w.write_u32(0);
    w.write_u32(1);
    let bytes = w.finish();

    let mut expected = vec![36, 0, 0, 0];
    expected.extend_from_slice(id.as_bytes());
    expected.push(0); // ends at 40, no padding
    expected.extend_from_slice(&[0, 0, 0, 0]); // minor
    expected.extend_from_slice(&[1, 0, 0, 0]); // COMPLETED_NO
    assert_eq!(bytes, expected);
}

#[test]
fn corba_9_3_2_6_not_found_body_pads_the_enum_behind_the_repository_id() {
    // `encode_not_found(NotContext, [])`: the id is 48 characters, so the
    // string ends at 53 and `why` starts at 56.
    let id = "IDL:omg.org/CosNaming/NamingContext/NotFound:1.0";
    let mut w = Writer::new(Endianness::Big);
    w.write_string(id);
    w.write_u32(1);
    w.write_u32(0);
    let bytes = w.finish();

    let mut expected = vec![0, 0, 0, 49];
    expected.extend_from_slice(id.as_bytes());
    expected.push(0);
    expected.extend_from_slice(&[0, 0, 0]); // padding
    expected.extend_from_slice(&[0, 0, 0, 1]); // why = not_context
    expected.extend_from_slice(&[0, 0, 0, 0]); // rest_of_name: empty
    assert_eq!(bytes, expected);

    let mut r = Reader::new(&bytes, Endianness::Big);
    assert_eq!(r.read_string().unwrap(), id);
    assert_eq!(r.read_u32().unwrap(), 1);
    assert_eq!(r.position(), 60);
    assert_eq!(r.read_u32().unwrap(), 0);
}

#[test]
fn a_cut_name_is_a_wire_error() {
    let mut r = Reader::new(&NAME_LE[..22], Endianness::Little);
    r.read_u32().unwrap();
    r.read_string().unwrap();
    r.read_string().unwrap();
    assert!(matches!(
        r.read_string(),
        Err(WireError::Truncated { .. } | WireError::ImpossibleLength { .. })
    ));

    let built = WireError::ImpossibleLength {
        at: 4,
        claimed: 1000,
        available: 30,
    };
    assert!(matches!(
        built,
        WireError::ImpossibleLength { claimed: 1000, .. }
    ));
}
