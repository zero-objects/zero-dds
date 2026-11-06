// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-spatial-dds` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire`: `build.rs` generates the
//! SpatialDDS types from `idl/spatial_core.idl` with
//! `use zerodds_wire::{Narrowed, Reader, TryConstruct, WireError, Writer}`,
//! and every generated type carries the inherent pair
//! `encode(&self, &mut Writer) -> Result<(), WireError>` /
//! `decode(&mut Reader) -> Result<Self, WireError>` in its public API
//! (`src/lib.rs` includes the generated file). The SpatialDDS aggregates
//! are `@appendable`, so the generated bodies are DELIMITED_CDR: a DHEADER
//! written with `write_dheader`/`finish_dheader` and read with
//! `read_dheader` and `position`, then the members with the primitive,
//! string and fixed-array calls. The generated bodies emit no alignment of
//! their own: the MAXALIGN is the one the caller's `Writer`/`Reader` was
//! built with, 4 for this XCDR2 codec.
//!
//! The tests drive the generated codec through that public API and compare
//! with octets written out from the clauses.
//!
//! Oracle: DDS-XTypes 1.3 §7.4.2 (DELIMITED_CDR is a DHEADER ahead of the
//! PLAIN_CDR2 body, 8-octet types aligned to 4), §7.4.3.2.2 Table 37
//! (MAXALIGN 4 for version 2), §7.4.3.4.1 (the DHEADER holds the size of
//! what follows it, not itself), §7.4.1.1.1 Table 31, §7.4.1.1.2,
//! §7.4.1.1.3.1 Table 32 (an enumeration without `@bit_bound` is an Int32),
//! §7.2.4.4.8 Table 19 (a member a shorter writer did not send takes its
//! default), IEEE 754 binary64.
#![allow(clippy::unwrap_used)]

use zerodds_spatial_dds::builtin::Time;
use zerodds_spatial_dds::spatial::common::{CoordConvention, FrameRef};
use zerodds_spatial_dds::spatial::core::PoseSE3;
use zerodds_wire::{Endianness, Reader, WireError, Writer};

fn xcdr2_writer() -> Writer {
    Writer::with_max_align(Endianness::Little, 4)
}

fn xcdr2_reader(bytes: &[u8]) -> Reader<'_> {
    Reader::with_max_align(bytes, Endianness::Little, 4)
}

/// `PoseSE3 { t: [1, 2, 3], q: [0, 0, 0, 1] }`: a DHEADER of 56 and seven
/// doubles from offset 4.
const POSE_SE3: [u8; 60] = [
    0x38, 0x00, 0x00, 0x00, // DHEADER
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0x3f, // t[0] = 1.0
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, // t[1] = 2.0
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x40, // t[2] = 3.0
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // q[0]
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // q[1]
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // q[2]
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0x3f, // q[3] = 1.0
];

fn pose() -> PoseSE3 {
    PoseSE3 {
        t: [1.0, 2.0, 3.0],
        q: [0.0, 0.0, 0.0, 1.0],
    }
}

#[test]
fn xtypes_7_4_2_pose_se3_is_a_dheader_and_seven_doubles_aligned_to_four() {
    let mut w = xcdr2_writer();
    pose().encode(&mut w).unwrap();
    assert_eq!(w.finish(), POSE_SE3);
}

#[test]
fn xtypes_7_4_2_pose_se3_reads_back_from_the_clause_octets() {
    let mut r = xcdr2_reader(&POSE_SE3);
    assert_eq!(PoseSE3::decode(&mut r).unwrap(), pose());
    assert_eq!(r.remaining(), 0);
}

/// `FrameRef { uuid: "u", fqn: "f", has_coord_convention: true,
/// coord_convention: NED }`.
const FRAME_REF: [u8; 24] = [
    0x14, 0x00, 0x00, 0x00, // DHEADER: 20
    0x02, 0x00, 0x00, 0x00, b'u', 0x00, // uuid
    0x00, 0x00, // padding
    0x02, 0x00, 0x00, 0x00, b'f', 0x00, // fqn
    0x01, // has_coord_convention
    0x00, // padding
    0x04, 0x00, 0x00, 0x00, // coord_convention = NED (@value(4))
];

#[test]
fn xtypes_7_4_1_1_2_frame_ref_strings_boolean_and_enumeration() {
    let frame = FrameRef {
        uuid: "u".into(),
        fqn: "f".into(),
        has_coord_convention: true,
        coord_convention: CoordConvention::NED,
    };
    let mut w = xcdr2_writer();
    frame.encode(&mut w).unwrap();
    assert_eq!(w.finish(), FRAME_REF);

    let mut r = xcdr2_reader(&FRAME_REF);
    assert_eq!(FrameRef::decode(&mut r).unwrap(), frame);
    assert_eq!(r.remaining(), 0);
}

#[test]
fn xtypes_7_2_4_4_8_a_dheader_that_ends_early_defaults_the_rest() {
    // `Time` from a writer that sent `sec` only: the DHEADER says four.
    let short = [0x04, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff];
    let mut r = xcdr2_reader(&short);
    assert_eq!(
        Time::decode(&mut r).unwrap(),
        Time {
            sec: -1,
            nanosec: 0
        }
    );
}

#[test]
fn xtypes_7_4_3_4_1_a_dheader_past_the_end_of_the_stream_is_refused() {
    let lying = [0x10, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
    let mut r = xcdr2_reader(&lying);
    assert!(matches!(
        Time::decode(&mut r),
        Err(WireError::ImpossibleLength { at: 0, .. })
    ));
}
