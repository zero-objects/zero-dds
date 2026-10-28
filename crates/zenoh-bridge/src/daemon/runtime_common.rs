// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-cutting daemon runtime for `zerodds-zenoh-bridged`.

#![allow(clippy::print_stderr)]

use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use zerodds_monitor::{Counter, Labels, Registry};
use zerodds_observability_otlp::{OtlpConfig, OtlpExporter};

/// Service-Name.
pub const SERVICE_NAME: &str = "zerodds-zenoh-bridged";
/// Crate-Version.
pub const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Last-synced values [`BridgeMetrics::sync_from`] adds as a delta to its
/// (monotonic) Prometheus counters — the engine's own `BridgeStats` atomics
/// already hold absolute totals, but `Counter` only exposes `inc`/`add`, no
/// `set` (a Prometheus counter, unlike a gauge, is a leaf a `rate()` query
/// depends on to keep monotonically increasing — see the `Counter`/`Gauge`
/// distinction in `crates/monitor/src`).
#[derive(Default)]
struct LastSynced {
    dds_to_zenoh: std::sync::atomic::AtomicU64,
    zenoh_to_dds: std::sync::atomic::AtomicU64,
    topics_rejected: std::sync::atomic::AtomicU64,
    forward_errors: std::sync::atomic::AtomicU64,
    lifecycle_type_unresolved: std::sync::atomic::AtomicU64,
    unknown_key_writers: std::sync::atomic::AtomicU64,
}

/// Standard counter set. Mirrors `runtime::BridgeStats` (the engine's own,
/// `zerodds-monitor`-free counters) so the admin endpoint can render them as
/// Prometheus without the engine crate depending on `zerodds-monitor`.
#[derive(Clone)]
pub struct BridgeMetrics {
    /// Samples forwarded DDS → Zenoh.
    pub dds_to_zenoh_total: Arc<Counter>,
    /// Samples forwarded Zenoh → DDS.
    pub zenoh_to_dds_total: Arc<Counter>,
    /// Topics refused at registration.
    pub topics_rejected_total: Arc<Counter>,
    /// Samples that arrived but could not be forwarded — logged and counted,
    /// never silently dropped.
    pub forward_errors_total: Arc<Counter>,
    /// Topics currently bridged.
    pub topics_active: Arc<zerodds_monitor::Gauge>,
    /// A dispose/unregister forwarded on a route bridged keyless because its
    /// type did not resolve at discovery time (`TopicSpec::
    /// type_resolution_uncertain`) — still forwarded, counted here instead
    /// of blending into `dds_to_zenoh_total`. See `crate::runtime::forward_pump`.
    pub lifecycle_type_unresolved_total: Arc<Counter>,
    /// A Zenoh→DDS writer registered for a keyed topic with no resolved
    /// type (`crate::runtime::BridgeStats::unknown_key_writers`) — counted
    /// once per such topic at registration, not per sample.
    pub unknown_key_writers_total: Arc<Counter>,
    last: Arc<LastSynced>,
}

impl BridgeMetrics {
    /// Registers the set.
    pub fn register(registry: &Registry) -> Self {
        registry.set_help(
            "zerodds_zenoh_dds_to_zenoh_total",
            "Samples forwarded DDS to Zenoh",
        );
        registry.set_help(
            "zerodds_zenoh_zenoh_to_dds_total",
            "Samples forwarded Zenoh to DDS",
        );
        registry.set_help(
            "zerodds_zenoh_topics_rejected_total",
            "Topics refused at registration",
        );
        registry.set_help(
            "zerodds_zenoh_forward_errors_total",
            "Samples that could not be forwarded",
        );
        registry.set_help("zerodds_zenoh_topics_active", "Topics currently bridged");
        registry.set_help(
            "zerodds_zenoh_lifecycle_type_unresolved_total",
            "Dispose/unregister samples forwarded on a route bridged keyless \
             because its type did not resolve at discovery time",
        );
        registry.set_help(
            "zerodds_zenoh_unknown_key_writers_total",
            "Zenoh->DDS writers registered for a keyed topic with no resolved type",
        );
        Self {
            dds_to_zenoh_total: registry.counter("zerodds_zenoh_dds_to_zenoh_total", Labels::new()),
            zenoh_to_dds_total: registry.counter("zerodds_zenoh_zenoh_to_dds_total", Labels::new()),
            topics_rejected_total: registry
                .counter("zerodds_zenoh_topics_rejected_total", Labels::new()),
            forward_errors_total: registry
                .counter("zerodds_zenoh_forward_errors_total", Labels::new()),
            topics_active: registry.gauge("zerodds_zenoh_topics_active", Labels::new()),
            lifecycle_type_unresolved_total: registry.counter(
                "zerodds_zenoh_lifecycle_type_unresolved_total",
                Labels::new(),
            ),
            unknown_key_writers_total: registry
                .counter("zerodds_zenoh_unknown_key_writers_total", Labels::new()),
            last: Arc::new(LastSynced::default()),
        }
    }

    /// Mirrors the engine's `runtime::BridgeStats` atomics into this set as
    /// a delta since the last call. Called on every admin-endpoint scrape
    /// (see `server.rs`), so `/metrics` is never more than one scrape
    /// interval stale.
    pub fn sync_from(&self, stats: &crate::runtime::BridgeStats) {
        sync_one(
            &self.dds_to_zenoh_total,
            &self.last.dds_to_zenoh,
            stats.dds_to_zenoh.load(Ordering::Relaxed),
        );
        sync_one(
            &self.zenoh_to_dds_total,
            &self.last.zenoh_to_dds,
            stats.zenoh_to_dds.load(Ordering::Relaxed),
        );
        sync_one(
            &self.topics_rejected_total,
            &self.last.topics_rejected,
            stats.topics_rejected.load(Ordering::Relaxed),
        );
        sync_one(
            &self.forward_errors_total,
            &self.last.forward_errors,
            stats.forward_errors.load(Ordering::Relaxed),
        );
        sync_one(
            &self.lifecycle_type_unresolved_total,
            &self.last.lifecycle_type_unresolved,
            stats.lifecycle_type_unresolved.load(Ordering::Relaxed),
        );
        sync_one(
            &self.unknown_key_writers_total,
            &self.last.unknown_key_writers,
            stats.unknown_key_writers.load(Ordering::Relaxed),
        );
    }
}

/// Adds `new_total - last` to `counter` and stores `new_total` as the new
/// `last`, saturating (never going backwards even if `new_total` somehow
/// regressed — a wraparound this daemon's lifetime will not reach).
fn sync_one(counter: &Counter, last: &std::sync::atomic::AtomicU64, new_total: u64) {
    let previous = last.swap(new_total, Ordering::Relaxed);
    let delta = new_total.saturating_sub(previous);
    if delta > 0 {
        counter.add(delta);
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
        .name("zerodds-zenoh-signals".into())
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

#[cfg(not(unix))]
pub fn install_signal_watcher(
    _shutdown_flag: Arc<AtomicBool>,
    _reload_flag: Arc<AtomicBool>,
) -> std::io::Result<JoinHandle<()>> {
    // Windows: signal_hook::iterator is POSIX-only. Shutdown still runs over
    // the normal handle-drop path once the caller sets the stop flag itself.
    Ok(thread::spawn(|| {}))
}

/// One `/catalog` topic entry — this crate's own live record (built from
/// `crate::runtime::TopicMapEntry`, `CatalogSnapshot::set_topics`'s only
/// caller, `server.rs`'s periodic sync loop), not the static config file:
/// an auto-discovered topic has no `TopicConfig` at all, only a
/// `TopicMapEntry` `ZenohBridge::add_topic` recorded once it was actually
/// bridged. Measured before this existed: `/catalog` listed `"topics":[]`
/// after the daemon had already logged `auto-bridged Square`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CatalogTopic {
    /// DDS topic name.
    pub dds_name: String,
    /// DDS IDL type name.
    pub dds_type: String,
    /// `"out"` (DDS→Zenoh), `"in"` (Zenoh→DDS), `"bidir"` (both), or
    /// `"none"` (neither — should not occur for a route that made it into
    /// the topic map, kept only so this is total rather than panicking).
    pub direction: String,
    /// Whether the topic is keyed — see `crate::runtime::TopicSpec::keyed`.
    pub keyed: bool,
}

/// Catalog snapshot — the daemon's admin `/catalog` endpoint. Holds the
/// *live* topic list, refreshed by [`Self::set_topics`] from the engine's
/// own `ZenohBridge::map()` (`server.rs`'s periodic sync loop, the same one
/// that mirrors `BridgeStats` into Prometheus) — every topic that is
/// actually bridged ends up in that map exactly once, whether it came from
/// the static `topics:` config or the auto-discovery loop, so this needs no
/// separate config-only path.
#[derive(Clone)]
pub struct CatalogSnapshot {
    /// Service.
    pub service: String,
    /// Version.
    pub version: String,
    topics: Arc<std::sync::Mutex<Vec<CatalogTopic>>>,
}

impl CatalogSnapshot {
    /// Empty snapshot — [`Self::set_topics`] fills it once the bridge has
    /// registered at least one topic.
    #[must_use]
    pub fn new() -> Self {
        Self {
            service: SERVICE_NAME.into(),
            version: SERVICE_VERSION.into(),
            topics: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    /// Replaces the live topic list. Called on the same periodic cadence as
    /// `BridgeMetrics::sync_from` (`server.rs`), so `/catalog` is never more
    /// than one sync interval stale — the same trade-off `/metrics`'s own
    /// doc comment already accepts.
    pub fn set_topics(&self, topics: Vec<CatalogTopic>) {
        if let Ok(mut t) = self.topics.lock() {
            *t = topics;
        }
    }

    /// JSON.
    #[must_use]
    pub fn render_json(&self) -> String {
        let topics = self.topics.lock().map(|t| t.clone()).unwrap_or_default();
        let mut out = String::with_capacity(256 + topics.len() * 128);
        out.push_str("{\"service\":\"");
        push_json_str(&mut out, &self.service);
        out.push_str("\",\"version\":\"");
        push_json_str(&mut out, &self.version);
        out.push_str("\",\"topics\":[");
        for (i, t) in topics.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str("{\"dds_name\":\"");
            push_json_str(&mut out, &t.dds_name);
            out.push_str("\",\"dds_type\":\"");
            push_json_str(&mut out, &t.dds_type);
            out.push_str("\",\"direction\":\"");
            push_json_str(&mut out, &t.direction);
            out.push_str("\",\"keyed\":");
            out.push_str(if t.keyed { "true" } else { "false" });
            out.push('}');
        }
        out.push_str("]}");
        out
    }
}

impl Default for CatalogSnapshot {
    fn default() -> Self {
        Self::new()
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

/// Admin HTTP server (TCP, since the daemon path itself is Zenoh transport).
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
        .name("zerodds-zenoh-admin".into())
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

/// Endpoint parser.
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

/// From ENV.
#[must_use]
pub fn otlp_config_from_env(service_name: &str) -> Option<OtlpConfig> {
    let raw = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok()?;
    Some(otlp_config_from_endpoint(service_name, &raw))
}

/// Periodic flush thread.
pub fn spawn_otlp_flush_loop(
    exporter: Arc<OtlpExporter>,
    stop: Arc<AtomicBool>,
    interval: Duration,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("zerodds-zenoh-otlp".into())
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
        m1.dds_to_zenoh_total.inc();
        assert_eq!(m2.dds_to_zenoh_total.get(), 1);
    }

    #[test]
    fn catalog_render_contains_topic_fields() {
        let snap = CatalogSnapshot::new();
        snap.set_topics(vec![CatalogTopic {
            dds_name: "Sensor".into(),
            dds_type: "Sensor".into(),
            direction: "out".into(),
            keyed: true,
        }]);
        let j = snap.render_json();
        assert!(j.contains("zerodds-zenoh-bridged"));
        assert!(j.contains("\"dds_name\":\"Sensor\""));
        assert!(j.contains("\"keyed\":true"));
    }

    #[test]
    fn catalog_render_is_empty_before_any_topic_is_bridged() {
        let snap = CatalogSnapshot::new();
        let j = snap.render_json();
        assert!(j.contains("\"topics\":[]"));
    }

    #[test]
    fn admin_endpoint_serves_metrics() {
        use std::io::{Read, Write};
        use std::net::TcpStream;
        let snap = Arc::new(CatalogSnapshot::new());
        let reg = Arc::new(Registry::new());
        let m = BridgeMetrics::register(&reg);
        m.dds_to_zenoh_total.add(11);
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
        assert!(body.contains("zerodds_zenoh_dds_to_zenoh_total 11"));
        stop.store(true, Ordering::SeqCst);
    }
}
