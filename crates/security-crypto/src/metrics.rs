// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Hot-Path-Hook-Points fuer `zerodds-monitor` (zerodds-monitor-1.0 §2.5).
//!
//! Existiert nur unter `cfg(feature = "metrics")`. Call-Sites in
//! `plugin.rs` tragen ein eigenes `#[cfg(feature = "metrics")]`-Attribut.

use std::sync::Arc;
use std::time::Instant;

use zerodds_monitor::{Counter, LabeledHistogram, Labels, default_registry, metric_names};

fn op_counter(operation: &'static str) -> Arc<Counter> {
    let r = default_registry();
    r.set_help(
        metric_names::DDS_SECURITY_CRYPTO_OPERATIONS_TOTAL,
        "Crypto-Operationen (zerodds-monitor-1.0 §2.5)",
    );
    r.counter(
        metric_names::DDS_SECURITY_CRYPTO_OPERATIONS_TOTAL,
        Labels::new().with("operation", operation),
    )
}

fn op_histogram(operation: &'static str) -> Arc<LabeledHistogram> {
    let r = default_registry();
    r.set_help(
        metric_names::DDS_SECURITY_CRYPTO_LATENCY_SECONDS,
        "Crypto-Latency (zerodds-monitor-1.0 §2.5)",
    );
    r.histogram(
        metric_names::DDS_SECURITY_CRYPTO_LATENCY_SECONDS,
        Labels::new().with("operation", operation),
    )
}

/// RAII-Span um eine Crypto-Operation: bei Drop werden Counter +
/// Histogramm aktualisiert.
pub struct CryptoOp {
    operation: &'static str,
    start: Instant,
}

impl CryptoOp {
    /// Startet einen Crypto-Op-Tracker.
    pub fn start(operation: &'static str) -> Self {
        Self {
            operation,
            start: Instant::now(),
        }
    }
}

impl Drop for CryptoOp {
    fn drop(&mut self) {
        op_counter(self.operation).inc();
        let elapsed = self.start.elapsed();
        let ns = elapsed.as_nanos().min(u64::MAX as u128) as u64;
        op_histogram(self.operation).record_ns(ns);
    }
}
