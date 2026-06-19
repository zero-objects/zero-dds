// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-transport-tcp`. Safety classification: **STANDARD**.
//!
//! ZeroDDS TCP transport — RTPS-over-TCP with length-prefix framing
//! and an optional ZeroDDS-own 16-byte handshake.
//!
//! ## Spec
//!
//! - **DDSI-RTPS 2.5 §9.4** — locator kinds `TCPv4`=4, `TCPv6`=8.
//! - **DDSI-RTPS 2.5 §9.5** — wire-bytes mapping (RTPS header +
//!   submessages, identical to the UDP PSM).
//! - **ZeroDDS TCP Transport 1.0** — vendor-specific transport spec
//!   (length-prefix framing + handshake),
//!   `docs/spec-coverage/zerodds-tcp-transport-1.0.md`.
//!
//! ## Note on OMG normativity
//!
//! OMG standardizes only the locator kind + wire mapping (see above).
//! There is **no** normative "DDS-TCP-PSM" handshake spec. Cyclone's
//! `ddsi_tcp` uses raw RTPS frames without a handshake; FastDDS and RTI
//! each have their own vendor-specific BindConnection formats.
//! ZeroDDS defines its own variant explicitly as the
//! ZeroDDS TCP Transport 1.0 spec (see above).
//!
//! ## Implemented (RC1)
//!
//! - Length-prefix frame (4 bytes BE) — DDSI-RTPS §9.5 conformant.
//! - 16-byte ZeroDDS handshake with magic + version + vendor id +
//!   logical port + reason code.
//! - Cyclone `ddsi_tcp` compat: handshake-skip mode for raw RTPS
//!   frames.
//! - `TcpTransport` pool with connection reuse + backoff.
//!
//! ## Cross-vendor interop
//!
//! - ZeroDDS ↔ ZeroDDS: full (handshake active).
//! - ZeroDDS ↔ Cyclone: full via handshake-skip mode.
//! - ZeroDDS ↔ FastDDS/RTI: extension point (vendor-specific
//!   handshake formats; non-OMG-normative). See the
//!   ZeroDDS TCP Transport 1.0 spec §6.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod framing;
pub mod handshake;
pub mod tcp_transport;

pub use tcp_transport::{TcpTransport, TcpTransportError};
