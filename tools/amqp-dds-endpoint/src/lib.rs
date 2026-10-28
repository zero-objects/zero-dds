// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-AMQP 1.0 Endpoint Daemon — Library-Layer.
//!
//! Safety classification: **TOOLING** (Server-Daemon-Bibliothek;
//! no safety-relevant runtime code, std-only).
//!
//! Synchronous, `std`-only AMQP-1.0 server daemon. We read
//! AMQP frames blocking on a `TcpStream` connection and
//! drive the `zerodds-amqp-endpoint` state machine.
//!
//! Spec-Quellen:
//! * dds-amqp-1.0 §2.1 Endpoint Profile (Cl. 2 Connection
//!   Acceptance, Cl. 3 Sender+Receiver Links, Cl. 6 SASL).
//! * `amqp-1.0-transport` §2.2 Protocol Header,
//!   §2.3 Frame Layout, §2.4 Connection State.
//!
//! What this crate provides:
//! * Protocol header read/write (`AMQP\0\1\0\0` and SASL variant).
//! * Frame-Read/Write (8-Byte Header + Body).
//! * Performative-Dispatch (Descriptor → `InboundFrameKind`).
//! * Per-connection handler (`handle_connection`) that
//!   drives the state machine.
//! * `run_server` — multi-connection accept loop with
//!   thread-per-connection.
//!
//! * TLS termination (`tls`, feature `tls`, default; Spec §10.1), which
//!   the binary uses when the configuration has a `<tls>` block.
//! * The outbound bridge client with its reconnect loop (`client`, Spec
//!   §2.2 Cl. 2/7, §10.8); the binary does not start it.
//!
//! The DDS side is the trait `dds_host::DdsHost`; the only implementation
//! here is `InMemoryDdsHost`, so this daemon reaches no DDS domain. The
//! daemon bridging AMQP to a DDS domain is `zerodds-amqp-bridged`
//! (`crates/amqp-endpoint`).

#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Daemon tool — eprintln is the natural logging form
// as long as tracing/log is not available workspace-wide.
#![allow(clippy::print_stderr)]

pub mod auth;
pub mod bridge;
pub mod client;
pub mod dds_host;
pub mod frame_io;
pub mod handler;
pub mod server;
#[cfg(feature = "tls")]
pub mod tls;

pub use client::{
    ClientConfig, ClientError, OutboundSession, ReconnectConfig, connect_outbound,
    connect_with_reconnect,
};
pub use frame_io::{
    AmqpProtocol, FrameIoError, ProtocolHeader, read_frame, read_protocol_header, write_frame,
    write_protocol_header,
};
pub use handler::{ConnectionStats, HandlerError, handle_connection};
pub use server::{ServerConfig, ServerError, run_server};
