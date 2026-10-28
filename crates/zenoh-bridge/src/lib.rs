// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-zenoh-bridge`. Safety classification: **STANDARD**.
//!
//! ZeroDDS ↔ Eclipse Zenoh bridge — a pure-Rust
//! mapping layer without a `zenoh` dep) plus the feature-gated `zenoh-runtime`
//! for the fully functional live bridge path, plus the feature-gated
//! `daemon` for the packaged `zerodds-zenoh-bridged` binary
//! (`crates/zenoh-bridge/src/bin/zerodds-zenoh-bridged.rs`).
//!
//! Connects a ZeroDDS `DcpsRuntime` with a Zenoh session, so that DDS topics
//! are mapped onto Zenoh key expressions and pumped in both directions —
//! both explicitly configured topics and every DDS topic the runtime
//! discovers at runtime via the DCPS built-in topics (`DiscoveredPublication`
//! / `DiscoveredSubscription`), the way `zenoh-plugin-dds` bridges topics.
//!
//! # Architecture
//!
//! ```text
//!  DDS Writer  ─────────►  ZeroDDS DataReader (raw bytes, UserSample::Alive)
//!     │                       │
//!     │                       ▼ forward pump (daemon/forward.rs)
//!     │                  + encapsulation header (mapping::encapsulation_header)
//!     │                       │
//!     │                       ▼ zenoh Publisher::put / PublicationCache (TRANSIENT_LOCAL)
//!     │                  Zenoh KeyExpr  <scope>/<partition>/<topic>
//!     │                       │
//!     │  ◄────────────────────┤  Zenoh Subscriber / FetchingSubscriber (TRANSIENT_LOCAL)
//!     │                       │
//!     │                  reverse pump (daemon/forward.rs)
//!     │                  − encapsulation header (mapping::decode_encapsulation_header)
//!     │                       ▼ DcpsRuntime::write_user_sample_borrowed
//!     ▼
//!  DDS Reader
//! ```
//!
//! # Feature gating
//!
//! - **std** — the pure-Rust topic-mapping logic (`mapping.rs`) only, no
//!   `zenoh` dep; `--no-default-features --features std` builds and tests it
//!   alone, so the mapping/QoS/wire-framing logic stays testable without an
//!   external dependency.
//! - **zenoh-runtime** — brings `zenoh = "1"` + `zenoh-ext` + `tokio`. With
//!   it, [`ZenohBridgeBuilder::build`] and [`ZenohBridge::add_topic`] pump
//!   samples end to end between a `DcpsRuntime` and a Zenoh `Session`.
//! - **daemon** *(default, with `std`)* — implies `zenoh-runtime`; adds the
//!   layered-config-driven `zerodds-zenoh-bridged` daemon (CLI, YAML config,
//!   SIGHUP reload, systemd `Type=notify`, Prometheus `/metrics`,
//!   auto-discovery of DDS topics). In `default` so a plain `cargo build
//!   --bin zerodds-zenoh-bridged` produces the binary, matching every other
//!   `*-bridged` daemon in this workspace.
//!
//! # QoS mapping (DDS ↔ Zenoh)
//!
//! Chosen to interoperate with `zenoh-plugin-dds` / `zenoh-bridge-dds`
//! (see `mapping` module docs for the exact source citations, version
//! `1.10.1`):
//!
//! | DDS QoS | Zenoh equivalent |
//! |---------|-------------------|
//! | `Reliability::Reliable` | Zenoh `Publisher`/`Subscriber` always declared `Reliability::Reliable`; `CongestionControl::Block` when `reliable_routes_blocking` (default on) |
//! | `Reliability::BestEffort` | `CongestionControl::Drop` |
//! | `Durability::TransientLocal` | DDS→Zenoh: `zenoh_ext::PublicationCache`; Zenoh→DDS: `zenoh_ext::FetchingSubscriber` querying `<ke>/@dds_pub_cache/*` |
//! | `Durability::Volatile` (default) | plain `Publisher`/`Subscriber`, no history replay |
//! | `Partition` (DDS) | one Zenoh key expression per partition: `<scope>/<partition>/<topic>` |
//!
//! # Spec reference
//!
//! There is no OMG spec for Zenoh bridges. This crate follows the
//! de-facto wire conventions of ZettaScale's `zenoh-plugin-dds` /
//! `zenoh-bridge-dds` (see <https://github.com/eclipse-zenoh/zenoh-plugin-dds>,
//! tag `1.10.1`), so `zerodds-zenoh-bridged` and `zenoh-bridge-dds`
//! interoperate on the same Zenoh network.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![forbid(unsafe_code)]

extern crate alloc;

mod mapping;

pub use mapping::{
    DEFAULT_SCOPE, DecodedHeader, Extensibility, InvalidKeyExprSegment, TopicMap, TopicMapEntry,
    ZenohCongestion, ZenohReliability, congestion_control, dds_reliability_to_zenoh,
    decode_encapsulation_header, encapsulation_header, glob_match, is_transient_local,
    key_expr_for_topic, representation_id, topic_allowed, validate_key_expr_segment,
};

#[cfg(feature = "zenoh-runtime")]
mod runtime;

#[cfg(feature = "zenoh-runtime")]
pub use runtime::{BridgeError, ZenohBridge, ZenohBridgeBuilder};

#[cfg(feature = "daemon")]
pub mod daemon;
