// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-discovery`. Safety classification: **SAFE**.
//!
//! DDSI-RTPS discovery for ZeroDDS — SPDP, SEDP, TypeLookup service.
//!
//! ## Spec
//!
//! - **DDSI-RTPS 2.5 §8.5** — Simple Discovery Protocol (SPDP/SEDP).
//! - **DDSI-RTPS 2.5 §8.5.3** — SPDP Builtin-Endpoints.
//! - **DDSI-RTPS 2.5 §8.5.4** — SEDP Builtin-Endpoints.
//! - **XTypes 1.3 §7.6.3.3.4** — TypeLookup-Service
//!   (`TL_SVC_REQ_{WRITER,READER}` + `TL_SVC_REPLY_{WRITER,READER}`).
//! - **DDS-Security 1.2 §7.4.2** — Stateless + Volatile-Secure
//!   builtin endpoints (sub-module `security`).
//!
//! ## Public API
//!
//! - [`spdp`] — SPDP beacon sender + receiver +
//!   `DiscoveredParticipantsCache` with `last_seen` lease tracking.
//! - [`sedp`] — SEDP stack (cache, reader, writer).
//! - [`type_lookup`] — TypeLookup service server + client +
//!   builtin-endpoint GUIDs.
//! - [`security`] — DDS-Security Stateless + Volatile-Secure
//!   builtin-endpoint slots.
//! - [`capabilities::PeerCapabilities`] — DDSI capability bits.
//!
//! ## Wiring into the DCPS runtime
//!
//! The discovery primitives are wire-format-complete. Instantiation of
//! the builtin-endpoint reliable writer/reader pairs lives in the DCPS
//! layer (`crates/dcps/src/runtime.rs`):
//! - SPDP: best-effort writer + reader on `ParticipantBuiltinTopicData`.
//! - SEDP: reliable writer + reader on
//!   `Publication-/Subscription-BuiltinTopicData`.
//! - TypeLookup: reliable writer + reader on the `TypesRequest`/
//!   `TypesReply` topic with a service instance name.

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
