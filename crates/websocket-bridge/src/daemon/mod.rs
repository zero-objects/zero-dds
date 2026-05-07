// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-ws-bridged` Daemon-Implementation.
//!
//! Spec: `docs/specs/zerodds-ws-bridge-1.0.md`.
//!
//! Conformance-Levels L1-L4 werden hier voll bedient:
//!
//! * **L1 — Wire**: HTTP-Upgrade-Handshake gemaess RFC 6455 §4 ueber das
//!   `crate::handshake`-Modul; Frame-Codec ueber `crate::codec`.
//! * **L2 — DDS**: jeder Daemon-Prozess startet einen `DcpsRuntime` mit
//!   `RuntimeConfig::default()` auf der konfigurierten Domain-ID.
//! * **L3 — Bridging**: pro `topics[]`-Eintrag ein Reader+Writer; WS-
//!   Frames `op:publish` schreiben in den Writer, DDS-Samples werden
//!   als `op:notify` an alle subskribierten WS-Clients gepusht.
//! * **L4 — Config**: YAML-Subset-Parser ohne externe deps (siehe
//!   [`config`]).
//!
//! L5 (Auth/TLS) und L6 (Multi-Tenant) sind als Stubs angelegt — der
//! Wire-Pfad ist L1-vollstaendig, L5/L6-Wireup wird in einem Folge-
//! Sprint nachgezogen.
//!
//! # Architektur
//!
//! Sync-Threads, blockierendes I/O. Pro WS-Connection ein Reader-
//! Thread + ein Writer-Thread (DDS-Pump → Connection-Queue). Keine
//! tokio-Abhaengigkeit — der Workspace ist sync.
//!
//! ```text
//!                 +--------------------+
//!                 |  TCP-Listener      |
//!                 +---------+----------+
//!                           |
//!                           v
//!     +----------------- Connection -----------------+
//!     |  read-thread  ←  WS-Frame-Decoder            |
//!     |       │                                      |
//!     |       └─→ DDS-Writer (write_user_sample)     |
//!     |                                              |
//!     |  write-thread  ←  DDS-Reader rx-channel      |
//!     |       │                                      |
//!     |       └─→ WS-Frame-Encoder ─→ TCP-Stream     |
//!     +----------------------------------------------+
//! ```

pub mod cli;
pub mod config;
pub mod qos_translation;
pub mod router;
pub mod runtime_common;
pub mod security;
pub mod server;
