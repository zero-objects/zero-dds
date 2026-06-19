// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Historical Data Reading — Spec §9.3.4.4.
//!
//! Conformance Point 4 ("Complete") from §2 Tab 2.1. Contains:
//!
//! * **HistoricalNodeConfig** — the OPC-UA variable attributes required
//!   for §9.3.4.4 (`Historizing = true`, `HistoryRead`
//!   bit in `AccessLevel`, optional `HasHistoricalConfiguration`
//!   reference to an HA configuration node).
//! * **HistoryQosValidator** — verifies that the HISTORY QoS
//!   policy of the underlying DataReader is conformant with Spec §9.3.4.4:
//!   `KEEP_ALL` or `KEEP_LAST` with a sufficient `HISTORY_DEPTH`.
//! * **HistoricalSampleAdapter** — trait + default impl that delivers a
//!   snapshot of the reader history cache in the form that
//!   an OPC-UA server needs for the `HistoryRead` service implementation
//!   (read-event / read-raw / read-at-time / read-processed
//!   per Spec Tab 8.6 Attribute Service Set).
//!
//! # What is not here
//!
//! We provide the **spec-compliance layer**: configuration,
//! QoS validation, adapter contract. The actual sample
//! persistence (e.g. on-disk, if KEEP_ALL goes beyond memory limits)
//! lives in the daemon crate, which clamps the DataReader cache from
//! `crates/dcps/` to a concrete history backend provider
//! (in-memory / RocksDB / TimescaleDB / ...).

pub mod adapter;
pub mod config;
pub mod qos;

pub use adapter::{
    HistoricalSample, HistoricalSampleAdapter, InMemoryHistoryCache, ReadHistoryQuery,
    ReadHistoryResult,
};
pub use config::{AccessLevel, HISTORY_READ_BIT, HistoricalConfigRef, HistoricalNodeConfig};
pub use qos::{HistoryQosKind, HistoryQosValidator, HistoryQosViolation};
