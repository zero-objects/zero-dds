// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-amqp-bridge`. Safety classification: **STANDARD**.
//!
//! AMQP 1.0 (OASIS Standard) Wire-Codec — pure-Rust no_std + alloc,
//! `forbid(unsafe_code)`. Implementiert das vollstaendige AMQP-1.0
//! Type-System, Frame-Format, alle 9 Performatives und alle 7
//! Message-Sections, plus den DDS-AMQP-1.0 Codec-/Codec-Lite-Profile-
//! Marker.
//!
//! Spec-Referenzen:
//!
//! - **OASIS AMQP 1.0** Part 1 (Types), Part 2 (Transport), Part 3
//!   (Messaging), Part 4 (Transactions), Part 5 (Security).
//! - **OMG DDS-AMQP 1.0 (formal/2024-08-01)** §2.3 (Codec Profile),
//!   §2.4 (Codec-Lite Profile), §6.1 (Direct-Embed Topology), §7
//!   (Type-System-Mapping), §8 (Message-Section-Mapping).
//!
//! ## Schichten-Position
//!
//! Layer 5 — Bridges. Substrat fuer:
//!
//! - [`zerodds-amqp-endpoint`](../zerodds_amqp_endpoint/index.html) —
//!   DDS-AMQP-1.0 Endpoint-Layer mit Direct-Embed-Topology.
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! **Type-System (`types`-Modul):**
//!
//! - [`AmqpValue`] / [`FormatCode`] / [`TypeError`].
//! - [`encode_null`] / [`encode_boolean`] / [`encode_long`] /
//!   [`encode_ulong`] / [`encode_string`] / [`encode_symbol`] /
//!   [`encode_binary`] — Primitiv-Encoder.
//! - [`decode_value`] — Variant-Decoder ueber alle Format-Codes.
//!
//! **Extended Types (`extended_types`-Modul):**
//!
//! - [`AmqpExtValue`] — Vollstaendiges Variant-Modell (Primitive +
//!   Compound: list / map / array).
//! - Encoder/Decoder fuer ubyte / ushort / uint / byte / short / int /
//!   float / double / char / decimal32-64-128 / timestamp / uuid.
//!
//! **Frame-Format (`frame`-Modul):**
//!
//! - [`FrameHeader`] / [`FrameType`] / [`FrameError`].
//! - [`encode_frame_header`] / [`decode_frame_header`] — 4-Byte SIZE
//!   BE + DOFF + TYPE + CHANNEL BE + Extended-Header.
//!
//! **Performatives (`performatives`-Modul):**
//!
//! - [`open`] / [`begin`] / [`attach`] / [`flow`] / [`transfer`] /
//!   [`disposition`] / [`detach`] / [`end`] / [`close`] — die 9
//!   Transport-Performatives.
//! - [`encode_performative`] / [`decode_performative`].
//!
//! **Message-Sections (`sections`-Modul):**
//!
//! - [`MessageSection`] — Header / Delivery-Annotations / Message-
//!   Annotations / Properties / Application-Properties / AmqpValue /
//!   AmqpSequence / Data / Footer.
//! - [`validate_section_sequence`] — Reihenfolge-Pruefung gemaess
//!   AMQP-1.0 Messaging §3.2.
//!
//! **Codec-Profile (`codec_profile`-Modul):**
//!
//! - `CodecProfile::{Full, Lite}`.
//! - `active_profile()` (const) — `Full` per Default, `Lite` mit
//!   Cargo-Feature `codec-lite`.
//! - `is_codec_lite_value` / `is_codec_lite_section` — Conformance-
//!   Pruefer fuer DDS-AMQP-1.0 §2.4.
//!
//! ## Beispiel
//!
//! ```rust
//! use zerodds_amqp_bridge::{decode_value, encode_long, AmqpValue};
//!
//! let buf = encode_long(42);
//! let (v, consumed) = decode_value(&buf).expect("decode");
//! assert_eq!(v, AmqpValue::Long(42));
//! assert_eq!(consumed, buf.len());
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod codec_profile;
pub mod extended_types;
pub mod frame;
pub mod performatives;
pub mod sections;
pub mod types;

pub use extended_types::{
    AmqpExtValue, decode_byte, decode_char, decode_double, decode_float, decode_int, decode_short,
    decode_timestamp, decode_ubyte, decode_uint, decode_ushort, decode_uuid, encode_byte,
    encode_char, encode_decimal32, encode_decimal64, encode_decimal128, encode_double,
    encode_float, encode_int, encode_short, encode_timestamp, encode_ubyte, encode_uint,
    encode_ushort, encode_uuid,
};
pub use frame::{FrameError, FrameHeader, FrameType, decode_frame_header, encode_frame_header};
pub use performatives::{
    attach, begin, close, decode_performative, detach, disposition, encode_performative, end, flow,
    open, transfer,
};
pub use sections::{MessageSection, validate_section_sequence};
pub use types::{
    AmqpValue, FormatCode, TypeError, decode_value, encode_binary, encode_boolean, encode_long,
    encode_null, encode_string, encode_symbol, encode_ulong,
};
