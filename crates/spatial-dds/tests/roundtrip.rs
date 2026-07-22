// SPDX-License-Identifier: Apache-2.0
//! The ZeroDDS-generated SpatialDDS types round-trip on the XCDR wire.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::field_reassign_with_default,
    missing_docs
)]

use zerodds_cdr::{BufferReader, BufferWriter, CdrDecode, CdrEncode, Endianness};
use zerodds_spatial_dds::spatial::anchors::{AnchorEntry, AnchorSet};
use zerodds_spatial_dds::spatial::argeo::NodeGeo;
use zerodds_spatial_dds::spatial::core::{FramedPose, GeoPose, PoseSE3};
use zerodds_spatial_dds::spatial::disco::{Announce, CoverageElement, ServiceKind};
use zerodds_spatial_dds::spatial::semantics::Detection3D;
use zerodds_spatial_dds::spatial::sensing::lidar::LidarMeta;
use zerodds_spatial_dds::spatial::sensing::rad::RadSensorMeta;
use zerodds_spatial_dds::spatial::sensing::vision::{Keypoint2D, VisionDetections};
use zerodds_spatial_dds::spatial::slam_frontend::Landmark;
use zerodds_spatial_dds::spatial::vio::ImuInfo;
use zerodds_spatial_dds::topic;

fn rt<T: CdrEncode + CdrDecode + PartialEq + std::fmt::Debug>(v: &T) -> T {
    let mut w = BufferWriter::new(Endianness::Little).xcdr2();
    v.encode(&mut w).expect("encode");
    let bytes = w.into_bytes();
    let mut r = BufferReader::new(&bytes, Endianness::Little).xcdr2();
    T::decode(&mut r).expect("decode")
}

#[test]
fn pose_se3_round_trips() {
    let p = PoseSE3 {
        t: [1.0, 2.0, 3.0],
        q: [0.0, 0.0, 0.0, 1.0],
    };
    assert_eq!(rt(&p), p);
}

#[test]
fn geopose_and_framedpose_round_trip() {
    let g = GeoPose {
        lat_deg: 52.52,
        lon_deg: 13.405,
        alt_m: 34.0,
        ..Default::default()
    };
    assert_eq!(rt(&g), g);
    assert_eq!(rt(&FramedPose::default()), FramedPose::default());
}

#[test]
fn discovery_announce_round_trips() {
    // Spatial discovery: a service Announce with a coverage element.
    let mut a = Announce::default();
    a.service_id = "vps-1".into();
    a.name = "relocalizer".into();
    a.kind = ServiceKind::VPS;
    let mut cov = CoverageElement::default();
    cov.r#type = "aabb".into();
    cov.global = true;
    a.coverage.push(cov);
    assert_eq!(rt(&a), a);
}

#[test]
fn anchor_set_round_trips() {
    // A geo-anchored registry: an AnchorSet holding an AnchorEntry (GeoPose).
    let mut entry = AnchorEntry::default();
    entry.anchor_id = "a-1".into();
    entry.geopose.lat_deg = 48.858;
    entry.geopose.lon_deg = 2.294;
    entry.confidence = 0.9;
    let mut set = AnchorSet::default();
    set.set_id = "paris".into();
    set.anchors.push(entry);
    assert_eq!(rt(&set), set);
}

#[test]
fn vision_detections_round_trips() {
    let mut d = VisionDetections::default();
    d.stream_id = "cam0".into();
    d.frame_seq = 7;
    d.keypoints.push(Keypoint2D {
        u: 1.0,
        v: 2.0,
        score: 0.8,
    });
    assert_eq!(rt(&d), d);
}

#[test]
fn lidar_meta_round_trips() {
    // sensing::lidar — nested `base: StreamMeta` + option-flag pattern.
    let mut m = LidarMeta::default();
    m.stream_id = "lidar-top".into();
    m.n_rings = 128;
    m.has_range_limits = true;
    m.max_range_m = 120.0;
    assert_eq!(rt(&m), m);
}

#[test]
fn rad_sensor_meta_round_trips() {
    // sensing::rad — radar sensor metadata.
    let mut m = RadSensorMeta::default();
    m.stream_id = "radar-front".into();
    m.has_velocity_limits = true;
    m.v_max_mps = 60.0;
    m.max_detections_per_frame = 512;
    assert_eq!(rt(&m), m);
}

#[test]
fn semantics_detection3d_round_trips() {
    // semantics — a 3-D detection with pose + class.
    let mut d = Detection3D::default();
    d.det_id = "obj-1".into();
    d.class_id = "car".into();
    d.score = 0.87;
    d.center = [1.0, 2.0, 3.0];
    d.size = [4.5, 1.8, 1.5];
    assert_eq!(rt(&d), d);
}

#[test]
fn slam_frontend_landmark_round_trips() {
    // slam_frontend — a map landmark carrying a binary descriptor.
    let mut lm = Landmark::default();
    lm.lm_id = "lm-42".into();
    lm.map_id = "map-a".into();
    lm.p = [10.0, -3.0, 0.5];
    lm.desc = vec![1, 2, 3, 4, 5, 6, 7, 8];
    lm.desc_type = "orb".into();
    assert_eq!(rt(&lm), lm);
}

#[test]
fn vio_imu_info_round_trips() {
    // vio — IMU noise characteristics (all f64).
    let mut info = ImuInfo::default();
    info.imu_id = "imu0".into();
    info.accel_noise_density = 0.0012;
    info.gyro_noise_density = 3.4e-5;
    info.accel_random_walk = 6.0e-4;
    assert_eq!(rt(&info), info);
}

#[test]
fn argeo_node_geo_round_trips() {
    // argeo — a pose-graph node with a GeoPose fix.
    let mut n = NodeGeo::default();
    n.map_id = "campus".into();
    n.node_id = "n-7".into();
    n.has_geopose = true;
    n.geopose.lat_deg = 51.31;
    n.geopose.lon_deg = 9.49;
    n.seq = 7;
    n.graph_epoch = 2;
    assert_eq!(rt(&n), n);
}

#[test]
fn topic_namespacing() {
    assert_eq!(topic("acme", "GeoPose"), "sdds/acme/GeoPose");
}
