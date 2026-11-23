// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The topics a PUBLISH of the broker is written into — Spec §4.3, §5.1.
//!
//! Each `in` or `bidir` topic is a subscription of the daemon. A Topic
//! Filter gets one SUBSCRIBE and one Subscription Identifier (MQTT 5.0
//! §3.8.2.1.2), counted from 1 in the order of the configuration; topics
//! with the same filter share it. A PUBLISH of the broker goes to the
//! topics its Subscription Identifiers name (MQTT 5.0 §3.3.4), each of whose
//! filter also matches its Topic Name, so that an identifier of an earlier
//! configuration that a resumed session still holds reaches no other topic.
//! A PUBLISH without an identifier (MQTT 3.1.1, a broker without
//! Subscription Identifiers) goes to every topic whose filter matches its
//! Topic Name (MQTT 5.0 §4.7).

use std::string::String;
use std::vec::Vec;

use super::config::TopicConfig;
use crate::topic_filter::matches;

/// One subscription of the daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    /// The Topic Filter (`mqtt_topic` of its topics).
    pub filter: String,
    /// Its Subscription Identifier, from 1.
    pub id: u32,
    /// The MQTT QoS it asks for: the highest of its topics (Spec §6).
    pub qos: u8,
    /// The DDS topics a PUBLISH matching it is written into.
    pub dds_names: Vec<String>,
    /// Whether the broker granted it; only a granted subscription routes.
    pub granted: bool,
}

/// The subscriptions of one broker connection and the routing of the
/// broker's PUBLISH packets to DDS topics.
#[derive(Debug, Clone, Default)]
pub struct InboundRoutes {
    subscriptions: Vec<Subscription>,
}

impl InboundRoutes {
    /// The subscriptions of the `in` and `bidir` topics of `topics` for
    /// which `allowed` says yes (the ACL read check of Spec §7.3).
    #[must_use]
    pub fn of_topics(
        topics: &[TopicConfig],
        mut allowed: impl FnMut(&TopicConfig) -> bool,
    ) -> Self {
        let mut subscriptions: Vec<Subscription> = Vec::new();
        for t in topics {
            if !matches!(t.direction.as_str(), "in" | "bidir") || !allowed(t) {
                continue;
            }
            let qos = t.effective_mqtt_qos();
            if let Some(s) = subscriptions.iter_mut().find(|s| s.filter == t.mqtt_topic) {
                s.qos = s.qos.max(qos);
                if !s.dds_names.contains(&t.dds_name) {
                    s.dds_names.push(t.dds_name.clone());
                }
                continue;
            }
            let id = u32::try_from(subscriptions.len() + 1).unwrap_or(u32::MAX);
            subscriptions.push(Subscription {
                filter: t.mqtt_topic.clone(),
                id,
                qos,
                dds_names: vec![t.dds_name.clone()],
                granted: false,
            });
        }
        Self { subscriptions }
    }

    /// The subscriptions, in the order of their identifiers.
    #[must_use]
    pub fn subscriptions(&self) -> &[Subscription] {
        &self.subscriptions
    }

    /// Marks the subscription of `filter` granted or refused by the broker's
    /// SUBACK.
    pub fn set_granted(&mut self, filter: &str, granted: bool) {
        if let Some(s) = self.subscriptions.iter_mut().find(|s| s.filter == filter) {
            s.granted = granted;
        }
    }

    /// The DDS topics a PUBLISH on `topic` with the Subscription Identifiers
    /// `ids` is written into, each once, in the order of the configuration.
    #[must_use]
    pub fn route(&self, topic: &str, ids: &[u32]) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for s in &self.subscriptions {
            if !s.granted || !matches(&s.filter, topic) {
                continue;
            }
            if !ids.is_empty() && !ids.contains(&s.id) {
                continue;
            }
            for d in &s.dds_names {
                if !out.contains(&d.as_str()) {
                    out.push(d);
                }
            }
        }
        out
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn topic(dds: &str, mqtt: &str, direction: &str) -> TopicConfig {
        TopicConfig {
            dds_name: dds.into(),
            dds_type: dds.into(),
            mqtt_topic: mqtt.into(),
            direction: direction.into(),
            ..TopicConfig::default()
        }
    }

    fn granted(topics: &[TopicConfig]) -> InboundRoutes {
        let mut r = InboundRoutes::of_topics(topics, |_| true);
        let filters: Vec<String> = r.subscriptions().iter().map(|s| s.filter.clone()).collect();
        for f in filters {
            r.set_granted(&f, true);
        }
        r
    }

    /// Spec §5.1: a `+` filter takes every topic of one level in its place,
    /// and no other.
    #[test]
    fn mqttbridge_5_1_a_plus_filter_maps_its_topics() {
        let r = granted(&[topic("Telemetry", "factory/+/telemetry", "in")]);
        assert_eq!(r.route("factory/press-1/telemetry", &[]), ["Telemetry"]);
        assert_eq!(r.route("factory/press-2/telemetry", &[]), ["Telemetry"]);
        assert!(r.route("factory/press-1/telemetry/x", &[]).is_empty());
        assert!(r.route("factory/telemetry", &[]).is_empty());
        assert!(r.route("plant/press-1/telemetry", &[]).is_empty());
    }

    /// Spec §5.1: `sensors/#` takes `sensors` and every topic below it
    /// (MQTT 5.0 §4.7.1.2), not a topic that only begins with the same
    /// characters; the trailing `/#` used to be matched as a text prefix.
    #[test]
    fn mqttbridge_5_1_a_hash_filter_stops_at_the_level_separator() {
        let r = granted(&[topic("Sensor", "sensors/#", "in")]);
        assert_eq!(r.route("sensors/temp/lab1", &[]), ["Sensor"]);
        assert_eq!(r.route("sensors", &[]), ["Sensor"]);
        assert!(r.route("sensorsX/a", &[]).is_empty());
        assert!(r.route("sensorsX", &[]).is_empty());
    }

    /// Spec §4.3: one identifier per filter from 1 in the order of the
    /// configuration; `out` topics subscribe nothing; topics with the same
    /// filter share one subscription at the higher QoS.
    #[test]
    fn mqttbridge_4_3_one_subscription_and_identifier_per_filter() {
        let mut a = topic("A", "a/in", "in");
        a.mqtt_qos = Some(0);
        let mut a2 = topic("A2", "a/in", "in");
        a2.mqtt_qos = Some(2);
        let r = InboundRoutes::of_topics(
            &[
                topic("Out", "out/x", "out"),
                a,
                topic("B", "b/+", "bidir"),
                a2,
                topic("Denied", "d/x", "in"),
            ],
            |t| t.dds_name != "Denied",
        );
        let subs: Vec<(&str, u32, u8, Vec<String>)> = r
            .subscriptions()
            .iter()
            .map(|s| (s.filter.as_str(), s.id, s.qos, s.dds_names.clone()))
            .collect();
        assert_eq!(
            subs,
            [
                ("a/in", 1, 2, vec!["A".to_string(), "A2".to_string()]),
                ("b/+", 2, 1, vec!["B".to_string()]),
            ]
        );
    }

    /// Spec §4.3: a PUBLISH goes to the topics its identifiers name whose
    /// filter matches its Topic Name; without identifiers to every matching
    /// topic; a refused subscription routes nothing.
    #[test]
    fn mqttbridge_4_3_the_identifier_selects_the_topic() {
        let mut r = granted(&[
            topic("All", "s/#", "in"),
            topic("Temp", "s/temp", "in"),
            topic("Refused", "s/+", "in"),
        ]);
        r.set_granted("s/+", false);
        assert_eq!(r.route("s/temp", &[2]), ["Temp"]);
        assert_eq!(r.route("s/temp", &[1]), ["All"]);
        assert_eq!(r.route("s/temp", &[1, 2]), ["All", "Temp"]);
        assert_eq!(r.route("s/temp", &[]), ["All", "Temp"]);
        // An identifier whose filter does not match the topic (a stale
        // subscription of a resumed session) reaches nothing.
        assert!(r.route("x/temp", &[1]).is_empty());
        assert!(r.route("s/humidity", &[2]).is_empty());
        assert!(r.route("s/temp", &[3]).is_empty());
    }

    /// Spec §4.3: before the SUBACK no subscription routes; a PUBLISH of a
    /// subscription the broker did not grant reaches no topic.
    #[test]
    fn mqttbridge_4_3_a_subscription_routes_once_it_is_granted() {
        let mut r = InboundRoutes::of_topics(&[topic("G", "g/in", "in")], |_| true);
        assert!(r.route("g/in", &[1]).is_empty(), "not granted yet");
        r.set_granted("g/in", true);
        assert_eq!(r.route("g/in", &[1]), ["G"]);
        r.set_granted("g/in", false);
        assert!(r.route("g/in", &[]).is_empty());
        r.set_granted("no/such/filter", true);
        assert!(r.route("no/such/filter", &[]).is_empty());
    }

    /// Spec §5.1: two DDS topics on one filter both get its PUBLISH packets,
    /// each once, in the order of the configuration; a bidir topic is a
    /// subscription like an in topic.
    #[test]
    fn mqttbridge_5_1_two_topics_of_one_filter_each_get_its_publish_once() {
        let r = granted(&[
            topic("First", "plant/+/temp", "bidir"),
            topic("Second", "plant/+/temp", "in"),
            topic("First", "plant/+/temp", "in"),
        ]);
        assert_eq!(r.subscriptions().len(), 1);
        assert_eq!(r.route("plant/a/temp", &[1]), ["First", "Second"]);
        assert_eq!(r.route("plant/a/temp", &[]), ["First", "Second"]);
    }
}
