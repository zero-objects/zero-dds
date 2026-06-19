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
//!   ActivateSession / CloseSession) plus the Read, Write and Call service sets.
//! - [`address_space`] — a small in-memory AddressSpace (node values + method
//!   handlers) the server serves.
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
pub mod client;
pub mod server;
pub mod services;
mod wire;

pub use address_space::{AddressSpace, MethodOutcome};
pub use client::Client;
pub use server::Server;
