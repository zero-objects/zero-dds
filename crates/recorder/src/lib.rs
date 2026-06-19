// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-recorder`. Safety classification: **STANDARD**.
//!
//! `.zddsrec` recording/replay format. Spec:
//! [`docs/specs/zddsrec-1.0.md`](../../docs/specs/zddsrec-1.0.md).
//!
//! ## Layer position
//!
//! Layer 4 — core services. Pure Rust + alloc, without ZeroDDS crate deps.
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! - [`Header`], [`Frame`], [`FrameView`], [`SampleKind`], `ParticipantEntry`, `TopicEntry`.
//! - [`RecordWriter`] / [`WriteError`] — writes a `.zddsrec` stream.
//! - [`RecordReader`] / [`ReadError`] — parses a `.zddsrec` stream.
//! - [`RecordingSession`] / [`SessionError`] / [`SessionOptions`] / [`TopicKey`] — high-level API.
//!
//! # Format layout
//!
//! A `.zddsrec` file consists of a [`Header`] followed by a
//! sequence of [`Frame`] records. Endianness: little-endian for all
//! multi-byte fields.
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
//! | ... more frames ...             |
//! +---------------------------------+
//! ```
//!
//! # Versioning
//!
//! Version = 1 ([`ZDDSREC_VERSION`]). Backward-incompatible changes
//! bump the version; the reader rejects unknown versions.

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
