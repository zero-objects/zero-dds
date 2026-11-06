// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-spatial-ros2` with `zerodds-lang-rules`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-lang-rules` (`build.rs`, a build
//! dependency): `apply` with `rule::rust::catalogue()`,
//! `adapter::cst::{rust_item_pairs, nest_by_module}`, and
//! `encode::rust_codec_pair` for every ROS 2 message of `idl/ros_msgs.idl`:
//! `encode`/`decode` under XCDR2 and `encode_xcdr1`/`decode_xcdr1` under
//! XCDR1 (`SECONDARY_SUFFIX`), the encoding the ROS 2 middlewares put on
//! the wire. `src/lib.rs` includes the result at the crate root.
//!
//! Oracle: the IDL (`geometry_msgs::msg::Point { double x; double y; double
//! z; }`, no extensibility annotation) and DDS-XTypes 1.3 §7.4.1.1 (XCDR1:
//! PLAIN_CDR, the members in order, a `double` aligned to eight) and
//! §7.4.3.4.2 with §7.3.1.2.1.8 (XCDR2: an unannotated aggregate is
//! APPENDABLE, a DHEADER before its members).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_spatial_ros2::geometry_msgs::msg::Point;
use zerodds_wire::{Endianness, Reader, Writer};

fn doubles(values: &[f64]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

#[test]
fn xtypes_7_4_1_1_a_ros_point_under_xcdr1_is_its_three_doubles() {
    let point = Point {
        x: 1.0,
        y: 2.0,
        z: 3.0,
    };
    let mut w = Writer::new(Endianness::Little);
    point.encode_xcdr1(&mut w).unwrap();
    let bytes = w.finish();
    assert_eq!(bytes, doubles(&[1.0, 2.0, 3.0]));
    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(Point::decode_xcdr1(&mut r).unwrap(), point);
}

#[test]
fn xtypes_7_4_3_4_2_the_same_point_under_xcdr2_carries_a_dheader() {
    let point = Point {
        x: 1.0,
        y: 2.0,
        z: 3.0,
    };
    let mut w = Writer::with_max_align(Endianness::Little, 4);
    point.encode(&mut w).unwrap();
    let mut expected = vec![24, 0, 0, 0];
    expected.extend(doubles(&[1.0, 2.0, 3.0]));
    assert_eq!(w.finish(), expected);
}
