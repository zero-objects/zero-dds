// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! # `zerodds-amqp-0-9-1`
//!
//! Crate `zerodds-amqp-0-9-1`. Safety classification: **STANDARD**.
//!
//! AMQP **0.9.1** — the classic, broker-centric class/method protocol that
//! RabbitMQ speaks by default and that is still >80% of deployed AMQP. This is
//! an **entirely separate protocol** from AMQP 1.0 (`zerodds-amqp-bridge`):
//! different framing, a different type system (field tables, not described
//! types), and a broker model baked into the wire (exchanges/queues/bindings,
//! connection/channel/basic methods).
//!
//! Layers:
//! - [`types`] — big-endian wire types + typed field tables (§4.2.5).
//! - [`frame`] — the `type/channel/size/payload/0xCE` frame format (§4.2.2).
//! - [`method`] — class/method framing + content properties: connection,
//!   channel (open/flow/close), exchange (declare/delete), queue
//!   (declare/bind/unbind/purge/delete), basic (publish/get/consume/ack/
//!   reject/nack/qos), confirm (publisher confirms) and tx (transactions).
//! - [`client`] — a synchronous broker client (`std`).
//!
//! `no_std + alloc`; the `client` needs `std` (TCP).

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod frame;
pub mod method;
pub mod types;

#[cfg(feature = "std")]
pub mod client;

pub use frame::{Frame, FrameType, PROTOCOL_HEADER};
pub use method::ContentProperties;
pub use types::{FieldValue, Reader, WireError, Writer, pack_bits};
