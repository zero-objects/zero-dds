// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-corba-cos-event`. Safety classification: **STANDARD**.
//!
//! OMG CosEventService 1.4 (`formal/2004-10-02`) voller Stack —
//! pure-Rust `no_std + alloc`, `forbid(unsafe_code)`. Implementiert:
//!
//! - **CosEventComm** (Spec §1.5): PushConsumer, PushSupplier,
//!   PullConsumer, PullSupplier mit Disconnect-Operations.
//! - **CosEventChannelAdmin** (Spec §1.6): EventChannel,
//!   ConsumerAdmin, SupplierAdmin, ProxyPushConsumer/Supplier,
//!   ProxyPullConsumer/Supplier.
//! - **CosTypedEventComm + CosTypedEventChannelAdmin** (Spec §2):
//!   TypedPushConsumer/Supplier mit `get_typed_consumer/supplier`-
//!   Operations.
//! - **AnyEvent**: opaque Event-Container fuer den Push-Modus.
//!
//! Spec: OMG CosEventService 1.4 (`formal/2004-10-02`) §1.5 + §1.6 + §2.
//!
//! ## Schichten-Position
//!
//! Layer 8 — CORBA-Stack (Tier-A). Caller-Layer (Daemon o.ae.)
//! konstruiert konkrete Channel-Instanzen, registriert Suppliers
//! und Consumers, und treibt die Connect/Disconnect-Lifecycle.
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`AnyEvent`] / [`Disconnected`] / [`ConnectError`] — Event-Body
//!   und Spec-§1.5-Errors.
//! - [`PushConsumer`] / [`PushSupplier`] / [`PullConsumer`] /
//!   [`PullSupplier`] — Spec §1.5 Trait-Surfaces.
//! - [`EventChannel`] / [`ConsumerAdmin`] / [`SupplierAdmin`] /
//!   [`ProxyPushConsumer`] / [`ProxyPushSupplier`] /
//!   [`ProxyPullConsumer`] / [`ProxyPullSupplier`] — Spec §1.6.
//! - [`TypedEventChannel`] / [`TypedPushConsumer`] /
//!   [`TypedPushSupplier`] — Spec §2.
//!
//! ## Beispiel
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
