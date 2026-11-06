// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Standalone DDS Durability-Service daemon (ADR 0009).
//!
//! Crate `zerodds-durability-service`. Safety classification: **STANDARD**.
//!
//! From an RTPS point of view the service is just another participant. For
//! each served topic it:
//!
//! 1. **Ingests** via a `RELIABLE / KEEP_ALL` reader — it receives every
//!    sample the application writers publish. The reader of
//!    [`serve`](DurabilityService::serve) is `TRANSIENT_LOCAL` and also takes
//!    the history of a writer that published before it matched; the reader
//!    of [`serve_typed`](DurabilityService::serve_typed) is `VOLATILE` and
//!    matches writers of every durability kind.
//! 2. **Stores** each sample in a [`DurabilityStore`] (sqlite/file/in-memory)
//!    with the representation identifier of the header it came under
//!    (DDSI-RTPS 2.5 §10.5, DDS-XTypes 1.3 Table 60).
//! 3. **Replays** via a `PERSISTENT` writer — a late-joining reader matches
//!    it and the standard RTPS path delivers the history, each sample under
//!    the representation identifier it was stored with. This keeps working
//!    after the original writer's process has died.
//! 4. **Startup-sync**: on [`serve`](DurabilityService::serve) the replay
//!    writer is primed from the store, so `PERSISTENT` history survives a full
//!    service restart with no application writer present.
//!
//! ## Resuming a served topic after a restart
//! `serve`/`serve_typed` persist what
//! serving a topic needs — its type name, keyedness, and retention contract —
//! via [`DurabilityStore::set_topic_meta`] the moment they first serve it.
//! `serve_if_stored` reads that back: if the store
//! already has [`zerodds_durability_store::TopicMeta`] for a topic (from an
//! earlier run of this daemon, or an earlier call in this one), the topic is
//! served immediately — startup-sync + ingest reader + pump — **without
//! waiting for a writer or reader to be (re-)discovered**. This is what
//! closes the restart bug: a configured topic (`durability.topics: [NAME]`
//! with no `type` override) used to resolve its type only from
//! `serve_discovered`, which waits for a writer's
//! SEDP announcement — on a restart with no application writer running, that
//! wait never ends, so `--auto`'s sibling, an explicitly named topic, replayed
//! nothing to a late joiner even though the store still held every sample.
//! `zerodds-durability-svc`'s `main` now tries `serve_if_stored` first for
//! such a topic and falls back to `serve_discovered` only when the store has
//! no metadata for it yet (a genuinely new topic).
//!
//! **Discovered reader alone is not enough to serve.** DDS 1.4 §2.2.3.5
//! defines `DURABILITY_SERVICE` as a `Topic`/`DataWriter` QoS policy only —
//! there is no such policy on a `DataReader`; a discovered subscription
//! carries a type name and keyedness (from its own SEDP data and RTPS
//! entityKind) but never a retention contract, because retention is not a
//! reader-side concept. Without either a writer (which supplies the
//! contract) or stored [`TopicMeta`] (which already resolved one), serving
//! from a reader alone would have nothing to declare beyond the daemon's bare
//! default and, more fundamentally, nothing to replay: no writer has ever
//! published, so the store holds no history for that topic regardless. A
//! reader discovered first still gets served the moment a writer follows —
//! ordinary RTPS matching, unaffected by which side showed up first. So this
//! is not implemented: serving stays keyed to stored metadata (this section)
//! or a discovered writer (`enable_auto_discovery`
//! / `serve_discovered`).
//!
//! ## Keyed topics
//! A discovered topic's keyedness is read off the writer's RTPS `entityKind`
//! octet (DDSI-RTPS 2.5 §9.3.1.2: `0x02`/`0xC2` = WITH_KEY, `0x03`/`0xC3` =
//! NO_KEY — [`zerodds_rtps::wire_types::EntityKind::topic_kind`]), not
//! assumed. A keyed topic is stored and replayed **per instance**: `KEEP_LAST`
//! applies per instance key, and instance lifecycle (dispose/unregister) and
//! `max_instances` follow the discovered contract. The replay writer numbers
//! its own changes, and every sample with a known source goes out on behalf
//! of the writer that first wrote it: its DATA carries
//! `PID_ORIGINAL_WRITER_INFO` with that writer's GUID and sequence number
//! (DDSI-RTPS 2.5 §8.7.9), so a reader that receives the sample from the
//! original writer as well, or from another service, takes it once.
//!
//! ## The retention contract
//! [`Contract`] is derived from the discovered writer's own
//! `DurabilityServiceQosPolicy` (`PID_DURABILITY_SERVICE` in its DATA(w), DDS
//! 1.4 §2.2.3.5) via [`Contract::from_qos`] — not a daemon-wide default —
//! whenever the writer's announced policy differs from the DDS default
//! (`KEEP_LAST(1)`, all caps unlimited; that literal value is
//! indistinguishable on the wire from "the writer never touched this
//! policy"). In that indistinguishable case the daemon falls back to its own
//! configured default (`--auto`'s `default_contract`, normally `KEEP_ALL` —
//! see `zerodds-durability-svc`'s README/man page), which is rarely what an
//! application meant but is a better fit for a service whose purpose is
//! retention than the spec default is.
//!
//! An explicit per-topic `contract` in the daemon's own configuration
//! (`durability.topics[].contract`, see `zerodds-durability-svc(5)`) outranks
//! both: config > discovered QoS > daemon default.
//!
//! **Two writers, one topic, different `DurabilityServiceQosPolicy`.** DDS
//! 1.4 §2.2.3.5 does not define a merge rule for this case, and changing a
//! store's retention parameters (history depth, caps) while it already holds
//! data under the old ones is not a well-defined operation (KEEP_LAST(2)
//! shrinking to KEEP_LAST(1) mid-flight, say, would need to decide which
//! retained sample to drop with no writer-side signal for it). This service
//! therefore applies the **first** discovered writer's contract to a topic
//! and keeps it for the topic's lifetime; a later writer that announces a
//! *different* contract does not change it — the difference is reported (see
//! [`ServiceEvent::ConflictingDurabilityService`]) rather than silently
//! ignored, so the operator sees the disagreement and can resolve it (fix the
//! writers, or pin the topic's contract in config, which then wins outright).

#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};

use zerodds_dcps::builtin_topics::PublicationBuiltinTopicData;
use zerodds_dcps::runtime::{UserReaderConfig, UserSample, UserWriterConfig};
use zerodds_dcps::{
    DdsType, DomainParticipant, DomainParticipantFactory, DomainParticipantQos, RawBytes,
};
use zerodds_durability_store::{Contract, DurabilitySample, DurabilityStore, TopicMeta};
use zerodds_qos::policies::durability::DurabilityKind;
use zerodds_qos::policies::durability_service::DurabilityServiceQosPolicy;
use zerodds_qos::policies::history::HistoryKind;
use zerodds_qos::{
    DeadlineQosPolicy, LifespanQosPolicy, LivelinessKind, LivelinessQosPolicy, OwnershipKind,
};
use zerodds_rtps::wire_types::{Guid, TopicKind};

/// The most recent notable events from the daemon's background threads,
/// bounded so a pathological topic churn cannot grow it without limit.
const MAX_EVENTS: usize = 256;

/// Errors from the durability daemon.
#[derive(Debug)]
pub enum ServiceError {
    /// A DCPS/RTPS operation failed.
    Dcps(zerodds_dcps::DdsError),
    /// A storage operation failed.
    Store(zerodds_durability_store::StoreError),
    /// An internal lock was poisoned.
    Poisoned(&'static str),
}

impl core::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Dcps(e) => write!(f, "durability service: dcps: {e}"),
            Self::Store(e) => write!(f, "durability service: store: {e}"),
            Self::Poisoned(w) => write!(f, "durability service: poisoned: {w}"),
        }
    }
}
impl core::error::Error for ServiceError {}
impl From<zerodds_dcps::DdsError> for ServiceError {
    fn from(e: zerodds_dcps::DdsError) -> Self {
        Self::Dcps(e)
    }
}
impl From<zerodds_durability_store::StoreError> for ServiceError {
    fn from(e: zerodds_durability_store::StoreError) -> Self {
        Self::Store(e)
    }
}

/// Result alias.
pub type Result<T> = core::result::Result<T, ServiceError>;

/// A notable event from the daemon's background threads, surfaced without
/// requiring a log scrape — via [`DurabilityService::events`] — and logged
/// (stderr, one line) as it happens. Bounded to the most recent
/// `MAX_EVENTS`.
#[derive(Debug, Clone)]
pub enum ServiceEvent {
    /// `enable_auto_discovery` (or [`DurabilityService::serve_discovered`])
    /// found a matching writer but [`DurabilityService::serve_typed`] failed
    /// for it — the topic is **not** served. Previously dropped without a
    /// word (`let _ = me.serve_typed(...)`).
    AutoServeFailed {
        /// Topic name.
        topic: String,
        /// Discovered IDL type name.
        type_name: String,
        /// What `serve_typed` returned.
        error: String,
    },
    /// A writer of an already-served topic announced a
    /// `DurabilityServiceQosPolicy` whose derived [`Contract`] differs from
    /// the one already in force (set by the first writer discovered for this
    /// topic — see the module docs, "Two writers, one topic"). The first
    /// writer's contract keeps governing the topic; `announced` is reported,
    /// not applied.
    ConflictingDurabilityService {
        /// Topic name.
        topic: String,
        /// The contract already in force for this topic.
        in_force: Contract,
        /// The differing contract the later writer announced.
        announced: Contract,
    },
    /// The replay writer rejected a write the daemon otherwise accepted (a
    /// stored sample it primed at startup, a lifecycle marker forwarded from
    /// the ingest pump, or a just-stored sample it grew the writer's history
    /// with). A late joiner may now miss this sample or lifecycle
    /// transition — previously dropped without a word (`let _ =
    /// rrt.write_user_sample_keyed(...)` / `write_user_lifecycle(...)`).
    ReplayWriteFailed {
        /// Topic name.
        topic: String,
        /// What kind of replay write failed: `"startup-sync sample"`,
        /// `"lifecycle marker"` or `"sample"`.
        what: &'static str,
        /// What the runtime call returned.
        error: String,
    },
    /// A topic was served immediately from persisted [`TopicMeta`] the store
    /// already had for it (module docs, "Resuming a served topic after a
    /// restart") — no writer or reader had to be discovered first.
    ServedFromStoredMeta {
        /// Topic name.
        topic: String,
        /// The persisted type name it is served under.
        type_name: String,
        /// The persisted keyedness it is served under.
        keyed: bool,
    },
    /// A topic already being served (from stored [`TopicMeta`] or from an
    /// earlier discovered writer) saw a writer announce a DIFFERENT type
    /// name or keyedness than what it is actually served under. Not
    /// applied — the topic keeps its original type/keyedness (never a
    /// silent override) — reported so the operator sees the disagreement.
    TopicMetaMismatch {
        /// Topic name.
        topic: String,
        /// Type name the topic is actually served under.
        served_type_name: String,
        /// Keyedness the topic is actually served under.
        served_keyed: bool,
        /// Type name the later writer announced.
        announced_type_name: String,
        /// Keyedness the later writer announced.
        announced_keyed: bool,
    },
}

impl core::fmt::Display for ServiceEvent {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::AutoServeFailed {
                topic,
                type_name,
                error,
            } => write!(
                f,
                "auto-discovery: serve failed for topic={topic} type={type_name}: {error}"
            ),
            Self::ConflictingDurabilityService {
                topic,
                in_force,
                announced,
            } => write!(
                f,
                "topic={topic}: a writer announced a DurabilityServiceQosPolicy \
                 (contract {announced:?}) that differs from the one already in force \
                 ({in_force:?}); the first writer's contract keeps governing this topic"
            ),
            Self::ReplayWriteFailed { topic, what, error } => write!(
                f,
                "topic={topic}: replay writer rejected a {what} write: {error}"
            ),
            Self::ServedFromStoredMeta {
                topic,
                type_name,
                keyed,
            } => write!(
                f,
                "topic={topic}: served immediately from stored metadata \
                 (type={type_name} keyed={keyed}), no writer/reader discovered first"
            ),
            Self::TopicMetaMismatch {
                topic,
                served_type_name,
                served_keyed,
                announced_type_name,
                announced_keyed,
            } => write!(
                f,
                "topic={topic}: a writer announced type={announced_type_name} \
                 keyed={announced_keyed}, which differs from what this topic is \
                 actually served under (type={served_type_name} keyed={served_keyed}); \
                 the topic keeps its original type/keyedness"
            ),
        }
    }
}

/// Whether a discovered writer's endpoint GUID carries the RTPS
/// `entityKind` octet for a keyed topic (DDSI-RTPS 2.5 §9.3.1.2:
/// `UserWriterWithKey`/`BuiltinWriterWithKey` = `0x02`/`0xC2`). `false` for
/// `NoKey` (`0x03`/`0xC3`) and for an octet the wire spec does not name
/// (`EntityKind::Other`, e.g. a vendor's own extension), which cannot be a
/// keyed writer's per Table 9.1.
fn writer_is_keyed(guid: &Guid) -> bool {
    matches!(
        guid.entity_id.entity_kind.topic_kind(),
        Some(TopicKind::WithKey)
    )
}

/// The contract to serve a discovered topic under: the writer's own
/// `DurabilityServiceQosPolicy` via [`Contract::from_qos`], unless that
/// policy is exactly the DDS default (`KEEP_LAST(1)`, all caps unlimited) —
/// indistinguishable on the wire from "the writer never touched this
/// policy" — in which case the daemon's own `default_contract` applies
/// instead (module docs, "The retention contract").
fn effective_contract(
    announced: DurabilityServiceQosPolicy,
    default_contract: Contract,
) -> Contract {
    if announced == DurabilityServiceQosPolicy::default() {
        default_contract
    } else {
        Contract::from_qos(&announced)
    }
}

struct Served {
    name: String,
    /// The contract this topic is being served under — the first writer
    /// discovered for it decides this (module docs, "Two writers, one
    /// topic"); kept here (not re-read from the store) so a later writer's
    /// differing announcement can be recognised and reported.
    contract: Contract,
    /// The type name this topic is served under. Empty for the
    /// `<auto-discovery>`/`<discover:NAME>` bookkeeping entries, which are
    /// never looked up by a real topic name.
    type_name: String,
    /// The keyedness this topic is served under.
    keyed: bool,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

/// The durability daemon. Uses TWO participants per domain — one for ingest
/// (reader) and one for replay (writer) — that mutually **ignore each other**.
/// This is set up before any endpoint exists, so the ingest reader can never
/// match the replay writer (race-free echo-loop prevention). Both serve any
/// number of TRANSIENT/PERSISTENT topics over a single [`DurabilityStore`].
pub struct DurabilityService {
    ingest: DomainParticipant,
    replay: DomainParticipant,
    store: Arc<dyn DurabilityStore>,
    served: Mutex<Vec<Served>>,
    /// `Arc`-wrapped (not a bare `Mutex`) so a pump thread — spawned from
    /// `&self`, not `Arc<Self>` — can still report a replay-write failure
    /// via its own clone, without needing this whole service Arc-shared.
    events: Arc<Mutex<VecDeque<ServiceEvent>>>,
    cleanup_stop: Arc<AtomicBool>,
    cleanup_handle: Mutex<Option<JoinHandle<()>>>,
}

/// How often the background sweep calls [`DurabilityStore::cleanup`] to
/// actually purge an instance whose `cleanup_delay` has elapsed since it was
/// unregistered. Nothing else calls `cleanup` — without this sweep,
/// `service_cleanup_delay` (DDS 1.4 §2.2.3.5) was accepted into every
/// [`Contract`] and never acted on, so a disposed/unregistered instance's
/// samples were retained forever regardless of the writer's own QoS.
const CLEANUP_SWEEP_INTERVAL: Duration = Duration::from_secs(30);

impl DurabilityService {
    /// Starts the daemon on `domain`, backed by `store`.
    ///
    /// # Errors
    /// Participant creation failed.
    pub fn start(domain: i32, store: Arc<dyn DurabilityStore>) -> Result<Self> {
        let config = zerodds_dcps::runtime::RuntimeConfig::from_standard_paths().map_err(|e| {
            zerodds_dcps::DdsError::Configuration {
                message: e.to_string(),
            }
        })?;
        Self::start_with_config(domain, store, config)
    }

    /// [`Self::start`] with the runtime configuration a command line loaded.
    /// Both participants of the daemon use it.
    ///
    /// # Errors
    /// Participant creation failed.
    pub fn start_with_config(
        domain: i32,
        store: Arc<dyn DurabilityStore>,
        config: zerodds_dcps::runtime::RuntimeConfig,
    ) -> Result<Self> {
        let factory = DomainParticipantFactory::instance();
        let ingest = factory.create_participant_with_config(
            domain,
            DomainParticipantQos::default(),
            config.clone(),
        )?;
        let replay = factory.create_participant_with_config(
            domain,
            DomainParticipantQos::default(),
            config,
        )?;
        // Race-free: mutual participant ignore BEFORE any reader/writer exists,
        // so the ingest reader and replay writer never match each other. Use the
        // GUID-derived `participant_handle()` (the discovery/ignore key space),
        // NOT `instance_handle()` (a local allocator counter that the ignore
        // filter never sees).
        let _ = ingest.ignore_participant(replay.participant_handle());
        let _ = replay.ignore_participant(ingest.participant_handle());
        let cleanup_stop = Arc::new(AtomicBool::new(false));
        let cleanup_handle = spawn_cleanup_sweep(Arc::clone(&store), Arc::clone(&cleanup_stop))?;
        Ok(Self {
            ingest,
            replay,
            store,
            served: Mutex::new(Vec::new()),
            events: Arc::new(Mutex::new(VecDeque::new())),
            cleanup_stop,
            cleanup_handle: Mutex::new(Some(cleanup_handle)),
        })
    }

    fn served(&self) -> Result<MutexGuard<'_, Vec<Served>>> {
        self.served
            .lock()
            .map_err(|_| ServiceError::Poisoned("served topics"))
    }

    /// The most recent [`ServiceEvent`]s (oldest first), bounded to
    /// `MAX_EVENTS`. Cleared by nothing — a caller that wants to observe
    /// only new events records the length it already saw.
    ///
    /// # Errors
    /// Lock poisoned.
    pub fn events(&self) -> Result<Vec<ServiceEvent>> {
        Ok(self
            .events
            .lock()
            .map_err(|_| ServiceError::Poisoned("events"))?
            .iter()
            .cloned()
            .collect())
    }

    /// Records `event` (bounded ring) and logs it — an operator watching the
    /// daemon's stderr sees the same thing a test asserting on
    /// [`Self::events`] does.
    fn report(&self, event: ServiceEvent) {
        report_event(&self.events, event);
    }

    /// The contract currently in force for `topic_name`, or `None` if it is
    /// not served. For an auto-discovered or `serve_discovered` topic this is
    /// the discovered writer's own `DurabilityServiceQosPolicy` (module docs,
    /// "The retention contract") — useful for an operator or a test to
    /// confirm what the daemon actually resolved, without re-deriving it.
    #[must_use]
    pub fn contract_of(&self, topic_name: &str) -> Option<Contract> {
        self.served
            .lock()
            .ok()?
            .iter()
            .find(|s| s.name == topic_name)
            .map(|s| s.contract)
    }

    /// `(type_name, keyed, contract)` for `topic_name` if it is served,
    /// else `None`. Used by [`consider_discovered_writer`](Self::consider_discovered_writer)
    /// to tell a genuinely new topic (serve it) from one already served
    /// under a different type/keyedness (report the mismatch) or a
    /// different contract (report the conflict).
    fn served_meta_of(&self, topic_name: &str) -> Option<(String, bool, Contract)> {
        self.served
            .lock()
            .ok()?
            .iter()
            .find(|s| s.name == topic_name)
            .map(|s| (s.type_name.clone(), s.keyed, s.contract))
    }

    /// Begins serving `topic_name` of the native type `zerodds::RawBytes`
    /// (unkeyed) with the given retention `contract`. Creates the ingest
    /// reader + replay writer, primes the writer from the store
    /// (startup-sync), and spawns the ingest pump.
    ///
    /// The ingest reader is TRANSIENT_LOCAL, so a writer that published
    /// before the service matched it hands over its history; the pump keeps
    /// each sample once by its writer and sequence number. It runs on the
    /// runtime-level path of [`serve_typed`](Self::serve_typed), which keeps
    /// the representation identifier each sample came under.
    ///
    /// # Errors
    /// Topic/reader/writer creation or the initial store/replay failed.
    pub fn serve(&self, topic_name: &str, contract: Contract) -> Result<()> {
        self.serve_runtime(
            topic_name,
            RawBytes::TYPE_NAME,
            false,
            contract,
            DurabilityKind::TransientLocal,
        )
    }

    /// Like [`serve`](Self::serve) but for a topic whose application type-name
    /// is `type_name` rather than the native `zerodds::RawBytes` — e.g. a
    /// foreign-vendor `TRANSIENT` writer (Cyclone/FastDDS/OpenDDS) discovered on
    /// the wire. The typed `RawBytes` topic is locked to `zerodds::RawBytes`, so
    /// it would never match a foreign writer under `use_xtypes=no`
    /// (match keyed on `topic_name` + `type_name`). This path uses the
    /// **runtime-level** user-entity API so the ingest reader + replay writer
    /// register under the *discovered* `type_name` (and the matching
    /// keyed/no-key kind) — exactly how the byte-oriented C-FFI achieves
    /// cross-vendor matching.
    ///
    /// Echo is avoided structurally: the ingest reader lives on the `ingest`
    /// participant and the replay writer on the `replay` participant, which
    /// mutually `ignore_participant` each other (set in [`start`](Self::start)).
    ///
    /// `keyed` must match the foreign writer's key kind (RTPS entityKind, Spec
    /// §9.3.1.2). Increment-1 scope is unkeyed; `false` is the safe default.
    ///
    /// # Errors
    /// Participant runtime unavailable, reader/writer registration failed, or
    /// the initial store read / replay-prime failed.
    pub fn serve_typed(
        &self,
        topic_name: &str,
        type_name: &str,
        keyed: bool,
        contract: Contract,
    ) -> Result<()> {
        self.serve_runtime(
            topic_name,
            type_name,
            keyed,
            contract,
            DurabilityKind::Volatile,
        )
    }

    /// [`serve_typed`](Self::serve_typed) with the durability its ingest
    /// reader requests.
    fn serve_runtime(
        &self,
        topic_name: &str,
        type_name: &str,
        keyed: bool,
        contract: Contract,
        reader_durability: DurabilityKind,
    ) -> Result<()> {
        // Persist what a restart needs to resume serving this topic without
        // waiting for a writer/reader to be re-discovered (module docs,
        // "Resuming a served topic after a restart").
        self.store.set_topic_meta(
            topic_name,
            TopicMeta {
                type_name: type_name.to_string(),
                keyed,
                contract,
            },
        )?;

        let irt = self
            .ingest
            .runtime()
            .ok_or(ServiceError::Poisoned("ingest runtime"))?
            .clone();
        let rrt = self
            .replay
            .runtime()
            .ok_or(ServiceError::Poisoned("replay runtime"))?
            .clone();

        let (_reid, rx) = irt
            .register_user_reader_kind(
                UserReaderConfig {
                    durability: reader_durability,
                    ..user_reader_cfg(topic_name, type_name)
                },
                keyed,
            )
            .map_err(|_| ServiceError::Poisoned("register ingest reader"))?;
        // `UserWriterConfig` (the runtime-level path this method uses) has no
        // HistoryQosPolicy field of its own — a fixed `KEEP_ALL` at
        // registration, unlike a typed `DataWriter`'s `create_datawriter`,
        // which threads it through directly. Without this call the replay
        // writer's own RTPS history cache stayed KEEP_ALL regardless of the
        // contract: a late joiner replayed every historical sample of every
        // instance, not the contracted KEEP_LAST(N) per instance the store
        // itself was already correctly enforcing (measured: KEEP_LAST(2)
        // with 3 writes to one instance replayed writes 1 and 2 — insertion
        // order, not depth — because the writer's cache still held all 3).
        let mut writer = ReplayWriter {
            rt: Arc::clone(&rrt),
            topic_name: topic_name.to_string(),
            type_name: type_name.to_string(),
            keyed,
            depth: match contract.history_kind {
                HistoryKind::KeepAll => usize::MAX,
                HistoryKind::KeepLast => contract.effective_depth(),
            },
            eid: None,
        };

        // Startup-sync: prime the replay writer's history from durable storage.
        // Each stored sample goes out under the representation identifier it
        // came under (DDSI-RTPS 2.5 §10.5, DDS-XTypes 1.3 Table 60): its XCDR
        // version, its format — plain, delimited or parameter list, which
        // follows the extensibility of the writer's type — and its byte order.
        let mut last_identifier: Option<[u8; 2]> = None;
        for sample in self.store.replay_for_topic(topic_name)? {
            let weid = match writer.for_identifier(sample.representation_identifier) {
                Ok(weid) => weid,
                Err(e) => {
                    self.report(ServiceEvent::ReplayWriteFailed {
                        topic: topic_name.to_string(),
                        what: "startup-sync sample",
                        error: e.to_string(),
                    });
                    continue;
                }
            };
            if let Err(e) = set_form(
                &rrt,
                weid,
                &mut last_identifier,
                sample.representation_identifier,
            ) {
                self.report(ServiceEvent::ReplayWriteFailed {
                    topic: topic_name.to_string(),
                    what: "startup-sync sample",
                    error: e.to_string(),
                });
                continue;
            }
            // The stored `instance_key` (real for a keyed topic, all-zero for
            // an unkeyed one) — not the unkeyed convenience write, which
            // would fold every instance into the replay writer's single
            // default one and break per-instance KEEP_LAST/late-join replay.
            if let Err(e) = write_replay_sample(
                &rrt,
                weid,
                &sample.payload,
                sample.instance_key,
                (sample.source_guid, sample.source_sequence),
                None,
            ) {
                self.report(ServiceEvent::ReplayWriteFailed {
                    topic: topic_name.to_string(),
                    what: "startup-sync sample",
                    error: e.to_string(),
                });
            }
        }

        let stop = Arc::new(AtomicBool::new(false));
        let pump_stop = Arc::clone(&stop);
        let store = Arc::clone(&self.store);
        let topic_owned = topic_name.to_string();
        let pump_rrt = Arc::clone(&rrt);
        let pump_events = Arc::clone(&self.events);
        let seq = AtomicU64::new(self.store.stats(topic_name)?.samples as u64);

        let handle = std::thread::Builder::new()
            .name(format!("durability-pump-{topic_name}"))
            .spawn(move || {
                // Representation-faithful replay: the ingested `payload` is the
                // body in the SOURCE writer's form (a foreign FINAL-type writer
                // emits XCDR1, an @appendable or @mutable type's writer
                // D_CDR2 or PL_CDR2). The replay writer sends it under the
                // header it came under; the identifier is set when it changes.
                let mut last_identifier = last_identifier;
                // Seed the source-identity dedup set from what is already
                // durable, so a restart's re-received TransientLocal history is
                // recognised and not re-stored.
                let mut seen: std::collections::HashSet<([u8; 16], i64)> = store
                    .replay_for_topic(&topic_owned)
                    .map(|v| {
                        v.into_iter()
                            .filter(|s| s.source_sequence >= 0)
                            .map(|s| (s.source_guid, s.source_sequence))
                            .collect()
                    })
                    .unwrap_or_default();
                // Event-driven: block on the reader's channel (no busy-poll),
                // waking on each sample or the 200ms stop-flag re-check.
                while !pump_stop.load(Ordering::Relaxed) {
                    let sample = match rx.recv_timeout(Duration::from_millis(200)) {
                        Ok(s) => s,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    let (
                        payload,
                        representation_identifier,
                        writer_guid,
                        source_seq,
                        key_hash,
                        source_timestamp,
                    ) = match sample {
                        UserSample::Alive {
                            key_hash,
                            payload,
                            representation_identifier,
                            writer_guid,
                            source_sequence_number,
                            source_timestamp,
                            ..
                        } => (
                            payload.to_vec(),
                            representation_identifier,
                            writer_guid,
                            source_sequence_number,
                            // `None` for a NO_KEY topic — the single
                            // implicit instance (DDS 1.4 §2.2.2.3.14).
                            key_hash.unwrap_or([0u8; 16]),
                            source_timestamp,
                        ),
                        // Dispose / unregister (DDS 1.4 §2.2.2.5.1.13):
                        // mark the instance unregistered in the store —
                        // its samples become eligible for purge after
                        // `contract.cleanup_delay` — and forward the
                        // marker to the replay writer so a late joiner
                        // sees the same instance-state transition. This
                        // was previously dropped unconditionally, which
                        // for a KEYED topic left every disposed/
                        // unregistered instance retained forever
                        // (`max_instances`/`cleanup_delay` had no way to
                        // ever apply) and for an unkeyed one left the
                        // service never noticing a writer had gone away.
                        UserSample::Lifecycle {
                            key_hash,
                            kind,
                            key_holder,
                            source_timestamp,
                            ..
                        } => {
                            let _ = store.unregister(&topic_owned, &key_hash, SystemTime::now());
                            if let Err(e) = writer.any().and_then(|weid| {
                                pump_rrt.write_user_lifecycle(
                                    weid,
                                    key_hash,
                                    key_holder.as_deref(),
                                    lifecycle_status_bits(kind),
                                    source_timestamp,
                                )
                            }) {
                                report_event(
                                    &pump_events,
                                    ServiceEvent::ReplayWriteFailed {
                                        topic: topic_owned.clone(),
                                        what: "lifecycle marker",
                                        error: e.to_string(),
                                    },
                                );
                            }
                            continue;
                        }
                        // The end-of-coherent-set marker (DDSI-RTPS 2.5
                        // §8.7.6) carries no user data and no source
                        // identity — nothing to store or replay.
                        UserSample::CoherentSetEnd { .. } => continue,
                    };
                    // O2 P5 dedup: a wire-delivered sample carries its source
                    // identity (writer_guid, source_seq). Skip one already
                    // stored — this makes a TransientLocal ingest reader safe
                    // (history+live overlap, and a service restart that
                    // re-receives a writer's history) without duplicating.
                    if source_seq >= 0 && !seen.insert((writer_guid, source_seq)) {
                        continue;
                    }
                    // A body under an identifier that names no CDR form
                    // (XML, a vendor's range) cannot be replayed under it:
                    // reported, not stored.
                    let weid = match writer.for_identifier(representation_identifier) {
                        Ok(weid) => weid,
                        Err(e) => {
                            report_event(
                                &pump_events,
                                ServiceEvent::ReplayWriteFailed {
                                    topic: topic_owned.clone(),
                                    what: "sample",
                                    error: e.to_string(),
                                },
                            );
                            continue;
                        }
                    };
                    if let Err(e) = set_form(
                        &pump_rrt,
                        weid,
                        &mut last_identifier,
                        representation_identifier,
                    ) {
                        report_event(
                            &pump_events,
                            ServiceEvent::ReplayWriteFailed {
                                topic: topic_owned.clone(),
                                what: "sample",
                                error: e.to_string(),
                            },
                        );
                        continue;
                    }
                    let ds = DurabilitySample {
                        topic: topic_owned.clone(),
                        instance_key: key_hash,
                        sequence: seq.fetch_add(1, Ordering::Relaxed),
                        payload: payload.clone(),
                        representation_identifier,
                        created_at: SystemTime::now(),
                        source_guid: writer_guid,
                        source_sequence: source_seq,
                    };
                    if store.store(ds).is_ok() {
                        // Grow the replay writer's history for future
                        // late-joiners, under the SAME instance key the
                        // store just retained it under — the unkeyed
                        // convenience write would fold every instance of a
                        // keyed topic into the replay writer's one default
                        // instance, breaking per-instance KEEP_LAST and
                        // late-join replay (measured: a 3rd write to one
                        // instance and 1 to another delivered the writer's
                        // first two writes overall, not KEEP_LAST(2) of each
                        // instance).
                        if let Err(e) = write_replay_sample(
                            &pump_rrt,
                            weid,
                            &payload,
                            key_hash,
                            (writer_guid, source_seq),
                            source_timestamp,
                        ) {
                            report_event(
                                &pump_events,
                                ServiceEvent::ReplayWriteFailed {
                                    topic: topic_owned.clone(),
                                    what: "sample",
                                    error: e.to_string(),
                                },
                            );
                        }
                    }
                }
            })
            .map_err(|_| ServiceError::Poisoned("spawn pump"))?;

        self.served()?.push(Served {
            name: topic_name.to_string(),
            contract,
            type_name: type_name.to_string(),
            keyed,
            stop,
            handle: Some(handle),
        });
        Ok(())
    }

    /// Serves `topic_name` immediately if the store already has persisted
    /// [`TopicMeta`] for it — from an earlier run of this daemon, or an
    /// earlier `serve`/`serve_typed` call in this one (module docs,
    /// "Resuming a served topic after a restart"): [`Self::serve_typed`]
    /// runs right away with the stored type name and keyedness, so the
    /// replay writer is primed from the store, the ingest reader is
    /// created, and the pump starts — all without waiting for any writer or
    /// reader to be (re-)discovered.
    ///
    /// `contract_override`, when given, wins over the stored contract — a
    /// config-pinned contract on THIS run outranks whatever was resolved and
    /// persisted on an earlier one (module docs, "The retention contract":
    /// config always wins outright). With `None`, the stored contract keeps
    /// governing, matching "the first writer's contract governs the topic's
    /// lifetime" (module docs, "Two writers, one topic") extended across a
    /// restart.
    ///
    /// Returns `true` if metadata was found (the topic is now served), or
    /// `false` if the store has nothing for this topic yet — the caller then
    /// falls back to [`Self::serve_discovered`], today's discovery-driven
    /// path for a genuinely new topic.
    ///
    /// # Errors
    /// The store lookup failed, or [`Self::serve_typed`] failed.
    pub fn serve_if_stored(
        &self,
        topic_name: &str,
        contract_override: Option<Contract>,
    ) -> Result<bool> {
        let Some(meta) = self.store.topic_meta(topic_name)? else {
            return Ok(false);
        };
        let contract = contract_override.unwrap_or(meta.contract);
        self.serve_typed(topic_name, &meta.type_name, meta.keyed, contract)?;
        self.report(ServiceEvent::ServedFromStoredMeta {
            topic: topic_name.to_string(),
            type_name: meta.type_name,
            keyed: meta.keyed,
        });
        Ok(true)
    }

    /// Topics currently served.
    ///
    /// # Errors
    /// Lock poisoned.
    pub fn served_topics(&self) -> Result<Vec<String>> {
        Ok(self.served()?.iter().map(|s| s.name.clone()).collect())
    }

    /// Auto-serve any topic whose discovered writers declare a durability of
    /// `TRANSIENT` or `PERSISTENT` — observed via the standard `DCPSPublication`
    /// builtin reader. This needs no application-side code: apps simply set
    /// their `DurabilityQosPolicy`, and the service picks the topic up.
    ///
    /// The type name and keyedness come from the discovered writer itself
    /// (module docs, "Keyed topics"), and the retention contract from the
    /// writer's own `DurabilityServiceQosPolicy`, falling back to
    /// `default_contract` only when the writer's announced policy is
    /// indistinguishable from "never touched" (module docs, "The retention
    /// contract").
    ///
    /// # Errors
    /// Spawning the discovery thread failed.
    pub fn enable_auto_discovery(self: &Arc<Self>, default_contract: Contract) -> Result<()> {
        let reader = self.ingest.get_builtin_subscriber().publication_reader();
        let me = Arc::clone(self);
        let stop = Arc::new(AtomicBool::new(false));
        let pstop = Arc::clone(&stop);
        let handle = std::thread::Builder::new()
            .name("durability-autodiscover".to_string())
            .spawn(move || {
                while !pstop.load(Ordering::Relaxed) {
                    let _ = reader.wait_for_data(Duration::from_millis(300));
                    let Ok(pubs) = reader.take_with_info() else {
                        continue;
                    };
                    for s in pubs {
                        if !s.info.valid_data || s.data.durability.kind < DurabilityKind::Transient
                        {
                            continue;
                        }
                        me.consider_discovered_writer(&s.data, default_contract);
                    }
                }
            })
            .map_err(|_| ServiceError::Poisoned("spawn auto-discovery"))?;
        self.served()?.push(Served {
            name: "<auto-discovery>".to_string(),
            contract: default_contract,
            type_name: String::new(),
            keyed: false,
            stop,
            handle: Some(handle),
        });
        Ok(())
    }

    /// Serves `topic_name` once **any** writer of it is discovered — the type
    /// name, keyedness and retention contract all come from that writer's own
    /// SEDP announcement, exactly like [`Self::enable_auto_discovery`] (which
    /// this shares its resolution logic with). This is what the daemon's
    /// explicit `--topic NAME` / config `durability.topics: [NAME]` entries
    /// use when the entry names no `type`/`keyed` override — see
    /// [`Self::serve_typed`] for the direct path config uses when it does.
    ///
    /// Unlike auto-discovery, this serves the topic **regardless of the
    /// discovered writer's durability kind**: the operator named the topic
    /// explicitly, so auto-discovery's TRANSIENT/PERSISTENT filter does not
    /// apply. Previously, an explicitly named topic was always registered as
    /// a native `Topic<RawBytes>` (locked to `zerodds::RawBytes`), so it
    /// never matched a writer of any other type name.
    ///
    /// # Errors
    /// Spawning the discovery thread failed.
    pub fn serve_discovered(
        self: &Arc<Self>,
        topic_name: &str,
        default_contract: Contract,
    ) -> Result<()> {
        let reader = self.ingest.get_builtin_subscriber().publication_reader();
        let me = Arc::clone(self);
        let stop = Arc::new(AtomicBool::new(false));
        let pstop = Arc::clone(&stop);
        let topic_owned = topic_name.to_string();
        let handle = std::thread::Builder::new()
            .name(format!("durability-discover-{topic_name}"))
            .spawn(move || {
                while !pstop.load(Ordering::Relaxed) {
                    let _ = reader.wait_for_data(Duration::from_millis(300));
                    let Ok(pubs) = reader.take_with_info() else {
                        continue;
                    };
                    for s in pubs {
                        if !s.info.valid_data || s.data.topic_name != topic_owned {
                            continue;
                        }
                        me.consider_discovered_writer(&s.data, default_contract);
                    }
                }
            })
            .map_err(|_| ServiceError::Poisoned("spawn discovery"))?;
        self.served()?.push(Served {
            name: format!("<discover:{topic_name}>"),
            contract: default_contract,
            type_name: String::new(),
            keyed: false,
            stop,
            handle: Some(handle),
        });
        Ok(())
    }

    /// The shared resolution step of [`Self::enable_auto_discovery`] and
    /// [`Self::serve_discovered`]: serve `data`'s topic if it is not served
    /// yet, else compare against what it is actually served under. A type
    /// name or keyedness disagreement (which can happen when the topic was
    /// served eagerly from stored [`TopicMeta`] — [`Self::serve_if_stored`] —
    /// and a writer now announces something else) is reported as a
    /// [`ServiceEvent::TopicMetaMismatch`], never silently applied. Only once
    /// type/keyedness agree does a differing `DurabilityServiceQosPolicy`
    /// get compared (module docs, "Two writers, one topic") and reported as
    /// [`ServiceEvent::ConflictingDurabilityService`]. A `serve_typed`
    /// failure is reported, not dropped.
    fn consider_discovered_writer(
        self: &Arc<Self>,
        data: &PublicationBuiltinTopicData,
        default_contract: Contract,
    ) {
        let topic = &data.topic_name;
        let contract = effective_contract(data.durability_service, default_contract);
        let keyed = writer_is_keyed(&data.key);
        match self.served_meta_of(topic) {
            None => {
                if let Err(e) = self.serve_typed(topic, &data.type_name, keyed, contract) {
                    self.report(ServiceEvent::AutoServeFailed {
                        topic: topic.clone(),
                        type_name: data.type_name.clone(),
                        error: e.to_string(),
                    });
                }
            }
            Some((served_type_name, served_keyed, _))
                if served_type_name != data.type_name || served_keyed != keyed =>
            {
                self.report(ServiceEvent::TopicMetaMismatch {
                    topic: topic.clone(),
                    served_type_name,
                    served_keyed,
                    announced_type_name: data.type_name.clone(),
                    announced_keyed: keyed,
                });
            }
            Some((_, _, in_force)) if in_force != contract => {
                self.report(ServiceEvent::ConflictingDurabilityService {
                    topic: topic.clone(),
                    in_force,
                    announced: contract,
                });
            }
            Some(_) => {}
        }
    }

    /// Stops all pumps + the auto-discovery thread and tears the daemon down.
    /// Takes `&self` (not `self`) so it is callable on an `Arc<DurabilityService>`
    /// even while the auto-discovery thread still holds a clone. Drains the join
    /// handles under the lock, then joins WITHOUT holding it (the auto-discovery
    /// thread may itself be inside `serve` taking the lock).
    pub fn shutdown(&self) {
        let handles: Vec<JoinHandle<()>> = {
            let Ok(mut served) = self.served.lock() else {
                return;
            };
            for s in served.iter() {
                s.stop.store(true, Ordering::Relaxed);
            }
            served.iter_mut().filter_map(|s| s.handle.take()).collect()
        };
        for h in handles {
            let _ = h.join();
        }
        self.cleanup_stop.store(true, Ordering::Relaxed);
        if let Ok(mut cleanup_handle) = self.cleanup_handle.lock()
            && let Some(h) = cleanup_handle.take()
        {
            let _ = h.join();
        }
    }
}

/// Writes a stored sample to the replay writer on behalf of the writer that
/// first wrote it, under that writer's sequence number (DDSI-RTPS 2.5
/// §8.7.9: the service "needs to send the data that has been received and
/// stored on behalf of the persistent writer"): a reader that has the sample
/// from that writer, or from another service, drops the replay as a
/// duplicate. A sample without a source identity (stored before it was
/// kept, or all-zero) goes out as the service's own.
fn write_replay_sample(
    rrt: &zerodds_dcps::runtime::DcpsRuntime,
    weid: zerodds_rtps::wire_types::EntityId,
    payload: &[u8],
    key_hash: [u8; 16],
    source: ([u8; 16], i64),
    source_timestamp: Option<zerodds_rtps::header_extension::HeTimestamp>,
) -> zerodds_dcps::Result<()> {
    let (guid, sequence) = source;
    if sequence >= 0 && guid != [0; 16] {
        rrt.write_user_sample_on_behalf(
            weid,
            payload,
            Some(key_hash),
            source_timestamp,
            Guid::from_bytes(guid),
            zerodds_rtps::wire_types::SequenceNumber(sequence),
        )
        .map(|_| ())
    } else {
        rrt.write_user_sample_keyed(weid, payload, key_hash, source_timestamp)
    }
}

/// Records `event` into the bounded ring and logs it (stderr, one line) — an
/// operator watching the daemon's output sees the same thing a test
/// asserting on [`DurabilityService::events`] does. A free function (not a
/// `DurabilityService` method) so a pump thread — spawned from `&self`, not
/// `Arc<Self>` — can call it too, via its own clone of the `events` `Arc`.
#[allow(clippy::print_stderr)]
fn report_event(events: &Mutex<VecDeque<ServiceEvent>>, event: ServiceEvent) {
    eprintln!("zerodds-durability-service: {event}");
    if let Ok(mut events) = events.lock() {
        if events.len() >= MAX_EVENTS {
            events.pop_front();
        }
        events.push_back(event);
    }
}

/// Periodically calls [`DurabilityStore::cleanup`] so an instance whose
/// `cleanup_delay` has elapsed since [`DurabilityStore::unregister`] is
/// actually purged, at [`CLEANUP_SWEEP_INTERVAL`].
fn spawn_cleanup_sweep(
    store: Arc<dyn DurabilityStore>,
    stop: Arc<AtomicBool>,
) -> Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("durability-cleanup".to_string())
        .spawn(move || {
            // Sleep in short slices (not one 30 s sleep) so `shutdown()`
            // does not have to wait out a full sweep interval to join this
            // thread — matching every other pump's 200 ms stop-flag re-check.
            let slice = Duration::from_millis(200);
            let mut waited = Duration::ZERO;
            while !stop.load(Ordering::Relaxed) {
                std::thread::sleep(slice);
                waited += slice;
                if waited >= CLEANUP_SWEEP_INTERVAL {
                    waited = Duration::ZERO;
                    let _ = store.cleanup(SystemTime::now());
                }
            }
        })
        .map_err(|_| ServiceError::Poisoned("spawn cleanup sweep"))
}

/// The replay writer of a served topic. It is registered with the first
/// sample it replays, so that it announces that sample's data
/// representation first: a reader matches a writer only when it accepts
/// the writer's first offered representation (DDS-XTypes 1.3 §7.6.3.1.1),
/// so a writer that announced XCDR first for samples that came under XCDR2
/// never matched a reader that accepts XCDR2 alone. A lifecycle marker
/// before any sample registers it with the runtime's default offer.
struct ReplayWriter {
    rt: Arc<zerodds_dcps::runtime::DcpsRuntime>,
    topic_name: String,
    type_name: String,
    keyed: bool,
    depth: usize,
    eid: Option<zerodds_rtps::EntityId>,
}

impl ReplayWriter {
    /// The writer for a sample under `identifier`, registered on the first
    /// call.
    fn for_identifier(
        &mut self,
        identifier: [u8; 2],
    ) -> zerodds_dcps::Result<zerodds_rtps::EntityId> {
        use zerodds_rtps::publication_data::data_representation as dr;
        let first = representation_of(identifier).ok_or(zerodds_dcps::DdsError::BadParameter {
            what: "representation identifier names no CDR form of XTypes 1.3 Table 60",
        })?;
        let other = if first == dr::XCDR {
            dr::XCDR2
        } else {
            dr::XCDR
        };
        self.register(Some(vec![first, other]))
    }

    /// The writer, registered with the runtime's default offer when no
    /// sample registered it.
    fn any(&mut self) -> zerodds_dcps::Result<zerodds_rtps::EntityId> {
        self.register(None)
    }

    fn register(
        &mut self,
        offer: Option<Vec<i16>>,
    ) -> zerodds_dcps::Result<zerodds_rtps::EntityId> {
        if let Some(eid) = self.eid {
            return Ok(eid);
        }
        let eid = self.rt.register_user_writer_kind(
            UserWriterConfig {
                data_representation_offer: offer,
                ..user_writer_cfg(&self.topic_name, &self.type_name)
            },
            self.keyed,
        )?;
        // `UserWriterConfig` has no HistoryQosPolicy of its own: the depth
        // of the contract (see `serve_runtime`).
        let _ = self.rt.set_user_writer_history_depth(eid, self.depth);
        self.eid = Some(eid);
        Ok(eid)
    }
}

/// The data representation (XCDR or XCDR2) a representation identifier
/// names (DDS-XTypes 1.3 Table 60), as
/// `DcpsRuntime::set_user_writer_representation_identifier` reads it.
fn representation_of(identifier: [u8; 2]) -> Option<i16> {
    use zerodds_rtps::publication_data::data_representation as dr;
    if identifier[0] != 0x00 {
        return None;
    }
    match identifier[1] & !0x01 {
        0x00 | 0x02 => Some(dr::XCDR),
        0x06 | 0x08 | 0x0a | 0x10 | 0x12 | 0x14 => Some(dr::XCDR2),
        _ => None,
    }
}

/// Frames the replay writer's next samples under `identifier` (the
/// representation, the extensibility and the byte order it names,
/// [`zerodds_dcps::runtime::DcpsRuntime::set_user_writer_representation_identifier`]),
/// unless it already is.
fn set_form(
    rt: &zerodds_dcps::runtime::DcpsRuntime,
    weid: zerodds_rtps::EntityId,
    last: &mut Option<[u8; 2]>,
    identifier: [u8; 2],
) -> zerodds_dcps::Result<()> {
    if *last != Some(identifier) {
        rt.set_user_writer_representation_identifier(weid, identifier)?;
        *last = Some(identifier);
    }
    Ok(())
}

/// `ChangeKind` → the `PID_STATUS_INFO` bits [`zerodds_dcps::runtime::DcpsRuntime::write_user_lifecycle`]
/// takes (DDSI-RTPS 2.5 §9.6.3.9), for forwarding a discovered dispose/
/// unregister marker to the replay writer.
fn lifecycle_status_bits(kind: zerodds_rtps::history_cache::ChangeKind) -> u32 {
    use zerodds_rtps::history_cache::ChangeKind;
    use zerodds_rtps::inline_qos::status_info::{DISPOSED, UNREGISTERED};
    match kind {
        ChangeKind::NotAliveDisposed => DISPOSED,
        ChangeKind::NotAliveUnregistered => UNREGISTERED,
        ChangeKind::NotAliveDisposedUnregistered => DISPOSED | UNREGISTERED,
        // Alive / AliveFiltered never reach here (handled as data).
        ChangeKind::Alive | ChangeKind::AliveFiltered => 0,
    }
}

/// Runtime-level ingest-reader config: RELIABLE, VOLATILE (a caller sets
/// another durability), with an explicit `type_name` for cross-vendor
/// matching. KEEP_ALL history is the reader default at the runtime level.
fn user_reader_cfg(topic_name: &str, type_name: &str) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic_name.to_string(),
        type_name: type_name.to_string(),
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        // Cross-vendor byte-path: no local TypeObject (matched by topic+name).
        // Accept BOTH XCDR1 (0) and XCDR2 (2): a foreign-vendor writer of a
        // FINAL-extensibility type offers only XCDR1 (verified live against
        // CycloneDDS — a final `dur::Sample` writer advertises
        // data_representation=0). An XCDR2-only reader is RxO-incompatible and
        // never matches it. A durability service must ingest whatever
        // representation the source emits, so it offers both.
        data_representation_offer: Some(vec![0, 2]),
        reliable: true,
        ..Default::default()
    }
}

/// Runtime-level replay-writer config: RELIABLE, PERSISTENT, with an
/// explicit `type_name`. The history depth follows the contract
/// (`set_user_writer_history_depth` in `serve_runtime`).
///
/// The durability it offers is PERSISTENT, the strongest kind: a reader
/// matches a writer only when the offered kind is at least the requested one
/// (DDS 1.4 §2.2.3.4, requested/offered), and the late joiners this service
/// exists for request TRANSIENT or PERSISTENT. Offering TRANSIENT_LOCAL, the
/// replay writer matched neither, and a TRANSIENT or PERSISTENT reader got
/// nothing once the original writer was gone (measured in the integration
/// pillar, tests/integration/tests/services.rs).
fn user_writer_cfg(topic_name: &str, type_name: &str) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic_name.to_string(),
        type_name: type_name.to_string(),
        reliable: true,
        // The strongest kind, so that every late joiner matches whatever
        // durability it requests (see above).
        durability: DurabilityKind::Persistent,
        // The replay writer forwards on the original writer's behalf (module
        // doc), it is not that writer, so it announces the spec default here;
        // what matters is the CONTRACT this service enforces internally
        // (derived from the discovered writer's own DurabilityServiceQosPolicy,
        // see `enable_auto_discovery`/`serve_typed`), not a second, redundant
        // announcement of it.
        durability_service: zerodds_qos::DurabilityServiceQosPolicy::default(),
        deadline: DeadlineQosPolicy::default(),
        latency_budget: Default::default(),
        destination_order: Default::default(),
        lifespan: LifespanQosPolicy::default(),
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        transport_priority: 0,
        presentation: Default::default(),
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        type_identifier: zerodds_types::TypeIdentifier::None,
        data_representation_offer: None,
        group_entity_id: None,
        property: Default::default(),
        data_tags: Default::default(),
        data_max_size_serialized: None,
        entity_name: None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_durability_store_sqlite::SqliteStore;

    /// DDSI-RTPS 2.5 §8.7.9: a stored sample with a known source goes out on
    /// behalf of that writer, so a reader takes it as that writer's change
    /// and its sequence number; one without a source goes out as the
    /// replay writer's own.
    #[test]
    fn rtps_8_7_9_a_replayed_sample_goes_out_on_behalf_of_its_writer() {
        use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig};
        let rt = DcpsRuntime::start(
            zerodds_test_domains::unique_domain(),
            zerodds_rtps::wire_types::GuidPrefix::from_bytes([0x5A; 12]),
            RuntimeConfig::default(),
        )
        .unwrap();
        let weid = rt
            .register_user_writer(user_writer_cfg("Replay", "ReplayType"))
            .unwrap();
        let (_reid, rx) = rt
            .register_user_reader(UserReaderConfig {
                topic_name: "Replay".into(),
                type_name: "ReplayType".into(),
                reliable: true,
                ..Default::default()
            })
            .unwrap();
        let alice = [0xA1; 16];
        write_replay_sample(&rt, weid, b"one", [1; 16], (alice, 4), None).unwrap();
        write_replay_sample(&rt, weid, b"two", [1; 16], ([0; 16], -1), None).unwrap();
        let wait = Duration::from_millis(100);
        let origin = |s: UserSample| match s {
            UserSample::Alive {
                writer_guid,
                source_sequence_number,
                ..
            } => (writer_guid, source_sequence_number),
            other => panic!("expected Alive, got {other:?}"),
        };
        assert_eq!(origin(rx.recv_timeout(wait).unwrap()), (alice, 4));
        let (own, _) = origin(rx.recv_timeout(wait).unwrap());
        assert_eq!(&own[..12], &[0x5A; 12], "the replay writer's own");
        rt.shutdown();
    }

    /// [`DurabilityService::events`] keeps the newest `MAX_EVENTS` events,
    /// oldest first (its documentation).
    #[test]
    fn the_event_ring_keeps_the_newest_max_events() {
        let domain = zerodds_test_domains::unique_domain();
        let store: Arc<dyn DurabilityStore> =
            Arc::new(SqliteStore::open_in_memory(Contract::keep_all()).unwrap());
        let service = DurabilityService::start(domain, store).unwrap();

        service.report(ServiceEvent::AutoServeFailed {
            topic: "T".to_string(),
            type_name: "Ty".to_string(),
            error: "boom".to_string(),
        });
        let events = service.events().unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], ServiceEvent::AutoServeFailed { topic, .. } if topic == "T"));

        // The ring is bounded — MAX_EVENTS + 50 more pushes still leave
        // exactly MAX_EVENTS, oldest dropped first.
        for i in 0..(MAX_EVENTS + 50) {
            service.report(ServiceEvent::AutoServeFailed {
                topic: format!("T{i}"),
                type_name: "Ty".to_string(),
                error: "boom".to_string(),
            });
        }
        let events = service.events().unwrap();
        assert_eq!(events.len(), MAX_EVENTS);
        let last = format!("T{}", MAX_EVENTS + 50 - 1);
        assert!(
            matches!(events.last(), Some(ServiceEvent::AutoServeFailed { topic, .. }) if topic == &last)
        );

        service.shutdown();
    }

    /// A discovered writer's announcement for `topic` with `type_name` and
    /// `durability_service`.
    fn announcement(
        topic: &str,
        type_name: &str,
        durability_service: zerodds_qos::DurabilityServiceQosPolicy,
    ) -> zerodds_dcps::builtin_topics::PublicationBuiltinTopicData {
        zerodds_dcps::builtin_topics::PublicationBuiltinTopicData {
            key: zerodds_rtps::wire_types::Guid::UNKNOWN,
            participant_key: zerodds_rtps::wire_types::Guid::UNKNOWN,
            topic_name: topic.to_string(),
            type_name: type_name.to_string(),
            durability: zerodds_qos::DurabilityQosPolicy {
                kind: zerodds_qos::DurabilityKind::Transient,
            },
            durability_service,
            reliability: zerodds_qos::ReliabilityQosPolicy {
                kind: zerodds_qos::ReliabilityKind::Reliable,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Two writers, one topic (module docs): the first writer's contract
    /// keeps governing the topic. A second writer that announces another
    /// `DurabilityServiceQosPolicy` (DDS 1.4 §2.2.3.5) is reported as a
    /// conflict, one that announces another type name as a mismatch; neither
    /// is applied.
    #[test]
    fn a_second_writers_other_contract_or_type_is_reported_and_not_applied() {
        let domain = zerodds_test_domains::unique_domain();
        let store: Arc<dyn DurabilityStore> =
            Arc::new(SqliteStore::open_in_memory(Contract::keep_all()).unwrap());
        let service = Arc::new(DurabilityService::start(domain, store).unwrap());
        let topic = "AlreadyServed";
        let first_contract = Contract::keep_all();
        service
            .serve_typed(topic, "T", false, first_contract)
            .unwrap();

        // Not the DDS default (depth 1), so `Contract::from_qos` would apply.
        let other_contract = zerodds_qos::DurabilityServiceQosPolicy {
            history_depth: 9,
            ..Default::default()
        };
        service
            .consider_discovered_writer(&announcement(topic, "T", other_contract), first_contract);
        service.consider_discovered_writer(
            &announcement(topic, "SomeOtherType", Default::default()),
            first_contract,
        );

        assert_eq!(service.contract_of(topic), Some(first_contract));
        assert_eq!(
            service.served_meta_of(topic).map(|(t, ..)| t),
            Some("T".to_string())
        );
        let events = service.events().unwrap();
        assert!(
            events.iter().any(|e| matches!(
                e,
                ServiceEvent::ConflictingDurabilityService { topic: t, .. } if t == topic
            )),
            "the other contract is reported: {events:?}"
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                ServiceEvent::TopicMetaMismatch {
                    topic: t,
                    served_type_name,
                    announced_type_name,
                    ..
                } if t == topic && served_type_name == "T" && announced_type_name == "SomeOtherType"
            )),
            "the other type name is reported: {events:?}"
        );
        service.shutdown();
    }

    /// Resuming a served topic after a restart (module docs): the type name,
    /// keyedness and contract an earlier run persisted are enough for
    /// [`DurabilityService::serve_if_stored`] to serve the topic again, with
    /// no writer or reader present, and it reports that it did; a contract
    /// the configuration pins wins over the stored one; a topic nobody served
    /// is not found and not served.
    #[test]
    fn serve_if_stored_resumes_a_topic_from_its_stored_meta() {
        let domain = zerodds_test_domains::unique_domain();
        let store: Arc<dyn DurabilityStore> =
            Arc::new(SqliteStore::open_in_memory(Contract::keep_all()).unwrap());
        let contract = Contract::keep_all();
        {
            // An earlier run: serve, then stop. The store holds the metadata
            // since `serve_typed`, not since the stop.
            let service = DurabilityService::start(domain, Arc::clone(&store)).unwrap();
            service
                .serve_typed("Stored", "demo::Command", true, contract)
                .unwrap();
            service.serve_typed("Pinned", "T", false, contract).unwrap();
            service.shutdown();
        }

        let service = DurabilityService::start(domain, Arc::clone(&store)).unwrap();
        assert!(!service.serve_if_stored("NeverServed", None).unwrap());
        assert!(service.serve_if_stored("Stored", None).unwrap());
        let pinned = Contract {
            history_depth: 42,
            ..Contract::keep_all()
        };
        assert!(service.serve_if_stored("Pinned", Some(pinned)).unwrap());

        let served = service.served_topics().unwrap();
        assert!(served.iter().any(|t| t == "Stored"));
        assert!(served.iter().all(|t| t != "NeverServed"));
        assert_eq!(service.contract_of("Stored"), Some(contract));
        assert_eq!(service.contract_of("Pinned"), Some(pinned));
        let events = service.events().unwrap();
        assert!(
            events.iter().any(|e| matches!(
                e,
                ServiceEvent::ServedFromStoredMeta { topic: t, type_name, keyed }
                    if t == "Stored" && type_name == "demo::Command" && *keyed
            )),
            "serving from stored metadata is reported: {events:?}"
        );
        service.shutdown();
    }
}
