// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! WebSocket server + DDS pump for the `zerodds-ws-bridged` daemon.
//!
//! Spec: `zerodds-ws-bridge-1.0.md` §4 + §9.
//!
//! The daemon logs through [`super::log`]: one JSON object per line on
//! stdout (Spec §8.1).
//!
//! Sync, blocking I/O on `std::net`. One reader thread per connection
//! (TCP→WS frames→router) plus one writer thread
//! (router channel→WS frames→TCP). The DDS pump thread consumes
//! `mpsc::Receiver<UserSample>` from each registered reader and
//! dispatches via the `Router`.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::connection::{Event, Failure, Limits, Receiver, Sender};
use crate::handshake::{build_server_response, parse_client_request, render_server_response};
use crate::masking::Role;
use crate::message::SendError;
use crate::origin::OriginPolicy;
use crate::permessage_deflate;

use super::config::{DaemonConfig, TopicConfig};
use super::json_frames::{self, ClientMessage, SampleFrame, SampleOp, TopicType};
use super::log::{self, Level};
use super::pump::{
    BINARY_MAGIC, binary_sample_frame, check_client_flags, dds_sample_of, json_sample_frames,
    no_type_error, split_binary_sample, writer_form,
};
use super::router::{ConnQueue, DdsSample, MAX_QUEUED_CONTROLS, Router, RouterMsg};
#[cfg(feature = "daemon")]
use super::runtime_common::{
    BridgeMetrics, CatalogSnapshot, SERVICE_NAME, auth_reason, health_response, http_response,
    install_signal_watcher, otlp_config_from_env, serve_admin_endpoints, spawn_otlp_flush_loop,
};
#[cfg(feature = "daemon")]
use super::security::{
    AclOp, AuthError, AuthSubject, Refusal, authenticate_ws, authorize, ctx_from_daemon_config,
    extract_authorization_header, render_refusal, serve_tls_handshake,
};
#[cfg(feature = "daemon")]
use rustls::{ServerConnection, StreamOwned};
#[cfg(feature = "daemon")]
use zerodds_monitor::Registry;
#[cfg(feature = "daemon")]
use zerodds_observability_otlp::OtlpExporter;

#[cfg(feature = "daemon")]
use super::trace::{ConnectionSpan, FrameIn, FrameOut, Tracer, unix_ns, unix_ns_since};

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
    /// Ends every thread: set when the drain of a shutdown is over.
    stop: Arc<AtomicBool>,
    /// SIGTERM or SIGINT arrived; the binary runs [`Self::shutdown`].
    shutdown_requested: Arc<AtomicBool>,
    /// The shutdown drains (§9.2): no connection is accepted, the pumps
    /// hand on what their DataReaders received and end.
    draining: Arc<AtomicBool>,
    /// `shutdown_drain_ms`: the bound of the drain.
    drain_bound: Duration,
    accept_thread: Option<JoinHandle<()>>,
    pump_threads: Vec<JoinHandle<()>>,
    /// The connection threads that still run; the shutdown waits for them.
    #[cfg(feature = "daemon")]
    connections: Arc<ConnectionThreads>,
    #[cfg(feature = "daemon")]
    admin_thread: Option<JoinHandle<()>>,
    #[cfg(feature = "daemon")]
    otlp_thread: Option<JoinHandle<()>>,
    /// The spans of the connections (§8.3): sent once more when the last
    /// connection ended.
    #[cfg(feature = "daemon")]
    tracer: Tracer,
    /// The participants of `domain` and of the tenants (§1 L6).
    #[cfg(feature = "daemon")]
    cores: Arc<Cores>,
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
    /// Tenant name → the DCPS runtime of its participant (§1 L6).
    #[cfg(feature = "daemon")]
    pub tenant_runtimes: std::collections::BTreeMap<String, Arc<DcpsRuntime>>,
}

impl DaemonHandle {
    /// Whether SIGTERM/SIGINT (or [`Self::shutdown`]) asked the daemon to
    /// stop. The binary waits for it and then runs the shutdown.
    #[must_use]
    pub fn stop_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::SeqCst) || self.stop.load(Ordering::SeqCst)
    }

    /// Shuts the daemon down in the order of zerodds-ws-bridge-1.0 §9.2,
    /// within `shutdown_drain_ms`: no new connection is accepted; the
    /// samples the DataReaders received are handed to the send queues;
    /// every open WebSocket connection sends what its queue holds, then
    /// `{"op":"shutdown"}` and a Close frame with 1001 (RFC 6455 §7.4.1,
    /// "going away"), and is closed the way §4.5 says. When the bound
    /// passes, the connections still open are closed with 1001 at once and
    /// waited for at most [`SHUTDOWN_CLOSE_WAIT`]. The DDS participant
    /// stops last.
    pub fn shutdown(&mut self) {
        // Logged once: the binary shuts down, then the drop runs it again.
        if self.accept_thread.is_some() {
            log::event(Level::Info, "shutdown").emit();
        }
        let deadline = Instant::now() + self.drain_bound;
        self.draining.store(true, Ordering::SeqCst);
        #[cfg(feature = "daemon")]
        {
            self.healthy.store(false, Ordering::SeqCst);
        }
        // Unblock accept().
        if let Ok(addr) = self.local_addr.parse::<std::net::SocketAddr>() {
            // Self-connect to wake accept().
            let _ = TcpStream::connect_timeout(&addr, Duration::from_millis(200));
        }
        if let Some(j) = self.accept_thread.take() {
            let _ = j.join();
        }
        // The pumps hand on what their DataReaders hold and end; a RELIABLE
        // topic waits for room in a full queue until the bound.
        while !self.pump_threads.iter().all(JoinHandle::is_finished) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        // Behind the samples in each queue: `shutdown`, then Close 1001.
        #[cfg(feature = "daemon")]
        for core in std::iter::once(&self.cores.base).chain(&self.cores.tenants) {
            if let Ok(r) = core.router.lock() {
                r.broadcast_shutdown();
            }
        }
        // The accept thread has ended, so no connection thread is added.
        #[cfg(feature = "daemon")]
        {
            let left = deadline.saturating_duration_since(Instant::now());
            if !self.connections.wait_until_none(left) {
                log::event(Level::Warn, "shutdown_drain_bound")
                    .number(
                        "drain_ms",
                        u64::try_from(self.drain_bound.as_millis()).unwrap_or(u64::MAX),
                    )
                    .emit();
            }
        }
        self.stop.store(true, Ordering::SeqCst);
        // Self-connect the admin server (so the accept loop unblocks).
        #[cfg(feature = "daemon")]
        if let Some(admin) = self.admin_addr.as_deref() {
            if let Ok(addr) = admin.parse::<std::net::SocketAddr>() {
                let _ = TcpStream::connect_timeout(&addr, Duration::from_millis(200));
            }
        }
        // What is left sees the stop flag, sends its Close 1001 and closes
        // its connection; the process must not end under them.
        #[cfg(feature = "daemon")]
        if !self.connections.wait_until_none(SHUTDOWN_CLOSE_WAIT) {
            log::event(Level::Warn, "shutdown_connections_left")
                .number(
                    "wait_ms",
                    u64::try_from(SHUTDOWN_CLOSE_WAIT.as_millis()).unwrap_or(u64::MAX),
                )
                .emit();
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
            // §8.3: the spans of the connections that ended after the
            // flush thread's last flush.
            self.tracer.flush();
            // Last, so no pump writes to a stopped runtime. Dropping the
            // handle alone would not stop it — the runtime's worker threads
            // hold `Arc`s of their own — and a daemon stopped in a process
            // that keeps running would leave its participant on the domain.
            self.runtime.shutdown();
            for runtime in self.tenant_runtimes.values() {
                runtime.shutdown();
            }
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
    // Spec §8.1: `log_level` is the threshold of the log.
    log::set_level(&cfg.log_level);
    log::event(Level::Info, "start")
        .field("listen", &cfg.listen)
        .integer("domain", i64::from(cfg.domain))
        .number("topics", cfg.topics.len() as u64)
        .emit();

    // 0. Metrics registry + standard counter set (§8.2).
    let registry = Arc::new(Registry::new());
    let metrics = BridgeMetrics::register(&registry).with_topics(&cfg.topics);

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
    // RFC 6455 §10.2 and §10.4: the origins the daemon expects and the
    // limits of a frame and a message, for every connection.
    let origin = cfg
        .origin_policy()
        .map_err(|e| ServerError::Config(alloc_format(format_args!("origin: {e}"))))?;
    if cfg.max_frame_size < super::config::MIN_MAX_FRAME_SIZE || cfg.max_message_size == 0 {
        return Err(ServerError::Config(alloc_format(format_args!(
            "limits: max_frame_size is at least {} and max_message_size at least 1, got {} and {}",
            super::config::MIN_MAX_FRAME_SIZE,
            cfg.max_frame_size,
            cfg.max_message_size
        ))));
    }
    if origin.is_checking() {
        log::event(Level::Info, "origin_check")
            .field("allowed", &cfg.origin_allowed.join(" "))
            .field(
                "required",
                if cfg.origin_required { "true" } else { "false" },
            )
            .emit();
    }
    if rotating_tls.is_some() {
        log::event(Level::Info, "tls_active")
            .field("cert_file", &cfg.tls_cert_file)
            .field(
                "mtls",
                if cfg.tls_client_ca_file.is_empty() {
                    "false"
                } else {
                    "true"
                },
            )
            .emit();
    }
    log::event(Level::Info, "auth")
        .field("mode", &cfg.auth_mode)
        .number("acl_entries", cfg.topic_acl.len() as u64)
        .emit();

    // 1. The DCPS participant of `domain` with the top-level topics, and
    //    one participant of each tenant with its topics (§1 L6).
    let healthy = Arc::new(AtomicBool::new(true));
    let stop = Arc::new(AtomicBool::new(false));
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let draining = Arc::new(AtomicBool::new(false));
    let reload_flag = Arc::new(AtomicBool::new(false));
    let mut pump_threads = Vec::new();
    let base = Arc::new(build_core(
        None,
        &cfg,
        runtime_config.clone(),
        metrics.clone(),
        &stop,
        &draining,
        &mut pump_threads,
    )?);
    let mut tenants = Vec::with_capacity(cfg.tenants.len());
    for tenant in &cfg.tenants {
        match build_core(
            Some(tenant),
            &cfg,
            runtime_config.clone(),
            metrics.for_tenant(&tenant.name, &tenant.topics),
            &stop,
            &draining,
            &mut pump_threads,
        ) {
            Ok(core) => tenants.push(Arc::new(core)),
            Err(e) => {
                abort_start(&stop, &base, &tenants);
                return Err(e);
            }
        }
    }
    let cores = Arc::new(Cores { base, tenants });
    // The admin endpoint's catalog (§4.2): the topics of `domain`.
    let catalog = Arc::clone(&cores.base.catalog);
    // §8.3: with `OTEL_EXPORTER_OTLP_ENDPOINT` the spans of the
    // connections go to an OTLP exporter (its flush thread starts below).
    let otlp_exporter = otlp_config_from_env(SERVICE_NAME).map(|c| Arc::new(OtlpExporter::new(c)));
    let tracer = Tracer::new(otlp_exporter.clone());
    let connection_settings = Arc::new(ConnectionSettings {
        limits: cfg.limits(),
        origin,
        healthy: Arc::clone(&healthy),
        max_send_queue: cfg.max_send_queue_per_connection,
        ping_interval: (cfg.ping_interval_ms > 0)
            .then(|| Duration::from_millis(cfg.ping_interval_ms)),
        draining: Arc::clone(&draining),
        catalog_on_connect: cfg.catalog_on_connect,
        tracer: tracer.clone(),
    });

    // 3. The TCP listener.
    let listener = match TcpListener::bind(&cfg.listen) {
        Ok(l) => l,
        Err(e) => {
            abort_start(&stop, &cores.base, &cores.tenants);
            return Err(ServerError::Bind(alloc_format(format_args!("{e}"))));
        }
    };
    let local_addr = listener
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| cfg.listen.clone());
    if let Err(e) = listener.set_nonblocking(false) {
        abort_start(&stop, &cores.base, &cores.tenants);
        return Err(ServerError::Io(alloc_format(format_args!("{e}"))));
    }

    log::event(Level::Info, "bound")
        .field("peer", &local_addr)
        .emit();

    // 5. Accept thread.
    let next_conn_id = Arc::new(AtomicU64::new(1));
    let stop_acc = Arc::clone(&stop);
    let cores_acc = Arc::clone(&cores);
    let metrics_acc = metrics.clone();
    let security_acc = Arc::clone(&security_ctx);
    let rotating_acc = rotating_tls.clone();
    let connections = Arc::new(ConnectionThreads::default());
    let connections_acc = Arc::clone(&connections);
    let settings_acc = Arc::clone(&connection_settings);

    let draining_acc = Arc::clone(&draining);
    let accept_thread = thread::spawn(move || {
        for incoming in listener.incoming() {
            if stop_acc.load(Ordering::SeqCst) || draining_acc.load(Ordering::SeqCst) {
                break;
            }
            match incoming {
                Ok(tcp) => {
                    // §8.2: every accepted TCP connection, also one whose TLS
                    // handshake then fails.
                    metrics_acc.connections_total.inc();
                    let conn_id = next_conn_id.fetch_add(1, Ordering::SeqCst);
                    let peer = tcp.peer_addr().map(|a| a.to_string()).unwrap_or_default();
                    let cores_h = Arc::clone(&cores_acc);
                    let stop_h = Arc::clone(&stop_acc);
                    let metrics_h = metrics_acc.clone();
                    let security_h = Arc::clone(&security_acc);
                    let rot_h = rotating_acc.clone();
                    let settings_h = Arc::clone(&settings_acc);
                    // Counted here, not in the thread: the shutdown reads
                    // the count once this accept thread has ended.
                    let running = connections_acc.enter();
                    thread::spawn(move || {
                        let _running = running;
                        // If TLS is configured: rustls handshake. Otherwise plain.
                        let (stream, mtls_subj) = if let Some(rot) = rot_h.as_ref() {
                            let cfg = rot.current();
                            match serve_tls_handshake(cfg, tcp, Duration::from_secs(5)) {
                                Ok((tcp, conn, subj)) => {
                                    (WsStream::Tls(Box::new(StreamOwned::new(conn, tcp))), subj)
                                }
                                Err(e) => {
                                    metrics_h.errors_total.inc();
                                    log::event(Level::Warn, "tls_handshake_failed")
                                        .number("connection_id", conn_id)
                                        .field("peer", &peer)
                                        .field("reason", &e.to_string())
                                        .emit();
                                    return;
                                }
                            }
                        } else {
                            (WsStream::Plain(tcp), None)
                        };
                        let conn = Arc::new(ConnLog { id: conn_id, peer });
                        let _ = serve_connection(
                            conn, stream, mtls_subj, cores_h, stop_h, metrics_h, security_h,
                            settings_h,
                        );
                    });
                }
                Err(e) => {
                    log::event(Level::Error, "accept_failed")
                        .field("reason", &e.to_string())
                        .emit();
                    continue;
                }
            }
        }
    });

    // 6. Admin endpoint (§8.2 Prometheus at `metrics.path`, §5.2
    //    catalog/healthz).
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
                match serve_admin_endpoints(
                    sock,
                    cfg.metrics_path.clone(),
                    Arc::clone(&catalog),
                    Arc::clone(&registry),
                    Arc::clone(&healthy),
                    Arc::clone(&stop),
                ) {
                    Ok((h, bound)) => {
                        log::event(Level::Info, "admin_bound")
                            .field("peer", &bound.to_string())
                            .field("metrics_path", &cfg.metrics_path)
                            .emit();
                        admin_addr = Some(bound.to_string());
                        admin_thread = Some(h);
                    }
                    Err(e) => {
                        log::event(Level::Error, "admin_bind_failed")
                            .field("peer", &bind_str)
                            .field("reason", &e.to_string())
                            .emit();
                    }
                }
            }
            Err(e) => {
                log::event(Level::Error, "admin_bind_failed")
                    .field("peer", &bind_str)
                    .field("reason", &e.to_string())
                    .emit();
            }
        }
    }

    // 7. Signal watcher (§9.2 graceful shutdown).
    if let Err(e) =
        install_signal_watcher(Arc::clone(&shutdown_requested), Arc::clone(&reload_flag))
    {
        log::event(Level::Error, "signal_watcher_failed")
            .field("reason", &e.to_string())
            .emit();
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
    let otlp_thread = if let Some(exporter) = otlp_exporter {
        match spawn_otlp_flush_loop(exporter, Arc::clone(&stop), Duration::from_secs(5)) {
            Ok(h) => {
                log::event(Level::Info, "otlp_active").emit();
                Some(h)
            }
            Err(e) => {
                log::event(Level::Error, "otlp_failed")
                    .field("reason", &e.to_string())
                    .emit();
                None
            }
        }
    } else {
        None
    };

    Ok(DaemonHandle {
        stop,
        shutdown_requested,
        draining,
        drain_bound: Duration::from_millis(cfg.shutdown_drain_ms),
        accept_thread: Some(accept_thread),
        pump_threads,
        connections,
        admin_thread,
        otlp_thread,
        tracer,
        runtime: Arc::clone(&cores.base.runtime),
        user_writers: (*cores.base.writers).clone(),
        user_readers: cores.base.reader_eids.clone(),
        tenant_runtimes: cores
            .tenants
            .iter()
            .map(|c| (c.tenant.clone(), Arc::clone(&c.runtime)))
            .collect(),
        cores,
        local_addr,
        admin_addr,
        reload_flag,
        healthy,
        metrics: Some(metrics),
    })
}

/// The DCPS participant of `domain` (or of a tenant, §1 L6) with its
/// topics: their endpoints, router, types and catalog.
#[cfg(feature = "daemon")]
struct Core {
    /// The tenant's name; empty for the participant of `domain`.
    tenant: String,
    /// The subjects bound to the tenant; empty for `domain`.
    tenant_config: Option<super::config::TenantConfig>,
    runtime: Arc<DcpsRuntime>,
    /// Topic name → its DataWriter.
    writers: Arc<std::collections::BTreeMap<String, EntityId>>,
    /// Topic name → the lock of its DataWriter, which all connections
    /// share: held from setting the writer's representation to the write,
    /// so that a sample goes out under the header its client named (§4.3).
    write_locks: std::collections::BTreeMap<String, Mutex<()>>,
    /// Topic name → its DataReader.
    reader_eids: std::collections::BTreeMap<String, EntityId>,
    topics: Arc<Vec<TopicConfig>>,
    types: Arc<std::collections::BTreeMap<String, TopicType>>,
    router: Arc<Mutex<Router>>,
    /// The catalog of §5.2.
    catalog: Arc<CatalogSnapshot>,
    /// The metrics of its topics (§8.2: a tenant's under
    /// `<tenant>/<topic>`).
    metrics: BridgeMetrics,
}

#[cfg(feature = "daemon")]
impl Core {
    /// The name of `topic` in the topic ACL (§7.3): a tenant's topic is
    /// `<tenant>/<topic>` (§3 `tenants`).
    fn acl_topic<'a>(&self, topic: &'a str) -> std::borrow::Cow<'a, str> {
        if self.tenant.is_empty() {
            std::borrow::Cow::Borrowed(topic)
        } else {
            std::borrow::Cow::Owned(super::config::tenant_acl_topic(&self.tenant, topic))
        }
    }

    /// Holds the DataWriter of `topic` for one write (see `write_locks`);
    /// `None` for a topic without one.
    fn hold_writer(&self, topic: &str) -> Option<std::sync::MutexGuard<'_, ()>> {
        self.write_locks.get(topic).map(|lock| {
            lock.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        })
    }

    /// Whether the topic ACL grants `subject` the operation `op` on this
    /// participant's `topic` (§7.3).
    fn allows(
        &self,
        security: &zerodds_bridge_security::LiveSecurity,
        subject: &AuthSubject,
        op: AclOp,
        topic: &str,
    ) -> bool {
        authorize(&security.current().acl, subject, op, &self.acl_topic(topic))
    }
}

/// The participants of the daemon (§1 L6).
#[cfg(feature = "daemon")]
struct Cores {
    /// The participant of `domain`.
    base: Arc<Core>,
    /// One per tenant, in the order of the configuration.
    tenants: Vec<Arc<Core>>,
}

#[cfg(feature = "daemon")]
impl Cores {
    /// The participant a connection of `subject` is bound to: the tenant
    /// that names the subject, else the one of `domain` (§1 L6).
    fn of(&self, subject: &AuthSubject) -> &Arc<Core> {
        self.tenants
            .iter()
            .find(|c| c.tenant_config.as_ref().is_some_and(|t| t.binds(subject)))
            .unwrap_or(&self.base)
    }
}

/// Stops what a start that fails has started: the pumps and the
/// participants.
#[cfg(feature = "daemon")]
fn abort_start(stop: &AtomicBool, base: &Core, tenants: &[Arc<Core>]) {
    stop.store(true, Ordering::SeqCst);
    base.runtime.shutdown();
    for core in tenants {
        core.runtime.shutdown();
    }
}

/// Starts the participant of `tenant` (`None`: the one of `domain`) with
/// its topics: their types, endpoints, router, pumps and the deadline
/// thread (§6, §9.1). A type that does not load or an endpoint that cannot
/// be created stops it again and fails the start.
#[cfg(feature = "daemon")]
#[allow(clippy::too_many_lines)]
fn build_core(
    tenant: Option<&super::config::TenantConfig>,
    cfg: &DaemonConfig,
    runtime_config: RuntimeConfig,
    metrics: BridgeMetrics,
    stop: &Arc<AtomicBool>,
    draining: &Arc<AtomicBool>,
    pump_threads: &mut Vec<JoinHandle<()>>,
) -> Result<Core, ServerError> {
    let (name, domain, topic_list) = match tenant {
        Some(t) => (t.name.as_str(), t.domain, t.topics.as_slice()),
        None => ("", cfg.domain, cfg.topics.as_slice()),
    };
    let in_tenant = |what: String| {
        if name.is_empty() {
            what
        } else {
            alloc_format(format_args!("tenant {name}: {what}"))
        }
    };
    // 2. The types of the topics with an `idl_file` (§3), which the JSON
    //    text frames (§4.3) read and write the samples with. A file that
    //    does not load stops the start.
    let mut types: std::collections::BTreeMap<String, TopicType> =
        std::collections::BTreeMap::new();
    for topic in topic_list {
        if topic.idl_file.is_empty() {
            continue;
        }
        let ty = TopicType::load(&topic.idl_file, &topic.type_name).map_err(|e| {
            ServerError::Config(in_tenant(alloc_format(format_args!(
                "topic {}: {e}",
                topic.name
            ))))
        })?;
        types.insert(topic.name.clone(), ty);
    }
    let types = Arc::new(types);

    // A participant of its own per tenant: its GUID prefix from the listen
    // address and the tenant's name.
    let seed = if name.is_empty() {
        cfg.listen.clone()
    } else {
        alloc_format(format_args!("{}#{name}", cfg.listen))
    };
    let prefix = zerodds_dcps::participant::bridge_guid_prefix(&seed);
    let runtime = DcpsRuntime::start(domain, prefix, runtime_config)
        .map_err(|e| ServerError::Dds(in_tenant(alloc_format(format_args!("{e:?}")))))?;

    // Register a reader+writer per topic.
    let mut writers: std::collections::BTreeMap<String, EntityId> =
        std::collections::BTreeMap::new();
    let mut reader_eids: std::collections::BTreeMap<String, EntityId> =
        std::collections::BTreeMap::new();
    let mut readers: Vec<(String, std::sync::mpsc::Receiver<UserSample>)> = Vec::new();
    for topic in topic_list {
        let (reader_eid, writer_eid) =
            match register_topic_endpoints(&runtime, topic, types.get(&topic.name)) {
                Ok(endpoints) => endpoints,
                Err(e) => {
                    runtime.shutdown();
                    return Err(e);
                }
            };
        if let Some((eid, rx)) = reader_eid {
            reader_eids.insert(topic.name.clone(), eid);
            readers.push((topic.name.clone(), rx));
        }
        if let Some(eid) = writer_eid {
            writers.insert(topic.name.clone(), eid);
        }
    }

    // The router. Each topic gets its behaviour of §6 from its QoS: what a
    // full send queue does, what it keeps for late subscribers, its
    // deadline.
    let mut router = Router::new().with_max_burst(cfg.max_burst_samples);
    for topic in topic_list {
        router.configure_topic(
            &topic.name,
            super::qos_translation::behavior_for(topic, cfg.queue_limits()),
        );
        router.set_keyed(
            &topic.name,
            types.get(&topic.name).is_some_and(TopicType::keyed),
        );
    }
    let router = Arc::new(Mutex::new(router));
    let stop = Arc::clone(stop);
    let draining = Arc::clone(draining);

    // 4. Pump threads per reader.
    for (topic_name, rx) in readers {
        let router_c = Arc::clone(&router);
        let stop_c = Arc::clone(&stop);
        let topic_name_c = topic_name.clone();
        let pump_metrics = metrics.clone();
        let errors = Arc::clone(&metrics.errors_total);
        let draining_c = Arc::clone(&draining);
        let h = thread::spawn(move || {
            let mut unknown_logged = false;
            while !stop_c.load(Ordering::SeqCst) {
                // §9.2: a shutdown drains what the DataReader holds, then
                // the pump ends.
                let next = if draining_c.load(Ordering::SeqCst) {
                    rx.try_recv().map_err(|e| match e {
                        std::sync::mpsc::TryRecvError::Empty => {
                            std::sync::mpsc::RecvTimeoutError::Disconnected
                        }
                        std::sync::mpsc::TryRecvError::Disconnected => {
                            std::sync::mpsc::RecvTimeoutError::Disconnected
                        }
                    })
                } else {
                    rx.recv_timeout(Duration::from_millis(200))
                };
                match next {
                    Ok(sample) => {
                        let Some(sample) = dds_sample_of(sample) else {
                            continue;
                        };
                        pump_metrics.dds_received(&topic_name_c);
                        // §6: routed under the lock, handed to the queues
                        // outside it; a RELIABLE topic waits here for a
                        // full queue, a BEST_EFFORT one drops its oldest.
                        let route = match router_c.lock() {
                            Ok(mut r) => r.route(&topic_name_c, sample),
                            Err(_) => break,
                        };
                        // §6: a sample of a topic with key without a
                        // KeyHash is delivered, but not kept and not
                        // counted for a deadline; each is an error in the
                        // metrics, the first one a log line.
                        if route.instance_unknown() {
                            errors.inc();
                            if !unknown_logged {
                                unknown_logged = true;
                                log::event(Level::Warn, "sample_without_key_hash")
                                    .field("topic", &topic_name_c)
                                    .field(
                                        "reason",
                                        "a sample without a key hash names no instance; it is not kept for late subscribers and starts no deadline (counted in zerodds_ws_bridge_errors_total)",
                                    )
                                    .emit();
                            }
                        }
                        let delivery = route.deliver(&stop_c);
                        for connection_id in delivery.dropped_on {
                            pump_metrics.send_queue_drop(connection_id);
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        pump_threads.push(h);
    }
    // §6: the deadline of the topics that have one.
    if topic_list.iter().any(|t| t.deadline_ms.is_some()) {
        let router_d = Arc::clone(&router);
        let stop_d = Arc::clone(&stop);
        let draining_d = Arc::clone(&draining);
        pump_threads.push(thread::spawn(move || {
            while !stop_d.load(Ordering::SeqCst) && !draining_d.load(Ordering::SeqCst) {
                thread::sleep(DEADLINE_TICK);
                match router_d.lock() {
                    Ok(mut r) => {
                        r.deadlines_missed(Instant::now());
                    }
                    Err(_) => break,
                }
            }
        }));
    }

    let catalog = Arc::new(CatalogSnapshot {
        topics: topic_list.to_vec(),
        ..CatalogSnapshot::from_config(cfg)
    });
    let write_locks = writers
        .keys()
        .map(|topic| (topic.clone(), Mutex::new(())))
        .collect();
    Ok(Core {
        tenant: name.to_string(),
        tenant_config: tenant.cloned(),
        runtime,
        writers: Arc::new(writers),
        write_locks,
        reader_eids,
        topics: Arc::new(topic_list.to_vec()),
        types,
        router,
        catalog,
        metrics,
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
    ty: Option<&TopicType>,
) -> Result<TopicEndpoints, ServerError> {
    let keyed = ty.is_some_and(TopicType::keyed);
    use zerodds_qos::DurabilityKind;
    let durability = match topic.durability.as_str() {
        "transient_local" => DurabilityKind::TransientLocal,
        "transient" => DurabilityKind::Transient,
        "persistent" => DurabilityKind::Persistent,
        _ => DurabilityKind::Volatile,
    };
    let reliable = !matches!(topic.reliability.as_str(), "best_effort");
    // §6: deadline, liveliness and partition of the topic, the same for
    // the DataReader and the DataWriter.
    let (writer_qos, reader_qos) = super::qos_translation::topic_qos(topic);
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
                    deadline: reader_qos.deadline,
                    liveliness: reader_qos.liveliness,
                    partition: reader_qos.partition.names.clone(),
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
                    deadline: writer_qos.deadline,
                    liveliness: writer_qos.liveliness,
                    partition: writer_qos.partition.names.clone(),
                    // §4.3: the JSON samples of a typed topic go out under
                    // XCDR2, the one representation the writer offers
                    // (DDS-XTypes 1.3 §7.6.3.1.1).
                    data_representation_offer: ty
                        .map(|_| vec![zerodds_rtps::publication_data::data_representation::XCDR2]),
                    ..Default::default()
                },
                keyed,
            )
            .map_err(|e| ServerError::Dds(alloc_format(format_args!("writer: {e:?}"))))?;
        // The header the JSON samples carry from the first one on: the
        // extensibility of the type and little endian.
        if let Some(ty) = ty {
            rt.set_user_writer_representation_identifier(eid, ty.identifier())
                .map_err(|e| ServerError::Dds(alloc_format(format_args!("writer: {e:?}"))))?;
        }
        Some(eid)
    } else {
        None
    };

    Ok((reader, writer))
}

/// What every connection of the daemon is held to, from the configuration
/// the daemon started with.
#[cfg(feature = "daemon")]
struct ConnectionSettings {
    /// `limits.*`: the largest frame and message of a client (RFC 6455
    /// §10.4).
    limits: Limits,
    /// `origin.*`: the origins the opening handshake may name (RFC 6455
    /// §10.2).
    origin: OriginPolicy,
    /// Whether the DCPS runtime runs: the answer of `/healthz` (§4.2).
    healthy: Arc<AtomicBool>,
    /// `max_send_queue_per_connection` (§6).
    max_send_queue: usize,
    /// `ping_interval_ms` (§6): `None` without Ping.
    ping_interval: Option<Duration>,
    /// The daemon shuts down (§9.2).
    draining: Arc<AtomicBool>,
    /// `catalog_on_connect` (§5.2).
    catalog_on_connect: bool,
    /// The OTLP spans of the connections (§8.3).
    tracer: Tracer,
}

#[cfg(feature = "daemon")]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn serve_connection(
    conn: Arc<ConnLog>,
    mut stream: WsStream,
    mtls_subject: Option<AuthSubject>,
    cores: Arc<Cores>,
    stop: Arc<AtomicBool>,
    metrics: BridgeMetrics,
    security: Arc<zerodds_bridge_security::LiveSecurity>,
    settings: Arc<ConnectionSettings>,
) -> Result<(), ServerError> {
    let conn_id = conn.id;
    // §8.3: the root span of the connection, from the start of its opening
    // handshake; sent for an opening handshake the daemon refused and for a
    // WebSocket connection when it ends.
    let span = settings.tracer.connection(unix_ns());
    if let Some(span) = &span {
        span.set("net.peer", &conn.peer);
        span.set("ws.connection_id", &conn_id.to_string());
    }
    // A connection that ends with an error before its reader loop.
    let failed_span = |reason: &str| {
        if let Some(span) = &span {
            span.fail(reason);
            span.end(unix_ns());
        }
    };
    let refused_span = |code: u16, reason: &str| {
        if let Some(span) = &span {
            span.set("http.status_code", &code.to_string());
        }
        failed_span(reason);
    };
    metrics.connections_active.inc();
    let conn_guard = ConnectionLifetime {
        active: Arc::clone(&metrics.connections_active),
    };
    stream.set_nodelay(true).ok();
    stream.set_read_timeout(Some(READ_POLL)).ok();

    // 1. Read in the HTTP upgrade handshake: at most `MAX_HANDSHAKE_SIZE`
    //    octets within `HANDSHAKE_TIMEOUT` (§4.1).
    let handshake_deadline = Instant::now() + HANDSHAKE_TIMEOUT;
    // A request the daemon refuses before it is parsed: the response, the
    // log line, the error counter and the span (§4.1, §8.1 to §8.3).
    let refuse_request = |stream: &mut WsStream, response: &str, code: u16, reason: &str| {
        metrics.errors_total.inc();
        let _ = stream.write_all(response.as_bytes());
        conn.event(Level::Warn, "handshake_reject")
            .number("code", u64::from(code))
            .field("reason", reason)
            .emit();
        refused_span(code, reason);
        Err(ServerError::Io(alloc_format(format_args!(
            "handshake parse: {reason}"
        ))))
    };
    let mut buf = [0u8; 4096];
    let mut accumulated = Vec::new();
    let req_str = loop {
        if Instant::now() >= handshake_deadline {
            // RFC 9110 §15.5.9: the request did not arrive within the time
            // the server waits for it.
            return refuse_request(
                &mut stream,
                "HTTP/1.1 408 Request Timeout\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                408,
                "the opening handshake did not arrive in time",
            );
        }
        match stream.read(&mut buf) {
            Ok(0) => return Err(ServerError::Io("eof during handshake".to_string())),
            Ok(n) => {
                accumulated.extend_from_slice(&buf[..n]);
                let head_end = accumulated.windows(4).position(|w| w == b"\r\n\r\n");
                if head_end.map_or(accumulated.len(), |i| i + 4) > MAX_HANDSHAKE_SIZE {
                    let e = crate::handshake::HandshakeError::MalformedRequest;
                    return refuse_request(
                        &mut stream,
                        &e.render_error_response(),
                        e.http_status(),
                        &alloc_format(format_args!(
                            "the request head is longer than {MAX_HANDSHAKE_SIZE} octets"
                        )),
                    );
                }
                if head_end.is_some() {
                    // An HTTP request is ASCII (RFC 9110 §5.5); octets that
                    // are no UTF-8 are no request (RFC 6455 §10.7).
                    match String::from_utf8(std::mem::take(&mut accumulated)) {
                        Ok(s) => break s,
                        Err(_) => {
                            let e = crate::handshake::HandshakeError::MalformedRequest;
                            return refuse_request(
                                &mut stream,
                                &e.render_error_response(),
                                e.http_status(),
                                "the request is not UTF-8",
                            );
                        }
                    }
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                // No WebSocket connection yet: a shutdown ends the TCP
                // connection without a Close frame (RFC 6455 §7.1.2 needs
                // an established connection).
                if stop.load(Ordering::SeqCst) || settings.draining.load(Ordering::SeqCst) {
                    return Err(ServerError::Io("shutdown during handshake".to_string()));
                }
            }
            Err(e) => return Err(ServerError::Io(e.to_string())),
        }
    };

    // §4.2: `/healthz` answers a plain HTTP GET on this listener, whether
    // or not the metrics endpoint is on, and to everyone: it names no
    // topic, and a probe carries no credentials.
    let plain_get = plain_get_path(&req_str);
    if plain_get == Some(super::config::HEALTH_PATH) {
        let _ = stream.write_all(health_response(&settings.healthy).as_bytes());
        return Ok(());
    }
    let catalog_over_http = plain_get == Some(super::config::CATALOG_PATH);
    // §5.2: the type of a topic at `/schema/<slug>`.
    let schema_request = plain_get.and_then(super::schema::route);

    let req = match parse_client_request(&req_str) {
        Ok(r) => r,
        // The catalog and the schema over HTTP are no opening handshake;
        // their origin and their credentials are checked like those of one.
        Err(_) if catalog_over_http || schema_request.is_some() => {
            crate::handshake::ClientHandshake {
                path: plain_get.unwrap_or_default().to_string(),
                origin: origin_field(&req_str),
                ..crate::handshake::ClientHandshake::default()
            }
        }
        Err(e) => {
            // RFC 6455 §4.2.1, §4.2.2 /version/, §10.7: the HTTP status of
            // the error, 426 with the version this server speaks for a
            // version it does not.
            metrics.errors_total.inc();
            let _ = stream.write_all(e.render_error_response().as_bytes());
            conn.event(Level::Warn, "handshake_reject")
                .number("code", u64::from(e.http_status()))
                .field("reason", &e.to_string())
                .emit();
            refused_span(e.http_status(), &e.to_string());
            return Err(ServerError::Io(alloc_format(format_args!(
                "handshake parse: {e:?}"
            ))));
        }
    };

    // RFC 6455 §10.2, §4.2.2 /origin/: an origin the daemon does not expect
    // gets HTTP 403 and the handshake ends. The check comes before the
    // authentication: a browser sends the user's credentials with the
    // request of any page, and the origin says whose script asked.
    if let Some(span) = &span {
        span.set("ws.path", &req.path);
    }
    if let Err(e) = settings.origin.check(req.origin.as_deref()) {
        metrics.errors_total.inc();
        let _ = stream.write_all(e.render_error_response().as_bytes());
        conn.event(Level::Warn, "origin_reject")
            .number("code", u64::from(e.http_status()))
            .field("origin", req.origin.as_deref().unwrap_or(""))
            .emit();
        if !catalog_over_http && schema_request.is_none() {
            refused_span(e.http_status(), &e.to_string());
        }
        return Err(ServerError::Io(alloc_format(format_args!(
            "origin reject: {e}"
        ))));
    }

    // §4.1, §7.2 — authentication. Credentials that authenticate no one
    // get HTTP 401, a verified token without the required claim 403; the
    // connection is closed.
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
            metrics.auth_failure(match &e {
                Refusal::Unauthenticated(AuthError::MissingCredentials) => auth_reason::MISSING,
                Refusal::Unauthenticated(_) => auth_reason::REJECTED,
                Refusal::MissingClaim(_) => auth_reason::SCOPE,
            });
            let _ = stream.write_all(render_refusal(&e).as_bytes());
            conn.event(Level::Warn, "auth_reject")
                .field("reason", &e.to_string())
                .emit();
            if !catalog_over_http && schema_request.is_none() {
                refused_span(
                    match &e {
                        Refusal::MissingClaim(_) => 403,
                        Refusal::Unauthenticated(_) => 401,
                    },
                    &e.to_string(),
                );
            }
            return Err(ServerError::Io(alloc_format(format_args!(
                "auth reject: {e}"
            ))));
        }
    };

    // §1 L6: the connection is bound to the participant of its subject's
    // tenant for its life; its paths, subscriptions, samples, catalog and
    // schemas are that participant's. Its topics are counted under the
    // tenant (§8.2).
    let core = Arc::clone(cores.of(&subject));
    let metrics = core.metrics.clone();
    if let Some(span) = &span {
        if !core.tenant.is_empty() {
            span.set("ws.tenant", &core.tenant);
        }
    }

    // §4.2: the catalog names the topics its client may read or write.
    let catalog_text = |subject: &AuthSubject| {
        core.catalog.render_json_where(|t| {
            core.allows(&security, subject, AclOp::Read, &t.name)
                || core.allows(&security, subject, AclOp::Write, &t.name)
        })
    };
    if catalog_over_http {
        let body = catalog_text(&subject);
        let _ = stream.write_all(http_response("200 OK", "application/json", &body).as_bytes());
        return Ok(());
    }
    if let Some((slug, form)) = schema_request {
        let answer = schema_response(slug, form, &core.topics, &core.types, |topic| {
            core.allows(&security, &subject, AclOp::Read, topic)
                || core.allows(&security, &subject, AclOp::Write, topic)
        });
        match answer.refusal {
            Some(SchemaRefusal::Acl) => metrics.auth_failure(auth_reason::ACL),
            Some(SchemaRefusal::NotFound) => metrics.errors_total.inc(),
            None => {}
        }
        conn.event(
            if answer.refusal.is_some() {
                Level::Warn
            } else {
                Level::Debug
            },
            "schema",
        )
        .field("path", &req.path)
        .number("code", u64::from(answer.code))
        .emit();
        let _ = stream.write_all(answer.http.as_bytes());
        return Ok(());
    }
    // Over WebSocket the catalog is the first frame of a connection of the
    // catalog path, and of every connection with `catalog_on_connect`.
    let catalog_frame = (req.path == super::config::CATALOG_PATH || settings.catalog_on_connect)
        .then(|| catalog_text(&subject));

    // Auto-subscribe if the path is the path of a topic (Spec §4.2): its
    // `ws_path`, or the default path where the entry names none.
    let mut auto_topic: Option<String> = core
        .topics
        .iter()
        .find(|t| t.path() == req.path)
        .map(|t| t.name.clone());

    // §7.3 — auto-subscribe topic ACL check (read).
    if let Some(topic) = &auto_topic {
        if !core.allows(&security, &subject, AclOp::Read, topic) {
            metrics.auth_failure(auth_reason::ACL);
            let body = format!("forbidden: read on {topic}");
            let resp = format!(
                "HTTP/1.1 403 Forbidden\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
            conn.event(Level::Warn, "acl_reject")
                .field("topic", topic)
                .field("subject", &subject.name)
                .emit();
            if let Some(span) = &span {
                span.set("auth.subject", &subject.name);
                span.set("dds.topic", topic);
            }
            refused_span(403, &body);
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
    // RFC 7692 §7.1: the first `permessage-deflate` offer of the client
    // that is valid and that this server serves is agreed to, and the
    // messages of the connection are compressed in both directions; RFC
    // 6455 §4.2.2 item 9: the response names what was agreed.
    let deflate = permessage_deflate::accept_offers(&req.extensions);
    if let Some(params) = &deflate {
        resp.extensions
            .push(permessage_deflate::render_accept(params));
    }
    let resp_bytes = render_server_response(&resp);
    if let Err(e) = stream.write_all(resp_bytes.as_bytes()) {
        failed_span(&alloc_format(format_args!(
            "write of the 101 response: {e}"
        )));
        return Err(ServerError::Io(e.to_string()));
    }
    conn.event(Level::Info, "connection_open")
        .field("path", &req.path)
        .field("topic", auto_topic.as_deref().unwrap_or(""))
        .field("subject", &subject.name)
        .field("mode", if json_mode { "json" } else { "binary" })
        .field("tenant", &core.tenant)
        .emit();
    if let Some(span) = &span {
        span.set("http.status_code", "101");
        span.set("auth.subject", &subject.name);
        span.set("ws.mode", if json_mode { "json" } else { "binary" });
        span.set("ws.subprotocol", resp.protocol.as_deref().unwrap_or(""));
        span.set("ws.extensions", &resp.extensions.join(", "));
        if let Some(topic) = &auto_topic {
            span.set("dds.topic", topic);
        }
    }
    let opened = Instant::now();

    // A JSON-mode client of a topic path gets the topic's samples as JSON
    // (§4.3), which needs the topic's type; without one the client is told
    // so (below, once the connection can send), and the connection carries
    // no samples of it.
    let untyped_topic = if json_mode
        && auto_topic
            .as_deref()
            .is_some_and(|topic| !core.types.contains_key(topic))
    {
        auto_topic.take()
    } else {
        None
    };

    // 2. Register the connection with the router: its send queue of §6,
    //    and the burst of a durable topic of its path.
    let queue = ConnQueue::new(settings.max_send_queue);
    if let Ok(mut r) = core.router.lock() {
        r.register_connection(conn_id, Arc::clone(&queue));
        if let Some(topic) = &auto_topic {
            r.subscribe(conn_id, topic.clone());
        }
    }
    // §9.2: a connection that completed its handshake while the daemon
    // began to shut down may have missed the broadcast; it is told too.
    if settings.draining.load(Ordering::SeqCst) {
        queue.push_control(RouterMsg::Shutdown);
    }
    // §6 liveliness: the reader thread notes each Pong, the writer thread
    // sends the Pings and fails the connection that misses one. A write
    // that the client does not take within `ping_interval_ms` fails as
    // well: such a client cannot get the Ping either, and a RELIABLE topic
    // waiting for its send queue holds back the samples of every other
    // connection of the topic.
    let pong_seen = Arc::new(AtomicBool::new(true));
    let writer_failed = Arc::new(AtomicBool::new(false));
    stream.set_write_timeout(settings.ping_interval).ok();

    // 3. Split stream into independent reader and writer handles so a blocking
    //    `read()` (500 ms timeout) never blocks outbound notify frames on the
    //    writer thread. For plain TCP, `try_clone()` duplicates the OS socket
    //    descriptor so reader and writer share no user-space lock at all. For
    //    TLS, the reader waits via `peek()` on a cloned `TcpStream` outside the
    //    session mutex and only locks the `StreamOwned` once bytes are in the
    //    kernel receive buffer.
    let (mut reader, sink) = match stream.into_split() {
        Ok(split) => split,
        Err(e) => {
            if let Ok(mut r) = core.router.lock() {
                r.deregister_connection(conn_id);
            }
            conn.event(Level::Info, "connection_close")
                .latency(opened)
                .emit();
            failed_span(&alloc_format(format_args!(
                "the socket could not be split: {e}"
            )));
            return Err(ServerError::Io(e.to_string()));
        }
    };
    let mut receiver = Receiver::new(Role::Server, settings.limits);
    let mut sender = Sender::server();
    if let Some(params) = &deflate {
        receiver = receiver.with_permessage_deflate(params);
        sender = sender.with_permessage_deflate(params);
    }
    let writer = Arc::new(WsOut {
        sink,
        sender: Mutex::new(sender),
        conn: Arc::clone(&conn),
        metrics: metrics.clone(),
        span: span.clone(),
    });

    // The catalog is the first text frame of the connection (§5.2), also
    // before the 415 error frame of a topic without a type.
    if let Some(catalog) = &catalog_frame {
        send_text_frame(&writer, catalog);
    }
    if let Some(topic) = &untyped_topic {
        metrics.errors_total.inc();
        send_text_frame(&writer, &no_type_error(topic));
    }
    let stop_w = Arc::clone(&stop);
    let errors_out = Arc::clone(&metrics.errors_total);
    let writer_w = Arc::clone(&writer);
    // Per-connection ACL state: the read check is done per topic
    // before we hand off to router.dispatch. Here before the send: we
    // have the subject + ACL via closure move.
    let security_w = Arc::clone(&security);
    let subject_w = subject.clone();
    let core_w = Arc::clone(&core);
    let queue_w = Arc::clone(&queue);
    let pong_seen_w = Arc::clone(&pong_seen);
    let writer_failed_w = Arc::clone(&writer_failed);
    let ping_interval = settings.ping_interval;
    let writer_thread = thread::spawn(move || {
        // §8.3: a write that failed fails the connection's span, unless
        // nothing was sent because a Close frame went out before.
        let failed_write = |e: &std::io::Error| {
            writer_failed_w.store(true, Ordering::SeqCst);
            if let (Some(span), Some(reason)) = (&writer_w.span, write_failure(e)) {
                span.fail(&reason);
            }
        };
        // One sample to the client; an error when the connection is closing
        // or broken: no frame follows a Close (RFC 6455 §5.5.1).
        let send_sample =
            |topic: &str, sample: &DdsSample, since: Instant| -> std::io::Result<()> {
                // §7.3 — read ACL (post-subscribe gate): if the ACL for this
                // subject + topic is deny, the sample is not sent (no
                // disclosure).
                if !core_w.allows(&security_w, &subject_w, AclOp::Read, topic) {
                    return Ok(());
                }
                let sent = if json_mode {
                    let texts = match json_sample_frames(topic, sample, core_w.types.get(topic)) {
                        Ok(texts) => texts,
                        Err(error) => {
                            errors_out.inc();
                            vec![error]
                        }
                    };
                    texts.iter().try_for_each(|text| {
                        writer_w
                            .message_on(topic, true, text.as_bytes(), since)
                            .map(|_| ())
                    })
                } else {
                    writer_w
                        .message_on(topic, false, &binary_sample_frame(sample), since)
                        .map(|_| ())
                };
                if sent.is_err() {
                    errors_out.inc();
                }
                sent
            };
        let poll = ping_interval.map_or(Duration::from_millis(200), |p| {
            p.min(Duration::from_millis(200))
        });
        let mut next_ping = ping_interval.map(|p| Instant::now() + p);
        let mut ping_count: u64 = 0;
        'samples: while !stop_w.load(Ordering::SeqCst) {
            // §6: a client that let more control messages pile up than its
            // queue holds is closed with 1008; what waits is not sent.
            if queue_w.overflowed() {
                errors_out.inc();
                writer_w
                    .conn
                    .event(Level::Warn, "send_queue_overflow")
                    .number("queued_controls", MAX_QUEUED_CONTROLS as u64)
                    .emit();
                if let Some(span) = &writer_w.span {
                    span.fail("send queue overflow");
                }
                let _ = writer_w.close(QUEUE_OVERFLOW_CLOSE, "send queue overflow");
                writer_failed_w.store(true, Ordering::SeqCst);
                break 'samples;
            }
            // §6: the daemon's Ping; a client that has not answered the
            // previous one by now is failed with 1011.
            if let (Some(due), Some(interval)) = (next_ping, ping_interval) {
                if Instant::now() >= due {
                    if !pong_seen_w.swap(false, Ordering::SeqCst) {
                        if let Some(span) = &writer_w.span {
                            span.fail("ping timeout");
                        }
                        let _ = writer_w.close(PING_TIMEOUT_CLOSE, "ping timeout");
                        writer_failed_w.store(true, Ordering::SeqCst);
                        break 'samples;
                    }
                    ping_count += 1;
                    if let Err(e) = writer_w.ping(&ping_count.to_be_bytes()) {
                        failed_write(&e);
                        break 'samples;
                    }
                    next_ping = Some(Instant::now() + interval);
                }
            }
            let popped = queue_w.pop(poll);
            // §8.1: `latency_us` of a frame out counts from here.
            let taken = Instant::now();
            match popped {
                Some(RouterMsg::Sample { topic, sample }) => {
                    if let Err(e) = send_sample(&topic, &sample, taken) {
                        failed_write(&e);
                        break 'samples;
                    }
                }
                // §6: the samples a durable topic keeps, before its live
                // ones.
                Some(RouterMsg::Burst { topic, samples }) => {
                    for sample in &samples {
                        if let Err(e) = send_sample(&topic, sample, taken) {
                            failed_write(&e);
                            break 'samples;
                        }
                    }
                }
                Some(RouterMsg::Text(text)) => {
                    if let Err(e) = writer_w.message(true, text.as_bytes()) {
                        failed_write(&e);
                        break 'samples;
                    }
                }
                // §6 `deadline_missed`: names the topic and an instance, so
                // it passes the read ACL as a sample does (§7.3); a reload
                // that withdraws the right withholds it.
                Some(RouterMsg::TopicText { topic, text }) => {
                    if !core_w.allows(&security_w, &subject_w, AclOp::Read, &topic) {
                        continue;
                    }
                    if let Err(e) = writer_w.message_on(&topic, true, text.as_bytes(), taken) {
                        failed_write(&e);
                        break 'samples;
                    }
                }
                Some(RouterMsg::Shutdown) => {
                    // §9.2: behind the samples the queue held, the
                    // `shutdown` message, then the Close 1001; the reader
                    // reads the client's Close for the linger and ends.
                    let _ = writer_w.message(true, json_frames::shutdown().as_bytes());
                    writer_w.going_away();
                    writer_failed_w.store(true, Ordering::SeqCst);
                    break;
                }
                None if queue_w.is_closed() => break,
                None => {}
            }
        }
        // Nothing more is sent: a pump waiting for room in the queue
        // returns.
        queue_w.close();
    });

    // 4. Reader loop (TCP/TLS → WS frames → router/DDS writer). Every frame
    //    goes through the receiver, which validates it (RFC 6455 §5, §8.2,
    //    §10.7): invalid data fails the connection with its close code.
    let mut frame_buf: Vec<u8> = Vec::new();
    let mut closing = false;
    'reader: loop {
        // §6: the writer thread sent the Close 1011 for a missed Pong, or
        // the client took no frame within `ping_interval_ms`; §9.2: it sent
        // the `shutdown` message and the Close 1001.
        if writer_failed.load(Ordering::SeqCst) {
            closing = true;
            break;
        }
        if stop.load(Ordering::SeqCst) {
            // zerodds-ws-bridge-1.0 §9.2: the daemon is going down. This
            // thread sends the Close itself: the stop flag is set before
            // the router hands `Shutdown` to the writer thread, and the
            // writer thread leaves its loop on the flag as well, so the
            // message alone does not reach every connection.
            writer.going_away();
            closing = true;
            break;
        }
        let read_result = reader.read(&mut buf);
        match read_result {
            Ok(0) => break,
            Ok(n) => {
                frame_buf.extend_from_slice(&buf[..n]);
                loop {
                    let event = match receiver.feed(&mut frame_buf) {
                        Ok(Some(event)) => event,
                        Ok(None) => break,
                        Err(failure) => {
                            // RFC 6455 §7.1.7, §10.7: the Close frame with
                            // the status code of the failure, then the
                            // connection ends; nothing more is processed.
                            metrics.errors_total.inc();
                            conn.event(Level::Warn, "connection_failed")
                                .number("code", u64::from(failure.close_code().to_u16()))
                                .field("reason", &failure.to_string())
                                .emit();
                            if let Some(span) = &span {
                                span.fail(&failure.to_string());
                            }
                            writer.fail(&failure);
                            closing = true;
                            break 'reader;
                        }
                    };
                    match event {
                        Event::Message(message)
                            if !message.is_text && message.payload.starts_with(&BINARY_MAGIC) =>
                        {
                            let decoded = Instant::now();
                            let topic = auto_topic.as_deref().unwrap_or("");
                            metrics.frame_in(topic, message.payload.len());
                            let mut dds_write = None;
                            let result = handle_binary_sample(
                                &message.payload,
                                auto_topic.as_deref(),
                                &core,
                                &security,
                                &subject,
                                &writer,
                                &mut dds_write,
                            );
                            trace_frame_in(
                                span.as_deref(),
                                topic,
                                &message,
                                decoded,
                                dds_write,
                                &result,
                            );
                            log_frame_in(&conn, topic, message.payload.len(), decoded, result);
                        }
                        Event::Message(message) => {
                            let decoded = Instant::now();
                            let mut topic = String::new();
                            let mut dds_write = None;
                            let result = handle_inbound_frame(
                                &message.payload,
                                message.is_text,
                                &conn,
                                json_mode,
                                &core,
                                &security,
                                &subject,
                                &writer,
                                &mut topic,
                                &mut dds_write,
                            );
                            trace_frame_in(
                                span.as_deref(),
                                &topic,
                                &message,
                                decoded,
                                dds_write,
                                &result,
                            );
                            log_frame_in(&conn, &topic, message.payload.len(), decoded, result);
                        }
                        // RFC 6455 §5.5.2, §5.5.3: a Pong with the Ping's
                        // application data.
                        Event::Ping(data) => {
                            let _ = writer.pong(&data);
                        }
                        // §6: the client is alive.
                        Event::Pong(_) => pong_seen.store(true, Ordering::SeqCst),
                        // RFC 6455 §5.5.1, §7.1: a Close is answered with a
                        // Close of the same status, unless this end sent
                        // its own before; then the connection ends.
                        Event::Close(body) => {
                            writer.close_received(body.as_ref());
                            closing = true;
                            break 'reader;
                        }
                    }
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            // §8.3: a socket error fails the connection.
            Err(e) => {
                if let Some(span) = &span {
                    span.fail(&alloc_format(format_args!("read: {e}")));
                }
                break;
            }
        }
    }
    if closing {
        // The Close frame is out. This end stops sending and reads what
        // the client still sends until it closes, at most 500 ms: closing
        // a socket with unread octets resets the connection, and the
        // client could lose the Close frame (RFC 6455 §7.1.1: the server
        // closes the TCP connection first).
        writer.sink.shutdown_write();
        let deadline = Instant::now() + CLOSE_LINGER;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() || reader.set_read_timeout(left).is_err() {
                break;
            }
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    }

    // 5. Cleanup.
    if let Ok(mut r) = core.router.lock() {
        r.deregister_connection(conn_id);
    }
    conn.event(Level::Info, "connection_close")
        .latency(opened)
        .emit();
    let _ = writer_thread.join();
    // §8.3: the root span, after the last message of the writer thread.
    if let Some(span) = &span {
        span.end(unix_ns());
    }
    drop(conn_guard);
    Ok(())
}

/// Why `/schema/<slug>` was refused.
#[cfg(feature = "daemon")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SchemaRefusal {
    /// No topic with `idl_file` at the slug (404).
    NotFound,
    /// The ACL grants the client neither read nor write of the topic (403).
    Acl,
}

/// The answer to `/schema/<slug>` (§5.2).
#[cfg(feature = "daemon")]
struct SchemaAnswer {
    code: u16,
    http: String,
    refusal: Option<SchemaRefusal>,
}

/// The answer to a plain GET of `/schema/<slug>` (§5.2): 200 with the IDL
/// of the topic whose slug it is (`text/plain`) or its XML type
/// representation (`application/xml`); 404 for a slug of no topic or of a
/// topic without `idl_file`; 403 for a topic the client may neither read
/// nor write (§7.3).
#[cfg(feature = "daemon")]
fn schema_response(
    slug: &str,
    form: super::schema::SchemaForm,
    topics: &[TopicConfig],
    types: &std::collections::BTreeMap<String, TopicType>,
    granted: impl Fn(&str) -> bool,
) -> SchemaAnswer {
    let refuse = |code: u16, status: &str, refusal: SchemaRefusal, message: String| SchemaAnswer {
        code,
        http: http_response(status, "text/plain; charset=utf-8", &format!("{message}\n")),
        refusal: Some(refusal),
    };
    // A topic without `idl_file` may share the slug of one with it (§3).
    let of_slug = |t: &&TopicConfig| super::schema::slug(&t.name) == slug;
    let Some(topic) = topics
        .iter()
        .filter(of_slug)
        .find(|t| types.contains_key(&t.name))
        .or_else(|| topics.iter().find(of_slug))
    else {
        return refuse(
            404,
            "404 Not Found",
            SchemaRefusal::NotFound,
            format!("no topic has the schema path /schema/{slug}"),
        );
    };
    if !granted(&topic.name) {
        return refuse(
            403,
            "403 Forbidden",
            SchemaRefusal::Acl,
            format!("{} is not granted", topic.name),
        );
    }
    let Some(ty) = types.get(&topic.name) else {
        return refuse(
            404,
            "404 Not Found",
            SchemaRefusal::NotFound,
            format!("{} has no idl_file", topic.name),
        );
    };
    let (content_type, body) = match form {
        super::schema::SchemaForm::Idl => ("text/plain; charset=utf-8", ty.idl().to_string()),
        super::schema::SchemaForm::Xml => {
            match super::schema::xml_type_representation(ty.dynamic_type()) {
                Ok(xml) => ("application/xml; charset=utf-8", xml),
                // A type the XML form cannot carry: the IDL form can.
                Err(e) => {
                    return SchemaAnswer {
                        code: 501,
                        http: http_response(
                            "501 Not Implemented",
                            "text/plain; charset=utf-8",
                            &format!("{e}; the IDL form is at /schema/{slug}.idl\n"),
                        ),
                        refusal: Some(SchemaRefusal::NotFound),
                    };
                }
            }
        }
    };
    SchemaAnswer {
        code: 200,
        http: http_response("200 OK", content_type, &body),
        refusal: None,
    }
}

/// The path of a plain HTTP GET: a request with the method GET and
/// without an `Upgrade` field, its target without the query. `None` for an
/// opening handshake and for any other method.
#[cfg(feature = "daemon")]
fn plain_get_path(request: &str) -> Option<&str> {
    let mut lines = request.split("\r\n");
    let mut line = lines.next()?.split(' ');
    let (method, target, version) = (line.next()?, line.next()?, line.next()?);
    if method != "GET" || !version.starts_with("HTTP/1.") || line.next().is_some() {
        return None;
    }
    let upgrade = lines
        .take_while(|l| !l.is_empty())
        .filter_map(|l| l.split_once(':'))
        .any(|(name, _)| name.trim().eq_ignore_ascii_case("upgrade"));
    if upgrade {
        return None;
    }
    target.split('?').next()
}

/// The `Origin` field of a request that is no opening handshake, as
/// `parse_client_request` reads it for one: the origins of several fields
/// are one list, separated by spaces, so that each is checked. `None`
/// without the field.
#[cfg(feature = "daemon")]
fn origin_field(request: &str) -> Option<String> {
    let values: Vec<&str> = request
        .split("\r\n")
        .skip(1)
        .take_while(|l| !l.is_empty())
        .filter_map(|l| l.split_once(':'))
        .filter(|(n, _)| n.trim().eq_ignore_ascii_case("origin"))
        .map(|(_, v)| v.trim())
        .collect();
    (!values.is_empty()).then(|| values.join(" "))
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

    /// The time one write waits for the client to take octets; the socket
    /// option holds for the clones of [`Self::into_split`] as well.
    fn set_write_timeout(&self, dur: Option<Duration>) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.set_write_timeout(dur),
            Self::Tls(s) => s.sock.set_write_timeout(dur),
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

/// How long the daemon reads on after its Close frame before it closes
/// the connection (zerodds-ws-bridge-1.0 §4.5).
#[cfg(feature = "daemon")]
const CLOSE_LINGER: Duration = Duration::from_millis(500);

/// How often the daemon looks for missed deadlines (§6).
#[cfg(feature = "daemon")]
const DEADLINE_TICK: Duration = Duration::from_millis(10);

/// The status code of the Close for a client that did not answer the
/// daemon's Ping (§6; RFC 6455 §7.4.1: 1011, the server cannot go on).
#[cfg(feature = "daemon")]
const PING_TIMEOUT_CLOSE: u16 = 1011;

/// The status code of the Close for a client that lets more control
/// messages wait in its send queue than it may (§6; RFC 6455 §7.4.1: 1008,
/// a message that violates the policy of the endpoint).
#[cfg(feature = "daemon")]
const QUEUE_OVERFLOW_CLOSE: u16 = 1008;

/// The largest opening handshake the daemon reads (§4.1): a request head of
/// more octets is answered with 400.
#[cfg(feature = "daemon")]
pub const MAX_HANDSHAKE_SIZE: usize = 64 * 1024;

/// How long after the TCP connection the request head of the opening
/// handshake may take to arrive (§4.1); then the daemon answers `408` and
/// closes the connection.
#[cfg(feature = "daemon")]
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// How long one `read` of a connection waits for octets before the thread
/// looks at the stop flag again.
#[cfg(feature = "daemon")]
const READ_POLL: Duration = Duration::from_millis(500);

/// How long [`DaemonHandle::shutdown`] waits for the connections to end
/// (zerodds-ws-bridge-1.0 §9.2): a connection sees the stop flag within
/// `READ_POLL`, sends its Close and reads on for at most `CLOSE_LINGER`;
/// the rest is for a client that takes the Close frame slowly.
#[cfg(feature = "daemon")]
pub const SHUTDOWN_CLOSE_WAIT: Duration = Duration::from_secs(2);

/// The number of connection threads that run, from the accepted TCP
/// connection to the end of `serve_connection`.
#[cfg(feature = "daemon")]
#[derive(Default)]
struct ConnectionThreads {
    running: Mutex<usize>,
    ended: Condvar,
}

#[cfg(feature = "daemon")]
impl ConnectionThreads {
    /// Counts a connection thread; the guard counts it out when the thread
    /// drops it.
    fn enter(self: &Arc<Self>) -> ConnectionThread {
        let mut running = match self.running.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        *running += 1;
        ConnectionThread(Arc::clone(self))
    }

    /// Waits until no connection thread runs, at most `limit`. Whether none
    /// runs.
    fn wait_until_none(&self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        let mut running = match self.running.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        while *running != 0 {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            running = match self.ended.wait_timeout(running, left) {
                Ok((g, _)) => g,
                Err(p) => p.into_inner().0,
            };
        }
        true
    }
}

/// One running connection thread in [`ConnectionThreads`].
#[cfg(feature = "daemon")]
struct ConnectionThread(Arc<ConnectionThreads>);

#[cfg(feature = "daemon")]
impl Drop for ConnectionThread {
    fn drop(&mut self) {
        let mut running = match self.0.running.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        *running = running.saturating_sub(1);
        self.0.ended.notify_all();
    }
}

#[cfg(feature = "daemon")]
impl WsReader {
    /// The time one `read` waits for octets.
    fn set_read_timeout(&self, timeout: Duration) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.set_read_timeout(Some(timeout)),
            Self::Tls { wait_sock, .. } => wait_sock.set_read_timeout(Some(timeout)),
        }
    }

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

/// A connection in the log (Spec §8.1): the daemon's number of it and the
/// client's address.
#[cfg(feature = "daemon")]
struct ConnLog {
    id: u64,
    peer: String,
}

#[cfg(feature = "daemon")]
impl ConnLog {
    /// A line about this connection, with `connection_id` and `peer`.
    fn event(&self, level: Level, name: &'static str) -> log::Event {
        log::event(level, name)
            .number("connection_id", self.id)
            .field("peer", &self.peer)
    }
}

/// The log line of a client's message (§8.1): `frame_in` with its topic
/// (none for a control message), its octets and `latency_us` from the
/// decoded message to the DDS write returned; a message the daemon refused
/// is `frame_refused` with the reason, the error frame told the client.
#[cfg(feature = "daemon")]
fn log_frame_in(
    conn: &ConnLog,
    topic: &str,
    bytes: usize,
    decoded: Instant,
    result: Result<(), String>,
) {
    // The threshold first: at the default `info` no frame_in line is
    // written, and a sample pays no allocation for it.
    let line = match result {
        Ok(()) if !log::enabled(Level::Debug) => return,
        Ok(()) => conn.event(Level::Debug, "frame_in"),
        Err(reason) => conn
            .event(Level::Warn, "frame_refused")
            .field("reason", &reason),
    };
    line.field("topic", topic)
        .number("bytes", bytes as u64)
        .latency(decoded)
        .emit();
}

/// The spans of a client's message (§8.3): `ws.frame.in` from the decoded
/// message to now, with `dds.write` when it wrote into DDS.
#[cfg(feature = "daemon")]
fn trace_frame_in(
    span: Option<&ConnectionSpan>,
    topic: &str,
    message: &crate::message::Message,
    decoded: Instant,
    dds_write: Option<(u64, u64)>,
    result: &Result<(), String>,
) {
    let Some(span) = span else {
        return;
    };
    span.frame_in(&FrameIn {
        start_ns: unix_ns_since(decoded),
        end_ns: unix_ns(),
        topic: topic.to_string(),
        bytes: message.payload.len(),
        text: message.is_text,
        dds_write,
        refused: result.as_ref().err().cloned(),
    });
}

/// The reason a failed write of a connection fails its span with (§8.3):
/// a write the client did not take within `ping_interval_ms` (the socket's
/// write timeout, §6), or the socket's error. `None` when nothing was sent
/// because a Close frame went out before (RFC 6455 §5.5.1): the connection
/// is closing, it did not fail.
#[cfg(feature = "daemon")]
fn write_failure(e: &std::io::Error) -> Option<String> {
    if e.get_ref()
        .and_then(|inner| inner.downcast_ref::<SendError>())
        .is_some_and(|s| *s == SendError::Closing)
    {
        return None;
    }
    Some(
        if matches!(
            e.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        ) {
            "write timeout: the client took no frame within ping_interval_ms".to_string()
        } else {
            alloc_format(format_args!("write: {e}"))
        },
    )
}

/// The sending side of an upgraded connection: the frames of
/// [`Sender`] written to the socket. One lock covers building a frame and
/// writing it, so the frames of the reader thread (Pong, Close, error
/// frames) and of the writer thread (samples) leave whole and in the order
/// the compression context of RFC 7692 saw them.
#[cfg(feature = "daemon")]
struct WsOut {
    sink: Arc<WsWriter>,
    sender: Mutex<Sender>,
    /// The connection in the log (§8.1).
    conn: Arc<ConnLog>,
    /// Counts each message sent (§8.2 `frames_out_total`, `bytes_out_total`).
    metrics: BridgeMetrics,
    /// The span of the connection (§8.3): each message sent is its child.
    span: Option<Arc<ConnectionSpan>>,
}

#[cfg(feature = "daemon")]
impl WsOut {
    fn send(
        &self,
        build: impl FnOnce(&mut Sender) -> Result<Option<Vec<u8>>, SendError>,
    ) -> std::io::Result<usize> {
        let mut sender = match self.sender.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        match build(&mut sender) {
            Ok(Some(wire)) => {
                self.sink.write_all(&wire)?;
                Ok(wire.len())
            }
            Ok(None) => Ok(0),
            Err(e) => Err(std::io::Error::other(e)),
        }
    }

    /// Sends a text (`is_text`) or binary message of no topic; the octets
    /// written.
    fn message(&self, is_text: bool, payload: &[u8]) -> std::io::Result<usize> {
        self.message_on("", is_text, payload, Instant::now())
    }

    /// Sends a message that carries a sample (or a `deadline_missed`) of
    /// `topic`; counted under the topic (§8.2).
    /// `since` is when the connection took it from its send queue, the
    /// start of the `latency_us` of the log line (§8.1).
    fn message_on(
        &self,
        topic: &str,
        is_text: bool,
        payload: &[u8],
        since: Instant,
    ) -> std::io::Result<usize> {
        let sent = self.send(|s| s.message(is_text, payload).map(Some));
        if let Some(span) = &self.span {
            span.frame_out(&FrameOut {
                start_ns: unix_ns_since(since),
                end_ns: unix_ns(),
                topic: topic.to_string(),
                bytes: payload.len(),
                text: is_text,
                failed: sent.as_ref().err().map(ToString::to_string),
            });
        }
        let written = sent?;
        self.metrics.frame_out(topic, payload.len());
        if log::enabled(Level::Debug) {
            self.conn
                .event(Level::Debug, "frame_out")
                .field("topic", topic)
                .number("bytes", payload.len() as u64)
                .latency(since)
                .emit();
        }
        Ok(written)
    }

    fn pong(&self, data: &[u8]) -> std::io::Result<usize> {
        self.send(|s| s.pong(data).map(Some))
    }

    /// A Ping of the daemon (§6, RFC 6455 §5.5.2).
    fn ping(&self, data: &[u8]) -> std::io::Result<usize> {
        self.send(|s| s.ping(data).map(Some))
    }

    /// Starts the closing handshake (RFC 6455 §7.1.2).
    fn close(&self, status: u16, reason: &str) -> std::io::Result<usize> {
        self.send(|s| s.close(status, reason).map(Some))
    }

    /// The Close frame of a daemon that shuts down: 1001, "going away"
    /// (RFC 6455 §7.1.2, §7.4.1). Sent once: nothing is sent when a Close
    /// frame went out before, from either thread or for another reason.
    fn going_away(&self) {
        let _ = self.close(1001, "going away");
    }

    /// Answers the client's Close frame (RFC 6455 §5.5.1).
    fn close_received(&self, body: Option<&(u16, String)>) {
        let _ = self.send(|s| s.close_received(body));
    }

    /// Fails the connection with the Close frame of `failure` (RFC 6455
    /// §7.1.7).
    fn fail(&self, failure: &Failure) {
        let _ = self.send(|s| Ok(s.fail(failure)));
    }
}

#[cfg(feature = "daemon")]
impl WsWriter {
    /// Ends the sending direction: TLS `close_notify`, then the TCP FIN.
    fn shutdown_write(&self) {
        match self {
            Self::Plain(m) => {
                let guard = match m.lock() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                let _ = guard.shutdown(std::net::Shutdown::Write);
            }
            Self::Tls(m) => {
                let mut guard = match m.lock() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                guard.conn.send_close_notify();
                let _ = guard.flush();
                let _ = guard.sock.shutdown(std::net::Shutdown::Write);
            }
        }
    }

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

/// A message from the client that is no binary sample frame: a text
/// message is a control message of §4.4 or a sample frame of §4.3; a binary
/// message without the magic of §4.3 is refused, whatever it holds (§4.4:
/// the control messages are text frames). What the daemon cannot do with a
/// message is answered with an error frame (§4.4), never dropped.
#[cfg(feature = "daemon")]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn handle_inbound_frame(
    payload: &[u8],
    is_text: bool,
    conn: &ConnLog,
    json_mode: bool,
    core: &Core,
    security: &Arc<zerodds_bridge_security::LiveSecurity>,
    subject: &AuthSubject,
    writer: &WsOut,
    topic_seen: &mut String,
    dds_write: &mut Option<(u64, u64)>,
) -> Result<(), String> {
    let conn_id = conn.id;
    let (router, writers, topics, types, runtime, metrics) = (
        &core.router,
        &core.writers,
        &core.topics,
        &core.types,
        &core.runtime,
        &core.metrics,
    );
    let refuse = |code: u16, message: String| {
        metrics.errors_total.inc();
        send_text_frame(writer, &json_frames::error(code, &message));
        Err(message)
    };
    // §7.3: a read or write the ACL does not grant, an error frame 403,
    // counted in `auth_failures_total`, not in `errors_total` (§8.2).
    let refuse_acl = |message: String| {
        metrics.auth_failure(auth_reason::ACL);
        send_text_frame(writer, &json_frames::error(403, &message));
        Err(message)
    };
    // A text message is UTF-8 when it gets here: the receiver failed the
    // connection with 1007 otherwise (RFC 6455 §8.2). A binary message is
    // not parsed: without the magic of §4.3 it is no message of a client.
    let message = if is_text {
        core::str::from_utf8(payload)
            .ok()
            .map(json_frames::parse_client)
    } else {
        None
    };
    // §8.2: a sample frame is counted under its topic, any other message
    // under none; §8.1: logged the same way.
    let topic_of_frame = match &message {
        Some(Ok(ClientMessage::Sample(frame))) => frame.topic.as_str(),
        _ => "",
    };
    metrics.frame_in(topic_of_frame, payload.len());
    topic_seen.push_str(topic_of_frame);
    let message = match message {
        Some(Ok(m)) => m,
        Some(Err(e)) => return refuse(400, e),
        None => {
            return refuse(
                400,
                "a binary frame begins with ZDB1, a control message or a JSON sample is a text frame"
                    .to_string(),
            );
        }
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
            for topic in &names {
                conn.event(Level::Debug, "unsubscribe")
                    .field("topic", topic)
                    .emit();
            }
            Ok(())
        }
        ClientMessage::Subscribe(names) => {
            let mut taken = Vec::new();
            for topic in names {
                if !core.allows(security, subject, AclOp::Read, &topic) {
                    let _ = refuse_acl(alloc_format(format_args!(
                        "{}: read on {topic} is not granted",
                        subject.name
                    )));
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
                // `subscribed` names each topic once (§4.4).
                if !taken.contains(&topic) {
                    taken.push(topic);
                }
            }
            // `subscribed` goes through the send queue, ahead of the
            // samples the topics keep (§6), which follow it there.
            if let Ok(mut r) = router.lock() {
                r.send_text(conn_id, json_frames::subscribed(&taken));
                for topic in &taken {
                    r.subscribe(conn_id, topic.clone());
                }
            }
            for topic in &taken {
                conn.event(Level::Debug, "subscribe")
                    .field("topic", topic)
                    .emit();
            }
            Ok(())
        }
        ClientMessage::Sample(frame) => {
            let topic = frame.topic.clone();
            if !core.allows(security, subject, AclOp::Write, &topic) {
                return refuse_acl(alloc_format(format_args!(
                    "{}: write on {topic} is not granted",
                    subject.name
                )));
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
            let started = unix_ns();
            let written = {
                let _held = core.hold_writer(&topic);
                write_sample_frame(runtime, eid, ty, &frame, source_ts)
            };
            // A `register` writes nothing (§4.3); a write that failed is
            // the error of the frame span.
            if written.is_ok() && frame.op != SampleOp::Register {
                *dds_write = Some((started, unix_ns()));
            }
            match written {
                Ok(()) => {
                    if frame.op != SampleOp::Register {
                        metrics.dds_published(&topic);
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

#[cfg(feature = "daemon")]
fn send_text_frame(writer: &WsOut, text: &str) {
    let _ = writer.message(true, text.as_bytes());
}

/// The subprotocol of the binary frames (zerodds-ws-bridge-1.0 §4.1). A
/// subprotocol is a token (RFC 6455 §4.1, the `token` of RFC 7230 §3.2.6):
/// a `/` is a separator and no character of one.
const SUBPROTOCOL_BINARY: &str = "zerodds-ws-bridge-1.0";
/// The subprotocol of the JSON text frames (§4.3).
const SUBPROTOCOL_JSON: &str = "zerodds-ws-bridge-1.0+json";
/// A binary frame from the client (§4.3) on a connection whose path names
/// a topic (§4.2): the sample is written into DDS with the representation
/// and byte order its header names. A client disposes or unregisters with a
/// JSON sample frame; a binary frame with those flags, with bit 1 or with a
/// reserved bit is refused with an error frame, not dropped.
#[cfg(feature = "daemon")]
#[allow(clippy::too_many_arguments)]
fn handle_binary_sample(
    frame: &[u8],
    auto_topic: Option<&str>,
    core: &Core,
    security: &Arc<zerodds_bridge_security::LiveSecurity>,
    subject: &AuthSubject,
    writer: &WsOut,
    dds_write: &mut Option<(u64, u64)>,
) -> Result<(), String> {
    let (writers, runtime, metrics) = (&core.writers, &core.runtime, &core.metrics);
    let refuse = |code: u16, message: String| {
        metrics.errors_total.inc();
        send_text_frame(writer, &json_frames::error(code, &message));
        Err(message)
    };
    let Some(topic) = auto_topic else {
        return refuse(
            400,
            "a binary frame needs a connection whose path names a topic".into(),
        );
    };
    if !core.allows(security, subject, AclOp::Write, topic) {
        // §7.3, §8.2: an ACL refusal, counted in `auth_failures_total`
        // and not in `errors_total`.
        metrics.auth_failure(auth_reason::ACL);
        let message = alloc_format(format_args!(
            "{}: write on {topic} is not granted",
            subject.name
        ));
        send_text_frame(writer, &json_frames::error(403, &message));
        return Err(message);
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
    if let Err(e) = check_client_flags(topic, flags) {
        return refuse(400, e);
    }
    // §4.3: the writer of a topic with `idl_file` offers XCDR2 alone, in
    // the format of the type's extensibility (DDS-XTypes 1.3 §7.6.3.1.1,
    // Table 60); a frame may name either byte order of that format.
    if let Some(ty) = core.types.get(topic) {
        let own = ty.identifier();
        if identifier[0] != own[0] || identifier[1] & !0x01 != own[1] & !0x01 {
            return refuse(
                400,
                alloc_format(format_args!(
                    "{topic}: encapsulation {:#06x}: the writer of the topic offers {:#06x} or {:#06x}",
                    u16::from_be_bytes(identifier),
                    u16::from_be_bytes(own) & !0x01,
                    u16::from_be_bytes(own) | 0x01
                )),
            );
        }
    }
    // The writer's header names what the frame's header named; the writer
    // is held from the first setter to the write.
    let (offer, extensibility, big_endian) = writer_form(identifier);
    let held = core.hold_writer(topic);
    let written = runtime
        .set_user_writer_data_rep_override(*eid, Some(vec![offer]))
        .and_then(|()| runtime.set_user_writer_byte_order_override(*eid, big_endian))
        .and_then(|()| runtime.set_user_writer_wire_extensibility(*eid, extensibility))
        .map(|()| unix_ns())
        .and_then(|started| {
            runtime
                .write_user_sample_borrowed(*eid, body)
                .map(|()| started)
        });
    drop(held);
    let started = match written {
        Ok(started) => started,
        // §4.4: 500, the DDS write failed.
        Err(e) => return refuse(500, alloc_format(format_args!("{topic}: dds-write: {e:?}"))),
    };
    *dds_write = Some((started, unix_ns()));
    metrics.dds_published(topic);
    Ok(())
}

fn alloc_format(args: core::fmt::Arguments<'_>) -> String {
    use core::fmt::Write as _;
    let mut s = String::new();
    let _ = s.write_fmt(args);
    s
}

/// The settings in which `new` differs from the configuration the daemon
/// `started` with and that a reload does not apply (Spec §9.2): they are
/// read once, when the daemon starts, and wait for a restart. The names
/// are the ones the reload logs.
fn settings_that_need_a_restart(started: &DaemonConfig, new: &DaemonConfig) -> Vec<&'static str> {
    let mut restart = Vec::new();
    if new.tls_cert_file != started.tls_cert_file
        || new.tls_key_file != started.tls_key_file
        || new.tls_client_ca_file != started.tls_client_ca_file
    {
        restart.push("tls file paths");
    }
    if new.topics != started.topics {
        restart.push("topics");
    }
    // §1 L6: the participants, their topics and their subjects; the ACL of
    // a tenant's topics applies at once.
    if new.tenants != started.tenants {
        restart.push("tenants");
    }
    if new.listen != started.listen || new.domain != started.domain {
        restart.push("listen/domain");
    }
    if new.metrics_enabled != started.metrics_enabled
        || new.metrics_addr != started.metrics_addr
        || new.metrics_path != started.metrics_path
    {
        restart.push("metrics");
    }
    if new.limits() != started.limits() {
        restart.push("limits");
    }
    if new.origin_allowed != started.origin_allowed
        || new.origin_required != started.origin_required
    {
        restart.push("origin");
    }
    if new.queue_limits() != started.queue_limits()
        || new.ping_interval_ms != started.ping_interval_ms
    {
        restart.push("send queue/burst/ping");
    }
    if new.shutdown_drain_ms != started.shutdown_drain_ms {
        restart.push("shutdown_drain_ms");
    }
    if new.catalog_on_connect != started.catalog_on_connect {
        restart.push("catalog_on_connect");
    }
    restart
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
            log::event(Level::Warn, "reload_refused")
                .field("reason", &e)
                .emit();
            return;
        }
        None => None,
    };
    // `listen` waits for a restart: the security of the reload is checked
    // against the address the daemon serves, not the one the file names
    // now (Spec §7.2, `none` on loopback only).
    let served = new.as_ref().map(|n| DaemonConfig {
        listen: started.listen.clone(),
        ..n.clone()
    });
    let ctx = match served.as_ref().map(ctx_from_daemon_config) {
        Some(Ok((ctx, _))) => Some(ctx),
        Some(Err(e)) => {
            log::event(Level::Warn, "reload_refused")
                .field("reason", &e.to_string())
                .emit();
            return;
        }
        None => None,
    };
    if let Some(r) = rot {
        if let Err(e) = r.reload() {
            log::event(Level::Warn, "reload_refused")
                .field("reason", &format!("TLS certificate: {e}"))
                .emit();
            return;
        }
    }
    let restart = new
        .as_ref()
        .map(|n| settings_that_need_a_restart(started, n))
        .unwrap_or_default();
    if let Some(ctx) = ctx {
        live.replace(ctx);
    }
    // Spec §8.1: a reload applies a changed `log_level`.
    if let Some(n) = &new {
        log::set_level(&n.log_level);
    }
    log::event(Level::Info, "reload")
        .field(
            "applied",
            if new.is_some() {
                "acl, auth, tls certificate, log_level"
            } else {
                "tls certificate"
            },
        )
        .emit();
    if !restart.is_empty() {
        log::event(Level::Warn, "reload_needs_restart")
            .field("settings", &restart.join(", "))
            .emit();
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// zerodds-ws-bridge-1.0 §8.3 with §6: a write the client does not take
    /// within the write timeout (`WouldBlock` from `SO_SNDTIMEO`, or
    /// `TimedOut`) and a socket error fail the connection's span with their
    /// reason; a message not sent because a Close frame went out before
    /// does not.
    #[test]
    fn wsbridge_8_3_a_failed_write_fails_the_connection_span() {
        use std::io::{Error, ErrorKind};
        for kind in [ErrorKind::WouldBlock, ErrorKind::TimedOut] {
            assert_eq!(
                write_failure(&Error::from(kind)).as_deref(),
                Some("write timeout: the client took no frame within ping_interval_ms")
            );
        }
        let reset = write_failure(&Error::from(ErrorKind::ConnectionReset)).unwrap();
        assert!(reset.starts_with("write: "), "{reset}");
        assert_eq!(write_failure(&Error::other(SendError::Closing)), None);
        assert!(write_failure(&Error::other(SendError::Deflate)).is_some());
    }

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

    /// zerodds-ws-bridge-1.0 §4.2: a plain HTTP GET is told from an opening
    /// handshake by the `Upgrade` field; its path is the target without
    /// the query.
    #[cfg(feature = "daemon")]
    #[test]
    fn wsbridge_4_2_a_plain_get_is_told_from_an_opening_handshake() {
        assert_eq!(
            plain_get_path("GET /healthz HTTP/1.1\r\nHost: x\r\n\r\n"),
            Some("/healthz")
        );
        assert_eq!(
            plain_get_path("GET /topics/__catalog__?pretty=1 HTTP/1.1\r\nHost: x\r\n\r\n"),
            Some("/topics/__catalog__")
        );
        assert_eq!(
            plain_get_path("GET /healthz HTTP/1.0\r\n\r\n"),
            Some("/healthz")
        );
        for request in [
            "GET /healthz HTTP/1.1\r\nHost: x\r\nUPGRADE: websocket\r\n\r\n",
            "POST /healthz HTTP/1.1\r\nHost: x\r\n\r\n",
            "HEAD /healthz HTTP/1.1\r\nHost: x\r\n\r\n",
            "GET /healthz\r\n\r\n",
            "GET /healthz HTTP/1.1 extra\r\n\r\n",
            "GET /healthz HTTP/2\r\n\r\n",
            "",
        ] {
            assert_eq!(plain_get_path(request), None, "{request:?}");
        }
        // A field named in the body of the request is no header field.
        assert_eq!(
            plain_get_path("GET /healthz HTTP/1.1\r\nHost: x\r\n\r\nUpgrade: websocket\r\n"),
            Some("/healthz")
        );
        assert_eq!(
            origin_field(
                "GET / HTTP/1.1\r\nOrigin: https://a.example\r\norigin: https://b.example\r\n\r\n"
            )
            .as_deref(),
            Some("https://a.example https://b.example")
        );
        assert_eq!(origin_field("GET / HTTP/1.1\r\nHost: x\r\n\r\n"), None);
    }

    /// zerodds-ws-bridge-1.0 §9.2: a reload applies the ACL, the
    /// authentication and the TLS certificate; a change of the topic map,
    /// of `metrics`, of `limits`, of `origin`, of the TLS file paths, of
    /// the listen address or of the domain waits for a restart, and the
    /// reload names each of them.
    #[test]
    fn wsbridge_9_2_a_reload_names_the_changed_settings_that_need_a_restart() {
        let started = DaemonConfig::default_for_dev();
        let same = started.clone();
        assert!(settings_that_need_a_restart(&started, &same).is_empty());

        let change = |edit: &dyn Fn(&mut DaemonConfig)| {
            let mut new = started.clone();
            edit(&mut new);
            settings_that_need_a_restart(&started, &new)
        };
        assert_eq!(
            change(&|c| c.max_frame_size = started.max_frame_size / 2),
            ["limits"]
        );
        assert_eq!(
            change(&|c| c.max_message_size = started.max_message_size + 1),
            ["limits"]
        );
        assert_eq!(
            change(&|c| c.origin_allowed = vec!["https://app.example.org".to_string()]),
            ["origin"]
        );
        assert_eq!(
            change(&|c| c.origin_required = !started.origin_required),
            ["origin"]
        );
        assert_eq!(
            change(&|c| c.topics.push(TopicConfig {
                name: "Added".to_string(),
                ..Default::default()
            })),
            ["topics"]
        );
        // §1 L6: a tenant added; its topics' ACL changes no setting.
        assert_eq!(
            change(&|c| c.tenants.push(super::super::config::TenantConfig {
                name: "plant-a".to_string(),
                subjects: vec!["alice".to_string()],
                ..Default::default()
            })),
            ["tenants"]
        );
        assert!(
            change(&|c| {
                c.topic_acl.insert(
                    super::super::config::tenant_acl_topic("plant-a", "T"),
                    (vec!["alice".to_string()], Vec::new()),
                );
            })
            .is_empty()
        );
        assert_eq!(
            change(&|c| c.tls_cert_file = "/other/cert.pem".to_string()),
            ["tls file paths"]
        );
        assert_eq!(
            change(&|c| c.tls_key_file = "/other/key.pem".to_string()),
            ["tls file paths"]
        );
        assert_eq!(
            change(&|c| c.tls_client_ca_file = "/other/ca.pem".to_string()),
            ["tls file paths"]
        );
        assert_eq!(
            change(&|c| c.listen = "127.0.0.1:1".to_string()),
            ["listen/domain"]
        );
        assert_eq!(change(&|c| c.shutdown_drain_ms = 5), ["shutdown_drain_ms"]);
        assert_eq!(
            change(&|c| c.catalog_on_connect = true),
            ["catalog_on_connect"]
        );
        assert_eq!(
            change(&|c| c.domain = started.domain + 1),
            ["listen/domain"]
        );
        assert_eq!(
            change(&|c| c.metrics_path = "/internal/metrics".to_string()),
            ["metrics"]
        );
        assert_eq!(change(&|c| c.metrics_enabled = true), ["metrics"]);
        assert_eq!(
            change(&|c| c.metrics_addr = "127.0.0.1:1".to_string()),
            ["metrics"]
        );
        // The daemon-wide keys of §6.
        assert_eq!(
            change(&|c| c.max_send_queue_per_connection = 1),
            ["send queue/burst/ping"]
        );
        assert_eq!(
            change(&|c| c.max_burst_samples = 1),
            ["send queue/burst/ping"]
        );
        assert_eq!(
            change(&|c| c.ping_interval_ms = 0),
            ["send queue/burst/ping"]
        );
        // What the reload applies is not in the list.
        assert!(change(&|c| c.auth_mode = "bearer".to_string()).is_empty());
        // Several changes are all named, in the order of the log line.
        assert_eq!(
            change(&|c| {
                c.origin_required = !started.origin_required;
                c.max_frame_size = started.max_frame_size / 2;
                c.listen = "127.0.0.1:1".to_string();
            }),
            ["listen/domain", "limits", "origin"]
        );
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
