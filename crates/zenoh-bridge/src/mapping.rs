// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Pure-Rust-Mapping zwischen DDS-Topic-Namen und Zenoh-Key-Expressions.
//! Lebt ohne `zenoh`-Dep, damit das Workspace-CI ohne externe Deps
//! gegen die Bridge-Logik testen kann.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_qos::{DurabilityKind, ReliabilityKind};

/// Zenoh-Key-Expression-Konventionen.
#[allow(dead_code)] // nur unter feature `zenoh-runtime` als Builder-Default genutzt
pub const DEFAULT_PREFIX: &str = "dds";

/// Bildet einen DDS-Topic-Namen (mit optionalen Partition-Tags) auf eine
/// Zenoh-Key-Expression ab. Reservierte DDS-Sonderzeichen (`*`, `?`, `[`,
/// `]`) werden mit `_` substituiert — Zenoh-KeyExpr-Spec verbietet sie.
///
/// Layout: `<prefix>/<partition>/<topic>` (Partition leer → ohne Slot).
///
/// # Beispiele
/// ```
/// use zerodds_zenoh_bridge::key_expr_for_topic;
/// assert_eq!(key_expr_for_topic("dds", "", "Chatter"), "dds/Chatter");
/// assert_eq!(key_expr_for_topic("dds", "robot1", "Sensor"), "dds/robot1/Sensor");
/// assert_eq!(key_expr_for_topic("dds", "", "Foo*Bar"), "dds/Foo_Bar");
/// ```
#[must_use]
pub fn key_expr_for_topic(prefix: &str, partition: &str, topic: &str) -> String {
    let safe_topic = sanitize(topic);
    if partition.is_empty() {
        alloc::format!("{prefix}/{safe_topic}")
    } else {
        let safe_part = sanitize(partition);
        alloc::format!("{prefix}/{safe_part}/{safe_topic}")
    }
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '*' | '?' | '[' | ']' | '$' | '#' => '_',
            other => other,
        })
        .collect()
}

/// Zenoh-Reliability-Aequivalent zu einer DDS-Reliability-QoS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZenohReliability {
    /// Reliable end-to-end (Zenoh: `Reliability::Reliable`).
    Reliable,
    /// Best-Effort (Zenoh: `Reliability::BestEffort`).
    BestEffort,
}

/// Zenoh-CongestionControl-Aequivalent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZenohCongestion {
    /// Block (Spec-Aequivalent zu DDS Reliable + max_blocking_time).
    Block,
    /// Drop (Spec-Aequivalent zu DDS BestEffort + Volatile).
    Drop,
}

/// Mapped einen DDS-QoS-Tupel auf Zenoh-Reliability + Congestion-Control.
///
/// # Ableitung
/// - DDS Reliable → Zenoh Reliable.
/// - DDS BestEffort → Zenoh BestEffort.
/// - DDS Durability=TransientLocal/Transient/Persistent → Block (Zenoh
///   schickt nicht ohne Subscriber-Window — TransientLocal-Aequivalent
///   ist Zenoh's `Storage`-Plugin, hier nur Block-Pressure).
/// - DDS Durability=Volatile → Drop.
#[must_use]
pub fn dds_qos_to_zenoh(
    reliability: ReliabilityKind,
    durability: DurabilityKind,
) -> (ZenohReliability, ZenohCongestion) {
    let rel = match reliability {
        ReliabilityKind::Reliable => ZenohReliability::Reliable,
        ReliabilityKind::BestEffort => ZenohReliability::BestEffort,
    };
    let cong = match durability {
        DurabilityKind::Volatile => ZenohCongestion::Drop,
        _ => ZenohCongestion::Block,
    };
    (rel, cong)
}

/// Topic-Map: bidirektionale Zuordnung DDS-Topic ↔ Zenoh-KeyExpr.
#[derive(Debug, Default, Clone)]
pub struct TopicMap {
    entries: Vec<TopicMapEntry>,
}

/// Einzelner Topic-Eintrag.
#[derive(Debug, Clone)]
pub struct TopicMapEntry {
    /// DDS-Topic-Name.
    pub topic: String,
    /// DDS-Type-Name.
    pub type_name: String,
    /// Zenoh-Key-Expression.
    pub key_expr: String,
    /// DDS-Reliability.
    pub reliability: ReliabilityKind,
    /// DDS-Durability.
    pub durability: DurabilityKind,
}

impl TopicMap {
    /// Neue, leere Map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Fuegt einen Eintrag hinzu.
    pub fn add(&mut self, entry: TopicMapEntry) {
        self.entries.push(entry);
    }

    /// Schaut einen Eintrag nach DDS-Topic-Namen nach.
    #[must_use]
    pub fn by_topic(&self, topic: &str) -> Option<&TopicMapEntry> {
        self.entries.iter().find(|e| e.topic == topic)
    }

    /// Schaut einen Eintrag nach Zenoh-Key-Expression nach.
    #[must_use]
    pub fn by_key_expr(&self, key_expr: &str) -> Option<&TopicMapEntry> {
        self.entries.iter().find(|e| e.key_expr == key_expr)
    }

    /// Liefert alle Eintraege.
    #[must_use]
    pub fn entries(&self) -> &[TopicMapEntry] {
        &self.entries
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn key_expr_no_partition() {
        assert_eq!(key_expr_for_topic("dds", "", "Chatter"), "dds/Chatter");
    }

    #[test]
    fn key_expr_with_partition() {
        assert_eq!(
            key_expr_for_topic("dds", "robot1", "Sensor"),
            "dds/robot1/Sensor"
        );
    }

    #[test]
    fn key_expr_sanitizes_wildcards() {
        // Zenoh erlaubt `*` als Wildcard im Subscribe-Pattern, aber
        // als Topic-Name-Bestandteil ist es ambig — wir ersetzen mit `_`.
        assert_eq!(key_expr_for_topic("dds", "", "Foo*Bar"), "dds/Foo_Bar");
        assert_eq!(key_expr_for_topic("dds", "", "Topic[1]"), "dds/Topic_1_");
    }

    #[test]
    fn reliability_mapping() {
        let (r, c) = dds_qos_to_zenoh(ReliabilityKind::Reliable, DurabilityKind::Volatile);
        assert_eq!(r, ZenohReliability::Reliable);
        assert_eq!(c, ZenohCongestion::Drop);

        let (r, c) = dds_qos_to_zenoh(ReliabilityKind::BestEffort, DurabilityKind::TransientLocal);
        assert_eq!(r, ZenohReliability::BestEffort);
        assert_eq!(c, ZenohCongestion::Block);
    }

    #[test]
    fn topic_map_lookup() {
        let mut m = TopicMap::new();
        m.add(TopicMapEntry {
            topic: "Chatter".into(),
            type_name: "std_msgs::String".into(),
            key_expr: "dds/Chatter".into(),
            reliability: ReliabilityKind::Reliable,
            durability: DurabilityKind::Volatile,
        });
        assert!(m.by_topic("Chatter").is_some());
        assert!(m.by_key_expr("dds/Chatter").is_some());
        assert!(m.by_topic("Unknown").is_none());
    }
}
