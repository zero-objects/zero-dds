// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Gauge — bidirektionaler i64 (Spec §1.3).

use std::sync::atomic::{AtomicI64, Ordering};

use crate::Labels;

/// Bidirectionally mutable i64 value.
#[derive(Debug)]
pub struct Gauge {
    name: &'static str,
    labels: Labels,
    value: AtomicI64,
}

impl Gauge {
    /// Constructor — starts at 0.
    #[must_use]
    pub fn new(name: &'static str, labels: Labels) -> Self {
        Self {
            name,
            labels,
            value: AtomicI64::new(0),
        }
    }

    /// Set the value.
    pub fn set(&self, v: i64) {
        self.value.store(v, Ordering::Relaxed);
    }

    /// Inkrement um 1.
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Dekrement um 1.
    pub fn dec(&self) {
        self.value.fetch_sub(1, Ordering::Relaxed);
    }

    /// Add (can be negative).
    pub fn add(&self, n: i64) {
        self.value.fetch_add(n, Ordering::Relaxed);
    }

    /// Current value.
    #[must_use]
    pub fn get(&self) -> i64 {
        self.value.load(Ordering::Relaxed)
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
    fn gauge_set_and_get() {
        let g = Gauge::new("x", Labels::new());
        g.set(42);
        assert_eq!(g.get(), 42);
        g.set(-3);
        assert_eq!(g.get(), -3);
    }

    #[test]
    fn gauge_inc_dec() {
        let g = Gauge::new("x", Labels::new());
        g.inc();
        g.inc();
        g.dec();
        assert_eq!(g.get(), 1);
    }

    #[test]
    fn gauge_add_negative() {
        let g = Gauge::new("x", Labels::new());
        g.add(10);
        g.add(-3);
        assert_eq!(g.get(), 7);
    }
}
