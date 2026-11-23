// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-cutting daemon runtime for the gRPC daemon.
//!
//! Provides the standard counters (§8.2 Prometheus), the `/catalog` /
//! `/healthz` / `/metrics` endpoints (§5.2), the signal watcher (§9.2),
//! and the OTLP exporter (§8.3) — all as generic components wired up in
//! the binary's main loop.

#![allow(clippy::print_stderr)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use zerodds_monitor::admin::AdminEndpoints;
use zerodds_monitor::json::push_escaped as push_json_str;
use zerodds_monitor::{Counter, Gauge, Labels, Registry};
use zerodds_observability_otlp::OtlpConfig;

/// Service name (used in the catalog + OTel resource).
pub const SERVICE_NAME: &str = "zerodds-grpc-bridged";
/// Crate version.
pub const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Standard metric set for the gRPC daemon.
#[derive(Clone)]
pub struct BridgeMetrics {
    /// Incoming gRPC frames.
    pub frames_in_total: Arc<Counter>,
    /// Outgoing gRPC frames.
    pub frames_out_total: Arc<Counter>,
    /// Bytes in.
    pub bytes_in_total: Arc<Counter>,
    /// Bytes out.
    pub bytes_out_total: Arc<Counter>,
    /// Active broker connections.
    pub connections_active: Arc<Gauge>,
    /// Lifetime broker connect attempts.
    pub connections_total: Arc<Counter>,
    /// gRPC → DDS samples.
    pub dds_samples_in_total: Arc<Counter>,
    /// DDS → gRPC samples.
    pub dds_samples_out_total: Arc<Counter>,
    /// Wire errors.
    pub errors_total: Arc<Counter>,
}

impl BridgeMetrics {
    /// Registers the standard set.
    pub fn register(registry: &Registry) -> Self {
        registry.set_help("zerodds_grpc_frames_in_total", "gRPC frames received");
        registry.set_help("zerodds_grpc_frames_out_total", "gRPC frames sent");
        registry.set_help("zerodds_grpc_bytes_in_total", "gRPC bytes received");
        registry.set_help("zerodds_grpc_bytes_out_total", "gRPC bytes sent");
        registry.set_help(
            "zerodds_grpc_connections_active",
            "Active HTTP/2 connections",
        );
        registry.set_help("zerodds_grpc_connections_total", "Lifetime HTTP/2 accepts");
        registry.set_help(
            "zerodds_grpc_dds_samples_in_total",
            "Samples written into DDS",
        );
        registry.set_help(
            "zerodds_grpc_dds_samples_out_total",
            "Samples emitted to gRPC stream",
        );
        registry.set_help("zerodds_grpc_errors_total", "Wire/codec errors");
        Self {
            frames_in_total: registry.counter("zerodds_grpc_frames_in_total", Labels::new()),
            frames_out_total: registry.counter("zerodds_grpc_frames_out_total", Labels::new()),
            bytes_in_total: registry.counter("zerodds_grpc_bytes_in_total", Labels::new()),
            bytes_out_total: registry.counter("zerodds_grpc_bytes_out_total", Labels::new()),
            connections_active: registry.gauge("zerodds_grpc_connections_active", Labels::new()),
            connections_total: registry.counter("zerodds_grpc_connections_total", Labels::new()),
            dds_samples_in_total: registry
                .counter("zerodds_grpc_dds_samples_in_total", Labels::new()),
            dds_samples_out_total: registry
                .counter("zerodds_grpc_dds_samples_out_total", Labels::new()),
            errors_total: registry.counter("zerodds_grpc_errors_total", Labels::new()),
        }
    }
}

/// Catalog topic entry — independent of the daemon type.
#[derive(Clone, Debug)]
pub struct CatalogTopic {
    /// DDS topic name.
    pub dds_name: String,
    /// The fully-qualified gRPC service (`zerodds.chat.v1.ChatMessageStream`).
    pub grpc_service: String,
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
            out.push_str("\",\"grpc_service\":\"");
            push_json_str(&mut out, &t.grpc_service);
            out.push_str("\",\"direction\":\"");
            push_json_str(&mut out, &t.direction);
            out.push_str("\"}");
        }
        out.push_str("]}");
        out
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
        .name("zerodds-grpc-signals".into())
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

/// Off Unix (Windows): the requests of the service control manager and of
/// the console (`zerodds_dcps::cli::run_as_service`, Spec §11) in place of
/// the signals: a stop request sets `shutdown_flag` as SIGTERM does and
/// starts the drain of §9.2, a `paramchange` sets `reload_flag` as SIGHUP
/// does.
#[cfg(not(unix))]
pub fn install_signal_watcher(
    shutdown_flag: Arc<AtomicBool>,
    reload_flag: Arc<AtomicBool>,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("zerodds-grpc-signals".into())
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

/// Admin HTTP server `/catalog`/`/healthz`/`/metrics`.
pub fn serve_admin_endpoints(
    addr: SocketAddr,
    catalog: Arc<CatalogSnapshot>,
    registry: Arc<Registry>,
    healthy: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
) -> std::io::Result<(JoinHandle<()>, SocketAddr)> {
    serve_admin_endpoints_at(addr, "/metrics".into(), catalog, registry, healthy, stop)
}

/// [`serve_admin_endpoints`] with the Prometheus page at `metrics_path`
/// (spec §3 `metrics.path`).
pub fn serve_admin_endpoints_at(
    addr: SocketAddr,
    metrics_path: String,
    catalog: Arc<CatalogSnapshot>,
    registry: Arc<Registry>,
    healthy: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
) -> std::io::Result<(JoinHandle<()>, SocketAddr)> {
    zerodds_monitor::admin::serve_admin_endpoints(
        addr,
        "zerodds-grpc-admin",
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

/// The environment variable of the OTLP endpoint (spec §8.3).
pub const OTEL_EXPORTER_OTLP_ENDPOINT: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";

/// Spec §8.3: the OTLP endpoint `http://host[:port]`, with an optional
/// trailing `/`; without a port 80, the default port of `http` (RFC 9110
/// §4.2.1). `host` is a name, an IPv4 address or an IPv6 address in
/// brackets (RFC 3986 §3.2.2). The exporter speaks HTTP/1.1 without TLS and
/// posts to `/v1/traces` at the root of the endpoint, so what it cannot
/// serve is refused: another scheme (`https` among them), user information,
/// a path, a query or a fragment, a port that is not a number from 1 to
/// 65535.
///
/// # Errors
/// The value and what of it the exporter cannot serve.
pub fn otlp_config_from_endpoint(service_name: &str, raw: &str) -> Result<OtlpConfig, String> {
    let refused = |why: &str| format!("{OTEL_EXPORTER_OTLP_ENDPOINT} {raw:?}: {why}");
    let (scheme, rest) = raw
        .split_once("://")
        .ok_or_else(|| refused("not a URL http://host[:port]"))?;
    if scheme.eq_ignore_ascii_case("https") {
        return Err(refused(
            "https: the OTLP exporter speaks no TLS; only http://host[:port]",
        ));
    }
    if !scheme.eq_ignore_ascii_case("http") {
        return Err(refused(&format!(
            "scheme {scheme:?}: only http://host[:port]"
        )));
    }
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    if !tail.is_empty() && tail != "/" {
        return Err(refused(&format!(
            "{tail:?}: the exporter posts to /v1/traces at the root; no path, query or fragment"
        )));
    }
    if authority.contains('@') {
        return Err(refused("user information is not supported"));
    }
    let (host, port) = if let Some(v6) = authority.strip_prefix('[') {
        let (addr, after) = v6
            .split_once(']')
            .ok_or_else(|| refused("an IPv6 address without its closing ]"))?;
        if addr.parse::<std::net::Ipv6Addr>().is_err() {
            return Err(refused(&format!("[{addr}] is not an IPv6 address")));
        }
        let port = match after {
            "" => None,
            p => Some(
                p.strip_prefix(':')
                    .ok_or_else(|| refused(&format!("{after:?} after the IPv6 address")))?,
            ),
        };
        (format!("[{addr}]"), port)
    } else {
        if authority.matches(':').count() > 1 {
            return Err(refused("an IPv6 address must be in brackets"));
        }
        match authority.split_once(':') {
            Some((h, p)) => (h.to_string(), Some(p)),
            None => (authority.to_string(), None),
        }
    };
    if host.is_empty() {
        return Err(refused("no host"));
    }
    let port = match port {
        None => 80,
        Some(p) => match p.parse::<u16>() {
            Ok(n) if n > 0 && p.bytes().all(|b| b.is_ascii_digit()) => n,
            _ => {
                return Err(refused(&format!(
                    "port {p:?}: expected a number from 1 to 65535"
                )));
            }
        },
    };
    Ok(OtlpConfig {
        host,
        port,
        service_name: service_name.into(),
        service_version: SERVICE_VERSION.into(),
        timeout: Duration::from_secs(5),
    })
}

/// Spec §8.3: the OTLP endpoint of `OTEL_EXPORTER_OTLP_ENDPOINT`; `None`
/// when the variable is unset or empty (an empty value is unset, as the
/// OpenTelemetry environment variable specification has it).
///
/// # Errors
/// A value [`otlp_config_from_endpoint`] refuses, or one that is not
/// UTF-8.
pub fn otlp_config_from_env(service_name: &str) -> Result<Option<OtlpConfig>, String> {
    match std::env::var(OTEL_EXPORTER_OTLP_ENDPOINT) {
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(v)) => {
            Err(format!("{OTEL_EXPORTER_OTLP_ENDPOINT} {v:?}: not UTF-8"))
        }
        Ok(raw) if raw.is_empty() => Ok(None),
        Ok(raw) => otlp_config_from_endpoint(service_name, &raw).map(Some),
    }
}

/// Spec §7.1 — gRPC bridge TLS server configuration with ALPN `h2`.
///
/// Reads the PEM cert+key directly with `rustls-pemfile` and builds a
/// `rustls::ServerConfig` with `alpn_protocols = [b"h2"]` (HTTP/2 as the
/// ALPN token, the standard for gRPC). The caller can pass the resulting
/// `Arc<ServerConfig>` to [`zerodds_bridge_security::serve_tls_handshake`]
/// or rotate it via [`zerodds_bridge_security::RotatingTlsConfig`] — the
/// latter, however, does not set ALPN; for gRPC this path must be used.
///
/// # Errors
/// String diagnostic on IO/PEM/rustls errors.
#[cfg(feature = "std")]
pub fn build_grpc_tls_config(
    cert_path: std::path::PathBuf,
    key_path: std::path::PathBuf,
    client_ca_path: Option<std::path::PathBuf>,
) -> Result<Arc<rustls::ServerConfig>, String> {
    build_grpc_tls_config_with(cert_path, key_path, client_ca_path, true)
}

/// [`build_grpc_tls_config`] with the choice of Spec §3
/// `grpc.tls.require_client_cert`: with a client CA and `false`, a client
/// without a certificate completes the handshake and a presented one is
/// still verified against the CA.
///
/// # Errors
/// String diagnostic on IO/PEM/rustls errors.
#[cfg(feature = "std")]
pub fn build_grpc_tls_config_with(
    cert_path: std::path::PathBuf,
    key_path: std::path::PathBuf,
    client_ca_path: Option<std::path::PathBuf>,
    require_client_cert: bool,
) -> Result<Arc<rustls::ServerConfig>, String> {
    build_grpc_tls_config_opts(
        cert_path,
        key_path,
        client_ca_path,
        require_client_cert,
        &GrpcTlsOptions::default(),
    )
}

/// The lowest TLS version the listener accepts (spec §3
/// `grpc.tls.min_version`, §7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TlsMinVersion {
    /// TLS 1.2 and TLS 1.3 (the default); TLS 1.3 when the client offers
    /// it.
    #[default]
    Tls12,
    /// TLS 1.3 only.
    Tls13,
}

impl TlsMinVersion {
    /// The value of `grpc.tls.min_version`: `1.2` or `1.3`.
    #[must_use]
    pub fn of(value: &str) -> Option<Self> {
        match value {
            "1.2" => Some(Self::Tls12),
            "1.3" => Some(Self::Tls13),
            _ => None,
        }
    }
}

/// The TLS settings of spec §3 `grpc.tls` beyond the material.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GrpcTlsOptions {
    /// `grpc.tls.min_version`.
    pub min_version: TlsMinVersion,
    /// `grpc.tls.cipher_suites`: IANA names, in the order of preference;
    /// empty is every suite of the crypto provider.
    pub cipher_suites: Vec<String>,
}

/// The IANA name of a cipher suite: `TLS_AES_128_GCM_SHA256` for the TLS
/// 1.3 suite rustls calls `TLS13_AES_128_GCM_SHA256` (RFC 8446 §B.4), the
/// RFC name for a TLS 1.2 suite.
#[cfg(feature = "std")]
fn iana_name(suite: rustls::SupportedCipherSuite) -> String {
    let name = suite
        .suite()
        .as_str()
        .map_or_else(|| format!("{:?}", suite.suite()), str::to_string);
    match name.strip_prefix("TLS13_") {
        Some(rest) => format!("TLS_{rest}"),
        None => name,
    }
}

/// The cipher suites the crypto provider of this build offers (ring, or
/// aws-lc-rs in a FIPS build), by their IANA names.
#[cfg(feature = "std")]
#[must_use]
pub fn provider_cipher_suites() -> Vec<String> {
    zerodds_bridge_security::tls_provider()
        .cipher_suites
        .iter()
        .map(|s| iana_name(*s))
        .collect()
}

/// The cipher suites and protocol versions `opts` selects from the
/// provider's: every named suite in the order named, and the versions from
/// `min_version` up that keep a suite.
///
/// # Errors
/// A name the provider does not offer (in a FIPS build the suites outside
/// FIPS 140-3 are not offered), a name given twice, or a selection that
/// leaves no suite of a version the listener accepts.
#[cfg(feature = "std")]
pub fn select_tls_suites(
    opts: &GrpcTlsOptions,
) -> Result<
    (
        Vec<rustls::SupportedCipherSuite>,
        Vec<&'static rustls::SupportedProtocolVersion>,
    ),
    String,
> {
    let offered = zerodds_bridge_security::tls_provider().cipher_suites;
    let suites = if opts.cipher_suites.is_empty() {
        offered.clone()
    } else {
        let mut picked = Vec::with_capacity(opts.cipher_suites.len());
        for name in &opts.cipher_suites {
            let wanted = name.trim();
            let Some(s) = offered.iter().find(|s| {
                let iana = iana_name(**s);
                iana == wanted || format!("{:?}", s.suite()) == wanted
            }) else {
                return Err(format!(
                    "cipher suite {wanted:?} is not offered by this build; it offers {}",
                    offered
                        .iter()
                        .map(|s| iana_name(*s))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            };
            if picked.contains(s) {
                return Err(format!("cipher suite {wanted:?} is named twice"));
            }
            picked.push(*s);
        }
        picked
    };
    let mut versions: Vec<&'static rustls::SupportedProtocolVersion> = Vec::new();
    let has = |v: rustls::ProtocolVersion| suites.iter().any(|s| s.version().version == v);
    if opts.min_version == TlsMinVersion::Tls12 && has(rustls::ProtocolVersion::TLSv1_2) {
        versions.push(&rustls::version::TLS12);
    }
    if has(rustls::ProtocolVersion::TLSv1_3) {
        versions.push(&rustls::version::TLS13);
    }
    if versions.is_empty() {
        return Err(format!(
            "grpc.tls.cipher_suites keeps no suite of TLS {} or above",
            if opts.min_version == TlsMinVersion::Tls13 {
                "1.3"
            } else {
                "1.2"
            }
        ));
    }
    Ok((suites, versions))
}

/// [`build_grpc_tls_config_with`] with the protocol versions and cipher
/// suites of `opts` (spec §3 `grpc.tls.min_version`,
/// `grpc.tls.cipher_suites`, §7.1).
///
/// # Errors
/// String diagnostic on IO/PEM/rustls errors and the errors of
/// [`select_tls_suites`].
#[cfg(feature = "std")]
pub fn build_grpc_tls_config_opts(
    cert_path: std::path::PathBuf,
    key_path: std::path::PathBuf,
    client_ca_path: Option<std::path::PathBuf>,
    require_client_cert: bool,
    opts: &GrpcTlsOptions,
) -> Result<Arc<rustls::ServerConfig>, String> {
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};
    use rustls_pemfile::Item;
    use std::fs::File;
    use std::io::BufReader;

    fn read_certs(p: &std::path::Path) -> Result<Vec<CertificateDer<'static>>, String> {
        let f = File::open(p).map_err(|e| format!("open {}: {e}", p.display()))?;
        let mut br = BufReader::new(f);
        let mut out = Vec::new();
        for item in rustls_pemfile::read_all(&mut br) {
            let item = item.map_err(|e| format!("pem: {e}"))?;
            if let Item::X509Certificate(d) = item {
                out.push(d);
            }
        }
        if out.is_empty() {
            return Err("no X509 certs in PEM".into());
        }
        Ok(out)
    }
    fn read_key(p: &std::path::Path) -> Result<PrivateKeyDer<'static>, String> {
        let f = File::open(p).map_err(|e| format!("open {}: {e}", p.display()))?;
        let mut br = BufReader::new(f);
        for item in rustls_pemfile::read_all(&mut br) {
            let item = item.map_err(|e| format!("pem: {e}"))?;
            match item {
                Item::Pkcs8Key(k) => return Ok(PrivateKeyDer::Pkcs8(k)),
                Item::Pkcs1Key(k) => return Ok(PrivateKeyDer::Pkcs1(k)),
                Item::Sec1Key(k) => return Ok(PrivateKeyDer::Sec1(k)),
                _ => {}
            }
        }
        Err("no supported private key in PEM".into())
    }

    let certs = read_certs(&cert_path)?;
    let key = read_key(&key_path)?;
    // The backend bridge-security was built with: ring, or aws-lc-rs (FIPS),
    // for the server config and the client-certificate verifier alike — the
    // implicit process provider is ambiguous when both are linked.
    let (suites, versions) = select_tls_suites(opts)?;
    let mut provider = zerodds_bridge_security::tls_provider();
    provider.cipher_suites = suites;
    let provider = Arc::new(provider);
    let builder = rustls::ServerConfig::builder_with_provider(Arc::clone(&provider))
        .with_protocol_versions(&versions)
        .map_err(|e| format!("rustls: {e}"))?;
    let mut sc = match &client_ca_path {
        Some(ca) => {
            let ca_certs = read_certs(ca)?;
            let mut roots = rustls::RootCertStore::empty();
            for c in ca_certs {
                roots.add(c).map_err(|e| format!("ca add: {e}"))?;
            }
            let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
                Arc::new(roots),
                Arc::clone(&provider),
            );
            let verifier = if require_client_cert {
                verifier
            } else {
                verifier.allow_unauthenticated()
            };
            let verifier = verifier
                .build()
                .map_err(|e| format!("client verifier: {e}"))?;
            builder
                .with_client_cert_verifier(verifier)
                .with_single_cert(certs, key)
                .map_err(|e| format!("rustls cert: {e}"))?
        }
        None => builder
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|e| format!("rustls cert: {e}"))?,
    };
    sc.alpn_protocols = vec![b"h2".to_vec()];
    // A configured list is the server's order of preference.
    sc.ignore_client_order = !opts.cipher_suites.is_empty();
    Ok(Arc::new(sc))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
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
    fn grpcbridge_5_2_catalog_render_json_is_valid_shape() {
        let snap = CatalogSnapshot::new(vec![CatalogTopic {
            dds_name: "Sensor::Reading".into(),
            grpc_service: "zerodds.sensor.v1.SensorReadingStream".into(),
            direction: "out".into(),
        }]);
        let j = snap.render_json();
        assert!(j.contains("zerodds-grpc-bridged"));
        assert!(j.contains("\"dds_name\":\"Sensor::Reading\""));
        assert!(j.contains("\"grpc_service\":\"zerodds.sensor.v1.SensorReadingStream\""));
        assert!(!j.contains("amqp"));
    }

    #[test]
    fn grpcbridge_8_2_admin_endpoint_serves_metrics_path() {
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
        assert!(body.contains("zerodds_grpc_frames_in_total 13"));
        stop.store(true, Ordering::SeqCst);
    }

    /// Spec §3 `metrics.path`: the Prometheus page is served there, and
    /// `/metrics` is not.
    #[test]
    fn grpcbridge_3_metrics_path_is_served() {
        use std::io::{Read, Write};
        use std::net::TcpStream;
        let reg = Arc::new(Registry::new());
        BridgeMetrics::register(&reg).frames_in_total.add(5);
        let stop = Arc::new(AtomicBool::new(false));
        let (_h, bound) = serve_admin_endpoints_at(
            "127.0.0.1:0".parse().unwrap(),
            "/prom".into(),
            Arc::new(CatalogSnapshot::new(Vec::new())),
            Arc::clone(&reg),
            Arc::new(AtomicBool::new(true)),
            Arc::clone(&stop),
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
        let prom = get("/prom");
        assert!(prom.starts_with("HTTP/1.1 200"), "{prom}");
        assert!(prom.contains("zerodds_grpc_frames_in_total 5"), "{prom}");
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
    fn grpcbridge_11_the_service_requests_set_the_shutdown_and_reload_flags() {
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

    /// Spec §8.3: `http://host[:port]` with an optional trailing `/`; the
    /// port 80 when it is missing (RFC 9110 §4.2.1); an IPv6 address in
    /// brackets.
    #[test]
    fn grpcbridge_8_3_otlp_config_parses_endpoint() {
        for (raw, host, port) in [
            ("http://collector:4318", "collector", 4318),
            ("http://collector:4318/", "collector", 4318),
            ("HTTP://10.0.0.7:55681", "10.0.0.7", 55681),
            ("http://collector", "collector", 80),
            ("http://collector/", "collector", 80),
            ("http://[::1]:4318", "[::1]", 4318),
            ("http://[fe80::1]", "[fe80::1]", 80),
        ] {
            let c = otlp_config_from_endpoint("svc", raw).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!((c.host.as_str(), c.port), (host, port), "{raw}");
            assert_eq!(c.service_name, "svc");
        }
    }

    /// Spec §8.3: what the exporter cannot serve is refused, the value
    /// named: `https` (it speaks no TLS), another scheme, a path (it posts
    /// to `/v1/traces` at the root), a query, a fragment, user
    /// information, a port that is no number from 1 to 65535, no host, an
    /// IPv6 address without brackets.
    #[test]
    fn grpcbridge_8_3_an_otlp_endpoint_the_exporter_cannot_serve_is_refused() {
        for (raw, why) in [
            ("https://otel.example:4318", "no TLS"),
            ("grpc://otel.example:4317", "scheme"),
            ("otel.example:4318", "not a URL"),
            ("http://gw:80/otlp", "/otlp"),
            ("http://gw:4318/v1/traces", "/v1/traces\""),
            ("http://gw:4318//", "path"),
            ("http://gw:4318?x=1", "query"),
            ("http://gw:4318#f", "fragment"),
            ("http://user:pw@gw:4318", "user information"),
            ("http://gw:43x8", "port \"43x8\""),
            ("http://gw:0", "port \"0\""),
            ("http://gw:65536", "port \"65536\""),
            ("http://gw:+4318", "port \"+4318\""),
            ("http://gw:", "port \"\""),
            ("http://:4318", "no host"),
            ("http://", "no host"),
            ("http://::1:4318", "brackets"),
            ("http://[::1", "closing ]"),
            ("http://[zz]:4318", "not an IPv6 address"),
            ("http://[::1]x", "after the IPv6 address"),
        ] {
            let e = otlp_config_from_endpoint("svc", raw).expect_err(raw);
            assert!(e.contains(&format!("{raw:?}")), "{raw}: {e}");
            assert!(e.contains(why), "{raw}: {e}");
        }
    }
}
