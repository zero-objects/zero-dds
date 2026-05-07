// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-Cutting Daemon-Runtime: §8.2 Prometheus-Metrics, §8.3 OTLP-Spans,
//! §9.2 Graceful Shutdown, §5.2 Catalog-/Healthz-Endpoint, §6 QoS-Mapping.
//!
//! Wiederverwendbar fuer den `zerodds-ws-bridged`-Daemon — die Logik ist
//! library-side gehalten, damit Tests den `BridgeMetrics`-Set ohne
//! Subprocess-Boot durchspielen koennen.

#![allow(clippy::print_stderr)]

use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use zerodds_monitor::{Counter, Gauge, Labels, Registry};
use zerodds_observability_otlp::{OtlpConfig, OtlpExporter};

use super::config::{DaemonConfig, TopicConfig};

/// Service-Name fuer OTel-Resource und Catalog.
pub const SERVICE_NAME: &str = "zerodds-ws-bridged";

/// Versions-String — Cargo-Crate-Version (Single-Source).
pub const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

// ============================================================================
// A2 — Prometheus-Metrics-Set (§8.2).
// ============================================================================

/// Standard-Metric-Set fuer alle Bridges.
///
/// Die Metric-Namen folgen dem Bridge-Spec-Schema
/// `zerodds_<bridge>_<thing>_total`. Counter sind monotonically-increasing,
/// Gauges sind reset-bar.
#[derive(Clone)]
pub struct BridgeMetrics {
    /// Anzahl eingehender WS-Frames (Text+Binary).
    pub frames_in_total: Arc<Counter>,
    /// Anzahl ausgehender WS-Frames.
    pub frames_out_total: Arc<Counter>,
    /// Bytes in (Frame-Payload).
    pub bytes_in_total: Arc<Counter>,
    /// Bytes out.
    pub bytes_out_total: Arc<Counter>,
    /// Aktuell offene Connections.
    pub connections_active: Arc<Gauge>,
    /// Lifetime-Connections-akzeptiert.
    pub connections_total: Arc<Counter>,
    /// DDS-Samples published into runtime.
    pub dds_samples_in_total: Arc<Counter>,
    /// DDS-Samples received from runtime.
    pub dds_samples_out_total: Arc<Counter>,
    /// Wire-Errors (decode/encode/socket).
    pub errors_total: Arc<Counter>,
}

impl BridgeMetrics {
    /// Registriert das Standard-Metric-Set in der gegebenen Registry.
    /// Idempotent — mehrfacher Aufruf liefert dieselben Counter-/Gauge-
    /// Instanzen zurueck.
    pub fn register(registry: &Registry) -> Self {
        registry.set_help(
            "zerodds_ws_frames_in_total",
            "WebSocket frames received from peer",
        );
        registry.set_help(
            "zerodds_ws_frames_out_total",
            "WebSocket frames sent to peer",
        );
        registry.set_help("zerodds_ws_bytes_in_total", "WebSocket bytes received");
        registry.set_help("zerodds_ws_bytes_out_total", "WebSocket bytes sent");
        registry.set_help(
            "zerodds_ws_connections_active",
            "Currently open WebSocket connections",
        );
        registry.set_help(
            "zerodds_ws_connections_total",
            "Lifetime accepted WebSocket connections",
        );
        registry.set_help(
            "zerodds_ws_dds_samples_in_total",
            "DDS samples written into runtime via WS",
        );
        registry.set_help(
            "zerodds_ws_dds_samples_out_total",
            "DDS samples emitted to WS subscribers",
        );
        registry.set_help("zerodds_ws_errors_total", "Frame/codec/socket errors");

        Self {
            frames_in_total: registry.counter("zerodds_ws_frames_in_total", Labels::new()),
            frames_out_total: registry.counter("zerodds_ws_frames_out_total", Labels::new()),
            bytes_in_total: registry.counter("zerodds_ws_bytes_in_total", Labels::new()),
            bytes_out_total: registry.counter("zerodds_ws_bytes_out_total", Labels::new()),
            connections_active: registry.gauge("zerodds_ws_connections_active", Labels::new()),
            connections_total: registry.counter("zerodds_ws_connections_total", Labels::new()),
            dds_samples_in_total: registry
                .counter("zerodds_ws_dds_samples_in_total", Labels::new()),
            dds_samples_out_total: registry
                .counter("zerodds_ws_dds_samples_out_total", Labels::new()),
            errors_total: registry.counter("zerodds_ws_errors_total", Labels::new()),
        }
    }
}

// ============================================================================
// A1 — Graceful-Shutdown (§9.2).
// ============================================================================

/// Lifecycle-Signale, die der Daemon empfangen kann.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleSignal {
    /// SIGTERM/SIGINT — beginne mit Shutdown.
    Shutdown,
    /// SIGHUP — Config-Reload (TLS-Cert + ACL hot-reload).
    Reload,
}

/// Installiert einen Signal-Watcher der `SIGTERM`/`SIGINT`/`SIGHUP`
/// abfaengt und das `shutdown_flag` setzt bzw. den `reload_flag`-
/// Hook anstoesst.
///
/// Der Worker laeuft in einem dedizierten Thread und beendet sich
/// sobald das `shutdown_flag` gesetzt ist (Self-Notification durch
/// SIGTERM-Path).
///
/// Die Funktion gibt einen Join-Handle zurueck; im Daemon-`Drop`-Pfad
/// soll dieser nicht aktiv gejoint werden — der Worker-Thread ist
/// detached gegen das Process-Lifecycle.
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
// A5 — Catalog/Healthz-Endpoint (§5.2).
// ============================================================================

/// Catalog-Snapshot fuer `/catalog`-Endpoint.
#[derive(Clone)]
pub struct CatalogSnapshot {
    /// Service-Name.
    pub service: String,
    /// Version.
    pub version: String,
    /// Bridge-Topics (DDS-Name + Direction + WS-Pfad + QoS).
    pub topics: Vec<TopicConfig>,
}

impl CatalogSnapshot {
    /// Aus Daemon-Config bauen.
    #[must_use]
    pub fn from_config(cfg: &DaemonConfig) -> Self {
        Self {
            service: SERVICE_NAME.into(),
            version: SERVICE_VERSION.into(),
            topics: cfg.topics.clone(),
        }
    }

    /// Render als JSON (Spec §5.2).
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
            out.push_str("{\"name\":\"");
            push_json_str(&mut out, &t.name);
            out.push_str("\",\"type\":\"");
            push_json_str(&mut out, &t.type_name);
            out.push_str("\",\"direction\":\"");
            push_json_str(&mut out, &t.direction);
            out.push_str("\",\"ws_path\":\"");
            push_json_str(&mut out, &t.ws_path);
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

/// Mini-HTTP-Worker der `/catalog`, `/healthz` und `/metrics` bedient.
///
/// Returns `JoinHandle` + bound `SocketAddr`. Beim Drop des stop-flags
/// laeuft die accept-Loop noch eine Iteration durch (dank Self-Connect).
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
        .name("zerodds-ws-admin".into())
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
                    Err(_) => {
                        thread::sleep(Duration::from_millis(100));
                    }
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
    let first_line = req.lines().next().unwrap_or("");

    // Path extrahieren.
    let path = first_line
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
            format!("{}\nendpoints: /catalog /healthz /metrics\n", SERVICE_NAME),
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

// ============================================================================
// A3 — OTLP-Span-Exporter (§8.3).
// ============================================================================

/// Parst `OTEL_EXPORTER_OTLP_ENDPOINT`-aehnlichen String in eine
/// `OtlpConfig`. Akzeptiert `http://host:port`, `host:port`, oder
/// nur `host` (default Port 4318).
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

/// Holt `OTEL_EXPORTER_OTLP_ENDPOINT` aus der Umgebung und parst
/// `host:port`. Liefert `None` wenn die ENV nicht gesetzt ist.
#[must_use]
pub fn otlp_config_from_env(service_name: &str) -> Option<OtlpConfig> {
    let raw = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok()?;
    Some(otlp_config_from_endpoint(service_name, &raw))
}

/// Spawn-t einen periodischen `OtlpExporter::flush()`-Thread.
pub fn spawn_otlp_flush_loop(
    exporter: Arc<OtlpExporter>,
    stop: Arc<AtomicBool>,
    interval: Duration,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("zerodds-ws-otlp".into())
        .spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                thread::sleep(interval);
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                // Fehler werden geschluckt — der Collector kann offline
                // sein; das ist fuer den Daemon-Pfad nicht fatal.
                let _ = exporter.flush();
            }
            // Final flush.
            let _ = exporter.flush();
        })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn metrics_register_is_idempotent() {
        let r = Registry::new();
        let m1 = BridgeMetrics::register(&r);
        let m2 = BridgeMetrics::register(&r);
        m1.frames_in_total.inc();
        // Same registry-key => same Counter instance => share state.
        assert_eq!(m2.frames_in_total.get(), 1);
    }

    #[test]
    fn metrics_counter_visible_in_prometheus_render() {
        let r = Registry::new();
        let m = BridgeMetrics::register(&r);
        m.frames_in_total.add(7);
        m.connections_active.set(2);
        let text = r.render_prometheus();
        assert!(
            text.contains("zerodds_ws_frames_in_total 7"),
            "expected counter in render, got:\n{text}"
        );
        assert!(
            text.contains("zerodds_ws_connections_active 2"),
            "expected gauge in render, got:\n{text}"
        );
    }

    #[test]
    fn catalog_render_json_contains_topic_fields() {
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.topics.push(TopicConfig {
            name: "Chat::Message".into(),
            type_name: "Chat::Message".into(),
            direction: "bidir".into(),
            ws_path: "/topics/chat/message".into(),
            reliability: "reliable".into(),
            durability: "volatile".into(),
            history_depth: 10,
        });
        let snap = CatalogSnapshot::from_config(&cfg);
        let j = snap.render_json();
        assert!(j.contains("\"service\":\"zerodds-ws-bridged\""));
        assert!(j.contains("\"name\":\"Chat::Message\""));
        assert!(j.contains("\"ws_path\":\"/topics/chat/message\""));
        assert!(j.contains("\"reliability\":\"reliable\""));
    }

    #[test]
    fn admin_endpoint_serves_catalog_and_healthz_and_metrics() {
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
        });
        let snap = Arc::new(CatalogSnapshot::from_config(&cfg));
        let reg = Arc::new(Registry::new());
        let metrics = BridgeMetrics::register(&reg);
        metrics.frames_in_total.add(42);

        let healthy = Arc::new(AtomicBool::new(true));
        let stop = Arc::new(AtomicBool::new(false));

        let (_h, bound) = serve_admin_endpoints(
            "127.0.0.1:0".parse().unwrap(),
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
        s.write_all(b"GET /metrics HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap();
        let mut body = String::new();
        s.read_to_string(&mut body).ok();
        assert!(
            body.contains("zerodds_ws_frames_in_total 42"),
            "got: {body}"
        );

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
    fn otlp_config_from_endpoint_parses_http_url() {
        let c = otlp_config_from_endpoint("svc-1", "http://collector.local:4318");
        assert_eq!(c.host, "collector.local");
        assert_eq!(c.port, 4318);
        assert_eq!(c.service_name, "svc-1");
    }

    #[test]
    fn otlp_config_from_endpoint_parses_bare_host_port() {
        let c = otlp_config_from_endpoint("svc", "host:9999");
        assert_eq!(c.host, "host");
        assert_eq!(c.port, 9999);
    }

    #[test]
    fn otlp_config_from_endpoint_handles_https_and_trailing_slash() {
        let c = otlp_config_from_endpoint("svc", "https://otel.svc.local:4318/");
        assert_eq!(c.host, "otel.svc.local");
        assert_eq!(c.port, 4318);
    }

    #[test]
    fn otlp_config_from_endpoint_falls_back_to_default_port() {
        let c = otlp_config_from_endpoint("svc", "host-only");
        assert_eq!(c.host, "host-only");
        assert_eq!(c.port, 4318);
    }
}
