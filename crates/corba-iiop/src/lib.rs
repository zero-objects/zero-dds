// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG CORBA 3.3 Part 2 §14 + §15.7 — Internet Inter-ORB Protocol (IIOP).
//!
//! Crate `zerodds-corba-iiop`. Safety classification: **STANDARD**.
//! Spec OMG CORBA 3.3 Part 2 §14 (IIOP Overview) + §15.7 (IIOP Profile)
//! + §15.9 (Bidirectional-GIOP).
//!
//! # Scope
//!
//! Voller IIOP-TCP-Transport-Stack:
//!
//! * **ProfileBody** — alle 4 IIOP-Versionen (1.0/1.1/1.2/1.3) inkl.
//!   `host`/`port`/`object_key`/`components` (Spec §15.7.2). 1.0 hat
//!   keine Components-Sequenz; ab 1.1 sind sie Pflicht-Bestandteil.
//! * **Connection** — TCP-Stream-Wrapper, der GIOP-Messages frame-genau
//!   liest und schreibt (12-Byte-Header -> `message_size` ->
//!   Body-Read).
//! * **Connector (Client)** — TCP-Connect mit Connection-Reuse und
//!   thread-safe Pool, Connect-Timeout und automatische Reconnect-
//!   Logik bei `CloseConnection`-Empfang.
//! * **Acceptor (Server)** — TCP-Listener-Loop mit Per-Connection-
//!   Worker-Thread.
//! * **Bidirectional-GIOP** (Spec §15.9) — Server kann Requests an
//!   den Client schicken nach Aushandlung via `BiDirIIOPServiceContext`.
//!
//! ## Beispiel
//!
//! ```
//! use zerodds_corba_iiop::IiopVersion;
//! assert_eq!(IiopVersion::V1_2.major, 1);
//! assert_eq!(IiopVersion::V1_2.minor, 2);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod bidir;
pub mod profile_body;

#[cfg(feature = "std")]
pub mod acceptor;
#[cfg(feature = "std")]
pub mod connection;
#[cfg(feature = "std")]
pub mod connector;
#[cfg(feature = "std")]
pub mod error;
#[cfg(feature = "std")]
pub mod framing;

pub use bidir::{BiDirIiopListenPoint, BiDirIiopServiceContext, IIOP_BI_DIR_TAG};
pub use profile_body::{IiopProfileBody, IiopVersion, TaggedComponent};

#[cfg(feature = "std")]
pub use acceptor::{Acceptor, AcceptorConfig};
#[cfg(feature = "std")]
pub use connection::Connection;
#[cfg(feature = "std")]
pub use connector::{Connector, ConnectorConfig};
#[cfg(feature = "std")]
pub use error::IiopError;
#[cfg(feature = "std")]
pub use framing::{read_giop_message, write_giop_message};
