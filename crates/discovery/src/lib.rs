// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-discovery`. Safety classification: **SAFE**.
//!
//! DDSI-RTPS-Discovery für ZeroDDS — SPDP, SEDP, TypeLookup-Service.
//!
//! ## Spec
//!
//! - **DDSI-RTPS 2.5 §8.5** — Simple Discovery Protocol (SPDP/SEDP).
//! - **DDSI-RTPS 2.5 §8.5.3** — SPDP Builtin-Endpoints.
//! - **DDSI-RTPS 2.5 §8.5.4** — SEDP Builtin-Endpoints.
//! - **XTypes 1.3 §7.6.3.3.4** — TypeLookup-Service
//!   (`TL_SVC_REQ_{WRITER,READER}` + `TL_SVC_REPLY_{WRITER,READER}`).
//! - **DDS-Security 1.2 §7.4.2** — Stateless + Volatile-Secure
//!   Builtin-Endpoints (sub-module `security`).
//!
//! ## Public API
//!
//! - [`spdp`] — SPDP Beacon-Sender + -Receiver +
//!   `DiscoveredParticipantsCache` mit `last_seen`-Lease-Tracking.
//! - [`sedp`] — SEDP Stack (Cache, Reader, Writer).
//! - [`type_lookup`] — TypeLookup-Service Server + Client +
//!   Builtin-Endpoint-GUIDs.
//! - [`security`] — DDS-Security Stateless + Volatile-Secure
//!   Builtin-Endpoint-Slots.
//! - [`capabilities::PeerCapabilities`] — DDSI-Capability-Bits.
//!
//! ## Wiring an DCPS-Runtime
//!
//! Die Discovery-Primitives sind wire-format-vollständig. Die
//! Instantiierung der Builtin-Endpoint-Reliable-Writer/Reader-Pairs
//! liegt im DCPS-Layer (`crates/dcps/src/runtime.rs`):
//! - SPDP: Best-Effort Writer + Reader auf `ParticipantBuiltinTopicData`.
//! - SEDP: Reliable Writer + Reader auf
//!   `Publication-/Subscription-BuiltinTopicData`.
//! - TypeLookup: Reliable Writer + Reader auf `TypesRequest`/
//!   `TypesReply`-Topic mit Service-Instance-Name.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "alloc")]
pub mod capabilities;
#[cfg(feature = "metrics")]
pub mod metrics;
#[cfg(feature = "alloc")]
pub mod security;
#[cfg(feature = "alloc")]
pub mod sedp;
#[cfg(feature = "alloc")]
pub mod spdp;
#[cfg(feature = "alloc")]
pub mod type_lookup;

#[cfg(feature = "alloc")]
pub use capabilities::PeerCapabilities;
#[cfg(feature = "alloc")]
pub use sedp::{
    CacheCaps, DiscoveredEndpointsCache, DiscoveredPublication, DiscoveredSubscription,
};
#[cfg(feature = "alloc")]
pub use spdp::{
    DiscoveredParticipant, DiscoveredParticipantsCache, SpdpBeacon, SpdpError, SpdpReader,
};
