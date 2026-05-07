// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Historical Data Reading — Spec §9.3.4.4.
//!
//! Conformance Point 4 ("Complete") aus §2 Tab 2.1. Enthaelt:
//!
//! * **HistoricalNodeConfig** — die fuer §9.3.4.4 erforderlichen
//!   OPC-UA-Variable-Attribute (`Historizing = true`, `HistoryRead`-
//!   Bit im `AccessLevel`, optionale `HasHistoricalConfiguration`-
//!   Reference auf eine HA-Configuration-Node).
//! * **HistoryQosValidator** — verifiziert, dass die HISTORY-QoS-
//!   Policy des unterliegenden DataReaders Spec §9.3.4.4 konform ist:
//!   `KEEP_ALL` oder `KEEP_LAST` mit ausreichender `HISTORY_DEPTH`.
//! * **HistoricalSampleAdapter** — Trait + Default-Impl, die einen
//!   Snapshot des Reader-History-Cache in der Form ausliefert, die
//!   ein OPC-UA-Server fuer die `HistoryRead`-Service-Implementation
//!   benoetigt (read-event / read-raw / read-at-time / read-processed
//!   gemaess Spec Tab 8.6 Attribute Service Set).
//!
//! # Was hier nicht ist
//!
//! Wir liefern die **Spec-Compliance-Schicht**: Konfiguration,
//! QoS-Validation, Adapter-Contract. Die eigentliche Sample-
//! Persistierung (z.B. on-disk, falls KEEP_ALL ueber Memory-Limits
//! hinaus geht) liegt im Daemon-Crate, das den DataReader-Cache von
//! `crates/dcps/` an einen konkreten History-Backend-Provider
//! (in-memory / RocksDB / TimescaleDB / ...) klemmt.

pub mod adapter;
pub mod config;
pub mod qos;

pub use adapter::{
    HistoricalSample, HistoricalSampleAdapter, InMemoryHistoryCache, ReadHistoryQuery,
    ReadHistoryResult,
};
pub use config::{AccessLevel, HISTORY_READ_BIT, HistoricalConfigRef, HistoricalNodeConfig};
pub use qos::{HistoryQosKind, HistoryQosValidator, HistoryQosViolation};
