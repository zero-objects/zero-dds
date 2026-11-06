// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-spatial-ros2` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire`: `build.rs` generates the ROS 2
//! message types from `idl/ros_msgs.idl` with
//! `use zerodds_wire::{Narrowed, Reader, TryConstruct, WireError, Writer}`.
//! Every generated type carries two inherent pairs in its public API:
//! `encode`/`decode` (XCDR2; the messages declare no extensibility, so they
//! are `@appendable` and DELIMITED_CDR) and `encode_xcdr1`/`decode_xcdr1`
//! (PLAIN_CDR, the `CDR_LE` form ROS 2 middlewares use), each taking a
//! `&mut Writer` or `&mut Reader` and returning `Result<_, WireError>`. The
//! bodies call `write_dheader`/`finish_dheader`/`read_dheader`/`position`
//! (version 2 only), the primitive and string calls, and the delimited
//! sequence calls for a sequence of aggregates. They emit no alignment of
//! their own: MAXALIGN is the caller's `Writer`/`Reader`, 4 for `encode`
//! and 8 for `encode_xcdr1`.
//!
//! The tests drive the generated codecs through that public API and compare
//! with octets written out from the clauses.
//!
//! Oracle: DDS-XTypes 1.3 §7.4.3.1 Table 35 (APPENDABLE is PLAIN_CDR in
//! version 1 and DELIMITED_CDR in version 2), §7.4.2 (8-octet types aligned
//! to 4 in version 2), §7.4.3.2.2 Table 37 (MAXALIGN 8 and 4), §7.4.3.4.1
//! (the DHEADER counts what follows it), §7.4.3.5.3 (a sequence of a
//! non-primitive type is delimited in version 2), §7.4.1.1.1 Table 31,
//! §7.4.1.1.2, IEEE 754 binary64.
#![allow(clippy::unwrap_used)]

use zerodds_spatial_ros2::builtin_interfaces::msg::Time;
use zerodds_spatial_ros2::geometry_msgs::msg::{Point, Pose, PoseStamped, Quaternion};
use zerodds_spatial_ros2::std_msgs::msg::Header;
use zerodds_spatial_ros2::tf2_msgs::msg::TFMessage;
use zerodds_wire::{Endianness, Reader, Writer};

fn xcdr2_writer() -> Writer {
    Writer::with_max_align(Endianness::Little, 4)
}

fn xcdr1_writer() -> Writer {
    Writer::with_max_align(Endianness::Little, 8)
}

const ONE: [u8; 8] = [0, 0, 0, 0, 0, 0, 0xf0, 0x3f];
const TWO: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0x40];
const THREE: [u8; 8] = [0, 0, 0, 0, 0, 0, 0x08, 0x40];
const ZERO: [u8; 8] = [0; 8];

fn point() -> Point {
    Point {
        x: 1.0,
        y: 2.0,
        z: 3.0,
    }
}

fn pose_stamped() -> PoseStamped {
    PoseStamped {
        header: Header {
            stamp: Time { sec: 1, nanosec: 2 },
            frame_id: "map1".into(),
        },
        pose: Pose {
            position: point(),
            orientation: Quaternion {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        },
    }
}

fn concat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

#[test]
fn xtypes_7_4_2_point_in_xcdr2_is_a_dheader_and_doubles_from_offset_4() {
    let expected = concat(&[&[0x18, 0, 0, 0], &ONE, &TWO, &THREE]);
    let mut w = xcdr2_writer();
    point().encode(&mut w).unwrap();
    assert_eq!(w.finish(), expected);

    let mut r = Reader::with_max_align(&expected, Endianness::Little, 4);
    assert_eq!(Point::decode(&mut r).unwrap(), point());
}

#[test]
fn xtypes_7_4_3_1_point_in_xcdr1_is_the_doubles_without_a_dheader() {
    let expected = concat(&[&ONE, &TWO, &THREE]);
    let mut w = xcdr1_writer();
    point().encode_xcdr1(&mut w).unwrap();
    assert_eq!(w.finish(), expected);

    let mut r = Reader::with_max_align(&expected, Endianness::Little, 8);
    assert_eq!(Point::decode_xcdr1(&mut r).unwrap(), point());
}

#[test]
fn xtypes_7_4_3_4_1_pose_stamped_in_xcdr2_delimits_every_nested_struct() {
    let expected = concat(&[
        &[0x60, 0, 0, 0], // PoseStamped DHEADER: 96
        &[0x15, 0, 0, 0], // Header DHEADER: 21
        &[0x08, 0, 0, 0], // Time DHEADER: 8
        &[1, 0, 0, 0, 2, 0, 0, 0],
        &[5, 0, 0, 0, b'm', b'a', b'p', b'1', 0], // frame_id at 20
        &[0, 0, 0],                               // padding to 32
        &[0x40, 0, 0, 0],                         // Pose DHEADER: 64
        &[0x18, 0, 0, 0],                         // Point DHEADER: 24
        &ONE,
        &TWO,
        &THREE,
        &[0x20, 0, 0, 0], // Quaternion DHEADER: 32, x at 68 not 72
        &ZERO,
        &ZERO,
        &ZERO,
        &ONE,
    ]);
    assert_eq!(expected.len(), 100);
    let mut w = xcdr2_writer();
    pose_stamped().encode(&mut w).unwrap();
    assert_eq!(w.finish(), expected);

    let mut r = Reader::with_max_align(&expected, Endianness::Little, 4);
    assert_eq!(PoseStamped::decode(&mut r).unwrap(), pose_stamped());
    assert_eq!(r.remaining(), 0);
}

#[test]
fn xtypes_7_4_3_1_pose_stamped_in_xcdr1_aligns_the_doubles_to_eight() {
    let expected = concat(&[
        &[1, 0, 0, 0, 2, 0, 0, 0],
        &[5, 0, 0, 0, b'm', b'a', b'p', b'1', 0], // frame_id at 8
        &[0, 0, 0, 0, 0, 0, 0],                   // padding to 24
        &ONE,
        &TWO,
        &THREE,
        &ZERO,
        &ZERO,
        &ZERO,
        &ONE,
    ]);
    assert_eq!(expected.len(), 80);
    let mut w = xcdr1_writer();
    pose_stamped().encode_xcdr1(&mut w).unwrap();
    assert_eq!(w.finish(), expected);

    let mut r = Reader::with_max_align(&expected, Endianness::Little, 8);
    assert_eq!(PoseStamped::decode_xcdr1(&mut r).unwrap(), pose_stamped());
    assert_eq!(r.remaining(), 0);
}

#[test]
fn xtypes_7_4_3_5_3_a_sequence_of_structs_is_delimited_in_xcdr2_only() {
    let empty = TFMessage {
        transforms: Vec::new(),
    };
    let mut w = xcdr2_writer();
    empty.encode(&mut w).unwrap();
    assert_eq!(
        w.finish(),
        [
            0x08, 0, 0, 0, // TFMessage DHEADER: 8
            0x04, 0, 0, 0, // sequence DHEADER: 4
            0x00, 0, 0, 0, // count
        ]
    );

    let mut w = xcdr1_writer();
    empty.encode_xcdr1(&mut w).unwrap();
    assert_eq!(w.finish(), [0, 0, 0, 0]);
}
