// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-coap-bridge`. Safety classification: **STANDARD**.
//!
//! CoAP (RFC 7252) wire codec + reliability + discovery + bridge —
//! pure Rust `no_std + alloc`, `forbid(unsafe_code)`. Beyond the bare
//! wire codec it also implements all the accompanying CoAP layers:
//! reliability/congestion control (§4.2-§4.7), request/response
//! matching (§5.3), block-wise transfer (RFC 7959), resource discovery
//! in CoRE-Link format (RFC 6690), observe registry (RFC 7641),
//! multicast discovery, caching/proxying (§5.6 + §5.7), DTLS mode
//! marker (§9), and a bidirectional CoAP↔DDS topic bridge.
//!
//! Spec references:
//!
//! - **RFC 7252** — CoAP (Constrained Application Protocol).
//! - **RFC 7641** — Observing Resources in CoAP.
//! - **RFC 7959** — Block-Wise Transfer.
//! - **RFC 6690** — CoRE-Link-Format.
//!
//! ## Layer position
//!
//! Layer 5 — bridges. Substrate for DDS↔IoT endpoint mapping
//! (constrained devices, OPC-UA-PubSub mapping, ROS-2-on-constrained
//! bridges).
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! - [`CoapMessage`] / [`CoapCode`] / [`MessageType`] — message model
//!   (§3 + §12.1).
//! - [`encode`] / [`decode`] / [`CodecError`] — wire codec.
//! - [`CoapOption`] / [`OptionNumber`] / [`OptionValue`] — options
//!   with delta encoding + extended length (§3.1, §5.10).
//! - [`BlockOption`] / [`BlockValue`] / [`BlockReassembler`] /
//!   [`BlockError`] — block-wise transfer (RFC 7959).
//! - [`CoreLink`] / [`encode_links`] / [`decode_links`] — resource
//!   discovery (RFC 6690).
//! - [`OBSERVE_OPTION_NUMBER`] / [`ObserveRegistry`] /
//!   [`ObserverEntry`] — RFC 7641 observer state.
//! - [`PendingConfirmable`] / [`ReliabilityTracker`] / [`TickOutput`] /
//!   [`ACK_TIMEOUT_MS`] / [`MAX_RETRANSMIT`] — reliability §4.
//! - [`CoapDdsBridge`] / [`BridgeOp`] / [`BridgeError`] /
//!   [`map_method`] / [`parse_dds_path`] — CoAP↔DDS topic mapping.
//!
//! ## Example
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
#[cfg(feature = "dtls")]
pub mod dtls_transport;
pub mod matching;
pub mod message;
pub mod method_props;
pub mod multicast;
pub mod observe;
pub mod option;
#[cfg(feature = "oscore")]
pub mod oscore;
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
