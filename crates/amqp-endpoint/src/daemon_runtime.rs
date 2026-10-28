// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-cutting daemon runtime for the AMQP daemon.
//!
//! Provides standard counters (§8.2 Prometheus), `/catalog`-/`/healthz`-
//! /`/metrics`-Endpoint (§5.2), Signal-Watcher (§9.2), OTLP-Exporter
//! (§8.3) — all as generic ones, wired up in the binary main loop
//! Components.

#![allow(clippy::print_stderr)]

use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

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
}

impl BridgeMetrics {
    /// Registers the standard set.
    pub fn register(registry: &Registry) -> Self {
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
            frames_in_total: registry.counter("zerodds_amqp_frames_in_total", Labels::new()),
            frames_out_total: registry.counter("zerodds_amqp_frames_out_total", Labels::new()),
            bytes_in_total: registry.counter("zerodds_amqp_bytes_in_total", Labels::new()),
            bytes_out_total: registry.counter("zerodds_amqp_bytes_out_total", Labels::new()),
            connections_active: registry.gauge("zerodds_amqp_connections_active", Labels::new()),
            connections_total: registry.counter("zerodds_amqp_connections_total", Labels::new()),
            dds_samples_in_total: registry
                .counter("zerodds_amqp_dds_samples_in_total", Labels::new()),
            dds_samples_out_total: registry
                .counter("zerodds_amqp_dds_samples_out_total", Labels::new()),
            errors_total: registry.counter("zerodds_amqp_errors_total", Labels::new()),
            registry: None,
            spec: None,
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
            out.spec = Some(Arc::new(SpecMetrics::register(r, topics, sasl_mechanism)));
        }
        out
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
    fn register(registry: &Arc<Registry>, topics: &[String], sasl_mechanism: &str) -> Self {
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
        let topic = |t: &String| Labels::new().with("dds_topic", t.clone());
        let conn = |state: &str| {
            registry.counter(
                "zerodds_amqp_bridge_connections_total",
                Labels::new().with("state", state.to_string()),
            )
        };
        let role = |r: &str| {
            registry.gauge(
                "zerodds_amqp_bridge_links_active",
                Labels::new().with("role", r.to_string()),
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
            dispositions_accepted: registry.counter(
                "zerodds_amqp_bridge_dispositions_accepted_total",
                Labels::new(),
            ),
            dispositions_released: registry.counter(
                "zerodds_amqp_bridge_dispositions_released_total",
                Labels::new(),
            ),
            credit_topup: registry.counter("zerodds_amqp_bridge_credit_topup_total", Labels::new()),
            sasl_failures: registry.counter(
                "zerodds_amqp_bridge_sasl_failures_total",
                Labels::new().with("mechanism", sasl_mechanism.to_string()),
            ),
            registry: Arc::clone(registry),
        }
    }

    /// A delivery settled `rejected` with the AMQP error condition `condition`.
    pub fn rejected(&self, condition: Option<&str>) {
        let reason = match condition {
            Some(c) if c.starts_with("amqp:") && c.len() <= 64 => c.to_string(),
            Some(_) => "other".to_string(),
            None => "none".to_string(),
        };
        self.registry
            .counter(
                "zerodds_amqp_bridge_dispositions_rejected_total",
                Labels::new().with("reason", reason),
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
            out.push_str("\"}");
        }
        out.push_str("]}");
        out
    }
}

fn push_json_str(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
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
    let listener = TcpListener::bind(addr)?;
    let bound = listener.local_addr()?;
    listener.set_nonblocking(true)?;
    let h = thread::Builder::new()
        .name("zerodds-amqp-admin".into())
        .spawn(move || {
            loop {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                match listener.accept() {
                    Ok((mut s, _peer)) => {
                        let _ = s.set_nonblocking(false);
                        let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
                        let _ = s.set_write_timeout(Some(Duration::from_secs(2)));
                        admin_handle(&mut s, &catalog, &registry, &healthy);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(_) => thread::sleep(Duration::from_millis(100)),
                }
            }
        })?;
    Ok((h, bound))
}

fn admin_handle(
    stream: &mut std::net::TcpStream,
    catalog: &CatalogSnapshot,
    registry: &Registry,
    healthy: &AtomicBool,
) {
    use std::io::{Read, Write};
    let mut buf = [0u8; 1024];
    let n = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return,
    };
    let req = String::from_utf8_lossy(&buf[..n]);
    let first = req.lines().next().unwrap_or("");
    let path = first
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/");
    let (status, ctype, body) = match path {
        "/catalog" => ("200 OK", "application/json", catalog.render_json()),
        "/healthz" => {
            if healthy.load(Ordering::SeqCst) {
                ("200 OK", "text/plain; charset=utf-8", "OK\n".to_string())
            } else {
                (
                    "503 Service Unavailable",
                    "text/plain; charset=utf-8",
                    "DOWN\n".to_string(),
                )
            }
        }
        "/metrics" => (
            "200 OK",
            "text/plain; version=0.0.4; charset=utf-8",
            registry.render_prometheus(),
        ),
        "/" => (
            "200 OK",
            "text/plain; charset=utf-8",
            format!("{SERVICE_NAME}\nendpoints: /catalog /healthz /metrics\n"),
        ),
        _ => ("404 Not Found", "text/plain; charset=utf-8", String::new()),
    };
    let resp = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: {ctype}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes());
}

/// Parse endpoint string.
#[must_use]
pub fn otlp_config_from_endpoint(service_name: &str, raw: &str) -> OtlpConfig {
    let trimmed = raw
        .strip_prefix("http://")
        .or_else(|| raw.strip_prefix("https://"))
        .unwrap_or(raw)
        .trim_end_matches('/');
    let (host, port) = match trimmed.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().unwrap_or(4318)),
        None => (trimmed.to_string(), 4318),
    };
    OtlpConfig {
        host,
        port,
        service_name: service_name.into(),
        service_version: SERVICE_VERSION.into(),
        timeout: Duration::from_secs(5),
    }
}

/// Aus ENV.
#[must_use]
pub fn otlp_config_from_env(service_name: &str) -> Option<OtlpConfig> {
    let raw = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok()?;
    Some(otlp_config_from_endpoint(service_name, &raw))
}

/// Periodischer flush.
pub fn spawn_otlp_flush_loop(
    exporter: Arc<OtlpExporter>,
    stop: Arc<AtomicBool>,
    interval: Duration,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("zerodds-amqp-otlp".into())
        .spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                thread::sleep(interval);
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let _ = exporter.flush();
            }
            let _ = exporter.flush();
        })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

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
    fn otlp_config_parses_endpoint() {
        let c = otlp_config_from_endpoint("svc", "http://collector:4318");
        assert_eq!(c.host, "collector");
        assert_eq!(c.port, 4318);
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
