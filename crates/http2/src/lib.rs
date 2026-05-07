// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-http2`. Safety classification: **STANDARD**.
//!
//! HTTP/2 (RFC 9113) Wire-Codec — no_std Framing + Stream-State-
//! Machine + Flow-Control + Connection-Preface + Settings.
//! `forbid(unsafe_code)`. Implementiert die Wire-Schicht von HTTP/2
//! ohne Heap-Buffering oder Async-Runtime: Frames werden in/aus
//! byte-Slices kodiert/dekodiert, die State-Machine ist callback-
//! basiert.
//!
//! Hinweis: RFC 9113 (HTTP/2) hat RFC 7540 abgeloest, behaelt das
//! Wire-Format und die meisten §-Nummern bei und entfernt einige
//! ungenutzte Features (Priority, Hint-Direktiven). Diese Crate
//! folgt RFC 9113.
//!
//! Spec: RFC 9113 §3 (Connection-Preface) + §4 (Frame-Layer) + §5
//! (Streams + Multiplexing) + §6 (Frame-Definitions) + §7 (Error-
//! Codes).
//!
//! ## Schichten-Position
//!
//! Layer 5 — Bridges. Substrat fuer:
//!
//! - [`zerodds-grpc-bridge`](../zerodds_grpc_bridge/index.html) —
//!   gRPC-over-HTTP/2 + gRPC-Web Length-Prefixed-Message-Codec
//!   (Header-Block via [`zerodds-hpack`](../zerodds_hpack/index.html)).
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`Frame`] / [`FrameHeader`] / [`FrameType`] / [`Flags`] /
//!   [`encode_frame`] / [`decode_frame`] — Frame-Layer-Codec (§4 +
//!   §6).
//! - [`CLIENT_PREFACE`] / [`check_preface`] — Connection-Preface
//!   (§3.4).
//! - [`Settings`] / [`Setting`] / [`SettingId`] — Settings-Frame-
//!   Codec + Defaults (§6.5).
//! - [`StreamId`] / [`StreamState`] — Stream-State-Machine (§5.1).
//! - [`FlowControl`] — Connection + Stream Flow-Control (§5.2).
//! - [`ErrorCode`] / [`Http2Error`] — Error-Codes (§7).
//!
//! ## Beispiel
//!
//! ```rust
//! use zerodds_http2::{FrameHeader, FrameType, Flags, encode_frame, decode_frame};
//! use zerodds_http2::frame::DEFAULT_MAX_FRAME_SIZE;
//!
//! // PING-Frame (8-Byte-Opaque-Payload, Stream-ID 0).
//! let payload = [0u8; 8];
//! let header = FrameHeader {
//!     length: 8,
//!     frame_type: FrameType::Ping,
//!     flags: Flags(0),
//!     stream_id: 0,
//! };
//!
//! let mut buf = [0u8; 17]; // 9-Byte-Header + 8-Byte-Payload
//! let written = encode_frame(&header, &payload, &mut buf, DEFAULT_MAX_FRAME_SIZE)
//!     .expect("encode");
//! assert_eq!(written, 17);
//!
//! let (decoded, consumed) = decode_frame(&buf, DEFAULT_MAX_FRAME_SIZE).expect("decode");
//! assert_eq!(consumed, 17);
//! assert_eq!(decoded.header.frame_type, FrameType::Ping);
//! ```

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod error;
pub mod flow;
pub mod frame;
pub mod preface;
pub mod settings;
pub mod stream;

pub use error::{ErrorCode, Http2Error};
pub use flow::FlowControl;
pub use frame::{Flags, Frame, FrameHeader, FrameType, decode_frame, encode_frame};
pub use preface::{CLIENT_PREFACE, check_preface};
pub use settings::{Setting, SettingId, Settings};
pub use stream::{StreamId, StreamState};
