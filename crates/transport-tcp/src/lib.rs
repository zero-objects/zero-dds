// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-transport-tcp`. Safety classification: **STANDARD**.
//!
//! ZeroDDS-TCP-Transport — RTPS-over-TCP mit Length-Prefix-Framing
//! und optionalem ZeroDDS-eigenem 16-Byte-Handshake.
//!
//! ## Spec
//!
//! - **DDSI-RTPS 2.5 §9.4** — Locator-Kinds `TCPv4`=4, `TCPv6`=8.
//! - **DDSI-RTPS 2.5 §9.5** — Wire-Bytes-Mapping (RTPS-Header +
//!   Submessages, identisch zum UDP-PSM).
//! - **ZeroDDS-TCP-Transport 1.0** — vendor-spezifischer Transport-
//!   Spec (Length-Prefix-Framing + Handshake),
//!   `docs/spec-coverage/zerodds-tcp-transport-1.0.md`.
//!
//! ## Hinweis zur OMG-Normativität
//!
//! OMG normiert nur Locator-Kind + Wire-Mapping (s.o.). Es gibt
//! **keinen** normativen "DDS-TCP-PSM"-Handshake-Spec. Cyclone's
//! `ddsi_tcp` nutzt raw RTPS-Frames ohne Handshake; FastDDS und RTI
//! haben jeweils eigene vendor-spezifische BindConnection-Formate.
//! ZeroDDS definiert seine eigene Variante explizit als
//! ZeroDDS-TCP-Transport-1.0-Spec (siehe oben).
//!
//! ## Implementiert (RC1)
//!
//! - Length-Prefix-Frame (4 Byte BE) — DDSI-RTPS-§9.5-konform.
//! - 16-Byte ZeroDDS-Handshake mit Magic + Version + Vendor-Id +
//!   Logical-Port + Reason-Code.
//! - Cyclone-`ddsi_tcp`-Compat: Handshake-Skip-Mode für raw RTPS-
//!   Frames.
//! - `TcpTransport`-Pool mit Connection-Reuse + Backoff.
//!
//! ## Cross-Vendor-Interop
//!
//! - ZeroDDS ↔ ZeroDDS: voll (Handshake aktiv).
//! - ZeroDDS ↔ Cyclone: voll via Handshake-Skip-Mode.
//! - ZeroDDS ↔ FastDDS/RTI: Erweiterungspunkt (vendor-spezifische
//!   Handshake-Formate; nicht-OMG-normativ). Siehe
//!   ZeroDDS-TCP-Transport-1.0-Spec §6.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod framing;
pub mod handshake;
pub mod tcp_transport;

pub use tcp_transport::{TcpTransport, TcpTransportError};
