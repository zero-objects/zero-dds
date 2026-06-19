// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! §6 — DDS QoS → MQTT behavior translation.
//!
//! Mapping per Spec `zerodds-mqtt-bridge-1.0.md` §6:
//!
//! * `Reliability::Reliable`   → MQTT QoS-1 (at-least-once); `BestEffort` → QoS-0.
//!   `Reliable` with `History::KeepAll` → QoS-2 (exactly-once).
//! * `Durability::Volatile`           → retain=false.
//! * `Durability::TransientLocal+`    → retain=true (broker-side cache).
//! * `Deadline`                       → ignored on the MQTT side
//!   (MQTT-5 has no deadline concept; tracking happens on the DDS side).

use zerodds_qos::{
    DurabilityKind, HistoryKind, HistoryQosPolicy, ReaderQos, ReliabilityKind, WriterQos,
};

/// MQTT behavior for a topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MqttBehavior {
    /// MQTT-QoS-Level (0/1/2).
    pub mqtt_qos: u8,
    /// Retain-Flag.
    pub retain: bool,
    /// Subscriber-Receive-Maximum (Flow-Control).
    pub receive_maximum: u16,
}

impl Default for MqttBehavior {
    fn default() -> Self {
        Self {
            mqtt_qos: 1,
            retain: false,
            receive_maximum: 65535,
        }
    }
}

impl MqttBehavior {
    /// Default behavior per `WriterQos::default()` (Reliable+Volatile).
    #[must_use]
    pub fn default_for_topic() -> Self {
        let w = WriterQos::default();
        let r = ReaderQos::default();
        dds_qos_to_mqtt_behavior(&w, &r)
    }
}

/// Hauptfunktion.
#[must_use]
pub fn dds_qos_to_mqtt_behavior(writer: &WriterQos, reader: &ReaderQos) -> MqttBehavior {
    let mqtt_qos = mqtt_qos_for(
        writer.reliability.kind,
        reader.reliability.kind,
        &writer.history,
    );
    let retain = matches!(
        writer.durability.kind,
        DurabilityKind::TransientLocal | DurabilityKind::Transient | DurabilityKind::Persistent,
    );
    let receive_maximum = match writer.history.kind {
        HistoryKind::KeepLast => (writer.history.depth.max(1) as u16).max(1),
        HistoryKind::KeepAll => 65535,
    };
    MqttBehavior {
        mqtt_qos,
        retain,
        receive_maximum,
    }
}

fn mqtt_qos_for(
    writer: ReliabilityKind,
    reader: ReliabilityKind,
    history: &HistoryQosPolicy,
) -> u8 {
    let any_reliable =
        matches!(writer, ReliabilityKind::Reliable) || matches!(reader, ReliabilityKind::Reliable);
    if !any_reliable {
        return 0;
    }
    if matches!(history.kind, HistoryKind::KeepAll) {
        2
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zerodds_qos::ReliabilityQosPolicy;

    #[test]
    fn best_effort_yields_qos0_no_retain() {
        let mut w = WriterQos::default();
        w.reliability = ReliabilityQosPolicy {
            kind: ReliabilityKind::BestEffort,
            ..w.reliability
        };
        let mut r = ReaderQos::default();
        r.reliability = ReliabilityQosPolicy {
            kind: ReliabilityKind::BestEffort,
            ..r.reliability
        };
        let b = dds_qos_to_mqtt_behavior(&w, &r);
        assert_eq!(b.mqtt_qos, 0);
        assert!(!b.retain);
    }

    #[test]
    fn reliable_keep_last_yields_qos1() {
        let w = WriterQos::default(); // Reliable + KeepLast(1) default.
        let r = ReaderQos::default();
        let b = dds_qos_to_mqtt_behavior(&w, &r);
        assert_eq!(b.mqtt_qos, 1);
    }

    #[test]
    fn reliable_keep_all_yields_qos2() {
        let mut w = WriterQos::default();
        w.history.kind = HistoryKind::KeepAll;
        let r = ReaderQos::default();
        let b = dds_qos_to_mqtt_behavior(&w, &r);
        assert_eq!(b.mqtt_qos, 2);
    }

    #[test]
    fn transient_local_yields_retain_true() {
        let mut w = WriterQos::default();
        w.durability.kind = DurabilityKind::TransientLocal;
        let r = ReaderQos::default();
        let b = dds_qos_to_mqtt_behavior(&w, &r);
        assert!(b.retain);
    }

    #[test]
    fn volatile_yields_retain_false() {
        let mut w = WriterQos::default();
        w.durability.kind = DurabilityKind::Volatile;
        let r = ReaderQos::default();
        let b = dds_qos_to_mqtt_behavior(&w, &r);
        assert!(!b.retain);
    }

    #[test]
    fn keep_last_uses_depth_for_receive_max() {
        let mut w = WriterQos::default();
        w.history.kind = HistoryKind::KeepLast;
        w.history.depth = 100;
        let r = ReaderQos::default();
        let b = dds_qos_to_mqtt_behavior(&w, &r);
        assert_eq!(b.receive_maximum, 100);
    }

    #[test]
    fn default_for_topic_yields_qos1_no_retain() {
        let b = MqttBehavior::default_for_topic();
        assert_eq!(b.mqtt_qos, 1);
        assert!(!b.retain);
    }
}
