// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-Cutting Daemon-Runtime fuer den AMQP-Daemon.
//!
//! Bietet Standard-Counter (§8.2 Prometheus), `/catalog`-/`/healthz`-
//! /`/metrics`-Endpoint (§5.2), Signal-Watcher (§9.2), OTLP-Exporter
//! (§8.3) — alle als generische, im Binary-Mainloop wired-up
//! Komponenten.

#![allow(clippy::print_stderr)]

use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use zerodds_monitor::{Counter, Gauge, Labels, Registry};
use zerodds_observability_otlp::{OtlpConfig, OtlpExporter};

/// Service-Name (verwendet im Catalog + OTel-Resource).
pub const SERVICE_NAME: &str = "zerodds-amqp-bridged";
/// Crate-Version.
pub const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Standard-Metric-Set fuer den AMQP-Daemon.
#[derive(Clone)]
pub struct BridgeMetrics {
    /// Eingehende AMQP-Frames.
    pub frames_in_total: Arc<Counter>,
    /// Ausgehende AMQP-Frames.
    pub frames_out_total: Arc<Counter>,
    /// Bytes in.
    pub bytes_in_total: Arc<Counter>,
    /// Bytes out.
    pub bytes_out_total: Arc<Counter>,
    /// Aktive Broker-Connections.
    pub connections_active: Arc<Gauge>,
    /// Lifetime Broker-Connect-Versuche.
    pub connections_total: Arc<Counter>,
    /// AMQP → DDS Samples.
    pub dds_samples_in_total: Arc<Counter>,
    /// DDS → AMQP Samples.
    pub dds_samples_out_total: Arc<Counter>,
    /// Wire-Errors.
    pub errors_total: Arc<Counter>,
}

impl BridgeMetrics {
    /// Registriert das Standard-Set.
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
        }
    }
}

/// Catalog-Topic-Eintrag — Daemon-typ-unabhaengig.
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
    /// Konstruktor.
    #[must_use]
    pub fn new(topics: Vec<CatalogTopic>) -> Self {
        Self {
            service: SERVICE_NAME.into(),
            version: SERVICE_VERSION.into(),
            topics,
        }
    }

    /// JSON-Render fuer `/catalog`.
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

/// Endpoint-String parsen.
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
    // Windows: signal_hook::iterator nur POSIX. Spawn dummy thread,
    // shutdown laeuft ueber die normalen socket-close-Pfade.
    Ok(std::thread::spawn(|| {}))
}
