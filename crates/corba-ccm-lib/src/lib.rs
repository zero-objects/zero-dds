// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-corba-ccm-lib` — Production-Ready CCM-Components.
//!
//! Crate `zerodds-corba-ccm-lib`. Safety classification: **STANDARD**.
//! Spec: OMG CCM 4.0 (`formal/2006-04-01`) §6 + §10 (Persistent State).
//!
//! Enthaelt drei produktionsreife CCM-Components, die als Schablone
//! oder direkt in Plans verwendet werden koennen:
//!
//! * [`dds_bridge`] — `DdsBridgeComponent`: bidirektionale CCM↔DDS-
//!   Bruecke, mappt CCM-EventSinks auf DDS-Topics und DDS-Reader auf
//!   CCM-EventSources.
//! * [`persistence`] — `PersistenceStorageComponent`: Persistent
//!   State Service §10 (`/storage`).
//! * [`telemetry`] — `TelemetryComponent`: emittiert Component-
//!   Lifecycle-Metriken via DCPS-Topic `__ZeroDDS_CcmTelemetry`.
//!
//! ## Beispiel
//!
//! ```
//! use zerodds_corba_ccm_lib::{MappingDirection, TopicMapping};
//! let m = TopicMapping {
//!     port_name: "in".into(),
//!     topic_name: "Sensor".into(),
//!     type_name: "sensors::Tick".into(),
//!     direction: MappingDirection::SinkSubscribesTopic,
//! };
//! assert_eq!(m.direction, MappingDirection::SinkSubscribesTopic);
//! ```

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod dds_bridge;
pub mod persistence;
pub mod telemetry;

pub use dds_bridge::{BridgeError, DdsBridgeComponent, MappingDirection, TopicMapping};
pub use persistence::{PersistenceError, PersistenceStorageComponent, StorageEntry};
pub use telemetry::{TelemetryComponent, TelemetryEvent, TelemetryKind};
