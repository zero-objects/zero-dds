// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! # ZeroDDS routing service
//!
//! A standalone DDS routing service: it forwards samples between DDS domains,
//! topics, QoS profiles and partitions **within** the DDS bus — the in-DDS
//! routing counterpart to the protocol bridges (which cross protocol
//! boundaries). It is the RTI-Routing-Service equivalent for ZeroDDS.
//!
//! Safety classification: **COMFORT**. Part of [**ZeroDDS**](../../README.md).
//!
//! ## What it does
//!
//! A [`config::Route`] couples an input [`config::Endpoint`] (a reader on a
//! domain/topic) to an output endpoint (a writer on a — possibly different —
//! domain/topic), with:
//!
//! * **Topic renaming** — input topic `A` → output topic `B`.
//! * **Domain bridging** — read on domain 0, write on domain 1, in one process.
//! * **QoS mapping** — per-endpoint reliability / durability / ownership /
//!   partition / data-representation.
//! * **Type-agnostic byte forwarding** — the CDR body is forwarded verbatim
//!   (representation-faithful: XCDR1/XCDR2 encapsulation preserved); no type
//!   recompilation, cross-vendor.
//! * **Keyed-instance lifecycle propagation** — dispose / unregister events are
//!   forwarded for keyed topics.
//! * **Loop guard** — on any domain that is both an input and an output
//!   domain, the router's input participant and output participant are made
//!   mutually invisible (`ignore_participant`), so no router output is ever
//!   re-ingested by a router input on the same domain — preventing
//!   router-internal loops (bidirectional bridges, multi-route cycles) while
//!   application writers stay visible.
//! * **Content filtering** (v3) — a DDS SQL filter (`temp > 50 AND zone = 'A'`)
//!   evaluated per sample against the decoded body.
//! * **Field transformation** (v3) — rename / constant-set / drop members,
//!   mapping an input shape to an output shape.
//!
//! ## Surfaces
//!
//! * Library — [`Router::start`] / [`Router::start_with_types`].
//! * Config — JSON ([`config::RouterConfig::from_json`]) or XML
//!   ([`config::RouterConfig::from_xml`], native + RTI subset).
//! * Daemon — the `zerodds-router` binary.
//! * Metrics — per-route counters in the `zerodds-monitor` registry (rendered
//!   by the existing Prometheus exporter).
//!
//! ## Limitations
//!
//! * **`preserve_source_timestamp`** is parsed but rejected at route build: the
//!   runtime user byte-path does not surface a sample's source timestamp, so
//!   the router cannot reproduce it. Wiring `source_timestamp` through
//!   `UserSample` + a timestamped write is the prerequisite follow-up.
//! * **Filter / transform** operate on flat `@final` / `@appendable` structs of
//!   scalar and string members (the common routing case), decoded
//!   little-endian. Nested structs, sequences, unions and `@mutable` member
//!   headers are a documented extension; byte pass-through routing has no such
//!   restriction.
//! * **Loop guard** covers router-*internal* loops (within one process). Loops
//!   spanning two separate `zerodds-router` processes need a discovery
//!   discriminator (a router tag in `user_data`), which the DCPS builtin
//!   publication reader does not currently surface — a documented follow-up.

#![forbid(unsafe_code)]

pub mod config;
pub mod engine;
pub mod error;
pub mod forwarding;
pub mod metrics;
pub mod transform;
mod xml;

pub use config::{
    ContentFilter, Durability, Endpoint, Ownership, QosSpec, RenameRule, Route, RouterConfig,
    SetConstRule, Transform,
};
pub use engine::Router;
pub use error::{Result, RoutingError};
pub use metrics::{RouteMetrics, RouteMetricsSnapshot};
pub use transform::{Member, ScalarKind, TypeRegistry, TypeShape};
