// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// `zerodds-grpc-bridged` — DDS↔gRPC-Bridge-Daemon.
//
// Spec: `docs/specs/zerodds-grpc-bridge-1.0.md`. Implementiert die
// L1-L4-Pflichtschicht: HTTP/2-Server (Preface + SETTINGS-Handshake +
// HEADERS/DATA-Frames), gRPC-Length-Prefixed-Messaging, Reflection-
// Service-Stub, YAML-Config-File, CLI-Surface §2.
//
// L5 (TLS+Auth) und L6 (Multi-Tenant) sind als FUTURE-Hooks markiert
// (siehe `tls_active` und `auth_mode`-Felder in Config).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::approx_constant,
    clippy::unreachable,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    clippy::useless_conversion,
    missing_docs
)]

use std::env;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use zerodds_grpc_bridge::daemon_runtime::{
    BridgeMetrics, CatalogSnapshot, CatalogTopic, SERVICE_NAME, install_signal_watcher,
    otlp_config_from_env, serve_admin_endpoints, spawn_otlp_flush_loop,
};
use zerodds_grpc_bridge::server::{GrpcRequest, GrpcResponse, GrpcServer};
use zerodds_grpc_bridge::status::Status;
use zerodds_http2::settings::encode_settings;
use zerodds_http2::{Flags, FrameHeader, FrameType, check_preface, encode_frame};
use zerodds_monitor::Registry;
use zerodds_observability_otlp::OtlpExporter;

const VERSION: &str = "1.0.0";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(e.exit_code())
        }
    }
}

#[derive(Debug)]
struct DaemonError {
    code: u8,
    msg: String,
}

impl DaemonError {
    fn new(code: u8, msg: impl Into<String>) -> Self {
        Self {
            code,
            msg: msg.into(),
        }
    }
    fn exit_code(&self) -> u8 {
        self.code
    }
}

impl std::fmt::Display for DaemonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

impl std::error::Error for DaemonError {}

impl From<std::io::Error> for DaemonError {
    fn from(e: std::io::Error) -> Self {
        Self::new(2, format!("io: {e}"))
    }
}

fn run() -> Result<(), DaemonError> {
    let args: Vec<String> = env::args().collect();
    let mut bind = "0.0.0.0:50051".to_string();
    let mut config_path: Option<String> = None;
    let mut domain: i32 = 0;
    let mut tls_cert: Option<String> = None;
    let mut tls_key: Option<String> = None;
    let mut reflection = false;
    let mut topic_overrides: Vec<String> = Vec::new();
    let mut log_level = String::from("info");
    let mut metrics: Option<String> = None;
    let mut once_shot: bool = false;
    let mut idle_timeout_ms: u64 = 0;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config" => {
                i += 1;
                config_path = args.get(i).cloned();
            }
            "--bind" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    bind = v.clone();
                }
            }
            "--domain" => {
                i += 1;
                domain = args
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| DaemonError::new(1, "--domain needs i32"))?;
            }
            "--tls-cert" => {
                i += 1;
                tls_cert = args.get(i).cloned();
            }
            "--tls-key" => {
                i += 1;
                tls_key = args.get(i).cloned();
            }
            "--reflection" => reflection = true,
            "--topic" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    topic_overrides.push(v.clone());
                }
            }
            "--log-level" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    log_level = v.clone();
                }
            }
            "--metrics" => {
                i += 1;
                metrics = args.get(i).cloned();
            }
            "--version" | "-V" => {
                println!("zerodds-grpc-bridged {VERSION}");
                return Ok(());
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            // Test-only: serve exactly one connection then exit.
            "--once" => once_shot = true,
            "--idle-timeout-ms" => {
                i += 1;
                if let Some(v) = args.get(i).and_then(|s| s.parse().ok()) {
                    idle_timeout_ms = v;
                }
            }
            other => {
                return Err(DaemonError::new(1, format!("unknown argument: {other}")));
            }
        }
        i += 1;
    }

    let mut cfg = if let Some(path) = config_path.as_deref() {
        load_config(path).map_err(|e| DaemonError::new(1, format!("config: {e}")))?
    } else {
        DaemonConfig::default()
    };

    // CLI overrides config.
    if bind != "0.0.0.0:50051" {
        cfg.bind = bind.clone();
    }
    if domain != 0 {
        cfg.domain = domain;
    }
    cfg.reflection_enabled = reflection || cfg.reflection_enabled;
    if let (Some(c), Some(k)) = (tls_cert, tls_key) {
        cfg.tls_cert_file = c;
        cfg.tls_key_file = k;
        cfg.tls_enabled = true;
    }
    for ov in topic_overrides {
        // CLI-Format `<DDS-Name>=<gRPC-Service>` (Spec §2 single-topic
        // override). Wir nehmen das letzte `=` damit DDS-Namen mit
        // `::`-Scope (Chat::Message) intakt bleiben.
        if let Some(eq_pos) = ov.rfind('=') {
            cfg.topics.push(TopicMapping {
                dds_name: ov[..eq_pos].into(),
                grpc_service: ov[eq_pos + 1..].into(),
                direction: "bidir".into(),
            });
        } else if let Some(colon_pos) = ov.rfind(':') {
            // Fallback: split at last `:` (in case caller used colon-form).
            cfg.topics.push(TopicMapping {
                dds_name: ov[..colon_pos].into(),
                grpc_service: ov[colon_pos + 1..].into(),
                direction: "bidir".into(),
            });
        }
    }

    let _ = (log_level, &cfg.tls_cert_file);

    eprintln!(
        "{{\"event\":\"startup\",\"bind\":\"{}\",\"domain\":{},\"topics\":{},\"reflection\":{}}}",
        cfg.bind,
        cfg.domain,
        cfg.topics.len(),
        cfg.reflection_enabled,
    );

    let shutdown = Arc::new(AtomicBool::new(false));
    let reload = Arc::new(AtomicBool::new(false));

    // Metrics-Registry + Standard-Counter (§8.2 Prometheus).
    let registry = Arc::new(Registry::new());
    let bridge_metrics = BridgeMetrics::register(&registry);

    // Signal-Watcher (§9.2 Graceful Shutdown).
    if let Err(e) = install_signal_watcher(Arc::clone(&shutdown), Arc::clone(&reload)) {
        eprintln!("{{\"event\":\"signal_watcher_init_failed\",\"err\":\"{e}\"}}");
    }

    // Admin-Endpoint (§5.2 Catalog/Healthz + §8.2 Metrics).
    let healthy = Arc::new(AtomicBool::new(true));
    let _admin_h = if let Some(addr_s) = metrics.as_deref().filter(|s| !s.is_empty()) {
        match addr_s.parse::<std::net::SocketAddr>() {
            Ok(sa) => {
                let topics: Vec<CatalogTopic> = cfg
                    .topics
                    .iter()
                    .map(|t| CatalogTopic {
                        dds_name: t.dds_name.clone(),
                        amqp_address: t.grpc_service.clone(),
                        direction: t.direction.clone(),
                    })
                    .collect();
                let snap = Arc::new(CatalogSnapshot::new(topics));
                match serve_admin_endpoints(
                    sa,
                    snap,
                    Arc::clone(&registry),
                    Arc::clone(&healthy),
                    Arc::clone(&shutdown),
                ) {
                    Ok((h, bound)) => {
                        eprintln!("{{\"event\":\"admin_endpoint\",\"addr\":\"{bound}\"}}");
                        Some(h)
                    }
                    Err(e) => {
                        eprintln!("{{\"event\":\"admin_endpoint_failed\",\"err\":\"{e}\"}}");
                        None
                    }
                }
            }
            Err(_) => None,
        }
    } else {
        None
    };

    // OTLP-Exporter (§8.3).
    let _otlp_h = if let Some(otlp_cfg) = otlp_config_from_env(SERVICE_NAME) {
        let exp = Arc::new(OtlpExporter::new(otlp_cfg));
        spawn_otlp_flush_loop(exp, Arc::clone(&shutdown), Duration::from_secs(5)).ok()
    } else {
        None
    };

    // FUTURE (L2): DcpsRuntime::start(domain, …) — DDS-Side. Im
    // L1-L4-Pflichtumfang reicht der HTTP/2-Server-Kern.

    let res = serve(
        &cfg,
        shutdown.clone(),
        once_shot,
        idle_timeout_ms,
        &bridge_metrics,
    );
    healthy.store(false, Ordering::SeqCst);
    res?;
    Ok(())
}

fn print_usage() {
    eprintln!("zerodds-grpc-bridged {VERSION}");
    eprintln!();
    eprintln!("USAGE: zerodds-grpc-bridged [OPTIONS]");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  --config <FILE>          YAML config (Spec §3)");
    eprintln!("  --bind <ADDR>            HTTP/2 bind addr (default 0.0.0.0:50051)");
    eprintln!("  --domain <ID>            DDS Domain ID (default 0)");
    eprintln!("  --tls-cert <FILE>        TLS cert PEM");
    eprintln!("  --tls-key <FILE>         TLS key PEM");
    eprintln!("  --reflection             Enable gRPC reflection service");
    eprintln!("  --topic <DDS:SVC>        Inline topic mapping override");
    eprintln!("  --log-level <LEVEL>      trace|debug|info|warn|error");
    eprintln!("  --metrics <ADDR>         Prometheus listen addr");
    eprintln!("  --version                Print version");
    eprintln!("  --help                   Show this help");
}

// ============================================================================
// Server
// ============================================================================

fn serve(
    cfg: &DaemonConfig,
    shutdown: Arc<AtomicBool>,
    once: bool,
    idle_timeout_ms: u64,
    metrics: &BridgeMetrics,
) -> Result<(), DaemonError> {
    let listener = TcpListener::bind(&cfg.bind)
        .map_err(|e| DaemonError::new(2, format!("bind {}: {}", cfg.bind, e)))?;
    listener
        .set_nonblocking(false)
        .map_err(|e| DaemonError::new(2, format!("set_nonblocking: {e}")))?;

    if idle_timeout_ms > 0 {
        // For test mode: stop accepting after timeout.
        let sd = Arc::clone(&shutdown);
        let dur = Duration::from_millis(idle_timeout_ms);
        std::thread::spawn(move || {
            std::thread::sleep(dur);
            sd.store(true, Ordering::SeqCst);
        });
    }

    eprintln!("{{\"event\":\"listening\",\"addr\":\"{}\"}}", cfg.bind);

    listener
        .set_nonblocking(true)
        .map_err(|e| DaemonError::new(2, format!("set_nonblocking: {e}")))?;

    let mut accepted = 0usize;
    while !shutdown.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, peer)) => {
                eprintln!("{{\"event\":\"accept\",\"peer\":\"{peer}\"}}");
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_nodelay(true);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                metrics.connections_total.inc();
                metrics.connections_active.inc();
                if let Err(e) = handle_connection(stream, cfg, metrics) {
                    metrics.errors_total.inc();
                    eprintln!("{{\"event\":\"conn_error\",\"msg\":\"{e}\"}}");
                }
                metrics.connections_active.dec();
                accepted += 1;
                if once {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                eprintln!("{{\"event\":\"accept_error\",\"msg\":\"{e}\"}}");
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
    eprintln!("{{\"event\":\"shutdown\",\"connections\":{accepted}}}");
    Ok(())
}

fn handle_connection(
    mut stream: TcpStream,
    cfg: &DaemonConfig,
    metrics: &BridgeMetrics,
) -> std::io::Result<()> {
    // Read client preface (24 bytes).
    let mut preface_buf = [0u8; 24];
    stream.read_exact(&mut preface_buf)?;
    if check_preface(&preface_buf).is_err() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "bad preface",
        ));
    }

    // Send our SETTINGS frame (empty).
    let settings_payload = encode_settings(&[]);
    let h = FrameHeader {
        length: settings_payload.len() as u32,
        frame_type: FrameType::Settings,
        flags: Flags(0),
        stream_id: 0u32,
    };
    let mut buf = vec![0u8; 9 + settings_payload.len()];
    encode_frame(&h, &settings_payload, &mut buf, 16384)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "encode settings"))?;
    stream.write_all(&buf)?;

    // Run the gRPC server loop.
    let mut grpc = GrpcServer::new();
    let mut read_buf = vec![0u8; 65536];
    let mut acc: Vec<u8> = Vec::new();

    let mut _responded = 0u32;
    loop {
        let n = match stream.read(&mut read_buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(e) => return Err(e),
        };
        acc.extend_from_slice(&read_buf[..n]);
        metrics.bytes_in_total.add(n as u64);

        // Process all frames available.
        loop {
            if acc.len() < 9 {
                break;
            }
            match grpc.process_frame(&acc) {
                Ok((maybe_req, consumed)) => {
                    if consumed == 0 {
                        break;
                    }
                    acc.drain(..consumed);
                    if let Some(req) = maybe_req {
                        metrics.frames_in_total.inc();
                        if matches!(req.method.as_str(), "Publish" | "PublishOne") {
                            metrics.dds_samples_in_total.inc();
                        }
                        let resp = dispatch(&req, cfg);
                        match grpc.encode_response(&resp) {
                            Ok(out) => {
                                metrics.bytes_out_total.add(out.len() as u64);
                                stream.write_all(&out)?;
                                metrics.frames_out_total.inc();
                                _responded += 1;
                            }
                            Err(e) => {
                                metrics.errors_total.inc();
                                eprintln!("{{\"event\":\"encode_error\",\"msg\":\"{e}\"}}");
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    metrics.errors_total.inc();
                    eprintln!("{{\"event\":\"decode_error\",\"msg\":\"{e}\"}}");
                    return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e));
                }
            }
        }
    }

    Ok(())
}

fn dispatch(req: &GrpcRequest, cfg: &DaemonConfig) -> GrpcResponse {
    eprintln!(
        "{{\"event\":\"rpc\",\"service\":\"{}\",\"method\":\"{}\",\"stream\":{}}}",
        req.service,
        req.method,
        u32::from(req.stream_id),
    );

    // Reflection service (Spec §4.6).
    if req.service == "grpc.reflection.v1alpha.ServerReflection" {
        if cfg.reflection_enabled {
            return GrpcResponse {
                stream_id: req.stream_id,
                status: Status::Ok,
                message: None,
                body: build_reflection_catalog(cfg).into_bytes(),
            };
        }
        return GrpcResponse {
            stream_id: req.stream_id,
            status: Status::Unimplemented,
            message: Some("reflection disabled".into()),
            body: Vec::new(),
        };
    }

    // Catalog RPC (Spec §4.2 + §5.2).
    if req.method == "Catalog" {
        return GrpcResponse {
            stream_id: req.stream_id,
            status: Status::Ok,
            message: None,
            body: build_reflection_catalog(cfg).into_bytes(),
        };
    }

    // Topic-Mapping-Lookup. Match auf Full-Service-Name oder
    // Last-Component (Spec §5.1: `zerodds.chat.v1.ChatMessageStream`
    // ist das voll-qualifizierte Service, `ChatMessageStream` der Slug).
    if let Some(topic) = cfg.topics.iter().find(|t| {
        t.grpc_service == req.service
            || req
                .service
                .rsplit('.')
                .next()
                .map(|last| last == t.grpc_service)
                .unwrap_or(false)
    }) {
        match req.method.as_str() {
            "Publish" | "PublishOne" => GrpcResponse {
                stream_id: req.stream_id,
                status: Status::Ok,
                message: None,
                // Echo back as PublishAck stub. FUTURE: write to DDS Writer.
                body: req.body.clone(),
            },
            "Subscribe" => {
                // FUTURE: stream from DDS Reader. Minimal: empty stream-end.
                GrpcResponse {
                    stream_id: req.stream_id,
                    status: Status::Ok,
                    message: Some(format!("dds_topic={}", topic.dds_name)),
                    body: Vec::new(),
                }
            }
            _ => GrpcResponse {
                stream_id: req.stream_id,
                status: Status::Unimplemented,
                message: Some(format!("method {} not on {}", req.method, req.service)),
                body: Vec::new(),
            },
        }
    } else {
        GrpcResponse {
            stream_id: req.stream_id,
            status: Status::NotFound,
            message: Some(format!("unknown service {}", req.service)),
            body: Vec::new(),
        }
    }
}

fn build_reflection_catalog(cfg: &DaemonConfig) -> String {
    let mut out = String::from("{\"topics\":[");
    for (i, t) in cfg.topics.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"dds_name\":\"{}\",\"grpc_service\":\"{}\",\"direction\":\"{}\"}}",
            t.dds_name, t.grpc_service, t.direction
        ));
    }
    out.push_str("]}");
    out
}

// ============================================================================
// Config
// ============================================================================

#[derive(Debug, Clone)]
pub(crate) struct DaemonConfig {
    pub bind: String,
    pub domain: i32,
    pub tls_enabled: bool,
    pub tls_cert_file: String,
    pub tls_key_file: String,
    pub reflection_enabled: bool,
    pub auth_mode: String,
    pub topics: Vec<TopicMapping>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:50051".into(),
            domain: 0,
            tls_enabled: false,
            tls_cert_file: String::new(),
            tls_key_file: String::new(),
            reflection_enabled: false,
            auth_mode: "none".into(),
            topics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TopicMapping {
    pub dds_name: String,
    pub grpc_service: String,
    pub direction: String,
}

fn load_config(path: &str) -> Result<DaemonConfig, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
    parse_yaml_subset(&raw)
}

fn parse_yaml_subset(s: &str) -> Result<DaemonConfig, String> {
    let mut cfg = DaemonConfig::default();
    let mut section: Vec<String> = Vec::new();
    let mut cur_topic: Option<TopicMapping> = None;

    for (lineno, raw) in s.lines().enumerate() {
        let line = match raw.find('#') {
            Some(i) => &raw[..i],
            None => raw,
        };
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.bytes().take_while(|b| *b == b' ').count();
        let trimmed = line.trim_start();
        let depth = indent / 2;
        section.truncate(depth);

        if let Some(rest) = trimmed.strip_prefix("- ") {
            if section.last().map(String::as_str) == Some("topics") {
                if let Some(t) = cur_topic.take() {
                    cfg.topics.push(t);
                }
                let mut t = TopicMapping {
                    dds_name: String::new(),
                    grpc_service: String::new(),
                    direction: "bidir".into(),
                };
                if let Some((k, v)) = split_kv(rest) {
                    match k {
                        "dds_name" => t.dds_name = unquote(v).into(),
                        "grpc_service" => t.grpc_service = unquote(v).into(),
                        "direction" => t.direction = unquote(v).into(),
                        _ => {}
                    }
                }
                cur_topic = Some(t);
            }
            continue;
        }

        let Some((key, value)) = split_kv(trimmed) else {
            return Err(format!("line {}: not k:v: `{trimmed}`", lineno + 1));
        };

        if value.is_empty() {
            section.push(key.into());
            continue;
        }

        let path_str = section.join("/");
        match (path_str.as_str(), key) {
            ("", "domain") => {
                cfg.domain = value
                    .parse()
                    .map_err(|e| format!("line {}: bad domain: {e}", lineno + 1))?;
            }
            ("grpc", "bind") => cfg.bind = unquote(value).into(),
            ("grpc/tls", "enabled") => cfg.tls_enabled = matches!(value, "true" | "yes" | "1"),
            ("grpc/tls", "cert_file") => cfg.tls_cert_file = unquote(value).into(),
            ("grpc/tls", "key_file") => cfg.tls_key_file = unquote(value).into(),
            ("grpc/reflection", "enabled") => {
                cfg.reflection_enabled = matches!(value, "true" | "yes" | "1");
            }
            ("auth", "mode") => cfg.auth_mode = unquote(value).into(),
            (_, _) => {
                if section.last().map(String::as_str) == Some("topics") {
                    if let Some(t) = cur_topic.as_mut() {
                        match key {
                            "dds_name" => t.dds_name = unquote(value).into(),
                            "grpc_service" => t.grpc_service = unquote(value).into(),
                            "direction" => t.direction = unquote(value).into(),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    if let Some(t) = cur_topic.take() {
        cfg.topics.push(t);
    }
    Ok(cfg)
}

fn split_kv(s: &str) -> Option<(&str, &str)> {
    let i = s.find(':')?;
    Some((s[..i].trim(), s[i + 1..].trim()))
}

fn unquote(s: &str) -> &str {
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        if s.len() >= 2 {
            return &s[1..s.len() - 1];
        }
    }
    s
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn config_default_has_default_bind() {
        let c = DaemonConfig::default();
        assert_eq!(c.bind, "0.0.0.0:50051");
        assert!(!c.tls_enabled);
    }

    #[test]
    fn parse_minimal_grpc_config() {
        let yaml = "\
domain: 5
grpc:
  bind: \"127.0.0.1:50059\"
  reflection:
    enabled: true
auth:
  mode: \"jwt\"
";
        let cfg = parse_yaml_subset(yaml).expect("parse");
        assert_eq!(cfg.domain, 5);
        assert_eq!(cfg.bind, "127.0.0.1:50059");
        assert!(cfg.reflection_enabled);
        assert_eq!(cfg.auth_mode, "jwt");
    }

    #[test]
    fn parse_topics_array() {
        let yaml = "\
topics:
  - dds_name: \"Chat::Message\"
    grpc_service: \"ChatMessageStream\"
    direction: \"bidir\"
  - dds_name: \"Sensor::Reading\"
    grpc_service: \"SensorReadingStream\"
    direction: \"out\"
";
        let cfg = parse_yaml_subset(yaml).expect("parse");
        assert_eq!(cfg.topics.len(), 2);
        assert_eq!(cfg.topics[0].dds_name, "Chat::Message");
        assert_eq!(cfg.topics[0].grpc_service, "ChatMessageStream");
        assert_eq!(cfg.topics[1].direction, "out");
    }

    #[test]
    fn parse_invalid_yaml_returns_err() {
        let res = parse_yaml_subset("just a string");
        assert!(res.is_err());
    }

    #[test]
    fn build_catalog_emits_topics() {
        let cfg = DaemonConfig {
            topics: vec![TopicMapping {
                dds_name: "Foo::Bar".into(),
                grpc_service: "FooBarStream".into(),
                direction: "bidir".into(),
            }],
            ..DaemonConfig::default()
        };
        let cat = build_reflection_catalog(&cfg);
        assert!(cat.contains("Foo::Bar"));
        assert!(cat.contains("FooBarStream"));
    }

    #[test]
    fn dispatch_unknown_service_yields_not_found() {
        let cfg = DaemonConfig::default();
        let req = GrpcRequest {
            stream_id: 1u32,
            path: "/no.such.Svc/X".into(),
            service: "no.such.Svc".into(),
            method: "X".into(),
            encoding: None,
            body: Vec::new(),
        };
        let resp = dispatch(&req, &cfg);
        assert_eq!(resp.status, Status::NotFound);
    }

    #[test]
    fn dispatch_publish_on_known_topic_is_ok() {
        let cfg = DaemonConfig {
            topics: vec![TopicMapping {
                dds_name: "T::M".into(),
                grpc_service: "TMStream".into(),
                direction: "bidir".into(),
            }],
            ..DaemonConfig::default()
        };
        let req = GrpcRequest {
            stream_id: 3u32,
            path: "/zerodds.t.v1.TMStream/Publish".into(),
            service: "TMStream".into(),
            method: "Publish".into(),
            encoding: None,
            body: vec![1, 2, 3, 4],
        };
        let resp = dispatch(&req, &cfg);
        assert_eq!(resp.status, Status::Ok);
        assert_eq!(resp.body, vec![1, 2, 3, 4]);
    }
}
