// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ROS 2 RMW Bridge Layer.
//!
//! Crate `zerodds-ros2-rmw`. Safety classification: **STANDARD**.
//! Specs: `internal/standards/cache/ros2/rep-{2003,2004,2005,2007,2008,
//! 2009}.html` (ROS Enhancement Proposals).
//!
//! # Scope
//!
//! We implement the ROS-2 DDS mapping conventions as a pure-Rust
//! no_std+alloc library:
//!
//! * **Topic name mangling** (de-facto standard from `rmw_dds_common`):
//!   ROS 2 logical names are mapped to DDS topic names with a prefix code
//!   (`rt/`, `rq/`, `rr/`, `rs/`).
//! * **REP-2003 QoS settings** — map/sensor-data QoS profiles
//!   (Reliable+TransientLocal for maps; BestEffort+Volatile for
//!   sensor data).
//! * **REP-2004 package quality categories** — Q1-Q5 classification.
//! * **Standard RMW QoS profiles** — DEFAULT/SENSOR_DATA/PARAMETERS/
//!   SERVICES_DEFAULT/PARAMETER_EVENTS/SYSTEM_DEFAULT.
//!
//! # What is not covered
//!
//! * **REP-2005 Common Packages** — informational.
//! * **REP-2007 Type Adaptation** — a compile-time feature in C++/Python
//!   `rclcpp`/`rclpy`; not a wire layer.
//! * **REP-2008 Hardware Acceleration** — driver layer.
//! * **REP-2009 Type Negotiation** — compile-time feature.
//! * **`rmw` C API** — the Rust crate provides mapping constants and
//!   profiles; the actual `rmw_zerodds` C-FFI crate is its own
//!   sprint.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod action;
pub mod ffi_api;
pub mod json_log;
pub mod msg_to_idl;
pub mod qos_profiles;
pub mod quality;
pub mod rmw_qos_mapping;
pub mod service;
pub mod topic_mangling;
pub mod type_mapping;

pub use ffi_api::{
    RMW_IMPLEMENTATION_IDENTIFIER, RmwNode, RmwRet, check_rmw_identifier, map_to_rmw_ret,
};
pub use qos_profiles::{Durability, History, QosProfile, Reliability, profiles};
pub use quality::QualityLevel;
pub use rmw_qos_mapping::{
    RmwDurability, RmwHistory, RmwQosProfile, RmwReliability, dds_to_rmw, rmw_to_dds,
};
pub use topic_mangling::{
    NameMangleError, RosKind, demangle_topic_name, is_ros_topic, mangle_topic_name,
};
pub use type_mapping::{RosBuiltinType, RosNamespace, RosTypeRef};
