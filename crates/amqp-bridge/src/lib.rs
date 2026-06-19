// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-amqp-bridge`. Safety classification: **STANDARD**.
//!
//! AMQP 1.0 (OASIS Standard) wire codec — pure Rust no_std + alloc,
//! `forbid(unsafe_code)`. Implements the full AMQP 1.0
//! type system, frame format, all 9 performatives and all 7
//! message sections, plus the DDS-AMQP-1.0 codec/codec-lite profile
//! marker.
//!
//! Spec references:
//!
//! - **OASIS AMQP 1.0** Part 1 (Types), Part 2 (Transport), Part 3
//!   (Messaging), Part 4 (Transactions), Part 5 (Security).
//! - **OMG DDS-AMQP 1.0 (formal/2024-08-01)** §2.3 (Codec Profile),
//!   §2.4 (Codec-Lite Profile), §6.1 (Direct-Embed Topology), §7
//!   (type-system mapping), §8 (message-section mapping).
//!
//! ## Layer position
//!
//! Layer 5 — bridges. Substrate for:
//!
//! - [`zerodds-amqp-endpoint`](../zerodds_amqp_endpoint/index.html) —
//!   DDS-AMQP-1.0 endpoint layer with direct-embed topology.
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! **Type system (`types` module):**
//!
//! - [`AmqpValue`] / [`FormatCode`] / [`TypeError`].
//! - [`encode_null`] / [`encode_boolean`] / [`encode_long`] /
//!   [`encode_ulong`] / [`encode_string`] / [`encode_symbol`] /
//!   [`encode_binary`] — primitive encoders.
//! - [`decode_value`] — variant decoder over all format codes.
//!
//! **Extended Types (`extended_types` module):**
//!
//! - [`AmqpExtValue`] — full variant model (primitives +
//!   compound: list / map / array).
//! - Encoder/decoder for ubyte / ushort / uint / byte / short / int /
//!   float / double / char / decimal32-64-128 / timestamp / uuid.
//!
//! **Frame format (`frame` module):**
//!
//! - [`FrameHeader`] / [`FrameType`] / [`FrameError`].
//! - [`encode_frame_header`] / [`decode_frame_header`] — 4-byte SIZE
//!   BE + DOFF + TYPE + CHANNEL BE + extended header.
//!
//! **Performatives (`performatives` module):**
//!
//! - [`open`] / [`begin`] / [`attach`] / [`flow`] / [`transfer`] /
//!   [`disposition`] / [`detach`] / [`end`] / [`close`] — the 9
//!   transport performatives.
//! - [`encode_performative`] / [`decode_performative`].
//!
//! **Message sections (`sections` module):**
//!
//! - [`MessageSection`] — Header / Delivery-Annotations / Message-
//!   Annotations / Properties / Application-Properties / AmqpValue /
//!   AmqpSequence / Data / Footer.
//! - [`validate_section_sequence`] — order check per
//!   AMQP-1.0 Messaging §3.2.
//!
//! **Codec profile (`codec_profile` module):**
//!
//! - `CodecProfile::{Full, Lite}`.
//! - `active_profile()` (const) — `Full` by default, `Lite` with
//!   the `codec-lite` Cargo feature.
//! - `is_codec_lite_value` / `is_codec_lite_section` — conformance
//!   checkers for DDS-AMQP-1.0 §2.4.
//!
//! ## Example
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
    attach, attach_to, begin, close, data_payload_from_transfer, decode_performative, descriptor,
    detach, disposition, disposition_accept, encode_performative, end, flow, flow_link_credit,
    open, sasl_init, sasl_mechanisms_from_body, sasl_outcome_code, sasl_plain_response,
    sasl_response, transfer, transfer_message,
};
pub use sections::{MessageSection, validate_section_sequence};
pub use types::{
    AmqpValue, FormatCode, TypeError, decode_value, encode_binary, encode_boolean, encode_long,
    encode_null, encode_string, encode_symbol, encode_ulong,
};
