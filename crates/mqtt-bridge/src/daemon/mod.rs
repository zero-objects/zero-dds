// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-mqtt-bridged` daemon implementation.
//!
//! Spec: `docs/specs/zerodds-mqtt-bridge-1.0.md`.
//!
//! Conformance L1-L4:
//!
//! * **L1 — wire**: MQTT-5 (OASIS 2019) over the existing codec modules
//!   (`crate::control_packets`, `crate::codec`, `crate::vbi`).
//! * **L2 — DDS**: `DcpsRuntime` on the domain ID from the config.
//! * **L3 — bridging**: per topic entry a DDS reader (out direction)
//!   plus a DDS writer (in direction); MQTT PUBLISH ↔ DDS sample.
//! * **L4 — config**: YAML subset like the WS daemon (own mini parser).
//!
//! L5/L6 are stubs.

pub mod cli;
pub mod client;
pub mod config;
pub mod qos_translation;
pub mod runtime_common;
pub mod security;
pub mod server;
pub mod yaml;
