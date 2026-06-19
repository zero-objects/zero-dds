// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Hot-path hook points for `zerodds-monitor` (zerodds-monitor-1.1 §2.5).
//!
//! Exists only under `cfg(feature = "metrics")`. Call sites in
//! `plugin.rs` carry their own `#[cfg(feature = "metrics")]` attribute.

use std::sync::Arc;
use std::time::Instant;

use zerodds_monitor::{Counter, LabeledHistogram, Labels, default_registry, metric_names};

fn op_counter(operation: &'static str) -> Arc<Counter> {
    let r = default_registry();
    r.set_help(
        metric_names::DDS_SECURITY_CRYPTO_OPERATIONS_TOTAL,
        "Crypto operations (zerodds-monitor-1.1 §2.5)",
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
        "Crypto-Latency (zerodds-monitor-1.1 §2.5)",
    );
    r.histogram(
        metric_names::DDS_SECURITY_CRYPTO_LATENCY_SECONDS,
        Labels::new().with("operation", operation),
    )
}

/// RAII span around a crypto operation: on drop the counter +
/// histogram are updated.
pub struct CryptoOp {
    operation: &'static str,
    start: Instant,
}

impl CryptoOp {
    /// Starts a crypto-op tracker.
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
