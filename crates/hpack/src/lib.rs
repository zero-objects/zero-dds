// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-hpack`. Safety classification: **STANDARD**.
//!
//! HPACK (RFC 7541) Header-Compression-Codec fuer HTTP/2 — no_std,
//! `forbid(unsafe_code)`. Implementiert die Wire-Schicht aus
//! Variable-Length-Integer-Coding (§5.1), String-Literal-Coding
//! (§5.2) inkl. statischem Huffman-Code (Appendix B), Static- und
//! Dynamic-Table (§2.3, §4) sowie alle vier Header-Field-
//! Repraesentationen (§6: Indexed / Literal-with-Indexing /
//! Literal-without-Indexing / Literal-Never-Indexed).
//!
//! Spec: RFC 7541 §5 (Primitive Type Representations) + §6 (Binary
//! Format).
//!
//! ## Schichten-Position
//!
//! Layer 5 — Bridges. Substrat fuer:
//!
//! - [`zerodds-http2`](../zerodds_http2/index.html) — RFC 9113
//!   Framing + Stream-State-Machine.
//! - [`zerodds-grpc-bridge`](../zerodds_grpc_bridge/index.html) —
//!   gRPC-over-HTTP/2 Length-Prefixed-Message-Codec.
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`Encoder`] / [`EncoderError`] — High-Level-Encoder mit eigener
//!   Dynamic-Table.
//! - [`Decoder`] / [`DecoderError`] — High-Level-Decoder mit eigener
//!   Dynamic-Table und Size-Update-Handling.
//! - [`Table`] / [`HeaderField`] / [`STATIC_TABLE`] /
//!   [`StaticTableEntry`] — Static- + Dynamic-Table-Modelle (§2.3).
//! - [`encode_integer`] / [`decode_integer`] — Variable-Length-
//!   Integer-Coding-Primitive (§5.1).
//! - [`encode_string`] / [`decode_string`] — String-Literal-Coding-
//!   Primitive (§5.2) mit optionalem Huffman-Pfad.
//! - [`huffman::encode`] / [`huffman::decode`] /
//!   [`huffman::HuffmanError`] — Static-Huffman-Code aus Appendix B
//!   (Feature-gated nicht; ist fuer no_std erforderlich).
//! - [`integer::IntegerError`] / [`string::StringError`] —
//!   Modul-private Fehler-Typen, ueber die Module-Pfade exposed.
//!
//! ## Beispiel
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
