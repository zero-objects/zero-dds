// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ROS 2 RMW Bridge Layer.
//!
//! Crate `zerodds-ros2-rmw`. Safety classification: **STANDARD**.
//! Specs: `docs/standards/cache/ros2/rep-{2003,2004,2005,2007,2008,
//! 2009}.html` (ROS Enhancement Proposals).
//!
//! # Scope
//!
//! Wir implementieren die ROS-2-DDS-Mapping-Conventions als pure-Rust
//! no_std+alloc Library:
//!
//! * **Topic-Name-Mangling** (de-facto-Standard aus `rmw_dds_common`):
//!   ROS 2 logical names werden mit Prefix-Code (`rt/`, `rq/`, `rr/`,
//!   `rs/`) auf DDS-Topic-Namen gemapped.
//! * **REP-2003 QoS Settings** — Map-/Sensor-Data-QoS-Profiles
//!   (Reliable+TransientLocal fuer Maps; BestEffort+Volatile fuer
//!   Sensor-Data).
//! * **REP-2004 Package Quality Categories** — Q1-Q5-Klassifikation.
//! * **Standard RMW QoS Profiles** — DEFAULT/SENSOR_DATA/PARAMETERS/
//!   SERVICES_DEFAULT/PARAMETER_EVENTS/SYSTEM_DEFAULT.
//!
//! # Was nicht abgedeckt ist
//!
//! * **REP-2005 Common Packages** — Informational.
//! * **REP-2007 Type Adaptation** — Compile-Time-Feature in C++/Python
//!   `rclcpp`/`rclpy`; nicht Wire-Layer.
//! * **REP-2008 Hardware Acceleration** — Driver-Layer.
//! * **REP-2009 Type Negotiation** — Compile-Time-Feature.
//! * **`rmw`-C-API** — die Rust-Crate liefert Mapping-Konstanten und
//!   Profile; das eigentliche `rmw_zerodds`-C-FFI-Crate ist eigener
//!   Sprint.

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
