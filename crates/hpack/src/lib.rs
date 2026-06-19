// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-hpack`. Safety classification: **STANDARD**.
//!
//! HPACK (RFC 7541) header-compression codec for HTTP/2 — no_std,
//! `forbid(unsafe_code)`. Implements the wire layer of
//! variable-length integer coding (§5.1), string-literal coding
//! (§5.2) including the static Huffman code (Appendix B), the static
//! and dynamic table (§2.3, §4), as well as all four header-field
//! representations (§6: Indexed / Literal-with-Indexing /
//! Literal-without-Indexing / Literal-Never-Indexed).
//!
//! Spec: RFC 7541 §5 (Primitive Type Representations) + §6 (Binary
//! Format).
//!
//! ## Layer position
//!
//! Layer 5 — Bridges. Substrate for:
//!
//! - [`zerodds-http2`](../zerodds_http2/index.html) — RFC 9113
//!   Framing + Stream-State-Machine.
//! - [`zerodds-grpc-bridge`](../zerodds_grpc_bridge/index.html) —
//!   gRPC-over-HTTP/2 Length-Prefixed-Message-Codec.
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! - [`Encoder`] / [`EncoderError`] — high-level encoder with its own
//!   dynamic table.
//! - [`Decoder`] / [`DecoderError`] — high-level decoder with its own
//!   dynamic table and size-update handling.
//! - [`Table`] / [`HeaderField`] / [`STATIC_TABLE`] /
//!   [`StaticTableEntry`] — static + dynamic table models (§2.3).
//! - [`encode_integer`] / [`decode_integer`] — variable-length
//!   integer coding primitives (§5.1).
//! - [`encode_string`] / [`decode_string`] — string-literal coding
//!   primitives (§5.2) with an optional Huffman path.
//! - [`huffman::encode`] / [`huffman::decode`] /
//!   [`huffman::HuffmanError`] — static Huffman code from Appendix B
//!   (not feature-gated; required for no_std).
//! - [`integer::IntegerError`] / [`string::StringError`] —
//!   module-private error types, exposed via the module paths.
//!
//! ## Example
//!
//! ```rust
//! use zerodds_hpack::{Decoder, Encoder, HeaderField};
//!
//! let mut encoder = Encoder::new();
//! let mut decoder = Decoder::new();
//!
//! let headers = vec![
//!     HeaderField { name: ":method".into(), value: "GET".into() },
//!     HeaderField { name: ":scheme".into(), value: "https".into() },
//!     HeaderField { name: "custom-key".into(), value: "custom-value".into() },
//! ];
//!
//! let wire = encoder.encode(&headers);
//! let decoded = decoder.decode(&wire).expect("roundtrip");
//! assert_eq!(decoded, headers);
//! ```

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod decoder;
pub mod encoder;
pub mod huffman;
pub mod integer;
pub mod string;
pub mod table;

pub use decoder::{Decoder, DecoderError};
pub use encoder::{Encoder, EncoderError};
pub use integer::{decode_integer, encode_integer};
pub use string::{decode_string, encode_string};
pub use table::{HeaderField, STATIC_TABLE, StaticTableEntry, Table};
