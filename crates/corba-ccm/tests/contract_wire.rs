// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-corba-ccm` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire` (`src/dynamic_api.rs`):
//! `DynAny::from_type_code` checks that the CDR bytes of a primitive
//! `any` value hold the primitive's layout with
//! `Reader::new(raw, Endianness::Little)` and one of `read_u8`
//! (boolean, char, octet), `read_u16` (short, unsigned short, wchar),
//! `read_u32` (long, unsigned long, float, and the length of a string) or
//! `read_u64` (long long, unsigned long long, double), and propagates a
//! `WireError` as `DynAnyError::Decode` (its `Display` goes into
//! `DynAnyError`'s). `Endianness` is in the public signatures of
//! `Request::invoke` and `dispatch_dsi`.
//!
//! Oracle: CORBA 3.3 part 2 §9.3.1 (sizes of the primitive types: octet,
//! boolean and char one octet, short two, long and float four, long long
//! and double eight; each aligned to its size from the start of the
//! stream), §9.3.2.7 (a string starts with its unsigned long length).
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, WireError};

#[test]
fn corba_9_3_1_each_primitive_size_is_read_from_a_buffer_that_holds_it() {
    assert_eq!(
        Reader::new(&[0x01], Endianness::Little).read_u8().unwrap(),
        1
    );
    assert_eq!(
        Reader::new(&[0x34, 0x12], Endianness::Little)
            .read_u16()
            .unwrap(),
        0x1234
    );
    assert_eq!(
        Reader::new(&[0x78, 0x56, 0x34, 0x12], Endianness::Little)
            .read_u32()
            .unwrap(),
        0x1234_5678
    );
    assert_eq!(
        Reader::new(&[1, 0, 0, 0, 0, 0, 0, 0], Endianness::Little)
            .read_u64()
            .unwrap(),
        1
    );
}

#[test]
fn corba_9_3_1_a_buffer_one_octet_short_of_the_primitive_is_a_decode_error() {
    assert!(matches!(
        Reader::new(&[], Endianness::Little).read_u8(),
        Err(WireError::Truncated { needed: 1, .. })
    ));
    assert!(matches!(
        Reader::new(&[0], Endianness::Little).read_u16(),
        Err(WireError::Truncated { needed: 2, .. })
    ));
    assert!(matches!(
        Reader::new(&[0; 3], Endianness::Little).read_u32(),
        Err(WireError::Truncated { needed: 4, .. })
    ));
    let error = Reader::new(&[0; 7], Endianness::Little)
        .read_u64()
        .unwrap_err();
    assert!(matches!(error, WireError::Truncated { needed: 8, .. }));
    assert!(!error.to_string().is_empty());
}
