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
// L5: TLS with ALPN `h2` (§7.1), the auth modes of §7.2 checked per RPC
// on the `authorization` metadata or the client certificate, the topic ACL
// of §7.3; SIGHUP reloads all three (§9.2).

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

use zerodds_grpc_bridge::bridge_security::{
    AclEntry, AclOp, AuthSubject, SecurityConfig, SecurityCtx, authenticate_grpc, authorize,
    build_ctx, extract_mtls_subject,
};
use zerodds_grpc_bridge::daemon_runtime::build_grpc_tls_config_with;

use zerodds_grpc_bridge::daemon_runtime::{
    BridgeMetrics, CatalogSnapshot, CatalogTopic, SERVICE_NAME, install_signal_watcher,
    otlp_config_from_env, serve_admin_endpoints, spawn_otlp_flush_loop,
};
use zerodds_grpc_bridge::decode_message;
use zerodds_grpc_bridge::reflection::encode_list_services;
use zerodds_grpc_bridge::server::{GrpcRequest, GrpcResponse, GrpcServer};
use zerodds_grpc_bridge::service_gen::{DEFAULT_PACKAGE, ServiceCatalog, TopicService};
use zerodds_grpc_bridge::status::Status;
use zerodds_http2::settings::encode_settings;
use zerodds_http2::{Flags, FrameHeader, FrameType, check_preface, encode_frame};
use zerodds_monitor::Registry;
use zerodds_observability_otlp::OtlpExporter;

const VERSION: &str = env!("CARGO_PKG_VERSION");

// ============================================================================
// L2 — DDS side (bridge spec §4.2).
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

mod bridge_dds {
    //! The DDS side of the bridge daemon.

    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use zerodds_dcps::runtime::{
        DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
    };
    use zerodds_qos::{LivelinessKind, LivelinessQosPolicy};
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
        /// Starts the DDS participant for `domain` and registers, per DDS
        /// topic, a writer when a mapping carries it into DDS (`direction`
        /// `in`/`bidir`) and a reader when one carries it out (`out`/`bidir`).
        /// With `bidir` the bridge's own reader observes what `Publish` writes.
        pub fn start(
            domain: i32,
            topics: &[TopicMapping],
            runtime: RuntimeConfig,
        ) -> Result<Self, String> {
            let rt = DcpsRuntime::start(domain, guid_prefix(), runtime)
                .map_err(|e| format!("DcpsRuntime::start: {e:?}"))?;
            let mut writers = HashMap::new();
            let mut readers = HashMap::new();
            for t in topics {
                if t.dds_name.is_empty() {
                    continue;
                }
                if super::bridged_into_dds(t) && !writers.contains_key(&t.dds_name) {
                    let weid = rt
                        .register_user_writer(writer_cfg(&t.dds_name))
                        .map_err(|e| format!("register_user_writer({}): {e:?}", t.dds_name))?;
                    writers.insert(t.dds_name.clone(), weid);
                }
                if !super::bridged_out_of_dds(t) || readers.contains_key(&t.dds_name) {
                    continue;
                }
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
                    // DDSI-RTPS 2.5 §8.7.6: the ECS marker carries no user
                    // data. The bridge does not offer PRESENTATION
                    // `coherent_access`, so it is skipped like a lifecycle
                    // marker rather than handed to a gRPC client as a sample.
                    Ok(UserSample::CoherentSetEnd { .. }) => continue,
                    Err(_) => return None,
                }
            }
        }
    }

    fn writer_cfg(topic: &str) -> UserWriterConfig {
        UserWriterConfig {
            topic_name: topic.to_string(),
            type_name: BRIDGE_TYPE_NAME.to_string(),
            liveliness: LivelinessQosPolicy {
                kind: LivelinessKind::Automatic,
                ..Default::default()
            },
            type_identifier: Default::default(),
            ..Default::default()
        }
    }

    fn reader_cfg(topic: &str) -> UserReaderConfig {
        UserReaderConfig {
            topic_name: topic.to_string(),
            type_name: BRIDGE_TYPE_NAME.to_string(),
            liveliness: LivelinessQosPolicy {
                kind: LivelinessKind::Automatic,
                ..Default::default()
            },
            type_identifier: Default::default(),
            ..Default::default()
        }
    }

    fn guid_prefix() -> GuidPrefix {
        GuidPrefix::zerodds(
            zerodds_dcps::participant::host_id_bytes(),
            zerodds_dcps::participant::next_process_unique(),
        )
    }
}

use bridge_dds::BridgeDds;

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
    let args: Vec<String> = env::args().skip(1).collect();
    let (mut command_line, rest) = zerodds_config::CommandLine::extract(args)
        .map_err(|e| DaemonError::new(1, e.to_string()))?;
    let mut sets: Vec<String> = Vec::new();
    let mut tls_cert: Option<String> = None;
    let mut tls_key: Option<String> = None;
    let mut once_shot: bool = false;
    let mut idle_timeout_ms: u64 = 0;

    let mut i = 0;
    while i < rest.len() {
        let flag = rest[i].as_str();
        let mut value = || -> Result<String, DaemonError> {
            i += 1;
            rest.get(i)
                .cloned()
                .ok_or_else(|| DaemonError::new(1, format!("{flag} needs a value")))
        };
        // Spec §2: each option is `--set` of its file key, after any
        // explicit `--set`, so it wins over the files and names itself as
        // the origin — `--domain 0` over a file's `domain: 7` included.
        match flag {
            "--bind" => sets.push(format!("grpc.bind={}", value()?)),
            "--domain" => sets.push(format!("domain={}", value()?)),
            "--tls-cert" => tls_cert = Some(value()?),
            "--tls-key" => tls_key = Some(value()?),
            "--tls-client-ca" => sets.push(format!("grpc.tls.client_ca_file={}", value()?)),
            "--reflection" => sets.push("grpc.reflection.enabled=true".to_string()),
            "--topic" => sets.push(format!("+topics={}", value()?)),
            "--log-level" => sets.push(format!("log_level={}", value()?)),
            "--metrics" => {
                sets.push(format!("metrics.listen={}", value()?));
                sets.push("metrics.enabled=true".to_string());
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
                let v = value()?;
                idle_timeout_ms = v.parse().map_err(|_| {
                    DaemonError::new(1, format!("--idle-timeout-ms {v}: expected milliseconds"))
                })?;
            }
            other => {
                return Err(DaemonError::new(1, format!("unknown argument: {other}")));
            }
        }
        i += 1;
    }
    match (tls_cert, tls_key) {
        (Some(c), Some(k)) => {
            sets.push(format!("grpc.tls.cert_file={c}"));
            sets.push(format!("grpc.tls.key_file={k}"));
            sets.push("grpc.tls.enabled=true".to_string());
        }
        (None, None) => {}
        (Some(_), None) => return Err(DaemonError::new(1, "--tls-cert needs --tls-key")),
        (None, Some(_)) => return Err(DaemonError::new(1, "--tls-key needs --tls-cert")),
    }
    for assignment in &sets {
        command_line
            .push_set(assignment)
            .map_err(|e| DaemonError::new(1, e.to_string()))?;
    }
    // Kept for SIGHUP: the reload reads the same layered sources again.
    let reload_command_line = command_line.clone();
    // A file that does not load (unreadable, syntax, include) is exit 2
    // under `--check-config` (deployment §2.1); at start it is a
    // configuration error, exit 1 — 2 is the bind error of spec §2.
    let checking = command_line.check();
    let configured = zerodds_dcps::cli::Configured::from_command_line(
        command_line,
        Vec::new(),
        CONFIG_STEM,
        is_key,
        &[],
    )
    .map_err(|e| DaemonError::new(if checking { 2 } else { 1 }, format!("config: {e}")))?;
    let cfg = match DaemonConfig::from_config(configured.source.tree()) {
        Ok(cfg) => cfg,
        Err(e) if configured.checking() => {
            println!("error: {e}\nconfiguration invalid");
            return Err(DaemonError::new(1, "configuration invalid"));
        }
        Err(e) => return Err(DaemonError::new(1, format!("config: {e}"))),
    };
    if let Some(valid) = configured.check_config_valid() {
        return if valid {
            Ok(())
        } else {
            Err(DaemonError::new(1, "configuration invalid"))
        };
    }
    let runtime_config = configured.runtime;
    // §7: a setting that asks for TLS or authentication and cannot be
    // served stops the start. It used to be read and then ignored, so a
    // daemon configured for `auth.mode: jwt` answered every call unchecked.
    let live = Arc::new(zerodds_bridge_security::LiveSecurity::new(
        security_from(&cfg).map_err(|(code, msg)| DaemonError::new(code, msg))?,
    ));

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
    let bridge_metrics = DaemonMetrics {
        base: BridgeMetrics::register(&registry),
        spec: SpecMetrics::register(&registry, &cfg),
    };

    // Signal watcher (§9.2 graceful shutdown).
    if let Err(e) = install_signal_watcher(Arc::clone(&shutdown), Arc::clone(&reload)) {
        eprintln!("{{\"event\":\"signal_watcher_init_failed\",\"err\":\"{e}\"}}");
    }
    // §9.2 SIGHUP: TLS material, auth and ACL from the files as they are now.
    {
        let (flag, stop, live, started) = (
            Arc::clone(&reload),
            Arc::clone(&shutdown),
            Arc::clone(&live),
            cfg.clone(),
        );
        std::thread::spawn(move || {
            let load = || -> Result<DaemonConfig, String> {
                let configured = zerodds_dcps::cli::Configured::from_command_line(
                    reload_command_line.clone(),
                    Vec::new(),
                    CONFIG_STEM,
                    is_key,
                    &[],
                )
                .map_err(|e| e.to_string())?;
                DaemonConfig::from_config(configured.source.tree()).map_err(|e| e.to_string())
            };
            while !stop.load(Ordering::SeqCst) {
                if flag.swap(false, Ordering::SeqCst) {
                    apply_reload(&live, &started, load());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        });
    }

    // Admin endpoint (§5.2 catalog/healthz + §8.2 metrics).
    let healthy = Arc::new(AtomicBool::new(true));
    let _admin_h = if let Some(addr_s) = metrics_addr_for(&cfg) {
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

    // L2 — DDS side (bridge spec §4.2): a DCPS participant with a writer
    // per topic bridged into DDS and a reader per topic bridged out of it
    // (`direction`), so `Publish` writes to DDS and `Subscribe` drains it.
    let dds_handle = match BridgeDds::start(cfg.domain, &cfg.topics, runtime_config) {
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
    let dds_ref = dds_handle.as_ref();

    let res = serve(
        &cfg,
        shutdown.clone(),
        once_shot,
        idle_timeout_ms,
        &bridge_metrics,
        dds_ref,
        &live,
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
    eprintln!("  --config <FILE>          base file (default: grpc-bridged.yaml in the");
    eprintln!("                           system configuration directory) with FILE.d/");
    eprintln!("  --set <PATH=VALUE>       one setting above every file, repeatable");
    eprintln!("  --check-config           validate, print every value with its origin, exit");
    eprintln!("  --bind <ADDR>            HTTP/2 bind addr (default 0.0.0.0:50051)");
    eprintln!("  --domain <ID>            DDS Domain ID (default 0)");
    eprintln!("  --tls-cert <FILE>        TLS cert PEM");
    eprintln!("  --tls-key <FILE>         TLS key PEM");
    eprintln!("  --tls-client-ca <FILE>   Client CA PEM for mTLS");
    eprintln!("  --reflection             Enable gRPC reflection service");
    eprintln!("  --topic <DDS:SVC>        Inline topic mapping override");
    eprintln!("  --log-level <LEVEL>      trace|debug|info|warn|error");
    eprintln!("  --metrics <ADDR>         Prometheus listen addr");
    eprintln!();
    eprintln!("  Test hooks. Real options, listed because the program accepts");
    eprintln!("  them and an option nobody can find is one nobody maintains:");
    eprintln!("  --once                   Serve one request, then exit");
    eprintln!("  --idle-timeout-ms <N>    Exit after N ms without a request");
    eprintln!();
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
    metrics: &DaemonMetrics,
    dds: Option<&BridgeDds>,
    live: &zerodds_bridge_security::LiveSecurity,
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
    zerodds_dcps::cli::notify_service_manager("READY=1");

    listener
        .set_nonblocking(true)
        .map_err(|e| DaemonError::new(2, format!("set_nonblocking: {e}")))?;

    // One thread per connection: a client that keeps its connection open
    // (every gRPC client does) held the accept loop until its 5 s read
    // timeout, and each further client waited behind it.
    let mut accepted = 0usize;
    std::thread::scope(|scope| {
        while !shutdown.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, peer)) => {
                    eprintln!("{{\"event\":\"accept\",\"peer\":\"{peer}\"}}");
                    let _ = stream.set_nonblocking(false);
                    let _ = stream.set_nodelay(true);
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                    metrics.base.connections_total.inc();
                    metrics.base.connections_active.inc();
                    let env = ConnEnv {
                        cfg,
                        metrics,
                        dds,
                        live,
                        shutdown: &shutdown,
                    };
                    let conn = scope.spawn(move || {
                        // The TLS configuration at accept time: a SIGHUP
                        // applies to the next connection, an open one keeps
                        // its session.
                        let served = match stream.try_clone() {
                            Err(e) => Err(e),
                            Ok(ctl) => match live.current().tls.clone() {
                                Some(tls) => serve_tls(stream, &ctl, tls, &env),
                                None => handle_connection(stream, &ctl, &env, None),
                            },
                        };
                        if let Err(e) = served {
                            metrics.base.errors_total.inc();
                            eprintln!("{{\"event\":\"conn_error\",\"msg\":\"{e}\"}}");
                        }
                        metrics.base.connections_active.dec();
                    });
                    accepted += 1;
                    if once {
                        let _ = conn.join();
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
        // The accept loop has seen the stop; the connections now send
        // GOAWAY and end.
        zerodds_dcps::cli::notify_service_manager("STOPPING=1");
    });
    eprintln!("{{\"event\":\"shutdown\",\"connections\":{accepted}}}");
    Ok(())
}

/// §7.1: the TLS handshake (ALPN `h2`), then the HTTP/2 connection over
/// it with the client certificate's subject for `auth.mode: mtls`.
fn serve_tls(
    stream: TcpStream,
    ctl: &TcpStream,
    tls: Arc<rustls::ServerConfig>,
    env: &ConnEnv<'_>,
) -> std::io::Result<()> {
    let conn = rustls::ServerConnection::new(tls).map_err(std::io::Error::other)?;
    let mut tls = rustls::StreamOwned::new(conn, stream);
    while tls.conn.is_handshaking() {
        tls.conn
            .complete_io(&mut tls.sock)
            .map_err(|e| std::io::Error::new(e.kind(), format!("tls handshake: {e}")))?;
    }
    let subject = extract_mtls_subject(&tls.conn);
    handle_connection(tls, ctl, env, subject)
}

/// What a connection thread shares with the daemon.
struct ConnEnv<'a> {
    cfg: &'a DaemonConfig,
    metrics: &'a DaemonMetrics,
    dds: Option<&'a BridgeDds>,
    live: &'a zerodds_bridge_security::LiveSecurity,
    shutdown: &'a AtomicBool,
}

/// How often a connection looks at the shutdown flag while it waits for
/// the client.
const CONN_POLL: Duration = Duration::from_millis(200);
/// A connection without a frame for this long is closed (with GOAWAY).
const CONN_IDLE: Duration = Duration::from_secs(5);

/// RFC 7540 §6.8: GOAWAY with the last stream the server processed and
/// NO_ERROR.
fn send_goaway<S: Write>(stream: &mut S, last_stream_id: u32) -> std::io::Result<()> {
    let mut payload = [0u8; 8];
    payload[..4].copy_from_slice(&(last_stream_id & 0x7fff_ffff).to_be_bytes());
    let h = FrameHeader {
        length: 8,
        frame_type: FrameType::GoAway,
        flags: Flags(0),
        stream_id: 0,
    };
    let mut buf = [0u8; 17];
    encode_frame(&h, &payload, &mut buf, 16384)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "encode goaway"))?;
    stream.write_all(&buf)?;
    stream.flush()
}

fn handle_connection<S: Read + Write>(
    mut stream: S,
    ctl: &TcpStream,
    env: &ConnEnv<'_>,
    mtls_subject: Option<AuthSubject>,
) -> std::io::Result<()> {
    let (cfg, metrics, dds, live) = (env.cfg, env.metrics, env.dds, env.live);
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

    // §9.2: after the preface the connection polls, so that SIGTERM
    // reaches it; a call being served finishes first, since it is served
    // on this thread. The idle close keeps the former 5 s read timeout and
    // now says so with GOAWAY too.
    ctl.set_read_timeout(Some(CONN_POLL))?;
    let mut idle = Duration::ZERO;
    let mut last_stream_id = 0u32;
    let mut _responded = 0u32;
    loop {
        if env.shutdown.load(Ordering::SeqCst) {
            eprintln!("{{\"event\":\"goaway\",\"last_stream_id\":{last_stream_id}}}");
            send_goaway(&mut stream, last_stream_id)?;
            break;
        }
        let n = match stream.read(&mut read_buf) {
            Ok(0) => break,
            Ok(n) => {
                idle = Duration::ZERO;
                n
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                idle += CONN_POLL;
                if idle >= CONN_IDLE {
                    send_goaway(&mut stream, last_stream_id)?;
                    break;
                }
                continue;
            }
            Err(e) => return Err(e),
        };
        acc.extend_from_slice(&read_buf[..n]);
        metrics.base.bytes_in_total.add(n as u64);

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
            // §8.2 `h2_resets_total`: a RST_STREAM from the client
            // (RFC 7540 §6.4, a 4-octet error code).
            if acc[3] == FrameType::RstStream as u8 && frame_len == 4 {
                metrics
                    .spec
                    .h2_reset(u32::from_be_bytes([acc[9], acc[10], acc[11], acc[12]]));
            }
            match grpc.process_frame(&acc) {
                Ok((maybe_req, consumed)) => {
                    if consumed == 0 {
                        break;
                    }
                    acc.drain(..consumed);
                    // RFC 7540 §6.5.3 / §6.7 — the SETTINGS and PING
                    // acknowledgements the frame just read made due.
                    // Without them a Go gRPC client (`grpcurl`, and every
                    // client built on `google.golang.org/grpc`) closes the
                    // connection before it sends a request.
                    let control = grpc.take_control_frames();
                    if !control.is_empty() {
                        metrics.base.bytes_out_total.add(control.len() as u64);
                        stream.write_all(&control)?;
                    }
                    if let Some(req) = maybe_req {
                        last_stream_id = last_stream_id.max(u32::from(req.stream_id));
                        metrics.base.frames_in_total.inc();
                        if matches!(req.method.as_str(), "Publish" | "PublishOne") {
                            metrics.base.dds_samples_in_total.inc();
                        }
                        // §7.2 per call, against the settings of now.
                        let ctx = live.current();
                        let stream_gauge = metrics.spec.stream_gauge(cfg, &req);
                        if let Some(g) = &stream_gauge {
                            g.inc();
                        }
                        let resp = match authenticate_grpc(
                            &ctx.auth,
                            &req.metadata,
                            mtls_subject.clone(),
                        ) {
                            Ok(subject) => dispatch(&req, cfg, dds, &ctx.acl, &subject),
                            Err(e) => {
                                metrics.spec.auth_failure(&e);
                                eprintln!(
                                    "{{\"event\":\"auth_failed\",\"service\":\"{}\",\"err\":{:?}}}",
                                    req.service,
                                    e.to_string()
                                );
                                GrpcResponse {
                                    stream_id: req.stream_id,
                                    status: Status::Unauthenticated,
                                    message: Some(e.to_string()),
                                    body: Vec::new(),
                                }
                            }
                        };
                        metrics.spec.count_call(cfg, &req, &resp);
                        let written = grpc.encode_response(&resp);
                        if let Some(g) = &stream_gauge {
                            g.dec();
                        }
                        match written {
                            Ok(out) => {
                                metrics.base.bytes_out_total.add(out.len() as u64);
                                stream.write_all(&out)?;
                                metrics.base.frames_out_total.inc();
                                _responded += 1;
                            }
                            Err(e) => {
                                metrics.base.errors_total.inc();
                                eprintln!("{{\"event\":\"encode_error\",\"msg\":\"{e}\"}}");
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    metrics.base.errors_total.inc();
                    eprintln!("{{\"event\":\"decode_error\",\"msg\":\"{e}\"}}");
                    return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e));
                }
            }
        }
    }

    Ok(())
}

/// `direction` `in`/`bidir`: `Publish` carries gRPC messages into DDS.
fn bridged_into_dds(t: &TopicMapping) -> bool {
    matches!(t.direction.as_str(), "in" | "bidir")
}

/// `direction` `out`/`bidir`: `Subscribe` carries DDS samples to gRPC.
fn bridged_out_of_dds(t: &TopicMapping) -> bool {
    matches!(t.direction.as_str(), "out" | "bidir")
}

fn failed(req: &GrpcRequest, status: Status, message: String) -> GrpcResponse {
    GrpcResponse {
        stream_id: req.stream_id,
        status,
        message: Some(message),
        body: Vec::new(),
    }
}

fn dispatch(
    req: &GrpcRequest,
    cfg: &DaemonConfig,
    dds: Option<&BridgeDds>,
    acl: &zerodds_grpc_bridge::bridge_security::Acl,
    subject: &AuthSubject,
) -> GrpcResponse {
    eprintln!(
        "{{\"event\":\"rpc\",\"service\":\"{}\",\"method\":\"{}\",\"stream\":{}}}",
        req.service,
        req.method,
        u32::from(req.stream_id),
    );

    // Reflection service (Spec §4.6).
    // Both names of the reflection service — a client asks for `v1`
    // first and only falls back to `v1alpha` on UNIMPLEMENTED.
    if zerodds_grpc_bridge::reflection::is_reflection_service(&req.service) {
        if cfg.reflection_enabled {
            // `encode_list_services` — the protobuf `ServerReflectionResponse`
            // the reflection module has always been able to write and that
            // nothing but its own unit test ever called. What went on the
            // wire here was `build_reflection_catalog`, a JSON string, and a
            // gRPC client answers that with "failed to unmarshal the received
            // message: proto: cannot parse invalid wire-format data" —
            // measured against grpcurl 1.9.3. JSON is what `/catalog` on the
            // admin port serves, and it stays there.
            return GrpcResponse {
                stream_id: req.stream_id,
                status: Status::Ok,
                message: None,
                body: encode_list_services(&reflection_catalog(cfg)),
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
    if let Some(topic) = topic_for(cfg, &req.service) {
        // §7.3: `Publish` writes the topic, `Subscribe` reads it.
        let op = match req.method.as_str() {
            "Publish" | "PublishOne" => Some(AclOp::Write),
            "Subscribe" => Some(AclOp::Read),
            _ => None,
        };
        if let Some(op) = op {
            if !authorize(acl, subject, op, &topic.dds_name) {
                return GrpcResponse {
                    stream_id: req.stream_id,
                    status: Status::PermissionDenied,
                    message: Some(format!(
                        "{} may not {} {}",
                        subject.name,
                        if op == AclOp::Write {
                            "publish"
                        } else {
                            "subscribe"
                        },
                        topic.dds_name
                    )),
                    body: Vec::new(),
                };
            }
        }
        match req.method.as_str() {
            "Publish" | "PublishOne" => {
                // `req.body` is the LPM-framed gRPC message; strip the 5-byte
                // prefix, then read `Sample.payload` (field 1) and write it to
                // the DDS DataWriter for this topic. PublishAck.accepted = 1
                // on a successful write; a topic not bridged into DDS, a
                // missing DDS side or a failed write is a status, not 0.
                if !bridged_into_dds(topic) {
                    return failed(
                        req,
                        Status::FailedPrecondition,
                        format!(
                            "{} is not bridged into DDS (direction {})",
                            topic.dds_name, topic.direction
                        ),
                    );
                }
                let Some(d) = dds else {
                    return failed(req, Status::Unavailable, "no DDS participant".into());
                };
                let sample_msg = decode_message(&req.body)
                    .map(|(_, msg, _)| msg)
                    .unwrap_or_default();
                let payload = proto_sample_payload(&sample_msg);
                if !d.publish_to(&topic.dds_name, &payload) {
                    return failed(
                        req,
                        Status::Unavailable,
                        format!("DDS write to {} failed", topic.dds_name),
                    );
                }
                GrpcResponse {
                    stream_id: req.stream_id,
                    status: Status::Ok,
                    message: None,
                    body: proto_publish_ack(1),
                }
            }
            "Subscribe" => {
                // Drain the next available sample from the DDS DataReader and
                // return it as one `Sample` message (pull-based server-stream
                // cardinality, §4.2). Empty body = stream-end when no data.
                if !bridged_out_of_dds(topic) {
                    return failed(
                        req,
                        Status::FailedPrecondition,
                        format!(
                            "{} is not bridged out of DDS (direction {})",
                            topic.dds_name, topic.direction
                        ),
                    );
                }
                let Some(d) = dds else {
                    return failed(req, Status::Unavailable, "no DDS participant".into());
                };
                let body = match d.take_one(&topic.dds_name) {
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

/// The mapping a service name addresses: the full name or its last
/// component (spec §5.1: `zerodds.chat.v1.ChatMessageStream` is the
/// fully-qualified service, `ChatMessageStream` the slug).
fn topic_for<'a>(cfg: &'a DaemonConfig, service: &str) -> Option<&'a TopicMapping> {
    cfg.topics.iter().find(|t| {
        t.grpc_service == service || service.rsplit('.').next() == Some(t.grpc_service.as_str())
    })
}

/// The bin's metric sets: the generic `zerodds_grpc_*` counters and the
/// `zerodds_grpc_bridge_*` families of spec §8.2.
struct DaemonMetrics {
    base: BridgeMetrics,
    spec: SpecMetrics,
}

/// Per-topic counters of §8.2, labelled `dds_topic`.
struct TopicMetrics {
    messages_in: Arc<zerodds_monitor::Counter>,
    messages_out: Arc<zerodds_monitor::Counter>,
    bytes_in: Arc<zerodds_monitor::Counter>,
    bytes_out: Arc<zerodds_monitor::Counter>,
    samples_received: Arc<zerodds_monitor::Counter>,
    samples_published: Arc<zerodds_monitor::Counter>,
}

/// Spec §8.2. Label values come from the configuration or a fixed set, so
/// a client naming arbitrary services cannot grow the registry: a call to
/// a service that is not mapped counts as `service="unknown"`, a method
/// outside the bridge's own as `method="other"`.
struct SpecMetrics {
    registry: Arc<Registry>,
    topics: std::collections::HashMap<String, TopicMetrics>,
    streams: std::collections::HashMap<String, Arc<zerodds_monitor::Gauge>>,
}

const KNOWN_METHODS: &[&str] = &[
    "Publish",
    "PublishOne",
    "Subscribe",
    "Catalog",
    "ServerReflectionInfo",
];

impl SpecMetrics {
    fn register(registry: &Arc<Registry>, cfg: &DaemonConfig) -> Self {
        use zerodds_monitor::Labels;
        for (name, help) in [
            (
                "zerodds_grpc_bridge_rpcs_total",
                "gRPC calls answered, by status",
            ),
            ("zerodds_grpc_bridge_streams_active", "Calls being served"),
            (
                "zerodds_grpc_bridge_messages_in_total",
                "gRPC messages published to a topic",
            ),
            (
                "zerodds_grpc_bridge_messages_out_total",
                "gRPC messages sent from a topic",
            ),
            (
                "zerodds_grpc_bridge_bytes_in_total",
                "Bytes of the Length-Prefixed-Messages published",
            ),
            (
                "zerodds_grpc_bridge_bytes_out_total",
                "Bytes of the Length-Prefixed-Messages sent",
            ),
            (
                "zerodds_grpc_bridge_dds_samples_received",
                "DDS samples taken for Subscribe",
            ),
            (
                "zerodds_grpc_bridge_dds_samples_published",
                "DDS samples written by Publish",
            ),
            (
                "zerodds_grpc_bridge_auth_failures_total",
                "Calls refused by the auth check",
            ),
            (
                "zerodds_grpc_bridge_h2_resets_total",
                "RST_STREAM frames received",
            ),
        ] {
            registry.set_help(name, help);
        }
        let mut topics = std::collections::HashMap::new();
        let mut streams = std::collections::HashMap::new();
        for t in &cfg.topics {
            let l = || Labels::new().with("dds_topic", t.dds_name.clone());
            topics.insert(
                t.dds_name.clone(),
                TopicMetrics {
                    messages_in: registry.counter("zerodds_grpc_bridge_messages_in_total", l()),
                    messages_out: registry.counter("zerodds_grpc_bridge_messages_out_total", l()),
                    bytes_in: registry.counter("zerodds_grpc_bridge_bytes_in_total", l()),
                    bytes_out: registry.counter("zerodds_grpc_bridge_bytes_out_total", l()),
                    samples_received: registry
                        .counter("zerodds_grpc_bridge_dds_samples_received", l()),
                    samples_published: registry
                        .counter("zerodds_grpc_bridge_dds_samples_published", l()),
                },
            );
            streams.insert(
                t.grpc_service.clone(),
                registry.gauge(
                    "zerodds_grpc_bridge_streams_active",
                    Labels::new().with("service", t.grpc_service.clone()),
                ),
            );
        }
        Self {
            registry: Arc::clone(registry),
            topics,
            streams,
        }
    }

    /// The `service` label of a call.
    fn service_label(cfg: &DaemonConfig, req: &GrpcRequest) -> String {
        if let Some(t) = topic_for(cfg, &req.service) {
            t.grpc_service.clone()
        } else if zerodds_grpc_bridge::reflection::is_reflection_service(&req.service) {
            req.service.clone()
        } else {
            "unknown".into()
        }
    }

    fn stream_gauge(
        &self,
        cfg: &DaemonConfig,
        req: &GrpcRequest,
    ) -> Option<Arc<zerodds_monitor::Gauge>> {
        let t = topic_for(cfg, &req.service)?;
        self.streams.get(&t.grpc_service).cloned()
    }

    /// `rpcs_total` and, for a mapped topic, the message, byte and sample
    /// counters of the call.
    fn count_call(&self, cfg: &DaemonConfig, req: &GrpcRequest, resp: &GrpcResponse) {
        use zerodds_monitor::Labels;
        let method = if KNOWN_METHODS.contains(&req.method.as_str()) {
            req.method.clone()
        } else {
            "other".into()
        };
        self.registry
            .counter(
                "zerodds_grpc_bridge_rpcs_total",
                Labels::new()
                    .with("service", Self::service_label(cfg, req))
                    .with("method", method)
                    .with("status", resp.status.name()),
            )
            .inc();
        if resp.status != Status::Ok {
            return;
        }
        let Some(m) = topic_for(cfg, &req.service).and_then(|t| self.topics.get(&t.dds_name))
        else {
            return;
        };
        match req.method.as_str() {
            "Publish" | "PublishOne" => {
                m.messages_in.inc();
                m.bytes_in.add(req.body.len() as u64);
                if resp.body == proto_publish_ack(1) {
                    m.samples_published.inc();
                }
            }
            "Subscribe" if !resp.body.is_empty() => {
                m.samples_received.inc();
                m.messages_out.inc();
                // The reply's Length-Prefixed-Message, prefix included,
                // as `bytes_in` counts the request's.
                m.bytes_out.add(resp.body.len() as u64 + 5);
            }
            _ => {}
        }
    }

    fn auth_failure(&self, e: &zerodds_grpc_bridge::bridge_security::AuthError) {
        use zerodds_grpc_bridge::bridge_security::AuthError;
        let reason = match e {
            AuthError::MissingCredentials => "missing",
            AuthError::MalformedCredentials(_) => "malformed",
            AuthError::Rejected(_) => "rejected",
            AuthError::Misconfigured(_) => "misconfigured",
        };
        self.registry
            .counter(
                "zerodds_grpc_bridge_auth_failures_total",
                zerodds_monitor::Labels::new().with("reason", reason),
            )
            .inc();
    }

    /// RFC 7540 §7 error code names; others count as `unknown`.
    fn h2_reset(&self, code: u32) {
        const NAMES: [&str; 14] = [
            "NO_ERROR",
            "PROTOCOL_ERROR",
            "INTERNAL_ERROR",
            "FLOW_CONTROL_ERROR",
            "SETTINGS_TIMEOUT",
            "STREAM_CLOSED",
            "FRAME_SIZE_ERROR",
            "REFUSED_STREAM",
            "CANCEL",
            "COMPRESSION_ERROR",
            "CONNECT_ERROR",
            "ENHANCE_YOUR_CALM",
            "INADEQUATE_SECURITY",
            "HTTP_1_1_REQUIRED",
        ];
        let name = usize::try_from(code)
            .ok()
            .and_then(|i| NAMES.get(i))
            .copied()
            .unwrap_or("unknown");
        self.registry
            .counter(
                "zerodds_grpc_bridge_h2_resets_total",
                zerodds_monitor::Labels::new().with("error_code", name),
            )
            .inc();
    }
}

/// The service catalog the reflection service reports.
///
/// The `--topic`/YAML mapping names the gRPC service outright, so the
/// entry is built from that name rather than derived from the DDS topic.
/// The reflection service itself is listed alongside them: a client that
/// asks `ListServices` expects every service the server dispatches,
/// which is what grpc-go's own reflection implementation returns.
fn reflection_catalog(cfg: &DaemonConfig) -> ServiceCatalog {
    let mut catalog = ServiceCatalog::new();
    for t in &cfg.topics {
        catalog.register(TopicService {
            package: DEFAULT_PACKAGE.into(),
            service: t.grpc_service.clone(),
            dds_topic: t.dds_name.clone(),
            dds_type: t.dds_name.clone(),
        });
    }
    catalog.register(TopicService {
        package: "grpc.reflection.v1".into(),
        service: "ServerReflection".into(),
        dds_topic: String::new(),
        dds_type: String::new(),
    });
    catalog
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
    /// `grpc.tls.client_ca_file` — CA for client certificates (mTLS).
    pub tls_client_ca_file: String,
    /// `grpc.tls.require_client_cert` — with a client CA, whether a
    /// client without a certificate fails the handshake.
    pub tls_require_client_cert: bool,
    pub reflection_enabled: bool,
    pub auth_mode: String,
    /// `auth.bearer_token` / `auth.bearer_subject` — `auth.mode: bearer`.
    pub auth_bearer_token: Option<String>,
    pub auth_bearer_subject: Option<String>,
    /// `auth.jwt.*` — `auth.mode: jwt`.
    pub auth_jwt: zerodds_bridge_security::JwtSettings,
    /// `log_level:` — Spec §3.
    pub log_level: String,
    /// `metrics.enabled:` — Spec §3/§8.2.
    pub metrics_enabled: bool,
    /// `metrics.listen:` — Spec §3/§8.2. `--metrics` overrides it.
    pub metrics_listen: String,
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
            tls_client_ca_file: String::new(),
            tls_require_client_cert: false,
            reflection_enabled: false,
            auth_mode: "none".into(),
            auth_bearer_token: None,
            auth_bearer_subject: None,
            auth_jwt: zerodds_bridge_security::JwtSettings::default(),
            log_level: "info".into(),
            metrics_enabled: false,
            metrics_listen: String::new(),
            topics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TopicMapping {
    pub dds_name: String,
    pub grpc_service: String,
    pub direction: String,
    /// `acl: { publish: [...], subscribe: [...] }` — Spec §7.3.
    pub acl: Option<(Vec<String>, Vec<String>)>,
}

/// The base name of the daemon's file: `grpc-bridged.yaml`, `grpc-bridged.d/`.
const CONFIG_STEM: &str = "grpc-bridged";

/// Every key this daemon reads (Spec §3); `topics` stops at the list.
const KEYS: &[&str] = &[
    "domain",
    "log_level",
    "grpc.bind",
    "grpc.tls.enabled",
    "grpc.tls.cert_file",
    "grpc.tls.key_file",
    "grpc.tls.client_ca_file",
    "grpc.tls.require_client_cert",
    "grpc.reflection.enabled",
    "auth.mode",
    "auth.bearer_token",
    "auth.bearer_subject",
    "auth.jwt.public_key",
    "auth.jwt.issuer",
    "auth.jwt.audience",
    "auth.jwt.required_claim",
    "metrics.enabled",
    "metrics.listen",
    "topics",
];

fn is_key(path: &str) -> bool {
    KEYS.contains(&path)
}

impl DaemonConfig {
    /// Reads this daemon's keys out of a merged tree, over the defaults.
    fn from_config(
        config: Option<&zerodds_config::Config>,
    ) -> Result<Self, zerodds_config::ConfigError> {
        use zerodds_config::read::{Section, first_origin, split_topic_shorthand};
        let root = Section::root(config.and_then(zerodds_config::Config::root))?;
        let mut cfg = Self::default();
        if let Some(v) = root.parse::<i32>("domain", "a DDS domain id")? {
            cfg.domain = v;
        }
        if let Some(v) = root.choice("log_level", &["trace", "debug", "info", "warn", "error"])? {
            cfg.log_level = v;
        }
        let grpc = root.section("grpc")?;
        grpc.only(&["bind", "tls", "reflection"])?;
        if let Some(v) = grpc.string("bind")? {
            cfg.bind = v;
        }
        let tls = grpc.section("tls")?;
        tls.only(&[
            "enabled",
            "cert_file",
            "key_file",
            "client_ca_file",
            "require_client_cert",
        ])?;
        if let Some(v) = tls.boolean("enabled")? {
            cfg.tls_enabled = v;
        }
        if let Some(v) = tls.string("cert_file")? {
            cfg.tls_cert_file = v;
        }
        if let Some(v) = tls.string("key_file")? {
            cfg.tls_key_file = v;
        }
        if let Some(v) = tls.string("client_ca_file")? {
            cfg.tls_client_ca_file = v;
        }
        if let Some(v) = tls.boolean("require_client_cert")? {
            cfg.tls_require_client_cert = v;
        }
        let reflection = grpc.section("reflection")?;
        reflection.only(&["enabled"])?;
        if let Some(v) = reflection.boolean("enabled")? {
            cfg.reflection_enabled = v;
        }
        let auth = root.section("auth")?;
        auth.only(&["mode", "bearer_token", "bearer_subject", "jwt"])?;
        if let Some(v) = auth.choice("mode", &["none", "bearer", "jwt", "mtls"])? {
            cfg.auth_mode = v;
        }
        cfg.auth_bearer_token = auth.string("bearer_token")?;
        cfg.auth_bearer_subject = auth.string("bearer_subject")?;
        let jwt = auth.section("jwt")?;
        jwt.only(&["public_key", "issuer", "audience", "required_claim"])?;
        cfg.auth_jwt = zerodds_bridge_security::JwtSettings {
            public_key: jwt.string("public_key")?.unwrap_or_default(),
            issuer: jwt.string("issuer")?,
            audience: jwt.string("audience")?,
            required_claim: jwt.string("required_claim")?,
        };
        let metrics = root.section("metrics")?;
        metrics.only(&["enabled", "listen"])?;
        if let Some(v) = metrics.boolean("enabled")? {
            cfg.metrics_enabled = v;
        }
        if let Some(v) = metrics.string("listen")? {
            cfg.metrics_listen = v;
        }
        for (path, item) in root.items("topics")? {
            let mut t = TopicMapping {
                dds_name: String::new(),
                grpc_service: String::new(),
                direction: "bidir".into(),
                acl: None,
            };
            if let zerodds_config::Node::Scalar(text, _) = item {
                // `--topic DDS=SERVICE` (Spec §2), or `DDS:SERVICE` split at
                // the lone colon so that `Chat::Message` stays whole.
                let (dds, service) = match text.rsplit_once('=') {
                    Some((d, s)) => (d.trim().to_string(), Some(s.trim().to_string())),
                    None => split_topic_shorthand(text),
                };
                t.dds_name = dds;
                t.grpc_service = service.unwrap_or_default();
            } else {
                let entry = Section::of(Some(item), path.clone())?;
                entry.only(&["dds_name", "grpc_service", "direction", "acl"])?;
                t.dds_name = entry.string("dds_name")?.unwrap_or_default();
                t.grpc_service = entry.string("grpc_service")?.unwrap_or_default();
                if let Some(v) = entry.choice("direction", &["in", "out", "bidir"])? {
                    t.direction = v;
                }
                let acl = entry.section("acl")?;
                acl.only(&["publish", "subscribe"])?;
                if entry.node("acl").is_some() {
                    t.acl = Some((
                        acl.strings("publish")?.unwrap_or_default(),
                        acl.strings("subscribe")?.unwrap_or_default(),
                    ));
                }
            }
            if t.dds_name.is_empty() || t.grpc_service.is_empty() {
                return Err(zerodds_config::ConfigError::Invalid {
                    setting: path,
                    origin: first_origin(item),
                    message: "a topic entry needs dds_name and grpc_service".to_string(),
                });
            }
            cfg.topics.push(t);
        }
        Ok(cfg)
    }
}

/// The §7 settings as the daemon serves them: TLS with ALPN `h2`, the
/// auth mode, the topic ACL. `Err((exit code, message))`; exit code 4 is
/// Spec §2's TLS error.
fn security_from(cfg: &DaemonConfig) -> Result<SecurityCtx, (u8, String)> {
    let config_error = |m: String| (1u8, format!("config: {m}"));
    let mut sc = SecurityConfig::default();
    sc.auth_mode = cfg.auth_mode.clone();
    match cfg.auth_mode.as_str() {
        "bearer" => {
            let token = cfg
                .auth_bearer_token
                .clone()
                .ok_or_else(|| config_error("auth.mode: bearer needs auth.bearer_token".into()))?;
            let subject = cfg
                .auth_bearer_subject
                .clone()
                .unwrap_or_else(|| "anonymous".into());
            sc.bearer_tokens.insert(token, subject);
        }
        "jwt" => sc
            .apply_jwt(&cfg.auth_jwt)
            .map_err(|e| config_error(e.to_string()))?,
        "mtls" => {
            if !cfg.tls_enabled || cfg.tls_client_ca_file.is_empty() {
                return Err(config_error(
                    "auth.mode: mtls needs grpc.tls.enabled and grpc.tls.client_ca_file".into(),
                ));
            }
        }
        _ => {}
    }
    for t in &cfg.topics {
        if let Some((publish, subscribe)) = &t.acl {
            sc.topic_acl.insert(
                t.dds_name.clone(),
                AclEntry {
                    read: subscribe.clone(),
                    write: publish.clone(),
                },
            );
        }
    }
    let mut ctx = build_ctx(&sc).map_err(|e| config_error(e.to_string()))?;
    if cfg.tls_enabled {
        if cfg.tls_cert_file.is_empty() || cfg.tls_key_file.is_empty() {
            return Err((
                4,
                "tls: grpc.tls.enabled needs cert_file and key_file".into(),
            ));
        }
        let ca = (!cfg.tls_client_ca_file.is_empty())
            .then(|| std::path::PathBuf::from(&cfg.tls_client_ca_file));
        ctx.tls = Some(
            build_grpc_tls_config_with(
                cfg.tls_cert_file.clone().into(),
                cfg.tls_key_file.clone().into(),
                ca,
                cfg.tls_require_client_cert,
            )
            .map_err(|e| (4, format!("tls: {e}")))?,
        );
    }
    Ok(ctx)
}

/// §9.2 SIGHUP: the TLS material, the auth settings and the ACL of the
/// reloaded files replace the live ones. A file that does not load, or
/// settings that cannot be served, change nothing. What needs a restart
/// (the listener, the DDS side, the topic mapping, TLS on or off) keeps
/// its started value and is logged.
fn apply_reload(
    live: &zerodds_bridge_security::LiveSecurity,
    started: &DaemonConfig,
    new: Result<DaemonConfig, String>,
) {
    let refused = |e: &str| eprintln!("{{\"event\":\"reload_refused\",\"err\":{e:?}}}");
    let mut new = match new {
        Ok(c) => c,
        Err(e) => return refused(&e),
    };
    let mapping = |c: &DaemonConfig| -> Vec<(String, String, String)> {
        c.topics
            .iter()
            .map(|t| {
                (
                    t.dds_name.clone(),
                    t.grpc_service.clone(),
                    t.direction.clone(),
                )
            })
            .collect()
    };
    let mut restart: Vec<&str> = Vec::new();
    if new.bind != started.bind || new.domain != started.domain {
        restart.push("bind/domain");
    }
    if mapping(&new) != mapping(started) {
        restart.push("topics");
    }
    if new.reflection_enabled != started.reflection_enabled {
        restart.push("grpc.reflection.enabled");
    }
    let tls_toggled = new.tls_enabled != started.tls_enabled;
    if tls_toggled {
        restart.push("grpc.tls.enabled");
        new.tls_enabled = false;
    }
    let mut ctx = match security_from(&new) {
        Ok(c) => c,
        Err((_, e)) => return refused(&e),
    };
    if tls_toggled {
        ctx.tls = live.current().tls.clone();
    }
    live.replace(ctx);
    eprintln!("{{\"event\":\"reloaded\",\"applied\":\"tls, auth, acl\"}}");
    if !restart.is_empty() {
        eprintln!(
            "{{\"event\":\"reload_needs_restart\",\"settings\":\"{}\"}}",
            restart.join(", ")
        );
    }
}

/// Documented fallback when `metrics.enabled: true` names no `listen:`.
const DEFAULT_METRICS_LISTEN: &str = "127.0.0.1:9095";

/// Admin-endpoint bind address. Precedence: CLI `--metrics` beats the
/// config file `metrics.listen:` (Spec Â§3 / Â§8.2); `metrics.enabled: false`
/// keeps the endpoint off. An enabled block without `listen:` falls back
/// to the documented default.
fn metrics_addr_for(cfg: &DaemonConfig) -> Option<String> {
    if !cfg.metrics_enabled {
        return None;
    }
    if cfg.metrics_listen.is_empty() {
        return Some(DEFAULT_METRICS_LISTEN.to_string());
    }
    Some(cfg.metrics_listen.clone())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use zerodds_grpc_bridge::bridge_security::Acl;

    /// One YAML text through the shared parser and this daemon's reader.
    fn parse_yaml_subset(yaml: &str) -> Result<DaemonConfig, String> {
        let mut config = zerodds_config::Config::default();
        let node = zerodds_config::parse_str(yaml, "<string>").map_err(|e| e.to_string())?;
        config
            .push(node, "<string>".to_string())
            .map_err(|e| e.to_string())?;
        DaemonConfig::from_config(Some(&config)).map_err(|e| e.to_string())
    }
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
                acl: None,
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
            metadata: Vec::new(),
            body: Vec::new(),
        };
        let resp = dispatch(
            &req,
            &cfg,
            None,
            &Acl::allow_all(),
            &AuthSubject::anonymous(),
        );
        assert_eq!(resp.status, Status::NotFound);
    }

    #[test]
    fn dispatch_publish_on_known_topic_is_ok() {
        let cfg = DaemonConfig {
            topics: vec![TopicMapping {
                dds_name: "T::M".into(),
                grpc_service: "TMStream".into(),
                direction: "bidir".into(),
                acl: None,
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
            metadata: Vec::new(),
            body,
        };
        // No DDS handle in the unit test → UNAVAILABLE, not an Ok with
        // accepted = 0 that a client would read as delivered-to-nobody.
        let resp = dispatch(
            &req,
            &cfg,
            None,
            &Acl::allow_all(),
            &AuthSubject::anonymous(),
        );
        assert_eq!(resp.status, Status::Unavailable);
    }

    /// `direction` decides which of `Publish` and `Subscribe` a topic
    /// answers; the other is FAILED_PRECONDITION, before the DDS side is
    /// consulted.
    #[test]
    fn dispatch_refuses_the_direction_a_topic_does_not_carry() {
        let mapping = |direction: &str| TopicMapping {
            dds_name: "T::M".into(),
            grpc_service: "TMStream".into(),
            direction: direction.into(),
            acl: None,
        };
        let request = |method: &str| GrpcRequest {
            stream_id: 3u32,
            path: format!("/zerodds.t.v1.TMStream/{method}"),
            service: "TMStream".into(),
            method: method.into(),
            encoding: None,
            metadata: Vec::new(),
            body: encode_message(&proto_sample(b"hi"), false).expect("lpm"),
        };
        for (direction, method, status) in [
            ("out", "Publish", Status::FailedPrecondition),
            ("in", "Subscribe", Status::FailedPrecondition),
            ("in", "Publish", Status::Unavailable),
            ("out", "Subscribe", Status::Unavailable),
        ] {
            let cfg = DaemonConfig {
                topics: vec![mapping(direction)],
                ..DaemonConfig::default()
            };
            let resp = dispatch(
                &request(method),
                &cfg,
                None,
                &Acl::allow_all(),
                &AuthSubject::anonymous(),
            );
            assert_eq!(resp.status, status, "{direction} {method}");
        }
    }

    // ---------------------------------------------------------------
    // Loadback of the shipped example config (Spec §3 ↔ parser ↔
    // packaging). The example historically declared `participant:` /
    // `routes:` / `logging:` / `observability:` — none of which this
    // parser reads, so the declared routes never reached the daemon.
    // ---------------------------------------------------------------

    /// The shipped example YAML, embedded against the real file.
    const EXAMPLE_YAML: &str =
        include_str!("../../../../packaging/linux/configs/grpc-bridged.yaml.example");

    /// Top-level (column-0) mapping keys of a YAML document.
    fn top_level_keys(yaml: &str) -> Vec<String> {
        yaml.lines()
            .filter(|l| !l.starts_with([' ', '\t', '#', '-']) && !l.trim().is_empty())
            .filter_map(|l| l.split_once(':').map(|(k, _)| k.trim().to_string()))
            .collect()
    }

    #[test]
    fn example_yaml_parses_without_error() {
        let res = parse_yaml_subset(EXAMPLE_YAML);
        assert!(
            res.is_ok(),
            "grpc-bridged.yaml.example does not parse: {:?}",
            res.err()
        );
    }

    #[test]
    fn example_yaml_key_fields_match_intent() {
        let cfg = parse_yaml_subset(EXAMPLE_YAML).expect("example yaml must parse");
        assert_eq!(cfg.domain, 0);
        assert_eq!(cfg.log_level, "info");
        assert_eq!(cfg.bind, "0.0.0.0:50051");
        assert!(!cfg.tls_enabled);
        assert_eq!(cfg.tls_cert_file, "/etc/zerodds/certs/grpc-bridged.crt");
        assert_eq!(cfg.tls_key_file, "/etc/zerodds/certs/grpc-bridged.key");
        // reflection deviates from the default (false).
        assert!(cfg.reflection_enabled);
        assert_eq!(cfg.auth_mode, "none");
        // metrics — endpoint off, but the address has to arrive (Spec §3).
        assert!(!cfg.metrics_enabled);
        assert_eq!(cfg.metrics_listen, "127.0.0.1:9095");
    }

    #[test]
    fn example_yaml_topics_reach_the_daemon() {
        let cfg = parse_yaml_subset(EXAMPLE_YAML).expect("example yaml must parse");
        assert_eq!(cfg.topics.len(), 1, "example shows exactly one demo route");
        assert_eq!(cfg.topics[0].dds_name, "Inventory");
        assert_eq!(cfg.topics[0].grpc_service, "InventoryStream");
        assert_eq!(cfg.topics[0].direction, "out");
    }

    #[test]
    fn example_yaml_has_no_unknown_top_level_keys() {
        for key in top_level_keys(EXAMPLE_YAML) {
            assert!(
                KEYS.iter()
                    .any(|k| k.split('.').next() == Some(key.as_str())),
                "grpc-bridged.yaml.example declares top-level key {key:?}, \
                 which parse_yaml_subset does not read"
            );
        }
    }

    #[test]
    fn unknown_top_level_key_does_not_abort_the_parse() {
        let yaml = "\
participant:
  domain_id: 7
observability:
  metrics_bind: \"127.0.0.1:9999\"
domain: 5
";
        let cfg = parse_yaml_subset(yaml).expect("unknown keys must not abort");
        assert_eq!(cfg.domain, 5);
        assert!(!cfg.metrics_enabled, "observability: is not a known key");
    }

    #[test]
    fn env_substitution_applies_before_the_parse() {
        let yaml = "\
grpc:
  bind: \"${ZERODDS_UNSET_PROBE_a41f:-127.0.0.1:50099}\"
";
        let cfg = parse_yaml_subset(yaml).expect("parse");
        assert_eq!(cfg.bind, "127.0.0.1:50099");
    }

    /// Regression: comments used to be cut at the first `#`. The shared
    /// parser keeps a `#` inside quotes and still removes a trailing comment.
    #[test]
    fn a_hash_inside_quotes_is_part_of_the_value() {
        let cfg = parse_yaml_subset("grpc:\n  bind: \"0.0.0.0:1\"   # listen\n").expect("parse");
        assert_eq!(cfg.bind, "0.0.0.0:1");
    }

    #[test]
    fn what_the_old_parser_ignored_is_refused() {
        for (yaml, needle) in [
            ("grpc:\n  tls:\n    enabled: yes\n", "grpc.tls.enabled"),
            ("grpc:\n  reflecton:\n    enabled: true\n", "grpc.reflecton"),
            ("auth:\n  mode: basic\n", "auth.mode"),
            (
                "topics:\n  - dds_name: T\n    grpc_servce: S\n",
                "topics[0].grpc_servce",
            ),
        ] {
            let err = parse_yaml_subset(yaml).expect_err(yaml);
            assert!(err.contains(needle), "{yaml}: {err}");
        }
    }

    #[test]
    fn a_topic_shorthand_keeps_the_idl_scope() {
        let cfg = parse_yaml_subset(
            "topics:\n  - \"Chat::Message=chat.Chat\"\n  - \"Chat::Message:chat.Other\"\n",
        )
        .expect("parse");
        assert_eq!(cfg.topics[0].dds_name, "Chat::Message");
        assert_eq!(cfg.topics[0].grpc_service, "chat.Chat");
        assert_eq!(cfg.topics[1].dds_name, "Chat::Message");
        assert_eq!(cfg.topics[1].grpc_service, "chat.Other");
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod metrics_default_tests {
    use super::DEFAULT_METRICS_LISTEN;

    /// Each bridge daemon has its own admin port, so that several with
    /// `metrics.enabled: true` and no `metrics.listen` run on one host;
    /// man5 names it as the default.
    #[test]
    fn the_metrics_default_is_the_one_man5_names() {
        let page = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../man/man5/zerodds-grpc-bridged.yaml.5"
        ))
        .unwrap_or_else(|e| panic!("man5 page: {e}"));
        let named = format!("Default\n.IR \"{DEFAULT_METRICS_LISTEN}\"\nwhen");
        assert!(
            page.contains(&named),
            "man5 does not name {DEFAULT_METRICS_LISTEN}"
        );
    }
}
