// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Live runtime — only active with the `zenoh-runtime` feature. This is the
//! actual bridge engine: it couples a ZeroDDS [`DcpsRuntime`] (the same raw,
//! type-agnostic byte-sample channel `crates/routing-service/src/forwarding.rs`
//! and every other bridge daemon use — see `crates/dcps/src/serialized_type.rs`
//! for the typed-topic counterpart) and a Zenoh [`Session`] together, per
//! topic, in both directions.
//!
//! # Lifecycle
//!
//! 1. [`ZenohBridgeBuilder`] collects the Zenoh session config, the
//!    [`DcpsRuntime`] to bridge, and (optionally) a starting set of
//!    [`TopicSpec`]s.
//! 2. [`ZenohBridgeBuilder::build`] opens the Zenoh session and registers the
//!    starting topics.
//! 3. [`ZenohBridge::add_topic`] registers further topics at any time — the
//!    daemon's auto-discovery loop (`crates/zenoh-bridge/src/daemon`) calls
//!    this as new DDS publications/subscriptions are discovered.
//! 4. [`ZenohBridge::stop`] joins every forward pump and undeclares every
//!    Zenoh subscriber / publication cache.
//!
//! # Per-topic wiring
//!
//! * **DDS → Zenoh**: a `DcpsRuntime` user reader delivers raw
//!   [`UserSample`]s over an `mpsc::Receiver`; a forward-pump thread prepends
//!   the RTPS encapsulation header ([`crate::mapping::encapsulation_header`])
//!   and `put`s the result on the topic's Zenoh key expression
//!   ([`crate::mapping::key_expr_for_topic`]), exactly the bytes
//!   `zenoh-plugin-dds` would put for the same DDS sample (see the
//!   `mapping` module docs for the source citation). A `Durability::
//!   TransientLocal` topic additionally gets a `zenoh_ext::PublicationCache`
//!   on the same key, so a late-joining Zenoh subscriber can query the
//!   retained sample — the same mechanism `zenoh-plugin-dds` uses
//!   (`route_dds_zenoh.rs`, cited in the `mapping` module docs).
//! * **Zenoh → DDS**: a Zenoh subscriber (a `zenoh_ext::FetchingSubscriber`
//!   for a `TransientLocal` topic, a plain `Subscriber` otherwise) strips the
//!   encapsulation header off every `Put` sample
//!   ([`crate::mapping::decode_encapsulation_header`]) and writes the body
//!   into the matching DCPS user writer.
//! * **Dispose/unregister**: `zenoh-plugin-dds` does not forward DDS
//!   dispose/unregister at all — its DDS→Zenoh listener only republishes
//!   samples with `valid_data` set (`dds_mgt.rs` fn `data_forwarder_listener`,
//!   cited in the `mapping` module docs), and its Zenoh→DDS path writes
//!   *any* incoming sample, `Put` or `Delete`, as an ordinary DDS write
//!   (`route_zenoh_dds.rs` fn `do_route_data`) — so a `Delete` it received
//!   would not actually dispose anything. This project's rule is that a
//!   sample the bridge cannot forward is logged and counted, never silently
//!   dropped, so `zerodds-zenoh-bridged` does forward dispose/unregister: as
//!   a Zenoh `delete` on `<key-expr>/<key-hash-hex>` — the topic's ordinary
//!   key expression with the 16-byte DDS key hash appended as one more
//!   segment, so every reverse subscriber is declared on `<key-expr>/**`
//!   (which also matches the bare `<key-expr>` the ordinary `put` path
//!   uses). This is an *extension* of the plugin's key-expression
//!   convention, not a replacement — a `zenoh-bridge-dds` peer that never
//!   sends such a delete is unaffected by it.

// The forward/reverse pumps run on plain OS threads outside any request
// framework (like every other bridge daemon's DDS pump in this workspace,
// e.g. `crates/coap-bridge/src/daemon/server.rs`), so `eprintln!` is this
// crate's log sink, not a leftover debug print.
#![allow(clippy::print_stderr)]

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use zenoh::Session;
use zenoh::qos::CongestionControl as ZCongestionControl;
use zenoh::sample::{Locality, Sample, SampleKind};
use zerodds_dcps::runtime::{DcpsRuntime, UserReaderConfig, UserSample, UserWriterConfig};
use zerodds_qos::DurabilityKind;
use zerodds_rtps::EntityId;
use zerodds_types::qos::ExtensibilityForRepr;

use crate::mapping::{self, Extensibility, TopicMap, TopicMapEntry};

/// Bridge error.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BridgeError {
    /// The Zenoh session could not be established.
    #[error("zenoh session failed: {0}")]
    ZenohSession(String),
    /// A DDS reader/writer could not be registered.
    #[error("dds registration failed: {0}")]
    Dds(String),
    /// No [`DcpsRuntime`] was given to the builder.
    #[error("no DcpsRuntime given (ZenohBridgeBuilder::with_dcps_runtime)")]
    NoRuntime,
    /// The topic's key expression is not valid (see
    /// [`crate::mapping::validate_key_expr_segment`]).
    #[error("invalid key expression: {0}")]
    InvalidKeyExpr(String),
}

/// Which way a [`TopicSpec`] is bridged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// DDS → Zenoh only.
    DdsToZenoh,
    /// Zenoh → DDS only.
    ZenohToDds,
    /// Both directions.
    Both,
}

impl Direction {
    fn wants_dds_to_zenoh(self) -> bool {
        matches!(self, Self::DdsToZenoh | Self::Both)
    }
    fn wants_zenoh_to_dds(self) -> bool {
        matches!(self, Self::ZenohToDds | Self::Both)
    }
}

/// One DDS topic to bridge.
#[derive(Debug, Clone)]
pub struct TopicSpec {
    /// DDS topic name.
    pub dds_topic: String,
    /// DDS IDL type name (informational + carried into discovery).
    pub dds_type: String,
    /// Direction to bridge.
    pub direction: Direction,
    /// DDS partition (empty = default partition, no key-expression segment).
    pub partition: String,
    /// Overrides the derived key expression (`crate::mapping::
    /// key_expr_for_topic`) with an explicit one. `None` = derive it.
    pub key_expr_override: Option<String>,
    /// `true` = RELIABLE, `false` = BEST_EFFORT.
    pub reliable: bool,
    /// DDS durability. `TransientLocal` turns on the late-joiner path
    /// (`PublicationCache` / `FetchingSubscriber`).
    pub durability: DurabilityKind,
    /// The topic root type's extensibility — picks the RTPS encapsulation
    /// header (see the module docs and `crate::mapping::Extensibility`).
    pub extensibility: Extensibility,
    /// Whether the topic is keyed (has one or more `@key` fields), asserted
    /// by an explicit `topics:` entry or resolved through TypeLookup for an
    /// auto-discovered one — or, when neither, this bridge's own
    /// GUID-entity-kind fallback (DDSI-RTPS 2.5 §9.3.1.2 Table 9.1, see
    /// `crate::daemon::server::note_discovery`). With [`Self::type_support`]
    /// present, the Zenoh→DDS direction derives the real per-sample key
    /// hash from it (`reverse_deliver`). Without it, every sample writes
    /// via `write_user_sample_borrowed` — no key hash, spec-conformant
    /// (DDSI-RTPS 2.5 §8.7.10) rather than a fabricated wrong one — and
    /// this route's own retained history is KEEP_ALL-bounded instead of
    /// per-instance KEEP_LAST; see [`ZenohBridge::wire_reverse`]'s doc
    /// comment for the full design.
    pub keyed: bool,
    /// `PublicationCache`/`FetchingSubscriber` history depth for a
    /// `TransientLocal` topic. Default `1` (matches `zenoh_ext`'s own
    /// `PublicationCacheBuilder` default).
    pub history_depth: usize,
    /// Type support built from a `CompleteTypeObject` (the daemon config's
    /// per-topic `type_file`, see `crates/zenoh-bridge/src/daemon/config.rs`)
    /// — used on the Zenoh→DDS direction of a `keyed` topic to compute the
    /// DDS key hash of an incoming Zenoh sample
    /// (`SerializedTypeSupport::read_body(..).key_material()`) so
    /// `write_user_sample_keyed` registers/disposes the right instance
    /// instead of the one default instance a `keyed: false` topic gets.
    /// Unused (and unnecessary) for the DDS→Zenoh direction, whose
    /// `UserSample` already carries the key hash ZeroDDS' own RTPS layer
    /// extracted, no type needed.
    pub type_support: Option<Arc<zerodds_dcps::serialized_type::SerializedTypeSupport>>,
    /// `true` when `keyed: false` is *not* an operator assertion that the
    /// topic really has no key, but this crate's own fallback for a topic
    /// whose type it could not resolve (auto-discovery: no
    /// `EquivalenceHashComplete` `TypeIdentifier`, or the TypeLookup
    /// registry does not have it — see `crate::daemon::server::
    /// resolve_discovered_type`). A route built from an explicit `topics:`
    /// entry is never uncertain: the operator said `keyed: false` and is
    /// trusted on it. The difference matters only for a lifecycle
    /// (dispose/unregister) sample: on an uncertain route it is still
    /// forwarded (never dropped), but counted and logged once per topic as
    /// what it is — a key hash this bridge could not verify — instead of
    /// blending silently into the ordinary alive-sample traffic. Default
    /// `false`.
    pub type_resolution_uncertain: bool,
}

impl Default for TopicSpec {
    fn default() -> Self {
        Self {
            dds_topic: String::new(),
            dds_type: String::new(),
            direction: Direction::Both,
            partition: String::new(),
            key_expr_override: None,
            reliable: true,
            durability: DurabilityKind::Volatile,
            extensibility: Extensibility::Final,
            keyed: false,
            history_depth: 1,
            type_support: None,
            type_resolution_uncertain: false,
        }
    }
}

/// Builder for a [`ZenohBridge`].
pub struct ZenohBridgeBuilder {
    scope: Option<String>,
    reliable_routes_blocking: bool,
    queries_timeout: Duration,
    zenoh_config: zenoh::Config,
    runtime: Option<Arc<DcpsRuntime>>,
    topics: Vec<TopicSpec>,
}

impl ZenohBridgeBuilder {
    /// New builder: default scope ([`crate::mapping::DEFAULT_SCOPE`], i.e.
    /// none — matching `zenoh-plugin-dds`'s own default), peer-mode Zenoh
    /// config, `reliable_routes_blocking` on (the plugin's default), no
    /// `DcpsRuntime` yet (set via [`Self::with_dcps_runtime`]) and no topics.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scope: mapping::DEFAULT_SCOPE.map(str::to_string),
            reliable_routes_blocking: true,
            queries_timeout: Duration::from_secs(5),
            zenoh_config: zenoh::Config::default(),
            runtime: None,
            topics: Vec::new(),
        }
    }

    /// Connects the bridge with an active [`DcpsRuntime`]. The DDS-side
    /// forward/reverse path registers user readers/writers on it.
    #[must_use]
    pub fn with_dcps_runtime(mut self, runtime: Arc<DcpsRuntime>) -> Self {
        self.runtime = Some(runtime);
        self
    }

    /// Sets the Zenoh key-expression scope (`None` = no scope segment,
    /// matching `zenoh-plugin-dds`'s default; `Some("dds")` reproduces the
    /// pre-existing ZeroDDS-only convention). See the `mapping` module docs.
    #[must_use]
    pub fn scope(mut self, scope: impl Into<String>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    /// Clears the scope (no key-expression segment). The builder already
    /// starts with no scope; this exists for callers that built a scope
    /// conditionally.
    #[must_use]
    pub fn no_scope(mut self) -> Self {
        self.scope = None;
        self
    }

    /// Sets `reliable_routes_blocking` (default `true`, matching the
    /// plugin): whether a `put` for a `RELIABLE` DDS writer blocks
    /// (`CongestionControl::Block`) instead of dropping under Zenoh
    /// backpressure.
    #[must_use]
    pub fn reliable_routes_blocking(mut self, v: bool) -> Self {
        self.reliable_routes_blocking = v;
        self
    }

    /// Sets the timeout for the `TransientLocal` late-joiner query (default
    /// 5 s, matching `zenoh-plugin-dds`'s `queries_timeout`).
    #[must_use]
    pub fn queries_timeout(mut self, timeout: Duration) -> Self {
        self.queries_timeout = timeout;
        self
    }

    /// Full passthrough for the underlying `zenoh::Config` (mode, connect,
    /// listen, scouting, TLS, ...). Overwrites whatever a previous call to
    /// this method set.
    #[must_use]
    pub fn zenoh_config(mut self, cfg: zenoh::Config) -> Self {
        self.zenoh_config = cfg;
        self
    }

    /// Adds a topic to bridge as soon as [`Self::build`] runs. Further
    /// topics can be added later via [`ZenohBridge::add_topic`].
    #[must_use]
    pub fn add_topic(mut self, spec: TopicSpec) -> Self {
        self.topics.push(spec);
        self
    }

    /// Builds the bridge: opens the Zenoh session and registers every topic
    /// added via [`Self::add_topic`].
    ///
    /// # Errors
    /// [`BridgeError::NoRuntime`] without [`Self::with_dcps_runtime`];
    /// [`BridgeError::ZenohSession`] if the Zenoh open fails;
    /// [`BridgeError::Dds`] / [`BridgeError::InvalidKeyExpr`] if a starting
    /// topic fails to register (the first such failure aborts `build`; a
    /// topic added later via [`ZenohBridge::add_topic`] fails on its own
    /// without taking the bridge down).
    pub async fn build(self) -> Result<ZenohBridge, BridgeError> {
        let runtime = self.runtime.ok_or(BridgeError::NoRuntime)?;
        let session = zenoh::open(self.zenoh_config)
            .await
            .map_err(|e| BridgeError::ZenohSession(format!("{e}")))?;
        let bridge = ZenohBridge {
            session: Arc::new(session),
            runtime,
            scope: self.scope,
            reliable_routes_blocking: self.reliable_routes_blocking,
            queries_timeout: self.queries_timeout,
            stop: Arc::new(AtomicBool::new(false)),
            map: Mutex::new(TopicMap::new()),
            forward_threads: Mutex::new(Vec::new()),
            pub_caches: Mutex::new(Vec::new()),
            reverse_subs: Mutex::new(Vec::new()),
            reverse_writers: Arc::new(Mutex::new(BTreeMap::new())),
            stats: Arc::new(BridgeStats::default()),
        };
        for spec in self.topics {
            bridge.add_topic(spec).await?;
        }
        Ok(bridge)
    }
}

impl Default for ZenohBridgeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Live counters. `AtomicU64`, so a caller (e.g. the daemon's Prometheus
/// admin endpoint) can mirror them into its own metric registry without this
/// crate depending on one.
#[derive(Debug, Default)]
pub struct BridgeStats {
    /// Samples forwarded DDS → Zenoh (alive + dispose/unregister).
    pub dds_to_zenoh: AtomicU64,
    /// Samples forwarded Zenoh → DDS (alive + dispose).
    pub zenoh_to_dds: AtomicU64,
    /// Topics refused at registration (invalid key expression, DDS
    /// registration failure).
    pub topics_rejected: AtomicU64,
    /// A sample that arrived but could not be forwarded (decode failure,
    /// unmapped Zenoh key, DDS write failure) — logged by the caller and
    /// counted here, never silently dropped.
    pub forward_errors: AtomicU64,
    /// A lifecycle (dispose/unregister) sample forwarded on a route whose
    /// `keyed: false` is this crate's own fallback for an unresolved type,
    /// not an operator assertion — see [`TopicSpec::type_resolution_uncertain`].
    /// Still forwarded (never dropped); counted separately so it does not
    /// blend into [`Self::dds_to_zenoh`] as an ordinary sample.
    pub lifecycle_type_unresolved: AtomicU64,
    /// A Zenoh→DDS writer registered for a keyed topic with no
    /// [`TopicSpec::type_support`] (an auto-discovered topic whose type
    /// never resolved — see `crate::daemon::server::note_discovery`, and
    /// [`ZenohBridge::wire_reverse`]'s doc comment for the design this
    /// counts). Counted once per such topic at registration, not per
    /// sample.
    pub unknown_key_writers: AtomicU64,
}

/// A live Zenoh subscriber declared for the Zenoh→DDS direction — either
/// plain (`Volatile`) or querying for historical data at declaration time
/// (`TransientLocal`, `zenoh_ext::FetchingSubscriber`). Held only to keep the
/// declaration alive; dropped on [`ZenohBridge::stop`].
///
/// `zenoh_ext::FetchingSubscriber`/`PublicationCache` are marked deprecated
/// in favour of `AdvancedPublisher`/`AdvancedSubscriber` — see the module
/// docs: wire compatibility with `zenoh-plugin-dds` means using the same
/// mechanism it does, not the newer replacement a `zenoh-bridge-dds` peer
/// would not recognise.
#[allow(deprecated)]
enum ReverseSub {
    /// Never read: dropping it undeclares the subscriber, which is the
    /// point of holding it.
    Plain(#[allow(dead_code)] zenoh::pubsub::Subscriber<()>),
    /// Never read; see `Plain`.
    Fetching(#[allow(dead_code)] zenoh_ext::FetchingSubscriber<()>),
}

/// Retained-history depth [`ZenohBridge::wire_reverse`] sets for a keyed
/// Zenoh→DDS writer with no resolved type, in place of the default
/// per-instance `KEEP_LAST(1)`: KEEP_ALL bounded by a resource limit rather
/// than the unbounded `usize::MAX` `DcpsRuntime::set_user_writer_history_depth`
/// would otherwise take literally — see `wire_reverse`'s doc comment for
/// why folding every instance under one fallback key needs the bound
/// raised, not removed. Matches the `max_samples: 1024` bound
/// `DcpsRuntime::register_user_writer_kind` already gives this same
/// writer's own RTPS resend cache, for the same "cache as a whole" resource
/// budget rather than a second, independently chosen number.
const UNKNOWN_KEY_HISTORY_DEPTH: usize = 1024;

/// Live bridge: a Zenoh [`Session`] coupled with a [`DcpsRuntime`], one
/// forward pump thread and one reverse Zenoh subscriber per bridged topic.
#[allow(deprecated)] // `zenoh_ext::PublicationCache` — see `ReverseSub`.
pub struct ZenohBridge {
    session: Arc<Session>,
    runtime: Arc<DcpsRuntime>,
    scope: Option<String>,
    reliable_routes_blocking: bool,
    queries_timeout: Duration,
    stop: Arc<AtomicBool>,
    map: Mutex<TopicMap>,
    forward_threads: Mutex<Vec<JoinHandle<()>>>,
    pub_caches: Mutex<Vec<zenoh_ext::PublicationCache>>,
    reverse_subs: Mutex<Vec<ReverseSub>>,
    /// The Zenoh → DDS writer of each topic, for the forward pump of the
    /// same topic: it records the key of every instance it carries to
    /// Zenoh with that writer, so a Zenoh `delete` coming back names an
    /// instance whose `serializedKey` the dispose can carry.
    reverse_writers: Arc<Mutex<BTreeMap<String, EntityId>>>,
    stats: Arc<BridgeStats>,
}

impl ZenohBridge {
    /// Returns the topic map (DDS topic ↔ Zenoh key expression) built so far.
    #[must_use]
    pub fn map(&self) -> TopicMap {
        self.map.lock().map(|m| m.clone()).unwrap_or_default()
    }

    /// Returns the underlying Zenoh session.
    #[must_use]
    pub fn session(&self) -> &Arc<Session> {
        &self.session
    }

    /// Returns the connected DCPS runtime.
    #[must_use]
    pub fn dcps_runtime(&self) -> &Arc<DcpsRuntime> {
        &self.runtime
    }

    /// Live counters.
    #[must_use]
    pub fn stats(&self) -> &Arc<BridgeStats> {
        &self.stats
    }

    /// Number of active forward-pump threads.
    #[must_use]
    pub fn forward_thread_count(&self) -> usize {
        self.forward_threads.lock().map(|v| v.len()).unwrap_or(0)
    }

    /// Registers and wires one more topic. A topic already bridged (by DDS
    /// topic name) keeps its key expression and gets only the directions
    /// `spec.direction` names that it does not have yet; a direction it
    /// has is never wired twice. This is `zenoh-plugin-dds`'s own
    /// per-direction check (`routes_from_dds.contains_key` /
    /// `routes_to_dds.get`): the route from DDS and the route to DDS of one
    /// topic are created independently, each when its side is discovered.
    /// The map entry keeps the type name, QoS and keyed-ness it was first
    /// registered with.
    ///
    /// # Errors
    /// [`BridgeError::InvalidKeyExpr`] when the derived (or overridden) key
    /// expression is not valid; [`BridgeError::Dds`] when the DDS reader/
    /// writer registration fails. A `spec.keyed` topic bridged Zenoh→DDS
    /// without `spec.type_support` is **not** refused — see
    /// `Self::wire_reverse`'s doc comment for the design (no fabricated
    /// key hash, KEEP_ALL-bounded retained history instead of folding
    /// instances together) and `BridgeStats::unknown_key_writers` for how
    /// it is counted, not silently accepted.
    #[allow(clippy::too_many_lines)]
    pub async fn add_topic(&self, spec: TopicSpec) -> Result<String, BridgeError> {
        let existing = self.map.lock().ok().and_then(|m| {
            m.by_topic(&spec.dds_topic)
                .map(|e| (e.key_expr.clone(), e.dds_to_zenoh, e.zenoh_to_dds))
        });
        let (key_expr, has_forward, has_reverse) = match existing.clone() {
            Some(e) => e,
            None => {
                let key_expr = match &spec.key_expr_override {
                    Some(ke) => {
                        mapping::validate_key_expr_segment(ke).map_err(|e| {
                            BridgeError::InvalidKeyExpr(format!("key_expr_override: {e}"))
                        })?;
                        ke.clone()
                    }
                    None => mapping::key_expr_for_topic(
                        self.scope.as_deref(),
                        &spec.partition,
                        &spec.dds_topic,
                    )
                    .map_err(|e| BridgeError::InvalidKeyExpr(e.to_string()))?,
                };
                (key_expr, false, false)
            }
        };
        let forward = spec.direction.wants_dds_to_zenoh() && !has_forward;
        let reverse = spec.direction.wants_zenoh_to_dds() && !has_reverse;

        let transient_local = mapping::is_transient_local(spec.durability);

        if forward {
            self.wire_forward(&spec, &key_expr, transient_local).await?;
        }
        if reverse {
            self.wire_reverse(&spec, &key_expr, transient_local).await?;
        }

        if existing.is_some() {
            if let Ok(mut map) = self.map.lock() {
                map.widen(&spec.dds_topic, forward, reverse);
            }
            return Ok(key_expr);
        }
        if let Ok(mut map) = self.map.lock() {
            map.add(TopicMapEntry {
                topic: spec.dds_topic.clone(),
                type_name: spec.dds_type.clone(),
                key_expr: key_expr.clone(),
                reliability: if spec.reliable {
                    zerodds_qos::ReliabilityKind::Reliable
                } else {
                    zerodds_qos::ReliabilityKind::BestEffort
                },
                durability: spec.durability,
                dds_to_zenoh: spec.direction.wants_dds_to_zenoh(),
                zenoh_to_dds: spec.direction.wants_zenoh_to_dds(),
                keyed: spec.keyed,
            });
        }
        Ok(key_expr)
    }

    /// DDS → Zenoh: register the user reader, spawn the forward pump, and —
    /// for `TransientLocal` — declare the `PublicationCache` late-joiner
    /// queryable on the same key.
    async fn wire_forward(
        &self,
        spec: &TopicSpec,
        key_expr: &str,
        transient_local: bool,
    ) -> Result<(), BridgeError> {
        let (_eid, rx) = self
            .runtime
            .register_user_reader_kind(
                UserReaderConfig {
                    topic_name: spec.dds_topic.clone(),
                    type_name: spec.dds_type.clone(),
                    reliable: spec.reliable,
                    durability: spec.durability,
                    partition: if spec.partition.is_empty() {
                        Vec::new()
                    } else {
                        alloc::vec![spec.partition.clone()]
                    },
                    ..UserReaderConfig::default()
                },
                spec.keyed,
            )
            .map_err(|e| {
                self.stats.topics_rejected.fetch_add(1, Ordering::Relaxed);
                BridgeError::Dds(format!("register reader for '{}': {e:?}", spec.dds_topic))
            })?;

        if transient_local {
            #[allow(deprecated)] // zenoh-ext's PublicationCache/SessionExt: see module docs —
            // this is the mechanism `zenoh-plugin-dds` itself uses for the
            // TRANSIENT_LOCAL / late-joiner path, so wire compatibility means
            // using the same (now-deprecated-in-favour-of-AdvancedPublisher)
            // API rather than the newer one, which a real `zenoh-bridge-dds`
            // peer would not recognise.
            let cache = {
                use zenoh_ext::SessionExt as _;
                self.session
                    .declare_publication_cache(key_expr)
                    .history(spec.history_depth.max(1))
                    .queryable_suffix(alloc::format!("@dds_pub_cache/{}", self.session.zid()))
                    .queryable_allowed_origin(Locality::Remote)
                    .await
                    .map_err(|e| BridgeError::ZenohSession(format!("publication cache: {e}")))?
            };
            if let Ok(mut caches) = self.pub_caches.lock() {
                caches.push(cache);
            }
        }

        let reliable = spec.reliable;
        let congestion_blocking = self.reliable_routes_blocking;
        let cc = if mapping::congestion_control(reliable, congestion_blocking)
            == mapping::ZenohCongestion::Block
        {
            ZCongestionControl::Block
        } else {
            ZCongestionControl::Drop
        };
        let params = ForwardPumpParams {
            rx,
            session: Arc::clone(&self.session),
            key_expr: key_expr.to_string(),
            ext: spec.extensibility,
            cc,
            stop: Arc::clone(&self.stop),
            stats: Arc::clone(&self.stats),
            topic: spec.dds_topic.clone(),
            type_resolution_uncertain: spec.type_resolution_uncertain,
            runtime: Arc::clone(&self.runtime),
            reverse_writers: Arc::clone(&self.reverse_writers),
        };
        let handle = thread::Builder::new()
            .name(alloc::format!("zenoh-fwd-{}", spec.dds_topic))
            .spawn(move || forward_pump(&params))
            .map_err(|e| BridgeError::Dds(format!("spawn forward pump: {e}")))?;
        if let Ok(mut threads) = self.forward_threads.lock() {
            threads.push(handle);
        }
        Ok(())
    }

    /// Zenoh → DDS: register the user writer and declare the reverse
    /// subscriber — a `FetchingSubscriber` (queries `<key>/@dds_pub_cache/*`
    /// once at declaration, matching `zenoh-plugin-dds`) for `TransientLocal`,
    /// a plain callback `Subscriber` otherwise. Both are declared on
    /// `<key_expr>/**` so a dispose (`<key_expr>/<key-hash-hex>`, see the
    /// module docs) is matched too, alongside the ordinary `<key_expr>` puts.
    ///
    /// # Keyed, but `spec.type_support` unresolved
    ///
    /// The writer is still registered `spec.keyed` (`is_keyed` below) so its
    /// GUID announces the right `entityKind` (DDSI-RTPS 2.5 §9.3.1.2 Table
    /// 9.1: `0x02` `WITH_KEY`) — a real DDS peer's own keyed reader (`0x07`)
    /// only matches a keyed writer, never a `0x03` `NO_KEY` one, so
    /// under-announcing this would silently stop the very direction this
    /// route exists for.
    ///
    /// [`reverse_deliver`]'s `type_support: None` branch then writes every
    /// incoming Zenoh sample via [`DcpsRuntime::write_user_sample_borrowed`]
    /// — no `PID_KEY_HASH` in the inline QoS. This is spec-conformant, not a
    /// shortcut: DDSI-RTPS 2.5 §8.7.10 calls the key hash a hint ("Nominally
    /// the key is part of the serialized data... When the key hash is not
    /// received by a DataReader, it should be computed from the data
    /// itself"), so a writer that lacks the type — and so cannot compute
    /// it — is still within spec omitting it; the burden is on a
    /// conformant reader to derive the key from the payload, which any real
    /// DDS reader of this topic already does for its own instance
    /// bookkeeping. What §8.7.10 does not license is sending a *wrong* key
    /// hash to save that reader the trouble — this route never does: it is
    /// either the real one (`spec.type_support: Some`, `reverse_deliver`'s
    /// other branch) or none at all.
    ///
    /// The one caller-visible consequence is this writer's own retained-
    /// sample bookkeeping for a non-`Volatile` topic
    /// (`DcpsRuntime::write_user_sample_with_key`'s `slot.retained`, read by
    /// a late-joining *local* reader or the durability service): with no
    /// known key, every sample folds under the same all-zero fallback
    /// bucket a genuinely keyless topic also uses, and that bucket's normal
    /// per-instance `KEEP_LAST(1)` would evict an older sample of the
    /// bucket whenever *any* instance writes again — silently discarding a
    /// *different* real instance's only retained sample, which a real
    /// per-instance `KEEP_LAST` would never have touched. [`DcpsRuntime::
    /// set_user_writer_history_depth`] already models `KEEP_ALL` as
    /// `usize::MAX` (see its doc comment) — no new DCPS API is needed for
    /// this — but plain `KEEP_ALL` has no bound, so this writer instead gets
    /// [`UNKNOWN_KEY_HISTORY_DEPTH`]: KEEP_ALL bounded by a resource limit,
    /// per the same trade-off `DcpsRuntime` itself already documents for
    /// `HistoryKind::KeepLast` vs `KeepAll` (`max_samples` still applies to
    /// the cache as a whole). Only matters for a non-`Volatile` topic; the
    /// measured bridging scenario this task fixes is `Volatile`, where
    /// `slot.retained` is not consulted at all.
    async fn wire_reverse(
        &self,
        spec: &TopicSpec,
        key_expr: &str,
        transient_local: bool,
    ) -> Result<(), BridgeError> {
        let writer_eid = self
            .runtime
            .register_user_writer_kind(
                UserWriterConfig {
                    topic_name: spec.dds_topic.clone(),
                    type_name: spec.dds_type.clone(),
                    reliable: spec.reliable,
                    durability: spec.durability,
                    partition: if spec.partition.is_empty() {
                        Vec::new()
                    } else {
                        alloc::vec![spec.partition.clone()]
                    },
                    ..Default::default()
                },
                spec.keyed,
            )
            .map_err(|e| {
                self.stats.topics_rejected.fetch_add(1, Ordering::Relaxed);
                BridgeError::Dds(format!("register writer for '{}': {e:?}", spec.dds_topic))
            })?;
        let _ = self.runtime.set_user_writer_wire_extensibility(
            writer_eid,
            to_extensibility_for_repr(spec.extensibility),
        );
        // The route's type reads the instance keys: a Zenoh `delete` names
        // its instance by the key hash alone, and the writer then carries
        // the key it recorded, or the one the hash spells out.
        if let Some(support) = spec
            .type_support
            .as_ref()
            .filter(|s| spec.keyed && s.is_keyed())
        {
            let _ = self
                .runtime
                .set_user_writer_key_support(writer_eid, Arc::clone(support));
        }
        if let Ok(mut writers) = self.reverse_writers.lock() {
            writers.insert(spec.dds_topic.clone(), writer_eid);
        }
        if spec.keyed && spec.type_support.is_none() {
            self.stats
                .unknown_key_writers
                .fetch_add(1, Ordering::Relaxed);
            // See this method's doc comment: KEEP_ALL bounded by a resource
            // limit instead of the default per-instance KEEP_LAST(1), which
            // would fold every real instance's samples under one shared
            // fallback key and evict a different instance's still-needed
            // one. `set_user_writer_history_depth` already exists — no new
            // DCPS API needed.
            let _ = self
                .runtime
                .set_user_writer_history_depth(writer_eid, UNKNOWN_KEY_HISTORY_DEPTH);
            eprintln!(
                "[zerodds-zenoh-bridge] topic '{}' is keyed and bridged Zenoh->DDS with no \
                 resolved type: writing without PID_KEY_HASH (DDSI-RTPS 2.5 §8.7.10 — a hint \
                 a writer may omit; a conformant reader derives the key from the serialized \
                 data itself) and with its retained history raised to KEEP_ALL bounded at {} \
                 samples instead of per-instance KEEP_LAST, since this route cannot tell real \
                 instances apart from the wire alone; configure a type_file for the topic to \
                 get the real per-instance key hash",
                spec.dds_topic, UNKNOWN_KEY_HISTORY_DEPTH
            );
        }

        let sub_ke = alloc::format!("{key_expr}/**");
        let runtime = Arc::clone(&self.runtime);
        let stats = Arc::clone(&self.stats);
        let ke_owned = key_expr.to_string();
        let last_repr = Mutex::new((false, false));
        let type_support = spec.type_support.clone();
        let callback = move |sample: Sample| {
            reverse_deliver(
                &sample,
                &runtime,
                writer_eid,
                &ke_owned,
                &stats,
                &last_repr,
                type_support.as_deref(),
            );
        };

        if transient_local {
            #[allow(deprecated)] // see wire_forward: wire-compat with zenoh-plugin-dds.
            let fetching = {
                use zenoh::query::ReplyKeyExpr;
                use zenoh_ext::SubscriberBuilderExt as _;
                self.session
                    .declare_subscriber(sub_ke.clone())
                    .callback(callback)
                    .allowed_origin(Locality::Remote)
                    .querying()
                    .query_timeout(self.queries_timeout)
                    .query_selector(alloc::format!("{key_expr}/@dds_pub_cache/*").as_str())
                    .query_accept_replies(ReplyKeyExpr::Any)
                    .await
                    .map_err(|e| BridgeError::ZenohSession(format!("fetching subscriber: {e}")))?
            };
            if let Ok(mut subs) = self.reverse_subs.lock() {
                subs.push(ReverseSub::Fetching(fetching));
            }
        } else {
            let sub = self
                .session
                .declare_subscriber(sub_ke)
                .callback(callback)
                .allowed_origin(Locality::Remote)
                .await
                .map_err(|e| BridgeError::ZenohSession(format!("subscriber: {e}")))?;
            if let Ok(mut subs) = self.reverse_subs.lock() {
                subs.push(ReverseSub::Plain(sub));
            }
        }
        Ok(())
    }

    /// Stops every forward pump and undeclares every Zenoh subscriber /
    /// publication cache. Idempotent.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Ok(mut threads) = self.forward_threads.lock() {
            for h in threads.drain(..) {
                let _ = h.join();
            }
        }
        if let Ok(mut subs) = self.reverse_subs.lock() {
            subs.clear();
        }
        if let Ok(mut caches) = self.pub_caches.lock() {
            caches.clear();
        }
    }
}

impl Drop for ZenohBridge {
    fn drop(&mut self) {
        self.stop();
    }
}

fn to_extensibility_for_repr(ext: Extensibility) -> ExtensibilityForRepr {
    match ext {
        Extensibility::Final => ExtensibilityForRepr::Final,
        Extensibility::Appendable => ExtensibilityForRepr::Appendable,
        Extensibility::Mutable => ExtensibilityForRepr::Mutable,
    }
}

/// Bundled arguments for [`forward_pump`] — avoids an 8-argument function
/// signature (the same reasoning `crates/routing-service/src/forwarding.rs`'s
/// `SessionParts` documents).
struct ForwardPumpParams {
    rx: std::sync::mpsc::Receiver<UserSample>,
    session: Arc<Session>,
    key_expr: String,
    ext: Extensibility,
    cc: ZCongestionControl,
    stop: Arc<AtomicBool>,
    stats: Arc<BridgeStats>,
    topic: String,
    /// See [`TopicSpec::type_resolution_uncertain`].
    type_resolution_uncertain: bool,
    runtime: Arc<DcpsRuntime>,
    /// See [`ZenohBridge::reverse_writers`].
    reverse_writers: Arc<Mutex<BTreeMap<String, EntityId>>>,
}

/// DDS → Zenoh forward pump: reads `UserSample`s until `stop` is set,
/// prepends the RTPS encapsulation header and `put`s (alive) or `delete`s
/// (dispose/unregister — see the module docs) on Zenoh.
fn forward_pump(p: &ForwardPumpParams) {
    use std::sync::mpsc::RecvTimeoutError;
    use zenoh::Wait as _;
    let ForwardPumpParams {
        rx,
        session,
        key_expr,
        ext,
        cc,
        stop,
        stats,
        topic,
        type_resolution_uncertain,
        runtime,
        reverse_writers,
    } = p;
    // Filed once per topic on the first lifecycle sample this route forwards
    // with an unresolved-type key hash (see `TopicSpec::type_resolution_uncertain`)
    // — every occurrence is still counted in `stats.lifecycle_type_unresolved`,
    // this flag only bounds the log line to one per topic instead of one per
    // dispose.
    let mut reported_unresolved_lifecycle = false;
    while !stop.load(Ordering::Relaxed) {
        let sample = match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(s) => s,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        match sample {
            UserSample::Alive {
                payload,
                representation,
                big_endian,
                key_hash,
                ..
            } => {
                let reverse_writer = reverse_writers
                    .lock()
                    .ok()
                    .and_then(|writers| writers.get(topic).copied());
                if let Some(writer_eid) = reverse_writer {
                    let _ = runtime.remember_user_writer_instance(
                        writer_eid,
                        key_hash,
                        payload.as_slice(),
                        representation,
                        big_endian,
                    );
                }
                let xcdr2 = representation != 0;
                let header = mapping::encapsulation_header(xcdr2, big_endian, *ext);
                let mut body = Vec::with_capacity(4 + payload.as_slice().len());
                body.extend_from_slice(&header);
                body.extend_from_slice(payload.as_slice());
                match session
                    .put(key_expr.as_str(), body)
                    .congestion_control(*cc)
                    .wait()
                {
                    Ok(()) => {
                        stats.dds_to_zenoh.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(e) => {
                        stats.forward_errors.fetch_add(1, Ordering::Relaxed);
                        eprintln!(
                            "[zerodds-zenoh-bridge] dds->zenoh put failed topic={topic} key={key_expr}: {e}"
                        );
                    }
                }
            }
            UserSample::Lifecycle { key_hash, .. } => {
                // See `TopicSpec::type_resolution_uncertain`: on this route
                // `keyed: false` is this bridge's own fallback for a type it
                // could not resolve, not an operator assertion of
                // keylessness — this dispose's key hash may be the fallback
                // DCPS computed for the same reason (a genuine PID_KEY_HASH
                // on the wire is used regardless of a reader's own keyed
                // status, so it may equally be a real one; this bridge
                // cannot tell which from `UserSample::Lifecycle` alone).
                // Never dropped either way — counted and reported once per
                // topic so it never blends silently into ordinary traffic.
                if *type_resolution_uncertain {
                    stats
                        .lifecycle_type_unresolved
                        .fetch_add(1, Ordering::Relaxed);
                    if !reported_unresolved_lifecycle {
                        reported_unresolved_lifecycle = true;
                        eprintln!(
                            "[zerodds-zenoh-bridge] observability: topic={topic} forwarded a \
                             dispose/unregister on a route bridged keyless because its type \
                             could not be resolved at discovery time — its key hash may be a \
                             fallback, not verified against the real type (further occurrences \
                             on this topic are counted in lifecycle_type_unresolved_total, not \
                             logged again)"
                        );
                    }
                }
                let ke = alloc::format!("{key_expr}/{}", hex_encode(&key_hash));
                match session.delete(ke.as_str()).congestion_control(*cc).wait() {
                    Ok(()) => {
                        stats.dds_to_zenoh.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(e) => {
                        stats.forward_errors.fetch_add(1, Ordering::Relaxed);
                        eprintln!(
                            "[zerodds-zenoh-bridge] dds->zenoh dispose failed topic={topic} key={ke}: {e}"
                        );
                    }
                }
            }
            // No user data, and no Zenoh equivalent — RTPS wire-internal only.
            UserSample::CoherentSetEnd { .. } => {}
        }
    }
}

/// Zenoh → DDS reverse delivery, shared by the plain and the `FetchingSubscriber`
/// callback. `last_repr` avoids a redundant representation/byte-order override
/// call when consecutive samples share it (mirrors
/// `crates/routing-service/src/forwarding.rs` fn `set_rep`).
fn reverse_deliver(
    sample: &Sample,
    runtime: &DcpsRuntime,
    writer_eid: EntityId,
    key_expr: &str,
    stats: &BridgeStats,
    last_repr: &Mutex<(bool, bool)>,
    type_support: Option<&zerodds_dcps::serialized_type::SerializedTypeSupport>,
) {
    let ke = sample.key_expr().as_str();
    if sample.kind() == SampleKind::Delete {
        // Dispose: `<key_expr>/<key-hash-hex>` (see the module docs). A
        // bare delete on `key_expr` itself names no instance — refused, not
        // guessed, and counted rather than silently ignored.
        let Some(suffix) = ke.strip_prefix(key_expr).and_then(|s| s.strip_prefix('/')) else {
            stats.forward_errors.fetch_add(1, Ordering::Relaxed);
            eprintln!(
                "[zerodds-zenoh-bridge] zenoh delete on {ke} carries no key-hash suffix; dropped as a rejected, not silent, sample"
            );
            return;
        };
        match hex_decode_16(suffix) {
            Some(key_hash) => {
                use zerodds_rtps::inline_qos::status_info::DISPOSED;
                // A Zenoh `delete` names the instance by this hex key hash
                // alone (see the module docs cited above), so no key
                // holder is passed. With the route's type set on the
                // writer (`wire_reverse`), the writer takes the key it
                // recorded — from a sample this bridge wrote to DDS, or one
                // the forward pump carried to Zenoh — or reads it from a
                // key hash that spells it out (a key within 16 octets), and
                // the dispose carries the serializedKey Cyclone, Fast DDS
                // and OpenDDS need. Otherwise it goes out as the hash alone
                // and the writer reports that once.
                match runtime.write_user_lifecycle(writer_eid, key_hash, None, DISPOSED, None) {
                    Ok(()) => {
                        stats.zenoh_to_dds.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(e) => {
                        stats.forward_errors.fetch_add(1, Ordering::Relaxed);
                        eprintln!(
                            "[zerodds-zenoh-bridge] zenoh->dds dispose failed key={ke}: {e:?}"
                        );
                    }
                }
            }
            None => {
                stats.forward_errors.fetch_add(1, Ordering::Relaxed);
                eprintln!(
                    "[zerodds-zenoh-bridge] zenoh delete on {ke} has a malformed key-hash suffix; dropped as a rejected, not silent, sample"
                );
            }
        }
        return;
    }

    if ke != key_expr {
        // A deeper key that isn't a recognised dispose suffix — refused.
        stats.forward_errors.fetch_add(1, Ordering::Relaxed);
        eprintln!(
            "[zerodds-zenoh-bridge] unmapped zenoh key {ke} under {key_expr}; sample rejected, not silently dropped"
        );
        return;
    }

    let bytes = sample.payload().to_bytes();
    match mapping::decode_encapsulation_header(&bytes) {
        Ok((decoded, body)) => {
            // The RTPS DataRepresentation id (RTPS 2.5 PID_DATA_REPRESENTATION):
            // 0 = XCDR1, 2 = XCDR2 — not the 10-value representation-identifier
            // `representation_id` names; only these two ids are valid overrides.
            let rep_id: i16 = if decoded.xcdr2 { 2 } else { 0 };
            if let Ok(mut last) = last_repr.lock() {
                if *last != (decoded.xcdr2, decoded.big_endian) {
                    let _ = runtime
                        .set_user_writer_data_rep_override(writer_eid, Some(alloc::vec![rep_id]));
                    let _ =
                        runtime.set_user_writer_byte_order_override(writer_eid, decoded.big_endian);
                    *last = (decoded.xcdr2, decoded.big_endian);
                }
            }
            // Keyed topic: derive the real per-instance key hash from the
            // decoded body (see the `TopicSpec::keyed`/`type_support` docs) —
            // `write_user_sample_borrowed` alone would collapse every
            // instance onto the one default key.
            let write_result = match type_support {
                Some(support) => support
                    .read_body(body, rep_id as u8, decoded.big_endian)
                    .map_err(BridgeError::Dds)
                    .and_then(|sample| {
                        let (hash, _holder) = support.key_material(&sample).ok_or_else(|| {
                            BridgeError::Dds("keyed type_support returned no key".into())
                        })?;
                        runtime
                            .write_user_sample_keyed(writer_eid, body, hash, None)
                            .map_err(|e| BridgeError::Dds(format!("{e:?}")))
                    }),
                None => runtime
                    .write_user_sample_borrowed(writer_eid, body)
                    .map_err(|e| BridgeError::Dds(format!("{e:?}"))),
            };
            match write_result {
                Ok(()) => {
                    stats.zenoh_to_dds.fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    stats.forward_errors.fetch_add(1, Ordering::Relaxed);
                    eprintln!("[zerodds-zenoh-bridge] zenoh->dds write failed key={ke}: {e}");
                }
            }
        }
        Err(mapping::InvalidEncapsulationHeader) => {
            stats.forward_errors.fetch_add(1, Ordering::Relaxed);
            eprintln!(
                "[zerodds-zenoh-bridge] zenoh sample on {ke} has no valid RTPS encapsulation header ({} bytes); rejected, not silently dropped",
                bytes.len()
            );
        }
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&alloc::format!("{b:02x}"));
    }
    out
}

fn hex_decode_16(s: &str) -> Option<[u8; 16]> {
    if s.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        let bytes: [u8; 16] = [
            0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55,
            0x66, 0x77,
        ];
        let hex = hex_encode(&bytes);
        assert_eq!(hex.len(), 32);
        assert_eq!(hex_decode_16(&hex), Some(bytes));
    }

    #[test]
    fn hex_decode_rejects_bad_length_and_chars() {
        assert_eq!(hex_decode_16("00112233"), None);
        assert_eq!(hex_decode_16(&"zz".repeat(16)), None);
    }

    #[test]
    fn builder_defaults_to_plugin_compatible_scope() {
        let b = ZenohBridgeBuilder::new();
        assert_eq!(b.scope, None);
        assert!(b.reliable_routes_blocking);
    }

    #[test]
    fn topic_spec_default_is_bidir_reliable_volatile() {
        let spec = TopicSpec::default();
        assert_eq!(spec.direction, Direction::Both);
        assert!(spec.reliable);
        assert_eq!(spec.durability, DurabilityKind::Volatile);
        assert_eq!(spec.extensibility, Extensibility::Final);
    }
}
