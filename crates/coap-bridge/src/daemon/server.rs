// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CoAP server for `zerodds-coap-bridged`.
//!
//! Spec: `zerodds-coap-bridge-1.0.md` §4 + §9.
//!
//! UDP-only sync server. For each received CoAP datagram:
//! * `POST` / `PUT` with a matching `Uri-Path` → DDS write.
//! * `GET` with `Observe: 0` → observer registry; every new DDS
//!   sample is sent to the peer as a CoAP notify (NON).
//! * `GET` with `Observe: 1` → deregister.
//! * `DELETE` → DDS dispose marker (L1-L3 stub: only acks back).
//!
//! Block-wise (RFC 7959) is handled inline. DTLS (Spec §7.1, feature
//! `dtls`) runs as a second listener through the shared `dispatch`;
//! OSCORE (Spec §7.2, RFC 8613, feature `oscore`) is applied inside
//! `dispatch` — requests are unprotected on entry, responses and
//! Observe notifications protected on exit.

use std::collections::BTreeMap;
use std::net::{SocketAddr, UdpSocket};
use std::string::{String, ToString};
#[cfg(feature = "dtls")]
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::vec::Vec;

#[cfg(feature = "daemon")]
use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
#[cfg(feature = "daemon")]
use zerodds_rtps::wire_types::EntityId;

use crate::blockwise::{BlockReassembler, BlockValue};
use crate::codec::{decode, encode};
use crate::message::{CoapCode, CoapMessage, MessageType};
use crate::option::{CoapOption, OptionValue, numbers};

use super::config::{DaemonConfig, TopicConfig};
#[cfg(all(feature = "daemon", feature = "oscore"))]
use super::oscore_gateway::OscoreReject;
#[cfg(feature = "daemon")]
use super::oscore_gateway::{OscoreExchange, OscoreGateway};
#[cfg(feature = "daemon")]
use super::runtime_common::{
    BridgeMetrics, CatalogSnapshot, SERVICE_NAME, install_signal_watcher, otlp_config_from_env,
    serve_admin_endpoints, spawn_otlp_flush_loop,
};
#[cfg(feature = "daemon")]
use super::security::{
    AclOp, AuthSubject, COAP_OPTION_AUTH_TOKEN, authenticate_coap, authorize,
    ctx_from_daemon_config,
};
#[cfg(feature = "daemon")]
use zerodds_monitor::Registry;
#[cfg(feature = "daemon")]
use zerodds_observability_otlp::OtlpExporter;

/// Admin endpoint address when `metrics.enabled` is set without
/// `metrics.listen`; one port per bridge daemon, see man5.
pub const DEFAULT_METRICS_LISTEN: &str = "127.0.0.1:9093";

/// Daemon top-level error.
#[derive(Debug)]
pub enum ServerError {
    /// A setting that cannot be served (exit 1).
    Config(String),
    /// UDP bind (exit 2).
    Bind(String),
    /// DDS init (exit 3).
    Dds(String),
    /// DTLS (exit 4).
    Dtls(String),
    /// OSCORE setup (exit 4 — security setup, like DTLS).
    Oscore(String),
    /// IO.
    Io(String),
}

impl core::fmt::Display for ServerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Config(m) => write!(f, "config: {m}"),
            Self::Bind(m) => write!(f, "bind: {m}"),
            Self::Dds(m) => write!(f, "dds: {m}"),
            Self::Dtls(m) => write!(f, "dtls: {m}"),
            Self::Oscore(m) => write!(f, "oscore: {m}"),
            Self::Io(m) => write!(f, "io: {m}"),
        }
    }
}

impl std::error::Error for ServerError {}

/// DTLS (`coaps://`) listener: its runtime thread, a shutdown signal, and the
/// slot the accept loop publishes its bound address into.
#[cfg(feature = "dtls")]
struct DtlsHandle {
    thread: JoinHandle<()>,
    shutdown: Arc<tokio::sync::Notify>,
    bound: Arc<Mutex<Option<SocketAddr>>>,
}

/// Daemon handle. Drop initiates shutdown.
pub struct DaemonHandle {
    stop: Arc<AtomicBool>,
    accept_thread: Option<JoinHandle<()>>,
    pump_threads: Vec<JoinHandle<()>>,
    /// Bound address (may differ from the config bind when port=0).
    pub local_addr: String,
    #[cfg(feature = "daemon")]
    admin_thread: Option<JoinHandle<()>>,
    #[cfg(feature = "daemon")]
    otlp_thread: Option<JoinHandle<()>>,
    /// Bound admin address.
    #[cfg(feature = "daemon")]
    pub admin_addr: Option<String>,
    /// SIGHUP reload flag.
    #[cfg(feature = "daemon")]
    pub reload_flag: Arc<AtomicBool>,
    /// Healthz.
    #[cfg(feature = "daemon")]
    pub healthy: Arc<AtomicBool>,
    /// Metric set for tests.
    #[cfg(feature = "daemon")]
    pub metrics: Option<BridgeMetrics>,
    /// DTLS (`coaps://`) listener — present only when `dtls.enabled` + feature.
    #[cfg(feature = "dtls")]
    dtls: Option<DtlsHandle>,
    /// §9.2: what the shutdown needs to end every observation.
    observations: Option<Observations>,
    /// The daemon's DDS participant. [`Self::shutdown`] stops it: dropping
    /// the handle alone would not, because the runtime's worker threads and
    /// the pumps hold `Arc`s of their own.
    #[cfg(feature = "daemon")]
    runtime: Arc<DcpsRuntime>,
}

/// The observe registry and what a notification to it is sent with.
struct Observations {
    socket: UdpSocket,
    topic_state: Arc<Mutex<BTreeMap<String, TopicState>>>,
    mid: Arc<AtomicU16>,
    oscore: Option<Arc<OscoreGateway>>,
}

impl DaemonHandle {
    /// The bound `coaps://` (DTLS) address, once the listener is up.
    #[cfg(feature = "dtls")]
    #[must_use]
    pub fn dtls_local_addr(&self) -> Option<SocketAddr> {
        self.dtls
            .as_ref()
            .and_then(|d| d.bound.lock().ok().and_then(|b| *b))
    }

    /// Whether SIGTERM/SIGINT (or [`Self::shutdown`]) asked the daemon to
    /// stop. The binary waits for it and then runs the shutdown.
    #[must_use]
    pub fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// Spec §9.2: stops taking requests and DDS samples, ends every
    /// observation, then joins the workers and stops the DDS participant.
    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // The pumps first, so that no notification follows the last one.
        for j in self.pump_threads.drain(..) {
            let _ = j.join();
        }
        if let Some(obs) = self.observations.take() {
            end_observations(&obs);
        }
        #[cfg(feature = "dtls")]
        if let Some(d) = self.dtls.take() {
            d.shutdown.notify_one();
            let _ = d.thread.join();
        }
        #[cfg(feature = "daemon")]
        {
            self.healthy.store(false, Ordering::SeqCst);
            if let Some(admin) = self.admin_addr.as_deref() {
                if let Ok(addr) = admin.parse::<SocketAddr>() {
                    let _ = std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(200));
                }
            }
        }
        // Wake socket-recv loop with a self-sent dgram.
        if let Ok(addr) = self.local_addr.parse::<SocketAddr>() {
            if let Ok(sock) = UdpSocket::bind("127.0.0.1:0") {
                let _ = sock.send_to(&[0u8; 4], addr);
            }
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
            // Last, so no pump writes to a stopped runtime. A daemon stopped
            // in a process that keeps running (an embedding, a test)
            // otherwise leaves its participant on the domain.
            self.runtime.shutdown();
        }
    }
}

impl Drop for DaemonHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Observer registry entry — a registered CoAP Observe (RFC 7641) client,
/// reachable either over the plaintext UDP socket (by peer addr) or over a
/// DTLS session (by a channel into that session's task).
#[derive(Debug, Clone)]
enum Observer {
    /// Plaintext: notify by sending to `addr` on the shared UDP socket.
    Udp { addr: SocketAddr, token: Vec<u8> },
    /// DTLS: hand the built notification to the owning session task, which
    /// writes it as DTLS application data. `session` identifies the session so
    /// a deregister only drops this client's registration.
    #[cfg(feature = "dtls")]
    Dtls {
        session: u64,
        token: Vec<u8>,
        tx: tokio::sync::mpsc::UnboundedSender<CoapMessage>,
    },
}

impl Observer {
    fn token(&self) -> &[u8] {
        match self {
            Observer::Udp { token, .. } => token,
            #[cfg(feature = "dtls")]
            Observer::Dtls { token, .. } => token,
        }
    }

    /// True when both observers are the same transport endpoint (UDP peer addr
    /// / DTLS session), ignoring the token — used to scope a deregister.
    fn same_endpoint_as(&self, other: &Observer) -> bool {
        match (self, other) {
            (Observer::Udp { addr: a, .. }, Observer::Udp { addr: b, .. }) => a == b,
            #[cfg(feature = "dtls")]
            (Observer::Dtls { session: a, .. }, Observer::Dtls { session: b, .. }) => a == b,
            #[cfg(feature = "dtls")]
            _ => false,
        }
    }
}

/// A registered observer: its transport endpoint plus, when the register
/// arrived OSCORE-protected, the exchange parameters notifications to it are
/// protected with (Spec §7.2 / RFC 8613 §4.1.3.5.2).
#[derive(Debug, Clone)]
struct RegisteredObserver {
    transport: Observer,
    oscore: Option<OscoreExchange>,
}

/// Topic state.
#[derive(Debug, Default)]
struct TopicState {
    observers: Vec<RegisteredObserver>,
    next_seq: u32,
}

/// Block1 reassembly key (peer + token + path).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct BlockKey {
    peer: SocketAddr,
    token: Vec<u8>,
    path: String,
}
/// zerodds-lint: recursion-depth 64 (start bounded by AST depth)
/// Starts the daemon with the given config.
///
/// # Errors
/// `Bind` (exit 2), `Dds` (exit 3), `Dtls` (exit 4).
#[cfg(feature = "daemon")]
pub fn start(cfg: DaemonConfig) -> Result<DaemonHandle, ServerError> {
    let runtime = RuntimeConfig::from_standard_paths()
        .map_err(|e| ServerError::Dds(format!("runtime configuration: {e}")))?;
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

/// [`start_with_runtime`], reloading on SIGHUP (§9.2): with `load` the
/// configuration is read again and the ACL and auth settings apply to the
/// requests that follow; the DTLS credentials are read again in any case.
/// Changed topics, binds, domain or DTLS paths need a restart and are
/// logged. A configuration that does not load changes nothing.
///
/// # Errors
/// As [`start`].
#[cfg(feature = "daemon")]
#[allow(clippy::too_many_lines)]
pub fn start_with_reload(
    cfg: DaemonConfig,
    runtime_config: RuntimeConfig,
    load: Option<ConfigLoader>,
) -> Result<DaemonHandle, ServerError> {
    eprintln!(
        "[zerodds-coap-bridged] starting bind={} domain={} topics={}",
        cfg.bind,
        cfg.domain,
        cfg.topics.len()
    );

    // 0. Metrics registry + standard counters (§8.2 Prometheus).
    let registry = Arc::new(Registry::new());
    let metrics = BridgeMetrics::register(&registry);
    let healthy = Arc::new(AtomicBool::new(true));
    let reload_flag = Arc::new(AtomicBool::new(false));

    // 0b. Bridge security: auth + ACL (§7.2/§7.3; DTLS via separate ADR).
    // Spec §2: an auth setting that cannot be served is exit 1; exit 4
    // stays for the security setup of DTLS/OSCORE.
    let security_ctx = ctx_from_daemon_config(&cfg).map_err(|e| match e {
        zerodds_bridge_security::SecurityError::Tls(_) => {
            ServerError::Dtls(format!("security: {e}"))
        }
        _ => ServerError::Config(format!("security: {e}")),
    })?;
    let security_ctx = Arc::new(zerodds_bridge_security::LiveSecurity::new(security_ctx));
    // DTLS reloads its credentials on this flag, set after the reload of the
    // configuration has run (both used to read the SIGHUP flag; the second
    // reader would have missed the signal).
    let dtls_reload = Arc::new(AtomicBool::new(false));
    eprintln!(
        "[zerodds-coap-bridged] auth-mode={} acl-entries={} dtls={}",
        cfg.auth_mode,
        cfg.topic_acl.len(),
        if cfg.dtls_enabled {
            "enabled (fail-closed: see below)"
        } else {
            "off"
        },
    );

    // 0c. OSCORE (§7.2, RFC 8613, ADR 0010). Fail closed without the `oscore`
    // feature: the operator asked for object security this binary cannot
    // provide, so refuse to start rather than silently serve unprotected.
    #[cfg(not(feature = "oscore"))]
    let oscore_gw: Option<Arc<OscoreGateway>> = if cfg.oscore.enabled {
        return Err(ServerError::Oscore(
            "oscore.enabled=true but this build has no `oscore` feature (rebuild \
             with --features oscore); refusing to start so the bridge never \
             serves a port expected to be OSCORE-protected in the clear. Set \
             oscore.enabled=false to run without object security."
                .to_string(),
        ));
    } else {
        None
    };
    #[cfg(feature = "oscore")]
    let oscore_gw: Option<Arc<OscoreGateway>> = if cfg.oscore.enabled {
        eprintln!(
            "[zerodds-coap-bridged] oscore enabled sender-id={:02x?} replay-window={}",
            cfg.oscore.sender_id, cfg.oscore.replay_window,
        );
        Some(Arc::new(OscoreGateway::new(cfg.oscore.clone())))
    } else {
        None
    };

    // 1. Bind the UDP socket.
    let socket = UdpSocket::bind(&cfg.bind).map_err(|e| ServerError::Bind(format!("{e}")))?;
    let local_addr = socket
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| cfg.bind.clone());
    socket
        .set_read_timeout(Some(Duration::from_millis(500)))
        .map_err(|e| ServerError::Io(format!("set timeout: {e}")))?;

    eprintln!("[zerodds-coap-bridged] bound on {local_addr}");

    // DTLS (`coaps://`, RFC 7252 §9). Fail closed without the `dtls` feature:
    // the operator asked for DTLS but this binary can't provide it, so refuse to
    // start rather than silently serve plaintext on a port expected to be
    // secured. With the feature, the real coaps:// listener is started further
    // down, once the shared DDS state exists.
    #[cfg(not(feature = "dtls"))]
    if cfg.dtls_enabled {
        return Err(ServerError::Dtls(
            "dtls.enabled=true but this build has no `dtls` feature (rebuild with \
             --features dtls); refusing to start so the bridge never serves \
             plaintext on a port expected to be coaps://. Set dtls.enabled=false \
             to run the plaintext coap:// bridge."
                .to_string(),
        ));
    }

    // 2. DCPS runtime.
    let prefix = zerodds_dcps::participant::bridge_guid_prefix(&local_addr);
    let runtime = DcpsRuntime::start(cfg.domain, prefix, runtime_config)
        .map_err(|e| ServerError::Dds(format!("{e:?}")))?;

    // 3. Reader+writer per topic + path map.
    let mut writers: BTreeMap<String, EntityId> = BTreeMap::new();
    let mut path_to_dds: BTreeMap<String, String> = BTreeMap::new();
    let mut readers: Vec<(String, String, std::sync::mpsc::Receiver<UserSample>)> = Vec::new();
    for topic in &cfg.topics {
        register_topic(
            &runtime,
            topic,
            &mut writers,
            &mut path_to_dds,
            &mut readers,
        )?;
    }

    let stop = Arc::new(AtomicBool::new(false));
    let topic_state: Arc<Mutex<BTreeMap<String, TopicState>>> =
        Arc::new(Mutex::new(BTreeMap::new()));
    let next_mid = Arc::new(AtomicU16::new(1));
    // Block1 (RFC 7959 §2.5) — reassembly per peer+token+path.
    let block1_state: Arc<Mutex<BTreeMap<BlockKey, BlockReassembler>>> =
        Arc::new(Mutex::new(BTreeMap::new()));

    // 4. Pump threads per reader topic.
    let mut pump_threads = Vec::new();
    for (dds_topic_name, coap_path, rx) in readers {
        let stop_c = Arc::clone(&stop);
        let socket_c = socket
            .try_clone()
            .map_err(|e| ServerError::Io(format!("{e}")))?;
        let state_c = Arc::clone(&topic_state);
        let mid_c = Arc::clone(&next_mid);
        let _topic_name = dds_topic_name.clone();
        let path = coap_path.clone();
        let dds_out = Arc::clone(&metrics.dds_samples_out_total);
        let oscore_pump = oscore_gw.clone();
        let h = thread::spawn(move || {
            while !stop_c.load(Ordering::SeqCst) {
                match rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(UserSample::Alive { payload, .. }) => {
                        dds_out.inc();
                        push_notify(&socket_c, &state_c, &path, &payload, &mid_c, &oscore_pump);
                    }
                    Ok(UserSample::Lifecycle { .. }) => {}
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

    let socket_end = socket
        .try_clone()
        .map_err(|e| ServerError::Io(format!("{e}")))?;

    // 5. Accept thread (recv loop).
    let stop_acc = Arc::clone(&stop);
    let socket_acc = socket
        .try_clone()
        .map_err(|e| ServerError::Io(format!("{e}")))?;
    let writers_arc = Arc::new(writers);
    let path_to_dds_arc = Arc::new(path_to_dds);
    // The DTLS server shares the same DDS state — clone the handles before the
    // plaintext accept thread below consumes `writers_arc` / `path_to_dds_arc`.
    #[cfg(feature = "dtls")]
    let dtls_shared = (Arc::clone(&writers_arc), Arc::clone(&path_to_dds_arc));
    let runtime_arc = Arc::clone(&runtime);
    let topic_state_acc = Arc::clone(&topic_state);
    let cfg_arc = Arc::new(cfg.clone());
    let mid_acc = Arc::clone(&next_mid);
    let block1_acc = Arc::clone(&block1_state);
    let metrics_acc = metrics.clone();
    let security_acc = Arc::clone(&security_ctx);
    #[cfg(feature = "dtls")]
    let oscore_dtls = oscore_gw.clone();
    let oscore_acc = oscore_gw.clone();
    let accept_thread = thread::spawn(move || {
        let mut buf = [0u8; 65535];
        while !stop_acc.load(Ordering::SeqCst) {
            match socket_acc.recv_from(&mut buf) {
                Ok((n, peer)) => {
                    let bytes = &buf[..n];
                    metrics_acc.frames_in_total.inc();
                    metrics_acc.bytes_in_total.add(n as u64);
                    if n > cfg_arc.max_message_size {
                        // Spec §4.2 — `4.13 Request Entity Too Large`.
                        if let Ok(req) = decode(bytes) {
                            let resp = make_response(&req, CoapCode::new(4, 13), Vec::new());
                            send_msg(&socket_acc, &resp, peer);
                            metrics_acc.frames_out_total.inc();
                        }
                        metrics_acc.errors_total.inc();
                        continue;
                    }
                    if let Err(e) = handle_request(
                        bytes,
                        peer,
                        &socket_acc,
                        &writers_arc,
                        &path_to_dds_arc,
                        &runtime_arc,
                        &topic_state_acc,
                        &mid_acc,
                        &cfg_arc,
                        &block1_acc,
                        &metrics_acc,
                        &security_acc,
                        &oscore_acc,
                    ) {
                        metrics_acc.errors_total.inc();
                        eprintln!("[zerodds-coap-bridged] handle err from {peer}: {e}");
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
                Err(e) => {
                    if !stop_acc.load(Ordering::SeqCst) {
                        eprintln!("[zerodds-coap-bridged] recv err: {e}");
                    }
                    continue;
                }
            }
        }
    });

    // 5b. DTLS (`coaps://`) listener — shares the DDS state with the plaintext
    // path through the same `dispatch` core (opt-in `dtls` feature, ADR 0011).
    #[cfg(feature = "dtls")]
    let dtls = {
        let (dtls_writers, dtls_path_to_dds) = dtls_shared;
        if cfg.dtls_enabled {
            let dtls_bind = cfg
                .bind_dtls
                .clone()
                .unwrap_or_else(|| default_coaps_bind(&local_addr));
            let shutdown = Arc::new(tokio::sync::Notify::new());
            let bound = Arc::new(Mutex::new(None));
            let thread = spawn_dtls_server(
                dtls_bind,
                cfg.dtls.clone(),
                Arc::clone(&dtls_reload),
                Arc::clone(&shutdown),
                Arc::clone(&bound),
                Arc::clone(&runtime),
                dtls_writers,
                dtls_path_to_dds,
                Arc::clone(&topic_state),
                Arc::clone(&block1_state),
                metrics.clone(),
                Arc::clone(&security_ctx),
                oscore_dtls,
            )?;
            Some(DtlsHandle {
                thread,
                shutdown,
                bound,
            })
        } else {
            // Not serving DTLS — drop the cloned shared handles.
            let _ = (dtls_writers, dtls_path_to_dds, oscore_dtls);
            None
        }
    };

    // 6. Admin-Endpoint (§5.2 Catalog/Healthz + §8.2 Metrics).
    let mut admin_thread: Option<JoinHandle<()>> = None;
    let mut admin_addr: Option<String> = None;
    if cfg.metrics_enabled || !cfg.metrics_addr.is_empty() {
        let bind_str = if cfg.metrics_addr.is_empty() {
            DEFAULT_METRICS_LISTEN.to_string()
        } else {
            cfg.metrics_addr.clone()
        };
        match bind_str.parse::<SocketAddr>() {
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
                    Err(e) => eprintln!("[{SERVICE_NAME}] admin bind error: {e}"),
                }
            }
            Err(e) => eprintln!("[{SERVICE_NAME}] admin addr parse error: {e}"),
        }
    }

    // 7. Signal watcher.
    if let Err(e) = install_signal_watcher(Arc::clone(&stop), Arc::clone(&reload_flag)) {
        eprintln!("[{SERVICE_NAME}] signal watcher init failed: {e}");
    }
    // 7b. SIGHUP reload (§9.2).
    {
        let (stop_r, flag_r, dtls_r, live, started) = (
            Arc::clone(&stop),
            Arc::clone(&reload_flag),
            Arc::clone(&dtls_reload),
            Arc::clone(&security_ctx),
            cfg.clone(),
        );
        let _ = thread::Builder::new()
            .name("zerodds-coap-reload".into())
            .spawn(move || {
                while !stop_r.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(100));
                    if flag_r.swap(false, Ordering::SeqCst) {
                        if let Some(l) = &load {
                            reload(&live, &started, l());
                        }
                        dtls_r.store(true, Ordering::SeqCst);
                    }
                }
            });
    }

    // 8. OTLP.
    let otlp_thread = if let Some(otlp_cfg) = otlp_config_from_env(SERVICE_NAME) {
        let exp = Arc::new(OtlpExporter::new(otlp_cfg));
        match spawn_otlp_flush_loop(exp, Arc::clone(&stop), Duration::from_secs(5)) {
            Ok(h) => Some(h),
            Err(e) => {
                eprintln!("[{SERVICE_NAME}] OTLP spawn failed: {e}");
                None
            }
        }
    } else {
        None
    };

    let observations = Some(Observations {
        socket: socket_end,
        topic_state: Arc::clone(&topic_state),
        mid: Arc::clone(&next_mid),
        oscore: oscore_gw.clone(),
    });
    Ok(DaemonHandle {
        observations,
        stop,
        accept_thread: Some(accept_thread),
        pump_threads,
        local_addr,
        admin_thread,
        otlp_thread,
        admin_addr,
        reload_flag,
        healthy,
        metrics: Some(metrics),
        #[cfg(feature = "dtls")]
        dtls,
        runtime,
    })
}

/// What a dispatched request wants done with the observe registry. The
/// transport-specific caller applies it with its own [`Observer`] flavour.
#[cfg(feature = "daemon")]
enum ObserveDirective {
    None,
    Register {
        path: String,
        token: Vec<u8>,
        /// Set when the register arrived OSCORE-protected — notifications to
        /// this observer are protected with these exchange parameters.
        oscore: Option<OscoreExchange>,
    },
    Deregister {
        path: String,
        token: Vec<u8>,
    },
}

/// Transport-agnostic outcome of handling one CoAP request: the responses to
/// send back (in order) + an observe-registry directive. DDS writes and the
/// per-request metric side effects already happened inside [`dispatch`]; the
/// caller only performs the transport I/O and applies the observe directive
/// with its transport's observer kind.
#[cfg(feature = "daemon")]
struct Dispatched {
    responses: Vec<CoapMessage>,
    observe: ObserveDirective,
}

/// Handle one CoAP request against the DDS runtime. Shared by the plaintext
/// UDP path ([`handle_request`]) and the DTLS path (`super::dtls_server`) so
/// both transports behave identically (no drift). With OSCORE enabled
/// (Spec §7.2, RFC 8613) the request is unprotected here and every response
/// protected before it leaves; unprotected requests (other than
/// `/.well-known/core`) are refused with `4.01 Unauthorized`.
#[cfg(feature = "daemon")]
#[allow(clippy::too_many_arguments)]
fn dispatch(
    bytes: &[u8],
    peer: SocketAddr,
    writers: &Arc<BTreeMap<String, EntityId>>,
    path_to_dds: &Arc<BTreeMap<String, String>>,
    runtime: &Arc<DcpsRuntime>,
    block1_state: &Arc<Mutex<BTreeMap<BlockKey, BlockReassembler>>>,
    metrics: &BridgeMetrics,
    security: &Arc<zerodds_bridge_security::LiveSecurity>,
    oscore: &Option<Arc<OscoreGateway>>,
    dtls_peer: Option<&AuthSubject>,
) -> Result<Dispatched, String> {
    let outer = decode(bytes).map_err(|e| format!("decode: {e}"))?;

    // §7.2 OSCORE gate — unprotect (RFC 8613 §8.2) or refuse.
    #[cfg(feature = "oscore")]
    let (req, exchange): (CoapMessage, Option<OscoreExchange>) = match oscore.as_ref() {
        Some(gw) => match gw.unprotect_request(&outer)? {
            Ok((inner, ex)) => (inner, Some(ex)),
            Err(OscoreReject::NotProtected) => {
                if extract_uri_path(&outer) == ".well-known/core" {
                    // The discovery catalog stays reachable in the clear.
                    (outer, None)
                } else {
                    metrics.errors_total.inc();
                    return Ok(Dispatched {
                        responses: vec![make_response(
                            &outer,
                            CoapCode::new(4, 1),
                            b"oscore required".to_vec(),
                        )],
                        observe: ObserveDirective::None,
                    });
                }
            }
            Err(OscoreReject::Replay) => {
                // §7.4 — replayed Partial IV.
                metrics.errors_total.inc();
                metrics.oscore_replays_total.inc();
                return Ok(Dispatched {
                    responses: vec![make_response(
                        &outer,
                        CoapCode::new(4, 1),
                        b"oscore replay".to_vec(),
                    )],
                    observe: ObserveDirective::None,
                });
            }
            Err(OscoreReject::Verify) => {
                // §8.2 — decompression/verification failure.
                metrics.errors_total.inc();
                return Ok(Dispatched {
                    responses: vec![make_response(
                        &outer,
                        CoapCode::new(4, 0),
                        b"oscore verification failed".to_vec(),
                    )],
                    observe: ObserveDirective::None,
                });
            }
        },
        None => (outer, None),
    };
    #[cfg(not(feature = "oscore"))]
    let req = {
        let _ = oscore;
        outer
    };

    #[allow(unused_mut)]
    let mut d = dispatch_core(
        &req,
        peer,
        writers,
        path_to_dds,
        runtime,
        block1_state,
        metrics,
        security,
        dtls_peer,
    )?;

    // §8.3 — protect every response of an OSCORE exchange before it leaves.
    #[cfg(feature = "oscore")]
    if let (Some(gw), Some(ex)) = (oscore.as_ref(), exchange) {
        let mut protected = Vec::with_capacity(d.responses.len());
        for resp in &d.responses {
            protected.push(
                gw.protect_response(&ex, resp)
                    .map_err(|e| format!("oscore: {e}"))?,
            );
        }
        d.responses = protected;
        if let ObserveDirective::Register { oscore: slot, .. } = &mut d.observe {
            *slot = Some(ex);
        }
    }
    Ok(d)
}

/// The transport- and security-agnostic request core: the (already
/// unprotected) request against the DDS runtime.
#[cfg(feature = "daemon")]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn dispatch_core(
    req: &CoapMessage,
    peer: SocketAddr,
    writers: &Arc<BTreeMap<String, EntityId>>,
    path_to_dds: &Arc<BTreeMap<String, String>>,
    runtime: &Arc<DcpsRuntime>,
    block1_state: &Arc<Mutex<BTreeMap<BlockKey, BlockReassembler>>>,
    metrics: &BridgeMetrics,
    security: &Arc<zerodds_bridge_security::LiveSecurity>,
    dtls_peer: Option<&AuthSubject>,
) -> Result<Dispatched, String> {
    let mut out = Dispatched {
        responses: Vec::new(),
        observe: ObserveDirective::None,
    };
    let path = extract_uri_path(req);

    // `/.well-known/core` → resource catalog (open, no auth).
    if path == ".well-known/core" {
        let body = render_well_known_core(path_to_dds);
        let mut resp = make_response(req, CoapCode::CONTENT, body.into_bytes());
        resp.options.push(CoapOption::content_format(40)); // application/link-format
        out.responses.push(resp);
        return Ok(out);
    }

    // §7.2 — Authentication via CoAP-Option 65000 (Bearer-Token).
    let auth_token = extract_auth_token(req);
    let subject =
        match authenticate_coap(&security.current().auth, auth_token.as_deref(), dtls_peer) {
            Ok(s) => s,
            Err(_e) => {
                metrics.errors_total.inc();
                // 4.01 Unauthorized.
                out.responses.push(make_response(
                    req,
                    CoapCode::new(4, 1),
                    b"unauthorized".to_vec(),
                ));
                return Ok(out);
            }
        };

    if req.code == CoapCode::POST || req.code == CoapCode::PUT {
        // POST/PUT → DDS write. Optional Block1 (RFC 7959 §2.5).
        let dds_topic = path_to_dds
            .get(&path)
            .ok_or_else(|| format!("unknown path: {path}"))?;
        // §7.3 — ACL write check.
        if !authorize(&security.current().acl, &subject, AclOp::Write, dds_topic) {
            metrics.errors_total.inc();
            // 4.03 Forbidden.
            out.responses.push(make_response(
                req,
                CoapCode::new(4, 3),
                b"forbidden".to_vec(),
            ));
            return Ok(out);
        }
        let eid = writers.get(dds_topic).ok_or("no writer")?;

        // Block1 path: collect chunks until !more, then DDS write.
        if let Some(b1) = extract_block(req, numbers::BLOCK1) {
            let key = BlockKey {
                peer,
                token: req.token.clone(),
                path: path.clone(),
            };
            let assembled = {
                let mut map = block1_state.lock().map_err(|_| "block1 mutex poisoned")?;
                let entry = map
                    .entry(key.clone())
                    .or_insert_with(|| BlockReassembler::new(b1.block_size().unwrap_or(64)));
                entry
                    .accept(b1, &req.payload)
                    .map_err(|e| format!("block1 reassemble: {e}"))?;
                if entry.is_complete() {
                    // SAFETY: just verified is_complete(); remove() returns Some.
                    #[allow(clippy::expect_used)]
                    let r = map.remove(&key).expect("just complete");
                    Some(r.into_payload())
                } else {
                    None
                }
            };
            if let Some(payload) = assembled {
                runtime
                    .write_user_sample(*eid, payload)
                    .map_err(|e| format!("dds-write: {e:?}"))?;
                metrics.dds_samples_in_total.inc();
                // 2.04 Changed with Block1 echo (more=false).
                let mut resp = make_response(req, CoapCode::CHANGED, Vec::new());
                resp.options.push(CoapOption::block1(b1.num, false, b1.szx));
                out.responses.push(resp);
            } else {
                // 2.31 Continue with Block1 echo (more=true).
                let mut resp = make_response(req, CoapCode::new(2, 31), Vec::new());
                resp.options.push(CoapOption::block1(b1.num, true, b1.szx));
                out.responses.push(resp);
            }
            return Ok(out);
        }

        runtime
            .write_user_sample(*eid, req.payload.clone())
            .map_err(|e| format!("dds-write: {e:?}"))?;
        metrics.dds_samples_in_total.inc();
        out.responses
            .push(make_response(req, CoapCode::CHANGED, Vec::new()));
        return Ok(out);
    }

    if req.code == CoapCode::GET {
        let observe = extract_observe(req);
        let dds_topic = path_to_dds
            .get(&path)
            .ok_or_else(|| format!("unknown path: {path}"))?;
        // §7.3 — ACL read check for observe-register and plain GET.
        if !authorize(&security.current().acl, &subject, AclOp::Read, dds_topic) {
            metrics.errors_total.inc();
            out.responses.push(make_response(
                req,
                CoapCode::new(4, 3),
                b"forbidden".to_vec(),
            ));
            return Ok(out);
        }
        match observe {
            Some(0) => {
                // Register — the caller inserts the transport-specific observer.
                out.observe = ObserveDirective::Register {
                    path: path.clone(),
                    token: req.token.clone(),
                    oscore: None,
                };
                // Initial response 2.05 with observe seq 0 (Spec §4.3).
                let mut resp = make_response(req, CoapCode::CONTENT, Vec::new());
                resp.options.push(CoapOption::observe(0));
                resp.options.push(CoapOption::content_format(65000));
                out.responses.push(resp);
                let _ = dds_topic; // placeholder for catalog
                return Ok(out);
            }
            Some(1) => {
                // Deregister.
                out.observe = ObserveDirective::Deregister {
                    path: path.clone(),
                    token: req.token.clone(),
                };
                out.responses
                    .push(make_response(req, CoapCode::CONTENT, Vec::new()));
                return Ok(out);
            }
            _ => {
                // Plain GET — empty content (spec says 2.05 with the current
                // sample; we have no cache layer in L1-L4).
                out.responses
                    .push(make_response(req, CoapCode::CONTENT, Vec::new()));
                return Ok(out);
            }
        }
    }

    if req.code == CoapCode::DELETE {
        // DELETE → dispose marker (L1-L3 stub: only 2.02 back).
        out.responses
            .push(make_response(req, CoapCode::DELETED, Vec::new()));
        return Ok(out);
    }

    // Unknown code — 4.00 Bad Request.
    out.responses
        .push(make_response(req, CoapCode::BAD_REQUEST, Vec::new()));
    Ok(out)
}

/// Apply an [`ObserveDirective`] to the shared observer registry. `make_observer`
/// builds the transport-specific observer for a register and, for a deregister,
/// a probe used to scope the removal to this client's endpoint.
#[cfg(feature = "daemon")]
fn apply_observe(
    directive: ObserveDirective,
    topic_state: &Arc<Mutex<BTreeMap<String, TopicState>>>,
    metrics: &BridgeMetrics,
    make_observer: &dyn Fn(Vec<u8>) -> Observer,
) {
    match directive {
        ObserveDirective::None => {}
        ObserveDirective::Register {
            path,
            token,
            oscore,
        } => {
            let obs = RegisteredObserver {
                transport: make_observer(token),
                oscore,
            };
            if let Ok(mut st) = topic_state.lock() {
                st.entry(path).or_default().observers.push(obs);
            }
            metrics.connections_total.inc();
            metrics.connections_active.inc();
        }
        ObserveDirective::Deregister { path, token } => {
            let probe = make_observer(token.clone());
            if let Ok(mut st) = topic_state.lock() {
                if let Some(entry) = st.get_mut(&path) {
                    let before = entry.observers.len();
                    entry.observers.retain(|o| {
                        !(o.transport.same_endpoint_as(&probe)
                            && o.transport.token() == token.as_slice())
                    });
                    let removed = before - entry.observers.len();
                    for _ in 0..removed {
                        metrics.connections_active.dec();
                    }
                }
            }
        }
    }
}

/// Plaintext UDP path: dispatch + send the responses over the socket + apply
/// the observe directive with a UDP (`peer`-addressed) observer.
#[cfg(feature = "daemon")]
#[allow(clippy::too_many_arguments)]
fn handle_request(
    bytes: &[u8],
    peer: SocketAddr,
    socket: &UdpSocket,
    writers: &Arc<BTreeMap<String, EntityId>>,
    path_to_dds: &Arc<BTreeMap<String, String>>,
    runtime: &Arc<DcpsRuntime>,
    topic_state: &Arc<Mutex<BTreeMap<String, TopicState>>>,
    _mid: &Arc<AtomicU16>,
    _cfg: &Arc<DaemonConfig>,
    block1_state: &Arc<Mutex<BTreeMap<BlockKey, BlockReassembler>>>,
    metrics: &BridgeMetrics,
    security: &Arc<zerodds_bridge_security::LiveSecurity>,
    oscore: &Option<Arc<OscoreGateway>>,
) -> Result<(), String> {
    let d = dispatch(
        bytes,
        peer,
        writers,
        path_to_dds,
        runtime,
        block1_state,
        metrics,
        security,
        oscore,
        None,
    )?;
    for resp in &d.responses {
        send_msg(socket, resp, peer);
        metrics.frames_out_total.inc();
    }
    apply_observe(d.observe, topic_state, metrics, &|token| Observer::Udp {
        addr: peer,
        token,
    });
    Ok(())
}

#[cfg(feature = "daemon")]
fn extract_auth_token(msg: &CoapMessage) -> Option<Vec<u8>> {
    for opt in &msg.options {
        if opt.number == COAP_OPTION_AUTH_TOKEN {
            return match &opt.value {
                OptionValue::Opaque(b) => Some(b.clone()),
                OptionValue::String(s) => Some(s.as_bytes().to_vec()),
                _ => None,
            };
        }
    }
    None
}

fn extract_uri_path(msg: &CoapMessage) -> String {
    // Spec §3.2 Uri-Path options are carried as strings; on the wire,
    // though, they may arrive as opaque (the decoder does not normalize).
    // We accept both variants.
    let mut buf = String::new();
    let mut first = true;
    for opt in &msg.options {
        if opt.number == numbers::URI_PATH {
            let segment = match &opt.value {
                OptionValue::String(s) => s.clone(),
                OptionValue::Opaque(b) => String::from_utf8_lossy(b).into_owned(),
                OptionValue::Empty => String::new(),
                OptionValue::Uint(v) => format!("{v}"),
            };
            if !first {
                buf.push('/');
            }
            buf.push_str(&segment);
            first = false;
        }
    }
    buf
}

/// Extracts the Block1 or Block2 option from a CoapMessage.
/// Spec RFC 7959 §2.1.
fn extract_block(msg: &CoapMessage, option_number: u16) -> Option<BlockValue> {
    for opt in &msg.options {
        if opt.number != option_number {
            continue;
        }
        let bytes: Vec<u8> = match &opt.value {
            OptionValue::Opaque(b) => b.clone(),
            OptionValue::Uint(v) => {
                let raw = *v;
                if raw < 0x100 {
                    vec![raw as u8]
                } else if raw < 0x1_0000 {
                    vec![((raw >> 8) & 0xff) as u8, (raw & 0xff) as u8]
                } else {
                    vec![
                        ((raw >> 16) & 0xff) as u8,
                        ((raw >> 8) & 0xff) as u8,
                        (raw & 0xff) as u8,
                    ]
                }
            }
            OptionValue::Empty => vec![0u8],
            _ => continue,
        };
        if let Ok(v) = BlockValue::decode(&bytes) {
            return Some(v);
        }
    }
    None
}

fn extract_observe(msg: &CoapMessage) -> Option<u32> {
    use crate::option::uint_from_bytes;
    for opt in &msg.options {
        if opt.number == numbers::OBSERVE {
            let v = match &opt.value {
                OptionValue::Uint(v) => Some(u32::try_from(*v).unwrap_or(u32::MAX)),
                OptionValue::Opaque(b) => {
                    uint_from_bytes(b).map(|v| u32::try_from(v).unwrap_or(u32::MAX))
                }
                OptionValue::Empty => Some(0),
                _ => None,
            };
            if v.is_some() {
                return v;
            }
        }
    }
    None
}

fn render_well_known_core(path_to_dds: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    let mut first = true;
    for path in path_to_dds.keys() {
        if !first {
            out.push(',');
        }
        out.push('<');
        out.push('/');
        out.push_str(path);
        out.push_str(">;rt=\"dds.topic\";ct=65000");
        first = false;
    }
    out
}

fn make_response(req: &CoapMessage, code: CoapCode, payload: Vec<u8>) -> CoapMessage {
    // ACK for a CON request, otherwise NON.
    let mtype = if matches!(req.message_type, MessageType::Confirmable) {
        MessageType::Acknowledgement
    } else {
        MessageType::NonConfirmable
    };
    CoapMessage {
        version: 1,
        message_type: mtype,
        code,
        message_id: req.message_id,
        token: req.token.clone(),
        options: Vec::new(),
        payload,
    }
}

fn send_msg(socket: &UdpSocket, msg: &CoapMessage, peer: SocketAddr) {
    if let Ok(bytes) = encode(msg) {
        let _ = socket.send_to(&bytes, peer);
    }
}

/// RFC 7641 §4.2: a notification with a non-2.xx code ends an observation
/// on the client. At shutdown every observer gets 5.03 Service Unavailable
/// without an Observe option (what a GET would now return) and the
/// registry is emptied. Before, the observers were dropped silently and a
/// client kept waiting for notifications that would never come.
fn end_observations(obs: &Observations) {
    let observers: Vec<RegisteredObserver> = match obs.topic_state.lock() {
        Ok(mut st) => st
            .values_mut()
            .flat_map(|e| std::mem::take(&mut e.observers))
            .collect(),
        Err(_) => return,
    };
    #[cfg(feature = "dtls")]
    let mut over_dtls = false;
    for o in &observers {
        let msg = CoapMessage {
            version: 1,
            message_type: MessageType::NonConfirmable,
            code: CoapCode::new(5, 3),
            message_id: obs.mid.fetch_add(1, Ordering::SeqCst),
            token: o.transport.token().to_vec(),
            options: Vec::new(),
            payload: b"shutting down".to_vec(),
        };
        let msg = match (&o.oscore, obs.oscore.as_ref()) {
            (None, _) => msg,
            #[cfg(feature = "oscore")]
            (Some(ex), Some(gw)) => match gw.protect_notification(ex, &msg) {
                Ok(protected) => protected,
                Err(_) => continue,
            },
            (Some(_), _) => continue,
        };
        match &o.transport {
            Observer::Udp { addr, .. } => send_msg(&obs.socket, &msg, *addr),
            #[cfg(feature = "dtls")]
            Observer::Dtls { tx, .. } => {
                over_dtls = true;
                let _ = tx.send(msg);
            }
        }
    }
    // A DTLS notification is written by its session task; give the tasks
    // the moment to do so before the DTLS listener stops.
    #[cfg(feature = "dtls")]
    if over_dtls {
        thread::sleep(Duration::from_millis(200));
    }
    if !observers.is_empty() {
        eprintln!(
            "[zerodds-coap-bridged] ended {} observation(s) with 5.03",
            observers.len()
        );
    }
}

fn push_notify(
    socket: &UdpSocket,
    topic_state: &Arc<Mutex<BTreeMap<String, TopicState>>>,
    path: &str,
    payload: &[u8],
    mid: &Arc<AtomicU16>,
    oscore: &Option<Arc<OscoreGateway>>,
) {
    let observers = match topic_state.lock() {
        Ok(mut st) => {
            let entry = st.entry(path.to_string()).or_default();
            entry.next_seq = entry.next_seq.wrapping_add(1).max(1);
            let seq = entry.next_seq;
            entry
                .observers
                .iter()
                .map(|o| (o.clone(), seq))
                .collect::<Vec<_>>()
        }
        Err(_) => return,
    };
    for (obs, seq) in observers {
        let new_mid = mid.fetch_add(1, Ordering::SeqCst);
        let msg = CoapMessage {
            version: 1,
            message_type: MessageType::NonConfirmable,
            code: CoapCode::CONTENT,
            message_id: new_mid,
            token: obs.transport.token().to_vec(),
            options: vec![CoapOption::observe(seq), CoapOption::content_format(65000)],
            payload: payload.to_vec(),
        };
        // §7.2 — a notification to an OSCORE observer leaves protected or not
        // at all (RFC 8613 §4.1.3.5.2: the daemon's own Partial IV).
        let msg = match (&obs.oscore, oscore.as_ref()) {
            (None, _) => msg,
            #[cfg(feature = "oscore")]
            (Some(ex), Some(gw)) => match gw.protect_notification(ex, &msg) {
                Ok(protected) => protected,
                Err(e) => {
                    eprintln!("[zerodds-coap-bridged] oscore notify protect failed: {e}");
                    continue;
                }
            },
            (Some(_), _) => {
                // Registered protected, but no gateway in this build/run —
                // never send the sample in the clear.
                eprintln!("[zerodds-coap-bridged] oscore observer without gateway; notify dropped");
                continue;
            }
        };
        match obs.transport {
            Observer::Udp { addr, .. } => send_msg(socket, &msg, addr),
            // DTLS: hand the built notification to the owning session task,
            // which writes it as DTLS application data. A closed channel means
            // the session ended; the stale observer is reaped on its next
            // deregister / topic sweep.
            #[cfg(feature = "dtls")]
            Observer::Dtls { tx, .. } => {
                let _ = tx.send(msg);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// DTLS (`coaps://`) serving — RFC 7252 §9, opt-in `dtls` feature (ADR 0011).
// Runs the webrtc-dtls record layer on its own tokio runtime thread and feeds
// accepted sessions through the *same* `dispatch` core as the plaintext path,
// so request/response + CoAP Observe behave identically over coaps://.
// ---------------------------------------------------------------------------

/// Default `coaps://` bind: same host as the plaintext bind, port 5684.
#[cfg(feature = "dtls")]
fn default_coaps_bind(plain: &str) -> String {
    match plain.parse::<SocketAddr>() {
        Ok(a) => SocketAddr::new(a.ip(), crate::dtls_transport::COAPS_PORT).to_string(),
        Err(_) => format!("0.0.0.0:{}", crate::dtls_transport::COAPS_PORT),
    }
}

/// Spawn the DTLS server on its own thread (hosting a tokio runtime). The bound
/// address is published into `bound` once the listener is up.
#[cfg(feature = "dtls")]
#[allow(clippy::too_many_arguments)]
fn spawn_dtls_server(
    bind_addr: String,
    credentials: crate::dtls::DtlsCredentials,
    reload_flag: Arc<AtomicBool>,
    shutdown: Arc<tokio::sync::Notify>,
    bound: Arc<Mutex<Option<SocketAddr>>>,
    runtime: Arc<DcpsRuntime>,
    writers: Arc<BTreeMap<String, EntityId>>,
    path_to_dds: Arc<BTreeMap<String, String>>,
    topic_state: Arc<Mutex<BTreeMap<String, TopicState>>>,
    block1_state: Arc<Mutex<BTreeMap<BlockKey, BlockReassembler>>>,
    metrics: BridgeMetrics,
    security: Arc<zerodds_bridge_security::LiveSecurity>,
    oscore: Option<Arc<OscoreGateway>>,
) -> Result<JoinHandle<()>, ServerError> {
    let addr: SocketAddr = bind_addr
        .parse()
        .map_err(|e| ServerError::Dtls(format!("dtls bind addr {bind_addr}: {e}")))?;
    thread::Builder::new()
        .name("coap-dtls".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("[zerodds-coap-bridged] dtls tokio runtime: {e}");
                    return;
                }
            };
            rt.block_on(dtls_accept_loop(
                addr,
                credentials,
                reload_flag,
                shutdown,
                bound,
                runtime,
                writers,
                path_to_dds,
                topic_state,
                block1_state,
                metrics,
                security,
                oscore,
            ));
        })
        .map_err(|e| ServerError::Io(format!("spawn dtls thread: {e}")))
}

#[cfg(feature = "dtls")]
#[allow(clippy::too_many_arguments)]
async fn dtls_accept_loop(
    addr: SocketAddr,
    credentials: crate::dtls::DtlsCredentials,
    reload_flag: Arc<AtomicBool>,
    shutdown: Arc<tokio::sync::Notify>,
    bound: Arc<Mutex<Option<SocketAddr>>>,
    runtime: Arc<DcpsRuntime>,
    writers: Arc<BTreeMap<String, EntityId>>,
    path_to_dds: Arc<BTreeMap<String, String>>,
    topic_state: Arc<Mutex<BTreeMap<String, TopicState>>>,
    block1_state: Arc<Mutex<BTreeMap<BlockKey, BlockReassembler>>>,
    metrics: BridgeMetrics,
    security: Arc<zerodds_bridge_security::LiveSecurity>,
    oscore: Option<Arc<OscoreGateway>>,
) {
    let server = match crate::dtls_transport::DtlsCoapServer::bind_with(addr, &credentials).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[zerodds-coap-bridged] dtls bind {addr}: {e}");
            return;
        }
    };
    if let Ok(la) = server.local_addr().await {
        if let Ok(mut slot) = bound.lock() {
            *slot = Some(la);
        }
        eprintln!("[zerodds-coap-bridged] coaps:// (DTLS) listening on {la}");
    }
    let session_ctr = AtomicU64::new(1);
    loop {
        // Spec §7.1: "Cert-Rotation via SIGHUP (Daemon liest cert/key neu,
        // Sessions halten)." The socket and the accepted sessions survive;
        // only the configuration the next handshake uses is replaced.
        if reload_flag.swap(false, Ordering::SeqCst) {
            match server.rotate(&credentials) {
                Ok(()) => eprintln!("[zerodds-coap-bridged] dtls credentials reloaded (SIGHUP)"),
                Err(e) => eprintln!(
                    "[zerodds-coap-bridged] dtls reload failed, keeping the \
                     previous credentials: {e}"
                ),
            }
        }
        tokio::select! {
            () = shutdown.notified() => break,
            () = tokio::time::sleep(std::time::Duration::from_millis(200)) => continue,
            accepted = server.accept() => {
                let session = match accepted {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("[zerodds-coap-bridged] dtls accept: {e}");
                        continue;
                    }
                };
                let sid = session_ctr.fetch_add(1, Ordering::SeqCst);
                metrics.connections_total.inc();
                tokio::spawn(dtls_session_task(
                    session,
                    sid,
                    Arc::clone(&runtime),
                    Arc::clone(&writers),
                    Arc::clone(&path_to_dds),
                    Arc::clone(&topic_state),
                    Arc::clone(&block1_state),
                    metrics.clone(),
                    Arc::clone(&security),
                    oscore.clone(),
                ));
            }
        }
    }
}

/// One accepted DTLS session: `select!` between inbound CoAP requests (handled
/// by the shared `dispatch`) and outbound observe notifications pushed by the
/// DDS pump threads through this session's channel.
#[cfg(feature = "dtls")]
#[allow(clippy::too_many_arguments)]
async fn dtls_session_task(
    session: crate::dtls_transport::DtlsCoapSession,
    sid: u64,
    runtime: Arc<DcpsRuntime>,
    writers: Arc<BTreeMap<String, EntityId>>,
    path_to_dds: Arc<BTreeMap<String, String>>,
    topic_state: Arc<Mutex<BTreeMap<String, TopicState>>>,
    block1_state: Arc<Mutex<BTreeMap<BlockKey, BlockReassembler>>>,
    metrics: BridgeMetrics,
    security: Arc<zerodds_bridge_security::LiveSecurity>,
    oscore: Option<Arc<OscoreGateway>>,
) {
    let peer = session.peer_addr();
    // §7.2 `mtls`: the identity of the DTLS handshake — `mtls:<SHA-256>`
    // of the client certificate as in every bridge, or `psk:<identity>`.
    let dtls_peer = session.peer_identity().await.map(|id| match id {
        crate::dtls_transport::DtlsPeerIdentity::Certificate(der) => {
            zerodds_bridge_security::mtls_subject_from_der(&der)
        }
        crate::dtls_transport::DtlsPeerIdentity::Psk(identity) => {
            AuthSubject::new(format!("psk:{}", String::from_utf8_lossy(&identity)))
        }
    });
    let (notify_tx, mut notify_rx) = tokio::sync::mpsc::unbounded_channel::<CoapMessage>();
    loop {
        tokio::select! {
            recv = session.recv_message() => {
                let req = match recv {
                    Ok(m) => m,
                    Err(_) => break, // peer closed / DTLS error
                };
                let bytes = match encode(&req) {
                    Ok(b) => b,
                    Err(_) => { metrics.errors_total.inc(); continue; }
                };
                metrics.frames_in_total.inc();
                metrics.bytes_in_total.add(bytes.len() as u64);
                match dispatch(
                    &bytes, peer, &writers, &path_to_dds, &runtime,
                    &block1_state, &metrics, &security, &oscore, dtls_peer.as_ref(),
                ) {
                    Ok(d) => {
                        for resp in &d.responses {
                            if session.send_message(resp).await.is_ok() {
                                metrics.frames_out_total.inc();
                            }
                        }
                        let tx = notify_tx.clone();
                        apply_observe(d.observe, &topic_state, &metrics, &move |token| {
                            Observer::Dtls { session: sid, token, tx: tx.clone() }
                        });
                    }
                    Err(e) => {
                        metrics.errors_total.inc();
                        eprintln!("[zerodds-coap-bridged] dtls handle err from {peer}: {e}");
                    }
                }
            }
            note = notify_rx.recv() => {
                if let Some(msg) = note {
                    let _ = session.send_message(&msg).await;
                }
            }
        }
    }
    // Session ended — reap its observers + keep `connections_active` accurate.
    if let Ok(mut st) = topic_state.lock() {
        for entry in st.values_mut() {
            let before = entry.observers.len();
            entry.observers.retain(
                |o| !matches!(&o.transport, Observer::Dtls { session, .. } if *session == sid),
            );
            for _ in 0..(before - entry.observers.len()) {
                metrics.connections_active.dec();
            }
        }
    }
    let _ = session.close().await;
}

#[cfg(feature = "daemon")]
fn register_topic(
    rt: &Arc<DcpsRuntime>,
    topic: &TopicConfig,
    writers: &mut BTreeMap<String, EntityId>,
    path_to_dds: &mut BTreeMap<String, String>,
    readers: &mut Vec<(String, String, std::sync::mpsc::Receiver<UserSample>)>,
) -> Result<(), ServerError> {
    use zerodds_qos::DurabilityKind;
    let durability = match topic.durability.as_str() {
        "transient_local" => DurabilityKind::TransientLocal,
        "transient" => DurabilityKind::Transient,
        "persistent" => DurabilityKind::Persistent,
        _ => DurabilityKind::Volatile,
    };
    let reliable = !matches!(topic.reliability.as_str(), "best_effort");
    let want_writer = matches!(topic.direction.as_str(), "in" | "bidir");
    let want_reader = matches!(topic.direction.as_str(), "out" | "bidir");

    if want_reader {
        let (_eid, rx) = rt
            .register_user_reader(UserReaderConfig {
                topic_name: topic.dds_name.clone(),
                type_name: topic.dds_type.clone(),
                reliable,
                durability,
                ..Default::default()
            })
            .map_err(|e| ServerError::Dds(format!("reader: {e:?}")))?;
        readers.push((topic.dds_name.clone(), topic.coap_uri_path.clone(), rx));
    }

    if want_writer {
        let eid = rt
            .register_user_writer(UserWriterConfig {
                topic_name: topic.dds_name.clone(),
                type_name: topic.dds_type.clone(),
                reliable,
                durability,
                ..Default::default()
            })
            .map_err(|e| ServerError::Dds(format!("writer: {e:?}")))?;
        writers.insert(topic.dds_name.clone(), eid);
    }
    path_to_dds.insert(topic.coap_uri_path.clone(), topic.dds_name.clone());
    Ok(())
}

/// One SIGHUP reload of the configuration (see [`start_with_reload`]).
#[cfg(feature = "daemon")]
fn reload(
    live: &zerodds_bridge_security::LiveSecurity,
    started: &DaemonConfig,
    new: Result<DaemonConfig, String>,
) {
    let new = match new {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[{SERVICE_NAME}] reload refused: {e}");
            return;
        }
    };
    let ctx = match ctx_from_daemon_config(&new) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[{SERVICE_NAME}] reload refused: {e}");
            return;
        }
    };
    let mut restart: Vec<&str> = Vec::new();
    if new.topics != started.topics {
        restart.push("topics");
    }
    if new.bind != started.bind || new.domain != started.domain {
        restart.push("bind/domain");
    }
    live.replace(ctx);
    eprintln!("[{SERVICE_NAME}] reloaded: acl, auth");
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

    fn req_with_path(path: &[&str]) -> CoapMessage {
        let mut m = CoapMessage::new(MessageType::Confirmable, CoapCode::POST, 1);
        m.token = b"tok".to_vec();
        for seg in path {
            m.options.push(CoapOption::uri_path(*seg));
        }
        m
    }

    #[test]
    fn extract_uri_path_joins_segments() {
        let msg = req_with_path(&["chat", "message"]);
        assert_eq!(extract_uri_path(&msg), "chat/message");
    }

    #[test]
    fn extract_uri_path_empty_when_no_options() {
        let msg = CoapMessage::new(MessageType::NonConfirmable, CoapCode::GET, 1);
        assert_eq!(extract_uri_path(&msg), "");
    }

    #[test]
    fn make_response_uses_ack_for_con() {
        let req = req_with_path(&["x"]);
        let resp = make_response(&req, CoapCode::CHANGED, Vec::new());
        assert!(matches!(resp.message_type, MessageType::Acknowledgement));
        assert_eq!(resp.message_id, req.message_id);
        assert_eq!(resp.token, req.token);
    }

    #[test]
    fn make_response_uses_non_for_non_request() {
        let mut req = req_with_path(&["x"]);
        req.message_type = MessageType::NonConfirmable;
        let resp = make_response(&req, CoapCode::CONTENT, Vec::new());
        assert!(matches!(resp.message_type, MessageType::NonConfirmable));
    }

    #[test]
    fn render_well_known_core_format() {
        let mut map = BTreeMap::new();
        map.insert("chat/message".to_string(), "Chat::Message".to_string());
        let s = render_well_known_core(&map);
        assert!(s.contains("</chat/message>"));
        assert!(s.contains("rt=\"dds.topic\""));
        assert!(s.contains("ct=65000"));
    }

    #[test]
    fn extract_observe_register() {
        let mut msg = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 1);
        msg.options.push(CoapOption::observe(0));
        assert_eq!(extract_observe(&msg), Some(0));
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
            "/../../man/man5/zerodds-coap-bridged.yaml.5"
        ))
        .unwrap_or_else(|e| panic!("man5 page: {e}"));
        let named = format!("Default\n.IR \"{DEFAULT_METRICS_LISTEN}\"\nwhen");
        assert!(
            page.contains(&named),
            "man5 does not name {DEFAULT_METRICS_LISTEN}"
        );
    }
}
