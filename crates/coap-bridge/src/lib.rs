// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-coap-bridge`. Safety classification: **STANDARD**.
//!
//! CoAP (RFC 7252) Wire-Codec + Reliability + Discovery + Bridge —
//! pure-Rust `no_std + alloc`, `forbid(unsafe_code)`. Implementiert
//! neben dem reinen Wire-Codec auch alle CoAP-Begleit-Schichten:
//! Reliability/Congestion-Control (§4.2-§4.7), Request/Response-
//! Matching (§5.3), Block-Wise-Transfer (RFC 7959), Resource-Discovery
//! im CoRE-Link-Format (RFC 6690), Observe-Registry (RFC 7641),
//! Multicast-Diskovery, Caching/Proxying (§5.6 + §5.7), DTLS-Mode-
//! Marker (§9) und einen Bidirectional-CoAP↔DDS-Topic-Bridge.
//!
//! Spec-Referenzen:
//!
//! - **RFC 7252** — CoAP (Constrained Application Protocol).
//! - **RFC 7641** — Observing Resources in CoAP.
//! - **RFC 7959** — Block-Wise Transfer.
//! - **RFC 6690** — CoRE-Link-Format.
//!
//! ## Schichten-Position
//!
//! Layer 5 — Bridges. Substrat fuer DDS↔IoT-Endpoint-Mapping
//! (Constrained-Devices, OPC-UA-PubSub-Mapping, ROS-2-on-Constrained-
//! Bridges).
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`CoapMessage`] / [`CoapCode`] / [`MessageType`] — Message-Modell
//!   (§3 + §12.1).
//! - [`encode`] / [`decode`] / [`CodecError`] — Wire-Codec.
//! - [`CoapOption`] / [`OptionNumber`] / [`OptionValue`] — Options
//!   mit Delta-Encoding + Extended-Length (§3.1, §5.10).
//! - [`BlockOption`] / [`BlockValue`] / [`BlockReassembler`] /
//!   [`BlockError`] — Block-Wise-Transfer (RFC 7959).
//! - [`CoreLink`] / [`encode_links`] / [`decode_links`] — Resource-
//!   Discovery (RFC 6690).
//! - [`OBSERVE_OPTION_NUMBER`] / [`ObserveRegistry`] /
//!   [`ObserverEntry`] — RFC 7641 Observer-State.
//! - [`PendingConfirmable`] / [`ReliabilityTracker`] / [`TickOutput`] /
//!   [`ACK_TIMEOUT_MS`] / [`MAX_RETRANSMIT`] — Reliability §4.
//! - [`CoapDdsBridge`] / [`BridgeOp`] / [`BridgeError`] /
//!   [`map_method`] / [`parse_dds_path`] — CoAP↔DDS-Topic-Mapping.
//!
//! ## Beispiel
//!
//! ```rust
//! use zerodds_coap_bridge::{decode, encode, CoapCode, CoapMessage, MessageType};
//!
//! let msg = CoapMessage {
//!     version: 1,
//!     message_type: MessageType::Confirmable,
//!     token: Vec::new(),
//!     code: CoapCode::GET,
//!     message_id: 0xBEEF,
//!     options: Vec::new(),
//!     payload: Vec::new(),
//! };
//!
//! let wire = encode(&msg).expect("encode");
//! let decoded = decode(&wire).expect("decode");
//! assert_eq!(decoded.code, CoapCode::GET);
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

pub mod blockwise;
pub mod bridge;
pub mod caching_proxy;
pub mod codec;
pub mod core_link;
#[cfg(feature = "daemon")]
pub mod daemon;
pub mod dtls;
pub mod matching;
pub mod message;
pub mod method_props;
pub mod multicast;
pub mod observe;
pub mod option;
pub mod reliability;
pub mod uri;

pub use blockwise::{BlockError, BlockOption, BlockReassembler, BlockValue};
pub use bridge::{BridgeError, BridgeOp, CoapDdsBridge, map_method, parse_dds_path};
pub use codec::{CodecError, decode, encode};
pub use core_link::{CoreLink, decode_links, encode_links};
pub use message::{CoapCode, CoapMessage, MessageType};
pub use observe::{OBSERVE_OPTION_NUMBER, ObserveRegistry, ObserverEntry};
pub use option::{CoapOption, OptionNumber, OptionValue};
pub use reliability::{
    ACK_TIMEOUT_MS, MAX_RETRANSMIT, PendingConfirmable, ReliabilityTracker, TickOutput,
};
