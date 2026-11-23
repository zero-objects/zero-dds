// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CoAP server for `zerodds-coap-bridged`.
//!
//! Spec: `zerodds-coap-bridge-1.0.md` §4 to §9.
//!
//! A UDP server; for each request to a topic's resource:
//! * `POST` / `PUT` → a DDS write of the sample, in the form its
//!   Content-Format names (§4.2, §4.5);
//! * `GET` → the current sample in the form Accept names; with `Observe: 0`
//!   an observation (§4.3, `super::observe`), `Observe: 1` ends it;
//! * `DELETE` → a dispose of the instance the request names (§4.2).
//!
//! `/.well-known/core` is the catalog (§5.2), `/schema/<slug>` a topic's
//! IDL. Block-wise transfers (RFC 7959) are handled inline. DTLS (§7.1,
//! feature `dtls`) runs as a second listener through the same `dispatch`;
//! OSCORE (§7.2, RFC 8613, feature `oscore`) is applied inside `dispatch`.
//! Every exchange is logged (§8.1), counted (§8.2) and, with an OTLP
//! endpoint, a span (§8.3).

use std::collections::BTreeMap;
use std::net::{SocketAddr, UdpSocket};
use std::string::{String, ToString};
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use std::vec::Vec;

use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_rtps::wire_types::EntityId;

use crate::blockwise::{BlockReassembler, BlockValue};
use crate::codec::{decode, encode};
use crate::content_format;
use crate::message::{CoapCode, CoapMessage, MessageType};
use crate::option::{CoapOption, OptionValue, numbers};
use crate::reliability::{DedupKind, DuplicateDetector, Reception};

use super::config::{DaemonConfig, TopicConfig};
use super::log::{self, Level};
use super::observe::{Endpoint, ObserveDirective, Observer, Registry, Stored, Timing};
use super::oscore_gateway::OscoreGateway;
#[cfg(feature = "oscore")]
use super::oscore_gateway::{OscoreExchange, OscoreReject};
use super::proxy::{ClientLeg, Handled, Proxy, ReplyTo};
use super::qos_translation::{CoapBehavior, topic_behavior};
use super::representation::{ContentFormats, Extensibility, Form, TopicType, encoding_of};
use super::runtime_common::{
    BridgeMetrics, CatalogSnapshot, ExchangeSpan, SERVICE_NAME, Tracer, install_signal_watcher,
    otlp_config_from_env, serve_admin_endpoints, spawn_otlp_flush_loop, unix_ns,
};
use super::security::{
    AclOp, AuthSubject, COAP_OPTION_AUTH_TOKEN, authenticate_coap, authorize,
    ctx_from_daemon_config,
};
use zerodds_monitor::Registry as MetricRegistry;
use zerodds_observability_otlp::OtlpExporter;

/// Admin endpoint address when `metrics.enabled` is set without
/// `metrics.listen`; one port per bridge daemon, see man5.
pub const DEFAULT_METRICS_LISTEN: &str = "127.0.0.1:9093";

/// The resource of the catalog (RFC 6690).
const WELL_KNOWN_CORE: &str = ".well-known/core";

/// The prefix of a topic's IDL (spec §5.2): `/schema/<slug>`.
const SCHEMA_PREFIX: &str = "schema/";

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

/// DTLS (`coaps://`) listener: its accepting thread, which joins the
/// session threads, the flag that ends them, and the bound address.
#[cfg(feature = "dtls")]
struct DtlsHandle {
    thread: JoinHandle<()>,
    stop: Arc<AtomicBool>,
    bound: SocketAddr,
}

/// How a message reaches a DTLS session from another thread: queued, and
/// the session's waiting receive interrupted so that it sends the queue.
#[cfg(feature = "dtls")]
#[derive(Clone)]
pub(super) struct DtlsTx {
    tx: std::sync::mpsc::Sender<CoapMessage>,
    waker: crate::dtls_transport::Waker,
}

#[cfg(feature = "dtls")]
impl DtlsTx {
    /// Queues `msg`; `false` once the session is gone.
    pub(super) fn send(&self, msg: CoapMessage) -> bool {
        self.tx.send(msg).is_ok() && self.waker.wake()
    }
}

#[cfg(feature = "dtls")]
impl core::fmt::Debug for DtlsTx {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("DtlsTx")
    }
}

/// Daemon handle. Drop initiates shutdown.
pub struct DaemonHandle {
    stop: Arc<AtomicBool>,
    accept_thread: Option<JoinHandle<()>>,
    pump_threads: Vec<JoinHandle<()>>,
    /// Bound address (may differ from the config bind when port=0).
    pub local_addr: String,
    admin_thread: Option<JoinHandle<()>>,
    otlp_thread: Option<JoinHandle<()>>,
    /// Bound admin address.
    pub admin_addr: Option<String>,
    /// SIGHUP reload flag.
    pub reload_flag: Arc<AtomicBool>,
    /// Healthz.
    pub healthy: Arc<AtomicBool>,
    /// Metric set for tests.
    pub metrics: Option<BridgeMetrics>,
    /// DTLS (`coaps://`) listener — present only when `dtls.enabled` + feature.
    #[cfg(feature = "dtls")]
    dtls: Option<DtlsHandle>,
    /// What the daemon serves, a core per participant (§1 L6); the
    /// shutdown ends their observations (§9.2).
    cores: Vec<Arc<Core>>,
    /// The daemon's DDS participants. [`Self::shutdown`] stops them:
    /// dropping the handle alone would not, because the runtime's worker
    /// threads and the pumps hold `Arc`s of their own.
    runtimes: Vec<Arc<DcpsRuntime>>,
}

impl DaemonHandle {
    /// The bound `coaps://` (DTLS) address, once the listener is up.
    #[cfg(feature = "dtls")]
    #[must_use]
    pub fn dtls_local_addr(&self) -> Option<SocketAddr> {
        self.dtls.as_ref().map(|d| d.bound)
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
        for core in self.cores.drain(..) {
            core.end_all();
        }
        // The DTLS sessions send what `end_all` queued before they end.
        #[cfg(feature = "dtls")]
        if let Some(d) = self.dtls.take() {
            d.stop.store(true, Ordering::SeqCst);
            let _ = d.thread.join();
        }
        self.healthy.store(false, Ordering::SeqCst);
        if let Some(admin) = self.admin_addr.as_deref() {
            if let Ok(addr) = admin.parse::<SocketAddr>() {
                let _ = std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(200));
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
        if let Some(j) = self.admin_thread.take() {
            let _ = j.join();
        }
        if let Some(j) = self.otlp_thread.take() {
            let _ = j.join();
        }
        // Last, so no pump writes to a stopped runtime. A daemon stopped
        // in a process that keeps running (an embedding, a test)
        // otherwise leaves its participants on the domain.
        for runtime in self.runtimes.drain(..) {
            runtime.shutdown();
        }
    }
}

impl Drop for DaemonHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// A participant of the daemon (spec §1 L6): the one of `domain` or a
/// tenant's.
struct Participant {
    /// The tenant's name; `None` for the participant of `domain`.
    tenant: Option<String>,
    domain: i32,
    topics: Vec<TopicConfig>,
    /// The DTLS subjects bound to the tenant.
    subjects: Vec<String>,
}

/// A bridged topic (spec §5): what the daemon serves at its path.
pub(super) struct Topic {
    /// The DDS topic.
    pub(super) dds_name: String,
    /// The `topic` label of its metrics (§8.2): the DDS topic, behind the
    /// tenant's name for a tenant's topic.
    pub(super) label: String,
    /// The DDS type's name.
    pub(super) dds_type: String,
    /// The type from `idl_file`; `None`: the samples pass through.
    pub(super) ty: Option<Arc<TopicType>>,
    /// The writer, for `direction: in` and `bidir`.
    pub(super) writer: Option<EntityId>,
    /// Whether there is a reader (`direction: out` and `bidir`).
    pub(super) reader: bool,
    /// Spec §6.
    pub(super) behavior: CoapBehavior,
    /// The form the writer sends (spec §3 `data_representation`): XCDR2 or
    /// XCDR1, little endian.
    pub(super) wire: Form,
}

/// What a daemon serves (spec §4): its topics, their DDS endpoints, their
/// observers, and what every request is checked and counted with.
pub(super) struct Core {
    /// The topics by their CoAP path.
    pub(super) topics: BTreeMap<String, Topic>,
    pub(super) runtime: Arc<DcpsRuntime>,
    /// Block1 (RFC 7959 §2.5): the transfers being assembled.
    block1: Mutex<BTreeMap<BlockKey, BlockReassembler>>,
    pub(super) metrics: BridgeMetrics,
    security: Arc<zerodds_bridge_security::LiveSecurity>,
    pub(super) oscore: Option<Arc<OscoreGateway>>,
    /// §4.5: the Content-Format number of each form.
    pub(super) formats: ContentFormats,
    /// `coap.block_size` as SZX (RFC 7959 §2.2).
    block_szx: u8,
    /// `coap.max_body_size`.
    max_body_size: usize,
    /// §4.3: the observers.
    pub(super) observe: Registry,
    /// §8.3.
    tracer: Tracer,
    /// The tenant this core serves (§1 L6); `None` for the one of `domain`.
    pub(super) tenant: Option<String>,
}

/// Why a sample has no representation in a form (spec §4.5).
#[derive(Debug)]
pub(super) enum Unrepresentable {
    /// A topic without `idl_file` has only the form of the sample.
    NotAcceptable,
    /// The sample's representation identifier has no Content-Format
    /// (XCDR1 big endian, XML), and without `idl_file` it cannot be
    /// converted.
    NoForm([u8; 2]),
    /// The sample is no sample of the topic's type.
    Broken(String),
}

impl core::fmt::Display for Unrepresentable {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotAcceptable => f.write_str("the topic has no idl_file: only the sample's form"),
            Self::NoForm(id) => write!(
                f,
                "the representation {:02x}{:02x} has no Content-Format and the topic has no idl_file",
                id[0], id[1]
            ),
            Self::Broken(e) => write!(f, "the sample does not decode: {e}"),
        }
    }
}

impl Core {
    /// The payload and Content-Format of `stored` in `form` (spec §4.5).
    /// `None` is the default form: XCDR2 little endian for a topic with a
    /// type, the sample's own form for one without.
    pub(super) fn represent(
        &self,
        topic: &Topic,
        stored: &Stored,
        form: Option<Form>,
    ) -> Result<(Vec<u8>, u16), Unrepresentable> {
        let native = Form::of_identifier(stored.identifier);
        let Some(ty) = &topic.ty else {
            let native = native.ok_or(Unrepresentable::NoForm(stored.identifier))?;
            if form.is_some_and(|f| f != native) {
                return Err(Unrepresentable::NotAcceptable);
            }
            return Ok((stored.body.to_vec(), self.formats.number(native)));
        };
        let form = form.unwrap_or(Form::Cdr2Le);
        // A body already in that form of the type's extensibility passes.
        if native == Some(form)
            && ty.extensibility().encoding(form) == encoding_of(stored.identifier)
        {
            return Ok((stored.body.to_vec(), self.formats.number(form)));
        }
        let value = ty
            .read_body(&stored.body, stored.identifier)
            .map_err(Unrepresentable::Broken)?;
        let payload = ty.write(form, &value).map_err(Unrepresentable::Broken)?;
        Ok((payload, self.formats.number(form)))
    }
}

/// Block1 reassembly key: the client's endpoint and the resource (RFC 7959
/// §2.5; a client may change the token from block to block, §2.3).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct BlockKey {
    peer: SocketAddr,
    path: String,
}

/// zerodds-lint: recursion-depth 64 (start bounded by AST depth)
/// Starts the daemon with the given config.
///
/// # Errors
/// `Config` (exit 1), `Bind` (exit 2), `Dds` (exit 3), `Dtls` (exit 4).
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
#[allow(clippy::too_many_lines)]
pub fn start_with_reload(
    cfg: DaemonConfig,
    runtime_config: RuntimeConfig,
    load: Option<ConfigLoader>,
) -> Result<DaemonHandle, ServerError> {
    log::set_level(&cfg.log_level);
    log::event(Level::Info, "starting")
        .field("bind", &cfg.bind)
        .number("domain", u64::try_from(cfg.domain).unwrap_or(0))
        .number("topics", cfg.topics.len() as u64)
        .emit();

    // 0. Metrics registry (§8.2).
    let registry = Arc::new(MetricRegistry::new());
    let metrics = BridgeMetrics::register(&registry);
    let healthy = Arc::new(AtomicBool::new(true));
    let reload_flag = Arc::new(AtomicBool::new(false));

    // 0b. Bridge security: auth + ACL (§7.2/§7.3). Spec §2: an auth setting
    // that cannot be served is exit 1; exit 4 stays for the security setup
    // of DTLS/OSCORE.
    let security_ctx = ctx_from_daemon_config(&cfg).map_err(|e| match e {
        zerodds_bridge_security::SecurityError::Tls(_) => {
            ServerError::Dtls(format!("security: {e}"))
        }
        _ => ServerError::Config(format!("security: {e}")),
    })?;
    let security_ctx = Arc::new(zerodds_bridge_security::LiveSecurity::new(security_ctx));
    // DTLS reloads its credentials on this flag, set after the reload of the
    // configuration has run.
    let dtls_reload = Arc::new(AtomicBool::new(false));
    log::event(Level::Info, "security")
        .field("auth_mode", &cfg.auth_mode)
        .number("acl_entries", cfg.topic_acl.len() as u64)
        .field("dtls", if cfg.dtls_enabled { "enabled" } else { "off" })
        .emit();

    // 0c. OSCORE (§7.2, RFC 8613, ADR 0010). Fail closed without the
    // `oscore` feature.
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
        log::event(Level::Info, "oscore")
            .token(&cfg.oscore.sender_id)
            .number("replay_window", cfg.oscore.replay_window)
            .emit();
        Some(Arc::new(OscoreGateway::new(cfg.oscore.clone())))
    } else {
        None
    };

    // 0d. The participants (spec §1 L6): the one of `domain` and `topics`,
    // and one per tenant; the types of their topics with an `idl_file`
    // (§4.5, §5.2). A file that does not load stops the start (exit 1).
    let mut participants: Vec<Participant> = vec![Participant {
        tenant: None,
        domain: cfg.domain,
        topics: cfg.topics.clone(),
        subjects: Vec::new(),
    }];
    participants.extend(cfg.tenants.iter().map(|t| Participant {
        tenant: Some(t.name.clone()),
        domain: t.domain,
        topics: t.topics.clone(),
        subjects: t.subjects.clone(),
    }));
    let mut types: Vec<BTreeMap<String, Arc<TopicType>>> = Vec::new();
    for p in &participants {
        let mut of_participant = BTreeMap::new();
        for topic in p.topics.iter().filter(|t| !t.idl_file.is_empty()) {
            let ty = TopicType::load(&topic.idl_file, &topic.dds_type).map_err(|e| {
                ServerError::Config(format!("topic {}: idl_file: {e}", topic.dds_name))
            })?;
            of_participant.insert(topic.dds_name.clone(), Arc::new(ty));
        }
        types.push(of_participant);
    }

    // 1. Bind the UDP socket.
    let socket = UdpSocket::bind(&cfg.bind).map_err(|e| ServerError::Bind(format!("{e}")))?;
    let local_addr = socket
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| cfg.bind.clone());
    socket
        .set_read_timeout(Some(Duration::from_millis(500)))
        .map_err(|e| ServerError::Io(format!("set timeout: {e}")))?;
    log::event(Level::Info, "bound")
        .field("peer", &local_addr)
        .emit();

    // DTLS without the `dtls` feature fails closed.
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

    // 2. A DCPS runtime per participant, its topics' readers and writers,
    // its observers (§9.1 steps 3 and 4).
    let stop = Arc::new(AtomicBool::new(false));
    let next_mid = Arc::new(AtomicU16::new(first_message_id()));
    let tracer =
        Tracer::new(otlp_config_from_env(SERVICE_NAME).map(|c| Arc::new(OtlpExporter::new(c))));
    let mut cores: Vec<Arc<Core>> = Vec::new();
    let mut tenants: Vec<(Vec<String>, Arc<Core>)> = Vec::new();
    let mut runtimes: Vec<Arc<DcpsRuntime>> = Vec::new();
    let mut pump_threads = Vec::new();
    for (p, types) in participants.iter().zip(types) {
        let seed = match &p.tenant {
            None => local_addr.clone(),
            Some(name) => format!("{local_addr}/{name}"),
        };
        let prefix = zerodds_dcps::participant::bridge_guid_prefix(&seed);
        let runtime = DcpsRuntime::start(p.domain, prefix, runtime_config.clone())
            .map_err(|e| ServerError::Dds(format!("{e:?}")))?;
        runtimes.push(Arc::clone(&runtime));
        let mut topics: BTreeMap<String, Topic> = BTreeMap::new();
        let mut readers: Vec<(String, String, std::sync::mpsc::Receiver<UserSample>)> = Vec::new();
        for topic in &p.topics {
            let ty = types.get(&topic.dds_name).cloned();
            let (mut entry, reader) = register_topic(&runtime, topic, ty)?;
            if let Some(name) = &p.tenant {
                entry.label = format!("{name}/{}", topic.dds_name);
            }
            if let Some(rx) = reader {
                readers.push((entry.label.clone(), topic.coap_uri_path.clone(), rx));
            }
            topics.insert(topic.coap_uri_path.clone(), entry);
        }
        let core = Arc::new(Core {
            topics,
            runtime: Arc::clone(&runtime),
            block1: Mutex::new(BTreeMap::new()),
            metrics: metrics.clone(),
            security: Arc::clone(&security_ctx),
            oscore: oscore_gw.clone(),
            formats: cfg.content_formats,
            block_szx: szx_of(cfg.block_size),
            max_body_size: cfg.max_body_size,
            observe: Registry::new(
                socket
                    .try_clone()
                    .map_err(|e| ServerError::Io(format!("{e}")))?,
                Arc::clone(&next_mid),
                Timing::of(&cfg),
            ),
            tracer: tracer.clone(),
            tenant: p.tenant.clone(),
        });
        if let Some(name) = &p.tenant {
            log::event(Level::Info, "tenant")
                .field("tenant", name)
                .number("domain", u64::try_from(p.domain).unwrap_or(0))
                .number("topics", p.topics.len() as u64)
                .emit();
            tenants.push((p.subjects.clone(), Arc::clone(&core)));
        }
        // A pump per reader topic: a DDS sample is kept and notified.
        for (label, path, rx) in readers {
            let (stop_c, core_c) = (Arc::clone(&stop), Arc::clone(&core));
            pump_threads.push(thread::spawn(move || {
                while !stop_c.load(Ordering::SeqCst) {
                    match rx.recv_timeout(Duration::from_millis(200)) {
                        Ok(UserSample::Alive {
                            payload,
                            representation_identifier,
                            ..
                        }) => {
                            core_c.metrics.dds_received(&label);
                            core_c.on_sample(
                                &path,
                                Stored {
                                    body: Arc::from(&payload[..]),
                                    identifier: representation_identifier,
                                    at: Instant::now(),
                                },
                            );
                        }
                        // A dispose or unregister changes no representation;
                        // the end-of-coherent-set marker (DDSI-RTPS 2.5
                        // §8.7.6) carries no user data.
                        Ok(UserSample::Lifecycle { .. } | UserSample::CoherentSetEnd { .. }) => {}
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            }));
        }
        // The observe timers: retransmissions, pings, deadlines (§4.3, §6).
        {
            let (stop_c, core_c) = (Arc::clone(&stop), Arc::clone(&core));
            pump_threads.push(thread::spawn(move || {
                while !stop_c.load(Ordering::SeqCst) {
                    core_c.tick(Instant::now());
                    thread::sleep(Duration::from_millis(50));
                }
            }));
        }
        cores.push(core);
    }
    // The participant of `domain`: plaintext, multicast, and every DTLS
    // session no tenant names.
    let core = Arc::clone(&cores[0]);

    // 4b. The proxy (RFC 7252 §5.7).
    let bound: SocketAddr = socket
        .local_addr()
        .map_err(|e| ServerError::Io(format!("{e}")))?;
    let dtls_port = if cfg.dtls_enabled {
        cfg.bind_dtls
            .as_deref()
            .and_then(|b| b.parse::<SocketAddr>().ok())
            .map_or(Some(5684), |a| Some(a.port()).filter(|p| *p != 0))
    } else {
        None
    };
    let mut local_resources: Vec<String> = core.topics.keys().cloned().collect();
    local_resources.extend(core.topics.keys().map(|p| format!("{SCHEMA_PREFIX}{p}")));
    let proxy = Arc::new(
        Proxy::new(
            cfg.proxy.clone(),
            &local_resources,
            Arc::clone(&next_mid),
            Arc::clone(&security_ctx),
            bound,
            dtls_port,
        )
        .map_err(ServerError::Config)?,
    );

    // 5. Accept thread. With multicast (RFC 7252 §8) the daemon joins its
    // groups on the socket; an IPv6 group with an IPv4 bind address takes a
    // second socket.
    let dedup = Arc::new(Dedup::new());
    #[cfg(feature = "dtls")]
    let dtls_shared = DtlsShared {
        core: Arc::clone(&core),
        tenants: Arc::new(tenants),
        proxy: Arc::clone(&proxy),
        dedup: Arc::clone(&dedup),
    };
    #[cfg(not(feature = "dtls"))]
    drop(tenants);
    let (extra_socket, leisure) = if cfg.multicast.enabled {
        let extra = super::multicast_io::join(&cfg.multicast, &socket, bound)
            .map_err(|e| ServerError::Bind(format!("coap.multicast: {e}")))?;
        let (leisure, sender) = super::multicast_io::Leisure::start(
            Duration::from_millis(cfg.multicast.leisure_ms),
            Arc::clone(&stop),
        );
        pump_threads.push(sender);
        log::event(Level::Info, "multicast")
            .field(
                "groups",
                &format!(
                    "{:?}",
                    super::multicast_io::groups_of(&cfg.multicast, bound)
                ),
            )
            .emit();
        (extra, Some(leisure))
    } else {
        (None, None)
    };
    let ctx = AcceptCtx {
        core: Arc::clone(&core),
        cfg: Arc::new(cfg.clone()),
        leisure,
        proxy,
        bound,
        dedup: Arc::clone(&dedup),
        stop: Arc::clone(&stop),
    };
    let accept_socket = Arc::new(
        socket
            .try_clone()
            .map_err(|e| ServerError::Io(format!("{e}")))?,
    );
    let accept_ctx = ctx.clone();
    let accept_thread = thread::spawn(move || accept_loop(&accept_socket, &accept_ctx));
    if let Some(extra) = extra_socket {
        let extra = Arc::new(extra);
        let extra_ctx = ctx.clone();
        pump_threads.push(thread::spawn(move || accept_loop(&extra, &extra_ctx)));
    }
    drop(ctx);

    // 5b. DTLS (`coaps://`) listener; a bind or credential error stops the
    // start (exit 4).
    #[cfg(feature = "dtls")]
    let dtls = if cfg.dtls_enabled {
        let dtls_bind = cfg
            .bind_dtls
            .clone()
            .unwrap_or_else(|| default_coaps_bind(&local_addr));
        let dtls_stop = Arc::new(AtomicBool::new(false));
        let (thread, bound) = spawn_dtls_server(
            &dtls_bind,
            cfg.dtls.clone(),
            Arc::clone(&dtls_reload),
            Arc::clone(&dtls_stop),
            dtls_shared,
        )?;
        Some(DtlsHandle {
            thread,
            stop: dtls_stop,
            bound,
        })
    } else {
        drop(dtls_shared);
        None
    };

    // 6. Admin endpoint (catalog, healthz, §8.2 metrics).
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
                    cfg.metrics_path.clone(),
                ) {
                    Ok((h, bound)) => {
                        log::event(Level::Info, "admin_endpoint")
                            .field("peer", &bound.to_string())
                            .field("path", &cfg.metrics_path)
                            .emit();
                        admin_addr = Some(bound.to_string());
                        admin_thread = Some(h);
                    }
                    Err(e) => log::event(Level::Error, "admin_bind_failed")
                        .field("reason", &e.to_string())
                        .emit(),
                }
            }
            Err(e) => log::event(Level::Error, "admin_bind_failed")
                .field("reason", &e.to_string())
                .emit(),
        }
    }

    // 7. Signal watcher.
    if let Err(e) = install_signal_watcher(Arc::clone(&stop), Arc::clone(&reload_flag)) {
        log::event(Level::Error, "signal_watcher_failed")
            .field("reason", &e.to_string())
            .emit();
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

    // 8. OTLP (§8.3): the spans go out every 5 s and at the shutdown.
    let otlp_thread = tracer.exporter().and_then(|exp| {
        spawn_otlp_flush_loop(exp, Arc::clone(&stop), Duration::from_secs(5))
            .map_err(|e| {
                log::event(Level::Error, "otlp_failed")
                    .field("reason", &e.to_string())
                    .emit();
            })
            .ok()
    });

    drop(core);
    Ok(DaemonHandle {
        cores,
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
        runtimes,
    })
}

/// RFC 7252 §4.4: a Message ID that does not repeat the IDs of a previous
/// run of the daemon on the same endpoint soon after it; a random start.
fn first_message_id() -> u16 {
    let mut b = [0u8; 2];
    if getrandom::getrandom(&mut b).is_ok() {
        u16::from_be_bytes(b)
    } else {
        1
    }
}

/// The SZX of a block size of 16 to 1024 (RFC 7959 §2.2).
fn szx_of(block_size: u16) -> u8 {
    let mut szx = 0u8;
    while szx < 6 && (16u16 << (szx + 1)) <= block_size {
        szx += 1;
    }
    szx
}

/// What the receiving loop of a socket needs.
#[derive(Clone)]
struct AcceptCtx {
    core: Arc<Core>,
    cfg: Arc<DaemonConfig>,
    /// Set with `coap.multicast.enabled`.
    leisure: Option<Arc<super::multicast_io::Leisure>>,
    /// RFC 7252 §5.7.
    proxy: Arc<Proxy>,
    /// The address the socket is bound to.
    bound: SocketAddr,
    /// RFC 7252 §4.5: the requests seen, shared with the DTLS sessions.
    dedup: Arc<Dedup>,
    stop: Arc<AtomicBool>,
}

/// The Message IDs of the requests the daemon served (RFC 7252 §4.5: "The
/// recipient SHOULD acknowledge each duplicate copy of a Confirmable
/// message using the same Acknowledgement [...], but SHOULD process any
/// request [...] only once"; a duplicate Non-confirmable message is
/// ignored).
struct Dedup {
    seen: Mutex<DuplicateDetector>,
    started: Instant,
}

/// What became of a request at the deduplication.
enum Seen {
    /// First sighting: serve it.
    Fresh,
    /// A duplicate: answer with this (the acknowledgement of the first),
    /// or not at all.
    Again(Option<CoapMessage>),
}

impl Dedup {
    fn new() -> Self {
        Self {
            seen: Mutex::new(DuplicateDetector::with_cap(4096)),
            started: Instant::now(),
        }
    }

    fn now_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn receive(&self, endpoint: &str, msg: &CoapMessage) -> Seen {
        let kind = if matches!(msg.message_type, MessageType::Confirmable) {
            DedupKind::Confirmable
        } else {
            DedupKind::NonConfirmable
        };
        let now = self.now_ms();
        match self.seen.lock() {
            Ok(mut seen) => match seen.receive(endpoint.as_bytes(), msg.message_id, kind, now) {
                Reception::Fresh => Seen::Fresh,
                Reception::Duplicate { cached_ack } => {
                    Seen::Again(cached_ack.and_then(|b| decode(&b).ok()))
                }
            },
            Err(_) => Seen::Fresh,
        }
    }

    /// Remembers the piggybacked response of a Confirmable request.
    fn remember(&self, endpoint: &str, responses: &[CoapMessage]) {
        let Some(ack) = responses
            .iter()
            .find(|r| matches!(r.message_type, MessageType::Acknowledgement))
        else {
            return;
        };
        if let (Ok(bytes), Ok(mut seen)) = (encode(ack), self.seen.lock()) {
            seen.remember_ack(endpoint.as_bytes(), ack.message_id, bytes);
        }
    }
}

/// Receives the datagrams of `socket` and answers them until `stop`; a
/// request whose destination is a multicast group goes to
/// [`handle_multicast`].
fn accept_loop(socket: &Arc<UdpSocket>, ctx: &AcceptCtx) {
    let mut buf = [0u8; 65535];
    while !ctx.stop.load(Ordering::SeqCst) {
        match super::multicast_io::recv_with_destination(socket, &mut buf) {
            Ok((n, peer, destination)) => {
                // The shutdown wakes the loop with a datagram of its own; it
                // is no request and no error.
                if ctx.stop.load(Ordering::SeqCst) {
                    break;
                }
                let started = (Instant::now(), unix_ns());
                let bytes = &buf[..n];
                let core = &ctx.core;
                core.metrics.frames_in_total.inc();
                core.metrics.bytes_in_total.add(n as u64);
                if let Some(group) = destination.filter(std::net::IpAddr::is_multicast) {
                    if let Err(e) = handle_multicast(bytes, peer, group, socket, ctx) {
                        core.metrics.errors_total.inc();
                        log::event(Level::Warn, "multicast_request_failed")
                            .field("peer", &peer.to_string())
                            .field("reason", &e)
                            .emit();
                    }
                    continue;
                }
                if n > ctx.cfg.max_message_size {
                    // Spec §4.2 — `4.13 Request Entity Too Large` with the
                    // largest size in Size1 (RFC 7252 §5.10.9).
                    if let Ok(req) = decode(bytes) {
                        let mut resp = make_response(&req, CoapCode::new(4, 13), Vec::new());
                        resp.options.push(CoapOption::new(
                            numbers::SIZE1,
                            OptionValue::Uint(ctx.cfg.max_message_size as u64),
                        ));
                        send_msg(socket, &resp, peer);
                        core.metrics.frames_out_total.inc();
                        core.metrics.request(&method_name(req.code), "4.13");
                    }
                    core.metrics.errors_total.inc();
                    continue;
                }
                // RFC 7252 §4.2, §4.3: the message layer first.
                let msg = match decode(bytes) {
                    Ok(msg) => msg,
                    Err(e) => {
                        reject_undecodable(bytes, &|m| send_msg(socket, m, peer));
                        core.metrics.errors_total.inc();
                        log::event(Level::Info, "undecodable")
                            .field("peer", &peer.to_string())
                            .field("reason", &e.to_string())
                            .number("bytes", n as u64)
                            .emit();
                        continue;
                    }
                };
                if !msg.code.is_request() {
                    message_layer(&msg, Endpoint::Udp(peer), peer, core, &ctx.proxy, &|m| {
                        send_msg(socket, m, peer);
                    });
                    continue;
                }
                let leg = ClientLeg {
                    reply: ReplyTo::Udp {
                        socket: Arc::clone(socket),
                        peer,
                    },
                    destination: destination
                        .map_or(ctx.bound, |ip| SocketAddr::new(ip, ctx.bound.port())),
                    dtls_peer: None,
                    request_len: n,
                };
                let rewritten = match ctx.proxy.handle(&msg, leg) {
                    Handled::Proxied => continue,
                    Handled::Local(rewritten) => rewritten.and_then(|r| encode(&r).ok()),
                };
                let endpoint = format!("udp:{peer}");
                if let Seen::Again(ack) = ctx.dedup.receive(&endpoint, &msg) {
                    if let Some(ack) =
                        ack.filter(|_| matches!(msg.message_type, MessageType::Confirmable))
                    {
                        send_msg(socket, &ack, peer);
                    }
                    continue;
                }
                let bytes = rewritten.as_deref().unwrap_or(bytes);
                match dispatch(core, bytes, peer, None) {
                    Ok(d) => {
                        for resp in &d.responses {
                            send_msg(socket, resp, peer);
                            core.metrics.frames_out_total.inc();
                        }
                        ctx.dedup.remember(&endpoint, &d.responses);
                        finish(core, &peer.to_string(), n, started, &d);
                        core.apply_observe(d.observe, &|token| Observer::Udp { addr: peer, token });
                    }
                    Err(e) => {
                        core.metrics.errors_total.inc();
                        log::event(Level::Warn, "request_failed")
                            .field("peer", &peer.to_string())
                            .field("reason", &e)
                            .emit();
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(e) => {
                if !ctx.stop.load(Ordering::SeqCst) {
                    log::event(Level::Warn, "recv_failed")
                        .field("reason", &e.to_string())
                        .emit();
                }
            }
        }
    }
}

/// A request that arrived through the multicast group `group` (RFC 7252
/// §8). It is answered only when it is Non-confirmable with a token
/// (§8.1), addresses one of `coap.multicast.resources` and, without
/// OSCORE, one of `coap.multicast.unauthenticated_resources` (§11.3). No
/// Reset, no error and no empty response leaves (§8.1, §8.2); a response
/// goes out as a Non-confirmable message of its own at a random point of a
/// leisure period (§8.2). An observation is not registered for it.
fn handle_multicast(
    bytes: &[u8],
    peer: SocketAddr,
    group: std::net::IpAddr,
    socket: &Arc<UdpSocket>,
    ctx: &AcceptCtx,
) -> Result<(), String> {
    let Some(leisure) = ctx.leisure.as_ref() else {
        // Not a member of any group: the request is not for this daemon.
        return Ok(());
    };
    let outer = decode(bytes).map_err(|e| format!("decode: {e}"))?;
    if !outer.code.is_request()
        || crate::multicast::validate_multicast_request(
            matches!(outer.message_type, MessageType::Confirmable),
            &outer.token,
        )
        .is_err()
    {
        return Ok(());
    }
    let Some(name) = resource_name(&outer) else {
        return Ok(());
    };
    if !ctx.cfg.multicast.resources.contains(&name) {
        return Ok(());
    }
    let protected = outer.options.iter().any(|o| o.number == numbers::OSCORE);
    if !protected && !ctx.cfg.multicast.unauthenticated_resources.contains(&name) {
        return Ok(());
    }
    // §4.5: a duplicate Non-confirmable request is ignored.
    if let Seen::Again(_) = ctx.dedup.receive(&format!("udp:{peer}"), &outer) {
        return Ok(());
    }
    let d = dispatch(&ctx.core, bytes, peer, None)?;
    for mut resp in d.responses {
        if resp.code.class != 2 || (resp.code == CoapCode::CONTENT && resp.payload.is_empty()) {
            continue;
        }
        resp.message_type = MessageType::NonConfirmable;
        resp.message_id = ctx.core.observe_mid();
        resp.options.retain(|o| o.number != numbers::OBSERVE);
        let token = resp.token.clone();
        let encoded = encode(&resp).map_err(|e| format!("encode: {e}"))?;
        if leisure.schedule(group, &token, encoded, peer, Arc::clone(socket)) {
            ctx.core.metrics.frames_out_total.inc();
        } else {
            ctx.core.metrics.errors_total.inc();
        }
    }
    Ok(())
}

/// RFC 7252 §4.2: a Confirmable message that cannot be decoded is
/// rejected with a Reset, when its header can be read; any other is
/// ignored.
fn reject_undecodable(bytes: &[u8], send: &dyn Fn(&CoapMessage)) {
    if bytes.len() >= 4 && bytes[0] >> 6 == 1 && (bytes[0] >> 4) & 0b11 == 0 {
        send(&CoapMessage::new(
            MessageType::Reset,
            CoapCode::EMPTY,
            u16::from_be_bytes([bytes[2], bytes[3]]),
        ));
    }
}

/// A message that is no request (RFC 7252 §4.2, §4.3): an Acknowledgement
/// or Reset is the answer to a Confirmable notification or ping of the
/// daemon (RFC 7641 §3.6: a Reset ends the observation), or to a separate
/// response of the proxy; an empty Confirmable message (a "CoAP ping",
/// §4.3) and any other Confirmable message the daemon has no use for are
/// answered with Reset; a Non-confirmable one is ignored.
fn message_layer(
    msg: &CoapMessage,
    endpoint: Endpoint,
    peer: SocketAddr,
    core: &Core,
    proxy: &Proxy,
    send: &dyn Fn(&CoapMessage),
) {
    match msg.message_type {
        MessageType::Acknowledgement | MessageType::Reset => {
            let reset = matches!(msg.message_type, MessageType::Reset);
            if !core.confirm(endpoint, msg.message_id, reset) {
                proxy.confirm(peer, msg.message_id, reset);
            }
        }
        MessageType::Confirmable => send(&CoapMessage::new(
            MessageType::Reset,
            CoapCode::EMPTY,
            msg.message_id,
        )),
        MessageType::NonConfirmable => {}
    }
}

/// What one request did, for the log, the metrics and the span (§8).
#[derive(Debug, Default)]
struct Exchange {
    /// The resource it addressed.
    uri: String,
    /// The DDS topic of that resource.
    dds_topic: Option<String>,
    /// The DDS write it made: start and end in ns since the epoch.
    written: Option<(u64, u64)>,
}

/// Transport-agnostic outcome of handling one CoAP request: the responses to
/// send back (in order) + an observe-registry directive. DDS writes and the
/// per-request metric side effects already happened inside [`dispatch`].
struct Dispatched {
    responses: Vec<CoapMessage>,
    observe: ObserveDirective,
    /// The request as the daemon read it (unprotected).
    request: CoapMessage,
    exchange: Exchange,
}

/// Handle one CoAP request against the DDS runtime. Shared by the plaintext
/// UDP path, multicast and the DTLS path so all behave identically. With
/// OSCORE enabled (Spec §7.2, RFC 8613) the request is unprotected here and
/// every response protected before it leaves; unprotected requests (other
/// than `/.well-known/core`) are refused with `4.01 Unauthorized`. A
/// response the request's No-Response option suppresses (RFC 7967) does not
/// leave; a Confirmable request then gets an empty acknowledgement (RFC
/// 7252 §4.2).
fn dispatch(
    core: &Core,
    bytes: &[u8],
    peer: SocketAddr,
    dtls_peer: Option<&AuthSubject>,
) -> Result<Dispatched, String> {
    let outer = decode(bytes).map_err(|e| format!("decode: {e}"))?;
    let refuse = |code: CoapCode, text: &[u8], outer: CoapMessage| Dispatched {
        responses: vec![make_response(&outer, code, text.to_vec())],
        observe: ObserveDirective::None,
        exchange: Exchange {
            uri: resource_name(&outer).unwrap_or_default(),
            ..Exchange::default()
        },
        request: outer,
    };

    // §7.2 OSCORE gate — unprotect (RFC 8613 §8.2) or refuse.
    #[cfg(feature = "oscore")]
    let (req, exchange): (CoapMessage, Option<OscoreExchange>) = match core.oscore.as_ref() {
        Some(gw) => match gw.unprotect_request(&outer)? {
            Ok((inner, ex)) => (inner, Some(ex)),
            Err(OscoreReject::NotProtected) => {
                if resource_name(&outer).as_deref() == Some(WELL_KNOWN_CORE) {
                    // The discovery catalog stays reachable in the clear.
                    (outer, None)
                } else {
                    core.metrics.errors_total.inc();
                    core.metrics.acl_denied("unauthenticated");
                    return Ok(refuse(CoapCode::new(4, 1), b"oscore required", outer));
                }
            }
            Err(OscoreReject::Replay) => {
                // §7.4 — replayed Partial IV.
                core.metrics.errors_total.inc();
                core.metrics.oscore_replays_total.inc();
                return Ok(refuse(CoapCode::new(4, 1), b"oscore replay", outer));
            }
            Err(OscoreReject::Verify) => {
                // §8.2 — decompression/verification failure.
                core.metrics.errors_total.inc();
                return Ok(refuse(
                    CoapCode::new(4, 0),
                    b"oscore verification failed",
                    outer,
                ));
            }
        },
        None => (outer, None),
    };
    #[cfg(not(feature = "oscore"))]
    let req = outer;
    #[cfg(not(feature = "oscore"))]
    let _ = refuse;

    #[cfg(feature = "oscore")]
    let protected = exchange.is_some();
    #[cfg(not(feature = "oscore"))]
    let protected = false;
    let mut d = dispatch_core(
        core,
        &req,
        bytes.len(),
        protected || dtls_peer.is_some(),
        peer,
        dtls_peer,
    );

    // RFC 7967 §2.1: the responses of the classes the client said it is
    // not interested in do not leave; a Confirmable request is acknowledged
    // empty instead of its piggybacked response.
    let confirmable = matches!(req.message_type, MessageType::Confirmable);
    let mut kept = Vec::with_capacity(d.responses.len());
    for resp in d.responses.drain(..) {
        if matches!(resp.message_type, MessageType::Reset) || !suppressed(&req, resp.code) {
            kept.push(resp);
        } else if confirmable && matches!(resp.message_type, MessageType::Acknowledgement) {
            kept.push(CoapMessage::new(
                MessageType::Acknowledgement,
                CoapCode::EMPTY,
                req.message_id,
            ));
        }
    }
    d.responses = kept;

    // §8.3 — protect every response of an OSCORE exchange before it leaves.
    #[cfg(feature = "oscore")]
    if let (Some(gw), Some(ex)) = (core.oscore.as_ref(), exchange) {
        let mut out = Vec::with_capacity(d.responses.len());
        for resp in &d.responses {
            // A Reset or an empty acknowledgement is a message of the
            // transport, not a response.
            if matches!(resp.message_type, MessageType::Reset) || resp.code == CoapCode::EMPTY {
                out.push(resp.clone());
                continue;
            }
            out.push(
                gw.protect_response(&ex, resp)
                    .map_err(|e| format!("oscore: {e}"))?,
            );
        }
        d.responses = out;
        if let ObserveDirective::Register { oscore: slot, .. } = &mut d.observe {
            *slot = Some(ex);
        }
    }
    Ok(d)
}

/// RFC 7967 §2.1: whether the request's No-Response option suppresses a
/// response of `code`'s class (2 → 2.xx, 8 → 4.xx, 16 → 5.xx; an empty
/// value suppresses none).
pub(super) fn suppressed(req: &CoapMessage, code: CoapCode) -> bool {
    let Some(mask) = uint_option(req, numbers::NO_RESPONSE) else {
        return false;
    };
    let bit = match code.class {
        2 => 2,
        4 => 8,
        5 => 16,
        _ => return false,
    };
    mask & bit != 0
}

/// The transport- and security-agnostic request core: the (already
/// unprotected) request against the topics.
#[allow(clippy::too_many_lines)]
fn dispatch_core(
    core: &Core,
    req: &CoapMessage,
    request_len: usize,
    secured: bool,
    peer: SocketAddr,
    dtls_peer: Option<&AuthSubject>,
) -> Dispatched {
    let mut out = Dispatched {
        responses: Vec::new(),
        observe: ObserveDirective::None,
        request: req.clone(),
        exchange: Exchange::default(),
    };
    let respond = |out: &mut Dispatched, code: CoapCode, text: &str| {
        out.responses
            .push(make_response(req, code, text.as_bytes().to_vec()));
    };
    // RFC 7252 §5.4.1: an unrecognized critical option (§5.4.5: also a
    // second occurrence of a non-repeatable one) is 4.02 Bad Option in a
    // Confirmable request; a Non-confirmable one is rejected (§4.3).
    if let Some(n) = unrecognized_critical(req) {
        core.metrics.errors_total.inc();
        if matches!(req.message_type, MessageType::Confirmable) {
            respond(
                &mut out,
                CoapCode::new(4, 2),
                &format!("unrecognized critical option {n}"),
            );
        } else {
            out.responses.push(CoapMessage::new(
                MessageType::Reset,
                CoapCode::EMPTY,
                req.message_id,
            ));
        }
        return out;
    }
    // §5.8: "A request with an unrecognized or unsupported Method Code MUST
    // generate a 4.05 (Method Not Allowed) piggybacked response".
    let method_known = [
        CoapCode::GET,
        CoapCode::POST,
        CoapCode::PUT,
        CoapCode::DELETE,
    ]
    .contains(&req.code);
    if !method_known {
        respond(&mut out, CoapCode::new(4, 5), "");
        return out;
    }
    // RFC 7252 §6.5: the resource name the request addresses; a Uri-Path
    // of "." or ".." is no valid request (§5.10.1).
    let Some(path) = resource_name(req) else {
        respond(&mut out, CoapCode::BAD_REQUEST, "Uri-Path \".\" or \"..\"");
        return out;
    };
    out.exchange.uri = path.clone();

    // `/.well-known/core` → the catalog (open, no auth).
    if path == WELL_KNOWN_CORE {
        if req.code != CoapCode::GET {
            respond(&mut out, CoapCode::new(4, 5), "");
            return out;
        }
        // §5.10.4: "If the preferred Content-Format cannot be returned,
        // then a 4.06 "Not Acceptable" MUST be sent as a response".
        if uint_option(req, numbers::ACCEPT)
            .is_some_and(|f| f != u64::from(content_format::LINK_FORMAT))
        {
            respond(&mut out, CoapCode::new(4, 6), "");
            return out;
        }
        let body = render_well_known_core(core, &uri_queries(req));
        let limit = core.block_limit(secured || extract_auth_token(req).is_some(), request_len);
        let mut resp = representation(req, body.into_bytes(), Some(limit));
        resp.options
            .push(CoapOption::content_format(content_format::LINK_FORMAT));
        out.responses.push(resp);
        return out;
    }

    // `/schema/<slug>` → the topic's IDL (§5.2); a topic of that name with
    // its own path wins.
    let schema_of = path
        .strip_prefix(SCHEMA_PREFIX)
        .filter(|slug| !core.topics.contains_key(&path) && core.topics.contains_key(*slug))
        .map(ToString::to_string);
    let topic_path = schema_of.clone().unwrap_or_else(|| path.clone());

    // RFC 7252 §5.9.2.5: a resource the server does not have is 4.04.
    let Some(topic) = core.topics.get(&topic_path) else {
        core.metrics.errors_total.inc();
        respond(&mut out, CoapCode::new(4, 4), "");
        return out;
    };
    out.exchange.dds_topic = Some(topic.dds_name.clone());

    // §7.2 — authentication: DTLS, OSCORE or the auth-token option.
    let auth_token = extract_auth_token(req);
    let subject = match authenticate_coap(
        &core.security.current().auth,
        auth_token.as_deref(),
        dtls_peer,
    ) {
        Ok(s) => s,
        Err(_) => {
            core.metrics.errors_total.inc();
            core.metrics.acl_denied("unauthenticated");
            respond(&mut out, CoapCode::new(4, 1), "unauthorized");
            return out;
        }
    };
    let authenticated = secured || auth_token.is_some();

    if let Some(slug) = schema_of {
        serve_schema(core, req, topic, &slug, &subject, &mut out);
        return out;
    }
    match req.code {
        CoapCode::POST | CoapCode::PUT => {
            serve_write(core, req, &path, topic, &subject, peer, &mut out);
        }
        CoapCode::DELETE => serve_delete(core, req, topic, &subject, &mut out),
        _ => serve_get(
            core,
            req,
            &path,
            topic,
            &subject,
            core.block_limit(authenticated, request_len),
            &mut out,
        ),
    }
    out
}

impl Core {
    /// The largest block (as SZX) a response may carry: `coap.block_size`,
    /// and for a request that is not authenticated at most
    /// [`UNAUTHENTICATED_AMPLIFICATION`] times its size (RFC 7252 §11.3).
    fn block_limit(&self, authenticated: bool, request_len: usize) -> u8 {
        block_limit(authenticated, request_len).map_or(self.block_szx, |l| l.min(self.block_szx))
    }

    /// The ACL check; a denial is `4.03` and counted.
    fn allowed(&self, subject: &AuthSubject, op: AclOp, topic: &Topic) -> bool {
        let ok = authorize(&self.security.current().acl, subject, op, &topic.dds_name);
        if !ok {
            self.metrics.errors_total.inc();
            self.metrics.acl_denied(match op {
                AclOp::Read => "read",
                AclOp::Write => "write",
            });
        }
        ok
    }

    /// A Message ID of the daemon's own messages.
    fn observe_mid(&self) -> u16 {
        self.observe.next_mid()
    }
}

/// `GET /schema/<slug>`: the IDL of the topic's type as text (spec §5.2);
/// `4.04` for a topic without `idl_file`.
fn serve_schema(
    core: &Core,
    req: &CoapMessage,
    topic: &Topic,
    slug: &str,
    subject: &AuthSubject,
    out: &mut Dispatched,
) {
    if req.code != CoapCode::GET {
        out.responses
            .push(make_response(req, CoapCode::new(4, 5), Vec::new()));
        return;
    }
    if !core.allowed(subject, AclOp::Read, topic) {
        out.responses.push(make_response(
            req,
            CoapCode::new(4, 3),
            b"forbidden".to_vec(),
        ));
        return;
    }
    if uint_option(req, numbers::ACCEPT)
        .is_some_and(|f| f != u64::from(content_format::TEXT_PLAIN_UTF8))
    {
        out.responses
            .push(make_response(req, CoapCode::new(4, 6), Vec::new()));
        return;
    }
    let Some(ty) = &topic.ty else {
        out.responses.push(make_response(
            req,
            CoapCode::new(4, 4),
            format!("{slug} has no idl_file").into_bytes(),
        ));
        return;
    };
    let mut resp = representation(req, ty.idl().as_bytes().to_vec(), Some(core.block_szx));
    resp.options
        .push(CoapOption::content_format(content_format::TEXT_PLAIN_UTF8));
    out.responses.push(resp);
}

/// POST / PUT: the sample written to DDS (spec §4.2), its blocks first
/// assembled (RFC 7959 §2.5).
#[allow(clippy::too_many_arguments)]
fn serve_write(
    core: &Core,
    req: &CoapMessage,
    path: &str,
    topic: &Topic,
    subject: &AuthSubject,
    peer: SocketAddr,
    out: &mut Dispatched,
) {
    let reply = |out: &mut Dispatched, code: CoapCode, text: String| {
        out.responses
            .push(make_response(req, code, text.into_bytes()));
    };
    // §5.8: a topic without a writer (`direction: out`) takes no sample.
    let Some(eid) = topic.writer else {
        reply(
            out,
            CoapCode::new(4, 5),
            format!("{} is direction out", topic.dds_name),
        );
        return;
    };
    if !core.allowed(subject, AclOp::Write, topic) {
        reply(out, CoapCode::new(4, 3), "forbidden".to_string());
        return;
    }
    // §4.5, RFC 7252 §5.10.3: the form of the body; without Content-Format
    // XCDR2 little endian.
    let form = match uint_option(req, numbers::CONTENT_FORMAT) {
        None => Form::Cdr2Le,
        Some(n) => match core.formats.form(n) {
            Some(f) if f == Form::Cdr2Le || topic.ty.is_some() => f,
            Some(_) => {
                core.metrics.errors_total.inc();
                reply(
                    out,
                    CoapCode::new(4, 15),
                    format!(
                        "{} has no idl_file: it takes {} only",
                        topic.dds_name, core.formats.cdr2_le
                    ),
                );
                return;
            }
            None => {
                core.metrics.errors_total.inc();
                reply(
                    out,
                    CoapCode::new(4, 15),
                    format!("Content-Format {n} is no form of a sample"),
                );
                return;
            }
        },
    };
    // §5.10.8: If-Match and If-None-Match against the current
    // representation.
    let current = core.current(path, topic, Instant::now());
    if !preconditions_hold(req, current.as_ref().map(|s| &*s.body)) {
        reply(out, CoapCode::new(4, 12), String::new());
        return;
    }

    // Block1: collect the blocks, then write.
    let payload = if let Some(b1) = extract_block(req, numbers::BLOCK1) {
        core.metrics.block_in(&topic.label);
        let key = BlockKey {
            peer,
            path: path.to_string(),
        };
        let Ok(mut map) = core.block1.lock() else {
            reply(out, CoapCode::INTERNAL_SERVER_ERROR, String::new());
            return;
        };
        let entry = map
            .entry(key.clone())
            .or_insert_with(|| BlockReassembler::new(b1.block_size().unwrap_or(16)));
        if let Err(e) = entry.accept(b1, &req.payload) {
            // RFC 7959 §2.9.2: a block that does not continue the
            // transfer is 4.08 Request Entity Incomplete.
            map.remove(&key);
            core.metrics.errors_total.inc();
            reply(out, CoapCode::new(4, 8), e.to_string());
            return;
        }
        if entry.len() > core.max_body_size {
            // RFC 7959 §2.9.3: 4.13 with the largest size in Size1.
            map.remove(&key);
            core.metrics.errors_total.inc();
            let mut resp = make_response(req, CoapCode::new(4, 13), Vec::new());
            resp.options.push(CoapOption::new(
                numbers::SIZE1,
                OptionValue::Uint(core.max_body_size as u64),
            ));
            out.responses.push(resp);
            return;
        }
        if !entry.is_complete() {
            let mut resp = make_response(req, CoapCode::new(2, 31), Vec::new());
            resp.options.push(CoapOption::block1(b1.num, true, b1.szx));
            out.responses.push(resp);
            return;
        }
        let Some(done) = map.remove(&key) else {
            reply(out, CoapCode::INTERNAL_SERVER_ERROR, String::new());
            return;
        };
        Some((done.into_payload(), b1))
    } else {
        None
    };
    let (body, block1) = match payload {
        Some((body, b1)) => (body, Some(b1)),
        None => (req.payload.clone(), None),
    };

    let started = unix_ns();
    match write_sample(core, topic, eid, form, body) {
        Ok(()) => {
            out.exchange.written = Some((started, unix_ns()));
            core.metrics.dds_published(&topic.label);
            let mut resp = make_response(req, CoapCode::CHANGED, Vec::new());
            if let Some(b1) = block1 {
                resp.options.push(CoapOption::block1(b1.num, false, b1.szx));
            }
            out.responses.push(resp);
        }
        Err((code, text)) => {
            core.metrics.errors_total.inc();
            reply(out, code, text);
        }
    }
}

/// The body in `form` as a DDS sample of the topic: with a type, read in
/// that form and written in the topic's `data_representation` (XCDR2 or
/// XCDR1, little endian) in the format of the type's extensibility, keyed by its KeyHash for a keyed type (`4.00` for
/// a body that is no sample of the type); without one, the XCDR2 body as
/// it is. A failed DDS write is `5.00` (spec §4.2).
fn write_sample(
    core: &Core,
    topic: &Topic,
    eid: EntityId,
    form: Form,
    body: Vec<u8>,
) -> Result<(), (CoapCode, String)> {
    let dds_error = |e: zerodds_dcps::DdsError| {
        (
            CoapCode::INTERNAL_SERVER_ERROR,
            format!("the DDS write failed: {e:?}"),
        )
    };
    let Some(ty) = &topic.ty else {
        return core.runtime.write_user_sample(eid, body).map_err(dds_error);
    };
    let value = ty.read(form, &body).map_err(|e| {
        (
            CoapCode::BAD_REQUEST,
            format!("no sample of {}: {e}", topic.dds_type),
        )
    })?;
    let wire = if form == topic.wire {
        body
    } else {
        ty.write(topic.wire, &value)
            .map_err(|e| (CoapCode::BAD_REQUEST, e))?
    };
    if ty.keyed() {
        let key_hash = ty
            .key_hash(&value)
            .map_err(|e| (CoapCode::BAD_REQUEST, e))?;
        // The writer keeps the holder of the instance: a later DELETE by
        // its KeyHash sends it as `serializedKey`.
        let holder = ty
            .key_holder(&value)
            .map_err(|e| (CoapCode::BAD_REQUEST, e))?;
        core.runtime
            .write_user_sample_keyed_with_holder(eid, &wire, key_hash, &|| holder.clone(), None)
            .map_err(dds_error)
    } else {
        core.runtime.write_user_sample(eid, wire).map_err(dds_error)
    }
}

/// DELETE: a dispose of the instance the request names (spec §4.2) — by
/// `key_hash=<32 hexadecimal digits>` in Uri-Query, or by the key members
/// of a sample in the body (in the form its Content-Format names; it needs
/// the type). `2.02 Deleted`; a topic without key or without writer is
/// `4.05`.
fn serve_delete(
    core: &Core,
    req: &CoapMessage,
    topic: &Topic,
    subject: &AuthSubject,
    out: &mut Dispatched,
) {
    let reply = |out: &mut Dispatched, code: CoapCode, text: String| {
        out.responses
            .push(make_response(req, code, text.into_bytes()));
    };
    let Some(eid) = topic.writer else {
        reply(
            out,
            CoapCode::new(4, 5),
            format!("{} is direction out", topic.dds_name),
        );
        return;
    };
    if !core.allowed(subject, AclOp::Write, topic) {
        reply(out, CoapCode::new(4, 3), "forbidden".to_string());
        return;
    }
    if topic.ty.as_ref().is_some_and(|t| !t.keyed()) {
        reply(
            out,
            CoapCode::new(4, 5),
            format!("{} has no key: no instance to dispose", topic.dds_type),
        );
        return;
    }
    let by_query = uri_queries(req)
        .iter()
        .find_map(|q| q.strip_prefix("key_hash=").map(ToString::to_string));
    // DDSI-RTPS 2.5 §9.4.2.11: the dispose carries the key holder of a
    // sample in the body; by KeyHash the writer's recorded one.
    let mut key_holder: Option<Vec<u8>> = None;
    let key_hash = match (by_query, &topic.ty) {
        (Some(hex), _) => match unhex16(&hex) {
            Some(h) => h,
            None => {
                reply(
                    out,
                    CoapCode::BAD_REQUEST,
                    "key_hash is 32 hexadecimal digits".to_string(),
                );
                return;
            }
        },
        (None, Some(ty)) if !req.payload.is_empty() => {
            let form = match uint_option(req, numbers::CONTENT_FORMAT) {
                None => Form::Cdr2Le,
                Some(n) => match core.formats.form(n) {
                    Some(f) => f,
                    None => {
                        reply(
                            out,
                            CoapCode::new(4, 15),
                            format!("Content-Format {n} is no form of a sample"),
                        );
                        return;
                    }
                },
            };
            match ty
                .read(form, &req.payload)
                .and_then(|v| Ok((ty.key_hash(&v)?, ty.key_holder(&v)?)))
            {
                Ok((h, holder)) => {
                    key_holder = Some(holder);
                    h
                }
                Err(e) => {
                    reply(out, CoapCode::BAD_REQUEST, e);
                    return;
                }
            }
        }
        _ => {
            reply(
                out,
                CoapCode::BAD_REQUEST,
                "DELETE names its instance by Uri-Query key_hash or a sample in the body"
                    .to_string(),
            );
            return;
        }
    };
    let started = unix_ns();
    match core.runtime.write_user_lifecycle(
        eid,
        key_hash,
        key_holder.as_deref(),
        zerodds_rtps::inline_qos::status_info::DISPOSED,
        None,
    ) {
        Ok(()) => {
            out.exchange.written = Some((started, unix_ns()));
            core.metrics.dds_published(&topic.label);
            out.responses
                .push(make_response(req, CoapCode::DELETED, Vec::new()));
        }
        Err(e) => {
            core.metrics.errors_total.inc();
            reply(
                out,
                CoapCode::INTERNAL_SERVER_ERROR,
                format!("the DDS dispose failed: {e:?}"),
            );
        }
    }
}

/// GET of a topic: the current sample, or an observation (spec §4.3).
fn serve_get(
    core: &Core,
    req: &CoapMessage,
    path: &str,
    topic: &Topic,
    subject: &AuthSubject,
    limit: u8,
    out: &mut Dispatched,
) {
    let now = Instant::now();
    // §5.8: a topic without a reader (`direction: in`) has nothing to give.
    if !topic.reader {
        out.responses.push(make_response(
            req,
            CoapCode::new(4, 5),
            format!("{} is direction in", topic.dds_name).into_bytes(),
        ));
        return;
    }
    if !core.allowed(subject, AclOp::Read, topic) {
        out.responses.push(make_response(
            req,
            CoapCode::new(4, 3),
            b"forbidden".to_vec(),
        ));
        return;
    }
    // §5.10.4: the form Accept asks for; a number of no form is 4.06, and
    // so is JSON or CBOR of a topic without a type (§4.5: it has only the
    // CDR forms of its samples).
    let form = match uint_option(req, numbers::ACCEPT) {
        None => None,
        Some(n) => match core.formats.form(n) {
            Some(Form::Json | Form::Cbor) if topic.ty.is_none() => {
                out.responses.push(make_response(
                    req,
                    CoapCode::new(4, 6),
                    Unrepresentable::NotAcceptable.to_string().into_bytes(),
                ));
                return;
            }
            Some(f) => Some(f),
            None => {
                out.responses
                    .push(make_response(req, CoapCode::new(4, 6), Vec::new()));
                return;
            }
        },
    };
    let observe = extract_observe(req);
    if observe == Some(1) {
        // RFC 7641 §3.6: deregister; the response is the current state.
        out.observe = ObserveDirective::Deregister {
            path: path.to_string(),
            token: req.token.clone(),
        };
    }
    // Spec §6: a missed deadline is 5.03 with Max-Age 0.
    if core.deadline_missed(path, topic, now) {
        let mut resp = make_response(req, CoapCode::new(5, 3), b"deadline missed".to_vec());
        resp.options
            .push(CoapOption::new(numbers::MAX_AGE, OptionValue::Uint(0)));
        out.responses.push(resp);
        return;
    }
    if observe == Some(0) {
        let window = core.window(path, topic, now);
        let mut resp = match &window.first {
            Some(stored) => match content_response(core, req, topic, stored, form, limit, now) {
                Ok(resp) => resp,
                Err(resp) => {
                    out.responses.push(resp);
                    return;
                }
            },
            // Spec §4.3: `2.05` empty for VOLATILE and before the first.
            None => make_response(req, CoapCode::CONTENT, Vec::new()),
        };
        resp.options.push(CoapOption::observe(window.sequence));
        resp.options.sort_by_key(|o| o.number);
        out.responses.push(resp);
        out.observe = ObserveDirective::Register {
            path: path.to_string(),
            token: req.token.clone(),
            oscore: None,
            block_szx: limit,
            form,
            replay: window.rest,
        };
        return;
    }
    let Some(current) = core.current(path, topic, now) else {
        // Before the first sample, and once its lifespan has passed: no
        // representation.
        out.responses
            .push(make_response(req, CoapCode::CONTENT, Vec::new()));
        return;
    };
    // §5.10.6.2: an ETag of the request that names the current
    // representation is answered with 2.03 Valid (§5.9.1.3).
    let etag = etag_of(&current.body);
    if option_values(req, numbers::ETAG).contains(&etag) {
        let mut valid = make_response(req, CoapCode::VALID, Vec::new());
        valid
            .options
            .push(CoapOption::new(numbers::ETAG, OptionValue::Opaque(etag)));
        if let Some(age) = current.max_age(topic.behavior.lifespan, now) {
            valid.options.push(CoapOption::new(
                numbers::MAX_AGE,
                OptionValue::Uint(u64::from(age)),
            ));
        }
        out.responses.push(valid);
        return;
    }
    match content_response(core, req, topic, &current, form, limit, now) {
        Ok(resp) | Err(resp) => out.responses.push(resp),
    }
}

/// The `2.05` of `stored` in `form`: its Content-Format, its ETag, its
/// Max-Age, the block the request asks for or the first (RFC 7959 §2.2);
/// `Err` with `4.06` for a form the topic cannot give, `5.00` for a sample
/// that does not decode.
fn content_response(
    core: &Core,
    req: &CoapMessage,
    topic: &Topic,
    stored: &Stored,
    form: Option<Form>,
    limit: u8,
    now: Instant,
) -> Result<CoapMessage, CoapMessage> {
    let (payload, format) = core.represent(topic, stored, form).map_err(|e| {
        let code = match e {
            Unrepresentable::NotAcceptable | Unrepresentable::NoForm(_) => CoapCode::new(4, 6),
            Unrepresentable::Broken(_) => CoapCode::INTERNAL_SERVER_ERROR,
        };
        core.metrics.errors_total.inc();
        make_response(req, code, e.to_string().into_bytes())
    })?;
    let blocked = payload.len() > 16usize << limit || extract_block(req, numbers::BLOCK2).is_some();
    let mut resp = representation(req, payload, Some(limit));
    if resp.code != CoapCode::CONTENT {
        return Err(resp);
    }
    if blocked {
        core.metrics.block_out(&topic.label);
    }
    resp.options.retain(|o| o.number != numbers::ETAG);
    resp.options.push(CoapOption::new(
        numbers::ETAG,
        OptionValue::Opaque(etag_of(&stored.body)),
    ));
    resp.options.push(CoapOption::content_format(format));
    if let Some(age) = stored.max_age(topic.behavior.lifespan, now) {
        resp.options.push(CoapOption::new(
            numbers::MAX_AGE,
            OptionValue::Uint(u64::from(age)),
        ));
    }
    resp.options.sort_by_key(|o| o.number);
    Ok(resp)
}

/// The options the daemon processes in a request to its own resources.
const LOCAL_OPTIONS: [u16; 19] = [
    numbers::IF_MATCH,
    numbers::URI_HOST,
    numbers::ETAG,
    numbers::IF_NONE_MATCH,
    numbers::OBSERVE,
    numbers::URI_PORT,
    numbers::OSCORE,
    numbers::URI_PATH,
    numbers::CONTENT_FORMAT,
    numbers::URI_QUERY,
    numbers::HOP_LIMIT,
    numbers::ACCEPT,
    numbers::BLOCK2,
    numbers::BLOCK1,
    numbers::SIZE2,
    numbers::PROXY_URI,
    numbers::PROXY_SCHEME,
    numbers::SIZE1,
    COAP_OPTION_AUTH_TOKEN,
];

/// The first critical option of `req` the daemon does not recognize, or a
/// second occurrence of a critical option that is not repeatable (RFC 7252
/// §5.4.1, §5.4.5).
fn unrecognized_critical(req: &CoapMessage) -> Option<u16> {
    let mut seen = std::collections::BTreeSet::new();
    req.options.iter().map(|o| o.number).find(|&n| {
        let again = !seen.insert(n) && !crate::option::is_repeatable(n);
        crate::option::is_critical(n) && (again || !LOCAL_OPTIONS.contains(&n))
    })
}

fn option_values(msg: &CoapMessage, number: u16) -> Vec<Vec<u8>> {
    msg.options
        .iter()
        .filter(|o| o.number == number)
        .map(|o| o.value.to_wire_bytes())
        .collect()
}

fn uint_option(msg: &CoapMessage, number: u16) -> Option<u64> {
    msg.options
        .iter()
        .find(|o| o.number == number)
        .and_then(|o| crate::option::uint_from_bytes(&o.value.to_wire_bytes()))
}

/// The ETag of a representation: 8 octets of a hash of the sample, the same
/// for the same sample in every run and every form (RFC 7252 §5.10.6.1).
pub(super) fn etag_of(payload: &[u8]) -> Vec<u8> {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    payload.hash(&mut hasher);
    hasher.finish().to_be_bytes().to_vec()
}

/// RFC 7252 §5.10.8: If-Match holds when one of its values is the ETag of
/// the current representation, or is empty and there is one; If-None-Match
/// holds when there is none. A topic has a representation once it has a
/// sample.
fn preconditions_hold(req: &CoapMessage, current: Option<&[u8]>) -> bool {
    let if_match = option_values(req, numbers::IF_MATCH);
    if !if_match.is_empty() {
        let Some(sample) = current else {
            return false;
        };
        let etag = etag_of(sample);
        if !if_match.iter().any(|v| v.is_empty() || *v == etag) {
            return false;
        }
    }
    let if_none_match = req
        .options
        .iter()
        .any(|o| o.number == numbers::IF_NONE_MATCH);
    !(if_none_match && current.is_some())
}

/// The method of a request as text, for the log and the metrics.
fn method_name(code: CoapCode) -> String {
    match code {
        CoapCode::GET => "GET".to_string(),
        CoapCode::POST => "POST".to_string(),
        CoapCode::PUT => "PUT".to_string(),
        CoapCode::DELETE => "DELETE".to_string(),
        other => other.to_string(),
    }
}

/// The end of an exchange (spec §8): the line of the log, the count of
/// `requests_total{method, code}`, the span.
fn finish(core: &Core, peer: &str, request_len: usize, started: (Instant, u64), d: &Dispatched) {
    let req = &d.request;
    let code = d.responses.first().map_or(CoapCode::EMPTY, |r| r.code);
    let code_text = code.to_string();
    let method = method_name(req.code);
    core.metrics.request(&method, &code_text);
    let latency_us = u64::try_from(started.0.elapsed().as_micros()).unwrap_or(u64::MAX);
    let level = match code.class {
        5 => Level::Warn,
        4 => Level::Info,
        _ => Level::Debug,
    };
    if log::enabled(level) {
        let mut event = log::event(level, "request")
            .field("peer", peer)
            .field("method", &method)
            .field("code", &code_text)
            .field("coap_uri", &d.exchange.uri)
            .number("mid", u64::from(req.message_id))
            .token(&req.token)
            .number("bytes", request_len as u64)
            .number("latency_us", latency_us);
        if let Some(topic) = &d.exchange.dds_topic {
            event = event.field("dds_topic", topic);
        }
        if let Some(tenant) = &core.tenant {
            event = event.field("tenant", tenant);
        }
        if let Some(b) =
            extract_block(req, numbers::BLOCK1).or_else(|| extract_block(req, numbers::BLOCK2))
        {
            event = event.number("block_num", u64::from(b.num));
        }
        event.emit();
    }
    if core.tracer.enabled() {
        let mut attributes = vec![
            ("coap.method", method),
            ("coap.code", code_text),
            ("coap.uri", d.exchange.uri.clone()),
            ("net.peer", peer.to_string()),
        ];
        if let Some(topic) = &d.exchange.dds_topic {
            attributes.push(("dds.topic", topic.clone()));
        }
        if let Some(tenant) = &core.tenant {
            attributes.push(("zerodds.tenant", tenant.clone()));
        }
        core.tracer.exchange(ExchangeSpan {
            start_ns: started.1,
            end_ns: unix_ns(),
            ok: code.class == 2 || code == CoapCode::EMPTY,
            attributes,
            write: d.exchange.written,
        });
    }
}

pub(super) fn extract_auth_token(msg: &CoapMessage) -> Option<Vec<u8>> {
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

/// The resource name of a request (RFC 7252 §6.5 step 6), without the
/// leading "/": the Uri-Path options joined by "/", every byte of a
/// segment outside the unreserved and sub-delims sets of RFC 3986, ':' and
/// '@' percent-encoded with uppercase digits. A segment holding "/" is thus
/// `%2F` and never names another resource than the one it is part of.
/// `None` for a Uri-Path "." or ".." (§5.10.1: "MUST NOT be").
fn resource_name(msg: &CoapMessage) -> Option<String> {
    let mut name = String::new();
    let mut first = true;
    for opt in &msg.options {
        if opt.number != numbers::URI_PATH {
            continue;
        }
        let bytes: Vec<u8> = match &opt.value {
            OptionValue::String(s) => s.as_bytes().to_vec(),
            OptionValue::Opaque(b) => b.clone(),
            OptionValue::Empty => Vec::new(),
            OptionValue::Uint(v) => {
                let all = v.to_be_bytes();
                let skip = all.iter().take_while(|b| **b == 0).count();
                all[skip..].to_vec()
            }
        };
        if bytes == b"." || bytes == b".." {
            return None;
        }
        if !first {
            name.push('/');
        }
        first = false;
        for byte in bytes {
            let keep = byte.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@".contains(&byte);
            if keep {
                name.push(char::from(byte));
            } else {
                name.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    Some(name)
}

/// The factor a response to a request that is not authenticated may be
/// larger than the request, at most (RFC 7252 §11.3: "large amplification
/// factors SHOULD NOT be provided in the response if the request is not
/// authenticated"; its example, a 10-byte request answered with a 64-byte
/// block, is a factor of 8).
pub const UNAUTHENTICATED_AMPLIFICATION: usize = 8;

/// The largest block (as SZX) a response may carry: `None` for an
/// authenticated request, else the largest block of at most
/// [`UNAUTHENTICATED_AMPLIFICATION`] times the request's size (16 octets at
/// least, SZX 0).
pub(super) fn block_limit(authenticated: bool, request_len: usize) -> Option<u8> {
    if authenticated {
        return None;
    }
    let budget = request_len.saturating_mul(UNAUTHENTICATED_AMPLIFICATION);
    let mut szx = 0u8;
    while szx < 6 && (16usize << (szx + 1)) <= budget {
        szx += 1;
    }
    Some(szx)
}

/// A `2.05` response with `payload`, the block the request asks for with
/// Block2 (RFC 7959 §2.2, §2.4) or, where `limit` is set or the request
/// asks for a smaller one, the first block; a block response carries
/// Block2, the ETag of the whole representation (so that a client sees a
/// change between blocks) and, in the first block, Size2 (§4). A block
/// past the end is `4.02 Bad Option`.
pub(super) fn representation(
    req: &CoapMessage,
    payload: Vec<u8>,
    limit: Option<u8>,
) -> CoapMessage {
    let requested = extract_block(req, numbers::BLOCK2);
    let max_szx = limit.unwrap_or(6);
    let (num, szx) = match requested {
        // §2.4: a smaller block size than asked for scales the number.
        Some(b) if b.szx > max_szx => (b.num << (b.szx - max_szx), max_szx),
        Some(b) => (b.num, b.szx),
        None => (0, max_szx),
    };
    let size = 16usize << szx;
    if requested.is_none() && payload.len() <= size {
        return make_response(req, CoapCode::CONTENT, payload);
    }
    let start = (num as usize).saturating_mul(size);
    if start > payload.len() || (start == payload.len() && start != 0) {
        return make_response(req, CoapCode::new(4, 2), Vec::new());
    }
    let end = (start + size).min(payload.len());
    let more = end < payload.len();
    let etag = etag_of(&payload);
    let mut resp = make_response(req, CoapCode::CONTENT, payload[start..end].to_vec());
    resp.options
        .push(CoapOption::new(numbers::ETAG, OptionValue::Opaque(etag)));
    resp.options.push(CoapOption::block2(num, more, szx));
    if num == 0 {
        resp.options.push(CoapOption::size2(payload.len() as u64));
    }
    resp
}

/// Extracts the Block1 or Block2 option from a CoapMessage.
/// Spec RFC 7959 §2.1.
pub(super) fn extract_block(msg: &CoapMessage, option_number: u16) -> Option<BlockValue> {
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

/// The catalog (RFC 6690, spec §5.2): a link per topic with
/// `rt="dds.topic"`, the Content-Formats it gives (all five with a type,
/// XCDR2 little endian without) and the DDS type in `type`; a link per
/// topic with a type to its IDL with `rt="dds.schema"`. Filtered by each
/// query of the request (RFC 6690 §4.1); a query that is no `name=value`
/// filters nothing.
fn render_well_known_core(core: &Core, queries: &[String]) -> String {
    let mut links: Vec<crate::core_link::CoreLink> = Vec::new();
    for (path, topic) in &core.topics {
        let ct = if topic.ty.is_some() {
            Form::ALL
                .iter()
                .map(|f| core.formats.number(*f).to_string())
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            core.formats.cdr2_le.to_string()
        };
        links.push(
            crate::core_link::CoreLink::new(&format!("/{path}"))
                .attr("rt", "dds.topic")
                .attr("ct", &ct)
                .attr("type", &topic.dds_type),
        );
        if topic.ty.is_some() {
            links.push(
                crate::core_link::CoreLink::new(&format!("/{SCHEMA_PREFIX}{path}"))
                    .attr("rt", "dds.schema")
                    .attr("ct", &content_format::TEXT_PLAIN_UTF8.to_string()),
            );
        }
    }
    for query in queries {
        if let Some(filtered) = crate::core_link::filter_links(&links, query) {
            links = filtered;
        }
    }
    crate::core_link::encode_links(&links)
}

/// The Uri-Query options of a request, as text.
fn uri_queries(msg: &CoapMessage) -> Vec<String> {
    msg.options
        .iter()
        .filter(|o| o.number == numbers::URI_QUERY)
        .map(|o| match &o.value {
            OptionValue::String(s) => s.clone(),
            OptionValue::Opaque(b) => String::from_utf8_lossy(b).into_owned(),
            OptionValue::Empty => String::new(),
            OptionValue::Uint(v) => format!("{v}"),
        })
        .collect()
}

fn unhex16(text: &str) -> Option<[u8; 16]> {
    if text.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(out)
}

pub(super) fn make_response(req: &CoapMessage, code: CoapCode, payload: Vec<u8>) -> CoapMessage {
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

// ---------------------------------------------------------------------------
// DTLS (`coaps://`) serving — RFC 7252 §9, opt-in `dtls` feature (ADR 0014).
// A thread accepts the sessions of `zerodds-dtls-openssl`; each session runs
// on a thread of its own through the *same* `dispatch` core as the plaintext
// path, so request/response + CoAP Observe behave identically over coaps://.
// ---------------------------------------------------------------------------

/// Default `coaps://` bind: same host as the plaintext bind, port 5684.
#[cfg(feature = "dtls")]
fn default_coaps_bind(plain: &str) -> String {
    match plain.parse::<SocketAddr>() {
        Ok(a) => SocketAddr::new(a.ip(), crate::dtls_transport::COAPS_PORT).to_string(),
        Err(_) => format!("0.0.0.0:{}", crate::dtls_transport::COAPS_PORT),
    }
}

/// What a DTLS session serves with: the state the plaintext path has, and
/// the tenants (§1 L6) a session's subject may bind it to.
#[cfg(feature = "dtls")]
#[derive(Clone)]
struct DtlsShared {
    core: Arc<Core>,
    tenants: Arc<Vec<(Vec<String>, Arc<Core>)>>,
    proxy: Arc<Proxy>,
    dedup: Arc<Dedup>,
}

#[cfg(feature = "dtls")]
impl DtlsShared {
    /// Spec §1 L6: the core a session of `subject` is bound to — the
    /// tenant that names its subject (its name, or the `mtls:` fingerprint
    /// of a certificate), else the participant of `domain`.
    fn core_of(&self, subject: Option<&AuthSubject>) -> &Arc<Core> {
        let Some(subject) = subject else {
            return &self.core;
        };
        self.tenants
            .iter()
            .find(|(subjects, _)| {
                subjects
                    .iter()
                    .any(|s| *s == subject.name || subject.fingerprint.as_deref() == Some(s))
            })
            .map_or(&self.core, |(_, core)| core)
    }
}

/// Binds the DTLS listener and starts the thread that accepts its sessions
/// until `stop`; the credentials are taken again on `reload_flag` (SIGHUP,
/// Bridge-Spec §7.1: the established sessions keep theirs).
///
/// # Errors
/// [`ServerError::Dtls`] for an address, a socket or credentials that do
/// not make a listener.
#[cfg(feature = "dtls")]
fn spawn_dtls_server(
    bind_addr: &str,
    credentials: crate::dtls::DtlsCredentials,
    reload_flag: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    shared: DtlsShared,
) -> Result<(JoinHandle<()>, SocketAddr), ServerError> {
    let addr: SocketAddr = bind_addr
        .parse()
        .map_err(|e| ServerError::Dtls(format!("dtls bind addr {bind_addr}: {e}")))?;
    let server = crate::dtls_transport::DtlsCoapServer::bind_with(addr, &credentials)
        .map_err(|e| ServerError::Dtls(format!("dtls on {addr}: {e}")))?;
    let local = server
        .local_addr()
        .map_err(|e| ServerError::Dtls(format!("{e}")))?;
    log::event(Level::Info, "dtls_listening")
        .field("peer", &local.to_string())
        .emit();
    let thread = thread::Builder::new()
        .name("coap-dtls".into())
        .spawn(move || {
            let mut sessions: Vec<JoinHandle<()>> = Vec::new();
            let mut next_sid = 1u64;
            let mut failed_seen = 0u64;
            while !stop.load(Ordering::SeqCst) {
                if reload_flag.swap(false, Ordering::SeqCst) {
                    match server.rotate(&credentials) {
                        Ok(()) => log::event(Level::Info, "dtls_reloaded").emit(),
                        Err(e) => log::event(Level::Error, "dtls_reload_failed")
                            .field("reason", &e.to_string())
                            .emit(),
                    }
                }
                let session = server.accept(Duration::from_millis(200));
                // §8.2: `dtls_handshakes_total{result}`.
                let failed = server.failed_handshakes();
                for _ in failed_seen..failed {
                    shared.core.metrics.handshake("failed");
                }
                failed_seen = failed;
                let Some(session) = session else {
                    continue;
                };
                shared.core.metrics.handshake("ok");
                let sid = next_sid;
                next_sid += 1;
                shared.core.metrics.connections_total.inc();
                sessions.retain(|h| !h.is_finished());
                let (shared, stop) = (shared.clone(), Arc::clone(&stop));
                let spawned = thread::Builder::new()
                    .name("coap-dtls-session".into())
                    .spawn(move || dtls_session(session, sid, local, &shared, &stop));
                if let Ok(h) = spawned {
                    sessions.push(h);
                }
            }
            for h in sessions {
                let _ = h.join();
            }
        })
        .map_err(|e| ServerError::Io(format!("spawn dtls thread: {e}")))?;
    Ok((thread, local))
}

/// The bridge's subject for a DTLS peer (§7.1, §7.3): the certificate's
/// subject DN (its SHA-256 fingerprint kept beside it), `psk:<identity>`,
/// or `rpk:<ni URI>` of the raw public key (RFC 6920, sha-256).
#[cfg(feature = "dtls")]
fn dtls_subject(identity: &crate::dtls_transport::DtlsPeerIdentity) -> AuthSubject {
    use crate::dtls_transport::DtlsPeerIdentity;
    match identity {
        DtlsPeerIdentity::Certificate(der) => zerodds_bridge_security::mtls_subject_from_der(der),
        DtlsPeerIdentity::Psk(id) => {
            AuthSubject::new(format!("psk:{}", String::from_utf8_lossy(id)))
        }
        DtlsPeerIdentity::RawPublicKey(spki) => AuthSubject::new(format!(
            "rpk:{}",
            crate::dtls_transport::KeyName::of_key(spki, 1)
                .map_or_else(|_| String::new(), |n| n.to_ni_uri())
        )),
    }
}

/// One DTLS session: its requests through the shared `dispatch`, and what
/// other threads queued for it (notifications, the proxy's separate
/// responses, the shutdown's 5.03) written when its receive is woken.
#[cfg(feature = "dtls")]
fn dtls_session(
    mut session: crate::dtls_transport::DtlsCoapSession,
    sid: u64,
    local: SocketAddr,
    s: &DtlsShared,
    stop: &AtomicBool,
) {
    let peer = session.peer_addr();
    let dtls_peer = session.peer_identity().map(|id| dtls_subject(&id));
    // §1 L6: the session's tenant, for its whole life.
    let core = s.core_of(dtls_peer.as_ref());
    if let Some(tenant) = &core.tenant {
        log::event(Level::Info, "tenant_bound")
            .field("peer", &peer.to_string())
            .field("tenant", tenant)
            .emit();
    }
    let endpoint = format!("dtls:{sid}:{peer}");
    let peer_text = format!("{peer} ({})", dtls_peer.as_ref().map_or("", |a| &a.name));
    let (tx, rx) = std::sync::mpsc::channel::<CoapMessage>();
    let out = DtlsTx {
        tx,
        waker: session.waker(),
    };
    loop {
        while let Ok(msg) = rx.try_recv() {
            if session.send_message(&msg).is_ok() {
                core.metrics.frames_out_total.inc();
            }
        }
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let req = match session.recv_message(Duration::from_millis(500)) {
            Ok(Some(m)) => m,
            Ok(None) => continue,
            Err(crate::dtls_transport::DtlsError::Codec(_)) => {
                core.metrics.errors_total.inc();
                continue;
            }
            // The peer closed the session or it broke.
            Err(_) => break,
        };
        let started = (Instant::now(), unix_ns());
        let Ok(bytes) = encode(&req) else {
            core.metrics.errors_total.inc();
            continue;
        };
        core.metrics.frames_in_total.inc();
        core.metrics.bytes_in_total.add(bytes.len() as u64);
        if !req.code.is_request() {
            let queue = out.clone();
            message_layer(&req, Endpoint::Dtls(sid), peer, core, &s.proxy, &move |m| {
                let _ = queue.send(m.clone());
            });
            continue;
        }
        let leg = ClientLeg {
            reply: ReplyTo::Dtls {
                peer,
                tx: out.clone(),
            },
            destination: local,
            dtls_peer: dtls_peer.clone(),
            request_len: bytes.len(),
        };
        let bytes = match s.proxy.handle(&req, leg) {
            Handled::Proxied => continue,
            Handled::Local(Some(rewritten)) => match encode(&rewritten) {
                Ok(b) => b,
                Err(_) => continue,
            },
            Handled::Local(None) => bytes,
        };
        if let Seen::Again(ack) = s.dedup.receive(&endpoint, &req) {
            if let Some(ack) = ack.filter(|_| matches!(req.message_type, MessageType::Confirmable))
            {
                let _ = session.send_message(&ack);
            }
            continue;
        }
        match dispatch(core, &bytes, peer, dtls_peer.as_ref()) {
            Ok(d) => {
                for resp in &d.responses {
                    if session.send_message(resp).is_ok() {
                        core.metrics.frames_out_total.inc();
                    }
                }
                s.dedup.remember(&endpoint, &d.responses);
                finish(core, &peer_text, bytes.len(), started, &d);
                let observer_tx = out.clone();
                core.apply_observe(d.observe, &move |token| Observer::Dtls {
                    session: sid,
                    token,
                    tx: observer_tx.clone(),
                });
            }
            Err(e) => {
                core.metrics.errors_total.inc();
                log::event(Level::Warn, "request_failed")
                    .field("peer", &peer_text)
                    .field("reason", &e)
                    .emit();
            }
        }
    }
    // The session ended: its observers go.
    core.drop_endpoint(Endpoint::Dtls(sid));
    session.close();
}

/// The DDS endpoints of a topic (spec §9.1 step 4): a writer for `in` and
/// `bidir`, a reader for `out` and `bidir`, with the topic's QoS (§6). The
/// writer sends XCDR2 under the header of the type's extensibility, or of
/// `extensibility:` without a type (§4.5).
fn register_topic(
    rt: &Arc<DcpsRuntime>,
    topic: &TopicConfig,
    ty: Option<Arc<TopicType>>,
) -> Result<(Topic, Option<std::sync::mpsc::Receiver<UserSample>>), ServerError> {
    use zerodds_qos::{
        DeadlineQosPolicy, DurabilityKind, LifespanQosPolicy, LivelinessKind, LivelinessQosPolicy,
    };
    let durability = match topic.durability.as_str() {
        "transient_local" => DurabilityKind::TransientLocal,
        "transient" => DurabilityKind::Transient,
        "persistent" => DurabilityKind::Persistent,
        _ => DurabilityKind::Volatile,
    };
    let reliable = !matches!(topic.reliability.as_str(), "best_effort");
    let want_writer = matches!(topic.direction.as_str(), "in" | "bidir");
    let want_reader = matches!(topic.direction.as_str(), "out" | "bidir");
    let duration =
        |ms: u64| zerodds_qos::Duration::from_millis(i32::try_from(ms).unwrap_or(i32::MAX));
    let deadline = topic
        .deadline_ms
        .map_or_else(DeadlineQosPolicy::default, |ms| DeadlineQosPolicy {
            period: duration(ms),
        });
    let liveliness = topic
        .liveliness_lease_ms
        .map_or_else(LivelinessQosPolicy::default, |ms| LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            lease_duration: duration(ms),
        });
    let extensibility = ty.as_ref().map_or_else(
        || Extensibility::parse(&topic.extensibility).unwrap_or(Extensibility::Appendable),
        |t| t.extensibility(),
    );
    // `xcdr1` needs the type (the configuration refuses it without one).
    let wire = if topic.data_representation == "xcdr1" && ty.is_some() {
        Form::Cdr1Le
    } else {
        Form::Cdr2Le
    };

    let reader = if want_reader {
        let (_eid, rx) = rt
            .register_user_reader(UserReaderConfig {
                topic_name: topic.dds_name.clone(),
                type_name: topic.dds_type.clone(),
                reliable,
                durability,
                deadline,
                liveliness,
                partition: topic.partition.clone(),
                ..Default::default()
            })
            .map_err(|e| ServerError::Dds(format!("reader: {e:?}")))?;
        Some(rx)
    } else {
        None
    };

    let writer = if want_writer {
        let eid = rt
            .register_user_writer(UserWriterConfig {
                topic_name: topic.dds_name.clone(),
                type_name: topic.dds_type.clone(),
                reliable,
                durability,
                deadline,
                liveliness,
                lifespan: topic
                    .lifespan_ms
                    .map_or_else(LifespanQosPolicy::default, |ms| LifespanQosPolicy {
                        duration: duration(ms),
                    }),
                partition: topic.partition.clone(),
                // §3, §4.5: the writer sends XCDR2, or XCDR1 where the topic
                // says so.
                data_representation_offer: Some(vec![if wire == Form::Cdr1Le {
                    zerodds_rtps::publication_data::data_representation::XCDR
                } else {
                    zerodds_rtps::publication_data::data_representation::XCDR2
                }]),
                ..Default::default()
            })
            .map_err(|e| ServerError::Dds(format!("writer: {e:?}")))?;
        rt.set_user_writer_wire_extensibility(eid, extensibility.for_repr())
            .map_err(|e| ServerError::Dds(format!("writer: {e:?}")))?;
        Some(eid)
    } else {
        None
    };
    Ok((
        Topic {
            dds_name: topic.dds_name.clone(),
            label: topic.dds_name.clone(),
            dds_type: topic.dds_type.clone(),
            ty,
            writer,
            reader: want_reader,
            behavior: topic_behavior(topic),
            wire,
        },
        reader,
    ))
}

/// One SIGHUP reload of the configuration (see [`start_with_reload`]).
fn reload(
    live: &zerodds_bridge_security::LiveSecurity,
    started: &DaemonConfig,
    new: Result<DaemonConfig, String>,
) {
    let new = match new {
        Ok(c) => c,
        Err(e) => {
            log::event(Level::Error, "reload_refused")
                .field("reason", &e)
                .emit();
            return;
        }
    };
    let ctx = match ctx_from_daemon_config(&new) {
        Ok(c) => c,
        Err(e) => {
            log::event(Level::Error, "reload_refused")
                .field("reason", &e.to_string())
                .emit();
            return;
        }
    };
    let mut restart: Vec<&str> = Vec::new();
    if new.topics != started.topics {
        restart.push("topics");
    }
    if new.tenants != started.tenants {
        restart.push("tenants");
    }
    if new.bind != started.bind || new.domain != started.domain {
        restart.push("bind/domain");
    }
    live.replace(ctx);
    log::event(Level::Info, "reloaded")
        .field("parts", "acl, auth")
        .emit();
    if !restart.is_empty() {
        log::event(Level::Warn, "reload_needs_restart")
            .field("parts", &restart.join(", "))
            .emit();
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
    fn coap_6_5_the_resource_name_joins_the_segments() {
        let msg = req_with_path(&["chat", "message"]);
        assert_eq!(resource_name(&msg).as_deref(), Some("chat/message"));
        let empty = CoapMessage::new(MessageType::NonConfirmable, CoapCode::GET, 1);
        assert_eq!(resource_name(&empty).as_deref(), Some(""));
    }

    /// RFC 7252 §6.5 step 6 and §11.1: a "/" inside one Uri-Path segment is
    /// percent-encoded, so `["chat/message"]` is not the resource
    /// `chat/message` the ACL and the topic map name; other characters
    /// outside the allowed sets are encoded with uppercase digits.
    #[test]
    fn coap_11_1_a_slash_in_a_segment_does_not_name_another_resource() {
        let one = req_with_path(&["chat/message"]);
        assert_eq!(resource_name(&one).as_deref(), Some("chat%2Fmessage"));
        assert_ne!(
            resource_name(&one),
            resource_name(&req_with_path(&["chat", "message"]))
        );
        let spaced = req_with_path(&["a b", "ü", "x:y@z"]);
        assert_eq!(
            resource_name(&spaced).as_deref(),
            Some("a%20b/%C3%BC/x:y@z")
        );
    }

    /// RFC 7252 §5.10.1: "The value of a Uri-Path Option MUST NOT be "." or
    /// ".."".
    #[test]
    fn coap_5_10_1_a_dot_segment_is_no_resource_name() {
        assert_eq!(resource_name(&req_with_path(&["a", ".."])), None);
        assert_eq!(resource_name(&req_with_path(&["."])), None);
        assert_eq!(
            resource_name(&req_with_path(&["a.b"])).as_deref(),
            Some("a.b")
        );
    }

    /// RFC 7252 §11.3: a response to a request that is not authenticated
    /// is at most 8 times the request's size, sliced into Block2 blocks
    /// (RFC 7959); the client gets every block by number, the last without
    /// More, a block past the end is 4.02; an authenticated one gets all.
    #[test]
    fn coap_11_3_an_unauthenticated_response_is_sliced_into_small_blocks() {
        let req = req_with_path(&["trade"]);
        let payload: Vec<u8> = (0..200u8).collect();
        assert_eq!(block_limit(false, 12), Some(2)); // 96 octets: 64-octet blocks
        assert_eq!(block_limit(false, 1), Some(0)); // 16 octets at least
        assert_eq!(block_limit(true, 12), None);
        let first = representation(&req, payload.clone(), block_limit(false, 12));
        assert_eq!(first.payload, payload[..64].to_vec());
        let block2 = extract_block(&first, numbers::BLOCK2).expect("Block2");
        assert_eq!((block2.num, block2.more, block2.szx), (0, true, 2));
        assert!(first.options.iter().any(|o| o.number == numbers::SIZE2));
        let etag = first
            .options
            .iter()
            .find(|o| o.number == numbers::ETAG)
            .expect("ETag")
            .clone();
        let mut ask = req.clone();
        ask.options.push(CoapOption::block2(3, false, 2));
        let last = representation(&ask, payload.clone(), block_limit(false, 12));
        assert_eq!(last.payload, payload[192..].to_vec());
        let block2 = extract_block(&last, numbers::BLOCK2).expect("Block2");
        assert_eq!((block2.num, block2.more), (3, false));
        assert!(last.options.contains(&etag));
        let mut past = req.clone();
        past.options.push(CoapOption::block2(4, false, 2));
        assert_eq!(
            representation(&past, payload.clone(), Some(2)).code,
            CoapCode::new(4, 2)
        );
        // A block size larger than allowed: the number scales (§2.4).
        let mut big = req.clone();
        big.options.push(CoapOption::block2(1, false, 3));
        let scaled = representation(&big, payload.clone(), Some(2));
        let block2 = extract_block(&scaled, numbers::BLOCK2).expect("Block2");
        assert_eq!((block2.num, block2.szx), (2, 2));
        assert_eq!(scaled.payload, payload[128..192].to_vec());
        let whole = representation(&req, payload.clone(), None);
        assert_eq!(whole.payload, payload);
        assert!(extract_block(&whole, numbers::BLOCK2).is_none());
    }

    /// RFC 7959 §2.2: `coap.block_size` as SZX, 16 → 0 to 1024 → 6.
    #[test]
    fn rfc7959_2_2_the_block_size_as_szx() {
        for (size, szx) in [
            (16, 0),
            (32, 1),
            (64, 2),
            (128, 3),
            (256, 4),
            (512, 5),
            (1024, 6),
        ] {
            assert_eq!(szx_of(size), szx, "{size}");
        }
    }

    /// RFC 7967 §2.1: 2 suppresses 2.xx, 8 4.xx, 16 5.xx, their sum all
    /// three; an empty No-Response suppresses none.
    #[test]
    fn rfc7967_2_1_the_bits_suppress_the_classes() {
        let with = |v: u64| {
            let mut r = req_with_path(&["t"]);
            r.options.push(CoapOption::new(
                numbers::NO_RESPONSE,
                if v == 0 {
                    OptionValue::Empty
                } else {
                    OptionValue::Uint(v)
                },
            ));
            r
        };
        assert!(suppressed(&with(2), CoapCode::CHANGED));
        assert!(!suppressed(&with(2), CoapCode::new(4, 4)));
        assert!(suppressed(&with(8), CoapCode::new(4, 4)));
        assert!(suppressed(&with(16), CoapCode::INTERNAL_SERVER_ERROR));
        assert!(suppressed(&with(26), CoapCode::CONTENT));
        assert!(!suppressed(&with(0), CoapCode::CONTENT));
        assert!(!suppressed(&req_with_path(&["t"]), CoapCode::CONTENT));
    }

    /// Spec §4.2: a DDS write that fails is `5.00 Internal Server Error`
    /// with the reason, not a request without an answer (before, the
    /// request went unanswered and a Confirmable one was retransmitted).
    #[test]
    fn coapbridge_4_2_a_failed_dds_write_is_5_00() {
        let lease = zerodds_test_domains::lease_domain();
        let mut prefix = [0u8; 12];
        prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
        prefix[11] = 0x5f;
        let runtime = DcpsRuntime::start(
            lease.id(),
            zerodds_rtps::wire_types::GuidPrefix::from_bytes(prefix),
            RuntimeConfig::default(),
        )
        .unwrap();
        let cfg = DaemonConfig::default_for_dev();
        let config = TopicConfig {
            dds_name: "T".into(),
            dds_type: "T".into(),
            coap_uri_path: "t".into(),
            direction: "in".into(),
            ..TopicConfig::default()
        };
        let (topic, _) = register_topic(&runtime, &config, None).unwrap();
        // The writer is gone: the next write fails.
        runtime.remove_user_writer(topic.writer.unwrap());
        let core = Core {
            topics: BTreeMap::from([("t".to_string(), topic)]),
            runtime: Arc::clone(&runtime),
            block1: Mutex::new(BTreeMap::new()),
            metrics: BridgeMetrics::register(&Arc::new(MetricRegistry::new())),
            security: Arc::new(zerodds_bridge_security::LiveSecurity::new(
                ctx_from_daemon_config(&cfg).unwrap(),
            )),
            oscore: None,
            formats: ContentFormats::default(),
            block_szx: 6,
            max_body_size: 65_536,
            observe: Registry::new(
                UdpSocket::bind("127.0.0.1:0").unwrap(),
                Arc::new(AtomicU16::new(1)),
                Timing::of(&cfg),
            ),
            tracer: Tracer::default(),
            tenant: None,
        };
        let mut req = req_with_path(&["t"]);
        req.payload = vec![1, 2, 3, 4];
        let d = dispatch_core(&core, &req, 24, false, "127.0.0.1:9".parse().unwrap(), None);
        assert_eq!(d.responses[0].code, CoapCode::INTERNAL_SERVER_ERROR);
        assert!(
            String::from_utf8_lossy(&d.responses[0].payload).contains("DDS write failed"),
            "{:?}",
            d.responses[0]
        );
        runtime.shutdown();
    }

    /// Spec §4.2: `key_hash=` of a DELETE is 32 hexadecimal digits.
    #[test]
    fn coapbridge_4_2_a_key_hash_is_32_hex_digits() {
        let mut want = [0u8; 16];
        want[3] = 7;
        assert_eq!(unhex16("00000007000000000000000000000000"), Some(want));
        assert_eq!(unhex16("0007"), None);
        assert_eq!(unhex16("zz000007000000000000000000000000"), None);
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
