// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-corba-cos-event`. Safety classification: **STANDARD**.
//!
//! OMG CosEventService 1.4 (`formal/2004-10-02`) full stack —
//! pure Rust `no_std + alloc`, `forbid(unsafe_code)`. Implements:
//!
//! - **CosEventComm** (spec §1.5): PushConsumer, PushSupplier,
//!   PullConsumer, PullSupplier with disconnect operations.
//! - **CosEventChannelAdmin** (spec §1.6): EventChannel,
//!   ConsumerAdmin, SupplierAdmin, ProxyPushConsumer/Supplier,
//!   ProxyPullConsumer/Supplier.
//! - **CosTypedEventComm + CosTypedEventChannelAdmin** (spec §2):
//!   TypedPushConsumer/Supplier with `get_typed_consumer/supplier`
//!   operations.
//! - **AnyEvent**: opaque event container for the push mode.
//!
//! Spec: OMG CosEventService 1.4 (`formal/2004-10-02`) §1.5 + §1.6 + §2.
//!
//! ## Layer position
//!
//! Layer 8 — CORBA stack (Tier-A). The caller layer (a daemon or similar)
//! constructs concrete channel instances, registers suppliers
//! and consumers, and drives the connect/disconnect lifecycle.
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! - [`AnyEvent`] / [`Disconnected`] / [`ConnectError`] — event body
//!   and spec §1.5 errors.
//! - [`PushConsumer`] / [`PushSupplier`] / [`PullConsumer`] /
//!   [`PullSupplier`] — spec §1.5 trait surfaces.
//! - [`EventChannel`] / [`ConsumerAdmin`] / [`SupplierAdmin`] /
//!   [`ProxyPushConsumer`] / [`ProxyPushSupplier`] /
//!   [`ProxyPullConsumer`] / [`ProxyPullSupplier`] — Spec §1.6.
//! - [`TypedEventChannel`] / [`TypedPushConsumer`] /
//!   [`TypedPushSupplier`] — Spec §2.
//!
//! ## Example
//!
//! ```rust
//! use zerodds_corba_cos_event::{AnyEvent, EventChannel};
//!
//! let _channel = EventChannel::new();
//! let event = AnyEvent::new("IDL:Foo:1.0".to_string(), vec![1, 2, 3]);
//! assert_eq!(event.type_id, "IDL:Foo:1.0");
//! assert_eq!(event.data, vec![1, 2, 3]);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod channel;
pub mod comm;
pub mod typed;

pub use channel::{
    ConsumerAdmin, EventChannel, ProxyPullConsumer, ProxyPullSupplier, ProxyPushConsumer,
    ProxyPushSupplier, SupplierAdmin,
};
pub use comm::{
    AnyEvent, ConnectError, Disconnected, PullConsumer, PullSupplier, PushConsumer, PushSupplier,
};
pub use typed::{TypedEventChannel, TypedPushConsumer, TypedPushSupplier};
