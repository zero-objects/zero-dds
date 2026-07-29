// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// `zerodds-grpc-bridged` — DDS↔gRPC bridge daemon.
//
// Spec: `docs/specs/zerodds-grpc-bridge-1.0.md`. Implements the
// mandatory L1-L4 layer: HTTP/2 server (preface + SETTINGS handshake +
// HEADERS/DATA frames), gRPC length-prefixed messaging, reflection
// service stub, YAML config file, CLI surface §2.
//
// L5 (TLS+auth) and L6 (multi-tenant) are marked as FUTURE hooks
// (see the `tls_active` and `auth_mode` fields in Config).

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
use zerodds_grpc_bridge::decode_message;
use zerodds_grpc_bridge::server::{GrpcRequest, GrpcResponse, GrpcServer};
use zerodds_grpc_bridge::status::Status;
use zerodds_http2::settings::encode_settings;
use zerodds_http2::{Flags, FrameHeader, FrameType, check_preface, encode_frame};
use zerodds_monitor::Registry;
use zerodds_observability_otlp::OtlpExporter;

const VERSION: &str = "1.0.0";

// ============================================================================
// L2 — DDS side (bridge spec §4.2). Feature `dds-runtime`.
//
// Closes the former `FUTURE (L2)` stub: `Publish` writes the gRPC `Sample`
// payload to a real DDS DataWriter, `Subscribe` drains a DataReader. The
// opaque-bytes path mirrors the C-FFI writer/reader (`zerodds_writer_write`
// / `zerodds_reader_take`) — no typed Topic-Type, the gRPC `bytes payload`
// is the on-wire DDS user data 1:1.
// ============================================================================

/// Minimal protobuf reader for `Sample { bytes payload = 1; }`. Returns the
/// `payload` field (tag `0x0A`, wire-type 2 = LEN). Tolerant: unknown fields
/// are skipped, a missing field yields an empty slice.
fn proto_sample_payload(msg: &[u8]) -> Vec<u8> {
    let mut i = 0usize;
    while i < msg.len() {
        let tag = msg[i];
        i += 1;
        let field = tag >> 3;
        let wire = tag & 0x07;
        match wire {
            2 => {
                // LEN: varint length + bytes.
                let (len, adv) = proto_varint(&msg[i..]);
                i += adv;
                let end = (i + len as usize).min(msg.len());
                if field == 1 {
                    return msg[i..end].to_vec();
                }
                i = end;
            }
            0 => {
                // VARINT: skip.
                let (_, adv) = proto_varint(&msg[i..]);
                i += adv;
            }
            5 => i += 4, // I32
            1 => i += 8, // I64
            _ => break,  // unknown wire-type → stop
        }
    }
    Vec::new()
}

/// Encodes `PublishAck { uint64 accepted = 1; }` (tag `0x08`, wire-type 0).
fn proto_publish_ack(accepted: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(11);
    out.push(0x08);
    proto_put_varint(&mut out, accepted);
    out
}

/// Encodes `Sample { bytes payload = 1; }` (tag `0x0A`, wire-type 2).
fn proto_sample(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 6);
    out.push(0x0A);
    proto_put_varint(&mut out, payload.len() as u64);
    out.extend_from_slice(payload);
    out
}

/// Reads a base-128 varint; returns `(value, bytes_consumed)`.
fn proto_varint(b: &[u8]) -> (u64, usize) {
    let mut val = 0u64;
    let mut shift = 0u32;
    let mut i = 0usize;
    while i < b.len() && shift < 64 {
        let byte = b[i];
        i += 1;
        val |= u64::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    (val, i)
}

/// Appends `v` as a base-128 varint.
fn proto_put_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if v == 0 {
            break;
        }
    }
}

#[cfg(feature = "dds-runtime")]
mod bridge_dds {
    //! Real DDS runtime for the bridge daemon (feature `dds-runtime`).

    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};

    use zerodds_dcps::runtime::{
        DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
    };
    use zerodds_qos::{
        DeadlineQosPolicy, DurabilityKind, LifespanQosPolicy, LivelinessKind, LivelinessQosPolicy,
        OwnershipKind,
    };
    use zerodds_rtps::wire_types::{EntityId, GuidPrefix};

    use super::TopicMapping;

    /// Type name advertised by the bridge's opaque-bytes writer/reader. A
    /// peer DataReader must register the same `(topic, type)` pair to match.
    pub const BRIDGE_TYPE_NAME: &str = "ZeroDdsBridgeBytes";

    /// Per-topic egress reader: entity + its delivery channel.
    struct ReaderSlot {
        _eid: EntityId,
        rx: Mutex<std::sync::mpsc::Receiver<UserSample>>,
    }

    /// Holds the DDS participant + the per-topic writer/reader entities.
    pub struct BridgeDds {
        rt: Arc<DcpsRuntime>,
        /// `dds_name → ingress DataWriter` (gRPC `Publish` → DDS).
        writers: HashMap<String, EntityId>,
        /// `dds_name → egress DataReader` (DDS → gRPC `Subscribe`).
        readers: HashMap<String, ReaderSlot>,
    }

    impl BridgeDds {
        /// Starts the DDS participant for `domain` and registers a writer +
        /// reader on the same topic per mapping (loopback-capable: a peer or
        /// the bridge's own reader observes what `Publish` writes).
        pub fn start(domain: i32, topics: &[TopicMapping]) -> Result<Self, String> {
            let rt = DcpsRuntime::start(domain, guid_prefix(), RuntimeConfig::default())
                .map_err(|e| format!("DcpsRuntime::start: {e:?}"))?;
            let mut writers = HashMap::new();
            let mut readers = HashMap::new();
            for t in topics {
                if t.dds_name.is_empty() || writers.contains_key(&t.dds_name) {
                    continue;
                }
                let weid = rt
                    .register_user_writer(writer_cfg(&t.dds_name))
                    .map_err(|e| format!("register_user_writer({}): {e:?}", t.dds_name))?;
                writers.insert(t.dds_name.clone(), weid);
                let (reid, rx) = rt
                    .register_user_reader(reader_cfg(&t.dds_name))
                    .map_err(|e| format!("register_user_reader({}): {e:?}", t.dds_name))?;
                readers.insert(
                    t.dds_name.clone(),
                    ReaderSlot {
                        _eid: reid,
                        rx: Mutex::new(rx),
                    },
                );
            }
            Ok(Self {
                rt,
                writers,
                readers,
            })
        }

        /// Writes `payload` to the DDS DataWriter for `dds_name`. Returns
        /// `true` on success (writer present + write ok).
        pub fn publish_to(&self, dds_name: &str, payload: &[u8]) -> bool {
            match self.writers.get(dds_name) {
                Some(&eid) => self.rt.write_user_sample_borrowed(eid, payload).is_ok(),
                None => false,
            }
        }

        /// Takes the next available Alive sample from the DDS DataReader for
        /// `dds_name` (single `try_recv`, skipping lifecycle markers — matches
        /// the C-FFI `zerodds_reader_take`). Returns `None` when no reader or
        /// no data. Repeated `Subscribe` calls drain the queue one sample at a
        /// time (pull-based server-stream cardinality over the request/
        /// response transport).
        pub fn take_one(&self, dds_name: &str) -> Option<Vec<u8>> {
            let slot = self.readers.get(dds_name)?;
            let rx = slot.rx.lock().ok()?;
            loop {
                match rx.try_recv() {
                    Ok(UserSample::Alive { payload, .. }) => return Some(payload.to_vec()),
                    Ok(UserSample::Lifecycle { .. }) => continue,
                    Err(_) => return None,
                }
            }
        }
    }

    fn writer_cfg(topic: &str) -> UserWriterConfig {
        UserWriterConfig {
            topic_name: topic.to_string(),
            type_name: BRIDGE_TYPE_NAME.to_string(),
            reliable: true,
            durability: DurabilityKind::Volatile,
            deadline: DeadlineQosPolicy::default(),
            lifespan: LifespanQosPolicy::default(),
            liveliness: LivelinessQosPolicy {
                kind: LivelinessKind::Automatic,
                ..Default::default()
            },
            ownership: OwnershipKind::Shared,
            ownership_strength: 0,
            presentation: Default::default(),
            partition: Vec::new(),
            user_data: Vec::new(),
            topic_data: Vec::new(),
            group_data: Vec::new(),
            type_identifier: Default::default(),
            data_representation_offer: None,
        }
    }

    fn reader_cfg(topic: &str) -> UserReaderConfig {
        UserReaderConfig {
            topic_name: topic.to_string(),
            type_name: BRIDGE_TYPE_NAME.to_string(),
            reliable: true,
            durability: DurabilityKind::Volatile,
            deadline: DeadlineQosPolicy::default(),
            liveliness: LivelinessQosPolicy {
                kind: LivelinessKind::Automatic,
                ..Default::default()
            },
            ownership: OwnershipKind::Shared,
            presentation: Default::default(),
            partition: Vec::new(),
            user_data: Vec::new(),
            topic_data: Vec::new(),
            group_data: Vec::new(),
            type_identifier: Default::default(),
            type_consistency: Default::default(),
            data_representation_offer: None,
        }
    }

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn guid_prefix() -> GuidPrefix {
        let pid = std::process::id();
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let c = COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut bytes = [0u8; 12];
        bytes[0..4].copy_from_slice(&zerodds_dcps::participant::host_id_bytes());
        bytes[4..8].copy_from_slice(&pid.to_le_bytes());
        bytes[8..12].copy_from_slice(&(t as u32).wrapping_add(c).to_le_bytes());
        GuidPrefix::from_bytes(bytes)
    }
}

/// Bridge DDS handle. Under `dds-runtime` it is the real participant; without
/// the feature it is an uninhabited placeholder so the daemon still builds as
/// a pure gRPC codec (dispatch then runs with `dds: None`).
#[cfg(feature = "dds-runtime")]
use bridge_dds::BridgeDds;
#[cfg(not(feature = "dds-runtime"))]
enum BridgeDds {}
#[cfg(not(feature = "dds-runtime"))]
impl BridgeDds {
    fn publish_to(&self, _dds_name: &str, _payload: &[u8]) -> bool {
        match *self {}
    }
    fn take_one(&self, _dds_name: &str) -> Option<Vec<u8>> {
        match *self {}
    }
}

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
        // CLI format `<DDS-Name>=<gRPC-Service>` (spec §2 single-topic
        // override). We take the last `=` so that DDS names with
        // `::` scope (Chat::Message) stay intact.
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

    // OTLP exporter (§8.3).
    let _otlp_h = if let Some(otlp_cfg) = otlp_config_from_env(SERVICE_NAME) {
        let exp = Arc::new(OtlpExporter::new(otlp_cfg));
        spawn_otlp_flush_loop(exp, Arc::clone(&shutdown), Duration::from_secs(5)).ok()
    } else {
        None
    };

    // L2 — DDS side (bridge spec §4.2). With feature `dds-runtime` the
    // daemon starts a real DCPS participant and registers a writer+reader
    // per topic, so `Publish` writes to DDS and `Subscribe` drains it.
    // Without the feature the daemon runs as a pure gRPC codec (`dds: None`).
    #[cfg(feature = "dds-runtime")]
    let dds_handle = match BridgeDds::start(cfg.domain, &cfg.topics) {
        Ok(d) => {
            eprintln!(
                "{{\"event\":\"dds_started\",\"domain\":{},\"topics\":{}}}",
                cfg.domain,
                cfg.topics.len()
            );
            Some(d)
        }
        Err(e) => {
            eprintln!("{{\"event\":\"dds_start_failed\",\"err\":\"{e}\"}}");
            return Err(DaemonError::new(3, format!("dds: {e}")));
        }
    };
    #[cfg(feature = "dds-runtime")]
    let dds_ref = dds_handle.as_ref();
    #[cfg(not(feature = "dds-runtime"))]
    let dds_ref: Option<&BridgeDds> = None;

    let res = serve(
        &cfg,
        shutdown.clone(),
        once_shot,
        idle_timeout_ms,
        &bridge_metrics,
        dds_ref,
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
    dds: Option<&BridgeDds>,
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
                if let Err(e) = handle_connection(stream, cfg, metrics, dds) {
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
    dds: Option<&BridgeDds>,
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
            // Only decode once the full frame is buffered. A frame can be
            // split across TCP reads (large HPACK header blocks especially);
            // `process_frame` would otherwise return a fatal ShortPayload.
            let frame_len = ((acc[0] as usize) << 16) | ((acc[1] as usize) << 8) | acc[2] as usize;
            if acc.len() < 9 + frame_len {
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
                        let resp = dispatch(&req, cfg, dds);
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

fn dispatch(req: &GrpcRequest, cfg: &DaemonConfig, dds: Option<&BridgeDds>) -> GrpcResponse {
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

    // Topic-mapping lookup. Match on the full service name or the
    // last component (spec §5.1: `zerodds.chat.v1.ChatMessageStream`
    // is the fully-qualified service, `ChatMessageStream` the slug).
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
            "Publish" | "PublishOne" => {
                // `req.body` is the LPM-framed gRPC message; strip the 5-byte
                // prefix, then read `Sample.payload` (field 1) and write it to
                // the DDS DataWriter for this topic. PublishAck.accepted = 1
                // on a successful write, 0 when the DDS side is absent.
                let sample_msg = decode_message(&req.body)
                    .map(|(_, msg, _)| msg)
                    .unwrap_or_default();
                let payload = proto_sample_payload(&sample_msg);
                let accepted = match dds {
                    Some(d) if d.publish_to(&topic.dds_name, &payload) => 1u64,
                    _ => 0u64,
                };
                GrpcResponse {
                    stream_id: req.stream_id,
                    status: Status::Ok,
                    message: None,
                    body: proto_publish_ack(accepted),
                }
            }
            "Subscribe" => {
                // Drain the next available sample from the DDS DataReader and
                // return it as one `Sample` message (pull-based server-stream
                // cardinality, §4.2). Empty body = stream-end when no data.
                let body = match dds.and_then(|d| d.take_one(&topic.dds_name)) {
                    Some(payload) => proto_sample(&payload),
                    None => Vec::new(),
                };
                GrpcResponse {
                    stream_id: req.stream_id,
                    status: Status::Ok,
                    message: Some(format!("dds_topic={}", topic.dds_name)),
                    body,
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
    use zerodds_grpc_bridge::encode_message;

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
        let resp = dispatch(&req, &cfg, None);
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
        // A valid LPM-framed Sample{payload="hi"}.
        let body = encode_message(&proto_sample(b"hi"), false).expect("lpm");
        let req = GrpcRequest {
            stream_id: 3u32,
            path: "/zerodds.t.v1.TMStream/Publish".into(),
            service: "TMStream".into(),
            method: "Publish".into(),
            encoding: None,
            body,
        };
        // No DDS handle in the unit test → PublishAck.accepted = 0, status Ok.
        let resp = dispatch(&req, &cfg, None);
        assert_eq!(resp.status, Status::Ok);
        assert_eq!(resp.body, proto_publish_ack(0));
    }
}
