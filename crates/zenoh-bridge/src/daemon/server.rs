// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-zenoh-bridged` server: builds the Zenoh session + `DcpsRuntime`,
//! registers the configured topics, and runs the auto-discovery loop that
//! bridges every further DDS publication/subscription the runtime discovers
//! — the way `zenoh-plugin-dds` bridges every DDS topic it sees (see the
//! `mapping`/`runtime` module docs for the source citations).
//!
//! # Auto-discovery
//!
//! Reads `DcpsRuntime::discovered_publication_endpoints` /
//! `discovered_subscription_endpoints` (the DCPS built-in-topic data ZeroDDS'
//! SEDP cache holds) whenever the runtime's discovery generation moves
//! (`DcpsRuntime::wait_discovery_change`: an SEDP datagram added or removed
//! an endpoint, or a TypeLookup reply added types), and at the latest every
//! `discovery.poll_interval_ms`, filtered by `discovery.allow`/
//! `discovery.deny` against the derived key expression. A discovered
//! publication bridges the topic DDS → Zenoh, a discovered subscription
//! Zenoh → DDS; both seen in one pass bridge `bidir` at once. When the
//! opposite side of a topic bridged one way is discovered later, that
//! direction is added (`ZenohBridge::add_topic`), the way
//! `zenoh-plugin-dds` creates its route from DDS and its route to DDS
//! independently; the DDS endpoint of the first direction stays as it is.
//! A topic listed under `topics:` is bridged as configured and never
//! changed by discovery.
//!
//! An auto-discovered topic's type is resolved through the same TypeLookup
//! registry a keyed reader's own KeyHash computation uses
//! (`DcpsRuntime::type_lookup_server`, filled by `DcpsRuntime::
//! register_type_object` for a local type and by a `getTypes` reply for a
//! remote one — see `crates/dcps/src/runtime.rs`'s
//! `UserReaderSlot::resolve_key_hash_support` / `build_key_hash_support`,
//! the pattern `resolve_discovered_type` mirrors) via `DiscoveredEndpointInfo::
//! type_identifier`. When it resolves, the topic is bridged with its real
//! keyed-ness and — if keyed — its `SerializedTypeSupport` for the
//! Zenoh→DDS key-hash derivation, exactly as an explicit `topics:` entry
//! with a `type_file` would be. When it does not (yet) resolve — the
//! `TypeIdentifier` is not an `EquivalenceHashComplete`, or the registry does
//! not have the root or every dependency — the topic is bridged `keyed:
//! false` as before, but `TopicSpec::type_resolution_uncertain` marks that
//! this is this bridge's own fallback, not a known fact: a lifecycle
//! (dispose/unregister) sample on such a route is still forwarded, but
//! counted (`BridgeStats::lifecycle_type_unresolved`) and reported once per
//! topic rather than blending silently into ordinary traffic (see
//! `crate::runtime::forward_pump`). A direction whose `EquivalenceHashComplete`
//! the registry does not have yet waits: the daemon asks the announcing
//! participant for it (`getTypes`), again every `TYPE_LOOKUP_RETRY`, and
//! wires the direction as soon as the type resolves, or after
//! `TYPE_RESOLUTION_DEADLINE` with the fallback above. A type that arrives
//! in the registry after that does not retroactively upgrade an
//! already-wired direction: upgrading in place would mean re-registering its
//! DDS endpoint and losing in-flight samples.

// This daemon's own log sink — see every other bridge daemon's
// `daemon/server.rs`/`daemon/runtime_common.rs` in this workspace.
#![allow(clippy::print_stderr)]

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig};
use zerodds_monitor::Registry;
use zerodds_observability_otlp::OtlpExporter;
use zerodds_qos::DurabilityKind;

use crate::mapping::{self, Extensibility};
use crate::runtime::{BridgeError, Direction, TopicSpec, ZenohBridge, ZenohBridgeBuilder};

use super::config::{DaemonConfig, TopicConfig, ZenohConfig};
use super::runtime_common::{
    BridgeMetrics, CatalogSnapshot, CatalogTopic, SERVICE_NAME, install_signal_watcher,
    otlp_config_from_env, serve_admin_endpoints, spawn_otlp_flush_loop,
};

/// Admin endpoint address when `metrics.enabled` is set without
/// `metrics.listen`; one port per bridge daemon, see man5.
pub const DEFAULT_METRICS_LISTEN: &str = "127.0.0.1:9097";

/// Daemon top-level error.
#[derive(Debug)]
pub enum ServerError {
    /// A setting that cannot be served (exit 1).
    Config(String),
    /// An admin/metrics bind error (exit 2).
    Bind(String),
    /// DDS init (exit 3).
    Dds(String),
    /// Zenoh session error (exit 4).
    Zenoh(String),
}

impl core::fmt::Display for ServerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Config(m) => write!(f, "config: {m}"),
            Self::Bind(m) => write!(f, "bind: {m}"),
            Self::Dds(m) => write!(f, "dds: {m}"),
            Self::Zenoh(m) => write!(f, "zenoh: {m}"),
        }
    }
}

impl std::error::Error for ServerError {}

/// Reads and validates the configuration again, for a SIGHUP reload.
pub type ConfigLoader = Box<dyn Fn() -> Result<DaemonConfig, String> + Send + Sync>;

/// Daemon handle. Dropping it stops the daemon.
pub struct DaemonHandle {
    stop: Arc<AtomicBool>,
    discovery_thread: Option<JoinHandle<()>>,
    admin_thread: Option<JoinHandle<()>>,
    otlp_thread: Option<JoinHandle<()>>,
    metrics_thread: Option<JoinHandle<()>>,
    /// Bound admin address.
    pub admin_addr: Option<String>,
    /// SIGHUP reload flag.
    pub reload_flag: Arc<AtomicBool>,
    /// Healthz.
    pub healthy: Arc<AtomicBool>,
    /// Metric set, for tests.
    pub metrics: Option<BridgeMetrics>,
    /// Kept alive so its `Drop` stops every forward pump / Zenoh
    /// subscriber; also the handle tests and the admin thread read.
    bridge: Arc<ZenohBridge>,
    /// The daemon's DDS participant. [`Self::shutdown`] stops it: dropping
    /// the handle alone would not, because the runtime's worker threads and
    /// the bridge hold `Arc`s of their own.
    dcps: Arc<DcpsRuntime>,
    /// Owns the tokio executor Zenoh's session tasks run on — kept alive
    /// for the daemon's lifetime, dropped (and so shut down) last.
    _tokio_rt: tokio::runtime::Runtime,
}

impl DaemonHandle {
    /// Whether SIGTERM/SIGINT (or [`Self::shutdown`]) asked the daemon to
    /// stop.
    #[must_use]
    pub fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// The live bridge (topic map, stats, Zenoh session).
    #[must_use]
    pub fn bridge(&self) -> &Arc<ZenohBridge> {
        &self.bridge
    }

    /// The DCPS runtime the bridge is wired to.
    #[must_use]
    pub fn dcps_runtime(&self) -> &Arc<DcpsRuntime> {
        &self.dcps
    }

    /// Stops discovery, undeclares every Zenoh subscriber/publisher, joins
    /// every worker thread and stops the DDS participant.
    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.dcps.wake_discovery_waiters();
        if let Some(j) = self.discovery_thread.take() {
            let _ = j.join();
        }
        self.bridge.stop();
        self.healthy.store(false, Ordering::SeqCst);
        if let Some(addr) = self.admin_addr.as_deref() {
            if let Ok(addr) = addr.parse::<SocketAddr>() {
                let _ = std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(200));
            }
        }
        if let Some(j) = self.admin_thread.take() {
            let _ = j.join();
        }
        if let Some(j) = self.metrics_thread.take() {
            let _ = j.join();
        }
        if let Some(j) = self.otlp_thread.take() {
            let _ = j.join();
        }
        // Last, so no pump writes to a stopped runtime. A daemon stopped in a
        // process that keeps running (an embedding, a test) otherwise leaves
        // its participant announcing its endpoints on the domain.
        self.dcps.shutdown();
    }
}

impl Drop for DaemonHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Starts the daemon with the given config, discovering the DDS runtime
/// configuration from the standard layers.
///
/// # Errors
/// See [`start_with_reload`].
pub fn start(cfg: DaemonConfig) -> Result<DaemonHandle, ServerError> {
    let runtime = RuntimeConfig::from_standard_paths()
        .map_err(|e| ServerError::Dds(format!("runtime configuration: {e}")))?;
    start_with_runtime(cfg, runtime)
}

/// [`start`] with the DDS runtime configuration a command line already
/// loaded.
///
/// # Errors
/// As [`start_with_reload`].
pub fn start_with_runtime(
    cfg: DaemonConfig,
    runtime_config: RuntimeConfig,
) -> Result<DaemonHandle, ServerError> {
    start_with_reload(cfg, runtime_config, None)
}

/// [`start_with_runtime`], reloading on SIGHUP: the discovery allow/deny
/// list and `discovery.auto_bridge` apply to the poll iterations that
/// follow. A changed domain, Zenoh mode/connect/listen/scope, or the
/// `topics:` list needs a restart and is logged, not applied. A
/// configuration that does not load changes nothing.
///
/// # Errors
/// [`ServerError::Zenoh`] if the Zenoh session fails to open;
/// [`ServerError::Dds`] if the `DcpsRuntime` fails to start or a starting
/// topic's DDS registration fails; [`ServerError::Config`] for a bad
/// `zenoh.mode`/endpoint/`type_file`; [`ServerError::Bind`] if the admin
/// endpoint's address is already in use.
#[allow(clippy::too_many_lines)]
pub fn start_with_reload(
    cfg: DaemonConfig,
    runtime_config: RuntimeConfig,
    load: Option<ConfigLoader>,
) -> Result<DaemonHandle, ServerError> {
    eprintln!(
        "[{SERVICE_NAME}] starting domain={} zenoh.mode={} topics={}",
        cfg.domain,
        cfg.zenoh.mode,
        cfg.topics.len()
    );

    // 0. Metrics registry.
    let registry = Arc::new(Registry::new());
    let metrics = BridgeMetrics::register(&registry);
    let healthy = Arc::new(AtomicBool::new(true));
    let reload_flag = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));

    // 1. DCPS runtime.
    let prefix = zerodds_dcps::participant::bridge_guid_prefix(&format!(
        "zenoh-bridge-{}-{}",
        cfg.domain,
        std::process::id()
    ));
    let dcps = DcpsRuntime::start(cfg.domain, prefix, runtime_config)
        .map_err(|e| ServerError::Dds(format!("{e:?}")))?;

    // 2. Zenoh session, on a dedicated tokio runtime kept alive for the
    // daemon's lifetime (its background tasks — Zenoh's link I/O, the
    // engine's PublicationCache queryables — need it to keep running).
    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| ServerError::Zenoh(format!("tokio runtime: {e}")))?;
    let zconfig = build_zenoh_config(&cfg.zenoh)?;
    let mut builder = ZenohBridgeBuilder::new()
        .with_dcps_runtime(Arc::clone(&dcps))
        .reliable_routes_blocking(cfg.zenoh.reliable_routes_blocking)
        .queries_timeout(Duration::from_millis(cfg.zenoh.queries_timeout_ms))
        .zenoh_config(zconfig);
    if !cfg.zenoh.scope.is_empty() {
        builder = builder.scope(cfg.zenoh.scope.clone());
    }
    let bridge = tokio_rt
        .block_on(builder.build())
        .map_err(|e| ServerError::Zenoh(format!("{e}")))?;
    let bridge = Arc::new(bridge);
    eprintln!(
        "[{SERVICE_NAME}] zenoh session open, zid={}",
        bridge.session().zid()
    );

    // 3. Register the explicitly configured topics.
    let mut registered: HashSet<String> = HashSet::new();
    for t in &cfg.topics {
        let spec = topic_spec_from_config(t)?;
        tokio_rt
            .block_on(bridge.add_topic(spec))
            .map_err(|e| ServerError::Dds(format!("topic '{}': {e}", t.dds_name)))?;
        registered.insert(t.dds_name.clone());
    }

    // 4. Auto-discovery loop. `live_discovery` is shared with the SIGHUP
    // reload below: `discovery.allow`/`deny`/`auto_bridge` apply to the poll
    // iterations that follow a reload without a restart (see the module
    // docs — `domain`/`zenoh`/`topics` still need one).
    let live_discovery = Arc::new(std::sync::Mutex::new(cfg.discovery.clone()));
    let discovery_thread = Some(
        spawn_discovery_loop(
            Arc::clone(&dcps),
            Arc::clone(&bridge),
            tokio_rt.handle().clone(),
            Arc::clone(&live_discovery),
            cfg.zenoh.scope.clone(),
            registered,
            Arc::clone(&stop),
        )
        .map_err(|e| ServerError::Dds(format!("spawn discovery thread: {e}")))?,
    );

    // 5. Admin endpoint (Prometheus + catalog + healthz), with a periodic
    // sync of the engine's own atomics into the Prometheus counters.
    let mut admin_thread: Option<JoinHandle<()>> = None;
    let mut admin_addr: Option<String> = None;
    let mut metrics_thread: Option<JoinHandle<()>> = None;
    if cfg.metrics_enabled || !cfg.metrics_addr.is_empty() {
        let bind_str = if cfg.metrics_addr.is_empty() {
            DEFAULT_METRICS_LISTEN.to_string()
        } else {
            cfg.metrics_addr.clone()
        };
        let sock: SocketAddr = bind_str
            .parse()
            .map_err(|e| ServerError::Config(format!("metrics.listen: {e}")))?;
        let snap = Arc::new(CatalogSnapshot::new());
        let (h, bound) = serve_admin_endpoints(
            sock,
            Arc::clone(&snap),
            Arc::clone(&registry),
            Arc::clone(&healthy),
            Arc::clone(&stop),
        )
        .map_err(|e| ServerError::Bind(format!("{e}")))?;
        eprintln!("[{SERVICE_NAME}] admin endpoint on {bound} (/metrics /catalog /healthz)");
        admin_addr = Some(bound.to_string());
        admin_thread = Some(h);

        let sync_stats = Arc::clone(bridge.stats());
        let sync_metrics = metrics.clone();
        let sync_stop = Arc::clone(&stop);
        // Same loop also refreshes /catalog and zerodds_zenoh_topics_active
        // from the engine's own live topic map (`ZenohBridge::map()`),
        // which — unlike the static `topics:` config — already holds every
        // topic actually bridged, auto-discovered or explicit. Measured
        // before this: `/catalog` reported `"topics":[]` after the daemon
        // had logged `auto-bridged Square`.
        let sync_bridge = Arc::clone(&bridge);
        let sync_catalog = Arc::clone(&snap);
        metrics_thread = thread::Builder::new()
            .name("zerodds-zenoh-metrics-sync".into())
            .spawn(move || {
                while !sync_stop.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(500));
                    sync_metrics.sync_from(&sync_stats);
                    sync_catalog_and_gauge(&sync_bridge, &sync_metrics, &sync_catalog);
                }
                sync_metrics.sync_from(&sync_stats);
                sync_catalog_and_gauge(&sync_bridge, &sync_metrics, &sync_catalog);
            })
            .ok();
    }

    // 6. Signal watcher.
    if let Err(e) = install_signal_watcher(Arc::clone(&stop), Arc::clone(&reload_flag)) {
        eprintln!("[{SERVICE_NAME}] signal watcher init failed: {e}");
    }
    // 6b. SIGHUP reload.
    {
        let (stop_r, flag_r, live_r) = (
            Arc::clone(&stop),
            Arc::clone(&reload_flag),
            Arc::clone(&live_discovery),
        );
        let started = cfg.clone();
        let _ = thread::Builder::new()
            .name("zerodds-zenoh-reload".into())
            .spawn(move || {
                while !stop_r.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(100));
                    if flag_r.swap(false, Ordering::SeqCst) {
                        if let Some(l) = &load {
                            reload(&started, l(), &live_r);
                        }
                    }
                }
            });
    }

    // 7. OTLP.
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

    Ok(DaemonHandle {
        stop,
        discovery_thread,
        admin_thread,
        otlp_thread,
        metrics_thread,
        admin_addr,
        reload_flag,
        healthy,
        metrics: Some(metrics),
        bridge,
        dcps,
        _tokio_rt: tokio_rt,
    })
}

/// One SIGHUP reload: `discovery.allow`/`deny`/`auto_bridge` apply live —
/// written into `live_discovery`, which the discovery loop (`spawn_discovery_loop`)
/// re-reads every poll iteration. `domain`, `zenoh.*` and `topics:` need a
/// restart (the DDS runtime, the Zenoh session and the registered DCPS
/// readers/writers are not torn down and rebuilt on reload) and are logged,
/// not applied.
fn reload(
    started: &DaemonConfig,
    new: Result<DaemonConfig, String>,
    live_discovery: &Arc<std::sync::Mutex<super::config::DiscoveryConfig>>,
) {
    let new = match new {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[{SERVICE_NAME}] reload refused: {e}");
            return;
        }
    };
    if let Ok(mut live) = live_discovery.lock() {
        *live = new.discovery.clone();
    }
    let mut restart: Vec<&str> = Vec::new();
    if new.topics != started.topics {
        restart.push("topics");
    }
    if new.domain != started.domain {
        restart.push("domain");
    }
    if new.zenoh != started.zenoh {
        restart.push("zenoh");
    }
    eprintln!("[{SERVICE_NAME}] reloaded: discovery.allow/deny/auto_bridge");
    if !restart.is_empty() {
        eprintln!(
            "[{SERVICE_NAME}] reload needs a restart for: {}",
            restart.join(", ")
        );
    }
}

/// Refreshes `/catalog` and `zerodds_zenoh_topics_active` from the engine's
/// own live topic map (`ZenohBridge::map()`) — the periodic counterpart to
/// `BridgeMetrics::sync_from`, called from the same loop (`start_with_reload`
/// step 5). `ZenohBridge::add_topic` records every topic it actually
/// bridges here, whether from the static `topics:` config or the
/// auto-discovery loop, so this one source covers both — unlike the old
/// config-only `CatalogSnapshot`, which never reflected an auto-discovered
/// topic.
fn sync_catalog_and_gauge(
    bridge: &ZenohBridge,
    metrics: &BridgeMetrics,
    catalog: &CatalogSnapshot,
) {
    let map = bridge.map();
    let entries = map.entries();
    metrics.topics_active.set(entries.len() as i64);
    let topics = entries
        .iter()
        .map(|e| CatalogTopic {
            dds_name: e.topic.clone(),
            dds_type: e.type_name.clone(),
            direction: match (e.dds_to_zenoh, e.zenoh_to_dds) {
                (true, true) => "bidir",
                (true, false) => "out",
                (false, true) => "in",
                (false, false) => "none",
            }
            .to_string(),
            keyed: e.keyed,
        })
        .collect();
    catalog.set_topics(topics);
}

/// Builds a `zenoh::Config` from the daemon's `zenoh:` block: an optional
/// full-config passthrough file as the base, with mode/connect/listen/
/// scouting applied over it — the same layering `zenoh-bridge-dds` itself
/// uses (`-c/--config` as the base, `-m/--mode`/`-e/--connect`/`-l/--listen`/
/// `--no-multicast-scouting` applied over it, `zenoh-bridge-dds/src/main.rs`
/// fn `parse_args`).
fn build_zenoh_config(z: &ZenohConfig) -> Result<zenoh::Config, ServerError> {
    let mut config = match &z.config_file {
        Some(path) => zenoh::Config::from_file(path)
            .map_err(|e| ServerError::Config(format!("zenoh.config_file '{path}': {e}")))?,
        None => zenoh::Config::default(),
    };
    let mode = z
        .mode
        .parse()
        .map_err(|_| ServerError::Config(format!("zenoh.mode: invalid mode '{}'", z.mode)))?;
    config
        .set_mode(Some(mode))
        .map_err(|e| ServerError::Config(format!("zenoh.mode: {e:?}")))?;
    if !z.connect.is_empty() {
        let endpoints: Vec<_> = z
            .connect
            .iter()
            .map(|s| {
                s.parse().map_err(|_| {
                    ServerError::Config(format!("zenoh.connect: invalid endpoint '{s}'"))
                })
            })
            .collect::<Result<_, _>>()?;
        config
            .connect
            .endpoints
            .set(endpoints)
            .map_err(|e| ServerError::Config(format!("zenoh.connect: {e:?}")))?;
    }
    if !z.listen.is_empty() {
        let endpoints: Vec<_> = z
            .listen
            .iter()
            .map(|s| {
                s.parse().map_err(|_| {
                    ServerError::Config(format!("zenoh.listen: invalid endpoint '{s}'"))
                })
            })
            .collect::<Result<_, _>>()?;
        config
            .listen
            .endpoints
            .set(endpoints)
            .map_err(|e| ServerError::Config(format!("zenoh.listen: {e:?}")))?;
    }
    config
        .scouting
        .multicast
        .set_enabled(Some(z.multicast_scouting))
        .map_err(|e| ServerError::Config(format!("zenoh.scouting.multicast: {e:?}")))?;
    // Required for PublicationCache (TRANSIENT_LOCAL) — zenoh-bridge-dds
    // itself always turns this on (`main.rs`: "Always add timestamps to
    // publications (required for PublicationCache...)"). Set through the
    // generic JSON5 path rather than the typed `ModeDependentValue` setter,
    // which would need `zenoh-config` as a direct dependency for one field.
    let _ = config.insert_json5("timestamping/enabled", "true");
    Ok(config)
}

/// Builds a [`TopicSpec`] from a configured topic, loading its `type_file`
/// (if any) into a `SerializedTypeSupport` for the keyed Zenoh→DDS key-hash
/// derivation (see the `runtime` module docs).
fn topic_spec_from_config(t: &TopicConfig) -> Result<TopicSpec, ServerError> {
    let direction = match t.direction.as_str() {
        "in" => Direction::ZenohToDds,
        "out" => Direction::DdsToZenoh,
        _ => Direction::Both,
    };
    let durability = match t.durability.as_str() {
        "transient_local" => DurabilityKind::TransientLocal,
        "transient" => DurabilityKind::Transient,
        "persistent" => DurabilityKind::Persistent,
        _ => DurabilityKind::Volatile,
    };
    let extensibility = match t.extensibility.as_str() {
        "appendable" => Extensibility::Appendable,
        "mutable" => Extensibility::Mutable,
        _ => Extensibility::Final,
    };
    let type_support = if t.type_file.is_empty() {
        None
    } else {
        let bytes = std::fs::read(&t.type_file).map_err(|e| {
            ServerError::Config(format!(
                "topics[{}].type_file '{}': {e}",
                t.dds_name, t.type_file
            ))
        })?;
        let ext_u8 = match extensibility {
            Extensibility::Final => 0,
            Extensibility::Appendable => 1,
            Extensibility::Mutable => 2,
        };
        let support = zerodds_dcps::serialized_type::SerializedTypeSupport::from_type_object(
            &t.dds_type,
            &bytes,
            t.keyed,
            ext_u8,
        )
        .map_err(|e| {
            ServerError::Config(format!(
                "topics[{}].type_file '{}': {e}",
                t.dds_name, t.type_file
            ))
        })?;
        Some(support)
    };
    Ok(TopicSpec {
        dds_topic: t.dds_name.clone(),
        dds_type: t.dds_type.clone(),
        direction,
        partition: t.partition.clone(),
        key_expr_override: if t.key_expr.is_empty() {
            None
        } else {
            Some(t.key_expr.clone())
        },
        reliable: !matches!(t.reliability.as_str(), "best_effort"),
        durability,
        extensibility,
        keyed: t.keyed,
        history_depth: t.history_depth.max(1),
        type_support,
        // An explicitly configured topic's `keyed` is an operator
        // assertion, not this bridge's own fallback — never uncertain.
        type_resolution_uncertain: false,
    })
}

/// Reads `DcpsRuntime::discovered_publication_endpoints` /
/// `discovered_subscription_endpoints` on every discovery change and
/// bridges every newly seen topic and every newly seen side of a topic
/// (see the module docs).
///
/// # Errors
/// The OS refused to spawn the thread (out of resources) — propagated to
/// the caller as [`ServerError::Dds`] rather than panicking, since a
/// [`DaemonHandle`] the caller might still try to shut down cleanly should
/// not have been torn out from under it by a panicking thread.
#[allow(clippy::too_many_arguments)]
fn spawn_discovery_loop(
    dcps: Arc<DcpsRuntime>,
    bridge: Arc<ZenohBridge>,
    tokio_handle: tokio::runtime::Handle,
    live_discovery: Arc<std::sync::Mutex<super::config::DiscoveryConfig>>,
    scope: String,
    already: HashSet<String>,
    stop: Arc<AtomicBool>,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("zerodds-zenoh-discovery".into())
        .spawn(move || {
            // Topics listed under `topics:`: bridged as configured, never
            // touched here.
            let configured: HashSet<String> = already;
            // Per auto-discovered topic: whether its publication side and
            // its subscription side have been handled. Each side is
            // attempted once, like the topic as a whole before.
            let mut handled: HashMap<String, (bool, bool)> = HashMap::new();
            // Per topic whose announced type is asked for and not yet
            // answered: see `resolve_or_defer`.
            let mut awaited: HashMap<String, AwaitedType> = HashMap::new();
            // Fixed at start: the fallback cadence itself does not reload
            // live (see the module docs — only allow/deny/auto_bridge do).
            let interval = Duration::from_millis(
                live_discovery
                    .lock()
                    .map(|d| d.poll_interval_ms)
                    .unwrap_or(1000)
                    .max(50),
            );
            // The runtime lists its own endpoints among the discovered
            // ones; the bridge's own reader and writer are not a side of a
            // topic to bridge (zenoh-plugin-dds skips its own participant
            // the same way).
            let own_prefix = dcps.guid_prefix.0;
            let is_own = |ep: &zerodds_dcps::runtime::DiscoveredEndpointInfo| {
                ep.endpoint_guid[..12] == own_prefix
            };
            let mut generation = dcps.discovery_generation();
            while !stop.load(Ordering::SeqCst) {
                let discovery = live_discovery.lock().map(|d| d.clone()).ok();
                if let Some(discovery) = discovery.filter(|d| d.auto_bridge) {
                    // Group this pass's not-yet-handled sides by topic name,
                    // so a topic whose publication and subscription are
                    // both new bridges `Both` in one call.
                    let mut this_pass: HashMap<String, DiscoveredSides> = HashMap::new();
                    for ep in dcps.discovered_publication_endpoints() {
                        if is_own(&ep)
                            || configured.contains(&ep.topic_name)
                            || handled.get(&ep.topic_name).is_some_and(|h| h.0)
                        {
                            continue;
                        }
                        let key = ep.topic_name.clone();
                        this_pass.entry(key).or_default().publication = Some(ep);
                    }
                    for ep in dcps.discovered_subscription_endpoints() {
                        if is_own(&ep)
                            || configured.contains(&ep.topic_name)
                            || handled.get(&ep.topic_name).is_some_and(|h| h.1)
                        {
                            continue;
                        }
                        let key = ep.topic_name.clone();
                        this_pass.entry(key).or_default().subscription = Some(ep);
                    }
                    // A topic whose endpoints are gone no longer waits.
                    awaited.retain(|topic, _| this_pass.contains_key(topic));
                    for (topic_name, sides) in this_pass {
                        let publication = sides.publication.is_some();
                        let subscription = sides.subscription.is_some();
                        let noted = note_discovery(
                            &dcps,
                            &bridge,
                            &tokio_handle,
                            &discovery,
                            &scope,
                            &topic_name,
                            sides,
                            &mut awaited,
                        );
                        // A deferred side stays unhandled and is noted
                        // again on the next pass.
                        if noted == Noted::Handled {
                            let h = handled.entry(topic_name).or_default();
                            h.0 |= publication;
                            h.1 |= subscription;
                        }
                    }
                }
                // A side that waits for its type is looked at again within
                // `TYPE_LOOKUP_RETRY`, whatever the poll interval.
                let wait = if awaited.is_empty() {
                    interval
                } else {
                    interval.min(TYPE_LOOKUP_RETRY)
                };
                generation = dcps.wait_discovery_change(generation, wait);
            }
        })
}

/// One topic name's not-yet-handled endpoints seen in one discovery pass —
/// see [`spawn_discovery_loop`].
#[derive(Default)]
struct DiscoveredSides {
    publication: Option<zerodds_dcps::runtime::DiscoveredEndpointInfo>,
    subscription: Option<zerodds_dcps::runtime::DiscoveredEndpointInfo>,
}

/// Bridges the side(s) of one topic that [`spawn_discovery_loop`] saw new
/// in this pass — `Both` when a publication and a subscription were both
/// new, one direction otherwise; a direction the topic already has is left
/// as it is (`ZenohBridge::add_topic`). The side(s) wait while the type
/// they announce is asked for and not yet answered (`resolve_or_defer`).
#[allow(clippy::too_many_arguments)]
fn note_discovery(
    dcps: &DcpsRuntime,
    bridge: &Arc<ZenohBridge>,
    tokio_handle: &tokio::runtime::Handle,
    discovery: &super::config::DiscoveryConfig,
    scope: &str,
    topic_name: &str,
    sides: DiscoveredSides,
    awaited: &mut HashMap<String, AwaitedType>,
) -> Noted {
    if topic_name.starts_with("DCPS") {
        // Built-in topics are never user data.
        return Noted::Handled;
    }
    let scope_opt = if scope.is_empty() { None } else { Some(scope) };
    let Ok(ke) = mapping::key_expr_for_topic(scope_opt, "", topic_name) else {
        return Noted::Handled;
    };
    if !mapping::topic_allowed(&ke, &discovery.allow, &discovery.deny) {
        return Noted::Handled;
    }
    // The publication side leads when both are present: it is what the
    // DDS->Zenoh direction (the common case for a keyed topic — instances
    // and dispose) needs resolved, and one endpoint's type identity is the
    // same as the other's for the same topic in every real deployment.
    let (direction, primary) = match (&sides.publication, &sides.subscription) {
        (Some(p), Some(_)) => (Direction::Both, p),
        (Some(p), None) => (Direction::DdsToZenoh, p),
        (None, Some(s)) => (Direction::ZenohToDds, s),
        (None, None) => return Noted::Handled,
    };
    // Resolve the type through the TypeLookup registry (see the module
    // docs): known keyed-ness + real key material, or, once it is given up
    // on, a fallback derived from the endpoint's own GUID entity kind, with
    // `type_resolution_uncertain` set so a lifecycle sample on it is never
    // silently blended into ordinary traffic.
    let resolved = match resolve_or_defer(dcps, primary, topic_name, awaited) {
        Resolution::Awaited => return Noted::Deferred,
        Resolution::Resolved(support) => Some(support),
        Resolution::Unresolved => None,
    };
    // Both endpoints of the one topic name disagreeing on keyed-ness by
    // GUID entity kind is a wire-level inconsistency worth an operator's
    // attention, not a silent choice between them — flagged regardless of
    // whether the type resolved above (that path only picks `primary`,
    // which never surfaces a mismatch on the OTHER side).
    if let (Some(p), Some(s)) = (&sides.publication, &sides.subscription) {
        if let (Some(pub_kind), Some(sub_kind)) = (
            topic_kind_from_guid(p.endpoint_guid),
            topic_kind_from_guid(s.endpoint_guid),
        ) {
            if pub_kind != sub_kind {
                eprintln!(
                    "[{SERVICE_NAME}] auto-discovery: {topic_name}'s publication and \
                     subscription endpoints disagree on keyed-ness by GUID entity kind \
                     (publication={pub_kind:?}, subscription={sub_kind:?}, DDSI-RTPS 2.5 \
                     §9.3.1.2 Table 9.1) — bridging by the publication's, as this daemon \
                     always does when both sides are seen in one pass"
                );
            }
        }
    }
    let mut spec = TopicSpec {
        dds_topic: topic_name.to_string(),
        dds_type: primary.type_name.clone(),
        direction,
        reliable: primary.reliable,
        durability: if primary.transient_local {
            DurabilityKind::TransientLocal
        } else {
            DurabilityKind::Volatile
        },
        keyed: false,
        type_resolution_uncertain: true,
        ..TopicSpec::default()
    };
    match resolved {
        Some(support) => {
            spec.keyed = support.is_keyed();
            spec.extensibility = to_mapping_extensibility(support.extensibility());
            spec.type_resolution_uncertain = false;
            if spec.keyed {
                spec.type_support = Some(support);
            }
        }
        None => {
            // The type did not resolve, but the wire already answers the
            // one question that matters most for this route to even
            // deliver anything: is the endpoint keyed? DDSI-RTPS 2.5
            // §9.3.1.2 Table 9.1 ties that to the `entityKind` octet (the
            // last byte of the GUID) regardless of the type — `0x02`/`0x07`
            // (and their builtin equivalents `0xC2`/`0xC7`) WITH_KEY,
            // `0x03`/`0x04` (`0xC3`/`0xC4`) NO_KEY. `zenoh-plugin-dds` reads
            // this exact byte the exact same way for its own auto-discovery
            // (`dds_mgt.rs`: `let keyless = (*sample).key.v[15] == 3 ||
            // (*sample).key.v[15] == 4;`), rather than defaulting to
            // keyless. Defaulting to keyless here is what was measured
            // against Cyclone DDS 11.0.1 on domain 151/152: the daemon
            // registered a keyless (`0x04`) reader against Cyclone's keyed
            // (`0x02`) writer, and DDSI-RTPS 2.5 §8.4.1's writer/reader
            // matching never pairs the two — 0 samples received, though
            // both sides logged as if bridged.
            spec.keyed = topic_kind_from_guid(primary.endpoint_guid)
                == Some(zerodds_rtps::wire_types::TopicKind::WithKey);
            eprintln!(
                "[{SERVICE_NAME}] auto-discovery: {topic_name}'s type did not resolve through \
                 the TypeLookup registry (yet) — bridged {} from its endpoint GUID's entity \
                 kind (DDSI-RTPS 2.5 §9.3.1.2 Table 9.1); a dispose/unregister on it is \
                 forwarded but counted separately (see lifecycle_type_unresolved_total), not \
                 dropped",
                if spec.keyed { "keyed" } else { "keyless" }
            );
        }
    }
    match tokio_handle.block_on(bridge.add_topic(spec)) {
        Ok(key_expr) => {
            eprintln!(
                "[{SERVICE_NAME}] auto-bridged {topic_name} ({}) -> {key_expr}",
                match direction {
                    Direction::DdsToZenoh => "dds->zenoh",
                    Direction::ZenohToDds => "zenoh->dds",
                    Direction::Both => "bidir",
                }
            );
        }
        Err(BridgeError::InvalidKeyExpr(e)) => {
            eprintln!("[{SERVICE_NAME}] auto-discovery skipped {topic_name}: {e}");
        }
        Err(e) => {
            eprintln!("[{SERVICE_NAME}] auto-discovery failed for {topic_name}: {e}");
        }
    }
    Noted::Handled
}

/// What [`note_discovery`] did with a topic's side(s).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Noted {
    /// Bridged, or not to be bridged: not looked at again.
    Handled,
    /// Waits for its announced type: looked at again on the next pass.
    Deferred,
}

/// Where a discovered endpoint's type stands for [`note_discovery`].
enum Resolution {
    /// In the registry: the direction is wired with it.
    Resolved(Arc<zerodds_dcps::serialized_type::SerializedTypeSupport>),
    /// Asked for and not yet answered: the direction waits.
    Awaited,
    /// Nothing a `getTypes` reply could fill in, or no answer by
    /// [`TYPE_RESOLUTION_DEADLINE`]: the direction is wired with the
    /// fallback.
    Unresolved,
}

/// A topic whose announced type is asked for and not yet answered: since
/// when, and when it was last asked for.
struct AwaitedType {
    since: std::time::Instant,
    asked: std::time::Instant,
}

/// [`resolve_discovered_type`], and for an `EquivalenceHashComplete` the
/// registry does not have, a `getTypes` request to the announcing
/// participant: then the direction waits, the request is repeated every
/// [`TYPE_LOOKUP_RETRY`], and after [`TYPE_RESOLUTION_DEADLINE`] the
/// direction is wired without the type.
///
/// `crates/dcps/src/runtime.rs`'s automatic `send_type_lookup_request` call
/// sites (`revisit_wired_writer_match` / the first-pass match path) fire
/// only when a *local* user reader/writer is matched against the remote
/// endpoint — this function runs *before* this bridge has registered one
/// (whether to register it `keyed` at all is exactly what the answer
/// decides), so no local match has happened yet to trigger the request.
/// A `getTypes` reply that adds types moves the runtime's discovery
/// generation and with it the discovery loop
/// (`DcpsRuntime::wait_discovery_change`), so a waiting direction is wired
/// as soon as the reply is in. The loop does not block on the reply: other
/// topics are bridged meanwhile.
fn resolve_or_defer(
    dcps: &DcpsRuntime,
    endpoint: &zerodds_dcps::runtime::DiscoveredEndpointInfo,
    topic_name: &str,
    awaited: &mut HashMap<String, AwaitedType>,
) -> Resolution {
    if let Some(support) =
        resolve_discovered_type(dcps, &endpoint.type_identifier, &endpoint.type_name)
    {
        awaited.remove(topic_name);
        return Resolution::Resolved(support);
    }
    if !matches!(
        endpoint.type_identifier,
        zerodds_types::TypeIdentifier::EquivalenceHashComplete(_)
    ) {
        // Nothing a `getTypes` reply could ever fill in for this identifier
        // kind — no point asking or waiting.
        return Resolution::Unresolved;
    }
    let now = std::time::Instant::now();
    match awaited.get_mut(topic_name) {
        Some(waiting) if now.duration_since(waiting.since) >= TYPE_RESOLUTION_DEADLINE => {
            awaited.remove(topic_name);
            return Resolution::Unresolved;
        }
        Some(waiting) if now.duration_since(waiting.asked) < TYPE_LOOKUP_RETRY => {
            return Resolution::Awaited;
        }
        Some(waiting) => waiting.asked = now,
        None => {
            awaited.insert(
                topic_name.to_string(),
                AwaitedType {
                    since: now,
                    asked: now,
                },
            );
        }
    }
    let mut prefix_bytes = [0u8; 12];
    prefix_bytes.copy_from_slice(&endpoint.endpoint_guid[..12]);
    let peer_prefix = zerodds_rtps::wire_types::GuidPrefix::from_bytes(prefix_bytes);
    let request = core::slice::from_ref(&endpoint.type_identifier);
    let _ = dcps.send_type_lookup_request(peer_prefix, request);
    let _ = dcps.send_type_dependencies_request(peer_prefix, request);
    Resolution::Awaited
}

/// How often [`resolve_or_defer`] asks a peer again for a type it announced
/// and has not answered for.
const TYPE_LOOKUP_RETRY: Duration = Duration::from_secs(1);

/// How long a direction waits for its announced type before it is wired
/// without it. One wait of 1 s was too short on a loaded CI host: the
/// `getTypes` reply came later and the topic stayed bridged without its type
/// (`tests/daemon_e2e.rs`,
/// `auto_discovered_keyed_topic_with_registered_type_bridges_keyed`, red in
/// one of three pipelines).
const TYPE_RESOLUTION_DEADLINE: Duration = Duration::from_secs(10);

/// Resolves a discovered endpoint's type through the runtime's TypeLookup
/// registry — the same registry lookup `crates/dcps/src/runtime.rs`'s
/// `UserReaderSlot::build_key_hash_support` performs for a keyed reader's
/// own KeyHash computation (`type_identifier` → `TypeRegistry::get_complete`
/// for the root and every transitive dependency →
/// `SerializedTypeSupport::from_complete_type_objects`, which reads
/// `keyed`/`extensibility` off the resolved type rather than requiring the
/// caller to assert them). Returns `None` when `type_identifier` is not an
/// `EquivalenceHashComplete` (nothing to look up by), or the registry does
/// not yet have the root or every dependency the root needs.
fn resolve_discovered_type(
    dcps: &DcpsRuntime,
    type_identifier: &zerodds_types::TypeIdentifier,
    type_name: &str,
) -> Option<Arc<zerodds_dcps::serialized_type::SerializedTypeSupport>> {
    let zerodds_types::TypeIdentifier::EquivalenceHashComplete(hash) = type_identifier else {
        return None;
    };
    let server = dcps.type_lookup_server.lock().ok()?;
    let root = server.registry.get_complete(hash)?.clone();
    let dependency_hashes = server
        .registry
        .transitive_dependencies(hash, zerodds_types::resolve::DEFAULT_MAX_RESOLVE_NODES);
    let mut dependencies = Vec::with_capacity(dependency_hashes.len());
    for dependency_hash in &dependency_hashes {
        dependencies.push(server.registry.get_complete(dependency_hash)?.clone());
    }
    drop(server);
    zerodds_dcps::serialized_type::SerializedTypeSupport::from_complete_type_objects(
        type_name,
        root,
        dependencies,
    )
    .ok()
}

/// Fallback keyed-ness for a discovered endpoint whose type did not
/// resolve: DDSI-RTPS 2.5 §9.3.1.2 Table 9.1 ties an endpoint's
/// `entityKind` octet — the last byte of its 16-byte GUID (12-byte
/// participant prefix + 4-byte `EntityId`, and within the `EntityId` itself
/// 3 bytes `entityKey` + 1 byte `entityKind`, `crates/rtps/src/wire_types.rs`)
/// — to [`zerodds_rtps::wire_types::TopicKind`] regardless of the type:
/// `0x02`/`0x07` (builtin `0xC2`/`0xC7`) `WITH_KEY`, `0x03`/`0x04`
/// (builtin `0xC3`/`0xC4`) `NO_KEY`. `zenoh-plugin-dds` reads the exact
/// same byte the exact same way for its own auto-discovery
/// (`zenoh-plugin-dds/src/dds_mgt.rs`: `let keyless = (*sample).key.v[15]
/// == 3 || (*sample).key.v[15] == 4;`, checked against that crate's
/// upstream source on GitHub). `None` when the octet names neither — a
/// vendor-quirky or malformed GUID on what should be a *user* endpoint,
/// which Table 9.1 does not otherwise allow for a publication or
/// subscription.
fn topic_kind_from_guid(endpoint_guid: [u8; 16]) -> Option<zerodds_rtps::wire_types::TopicKind> {
    zerodds_rtps::wire_types::EntityKind::from_byte(endpoint_guid[15]).topic_kind()
}

/// Maps `crates/dcps`'s `Extensibility` (read off a resolved
/// `SerializedTypeSupport`) to this crate's own — the two enums are
/// identical in meaning (XTypes 1.3 §7.2.2 Final/Appendable/Mutable) but
/// distinct types, since `crate::mapping` has no dependency on `zerodds-dcps`
/// (it stays buildable with `--no-default-features --features std`).
fn to_mapping_extensibility(ext: zerodds_dcps::dds_type::Extensibility) -> Extensibility {
    match ext {
        zerodds_dcps::dds_type::Extensibility::Final => Extensibility::Final,
        zerodds_dcps::dds_type::Extensibility::Appendable => Extensibility::Appendable,
        zerodds_dcps::dds_type::Extensibility::Mutable => Extensibility::Mutable,
    }
}
