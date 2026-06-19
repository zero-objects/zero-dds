// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Labeled histogram — wraps `foundation::tracing::Histogram` with labels (spec §1.4).

use std::sync::Mutex;

use zerodds_foundation::tracing::Histogram;

use crate::Labels;

/// Histogram + labels + mutex protection for parallel `record_ns` calls.
#[derive(Debug)]
pub struct LabeledHistogram {
    name: &'static str,
    labels: Labels,
    inner: Mutex<Histogram>,
}

impl LabeledHistogram {
    /// Constructor.
    #[must_use]
    pub fn new(name: &'static str, labels: Labels) -> Self {
        Self {
            name,
            labels,
            inner: Mutex::new(Histogram::new(name)),
        }
    }

    /// Measures a value in nanoseconds.
    pub fn record_ns(&self, ns: u64) {
        if let Ok(mut h) = self.inner.lock() {
            h.record_ns(ns);
        }
    }

    /// Measures a value in seconds (converted internally to ns).
    pub fn record_seconds(&self, seconds: f64) {
        let ns = (seconds * 1.0e9).max(0.0) as u64;
        self.record_ns(ns);
    }

    /// Snapshot of the current histogram state (clone).
    #[must_use]
    pub fn snapshot(&self) -> Histogram {
        self.inner
            .lock()
            .map(|h| h.clone())
            .unwrap_or_else(|_| Histogram::new(self.name))
    }

    /// Metric-Name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Labels.
    #[must_use]
    pub fn labels(&self) -> &Labels {
        &self.labels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn histogram_records_ns() {
        let h = LabeledHistogram::new("x", Labels::new());
        h.record_ns(500);
        h.record_ns(1_500_000);
        let s = h.snapshot();
        assert_eq!(s.count, 2);
        assert_eq!(s.sum_ns, 500 + 1_500_000);
        assert_eq!(s.min_ns, 500);
        assert_eq!(s.max_ns, 1_500_000);
    }

    #[test]
    fn histogram_records_seconds_converts() {
        let h = LabeledHistogram::new("x", Labels::new());
        h.record_seconds(0.001); // 1ms = 1_000_000 ns
        let s = h.snapshot();
        assert_eq!(s.count, 1);
        assert_eq!(s.sum_ns, 1_000_000);
    }
}
