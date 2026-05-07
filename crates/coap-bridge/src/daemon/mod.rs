// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-coap-bridged` Daemon-Implementation.
//!
//! Spec: `docs/specs/zerodds-coap-bridge-1.0.md`.
//!
//! Conformance L1-L4:
//!
//! * **L1 — Wire**: CoAP (RFC 7252) UDP-Server auf Port 5683 ueber
//!   die existierenden `crate::codec` + `crate::message` + `crate::option`
//!   Module. Block1/Block2 (RFC 7959) sind als Hooks angelegt; in der
//!   Daemon-L1-Surface unterstuetzt das DDS-Pump-Pfad nur
//!   ungeblockte Samples ≤ `max_message_size`.
//! * **L2 — DDS**: `DcpsRuntime` auf Domain-ID aus Config.
//! * **L3 — Bridging**: POST/PUT → DDS-Write; Observe-GET → Notify-
//!   Stream beim Reader-Sample.
//! * **L4 — Config**: YAML-Subset eigener Mini-Parser.
//!
//! L5 (DTLS/OSCORE) und L6 (Multi-Tenant) sind Stubs.

pub mod cli;
pub mod config;
pub mod qos_translation;
pub mod runtime_common;
pub mod security;
pub mod server;
pub mod yaml;
