// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OPC-UA Pub/Sub **Part 14** (UADP) — native pure-Rust wire stack.
//!
//! Crate `zerodds-opcua-pubsub`. Safety classification: **STANDARD**.
//! Spec: OPC Foundation **Part 14 — PubSub** + **Part 6 — Mappings**
//! (binary encoding).
//!
//! # Scope and relationship to `zerodds-opcua-gateway`
//!
//! [`zerodds-opcua-gateway`](https://docs.rs/zerodds-opcua-gateway) is
//! the OMG **DDS-OPCUA 1.0** bridge — a pure type-system + AddressSpace
//! *mapping* library with no wire I/O. This crate adds the OPC Foundation
//! **Part 14** Pub/Sub *transport*: it serialises the gateway type model
//! (`NodeId`, `Variant`, `DataValue`, …) into the OPC-UA binary encoding
//! and frames it as UADP `NetworkMessage`s.
//!
//! The two specs are distinct families: DDS-OPCUA 1.0 bridges OPC-UA into
//! the DDS global data space; Part 14 is OPC-UA's own publish/subscribe
//! wire protocol. This crate is therefore an additive profile, not a
//! change to the gateway.
//!
//! # Layering (built incrementally)
//!
//! 1. [`binary`] — OPC-UA Part 6 binary encoding (the foundation; also
//!    usable by the Client/Server gateway).
//! 2. UADP `NetworkMessage` / `DataSetMessage` framing.
//! 3. PubSub configuration model + `DataSetWriter` / `DataSetReader`.
//! 4. UADP discovery (`DataSetMetaData`).
//! 5. PubSub security (`SecurityGroup` + Security Key Service).
//! 6. Transport carriers (UDP multicast, Ethernet, MQTT/AMQP).
//! 7. DataSet ↔ DDS-topic bridge + daemon.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Tests may panic/expect/unwrap freely; production code stays strict
// (the workspace denies these globally).
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used, clippy::panic))]

extern crate alloc;

pub mod binary;
pub mod bridge;
pub mod config;
pub mod daemon;
pub mod dynamic;
pub mod error;
pub mod infomodel;
#[cfg(feature = "json")]
pub mod json;
pub mod reader;
#[cfg(feature = "security")]
pub mod security;
pub mod transport;
pub mod uadp;
pub mod writer;

pub use binary::{UaDecode, UaEncode, UaReader, UaWriter, from_binary, to_binary};
pub use error::{DecodeError, EncodeError};
