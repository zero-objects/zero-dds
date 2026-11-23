// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-cutting daemon runtime: §8.2 Prometheus metrics, §8.3 OTLP spans,
//! §9.2 graceful shutdown, §5.2 catalog/healthz endpoint, §6 QoS mapping.
//!
//! Reusable for the `zerodds-ws-bridged` daemon — the logic is kept
//! library-side so that tests can exercise the `BridgeMetrics` set without
//! a subprocess boot.

#![allow(clippy::print_stderr)]

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use zerodds_monitor::admin::AdminEndpoints;
pub use zerodds_monitor::admin::{health_response, http_response};
use zerodds_monitor::json::push_escaped as push_json_str;
use zerodds_monitor::{Counter, Gauge, Labels, Registry};
use zerodds_observability_otlp::{OtlpConfig, OtlpExporter};

use super::config::{DaemonConfig, TopicConfig};

/// Service name for the OTel resource and catalog.
pub const SERVICE_NAME: &str = "zerodds-ws-bridged";

/// Version string — Cargo crate version (single source).
pub const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

// ============================================================================
// A2 — Prometheus metrics set (§8.2).
// ============================================================================

/// The families of Spec §8.2.
pub mod family {
    /// `counter`: WebSocket connections accepted since the start.
    pub const CONNECTIONS_TOTAL: &str = "zerodds_ws_bridge_connections_total";
    /// `gauge`: open WebSocket connections.
    pub const CONNECTIONS_ACTIVE: &str = "zerodds_ws_bridge_connections_active";
    /// `counter{topic, direction}`: messages from the clients.
    pub const FRAMES_IN: &str = "zerodds_ws_bridge_frames_in_total";
    /// `counter{topic, direction}`: payload octets of the messages from the
    /// clients.
    pub const BYTES_IN: &str = "zerodds_ws_bridge_bytes_in_total";
    /// `counter{topic, direction}`: messages to the clients.
    pub const FRAMES_OUT: &str = "zerodds_ws_bridge_frames_out_total";
    /// `counter{topic, direction}`: payload octets of the messages to the
    /// clients.
    pub const BYTES_OUT: &str = "zerodds_ws_bridge_bytes_out_total";
    /// `counter{connection_id}`: samples dropped on a full send queue (§6).
    pub const SEND_QUEUE_DROPS: &str = "zerodds_ws_bridge_send_queue_drops_total";
    /// `counter{topic}`: samples the DataReaders received from DDS.
    pub const DDS_RECEIVED: &str = "zerodds_ws_bridge_dds_samples_received";
    /// `counter{topic}`: samples the DataWriters published into DDS.
    pub const DDS_PUBLISHED: &str = "zerodds_ws_bridge_dds_samples_published";
    /// `counter{reason}`: handshakes and messages refused for their
    /// credentials or the ACL.
    pub const AUTH_FAILURES: &str = "zerodds_ws_bridge_auth_failures_total";
    /// `counter`: frame, codec and socket errors, and samples of a topic
    /// with key that carry no key hash (§6).
    pub const ERRORS: &str = "zerodds_ws_bridge_errors_total";
}

/// The `reason` of `auth_failures_total` (Spec §8.2).
pub mod auth_reason {
    /// No credentials where the auth mode needs some (401).
    pub const MISSING: &str = "missing";
    /// Credentials that authenticate no one: malformed, unknown, a token
    /// that does not verify, no client certificate (401).
    pub const REJECTED: &str = "rejected";
    /// A verified token without the required claim (403).
    pub const SCOPE: &str = "scope";
    /// The ACL does not grant the read or the write (403, or an error
    /// frame 403).
    pub const ACL: &str = "acl";
}

/// The metrics of Spec §8.2 in a registry. The families with labels are
/// counted through the methods; `topic` is the DDS topic a message carries
/// a sample (or a `deadline_missed`) of and `direction` that topic's
/// `direction` in the configuration (`in`, `out`, `bidir`); a message of no
/// topic (a control message, the catalog, an error frame) is counted with
/// both labels empty.
#[derive(Clone)]
pub struct BridgeMetrics {
    registry: Arc<Registry>,
    /// `topic → direction` of the configuration.
    directions: Arc<BTreeMap<String, String>>,
    /// The tenant whose topics these are (Spec §1 L6): their `topic` label
    /// is `<tenant>/<topic>`. `None`: the topics of `domain`.
    tenant: Option<Arc<str>>,
    /// `connections_total`.
    pub connections_total: Arc<Counter>,
    /// `connections_active`.
    pub connections_active: Arc<Gauge>,
    /// `errors_total`.
    pub errors_total: Arc<Counter>,
}

impl BridgeMetrics {
    /// Registers the families in `registry`. Idempotent: a second call
    /// counts in the same series.
    pub fn register(registry: &Arc<Registry>) -> Self {
        for (name, help) in [
            (
                family::CONNECTIONS_TOTAL,
                "WebSocket connections accepted since the start",
            ),
            (family::CONNECTIONS_ACTIVE, "Open WebSocket connections"),
            (
                family::FRAMES_IN,
                "Messages from the clients, by topic and the topic's direction",
            ),
            (
                family::BYTES_IN,
                "Payload octets of the messages from the clients",
            ),
            (
                family::FRAMES_OUT,
                "Messages to the clients, by topic and the topic's direction",
            ),
            (
                family::BYTES_OUT,
                "Payload octets of the messages to the clients",
            ),
            (
                family::SEND_QUEUE_DROPS,
                "Samples dropped on a full send queue, by connection",
            ),
            (family::DDS_RECEIVED, "Samples received from DDS, by topic"),
            (
                family::DDS_PUBLISHED,
                "Samples published into DDS, by topic",
            ),
            (
                family::AUTH_FAILURES,
                "Handshakes and messages refused for credentials or the ACL, by reason",
            ),
            (
                family::ERRORS,
                "Frame, codec and socket errors and samples without key hash",
            ),
        ] {
            registry.set_help(name, help);
        }
        Self {
            registry: Arc::clone(registry),
            directions: Arc::new(BTreeMap::new()),
            tenant: None,
            connections_total: registry.counter(family::CONNECTIONS_TOTAL, Labels::new()),
            connections_active: registry.gauge(family::CONNECTIONS_ACTIVE, Labels::new()),
            errors_total: registry.counter(family::ERRORS, Labels::new()),
        }
    }

    /// The `direction` label of each topic, from the configuration.
    #[must_use]
    pub fn with_topics(mut self, topics: &[TopicConfig]) -> Self {
        self.directions = Arc::new(
            topics
                .iter()
                .map(|t| (t.name.clone(), t.direction.clone()))
                .collect(),
        );
        self
    }

    /// The metrics of the topics of the tenant `tenant` (Spec §1 L6): the
    /// same families and daemon-wide series, a topic's `topic` label
    /// `<tenant>/<topic>`.
    #[must_use]
    pub fn for_tenant(&self, tenant: &str, topics: &[TopicConfig]) -> Self {
        let mut out = self.clone().with_topics(topics);
        out.tenant = Some(Arc::from(tenant));
        out
    }

    /// The `topic` label of `topic`: `<tenant>/<topic>` for a tenant's.
    fn topic_label(&self, topic: &str) -> String {
        match &self.tenant {
            Some(tenant) if !topic.is_empty() => super::config::tenant_acl_topic(tenant, topic),
            _ => topic.to_string(),
        }
    }

    /// The labels of a message of `topic`. A name that is no configured
    /// topic is no bridged DDS topic: it is counted under `topic=""` like a
    /// message of no topic, so a client cannot add series with names of its
    /// own (§8.2).
    fn by_topic(&self, topic: &str) -> Labels {
        let (topic, direction) = self
            .directions
            .get_key_value(topic)
            .map_or(("", ""), |(t, d)| (t.as_str(), d.as_str()));
        Labels::new()
            .with("topic", self.topic_label(topic))
            .with("direction", direction)
    }

    /// A message of `octets` payload octets from a client.
    pub fn frame_in(&self, topic: &str, octets: usize) {
        let labels = self.by_topic(topic);
        self.registry
            .counter(family::BYTES_IN, labels.clone())
            .add(octets as u64);
        self.registry.counter(family::FRAMES_IN, labels).inc();
    }

    /// A message of `octets` payload octets to a client.
    pub fn frame_out(&self, topic: &str, octets: usize) {
        let labels = self.by_topic(topic);
        self.registry
            .counter(family::BYTES_OUT, labels.clone())
            .add(octets as u64);
        self.registry.counter(family::FRAMES_OUT, labels).inc();
    }

    /// A sample of `topic` came from DDS.
    pub fn dds_received(&self, topic: &str) {
        self.registry
            .counter(
                family::DDS_RECEIVED,
                Labels::new().with("topic", self.topic_label(topic)),
            )
            .inc();
    }

    /// A sample of `topic` was published into DDS.
    pub fn dds_published(&self, topic: &str) {
        self.registry
            .counter(
                family::DDS_PUBLISHED,
                Labels::new().with("topic", self.topic_label(topic)),
            )
            .inc();
    }

    /// The send queue of connection `connection_id` dropped a sample.
    pub fn send_queue_drop(&self, connection_id: u64) {
        self.registry
            .counter(
                family::SEND_QUEUE_DROPS,
                Labels::new().with("connection_id", connection_id.to_string()),
            )
            .inc();
    }

    /// A handshake or a message refused for `reason` (see [`auth_reason`]).
    pub fn auth_failure(&self, reason: &str) {
        self.registry
            .counter(family::AUTH_FAILURES, Labels::new().with("reason", reason))
            .inc();
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

    /// The value of one series of a counter family; 0 for a series not
    /// counted yet.
    #[must_use]
    pub fn value(&self, family: &str, labels: &[(&str, &str)]) -> u64 {
        self.registry
            .snapshot()
            .counters
            .iter()
            .filter(|(key, _)| key.name == family)
            .find(|(key, _)| {
                key.labels.len() == labels.len()
                    && labels
                        .iter()
                        .all(|(k, v)| key.labels.iter().any(|(kk, vv)| kk == *k && vv == *v))
            })
            .map_or(0, |(_, v)| *v)
    }
}

// ============================================================================
// A1 — graceful shutdown (§9.2).
// ============================================================================

/// Lifecycle signals the daemon can receive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleSignal {
    /// SIGTERM/SIGINT — begin shutdown.
    Shutdown,
    /// SIGHUP — config reload (TLS cert + ACL hot-reload).
    Reload,
}

/// Installs a signal watcher that catches `SIGTERM`/`SIGINT`/`SIGHUP`
/// and sets the `shutdown_flag` or triggers the `reload_flag`
/// hook respectively.
///
/// The worker runs in a dedicated thread and exits as soon as the
/// `shutdown_flag` is set (self-notification via the SIGTERM path).
///
/// The function returns a join handle; in the daemon `Drop` path it
/// should not be actively joined — the worker thread is detached
/// against the process lifecycle.
#[cfg(unix)]
pub fn install_signal_watcher(
    shutdown_flag: Arc<AtomicBool>,
    reload_flag: Arc<AtomicBool>,
) -> std::io::Result<JoinHandle<()>> {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    use signal_hook::iterator::Signals;

    let mut signals = Signals::new([SIGTERM, SIGINT, SIGHUP])?;
    let h = thread::Builder::new()
        .name("zerodds-ws-signals".into())
        .spawn(move || {
            for sig in signals.forever() {
                match sig {
                    SIGTERM | SIGINT => {
                        shutdown_flag.store(true, Ordering::SeqCst);
                        break;
                    }
                    SIGHUP => {
                        reload_flag.store(true, Ordering::SeqCst);
                    }
                    _ => {}
                }
            }
        })?;
    Ok(h)
}

// ============================================================================
// A5 — catalog/healthz endpoint (§5.2).
// ============================================================================

/// Catalog snapshot for the `/catalog` endpoint.
#[derive(Clone)]
pub struct CatalogSnapshot {
    /// Service name.
    pub service: String,
    /// Version.
    pub version: String,
    /// Bridge topics (DDS name + direction + WS path + QoS).
    pub topics: Vec<TopicConfig>,
}

impl CatalogSnapshot {
    /// Build from the daemon config.
    #[must_use]
    pub fn from_config(cfg: &DaemonConfig) -> Self {
        Self {
            service: SERVICE_NAME.into(),
            version: SERVICE_VERSION.into(),
            topics: cfg.topics.clone(),
        }
    }

    /// Render as JSON (Spec §5.2).
    #[must_use]
    pub fn render_json(&self) -> String {
        self.render_json_where(|_| true)
    }

    /// [`Self::render_json`] with the topics `listed` takes: the catalog
    /// of the meta endpoint (Spec §4.2) names the topics its client may
    /// read or write.
    #[must_use]
    pub fn render_json_where(&self, listed: impl Fn(&TopicConfig) -> bool) -> String {
        let mut out = String::with_capacity(256 + self.topics.len() * 128);
        out.push_str("{\"op\":\"catalog\",\"service\":\"");
        push_json_str(&mut out, &self.service);
        out.push_str("\",\"version\":\"");
        push_json_str(&mut out, &self.version);
        out.push_str("\",\"topics\":[");
        for (i, t) in self.topics.iter().filter(|t| listed(t)).enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str("{\"name\":\"");
            push_json_str(&mut out, &t.name);
            out.push_str("\",\"type\":\"");
            push_json_str(&mut out, &t.type_name);
            out.push_str("\",\"direction\":\"");
            push_json_str(&mut out, &t.direction);
            out.push_str("\",\"ws_path\":\"");
            push_json_str(&mut out, &t.path());
            out.push_str("\",\"qos\":{\"reliability\":\"");
            push_json_str(&mut out, &t.reliability);
            out.push_str("\",\"durability\":\"");
            push_json_str(&mut out, &t.durability);
            out.push_str("\",\"history\":{\"kind\":\"");
            out.push_str(if t.keep_all() {
                "keep_all"
            } else {
                "keep_last"
            });
            out.push_str("\",\"depth\":");
            out.push_str(&t.history_depth.to_string());
            out.push_str("}}");
            // §5.2: the type of a topic with `idl_file` at its schema path.
            if !t.idl_file.is_empty() {
                out.push_str(",\"schema_url\":\"");
                push_json_str(&mut out, &super::schema::schema_url(&t.name));
                out.push('"');
            }
            out.push('}');
        }
        out.push_str("]}");
        out
    }
}

/// Mini HTTP worker serving `/catalog`, `/healthz` and the Prometheus
/// metrics at `metrics_path` (`metrics.path`, Spec §3).
///
/// Returns `JoinHandle` + bound `SocketAddr`. When the stop flag is set,
/// the accept loop runs one more iteration (thanks to self-connect).
pub fn serve_admin_endpoints(
    addr: SocketAddr,
    metrics_path: String,
    catalog: Arc<CatalogSnapshot>,
    registry: Arc<Registry>,
    healthy: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
) -> std::io::Result<(JoinHandle<()>, SocketAddr)> {
    zerodds_monitor::admin::serve_admin_endpoints(
        addr,
        "zerodds-ws-admin",
        AdminEndpoints {
            service_name: SERVICE_NAME.into(),
            metrics_path,
            catalog: move || catalog.render_json(),
            registry,
            healthy,
        },
        stop,
    )
}

// ============================================================================
// A3 — OTLP-Span-Exporter (§8.3).
// ============================================================================

/// Parses an `OTEL_EXPORTER_OTLP_ENDPOINT`-like string into an
/// `OtlpConfig`. Accepts `http://host:port`, `host:port`, or
/// just `host` (default port 4318).
#[must_use]
pub fn otlp_config_from_endpoint(service_name: &str, raw: &str) -> OtlpConfig {
    OtlpConfig::from_endpoint(service_name, SERVICE_VERSION, raw)
}

/// Fetches `OTEL_EXPORTER_OTLP_ENDPOINT` from the environment and parses
/// `host:port`. Returns `None` if the env var is not set.
#[must_use]
pub fn otlp_config_from_env(service_name: &str) -> Option<OtlpConfig> {
    OtlpConfig::from_env(service_name, SERVICE_VERSION)
}

/// Spawns a periodic `OtlpExporter::flush()` thread.
pub fn spawn_otlp_flush_loop(
    exporter: Arc<OtlpExporter>,
    stop: Arc<AtomicBool>,
    interval: Duration,
) -> std::io::Result<JoinHandle<()>> {
    zerodds_observability_otlp::spawn_flush_loop(
        exporter,
        stop,
        interval,
        Duration::from_millis(100),
        "zerodds-ws-otlp",
    )
}

// ============================================================================
// Tests
// ============================================================================

/// Off Unix (Windows): the requests of the service control manager and of
/// the console (`zerodds_dcps::cli::run_as_service`) in place of the
/// signals: a stop request sets `shutdown_flag` as SIGTERM does, a
/// `paramchange` sets `reload_flag` as SIGHUP does (Spec §9.2).
#[cfg(not(unix))]
pub fn install_signal_watcher(
    shutdown_flag: Arc<AtomicBool>,
    reload_flag: Arc<AtomicBool>,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("zerodds-ws-signals".into())
        .spawn(move || watch_service_requests(&shutdown_flag, &reload_flag))
}

/// The loop of the watcher off Unix: every 100 ms the stop and reload
/// requests of `zerodds_dcps::cli`; it ends with the stop request, when
/// `shutdown_flag` is set otherwise, or when nobody else holds it (the
/// daemon has gone).
pub fn watch_service_requests(shutdown_flag: &Arc<AtomicBool>, reload_flag: &Arc<AtomicBool>) {
    loop {
        if zerodds_dcps::cli::take_service_reload_request() {
            reload_flag.store(true, Ordering::SeqCst);
        }
        if zerodds_dcps::cli::service_stop_requested() {
            shutdown_flag.store(true, Ordering::SeqCst);
            return;
        }
        if shutdown_flag.load(Ordering::SeqCst) || Arc::strong_count(shutdown_flag) == 1 {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn wsbridge_8_2_metrics_register_is_idempotent() {
        let r = Arc::new(Registry::new());
        let m1 = BridgeMetrics::register(&r);
        let m2 = BridgeMetrics::register(&r);
        m1.connections_total.inc();
        m1.frame_in("T", 3);
        // Same registry key, same series.
        assert_eq!(m2.connections_total.get(), 1);
        assert_eq!(m2.total(family::FRAMES_IN), 1);
    }

    /// Spec §8.2: the ten families under their names, the counters with
    /// their labels (`direction` from the topic's configuration, empty for
    /// a message of no topic), and the error counter of §6.
    #[test]
    fn wsbridge_8_2_the_ten_families_carry_their_labels() {
        let r = Arc::new(Registry::new());
        let topics = [
            TopicConfig {
                name: "Chat::Message".into(),
                direction: "bidir".into(),
                ..TopicConfig::default()
            },
            TopicConfig {
                name: "Sensor::Reading".into(),
                direction: "out".into(),
                ..TopicConfig::default()
            },
        ];
        let m = BridgeMetrics::register(&r).with_topics(&topics);
        m.connections_total.inc();
        m.connections_active.set(2);
        m.frame_in("Chat::Message", 10);
        m.frame_in("Chat::Message", 5);
        m.frame_in("", 13);
        // A name a client wrote that is no configured topic: no series of
        // its own.
        m.frame_in("Not::Configured", 7);
        m.frame_out("Sensor::Reading", 40);
        m.send_queue_drop(7);
        m.send_queue_drop(7);
        m.dds_received("Sensor::Reading");
        m.dds_published("Chat::Message");
        m.auth_failure(auth_reason::MISSING);
        m.auth_failure(auth_reason::ACL);
        m.errors_total.inc();
        let text = r.render_prometheus();
        for line in [
            "zerodds_ws_bridge_connections_total 1",
            "zerodds_ws_bridge_connections_active 2",
            "zerodds_ws_bridge_frames_in_total{direction=\"bidir\",topic=\"Chat::Message\"} 2",
            "zerodds_ws_bridge_bytes_in_total{direction=\"bidir\",topic=\"Chat::Message\"} 15",
            "zerodds_ws_bridge_frames_in_total{direction=\"\",topic=\"\"} 2",
            "zerodds_ws_bridge_bytes_in_total{direction=\"\",topic=\"\"} 20",
            "zerodds_ws_bridge_frames_out_total{direction=\"out\",topic=\"Sensor::Reading\"} 1",
            "zerodds_ws_bridge_bytes_out_total{direction=\"out\",topic=\"Sensor::Reading\"} 40",
            "zerodds_ws_bridge_send_queue_drops_total{connection_id=\"7\"} 2",
            "zerodds_ws_bridge_dds_samples_received{topic=\"Sensor::Reading\"} 1",
            "zerodds_ws_bridge_dds_samples_published{topic=\"Chat::Message\"} 1",
            "zerodds_ws_bridge_auth_failures_total{reason=\"missing\"} 1",
            "zerodds_ws_bridge_auth_failures_total{reason=\"acl\"} 1",
            "zerodds_ws_bridge_errors_total 1",
        ] {
            assert!(text.lines().any(|l| l == line), "{line} not in:\n{text}");
        }
        assert!(!text.contains("zerodds_ws_frames"), "{text}");
        assert!(!text.contains("Not::Configured"), "{text}");
        assert_eq!(m.total(family::FRAMES_IN), 4);
        assert_eq!(
            m.value(
                family::BYTES_IN,
                &[("topic", "Chat::Message"), ("direction", "bidir")]
            ),
            15
        );
        assert_eq!(m.value(family::BYTES_IN, &[("topic", "Other")]), 0);
    }

    #[test]
    fn wsbridge_5_2_catalog_render_json_contains_topic_fields() {
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.topics.push(TopicConfig {
            name: "Chat::Message".into(),
            type_name: "Chat::Message".into(),
            direction: "bidir".into(),
            ws_path: "/topics/chat/message".into(),
            reliability: "reliable".into(),
            durability: "volatile".into(),
            history_depth: 10,
            ..TopicConfig::default()
        });
        let snap = CatalogSnapshot::from_config(&cfg);
        let j = snap.render_json();
        assert!(j.starts_with("{\"op\":\"catalog\","));
        assert!(j.contains("\"service\":\"zerodds-ws-bridged\""));
        assert!(j.contains("\"history\":{\"kind\":\"keep_last\",\"depth\":10}"));
        // Without `idl_file` no schema.
        assert!(!j.contains("schema_url"), "{j}");
        // The catalog of a client names the topics it may use.
        let none = snap.render_json_where(|_| false);
        assert!(none.ends_with("\"topics\":[]}"), "{none}");
        assert!(j.contains("\"name\":\"Chat::Message\""));
        assert!(j.contains("\"ws_path\":\"/topics/chat/message\""));
        assert!(j.contains("\"reliability\":\"reliable\""));
    }

    /// Spec §5.2: the catalog frame is `{"op":"catalog","topics":[...]}`,
    /// each topic with `name`, `type`, `direction`, `ws_path`, `qos`
    /// (`reliability`, `durability`, `history` with `kind` and `depth`) and,
    /// for a topic with `idl_file`, `schema_url`.
    #[test]
    fn wsbridge_5_2_the_catalog_frame_names_topics_qos_and_schema_url() {
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.topics.push(TopicConfig {
            name: "Chat::Message".into(),
            type_name: "Chat::Msg".into(),
            direction: "in".into(),
            ws_path: "/chat".into(),
            reliability: "best_effort".into(),
            durability: "transient_local".into(),
            history_kind: "keep_all".into(),
            history_depth: 3,
            idl_file: "/etc/zerodds/chat.idl".into(),
            ..TopicConfig::default()
        });
        let v: serde_json::Value =
            serde_json::from_str(&CatalogSnapshot::from_config(&cfg).render_json()).unwrap();
        assert_eq!(v["op"], "catalog");
        let t = &v["topics"][0];
        assert_eq!(t["name"], "Chat::Message");
        assert_eq!(t["type"], "Chat::Msg");
        assert_eq!(t["direction"], "in");
        assert_eq!(t["ws_path"], "/chat");
        assert_eq!(t["qos"]["reliability"], "best_effort");
        assert_eq!(t["qos"]["durability"], "transient_local");
        assert_eq!(t["qos"]["history"]["kind"], "keep_all");
        assert_eq!(t["qos"]["history"]["depth"], 3);
        // The slug of the name, not of `ws_path`.
        assert_eq!(t["schema_url"], "/schema/chat/message.idl");
    }

    #[test]
    fn wsbridge_5_2_admin_endpoint_serves_catalog_and_healthz_and_metrics() {
        use std::io::{Read, Write};
        use std::net::TcpStream;
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.topics.push(TopicConfig {
            name: "T".into(),
            type_name: "T".into(),
            direction: "out".into(),
            ws_path: "/topics/t".into(),
            reliability: "reliable".into(),
            durability: "volatile".into(),
            history_depth: 5,
            ..TopicConfig::default()
        });
        let snap = Arc::new(CatalogSnapshot::from_config(&cfg));
        let reg = Arc::new(Registry::new());
        let metrics = BridgeMetrics::register(&reg);
        metrics.connections_total.add(42);

        let healthy = Arc::new(AtomicBool::new(true));
        let stop = Arc::new(AtomicBool::new(false));

        let (_h, bound) = serve_admin_endpoints(
            "127.0.0.1:0".parse().unwrap(),
            "/internal/metrics".to_string(),
            Arc::clone(&snap),
            Arc::clone(&reg),
            Arc::clone(&healthy),
            Arc::clone(&stop),
        )
        .expect("spawn admin");

        // /catalog
        let mut s =
            TcpStream::connect_timeout(&bound, Duration::from_secs(2)).expect("connect catalog");
        s.set_read_timeout(Some(Duration::from_secs(2))).ok();
        s.write_all(b"GET /catalog HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap();
        let mut body = String::new();
        s.read_to_string(&mut body).ok();
        assert!(body.contains("HTTP/1.1 200"));
        assert!(body.contains("\"name\":\"T\""), "got: {body}");

        // /healthz
        let mut s =
            TcpStream::connect_timeout(&bound, Duration::from_secs(2)).expect("connect health");
        s.set_read_timeout(Some(Duration::from_secs(2))).ok();
        s.write_all(b"GET /healthz HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap();
        let mut body = String::new();
        s.read_to_string(&mut body).ok();
        assert!(body.contains("HTTP/1.1 200"));
        assert!(body.contains("OK"));

        // /metrics
        let mut s =
            TcpStream::connect_timeout(&bound, Duration::from_secs(2)).expect("connect metrics");
        s.set_read_timeout(Some(Duration::from_secs(2))).ok();
        s.write_all(b"GET /internal/metrics HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap();
        let mut body = String::new();
        s.read_to_string(&mut body).ok();
        assert!(
            body.contains("zerodds_ws_bridge_connections_total 42"),
            "got: {body}"
        );

        // `metrics.path` moved the metrics: the default path is not served.
        let mut s =
            TcpStream::connect_timeout(&bound, Duration::from_secs(2)).expect("connect metrics");
        s.set_read_timeout(Some(Duration::from_secs(2))).ok();
        s.write_all(b"GET /metrics HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap();
        let mut body = String::new();
        s.read_to_string(&mut body).ok();
        assert!(body.starts_with("HTTP/1.1 404"), "got: {body}");

        // /healthz down
        healthy.store(false, Ordering::SeqCst);
        let mut s =
            TcpStream::connect_timeout(&bound, Duration::from_secs(2)).expect("connect health2");
        s.set_read_timeout(Some(Duration::from_secs(2))).ok();
        s.write_all(b"GET /healthz HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap();
        let mut body = String::new();
        s.read_to_string(&mut body).ok();
        assert!(body.contains("HTTP/1.1 503"));

        stop.store(true, Ordering::SeqCst);
    }

    #[test]
    fn wsbridge_8_3_otlp_config_from_endpoint_parses_http_url() {
        let c = otlp_config_from_endpoint("svc-1", "http://collector.local:4318");
        assert_eq!(c.host, "collector.local");
        assert_eq!(c.port, 4318);
        assert_eq!(c.service_name, "svc-1");
    }

    #[test]
    fn wsbridge_8_3_otlp_config_from_endpoint_parses_bare_host_port() {
        let c = otlp_config_from_endpoint("svc", "host:9999");
        assert_eq!(c.host, "host");
        assert_eq!(c.port, 9999);
    }

    #[test]
    fn wsbridge_8_3_otlp_config_from_endpoint_handles_https_and_trailing_slash() {
        let c = otlp_config_from_endpoint("svc", "https://otel.svc.local:4318/");
        assert_eq!(c.host, "otel.svc.local");
        assert_eq!(c.port, 4318);
    }

    /// Spec §11 with §9.2: off Unix the watcher turns the requests of the
    /// Windows service control manager into the flags of the signals; run
    /// here in a thread of its own, as `install_signal_watcher` runs it on
    /// Windows. Without a request it waits, and it ends when the daemon
    /// that held the shutdown flag has gone; a `paramchange` sets the
    /// reload flag and the watcher goes on; a stop request sets the
    /// shutdown flag and ends it. One test: the requests are the process's.
    #[test]
    fn wsbridge_11_the_service_requests_set_the_shutdown_and_reload_flags() {
        let ended = |watcher: &JoinHandle<()>| {
            let until = std::time::Instant::now() + Duration::from_secs(5);
            while !watcher.is_finished() && std::time::Instant::now() < until {
                thread::sleep(Duration::from_millis(10));
            }
            watcher.is_finished()
        };
        let shutdown = Arc::new(AtomicBool::new(false));
        let reload = Arc::new(AtomicBool::new(false));
        let watcher = {
            let (shutdown, reload) = (Arc::clone(&shutdown), Arc::clone(&reload));
            thread::spawn(move || watch_service_requests(&shutdown, &reload))
        };
        thread::sleep(Duration::from_millis(300));
        assert!(!watcher.is_finished(), "no request: the watcher waits");
        drop(shutdown);
        assert!(ended(&watcher), "the watcher ends with the daemon");

        let shutdown = Arc::new(AtomicBool::new(false));
        let watcher = {
            let (shutdown, reload) = (Arc::clone(&shutdown), Arc::clone(&reload));
            thread::spawn(move || watch_service_requests(&shutdown, &reload))
        };
        zerodds_dcps::cli::request_service_reload();
        let until = std::time::Instant::now() + Duration::from_secs(5);
        while !reload.load(Ordering::SeqCst) && std::time::Instant::now() < until {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(reload.load(Ordering::SeqCst), "the reload request");
        assert!(!shutdown.load(Ordering::SeqCst));
        assert!(!watcher.is_finished());
        zerodds_dcps::cli::request_service_stop();
        assert!(ended(&watcher), "the watcher ends with the stop");
        assert!(shutdown.load(Ordering::SeqCst), "the stop request");
    }

    #[test]
    fn wsbridge_8_3_otlp_config_from_endpoint_falls_back_to_default_port() {
        let c = otlp_config_from_endpoint("svc", "host-only");
        assert_eq!(c.host, "host-only");
        assert_eq!(c.port, 4318);
    }
}
