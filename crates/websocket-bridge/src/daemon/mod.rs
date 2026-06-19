// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-ws-bridged` daemon implementation.
//!
//! Spec: `docs/specs/zerodds-ws-bridge-1.0.md`.
//!
//! Conformance levels L1-L4 are fully served here:
//!
//! * **L1 — Wire**: HTTP upgrade handshake per RFC 6455 §4 via the
//!   `crate::handshake` module; frame codec via `crate::codec`.
//! * **L2 — DDS**: each daemon process starts a `DcpsRuntime` with
//!   `RuntimeConfig::default()` on the configured domain id.
//! * **L3 — Bridging**: one reader+writer per `topics[]` entry; WS
//!   frames `op:publish` write into the writer, DDS samples are
//!   pushed as `op:notify` to all subscribed WS clients.
//! * **L4 — Config**: YAML-subset parser without external deps (see
//!   [`config`]).
//!
//! L5 (auth/TLS) and L6 (multi-tenant) are laid out as stubs — the
//! wire path is L1-complete, the L5/L6 wireup is added in a follow-up
//! sprint.
//!
//! # Architecture
//!
//! Sync threads, blocking I/O. One reader thread + one writer thread
//! per WS connection (DDS pump → connection queue). No tokio
//! dependency — the workspace is sync.
//!
//! ```text
//!                 +--------------------+
//!                 |  TCP listener      |
//!                 +---------+----------+
//!                           |
//!                           v
//!     +----------------- Connection -----------------+
//!     |  read-thread  ←  WS frame decoder            |
//!     |       │                                      |
//!     |       └─→ DDS writer (write_user_sample)     |
//!     |                                              |
//!     |  write-thread  ←  DDS reader rx-channel      |
//!     |       │                                      |
//!     |       └─→ WS frame encoder ─→ TCP stream     |
//!     +----------------------------------------------+
//! ```

pub mod cli;
pub mod config;
pub mod qos_translation;
pub mod router;
pub mod runtime_common;
pub mod security;
pub mod server;
