// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! SpatialDDS 1.6 profile for ZeroDDS.
//!
//! Crate `zerodds-spatial-dds`. Safety classification: **STANDARD**.
//!
//! The SpatialDDS types are **not hand-written** — they are the real
//! [OpenArCloud SpatialDDS](https://github.com/OpenArCloud/SpatialDDS-spec) IDL
//! (v1.6 `types.idl` + `core.idl` + `discovery.idl` + `anchors.idl` + the
//! `sensing` modules, vendored under `idl/`) run through ZeroDDS' own IDL→Rust
//! codegen at build time, so they carry ZeroDDS' XCDR encode/decode. This proves
//! ZeroDDS ingests the published spec IDL verbatim — every published SpatialDDS
//! 1.6 module is generated, none is stubbed.
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
//! These are ordinary ZeroDDS types (they `impl CdrEncode`/`CdrDecode`), so
//! they ride ZeroDDS' existing type-agnostic bridges — MQTT (`zerodds-mqtt-
//! bridge`), WebSocket (`zerodds-websocket-bridge`) and MCAP recording
//! (`zerodds-recorder`) — with no SpatialDDS-specific code: the profile is a
//! type layer, and the carriers are generic. The one carrier that needs
//! SpatialDDS-aware work is the **ROS-2 semantic mapping**
//! (sensor_msgs/vision_msgs/tf2/geometry_msgs/geographic_msgs ↔ SpatialDDS),
//! delivered as the `zerodds-spatial-ros2` crate.

#![forbid(unsafe_code)]
// The generated SpatialDDS types keep IDL-native identifiers (SCREAMING_CASE
// enum labels) and cdr_only pulls a wide `use`; silence those for the codegen.
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
