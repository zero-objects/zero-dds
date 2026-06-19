// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-http2`. Safety classification: **STANDARD**.
//!
//! HTTP/2 (RFC 9113) wire codec — no_std framing + stream state
//! machine + flow control + connection preface + settings.
//! `forbid(unsafe_code)`. Implements the HTTP/2 wire layer
//! without heap buffering or an async runtime: frames are
//! encoded to / decoded from byte slices, and the state machine is
//! callback-based.
//!
//! Note: RFC 9113 (HTTP/2) superseded RFC 7540, keeping the
//! wire format and most § numbers while removing a few
//! unused features (priority, hint directives). This crate
//! follows RFC 9113.
//!
//! Spec: RFC 9113 §3 (connection preface) + §4 (frame layer) + §5
//! (streams + multiplexing) + §6 (frame definitions) + §7 (error
//! codes).
//!
//! ## Layer position
//!
//! Layer 5 — bridges. Substrate for:
//!
//! - [`zerodds-grpc-bridge`](../zerodds_grpc_bridge/index.html) —
//!   gRPC-over-HTTP/2 + gRPC-Web length-prefixed message codec
//!   (header block via [`zerodds-hpack`](../zerodds_hpack/index.html)).
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! - [`Frame`] / [`FrameHeader`] / [`FrameType`] / [`Flags`] /
//!   [`encode_frame`] / [`decode_frame`] — frame-layer codec (§4 +
//!   §6).
//! - [`CLIENT_PREFACE`] / [`check_preface`] — connection preface
//!   (§3.4).
//! - [`Settings`] / [`Setting`] / [`SettingId`] — settings-frame
//!   codec + defaults (§6.5).
//! - [`StreamId`] / [`StreamState`] — stream state machine (§5.1).
//! - [`FlowControl`] — connection + stream flow control (§5.2).
//! - [`ErrorCode`] / [`Http2Error`] — error codes (§7).
//!
//! ## Example
//!
//! ```rust
//! use zerodds_http2::{FrameHeader, FrameType, Flags, encode_frame, decode_frame};
//! use zerodds_http2::frame::DEFAULT_MAX_FRAME_SIZE;
//!
//! // PING frame (8-byte opaque payload, stream ID 0).
//! let payload = [0u8; 8];
//! let header = FrameHeader {
//!     length: 8,
//!     frame_type: FrameType::Ping,
//!     flags: Flags(0),
//!     stream_id: 0,
//! };
//!
//! let mut buf = [0u8; 17]; // 9-byte header + 8-byte payload
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
