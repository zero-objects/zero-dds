// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-cutting daemon runtime for `zerodds-mqtt-bridged`:
//! §8.2 Prometheus, §8.3 OTLP spans, §9.2 graceful shutdown,
//! §5.2 catalog/healthz.

#![allow(clippy::print_stderr)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use zerodds_monitor::Registry;
use zerodds_monitor::admin::AdminEndpoints;
use zerodds_monitor::json::push_escaped as push_json_str;
use zerodds_observability_otlp::{OtlpConfig, OtlpExporter};

use super::config::{DaemonConfig, TopicConfig};

/// Service name for catalog/OTel.
pub const SERVICE_NAME: &str = "zerodds-mqtt-bridged";

/// Crate-Version.
pub const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

// ============================================================================
// A1 — Graceful Shutdown (§9.2).
// ============================================================================

/// Installs SIGTERM/SIGINT/SIGHUP-Watcher.
#[cfg(unix)]
pub fn install_signal_watcher(
    shutdown_flag: Arc<AtomicBool>,
    reload_flag: Arc<AtomicBool>,
) -> std::io::Result<JoinHandle<()>> {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    use signal_hook::iterator::Signals;
    let mut signals = Signals::new([SIGTERM, SIGINT, SIGHUP])?;
    thread::Builder::new()
        .name("zerodds-mqtt-signals".into())
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

// ============================================================================
// A5 — Catalog/Healthz/Metrics-Endpoint (§5.2).
// ============================================================================

/// The catalog of a broker connection (Spec §5.2): its topics.
#[derive(Clone)]
pub struct CatalogSnapshot {
    /// Bridge-Topics.
    pub topics: Vec<TopicConfig>,
}

impl CatalogSnapshot {
    /// The catalog of the top level of `cfg`.
    #[must_use]
    pub fn from_config(cfg: &DaemonConfig) -> Self {
        Self {
            topics: cfg.topics.clone(),
        }
    }

    /// The JSON document of Spec §5.2: `topics`, each with `dds_name`,
    /// `dds_type`, `mqtt_topic`, `qos` (`reliability` and `durability` as
    /// they apply) and `schema_url` when the topic names one. The retained
    /// PUBLISH on `<own_topic_prefix>/<client_id>/catalog` and the admin
    /// endpoint's `/catalog` carry it.
    #[must_use]
    pub fn render_json(&self) -> String {
        let mut out = String::with_capacity(32 + self.topics.len() * 192);
        out.push_str("{\"topics\":[");
        for (i, t) in self.topics.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str("{\"dds_name\":\"");
            push_json_str(&mut out, &t.dds_name);
            out.push_str("\",\"dds_type\":\"");
            push_json_str(&mut out, &t.dds_type);
            out.push_str("\",\"mqtt_topic\":\"");
            push_json_str(&mut out, &t.mqtt_topic);
            out.push_str("\",\"qos\":{\"reliability\":\"");
            push_json_str(&mut out, t.effective_reliability());
            out.push_str("\",\"durability\":\"");
            push_json_str(&mut out, t.effective_durability());
            out.push_str("\"}");
            if let Some(url) = &t.schema_url {
                out.push_str(",\"schema_url\":\"");
                push_json_str(&mut out, url);
                out.push('"');
            }
            out.push('}');
        }
        out.push_str("]}");
        out
    }
}

/// The topic of the retained catalog of the connection of `client_id`
/// under `own_prefix` (`mqtt.own_topic_prefix`, Spec §3, §5.2); `None` for
/// an identifier with `+` or `#`, which a topic name cannot carry (MQTT
/// 5.0 §4.7.1).
#[must_use]
pub fn catalog_topic(own_prefix: &str, client_id: &str) -> Option<String> {
    (!client_id.is_empty() && !client_id.contains(['+', '#', '\0']))
        .then(|| format!("{own_prefix}/{client_id}/catalog"))
}

/// Mini HTTP server serving `/catalog`, `/healthz` and the Prometheus
/// metrics on `metrics_path` (`metrics.path`, Spec §3, §8.2).
pub fn serve_admin_endpoints(
    addr: SocketAddr,
    catalog: Arc<CatalogSnapshot>,
    registry: Arc<Registry>,
    healthy: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    metrics_path: &str,
) -> std::io::Result<(JoinHandle<()>, SocketAddr)> {
    zerodds_monitor::admin::serve_admin_endpoints(
        addr,
        "zerodds-mqtt-admin",
        AdminEndpoints {
            service_name: SERVICE_NAME.into(),
            metrics_path: metrics_path.into(),
            catalog: move || catalog.render_json(),
            registry,
            healthy,
        },
        stop,
    )
}

// ============================================================================
// A3 — OTLP (§8.3).
// ============================================================================

/// Parses an endpoint string into an `OtlpConfig`.
#[must_use]
pub fn otlp_config_from_endpoint(service_name: &str, raw: &str) -> OtlpConfig {
    OtlpConfig::from_endpoint(service_name, SERVICE_VERSION, raw)
}

/// Reads from ENV.
#[must_use]
pub fn otlp_config_from_env(service_name: &str) -> Option<OtlpConfig> {
    OtlpConfig::from_env(service_name, SERVICE_VERSION)
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
        "zerodds-mqtt-otlp",
    )
}

/// Off Unix (Windows): the requests of the service control manager and of
/// the console (`zerodds_dcps::cli::run_as_service`) in place of the
/// signals: a stop request sets `shutdown_flag` as SIGTERM does, a
/// `paramchange` sets `reload_flag` as SIGHUP does (Spec §9.2, §11).
#[cfg(not(unix))]
pub fn install_signal_watcher(
    shutdown_flag: Arc<AtomicBool>,
    reload_flag: Arc<AtomicBool>,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("zerodds-mqtt-signals".into())
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

// ============================================================================
// Tests.
// ============================================================================

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Spec §5.2: the catalog is the JSON document of the spec, octet for
    /// octet: `schema_url` only where set, the reliability and durability
    /// as they apply (`reliable`, `volatile` when not set).
    #[test]
    fn mqttbridge_5_2_the_catalog_is_the_json_of_the_spec() {
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.topics.push(TopicConfig {
            dds_name: "Chat::Message".into(),
            dds_type: "Chat::Message".into(),
            mqtt_topic: "chat/message".into(),
            direction: "bidir".into(),
            reliability: "reliable".into(),
            durability: "volatile".into(),
            schema_url: Some("https://schema.example/chat-message.idl".into()),
            ..TopicConfig::default()
        });
        cfg.topics.push(TopicConfig {
            dds_name: "Sensor \"A\"".into(),
            dds_type: "S".into(),
            mqtt_topic: "s/a".into(),
            reliability: "best_effort".into(),
            durability: "transient_local".into(),
            ..TopicConfig::default()
        });
        cfg.topics.push(TopicConfig {
            dds_name: "D".into(),
            dds_type: "D".into(),
            mqtt_topic: "d".into(),
            ..TopicConfig::default()
        });
        assert_eq!(
            CatalogSnapshot::from_config(&cfg).render_json(),
            concat!(
                r#"{"topics":["#,
                r#"{"dds_name":"Chat::Message","dds_type":"Chat::Message","mqtt_topic":"chat/message","#,
                r#""qos":{"reliability":"reliable","durability":"volatile"},"#,
                r#""schema_url":"https://schema.example/chat-message.idl"},"#,
                r#"{"dds_name":"Sensor \"A\"","dds_type":"S","mqtt_topic":"s/a","#,
                r#""qos":{"reliability":"best_effort","durability":"transient_local"}},"#,
                r#"{"dds_name":"D","dds_type":"D","mqtt_topic":"d","#,
                r#""qos":{"reliability":"reliable","durability":"volatile"}}"#,
                r#"]}"#
            )
        );
        assert_eq!(
            catalog_topic("$zerodds", "bridge-1").as_deref(),
            Some("$zerodds/bridge-1/catalog")
        );
        assert_eq!(
            catalog_topic("plant/zerodds", "bridge-1").as_deref(),
            Some("plant/zerodds/bridge-1/catalog")
        );
        assert_eq!(catalog_topic("$zerodds", "a+b"), None);
        assert_eq!(catalog_topic("$zerodds", ""), None);
    }

    #[test]
    fn mqttbridge_5_2_admin_endpoint_serves_paths() {
        use std::io::{Read, Write};
        use std::net::TcpStream;
        let cfg = DaemonConfig::default_for_dev();
        let snap = Arc::new(CatalogSnapshot::from_config(&cfg));
        let reg = Arc::new(Registry::new());
        let _m = super::super::metrics::BridgeMetrics::register(&reg);
        let healthy = Arc::new(AtomicBool::new(true));
        let stop = Arc::new(AtomicBool::new(false));
        let (_h, bound) = serve_admin_endpoints(
            "127.0.0.1:0".parse().unwrap(),
            Arc::clone(&snap),
            Arc::clone(&reg),
            Arc::clone(&healthy),
            Arc::clone(&stop),
            "/metrics",
        )
        .expect("spawn");

        let mut s = TcpStream::connect_timeout(&bound, Duration::from_secs(2)).expect("conn");
        s.set_read_timeout(Some(Duration::from_secs(2))).ok();
        s.write_all(b"GET /catalog HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap();
        let mut body = String::new();
        s.read_to_string(&mut body).ok();
        assert!(body.contains("HTTP/1.1 200"));
        assert!(body.contains("Content-Type: application/json"), "{body}");
        assert!(body.ends_with("{\"topics\":[]}"), "{body}");

        stop.store(true, Ordering::SeqCst);
    }

    /// Spec §3 `metrics.path`: the Prometheus metrics are served on it, and
    /// not on `/metrics` when it names another path.
    #[test]
    fn mqttbridge_3_the_metrics_are_served_on_metrics_path() {
        use std::io::{Read, Write};
        use std::net::TcpStream;
        let reg = Arc::new(Registry::new());
        let m = super::super::metrics::BridgeMetrics::register(&reg);
        m.publish_out("T", 1, 4);
        let stop = Arc::new(AtomicBool::new(false));
        let (_h, bound) = serve_admin_endpoints(
            "127.0.0.1:0".parse().unwrap(),
            Arc::new(CatalogSnapshot::from_config(
                &DaemonConfig::default_for_dev(),
            )),
            Arc::clone(&reg),
            Arc::new(AtomicBool::new(true)),
            Arc::clone(&stop),
            "/prom/bridge",
        )
        .expect("spawn");
        let get = |path: &str| {
            let mut s = TcpStream::connect_timeout(&bound, Duration::from_secs(2)).expect("conn");
            s.set_read_timeout(Some(Duration::from_secs(2))).ok();
            s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes())
                .unwrap();
            let mut body = String::new();
            s.read_to_string(&mut body).ok();
            body
        };
        let prom = get("/prom/bridge");
        assert!(prom.starts_with("HTTP/1.1 200"), "{prom}");
        assert!(
            prom.contains("zerodds_mqtt_bridge_bytes_out_total{dds_topic=\"T\"} 4"),
            "{prom}"
        );
        assert!(get("/metrics").starts_with("HTTP/1.1 404"));
        stop.store(true, Ordering::SeqCst);
    }

    /// Spec §11 with §9.2: off Unix the watcher turns the requests of the
    /// Windows service control manager into the flags of the signals; run
    /// here in a thread of its own, as `install_signal_watcher` runs it on
    /// Windows. Without a request it waits, and it ends when the daemon
    /// that held the shutdown flag has gone; a `paramchange` sets the
    /// reload flag and the watcher goes on; a stop request sets the
    /// shutdown flag and ends it. One test: the requests are the process's.
    #[test]
    fn mqttbridge_11_the_service_requests_set_the_shutdown_and_reload_flags() {
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
    fn mqttbridge_8_3_otlp_config_parses_endpoint() {
        let c = otlp_config_from_endpoint("svc", "http://collector:4318");
        assert_eq!(c.host, "collector");
        assert_eq!(c.port, 4318);
    }
}
