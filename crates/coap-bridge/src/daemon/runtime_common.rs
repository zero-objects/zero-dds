// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-cutting daemon runtime for `zerodds-coap-bridged`.

#![allow(clippy::print_stderr)]

use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use zerodds_monitor::{Counter, Gauge, Labels, Registry};
use zerodds_observability_otlp::{OtlpConfig, OtlpExporter};

use super::config::{DaemonConfig, TopicConfig};

/// Service-Name.
pub const SERVICE_NAME: &str = "zerodds-coap-bridged";
/// Crate-Version.
pub const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Standard-Counter-Set.
#[derive(Clone)]
pub struct BridgeMetrics {
    /// Eingehende CoAP-Datagramme.
    pub frames_in_total: Arc<Counter>,
    /// Ausgehende CoAP-Datagramme.
    pub frames_out_total: Arc<Counter>,
    /// Bytes in.
    pub bytes_in_total: Arc<Counter>,
    /// Bytes out.
    pub bytes_out_total: Arc<Counter>,
    /// Aktive Observe-Subscriptions.
    pub connections_active: Arc<Gauge>,
    /// Lifetime accept-Counter (Observer-Setup-Counter).
    pub connections_total: Arc<Counter>,
    /// CoAP → DDS Samples.
    pub dds_samples_in_total: Arc<Counter>,
    /// DDS → CoAP Samples.
    pub dds_samples_out_total: Arc<Counter>,
    /// Wire-Errors.
    pub errors_total: Arc<Counter>,
}

impl BridgeMetrics {
    /// Registriert den Set.
    pub fn register(registry: &Registry) -> Self {
        registry.set_help("zerodds_coap_frames_in_total", "CoAP datagrams received");
        registry.set_help("zerodds_coap_frames_out_total", "CoAP datagrams sent");
        registry.set_help("zerodds_coap_bytes_in_total", "CoAP bytes received");
        registry.set_help("zerodds_coap_bytes_out_total", "CoAP bytes sent");
        registry.set_help(
            "zerodds_coap_connections_active",
            "Active observe subscriptions",
        );
        registry.set_help(
            "zerodds_coap_connections_total",
            "Lifetime observe-setup attempts",
        );
        registry.set_help(
            "zerodds_coap_dds_samples_in_total",
            "Samples written into DDS",
        );
        registry.set_help(
            "zerodds_coap_dds_samples_out_total",
            "Samples emitted from DDS",
        );
        registry.set_help("zerodds_coap_errors_total", "Wire/codec errors");
        Self {
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
}

impl CatalogSnapshot {
    /// Constructor.
    #[must_use]
    pub fn from_config(cfg: &DaemonConfig) -> Self {
        Self {
            service: SERVICE_NAME.into(),
            version: SERVICE_VERSION.into(),
            topics: cfg.topics.clone(),
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
        out.push_str("\",\"topics\":[");
        for (i, t) in self.topics.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str("{\"dds_name\":\"");
            push_json_str(&mut out, &t.dds_name);
            out.push_str("\",\"dds_type\":\"");
            push_json_str(&mut out, &t.dds_type);
            out.push_str("\",\"coap_uri_path\":\"");
            push_json_str(&mut out, &t.coap_uri_path);
            out.push_str("\",\"direction\":\"");
            push_json_str(&mut out, &t.direction);
            out.push_str("\",\"qos\":{\"reliability\":\"");
            push_json_str(&mut out, &t.reliability);
            out.push_str("\",\"durability\":\"");
            push_json_str(&mut out, &t.durability);
            out.push_str("\",\"history_depth\":");
            out.push_str(&t.history_depth.to_string());
            out.push_str("}}");
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

/// Admin HTTP server (TCP, since the daemon path itself is UDP/CoAP).
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
        .name("zerodds-coap-admin".into())
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

/// Endpoint-Parser.
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

/// Periodischer flush-Thread.
pub fn spawn_otlp_flush_loop(
    exporter: Arc<OtlpExporter>,
    stop: Arc<AtomicBool>,
    interval: Duration,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("zerodds-coap-otlp".into())
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
    fn catalog_render_contains_topic_fields() {
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.topics.push(TopicConfig {
            dds_name: "Sensor".into(),
            dds_type: "Sensor".into(),
            coap_uri_path: "sensors/data".into(),
            direction: "out".into(),
            reliability: "reliable".into(),
            durability: "volatile".into(),
            history_depth: 10,
        });
        let snap = CatalogSnapshot::from_config(&cfg);
        let j = snap.render_json();
        assert!(j.contains("zerodds-coap-bridged"));
        assert!(j.contains("\"coap_uri_path\":\"sensors/data\""));
    }

    #[test]
    fn admin_endpoint_serves_metrics() {
        use std::io::{Read, Write};
        use std::net::TcpStream;
        let cfg = DaemonConfig::default_for_dev();
        let snap = Arc::new(CatalogSnapshot::from_config(&cfg));
        let reg = Arc::new(Registry::new());
        let m = BridgeMetrics::register(&reg);
        m.frames_in_total.add(11);
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
        assert!(body.contains("zerodds_coap_frames_in_total 11"));
        stop.store(true, Ordering::SeqCst);
    }
}

#[cfg(not(unix))]
pub fn install_signal_watcher(
    _shutdown_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    _reload_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    // Windows: signal_hook::iterator nur POSIX. Spawn dummy thread,
    // shutdown runs over the normal socket-close paths.
    Ok(std::thread::spawn(|| {}))
}
