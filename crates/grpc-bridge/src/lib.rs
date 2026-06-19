// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-grpc-bridge`. Safety classification: **STANDARD**.
//!
//! gRPC-over-HTTP/2 + gRPC-Web wire codec — pure-Rust `no_std +
//! alloc`, `forbid(unsafe_code)`. Implements the gRPC-specific
//! wire elements that sit above the HTTP/2 stack: length-prefixed
//! message (LPM), path parsing, timeout header, status codes,
//! custom-metadata encoding (incl. the `-bin` suffix convention),
//! gRPC-Web trailer frames and a gRPC server skeleton for
//! caller-configured HTTP/2 listeners.
//!
//! Spec: gRPC HTTP/2 Protocol (length-prefixed message + path +
//! timeout + status + custom metadata) + gRPC-Web Specification
//! (trailer frame + content types).
//!
//! ## Layer position
//!
//! Layer 5 — bridges. Sits on [`zerodds-http2`](../zerodds_http2/index.html)
//! (RFC 9113 framing + stream state + flow control) and
//! [`zerodds-hpack`](../zerodds_hpack/index.html) (RFC 7541 header
//! compression). The HTTP/2 connection lifecycle and HEADERS/CONTINUATION
//! encoding come from these substrate crates; gRPC-bridge concentrates
//! on the gRPC application-layer wire format.
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`encode_message`] / [`decode_message`] / [`FrameError`] — gRPC
//!   Length-Prefixed-Message (LPM): Compressed-Flag (1 byte) +
//!   Message-Length (4-byte BE) + Message-Bytes.
//! - [`parse_path`] / [`PathError`] — `/<service>/<method>` from the HTTP
//!   `:path`.
//! - [`encode_timeout`] / [`decode_timeout`] / [`TimeoutUnit`] /
//!   [`TimeoutError`] — `grpc-timeout` Header value + unit
//!   (H/M/S/m/u/n).
//! - [`Status`] — all 17 gRPC status codes (0..=16).
//! - [`encode_header_value`] / [`decode_header_value`] /
//!   [`encode_base64`] / [`decode_base64`] / [`is_binary_header`] /
//!   [`BIN_SUFFIX`] / [`MetadataError`] — Custom-Metadata-Encoding
//!   (`-bin`-Suffix → Base64).
//! - [`request_headers`] / [`response_headers`] / [`content_types`]
//!   — standard header sets for request/response/gRPC-Web/JSON.
//! - [`GrpcServer`] / [`GrpcRequest`] / [`GrpcResponse`] — gRPC
//!   server skeleton for caller-configured HTTP/2 listeners.
//!
//! ## Example
//!
//! ```rust
//! use zerodds_grpc_bridge::{decode_message, encode_message};
//!
//! let msg = b"hello-grpc";
//! let wire = encode_message(msg, false).expect("encode");
//! let (flag, payload, consumed) = decode_message(&wire).expect("decode");
//! assert_eq!(flag, 0);
//! assert_eq!(payload, msg);
//! assert_eq!(consumed, wire.len());
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

// Bridge-Security-Adapter (Bridge-Spec §7.1 TLS via rustls/HTTP-2-ALPN,
// §7.2 Auth-Modes, §7.3 Topic-ACL). std-only.
#[cfg(feature = "std")]
pub mod bridge_security;
// Daemon-Runtime-Adapter + QoS-Translation. std-only.
#[cfg(feature = "std")]
pub mod daemon_runtime;
pub mod frame;
pub mod metadata;
pub mod path;
#[cfg(feature = "std")]
pub mod qos_translation;
pub mod reflection;
pub mod server;
pub mod service_gen;
pub mod status;
pub mod timeout;

pub use frame::{FrameError, decode_message, encode_message};
pub use metadata::{
    BIN_SUFFIX, MetadataError, content_types, decode_base64, decode_header_value, encode_base64,
    encode_header_value, is_binary_header, request_headers, response_headers,
};
pub use path::{PathError, parse_path};
pub use server::{GrpcRequest, GrpcResponse, GrpcServer};
pub use status::Status;
pub use timeout::{TimeoutError, TimeoutUnit, decode_timeout, encode_timeout};
