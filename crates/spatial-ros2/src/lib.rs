// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! SpatialDDS <-> ROS 2 semantic mapping for ZeroDDS.
//!
//! Crate `zerodds-spatial-ros2`. Safety classification: **STANDARD**.
//!
//! This is the ROS 2 carrier for the [SpatialDDS profile](zerodds_spatial_dds):
//! it converts the SpatialDDS types to and from the ROS 2 common-interfaces
//! messages so a ZeroDDS node and a native ROS 2 node share a world model over
//! standard ROS topics. The other SpatialDDS carriers (MQTT / WebSocket / MCAP)
//! need no profile code because they are type-agnostic; ROS 2 is the one that
//! needs a *semantic* map, because ROS and SpatialDDS name and shape the same
//! concepts differently (`geometry_msgs`/`sensor_msgs`/`vision_msgs`/`tf2_msgs`/
//! `geographic_msgs`).
//!
//! Both sides are ZeroDDS-generated: the ROS 2 message types come from the
//! vendored [`idl/ros_msgs.idl`] run through ZeroDDS' own IDL->Rust codegen (the
//! same pipeline that generates the SpatialDDS types), so a converted value
//! carries the ROS 2 XCDR wire layout directly.
//!
//! ## Mappings ([`convert`])
//!
//! | SpatialDDS | ROS 2 |
//! |---|---|
//! | `spatial::core::FramedPose` | `geometry_msgs/PoseStamped` |
//! | `spatial::core::FrameTransform` | `geometry_msgs/TransformStamped`, `tf2_msgs/TFMessage` |
//! | `spatial::core::GeoPose` | `geographic_msgs/GeoPoseStamped` |
//! | `spatial::semantics::Detection3D` | `vision_msgs/Detection3D` |
//! | `spatial::semantics::Detection3DSet` | `vision_msgs/Detection3DArray` |
//! | `spatial::vio::ImuSample` | `sensor_msgs/Imu` |
//!
//! Each conversion is bidirectional (`*_to_ros` / `*_from_ros`), with `From`
//! impls for the SpatialDDS->ROS direction. Field-fidelity notes (what each map
//! drops) are on [`convert`].
//!
//! ## Naming ([`naming`])
//!
//! [`naming::RosMessage`] gives the rmw-compatible DDS topic (`rt/…`) and type
//! name (`pkg::msg::dds_::Type_`) so a converted value publishes on the topic a
//! native ROS 2 subscriber is listening on.

#![forbid(unsafe_code)]
#![allow(
    missing_docs,
    clippy::all,
    clippy::pedantic,
    unused_imports,
    non_camel_case_types,
    non_snake_case
)]

// The generated ROS 2 message modules use crate-relative paths, so they are
// included at the crate root (exposing `geometry_msgs`, `sensor_msgs`, ...).
include!(concat!(env!("OUT_DIR"), "/ros_msgs.rs"));

pub mod convert;
pub mod naming;
