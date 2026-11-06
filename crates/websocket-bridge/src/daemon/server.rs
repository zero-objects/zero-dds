// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! WebSocket server + DDS pump for the `zerodds-ws-bridged` daemon.
//!
//! Spec: `zerodds-ws-bridge-1.0.md` §4 + §9.
//!
//! `eprintln!` logging in the daemon path: Spec §8.1 leaves
//! structured logging to the daemon. Until a workspace tracing stack
//! is wired in, `eprintln` is the sink; marked locally as a clippy-allow
//! on the affected functions.
//!
//! Sync, blocking I/O on `std::net`. One reader thread per connection
//! (TCP→WS frames→router) plus one writer thread
//! (router channel→WS frames→TCP). The DDS pump thread consumes
//! `mpsc::Receiver<UserSample>` from each registered reader and
//! dispatches via the `Router`.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::codec::{decode, encode};
use crate::frame::{Frame, Opcode};
use crate::handshake::{build_server_response, parse_client_request, render_server_response};

use super::config::{DaemonConfig, TopicConfig};
use super::json_frames::{self, ClientMessage, SampleFrame, SampleOp, TopicType};
use super::router::{DdsSample, Router, RouterMsg, SampleKind};
#[cfg(feature = "daemon")]
use super::runtime_common::{
    BridgeMetrics, CatalogSnapshot, SERVICE_NAME, install_signal_watcher, otlp_config_from_env,
    serve_admin_endpoints, spawn_otlp_flush_loop,
};
#[cfg(feature = "daemon")]
use super::security::{
    AclOp, AuthSubject, authenticate_ws, authorize, ctx_from_daemon_config,
    extract_authorization_header, serve_tls_handshake,
};
#[cfg(feature = "daemon")]
use rustls::{ServerConnection, StreamOwned};
#[cfg(feature = "daemon")]
use zerodds_monitor::Registry;
#[cfg(feature = "daemon")]
use zerodds_observability_otlp::OtlpExporter;

#[cfg(feature = "daemon")]
use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
#[cfg(feature = "daemon")]
use zerodds_rtps::wire_types::EntityId;

/// Admin endpoint address when `metrics.enabled` is set without
/// `metrics.listen`; one port per bridge daemon, see man5.
pub const DEFAULT_METRICS_LISTEN: &str = "127.0.0.1:9091";

/// Top-level error of the daemon.
#[derive(Debug)]
pub enum ServerError {
    /// A setting that cannot be served (exit code 1).
    Config(String),
    /// TLS material that does not load (exit code 4).
    Tls(String),
    /// Bind error (exit code 2).
    Bind(String),
    /// DCPS init error (exit code 3).
    Dds(String),
    /// I/O error during operation.
    Io(String),
}

impl core::fmt::Display for ServerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Config(m) => write!(f, "config error: {m}"),
            Self::Tls(m) => write!(f, "tls error: {m}"),
            Self::Bind(m) => write!(f, "bind error: {m}"),
            Self::Dds(m) => write!(f, "dds error: {m}"),
            Self::Io(m) => write!(f, "io error: {m}"),
        }
    }
}

impl std::error::Error for ServerError {}

/// Daemon handle. On drop, shutdown is invoked.
pub struct DaemonHandle {
    stop: Arc<AtomicBool>,
    accept_thread: Option<JoinHandle<()>>,
    pump_threads: Vec<JoinHandle<()>>,
    #[cfg(feature = "daemon")]
    admin_thread: Option<JoinHandle<()>>,
    #[cfg(feature = "daemon")]
    otlp_thread: Option<JoinHandle<()>>,
    router: Arc<Mutex<Router>>,
    /// Bound address (may differ from the configured listen if port=0).
    pub local_addr: String,
    /// Bound admin address (Prometheus + catalog + healthz).
    #[cfg(feature = "daemon")]
    pub admin_addr: Option<String>,
    /// Lifecycle: SIGHUP sets this; the server loop can react.
    #[cfg(feature = "daemon")]
    pub reload_flag: Arc<AtomicBool>,
    /// Healthz flag — DCPS runtime up == true.
    #[cfg(feature = "daemon")]
    pub healthy: Arc<AtomicBool>,
    /// Metric set for the §8.2 wireup. Reader-side for tests.
    #[cfg(feature = "daemon")]
    pub metrics: Option<BridgeMetrics>,
    /// Bridge-internal DCPS runtime — exported for E2E tests that
    /// synthetically inject samples into the daemon reader channel
    /// (via `DcpsRuntime::test_inject_user_alive`) without going through
    /// the wire path.
    #[cfg(feature = "daemon")]
    pub runtime: Arc<DcpsRuntime>,
    /// Topic name → registered writer EntityId.
    #[cfg(feature = "daemon")]
    pub user_writers: std::collections::BTreeMap<String, EntityId>,
    /// Topic name → registered reader EntityId.
    #[cfg(feature = "daemon")]
    pub user_readers: std::collections::BTreeMap<String, EntityId>,
}

impl DaemonHandle {
    /// Whether SIGTERM/SIGINT (or [`Self::shutdown`]) asked the daemon to
    /// stop. The binary waits for it and then runs the shutdown.
    #[must_use]
    pub fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// Initiates a graceful shutdown; the DDS participant stops last.
    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        #[cfg(feature = "daemon")]
        {
            self.healthy.store(false, Ordering::SeqCst);
        }
        // Unblock accept().
        if let Ok(addr) = self.local_addr.parse::<std::net::SocketAddr>() {
            // Self-connect to wake accept().
            let _ = TcpStream::connect_timeout(&addr, Duration::from_millis(200));
        }
        // Self-connect the admin server (so the accept loop unblocks).
        #[cfg(feature = "daemon")]
        if let Some(admin) = self.admin_addr.as_deref() {
            if let Ok(addr) = admin.parse::<std::net::SocketAddr>() {
                let _ = TcpStream::connect_timeout(&addr, Duration::from_millis(200));
            }
        }
        if let Ok(r) = self.router.lock() {
            r.broadcast_shutdown();
        }
        if let Some(j) = self.accept_thread.take() {
            let _ = j.join();
        }
        for j in self.pump_threads.drain(..) {
            let _ = j.join();
        }
        #[cfg(feature = "daemon")]
        {
            if let Some(j) = self.admin_thread.take() {
                let _ = j.join();
            }
            if let Some(j) = self.otlp_thread.take() {
                let _ = j.join();
            }
            // Last, so no pump writes to a stopped runtime. Dropping the
            // handle alone would not stop it — the runtime's worker threads
            // hold `Arc`s of their own — and a daemon stopped in a process
            // that keeps running would leave its participant on the domain.
            self.runtime.shutdown();
        }
    }
}

impl Drop for DaemonHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}
/// zerodds-lint: recursion-depth 64 (start bounded by AST depth)
/// Starts the daemon with the given config. Does NOT block —
/// returns a `DaemonHandle` through which the caller (either
/// the binary or an E2E test) controls the lifecycle.
///
/// # Errors
/// `Bind` if the TCP listener cannot bind (Spec exit code 2).
/// `Dds` if the `DcpsRuntime` cannot start (Spec exit code 3).
#[cfg(feature = "daemon")]
pub fn start(cfg: DaemonConfig) -> Result<DaemonHandle, ServerError> {
    let runtime = RuntimeConfig::from_standard_paths()
        .map_err(|e| ServerError::Dds(alloc_format(format_args!("runtime configuration: {e}"))))?;
    start_with_runtime(cfg, runtime)
}

/// [`start`] with the DDS runtime configuration a command line loaded
/// (`zerodds_dcps::cli::Configured`), which `start` reads from the standard
/// layers instead.
///
/// # Errors
/// As [`start`].
#[cfg(feature = "daemon")]
#[allow(clippy::too_many_lines)]
pub fn start_with_runtime(
    cfg: DaemonConfig,
    runtime_config: RuntimeConfig,
) -> Result<DaemonHandle, ServerError> {
    start_with_reload(cfg, runtime_config, None)
}

/// Reads and validates the configuration again, for a SIGHUP reload.
pub type ConfigLoader = Box<dyn Fn() -> Result<DaemonConfig, String> + Send + Sync>;

/// [`start_with_runtime`], reloading on SIGHUP (§9.2). With `load`, the
/// configuration is read again: the ACL and the auth settings apply at once
/// to every connection, the certificate files are read again for the
/// connections that follow; changed paths, topics, listen address or domain
/// need a restart and are logged. Without `load`, SIGHUP reads the
/// certificate files again only. A configuration or certificate that does
/// not load changes nothing.
///
/// # Errors
/// As [`start`].
pub fn start_with_reload(
    cfg: DaemonConfig,
    runtime_config: RuntimeConfig,
    load: Option<ConfigLoader>,
) -> Result<DaemonHandle, ServerError> {
    eprintln!(
        "[zerodds-ws-bridged] starting on {} domain={} topics={}",
        cfg.listen,
        cfg.domain,
        cfg.topics.len()
    );

    // 0. Metrics registry + standard counter set (§8.2).
    let registry = Arc::new(Registry::new());
    let metrics = BridgeMetrics::register(&registry);

    // 0b. Bridge security: security ctx + optional RotatingTlsConfig for
    //     SIGHUP hot-reload (§7.1 TLS / §7.2 auth / §7.3 ACL).
    // Spec §2: a setting that cannot be served is exit 1, TLS material
    // that does not load exit 4. Both used to leave as a bind error (2).
    let (security_ctx, rotating_tls) = ctx_from_daemon_config(&cfg).map_err(|e| match e {
        zerodds_bridge_security::SecurityError::Tls(_) => {
            ServerError::Tls(alloc_format(format_args!("security: {e}")))
        }
        _ => ServerError::Config(alloc_format(format_args!("security: {e}"))),
    })?;
    let security_ctx = Arc::new(zerodds_bridge_security::LiveSecurity::new(security_ctx));
    let rotating_tls = rotating_tls.map(Arc::new);
    if rotating_tls.is_some() {
        eprintln!(
            "[zerodds-ws-bridged] TLS active (cert={}, mtls={})",
            cfg.tls_cert_file,
            !cfg.tls_client_ca_file.is_empty(),
        );
    }
    eprintln!(
        "[zerodds-ws-bridged] auth-mode={} acl-entries={}",
        cfg.auth_mode,
        cfg.topic_acl.len()
    );

    // 1. Bring up the DCPS runtime.
    let prefix = zerodds_dcps::participant::bridge_guid_prefix(&cfg.listen);
    let runtime = DcpsRuntime::start(cfg.domain, prefix, runtime_config)
        .map_err(|e| ServerError::Dds(alloc_format(format_args!("{e:?}"))))?;
    let healthy = Arc::new(AtomicBool::new(true));

    // 2. The types of the topics with an `idl_file` (§3), which the JSON
    //    text frames (§4.3) read and write the samples with. A file that
    //    does not load stops the start.
    let mut types: std::collections::BTreeMap<String, TopicType> =
        std::collections::BTreeMap::new();
    for topic in &cfg.topics {
        if topic.idl_file.is_empty() {
            continue;
        }
        let ty = TopicType::load(&topic.idl_file, &topic.type_name).map_err(|e| {
            ServerError::Config(alloc_format(format_args!("topic {}: {e}", topic.name)))
        })?;
        types.insert(topic.name.clone(), ty);
    }
    let types = Arc::new(types);

    // Register a reader+writer per topic.
    let mut writers: std::collections::BTreeMap<String, EntityId> =
        std::collections::BTreeMap::new();
    let mut reader_eids: std::collections::BTreeMap<String, EntityId> =
        std::collections::BTreeMap::new();
    let mut readers: Vec<(String, std::sync::mpsc::Receiver<UserSample>)> = Vec::new();
    for topic in &cfg.topics {
        let keyed = types.get(&topic.name).is_some_and(TopicType::keyed);
        let (reader_eid, writer_eid) = register_topic_endpoints(&runtime, topic, keyed)?;
        if let Some((eid, rx)) = reader_eid {
            reader_eids.insert(topic.name.clone(), eid);
            readers.push((topic.name.clone(), rx));
        }
        if let Some(eid) = writer_eid {
            writers.insert(topic.name.clone(), eid);
        }
    }

    // 3. Router + TCP listener.
    let router = Arc::new(Mutex::new(Router::new()));
    let listener = TcpListener::bind(&cfg.listen)
        .map_err(|e| ServerError::Bind(alloc_format(format_args!("{e}"))))?;
    let local_addr = listener
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| cfg.listen.clone());
    listener
        .set_nonblocking(false)
        .map_err(|e| ServerError::Io(alloc_format(format_args!("{e}"))))?;

    eprintln!("[zerodds-ws-bridged] bound on {local_addr}");

    let stop = Arc::new(AtomicBool::new(false));
    let reload_flag = Arc::new(AtomicBool::new(false));

    // 4. Pump threads per reader.
    let mut pump_threads = Vec::new();
    for (topic_name, rx) in readers {
        let router_c = Arc::clone(&router);
        let stop_c = Arc::clone(&stop);
        let topic_name_c = topic_name.clone();
        let dds_out = Arc::clone(&metrics.dds_samples_out_total);
        let h = thread::spawn(move || {
            while !stop_c.load(Ordering::SeqCst) {
                match rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(UserSample::Alive {
                        payload,
                        representation_identifier,
                        key_hash,
                        source_timestamp,
                        ..
                    }) => {
                        dds_out.inc();
                        let sample = DdsSample {
                            payload: payload.to_vec(),
                            representation_identifier,
                            kind: SampleKind::Alive,
                            key_hash,
                            timestamp_ns: source_timestamp
                                .map(|t| json_frames::nanos_of(t.seconds, t.fraction)),
                        };
                        if let Ok(mut r) = router_c.lock() {
                            r.dispatch(&topic_name_c, sample);
                        }
                    }
                    // A dispose or an unregister goes to the clients too
                    // (§4.3: the dispose flag, the JSON ops), with the key
                    // holder where the marker carried it.
                    Ok(UserSample::Lifecycle {
                        key_hash,
                        kind,
                        key_holder,
                        source_timestamp,
                        ..
                    }) => {
                        use zerodds_rtps::history_cache::ChangeKind;
                        let kind = match kind {
                            ChangeKind::NotAliveDisposed => SampleKind::Disposed,
                            ChangeKind::NotAliveUnregistered => SampleKind::Unregistered,
                            ChangeKind::NotAliveDisposedUnregistered => {
                                SampleKind::DisposedUnregistered
                            }
                            ChangeKind::Alive | ChangeKind::AliveFiltered => continue,
                        };
                        dds_out.inc();
                        let sample = DdsSample {
                            payload: key_holder.unwrap_or_default(),
                            representation_identifier: KEY_HOLDER_IDENTIFIER,
                            kind,
                            key_hash: Some(key_hash),
                            timestamp_ns: source_timestamp
                                .map(|t| json_frames::nanos_of(t.seconds, t.fraction)),
                        };
                        if let Ok(mut r) = router_c.lock() {
                            r.dispatch(&topic_name_c, sample);
                        }
                    }
                    // The end-of-coherent-set marker (DDSI-RTPS 2.5 §8.7.6)
                    // carries no user data — nothing to forward.
                    Ok(UserSample::CoherentSetEnd { .. }) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        pump_threads.push(h);
    }

    // 5. Accept thread.
    let next_conn_id = Arc::new(AtomicU64::new(1));
    let stop_acc = Arc::clone(&stop);
    let router_acc = Arc::clone(&router);
    // Snapshot of the writer map for the DaemonHandle export, before `writers`
    // is moved into the Arc (accepted by the dispatch path).
    let writers_export = writers.clone();
    let writers_arc = Arc::new(writers);
    let runtime_acc = Arc::clone(&runtime);
    let topics_arc = Arc::new(cfg.topics.clone());
    let types_acc = Arc::clone(&types);
    let metrics_acc = metrics.clone();
    let security_acc = Arc::clone(&security_ctx);
    let rotating_acc = rotating_tls.clone();

    let accept_thread = thread::spawn(move || {
        for incoming in listener.incoming() {
            if stop_acc.load(Ordering::SeqCst) {
                break;
            }
            match incoming {
                Ok(tcp) => {
                    let conn_id = next_conn_id.fetch_add(1, Ordering::SeqCst);
                    let router_h = Arc::clone(&router_acc);
                    let writers_h = Arc::clone(&writers_arc);
                    let runtime_h = Arc::clone(&runtime_acc);
                    let stop_h = Arc::clone(&stop_acc);
                    let topics_h = Arc::clone(&topics_arc);
                    let types_h = Arc::clone(&types_acc);
                    let metrics_h = metrics_acc.clone();
                    let security_h = Arc::clone(&security_acc);
                    let rot_h = rotating_acc.clone();
                    thread::spawn(move || {
                        // If TLS is configured: rustls handshake. Otherwise plain.
                        let (stream, mtls_subj) = if let Some(rot) = rot_h.as_ref() {
                            let cfg = rot.current();
                            match serve_tls_handshake(cfg, tcp, Duration::from_secs(5)) {
                                Ok((tcp, conn, subj)) => {
                                    (WsStream::Tls(Box::new(StreamOwned::new(conn, tcp))), subj)
                                }
                                Err(e) => {
                                    metrics_h.errors_total.inc();
                                    eprintln!(
                                        "[zerodds-ws-bridged] tls handshake err conn={conn_id}: {e}"
                                    );
                                    return;
                                }
                            }
                        } else {
                            (WsStream::Plain(tcp), None)
                        };
                        let _ = serve_connection(
                            conn_id, stream, mtls_subj, router_h, writers_h, runtime_h, stop_h,
                            topics_h, types_h, metrics_h, security_h,
                        );
                    });
                }
                Err(e) => {
                    eprintln!("[zerodds-ws-bridged] accept error: {e}");
                    continue;
                }
            }
        }
    });

    // 6. Admin endpoint (§8.2 Prometheus + §5.2 catalog/healthz).
    let mut admin_thread: Option<JoinHandle<()>> = None;
    let mut admin_addr: Option<String> = None;
    if cfg.metrics_enabled || !cfg.metrics_addr.is_empty() {
        let bind_str = if cfg.metrics_addr.is_empty() {
            DEFAULT_METRICS_LISTEN.to_string()
        } else {
            cfg.metrics_addr.clone()
        };
        match bind_str.parse::<std::net::SocketAddr>() {
            Ok(sock) => {
                let snap = Arc::new(CatalogSnapshot::from_config(&cfg));
                match serve_admin_endpoints(
                    sock,
                    snap,
                    Arc::clone(&registry),
                    Arc::clone(&healthy),
                    Arc::clone(&stop),
                ) {
                    Ok((h, bound)) => {
                        eprintln!(
                            "[{SERVICE_NAME}] admin endpoint on {bound} (/metrics /catalog /healthz)"
                        );
                        admin_addr = Some(bound.to_string());
                        admin_thread = Some(h);
                    }
                    Err(e) => {
                        eprintln!("[{SERVICE_NAME}] admin bind error: {e}");
                    }
                }
            }
            Err(e) => {
                eprintln!("[{SERVICE_NAME}] admin addr parse error: {e}");
            }
        }
    }

    // 7. Signal watcher (§9.2 graceful shutdown).
    if let Err(e) = install_signal_watcher(Arc::clone(&stop), Arc::clone(&reload_flag)) {
        eprintln!("[{SERVICE_NAME}] signal watcher init failed: {e}");
    }

    // 7b. Bridge security: SIGHUP hook for TLS cert hot-reload. Polls the
    //     reload_flag and calls RotatingTlsConfig::reload().
    if rotating_tls.is_some() || load.is_some() {
        let stop_r = Arc::clone(&stop);
        let reload_r = Arc::clone(&reload_flag);
        let rot = rotating_tls.clone();
        let live = Arc::clone(&security_ctx);
        let started = cfg.clone();
        thread::Builder::new()
            .name("zerodds-ws-reload".into())
            .spawn(move || {
                while !stop_r.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(100));
                    if reload_r.swap(false, Ordering::SeqCst) {
                        reload(load.as_ref(), rot.as_deref(), &live, &started);
                    }
                }
            })
            .ok();
    }

    // 8. OTLP span exporter (§8.3) if the env var is set.
    let otlp_thread = if let Some(otlp_cfg) = otlp_config_from_env(SERVICE_NAME) {
        let exporter = Arc::new(OtlpExporter::new(otlp_cfg));
        match spawn_otlp_flush_loop(exporter, Arc::clone(&stop), Duration::from_secs(5)) {
            Ok(h) => {
                eprintln!("[{SERVICE_NAME}] OTLP exporter active");
                Some(h)
            }
            Err(e) => {
                eprintln!("[{SERVICE_NAME}] OTLP spawn failed: {e}");
                None
            }
        }
    } else {
        None
    };

    Ok(DaemonHandle {
        stop,
        accept_thread: Some(accept_thread),
        pump_threads,
        admin_thread,
        otlp_thread,
        router,
        local_addr,
        admin_addr,
        reload_flag,
        healthy,
        metrics: Some(metrics),
        runtime: Arc::clone(&runtime),
        user_writers: writers_export,
        user_readers: reader_eids,
    })
}

#[cfg(feature = "daemon")]
type ReaderEndpoint = (EntityId, std::sync::mpsc::Receiver<UserSample>);
#[cfg(feature = "daemon")]
type TopicEndpoints = (Option<ReaderEndpoint>, Option<EntityId>);

#[cfg(feature = "daemon")]
fn register_topic_endpoints(
    rt: &Arc<DcpsRuntime>,
    topic: &TopicConfig,
    keyed: bool,
) -> Result<TopicEndpoints, ServerError> {
    use zerodds_qos::DurabilityKind;
    let durability = match topic.durability.as_str() {
        "transient_local" => DurabilityKind::TransientLocal,
        "transient" => DurabilityKind::Transient,
        "persistent" => DurabilityKind::Persistent,
        _ => DurabilityKind::Volatile,
    };
    let reliable = !matches!(topic.reliability.as_str(), "best_effort");
    // Same meaning as in the other bridge daemons: `in` carries client
    // frames into DDS (a DataWriter), `out` carries DDS samples out to the
    // clients (a DataReader), `bidir` both.
    let want_writer = matches!(topic.direction.as_str(), "in" | "bidir");
    let want_reader = matches!(topic.direction.as_str(), "out" | "bidir");

    let reader = if want_reader {
        let (eid, rx) = rt
            .register_user_reader_kind(
                UserReaderConfig {
                    topic_name: topic.name.clone(),
                    type_name: if topic.type_name.is_empty() {
                        topic.name.clone()
                    } else {
                        topic.type_name.clone()
                    },
                    reliable,
                    durability,
                    ..Default::default()
                },
                keyed,
            )
            .map_err(|e| ServerError::Dds(alloc_format(format_args!("reader: {e:?}"))))?;
        Some((eid, rx))
    } else {
        None
    };

    let writer = if want_writer {
        let eid = rt
            .register_user_writer_kind(
                UserWriterConfig {
                    topic_name: topic.name.clone(),
                    type_name: if topic.type_name.is_empty() {
                        topic.name.clone()
                    } else {
                        topic.type_name.clone()
                    },
                    reliable,
                    durability,
                    ..Default::default()
                },
                keyed,
            )
            .map_err(|e| ServerError::Dds(alloc_format(format_args!("writer: {e:?}"))))?;
        Some(eid)
    } else {
        None
    };

    Ok((reader, writer))
}

#[cfg(feature = "daemon")]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn serve_connection(
    conn_id: u64,
    mut stream: WsStream,
    mtls_subject: Option<AuthSubject>,
    router: Arc<Mutex<Router>>,
    writers: Arc<std::collections::BTreeMap<String, EntityId>>,
    runtime: Arc<DcpsRuntime>,
    stop: Arc<AtomicBool>,
    topics: Arc<Vec<TopicConfig>>,
    types: Arc<std::collections::BTreeMap<String, TopicType>>,
    metrics: BridgeMetrics,
    security: Arc<zerodds_bridge_security::LiveSecurity>,
) -> Result<(), ServerError> {
    metrics.connections_total.inc();
    metrics.connections_active.inc();
    let conn_guard = ConnectionLifetime {
        active: Arc::clone(&metrics.connections_active),
    };
    stream.set_nodelay(true).ok();
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok();

    // 1. Read in the HTTP upgrade handshake.
    let mut buf = [0u8; 4096];
    let mut accumulated = Vec::new();
    let req_str = loop {
        match stream.read(&mut buf) {
            Ok(0) => return Err(ServerError::Io("eof during handshake".to_string())),
            Ok(n) => {
                accumulated.extend_from_slice(&buf[..n]);
                if accumulated.windows(4).any(|w| w == b"\r\n\r\n") {
                    let s = String::from_utf8_lossy(&accumulated).to_string();
                    break s;
                }
                if accumulated.len() > 64 * 1024 {
                    return Err(ServerError::Io("handshake too large".to_string()));
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(e) => return Err(ServerError::Io(e.to_string())),
        }
    };

    let req = match parse_client_request(&req_str) {
        Ok(r) => r,
        Err(e) => {
            let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n");
            return Err(ServerError::Io(alloc_format(format_args!(
                "handshake parse: {e:?}"
            ))));
        }
    };

    // §7.2 — authentication. On reject: HTTP 401 + close.
    let auth_header = extract_authorization_header(&req_str);
    let auth_headers: Vec<(String, String)> = if let Some(v) = auth_header {
        vec![("authorization".to_string(), v)]
    } else {
        Vec::new()
    };
    let subject = match authenticate_ws(
        &security.current().auth,
        &auth_headers,
        mtls_subject.clone(),
    ) {
        Ok(s) => s,
        Err(e) => {
            metrics.errors_total.inc();
            let body = b"unauthorized";
            let resp = format!(
                "HTTP/1.1 401 Unauthorized\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nWWW-Authenticate: Bearer realm=\"zerodds-ws\"\r\nConnection: close\r\n\r\nunauthorized",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
            eprintln!("[zerodds-ws-bridged] auth reject conn={conn_id} reason={e}");
            return Err(ServerError::Io(alloc_format(format_args!(
                "auth reject: {e}"
            ))));
        }
    };

    // Auto-subscribe if the path matches a topic (Spec §4.2).
    let mut auto_topic: Option<String> = None;
    for t in topics.iter() {
        if t.ws_path == req.path || super::config::default_ws_path(&t.name) == req.path {
            auto_topic = Some(t.name.clone());
            break;
        }
    }

    // §7.3 — auto-subscribe topic ACL check (read).
    if let Some(topic) = &auto_topic {
        if !authorize(&security.current().acl, &subject, AclOp::Read, topic) {
            metrics.errors_total.inc();
            let body = format!("forbidden: read on {topic}");
            let resp = format!(
                "HTTP/1.1 403 Forbidden\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
            eprintln!(
                "[zerodds-ws-bridged] acl reject conn={conn_id} subject={} topic={topic}",
                subject.name
            );
            return Err(ServerError::Io(alloc_format(format_args!(
                "acl reject: {topic}"
            ))));
        }
    }

    // §4.1, §4.3: binary frames unless the client asks for the JSON text
    // frames with the `+json` subprotocol; the chosen subprotocol is named
    // back (RFC 6455 §4.2.2 item 8).
    let json_mode = req.protocols.iter().any(|p| p == SUBPROTOCOL_JSON);
    let mut resp = build_server_response(&req);
    if json_mode {
        resp.protocol = Some(SUBPROTOCOL_JSON.to_string());
    } else if req.protocols.iter().any(|p| p == SUBPROTOCOL_BINARY) {
        resp.protocol = Some(SUBPROTOCOL_BINARY.to_string());
    }
    let resp_bytes = render_server_response(&resp);
    stream
        .write_all(resp_bytes.as_bytes())
        .map_err(|e| ServerError::Io(e.to_string()))?;

    // A JSON-mode client of a topic path gets the topic's samples as JSON
    // (§4.3), which needs the topic's type; without one the client is told
    // so, and the connection carries no samples of it.
    if json_mode
        && let Some(topic) = auto_topic.as_deref()
        && !types.contains_key(topic)
    {
        metrics.errors_total.inc();
        if let Ok(b) = encode(&Frame::text(no_type_error(topic))) {
            let _ = stream.write_all(&b);
        }
        auto_topic = None;
    }

    // 2. Register the connection with the router.
    let (tx, rx) = std::sync::mpsc::channel::<RouterMsg>();
    if let Ok(mut r) = router.lock() {
        r.register_connection(conn_id, tx);
        if let Some(topic) = &auto_topic {
            r.subscribe(conn_id, topic.clone());
        }
    }

    // 3. Split stream into independent reader and writer handles so a blocking
    //    `read()` (500 ms timeout) never blocks outbound notify frames on the
    //    writer thread. For plain TCP, `try_clone()` duplicates the OS socket
    //    descriptor so reader and writer share no user-space lock at all. For
    //    TLS, the reader waits via `peek()` on a cloned `TcpStream` outside the
    //    session mutex and only locks the `StreamOwned` once bytes are in the
    //    kernel receive buffer.
    let (mut reader, writer) = stream
        .into_split()
        .map_err(|e| ServerError::Io(e.to_string()))?;
    let stop_w = Arc::clone(&stop);
    let frames_out = Arc::clone(&metrics.frames_out_total);
    let bytes_out = Arc::clone(&metrics.bytes_out_total);
    let errors_out = Arc::clone(&metrics.errors_total);
    let writer_w = Arc::clone(&writer);
    // Per-connection ACL state: the read check is done per topic
    // before we hand off to router.dispatch. Here before the send: we
    // have the subject + ACL via closure move.
    let security_w = Arc::clone(&security);
    let subject_w = subject.clone();
    let types_w = Arc::clone(&types);
    let writer_thread = thread::spawn(move || {
        'samples: while !stop_w.load(Ordering::SeqCst) {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(RouterMsg::Sample { topic, sample }) => {
                    // §7.3 — read ACL (post-subscribe gate): if the
                    // ACL for this subject + topic is deny,
                    // drop the sample (no disclosure).
                    if !authorize(&security_w.current().acl, &subject_w, AclOp::Read, &topic) {
                        continue;
                    }
                    let frames: Vec<Frame> = if json_mode {
                        match json_sample_frames(&topic, &sample, types_w.get(&topic)) {
                            Ok(texts) => texts.into_iter().map(Frame::text).collect(),
                            Err(error) => {
                                errors_out.inc();
                                vec![Frame::text(error)]
                            }
                        }
                    } else {
                        vec![Frame::binary(binary_sample_frame(&sample))]
                    };
                    for frame in frames {
                        if let Ok(bytes) = encode(&frame) {
                            bytes_out.add(bytes.len() as u64);
                            if writer_w.write_all(&bytes).is_err() {
                                errors_out.inc();
                                break 'samples;
                            }
                            frames_out.inc();
                        } else {
                            errors_out.inc();
                        }
                    }
                }
                Ok(RouterMsg::Shutdown) => {
                    let close = Frame::close(1001, "going away");
                    if let Ok(b) = encode(&close) {
                        let _ = writer_w.write_all(&b);
                    }
                    break;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });

    // 4. Reader loop (TCP/TLS → WS frames → router/DDS writer).
    let mut frame_buf: Vec<u8> = Vec::new();
    'reader: loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let read_result = reader.read(&mut buf);
        match read_result {
            Ok(0) => break,
            Ok(n) => {
                frame_buf.extend_from_slice(&buf[..n]);
                while let Ok((frame, used)) = decode(&frame_buf) {
                    frame_buf.drain(..used);
                    match frame.opcode {
                        Opcode::Binary if frame.payload.starts_with(&BINARY_MAGIC) => {
                            metrics.frames_in_total.inc();
                            metrics.bytes_in_total.add(frame.payload.len() as u64);
                            if let Err(e) = handle_binary_sample(
                                &frame.payload,
                                auto_topic.as_deref(),
                                &writers,
                                &runtime,
                                &metrics,
                                &security,
                                &subject,
                                &writer,
                            ) {
                                metrics.errors_total.inc();
                                eprintln!("[zerodds-ws-bridged] inbound err conn={conn_id}: {e}");
                            }
                        }
                        Opcode::Text | Opcode::Binary => {
                            let payload = frame.payload;
                            metrics.frames_in_total.inc();
                            metrics.bytes_in_total.add(payload.len() as u64);
                            let result = handle_inbound_frame(
                                &payload, conn_id, json_mode, &router, &writers, &topics, &types,
                                &runtime, &metrics, &security, &subject, &writer,
                            );
                            if let Err(e) = result {
                                metrics.errors_total.inc();
                                eprintln!("[zerodds-ws-bridged] inbound err conn={conn_id}: {e}");
                            }
                        }
                        Opcode::Ping => {
                            let pong = Frame::pong(frame.payload);
                            if let Ok(b) = encode(&pong) {
                                let _ = writer.write_all(&b);
                            }
                        }
                        Opcode::Pong => {}
                        Opcode::Close => break 'reader,
                        _ => {}
                    }
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(_) => break,
        }
    }

    // 5. Cleanup.
    if let Ok(mut r) = router.lock() {
        r.deregister_connection(conn_id);
    }
    let _ = writer_thread.join();
    drop(conn_guard);
    Ok(())
}

/// A connection is either plain TCP or TLS-wrapped.
#[cfg(feature = "daemon")]
enum WsStream {
    /// Plain `TcpStream` — `tls_enabled=false`.
    Plain(TcpStream),
    /// Server-side TLS stream on an accepted connection.
    Tls(Box<StreamOwned<ServerConnection, TcpStream>>),
}

#[cfg(feature = "daemon")]
type SharedTlsStream = Arc<Mutex<Box<StreamOwned<ServerConnection, TcpStream>>>>;

/// Reader half of an upgraded WebSocket connection.
#[cfg(feature = "daemon")]
enum WsReader {
    /// Dedicated `TcpStream` for reading — no user-space lock shared with [`WsWriter`].
    Plain(TcpStream),
    /// Cloned `TcpStream` used to `peek()` without holding the TLS session lock,
    /// plus the shared `StreamOwned` locked only once kernel bytes are ready.
    Tls {
        wait_sock: TcpStream,
        stream: SharedTlsStream,
    },
}

/// Writer half of an upgraded WebSocket connection.
#[cfg(feature = "daemon")]
enum WsWriter {
    /// Cloned `TcpStream` serialized only among outbound frame writers.
    Plain(Mutex<TcpStream>),
    /// Shared TLS stream.
    Tls(SharedTlsStream),
}

#[cfg(feature = "daemon")]
impl WsStream {
    fn set_nodelay(&self, nodelay: bool) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.set_nodelay(nodelay),
            Self::Tls(s) => s.sock.set_nodelay(nodelay),
        }
    }

    fn set_read_timeout(&mut self, dur: Option<Duration>) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.set_read_timeout(dur),
            Self::Tls(s) => s.sock.set_read_timeout(dur),
        }
    }

    fn into_split(self) -> std::io::Result<(WsReader, Arc<WsWriter>)> {
        match self {
            Self::Plain(tcp) => {
                let writer_tcp = tcp.try_clone()?;
                Ok((
                    WsReader::Plain(tcp),
                    Arc::new(WsWriter::Plain(Mutex::new(writer_tcp))),
                ))
            }
            Self::Tls(tls) => {
                let wait_sock = tls.sock.try_clone()?;
                let shared = Arc::new(Mutex::new(tls));
                Ok((
                    WsReader::Tls {
                        wait_sock,
                        stream: Arc::clone(&shared),
                    },
                    Arc::new(WsWriter::Tls(shared)),
                ))
            }
        }
    }
}

#[cfg(feature = "daemon")]
impl WsReader {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(s) => s.read(b),
            Self::Tls { wait_sock, stream } => {
                // 1. Drain any already-decrypted plaintext buffered in rustls
                //    without touching the socket.
                {
                    let mut guard = match stream.lock() {
                        Ok(g) => g,
                        Err(p) => p.into_inner(),
                    };
                    match guard.conn.reader().read(b) {
                        Ok(n) => return Ok(n),
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                        Err(e) => return Err(e),
                    }
                }
                // 2. Wait for incoming TCP bytes (or EOF/timeout) WITHOUT holding
                //    the TLS mutex so outbound writes never block on `read_timeout`.
                let mut peek_buf = [0u8; 1];
                let _ = wait_sock.peek(&mut peek_buf)?;

                // 3. Bytes (or EOF) are ready in the kernel receive buffer; read
                //    and decrypt them under the TLS session lock.
                let mut guard = match stream.lock() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                let StreamOwned { conn, sock } = &mut **guard;
                let n_tls = conn.read_tls(sock)?;
                let state = conn
                    .process_new_packets()
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                if state.tls_bytes_to_write() > 0 {
                    let _ = conn.write_tls(sock);
                    let _ = sock.flush();
                }
                if n_tls == 0 {
                    return match conn.reader().read(b) {
                        Ok(n) => Ok(n),
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
                        Err(e) => Err(e),
                    };
                }
                conn.reader().read(b)
            }
        }
    }
}

#[cfg(feature = "daemon")]
impl WsWriter {
    fn write_all(&self, b: &[u8]) -> std::io::Result<()> {
        match self {
            Self::Plain(m) => {
                let mut guard = match m.lock() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                guard.write_all(b)
            }
            Self::Tls(m) => {
                let mut guard = match m.lock() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                guard.write_all(b)?;
                guard.flush()
            }
        }
    }
}

#[cfg(feature = "daemon")]
impl Read for WsStream {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(s) => s.read(b),
            Self::Tls(s) => s.read(b),
        }
    }
}

#[cfg(feature = "daemon")]
impl Write for WsStream {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(s) => s.write(b),
            Self::Tls(s) => s.write(b),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.flush(),
            Self::Tls(s) => s.flush(),
        }
    }
}

/// RAII guard that decrements `connections_active` on drop.
#[cfg(feature = "daemon")]
struct ConnectionLifetime {
    active: Arc<zerodds_monitor::Gauge>,
}

#[cfg(feature = "daemon")]
impl Drop for ConnectionLifetime {
    fn drop(&mut self) {
        self.active.dec();
    }
}

/// A text frame from the client: a control message of §4.4 or a sample
/// frame of §4.3. What the daemon cannot do with it is answered with an
/// error frame (§4.4), never dropped.
#[cfg(feature = "daemon")]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn handle_inbound_frame(
    payload: &[u8],
    conn_id: u64,
    json_mode: bool,
    router: &Arc<Mutex<Router>>,
    writers: &Arc<std::collections::BTreeMap<String, EntityId>>,
    topics: &[TopicConfig],
    types: &std::collections::BTreeMap<String, TopicType>,
    runtime: &Arc<DcpsRuntime>,
    metrics: &BridgeMetrics,
    security: &Arc<zerodds_bridge_security::LiveSecurity>,
    subject: &AuthSubject,
    writer: &WsWriter,
) -> Result<(), String> {
    let refuse = |code: u16, message: String| {
        metrics.errors_total.inc();
        send_text_frame(writer, &json_frames::error(code, &message));
        Err(message)
    };
    let text = match core::str::from_utf8(payload) {
        Ok(t) => t,
        Err(e) => {
            return refuse(
                400,
                alloc_format(format_args!("a text frame is UTF-8: {e}")),
            );
        }
    };
    let message = match json_frames::parse_client(text) {
        Ok(m) => m,
        Err(e) => return refuse(400, e),
    };
    match message {
        ClientMessage::Ping => {
            send_text_frame(writer, &json_frames::pong());
            Ok(())
        }
        ClientMessage::Unsubscribe(names) => {
            if let Ok(mut r) = router.lock() {
                for topic in &names {
                    r.unsubscribe(conn_id, topic);
                }
            }
            Ok(())
        }
        ClientMessage::Subscribe(names) => {
            let mut taken = Vec::new();
            for topic in names {
                if !authorize(&security.current().acl, subject, AclOp::Read, &topic) {
                    let _ = refuse(
                        403,
                        alloc_format(format_args!(
                            "{}: read on {topic} is not granted",
                            subject.name
                        )),
                    );
                    continue;
                }
                let bridged_out = topics
                    .iter()
                    .any(|t| t.name == topic && matches!(t.direction.as_str(), "out" | "bidir"));
                if !bridged_out {
                    let _ = refuse(
                        404,
                        alloc_format(format_args!("{topic} is not bridged out of DDS")),
                    );
                    continue;
                }
                if json_mode && !types.contains_key(&topic) {
                    metrics.errors_total.inc();
                    send_text_frame(writer, &no_type_error(&topic));
                    continue;
                }
                taken.push(topic);
            }
            if let Ok(mut r) = router.lock() {
                for topic in &taken {
                    r.subscribe(conn_id, topic.clone());
                }
            }
            send_text_frame(writer, &json_frames::subscribed(&taken));
            Ok(())
        }
        ClientMessage::Sample(frame) => {
            let topic = frame.topic.clone();
            if !authorize(&security.current().acl, subject, AclOp::Write, &topic) {
                return refuse(
                    403,
                    alloc_format(format_args!(
                        "{}: write on {topic} is not granted",
                        subject.name
                    )),
                );
            }
            let Some(eid) = writers.get(&topic).copied() else {
                return refuse(
                    404,
                    alloc_format(format_args!("{topic} is not bridged into DDS")),
                );
            };
            let Some(ty) = types.get(&topic) else {
                metrics.errors_total.inc();
                send_text_frame(writer, &no_type_error(&topic));
                return Err(alloc_format(format_args!("{topic} has no type")));
            };
            let source_ts = match frame.timestamp_ns {
                None => None,
                Some(ns) => match json_frames::time_of(ns) {
                    Some((seconds, fraction)) => {
                        Some(zerodds_rtps::header_extension::HeTimestamp { seconds, fraction })
                    }
                    None => {
                        return refuse(
                            400,
                            alloc_format(format_args!(
                                "timestamp_ns {ns} is no time of DDSI-RTPS 2.5 §9.3.2"
                            )),
                        );
                    }
                },
            };
            match write_sample_frame(runtime, eid, ty, &frame, source_ts) {
                Ok(()) => {
                    if frame.op != SampleOp::Register {
                        metrics.dds_samples_in_total.inc();
                    }
                    Ok(())
                }
                Err((code, message)) => {
                    refuse(code, alloc_format(format_args!("{topic}: {message}")))
                }
            }
        }
    }
}

/// Writes a client's sample frame (§4.3) into DDS: a `publish` as a sample
/// of the topic's type, under XCDR2 little endian in the format of its
/// extensibility, keyed by its KeyHash for a keyed type; a `dispose` or an
/// `unregister` as the marker of the instance its `key_hash` (or the key
/// members of its `data`) names; a `register` has no message on the wire
/// (DDS 1.4 §2.2.2.4.2.5) and writes nothing.
#[cfg(feature = "daemon")]
fn write_sample_frame(
    runtime: &Arc<DcpsRuntime>,
    eid: EntityId,
    ty: &TopicType,
    frame: &SampleFrame,
    source_ts: Option<zerodds_rtps::header_extension::HeTimestamp>,
) -> Result<(), (u16, String)> {
    use zerodds_rtps::inline_qos::status_info::{DISPOSED, UNREGISTERED};
    let write_err =
        |e: zerodds_dcps::DdsError| (500, alloc_format(format_args!("dds-write: {e:?}")));
    match frame.op {
        SampleOp::Register => Ok(()),
        SampleOp::Publish => {
            let data = frame
                .data
                .as_ref()
                .ok_or((400, "publish carries data".to_string()))?;
            let encoded = ty.encode(data).map_err(|e| (422, e))?;
            runtime
                .set_user_writer_representation_identifier(eid, encoded.identifier)
                .map_err(write_err)?;
            match (encoded.key_hash, source_ts) {
                (Some(hash), ts) => runtime.write_user_sample_keyed(eid, &encoded.body, hash, ts),
                (None, Some(ts)) => runtime.write_user_sample_stamped(eid, &encoded.body, ts),
                (None, None) => runtime.write_user_sample_borrowed(eid, &encoded.body),
            }
            .map_err(write_err)
        }
        SampleOp::Dispose | SampleOp::Unregister => {
            if !ty.keyed() {
                return Err((
                    400,
                    "a topic without key has no instance to dispose or unregister".into(),
                ));
            }
            let key_hash = match (frame.key_hash, &frame.data) {
                (Some(hash), _) => hash,
                (None, Some(data)) => ty.key_hash(data).map_err(|e| (422, e))?,
                (None, None) => return Err((400, "names its instance by key_hash or data".into())),
            };
            let bits = if frame.op == SampleOp::Dispose {
                DISPOSED
            } else {
                UNREGISTERED
            };
            runtime
                .write_user_lifecycle(eid, key_hash, None, bits, source_ts)
                .map_err(write_err)
        }
    }
}

/// The error frame for a JSON-mode client of a topic without `idl_file`
/// (§3): its samples cannot be JSON without the type.
#[cfg(feature = "daemon")]
fn no_type_error(topic: &str) -> String {
    json_frames::error(
        415,
        &alloc_format(format_args!(
            "{topic} has no idl_file: the JSON text frames need the topic's type"
        )),
    )
}

/// The text frames of a DDS sample for a JSON-mode client (§4.3): a
/// `publish` with the sample as JSON, a `dispose`, an `unregister` (both
/// for a marker that is both).
///
/// # Errors
/// The error frame (§4.4) for a topic without type or a sample that does
/// not decode as its type.
#[cfg(feature = "daemon")]
fn json_sample_frames(
    topic: &str,
    sample: &DdsSample,
    ty: Option<&TopicType>,
) -> Result<Vec<String>, String> {
    let frame = |op: SampleOp, data: Option<serde_json::Value>| SampleFrame {
        topic: topic.to_string(),
        op,
        key_hash: sample.key_hash,
        data,
        timestamp_ns: sample.timestamp_ns,
    };
    let frames = match sample.kind {
        SampleKind::Alive => {
            let ty = ty.ok_or_else(|| no_type_error(topic))?;
            let data = ty
                .decode(&sample.payload, sample.representation_identifier)
                .map_err(|e| {
                    json_frames::error(
                        422,
                        &alloc_format(format_args!("a sample of {topic} does not decode: {e}")),
                    )
                })?;
            vec![frame(SampleOp::Publish, Some(data))]
        }
        SampleKind::Disposed => vec![frame(SampleOp::Dispose, None)],
        SampleKind::Unregistered => vec![frame(SampleOp::Unregister, None)],
        SampleKind::DisposedUnregistered => vec![
            frame(SampleOp::Dispose, None),
            frame(SampleOp::Unregister, None),
        ],
    };
    Ok(frames.iter().map(SampleFrame::render).collect())
}

#[cfg(feature = "daemon")]
fn send_text_frame(writer: &WsWriter, text: &str) {
    let frame = Frame::text(text.to_string());
    if let Ok(b) = encode(&frame) {
        let _ = writer.write_all(&b);
    }
}

/// The subprotocol of the binary frames (zerodds-ws-bridge-1.0 §4.1). A
/// subprotocol is a token (RFC 6455 §4.1, the `token` of RFC 7230 §3.2.6):
/// a `/` is a separator and no character of one.
const SUBPROTOCOL_BINARY: &str = "zerodds-ws-bridge-1.0";
/// The subprotocol of the JSON text frames (§4.3).
const SUBPROTOCOL_JSON: &str = "zerodds-ws-bridge-1.0+json";
/// `ZDB1`, the magic of a binary frame (§4.3).
const BINARY_MAGIC: [u8; 4] = *b"ZDB1";
/// Flag bit 0 of a binary frame: a dispose (§4.3).
const FLAG_DISPOSE: u8 = 0x01;
/// Flag bit 2 of a binary frame: an unregister (§4.3).
const FLAG_UNREGISTER: u8 = 0x04;
/// The representation identifier of a key holder, PLAIN_CDR2 big endian
/// (DDS-XTypes 1.3 §7.6.8): the payload of a dispose or an unregister.
const KEY_HOLDER_IDENTIFIER: [u8; 2] = [0x00, 0x06];

/// A binary frame of §4.3: `ZDB1`, the flags octet (bit 0 a dispose, bit 2
/// an unregister), the encapsulation header the payload came under
/// (DDSI-RTPS 2.5 §10.5: its representation identifier, which names
/// version, format and byte order, and options), the CDR bytes as they
/// travelled over RTPS; for a dispose or an unregister the key holder.
fn binary_sample_frame(sample: &DdsSample) -> Vec<u8> {
    let flags = match sample.kind {
        SampleKind::Alive => 0,
        SampleKind::Disposed => FLAG_DISPOSE,
        SampleKind::Unregistered => FLAG_UNREGISTER,
        SampleKind::DisposedUnregistered => FLAG_DISPOSE | FLAG_UNREGISTER,
    };
    let identifier = sample.representation_identifier;
    let mut out = Vec::with_capacity(sample.payload.len() + 9);
    out.extend_from_slice(&BINARY_MAGIC);
    out.push(flags);
    out.extend_from_slice(&[identifier[0], identifier[1], 0, 0]);
    out.extend_from_slice(&sample.payload);
    out
}

/// The flags, the representation identifier and the CDR bytes of a binary
/// frame of §4.3.
///
/// # Errors
/// A frame shorter than magic, flags and header, or a header that names no
/// CDR representation (DDSI-RTPS 2.5 §10.5, DDS-XTypes 1.3 Table 60).
fn split_binary_sample(frame: &[u8]) -> Result<(u8, [u8; 2], &[u8]), String> {
    if frame.len() < 9 || frame[..4] != BINARY_MAGIC {
        return Err("a binary frame shorter than ZDB1, flags and encapsulation header".into());
    }
    let identifier = [frame[5], frame[6]];
    match u16::from_be_bytes(identifier) {
        0x0000..=0x0003 | 0x0006..=0x000b => Ok((frame[4], identifier, &frame[9..])),
        other => Err(alloc_format(format_args!(
            "encapsulation {other:#06x} is not a CDR representation"
        ))),
    }
}

/// How the DDS writer frames a body that came under `identifier` (DDS-XTypes
/// 1.3 Table 60): its data representation (`CDR`, `PL_CDR` are XCDR1, the
/// others XCDR2), the extensibility the format belongs to (a parameter list
/// is `@mutable`, a delimited body `@appendable`, anything else `@final`) and
/// whether it is big-endian (the even identifiers).
#[cfg(feature = "daemon")]
fn writer_form(identifier: [u8; 2]) -> (i16, zerodds_types::qos::ExtensibilityForRepr, bool) {
    use zerodds_rtps::publication_data::data_representation as dr;
    use zerodds_types::qos::ExtensibilityForRepr as Ext;
    let representation = if identifier[1] <= 0x03 {
        dr::XCDR
    } else {
        dr::XCDR2
    };
    let extensibility = match identifier[1] & !0x01 {
        0x02 | 0x0a => Ext::Mutable,
        0x08 => Ext::Appendable,
        _ => Ext::Final,
    };
    (representation, extensibility, identifier[1] & 0x01 == 0)
}

/// A binary frame from the client (§4.3) on a connection whose path names
/// a topic (§4.2): the sample is written into DDS with the representation
/// and byte order its header names. A client disposes or unregisters with a
/// JSON sample frame; a binary frame with those flags is refused with an
/// error frame, not dropped.
#[cfg(feature = "daemon")]
#[allow(clippy::too_many_arguments)]
fn handle_binary_sample(
    frame: &[u8],
    auto_topic: Option<&str>,
    writers: &Arc<std::collections::BTreeMap<String, EntityId>>,
    runtime: &Arc<DcpsRuntime>,
    metrics: &BridgeMetrics,
    security: &Arc<zerodds_bridge_security::LiveSecurity>,
    subject: &AuthSubject,
    writer: &WsWriter,
) -> Result<(), String> {
    let refuse = |code: u16, message: String| {
        send_text_frame(writer, &json_frames::error(code, &message));
        Err(message)
    };
    let Some(topic) = auto_topic else {
        return refuse(
            400,
            "a binary frame needs a connection whose path names a topic".into(),
        );
    };
    if !authorize(&security.current().acl, subject, AclOp::Write, topic) {
        return refuse(
            403,
            alloc_format(format_args!(
                "{}: write on {topic} is not granted",
                subject.name
            )),
        );
    }
    let Some(eid) = writers.get(topic) else {
        return refuse(
            404,
            alloc_format(format_args!("{topic} is not bridged into DDS")),
        );
    };
    let (flags, identifier, body) = match split_binary_sample(frame) {
        Ok(parts) => parts,
        Err(e) => return refuse(400, e),
    };
    if flags & (FLAG_DISPOSE | FLAG_UNREGISTER) != 0 {
        return refuse(
            400,
            alloc_format(format_args!(
                "{topic}: a client disposes or unregisters with a JSON sample frame"
            )),
        );
    }
    // The writer's header names what the frame's header named.
    let (offer, extensibility, big_endian) = writer_form(identifier);
    let _ = runtime.set_user_writer_data_rep_override(*eid, Some(vec![offer]));
    let _ = runtime.set_user_writer_byte_order_override(*eid, big_endian);
    let _ = runtime.set_user_writer_wire_extensibility(*eid, extensibility);
    runtime
        .write_user_sample(*eid, body.to_vec())
        .map_err(|e| alloc_format(format_args!("dds-write: {e:?}")))?;
    metrics.dds_samples_in_total.inc();
    Ok(())
}

fn alloc_format(args: core::fmt::Arguments<'_>) -> String {
    use core::fmt::Write as _;
    let mut s = String::new();
    let _ = s.write_fmt(args);
    s
}

/// One SIGHUP reload (see [`start_with_reload`]).
fn reload(
    load: Option<&ConfigLoader>,
    rot: Option<&zerodds_bridge_security::RotatingTlsConfig>,
    live: &zerodds_bridge_security::LiveSecurity,
    started: &DaemonConfig,
) {
    let new = match load.map(|l| l()) {
        Some(Ok(c)) => Some(c),
        Some(Err(e)) => {
            eprintln!("[{SERVICE_NAME}] reload refused: {e}");
            return;
        }
        None => None,
    };
    let ctx = match new.as_ref().map(ctx_from_daemon_config) {
        Some(Ok((ctx, _))) => Some(ctx),
        Some(Err(e)) => {
            eprintln!("[{SERVICE_NAME}] reload refused: {e}");
            return;
        }
        None => None,
    };
    if let Some(r) = rot {
        if let Err(e) = r.reload() {
            eprintln!("[{SERVICE_NAME}] reload refused: TLS certificate: {e}");
            return;
        }
    }
    let mut restart: Vec<&str> = Vec::new();
    if let Some(n) = &new {
        if n.tls_cert_file != started.tls_cert_file
            || n.tls_key_file != started.tls_key_file
            || n.tls_client_ca_file != started.tls_client_ca_file
        {
            restart.push("tls file paths");
        }
        if n.topics != started.topics {
            restart.push("topics");
        }
        if n.listen != started.listen || n.domain != started.domain {
            restart.push("listen/domain");
        }
    }
    if let Some(ctx) = ctx {
        live.replace(ctx);
    }
    eprintln!(
        "[{SERVICE_NAME}] reloaded: {}",
        if new.is_some() {
            "acl, auth, tls certificate"
        } else {
            "tls certificate"
        }
    );
    if !restart.is_empty() {
        eprintln!(
            "[{SERVICE_NAME}] reload needs a restart for: {}",
            restart.join(", ")
        );
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// zerodds-ws-bridge-1.0 §4.1 with RFC 6455 §4.1: each subprotocol a
    /// client names is a token, characters of RFC 7230 §3.2.6 `tchar`
    /// (letters, digits and ``!#$%&'*+-.^_`|~``); a client library refuses
    /// any other (Python `websockets` raises "invalid subprotocol" for a `/`).
    #[test]
    fn rfc_6455_4_1_the_subprotocols_are_tokens() {
        for name in [SUBPROTOCOL_BINARY, SUBPROTOCOL_JSON] {
            assert!(
                !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~".contains(c)),
                "{name} is no token"
            );
        }
    }

    /// zerodds-ws-bridge-1.0 §4.3 with DDS-XTypes 1.3 §7.6.3.1.2, Table 60:
    /// a binary frame carries the encapsulation header the sample came
    /// under. Before, it was built from the version and the byte order
    /// alone, so a `PL_CDR2` or `D_CDR2` sample went out as `CDR2`
    /// (OPEN-ITEMS 60).
    #[test]
    fn xtypes_7_6_3_1_2_a_binary_frame_carries_the_identifier_the_sample_came_under() {
        for identifier in [[0x00, 0x01], [0x00, 0x03], [0x00, 0x08], [0x00, 0x0b]] {
            let frame = binary_sample_frame(&DdsSample::alive(vec![1, 2, 3, 4], identifier));
            assert_eq!(
                frame[..9],
                [
                    b'Z',
                    b'D',
                    b'B',
                    b'1',
                    0,
                    identifier[0],
                    identifier[1],
                    0,
                    0
                ]
            );
            let (flags, back, body) = split_binary_sample(&frame).unwrap();
            assert_eq!((flags, back, body), (0, identifier, &[1u8, 2, 3, 4][..]));
        }
        // XML (`00 04`) is no CDR representation.
        assert!(split_binary_sample(b"ZDB1\x00\x00\x04\x00\x00").is_err());
    }

    /// DDS-XTypes 1.3 Table 60: the identifier of a client's frame names the
    /// data representation, the extensibility and the byte order the DDS
    /// writer gives the body.
    #[cfg(feature = "daemon")]
    #[test]
    fn xtypes_7_6_3_1_2_the_identifier_names_the_form_of_the_dds_writer() {
        use zerodds_types::qos::ExtensibilityForRepr as Ext;
        for (identifier, form) in [
            ([0x00, 0x00], (0, Ext::Final, true)),
            ([0x00, 0x01], (0, Ext::Final, false)),
            ([0x00, 0x02], (0, Ext::Mutable, true)),
            ([0x00, 0x03], (0, Ext::Mutable, false)),
            ([0x00, 0x06], (2, Ext::Final, true)),
            ([0x00, 0x07], (2, Ext::Final, false)),
            ([0x00, 0x08], (2, Ext::Appendable, true)),
            ([0x00, 0x09], (2, Ext::Appendable, false)),
            ([0x00, 0x0a], (2, Ext::Mutable, true)),
            ([0x00, 0x0b], (2, Ext::Mutable, false)),
        ] {
            assert_eq!(writer_form(identifier), form, "{identifier:02x?}");
        }
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
            "/../../man/man5/zerodds-ws-bridged.yaml.5"
        ))
        .unwrap_or_else(|e| panic!("man5 page: {e}"));
        let named = format!("Default\n.IR \"{DEFAULT_METRICS_LISTEN}\"\nwhen");
        assert!(
            page.contains(&named),
            "man5 does not name {DEFAULT_METRICS_LISTEN}"
        );
    }
}
