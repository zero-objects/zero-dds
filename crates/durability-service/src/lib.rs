// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Standalone DDS Durability-Service daemon (ADR 0009).
//!
//! Crate `zerodds-durability-service`. Safety classification: **STANDARD**.
//!
//! From an RTPS point of view the service is just another participant. For
//! each served topic it:
//!
//! 1. **Ingests** via a `RELIABLE / TRANSIENT_LOCAL / KEEP_ALL` reader — it
//!    receives every sample the application writers publish (and, thanks to
//!    `TRANSIENT_LOCAL`, the writer history if it joins late).
//! 2. **Stores** each sample in a [`DurabilityStore`] (sqlite/file/in-memory).
//! 3. **Replays** via a `TRANSIENT_LOCAL / KEEP_ALL` writer — a late-joining
//!    reader matches it and the standard RTPS path delivers the history. This
//!    keeps working after the original writer's process has died.
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
    DataReader, DataReaderQos, DataWriter, DataWriterQos, DdsType, DomainParticipant,
    DomainParticipantFactory, DomainParticipantQos, InstanceStateKind, Publisher, PublisherQos,
    RawBytes, Subscriber, SubscriberQos, Topic, TopicQos,
};
use zerodds_durability_store::{Contract, DurabilitySample, DurabilityStore, TopicMeta};
use zerodds_qos::policies::durability::DurabilityKind;
use zerodds_qos::policies::durability_service::DurabilityServiceQosPolicy;
use zerodds_qos::policies::history::HistoryKind;
use zerodds_qos::policies::reliability::ReliabilityKind;
use zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED;
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

/// Reader QoS for ingest: receive every sample reliably, **including a matched
/// writer's TransientLocal history** (O2 P5). A service that starts after a
/// writer published now back-fetches that writer's history. The history+live
/// overlap — and a service restart that re-receives the same history — are
/// deduped by the pump on `(writer_guid, source_sequence_number)`, which
/// `UserSample::Alive` now carries and the store persists; without that dedup
/// this had to be `VOLATILE` to avoid double-storing.
fn ingest_reader_qos() -> DataReaderQos {
    let mut q = DataReaderQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.durability.kind = DurabilityKind::TransientLocal;
    q.history.kind = HistoryKind::KeepAll;
    q.resource_limits.max_samples = LENGTH_UNLIMITED;
    q
}

/// Writer QoS for replay: deliver `contract`'s retained history to
/// late-joiners. `serve()`'s scope is a single unkeyed instance, so a
/// `KEEP_LAST(n)` contract here bounds that one instance's history — a
/// hardcoded `KEEP_ALL` regardless of `contract` used to leave the replay
/// writer's own cache unbounded even under a `KEEP_LAST` contract, so a late
/// joiner still saw every historical sample, not the contracted depth.
fn replay_writer_qos(contract: Contract) -> DataWriterQos {
    let mut q = DataWriterQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.durability.kind = DurabilityKind::TransientLocal;
    q.history = match contract.history_kind {
        HistoryKind::KeepAll => zerodds_qos::HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 0,
        },
        HistoryKind::KeepLast => zerodds_qos::HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: contract.effective_depth() as i32,
        },
    };
    q.resource_limits.max_samples = LENGTH_UNLIMITED;
    q
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
    publisher: Publisher,
    subscriber: Subscriber,
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
        let publisher = replay.create_publisher(PublisherQos::default());
        let subscriber = ingest.create_subscriber(SubscriberQos::default());
        let cleanup_stop = Arc::new(AtomicBool::new(false));
        let cleanup_handle = spawn_cleanup_sweep(Arc::clone(&store), Arc::clone(&cleanup_stop))?;
        Ok(Self {
            ingest,
            replay,
            publisher,
            subscriber,
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

    /// Begins serving `topic_name` with the given retention `contract`. Creates
    /// the ingest reader + replay writer, primes the writer from the store
    /// (startup-sync), and spawns the ingest pump.
    ///
    /// # Errors
    /// Topic/reader/writer creation or the initial store/replay failed.
    pub fn serve(&self, topic_name: &str, contract: Contract) -> Result<()> {
        // Persist what a restart needs to resume serving this topic without
        // waiting for a writer/reader to be re-discovered (module docs,
        // "Resuming a served topic after a restart"). `serve()`'s scope is a
        // single unkeyed instance of the native `zerodds::RawBytes` type.
        self.store.set_topic_meta(
            topic_name,
            TopicMeta {
                type_name: RawBytes::TYPE_NAME.to_string(),
                keyed: false,
                contract,
            },
        )?;

        // Topics are per-participant: the reader's on the ingest participant,
        // the writer's on the replay participant.
        let rtopic: Topic<RawBytes> = self
            .ingest
            .create_topic::<RawBytes>(topic_name, TopicQos::default())?;
        let wtopic: Topic<RawBytes> = self
            .replay
            .create_topic::<RawBytes>(topic_name, TopicQos::default())?;
        let writer = self
            .publisher
            .create_datawriter::<RawBytes>(&wtopic, replay_writer_qos(contract))?;
        let reader = self
            .subscriber
            .create_datareader::<RawBytes>(&rtopic, ingest_reader_qos())?;

        // Startup-sync: prime the replay writer's history from durable storage.
        for sample in self.store.replay_for_topic(topic_name)? {
            writer.write(&RawBytes::new(sample.payload))?;
        }

        let stop = Arc::new(AtomicBool::new(false));
        let pump_stop = Arc::clone(&stop);
        let store = Arc::clone(&self.store);
        let topic_owned = topic_name.to_string();
        let own_pub = writer.instance_handle();
        let seq = AtomicU64::new(self.store.stats(topic_name)?.samples as u64);

        let handle = std::thread::Builder::new()
            .name(format!("durability-pump-{topic_name}"))
            .spawn(move || {
                pump(
                    &pump_stop,
                    &reader,
                    &writer,
                    store.as_ref(),
                    &topic_owned,
                    own_pub,
                    &seq,
                );
            })
            .map_err(|_| ServiceError::Poisoned("spawn pump"))?;

        self.served()?.push(Served {
            name: topic_name.to_string(),
            contract,
            type_name: RawBytes::TYPE_NAME.to_string(),
            keyed: false,
            stop,
            handle: Some(handle),
        });
        Ok(())
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

        let weid = rrt
            .register_user_writer_kind(user_writer_cfg(topic_name, type_name), keyed)
            .map_err(|_| ServiceError::Poisoned("register replay writer"))?;
        let (_reid, rx) = irt
            .register_user_reader_kind(user_reader_cfg(topic_name, type_name), keyed)
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
        let _ = rrt.set_user_writer_history_depth(
            weid,
            match contract.history_kind {
                HistoryKind::KeepAll => usize::MAX,
                HistoryKind::KeepLast => contract.effective_depth(),
            },
        );

        // Startup-sync: prime the replay writer's history from durable storage.
        // Each stored sample carries its own representation + byte order; set
        // the writer overrides so the primed replay re-declares the original
        // wire (a big-endian peer's persisted sample replays with a BE encap).
        for sample in self.store.replay_for_topic(topic_name)? {
            let off: i16 = if sample.representation == 1 { 2 } else { 0 };
            let _ = rrt.set_user_writer_data_rep_override(weid, Some(vec![off]));
            let _ = rrt.set_user_writer_byte_order_override(weid, sample.big_endian);
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
                // CDR body in the SOURCE writer's representation (a foreign
                // FINAL-type writer emits XCDR1). The replay writer must declare
                // an encap that matches that body, else a strict foreign reader
                // misparses an alignment-sensitive type. We track the source
                // representation and override the replay writer's encap when it
                // changes (a topic is representation-consistent, so this is
                // effectively a one-time set). 255 = "not yet observed".
                let mut last_rep: u8 = 255;
                // Tracks the last byte order pushed to the replay writer's encap
                // override, so a BE peer's samples replay with a BE encap header.
                let mut last_be = false;
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
                        representation,
                        big_endian,
                        writer_guid,
                        source_seq,
                        key_hash,
                        source_timestamp,
                    ) = match sample {
                        UserSample::Alive {
                            key_hash,
                            payload,
                            representation,
                            big_endian,
                            writer_guid,
                            source_sequence_number,
                            source_timestamp,
                            ..
                        } => (
                            payload.to_vec(),
                            representation,
                            big_endian,
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
                            if let Err(e) = pump_rrt.write_user_lifecycle(
                                weid,
                                key_hash,
                                key_holder.as_deref(),
                                lifecycle_status_bits(kind),
                                source_timestamp,
                            ) {
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
                    if representation != last_rep {
                        // UserSample.representation: 0 = XCDR1, 1 = XCDR2 →
                        // data_representation offer id 0 (XCDR) / 2 (XCDR2).
                        let off: i16 = if representation == 1 { 2 } else { 0 };
                        let _ = pump_rrt.set_user_writer_data_rep_override(weid, Some(vec![off]));
                        last_rep = representation;
                    }
                    if big_endian != last_be {
                        // The stored body bytes are big-endian for a BE peer, so
                        // the replay writer must emit a matching `_BE` encap
                        // header — otherwise the late-joiner mis-decodes.
                        let _ = pump_rrt.set_user_writer_byte_order_override(weid, big_endian);
                        last_be = big_endian;
                    }
                    let ds = DurabilitySample {
                        topic: topic_owned.clone(),
                        instance_key: key_hash,
                        sequence: seq.fetch_add(1, Ordering::Relaxed),
                        payload: payload.clone(),
                        representation,
                        big_endian,
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
        if let Ok(mut cleanup_handle) = self.cleanup_handle.lock() {
            if let Some(h) = cleanup_handle.take() {
                let _ = h.join();
            }
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

/// Ingest pump: receive → store → replay-write, until `stop`.
fn pump(
    stop: &AtomicBool,
    reader: &DataReader<RawBytes>,
    writer: &DataWriter<RawBytes>,
    store: &dyn DurabilityStore,
    topic: &str,
    own_pub: zerodds_dcps::InstanceHandle,
    seq: &AtomicU64,
) {
    while !stop.load(Ordering::Relaxed) {
        // Block until data or a short timeout (re-checks the stop flag).
        let _ = reader.wait_for_data(Duration::from_millis(200));
        let samples = match reader.take_with_info() {
            Ok(s) => s,
            Err(_) => continue,
        };
        for sample in samples {
            if !sample.info.valid_data {
                // Dispose / unregister marker, no payload (DDS 1.4
                // §2.2.2.5.1.13). `serve()`'s scope is a single unkeyed
                // instance, so mark IT unregistered — its samples become
                // eligible for purge after `contract.cleanup_delay`. This
                // was previously dropped unconditionally: nothing ever told
                // the store an unkeyed topic's writer had gone away, so
                // `cleanup_delay` had no event to measure from. Not
                // forwarded to the replay writer (a `RawBytes` instance
                // handle is local to whichever writer registered it, and
                // this reader never has the replay writer's own handle for
                // the same instance) — a late joiner still sees the
                // contract-bounded retained history until `cleanup` purges
                // it, which is the ADR 0009 retention guarantee; it just
                // does not see the live state transition in real time.
                if sample.info.instance_state != InstanceStateKind::Alive {
                    let _ = store.unregister(topic, &[0u8; 16], SystemTime::now());
                }
                continue;
            }
            // Skip our own replay echo (defence in depth beyond ignore_publication).
            if sample.info.publication_handle == own_pub {
                continue;
            }
            let payload = sample.data.data;
            let ds = DurabilitySample {
                topic: topic.to_string(),
                instance_key: [0u8; 16], // unkeyed (increment 1)
                sequence: seq.fetch_add(1, Ordering::Relaxed),
                payload: payload.clone(),
                // The high-level `DataReader<RawBytes>` SampleInfo does not
                // surface the wire representation / byte order, so this DDS-API
                // ingest path assumes the canonical XCDR2 little-endian wire.
                // For a big-endian peer use `serve_typed` (the runtime-level
                // UserSample path), which captures both and replays them.
                representation: 1,
                big_endian: false,
                created_at: SystemTime::now(),
                // The high-level DataReader<RawBytes> path surfaces no source
                // identity, so this path does not participate in P5 dedup.
                source_guid: [0u8; 16],
                source_sequence: -1,
            };
            if store.store(ds).is_ok() {
                // Grow the replay writer's history so future late-joiners get it.
                let _ = writer.write(&RawBytes::new(payload));
            }
        }
    }
}

/// Runtime-level ingest-reader config mirroring [`ingest_reader_qos`]
/// (RELIABLE + VOLATILE) but with an explicit `type_name` for cross-vendor
/// matching. KEEP_ALL history is the reader default at the runtime level.
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
        ..Default::default()
    }
}

/// Runtime-level replay-writer config mirroring [`replay_writer_qos`]
/// (RELIABLE + TRANSIENT_LOCAL/KEEP_ALL) with an explicit `type_name`.
fn user_writer_cfg(topic_name: &str, type_name: &str) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic_name.to_string(),
        type_name: type_name.to_string(),
        reliable: true,
        durability: DurabilityKind::TransientLocal,
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
    fn a_replayed_sample_goes_out_on_behalf_of_its_writer() {
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

    /// The event mechanism itself: `report()` logs (stderr — not asserted
    /// here, `events()` is) and records into the bounded ring
    /// [`DurabilityService::events`] reads back. This does not force a real
    /// `serve_typed` failure (no reachable one was found through the public
    /// API — every registration path either succeeds or the failure mode
    /// needs a resource this test cannot deny it, e.g. a full file
    /// descriptor table); it proves the plumbing `consider_discovered_writer`
    /// calls on a real failure is itself correct, which is the part that was
    /// entirely missing (`let _ = me.serve_typed(...)`).
    #[test]
    fn report_records_and_bounds_events() {
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

    /// The three previously-silent replay-write drops (`serve_typed`'s
    /// startup-sync priming loop, the pump's lifecycle-marker forward, and
    /// the pump's per-sample replay write — module docs "let _ =") now go
    /// through the same `report()`/`events()` plumbing
    /// [`report_records_and_bounds_events`] already proves is correct for
    /// `AutoServeFailed`. No publicly-reachable way was found to make a real
    /// `write_user_sample_keyed`/`write_user_lifecycle` call fail either (the
    /// runtime-level replay writer's `EntityId` is internal to
    /// `serve_typed`, not returned to a caller); this proves the
    /// `ReplayWriteFailed` variant + its `Display` + the ring plumbing are
    /// themselves correct, which is the part that used to not exist at all.
    #[test]
    fn replay_write_failed_is_reported_not_dropped() {
        let domain = zerodds_test_domains::unique_domain();
        let store: Arc<dyn DurabilityStore> =
            Arc::new(SqliteStore::open_in_memory(Contract::keep_all()).unwrap());
        let service = DurabilityService::start(domain, store).unwrap();

        service.report(ServiceEvent::ReplayWriteFailed {
            topic: "T".to_string(),
            what: "sample",
            error: "boom".to_string(),
        });
        let events = service.events().unwrap();
        assert_eq!(events.len(), 1);
        let ServiceEvent::ReplayWriteFailed { topic, what, error } = &events[0] else {
            panic!("expected ReplayWriteFailed, got {:?}", events[0]);
        };
        assert_eq!(topic, "T");
        assert_eq!(*what, "sample");
        assert_eq!(error, "boom");
        assert_eq!(
            events[0].to_string(),
            "topic=T: replay writer rejected a sample write: boom"
        );

        service.shutdown();
    }

    /// `consider_discovered_writer` reports a conflict (not the discovered
    /// writer's contract silently taking over) when a topic already served
    /// under one contract sees a `PublicationBuiltinTopicData` announcing a
    /// different one.
    #[test]
    fn conflicting_durability_service_is_reported_not_applied() {
        let domain = zerodds_test_domains::unique_domain();
        let store: Arc<dyn DurabilityStore> =
            Arc::new(SqliteStore::open_in_memory(Contract::keep_all()).unwrap());
        let service = Arc::new(DurabilityService::start(domain, store).unwrap());
        let topic = "AlreadyServed";
        let first_contract = Contract::keep_all();
        service
            .serve_typed(topic, "T", false, first_contract)
            .unwrap();

        // != DDS default (1) -> Contract::from_qos wins.
        let differing = zerodds_qos::DurabilityServiceQosPolicy {
            history_depth: 9,
            ..Default::default()
        };
        let data = zerodds_dcps::builtin_topics::PublicationBuiltinTopicData {
            key: zerodds_rtps::wire_types::Guid::UNKNOWN,
            participant_key: zerodds_rtps::wire_types::Guid::UNKNOWN,
            topic_name: topic.to_string(),
            type_name: "T".to_string(),
            durability: zerodds_qos::DurabilityQosPolicy {
                kind: zerodds_qos::DurabilityKind::Transient,
            },
            durability_service: differing,
            reliability: zerodds_qos::ReliabilityQosPolicy {
                kind: zerodds_qos::ReliabilityKind::Reliable,
                ..Default::default()
            },
            ..Default::default()
        };
        service.consider_discovered_writer(&data, first_contract);

        assert_eq!(
            service.contract_of(topic),
            Some(first_contract),
            "the first writer's contract must keep governing the topic"
        );
        let events = service.events().unwrap();
        assert!(
            events.iter().any(|e| matches!(
                e,
                ServiceEvent::ConflictingDurabilityService { topic: t, .. } if t == topic
            )),
            "the differing announcement must be reported: {events:?}"
        );

        service.shutdown();
    }

    /// [`DurabilityService::serve_if_stored`] finds nothing for a topic no
    /// one has ever served, and does not serve it.
    #[test]
    fn serve_if_stored_returns_false_without_metadata() {
        let domain = zerodds_test_domains::unique_domain();
        let store: Arc<dyn DurabilityStore> =
            Arc::new(SqliteStore::open_in_memory(Contract::keep_all()).unwrap());
        let service = DurabilityService::start(domain, store).unwrap();

        assert!(!service.serve_if_stored("NeverServed", None).unwrap());
        assert!(
            service
                .served_topics()
                .unwrap()
                .iter()
                .all(|t| t != "NeverServed")
        );

        service.shutdown();
    }

    /// This is the fix for the durability-restart bug: a topic's `TopicMeta`
    /// persisted by an earlier `serve_typed` call (standing in for an
    /// earlier daemon run) is enough for `serve_if_stored` to serve it again
    /// — no writer, no reader, nothing but the store. Reports
    /// `ServedFromStoredMeta`.
    #[test]
    fn serve_if_stored_serves_from_metadata_and_reports_event() {
        let domain = zerodds_test_domains::unique_domain();
        let store: Arc<dyn DurabilityStore> =
            Arc::new(SqliteStore::open_in_memory(Contract::keep_all()).unwrap());
        let topic = "StoredMetaTopic";
        let contract = Contract::keep_all();
        {
            // Simulates an earlier run: serve once, then "crash" (drop the
            // service without a graceful shutdown — the store persisted the
            // metadata already, at `serve_typed` time, not at shutdown).
            let service = DurabilityService::start(domain, Arc::clone(&store)).unwrap();
            service
                .serve_typed(topic, "demo::Command", true, contract)
                .unwrap();
            service.shutdown();
        }

        let service2 = DurabilityService::start(domain, Arc::clone(&store)).unwrap();
        assert!(
            service2.serve_if_stored(topic, None).unwrap(),
            "stored metadata must be found and served"
        );
        assert!(service2.served_topics().unwrap().iter().any(|t| t == topic));
        assert_eq!(service2.contract_of(topic), Some(contract));
        let events = service2.events().unwrap();
        assert!(
            events.iter().any(|e| matches!(
                e,
                ServiceEvent::ServedFromStoredMeta { topic: t, type_name, keyed }
                    if t == topic && type_name == "demo::Command" && *keyed
            )),
            "serving from stored metadata must be reported: {events:?}"
        );

        service2.shutdown();
    }

    /// A config-pinned `contract_override` wins over the stored contract —
    /// config always outranks a previously-resolved value (module docs, "The
    /// retention contract").
    #[test]
    fn serve_if_stored_contract_override_wins_over_stored() {
        let domain = zerodds_test_domains::unique_domain();
        let store: Arc<dyn DurabilityStore> =
            Arc::new(SqliteStore::open_in_memory(Contract::keep_all()).unwrap());
        let topic = "OverrideTopic";
        {
            let service = DurabilityService::start(domain, Arc::clone(&store)).unwrap();
            service
                .serve_typed(topic, "T", false, Contract::keep_all())
                .unwrap();
            service.shutdown();
        }

        let pinned = Contract {
            history_depth: 42,
            ..Contract::keep_all()
        };
        let service2 = DurabilityService::start(domain, Arc::clone(&store)).unwrap();
        assert!(service2.serve_if_stored(topic, Some(pinned)).unwrap());
        assert_eq!(service2.contract_of(topic), Some(pinned));

        service2.shutdown();
    }

    /// `consider_discovered_writer` reports a `TopicMetaMismatch` (not a
    /// `ConflictingDurabilityService`, and never a silent override) when a
    /// topic already served under one type name/keyedness sees a writer
    /// announce a DIFFERENT one — e.g. a topic served eagerly from stored
    /// metadata whose type turns out to disagree with a real writer.
    #[test]
    fn topic_meta_mismatch_is_reported_not_applied() {
        let domain = zerodds_test_domains::unique_domain();
        let store: Arc<dyn DurabilityStore> =
            Arc::new(SqliteStore::open_in_memory(Contract::keep_all()).unwrap());
        let service = Arc::new(DurabilityService::start(domain, store).unwrap());
        let topic = "MismatchTopic";
        let contract = Contract::keep_all();
        service.serve_typed(topic, "T", false, contract).unwrap();

        let data = zerodds_dcps::builtin_topics::PublicationBuiltinTopicData {
            key: zerodds_rtps::wire_types::Guid::UNKNOWN,
            participant_key: zerodds_rtps::wire_types::Guid::UNKNOWN,
            topic_name: topic.to_string(),
            type_name: "SomeOtherType".to_string(),
            durability: zerodds_qos::DurabilityQosPolicy {
                kind: zerodds_qos::DurabilityKind::Transient,
            },
            durability_service: zerodds_qos::DurabilityServiceQosPolicy::default(),
            reliability: zerodds_qos::ReliabilityQosPolicy {
                kind: zerodds_qos::ReliabilityKind::Reliable,
                ..Default::default()
            },
            ..Default::default()
        };
        service.consider_discovered_writer(&data, contract);

        let events = service.events().unwrap();
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
            "the type-name disagreement must be reported: {events:?}"
        );
        // Never applied: still served under the original type.
        assert_eq!(
            service.served_meta_of(topic).map(|(t, ..)| t),
            Some("T".to_string())
        );

        service.shutdown();
    }
}
