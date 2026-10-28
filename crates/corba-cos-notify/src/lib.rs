// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-corba-cos-notify`. Safety classification: **STANDARD**.
//!
//! OMG **CosNotification 1.1** (formal/2004-10-11) — the structured-event
//! notification service, successor to CosEvent.
//!
//! In-memory model (analogous to `zerodds_corba_cos_event`, but for
//! `StructuredEvent` with an `any` payload):
//!
//! - [`event`] §2.2 — `StructuredEvent` / `EventType` / `Property`.
//! - [`comm`] §3 — structured push/pull consumer/supplier traits + publish/subscribe.
//! - [`channel`] §4 — `EventChannelFactory` / `EventChannel` / `ConsumerAdmin` /
//!   `SupplierAdmin` / structured-proxy hierarchy (push fanout + pull queue).
//!   **`std` only**, see below.
//! - [`filter`] §5 — `Filter` / `ConstraintExp` / `FilterFactory` (event-type
//!   match). **`std` only**, see below.
//! - [`qos`] §2.5 — QoS/admin property bag.
//!
//! Pure Rust, `forbid(unsafe_code)`. Live wiring over GIOP (analogous to the
//! CosEvent EventBus e2e) is the job of the interop layer.
//!
//! ## What `alloc` alone gives
//!
//! [`event`], [`comm`] and [`qos`] are `no_std + alloc`. [`channel`] and
//! [`filter`] hold shared mutable state behind a `std::sync::Mutex` and
//! are behind the `std` feature.
//!
//! They used to be behind nothing: both named `std::sync::Mutex` in a
//! crate declared `#![no_std]`, so `--no-default-features --features
//! alloc` did not compile at all while the manifest and this page both
//! offered it. A feature that cannot be built is worse than one that is
//! not offered — nothing depends on it, so nothing reports it.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
pub mod channel;
pub mod comm;
pub mod event;
#[cfg(feature = "std")]
pub mod filter;
pub mod qos;

#[cfg(feature = "std")]
pub use channel::{
    ConsumerAdmin, EventChannel, EventChannelFactory, StructuredProxyPullSupplier,
    StructuredProxyPushConsumer, StructuredProxyPushSupplier, SupplierAdmin,
};
pub use comm::{
    ConnectError, Disconnected, NotifyPublish, NotifySubscribe, StructuredPullConsumer,
    StructuredPullSupplier, StructuredPushConsumer, StructuredPushSupplier,
};
pub use event::{EventHeader, EventType, FixedEventHeader, Property, PropertySeq, StructuredEvent};
#[cfg(feature = "std")]
pub use filter::{ConstraintExp, Filter, FilterFactory, InstalledConstraint};
pub use qos::QoSProperties;
