// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA-object ↔ DDS-topic bridge.
//!
//! Crate `zerodds-corba-dds-bridge`. Safety classification: **STANDARD**.
//!
//! # Scope
//!
//! Bridges CORBA object method invocations bidirectionally onto DDS
//! topic publish/subscribe:
//!
//! * **CORBA → DDS**: A GIOP request received by the bridge POA is
//!   converted into a DDS sample and published on the associated
//!   topic. The reply comes from a second reply topic.
//! * **DDS → CORBA**: A DDS sample on a source topic is converted into
//!   a GIOP request and sent to a CORBA client endpoint (forwarding
//!   mode).
//!
//! The `BridgeMapping` allows a many-to-many configuration: a CORBA
//! object can serve multiple topics, and a DDS topic can accept
//! requests from multiple CORBA objects.
//!
//! The concrete wire binding to the CORBA world lives in [`wire`]:
//! [`decode_giop_request_bytes`] and [`object_key_from_ior`] encapsulate
//! the cross-crate calls into `corba-giop` and `corba-ior` respectively.
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
// Daemon runtime adapter + QoS translation. std-only.
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
