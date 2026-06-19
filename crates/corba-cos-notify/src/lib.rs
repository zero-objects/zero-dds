// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-corba-cos-notify`. Safety classification: **STANDARD**.
//!
//! OMG **CosNotification 1.1** (formal/2004-10-11) — the structured-event
//! notification service, successor to CosEvent.
//!
//! In-memory model (analogous to [`zerodds_corba_cos_event`], but for
//! `StructuredEvent` with an `any` payload):
//!
//! - [`event`] §2.2 — `StructuredEvent` / `EventType` / `Property`.
//! - [`comm`] §3 — structured push/pull consumer/supplier traits + publish/subscribe.
//! - [`channel`] §4 — `EventChannelFactory` / `EventChannel` / `ConsumerAdmin` /
//!   `SupplierAdmin` / structured-proxy hierarchy (push fanout + pull queue).
//! - [`filter`] §5 — `Filter` / `ConstraintExp` / `FilterFactory` (event-type match).
//! - [`qos`] §2.5 — QoS/admin property bag.
//!
//! Pure Rust, `forbid(unsafe_code)`. Live wiring over GIOP (analogous to the
//! CosEvent EventBus e2e) is the job of the interop layer.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

pub mod channel;
pub mod comm;
pub mod event;
pub mod filter;
pub mod qos;

pub use channel::{
    ConsumerAdmin, EventChannel, EventChannelFactory, StructuredProxyPullSupplier,
    StructuredProxyPushConsumer, StructuredProxyPushSupplier, SupplierAdmin,
};
pub use comm::{
    ConnectError, Disconnected, NotifyPublish, NotifySubscribe, StructuredPullConsumer,
    StructuredPullSupplier, StructuredPushConsumer, StructuredPushSupplier,
};
pub use event::{EventHeader, EventType, FixedEventHeader, Property, PropertySeq, StructuredEvent};
pub use filter::{ConstraintExp, Filter, FilterFactory, InstalledConstraint};
pub use qos::QoSProperties;
