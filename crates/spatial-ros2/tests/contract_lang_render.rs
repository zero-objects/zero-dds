// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-spatial-ros2` with `zerodds-lang-render`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-lang-render` (`build.rs`, a build
//! dependency): `rust::file`, which writes the ROS 2 messages and their
//! codecs as the module `src/lib.rs` includes. `src/convert.rs` and the
//! users of the crate name the messages by the ROS 2 package path
//! (`geometry_msgs::msg::Point`), so the crate relies on the IDL modules
//! arriving as nested Rust modules under those names.
//!
//! Oracle: the ROS 2 interface conventions (`<package>::msg::<Type>`, the
//! IDL `rosidl` writes for a message) and the Rust reference ("Modules": a
//! nested `pub mod` is reached by its path), checked by the compiler
//! building this target.
#![allow(clippy::unwrap_used, clippy::float_cmp)]

#[test]
fn a_message_is_reached_by_its_ros_package_path() {
    let point = zerodds_spatial_ros2::geometry_msgs::msg::Point {
        x: 0.5,
        y: 0.0,
        z: -0.5,
    };
    let vector = zerodds_spatial_ros2::geometry_msgs::msg::Vector3 {
        x: point.x,
        y: point.y,
        z: point.z,
    };
    assert_eq!(vector.z, -0.5);
}
