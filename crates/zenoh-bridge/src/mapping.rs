// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Pure-Rust mapping between DDS topic names and Zenoh key expressions.
//! Lives without the `zenoh` dep so the workspace CI can test the
//! bridge logic without external deps.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_qos::{DurabilityKind, ReliabilityKind};

/// Zenoh key-expression conventions.
#[allow(dead_code)] // only used under the `zenoh-runtime` feature as a builder default
pub const DEFAULT_PREFIX: &str = "dds";

/// Maps a DDS topic name (with optional partition tags) to a
/// Zenoh key expression. Reserved DDS special characters (`*`, `?`, `[`,
/// `]`) are substituted with `_` — the Zenoh KeyExpr spec forbids them.
///
/// Layout: `<prefix>/<partition>/<topic>` (empty partition → no slot).
///
/// # Examples
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

/// Zenoh reliability equivalent of a DDS reliability QoS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZenohReliability {
    /// Reliable end-to-end (Zenoh: `Reliability::Reliable`).
    Reliable,
    /// Best-Effort (Zenoh: `Reliability::BestEffort`).
    BestEffort,
}

/// Zenoh CongestionControl equivalent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZenohCongestion {
    /// Block (spec equivalent of DDS Reliable + max_blocking_time).
    Block,
    /// Drop (spec equivalent of DDS BestEffort + Volatile).
    Drop,
}

/// Maps a DDS QoS tuple to Zenoh reliability + congestion control.
///
/// # Derivation
/// - DDS Reliable → Zenoh Reliable.
/// - DDS BestEffort → Zenoh BestEffort.
/// - DDS Durability=TransientLocal/Transient/Persistent → Block (Zenoh
///   does not send without a subscriber window — the TransientLocal equivalent
///   is Zenoh's `Storage` plugin, here only block pressure).
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

/// Topic map: bidirectional mapping DDS topic ↔ Zenoh KeyExpr.
#[derive(Debug, Default, Clone)]
pub struct TopicMap {
    entries: Vec<TopicMapEntry>,
}

/// A single topic entry.
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
    /// New, empty map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an entry.
    pub fn add(&mut self, entry: TopicMapEntry) {
        self.entries.push(entry);
    }

    /// Looks up an entry by DDS topic name.
    #[must_use]
    pub fn by_topic(&self, topic: &str) -> Option<&TopicMapEntry> {
        self.entries.iter().find(|e| e.topic == topic)
    }

    /// Looks up an entry by Zenoh key expression.
    #[must_use]
    pub fn by_key_expr(&self, key_expr: &str) -> Option<&TopicMapEntry> {
        self.entries.iter().find(|e| e.key_expr == key_expr)
    }

    /// Returns all entries.
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
        // Zenoh allows `*` as a wildcard in the subscribe pattern, but
        // as part of a topic name it is ambiguous — we replace it with `_`.
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
