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
//! ## Scope (increment 1)
//! Unkeyed topics (`RawBytes` = single instance). The replay writer assigns
//! its own sequence numbers — a durability service is a new logical source, so
//! the original writer's sequence numbers need not be preserved. Per-instance
//! history for KEYED topics + exact sequence preservation need the per-sample
//! KeyHash/sequence exposed on the reader API and are a documented follow-up.

#![forbid(unsafe_code)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};

use zerodds_dcps::runtime::{UserReaderConfig, UserSample, UserWriterConfig};
use zerodds_dcps::{
    DataReader, DataReaderQos, DataWriter, DataWriterQos, DomainParticipant,
    DomainParticipantFactory, DomainParticipantQos, Publisher, PublisherQos, RawBytes, Subscriber,
    SubscriberQos, Topic, TopicQos,
};
use zerodds_durability_store::{Contract, DurabilitySample, DurabilityStore};
use zerodds_qos::policies::durability::DurabilityKind;
use zerodds_qos::policies::history::HistoryKind;
use zerodds_qos::policies::reliability::ReliabilityKind;
use zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED;
use zerodds_qos::{
    DeadlineQosPolicy, LifespanQosPolicy, LivelinessKind, LivelinessQosPolicy, OwnershipKind,
};

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

/// Writer QoS for replay: hold the full history and deliver it to late-joiners.
fn replay_writer_qos() -> DataWriterQos {
    let mut q = DataWriterQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.durability.kind = DurabilityKind::TransientLocal;
    q.history.kind = HistoryKind::KeepAll;
    q.resource_limits.max_samples = LENGTH_UNLIMITED;
    q
}

struct Served {
    name: String,
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
}

impl DurabilityService {
    /// Starts the daemon on `domain`, backed by `store`.
    ///
    /// # Errors
    /// Participant creation failed.
    pub fn start(domain: i32, store: Arc<dyn DurabilityStore>) -> Result<Self> {
        let factory = DomainParticipantFactory::instance();
        let ingest = factory.create_participant(domain, DomainParticipantQos::default())?;
        let replay = factory.create_participant(domain, DomainParticipantQos::default())?;
        // Race-free: mutual participant ignore BEFORE any reader/writer exists,
        // so the ingest reader and replay writer never match each other. Use the
        // GUID-derived `participant_handle()` (the discovery/ignore key space),
        // NOT `instance_handle()` (a local allocator counter that the ignore
        // filter never sees).
        let _ = ingest.ignore_participant(replay.participant_handle());
        let _ = replay.ignore_participant(ingest.participant_handle());
        let publisher = replay.create_publisher(PublisherQos::default());
        let subscriber = ingest.create_subscriber(SubscriberQos::default());
        Ok(Self {
            ingest,
            replay,
            publisher,
            subscriber,
            store,
            served: Mutex::new(Vec::new()),
        })
    }

    fn served(&self) -> Result<MutexGuard<'_, Vec<Served>>> {
        self.served
            .lock()
            .map_err(|_| ServiceError::Poisoned("served topics"))
    }

    /// Begins serving `topic_name` with the given retention `contract`. Creates
    /// the ingest reader + replay writer, primes the writer from the store
    /// (startup-sync), and spawns the ingest pump.
    ///
    /// # Errors
    /// Topic/reader/writer creation or the initial store/replay failed.
    pub fn serve(&self, topic_name: &str, contract: Contract) -> Result<()> {
        self.store.set_contract(topic_name, contract)?;

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
            .create_datawriter::<RawBytes>(&wtopic, replay_writer_qos())?;
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
        self.store.set_contract(topic_name, contract)?;

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

        // Startup-sync: prime the replay writer's history from durable storage.
        // Each stored sample carries its own representation + byte order; set
        // the writer overrides so the primed replay re-declares the original
        // wire (a big-endian peer's persisted sample replays with a BE encap).
        for sample in self.store.replay_for_topic(topic_name)? {
            let off: i16 = if sample.representation == 1 { 2 } else { 0 };
            let _ = rrt.set_user_writer_data_rep_override(weid, Some(vec![off]));
            let _ = rrt.set_user_writer_byte_order_override(weid, sample.big_endian);
            let _ = rrt.write_user_sample_borrowed(weid, &sample.payload);
        }

        let stop = Arc::new(AtomicBool::new(false));
        let pump_stop = Arc::clone(&stop);
        let store = Arc::clone(&self.store);
        let topic_owned = topic_name.to_string();
        let pump_rrt = Arc::clone(&rrt);
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
                    let (payload, representation, big_endian, writer_guid, source_seq) =
                        match sample {
                            UserSample::Alive {
                                payload,
                                representation,
                                big_endian,
                                writer_guid,
                                source_sequence_number,
                                ..
                            } => (
                                payload.to_vec(),
                                representation,
                                big_endian,
                                writer_guid,
                                source_sequence_number,
                            ),
                            UserSample::Lifecycle { .. } => continue,
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
                        instance_key: [0u8; 16], // unkeyed (increment 1)
                        sequence: seq.fetch_add(1, Ordering::Relaxed),
                        payload: payload.clone(),
                        representation,
                        big_endian,
                        created_at: SystemTime::now(),
                        source_guid: writer_guid,
                        source_sequence: source_seq,
                    };
                    if store.store(ds).is_ok() {
                        // Grow the replay writer's history for future late-joiners.
                        let _ = pump_rrt.write_user_sample_borrowed(weid, &payload);
                    }
                }
            })
            .map_err(|_| ServiceError::Poisoned("spawn pump"))?;

        self.served()?.push(Served {
            name: topic_name.to_string(),
            stop,
            handle: Some(handle),
        });
        Ok(())
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
    /// their `DurabilityQosPolicy`, and the service picks the topic up. Topics
    /// use `default_contract` (the `DurabilityServiceQosPolicy` is not carried
    /// in discovery; a per-topic override would come from service config).
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
                        if !s.info.valid_data || s.data.durability < DurabilityKind::Transient {
                            continue;
                        }
                        let topic = s.data.topic_name;
                        let type_name = s.data.type_name;
                        let already = me
                            .served_topics()
                            .map(|v| v.iter().any(|t| t == &topic))
                            .unwrap_or(true);
                        if !already {
                            // Runtime-level path keyed on the *discovered* type
                            // name → matches foreign-vendor writers too (the
                            // typed RawBytes topic is locked to
                            // `zerodds::RawBytes`). Increment-1 scope: unkeyed.
                            let _ = me.serve_typed(&topic, &type_name, false, default_contract);
                        }
                    }
                }
            })
            .map_err(|_| ServiceError::Poisoned("spawn auto-discovery"))?;
        self.served()?.push(Served {
            name: "<auto-discovery>".to_string(),
            stop,
            handle: Some(handle),
        });
        Ok(())
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
    }
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
                continue; // lifecycle marker, no payload
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
fn user_reader_cfg(topic_name: &str, type_name: &str) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic_name.to_string(),
        type_name: type_name.to_string(),
        reliable: true,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ownership: OwnershipKind::Shared,
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        // Cross-vendor byte-path: no local TypeObject (matched by topic+name).
        type_identifier: zerodds_types::TypeIdentifier::None,
        type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        // Accept BOTH XCDR1 (0) and XCDR2 (2): a foreign-vendor writer of a
        // FINAL-extensibility type offers only XCDR1 (verified live against
        // CycloneDDS — a final `dur::Sample` writer advertises
        // data_representation=0). An XCDR2-only reader is RxO-incompatible and
        // never matches it. A durability service must ingest whatever
        // representation the source emits, so it offers both.
        data_representation_offer: Some(vec![0, 2]),
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
        deadline: DeadlineQosPolicy::default(),
        lifespan: LifespanQosPolicy::default(),
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        type_identifier: zerodds_types::TypeIdentifier::None,
        data_representation_offer: None,
    }
}
