// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-spatial-dds` with `zerodds-lang-rules`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-lang-rules` (`build.rs`, a build
//! dependency): `apply` with `rule::rust::catalogue()`,
//! `adapter::cst::{rust_item_pairs, nest_by_module}` and `encode::rust_codec`
//! for every declared aggregate of `idl/spatial_core.idl`, keyed by the
//! encoding its extensibility asks for (DDS-XTypes 1.3 §7.4.2, Table 59);
//! `src/lib.rs` includes the result at the crate root. A SpatialDDS peer
//! reads what these codecs write, so the contract is their octets.
//!
//! Oracle: the IDL (`@extensibility(APPENDABLE)`, `int32`, `uint32`,
//! `typedef double Vec3[3]`, `typedef double QuaternionXYZW[4]`) and
//! DDS-XTypes 1.3 §7.4.3.4.2 (APPENDABLE under XCDR2 is DELIMITED_CDR: a
//! DHEADER that counts what follows), §7.4.1.1.1 (under XCDR2 a value
//! aligns to its size up to four), §7.4.3.5.6 (an array is its elements,
//! no length).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_spatial_dds::builtin::Time;
use zerodds_spatial_dds::spatial::core::PoseSE3;
use zerodds_wire::{Endianness, Reader, Writer};

#[test]
fn xtypes_7_4_3_4_2_an_appendable_time_is_a_dheader_and_its_two_members() {
    let mut w = Writer::with_max_align(Endianness::Little, 4);
    Time { sec: 1, nanosec: 2 }.encode(&mut w).unwrap();
    let bytes = w.finish();
    assert_eq!(bytes, [8, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0]);
    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(Time::decode(&mut r).unwrap(), Time { sec: 1, nanosec: 2 });
}

#[test]
fn xtypes_7_4_1_1_1_the_arrays_of_a_pose_follow_the_dheader_at_four_under_xcdr2() {
    let pose = PoseSE3 {
        t: [1.0, 2.0, 3.0],
        q: [0.0, 0.0, 0.0, 1.0],
    };
    let mut w = Writer::with_max_align(Endianness::Little, 4);
    pose.encode(&mut w).unwrap();
    let bytes = w.finish();
    let mut expected = vec![56, 0, 0, 0];
    for value in [1.0f64, 2.0, 3.0, 0.0, 0.0, 0.0, 1.0] {
        expected.extend_from_slice(&value.to_le_bytes());
    }
    assert_eq!(bytes, expected);
}
