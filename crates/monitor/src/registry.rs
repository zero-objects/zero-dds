// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Registry — Single-Source-of-Truth fuer Counter/Gauge/Histogram (Spec §1.6).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::{Counter, Gauge, LabeledHistogram, Labels, MetricKey};

/// Zentrale Registry. Idempotente Lookup + spaeterer Render-Pass.
#[derive(Debug, Default)]
pub struct Registry {
    counters: Mutex<HashMap<MetricKey, Arc<Counter>>>,
    gauges: Mutex<HashMap<MetricKey, Arc<Gauge>>>,
    histograms: Mutex<HashMap<MetricKey, Arc<LabeledHistogram>>>,
    helps: Mutex<HashMap<&'static str, &'static str>>,
}

impl Registry {
    /// Leere Registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Counter holen — bei Wiederholungs-Aufruf mit demselben
    /// `(name, labels)` wird die selbe Instanz zurueckgegeben.
    pub fn counter(&self, name: &'static str, labels: Labels) -> Arc<Counter> {
        let key = MetricKey::new(name, labels.clone());
        if let Ok(mut map) = self.counters.lock() {
            if let Some(existing) = map.get(&key) {
                return Arc::clone(existing);
            }
            let c = Arc::new(Counter::new(name, labels));
            map.insert(key, Arc::clone(&c));
            c
        } else {
            // Mutex poisoned — silent recovery: liefere eine isolierte
            // Instance, damit Hot-Path nicht panict. Sub-optimal weil
            // nicht in der Registry gespeichert; dafuer panic-frei.
            Arc::new(Counter::new(name, labels))
        }
    }

    /// Gauge holen.
    pub fn gauge(&self, name: &'static str, labels: Labels) -> Arc<Gauge> {
        let key = MetricKey::new(name, labels.clone());
        if let Ok(mut map) = self.gauges.lock() {
            if let Some(existing) = map.get(&key) {
                return Arc::clone(existing);
            }
            let g = Arc::new(Gauge::new(name, labels));
            map.insert(key, Arc::clone(&g));
            g
        } else {
            Arc::new(Gauge::new(name, labels))
        }
    }

    /// Histogram holen.
    pub fn histogram(&self, name: &'static str, labels: Labels) -> Arc<LabeledHistogram> {
        let key = MetricKey::new(name, labels.clone());
        if let Ok(mut map) = self.histograms.lock() {
            if let Some(existing) = map.get(&key) {
                return Arc::clone(existing);
            }
            let h = Arc::new(LabeledHistogram::new(name, labels));
            map.insert(key, Arc::clone(&h));
            h
        } else {
            Arc::new(LabeledHistogram::new(name, labels))
        }
    }

    /// HELP-Text fuer einen Metric-Namen registrieren (fuer Prometheus-
    /// Exposition). Ein Set pro Metric-Name; Re-Registrierung
    /// ueberschreibt den vorherigen Help-Text.
    pub fn set_help(&self, name: &'static str, help: &'static str) {
        if let Ok(mut map) = self.helps.lock() {
            map.insert(name, help);
        }
    }

    /// Snapshot — alle aktuellen Metric-Werte einfrieren fuer
    /// Render/Export.
    #[must_use]
    pub fn snapshot(&self) -> RegistrySnapshot {
        let counters = self
            .counters
            .lock()
            .map(|m| m.iter().map(|(k, c)| (k.clone(), c.get())).collect())
            .unwrap_or_default();
        let gauges = self
            .gauges
            .lock()
            .map(|m| m.iter().map(|(k, g)| (k.clone(), g.get())).collect())
            .unwrap_or_default();
        let histograms = self
            .histograms
            .lock()
            .map(|m| m.iter().map(|(k, h)| (k.clone(), h.snapshot())).collect())
            .unwrap_or_default();
        let helps = self
            .helps
            .lock()
            .map(|m| m.iter().map(|(k, v)| (*k, *v)).collect())
            .unwrap_or_default();
        RegistrySnapshot {
            counters,
            gauges,
            histograms,
            helps,
        }
    }

    /// Prometheus-Text-Format-Render — Convenience-Wrapper um
    /// [`crate::render_prometheus`].
    #[must_use]
    pub fn render_prometheus(&self) -> String {
        crate::render_prometheus(&self.snapshot())
    }
}

/// Eingefrorener Registry-State fuer Export.
#[derive(Clone, Debug, Default)]
pub struct RegistrySnapshot {
    /// Counter-Werte.
    pub counters: Vec<(MetricKey, u64)>,
    /// Gauge-Werte.
    pub gauges: Vec<(MetricKey, i64)>,
    /// Histogram-Snapshots.
    pub histograms: Vec<(MetricKey, zerodds_foundation::tracing::Histogram)>,
    /// HELP-Texte pro Metric-Name.
    pub helps: Vec<(&'static str, &'static str)>,
}

static DEFAULT_REGISTRY: OnceLock<Arc<Registry>> = OnceLock::new();

/// Globale Default-Registry (initialisiert beim ersten Aufruf).
#[must_use]
pub fn default_registry() -> Arc<Registry> {
    Arc::clone(DEFAULT_REGISTRY.get_or_init(|| Arc::new(Registry::new())))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn registry_returns_same_counter_for_same_key() {
        let r = Registry::new();
        let c1 = r.counter("x", Labels::new().with("topic", "A"));
        let c2 = r.counter("x", Labels::new().with("topic", "A"));
        c1.inc();
        assert_eq!(c2.get(), 1, "registry must return same instance");
    }

    #[test]
    fn registry_distinct_labels_distinct_counters() {
        let r = Registry::new();
        let c1 = r.counter("x", Labels::new().with("topic", "A"));
        let c2 = r.counter("x", Labels::new().with("topic", "B"));
        c1.inc();
        assert_eq!(c2.get(), 0);
    }

    #[test]
    fn snapshot_captures_all_three_kinds() {
        let r = Registry::new();
        r.counter("c", Labels::new()).add(5);
        r.gauge("g", Labels::new()).set(7);
        r.histogram("h", Labels::new()).record_ns(100);
        let s = r.snapshot();
        assert_eq!(s.counters.len(), 1);
        assert_eq!(s.gauges.len(), 1);
        assert_eq!(s.histograms.len(), 1);
        assert_eq!(s.counters[0].1, 5);
        assert_eq!(s.gauges[0].1, 7);
        assert_eq!(s.histograms[0].1.count, 1);
    }

    #[test]
    fn default_registry_is_singleton() {
        let r1 = default_registry();
        let r2 = default_registry();
        r1.counter("dds_test_singleton_total", Labels::new()).inc();
        assert_eq!(
            r2.counter("dds_test_singleton_total", Labels::new()).get(),
            1
        );
    }
}
