// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MQTT↔DDS topic bridge.
//!
//! Bidirectional mapping:
//! * MQTT topic name → DDS topic name (slash separation is
//!   preserved; the DDS layer accepts that topics may also contain
//!   slashes — that is QoS-profile-validated by the caller).
//! * MQTT-QoS → DDS-Reliability + Durability:
//!   - QoS 0 → Best-Effort + Volatile
//!   - QoS 1 → Reliable + Volatile
//!   - QoS 2 → Reliable + TransientLocal
//! * MQTT-User-Properties → DDS-User-Data (Caller-Layer serialisiert).

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::broker::QoS;

/// DDS-Reliability-Kind. Spec DDS 1.4 §2.2.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DdsReliability {
    /// `BEST_EFFORT`.
    BestEffort,
    /// `RELIABLE`.
    Reliable,
}

/// DDS-Durability-Kind. Spec DDS 1.4 §2.2.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DdsDurability {
    /// `VOLATILE`.
    Volatile,
    /// `TRANSIENT_LOCAL`.
    TransientLocal,
    /// `TRANSIENT`.
    Transient,
    /// `PERSISTENT`.
    Persistent,
}

/// QoS-Mapping MQTT → DDS. Spec-Convention.
#[must_use]
pub fn mqtt_qos_to_dds(qos: QoS) -> (DdsReliability, DdsDurability) {
    match qos {
        QoS::AtMostOnce => (DdsReliability::BestEffort, DdsDurability::Volatile),
        QoS::AtLeastOnce => (DdsReliability::Reliable, DdsDurability::Volatile),
        QoS::ExactlyOnce => (DdsReliability::Reliable, DdsDurability::TransientLocal),
    }
}

/// QoS-Mapping DDS → MQTT.
#[must_use]
pub fn dds_qos_to_mqtt(rel: DdsReliability, dur: DdsDurability) -> QoS {
    match (rel, dur) {
        (DdsReliability::BestEffort, _) => QoS::AtMostOnce,
        (DdsReliability::Reliable, DdsDurability::Volatile) => QoS::AtLeastOnce,
        (DdsReliability::Reliable, _) => QoS::ExactlyOnce,
    }
}

/// MQTT topic name → DDS topic name (default: 1:1).
/// The caller can register a mapping override for special topics.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TopicMapper {
    overrides: BTreeMap<String, String>,
}

impl TopicMapper {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a 1:1 override.
    pub fn map(&mut self, mqtt_topic: &str, dds_topic: &str) {
        self.overrides
            .insert(mqtt_topic.to_string(), dds_topic.to_string());
    }

    /// MQTT topic → DDS topic.
    #[must_use]
    pub fn mqtt_to_dds(&self, mqtt_topic: &str) -> String {
        self.overrides
            .get(mqtt_topic)
            .cloned()
            .unwrap_or_else(|| mqtt_topic.to_string())
    }

    /// DDS topic → MQTT topic (reverse lookup; with multiple mappings
    /// the first match is returned, otherwise identity).
    #[must_use]
    pub fn dds_to_mqtt(&self, dds_topic: &str) -> String {
        self.overrides
            .iter()
            .find(|(_, v)| v.as_str() == dds_topic)
            .map(|(k, _)| k.clone())
            .unwrap_or_else(|| dds_topic.to_string())
    }
}

/// User-property forwarder — passes (key, value) strings through.
#[must_use]
pub fn forward_user_properties(props: &[(String, String)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (k, v) in props {
        out.extend_from_slice(k.as_bytes());
        out.push(b'=');
        out.extend_from_slice(v.as_bytes());
        out.push(b'\n');
    }
    out
}

/// Bridge statistics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BridgeStats {
    /// MQTT→DDS forwards.
    pub mqtt_to_dds: u64,
    /// DDS→MQTT forwards.
    pub dds_to_mqtt: u64,
}

/// MQTT-DDS bridge with topic mapper + stats.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MqttDdsBridge {
    /// Topic mapper.
    pub mapper: TopicMapper,
    /// Stats.
    pub stats: BridgeStats,
}

impl MqttDdsBridge {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Forwards an MQTT publish message to the DDS side. Returns the
    /// tuple (dds_topic, dds_reliability, dds_durability).
    pub fn forward_to_dds(
        &mut self,
        mqtt_topic: &str,
        qos: QoS,
    ) -> (String, DdsReliability, DdsDurability) {
        let (rel, dur) = mqtt_qos_to_dds(qos);
        let dds_topic = self.mapper.mqtt_to_dds(mqtt_topic);
        self.stats.mqtt_to_dds += 1;
        (dds_topic, rel, dur)
    }

    /// Forwards a DDS sample to the MQTT side. Returns (mqtt_topic, qos).
    pub fn forward_to_mqtt(
        &mut self,
        dds_topic: &str,
        rel: DdsReliability,
        dur: DdsDurability,
    ) -> (String, QoS) {
        let qos = dds_qos_to_mqtt(rel, dur);
        let mqtt_topic = self.mapper.dds_to_mqtt(dds_topic);
        self.stats.dds_to_mqtt += 1;
        (mqtt_topic, qos)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn qos_0_maps_to_best_effort_volatile() {
        assert_eq!(
            mqtt_qos_to_dds(QoS::AtMostOnce),
            (DdsReliability::BestEffort, DdsDurability::Volatile)
        );
    }

    #[test]
    fn qos_1_maps_to_reliable_volatile() {
        assert_eq!(
            mqtt_qos_to_dds(QoS::AtLeastOnce),
            (DdsReliability::Reliable, DdsDurability::Volatile)
        );
    }

    #[test]
    fn qos_2_maps_to_reliable_transient_local() {
        assert_eq!(
            mqtt_qos_to_dds(QoS::ExactlyOnce),
            (DdsReliability::Reliable, DdsDurability::TransientLocal)
        );
    }

    #[test]
    fn dds_to_mqtt_round_trip() {
        for q in [QoS::AtMostOnce, QoS::AtLeastOnce, QoS::ExactlyOnce] {
            let (rel, dur) = mqtt_qos_to_dds(q);
            assert_eq!(dds_qos_to_mqtt(rel, dur), q);
        }
    }

    #[test]
    fn topic_mapper_default_is_identity() {
        let m = TopicMapper::new();
        assert_eq!(m.mqtt_to_dds("a/b"), "a/b");
        assert_eq!(m.dds_to_mqtt("a/b"), "a/b");
    }

    #[test]
    fn topic_mapper_override_round_trip() {
        let mut m = TopicMapper::new();
        m.map("sensors/temp", "Temperature");
        assert_eq!(m.mqtt_to_dds("sensors/temp"), "Temperature");
        assert_eq!(m.dds_to_mqtt("Temperature"), "sensors/temp");
    }

    #[test]
    fn forward_user_properties_uses_kv_format() {
        let p = alloc::vec![("key".into(), "value".into())];
        let buf = forward_user_properties(&p);
        assert_eq!(buf, b"key=value\n");
    }

    #[test]
    fn bridge_forward_to_dds_increments_stats() {
        let mut b = MqttDdsBridge::new();
        let (topic, rel, _) = b.forward_to_dds("a", QoS::AtLeastOnce);
        assert_eq!(topic, "a");
        assert_eq!(rel, DdsReliability::Reliable);
        assert_eq!(b.stats.mqtt_to_dds, 1);
    }

    #[test]
    fn bridge_forward_to_mqtt_increments_stats() {
        let mut b = MqttDdsBridge::new();
        let (topic, qos) =
            b.forward_to_mqtt("T", DdsReliability::Reliable, DdsDurability::Volatile);
        assert_eq!(topic, "T");
        assert_eq!(qos, QoS::AtLeastOnce);
        assert_eq!(b.stats.dds_to_mqtt, 1);
    }
}
