// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-grpc-bridge`. Safety classification: **STANDARD**.
//!
//! gRPC-over-HTTP/2 + gRPC-Web Wire-Codec — pure-Rust `no_std +
//! alloc`, `forbid(unsafe_code)`. Implementiert die gRPC-spezifischen
//! Wire-Elemente die ueber dem HTTP/2-Stack sitzen: Length-Prefixed-
//! Message (LPM), Path-Parsing, Timeout-Header, Status-Codes,
//! Custom-Metadata-Encoding (inkl. `-bin`-Suffix-Konvention),
//! gRPC-Web-Trailer-Frames und ein gRPC-Server-Skeleton fuer
//! Caller-konfigurierte HTTP/2-Listener.
//!
//! Spec: gRPC HTTP/2 Protocol (Length-Prefixed Message + Path +
//! Timeout + Status + Custom-Metadata) + gRPC-Web Specification
//! (Trailer-Frame + Content-Types).
//!
//! ## Schichten-Position
//!
//! Layer 5 — Bridges. Sitzt auf [`zerodds-http2`](../zerodds_http2/index.html)
//! (RFC 9113 Framing + Stream-State + Flow-Control) und
//! [`zerodds-hpack`](../zerodds_hpack/index.html) (RFC 7541 Header-
//! Compression). HTTP/2-Connection-Lifecycle und HEADERS/CONTINUATION-
//! Encoding kommen aus diesen Substrat-Crates; gRPC-Bridge konzentriert
//! sich auf das gRPC-Application-Layer-Wire-Format.
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`encode_message`] / [`decode_message`] / [`FrameError`] — gRPC
//!   Length-Prefixed-Message (LPM): Compressed-Flag (1 byte) +
//!   Message-Length (4-byte BE) + Message-Bytes.
//! - [`parse_path`] / [`PathError`] — `/<service>/<method>` aus HTTP
//!   `:path`.
//! - [`encode_timeout`] / [`decode_timeout`] / [`TimeoutUnit`] /
//!   [`TimeoutError`] — `grpc-timeout` Header value + unit
//!   (H/M/S/m/u/n).
//! - [`Status`] — alle 17 gRPC Status-Codes (0..=16).
//! - [`encode_header_value`] / [`decode_header_value`] /
//!   [`encode_base64`] / [`decode_base64`] / [`is_binary_header`] /
//!   [`BIN_SUFFIX`] / [`MetadataError`] — Custom-Metadata-Encoding
//!   (`-bin`-Suffix → Base64).
//! - [`request_headers`] / [`response_headers`] / [`content_types`]
//!   — Standard-Header-Sets fuer Request/Response/gRPC-Web/JSON.
//! - [`GrpcServer`] / [`GrpcRequest`] / [`GrpcResponse`] — gRPC-
//!   Server-Skeleton fuer Caller-konfigurierte HTTP/2-Listener.
//!
//! ## Beispiel
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
