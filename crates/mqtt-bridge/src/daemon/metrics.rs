// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The Prometheus families of Spec §8.2.
//!
//! The label values come from the configuration, never from the wire: a
//! `dds_topic` is a configured DDS topic (`<tenant>/<topic>` for a tenant's,
//! Spec §1 L6), empty for a PUBLISH of no topic (the catalog of §5.2); an
//! `mqtt_qos` is 0, 1 or 2; a `reason_code` is the Reason Code of the
//! DISCONNECT that ended the connection or `network`.

use std::collections::BTreeMap;
use std::string::{String, ToString};
use std::sync::{Arc, Mutex};

use zerodds_monitor::{Counter, Gauge, Labels, Registry};

use super::config::TopicConfig;

/// The families of Spec §8.2.
pub mod family {
    /// `counter`: connect attempts to the broker, at the start (§9.1) and
    /// after a lost connection (§9.3).
    pub const CONNECT_ATTEMPTS: &str = "zerodds_mqtt_bridge_connect_attempts_total";
    /// `counter`: connect attempts the broker accepted with its CONNACK.
    pub const CONNECT_SUCCESSES: &str = "zerodds_mqtt_bridge_connect_successes_total";
    /// `counter{dds_topic, mqtt_qos}`: PUBLISH packets from the broker, by
    /// the DDS topic they are routed to and the QoS they came with.
    pub const PUBLISH_IN: &str = "zerodds_mqtt_bridge_publish_in_total";
    /// `counter{dds_topic, mqtt_qos}`: PUBLISH packets to the broker.
    pub const PUBLISH_OUT: &str = "zerodds_mqtt_bridge_publish_out_total";
    /// `counter{dds_topic}`: payload octets of the PUBLISH packets from the
    /// broker.
    pub const BYTES_IN: &str = "zerodds_mqtt_bridge_bytes_in_total";
    /// `counter{dds_topic}`: payload octets of the PUBLISH packets to the
    /// broker.
    pub const BYTES_OUT: &str = "zerodds_mqtt_bridge_bytes_out_total";
    /// `gauge{direction}`: QoS 1 and 2 messages whose flow has not ended:
    /// `out` the bridge's PUBLISH packets the broker has not completed,
    /// `in` the broker's QoS 2 PUBLISH packets whose PUBREL has not come.
    pub const INFLIGHT: &str = "zerodds_mqtt_bridge_inflight_messages";
    /// `counter{dds_topic}`: samples the DataReaders received from DDS.
    pub const DDS_RECEIVED: &str = "zerodds_mqtt_bridge_dds_samples_received_total";
    /// `counter{dds_topic}`: samples the DataWriters wrote into DDS.
    pub const DDS_PUBLISHED: &str = "zerodds_mqtt_bridge_dds_samples_published_total";
    /// `counter{reason}`: PUBLISH packets and subscriptions the ACL of §7.3
    /// refused; `reason` is `publish` or `subscribe`.
    pub const ACL_DENIALS: &str = "zerodds_mqtt_bridge_acl_denials_total";
    /// `counter{reason_code}`: broker connections that ended other than by
    /// the daemon's shutdown.
    pub const BROKER_DISCONNECTS: &str = "zerodds_mqtt_bridge_broker_disconnects_total";
    /// `counter{dds_topic}`: samples a `keep_last` history dropped or whose
    /// lifespan passed while they waited for the broker (§6).
    pub const DDS_DROPPED: &str = "zerodds_mqtt_bridge_dds_samples_dropped_total";
    /// `counter`: PUBLISH packets and subscriptions that failed or were not
    /// sent, samples DDS refused, codec and socket errors.
    pub const ERRORS: &str = "zerodds_mqtt_bridge_errors_total";
}

/// Every family of [`family`], in the order of Spec §8.2.
pub const FAMILIES: [&str; 13] = [
    family::CONNECT_ATTEMPTS,
    family::CONNECT_SUCCESSES,
    family::PUBLISH_IN,
    family::PUBLISH_OUT,
    family::BYTES_IN,
    family::BYTES_OUT,
    family::INFLIGHT,
    family::DDS_RECEIVED,
    family::DDS_PUBLISHED,
    family::ACL_DENIALS,
    family::BROKER_DISCONNECTS,
    family::DDS_DROPPED,
    family::ERRORS,
];

/// The `reason` of `acl_denials_total` (Spec §7.3, §8.2).
pub mod acl_reason {
    /// A PUBLISH to the broker that `allow_publish` does not grant.
    pub const PUBLISH: &str = "publish";
    /// A subscription, or a PUBLISH from the broker, that
    /// `allow_subscribe` does not grant.
    pub const SUBSCRIBE: &str = "subscribe";
}

/// The `reason_code` of a connection that ended without a DISCONNECT: the
/// socket closed or failed, or the PINGRESP did not come.
pub const NETWORK: &str = "network";

/// The `reason_code` label of a DISCONNECT with `code`: `0x8e`.
#[must_use]
pub fn reason_code_label(code: Option<u8>) -> String {
    code.map_or_else(|| NETWORK.to_string(), |c| format!("0x{c:02x}"))
}

/// The in-flight counts of each broker connection, summed into the gauge.
type InflightBook = Mutex<BTreeMap<String, (usize, usize)>>;

/// The metrics of Spec §8.2 in a registry, for the connection of the top
/// level or of one tenant ([`Self::for_tenant`]); all of them count into
/// the same families.
#[derive(Clone)]
pub struct BridgeMetrics {
    registry: Arc<Registry>,
    /// The tenant (Spec §1 L6): its topics are labelled `<tenant>/<topic>`.
    tenant: Option<Arc<str>>,
    inflight: Arc<InflightBook>,
    inflight_in: Arc<Gauge>,
    inflight_out: Arc<Gauge>,
    /// `connect_attempts_total`.
    pub connect_attempts: Arc<Counter>,
    /// `connect_successes_total`.
    pub connect_successes: Arc<Counter>,
    /// `errors_total`.
    pub errors_total: Arc<Counter>,
}

impl BridgeMetrics {
    /// Registers the families in `registry`. Idempotent: a second call
    /// counts in the same series.
    pub fn register(registry: &Arc<Registry>) -> Self {
        for (name, help) in [
            (family::CONNECT_ATTEMPTS, "Connect attempts to the broker"),
            (
                family::CONNECT_SUCCESSES,
                "Connect attempts the broker accepted",
            ),
            (
                family::PUBLISH_IN,
                "PUBLISH packets from the broker, by DDS topic and QoS",
            ),
            (
                family::PUBLISH_OUT,
                "PUBLISH packets to the broker, by DDS topic and QoS",
            ),
            (
                family::BYTES_IN,
                "Payload octets of the PUBLISH packets from the broker",
            ),
            (
                family::BYTES_OUT,
                "Payload octets of the PUBLISH packets to the broker",
            ),
            (
                family::INFLIGHT,
                "QoS 1 and 2 messages whose flow has not ended, by direction",
            ),
            (family::DDS_RECEIVED, "Samples received from DDS, by topic"),
            (family::DDS_PUBLISHED, "Samples written into DDS, by topic"),
            (
                family::ACL_DENIALS,
                "PUBLISH packets and subscriptions the ACL refused, by reason",
            ),
            (
                family::BROKER_DISCONNECTS,
                "Broker connections that ended, by DISCONNECT reason code",
            ),
            (
                family::DDS_DROPPED,
                "Samples dropped while they waited for the broker, by topic",
            ),
            (
                family::ERRORS,
                "PUBLISH packets and subscriptions that failed, codec and socket errors",
            ),
        ] {
            registry.set_help(name, help);
        }
        Self {
            registry: Arc::clone(registry),
            tenant: None,
            inflight: Arc::new(Mutex::new(BTreeMap::new())),
            inflight_in: registry.gauge(family::INFLIGHT, Labels::new().with("direction", "in")),
            inflight_out: registry.gauge(family::INFLIGHT, Labels::new().with("direction", "out")),
            connect_attempts: registry.counter(family::CONNECT_ATTEMPTS, Labels::new()),
            connect_successes: registry.counter(family::CONNECT_SUCCESSES, Labels::new()),
            errors_total: registry.counter(family::ERRORS, Labels::new()),
        }
    }

    /// The metrics of the connection of tenant `tenant` (Spec §1 L6).
    #[must_use]
    pub fn for_tenant(&self, tenant: &str) -> Self {
        let mut out = self.clone();
        out.tenant = (!tenant.is_empty()).then(|| Arc::from(tenant));
        out
    }

    /// The `dds_topic` label of `topic`; empty stays empty.
    fn topic_label(&self, topic: &str) -> String {
        match &self.tenant {
            Some(t) if !topic.is_empty() => format!("{t}/{topic}"),
            _ => topic.to_string(),
        }
    }

    fn by_topic(&self, topic: &str) -> Labels {
        Labels::new().with("dds_topic", self.topic_label(topic))
    }

    /// The series of `topic` at zero (Spec §8.2): they show before the first
    /// message. The PUBLISH series carry the topic's QoS.
    pub fn register_topic(&self, topic: &TopicConfig) {
        let labels = self.by_topic(&topic.dds_name);
        let qos = topic.effective_mqtt_qos().to_string();
        if matches!(topic.direction.as_str(), "out" | "bidir") {
            self.registry.counter(
                family::PUBLISH_OUT,
                labels.clone().with("mqtt_qos", qos.clone()),
            );
            self.registry.counter(family::BYTES_OUT, labels.clone());
            self.registry.counter(family::DDS_RECEIVED, labels.clone());
            self.registry.counter(family::DDS_DROPPED, labels.clone());
        }
        if matches!(topic.direction.as_str(), "in" | "bidir") {
            self.registry
                .counter(family::PUBLISH_IN, labels.clone().with("mqtt_qos", qos));
            self.registry.counter(family::BYTES_IN, labels.clone());
            self.registry.counter(family::DDS_PUBLISHED, labels);
        }
    }

    /// A PUBLISH with `octets` payload octets and QoS `qos` went to the
    /// broker for `topic` (empty: no topic).
    pub fn publish_out(&self, topic: &str, qos: u8, octets: usize) {
        let labels = self.by_topic(topic);
        self.registry
            .counter(family::BYTES_OUT, labels.clone())
            .add(octets as u64);
        self.registry
            .counter(
                family::PUBLISH_OUT,
                labels.with("mqtt_qos", qos.to_string()),
            )
            .inc();
    }

    /// A PUBLISH with `octets` payload octets and QoS `qos` came from the
    /// broker for `topic` (empty: it matches no subscribed topic).
    pub fn publish_in(&self, topic: &str, qos: u8, octets: usize) {
        let labels = self.by_topic(topic);
        self.registry
            .counter(family::BYTES_IN, labels.clone())
            .add(octets as u64);
        self.registry
            .counter(family::PUBLISH_IN, labels.with("mqtt_qos", qos.to_string()))
            .inc();
    }

    /// A DataReader of `topic` received a sample.
    pub fn dds_received(&self, topic: &str) {
        self.registry
            .counter(family::DDS_RECEIVED, self.by_topic(topic))
            .inc();
    }

    /// A DataWriter of `topic` wrote a sample into DDS.
    pub fn dds_published(&self, topic: &str) {
        self.registry
            .counter(family::DDS_PUBLISHED, self.by_topic(topic))
            .inc();
    }

    /// A sample of `topic` was dropped while it waited for the broker.
    pub fn dds_dropped(&self, topic: &str) {
        self.registry
            .counter(family::DDS_DROPPED, self.by_topic(topic))
            .inc();
    }

    /// The ACL refused a PUBLISH or a subscription (see [`acl_reason`]).
    pub fn acl_denial(&self, reason: &'static str) {
        self.registry
            .counter(family::ACL_DENIALS, Labels::new().with("reason", reason))
            .inc();
    }

    /// The broker connection ended: with a DISCONNECT of reason `code`, or
    /// without one (`None`).
    pub fn broker_disconnect(&self, code: Option<u8>) {
        self.registry
            .counter(
                family::BROKER_DISCONNECTS,
                Labels::new().with("reason_code", reason_code_label(code)),
            )
            .inc();
    }

    /// The in-flight counts of this connection: the broker's QoS 2 messages
    /// whose PUBREL has not come (`inbound`) and the bridge's QoS 1/2
    /// messages the broker has not completed (`outbound`); the gauge is the
    /// sum over the connections.
    pub fn set_inflight(&self, inbound: usize, outbound: usize) {
        let Ok(mut book) = self.inflight.lock() else {
            return;
        };
        let key = self.tenant.as_deref().unwrap_or("").to_string();
        book.insert(key, (inbound, outbound));
        let (i, o) = book
            .values()
            .fold((0usize, 0usize), |(i, o), (a, b)| (i + a, o + b));
        self.inflight_in.set(i64::try_from(i).unwrap_or(i64::MAX));
        self.inflight_out.set(i64::try_from(o).unwrap_or(i64::MAX));
    }

    /// The sum of a counter family over its labels.
    #[must_use]
    pub fn total(&self, family: &str) -> u64 {
        self.registry
            .snapshot()
            .counters
            .iter()
            .filter(|(key, _)| key.name == family)
            .map(|(_, v)| *v)
            .sum()
    }

    /// The value of the series of `family` with `labels`; 0 for one not
    /// counted yet.
    #[must_use]
    pub fn value(&self, family: &str, labels: &[(&'static str, &str)]) -> u64 {
        let mut want = Labels::new();
        for (k, v) in labels {
            want.set(k, *v);
        }
        self.registry
            .snapshot()
            .counters
            .iter()
            .find(|(key, _)| key.name == family && key.labels == want)
            .map_or(0, |(_, v)| *v)
    }

    /// The value of the in-flight gauge of `direction` (`in`, `out`).
    #[must_use]
    pub fn inflight(&self, direction: &str) -> i64 {
        match direction {
            "in" => self.inflight_in.get(),
            _ => self.inflight_out.get(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn topic(name: &str, direction: &str, qos: u8) -> TopicConfig {
        TopicConfig {
            dds_name: name.into(),
            dds_type: name.into(),
            mqtt_topic: name.to_lowercase(),
            direction: direction.into(),
            mqtt_qos: Some(qos),
            ..TopicConfig::default()
        }
    }

    /// The family names and labels of the code block of Spec §8.2, read
    /// from `docs/specs/zerodds-mqtt-bridge-1.0.md`: `(name, kind, labels)`.
    fn families_of_the_spec() -> Vec<(String, String, Vec<String>)> {
        let spec = include_str!("../../../../docs/specs/zerodds-mqtt-bridge-1.0.md");
        let section = spec
            .split("### §8.2 Prometheus metrics")
            .nth(1)
            .and_then(|s| s.split("```").nth(1))
            .expect("the code block of §8.2");
        section
            .lines()
            .filter(|l| l.starts_with("zerodds_"))
            .map(|l| {
                let (name, kind) = l.split_once(char::is_whitespace).unwrap();
                let (name, kind) = (name.to_string(), kind.trim());
                let (kind, labels) = match kind.split_once('{') {
                    Some((k, rest)) => (
                        k.to_string(),
                        rest.trim_end_matches('}')
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .collect(),
                    ),
                    None => (kind.to_string(), Vec::new()),
                };
                (name, kind, labels)
            })
            .collect()
    }

    /// Spec §8.2: every family of the spec's list is rendered under its
    /// name, with its type and its labels, and the code has no family the
    /// list lacks.
    #[test]
    fn mqttbridge_8_2_every_family_of_the_spec_is_rendered_with_its_labels() {
        let registry = Arc::new(Registry::new());
        let m = BridgeMetrics::register(&registry);
        m.register_topic(&topic("Chat", "bidir", 1));
        m.connect_attempts.inc();
        m.connect_successes.inc();
        m.errors_total.inc();
        m.publish_in("Chat", 1, 3);
        m.publish_out("Chat", 1, 4);
        m.dds_received("Chat");
        m.dds_published("Chat");
        m.dds_dropped("Chat");
        m.acl_denial(acl_reason::PUBLISH);
        m.broker_disconnect(Some(0x8E));
        m.set_inflight(1, 2);
        let text = registry.render_prometheus();
        let spec = families_of_the_spec();
        assert_eq!(spec.len(), FAMILIES.len(), "{spec:?}");
        for (name, kind, labels) in &spec {
            assert!(
                FAMILIES.contains(&name.as_str()),
                "{name} is not in the code"
            );
            assert!(
                text.contains(&format!("# TYPE {name} {kind}\n")),
                "{name} {kind}:\n{text}"
            );
            let line = text
                .lines()
                .find(|l| {
                    l.strip_prefix(name.as_str())
                        .is_some_and(|rest| rest.starts_with(['{', ' ']))
                })
                .unwrap_or_else(|| panic!("{name}:\n{text}"));
            let got: Vec<&str> = line
                .split_once('{')
                .map(|(_, rest)| {
                    rest.split_once('}')
                        .unwrap()
                        .0
                        .split(',')
                        .map(|kv| kv.split_once('=').unwrap().0)
                        .collect()
                })
                .unwrap_or_default();
            let mut want: Vec<&str> = labels.iter().map(String::as_str).collect();
            want.sort_unstable();
            assert_eq!(got, want, "{line}");
        }
        assert!(
            !text.contains("zerodds_mqtt_frames") && !text.contains("zerodds_mqtt_dds_samples_in"),
            "{text}"
        );
    }

    #[test]
    fn mqttbridge_8_2_register_is_idempotent() {
        let registry = Arc::new(Registry::new());
        let a = BridgeMetrics::register(&registry);
        let b = BridgeMetrics::register(&registry);
        a.connect_attempts.inc();
        a.dds_published("T");
        assert_eq!(b.connect_attempts.get(), 1);
        assert_eq!(b.value(family::DDS_PUBLISHED, &[("dds_topic", "T")]), 1);
    }

    /// The series of a topic exist at zero from its registration, with the
    /// labels of its direction; a tenant's topic is `<tenant>/<topic>`.
    #[test]
    fn mqttbridge_8_2_a_topic_is_counted_under_its_name_and_its_tenant() {
        let registry = Arc::new(Registry::new());
        let top = BridgeMetrics::register(&registry);
        top.register_topic(&topic("Out", "out", 0));
        let text = registry.render_prometheus();
        assert!(
            text.contains(
                "zerodds_mqtt_bridge_publish_out_total{dds_topic=\"Out\",mqtt_qos=\"0\"} 0"
            ),
            "{text}"
        );
        assert!(!text.contains("zerodds_mqtt_bridge_publish_in_total{dds_topic=\"Out\""));
        let plant = top.for_tenant("plant-a");
        plant.publish_out("Status", 1, 10);
        plant.publish_out("Status", 1, 5);
        assert_eq!(
            top.value(
                family::PUBLISH_OUT,
                &[("dds_topic", "plant-a/Status"), ("mqtt_qos", "1")]
            ),
            2
        );
        assert_eq!(
            top.value(family::BYTES_OUT, &[("dds_topic", "plant-a/Status")]),
            15
        );
        assert_eq!(top.value(family::BYTES_OUT, &[("dds_topic", "Status")]), 0);
    }

    #[test]
    fn mqttbridge_8_2_a_disconnect_counts_its_reason_code() {
        let registry = Arc::new(Registry::new());
        let m = BridgeMetrics::register(&registry);
        m.broker_disconnect(Some(0x8E));
        m.broker_disconnect(Some(0x8E));
        m.broker_disconnect(None);
        assert_eq!(
            m.value(family::BROKER_DISCONNECTS, &[("reason_code", "0x8e")]),
            2
        );
        assert_eq!(
            m.value(family::BROKER_DISCONNECTS, &[("reason_code", "network")]),
            1
        );
    }

    /// The gauge is the sum of the connections' counts, and follows them
    /// down.
    #[test]
    fn mqttbridge_8_2_inflight_follows_the_counts_of_each_connection() {
        let registry = Arc::new(Registry::new());
        let top = BridgeMetrics::register(&registry);
        let tenant = top.for_tenant("t");
        top.set_inflight(1, 3);
        tenant.set_inflight(0, 2);
        assert_eq!((top.inflight("in"), top.inflight("out")), (1, 5));
        top.set_inflight(0, 0);
        assert_eq!((top.inflight("in"), top.inflight("out")), (0, 2));
        let text = registry.render_prometheus();
        assert!(
            text.contains("zerodds_mqtt_bridge_inflight_messages{direction=\"out\"} 2"),
            "{text}"
        );
    }

    #[test]
    fn mqttbridge_8_2_an_acl_denial_counts_its_reason() {
        let registry = Arc::new(Registry::new());
        let m = BridgeMetrics::register(&registry);
        m.acl_denial(acl_reason::SUBSCRIBE);
        m.acl_denial(acl_reason::PUBLISH);
        m.acl_denial(acl_reason::PUBLISH);
        assert_eq!(m.value(family::ACL_DENIALS, &[("reason", "publish")]), 2);
        assert_eq!(m.value(family::ACL_DENIALS, &[("reason", "subscribe")]), 1);
    }
}
