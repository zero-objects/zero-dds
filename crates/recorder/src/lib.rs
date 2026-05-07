// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-recorder`. Safety classification: **STANDARD**.
//!
//! `.zddsrec` Recording-/Replay-Format. Spec:
//! [`docs/specs/zddsrec-1.0.md`](../../docs/specs/zddsrec-1.0.md).
//!
//! ## Schichten-Position
//!
//! Layer 4 — Core Services. Pure-Rust + alloc, ohne ZeroDDS-Crate-Deps.
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`Header`], [`Frame`], [`FrameView`], [`SampleKind`], `ParticipantEntry`, `TopicEntry`.
//! - [`RecordWriter`] / [`WriteError`] — schreibt einen `.zddsrec`-Stream.
//! - [`RecordReader`] / [`ReadError`] — parsed einen `.zddsrec`-Stream.
//! - [`RecordingSession`] / [`SessionError`] / [`SessionOptions`] / [`TopicKey`] — high-level API.
//!
//! # Format-Layout
//!
//! Ein `.zddsrec`-File besteht aus einem [`Header`] gefolgt von einer
//! Sequenz von [`Frame`]-Records. Endianness: little-endian fuer alle
//! Multi-Byte-Felder.
//!
//! ```text
//! +---------------------------------+
//! | Magic "ZDDS" (4 bytes)          |
//! | Version u32 (=1)                |
//! | TimeBaseUnixNs i64              |
//! | ParticipantCount u32            |
//! | TopicCount u32                  |
//! | Participants[] (GUID16+nameLen+name)
//! | Topics[] (typeLen+typeName+nameLen+name)
//! +---------------------------------+
//! | FrameMagic 'F' (1 byte)         |
//! | TimestampDeltaNs i64            |
//! | ParticipantIdx u32              |
//! | TopicIdx u32                    |
//! | SampleKind u8 (0=Alive,1=Disposed,2=Unregistered)
//! | PayloadLen u32                  |
//! | CdrPayload[PayloadLen]          |
//! +---------------------------------+
//! | ... weitere Frames ...          |
//! +---------------------------------+
//! ```
//!
//! # Versionierung
//!
//! Version = 1 ([`ZDDSREC_VERSION`]). Backward-incompatible Aenderungen
//! erhoehen die Version; der Reader lehnt unbekannte Versionen ab.

#![warn(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod format;
pub mod reader;
pub mod session;
pub mod writer;

pub use format::{
    Frame, FrameView, Header, ParticipantEntry, SampleKind, TopicEntry, ZDDSREC_MAGIC,
    ZDDSREC_VERSION,
};
pub use reader::{ReadError, RecordReader};
pub use session::{RecordingSession, SessionError, SessionOptions, TopicKey};
pub use writer::{RecordWriter, WriteError};
