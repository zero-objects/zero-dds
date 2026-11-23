// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-cutting daemon runtime for `zerodds-coap-bridged`.

#![allow(clippy::print_stderr)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use zerodds_monitor::admin::AdminEndpoints;
use zerodds_monitor::json::push_escaped as push_json_str;
use zerodds_monitor::{Counter, Gauge, Labels, Registry};
use zerodds_observability_otlp::{OtlpConfig, OtlpExporter};

use super::config::{DaemonConfig, TopicConfig};

/// Service-Name.
pub const SERVICE_NAME: &str = "zerodds-coap-bridged";
/// Crate-Version.
pub const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The daemon's metrics (spec §8.2): the ten families of §8.2, with their
/// labels, and the datagram counters every bridge daemon has.
#[derive(Clone)]
pub struct BridgeMetrics {
    registry: Arc<Registry>,
    /// Incoming CoAP datagrams.
    pub frames_in_total: Arc<Counter>,
    /// Outgoing CoAP datagrams.
    pub frames_out_total: Arc<Counter>,
    /// Bytes in.
    pub bytes_in_total: Arc<Counter>,
    /// Bytes out.
    pub bytes_out_total: Arc<Counter>,
    /// Active Observe subscriptions, over all topics.
    pub connections_active: Arc<Gauge>,
    /// Observe registrations and DTLS sessions since the start.
    pub connections_total: Arc<Counter>,
    /// CoAP → DDS samples.
    pub dds_samples_in_total: Arc<Counter>,
    /// DDS → CoAP samples.
    pub dds_samples_out_total: Arc<Counter>,
    /// Wire and codec errors.
    pub errors_total: Arc<Counter>,
    /// `zerodds_coap_bridge_oscore_replays_total` (RFC 8613 §7.4).
    pub oscore_replays_total: Arc<Counter>,
}

/// The families of §8.2.
pub mod family {
    /// `counter{method, code}`.
    pub const REQUESTS: &str = "zerodds_coap_bridge_requests_total";
    /// `gauge{topic}`.
    pub const OBSERVERS: &str = "zerodds_coap_bridge_observers_active";
    /// `counter{topic}`.
    pub const NOTIFIES: &str = "zerodds_coap_bridge_notifies_sent_total";
    /// `counter{topic}`.
    pub const BLOCKS_IN: &str = "zerodds_coap_bridge_blocks_in_total";
    /// `counter{topic}`.
    pub const BLOCKS_OUT: &str = "zerodds_coap_bridge_blocks_out_total";
    /// `counter{result}`.
    pub const HANDSHAKES: &str = "zerodds_coap_bridge_dtls_handshakes_total";
    /// `counter`.
    pub const OSCORE_REPLAYS: &str = "zerodds_coap_bridge_oscore_replays_total";
    /// `counter{topic}`.
    pub const DDS_RECEIVED: &str = "zerodds_coap_bridge_dds_samples_received";
    /// `counter{topic}`.
    pub const DDS_PUBLISHED: &str = "zerodds_coap_bridge_dds_samples_published";
    /// `counter{reason}`.
    pub const ACL_DENIALS: &str = "zerodds_coap_bridge_acl_denials_total";
}

impl BridgeMetrics {
    /// Registers the set.
    pub fn register(registry: &Arc<Registry>) -> Self {
        for (name, help) in [
            ("zerodds_coap_frames_in_total", "CoAP datagrams received"),
            ("zerodds_coap_frames_out_total", "CoAP datagrams sent"),
            ("zerodds_coap_bytes_in_total", "CoAP bytes received"),
            ("zerodds_coap_bytes_out_total", "CoAP bytes sent"),
            (
                "zerodds_coap_connections_active",
                "Active observe subscriptions",
            ),
            (
                "zerodds_coap_connections_total",
                "Observe registrations and DTLS sessions",
            ),
            (
                "zerodds_coap_dds_samples_in_total",
                "Samples written into DDS",
            ),
            (
                "zerodds_coap_dds_samples_out_total",
                "Samples emitted from DDS",
            ),
            ("zerodds_coap_errors_total", "Wire/codec errors"),
            (
                family::REQUESTS,
                "CoAP requests by method and response code",
            ),
            (family::OBSERVERS, "Active observers per topic"),
            (family::NOTIFIES, "Notifications sent per topic"),
            (family::BLOCKS_IN, "Block1 blocks received per topic"),
            (family::BLOCKS_OUT, "Block2 blocks sent per topic"),
            (family::HANDSHAKES, "DTLS handshakes by result"),
            (family::OSCORE_REPLAYS, "Rejected OSCORE replays"),
            (family::DDS_RECEIVED, "DDS samples received per topic"),
            (family::DDS_PUBLISHED, "DDS samples published per topic"),
            (family::ACL_DENIALS, "Requests the ACL denied, by reason"),
        ] {
            registry.set_help(name, help);
        }
        Self {
            registry: Arc::clone(registry),
            frames_in_total: registry.counter("zerodds_coap_frames_in_total", Labels::new()),
            frames_out_total: registry.counter("zerodds_coap_frames_out_total", Labels::new()),
            bytes_in_total: registry.counter("zerodds_coap_bytes_in_total", Labels::new()),
            bytes_out_total: registry.counter("zerodds_coap_bytes_out_total", Labels::new()),
            connections_active: registry.gauge("zerodds_coap_connections_active", Labels::new()),
            connections_total: registry.counter("zerodds_coap_connections_total", Labels::new()),
            dds_samples_in_total: registry
                .counter("zerodds_coap_dds_samples_in_total", Labels::new()),
            dds_samples_out_total: registry
                .counter("zerodds_coap_dds_samples_out_total", Labels::new()),
            errors_total: registry.counter("zerodds_coap_errors_total", Labels::new()),
            oscore_replays_total: registry.counter(family::OSCORE_REPLAYS, Labels::new()),
        }
    }

    fn count(&self, family: &'static str, key: &'static str, value: &str) {
        self.registry
            .counter(family, Labels::new().with(key, value))
            .inc();
    }

    /// A response to a request: `requests_total{method, code}`.
    pub fn request(&self, method: &str, code: &str) {
        self.registry
            .counter(
                family::REQUESTS,
                Labels::new().with("method", method).with("code", code),
            )
            .inc();
    }

    /// An observer of `topic` registered.
    pub fn observer_added(&self, topic: &str) {
        self.connections_total.inc();
        self.connections_active.inc();
        self.registry
            .gauge(family::OBSERVERS, Labels::new().with("topic", topic))
            .inc();
    }

    /// An observer of `topic` is gone.
    pub fn observer_removed(&self, topic: &str) {
        self.connections_active.dec();
        self.registry
            .gauge(family::OBSERVERS, Labels::new().with("topic", topic))
            .dec();
    }

    /// A notification of `topic` went out.
    pub fn notified(&self, topic: &str) {
        self.count(family::NOTIFIES, "topic", topic);
    }

    /// A Block1 block of `topic` came in.
    pub fn block_in(&self, topic: &str) {
        self.count(family::BLOCKS_IN, "topic", topic);
    }

    /// A Block2 block of `topic` went out.
    pub fn block_out(&self, topic: &str) {
        self.count(family::BLOCKS_OUT, "topic", topic);
    }

    /// A DTLS handshake ended with `result` (`ok` or `failed`).
    pub fn handshake(&self, result: &str) {
        self.count(family::HANDSHAKES, "result", result);
    }

    /// A sample of `topic` came from DDS.
    pub fn dds_received(&self, topic: &str) {
        self.dds_samples_out_total.inc();
        self.count(family::DDS_RECEIVED, "topic", topic);
    }

    /// A sample of `topic` was written to DDS.
    pub fn dds_published(&self, topic: &str) {
        self.dds_samples_in_total.inc();
        self.count(family::DDS_PUBLISHED, "topic", topic);
    }

    /// The ACL denied a request, for `reason` (`read`, `write`,
    /// `unauthenticated`).
    pub fn acl_denied(&self, reason: &str) {
        self.count(family::ACL_DENIALS, "reason", reason);
    }
}

/// SIGTERM/SIGINT/SIGHUP-Watcher.
#[cfg(unix)]
pub fn install_signal_watcher(
    shutdown_flag: Arc<AtomicBool>,
    reload_flag: Arc<AtomicBool>,
) -> std::io::Result<JoinHandle<()>> {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    use signal_hook::iterator::Signals;
    let mut signals = Signals::new([SIGTERM, SIGINT, SIGHUP])?;
    thread::Builder::new()
        .name("zerodds-coap-signals".into())
        .spawn(move || {
            for sig in signals.forever() {
                match sig {
                    SIGTERM | SIGINT => {
                        shutdown_flag.store(true, Ordering::SeqCst);
                        break;
                    }
                    SIGHUP => reload_flag.store(true, Ordering::SeqCst),
                    _ => {}
                }
            }
        })
}

/// Catalog-Snapshot.
#[derive(Clone)]
pub struct CatalogSnapshot {
    /// Service.
    pub service: String,
    /// Version.
    pub version: String,
    /// Topics.
    pub topics: Vec<TopicConfig>,
    /// The tenants (spec §1 L6): name, domain, topics.
    pub tenants: Vec<(String, i32, Vec<TopicConfig>)>,
}

impl CatalogSnapshot {
    /// Constructor.
    #[must_use]
    pub fn from_config(cfg: &DaemonConfig) -> Self {
        Self {
            service: SERVICE_NAME.into(),
            version: SERVICE_VERSION.into(),
            topics: cfg.topics.clone(),
            tenants: cfg
                .tenants
                .iter()
                .map(|t| (t.name.clone(), t.domain, t.topics.clone()))
                .collect(),
        }
    }

    /// JSON.
    #[must_use]
    pub fn render_json(&self) -> String {
        let mut out = String::with_capacity(256 + self.topics.len() * 128);
        out.push_str("{\"service\":\"");
        push_json_str(&mut out, &self.service);
        out.push_str("\",\"version\":\"");
        push_json_str(&mut out, &self.version);
        out.push_str("\",\"topics\":");
        push_topics(&mut out, &self.topics);
        out.push_str(",\"tenants\":[");
        for (i, (name, domain, topics)) in self.tenants.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str("{\"name\":\"");
            push_json_str(&mut out, name);
            out.push_str("\",\"domain\":");
            out.push_str(&domain.to_string());
            out.push_str(",\"topics\":");
            push_topics(&mut out, topics);
            out.push('}');
        }
        out.push_str("]}");
        out
    }
}

fn push_topics(out: &mut String, topics: &[TopicConfig]) {
    out.push('[');
    for (i, t) in topics.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"dds_name\":\"");
        push_json_str(out, &t.dds_name);
        out.push_str("\",\"dds_type\":\"");
        push_json_str(out, &t.dds_type);
        out.push_str("\",\"coap_uri_path\":\"");
        push_json_str(out, &t.coap_uri_path);
        out.push_str("\",\"direction\":\"");
        push_json_str(out, &t.direction);
        out.push_str("\",\"qos\":{\"reliability\":\"");
        push_json_str(out, &t.reliability);
        out.push_str("\",\"durability\":\"");
        push_json_str(out, &t.durability);
        out.push_str("\",\"history_depth\":");
        out.push_str(&t.history_depth.to_string());
        out.push_str("}}");
    }
    out.push(']');
}

/// Admin HTTP server (TCP, since the daemon path itself is UDP/CoAP).
pub fn serve_admin_endpoints(
    addr: SocketAddr,
    catalog: Arc<CatalogSnapshot>,
    registry: Arc<Registry>,
    healthy: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    metrics_path: String,
) -> std::io::Result<(JoinHandle<()>, SocketAddr)> {
    zerodds_monitor::admin::serve_admin_endpoints(
        addr,
        "zerodds-coap-admin",
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

/// Endpoint-Parser.
#[must_use]
pub fn otlp_config_from_endpoint(service_name: &str, raw: &str) -> OtlpConfig {
    OtlpConfig::from_endpoint(service_name, SERVICE_VERSION, raw)
}

/// Aus ENV.
#[must_use]
pub fn otlp_config_from_env(service_name: &str) -> Option<OtlpConfig> {
    OtlpConfig::from_env(service_name, SERVICE_VERSION)
}

/// Spec §8.3: the spans of the daemon, sent when
/// `OTEL_EXPORTER_OTLP_ENDPOINT` is set.
#[derive(Clone, Default)]
pub struct Tracer {
    exporter: Option<Arc<OtlpExporter>>,
}

/// One CoAP exchange as a span.
#[derive(Debug, Clone, Default)]
pub struct ExchangeSpan {
    /// When the request arrived, in nanoseconds since the Unix epoch.
    pub start_ns: u64,
    /// When its response left.
    pub end_ns: u64,
    /// Whether the response is no error (2.xx).
    pub ok: bool,
    /// `coap.method`, `coap.code`, `coap.uri`, `net.peer`, `dds.topic`.
    pub attributes: Vec<(&'static str, String)>,
    /// The DDS write the request made: its start and end.
    pub write: Option<(u64, u64)>,
}

impl Tracer {
    /// A tracer sending to `exporter`; none sends nothing.
    #[must_use]
    pub fn new(exporter: Option<Arc<OtlpExporter>>) -> Self {
        Self { exporter }
    }

    /// Whether spans go anywhere.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.exporter.is_some()
    }

    /// The exporter, for the thread that flushes it.
    #[must_use]
    pub fn exporter(&self) -> Option<Arc<OtlpExporter>> {
        self.exporter.clone()
    }

    /// Spec §8.3: the span `coap.request` (`SpanKind::Server`) of the
    /// exchange and, when it wrote a sample, its child `dds.write`
    /// (`SpanKind::Client`).
    pub fn exchange(&self, span: ExchangeSpan) {
        use zerodds_foundation::observability::Attribute;
        use zerodds_foundation::tracing::{Span, SpanContext, SpanKind, SpanStatus};
        let Some(exporter) = &self.exporter else {
            return;
        };
        let root = SpanContext::new_root(new_trace_id(), new_span_id());
        let status = if span.ok {
            SpanStatus::Ok
        } else {
            SpanStatus::Error
        };
        let attributes: Vec<Attribute> = span
            .attributes
            .iter()
            .map(|(key, value)| Attribute {
                key,
                value: value.clone(),
            })
            .collect();
        if let Some((start, end)) = span.write {
            exporter.add_span(Span {
                context: SpanContext::child_of(&root, new_span_id()),
                name: "dds.write".to_string(),
                kind: SpanKind::Client,
                start_unix_ns: start,
                end_unix_ns: end,
                status,
                status_description: None,
                attributes: attributes
                    .iter()
                    .filter(|a| a.key == "dds.topic")
                    .cloned()
                    .collect(),
            });
        }
        exporter.add_span(Span {
            context: root,
            name: "coap.request".to_string(),
            kind: SpanKind::Server,
            start_unix_ns: span.start_ns,
            end_unix_ns: span.end_ns,
            status,
            status_description: None,
            attributes,
        });
    }
}

/// Nanoseconds since the Unix epoch.
#[must_use]
pub fn unix_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
}

/// Mixed into the span IDs: they only need to be told apart by a
/// collector.
static SPAN_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn new_trace_id() -> zerodds_foundation::tracing::TraceId {
    let mut bytes = [0u8; 16];
    if getrandom::getrandom(&mut bytes).is_err() || bytes.iter().all(|b| *b == 0) {
        bytes = u128::from(unix_ns()).max(1).to_be_bytes();
    }
    zerodds_foundation::tracing::TraceId(bytes)
}

fn new_span_id() -> zerodds_foundation::tracing::SpanId {
    let n = SPAN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mixed = n ^ (u64::from(std::process::id()).rotate_left(32));
    let mut bytes = mixed.to_be_bytes();
    if bytes.iter().all(|b| *b == 0) {
        bytes[7] = 1;
    }
    zerodds_foundation::tracing::SpanId(bytes)
}

/// Periodischer flush-Thread.
pub fn spawn_otlp_flush_loop(
    exporter: Arc<OtlpExporter>,
    stop: Arc<AtomicBool>,
    interval: Duration,
) -> std::io::Result<JoinHandle<()>> {
    zerodds_observability_otlp::spawn_flush_loop(
        exporter,
        stop,
        interval,
        interval,
        "zerodds-coap-otlp",
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn coapbridge_8_2_metrics_register_idempotent() {
        let r = Arc::new(Registry::new());
        let m1 = BridgeMetrics::register(&r);
        let m2 = BridgeMetrics::register(&r);
        m1.frames_in_total.inc();
        assert_eq!(m2.frames_in_total.get(), 1);
    }

    #[test]
    fn coapbridge_8_2_catalog_render_contains_topic_fields() {
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.topics.push(TopicConfig {
            dds_name: "Sensor".into(),
            dds_type: "Sensor".into(),
            coap_uri_path: "sensors/data".into(),
            direction: "out".into(),
            reliability: "reliable".into(),
            durability: "volatile".into(),
            history_depth: 10,
            ..Default::default()
        });
        let snap = CatalogSnapshot::from_config(&cfg);
        let j = snap.render_json();
        assert!(j.contains("zerodds-coap-bridged"));
        assert!(j.contains("\"coap_uri_path\":\"sensors/data\""));
    }

    #[test]
    fn coapbridge_8_2_admin_endpoint_serves_metrics() {
        use std::io::{Read, Write};
        use std::net::TcpStream;
        let cfg = DaemonConfig::default_for_dev();
        let snap = Arc::new(CatalogSnapshot::from_config(&cfg));
        let reg = Arc::new(Registry::new());
        let m = BridgeMetrics::register(&reg);
        m.frames_in_total.add(11);
        m.request("POST", "2.04");
        m.observer_added("Chat::Message");
        m.notified("Chat::Message");
        m.block_in("Chat::Message");
        m.block_out("Chat::Message");
        m.handshake("ok");
        m.oscore_replays_total.inc();
        m.dds_received("Chat::Message");
        m.dds_published("Chat::Message");
        m.acl_denied("write");
        let healthy = Arc::new(AtomicBool::new(true));
        let stop = Arc::new(AtomicBool::new(false));
        let (_h, bound) = serve_admin_endpoints(
            "127.0.0.1:0".parse().unwrap(),
            Arc::clone(&snap),
            Arc::clone(&reg),
            Arc::clone(&healthy),
            Arc::clone(&stop),
            "/prom".to_string(),
        )
        .expect("spawn");
        let mut s = TcpStream::connect_timeout(&bound, Duration::from_secs(2)).expect("conn");
        s.set_read_timeout(Some(Duration::from_secs(2))).ok();
        s.write_all(b"GET /prom HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap();
        let mut body = String::new();
        s.read_to_string(&mut body).ok();
        assert!(body.contains("zerodds_coap_frames_in_total 11"));
        // Spec §8.2: the ten families with their labels.
        for line in [
            "zerodds_coap_bridge_requests_total{code=\"2.04\",method=\"POST\"} 1",
            "zerodds_coap_bridge_observers_active{topic=\"Chat::Message\"} 1",
            "zerodds_coap_bridge_notifies_sent_total{topic=\"Chat::Message\"} 1",
            "zerodds_coap_bridge_blocks_in_total{topic=\"Chat::Message\"} 1",
            "zerodds_coap_bridge_blocks_out_total{topic=\"Chat::Message\"} 1",
            "zerodds_coap_bridge_dtls_handshakes_total{result=\"ok\"} 1",
            "zerodds_coap_bridge_oscore_replays_total 1",
            "zerodds_coap_bridge_dds_samples_received{topic=\"Chat::Message\"} 1",
            "zerodds_coap_bridge_dds_samples_published{topic=\"Chat::Message\"} 1",
            "zerodds_coap_bridge_acl_denials_total{reason=\"write\"} 1",
        ] {
            assert!(body.contains(line), "{line} in\n{body}");
        }
        stop.store(true, Ordering::SeqCst);
    }
}

#[cfg(not(unix))]
pub fn install_signal_watcher(
    _shutdown_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    _reload_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    // Windows: signal_hook::iterator is POSIX-only. Spawn a dummy thread,
    // shutdown runs over the normal socket-close paths.
    Ok(std::thread::spawn(|| {}))
}
