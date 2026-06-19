// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-monitor`. Safety classification: **STANDARD**.
//!
//! Observability substrate for ZeroDDS — metric registry,
//! Prometheus text exporter, W3C trace-context PID codec, span schema.
//!
//! Spec: `docs/specs/zerodds-monitor-1.1.md`.
//!
//! ## Layer position
//!
//! Layer 4 — core services. The consumer path builds on the
//! foundation tracing primitives ([`zerodds_foundation::tracing`]).
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! - [`Counter`] / [`Gauge`] / [`LabeledHistogram`] — metric types.
//! - [`Labels`] — key/value pairs for metric identity.
//! - [`Registry`] / [`default_registry`] — single source of truth.
//! - [`render_prometheus`] — Prometheus text exposition.
//! - [`TraceContextPid`] — `PID_VENDOR_TRACE_CONTEXT` (0x0D00) codec.
//! - [`MonitorConfig`] / [`TraceContextEmission`] — lifecycle config.
//! - [`metric_names`] / [`span_names`] — standard constants of the 31
//!   spec metrics + 9 spec spans.
//! - [`serve_prometheus`] (Feature `prometheus-server`) — Mini-HTTP-
//!   `/metrics`-Endpoint.
//!
//! ## Example
//!
//! ```rust
//! use std::sync::Arc;
//! use zerodds_monitor::{default_registry, Labels, metric_names};
//!
//! let reg = default_registry();
//! let counter = reg.counter(
//!     metric_names::DDS_DCPS_SAMPLES_WRITTEN_TOTAL,
//!     Labels::new().with("topic", "VehicleTracking.TrackUpdate"),
//! );
//! counter.inc();
//! counter.add(5);
//! assert_eq!(counter.get(), 6);
//!
//! let text = reg.render_prometheus();
//! assert!(text.contains("dds_dcps_samples_written_total"));
//! ```

#![warn(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

mod counter;
mod gauge;
mod histogram;
mod labels;
pub mod metric_names;
mod prometheus;
mod registry;
pub mod span_names;
mod trace_context;

#[cfg(feature = "prometheus-server")]
mod server;

mod config;

pub use counter::Counter;
pub use gauge::Gauge;
pub use histogram::LabeledHistogram;
pub use labels::{Labels, MetricKey};
pub use prometheus::render_prometheus;
pub use registry::{Registry, RegistrySnapshot, default_registry};
pub use trace_context::{
    PID_VENDOR_TRACE_CONTEXT, TraceContextError, TraceContextPid, TraceParent, TraceState,
};

pub use config::{
    GuidLabelPolicy, MonitorConfig, TopicLabelPolicy, TraceContextEmission, active_config,
    guid_label, set_config, topic_label,
};

#[cfg(feature = "prometheus-server")]
pub use server::{ServeError, serve_prometheus};

/// Re-export: `foundation::tracing::Histogram` is the substrate data
/// structure. The monitor crate only adds labels + registry
/// indirection.
pub use zerodds_foundation::tracing::Histogram;
