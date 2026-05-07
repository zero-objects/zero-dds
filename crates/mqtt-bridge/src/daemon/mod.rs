// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-mqtt-bridged` Daemon-Implementation.
//!
//! Spec: `docs/specs/zerodds-mqtt-bridge-1.0.md`.
//!
//! Conformance L1-L4:
//!
//! * **L1 — Wire**: MQTT-5 (OASIS 2019) ueber bestehende Codec-Module
//!   (`crate::control_packets`, `crate::codec`, `crate::vbi`).
//! * **L2 — DDS**: `DcpsRuntime` auf Domain-ID aus Config.
//! * **L3 — Bridging**: pro Topic-Eintrag DDS-Reader (out-Direction)
//!   plus DDS-Writer (in-Direction); MQTT-PUBLISH ↔ DDS-Sample.
//! * **L4 — Config**: YAML-Subset wie WS-Daemon (eigener Mini-Parser).
//!
//! L5/L6 sind Stubs.

pub mod cli;
pub mod client;
pub mod config;
pub mod qos_translation;
pub mod runtime_common;
pub mod security;
pub mod server;
pub mod yaml;
