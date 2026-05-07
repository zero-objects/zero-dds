// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-AMQP 1.0 Endpoint Daemon — Library-Layer.
//!
//! Safety classification: **TOOLING** (Server-Daemon-Bibliothek;
//! kein safety-relevanter Runtime-Code, std-only).
//!
//! Synchronen, `std`-only AMQP-1.0-Server-Daemon. Wir lesen
//! AMQP-Frames blocking auf einer `TcpStream`-Verbindung und
//! treiben die `zerodds-amqp-endpoint`-State-Machine.
//!
//! Spec-Quellen:
//! * dds-amqp-1.0 §2.1 Endpoint Profile (Cl. 2 Connection
//!   Acceptance, Cl. 3 Sender+Receiver Links, Cl. 6 SASL).
//! * `amqp-1.0-transport` §2.2 Protocol Header,
//!   §2.3 Frame Layout, §2.4 Connection State.
//!
//! Was diese Crate liefert:
//! * Protocol-Header-Read/Write (`AMQP\0\1\0\0` und SASL-Variant).
//! * Frame-Read/Write (8-Byte Header + Body).
//! * Performative-Dispatch (Descriptor → `InboundFrameKind`).
//! * Per-Connection-Handler (`handle_connection`), der die
//!   State-Machine treibt.
//! * `run_server` — Multi-Connection-Accept-Loop mit
//!   thread-per-connection.
//!
//! Was diese Crate **nicht** liefert (separate Folge-Schichten):
//! * TLS-Termination — Spec §10.1 Daemon-Layer-Welle.
//! * DDS-Side-Bruecke (DataWriter/Reader) — Spec §2.1 Cl. 3
//!   Folge-Welle.
//! * Outbound-Bridge-Mode — Spec §2.2 Cl. 2.
//! * Reconnect-Loop — Spec §10.8.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Daemon-Tool — eprintln ist die natuerliche Logging-Form
// solange tracing/log nicht workspace-weit verfuegbar ist.
#![allow(clippy::print_stderr)]

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
