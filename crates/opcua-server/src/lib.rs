// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Native OPC-UA Client/Server (OPC Foundation **Part 4 — Services**) on top
//! of the SecureChannel/UACP transport ([`zerodds-opcua-uacp`]).
//!
//! Crate `zerodds-opcua-server`. Safety classification: **STANDARD**.
//!
//! This is the request/response counterpart to the native UADP PubSub stack
//! ([`zerodds-opcua-pubsub`]) — completing OPC-UA the same way ZeroDDS ships
//! full CORBA/MQTT/AMQP stacks. It provides:
//!
//! - [`services`] — the service messages: Session lifecycle (CreateSession /
//!   ActivateSession / CloseSession) plus the Discovery, Attribute, View,
//!   Method, MonitoredItem and Subscription service sets.
//! - [`address_space`] — a small in-memory AddressSpace (Part-3 node attributes,
//!   a reference graph and method handlers) the server serves.
//! - [`attributes`] — the Part-3 attribute model: attribute ids, WriteMask and
//!   AccessLevel bits, `NumericRange`, `TimestampsToReturn`, `MonitoringMode`.
//! - [`status`] — the StatusCodes the services return.
//! - [`identity`] — the Part 4 §7.40 UserIdentityTokens and the server-side
//!   user store that validates them.
//! - [`server`] — an [`server::Server`] that runs the Hello→OpenSecureChannel→
//!   Session→service handshake and dispatches services against the
//!   AddressSpace.
//! - [`client`] — an [`client::Client`] that drives the same handshake and
//!   calls services.
//!
//! SecurityMode `None` is implemented end to end; the secured SecurityPolicies
//! (`crypto` feature of `zerodds-opcua-uacp`) layer on top.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used, clippy::panic))]

extern crate alloc;

pub mod address_space;
pub mod attributes;
pub mod client;
pub mod identity;
pub mod server;
pub mod services;
pub mod status;
mod wire;

pub use address_space::{AddressSpace, MethodOutcome};
pub use client::Client;
pub use server::Server;
