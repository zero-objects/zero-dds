// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-websocket-bridge`. Safety classification: **STANDARD**.
//!
//! WebSocket (RFC 6455) komplettes Stack-Set — pure-Rust `no_std +
//! alloc`, `forbid(unsafe_code)`. Implementiert die volle WebSocket-
//! Spec inklusive Base-Framing-Protocol (§5.2 + §5.3), Opening-
//! Handshake (§4) mit `Sec-WebSocket-Accept`-SHA1-Berechnung,
//! Extension- + Subprotocol-Negotiation (§9), Close-Frame-Status-
//! Code-Semantik (§7.4) inkl. Forbidden-on-Wire-Pruefung,
//! permessage-deflate Extension (RFC 7692), URI-Parser (`ws://` /
//! `wss://`, RFC 6455 §3), Streaming-UTF-8-Validator (§8.1) fuer
//! Text-Frames, sowie einen WebSocket↔DDS-Topic-Bridge.
//!
//! Spec-Referenzen:
//!
//! - **RFC 6455** — The WebSocket Protocol.
//! - **RFC 7692** — Compression Extensions for WebSocket
//!   (`permessage-deflate`).
//!
//! ## Schichten-Position
//!
//! Layer 5 — Bridges. Substrat fuer Browser↔DDS-Endpoint-Mapping
//! (Web-UIs, Realtime-Dashboards, DDS-Web-Gateway).
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`Frame`] / [`Opcode`] — Frame-Modell (§5.2).
//! - [`encode`] / [`decode`] / [`CodecError`] — Wire-Codec inklusive
//!   Payload-Length-Encoding (7-bit / 7+16-bit / 7+64-bit) und
//!   Masking-Application.
//! - [`apply_mask`] / [`generate_masking_key`] /
//!   [`MaskingKeyProvider`] / [`InsecureSplitmixProvider`] /
//!   [`ClosureMaskingKeyProvider`] — XOR-Masking (§5.3).
//! - [`CloseCode`] / [`ClosePayload`] / [`encode_close_payload`] /
//!   [`decode_close_payload`] — Close-Frame-Codec (§7.4).
//! - [`ClientHandshake`] / [`ServerHandshake`] /
//!   [`compute_accept`] / [`parse_client_request`] /
//!   [`build_server_response`] / [`render_server_response`] /
//!   [`HandshakeError`] — Opening-Handshake (§4).
//! - [`PermessageDeflateParams`] / [`parse_offer`] / [`render_accept`]
//!   / [`append_tail`] / [`strip_tail`] / [`DEFLATE_TAIL`] /
//!   [`NegotiationError`] — RFC 7692 permessage-deflate Negotiation.
//! - [`WebSocketUri`] / [`parse_websocket_uri`] / [`default_port`] /
//!   [`is_local_loopback`] / [`resource_name`] / [`UriError`] —
//!   `ws://` / `wss://` URI-Parser (§3).
//! - [`StreamingValidator`] / [`validate_utf8`] / [`Utf8Error`] —
//!   Text-Frame-UTF-8-Validator (§8.1).
//! - [`SubscriptionRegistry`] / [`Notification`] / [`BridgeOp`] /
//!   [`BridgeError`] / [`parse_op`] / [`render_notification`] —
//!   WebSocket↔DDS-Topic-Bridge.
//!
//! ## Beispiel
//!
//! ```rust
//! use zerodds_websocket_bridge::{compute_accept};
//!
//! // RFC 6455 §1.3: Sec-WebSocket-Accept-Beispiel.
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
