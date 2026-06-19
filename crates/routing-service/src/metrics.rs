// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Per-route metrics, backed by the `zerodds-monitor` default registry so the
//! existing Prometheus text exporter renders them. Each counter carries a
//! `route` label.

use std::sync::Arc;

use zerodds_monitor::{Counter, Labels, default_registry};

/// `dds_router_samples_forwarded_total` — alive samples written to the output.
pub const SAMPLES_FORWARDED_TOTAL: &str = "dds_router_samples_forwarded_total";
/// `dds_router_samples_dropped_loop_total` — dropped by the loop guard.
pub const SAMPLES_DROPPED_LOOP_TOTAL: &str = "dds_router_samples_dropped_loop_total";
/// `dds_router_samples_dropped_filter_total` — dropped by the content filter.
pub const SAMPLES_DROPPED_FILTER_TOTAL: &str = "dds_router_samples_dropped_filter_total";
/// `dds_router_lifecycle_forwarded_total` — dispose/unregister events forwarded.
pub const LIFECYCLE_FORWARDED_TOTAL: &str = "dds_router_lifecycle_forwarded_total";
/// `dds_router_forward_errors_total` — output write/lifecycle failures.
pub const FORWARD_ERRORS_TOTAL: &str = "dds_router_forward_errors_total";

/// Counters for one route. Cloneable handle (the counters are `Arc`).
#[derive(Clone)]
pub struct RouteMetrics {
    forwarded: Arc<Counter>,
    dropped_loop: Arc<Counter>,
    dropped_filter: Arc<Counter>,
    lifecycle: Arc<Counter>,
    errors: Arc<Counter>,
}

impl RouteMetrics {
    /// Registers (or fetches) the labelled counters for `route` in the default
    /// registry.
    #[must_use]
    pub fn new(route: &str) -> Self {
        let reg = default_registry();
        reg.set_help(
            SAMPLES_FORWARDED_TOTAL,
            "Alive samples forwarded by the router.",
        );
        reg.set_help(
            SAMPLES_DROPPED_LOOP_TOTAL,
            "Samples dropped by the router loop guard.",
        );
        reg.set_help(
            SAMPLES_DROPPED_FILTER_TOTAL,
            "Samples dropped by the router content filter.",
        );
        reg.set_help(
            LIFECYCLE_FORWARDED_TOTAL,
            "Instance lifecycle events forwarded by the router.",
        );
        reg.set_help(FORWARD_ERRORS_TOTAL, "Router output write failures.");
        let lbl = || Labels::new().with("route", route.to_string());
        Self {
            forwarded: reg.counter(SAMPLES_FORWARDED_TOTAL, lbl()),
            dropped_loop: reg.counter(SAMPLES_DROPPED_LOOP_TOTAL, lbl()),
            dropped_filter: reg.counter(SAMPLES_DROPPED_FILTER_TOTAL, lbl()),
            lifecycle: reg.counter(LIFECYCLE_FORWARDED_TOTAL, lbl()),
            errors: reg.counter(FORWARD_ERRORS_TOTAL, lbl()),
        }
    }

    /// Increments forwarded-alive.
    pub fn inc_forwarded(&self) {
        self.forwarded.inc();
    }
    /// Increments loop-dropped.
    pub fn inc_dropped_loop(&self) {
        self.dropped_loop.inc();
    }
    /// Increments filter-dropped.
    pub fn inc_dropped_filter(&self) {
        self.dropped_filter.inc();
    }
    /// Increments lifecycle-forwarded.
    pub fn inc_lifecycle(&self) {
        self.lifecycle.inc();
    }
    /// Increments forward-errors.
    pub fn inc_errors(&self) {
        self.errors.inc();
    }

    /// Point-in-time snapshot (for tests/diagnostics).
    #[must_use]
    pub fn snapshot(&self) -> RouteMetricsSnapshot {
        RouteMetricsSnapshot {
            forwarded: self.forwarded.get(),
            dropped_loop: self.dropped_loop.get(),
            dropped_filter: self.dropped_filter.get(),
            lifecycle: self.lifecycle.get(),
            errors: self.errors.get(),
        }
    }
}

/// Immutable snapshot of [`RouteMetrics`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteMetricsSnapshot {
    /// Alive samples forwarded.
    pub forwarded: u64,
    /// Dropped by the loop guard.
    pub dropped_loop: u64,
    /// Dropped by the content filter.
    pub dropped_filter: u64,
    /// Lifecycle events forwarded.
    pub lifecycle: u64,
    /// Output errors.
    pub errors: u64,
}
