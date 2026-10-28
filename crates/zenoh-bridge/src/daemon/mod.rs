// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-zenoh-bridged` daemon implementation.
//!
//! * **CLI + config**: `cli.rs` / `config.rs`, the shared layered loader
//!   `zerodds-config`.
//! * **Runtime**: `server.rs` — builds the Zenoh session + `DcpsRuntime`,
//!   wires the configured topics through `crate::runtime::ZenohBridge`, and
//!   runs the auto-discovery loop.
//! * **Observability**: `runtime_common.rs` — Prometheus `/metrics` +
//!   `/catalog` + `/healthz`, OTLP, signal handling.

pub mod cli;
pub mod config;
pub mod runtime_common;
pub mod server;
