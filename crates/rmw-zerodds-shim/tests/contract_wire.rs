// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `rmw-zerodds-shim` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire` (`src/spatial_bridge.rs`): the
//! `DdsType` wrappers of the bridged SpatialDDS and ROS 2 types build the
//! writer and reader for the generated codecs. The XCDR2 pair
//! (`bridge_xcdr2_codec!`, every bridged type) uses
//! `Writer::new(Endianness::Little)` and `Reader::new(bytes,
//! Endianness::Little)`; the XCDR1 pair of the ROS types
//! (`bridge_ros_type!`) uses `Writer::with_max_align(Little, 8)` and
//! `Reader::with_max_align(bytes, Little, 8)`; both call `finish`. The
//! generated `encode`/`decode` emit no alignment of their own, so the
//! MAXALIGN of the stream is the one the writer and reader were built with.
//!
//! Oracle: DDS-XTypes 1.3 §7.4.3.2.2 Table 37 (MAXALIGN is 8 for encoding
//! version 1 and 4 for version 2), §7.4.2 (DELIMITED_CDR: a DHEADER, then
//! the members with 8-octet types aligned to 4), §7.4.3.1 Table 35
//! (APPENDABLE is PLAIN_CDR in version 1), §7.4.1.1.1 Table 31, §7.4.1.1.2,
//! IEEE 754 binary64. No participant is created.
#![allow(clippy::unwrap_used)]

use zerodds_spatial_dds::spatial::core::GeoPose;
use zerodds_spatial_ros2::builtin_interfaces::msg::Time;
use zerodds_spatial_ros2::geometry_msgs::msg::{Point, Pose, PoseStamped, Quaternion};
use zerodds_spatial_ros2::std_msgs::msg::Header;
use zerodds_wire::{Endianness, Reader, Writer};

#[test]
fn xtypes_7_4_3_2_2_writer_new_and_reader_new_carry_the_version_1_maxalign() {
    assert_eq!(Writer::new(Endianness::Little).max_align(), 8);
    assert_eq!(Reader::new(&[], Endianness::Little).max_align(), 8);
    assert_eq!(Writer::with_max_align(Endianness::Little, 4).max_align(), 4);
    assert_eq!(
        Reader::with_max_align(&[], Endianness::Little, 4).max_align(),
        4
    );
}

#[test]
fn xtypes_7_4_2_an_xcdr2_geo_pose_has_lat_deg_right_behind_its_dheader() {
    let pose = GeoPose {
        lat_deg: 52.5,
        lon_deg: 13.25,
        ..Default::default()
    };
    let mut w = Writer::with_max_align(Endianness::Little, 4);
    pose.encode(&mut w).unwrap();
    let xcdr2 = w.finish();
    assert_eq!(xcdr2[4..12], 52.5_f64.to_le_bytes(), "lat_deg at 4");
    assert_eq!(xcdr2[12..20], 13.25_f64.to_le_bytes(), "lon_deg at 12");
    let mut r = Reader::with_max_align(&xcdr2, Endianness::Little, 4);
    assert_eq!(GeoPose::decode(&mut r).unwrap(), pose);

    // A writer with the version 1 MAXALIGN pads the double to 8 inside
    // the same DHEADER: four octets no version 2 reader expects.
    let mut w = Writer::new(Endianness::Little);
    pose.encode(&mut w).unwrap();
    let padded = w.finish();
    assert_eq!(padded[4..8], [0, 0, 0, 0]);
    assert_eq!(padded[8..16], 52.5_f64.to_le_bytes());
}

#[test]
fn xtypes_7_4_3_1_the_ros_side_xcdr1_form_aligns_the_doubles_to_eight() {
    let message = PoseStamped {
        header: Header {
            stamp: Time { sec: 1, nanosec: 2 },
            frame_id: "map1".into(),
        },
        pose: Pose {
            position: Point {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            orientation: Quaternion {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        },
    };
    let mut w = Writer::with_max_align(Endianness::Little, 8);
    message.encode_xcdr1(&mut w).unwrap();
    let bytes = w.finish();
    // sec, nanosec, frame_id "map1" (9 octets from 8), padding to 24.
    assert_eq!(bytes.len(), 80);
    assert_eq!(bytes[8..17], [5, 0, 0, 0, b'm', b'a', b'p', b'1', 0]);
    assert_eq!(bytes[17..24], [0; 7]);
    assert_eq!(bytes[24..32], 1.0_f64.to_le_bytes(), "position.x at 24");
    assert_eq!(bytes[72..80], 1.0_f64.to_le_bytes(), "orientation.w last");

    let mut r = Reader::with_max_align(&bytes, Endianness::Little, 8);
    assert_eq!(PoseStamped::decode_xcdr1(&mut r).unwrap(), message);
}
