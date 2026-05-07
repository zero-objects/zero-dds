// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-zenoh-bridge`. Safety classification: **STANDARD**.
//!
//! ZeroDDS ↔ Eclipse-Zenoh Bridge — `no_std + alloc` (pure-Rust
//! Mapping-Layer ohne `zenoh`-Dep) plus Feature-gated `zenoh-runtime`
//! fuer den voll-funktionalen Live-Bridge-Pfad.
//!
//! Verbindet einen ZeroDDS-DomainParticipant mit einer Zenoh-Session,
//! sodass DDS-Topics auf Zenoh-Key-Expressions gemappt werden und in
//! beide Richtungen gepumpt werden.
//!
//! # Architektur
//!
//! ```text
//!  DDS-Topic  ────────►  ZeroDDS DataReader
//!     │                       │
//!     │                       ▼ on_data
//!     │                  Bridge::forward
//!     │                       │
//!     │                       ▼ zenoh::publisher::put
//!     │                  Zenoh KeyExpr
//!     │                       │
//!     │  ◄────────────────────┤  Zenoh Subscriber
//!     │                       │
//!     │                  Bridge::reverse
//!     │                       │
//!     │                       ▼ DataWriter::write
//!     ▼
//!  DDS-Topic
//! ```
//!
//! # Feature-Gating
//!
//! - **default** — Skeleton-Crate ohne `zenoh`-Dep. Topic-Mapping-
//!   Logik + Tests laufen pure-Rust, sodass das Workspace-CI ohne
//!   externe Dependencies bleibt.
//! - **zenoh-runtime** — bringt `zenoh = "1"` + `tokio`. Damit ist
//!   `ZenohBridge::start` voll funktional.
//!
//! # QoS-Mapping (DDS → Zenoh)
//!
//! | DDS-QoS | Zenoh-Aequivalent |
//! |---------|-------------------|
//! | `Reliability::Reliable` | `Reliability::Reliable` (Zenoh) |
//! | `Reliability::BestEffort` | `Reliability::BestEffort` |
//! | `Durability::TransientLocal` | `CongestionControl::Block` + `Priority::DataHigh` |
//! | `Durability::Volatile` (Default) | `CongestionControl::Drop` |
//! | `History::KeepLast(n)` | (Zenoh hat kein History-Cache; n>1 ignoriert) |
//! | `Partition` | KeyExpr-Praefix (`<partition>/<topic>`) |
//!
//! # Spec-Referenz
//!
//! Es gibt keine OMG-Spec fuer Zenoh-Bridges. Diese Crate folgt dem
//! De-facto-Pattern von ZettaScale's `zenoh-bridge-dds` (siehe
//! <https://github.com/eclipse-zenoh/zenoh-plugin-dds>), aber als
//! eigenstaendige Rust-Library statt Plugin.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![forbid(unsafe_code)]

extern crate alloc;

mod mapping;

pub use mapping::{TopicMap, dds_qos_to_zenoh, key_expr_for_topic};

#[cfg(feature = "zenoh-runtime")]
mod runtime;

#[cfg(feature = "zenoh-runtime")]
pub use runtime::{BridgeError, ZenohBridge, ZenohBridgeBuilder};
