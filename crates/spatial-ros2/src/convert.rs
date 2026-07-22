// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Semantic conversions between the SpatialDDS profile types and the ROS 2
//! common-interfaces messages.
//!
//! Each conversion is a *semantic* map, not a byte reinterpretation: a
//! SpatialDDS `FramedPose` becomes a `geometry_msgs/PoseStamped`, a
//! `Detection3D` becomes a `vision_msgs/Detection3D`, and so on. Both the
//! SpatialDDS side and the ROS side are ZeroDDS-generated types carrying
//! ZeroDDS' XCDR encode/decode, so a converted value is directly publishable
//! on the matching ROS 2 topic (see [`crate::naming`]).
//!
//! ## Field fidelity
//!
//! ROS messages carry fewer fields than the SpatialDDS types in several places;
//! those SpatialDDS-only fields have no ROS destination and are dropped on the
//! `*_to_ros` direction (and defaulted on the way back). Per conversion:
//! * `FramedPose` <-> `PoseStamped` — the `CovMatrix` is dropped (PoseStamped
//!   has no covariance); the covariance-bearing path is `Detection3D`.
//! * `FrameTransform` <-> `TransformStamped` — `transform_id` + `CovMatrix`
//!   dropped.
//! * `GeoPose` <-> `GeoPoseStamped` — `frame_kind` + `CovMatrix` dropped.
//! * `Detection3D` <-> `vision_msgs/Detection3D` — `track_id`, `attributes`,
//!   `visibility`, point-count evidence, `tile_key`, `source_id` dropped; the
//!   3x3 position + 3x3 rotation covariances map into the block-diagonal ROS
//!   6x6 pose covariance.
//! * `ImuSample` <-> `sensor_msgs/Imu` — `source_id`/`seq` dropped; orientation
//!   is left unknown (ROS convention: `orientation_covariance[0] = -1`).

use crate::builtin_interfaces::msg as bi;
use crate::geographic_msgs::msg as geog;
use crate::geometry_msgs::msg as geo;
use crate::sensor_msgs::msg as sensor;
use crate::tf2_msgs::msg as tf2;
use crate::vision_msgs::msg as vision;
use zerodds_spatial_dds::builtin;
use zerodds_spatial_dds::spatial::common::FrameRef;
use zerodds_spatial_dds::spatial::core::{FrameTransform, FramedPose, GeoPose, PoseSE3};
use zerodds_spatial_dds::spatial::semantics::{Detection3D, Detection3DSet};
use zerodds_spatial_dds::spatial::vio::ImuSample;

// ---- primitives -----------------------------------------------------------

fn time_to_ros(t: &builtin::Time) -> bi::Time {
    bi::Time {
        sec: t.sec,
        nanosec: t.nanosec,
    }
}

fn time_from_ros(t: &bi::Time) -> builtin::Time {
    builtin::Time {
        sec: t.sec,
        nanosec: t.nanosec,
    }
}

fn header(stamp: &builtin::Time, frame_id: &str) -> crate::std_msgs::msg::Header {
    crate::std_msgs::msg::Header {
        stamp: time_to_ros(stamp),
        frame_id: frame_id.to_string(),
    }
}

/// The ROS `frame_id` for a SpatialDDS `FrameRef`: the fully-qualified name if
/// present, else the uuid.
fn frame_id_of(fr: &FrameRef) -> &str {
    if fr.fqn.is_empty() { &fr.uuid } else { &fr.fqn }
}

fn frame_ref_from_id(frame_id: &str) -> FrameRef {
    FrameRef {
        fqn: frame_id.to_string(),
        ..Default::default()
    }
}

fn point_from_vec3(v: &[f64; 3]) -> geo::Point {
    geo::Point {
        x: v[0],
        y: v[1],
        z: v[2],
    }
}

fn vec3_from_point(p: &geo::Point) -> [f64; 3] {
    [p.x, p.y, p.z]
}

fn vector3_from_vec3(v: &[f64; 3]) -> geo::Vector3 {
    geo::Vector3 {
        x: v[0],
        y: v[1],
        z: v[2],
    }
}

fn vec3_from_vector3(v: &geo::Vector3) -> [f64; 3] {
    [v.x, v.y, v.z]
}

fn quat_to_ros(q: &[f64; 4]) -> geo::Quaternion {
    geo::Quaternion {
        x: q[0],
        y: q[1],
        z: q[2],
        w: q[3],
    }
}

fn quat_from_ros(q: &geo::Quaternion) -> [f64; 4] {
    [q.x, q.y, q.z, q.w]
}

fn pose_se3_to_ros(p: &PoseSE3) -> geo::Pose {
    geo::Pose {
        position: point_from_vec3(&p.t),
        orientation: quat_to_ros(&p.q),
    }
}

fn pose_se3_from_ros(p: &geo::Pose) -> PoseSE3 {
    PoseSE3 {
        t: vec3_from_point(&p.position),
        q: quat_from_ros(&p.orientation),
    }
}

/// Block-diagonal 6x6 ROS pose covariance from the SpatialDDS 3x3 position and
/// 3x3 rotation covariances (row-major; off-diagonal blocks zero).
fn cov6_from(pos: &[f64; 9], rot: &[f64; 9]) -> [f64; 36] {
    let mut c = [0.0f64; 36];
    for r in 0..3 {
        for col in 0..3 {
            c[r * 6 + col] = pos[r * 3 + col];
            c[(r + 3) * 6 + (col + 3)] = rot[r * 3 + col];
        }
    }
    c
}

/// Inverse of [`cov6_from`]: the two 3x3 diagonal blocks of a ROS 6x6.
fn cov_blocks_from6(c: &[f64; 36]) -> ([f64; 9], [f64; 9]) {
    let mut pos = [0.0f64; 9];
    let mut rot = [0.0f64; 9];
    for r in 0..3 {
        for col in 0..3 {
            pos[r * 3 + col] = c[r * 6 + col];
            rot[r * 3 + col] = c[(r + 3) * 6 + (col + 3)];
        }
    }
    (pos, rot)
}

// ---- FramedPose <-> geometry_msgs/PoseStamped -----------------------------

#[must_use]
pub fn framed_pose_to_ros(f: &FramedPose) -> geo::PoseStamped {
    geo::PoseStamped {
        header: header(&f.stamp, frame_id_of(&f.frame_ref)),
        pose: pose_se3_to_ros(&f.pose),
    }
}

#[must_use]
pub fn framed_pose_from_ros(m: &geo::PoseStamped) -> FramedPose {
    FramedPose {
        pose: pose_se3_from_ros(&m.pose),
        frame_ref: frame_ref_from_id(&m.header.frame_id),
        cov: Default::default(),
        stamp: time_from_ros(&m.header.stamp),
    }
}

// ---- FrameTransform <-> geometry_msgs/TransformStamped + tf2_msgs/TFMessage

#[must_use]
pub fn frame_transform_to_ros(t: &FrameTransform) -> geo::TransformStamped {
    geo::TransformStamped {
        header: header(&t.stamp, frame_id_of(&t.parent_ref)),
        child_frame_id: frame_id_of(&t.child_ref).to_string(),
        transform: geo::Transform {
            translation: vector3_from_vec3(&t.T_parent_child.t),
            rotation: quat_to_ros(&t.T_parent_child.q),
        },
    }
}

#[must_use]
pub fn frame_transform_from_ros(m: &geo::TransformStamped) -> FrameTransform {
    FrameTransform {
        transform_id: String::new(),
        parent_ref: frame_ref_from_id(&m.header.frame_id),
        child_ref: frame_ref_from_id(&m.child_frame_id),
        T_parent_child: PoseSE3 {
            t: vec3_from_vector3(&m.transform.translation),
            q: quat_from_ros(&m.transform.rotation),
        },
        stamp: time_from_ros(&m.header.stamp),
        cov: Default::default(),
    }
}

/// Bundles a set of frame transforms into a single `tf2_msgs/TFMessage`.
#[must_use]
pub fn tf_message_from(transforms: &[FrameTransform]) -> tf2::TFMessage {
    tf2::TFMessage {
        transforms: transforms.iter().map(frame_transform_to_ros).collect(),
    }
}

// ---- GeoPose <-> geographic_msgs/GeoPoseStamped ---------------------------

#[must_use]
pub fn geo_pose_to_ros(g: &GeoPose) -> geog::GeoPoseStamped {
    geog::GeoPoseStamped {
        header: header(&g.stamp, frame_id_of(&g.frame_ref)),
        pose: geog::GeoPose {
            position: geog::GeoPoint {
                latitude: g.lat_deg,
                longitude: g.lon_deg,
                altitude: g.alt_m,
            },
            orientation: quat_to_ros(&g.q),
        },
    }
}

#[must_use]
pub fn geo_pose_from_ros(m: &geog::GeoPoseStamped) -> GeoPose {
    GeoPose {
        lat_deg: m.pose.position.latitude,
        lon_deg: m.pose.position.longitude,
        alt_m: m.pose.position.altitude,
        q: quat_from_ros(&m.pose.orientation),
        frame_ref: frame_ref_from_id(&m.header.frame_id),
        stamp: time_from_ros(&m.header.stamp),
        ..Default::default()
    }
}

// ---- Detection3D <-> vision_msgs/Detection3D ------------------------------

#[must_use]
pub fn detection3d_to_ros(d: &Detection3D) -> vision::Detection3D {
    let center = geo::Pose {
        position: point_from_vec3(&d.center),
        orientation: quat_to_ros(&d.q),
    };
    let covariance = if d.has_covariance {
        cov6_from(&d.cov_pos, &d.cov_rot)
    } else {
        [0.0f64; 36]
    };
    vision::Detection3D {
        header: header(&d.stamp, frame_id_of(&d.frame_ref)),
        results: vec![vision::ObjectHypothesisWithPose {
            hypothesis: vision::ObjectHypothesis {
                class_id: d.class_id.clone(),
                score: f64::from(d.score),
            },
            pose: geo::PoseWithCovariance {
                pose: center.clone(),
                covariance,
            },
        }],
        bbox: vision::BoundingBox3D {
            center,
            size: vector3_from_vec3(&d.size),
        },
        id: d.det_id.clone(),
    }
}

#[must_use]
pub fn detection3d_from_ros(m: &vision::Detection3D) -> Detection3D {
    let (class_id, score) = m
        .results
        .first()
        .map(|r| (r.hypothesis.class_id.clone(), r.hypothesis.score as f32))
        .unwrap_or_default();
    let has_covariance = m
        .results
        .first()
        .is_some_and(|r| r.pose.covariance.iter().any(|&x| x != 0.0));
    let (cov_pos, cov_rot) = m
        .results
        .first()
        .map(|r| cov_blocks_from6(&r.pose.covariance))
        .unwrap_or(([0.0; 9], [0.0; 9]));
    Detection3D {
        det_id: m.id.clone(),
        frame_ref: frame_ref_from_id(&m.header.frame_id),
        class_id,
        score,
        center: vec3_from_point(&m.bbox.center.position),
        size: vec3_from_vector3(&m.bbox.size),
        q: quat_from_ros(&m.bbox.center.orientation),
        has_covariance,
        cov_pos,
        cov_rot,
        stamp: time_from_ros(&m.header.stamp),
        ..Default::default()
    }
}

/// A `Detection3DSet` maps to a `vision_msgs/Detection3DArray`.
#[must_use]
pub fn detection3d_set_to_ros(s: &Detection3DSet) -> vision::Detection3DArray {
    vision::Detection3DArray {
        header: header(&s.stamp, frame_id_of(&s.frame_ref)),
        detections: s.dets.iter().map(detection3d_to_ros).collect(),
    }
}

#[must_use]
pub fn detection3d_set_from_ros(m: &vision::Detection3DArray) -> Detection3DSet {
    Detection3DSet {
        frame_ref: frame_ref_from_id(&m.header.frame_id),
        dets: m.detections.iter().map(detection3d_from_ros).collect(),
        stamp: time_from_ros(&m.header.stamp),
        ..Default::default()
    }
}

// ---- ImuSample <-> sensor_msgs/Imu ----------------------------------------

#[must_use]
pub fn imu_to_ros(s: &ImuSample) -> sensor::Imu {
    // ROS convention: orientation_covariance[0] = -1 => "no orientation
    // estimate provided". SpatialDDS ImuSample carries only accel + gyro.
    let mut orientation_covariance = [0.0f64; 9];
    orientation_covariance[0] = -1.0;
    sensor::Imu {
        header: header(&s.stamp, &s.imu_id),
        orientation: geo::Quaternion {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        },
        orientation_covariance,
        angular_velocity: vector3_from_vec3(&s.gyro),
        angular_velocity_covariance: [0.0; 9],
        linear_acceleration: vector3_from_vec3(&s.accel),
        linear_acceleration_covariance: [0.0; 9],
    }
}

#[must_use]
pub fn imu_from_ros(m: &sensor::Imu) -> ImuSample {
    ImuSample {
        imu_id: m.header.frame_id.clone(),
        accel: vec3_from_vector3(&m.linear_acceleration),
        gyro: vec3_from_vector3(&m.angular_velocity),
        stamp: time_from_ros(&m.header.stamp),
        ..Default::default()
    }
}

// ---- ergonomic From impls (SpatialDDS -> ROS, the lossless-target dir) -----

impl From<&FramedPose> for geo::PoseStamped {
    fn from(f: &FramedPose) -> Self {
        framed_pose_to_ros(f)
    }
}
impl From<&FrameTransform> for geo::TransformStamped {
    fn from(t: &FrameTransform) -> Self {
        frame_transform_to_ros(t)
    }
}
impl From<&GeoPose> for geog::GeoPoseStamped {
    fn from(g: &GeoPose) -> Self {
        geo_pose_to_ros(g)
    }
}
impl From<&Detection3D> for vision::Detection3D {
    fn from(d: &Detection3D) -> Self {
        detection3d_to_ros(d)
    }
}
impl From<&Detection3DSet> for vision::Detection3DArray {
    fn from(s: &Detection3DSet) -> Self {
        detection3d_set_to_ros(s)
    }
}
impl From<&ImuSample> for sensor::Imu {
    fn from(s: &ImuSample) -> Self {
        imu_to_ros(s)
    }
}
