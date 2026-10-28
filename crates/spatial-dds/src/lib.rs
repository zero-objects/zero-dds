// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! SpatialDDS 1.6 profile for ZeroDDS.
//!
//! Crate `zerodds-spatial-dds`. Safety classification: **STANDARD**.
//!
//! The SpatialDDS types are **generated at build time**, not hand-written.
//! The input is the vendored `idl/spatial_core.idl`, derived from
//! [OpenArCloud SpatialDDS](https://github.com/OpenArCloud/SpatialDDS-spec) 1.6,
//! with modules combined and identifier corrections recorded in its header.
//! It is **not verbatim upstream IDL**: renamed members affect Complete
//! TypeObjects and can affect name-derived mutable member IDs. Compatibility
//! with an unmodified upstream-generated peer must not be assumed from the
//! profile name alone.
//!
//! The build script generates inherent encode/decode methods using
//! `zerodds-wire`. It selects little-endian XCDR2 framing per aggregate's
//! extensibility annotation: plain for final, delimited for appendable and
//! parameter-list for mutable. This is local type and codec generation, not
//! an independent cross-vendor interoperability certification.
//!
//! Modules carried:
//! * `spatial::common` — `FrameRef`, `MetaKV`, `AssetRef`, `TileKey`, the
//!   `Vec3`/`Mat*`/`QuaternionXYZW`/`BBox2D` aliases,
//!   `CovarianceType`/`CoordConvention`.
//! * `spatial::core` — `PoseSE3`, `Aabb3`, `FramedPose`, `GeoPose`,
//!   `FrameTransform`, the `CovMatrix` union; `builtin::Time`.
//! * `spatial::disco` — **spatial discovery**: `Announce`, `Capabilities`,
//!   `CoverageElement`, `TopicMeta`, `ServiceKind`, `Transform`, `Depart`.
//! * `spatial::anchors` — **geo-anchored registries** (the federated
//!   shared-world-model core): `AnchorEntry`, `AnchorSet`, `AnchorDelta`.
//! * `spatial::sensing::common` — the sensing base (StreamMeta/FrameHeader/ROI/
//!   Codec/ROIRequest/ROIReply/...).
//! * `spatial::sensing::vision` — vision (CamIntrinsics/Keypoint2D/Track2D/
//!   VisionDetections).
//! * `spatial::sensing::lidar` — LiDAR (`LidarMeta`/`LidarFrame`/
//!   `LidarDetectionSet`).
//! * `spatial::sensing::rad` — radar (`RadSensorMeta`/`RadDetectionSet`/
//!   `RadTensorFrame`).
//! * `spatial::semantics` — 2-D/3-D detections (`Detection2D(Set)`/
//!   `Detection3D(Set)`).
//! * `spatial::slam_frontend` — SLAM front-end (`Feature2D`/`KeyframeFeatures`/
//!   `MatchSet`/`Landmark`/`Tracklet`).
//! * `spatial::vio` — visual-inertial odometry (`ImuInfo`/`ImuSample`/
//!   `SensorFusionSample`/`FusedState`).
//! * `spatial::argeo` — geo-referenced pose graph (`NodeGeo`).
//!
//! ## Carriers
//!
//! This crate supplies a type layer, not a running transport. Its generated
//! inherent codecs do not establish automatic integration with every bridge
//! or recorder. Those paths need their own adapter and end-to-end evidence.
//! Semantic conversion between selected ROS 2 messages and SpatialDDS types
//! is provided by the separate `zerodds-spatial-ros2` crate.

#![forbid(unsafe_code)]
// The generated SpatialDDS types keep IDL-native identifiers (SCREAMING_CASE
// enum labels) and the build script adds shared codec imports; silence those
// warnings for the codegen.
#![allow(
    missing_docs,
    clippy::all,
    clippy::pedantic,
    unused_imports,
    non_camel_case_types,
    non_snake_case
)]

// The generated modules (`spatial`, `builtin`) use crate-relative paths, so
// they are included at the crate root.
include!(concat!(env!("OUT_DIR"), "/spatial_core.rs"));

/// Builds a multi-operator SpatialDDS topic name: `sdds/<operator>/<kind>`.
/// SpatialDDS namespaces topics per operator so federated fleets share a world
/// model without colliding; `operator` is the operator/tenant id, `kind` the
/// SpatialDDS message kind (e.g. `"GeoPose"`, `"FramedPose"`).
#[must_use]
pub fn topic(operator: &str, kind: &str) -> String {
    format!("sdds/{operator}/{kind}")
}
