// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-rtps` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire` (`src/topic_data.rs`,
//! `src/publication_data.rs`, `src/subscription_data.rs`):
//! - `Writer::new(Endianness::Little)`, `write_u32`, `write_u16` and
//!   `finish` for the `DATA_REPRESENTATION` value of a topic (a `u32` count,
//!   then one `u16` per id); the octets are padded to four by this crate.
//! - `Reader::new(value, Endianness::Little | Big)`, `read_u32` and
//!   `read_u16` for the same value in either byte order; an error from the
//!   reader becomes `WireError::ValueOutOfRange` ("does not decode").
//! - The same `Writer` and `Reader` handed to the `encode_into` and
//!   `decode_from` of the `zerodds-qos` policies (`DURABILITY_SERVICE`,
//!   `PRESENTATION` and the policies of a topic), which are written and
//!   read with alignment counted from the start of the parameter value.
//! - `Endianness` compared with `==` to pass the byte order on.
//!
//! Oracle: CORBA 3.3 part 2 §9.3.1.1 (alignment from the start of the
//! stream), §9.3.1.2 (the two byte orders), §9.3.2.5 (a sequence is its
//! `unsigned long` length and the elements); DDS-XTypes 1.3 §7.6.3.1.1
//! (`XCDR_DATA_REPRESENTATION` = 0, `XCDR2_DATA_REPRESENTATION` = 2).
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, Writer};

#[test]
fn corba_9_3_2_5_data_representation_is_a_count_and_one_short_per_id_little_endian() {
    // DATA_REPRESENTATION = [XCDR, XCDR2], written as topic_data.rs does.
    let mut w = Writer::new(Endianness::Little);
    w.write_u32(2);
    w.write_u16(0);
    w.write_u16(2);
    assert_eq!(
        w.finish(),
        [0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00],
        "u32 count 2, then the shorts 0 and 2, least significant octet first"
    );
}

#[test]
fn corba_9_3_1_2_reader_takes_the_byte_order_it_is_given() {
    let big = [0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02];
    let mut r = Reader::new(&big, Endianness::Big);
    assert_eq!(r.read_u32().unwrap(), 2);
    assert_eq!(r.read_u16().unwrap(), 0);
    assert_eq!(r.read_u16().unwrap(), 2);

    let little = [0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00];
    let mut r = Reader::new(&little, Endianness::Little);
    assert_eq!(r.read_u32().unwrap(), 2);
    assert_eq!(r.read_u16().unwrap(), 0);
    assert_eq!(r.read_u16().unwrap(), 2);
}

#[test]
fn a_value_shorter_than_its_count_is_an_error_not_a_panic() {
    // Count 2, one short present: the second `read_u16` has one octet.
    let short = [0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02];
    let mut r = Reader::new(&short, Endianness::Little);
    assert_eq!(r.read_u32().unwrap(), 2);
    assert_eq!(r.read_u16().unwrap(), 0);
    assert!(r.read_u16().is_err());

    // A value of three octets has no `u32`.
    let mut r = Reader::new(&[0x01, 0x00, 0x00], Endianness::Big);
    assert!(r.read_u32().is_err());
}

#[test]
fn corba_9_3_1_1_alignment_counts_from_the_start_of_the_value() {
    // A short then a long: two octets of padding before the long, counted
    // from the writer's first octet (the start of the parameter value).
    let mut w = Writer::new(Endianness::Little);
    w.write_u16(0x0102);
    w.write_u32(0x0304_0506);
    let bytes = w.finish();
    assert_eq!(bytes, [0x02, 0x01, 0x00, 0x00, 0x06, 0x05, 0x04, 0x03]);

    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(r.read_u16().unwrap(), 0x0102);
    assert_eq!(r.read_u32().unwrap(), 0x0304_0506);

    let mut w = Writer::new(Endianness::Big);
    w.write_u16(0x0102);
    w.write_u32(0x0304_0506);
    assert_eq!(w.finish(), [0x01, 0x02, 0x00, 0x00, 0x03, 0x04, 0x05, 0x06]);
}

#[test]
fn the_qos_policies_take_this_writer_and_this_reader() {
    // `zerodds-qos` and this crate must name the same `Writer` and
    // `Reader`; a second copy of zerodds-wire in the graph fails here.
    let _: fn(zerodds_qos::DurabilityServiceQosPolicy, &mut Writer) =
        zerodds_qos::DurabilityServiceQosPolicy::encode_into;
    let _: fn(zerodds_qos::PresentationQosPolicy, &mut Writer) =
        zerodds_qos::PresentationQosPolicy::encode_into;
    let decode: for<'a, 'b> fn(
        &'a mut Reader<'b>,
    ) -> Result<
        zerodds_qos::PresentationQosPolicy,
        zerodds_wire::WireError,
    > = zerodds_qos::PresentationQosPolicy::decode_from;
    // An empty value does not decode; the caller falls back to the default.
    let mut r = Reader::new(&[], Endianness::Little);
    assert!(decode(&mut r).is_err());
}

#[test]
fn endianness_is_a_comparable_value() {
    fn value<T: Copy + Eq + core::fmt::Debug>() {}
    value::<Endianness>();
    assert_ne!(Endianness::Little, Endianness::Big);
    assert!(Endianness::Little.is_little());
    assert!(!Endianness::Big.is_little());
}
