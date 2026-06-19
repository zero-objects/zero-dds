// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-websocket-bridge`. Safety classification: **STANDARD**.
//!
//! Complete WebSocket (RFC 6455) stack set — pure-Rust `no_std +
//! alloc`, `forbid(unsafe_code)`. Implements the full WebSocket
//! spec including the base framing protocol (§5.2 + §5.3), opening
//! handshake (§4) with `Sec-WebSocket-Accept` SHA1 computation,
//! extension + subprotocol negotiation (§9), close-frame status
//! code semantics (§7.4) incl. forbidden-on-wire checking,
//! the permessage-deflate extension (RFC 7692), a URI parser (`ws://` /
//! `wss://`, RFC 6455 §3), a streaming UTF-8 validator (§8.1) for
//! text frames, as well as a WebSocket↔DDS topic bridge.
//!
//! Spec-Referenzen:
//!
//! - **RFC 6455** — The WebSocket Protocol.
//! - **RFC 7692** — Compression Extensions for WebSocket
//!   (`permessage-deflate`).
//!
//! ## Layer position
//!
//! Layer 5 — bridges. Substrate for browser↔DDS endpoint mapping
//! (web UIs, realtime dashboards, DDS web gateway).
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`Frame`] / [`Opcode`] — frame model (§5.2).
//! - [`encode`] / [`decode`] / [`CodecError`] — wire codec including
//!   payload length encoding (7-bit / 7+16-bit / 7+64-bit) and
//!   mask application.
//! - [`apply_mask`] / [`generate_masking_key`] /
//!   [`MaskingKeyProvider`] / [`InsecureSplitmixProvider`] /
//!   [`ClosureMaskingKeyProvider`] — XOR-Masking (§5.3).
//! - [`CloseCode`] / [`ClosePayload`] / [`encode_close_payload`] /
//!   [`decode_close_payload`] — Close-Frame-Codec (§7.4).
//! - [`ClientHandshake`] / [`ServerHandshake`] /
//!   [`compute_accept`] / [`parse_client_request`] /
//!   [`build_server_response`] / [`render_server_response`] /
//!   [`HandshakeError`] — opening handshake (§4).
//! - [`PermessageDeflateParams`] / [`parse_offer`] / [`render_accept`]
//!   / [`append_tail`] / [`strip_tail`] / [`DEFLATE_TAIL`] /
//!   [`NegotiationError`] — RFC 7692 permessage-deflate negotiation.
//! - [`WebSocketUri`] / [`parse_websocket_uri`] / [`default_port`] /
//!   [`is_local_loopback`] / [`resource_name`] / [`UriError`] —
//!   `ws://` / `wss://` URI parser (§3).
//! - [`StreamingValidator`] / [`validate_utf8`] / [`Utf8Error`] —
//!   text-frame UTF-8 validator (§8.1).
//! - [`SubscriptionRegistry`] / [`Notification`] / [`BridgeOp`] /
//!   [`BridgeError`] / [`parse_op`] / [`render_notification`] —
//!   WebSocket↔DDS topic bridge.
//!
//! ## Example
//!
//! ```rust
//! use zerodds_websocket_bridge::{compute_accept};
//!
//! // RFC 6455 §1.3: Sec-WebSocket-Accept example.
//! let accept = compute_accept("dGhlIHNhbXBsZSBub25jZQ==");
//! assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::approx_constant,
    clippy::unreachable,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    clippy::useless_conversion,
    clippy::manual_strip,
    missing_docs
)]

extern crate alloc;

pub mod close;
pub mod codec;
#[cfg(feature = "daemon")]
pub mod daemon;
pub mod dds_bridge;
pub mod frame;
pub mod handshake;
pub mod masking;
pub mod message;
pub mod negotiation;
pub mod permessage_deflate;
pub mod uri;
pub mod utf8;

pub use close::{CloseCode, ClosePayload, decode_close_payload, encode_close_payload};
pub use codec::{CodecError, decode, encode};
pub use dds_bridge::{
    BridgeError, BridgeOp, Notification, SubscriptionRegistry, parse_op, render_notification,
};
pub use frame::{Frame, Opcode};
pub use handshake::{
    ClientHandshake, HandshakeError, ServerHandshake, build_server_response, compute_accept,
    parse_client_request, render_server_response,
};
pub use masking::{
    ClosureMaskingKeyProvider, InsecureSplitmixProvider, MaskingKeyProvider, apply_mask,
    generate_masking_key,
};
pub use permessage_deflate::{
    DEFLATE_TAIL, NegotiationError, PermessageDeflateParams, append_tail, parse_offer,
    render_accept, strip_tail,
};
pub use uri::{
    UriError, WebSocketUri, default_port, is_local_loopback, parse_websocket_uri, resource_name,
};
pub use utf8::{StreamingValidator, Utf8Error, validate as validate_utf8};
