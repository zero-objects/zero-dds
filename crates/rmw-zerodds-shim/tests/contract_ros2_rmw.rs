// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `rmw-zerodds-shim` with `zerodds-ros2-rmw`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-ros2-rmw`:
//! - `src/ros2_config.rs` and the diagnostics CLI:
//!   `topic_mangling::mangle_topic_name(name, RosKind::Topic)` for the DDS
//!   topic of a ROS topic, and its refusal of a name ROS does not allow;
//! - `src/lib.rs`: `ffi_api::RMW_IMPLEMENTATION_IDENTIFIER`, the answer of
//!   `rmw_get_implementation_identifier`, and `ffi_api::RmwNode` with its
//!   four fields as the node's identity;
//! - the diagnostics CLI: `qos_profiles::profiles::DEFAULT` and
//!   `SENSOR_DATA` with their `Reliability`.
//!
//! Oracle: the design article "Topic and Service name mapping to DDS"
//! (§"ROS Specific Namespace Prefix", the name constraints), rmw/rmw.h and
//! the name ament registers the implementation under, rmw/qos_profiles.h.

#![allow(clippy::unwrap_used, missing_docs)]

use zerodds_ros2_rmw::ffi_api::{RMW_IMPLEMENTATION_IDENTIFIER, RmwNode};
use zerodds_ros2_rmw::qos_profiles::{Durability, History, Reliability, profiles};
use zerodds_ros2_rmw::topic_mangling::{NameMangleError, RosKind, mangle_topic_name};

#[test]
fn ros_names_prefix_a_ros_topic_is_rt_and_its_name_without_the_leading_slash() {
    assert_eq!(
        mangle_topic_name("/chatter", RosKind::Topic).unwrap(),
        "rt/chatter"
    );
    assert_eq!(
        mangle_topic_name("/robot1/scan", RosKind::Topic).unwrap(),
        "rt/robot1/scan"
    );
    assert!(matches!(
        mangle_topic_name("/1scan", RosKind::Topic),
        Err(NameMangleError::InvalidLeadingChar('1'))
    ));
    assert_eq!(
        mangle_topic_name("", RosKind::Topic),
        Err(NameMangleError::EmptyName)
    );
}

#[test]
fn rmw_h_the_identifier_is_rmw_zerodds_cpp_and_a_node_keeps_its_four_fields() {
    assert_eq!(RMW_IMPLEMENTATION_IDENTIFIER, "rmw_zerodds_cpp");
    let node = RmwNode {
        implementation_identifier: RMW_IMPLEMENTATION_IDENTIFIER.into(),
        name: "talker".into(),
        namespace: "/".into(),
        domain_id: 7,
    };
    assert_eq!(node.implementation_identifier, "rmw_zerodds_cpp");
    assert_eq!(node.name, "talker");
    assert_eq!(node.namespace, "/");
    assert_eq!(node.domain_id, 7);
}

#[test]
fn rmw_qos_profiles_h_default_is_reliable_and_sensor_data_best_effort() {
    assert_eq!(profiles::DEFAULT.reliability, Reliability::Reliable);
    assert_eq!(profiles::DEFAULT.durability, Durability::Volatile);
    assert_eq!(profiles::DEFAULT.history, History::KeepLast(10));
    assert_eq!(profiles::SENSOR_DATA.reliability, Reliability::BestEffort);
    assert_eq!(profiles::SENSOR_DATA.history, History::KeepLast(5));
}
