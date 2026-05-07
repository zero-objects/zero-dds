// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-monitor`. Safety classification: **STANDARD**.
//!
//! Observability-Substrate fuer ZeroDDS — Metric-Registry,
//! Prometheus-Text-Exporter, W3C-Trace-Context-PID-Codec, Span-Schema.
//!
//! Spec: `docs/specs/zerodds-monitor-1.0.md`.
//!
//! ## Schichten-Position
//!
//! Layer 4 — Core Services. Konsumenten-Pfad-Aufbau ueber den
//! Foundation-Tracing-Primitives ([`zerodds_foundation::tracing`]).
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`Counter`] / [`Gauge`] / [`LabeledHistogram`] — Metric-Typen.
//! - [`Labels`] — Schluessel/Wert-Paare fuer Metric-Identitaet.
//! - [`Registry`] / [`default_registry`] — Single-Source-of-Truth.
//! - [`render_prometheus`] — Prometheus-Text-Exposition.
//! - [`TraceContextPid`] — `PID_VENDOR_TRACE_CONTEXT` (0x0D00) Codec.
//! - [`MonitorConfig`] / [`TraceContextEmission`] — Lifecycle-Config.
//! - [`metric_names`] / [`span_names`] — Standard-Konstanten der 31
//!   Spec-Metrics + 9 Spec-Spans.
//! - [`serve_prometheus`] (Feature `prometheus-server`) — Mini-HTTP-
//!   `/metrics`-Endpoint.
//!
//! ## Beispiel
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

pub use config::{MonitorConfig, TraceContextEmission};

#[cfg(feature = "prometheus-server")]
pub use server::{ServeError, serve_prometheus};

/// Re-Export: das `foundation::tracing::Histogram` ist die Substrate-
/// Datenstruktur. Der monitor-Crate fuegt nur Labels + Registry-
/// Indirektion hinzu.
pub use zerodds_foundation::tracing::Histogram;
