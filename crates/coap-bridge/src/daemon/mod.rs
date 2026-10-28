// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-coap-bridged` daemon implementation.
//!
//! Spec: `docs/specs/zerodds-coap-bridge-1.0.md`.
//!
//! Conformance L1-L4:
//!
//! * **L1 — Wire**: CoAP (RFC 7252) UDP server on port 5683 over
//!   the existing `crate::codec` + `crate::message` + `crate::option`
//!   modules. Block1/Block2 (RFC 7959) are set up as hooks; on the
//!   daemon's L1 surface the DDS pump path only supports
//!   non-blocked samples ≤ `max_message_size`.
//! * **L2 — DDS**: `DcpsRuntime` on the domain ID from the config.
//! * **L3 — Bridging**: POST/PUT → DDS write; observe GET → notify
//!   stream on the reader sample.
//! * **L4 — Config**: the shared layered loader `zerodds-config`.
//! * **L5 — Auth**: bearer auth (§7.2) + topic ACL (§7.3); DTLS
//!   (§7.1, feature `dtls`, ADR 0011) and OSCORE (§7.2, feature
//!   `oscore`, ADR 0010) as opt-in features.
//!
//! L6 (multi-tenant) is a stub.

pub mod cli;
pub mod config;
pub mod oscore_gateway;
pub mod qos_translation;
pub mod runtime_common;
pub mod security;
pub mod server;
