// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Top-level server for `zerodds-mqtt-bridged`.
//!
//! Spec: `zerodds-mqtt-bridge-1.0.md` §9 (lifecycle), §1 L6 (tenants).
//!
//! Architecture, once for the top level and once per tenant (a *core*):
//! 1. Start the DCPS runtime on its domain.
//! 2. Register a reader+writer per topic.
//! 3. Connect the MQTT client to the broker, under its client identifier.
//! 4. SUBSCRIBE to all MQTT topics for `direction=in|bidir`.
//! 5. Inbound loop thread: MQTT PUBLISH → DDS writer; the Keep Alive.
//! 6. Outbound pump thread per topic: DDS sample → MQTT PUBLISH.

use std::collections::BTreeMap;
use std::string::{String, ToString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::vec::Vec;

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserSample};
use zerodds_rtps::wire_types::EntityId;

use super::acl::{DaemonAcl, LiveAcl, Subject, subject_of};
use super::auth::Credentials;
use super::client::{
    ClientError, InboundEvent, MqttClient, SubscribeEntry, SubscribeOutcome, connect_with_backoff,
};
use super::config::{BrokerUrl, DaemonConfig, TopicConfig, check_mqtt_topic, parse_broker_url};
use super::log::{self, Level};
use super::metrics::{BridgeMetrics, acl_reason};
use super::pump::{InboundMessage, inbound_message, offer_of, wire_extensibility_of};
use super::replay::ReplayBuffer;
use super::runtime_common::{
    CatalogSnapshot, SERVICE_NAME, catalog_topic, install_signal_watcher, otlp_config_from_env,
    serve_admin_endpoints, spawn_otlp_flush_loop,
};
use super::security::{SecurityError, client_tls_from_config};
use super::status_watch::{self, StatusWatch};
use super::topic_map::InboundRoutes;
use super::trace::{DdsWrite, PublishIn, Tracer, unix_ns, unix_ns_since};
use crate::version::ProtocolVersion;
use zerodds_monitor::Registry;
use zerodds_observability_otlp::OtlpExporter;

/// Admin endpoint address when `metrics.enabled` is set without
/// `metrics.listen`; one port per bridge daemon, see man5.
pub const DEFAULT_METRICS_LISTEN: &str = "127.0.0.1:9092";

/// Daemon top-level error (maps to Spec §2 exit codes).
#[derive(Debug)]
pub enum ServerError {
    /// A configuration the daemon cannot run with (exit 1): an auth mode
    /// without its input, a broker URL and a TLS section that contradict
    /// each other.
    Config(String),
    /// Broker-connect error (exit 2).
    BrokerConnect(String),
    /// DCPS init error (exit 3).
    Dds(String),
    /// TLS error (exit 4).
    Tls(String),
    /// Auth error (exit 5).
    Auth(String),
    /// Generic IO error.
    Io(String),
    /// SIGTERM or SIGINT came while the daemon was connecting (exit 0).
    Stopped,
}

impl core::fmt::Display for ServerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Config(m) => write!(f, "config: {m}"),
            Self::BrokerConnect(m) => write!(f, "broker connect: {m}"),
            Self::Dds(m) => write!(f, "dds: {m}"),
            Self::Tls(m) => write!(f, "tls: {m}"),
            Self::Auth(m) => write!(f, "auth: {m}"),
            Self::Io(m) => write!(f, "io: {m}"),
            Self::Stopped => f.write_str("stopped while connecting"),
        }
    }
}

impl std::error::Error for ServerError {}

/// The participant and the broker connection of the top level or of one
/// tenant (Spec §1 L6).
struct Core {
    /// The tenant's name; empty for the top level.
    name: String,
    runtime: Arc<DcpsRuntime>,
    client: Option<Arc<Mutex<MqttClient>>>,
    /// §9.2 drain: the pumps stop first (no new DDS samples go out) ...
    pump_stop: Arc<AtomicBool>,
    /// ... the inbound loop keeps reading the broker's acks until then.
    inbound_stop: Arc<AtomicBool>,
    inbound_thread: Option<JoinHandle<()>>,
    pump_threads: Vec<JoinHandle<()>>,
    /// Its ACL and subject, replaced by a SIGHUP reload (§7.3, §9.2).
    acl: Arc<LiveAcl>,
    /// Its credentials, replaced by a SIGHUP reload (§7.2, §9.2).
    auth: Arc<LiveAuth>,
    /// The topic of the retained catalog it published (§5.2), cleared at
    /// the shutdown.
    catalog_topic: Arc<Mutex<Option<String>>>,
    metrics: BridgeMetrics,
    user_writers: BTreeMap<String, EntityId>,
    user_readers: BTreeMap<String, EntityId>,
}

/// The participant of a tenant (Spec §1 L6), for tests and inspectors.
pub struct TenantHandle {
    /// The tenant's name.
    pub name: String,
    /// Its DCPS runtime.
    pub runtime: Arc<DcpsRuntime>,
    /// Topic name (DDS) → its writer's EntityId.
    pub user_writers: BTreeMap<String, EntityId>,
    /// Topic name (DDS) → its reader's EntityId.
    pub user_readers: BTreeMap<String, EntityId>,
}

/// Daemon-Handle. Drop initiates shutdown.
pub struct DaemonHandle {
    stop: Arc<AtomicBool>,
    /// The top level first, then the tenants.
    cores: Vec<Core>,
    /// `mqtt.drain_timeout_secs`.
    drain_timeout: Duration,
    admin_thread: Option<JoinHandle<()>>,
    otlp_thread: Option<JoinHandle<()>>,
    /// Bound admin address for `/metrics`, `/catalog`, `/healthz`.
    pub admin_addr: Option<String>,
    /// SIGHUP-Reload-Flag.
    pub reload_flag: Arc<AtomicBool>,
    /// Healthz-Flag.
    pub healthy: Arc<AtomicBool>,
    /// Standard metric set for tests.
    pub metrics: Option<BridgeMetrics>,
    /// Bridge-internal DCPS runtime of the top level — managed by the
    /// daemon itself, but exposed for E2E tests (and telemetry inspectors)
    /// so that one can publish against the same participant without
    /// cross-participant discovery.
    pub runtime: Arc<DcpsRuntime>,
    /// Topic name (DDS) → registered writer EntityId. Test helper for
    /// L4 inbound inspection (which EID belongs to which topic).
    pub user_writers: BTreeMap<String, EntityId>,
    /// Topic name (DDS) → registered reader EntityId. Test helper for
    /// L3 outbound: allows synthetic sample injection via
    /// `DcpsRuntime::test_inject_user_alive` without going through the
    /// wire path (CI-stable without multicast discovery).
    pub user_readers: BTreeMap<String, EntityId>,
    /// The participants of the tenants (Spec §1 L6).
    pub tenants: Vec<TenantHandle>,
}

impl DaemonHandle {
    /// Whether SIGTERM/SIGINT (or [`Self::shutdown`]) asked the daemon to
    /// stop. The binary waits for it and then runs the shutdown.
    #[must_use]
    pub fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// Spec §9.2: stop taking DDS samples, wait for the broker's acks of
    /// what was published with QoS 1/2 (at most `mqtt.drain_timeout_secs`;
    /// a time beyond what the clock holds is without end), send DISCONNECT
    /// 0x00, then join the workers and stop the DDS participants.
    pub fn shutdown(&mut self) {
        let deadline = std::time::Instant::now().checked_add(self.drain_timeout);
        for core in &mut self.cores {
            core.drain(deadline);
        }
        self.stop.store(true, Ordering::SeqCst);
        self.healthy.store(false, Ordering::SeqCst);
        if let Some(admin) = self.admin_addr.as_deref() {
            if let Ok(addr) = admin.parse::<std::net::SocketAddr>() {
                let _ = std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(200));
            }
        }
        for core in &mut self.cores {
            core.stop_threads();
        }
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
        for core in &self.cores {
            core.runtime.shutdown();
        }
    }
}

impl Core {
    /// §9.2 for this core: the pumps stop, the acks are awaited until
    /// `deadline` (`None`: without end), then DISCONNECT 0x00.
    fn drain(&mut self, deadline: Option<std::time::Instant>) {
        let Some(client) = self.client.take() else {
            return;
        };
        self.pump_stop.store(true, Ordering::SeqCst);
        for j in self.pump_threads.drain(..) {
            let _ = j.join();
        }
        // §5.2: a stopped bridge does not stay in the catalog: the retained
        // message is cleared (a PUBLISH without payload, MQTT 5.0 §3.3.1.3).
        let catalog = self.catalog_topic.lock().ok().and_then(|mut t| t.take());
        if let Some(topic) = catalog {
            if let Ok(mut c) = client.lock() {
                clear_catalog(&mut c, &topic, &self.metrics, &self.name);
            }
        }
        let pending = || client.lock().map(|c| c.pending_acks()).unwrap_or(0);
        while pending() > 0
            && deadline.is_none_or(|d| std::time::Instant::now() < d)
            && self
                .inbound_thread
                .as_ref()
                .is_some_and(|j| !j.is_finished())
        {
            thread::sleep(Duration::from_millis(20));
        }
        let left = pending();
        if left > 0 {
            log::event(Level::Warn, "drain_incomplete")
                .tenant(&self.name)
                .number("unacknowledged", left as u64)
                .emit();
        }
        self.inbound_stop.store(true, Ordering::SeqCst);
        if let Some(j) = self.inbound_thread.take() {
            let _ = j.join();
        }
        if let Ok(mut c) = client.lock() {
            c.disconnect();
        }
        log::event(Level::Info, "disconnect")
            .tenant(&self.name)
            .field("connect_state", "disconnected")
            .reason_code(0x00)
            .field("reason", "shutdown")
            .emit();
    }

    fn stop_threads(&mut self) {
        self.pump_stop.store(true, Ordering::SeqCst);
        self.inbound_stop.store(true, Ordering::SeqCst);
        if let Some(j) = self.inbound_thread.take() {
            let _ = j.join();
        }
        for j in self.pump_threads.drain(..) {
            let _ = j.join();
        }
    }
}

impl Drop for DaemonHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// `tenant <name>: ` for a tenant's message, empty for the top level's.
fn tenant_prefix(name: &str) -> String {
    if name.is_empty() {
        String::new()
    } else {
        format!("tenant {name}: ")
    }
}

/// zerodds-lint: recursion-depth 64 (start bounded by AST depth)
/// Starts the daemon with the given config.
///
/// # Errors
/// `Config` (Spec exit code 1), `BrokerConnect` (2), `Dds` (3), `Tls` (4),
/// `Auth` (5), `Stopped` (a stop while connecting).
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

/// A security configuration error: unreadable TLS material is a TLS error
/// (exit 4), anything else a configuration error (exit 1).
fn security_error(e: &SecurityError) -> ServerError {
    match e {
        SecurityError::Tls(_) => ServerError::Tls(format!("security: {e}")),
        _ => ServerError::Config(format!("security: {e}")),
    }
}

/// The credentials of a connection as they apply now (Spec §7.2, §9.2): a
/// SIGHUP reload replaces them; the next connection uses them, and a
/// standing connection under enhanced authentication re-authenticates with
/// them (MQTT 5.0 §4.12.1).
struct LiveAuth {
    now: std::sync::RwLock<DaemonConfig>,
    generation: std::sync::atomic::AtomicU64,
}

/// Copies the credentials of `from` into `to`: the keys of `auth` and
/// `mqtt.username`/`mqtt.password`.
fn copy_credentials(from: &DaemonConfig, to: &mut DaemonConfig) {
    to.username.clone_from(&from.username);
    to.password.clone_from(&from.password);
    to.auth_mode = from.auth_mode;
    to.auth_bearer_token.clone_from(&from.auth_bearer_token);
    to.auth_bearer_subject.clone_from(&from.auth_bearer_subject);
    to.outbound_username.clone_from(&from.outbound_username);
    to.outbound_password.clone_from(&from.outbound_password);
    to.auth_enhanced_method
        .clone_from(&from.auth_enhanced_method);
}

/// Whether `a` and `b` present the same credentials to the broker: the
/// keys [`copy_credentials`] copies, without `auth.bearer_subject`, which
/// only names the ACL subject (Spec §7.3).
fn same_credentials(a: &DaemonConfig, b: &DaemonConfig) -> bool {
    let presented = |c: &DaemonConfig| {
        (
            c.username.clone(),
            c.password.clone(),
            c.auth_mode,
            c.auth_bearer_token.clone(),
            c.outbound_username.clone(),
            c.outbound_password.clone(),
            c.auth_enhanced_method.clone(),
        )
    };
    presented(a) == presented(b)
}

/// Whether `a` and `b` authenticate the same way: one mode, one enhanced
/// method. Only then does a reload take the credentials of `b`.
fn same_auth_way(a: &DaemonConfig, b: &DaemonConfig) -> bool {
    let (x, y) = (Credentials::of(a), Credentials::of(b));
    x.kind() == y.kind() && x.method() == y.method()
}

impl LiveAuth {
    fn new(cfg: &DaemonConfig) -> Self {
        Self {
            now: std::sync::RwLock::new(cfg.clone()),
            generation: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// The credentials in force into `cfg`.
    fn apply_to(&self, cfg: &mut DaemonConfig) {
        match self.now.read() {
            Ok(now) => copy_credentials(&now, cfg),
            Err(p) => copy_credentials(&p.into_inner(), cfg),
        }
    }

    /// Takes the credentials of `cfg`. Only credentials that differ from
    /// those in force start a new generation, the one a connection with
    /// enhanced authentication re-authenticates for (Spec §7.2, §9.2).
    fn replace(&self, cfg: &DaemonConfig) {
        let mut now = match self.now.write() {
            Ok(now) => now,
            Err(p) => p.into_inner(),
        };
        let changed = !same_credentials(&now, cfg);
        copy_credentials(cfg, &mut now);
        if changed {
            self.generation.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }
}

/// [`start_with_runtime`], reloading on SIGHUP with `load` (§9.2): the ACL
/// applies at once (a withdrawn right stops the topic's samples in both
/// directions); the broker TLS material, server name and ALPN offer apply
/// to the next connection (§7.1, §9.3); a granted read right, changed
/// topics, broker, `mqtt.tls.enabled`, auth or domain need a restart and
/// are logged. A configuration that does not load changes nothing.
///
/// # Errors
/// As [`start`].
#[allow(clippy::too_many_lines)]
pub fn start_with_reload(
    cfg: DaemonConfig,
    runtime_config: RuntimeConfig,
    load: Option<ConfigLoader>,
) -> Result<DaemonHandle, ServerError> {
    // §8.1: the threshold of `log_level` from the first line on.
    log::set_level(&cfg.log_level);
    log::event(Level::Info, "start")
        .integer("domain", i64::from(cfg.domain))
        .field("broker", &cfg.broker_url)
        .number("topics", cfg.topics.len() as u64)
        .number("tenants", cfg.tenants.len() as u64)
        .emit();

    // 0. Metrics registry and the families of §8.2.
    let registry = Arc::new(Registry::new());
    let metrics = BridgeMetrics::register(&registry);
    let healthy = Arc::new(AtomicBool::new(true));
    let reload_flag = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));

    // 0a. Signal watcher (§9.1 step 7) before the connect, so a SIGTERM
    //     while the connect is retried stops the daemon through its own
    //     path.
    if let Err(e) = install_signal_watcher(Arc::clone(&stop), Arc::clone(&reload_flag)) {
        log::event(Level::Error, "signal_watcher_failed")
            .field("reason", &e.to_string())
            .emit();
    }

    // 0b. The TLS client configuration (§7.1); the auth settings of each
    //     connection (§7.2).
    let tls_client_cfg = client_tls_from_config(&cfg).map_err(|e| security_error(&e))?;
    for (owner, c) in std::iter::once((String::new(), cfg.clone())).chain(
        cfg.tenants
            .iter()
            .map(|t| (t.name.clone(), cfg.of_tenant(t))),
    ) {
        super::auth::check(&c)
            .map_err(|m| ServerError::Config(format!("{}{m}", tenant_prefix(&owner))))?;
    }
    log::event(Level::Info, "auth")
        .field("mode", Credentials::of(&cfg).kind().as_str())
        .number("acl_rules", cfg.acl_rules.len() as u64)
        .flag("broker_tls", cfg.broker_tls_enabled)
        .emit();

    // Spec §5.1: a topic the daemon publishes on has a topic name, one it
    // subscribes to a topic filter (a configuration built in code does not
    // pass the parser's check).
    for (owner, topics) in std::iter::once(("", &cfg.topics))
        .chain(cfg.tenants.iter().map(|t| (t.name.as_str(), &t.topics)))
    {
        for t in topics {
            check_mqtt_topic(t).map_err(|m| {
                ServerError::Config(format!("{}topic {}: {m}", tenant_prefix(owner), t.dds_name))
            })?;
        }
    }

    // 1. Broker-URL parsen.
    let broker =
        parse_broker_url(&cfg.broker_url).map_err(|e| ServerError::Config(format!("{e}")))?;
    if broker.tls && tls_client_cfg.is_none() {
        // mqtts:// or wss:// without tls setup → strict reject (spec §7.1).
        return Err(ServerError::Config(
            "mqtts:// and wss:// require mqtt.tls.enabled=true and ca_file (Spec §7.1)".to_string(),
        ));
    }
    if !broker.tls && tls_client_cfg.is_some() {
        // The URL says plain, the TLS section says encrypted: which one the
        // operator meant is not ours to guess.
        return Err(ServerError::Config(format!(
            "mqtt.tls.enabled=true with the plain URL {}; use mqtts:// or wss://",
            cfg.broker_url
        )));
    }

    // 1a. OTLP exporter (§8.3): with `OTEL_EXPORTER_OTLP_ENDPOINT` the
    //     spans of the PUBLISH packets of every core go to it.
    let exporter = otlp_config_from_env(SERVICE_NAME).map(|otlp_cfg| {
        log::event(Level::Info, "otlp_active")
            .field("endpoint", &format!("{}:{}", otlp_cfg.host, otlp_cfg.port))
            .emit();
        Arc::new(OtlpExporter::new(otlp_cfg))
    });
    let tracer = Tracer::new(exporter.clone());

    // 2.-7. The top level, then one core per tenant (§1 L6).
    let live_tls = Arc::new(LiveTls::new(&cfg, tls_client_cfg.clone()));
    let link = Link {
        broker: &broker,
        tls: &live_tls,
        metrics: &metrics,
        stop: &stop,
        health: Health {
            healthy: Arc::clone(&healthy),
            down: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        },
        tracer: &tracer,
    };
    let mut cores = vec![start_core("", &cfg, runtime_config.clone(), &link)?];
    for t in &cfg.tenants {
        let tenant_cfg = cfg.of_tenant(t);
        let started = start_core(&t.name, &tenant_cfg, runtime_config.clone(), &link);
        match started {
            Ok(core) => cores.push(core),
            Err(e) => {
                // The cores already started end with the start.
                let deadline = Some(std::time::Instant::now());
                for core in &mut cores {
                    core.drain(deadline);
                    core.stop_threads();
                    core.runtime.shutdown();
                }
                return Err(e);
            }
        }
    }

    // 8. Admin-Endpoint (§5.2 Catalog/Healthz + §8.2 Metrics).
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
                    &cfg.metrics_path,
                ) {
                    Ok((h, bound)) => {
                        log::event(Level::Info, "admin_bound")
                            .field("listen", &bound.to_string())
                            .field("metrics_path", &cfg.metrics_path)
                            .emit();
                        admin_addr = Some(bound.to_string());
                        admin_thread = Some(h);
                    }
                    Err(e) => log::event(Level::Error, "admin_bind_failed")
                        .field("listen", &bind_str)
                        .field("reason", &e.to_string())
                        .emit(),
                }
            }
            Err(e) => log::event(Level::Error, "admin_bind_failed")
                .field("listen", &bind_str)
                .field("reason", &e.to_string())
                .emit(),
        }
    }

    // 9. SIGHUP reload (§9.2).
    if let Some(load) = load {
        let lives: Vec<Live> = cores
            .iter()
            .map(|c| Live {
                name: c.name.clone(),
                acl: Arc::clone(&c.acl),
                auth: Arc::clone(&c.auth),
            })
            .collect();
        let (stop_r, flag_r, started) = (Arc::clone(&stop), Arc::clone(&reload_flag), cfg.clone());
        let tls_r = Arc::clone(&live_tls);
        let _ = thread::Builder::new()
            .name("zerodds-mqtt-reload".into())
            .spawn(move || {
                while !stop_r.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(100));
                    if flag_r.swap(false, Ordering::SeqCst) {
                        reload(&lives, &tls_r, &started, load());
                    }
                }
            });
    }

    // 10. The flush of the OTLP exporter (§8.3): every 5 s and once more at
    //     the end of the shutdown.
    let otlp_thread = if let Some(exp) = exporter {
        match spawn_otlp_flush_loop(exp, Arc::clone(&stop), Duration::from_secs(5)) {
            Ok(h) => Some(h),
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

    let top = &cores[0];
    let (runtime, user_writers, user_readers) = (
        Arc::clone(&top.runtime),
        top.user_writers.clone(),
        top.user_readers.clone(),
    );
    let tenants = cores[1..]
        .iter()
        .map(|c| TenantHandle {
            name: c.name.clone(),
            runtime: Arc::clone(&c.runtime),
            user_writers: c.user_writers.clone(),
            user_readers: c.user_readers.clone(),
        })
        .collect();
    Ok(DaemonHandle {
        stop,
        cores,
        drain_timeout: Duration::from_secs(cfg.drain_timeout_secs),
        admin_thread,
        otlp_thread,
        admin_addr,
        reload_flag,
        healthy,
        metrics: Some(metrics),
        runtime,
        user_writers,
        user_readers,
        tenants,
    })
}

/// What every core shares: the broker, its TLS configuration, the metrics,
/// the stop flag, the health of the connections.
struct Link<'a> {
    broker: &'a BrokerUrl,
    tls: &'a Arc<LiveTls>,
    metrics: &'a BridgeMetrics,
    stop: &'a Arc<AtomicBool>,
    health: Health,
    /// The spans of §8.3.
    tracer: &'a Tracer,
}

/// `/healthz` (§9.3): healthy while no broker connection of a core is lost.
#[derive(Clone)]
struct Health {
    healthy: Arc<AtomicBool>,
    down: Arc<std::sync::atomic::AtomicUsize>,
}

impl Health {
    /// A connection is lost.
    fn lost(&self) {
        self.down.fetch_add(1, Ordering::SeqCst);
        self.healthy.store(false, Ordering::SeqCst);
    }

    /// A lost connection stands again.
    fn back(&self) {
        if self.down.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.healthy.store(true, Ordering::SeqCst);
        }
    }
}

/// The TLS client settings of the broker connections as they apply now:
/// the client configuration (CA, certificate, key), the server name, the
/// ALPN offer. A SIGHUP reload replaces them (§7.1); each connection that
/// begins after it uses them, a connection that stands keeps its own.
pub struct LiveTls {
    now: std::sync::RwLock<TlsSettings>,
}

/// One state of [`LiveTls`].
#[derive(Clone)]
struct TlsSettings {
    client: Option<Arc<rustls::ClientConfig>>,
    server_name: String,
    alpn: Option<Vec<String>>,
}

impl LiveTls {
    fn new(cfg: &DaemonConfig, client: Option<Arc<rustls::ClientConfig>>) -> Self {
        Self {
            now: std::sync::RwLock::new(TlsSettings {
                client,
                server_name: cfg.broker_tls_server_name.clone(),
                alpn: cfg.broker_tls_alpn.clone(),
            }),
        }
    }

    fn current(&self) -> Option<TlsSettings> {
        self.now.read().ok().map(|t| t.clone())
    }

    fn replace(&self, cfg: &DaemonConfig, client: Option<Arc<rustls::ClientConfig>>) {
        if let Ok(mut t) = self.now.write() {
            *t = TlsSettings {
                client,
                server_name: cfg.broker_tls_server_name.clone(),
                alpn: cfg.broker_tls_alpn.clone(),
            };
        }
    }
}

/// What a connection of a core needs, at the start and at each reconnect
/// (§9.1, §9.3).
struct Session {
    cfg: DaemonConfig,
    broker: BrokerUrl,
    tls: Arc<LiveTls>,
    auth: Arc<LiveAuth>,
    acl: Arc<LiveAcl>,
    metrics: BridgeMetrics,
    /// The tenant's name (Spec §1 L6), empty for the top level: the
    /// `tenant` of its log lines (§8.1).
    tenant: String,
    replay: Arc<Mutex<ReplayBuffer>>,
    catalog_topic: Arc<Mutex<Option<String>>>,
    /// The spans of §8.3; each connection sends its PUBLISH spans to it.
    tracer: Tracer,
}

impl Session {
    /// The configuration of the next connection: the TLS settings and the
    /// credentials in force (§7.1, §7.2), and the TLS client configuration.
    fn settings(&self) -> Result<(DaemonConfig, Option<Arc<rustls::ClientConfig>>), ClientError> {
        let tls = self
            .tls
            .current()
            .ok_or_else(|| ClientError::Tls("the TLS settings are poisoned".into()))?;
        let mut cfg = self.cfg.clone();
        cfg.broker_tls_server_name = tls.server_name;
        cfg.broker_tls_alpn = tls.alpn;
        self.auth.apply_to(&mut cfg);
        Ok((cfg, tls.client))
    }

    /// §9.3: one attempt to connect with the settings in force; the
    /// reconnect loop waits between the attempts.
    fn attempt(&self) -> Result<MqttClient, ClientError> {
        self.metrics.connect_attempts.inc();
        let (cfg, tls) = self.settings()?;
        let mut client = MqttClient::connect_url(&self.broker, &cfg, tls)?;
        self.metrics.connect_successes.inc();
        client.set_tracer(self.tracer.clone());
        Ok(client)
    }

    /// §9.1 step 6: the connection of the start, with the waits of
    /// `mqtt.reconnect` while the broker cannot be reached, until
    /// `mqtt.connect_timeout_secs` (see [`connect_with_backoff`]).
    fn connect_at_start(&self, stop: &AtomicBool) -> Result<Option<MqttClient>, ClientError> {
        let (cfg, tls) = self.settings()?;
        let out = connect_with_backoff(&self.broker, &cfg, tls, stop, |e, wait| {
            self.metrics.connect_attempts.inc();
            self.metrics.errors_total.inc();
            log::event(Level::Warn, "connect_failed")
                .tenant(&self.tenant)
                .field("connect_state", "connecting")
                .field("reason", &e.to_string())
                .number("next_attempt_ms", millis(wait))
                .emit();
        });
        // §8.2: the attempt that ended the backoff.
        match &out {
            Ok(Some(_)) => {
                self.metrics.connect_attempts.inc();
                self.metrics.connect_successes.inc();
            }
            Err(_) => self.metrics.connect_attempts.inc(),
            Ok(None) => {}
        }
        out
    }

    /// After each CONNACK: the log line, the topics the broker's limits
    /// forbid, the subscriptions (§4.3), the catalog (§5.2), the replay
    /// buffer (§6).
    fn after_connack(&self, client: &mut MqttClient) -> Result<Connected, ClientError> {
        let (cfg, tenant, metrics) = (&self.cfg, self.tenant.as_str(), &self.metrics);
        let limits = client.broker_limits();
        log::event(Level::Info, "connect")
            .tenant(tenant)
            .field("connect_state", "connected")
            .field(
                "client_id",
                client.assigned_client_id().unwrap_or(&cfg.client_id),
            )
            .field(
                "protocol",
                match client.version() {
                    ProtocolVersion::V5 => "5.0",
                    ProtocolVersion::V311 => "3.1.1",
                },
            )
            .number("keep_alive_secs", client.keep_alive().as_secs())
            .flag("session_present", client.session_present())
            .emit();
        // What the broker's CONNACK forbids for a topic is said once here;
        // each such PUBLISH is refused and counted (`ClientError::NotSent`).
        for topic in cfg
            .topics
            .iter()
            .filter(|t| matches!(t.direction.as_str(), "out" | "bidir"))
        {
            if topic.effective_mqtt_qos() > limits.maximum_qos {
                log::event(Level::Warn, "topic_not_published")
                    .tenant(tenant)
                    .field("dds_topic", &topic.dds_name)
                    .field("mqtt_topic", &topic.mqtt_topic)
                    .number("mqtt_qos", u64::from(topic.effective_mqtt_qos()))
                    .field(
                        "reason",
                        &format!("above the broker's Maximum QoS {}", limits.maximum_qos),
                    )
                    .emit();
            }
            if topic.effective_retain() && !limits.retain_available {
                log::event(Level::Warn, "topic_not_published")
                    .tenant(tenant)
                    .field("dds_topic", &topic.dds_name)
                    .field("mqtt_topic", &topic.mqtt_topic)
                    .number("mqtt_qos", u64::from(topic.effective_mqtt_qos()))
                    .field("reason", "retained, and the broker has no Retain Available")
                    .emit();
            }
        }
        // §4.3, §7.3: a topic whose filter `allow_subscribe` does not grant
        // the bridge's subject is not subscribed.
        let live = self.acl.current();
        let (acl, subject) = (&live.0, &live.1);
        let routes = InboundRoutes::of_topics(&cfg.topics, |topic| {
            let allowed = acl.may_subscribe(subject, &topic.mqtt_topic);
            if !allowed {
                log::event(Level::Warn, "acl_denied")
                    .tenant(tenant)
                    .field("action", "subscribe")
                    .field("mqtt_topic", &topic.mqtt_topic)
                    .field("dds_topic", &topic.dds_name)
                    .field("subject", subject.display())
                    .emit();
                metrics.acl_denial(acl_reason::SUBSCRIBE);
            }
            allowed
        });
        let routes = subscribe_routes(client, routes, metrics, tenant)?;
        // §5.2: the retained catalog of this connection.
        let catalog = CatalogSnapshot {
            topics: cfg.topics.clone(),
        };
        let id = client
            .assigned_client_id()
            .unwrap_or(&cfg.client_id)
            .to_string();
        let published = publish_catalog(
            client,
            &cfg.own_topic_prefix,
            id.clone(),
            &catalog,
            metrics,
            tenant,
        );
        self.track_catalog(client, published);
        self.replay_to(client);
        Ok(Connected {
            routes,
            client_id: id,
        })
    }

    /// §5.2: the catalog topic of this connection is the one cleared at the
    /// shutdown. The catalog of an earlier connection under another client
    /// identifier (one the broker assigned anew at the reconnect) is
    /// cleared now, so a bridge is listed once. When none went out now, the
    /// earlier one stays the one to clear.
    fn track_catalog(&self, client: &mut MqttClient, published: Option<String>) {
        let Ok(mut t) = self.catalog_topic.lock() else {
            return;
        };
        let before = t.take();
        match (before, published) {
            (Some(old), Some(new)) => {
                if old != new {
                    clear_catalog(client, &old, &self.metrics, &self.tenant);
                }
                *t = Some(new);
            }
            (old, new) => *t = new.or(old),
        }
    }

    /// §6: the replay buffer of the transient and persistent topics,
    /// oldest first, with the QoS and the retain flag of its topic; the
    /// Message Expiry Interval is what is left when each goes out. Each
    /// PUBLISH is a roundtrip of its DDS topic (§8.3), from the replay to
    /// its acknowledgement.
    fn replay_to(&self, client: &mut MqttClient) {
        let samples = match self.replay.lock() {
            Ok(r) => r.replay(),
            Err(_) => return,
        };
        if samples.is_empty() {
            return;
        }
        let (mut sent, mut expired) = (0usize, 0usize);
        let live = self.acl.current();
        for (dds_name, message, props) in &samples {
            let Some(topic) = self.cfg.topics.iter().find(|t| {
                t.dds_name == *dds_name && matches!(t.direction.as_str(), "out" | "bidir")
            }) else {
                continue;
            };
            // §7.3: a right withdrawn since the sample went out holds.
            if !live.0.may_publish(&live.1, &topic.mqtt_topic) {
                self.metrics.acl_denial(acl_reason::PUBLISH);
                continue;
            }
            let expires = props.message_expiry_interval.and_then(|secs| {
                std::time::Instant::now().checked_add(Duration::from_secs(u64::from(secs)))
            });
            let start_ns = super::trace::unix_ns();
            match client.publish_until(
                &topic.mqtt_topic,
                message,
                topic.effective_mqtt_qos(),
                topic.effective_retain(),
                props,
                expires,
            ) {
                Ok(_) => {
                    // §8.3: the span of the roundtrip, from the replay.
                    if client.tracing() {
                        client.trace_publish(super::trace::PublishOut {
                            start_ns,
                            mqtt_topic: topic.mqtt_topic.clone(),
                            dds_topic: topic.dds_name.clone(),
                            qos: topic.effective_mqtt_qos(),
                            bytes: message.len(),
                            packet_id: None,
                        });
                    }
                    sent += 1;
                    self.metrics.publish_out(
                        &topic.dds_name,
                        topic.effective_mqtt_qos(),
                        message.len(),
                    );
                }
                // §6: a kept sample whose interval passed is not published
                // again.
                Err(ClientError::Expired(_)) => expired += 1,
                Err(e) => {
                    self.metrics.errors_total.inc();
                    log::event(Level::Warn, "replay_failed")
                        .tenant(&self.tenant)
                        .field("dds_topic", dds_name)
                        .field("mqtt_topic", &topic.mqtt_topic)
                        .field("reason", &e.to_string())
                        .emit();
                }
            }
        }
        log::event(Level::Info, "replay")
            .tenant(&self.tenant)
            .number("sent", sent as u64)
            .number("kept", samples.len() as u64)
            .number("expired", expired as u64)
            .emit();
    }
}

/// What a CONNACK gave a core (§9.1, §9.3): the routes of its
/// subscriptions and the client identifier of the connection, the one the
/// broker assigned when it assigned one.
struct Connected {
    routes: InboundRoutes,
    client_id: String,
}

/// §5.2: clears the retained catalog on `topic` with a retained PUBLISH
/// without payload (MQTT 5.0 §3.3.1.3); a PUBLISH that fails is logged and
/// counted as an error.
fn clear_catalog(client: &mut MqttClient, topic: &str, metrics: &BridgeMetrics, tenant: &str) {
    let qos = client.broker_limits().maximum_qos.min(1);
    match client.publish(topic, &[], qos, true, &Default::default()) {
        Ok(()) => {
            metrics.publish_out("", qos, 0);
            log::event(Level::Info, "catalog_cleared")
                .tenant(tenant)
                .field("mqtt_topic", topic)
                .emit();
        }
        Err(e) => {
            metrics.errors_total.inc();
            log::event(Level::Warn, "catalog_failed")
                .tenant(tenant)
                .field("mqtt_topic", topic)
                .field("reason", &format!("not cleared: {e}"))
                .emit();
        }
    }
}

/// A wait in whole milliseconds, for a log field.
fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// A handle on the socket of `client` for the inbound loop's wait, with
/// the read timeout of 200 ms.
fn wait_socket_of(client: &MqttClient) -> Option<std::net::TcpStream> {
    let sock = client.socket_clone().ok()?;
    let _ = sock.set_read_timeout(Some(Duration::from_millis(200)));
    Some(sock)
}

/// Reason Code 0x8E Session taken over (MQTT 5.0 §3.14.2.1): another
/// connection under the same client identifier took the session.
const SESSION_TAKEN_OVER: u8 = 0x8E;

/// §9.3: the connection of `session` is lost. The pumps hold their samples
/// (`connected` false), `/healthz` answers 503, the client lets go of its
/// Session State; then the daemon connects again: the first attempt at
/// once, each next one after the waits of `mqtt.reconnect`
/// (`initial_delay_ms`, then `factor` times the last one up to
/// `max_delay_ms`), until it stands or `stop` is set. A broker that cannot
/// be reached, a refusal of the TLS handshake or of the credentials and a
/// failed subscription are all tried again after the next wait. A
/// connection the broker ended with 0x8E Session taken over (`ended_by`) is
/// tried again only after `max_delay_ms`: at once, two bridges under one
/// client identifier would take the session from each other in a loop.
/// What the new CONNACK gave, or `None` on the stop.
fn reconnect(
    session: &Session,
    client: &Arc<Mutex<MqttClient>>,
    stop: &AtomicBool,
    connected: &AtomicBool,
    health: &Health,
    ended_by: Option<u8>,
) -> Option<Connected> {
    connected.store(false, Ordering::SeqCst);
    health.lost();
    if let Ok(mut c) = client.lock() {
        c.release();
    }
    let mut pause = if ended_by == Some(SESSION_TAKEN_OVER) {
        Duration::from_millis(session.cfg.reconnect.max_delay_ms)
    } else {
        Duration::ZERO
    };
    let mut wait = session.cfg.reconnect.first_delay();
    loop {
        let until = std::time::Instant::now() + pause;
        while std::time::Instant::now() < until {
            if stop.load(Ordering::SeqCst) {
                return None;
            }
            thread::sleep(Duration::from_millis(20));
        }
        if stop.load(Ordering::SeqCst) {
            return None;
        }
        pause = wait;
        wait = session.cfg.reconnect.next_delay(wait);
        match session.attempt() {
            Ok(mut c) => match session.after_connack(&mut c) {
                Ok(done) => {
                    match client.lock() {
                        Ok(mut old) => *old = c,
                        Err(_) => return None,
                    }
                    connected.store(true, Ordering::SeqCst);
                    health.back();
                    return Some(done);
                }
                Err(e) => {
                    session.metrics.errors_total.inc();
                    log::event(Level::Warn, "connect_failed")
                        .tenant(&session.tenant)
                        .field("connect_state", "reconnecting")
                        .field("reason", &e.to_string())
                        .number("next_attempt_ms", millis(pause))
                        .emit();
                    c.release();
                }
            },
            Err(e) => {
                session.metrics.errors_total.inc();
                log::event(Level::Warn, "connect_failed")
                    .tenant(&session.tenant)
                    .field("connect_state", "reconnecting")
                    .field("reason", &e.to_string())
                    .number("next_attempt_ms", millis(pause))
                    .emit();
            }
        }
    }
}

/// One reader of a core: its topic and the channel of its samples.
type ReaderTopic = (TopicConfig, std::sync::mpsc::Receiver<UserSample>);

/// Starts the participant, the endpoints, the broker connection, the
/// subscriptions and the threads of the top level (`name` empty) or of a
/// tenant with its configuration `cfg`.
#[allow(clippy::too_many_lines)]
fn start_core(
    name: &str,
    cfg: &DaemonConfig,
    runtime_config: RuntimeConfig,
    link: &Link<'_>,
) -> Result<Core, ServerError> {
    let metrics = &link.metrics.for_tenant(name);
    // §7.3: the ACL and the subject of this connection; the subject reads
    // the client certificate, which the TLS configuration loaded before.
    let subject = subject_of(cfg).map_err(ServerError::Tls)?;
    let acl = Arc::new(LiveAcl::new(DaemonAcl::of(cfg), subject));
    let auth = Arc::new(LiveAuth::new(cfg));

    // 2. DCPS-Runtime: a participant of its own per client identifier.
    let prefix = zerodds_dcps::participant::bridge_guid_prefix(&cfg.client_id);
    let runtime = DcpsRuntime::start(cfg.domain, prefix, runtime_config)
        .map_err(|e| ServerError::Dds(format!("{e:?}")))?;
    let fail = |e: ServerError| {
        runtime.shutdown();
        e
    };

    // 3. Pro Topic Reader+Writer.
    let mut writers: BTreeMap<String, EntityId> = BTreeMap::new();
    let mut reader_eids: BTreeMap<String, EntityId> = BTreeMap::new();
    let mut readers: Vec<ReaderTopic> = Vec::new();
    for topic in &cfg.topics {
        register_topic(
            &runtime,
            topic,
            &mut writers,
            &mut reader_eids,
            &mut readers,
        )
        .map_err(fail)?;
        metrics.register_topic(topic);
    }

    // §6: the replay buffer of the transient and persistent topics; a
    // persistent one comes back from `mqtt.session_dir`.
    let replay = ReplayBuffer::open(cfg)
        .map_err(|e| fail(ServerError::Config(format!("replay buffer: {e}"))))?;
    let session = Arc::new(Session {
        cfg: cfg.clone(),
        broker: link.broker.clone(),
        tls: Arc::clone(link.tls),
        auth: Arc::clone(&auth),
        acl: Arc::clone(&acl),
        metrics: metrics.clone(),
        tenant: name.to_string(),
        replay: Arc::new(Mutex::new(replay)),
        catalog_topic: Arc::new(Mutex::new(None)),
        tracer: link.tracer.clone(),
    });

    // 4. Connect the MQTT client (§9.1 step 5): with the backoff of
    //    `mqtt.reconnect` while the broker cannot be reached, at most
    //    `mqtt.connect_timeout_secs`; a refusal of the TLS handshake or of
    //    the credentials ends the start at once.
    let mut client = match session.connect_at_start(link.stop) {
        Ok(Some(mut c)) => {
            c.set_tracer(session.tracer.clone());
            c
        }
        Ok(None) => return Err(fail(ServerError::Stopped)),
        Err(e) => {
            metrics.errors_total.inc();
            return Err(fail(map_client_err(e)));
        }
    };
    // 5. SUBSCRIBE (§4.3), the catalog (§5.2), the replay (§6).
    let Connected {
        routes,
        client_id: catalog_id,
    } = match session.after_connack(&mut client) {
        Ok(r) => r,
        Err(e) => {
            client.disconnect();
            return Err(fail(map_client_err(e)));
        }
    };
    let echo = Arc::new(super::pump::EchoGuard::default());
    let version = client.version();

    let pump_stop = Arc::new(AtomicBool::new(false));
    let inbound_stop = Arc::new(AtomicBool::new(false));
    // §9.3: false while the connection is lost; the pumps hold their
    // samples by the history of their topic.
    let connected = Arc::new(AtomicBool::new(true));

    // We share the client over a mutex between the inbound
    // (next_event) and outbound pump (publish). That is sync enough
    // because our frames are small and the TCP stream pushes atomic writes
    // per frame with `write_all`.
    let client = Arc::new(Mutex::new(client));

    // 6. Outbound pump per reader topic, through the topic's history (§6).
    let mut pump_threads = Vec::new();
    for (topic, rx) in readers {
        // §4.3: the samples of the bridge's own writer of the topic came from
        // the broker; they do not go back to it.
        let own_writer = writers
            .get(&topic.dds_name)
            .map(|eid| zerodds_rtps::wire_types::Guid::new(runtime.guid_prefix, *eid).to_bytes());
        // §4.3: MQTT 3.1.1 has no No Local; the broker's copy of the bridge's
        // own PUBLISH on a bidir topic is recognised by the echo guard.
        let echo = (version == ProtocolVersion::V311 && topic.direction == "bidir")
            .then(|| Arc::clone(&echo));
        let pump = super::pump::Pump {
            own_writer,
            echo,
            version,
            topic,
            rx,
            client: Arc::clone(&client),
            metrics: metrics.clone(),
            acl: Arc::clone(&acl),
            tenant: name.to_string(),
            stop: Arc::clone(&pump_stop),
            connected: Arc::clone(&connected),
            replay: Arc::clone(&session.replay),
            lifecycle_logged: std::cell::Cell::new(false),
            expired_logged: std::cell::Cell::new(false),
            refused_logged: std::cell::Cell::new(false),
        };
        pump_threads.push(thread::spawn(move || pump.run()));
    }

    // 7. Inbound-Loop (MQTT → DDS), the Keep Alive (§1 L1), the statuses
    //    of §6, the reconnect of §9.3.
    let stop_inbound = Arc::clone(&inbound_stop);
    let client_in = Arc::clone(&client);
    let runtime_in = Arc::clone(&runtime);
    let writers_in = writers.clone();
    // §3: the data representation of the writers whose topic sets one.
    let representation_in: BTreeMap<String, i16> = cfg
        .topics
        .iter()
        .filter_map(|t| {
            t.data_representation_id()
                .map(|id| (t.dds_name.clone(), id))
        })
        .collect();
    let metrics_in = metrics.clone();
    let echo_in = Arc::clone(&echo);
    let mut unrouted: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let session_in = Arc::clone(&session);
    let connected_in = Arc::clone(&connected);
    let health_in = link.health.clone();
    let tenant_in = name.to_string();
    let mut routes = routes;
    // §7.3: the MQTT topics of a PUBLISH the ACL refused, logged once each.
    let mut refused: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    // §7.2: the credentials this connection authenticated with.
    let mut auth_seen = auth.generation();
    // The inbound loop waits for input on a duplicate of the socket, without
    // the client's lock, and takes the lock only once there is something to
    // read. It used to block in `next_event` under the lock for the whole
    // read timeout (5 s), and every publish of the pump threads waited for
    // it: a DDS sample took 4.5 s to reach the broker (measured,
    // `mqttbridge_9_2_a_reload_that_withdraws_the_read_right_stops_the_samples`).
    let mut wait_socket = client.lock().ok().and_then(|c| wait_socket_of(&c));
    // §6: the deadline and liveliness statuses of the topics, reported to
    // the broker.
    let mut watched: Vec<(String, Option<EntityId>, Option<EntityId>)> = Vec::new();
    for t in &cfg.topics {
        if !watched.iter().any(|(n, _, _)| *n == t.dds_name) {
            watched.push((
                t.dds_name.clone(),
                reader_eids.get(&t.dds_name).copied(),
                writers.get(&t.dds_name).copied(),
            ));
        }
    }
    let mut watch = StatusWatch::new(&runtime, &cfg.own_topic_prefix, &catalog_id, &watched);
    let mut last_watch = std::time::Instant::now();
    let inbound_thread = thread::spawn(move || {
        while !stop_inbound.load(Ordering::SeqCst) {
            if last_watch.elapsed() >= STATUS_INTERVAL {
                last_watch = std::time::Instant::now();
                for report in watch.changes(&runtime_in) {
                    let sent = match client_in.lock() {
                        Ok(mut c) => status_watch::send(&mut c, &report)
                            .map(|()| c.broker_limits().maximum_qos.min(1)),
                        Err(_) => break,
                    };
                    match sent {
                        Ok(qos) => metrics_in.publish_out("", qos, report.json.len()),
                        Err(e) => {
                            metrics_in.errors_total.inc();
                            log::event(Level::Warn, "status_report_failed")
                                .tenant(&tenant_in)
                                .field("mqtt_topic", &report.topic)
                                .field("reason", &e.to_string())
                                .emit();
                        }
                    }
                }
            }
            // §7.2, §9.2: credentials a reload replaced; a connection with
            // enhanced authentication re-authenticates with them (MQTT 5.0
            // §4.12.1), any other uses them from its next CONNECT.
            let generation = session_in.auth.generation();
            if generation != auth_seen {
                auth_seen = generation;
                let mut now = session_in.cfg.clone();
                session_in.auth.apply_to(&mut now);
                if let Ok(mut c) = client_in.lock() {
                    if c.auth_method().is_some() {
                        match c.reauthenticate(
                            &now,
                            &session_in.broker.host,
                            session_in.broker.port,
                        ) {
                            Ok(()) => log::event(Level::Info, "reauth_started")
                                .tenant(&tenant_in)
                                .emit(),
                            Err(e) => {
                                metrics_in.errors_total.inc();
                                log::event(Level::Warn, "reauth_failed")
                                    .tenant(&tenant_in)
                                    .field("reason", &e.to_string())
                                    .emit();
                            }
                        }
                    }
                }
            }
            // [MQTT-3.1.2-20]: PINGREQ when nothing was sent for the Keep
            // Alive; a missing PINGRESP ends the connection.
            let tick = match client_in.lock() {
                Ok(mut c) => {
                    metrics_in.set_inflight(c.pending_releases(), c.pending_acks());
                    c.keep_alive_tick()
                }
                Err(_) => break,
            };
            let event = match tick {
                Ok(Some(lost)) => Ok(Some(lost)),
                Err(e) => Err(e),
                Ok(None) => {
                    let buffered = client_in
                        .lock()
                        .map(|mut c| c.has_buffered_input())
                        .unwrap_or(false);
                    if !buffered {
                        if let Some(sock) = &wait_socket {
                            let mut one = [0u8; 1];
                            match sock.peek(&mut one) {
                                Ok(_) => {}
                                Err(e)
                                    if matches!(
                                        e.kind(),
                                        std::io::ErrorKind::WouldBlock
                                            | std::io::ErrorKind::TimedOut
                                    ) =>
                                {
                                    continue;
                                }
                                // A closed or failed socket: `next_event` reports it.
                                Err(_) => {}
                            }
                        }
                    }
                    let mut c = match client_in.lock() {
                        Ok(c) => c,
                        Err(_) => break,
                    };
                    c.next_event()
                }
            };
            // Mutex fairness: insert a short yield-sleep after each lock
            // release. Inbound + outbound pump share the
            // `client_in` mutex; without the yield the inbound loop
            // grabs the lock again immediately (especially on macOS, where the
            // OS mutex schedules unfairly), and the pump thread hangs
            // permanently on `client_c.lock()`. Verified via the
            // `mqttbridge_2_dds_publish_pumps_to_mqtt_broker` E2E test.
            thread::sleep(Duration::from_millis(1));
            // §8.1: `latency_us` of `publish_in` counts from here.
            let read_at = std::time::Instant::now();
            let lost = match event {
                Ok(Some(InboundEvent::Publish {
                    topic,
                    payload,
                    qos,
                    properties,
                })) => {
                    // §4.3: under MQTT 3.1.1 the broker sends the bridge's
                    // own PUBLISH on a bidir topic back; it is not written.
                    if echo_in.take(&topic, &payload) {
                        continue;
                    }
                    let targets = routes.route(&topic, &properties.subscription_identifiers);
                    for dds_topic in &targets {
                        metrics_in.publish_in(dds_topic, qos, payload.len());
                    }
                    // §8.3: the span of this PUBLISH, from its reading.
                    let tracing = session_in.tracer.enabled();
                    let span_in = |refused: Option<String>, writes: Vec<DdsWrite>| PublishIn {
                        start_ns: unix_ns_since(read_at),
                        end_ns: unix_ns(),
                        mqtt_topic: topic.clone(),
                        qos,
                        bytes: payload.len(),
                        writes,
                        refused,
                    };
                    if targets.is_empty() {
                        if tracing {
                            session_in.tracer.publish_in(&span_in(
                                Some("matches no subscribed topic".into()),
                                Vec::new(),
                            ));
                        }
                        metrics_in.publish_in("", qos, payload.len());
                        metrics_in.errors_total.inc();
                        if unrouted.insert(topic.clone()) {
                            log::event(Level::Warn, "publish_unrouted")
                                .tenant(&tenant_in)
                                .field("mqtt_topic", &topic)
                                .number("bytes", payload.len() as u64)
                                .number("mqtt_qos", u64::from(qos))
                                .field(
                                    "reason",
                                    &format!(
                                        "matches no subscribed topic (subscription identifiers {:?}); not written, further ones on it are counted",
                                        properties.subscription_identifiers
                                    ),
                                )
                                .emit();
                        }
                        continue;
                    }
                    // §7.3: what `allow_subscribe` does not grant (any more)
                    // is not written into DDS.
                    let live = session_in.acl.current();
                    if !live.0.may_receive(&live.1, &topic) {
                        metrics_in.acl_denial(acl_reason::SUBSCRIBE);
                        if tracing {
                            session_in.tracer.publish_in(&span_in(
                                Some("the ACL refuses it (§7.3)".into()),
                                Vec::new(),
                            ));
                        }
                        if refused.insert(topic.clone()) {
                            log::event(Level::Warn, "acl_denied")
                                .tenant(&tenant_in)
                                .field("action", "receive")
                                .field("mqtt_topic", &topic)
                                .field("subject", live.1.display())
                                .emit();
                        }
                        continue;
                    }
                    let mut writes: Vec<DdsWrite> = Vec::new();
                    for dds_topic in targets {
                        let Some(eid) = writers_in.get(dds_topic) else {
                            continue;
                        };
                        let write_start = if tracing { unix_ns() } else { 0 };
                        let result = write_inbound(
                            &runtime_in,
                            *eid,
                            representation_in.get(dds_topic).copied(),
                            &payload,
                            &properties,
                        );
                        // §8.3: a `dds.write` for each write; a register
                        // writes nothing.
                        if tracing && !matches!(result, Ok(false)) {
                            writes.push(DdsWrite {
                                dds_topic: dds_topic.to_string(),
                                start_ns: write_start,
                                end_ns: unix_ns(),
                                refused: result.as_ref().err().cloned(),
                            });
                        }
                        match result {
                            Ok(written) => {
                                if written {
                                    metrics_in.dds_published(dds_topic);
                                }
                                if log::enabled(Level::Debug) {
                                    log::event(Level::Debug, "publish_in")
                                        .tenant(&tenant_in)
                                        .field("mqtt_topic", &topic)
                                        .field("dds_topic", dds_topic)
                                        .number("bytes", payload.len() as u64)
                                        .number("mqtt_qos", u64::from(qos))
                                        .latency(read_at)
                                        .emit();
                                }
                            }
                            Err(e) => {
                                metrics_in.errors_total.inc();
                                log::event(Level::Warn, "publish_refused")
                                    .tenant(&tenant_in)
                                    .field("mqtt_topic", &topic)
                                    .field("dds_topic", dds_topic)
                                    .number("bytes", payload.len() as u64)
                                    .number("mqtt_qos", u64::from(qos))
                                    .field("reason", &e)
                                    .emit();
                            }
                        }
                    }
                    if tracing {
                        session_in.tracer.publish_in(&span_in(None, writes));
                    }
                    continue;
                }
                Ok(Some(InboundEvent::Disconnected(reason))) => reason,
                Ok(None) => continue,
                Err(e) => format!("inbound err: {e}"),
            };
            // §9.3: the connection is lost; the daemon connects again.
            metrics_in.errors_total.inc();
            let ended_by = client_in.lock().ok().and_then(|c| c.ended_by());
            metrics_in.broker_disconnect(ended_by);
            let line = log::event(Level::Warn, "disconnect")
                .tenant(&tenant_in)
                .field("connect_state", "disconnected")
                .field("reason", &lost);
            match ended_by {
                Some(code) => line.reason_code(code),
                None => line.field("reason_code", "network"),
            }
            .emit();
            match reconnect(
                &session_in,
                &client_in,
                &stop_inbound,
                &connected_in,
                &health_in,
                ended_by,
            ) {
                Some(done) => {
                    routes = done.routes;
                    // §6: the reports go under the identifier of this
                    // connection (one the broker assigned may be new).
                    watch.set_client_id(&done.client_id);
                    wait_socket = client_in.lock().ok().and_then(|c| wait_socket_of(&c));
                    unrouted.clear();
                    refused.clear();
                    auth_seen = session_in.auth.generation();
                }
                None => break,
            }
        }
    });

    Ok(Core {
        name: name.to_string(),
        runtime,
        client: Some(client),
        pump_stop,
        inbound_stop,
        inbound_thread: Some(inbound_thread),
        pump_threads,
        acl,
        auth,
        catalog_topic: Arc::clone(&session.catalog_topic),
        metrics: metrics.clone(),
        user_writers: writers,
        user_readers: reader_eids,
    })
}

/// Spec §5.2: the catalog of a connection as a retained PUBLISH on
/// `<own_topic_prefix>/<client_id>/catalog` (`mqtt.own_topic_prefix`; the
/// client identifier the broker assigned, when it assigned one), QoS 1 (the broker's Maximum QoS when
/// that is 0), Payload Format Indicator 1, Content Type `application/json`.
/// The topic when it went out; a PUBLISH the broker's CONNACK forbids or
/// that fails is logged and counted as an error.
fn publish_catalog(
    client: &mut MqttClient,
    own_prefix: &str,
    client_id: String,
    catalog: &CatalogSnapshot,
    metrics: &BridgeMetrics,
    tenant: &str,
) -> Option<String> {
    let Some(topic) = catalog_topic(own_prefix, &client_id) else {
        metrics.errors_total.inc();
        log::event(Level::Warn, "catalog_failed")
            .tenant(tenant)
            .field("client_id", &client_id)
            .field(
                "reason",
                "not published: the client identifier cannot be a topic level",
            )
            .emit();
        return None;
    };
    let json = catalog.render_json();
    let props = crate::property_set::PropertySet {
        payload_format_indicator: Some(1),
        content_type: Some("application/json".to_string()),
        ..Default::default()
    };
    let qos = client.broker_limits().maximum_qos.min(1);
    match client.publish(&topic, json.as_bytes(), qos, true, &props) {
        Ok(()) => {
            metrics.publish_out("", qos, json.len());
            log::event(Level::Info, "catalog_published")
                .tenant(tenant)
                .field("mqtt_topic", &topic)
                .number("topics", catalog.topics.len() as u64)
                .number("bytes", json.len() as u64)
                .number("mqtt_qos", u64::from(qos))
                .emit();
            Some(topic)
        }
        Err(e) => {
            metrics.errors_total.inc();
            log::event(Level::Warn, "catalog_failed")
                .tenant(tenant)
                .field("mqtt_topic", &topic)
                .field("reason", &format!("not published: {e}"))
                .emit();
            None
        }
    }
}

/// The subscriptions of `routes` on `client` (Spec §4.3), each refusal of
/// the broker logged and counted as an error, a QoS below the one asked for
/// logged; the routes with the granted ones marked.
fn subscribe_routes(
    client: &mut MqttClient,
    mut routes: InboundRoutes,
    metrics: &BridgeMetrics,
    tenant: &str,
) -> Result<InboundRoutes, ClientError> {
    let entries: Vec<SubscribeEntry> = routes
        .subscriptions()
        .iter()
        .map(|s| SubscribeEntry {
            filter: s.filter.clone(),
            qos: s.qos,
            id: s.id,
        })
        .collect();
    let outcomes = client.subscribe(&entries)?;
    let mut granted = 0usize;
    for (entry, outcome) in entries.iter().zip(outcomes) {
        match outcome {
            SubscribeOutcome::Granted(qos) => {
                routes.set_granted(&entry.filter, true);
                granted += 1;
                if qos < entry.qos {
                    log::event(Level::Warn, "subscription_downgraded")
                        .tenant(tenant)
                        .field("mqtt_topic", &entry.filter)
                        .number("mqtt_qos", u64::from(qos))
                        .number("requested_qos", u64::from(entry.qos))
                        .emit();
                }
            }
            SubscribeOutcome::Refused(reason) => {
                metrics.errors_total.inc();
                log::event(Level::Warn, "subscription_refused")
                    .tenant(tenant)
                    .field("mqtt_topic", &entry.filter)
                    .reason_code(reason)
                    .field("reason", "refused by the broker; its topic gets no message")
                    .emit();
            }
            SubscribeOutcome::NoWildcards => {
                metrics.errors_total.inc();
                log::event(Level::Warn, "subscription_refused")
                    .tenant(tenant)
                    .field("mqtt_topic", &entry.filter)
                    .field(
                        "reason",
                        "not sent: the broker has no Wildcard Subscription Available; its topic gets no message",
                    )
                    .emit();
            }
        }
    }
    log::event(Level::Info, "subscribed")
        .tenant(tenant)
        .number("granted", granted as u64)
        .number("filters", entries.len() as u64)
        .emit();
    Ok(routes)
}

/// A PUBLISH of the broker into the DDS writer `eid` (Spec §4.2, §4.4): a
/// sample is the encapsulation header and the CDR bytes, the header names
/// the representation and the byte order of the DDS write, the user
/// properties its key hash and source timestamp; `zerodds_op` dispose or
/// unregister is that change of the instance its key hash names; register
/// writes nothing. `Ok(true)` when something was written. `representation`
/// is the topic's `data_representation` (§3): the writer offers it since its
/// creation, and a sample whose header names the other XCDR version is
/// refused, since a DataWriter's representation does not change (DDS-XTypes
/// 1.3 §7.6.3.1); without it the writer takes the header's.
fn write_inbound(
    runtime: &DcpsRuntime,
    eid: EntityId,
    representation: Option<i16>,
    payload: &[u8],
    properties: &crate::property_set::PropertySet,
) -> Result<bool, String> {
    let sample = match inbound_message(payload, properties)? {
        InboundMessage::Sample(s) => s,
        InboundMessage::Register => return Ok(false),
        InboundMessage::Lifecycle {
            key_hash,
            key_holder,
            status_bits,
            source_timestamp,
        } => {
            return runtime
                .write_user_lifecycle(eid, key_hash, key_holder, status_bits, source_timestamp)
                .map(|()| true)
                .map_err(|e| format!("dds lifecycle write: {e:?}"));
        }
    };
    let identifier = sample.identifier;
    // The writer's header names what the PUBLISH's header named: version,
    // format and byte order; the version is the topic's when it sets one.
    let offer = offer_of(identifier);
    match representation {
        Some(fixed) if fixed != offer => {
            return Err(format!(
                "the encapsulation {:#06x} is {}, the topic's data_representation is {}",
                u16::from_be_bytes(identifier),
                xcdr_name(offer),
                xcdr_name(fixed)
            ));
        }
        Some(_) => {}
        None => {
            let _ = runtime.set_user_writer_data_rep_override(eid, Some(vec![offer]));
        }
    }
    let _ = runtime.set_user_writer_byte_order_override(eid, identifier[1] & 0x01 == 0);
    let _ = runtime.set_user_writer_wire_extensibility(eid, wire_extensibility_of(identifier));
    match (sample.key_hash, sample.source_timestamp) {
        (Some(key), ts) => runtime.write_user_sample_keyed(eid, sample.body, key, ts),
        (None, Some(ts)) => runtime.write_user_sample_stamped(eid, sample.body, ts),
        (None, None) => runtime.write_user_sample(eid, sample.body.to_vec()),
    }
    .map(|()| true)
    .map_err(|e| format!("dds write: {e:?}"))
}

/// `xcdr1` or `xcdr2`, the name of a DataRepresentationId in the
/// configuration (§3).
fn xcdr_name(id: i16) -> &'static str {
    if id == zerodds_rtps::publication_data::data_representation::XCDR {
        "xcdr1"
    } else {
        "xcdr2"
    }
}

/// How often the inbound loop looks at the deadline and liveliness statuses
/// of the topics (§6).
const STATUS_INTERVAL: Duration = Duration::from_millis(200);

/// Whether two tenant lists differ in more than their ACLs and
/// credentials, which a reload applies.
fn tenants_differ(a: &[super::config::TenantConfig], b: &[super::config::TenantConfig]) -> bool {
    a.len() != b.len()
        || a.iter().zip(b).any(|(x, y)| {
            (&x.name, x.domain, &x.client_id, &x.topics)
                != (&y.name, y.domain, &y.client_id, &y.topics)
        })
}

/// The settings that differ between `started` and `new` and need a restart
/// (§9.2): the connection, the participants and the subscriptions are made
/// at start.
fn settings_that_need_a_restart(started: &DaemonConfig, new: &DaemonConfig) -> Vec<&'static str> {
    let mut restart: Vec<&str> = Vec::new();
    if new.topics != started.topics {
        restart.push("topics");
    }
    if tenants_differ(&new.tenants, &started.tenants) {
        restart.push("tenants");
    }
    if new.broker_url != started.broker_url || new.client_id != started.client_id {
        restart.push("mqtt.broker_url/client_id");
    }
    if new.protocol_version != started.protocol_version
        || new.keep_alive_secs != started.keep_alive_secs
        || new.clean_start != started.clean_start
        || new.session_expiry_interval_secs != started.session_expiry_interval_secs
        || new.session_dir != started.session_dir
        || new.max_packet_size != started.max_packet_size
        || new.receive_maximum != started.receive_maximum
        || new.topic_alias_maximum != started.topic_alias_maximum
        || new.own_topic_prefix != started.own_topic_prefix
    {
        restart.push("mqtt (the CONNECT is sent at start)");
    }
    if new.broker_tls_enabled != started.broker_tls_enabled {
        restart.push("mqtt.tls.enabled");
    }
    if !same_auth_way(started, new)
        || new
            .tenants
            .iter()
            .zip(&started.tenants)
            .any(|(n, s)| !same_auth_way(&started.of_tenant(s), &new.of_tenant(n)))
    {
        restart.push("auth.mode/auth.enhanced.method");
    }
    if new.domain != started.domain {
        restart.push("domain");
    }
    restart
}

/// What a reload replaces in a core: its ACL and subject, its credentials.
struct Live {
    name: String,
    acl: Arc<LiveAcl>,
    auth: Arc<LiveAuth>,
}

/// One SIGHUP reload: the new ACL of the top level and of each tenant goes
/// into its core, with its subject; credentials that authenticate the same
/// way replace the old ones (§7.2); what needs a restart is named.
fn reload(
    lives: &[Live],
    tls: &LiveTls,
    started: &DaemonConfig,
    new: Result<DaemonConfig, String>,
) {
    let new = match new {
        Ok(c) => c,
        Err(e) => {
            reload_refused(&e);
            return;
        }
    };
    let new_tls = match client_tls_from_config(&new) {
        Ok(t) => t,
        Err(e) => {
            reload_refused(&e.to_string());
            return;
        }
    };
    let mut next: Vec<(&Live, DaemonConfig, Option<Subject>)> = Vec::with_capacity(lives.len());
    for live in lives {
        let (cfg, before) = if live.name.is_empty() {
            (new.clone(), started.clone())
        } else {
            match (
                new.tenants.iter().find(|t| t.name == live.name),
                started.tenants.iter().find(|t| t.name == live.name),
            ) {
                (Some(t), Some(s)) => (new.of_tenant(t), started.of_tenant(s)),
                // A tenant that is gone keeps its ACL until the restart.
                _ => continue,
            }
        };
        // The credentials and the subject follow only a reload that
        // authenticates the same way; another way needs a restart.
        let subject = if same_auth_way(&before, &cfg) {
            match subject_of(&cfg) {
                Ok(s) => Some(s),
                Err(e) => {
                    reload_refused(&e);
                    return;
                }
            }
        } else {
            None
        };
        next.push((live, cfg, subject));
    }
    for (live, cfg, subject) in next {
        let subject = match subject {
            Some(s) => {
                live.auth.replace(&cfg);
                s
            }
            None => live.acl.current().1.clone(),
        };
        live.acl.replace(DaemonAcl::of(&cfg), subject);
    }
    // §7.1: the TLS material (CA, certificate, key), the server name and
    // the ALPN offer apply to the next connection; the one that stands
    // keeps its own. A transport the URL and `mqtt.tls.enabled` name
    // differently needs a restart.
    let same_transport = new.broker_url == started.broker_url
        && new.broker_tls_enabled == started.broker_tls_enabled;
    // §8.1: a changed `log_level` applies at once.
    log::set_level(&new.log_level);
    let applied = if same_transport {
        tls.replace(&new, new_tls);
        "acl, auth credentials, log_level, mqtt.tls (from the next connection)"
    } else {
        "acl, auth credentials, log_level"
    };
    log::event(Level::Info, "reload")
        .field("applied", applied)
        .emit();
    let restart = settings_that_need_a_restart(started, &new);
    if !restart.is_empty() {
        log::event(Level::Warn, "reload_needs_restart")
            .field("settings", &restart.join(", "))
            .emit();
    }
}

/// §9.2: a reload whose configuration does not load or is refused changes
/// nothing.
fn reload_refused(reason: &str) {
    log::event(Level::Warn, "reload_refused")
        .field("reason", reason)
        .emit();
}

fn register_topic(
    rt: &Arc<DcpsRuntime>,
    topic: &TopicConfig,
    writers: &mut BTreeMap<String, EntityId>,
    reader_eids: &mut BTreeMap<String, EntityId>,
    readers: &mut Vec<ReaderTopic>,
) -> Result<(), ServerError> {
    // Spec §5.1: the daemon expects an entry with direction=`out|bidir|in`.
    let want_writer = matches!(topic.direction.as_str(), "in" | "bidir");
    let want_reader = matches!(topic.direction.as_str(), "out" | "bidir");

    // §6: reliability, durability, deadline, lifespan, liveliness and
    // partition of the topic's endpoints.
    if want_reader {
        let (eid, rx) = rt
            .register_user_reader(super::qos_translation::reader_config(topic))
            .map_err(|e| ServerError::Dds(format!("reader: {e:?}")))?;
        reader_eids.insert(topic.dds_name.clone(), eid);
        readers.push((topic.clone(), rx));
    }

    if want_writer {
        let eid = rt
            .register_user_writer(super::qos_translation::writer_config(topic))
            .map_err(|e| ServerError::Dds(format!("writer: {e:?}")))?;
        // `qos.history` (§3): KEEP_LAST(depth) or KEEP_ALL of the writer.
        rt.set_user_writer_history_depth(eid, topic.history_depth())
            .map_err(|e| ServerError::Dds(format!("writer history: {e:?}")))?;
        writers.insert(topic.dds_name.clone(), eid);
    }
    Ok(())
}

/// The exit class of a failed connect (Spec §2): a refusal of the
/// credentials or the authorization is 5, a TLS handshake 4, a Session
/// State that cannot be kept in `mqtt.session_dir` 1, everything else 2.
fn map_client_err(e: ClientError) -> ServerError {
    if e.is_auth_refusal() {
        return ServerError::Auth(e.to_string());
    }
    match e {
        ClientError::Tls(m) => ServerError::Tls(m),
        ClientError::Session(m) => ServerError::Config(format!("session state: {m}")),
        ClientError::Io(m) => ServerError::BrokerConnect(m),
        ClientError::TimedOut => {
            ServerError::BrokerConnect("the broker did not answer in time".to_string())
        }
        other => ServerError::BrokerConnect(other.to_string()),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::version::ProtocolVersion;

    fn refusal(reason: u8, version: ProtocolVersion) -> ClientError {
        ClientError::ConnAck {
            reason,
            version,
            detail: String::new(),
        }
    }

    /// Spec §2 and §4.1: a CONNACK that refuses the credentials or the
    /// authorization is exit 5, every other refusal exit 2; a TLS handshake
    /// exit 4; a Session State that cannot be kept exit 1.
    #[test]
    fn mqttbridge_2_the_exit_class_of_a_failed_connect() {
        for r in [0x86, 0x87, 0x8A, 0x8C] {
            assert!(matches!(
                map_client_err(refusal(r, ProtocolVersion::V5)),
                ServerError::Auth(_)
            ));
        }
        for r in [0x80, 0x84, 0x85, 0x88, 0x9C] {
            assert!(matches!(
                map_client_err(refusal(r, ProtocolVersion::V5)),
                ServerError::BrokerConnect(_)
            ));
        }
        assert!(matches!(
            map_client_err(refusal(5, ProtocolVersion::V311)),
            ServerError::Auth(_)
        ));
        assert!(matches!(
            map_client_err(ClientError::Tls("x".into())),
            ServerError::Tls(_)
        ));
        assert!(matches!(
            map_client_err(ClientError::Io("x".into())),
            ServerError::BrokerConnect(_)
        ));
        // §4.1: a Session State that cannot be kept in mqtt.session_dir.
        assert!(matches!(
            map_client_err(ClientError::Session("x".into())),
            ServerError::Config(_)
        ));
    }

    /// §9.2: a reload names what needs a restart, the tenants among them;
    /// a changed tenant ACL is applied and needs none.
    #[test]
    fn mqttbridge_9_2_a_reload_names_the_settings_that_need_a_restart() {
        let base = DaemonConfig::load_from_str(
            "tenants:\n  - name: a\n    mqtt:\n      client_id: id-a\n    acl:\n      default_deny: true\n",
        )
        .unwrap();
        let mut acl_only = base.clone();
        acl_only.tenants[0].acl_default_deny = Some(false);
        assert!(settings_that_need_a_restart(&base, &acl_only).is_empty());
        let mut moved = base.clone();
        moved.tenants[0].domain = 9;
        moved.session_expiry_interval_secs = 60;
        assert_eq!(
            settings_that_need_a_restart(&base, &moved),
            ["tenants", "mqtt (the CONNECT is sent at start)"]
        );
        // §3: the will of the CONNECT is under `mqtt.own_topic_prefix`.
        let mut prefix = base.clone();
        prefix.own_topic_prefix = "zerodds".into();
        assert_eq!(
            settings_that_need_a_restart(&base, &prefix),
            ["mqtt (the CONNECT is sent at start)"]
        );
        // §7.2: another `auth.mode`, another `auth.enhanced.method`; new
        // credentials of the same way need none.
        let mut mode = base.clone();
        mode.username = Some("u".into());
        assert_eq!(
            settings_that_need_a_restart(&base, &mode),
            ["auth.mode/auth.enhanced.method"]
        );
        let oauth = DaemonConfig::load_from_str(
            "auth:\n  mode: enhanced\n  bearer_token: t\n  enhanced:\n    method: OAUTHBEARER\n",
        )
        .unwrap();
        let mut method = oauth.clone();
        method.auth_enhanced_method = Some("JWT".into());
        assert_eq!(
            settings_that_need_a_restart(&oauth, &method),
            ["auth.mode/auth.enhanced.method"]
        );
        let mut token = oauth.clone();
        token.auth_bearer_token = Some("t2".into());
        assert!(settings_that_need_a_restart(&oauth, &token).is_empty());
    }

    /// The user name and password in force in `live`.
    fn in_force(live: &Live) -> (Option<String>, Option<Vec<u8>>) {
        let mut c = DaemonConfig::default_for_dev();
        live.auth.apply_to(&mut c);
        let k = Credentials::of(&c);
        (k.connect_user_name(), k.connect_password())
    }

    /// §7.2, §9.2: a reload that keeps the credentials (only the ACL
    /// changed) starts no new generation, so no connection re-authenticates;
    /// new credentials of the same mode do, the next connection uses them,
    /// and the subject follows the new user name. A reload that changes
    /// `auth.mode` or `auth.enhanced.method` keeps the credentials and the
    /// subject of before.
    #[test]
    fn mqttbridge_7_2_a_reload_takes_new_credentials_of_the_same_way_only() {
        let base = DaemonConfig::load_from_str(
            "auth:\n  mode: password\n  username: u1\n  password: p1\n",
        )
        .unwrap();
        let lives = [Live {
            name: String::new(),
            acl: Arc::new(LiveAcl::new(
                DaemonAcl::of(&base),
                subject_of(&base).unwrap(),
            )),
            auth: Arc::new(LiveAuth::new(&base)),
        }];
        let live = &lives[0];
        let tls = LiveTls::new(&base, None);
        assert_eq!(live.acl.current().1, Subject::named("u1"));

        let mut acl_only = base.clone();
        acl_only.acl_default_deny = Some(true);
        reload(&lives, &tls, &base, Ok(acl_only.clone()));
        let now = live.acl.current();
        assert!(!now.0.may_publish(&now.1, "any/topic"), "the new ACL");
        assert_eq!(live.auth.generation(), 0, "the credentials stayed");

        let mut subject_only = acl_only.clone();
        subject_only.auth_bearer_subject = Some("svc".into());
        reload(&lives, &tls, &base, Ok(subject_only));
        assert_eq!(live.acl.current().1, Subject::named("svc"));
        assert_eq!(live.auth.generation(), 0, "the subject is no credential");

        let mut renamed = acl_only.clone();
        renamed.outbound_username = Some("u2".into());
        renamed.outbound_password = Some("p2".into());
        reload(&lives, &tls, &base, Ok(renamed.clone()));
        assert_eq!(live.auth.generation(), 1);
        assert_eq!(in_force(live), (Some("u2".into()), Some(b"p2".to_vec())));
        assert_eq!(live.acl.current().1, Subject::named("u2"));
        reload(&lives, &tls, &base, Ok(renamed));
        assert_eq!(live.auth.generation(), 1, "the same credentials again");

        let mut scram = acl_only;
        scram.auth_mode = Some(super::super::auth::AuthKind::Enhanced);
        scram.auth_enhanced_method = Some("SCRAM-SHA-256".into());
        scram.outbound_username = Some("u3".into());
        scram.outbound_password = Some("p3".into());
        reload(&lives, &tls, &base, Ok(scram.clone()));
        assert_eq!(live.auth.generation(), 1);
        assert_eq!(
            in_force(live),
            (Some("u2".into()), Some(b"p2".to_vec())),
            "another mode keeps the credentials of before"
        );
        assert_eq!(live.acl.current().1, Subject::named("u2"));
        assert_eq!(
            settings_that_need_a_restart(&base, &scram),
            ["auth.mode/auth.enhanced.method"]
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
            "/../../man/man5/zerodds-mqtt-bridged.yaml.5"
        ))
        .unwrap_or_else(|e| panic!("man5 page: {e}"));
        let named = format!("Default\n.IR \"{DEFAULT_METRICS_LISTEN}\"\nwhen");
        assert!(
            page.contains(&named),
            "man5 does not name {DEFAULT_METRICS_LISTEN}"
        );
    }
}
