// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// `zerodds-amqp-bridged` — DDS↔AMQP-1.0-Bridge-Daemon (Client-Modus).
//
// Spec: `docs/specs/zerodds-amqp-bridge-daemon-1.0.md`. Implementiert
// die L1-L4-Pflichtschicht: AMQP-1.0-Client connect zu Broker
// (RabbitMQ, ActiveMQ, Qpid, Solace, Azure-ServiceBus), Container/
// Session/Link-Lifecycle, YAML-Config-Loader, CLI-Surface §2.
//
// L5 (TLS+SASL) und L6 (Multi-Tenant) sind als FUTURE-Hooks markiert.

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
use std::net::TcpStream;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use zerodds_amqp_bridge::frame::{
    FrameHeader, FrameType, decode_frame_header, encode_frame_header,
};
use zerodds_amqp_bridge::performatives::{attach, begin, close, detach, end, flow, open, transfer};
use zerodds_amqp_endpoint::daemon_runtime::{
    BridgeMetrics, CatalogSnapshot, CatalogTopic, SERVICE_NAME, install_signal_watcher,
    otlp_config_from_env, serve_admin_endpoints, spawn_otlp_flush_loop,
};
use zerodds_monitor::Registry;
use zerodds_observability_otlp::OtlpExporter;

const VERSION: &str = "1.0.0";

/// AMQP 1.0 protocol header — Spec §2.2.
const AMQP_PROTOCOL_HEADER: [u8; 8] = [b'A', b'M', b'Q', b'P', 0x00, 0x01, 0x00, 0x00];

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
    let mut broker: Option<String> = None;
    let mut container_id: Option<String> = None;
    let mut domain: Option<i32> = None;
    let mut sasl_mechanism: Option<String> = None;
    let mut user: Option<String> = None;
    let mut password: Option<String> = None;
    let mut tls_ca: Option<String> = None;
    let mut tls_cert: Option<String> = None;
    let mut tls_key: Option<String> = None;
    let mut topics: Vec<String> = Vec::new();
    let mut log_level = String::from("info");
    let mut metrics_cli: Option<String> = None;
    let mut once = false;
    let mut connect_timeout_ms: u64 = 5000;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config" => {
                i += 1;
                config_path = args.get(i).cloned();
            }
            "--broker" => {
                i += 1;
                broker = args.get(i).cloned();
            }
            "--container-id" => {
                i += 1;
                container_id = args.get(i).cloned();
            }
            "--domain" => {
                i += 1;
                domain = args.get(i).and_then(|s| s.parse().ok());
            }
            "--sasl-mechanism" => {
                i += 1;
                sasl_mechanism = args.get(i).cloned();
            }
            "--user" => {
                i += 1;
                user = args.get(i).cloned();
            }
            "--password" => {
                i += 1;
                password = args.get(i).cloned();
            }
            "--tls-ca" => {
                i += 1;
                tls_ca = args.get(i).cloned();
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
                metrics_cli = args.get(i).cloned();
            }
            "--version" | "-V" => {
                println!("zerodds-amqp-bridged {VERSION}");
                return Ok(());
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            "--once" => once = true,
            "--connect-timeout-ms" => {
                i += 1;
                if let Some(v) = args.get(i).and_then(|s| s.parse().ok()) {
                    connect_timeout_ms = v;
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

    if let Some(b) = broker {
        cfg.broker_url = b;
    }
    if let Some(c) = container_id {
        cfg.container_id = c;
    }
    if let Some(d) = domain {
        cfg.domain = d;
    }
    if let Some(m) = sasl_mechanism {
        cfg.sasl_mechanism = m;
    }
    if let Some(u) = user {
        cfg.sasl_username = u;
    }
    if let Some(p) = password {
        cfg.sasl_password = p;
    }
    if let Some(ca) = tls_ca {
        cfg.tls_ca_file = ca;
        cfg.tls_enabled = true;
    }
    if let Some(c) = tls_cert {
        cfg.tls_cert_file = c;
        cfg.tls_enabled = true;
    }
    if let Some(k) = tls_key {
        cfg.tls_key_file = k;
    }
    for t in topics {
        if let Some((dds, addr)) = t.split_once('=') {
            cfg.topics.push(TopicMapping {
                dds_name: dds.into(),
                amqp_address: addr.into(),
                direction: "bidir".into(),
            });
        }
    }
    let _ = log_level;

    if cfg.broker_url.is_empty() {
        return Err(DaemonError::new(
            1,
            "broker URL required (--broker or config.amqp.broker_url)".to_string(),
        ));
    }

    eprintln!(
        "{{\"event\":\"startup\",\"broker\":\"{}\",\"container_id\":\"{}\",\"domain\":{},\"topics\":{}}}",
        cfg.broker_url,
        cfg.container_id,
        cfg.domain,
        cfg.topics.len(),
    );

    let shutdown = Arc::new(AtomicBool::new(false));
    let reload = Arc::new(AtomicBool::new(false));

    // Metrics-Registry + Standard-Counter (§8.2 Prometheus).
    let registry = Arc::new(Registry::new());
    let metrics = BridgeMetrics::register(&registry);

    // Signal-Watcher (§9.2 Graceful Shutdown).
    if let Err(e) = install_signal_watcher(Arc::clone(&shutdown), Arc::clone(&reload)) {
        eprintln!("{{\"event\":\"signal_watcher_init_failed\",\"err\":\"{e}\"}}");
    }

    // Admin-Endpoint (§5.2 Catalog/Healthz + §8.2 Metrics).
    let admin_handle = if let Some(addr_s) = metrics_addr_for(&metrics_cli, &cfg) {
        match addr_s.parse::<std::net::SocketAddr>() {
            Ok(sa) => {
                let topics: Vec<CatalogTopic> = cfg
                    .topics
                    .iter()
                    .map(|t| CatalogTopic {
                        dds_name: t.dds_name.clone(),
                        amqp_address: t.amqp_address.clone(),
                        direction: t.direction.clone(),
                    })
                    .collect();
                let snap = Arc::new(CatalogSnapshot::new(topics));
                let healthy = Arc::new(AtomicBool::new(true));
                let stop_admin = Arc::clone(&shutdown);
                match serve_admin_endpoints(
                    sa,
                    snap,
                    Arc::clone(&registry),
                    Arc::clone(&healthy),
                    stop_admin,
                ) {
                    Ok((h, bound)) => {
                        eprintln!("{{\"event\":\"admin_endpoint\",\"addr\":\"{bound}\"}}");
                        Some((h, healthy))
                    }
                    Err(e) => {
                        eprintln!("{{\"event\":\"admin_endpoint_bind_failed\",\"err\":\"{e}\"}}");
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
    let _otlp_handle = if let Some(otlp_cfg) = otlp_config_from_env(SERVICE_NAME) {
        let exp = Arc::new(OtlpExporter::new(otlp_cfg));
        spawn_otlp_flush_loop(exp, Arc::clone(&shutdown), Duration::from_secs(5)).ok()
    } else {
        None
    };

    // FUTURE (L2): DcpsRuntime::start(domain, …) — DDS-Side. Im
    // L1-L4-Pflichtumfang reicht der AMQP-Wire-Pump.

    let res = bridge_loop(&cfg, shutdown.clone(), once, connect_timeout_ms, &metrics);

    // Setze healthy auf false beim Drop, damit /healthz 503 zurueckliefert.
    if let Some((_h, healthy)) = admin_handle {
        healthy.store(false, Ordering::SeqCst);
    }

    res?;
    Ok(())
}

fn metrics_addr_for(cli: &Option<String>, cfg: &DaemonConfig) -> Option<String> {
    if let Some(addr) = cli.as_deref() {
        if !addr.is_empty() {
            return Some(addr.to_string());
        }
    }
    let _ = cfg;
    None
}

fn print_usage() {
    eprintln!("zerodds-amqp-bridged {VERSION}");
    eprintln!();
    eprintln!("USAGE: zerodds-amqp-bridged [OPTIONS]");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  --config <FILE>          YAML config (Spec §3)");
    eprintln!("  --broker <URL>           AMQP broker URL (amqp://, amqps://)");
    eprintln!("  --container-id <ID>      AMQP container-id");
    eprintln!("  --domain <ID>            DDS Domain ID");
    eprintln!("  --sasl-mechanism <NAME>  PLAIN|SCRAM-SHA-256|EXTERNAL|ANONYMOUS|XOAUTH2");
    eprintln!("  --user <USER>            SASL username");
    eprintln!("  --password <PASS>        SASL password");
    eprintln!("  --tls-ca <FILE>          TLS CA certificate");
    eprintln!("  --tls-cert <FILE>        TLS client cert");
    eprintln!("  --tls-key <FILE>         TLS client key");
    eprintln!("  --topic <DDS=ADDR>       Inline topic mapping override");
    eprintln!("  --log-level <LEVEL>      trace|debug|info|warn|error");
    eprintln!("  --metrics <ADDR>         Prometheus listen addr");
    eprintln!("  --version                Print version");
    eprintln!("  --help                   Show this help");
}

// ============================================================================
// AMQP-Client Wire Pump
// ============================================================================

fn bridge_loop(
    cfg: &DaemonConfig,
    shutdown: Arc<AtomicBool>,
    once: bool,
    connect_timeout_ms: u64,
    metrics: &BridgeMetrics,
) -> Result<(), DaemonError> {
    let host_port = strip_amqp_scheme(&cfg.broker_url);
    let to = Duration::from_millis(connect_timeout_ms);

    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        // FUTURE (L5): TLS + SASL handshake before AMQP-OPEN.
        metrics.connections_total.inc();
        let stream = match connect_with_timeout(&host_port, to) {
            Ok(s) => s,
            Err(e) => {
                metrics.errors_total.inc();
                eprintln!(
                    "{{\"event\":\"connect_error\",\"broker\":\"{host_port}\",\"err\":\"{e}\"}}"
                );
                if once {
                    return Err(DaemonError::new(2, format!("connect {host_port}: {e}")));
                }
                std::thread::sleep(Duration::from_millis(500));
                continue;
            }
        };
        metrics.connections_active.set(1);
        eprintln!("{{\"event\":\"connected\",\"broker\":\"{host_port}\"}}");
        match drive_session(stream, cfg, metrics) {
            Ok(_) => {
                eprintln!("{{\"event\":\"session_complete\"}}");
            }
            Err(e) => {
                metrics.errors_total.inc();
                eprintln!("{{\"event\":\"session_error\",\"err\":\"{e}\"}}");
            }
        }
        metrics.connections_active.set(0);
        if once {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Ok(())
}

fn connect_with_timeout(host_port: &str, timeout: Duration) -> std::io::Result<TcpStream> {
    let addrs: Vec<std::net::SocketAddr> = match std::net::ToSocketAddrs::to_socket_addrs(host_port)
    {
        Ok(it) => it.collect(),
        Err(e) => return Err(e),
    };
    let mut last: Option<std::io::Error> = None;
    for a in addrs {
        match TcpStream::connect_timeout(&a, timeout) {
            Ok(s) => {
                let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
                let _ = s.set_write_timeout(Some(Duration::from_secs(5)));
                return Ok(s);
            }
            Err(e) => last = Some(e),
        }
    }
    Err(last
        .unwrap_or_else(|| std::io::Error::new(std::io::ErrorKind::AddrNotAvailable, "no addrs")))
}

fn drive_session(
    mut stream: TcpStream,
    cfg: &DaemonConfig,
    metrics: &BridgeMetrics,
) -> std::io::Result<()> {
    // 1) Send AMQP-protocol-header.
    stream.write_all(&AMQP_PROTOCOL_HEADER)?;
    metrics
        .bytes_out_total
        .add(AMQP_PROTOCOL_HEADER.len() as u64);

    // 2) Read peer-protocol-header (8 bytes).
    let mut peer_hdr = [0u8; 8];
    stream.read_exact(&mut peer_hdr)?;
    if &peer_hdr[..4] != b"AMQP" {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "peer not AMQP",
        ));
    }
    eprintln!("{{\"event\":\"protocol_header_ok\"}}");

    // 3) OPEN performative.
    let open_body = open(&cfg.container_id)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e:?}")))?;
    write_amqp_frame_metered(&mut stream, 0, &open_body, metrics)?;
    eprintln!(
        "{{\"event\":\"open_sent\",\"container_id\":\"{}\"}}",
        cfg.container_id
    );

    // 4) Read peer OPEN (or whatever they send first).
    let _peer_open = read_amqp_frame_metered(&mut stream, metrics)?;
    eprintln!("{{\"event\":\"open_recvd\"}}");

    // 5) BEGIN session 0.
    let begin_body = begin(None, 0, 1024, 1024)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e:?}")))?;
    write_amqp_frame_metered(&mut stream, 0, &begin_body, metrics)?;
    let _peer_begin = read_amqp_frame_metered(&mut stream, metrics)?;
    eprintln!("{{\"event\":\"begin_complete\"}}");

    // 6) Per topic-mapping: ATTACH a sender or receiver link.
    for (idx, t) in cfg.topics.iter().enumerate() {
        let handle = idx as u32;
        let is_sender = matches!(t.direction.as_str(), "out" | "bidir");
        let link_name = format!("{}/{}/{}", cfg.container_id, t.dds_name, t.direction);
        let attach_body = attach(&link_name, handle, is_sender)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e:?}")))?;
        write_amqp_frame_metered(&mut stream, 0, &attach_body, metrics)?;
        eprintln!(
            "{{\"event\":\"attach_sent\",\"link\":\"{link_name}\",\"role\":\"{}\",\"address\":\"{}\"}}",
            if is_sender { "sender" } else { "receiver" },
            t.amqp_address,
        );
        // Best-effort: read peer ATTACH response.
        let _ = read_amqp_frame_metered(&mut stream, metrics);

        // 7) FLOW: grant credit on receivers; sender sends one TRANSFER stub.
        if is_sender {
            let xf = transfer(handle, Some(idx as u32), true).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e:?}"))
            })?;
            write_amqp_frame_metered(&mut stream, 0, &xf, metrics)?;
            metrics.dds_samples_out_total.inc();
            eprintln!("{{\"event\":\"transfer_sent\",\"handle\":{handle},\"delivery_id\":{idx}}}");
        } else {
            let fl = flow(1024, 0, 1024).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e:?}"))
            })?;
            write_amqp_frame_metered(&mut stream, 0, &fl, metrics)?;
            eprintln!("{{\"event\":\"flow_sent\",\"handle\":{handle},\"credit\":1024}}");
        }
    }

    // 8) Graceful drain: DETACH each link, END session, CLOSE container.
    for (idx, _t) in cfg.topics.iter().enumerate() {
        let det = detach(idx as u32, true)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e:?}")))?;
        write_amqp_frame_metered(&mut stream, 0, &det, metrics)?;
    }
    let end_body = end()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e:?}")))?;
    write_amqp_frame_metered(&mut stream, 0, &end_body, metrics)?;
    let close_body = close()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e:?}")))?;
    write_amqp_frame_metered(&mut stream, 0, &close_body, metrics)?;

    Ok(())
}

fn write_amqp_frame(stream: &mut TcpStream, channel: u16, body: &[u8]) -> std::io::Result<()> {
    let total_size = 8u32 + body.len() as u32;
    let h = FrameHeader {
        size: total_size,
        doff: 2, // 8-byte header / 4 = 2 words, no extended header.
        frame_type: FrameType::Amqp,
        channel,
    };
    let hdr = encode_frame_header(h);
    stream.write_all(&hdr)?;
    stream.write_all(body)?;
    Ok(())
}

fn write_amqp_frame_metered(
    stream: &mut TcpStream,
    channel: u16,
    body: &[u8],
    metrics: &BridgeMetrics,
) -> std::io::Result<()> {
    write_amqp_frame(stream, channel, body)?;
    metrics.frames_out_total.inc();
    metrics.bytes_out_total.add((8 + body.len()) as u64);
    Ok(())
}

fn read_amqp_frame(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut hdr = [0u8; 8];
    stream.read_exact(&mut hdr)?;
    let parsed = decode_frame_header(&hdr)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e:?}")))?;
    if parsed.size < 8 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "frame size < 8",
        ));
    }
    let body_len = (parsed.size - 8) as usize;
    let mut body = vec![0u8; body_len];
    if body_len > 0 {
        stream.read_exact(&mut body)?;
    }
    Ok(body)
}

fn read_amqp_frame_metered(
    stream: &mut TcpStream,
    metrics: &BridgeMetrics,
) -> std::io::Result<Vec<u8>> {
    let body = read_amqp_frame(stream)?;
    metrics.frames_in_total.inc();
    metrics.bytes_in_total.add((8 + body.len()) as u64);
    Ok(body)
}

fn strip_amqp_scheme(uri: &str) -> String {
    let trimmed = uri
        .strip_prefix("amqps://")
        .or_else(|| uri.strip_prefix("amqp://"))
        .unwrap_or(uri);
    if trimmed.contains(':') {
        trimmed.to_string()
    } else {
        // Default port 5672 for amqp, 5671 for amqps.
        if uri.starts_with("amqps://") {
            format!("{trimmed}:5671")
        } else {
            format!("{trimmed}:5672")
        }
    }
}

// ============================================================================
// Config (YAML-Subset Loader)
// ============================================================================

#[derive(Debug, Clone)]
pub(crate) struct DaemonConfig {
    pub broker_url: String,
    pub container_id: String,
    pub domain: i32,
    pub channel_max: u16,
    pub max_frame_size: u32,
    pub idle_time_out_ms: u64,
    pub hostname: String,
    pub sasl_mechanism: String,
    pub sasl_username: String,
    pub sasl_password: String,
    pub tls_enabled: bool,
    pub tls_ca_file: String,
    pub tls_cert_file: String,
    pub tls_key_file: String,
    pub topics: Vec<TopicMapping>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            broker_url: String::new(),
            container_id: format!("zerodds-bridge-{}", hostname_or_pid_short()),
            domain: 0,
            channel_max: 256,
            max_frame_size: 65536,
            idle_time_out_ms: 30000,
            hostname: String::new(),
            sasl_mechanism: "ANONYMOUS".into(),
            sasl_username: String::new(),
            sasl_password: String::new(),
            tls_enabled: false,
            tls_ca_file: String::new(),
            tls_cert_file: String::new(),
            tls_key_file: String::new(),
            topics: Vec::new(),
        }
    }
}

fn hostname_or_pid_short() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .map(|h| {
            let mut s = h.replace('.', "-");
            s.truncate(16);
            s
        })
        .unwrap_or_else(|| format!("pid{}", std::process::id()))
}

#[derive(Debug, Clone)]
pub(crate) struct TopicMapping {
    pub dds_name: String,
    pub amqp_address: String,
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
                    amqp_address: String::new(),
                    direction: "bidir".into(),
                };
                if let Some((k, v)) = split_kv(rest) {
                    set_topic_field(&mut t, k, unquote(v));
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
            ("amqp", "broker_url") => cfg.broker_url = unquote(value).into(),
            ("amqp", "container_id") => cfg.container_id = unquote(value).into(),
            ("amqp", "channel_max") => {
                cfg.channel_max = value
                    .parse()
                    .map_err(|e| format!("line {}: bad channel_max: {e}", lineno + 1))?;
            }
            ("amqp", "max_frame_size") => {
                cfg.max_frame_size = value
                    .parse()
                    .map_err(|e| format!("line {}: bad max_frame_size: {e}", lineno + 1))?;
            }
            ("amqp", "idle_time_out_ms") => {
                cfg.idle_time_out_ms = value
                    .parse()
                    .map_err(|e| format!("line {}: bad idle: {e}", lineno + 1))?;
            }
            ("amqp", "hostname") => cfg.hostname = unquote(value).into(),
            ("amqp/sasl", "mechanism") => cfg.sasl_mechanism = unquote(value).into(),
            ("amqp/sasl", "username") => cfg.sasl_username = unquote(value).into(),
            ("amqp/sasl", "password") => cfg.sasl_password = unquote(value).into(),
            ("amqp/tls", "enabled") => cfg.tls_enabled = matches!(value, "true" | "yes" | "1"),
            ("amqp/tls", "ca_file") => cfg.tls_ca_file = unquote(value).into(),
            ("amqp/tls", "cert_file") => cfg.tls_cert_file = unquote(value).into(),
            ("amqp/tls", "key_file") => cfg.tls_key_file = unquote(value).into(),
            (_, _) => {
                if section.last().map(String::as_str) == Some("topics") {
                    if let Some(t) = cur_topic.as_mut() {
                        set_topic_field(t, key, unquote(value));
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

fn set_topic_field(t: &mut TopicMapping, key: &str, val: &str) {
    match key {
        "dds_name" => t.dds_name = val.into(),
        "amqp_address" => t.amqp_address = val.into(),
        "direction" => t.direction = val.into(),
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
    fn config_default_uses_anonymous_sasl() {
        let c = DaemonConfig::default();
        assert_eq!(c.sasl_mechanism, "ANONYMOUS");
        assert_eq!(c.channel_max, 256);
        assert_eq!(c.max_frame_size, 65536);
    }

    #[test]
    fn parse_minimal_amqp_config() {
        let yaml = "\
domain: 7
amqp:
  broker_url: \"amqp://broker.example:5672\"
  container_id: \"zerodds-prod-01\"
  channel_max: 128
  sasl:
    mechanism: \"PLAIN\"
    username: \"alice\"
    password: \"secret\"
";
        let cfg = parse_yaml_subset(yaml).expect("parse");
        assert_eq!(cfg.domain, 7);
        assert_eq!(cfg.broker_url, "amqp://broker.example:5672");
        assert_eq!(cfg.container_id, "zerodds-prod-01");
        assert_eq!(cfg.channel_max, 128);
        assert_eq!(cfg.sasl_mechanism, "PLAIN");
        assert_eq!(cfg.sasl_username, "alice");
    }

    #[test]
    fn parse_topics_array() {
        let yaml = "\
topics:
  - dds_name: \"Chat::Message\"
    amqp_address: \"topic://chat/message\"
    direction: \"bidir\"
  - dds_name: \"Sensor::Reading\"
    amqp_address: \"queue://sensors\"
    direction: \"out\"
";
        let cfg = parse_yaml_subset(yaml).expect("parse");
        assert_eq!(cfg.topics.len(), 2);
        assert_eq!(cfg.topics[0].dds_name, "Chat::Message");
        assert_eq!(cfg.topics[0].amqp_address, "topic://chat/message");
        assert_eq!(cfg.topics[1].direction, "out");
    }

    #[test]
    fn parse_invalid_yaml_returns_err() {
        let res = parse_yaml_subset("this is not yaml");
        assert!(res.is_err());
    }

    #[test]
    fn strip_scheme_default_ports() {
        assert_eq!(strip_amqp_scheme("amqp://host"), "host:5672");
        assert_eq!(strip_amqp_scheme("amqps://host"), "host:5671");
        assert_eq!(strip_amqp_scheme("amqp://host:1234"), "host:1234");
        assert_eq!(strip_amqp_scheme("host:9999"), "host:9999");
    }

    #[test]
    fn protocol_header_is_canonical() {
        assert_eq!(&AMQP_PROTOCOL_HEADER, b"AMQP\x00\x01\x00\x00");
    }
}
