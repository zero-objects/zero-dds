// SPDX-License-Identifier: Apache-2.0
//! SpatialDDS <-> ROS 2 conversions round-trip, and the generated ROS 2
//! message types round-trip on the XCDR wire.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    missing_docs
)]

use zerodds_cdr::{BufferReader, BufferWriter, CdrDecode, CdrEncode, Endianness};
use zerodds_spatial_dds::builtin::Time;
use zerodds_spatial_dds::spatial::common::FrameRef;
use zerodds_spatial_dds::spatial::core::{FrameTransform, FramedPose, GeoPose, PoseSE3};
use zerodds_spatial_dds::spatial::semantics::{Detection3D, Detection3DSet};
use zerodds_spatial_dds::spatial::vio::ImuSample;
use zerodds_spatial_ros2::convert::*;
use zerodds_spatial_ros2::naming::{RosMessage, ros_topic, ros_type_name};

/// XCDR2 wire round-trip.
fn wire<T: CdrEncode + CdrDecode + PartialEq + std::fmt::Debug>(v: &T) -> T {
    let mut w = BufferWriter::new(Endianness::Little).xcdr2();
    v.encode(&mut w).expect("encode");
    let bytes = w.into_bytes();
    let mut r = BufferReader::new(&bytes, Endianness::Little).xcdr2();
    T::decode(&mut r).expect("decode")
}

fn frame(fqn: &str) -> FrameRef {
    FrameRef {
        fqn: fqn.to_string(),
        ..Default::default()
    }
}

fn stamp() -> Time {
    Time {
        sec: 12,
        nanosec: 345,
    }
}

#[test]
fn framed_pose_maps_and_round_trips() {
    let f = FramedPose {
        pose: PoseSE3 {
            t: [1.0, 2.0, 3.0],
            q: [0.0, 0.0, 0.0, 1.0],
        },
        frame_ref: frame("map"),
        cov: Default::default(),
        stamp: stamp(),
    };
    let ros = framed_pose_to_ros(&f);
    assert_eq!(ros.header.frame_id, "map");
    assert_eq!(ros.pose.position.x, 1.0);
    assert_eq!(ros.header.stamp.sec, 12);
    // ROS message survives the XCDR wire.
    assert_eq!(wire(&ros), ros);
    // Semantic round-trip is lossless for the mapped fields.
    assert_eq!(framed_pose_from_ros(&ros), f);
}

#[test]
fn frame_transform_and_tf_message() {
    let t = FrameTransform {
        transform_id: String::new(),
        parent_ref: frame("odom"),
        child_ref: frame("base_link"),
        T_parent_child: PoseSE3 {
            t: [0.5, 0.0, 0.0],
            q: [0.0, 0.0, 0.0, 1.0],
        },
        stamp: stamp(),
        cov: Default::default(),
    };
    let ros = frame_transform_to_ros(&t);
    assert_eq!(ros.header.frame_id, "odom");
    assert_eq!(ros.child_frame_id, "base_link");
    assert_eq!(ros.transform.translation.x, 0.5);
    assert_eq!(wire(&ros), ros);
    assert_eq!(frame_transform_from_ros(&ros), t);

    let tf = tf_message_from(std::slice::from_ref(&t));
    assert_eq!(tf.transforms.len(), 1);
    assert_eq!(wire(&tf), tf);
}

#[test]
fn geo_pose_maps_and_round_trips() {
    let g = GeoPose {
        lat_deg: 52.52,
        lon_deg: 13.405,
        alt_m: 34.0,
        q: [0.0, 0.0, 0.0, 1.0],
        frame_ref: frame("earth"),
        stamp: stamp(),
        ..Default::default()
    };
    let ros = geo_pose_to_ros(&g);
    assert_eq!(ros.pose.position.latitude, 52.52);
    assert_eq!(ros.header.frame_id, "earth");
    assert_eq!(wire(&ros), ros);
    assert_eq!(geo_pose_from_ros(&ros), g);
}

#[test]
fn detection3d_maps_with_covariance() {
    let d = Detection3D {
        det_id: "obj-1".into(),
        frame_ref: frame("map"),
        class_id: "car".into(),
        score: 0.5,
        center: [1.0, 2.0, 3.0],
        size: [4.0, 2.0, 1.0],
        q: [0.0, 0.0, 0.0, 1.0],
        has_covariance: true,
        cov_pos: [1.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0],
        cov_rot: [4.0, 0.0, 0.0, 0.0, 5.0, 0.0, 0.0, 0.0, 6.0],
        stamp: stamp(),
        ..Default::default()
    };
    let ros = detection3d_to_ros(&d);
    assert_eq!(ros.id, "obj-1");
    assert_eq!(ros.results[0].hypothesis.class_id, "car");
    assert_eq!(ros.results[0].hypothesis.score, 0.5);
    assert_eq!(ros.bbox.size.x, 4.0);
    // Block-diagonal 6x6: pos block top-left, rot block bottom-right.
    assert_eq!(ros.results[0].pose.covariance[0], 1.0); // cov_pos[0,0]
    assert_eq!(ros.results[0].pose.covariance[6 * 3 + 3], 4.0); // cov_rot[0,0]
    assert_eq!(wire(&ros), ros);
    assert_eq!(detection3d_from_ros(&ros), d);
}

#[test]
fn detection3d_set_maps_to_array() {
    let d = Detection3D {
        det_id: "o".into(),
        frame_ref: frame("map"),
        class_id: "ped".into(),
        score: 0.25,
        center: [0.0, 0.0, 0.0],
        size: [0.5, 0.5, 1.8],
        q: [0.0, 0.0, 0.0, 1.0],
        stamp: stamp(),
        ..Default::default()
    };
    let s = Detection3DSet {
        frame_ref: frame("map"),
        dets: vec![d],
        stamp: stamp(),
        ..Default::default()
    };
    let ros = detection3d_set_to_ros(&s);
    assert_eq!(ros.detections.len(), 1);
    assert_eq!(ros.header.frame_id, "map");
    assert_eq!(wire(&ros), ros);
    assert_eq!(detection3d_set_from_ros(&ros), s);
}

#[test]
fn imu_maps_and_round_trips() {
    let s = ImuSample {
        imu_id: "imu0".into(),
        accel: [0.0, 0.0, 9.81],
        gyro: [0.1, 0.2, 0.3],
        stamp: stamp(),
        ..Default::default()
    };
    let ros = imu_to_ros(&s);
    assert_eq!(ros.header.frame_id, "imu0");
    assert_eq!(ros.linear_acceleration.z, 9.81);
    assert_eq!(ros.angular_velocity.x, 0.1);
    // ROS convention: no orientation estimate.
    assert_eq!(ros.orientation_covariance[0], -1.0);
    assert_eq!(wire(&ros), ros);
    assert_eq!(imu_from_ros(&ros), s);
}

#[test]
fn rmw_naming() {
    assert_eq!(ros_topic("/tf"), "rt/tf");
    assert_eq!(ros_topic("pose"), "rt/pose");
    assert_eq!(
        ros_type_name("geometry_msgs", "PoseStamped"),
        "geometry_msgs::msg::dds_::PoseStamped_"
    );
    assert_eq!(
        RosMessage::TfMessage.dds_type(),
        "tf2_msgs::msg::dds_::TFMessage_"
    );
    assert_eq!(
        RosMessage::Detection3DArray.dds_type(),
        "vision_msgs::msg::dds_::Detection3DArray_"
    );
}
