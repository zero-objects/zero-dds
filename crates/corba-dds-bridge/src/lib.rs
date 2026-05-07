// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA-Object ↔ DDS-Topic Bridge.
//!
//! Crate `zerodds-corba-dds-bridge`. Safety classification: **STANDARD**.
//!
//! # Scope
//!
//! Brueckt CORBA-Object-Method-Invocations bidirektional auf DDS-
//! Topic-Publish/Subscribe:
//!
//! * **CORBA → DDS**: Eine GIOP-Request, die der Bridge-POA empfaengt,
//!   wird in ein DDS-Sample umgewandelt und auf dem assoziierten
//!   Topic publiziert. Der Reply kommt aus einem zweiten Reply-Topic.
//! * **DDS → CORBA**: Ein DDS-Sample auf einem Source-Topic wird in
//!   eine GIOP-Request konvertiert und an einen CORBA-Client-Endpoint
//!   gesendet (Forwarding-Modus).
//!
//! Der `BridgeMapping` erlaubt eine Many-to-Many-Konfiguration: ein
//! CORBA-Object kann mehrere Topics bedienen, ein DDS-Topic kann
//! Requests von mehreren CORBA-Objects akzeptieren.
//!
//! Konkrete Wire-Anbindung an die CORBA-Welt liegt in [`wire`]:
//! [`decode_giop_request_bytes`] und [`object_key_from_ior`] kapseln
//! die Cross-Crate-Aufrufe an `corba-giop` bzw. `corba-ior`.
//!
//! ## Beispiel
//!
//! ```
//! use zerodds_corba_dds_bridge::Direction;
//! assert_ne!(Direction::CorbaToDds, Direction::DdsToCorba);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

// Bridge-Security-Adapter (Bridge-Spec §7.1 TLS via rustls/SSLIOP,
// §7.2 Auth-Modes, §7.3 Topic-ACL). std-only.
#[cfg(feature = "std")]
pub mod bridge_security;
// Daemon-Runtime-Adapter + QoS-Translation. std-only.
pub mod csiv2_wire;
#[cfg(feature = "std")]
pub mod daemon_runtime;
pub mod locate;
pub mod mapping;
pub mod notify;
#[cfg(feature = "std")]
pub mod qos_translation;
pub mod servant;
pub mod sync;
pub mod wire;

pub use mapping::{BridgeMapping, BridgeRoute, Direction, OperationMapping, TopicQosRef};
pub use servant::BridgeServant;
pub use sync::{LifecycleEvent, LifecycleSync};
pub use wire::{RequestSummary, decode_giop_request_bytes, object_key_from_ior};
