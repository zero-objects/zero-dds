// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-cutting daemon runtime for the AMQP daemon.
//!
//! Provides standard counters (§8.2 Prometheus), `/catalog`-/`/healthz`-
//! /`/metrics`-Endpoint (§5.2), Signal-Watcher (§9.2), OTLP-Exporter
//! (§8.3) — all as generic ones, wired up in the binary main loop
//! Components.

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

/// Service name (used in the catalog + OTel resource).
pub const SERVICE_NAME: &str = "zerodds-amqp-bridged";
/// Crate-Version.
pub const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Standard metric set for the AMQP daemon.
#[derive(Clone)]
pub struct BridgeMetrics {
    /// Incoming AMQP frames.
    pub frames_in_total: Arc<Counter>,
    /// Outgoing AMQP frames.
    pub frames_out_total: Arc<Counter>,
    /// Bytes in.
    pub bytes_in_total: Arc<Counter>,
    /// Bytes out.
    pub bytes_out_total: Arc<Counter>,
    /// Active broker connections.
    pub connections_active: Arc<Gauge>,
    /// Lifetime broker connection attempts.
    pub connections_total: Arc<Counter>,
    /// AMQP → DDS Samples.
    pub dds_samples_in_total: Arc<Counter>,
    /// DDS → AMQP Samples.
    pub dds_samples_out_total: Arc<Counter>,
    /// Wire-Errors.
    pub errors_total: Arc<Counter>,
    /// The registry the families of spec §8.2 go into once the topics are
    /// known; `None` from [`Self::register`].
    pub registry: Option<Arc<Registry>>,
    /// Spec §8.2 (`zerodds_amqp_bridge_*`), after [`Self::with_topics`].
    pub spec: Option<Arc<SpecMetrics>>,
    /// The tenant (§1 L6) whose label every family carries; `None` for
    /// the daemon's own participant.
    pub tenant: Option<String>,
}

impl BridgeMetrics {
    /// Registers the standard set.
    pub fn register(registry: &Registry) -> Self {
        Self::register_labelled(registry, None)
    }

    /// [`Self::register`] with the label `tenant` on every family when
    /// `tenant` is set (§1 L6).
    fn register_labelled(registry: &Registry, tenant: Option<&str>) -> Self {
        let base = || tenant.map_or_else(Labels::new, |t| Labels::new().with("tenant", t));
        registry.set_help("zerodds_amqp_frames_in_total", "AMQP frames received");
        registry.set_help("zerodds_amqp_frames_out_total", "AMQP frames sent");
        registry.set_help("zerodds_amqp_bytes_in_total", "AMQP bytes received");
        registry.set_help("zerodds_amqp_bytes_out_total", "AMQP bytes sent");
        registry.set_help(
            "zerodds_amqp_connections_active",
            "Active broker connections",
        );
        registry.set_help("zerodds_amqp_connections_total", "Lifetime broker connects");
        registry.set_help(
            "zerodds_amqp_dds_samples_in_total",
            "Samples written into DDS",
        );
        registry.set_help(
            "zerodds_amqp_dds_samples_out_total",
            "Samples emitted to AMQP",
        );
        registry.set_help("zerodds_amqp_errors_total", "Wire/codec errors");
        Self {
            frames_in_total: registry.counter("zerodds_amqp_frames_in_total", base()),
            frames_out_total: registry.counter("zerodds_amqp_frames_out_total", base()),
            bytes_in_total: registry.counter("zerodds_amqp_bytes_in_total", base()),
            bytes_out_total: registry.counter("zerodds_amqp_bytes_out_total", base()),
            connections_active: registry.gauge("zerodds_amqp_connections_active", base()),
            connections_total: registry.counter("zerodds_amqp_connections_total", base()),
            dds_samples_in_total: registry.counter("zerodds_amqp_dds_samples_in_total", base()),
            dds_samples_out_total: registry.counter("zerodds_amqp_dds_samples_out_total", base()),
            errors_total: registry.counter("zerodds_amqp_errors_total", base()),
            registry: None,
            spec: None,
            tenant: tenant.map(str::to_string),
        }
    }

    /// [`Self::register`], keeping the registry for the families of spec
    /// §8.2.
    pub fn register_shared(registry: &Arc<Registry>) -> Self {
        Self {
            registry: Some(Arc::clone(registry)),
            ..Self::register(registry)
        }
    }

    /// The same set with the families of spec §8.2 registered for these
    /// topics (index = topic index of the configuration) and the SASL
    /// mechanism of the configuration. Without a registry, unchanged.
    #[must_use]
    pub fn with_topics(&self, topics: &[String], sasl_mechanism: &str) -> Self {
        let mut out = self.clone();
        if let Some(r) = &self.registry {
            out.spec = Some(Arc::new(SpecMetrics::register(
                r,
                topics,
                sasl_mechanism,
                self.tenant.as_deref(),
            )));
        }
        out
    }

    /// The set of tenant `name` (§1 L6): every family again with the label
    /// `tenant`, in the same registry. Without a registry, unchanged.
    #[must_use]
    pub fn for_tenant(&self, name: &str) -> Self {
        match &self.registry {
            Some(r) => Self {
                registry: Some(Arc::clone(r)),
                ..Self::register_labelled(r, Some(name))
            },
            None => self.clone(),
        }
    }
}

/// Counters of one DDS topic (spec §8.2, label `dds_topic`).
pub struct TopicMetrics {
    /// AMQP transfers received for the topic (broker → DDS).
    pub transfers_in: Arc<Counter>,
    /// AMQP transfers sent for the topic (DDS → broker).
    pub transfers_out: Arc<Counter>,
    /// Message bytes received.
    pub bytes_in: Arc<Counter>,
    /// Message bytes sent.
    pub bytes_out: Arc<Counter>,
    /// DDS samples taken from the topic.
    pub dds_received: Arc<Counter>,
    /// DDS samples written to the topic.
    pub dds_published: Arc<Counter>,
}

/// Spec §8.2: `zerodds_amqp_bridge_*`. Label values come from the
/// configuration, and a rejection's `reason` is its AMQP error condition
/// when that is an `amqp:` symbol, else `other`: nothing a broker sends
/// grows the label set without bound.
pub struct SpecMetrics {
    registry: Arc<Registry>,
    /// The tenant label of every family (§1 L6).
    tenant: Option<String>,
    /// Per topic, by the configuration's topic index.
    pub topics: Vec<TopicMetrics>,
    /// `connections_total{state="established"}`.
    pub connections_established: Arc<Counter>,
    /// `connections_total{state="failed"}`: the connect failed.
    pub connections_failed: Arc<Counter>,
    /// `connections_total{state="lost"}`: an established connection ended.
    pub connections_lost: Arc<Counter>,
    /// `links_active{role="sender"}`.
    pub links_sender: Arc<Gauge>,
    /// `links_active{role="receiver"}`.
    pub links_receiver: Arc<Gauge>,
    /// Deliveries settled `accepted`, both directions.
    pub dispositions_accepted: Arc<Counter>,
    /// Deliveries settled `released` or `modified`, both directions.
    pub dispositions_released: Arc<Counter>,
    /// Link credit granted to the broker.
    pub credit_topup: Arc<Counter>,
    /// `sasl_failures_total{mechanism}` of the configured mechanism.
    pub sasl_failures: Arc<Counter>,
}

impl SpecMetrics {
    fn register(
        registry: &Arc<Registry>,
        topics: &[String],
        sasl_mechanism: &str,
        tenant: Option<&str>,
    ) -> Self {
        let base = || tenant.map_or_else(Labels::new, |t| Labels::new().with("tenant", t));
        for (name, help) in [
            (
                "zerodds_amqp_bridge_connections_total",
                "Broker connections by how they went",
            ),
            ("zerodds_amqp_bridge_links_active", "Attached links by role"),
            (
                "zerodds_amqp_bridge_transfers_in_total",
                "Transfers received from the broker",
            ),
            (
                "zerodds_amqp_bridge_transfers_out_total",
                "Transfers sent to the broker",
            ),
            (
                "zerodds_amqp_bridge_bytes_in_total",
                "Message bytes received from the broker",
            ),
            (
                "zerodds_amqp_bridge_bytes_out_total",
                "Message bytes sent to the broker",
            ),
            (
                "zerodds_amqp_bridge_dispositions_accepted_total",
                "Deliveries settled accepted, both directions",
            ),
            (
                "zerodds_amqp_bridge_dispositions_rejected_total",
                "Deliveries settled rejected, both directions",
            ),
            (
                "zerodds_amqp_bridge_dispositions_released_total",
                "Deliveries settled released or modified, both directions",
            ),
            (
                "zerodds_amqp_bridge_credit_topup_total",
                "Link credit grants to the broker",
            ),
            (
                "zerodds_amqp_bridge_dds_samples_received",
                "DDS samples taken for the broker",
            ),
            (
                "zerodds_amqp_bridge_dds_samples_published",
                "DDS samples written from the broker",
            ),
            (
                "zerodds_amqp_bridge_sasl_failures_total",
                "Connects refused in the SASL phase",
            ),
        ] {
            registry.set_help(name, help);
        }
        let topic = |t: &String| base().with("dds_topic", t.clone());
        let conn = |state: &str| {
            registry.counter(
                "zerodds_amqp_bridge_connections_total",
                base().with("state", state.to_string()),
            )
        };
        let role = |r: &str| {
            registry.gauge(
                "zerodds_amqp_bridge_links_active",
                base().with("role", r.to_string()),
            )
        };
        Self {
            topics: topics
                .iter()
                .map(|t| TopicMetrics {
                    transfers_in: registry
                        .counter("zerodds_amqp_bridge_transfers_in_total", topic(t)),
                    transfers_out: registry
                        .counter("zerodds_amqp_bridge_transfers_out_total", topic(t)),
                    bytes_in: registry.counter("zerodds_amqp_bridge_bytes_in_total", topic(t)),
                    bytes_out: registry.counter("zerodds_amqp_bridge_bytes_out_total", topic(t)),
                    dds_received: registry
                        .counter("zerodds_amqp_bridge_dds_samples_received", topic(t)),
                    dds_published: registry
                        .counter("zerodds_amqp_bridge_dds_samples_published", topic(t)),
                })
                .collect(),
            connections_established: conn("established"),
            connections_failed: conn("failed"),
            connections_lost: conn("lost"),
            links_sender: role("sender"),
            links_receiver: role("receiver"),
            dispositions_accepted: registry
                .counter("zerodds_amqp_bridge_dispositions_accepted_total", base()),
            dispositions_released: registry
                .counter("zerodds_amqp_bridge_dispositions_released_total", base()),
            credit_topup: registry.counter("zerodds_amqp_bridge_credit_topup_total", base()),
            sasl_failures: registry.counter(
                "zerodds_amqp_bridge_sasl_failures_total",
                base().with("mechanism", sasl_mechanism.to_string()),
            ),
            registry: Arc::clone(registry),
            tenant: tenant.map(str::to_string),
        }
    }

    /// A delivery settled `rejected` with the AMQP error condition `condition`.
    /// The label `reason` is the condition when it is one AMQP 1.0 defines
    /// (Part 2 §2.8.15 to §2.8.18, Part 4 §4.5.8), otherwise `other`, a
    /// rejection without an error included (spec §8.2): the label values
    /// stay a closed set however a broker names its conditions.
    pub fn rejected(&self, condition: Option<&str>) {
        let reason = condition
            .and_then(|c| crate::peer::condition::DEFINED.iter().find(|d| **d == c))
            .copied()
            .unwrap_or("other");
        let labels = self
            .tenant
            .as_deref()
            .map_or_else(Labels::new, |t| Labels::new().with("tenant", t));
        self.registry
            .counter(
                "zerodds_amqp_bridge_dispositions_rejected_total",
                labels.with("reason", reason),
            )
            .inc();
    }
}

/// Catalog topic entry — daemon-type-independent.
#[derive(Clone, Debug)]
pub struct CatalogTopic {
    /// DDS-Topic-Name.
    pub dds_name: String,
    /// AMQP-Address (queue:// / topic://).
    pub amqp_address: String,
    /// in/out/bidir.
    pub direction: String,
    /// The tenant (spec §1 L6) whose participant bridges the topic; `None`
    /// for the daemon's own.
    pub tenant: Option<String>,
}

/// Catalog-Snapshot.
#[derive(Clone, Debug)]
pub struct CatalogSnapshot {
    /// Service-Name.
    pub service: String,
    /// Version.
    pub version: String,
    /// Topics.
    pub topics: Vec<CatalogTopic>,
}

impl CatalogSnapshot {
    /// Constructor.
    #[must_use]
    pub fn new(topics: Vec<CatalogTopic>) -> Self {
        Self {
            service: SERVICE_NAME.into(),
            version: SERVICE_VERSION.into(),
            topics,
        }
    }

    /// JSON render for `/catalog`.
    #[must_use]
    pub fn render_json(&self) -> String {
        let mut out = String::with_capacity(256 + self.topics.len() * 96);
        out.push_str("{\"service\":\"");
        push_json_str(&mut out, &self.service);
        out.push_str("\",\"version\":\"");
        push_json_str(&mut out, &self.version);
        out.push_str("\",\"topics\":[");
        for (i, t) in self.topics.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str("{\"dds_name\":\"");
            push_json_str(&mut out, &t.dds_name);
            out.push_str("\",\"amqp_address\":\"");
            push_json_str(&mut out, &t.amqp_address);
            out.push_str("\",\"direction\":\"");
            push_json_str(&mut out, &t.direction);
            if let Some(tenant) = &t.tenant {
                out.push_str("\",\"tenant\":\"");
                push_json_str(&mut out, tenant);
            }
            out.push_str("\"}");
        }
        out.push_str("]}");
        out
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
        .name("zerodds-amqp-signals".into())
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

/// Admin-HTTP-Server `/catalog`/`/healthz`/`/metrics`.
pub fn serve_admin_endpoints(
    addr: SocketAddr,
    catalog: Arc<CatalogSnapshot>,
    registry: Arc<Registry>,
    healthy: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
) -> std::io::Result<(JoinHandle<()>, SocketAddr)> {
    zerodds_monitor::admin::serve_admin_endpoints(
        addr,
        "zerodds-amqp-admin",
        AdminEndpoints {
            service_name: SERVICE_NAME.into(),
            metrics_path: "/metrics".into(),
            catalog: move || catalog.render_json(),
            registry,
            healthy,
        },
        stop,
    )
}

/// Parse endpoint string.
#[must_use]
pub fn otlp_config_from_endpoint(service_name: &str, raw: &str) -> OtlpConfig {
    OtlpConfig::from_endpoint(service_name, SERVICE_VERSION, raw)
}

/// Aus ENV.
#[must_use]
pub fn otlp_config_from_env(service_name: &str) -> Option<OtlpConfig> {
    OtlpConfig::from_env(service_name, SERVICE_VERSION)
}

/// Flushes `exporter` every `interval` and once more when `stop` is set;
/// the thread ends within 100 ms of the stop, after that last flush (spec
/// §8.3).
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
        "zerodds-amqp-otlp",
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Spec §8.2: `reason` is the AMQP error condition when it is an `amqp:`
    /// symbol, otherwise `other`: a vendor symbol, a symbol longer than 64
    /// octets and a rejection without an error.
    #[test]
    fn amqpbridge_8_2_the_reason_label_is_the_condition_or_other() {
        let r = Arc::new(Registry::new());
        let m = BridgeMetrics::register_shared(&r).with_topics(&[], "PLAIN");
        let sm = m.spec.as_ref().unwrap();
        sm.rejected(Some("amqp:decode-error"));
        sm.rejected(Some("com.vendor:quota"));
        sm.rejected(None);
        sm.rejected(Some(&format!("amqp:{}", "x".repeat(60))));
        // An `amqp:` symbol AMQP 1.0 does not define is no label value.
        sm.rejected(Some("amqp:x1"));
        sm.rejected(Some("amqp:link:invented"));
        sm.rejected(Some("amqp:transaction:rollback"));
        sm.rejected(Some("amqp:session:window-violation"));
        let text = zerodds_monitor::render_prometheus(&r.snapshot());
        let line = |reason: &str| {
            text.lines()
                .find(|l| {
                    l.starts_with("zerodds_amqp_bridge_dispositions_rejected_total")
                        && l.contains(&format!("reason=\"{reason}\""))
                })
                .and_then(|l| l.rsplit(' ').next())
                .map(str::to_string)
        };
        assert_eq!(line("amqp:decode-error").as_deref(), Some("1"), "{text}");
        assert_eq!(
            line("amqp:transaction:rollback").as_deref(),
            Some("1"),
            "{text}"
        );
        assert_eq!(
            line("amqp:session:window-violation").as_deref(),
            Some("1"),
            "{text}"
        );
        assert_eq!(line("other").as_deref(), Some("5"), "{text}");
        assert!(line("none").is_none(), "{text}");
        assert!(!text.contains("com.vendor"), "{text}");
        assert!(!text.contains("amqp:x1"), "{text}");
        assert!(!text.contains("invented"), "{text}");
    }

    #[test]
    fn metrics_register_idempotent() {
        let r = Registry::new();
        let m1 = BridgeMetrics::register(&r);
        let m2 = BridgeMetrics::register(&r);
        m1.frames_in_total.inc();
        assert_eq!(m2.frames_in_total.get(), 1);
    }

    #[test]
    fn catalog_render_json_is_valid_shape() {
        let snap = CatalogSnapshot::new(vec![CatalogTopic {
            dds_name: "Sensor::Reading".into(),
            amqp_address: "queue://sensors".into(),
            direction: "out".into(),
            tenant: None,
        }]);
        let j = snap.render_json();
        assert!(j.contains("zerodds-amqp-bridged"));
        assert!(j.contains("\"dds_name\":\"Sensor::Reading\""));
        assert!(j.contains("\"amqp_address\":\"queue://sensors\""));
    }

    #[test]
    fn admin_endpoint_serves_metrics_path() {
        use std::io::{Read, Write};
        use std::net::TcpStream;
        let snap = Arc::new(CatalogSnapshot::new(Vec::new()));
        let reg = Arc::new(Registry::new());
        let m = BridgeMetrics::register(&reg);
        m.frames_in_total.add(13);
        let healthy = Arc::new(AtomicBool::new(true));
        let stop = Arc::new(AtomicBool::new(false));
        let (_h, bound) = serve_admin_endpoints(
            "127.0.0.1:0".parse().unwrap(),
            Arc::clone(&snap),
            Arc::clone(&reg),
            Arc::clone(&healthy),
            Arc::clone(&stop),
        )
        .expect("spawn");
        let mut s = TcpStream::connect_timeout(&bound, Duration::from_secs(2)).expect("conn");
        s.set_read_timeout(Some(Duration::from_secs(2))).ok();
        s.write_all(b"GET /metrics HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap();
        let mut body = String::new();
        s.read_to_string(&mut body).ok();
        assert!(body.contains("zerodds_amqp_frames_in_total 13"));
        stop.store(true, Ordering::SeqCst);
    }

    #[test]
    fn amqpbridge_8_3_otlp_config_parses_endpoint() {
        let c = otlp_config_from_endpoint("svc", "http://collector:4318");
        assert_eq!(c.host, "collector");
        assert_eq!(c.port, 4318);
    }
}

/// The console control handler of Windows (§9.2): CTRL_C_EVENT and
/// CTRL_BREAK_EVENT set the shutdown flag as SIGINT and SIGTERM do on Unix;
/// the drain of §9.2 follows. CTRL_CLOSE_EVENT and the logoff and shutdown
/// events set the flag too, but ctrlc's handler routine returns at once and
/// Windows then ends the process, before the drain. Windows has no SIGHUP:
/// a reload there is a restart. The returned thread ends at once; the
/// handler stays installed for the life of the process.
#[cfg(windows)]
pub fn install_signal_watcher(
    shutdown_flag: Arc<AtomicBool>,
    _reload_flag: Arc<AtomicBool>,
) -> std::io::Result<JoinHandle<()>> {
    ctrlc::set_handler(move || shutdown_flag.store(true, Ordering::SeqCst))
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    thread::Builder::new()
        .name("zerodds-amqp-signals".into())
        .spawn(|| {})
}

/// A platform with neither POSIX signals nor a Windows console: no stop
/// from outside; the program logs `signal_watcher_init_failed`.
#[cfg(not(any(unix, windows)))]
pub fn install_signal_watcher(
    _shutdown_flag: Arc<AtomicBool>,
    _reload_flag: Arc<AtomicBool>,
) -> std::io::Result<JoinHandle<()>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "no signals and no console control handler on this platform",
    ))
}
