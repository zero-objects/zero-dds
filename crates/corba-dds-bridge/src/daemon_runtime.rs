// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-cutting daemon runtime for the CORBA daemon.
//!
//! Provides standard counters (§8.2 Prometheus), the `/catalog`/`/healthz`/
//! `/metrics` endpoint (§5.2), a signal watcher (§9.2), and the OTLP exporter
//! (§8.3) — all as generic components wired up in the binary main loop.

#![allow(clippy::print_stderr)]

use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use zerodds_monitor::{Counter, Gauge, Labels, Registry};
use zerodds_observability_otlp::{OtlpConfig, OtlpExporter};

/// Service name (used in the catalog and the OTel resource).
pub const SERVICE_NAME: &str = "zerodds-corba-bridged";
/// Crate version.
pub const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Standard metric set for the CORBA daemon.
#[derive(Clone)]
pub struct BridgeMetrics {
    /// Inbound CORBA frames.
    pub frames_in_total: Arc<Counter>,
    /// Outbound CORBA frames.
    pub frames_out_total: Arc<Counter>,
    /// Bytes in.
    pub bytes_in_total: Arc<Counter>,
    /// Bytes out.
    pub bytes_out_total: Arc<Counter>,
    /// Active broker connections.
    pub connections_active: Arc<Gauge>,
    /// Lifetime broker connect attempts.
    pub connections_total: Arc<Counter>,
    /// CORBA → DDS samples.
    pub dds_samples_in_total: Arc<Counter>,
    /// DDS → CORBA samples.
    pub dds_samples_out_total: Arc<Counter>,
    /// Wire errors.
    pub errors_total: Arc<Counter>,
}

impl BridgeMetrics {
    /// Registers the standard set.
    pub fn register(registry: &Registry) -> Self {
        registry.set_help("zerodds_corba_frames_in_total", "IIOP frames received");
        registry.set_help("zerodds_corba_frames_out_total", "IIOP frames sent");
        registry.set_help("zerodds_corba_bytes_in_total", "IIOP bytes received");
        registry.set_help("zerodds_corba_bytes_out_total", "IIOP bytes sent");
        registry.set_help(
            "zerodds_corba_connections_active",
            "Active IIOP connections",
        );
        registry.set_help("zerodds_corba_connections_total", "Lifetime IIOP connects");
        registry.set_help(
            "zerodds_corba_dds_samples_in_total",
            "Samples written into DDS",
        );
        registry.set_help(
            "zerodds_corba_dds_samples_out_total",
            "Samples emitted to IIOP",
        );
        registry.set_help("zerodds_corba_errors_total", "Wire/codec errors");
        Self {
            frames_in_total: registry.counter("zerodds_corba_frames_in_total", Labels::new()),
            frames_out_total: registry.counter("zerodds_corba_frames_out_total", Labels::new()),
            bytes_in_total: registry.counter("zerodds_corba_bytes_in_total", Labels::new()),
            bytes_out_total: registry.counter("zerodds_corba_bytes_out_total", Labels::new()),
            connections_active: registry.gauge("zerodds_corba_connections_active", Labels::new()),
            connections_total: registry.counter("zerodds_corba_connections_total", Labels::new()),
            dds_samples_in_total: registry
                .counter("zerodds_corba_dds_samples_in_total", Labels::new()),
            dds_samples_out_total: registry
                .counter("zerodds_corba_dds_samples_out_total", Labels::new()),
            errors_total: registry.counter("zerodds_corba_errors_total", Labels::new()),
        }
    }
}

/// Catalog topic entry — independent of the daemon type.
#[derive(Clone, Debug)]
pub struct CatalogTopic {
    /// DDS topic name.
    pub dds_name: String,
    /// CORBA address (queue:// / topic://).
    pub amqp_address: String,
    /// in/out/bidir.
    pub direction: String,
}

/// Catalog snapshot.
#[derive(Clone, Debug)]
pub struct CatalogSnapshot {
    /// Service name.
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

    /// JSON rendering for `/catalog`.
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

/// SIGTERM/SIGINT/SIGHUP watcher.
#[cfg(unix)]
pub fn install_signal_watcher(
    shutdown_flag: Arc<AtomicBool>,
    reload_flag: Arc<AtomicBool>,
) -> std::io::Result<JoinHandle<()>> {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    use signal_hook::iterator::Signals;
    let mut signals = Signals::new([SIGTERM, SIGINT, SIGHUP])?;
    thread::Builder::new()
        .name("zerodds-corba-signals".into())
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

/// Admin HTTP server `/catalog`/`/healthz`/`/metrics`.
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
        .name("zerodds-corba-admin".into())
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

/// Parse an endpoint string.
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

/// From the environment.
#[must_use]
pub fn otlp_config_from_env(service_name: &str) -> Option<OtlpConfig> {
    let raw = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok()?;
    Some(otlp_config_from_endpoint(service_name, &raw))
}

/// Periodic flush.
pub fn spawn_otlp_flush_loop(
    exporter: Arc<OtlpExporter>,
    stop: Arc<AtomicBool>,
    interval: Duration,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("zerodds-corba-otlp".into())
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

/// Spec §7.1 — CORBA bridge SSLIOP TLS server configuration.
///
/// Builds a `rustls::ServerConfig` without ALPN (CORBA IIOP uses no
/// ALPN token; SSLIOP advertises itself solely via `TAG_SSL_SEC_TRANS`
/// (component ID 20) in the IOR). The caller uses it to wrap the GIOP
/// TcpStream.
///
/// # Errors
/// A string diagnostic on IO/PEM/rustls errors.
#[cfg(feature = "std")]
pub fn build_corba_tls_config(
    cert_path: std::path::PathBuf,
    key_path: std::path::PathBuf,
    client_ca_path: Option<std::path::PathBuf>,
) -> Result<Arc<rustls::ServerConfig>, String> {
    // We cannot use the foundation loader because ALPN must not be set;
    // the standard function already returns an ALPN-empty config — we
    // pass the foundation path straight through.
    match &client_ca_path {
        Some(ca) => zerodds_bridge_security::tls::load_server_config_with_client_auth(
            &cert_path, &key_path, ca,
        )
        .map_err(|e| format!("tls: {e}")),
        None => zerodds_bridge_security::tls::load_server_config(&cert_path, &key_path)
            .map_err(|e| format!("tls: {e}")),
    }
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
        assert!(j.contains("zerodds-corba-bridged"));
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
        assert!(body.contains("zerodds_corba_frames_in_total 13"));
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
    // Windows: signal_hook::iterator is POSIX-only. Spawn a dummy
    // thread; shutdown runs through the normal socket-close paths.
    Ok(std::thread::spawn(|| {}))
}
