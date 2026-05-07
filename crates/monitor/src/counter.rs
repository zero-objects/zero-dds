// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Counter — monotoner u64 (Spec §1.2).

use std::sync::atomic::{AtomicU64, Ordering};

use crate::Labels;

/// Monoton wachsender u64-Zaehler.
#[derive(Debug)]
pub struct Counter {
    name: &'static str,
    labels: Labels,
    value: AtomicU64,
}

impl Counter {
    /// Konstruktor — startet bei 0.
    #[must_use]
    pub fn new(name: &'static str, labels: Labels) -> Self {
        Self {
            name,
            labels,
            value: AtomicU64::new(0),
        }
    }

    /// Inkrement um 1.
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Inkrement um `n`.
    pub fn add(&self, n: u64) {
        self.value.fetch_add(n, Ordering::Relaxed);
    }

    /// Aktueller Wert.
    #[must_use]
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Metric-Name (Spec §2).
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
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn counter_starts_zero() {
        let c = Counter::new("x", Labels::new());
        assert_eq!(c.get(), 0);
    }

    #[test]
    fn counter_inc_and_add() {
        let c = Counter::new("x", Labels::new());
        c.inc();
        c.inc();
        c.add(10);
        assert_eq!(c.get(), 12);
    }

    #[test]
    fn counter_concurrent_increments_are_consistent() {
        let c = Arc::new(Counter::new("x", Labels::new()));
        let mut handles = Vec::new();
        for _ in 0..8 {
            let c = Arc::clone(&c);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    c.inc();
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(c.get(), 8 * 1000);
    }
}
