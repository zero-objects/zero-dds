// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Labels — Key/Value-Paare fuer Metric-Identitaet (Spec §1.5).

use std::cmp::Ordering;

/// Sortierte Key/Value-Paare. Keys sind `&'static str`, Values
/// `String`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Labels {
    pairs: Vec<(&'static str, String)>,
}

impl Labels {
    /// Leere Labels.
    #[must_use]
    pub fn new() -> Self {
        Self { pairs: Vec::new() }
    }

    /// Builder: ein weiteres Label setzen. Bei Duplikat-Key wird der
    /// vorherige Wert ersetzt — Idempotenz fuer Setter-Pattern.
    #[must_use]
    pub fn with(mut self, key: &'static str, value: impl Into<String>) -> Self {
        self.set(key, value);
        self
    }

    /// Setzt ein Label (mutation).
    pub fn set(&mut self, key: &'static str, value: impl Into<String>) {
        let value = value.into();
        match self.pairs.iter_mut().find(|(k, _)| *k == key) {
            Some(pair) => pair.1 = value,
            None => {
                self.pairs.push((key, value));
                self.pairs.sort_by(|a, b| a.0.cmp(b.0));
            }
        }
    }

    /// Iterator ueber die sortierten Paare.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &str)> + '_ {
        self.pairs.iter().map(|(k, v)| (*k, v.as_str()))
    }

    /// Anzahl der Paare.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// Leer?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
}

impl PartialOrd for Labels {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Labels {
    fn cmp(&self, other: &Self) -> Ordering {
        self.pairs.cmp(&other.pairs)
    }
}

/// Identifier-Tupel fuer Registry-Lookup. `(name, labels)` definiert
/// die Identitaet eines Metric-Items.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MetricKey {
    /// Statischer Metric-Name.
    pub name: &'static str,
    /// Labels (sortiert).
    pub labels: Labels,
}

impl MetricKey {
    /// Konstruktor.
    #[must_use]
    pub fn new(name: &'static str, labels: Labels) -> Self {
        Self { name, labels }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn empty_labels() {
        let l = Labels::new();
        assert!(l.is_empty());
        assert_eq!(l.len(), 0);
    }

    #[test]
    fn labels_sorted_by_key() {
        let l = Labels::new()
            .with("topic", "Foo")
            .with("domain_id", "0")
            .with("transport", "udp");
        let keys: Vec<&str> = l.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["domain_id", "topic", "transport"]);
    }

    #[test]
    fn labels_dedup_replaces_value() {
        let l = Labels::new().with("topic", "A").with("topic", "B");
        assert_eq!(l.len(), 1);
        let v: Vec<&str> = l.iter().map(|(_, v)| v).collect();
        assert_eq!(v, vec!["B"]);
    }

    #[test]
    fn metric_key_eq_uses_name_and_labels() {
        let k1 = MetricKey::new("dds_x", Labels::new().with("topic", "A"));
        let k2 = MetricKey::new("dds_x", Labels::new().with("topic", "A"));
        let k3 = MetricKey::new("dds_x", Labels::new().with("topic", "B"));
        assert_eq!(k1, k2);
        assert_ne!(k1, k3);
    }

    #[test]
    fn metric_key_hashable() {
        use std::collections::HashMap;
        let mut m = HashMap::new();
        m.insert(MetricKey::new("x", Labels::new()), 1);
        assert_eq!(m.get(&MetricKey::new("x", Labels::new())), Some(&1));
    }
}
