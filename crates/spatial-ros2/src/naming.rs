// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! rmw-compatible DDS topic + type naming for the mapped ROS 2 messages.
//!
//! ROS 2 rmw maps a logical topic `/foo` to the DDS topic `rt/foo` and a
//! message `pkg/msg/Type` to the DDS type name `pkg::msg::dds_::Type_`
//! (the `rmw_fastrtps` / `rmw_cyclonedds` convention, REP-2008). A ZeroDDS
//! publisher that uses these two strings for a converted value is discovered
//! and matched by a native ROS 2 subscriber.

/// The DDS topic name for a ROS 2 topic (pub/sub), i.e. the `rt/` prefix form.
/// `logical` is the ROS name with or without a leading `/`.
#[must_use]
pub fn ros_topic(logical: &str) -> String {
    format!("rt/{}", logical.trim_start_matches('/'))
}

/// The DDS type name for a ROS 2 message `package/msg/Type`.
#[must_use]
pub fn ros_type_name(package: &str, type_name: &str) -> String {
    format!("{package}::msg::dds_::{type_name}_")
}

/// The ROS 2 message identity (package + type) that each mapped SpatialDDS
/// type converts to. `dds_type()` returns the wire type name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RosMessage {
    /// `geometry_msgs/PoseStamped` — from `FramedPose`.
    PoseStamped,
    /// `geometry_msgs/TransformStamped` — from `FrameTransform`.
    TransformStamped,
    /// `tf2_msgs/TFMessage` — from a set of `FrameTransform`.
    TfMessage,
    /// `geographic_msgs/GeoPoseStamped` — from `GeoPose`.
    GeoPoseStamped,
    /// `vision_msgs/Detection3D` — from `Detection3D`.
    Detection3D,
    /// `vision_msgs/Detection3DArray` — from `Detection3DSet`.
    Detection3DArray,
    /// `sensor_msgs/Imu` — from `ImuSample`.
    Imu,
}

impl RosMessage {
    /// `(package, type)` for this message.
    #[must_use]
    pub const fn package_and_type(self) -> (&'static str, &'static str) {
        match self {
            Self::PoseStamped => ("geometry_msgs", "PoseStamped"),
            Self::TransformStamped => ("geometry_msgs", "TransformStamped"),
            Self::TfMessage => ("tf2_msgs", "TFMessage"),
            Self::GeoPoseStamped => ("geographic_msgs", "GeoPoseStamped"),
            Self::Detection3D => ("vision_msgs", "Detection3D"),
            Self::Detection3DArray => ("vision_msgs", "Detection3DArray"),
            Self::Imu => ("sensor_msgs", "Imu"),
        }
    }

    /// The DDS wire type name (`pkg::msg::dds_::Type_`).
    #[must_use]
    pub fn dds_type(self) -> String {
        let (pkg, ty) = self.package_and_type();
        ros_type_name(pkg, ty)
    }
}
