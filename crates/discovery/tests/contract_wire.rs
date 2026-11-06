// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-discovery` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire` (`src/type_lookup/client.rs`,
//! `src/type_lookup/mod.rs`): `Writer::new(Endianness::Little)` and
//! `finish` in `request_types_payload` and `request_dependencies_payload`,
//! the writer handed to the `encode_into` of the `zerodds-types` TypeLookup
//! requests; `WireError` as the error type of those two functions and of
//! the `TypeLookupStack` request and reply builders, propagated with `?`
//! from `zerodds-types`.
//!
//! What a writer of `Writer::new` is: little- or big-endian as asked, with
//! the alignment cap of eight that DDS-XTypes 1.3 §7.4.1.1.1 gives the
//! encoding version 1 (a `u64` after a `u32` is padded to eight). The
//! TypeLookup endpoints use the encoding version 2 (DDS-XTypes 1.4 Beta 1
//! §7.6.3.3.3), whose cap is four (§7.4.2); `contract_types.rs` checks the
//! version-2 bytes of the requests.
//!
//! Oracle: CORBA 3.3 part 2 §9.3.1.1 (alignment from the start of the
//! stream), §9.3.1.2 (byte order); DDS-XTypes 1.3 §7.4.1.1.1 and §7.4.2
//! (MAXALIGN 8 and 4).
#![allow(clippy::unwrap_used)]

use zerodds_types::TypeIdentifier;
use zerodds_types::type_lookup::ContinuationPoint;
use zerodds_wire::{Endianness, WireError, Writer};

#[test]
fn xtypes_7_4_1_1_1_writer_new_is_little_endian_with_maxalign_eight() {
    let mut w = Writer::new(Endianness::Little);
    assert_eq!(w.endianness(), Endianness::Little);
    assert_eq!(w.max_align(), 8);
    w.write_u32(0x0102_0304);
    w.write_u64(0x0506_0708_090a_0b0c);
    assert_eq!(
        w.finish(),
        [
            0x04, 0x03, 0x02, 0x01, // u32, least significant octet first
            0x00, 0x00, 0x00, 0x00, // padding to eight
            0x0c, 0x0b, 0x0a, 0x09, 0x08, 0x07, 0x06, 0x05,
        ]
    );
}

#[test]
fn xtypes_7_4_2_a_cap_of_four_is_what_tells_version_2_apart() {
    // The same two values under MAXALIGN 4: no padding before the u64.
    let mut w = Writer::with_max_align(Endianness::Little, 4);
    w.write_u32(0x0102_0304);
    w.write_u64(0x0506_0708_090a_0b0c);
    assert_eq!(
        w.finish(),
        [
            0x04, 0x03, 0x02, 0x01, 0x0c, 0x0b, 0x0a, 0x09, 0x08, 0x07, 0x06, 0x05,
        ]
    );
}

#[test]
fn the_payload_builders_return_this_wire_error() {
    // `zerodds-types` and this crate name the same `WireError`; `?` in
    // client.rs and mod.rs relies on it.
    let _: fn(&[TypeIdentifier]) -> Result<Vec<u8>, WireError> =
        zerodds_discovery::type_lookup::request_types_payload;
    let _: fn(&[TypeIdentifier], ContinuationPoint) -> Result<Vec<u8>, WireError> =
        zerodds_discovery::type_lookup::request_dependencies_payload;
    let _: fn(&zerodds_types::type_lookup::GetTypesRequest, &mut Writer) -> Result<(), WireError> =
        zerodds_types::type_lookup::GetTypesRequest::encode_into;
    fn error<E: core::error::Error + core::fmt::Display + Clone + PartialEq>() {}
    error::<WireError>();
}
