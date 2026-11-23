// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-mqtt-bridged` daemon implementation.
//!
//! Spec: `docs/specs/zerodds-mqtt-bridge-1.0.md`.
//!
//! Conformance L1-L4:
//!
//! * **L1 — wire**: MQTT-5 (OASIS 2019) over the existing codec modules
//!   (`crate::control_packets`, `crate::codec`, `crate::vbi`); MQTT 3.1.1
//!   as the compatibility mode (`mqtt.protocol_version: 3.1.1`).
//! * **L2 — DDS**: `DcpsRuntime` on the domain ID from the config.
//! * **L3 — bridging**: per topic entry a DDS reader (out direction)
//!   plus a DDS writer (in direction); MQTT PUBLISH ↔ DDS sample.
//! * **L4 — config**: the shared layered loader `zerodds-config`.
//! * **L6 — tenants**: one participant and one broker connection per
//!   `tenants` entry, bound to the tenant by its client identifier.

pub mod acl;
pub mod auth;
pub mod cli;
pub mod client;
pub mod config;
pub mod history;
pub mod log;
pub mod metrics;
pub mod pump;
pub mod qos_translation;
pub mod replay;
pub mod runtime_common;
pub mod security;
pub mod server;
pub mod session;
pub mod status_watch;
pub mod topic_map;
pub mod trace;
mod ws_transport;
