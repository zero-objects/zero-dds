// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-zenoh-bridge`. Safety classification: **STANDARD**.
//!
//! ZeroDDS ↔ Eclipse Zenoh bridge — `no_std + alloc` (pure-Rust
//! mapping layer without a `zenoh` dep) plus the feature-gated `zenoh-runtime`
//! for the fully functional live bridge path.
//!
//! Connects a ZeroDDS DomainParticipant with a Zenoh session,
//! so that DDS topics are mapped onto Zenoh key expressions and pumped
//! in both directions.
//!
//! # Architecture
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
//! # Feature gating
//!
//! - **default** — skeleton crate without a `zenoh` dep. The topic-mapping
//!   logic + tests run pure-Rust, so the workspace CI stays without
//!   external dependencies.
//! - **zenoh-runtime** — brings `zenoh = "1"` + `tokio`. With it,
//!   `ZenohBridge::start` is fully functional.
//!
//! # QoS mapping (DDS → Zenoh)
//!
//! | DDS QoS | Zenoh equivalent |
//! |---------|-------------------|
//! | `Reliability::Reliable` | `Reliability::Reliable` (Zenoh) |
//! | `Reliability::BestEffort` | `Reliability::BestEffort` |
//! | `Durability::TransientLocal` | `CongestionControl::Block` + `Priority::DataHigh` |
//! | `Durability::Volatile` (default) | `CongestionControl::Drop` |
//! | `History::KeepLast(n)` | (Zenoh has no history cache; n>1 ignored) |
//! | `Partition` | KeyExpr prefix (`<partition>/<topic>`) |
//!
//! # Spec reference
//!
//! There is no OMG spec for Zenoh bridges. This crate follows the
//! de-facto pattern of ZettaScale's `zenoh-bridge-dds` (see
//! <https://github.com/eclipse-zenoh/zenoh-plugin-dds>), but as a
//! standalone Rust library instead of a plugin.

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
