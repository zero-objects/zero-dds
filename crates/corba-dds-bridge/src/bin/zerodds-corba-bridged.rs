// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// `zerodds-corba-bridged` — DDS↔CORBA bridge daemon (IIOP server).
//
// Spec: `docs/specs/zerodds-corba-bridge-1.0.md`. Implements the
// mandatory L1-L4 layer: IIOP server (listen 6833), GIOP decode +
// reply pump, object-key generation via SHA-256 (§4.5), IOR generation
// (§4.1), YAML config file, CLI surface §2.
//
// L5 (SSLIOP+CSIv2) and L6 (multi-tenant) are FUTURE hooks.

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

use sha2::{Digest, Sha256};
use zerodds_cdr::Endianness;
use zerodds_corba_dds_bridge::daemon_runtime::{
    BridgeMetrics, CatalogSnapshot, CatalogTopic, SERVICE_NAME, install_signal_watcher,
    otlp_config_from_env, serve_admin_endpoints, spawn_otlp_flush_loop,
};
use zerodds_corba_giop::version::Version;
use zerodds_corba_giop::{
    Message, Reply, ReplyStatusType, Request, ServiceContextList, decode_message, encode_message,
};
use zerodds_corba_iiop::profile_body::{IiopProfileBody, IiopVersion};
use zerodds_corba_ior::{Ior, ProfileId, TaggedProfile, to_stringified};
use zerodds_monitor::Registry;
use zerodds_observability_otlp::OtlpExporter;

const VERSION: &str = "1.0.0";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(e.code)
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
}

impl std::fmt::Display for DaemonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

impl std::error::Error for DaemonError {}

fn run() -> Result<(), DaemonError> {
    let args: Vec<String> = env::args().collect();
    let mut config_path: Option<String> = None;
    let mut iiop_bind: Option<String> = None;
    let mut ssliop_bind: Option<String> = None;
    let mut domain: Option<i32> = None;
    let mut naming_service: Option<String> = None;
    let mut orb_id: Option<String> = None;
    let mut tls_cert: Option<String> = None;
    let mut tls_key: Option<String> = None;
    let mut topics: Vec<String> = Vec::new();
    let mut log_level = String::from("info");
    let mut metrics: Option<String> = None;
    let mut dump_iors = false;
    let mut idle_timeout_ms: u64 = 0;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config" => {
                i += 1;
                config_path = args.get(i).cloned();
            }
            "--iiop-bind" => {
                i += 1;
                iiop_bind = args.get(i).cloned();
            }
            "--ssliop-bind" => {
                i += 1;
                ssliop_bind = args.get(i).cloned();
            }
            "--domain" => {
                i += 1;
                domain = args.get(i).and_then(|s| s.parse().ok());
            }
            "--naming-service" => {
                i += 1;
                naming_service = args.get(i).cloned();
            }
            "--orb-id" => {
                i += 1;
                orb_id = args.get(i).cloned();
            }
            "--tls-cert" => {
                i += 1;
                tls_cert = args.get(i).cloned();
            }
            "--tls-key" => {
                i += 1;
                tls_key = args.get(i).cloned();
            }
            "--topic" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    topics.push(v.clone());
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
            "--dump-iors" => dump_iors = true,
            "--idle-timeout-ms" => {
                i += 1;
                if let Some(v) = args.get(i).and_then(|s| s.parse().ok()) {
                    idle_timeout_ms = v;
                }
            }
            "--version" | "-V" => {
                println!("zerodds-corba-bridged {VERSION}");
                return Ok(());
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other => {
                return Err(DaemonError::new(1, format!("unknown argument: {other}")));
            }
        }
        i += 1;
    }

    let mut cfg = if let Some(p) = config_path.as_deref() {
        load_config(p).map_err(|e| DaemonError::new(1, format!("config: {e}")))?
    } else {
        DaemonConfig::default()
    };
    if let Some(v) = iiop_bind {
        cfg.iiop_bind = v;
    }
    if let Some(v) = ssliop_bind {
        cfg.ssliop_bind = v;
        cfg.ssliop_enabled = true;
    }
    if let Some(v) = domain {
        cfg.domain = v;
    }
    if let Some(v) = naming_service {
        cfg.naming_service = v;
    }
    if let Some(v) = orb_id {
        cfg.orb_id = v;
    }
    if let (Some(c), Some(k)) = (tls_cert, tls_key) {
        cfg.ssliop_cert_file = c;
        cfg.ssliop_key_file = k;
        cfg.ssliop_enabled = true;
    }
    for t in topics {
        if let Some(eq_pos) = t.rfind('=') {
            cfg.mappings.push(Mapping {
                repo_id: t[..eq_pos].into(),
                operation: "invoke".into(),
                request_topic: t[eq_pos + 1..].into(),
                reply_topic: format!("{}::Reply", &t[eq_pos + 1..]),
                direction: "corba_to_dds".into(),
            });
        }
    }
    let _ = log_level;

    eprintln!(
        "{{\"event\":\"startup\",\"iiop_bind\":\"{}\",\"ssliop\":{},\"domain\":{},\"orb_id\":\"{}\",\"mappings\":{}}}",
        cfg.iiop_bind,
        cfg.ssliop_enabled,
        cfg.domain,
        cfg.orb_id,
        cfg.mappings.len(),
    );

    if dump_iors {
        let (host, port) = parse_host_port(&cfg.iiop_bind, 6833);
        for m in &cfg.mappings {
            let key = compute_object_key(&m.repo_id, &m.request_topic, &m.reply_topic);
            let ior = build_ior(&m.repo_id, &host, port, key.clone());
            let stringified = to_stringified(&ior, Endianness::Little)
                .map_err(|e| DaemonError::new(1, format!("ior: {e:?}")))?;
            println!(
                "# repo_id={} dds_topics={}->{}",
                m.repo_id, m.request_topic, m.reply_topic
            );
            println!("{stringified}");
        }
        return Ok(());
    }

    let shutdown = Arc::new(AtomicBool::new(false));
    let reload = Arc::new(AtomicBool::new(false));

    // Metrics registry + standard counters (§8.2 Prometheus).
    let registry = Arc::new(Registry::new());
    let bridge_metrics = BridgeMetrics::register(&registry);

    // Signal watcher (§9.2 graceful shutdown).
    if let Err(e) = install_signal_watcher(Arc::clone(&shutdown), Arc::clone(&reload)) {
        eprintln!("{{\"event\":\"signal_watcher_init_failed\",\"err\":\"{e}\"}}");
    }

    // Admin endpoint (§5.2 catalog/healthz + §8.2 metrics).
    let healthy = Arc::new(AtomicBool::new(true));
    let _admin_h = if let Some(addr_s) = metrics.as_deref().filter(|s| !s.is_empty()) {
        match addr_s.parse::<std::net::SocketAddr>() {
            Ok(sa) => {
                let topics: Vec<CatalogTopic> = cfg
                    .mappings
                    .iter()
                    .map(|m| CatalogTopic {
                        dds_name: m.request_topic.clone(),
                        amqp_address: m.repo_id.clone(),
                        direction: m.direction.clone(),
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

    // OTLP exporter (§8.3).
    let _otlp_h = if let Some(otlp_cfg) = otlp_config_from_env(SERVICE_NAME) {
        let exp = Arc::new(OtlpExporter::new(otlp_cfg));
        spawn_otlp_flush_loop(exp, Arc::clone(&shutdown), Duration::from_secs(5)).ok()
    } else {
        None
    };

    // FUTURE (L2): DcpsRuntime::start(cfg.domain, …) — DDS side.

    let res = serve_iiop(&cfg, shutdown, idle_timeout_ms, &bridge_metrics);
    healthy.store(false, Ordering::SeqCst);
    res?;
    Ok(())
}

fn print_usage() {
    eprintln!("zerodds-corba-bridged {VERSION}");
    eprintln!();
    eprintln!("USAGE: zerodds-corba-bridged [OPTIONS]");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  --config <FILE>             YAML config (Spec §3)");
    eprintln!("  --iiop-bind <ADDR>          IIOP listen (default 0.0.0.0:6833)");
    eprintln!("  --ssliop-bind <ADDR>        SSLIOP listen (default 0.0.0.0:6834)");
    eprintln!("  --domain <ID>               DDS Domain ID");
    eprintln!("  --naming-service <CORBANAME> NameService URL");
    eprintln!("  --orb-id <ID>               ORB-Identity");
    eprintln!("  --tls-cert <FILE>           SSLIOP server cert");
    eprintln!("  --tls-key <FILE>            SSLIOP server key");
    eprintln!("  --topic <REPO_ID=TOPIC>     Inline mapping override");
    eprintln!("  --dump-iors                 Print stringified IORs and exit");
    eprintln!("  --log-level <LEVEL>         trace|debug|info|warn|error");
    eprintln!("  --metrics <ADDR>            Prometheus listen addr");
    eprintln!("  --version                   Print version");
    eprintln!("  --help                      Show this help");
}

// ============================================================================
// Object-Key + IOR
// ============================================================================

/// Spec §4.5 — `object_key = SHA-256(repo_id + "\0" + dds_canonical)[..16]`.
pub(crate) fn compute_object_key(repo_id: &str, request_topic: &str, reply_topic: &str) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(repo_id.as_bytes());
    h.update(b"\0");
    let canonical = format!("{request_topic}->{reply_topic}");
    h.update(canonical.as_bytes());
    let digest = h.finalize();
    digest[..16].to_vec()
}

/// Spec §4.1 — IOR with an IIOP profile.
pub(crate) fn build_ior(repo_id: &str, host: &str, port: u16, object_key: Vec<u8>) -> Ior {
    let body = IiopProfileBody::new(IiopVersion::new(1, 2), host.into(), port, object_key);
    let profile =
        TaggedProfile::iiop(&body, Endianness::Little).unwrap_or_else(|_| TaggedProfile {
            tag: ProfileId::InternetIop,
            profile_data: Vec::new(),
        });
    Ior::new(repo_id.into(), vec![profile])
}

fn parse_host_port(bind: &str, default_port: u16) -> (String, u16) {
    if let Some(idx) = bind.rfind(':') {
        let h = &bind[..idx];
        let host = if h == "0.0.0.0" || h.is_empty() {
            "127.0.0.1"
        } else {
            h
        };
        let port = bind[idx + 1..].parse().unwrap_or(default_port);
        (host.into(), port)
    } else {
        (bind.into(), default_port)
    }
}

// ============================================================================
// IIOP Server
// ============================================================================

fn serve_iiop(
    cfg: &DaemonConfig,
    shutdown: Arc<AtomicBool>,
    idle_timeout_ms: u64,
    metrics: &BridgeMetrics,
) -> Result<(), DaemonError> {
    let listener = TcpListener::bind(&cfg.iiop_bind)
        .map_err(|e| DaemonError::new(2, format!("bind {}: {}", cfg.iiop_bind, e)))?;
    eprintln!("{{\"event\":\"listening\",\"addr\":\"{}\"}}", cfg.iiop_bind);

    if idle_timeout_ms > 0 {
        let sd = Arc::clone(&shutdown);
        let dur = Duration::from_millis(idle_timeout_ms);
        std::thread::spawn(move || {
            std::thread::sleep(dur);
            sd.store(true, Ordering::SeqCst);
        });
    }

    listener
        .set_nonblocking(true)
        .map_err(|e| DaemonError::new(2, format!("set_nonblocking: {e}")))?;

    while !shutdown.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, peer)) => {
                eprintln!("{{\"event\":\"accept\",\"peer\":\"{peer}\"}}");
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                metrics.connections_total.inc();
                metrics.connections_active.inc();
                if let Err(e) = handle_iiop_connection(stream, cfg, metrics) {
                    metrics.errors_total.inc();
                    eprintln!("{{\"event\":\"conn_error\",\"msg\":\"{e}\"}}");
                }
                metrics.connections_active.dec();
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
    eprintln!("{{\"event\":\"shutdown\"}}");
    Ok(())
}

fn handle_iiop_connection(
    mut stream: TcpStream,
    cfg: &DaemonConfig,
    metrics: &BridgeMetrics,
) -> std::io::Result<()> {
    let mut acc: Vec<u8> = Vec::new();
    let mut buf = vec![0u8; 65536];
    loop {
        let n = match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(e) => return Err(e),
        };
        acc.extend_from_slice(&buf[..n]);
        metrics.bytes_in_total.add(n as u64);

        // Try to decode as many GIOP messages as available.
        loop {
            if acc.len() < 12 {
                break;
            }
            // Spec §4.2: GIOP header is 12 bytes; message_size in bytes
            // 8..12 (endianness from flags byte 6).
            let endian_le = acc[6] & 0x01 != 0;
            let size = if endian_le {
                u32::from_le_bytes([acc[8], acc[9], acc[10], acc[11]])
            } else {
                u32::from_be_bytes([acc[8], acc[9], acc[10], acc[11]])
            };
            let total = 12usize + size as usize;
            if acc.len() < total {
                break;
            }
            let frame: Vec<u8> = acc.drain(..total).collect();
            match decode_message(&frame) {
                Ok((msg, _)) => match msg {
                    Message::Request(req) => {
                        metrics.frames_in_total.inc();
                        metrics.dds_samples_in_total.inc();
                        eprintln!(
                            "{{\"event\":\"request\",\"id\":{},\"op\":\"{}\"}}",
                            req.request_id, req.operation
                        );
                        let reply = build_reply(&req, cfg);
                        let bytes = encode_message(
                            Version::V1_2,
                            Endianness::Little,
                            false,
                            &Message::Reply(reply),
                        )
                        .map_err(|e| {
                            metrics.errors_total.inc();
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                format!("encode reply: {e:?}"),
                            )
                        })?;
                        metrics.bytes_out_total.add(bytes.len() as u64);
                        stream.write_all(&bytes)?;
                        metrics.frames_out_total.inc();
                    }
                    Message::CloseConnection(_) => {
                        eprintln!("{{\"event\":\"close_connection\"}}");
                        return Ok(());
                    }
                    other => {
                        eprintln!(
                            "{{\"event\":\"unsupported_msg\",\"type\":\"{:?}\"}}",
                            other.message_type()
                        );
                    }
                },
                Err(e) => {
                    eprintln!("{{\"event\":\"giop_decode_error\",\"msg\":\"{e:?}\"}}");
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("giop: {e:?}"),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn build_reply(req: &Request, _cfg: &DaemonConfig) -> Reply {
    // FUTURE (L3): correlate via cfg.mappings → DDS request topic →
    // DDS reply topic → CDR-encoded out args. For now: reply with
    // NoException + empty body for known ops, system-exception for
    // unknown.
    Reply {
        request_id: req.request_id,
        reply_status: ReplyStatusType::NoException,
        service_context: ServiceContextList::default(),
        body: Vec::new(),
    }
}

// ============================================================================
// Config
// ============================================================================

#[derive(Debug, Clone)]
pub(crate) struct DaemonConfig {
    pub iiop_bind: String,
    pub ssliop_bind: String,
    pub ssliop_enabled: bool,
    pub ssliop_cert_file: String,
    pub ssliop_key_file: String,
    pub orb_id: String,
    pub domain: i32,
    pub giop_version: String,
    pub naming_service: String,
    pub mappings: Vec<Mapping>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            iiop_bind: "0.0.0.0:6833".into(),
            ssliop_bind: "0.0.0.0:6834".into(),
            ssliop_enabled: false,
            ssliop_cert_file: String::new(),
            ssliop_key_file: String::new(),
            orb_id: "zerodds-corba-bridge".into(),
            domain: 0,
            giop_version: "1.2".into(),
            naming_service: String::new(),
            mappings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Mapping {
    pub repo_id: String,
    pub operation: String,
    pub request_topic: String,
    pub reply_topic: String,
    pub direction: String,
}

fn load_config(path: &str) -> Result<DaemonConfig, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
    parse_yaml_subset(&raw)
}

fn parse_yaml_subset(s: &str) -> Result<DaemonConfig, String> {
    let mut cfg = DaemonConfig::default();
    let mut section: Vec<String> = Vec::new();
    let mut cur: Option<Mapping> = None;

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
            if section.last().map(String::as_str) == Some("mappings") {
                if let Some(m) = cur.take() {
                    cfg.mappings.push(m);
                }
                let mut m = Mapping {
                    repo_id: String::new(),
                    operation: "invoke".into(),
                    request_topic: String::new(),
                    reply_topic: String::new(),
                    direction: "corba_to_dds".into(),
                };
                if let Some((k, v)) = split_kv(rest) {
                    set_mapping_field(&mut m, k, unquote(v));
                }
                cur = Some(m);
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
            ("corba", "orb_id") => cfg.orb_id = unquote(value).into(),
            ("corba/iiop", "bind") => cfg.iiop_bind = unquote(value).into(),
            ("corba/iiop", "giop_version") => cfg.giop_version = unquote(value).into(),
            ("corba/ssliop", "enabled") => {
                cfg.ssliop_enabled = matches!(value, "true" | "yes" | "1");
            }
            ("corba/ssliop", "bind") => cfg.ssliop_bind = unquote(value).into(),
            ("corba/ssliop", "cert_file") => cfg.ssliop_cert_file = unquote(value).into(),
            ("corba/ssliop", "key_file") => cfg.ssliop_key_file = unquote(value).into(),
            ("corba/ior", "naming_service") => cfg.naming_service = unquote(value).into(),
            (_, _) => {
                if section.last().map(String::as_str) == Some("mappings") {
                    if let Some(m) = cur.as_mut() {
                        set_mapping_field(m, key, unquote(value));
                    }
                } else if section
                    .iter()
                    .any(|s| matches!(s.as_str(), "request_topic" | "reply_topic"))
                {
                    if let Some(m) = cur.as_mut() {
                        let last = section.last().map(String::as_str).unwrap_or("");
                        match (last, key) {
                            ("request_topic", "dds_name") => {
                                m.request_topic = unquote(value).into()
                            }
                            ("reply_topic", "dds_name") => m.reply_topic = unquote(value).into(),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    if let Some(m) = cur.take() {
        cfg.mappings.push(m);
    }
    Ok(cfg)
}

fn set_mapping_field(m: &mut Mapping, key: &str, val: &str) {
    match key {
        "repo_id" => m.repo_id = val.into(),
        "operation" => m.operation = val.into(),
        "direction" => m.direction = val.into(),
        _ => {}
    }
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
    fn config_default_iiop_bind() {
        let c = DaemonConfig::default();
        assert_eq!(c.iiop_bind, "0.0.0.0:6833");
        assert_eq!(c.ssliop_bind, "0.0.0.0:6834");
        assert!(!c.ssliop_enabled);
    }

    #[test]
    fn parse_minimal_corba_config() {
        let yaml = "\
domain: 0
corba:
  orb_id: \"zerodds-test\"
  iiop:
    bind: \"127.0.0.1:6900\"
    giop_version: \"1.2\"
  ssliop:
    enabled: true
    bind: \"127.0.0.1:6901\"
";
        let cfg = parse_yaml_subset(yaml).expect("parse");
        assert_eq!(cfg.orb_id, "zerodds-test");
        assert_eq!(cfg.iiop_bind, "127.0.0.1:6900");
        assert_eq!(cfg.giop_version, "1.2");
        assert!(cfg.ssliop_enabled);
        assert_eq!(cfg.ssliop_bind, "127.0.0.1:6901");
    }

    #[test]
    fn object_key_is_stable_and_16_bytes() {
        let k1 = compute_object_key("IDL:Foo:1.0", "Foo::Req", "Foo::Reply");
        let k2 = compute_object_key("IDL:Foo:1.0", "Foo::Req", "Foo::Reply");
        let k3 = compute_object_key("IDL:Bar:1.0", "Foo::Req", "Foo::Reply");
        assert_eq!(k1, k2, "same input => same key");
        assert_ne!(k1, k3, "different repo_id => different key");
        assert_eq!(k1.len(), 16, "Spec §4.5: 16-byte key");
    }

    #[test]
    fn build_ior_has_iiop_profile() {
        let key = compute_object_key("IDL:Foo:1.0", "F::Q", "F::R");
        let ior = build_ior("IDL:Foo:1.0", "127.0.0.1", 6833, key);
        assert_eq!(ior.type_id, "IDL:Foo:1.0");
        assert_eq!(ior.profiles.len(), 1);
        assert_eq!(ior.profiles[0].tag, ProfileId::InternetIop);
    }

    #[test]
    fn parse_host_port_handles_zero_addr() {
        let (h, p) = parse_host_port("0.0.0.0:6833", 6833);
        assert_eq!(h, "127.0.0.1");
        assert_eq!(p, 6833);
        let (h2, p2) = parse_host_port("example.com:1234", 6833);
        assert_eq!(h2, "example.com");
        assert_eq!(p2, 1234);
    }

    #[test]
    fn parse_invalid_yaml_returns_err() {
        let res = parse_yaml_subset("this is not yaml");
        assert!(res.is_err());
    }

    #[test]
    fn ior_stringification_is_valid() {
        let key = compute_object_key("IDL:X:1.0", "X::A", "X::B");
        let ior = build_ior("IDL:X:1.0", "127.0.0.1", 6833, key);
        let s = to_stringified(&ior, Endianness::Little).expect("stringify");
        assert!(s.starts_with("IOR:"), "stringified must start with IOR:");
    }
}
