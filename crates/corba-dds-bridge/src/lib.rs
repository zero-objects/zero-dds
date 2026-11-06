// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA-object ↔ DDS-topic bridge.
//!
//! Crate `zerodds-corba-dds-bridge`. Safety classification: **STANDARD**.
//!
//! # Scope
//!
//! The daemon `zerodds-corba-bridged` (`zerodds-corba-bridge-1.0`):
//!
//! * **CORBA → DDS** (§4.3): a GIOP request to a mapped object key and
//!   operation becomes a sample on the mapping's request topic; the sample
//!   on its reply topic with the same `request_id` becomes the GIOP reply.
//! * **DDS → CORBA** (§4.4): a sample on a notify topic becomes a oneway
//!   GIOP request to each target of the mapping.
//!
//! [`bridge_daemon`] holds the configuration (§3), the IIOP and SSLIOP
//! listeners (§4, §7.1), LocateRequest (§4.7) and the DDS side;
//! [`daemon_runtime`], [`bridge_security`], [`csiv2_wire`] and
//! [`naming_client`] what it uses for metrics, TLS, CSIv2 and the
//! NameService. [`mapping`] describes mappings as code, for the examples
//! that do so.
//!
//! ## Example
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

// Bridge security adapter (bridge spec §7.1 TLS via rustls/SSLIOP,
// §7.2 auth modes, §7.3 topic ACL). std-only.
#[cfg(feature = "std")]
pub mod bridge_security;
// The CSIv2 service context of a GIOP message (§7.2).
pub mod csiv2_wire;
// Daemon runtime adapter: metrics, shutdown. std-only.
#[cfg(feature = "std")]
pub mod daemon_runtime;
// The bridge daemon: configuration, IIOP/SSLIOP server, the DDS side.
#[cfg(feature = "std")]
pub mod bridge_daemon;
pub mod mapping;
#[cfg(feature = "std")]
pub mod naming_client;

pub use mapping::{BridgeMapping, BridgeRoute, Direction, OperationMapping, TopicQosRef};
