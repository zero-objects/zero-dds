// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! [`RealDdsPort`] — the [`DdsPort`] the daemon binary runs: one
//! `Arc<DcpsRuntime>` per DDS domain id (shared across every XRCE
//! participant that names it, so two clients in the same domain match
//! each other's endpoints, matching how `crates/coap-bridge`'s daemon
//! shares one `DcpsRuntime` per bridge process), raw-byte writer/reader
//! registration via `DcpsRuntime::register_user_writer_kind`/
//! `register_user_reader_kind` (the same primitive
//! `crates/routing-service/src/forwarding.rs` uses to forward samples
//! without a compiled Rust type), and `SerializedTypeSupport` when a
//! topic's `TypeObject` is known (spec item 5).

// Observability events (a leaked/unroutable endpoint, an unknown-type
// topic) go to stderr for now, same as `crates/coap-bridge`'s daemon
// modules — a structured sink is future work, not a silent drop.
#![allow(clippy::print_stderr)]

use std::collections::{BTreeMap, VecDeque};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use zerodds_dcps::participant::bridge_guid_prefix;
use zerodds_dcps::runtime::{
    DcpsRuntime, GroupTopicQos, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_dcps::serialized_type::SerializedTypeSupport;
use zerodds_rtps::wire_types::EntityId;

use crate::daemon::dds_port::{
    ContainerSpec, DdsPort, DdsPortError, Durability, EndpointQos, History, ObjectKey,
    ParticipantQos, PolledSample, SourceTime, TopicSpec, WriteOutcome,
};

/// What makes the GUID prefix of an agent's runtime its own: the process and a
/// counter of the runtimes this process started. The prefix was derived from
/// the domain id and the host alone, so two agents on one host (two processes,
/// or two daemons in one) announced the same GUID in one domain and never
/// discovered each other; clients at different agents could not exchange data
/// through DDS (DDS-XRCE 1.0 §10.2).
fn runtime_instance() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    format!(
        "pid{}-rt{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// One client's `OBJK_PARTICIPANT` object → the domain it bound and,
/// derived from that, which `Arc<DcpsRuntime>` its topics/endpoints live
/// on.
struct ParticipantState {
    /// The key of the runtime the participant lives on: the domain id for a
    /// participant on the runtime the participants of its domain share, a
    /// negative slot (see `RealDdsPort::own_runtimes`) for one with a runtime
    /// of its own.
    domain_id: i32,
    /// ENTITY_FACTORY `autoenable_created_entities` of the participant.
    autoenable_created_entities: bool,
}

/// One client's `OBJK_PUBLISHER` or `OBJK_SUBSCRIBER` object → the DDS
/// Publisher or Subscriber it stands for (DDS-XRCE 1.0 §7.5): the group
/// entity its endpoints announce (`PID_GROUP_GUID`) and its policies.
struct ContainerState {
    participant: ObjectKey,
    domain_id: i32,
    group: EntityId,
    spec: ContainerSpec,
}

/// One client's `OBJK_TOPIC` object → enough to create endpoints on it.
struct TopicState {
    domain_id: i32,
    spec: TopicSpec,
    /// `Some` when the topic's type is known (`spec.known_type`) — spec
    /// item 5's "type is known" path: `write()` uses it to compute a real
    /// `KeyHash` and to honor dispose/unregister. `None` means every
    /// sample on this topic is forwarded opaque.
    type_support: Option<Arc<SerializedTypeSupport>>,
    /// The type's own keyed-ness; `false` for an opaque topic.
    keyed: bool,
}

/// An endpoint made but not yet enabled ([`DdsPort::enable`]): its id is
/// reserved and its configuration complete, and the runtime has neither
/// registered nor announced it.
enum PendingEndpoint {
    Writer {
        eid: EntityId,
        cfg: UserWriterConfig,
        domain_id: i32,
        history_depth: usize,
        resource_limits: zerodds_qos::ResourceLimitsQosPolicy,
        max_blocking_time: zerodds_qos::Duration,
    },
    Reader {
        eid: EntityId,
        cfg: UserReaderConfig,
        domain_id: i32,
        samples: Sender<UserSample>,
    },
}

#[derive(Clone, Copy)]
enum EndpointKind {
    /// The `EntityId`, the `ObjectKey` of the topic it writes/reads —
    /// needed so `write()` can look the topic's `type_support` back up —
    /// and the domain whose runtime holds it (kept here so the endpoint
    /// can be torn down after its topic is gone).
    Writer(EntityId, ObjectKey, i32),
    /// The real receive path is `spawn_reader_pump`'s dedicated thread
    /// draining into `reader_buffers`, not a re-poll of the
    /// `mpsc::Receiver` `register_user_reader_kind` returns — that
    /// receiver is moved into the pump thread directly and never stored
    /// here.
    Reader(EntityId, ObjectKey, i32),
}

impl EndpointKind {
    fn entity_id(self) -> EntityId {
        match self {
            Self::Writer(eid, _, _) | Self::Reader(eid, _, _) => eid,
        }
    }

    fn domain_id(self) -> i32 {
        match self {
            Self::Writer(_, _, d) | Self::Reader(_, _, d) => d,
        }
    }
}

/// One sample a reader's pump thread buffered, with enough of its wire
/// coordinates (`representation`/`big_endian`, RTPS 2.5 §10.5) to decode
/// it again for [`RealDdsPort::set_read_filter`] — the buffer itself
/// stores raw opaque bytes (spec item 5: an XRCE client decodes with its
/// own generated type support), so this is the only place that
/// information would otherwise be lost.
struct BufferedSample {
    payload: Vec<u8>,
    /// As `UserSample::Alive` carries it: 0 = XCDR1, 1 = XCDR2.
    representation: u8,
    big_endian: bool,
    /// The instance, `None` for an unkeyed or opaque topic (one instance).
    instance: Option<[u8; 16]>,
    /// `SampleInfoFlags` (DDS-XRCE 1.0 Annex A): the instance state of a
    /// dispose or unregister marker; `VIEW_STATE_NEW` is set when the sample
    /// enters the queue as the first of its instance.
    state: u8,
    /// The writer's source timestamp, if it sent one.
    source_time: Option<(i32, u32)>,
}

/// How many samples one reader's queue holds at most, across its
/// instances — the bound on what an XRCE client that stops reading can
/// make the agent hold.
const READER_QUEUE_CAP: usize = 1024;

/// One reader's queue between its pump thread and `poll_samples`, kept by
/// the reader's own HISTORY and TIME_BASED_FILTER (DDS 1.4 §2.2.3.18,
/// §2.2.3.12) the way a DataReader's cache is.
struct ReaderQueue {
    samples: VecDeque<BufferedSample>,
    history: History,
    minimum_separation: Option<std::time::Duration>,
    /// When the last sample of each instance was accepted, for the
    /// time-based filter.
    last_accepted: BTreeMap<Option<[u8; 16]>, Instant>,
    /// Samples refused because the queue was at [`READER_QUEUE_CAP`]
    /// under KEEP_ALL — each one reported.
    refused: u64,
    /// The instances this reader has seen a sample of, for `VIEW_STATE_NEW`.
    seen: std::collections::BTreeSet<Option<[u8; 16]>>,
}

impl ReaderQueue {
    fn new(qos: &EndpointQos) -> Self {
        Self {
            samples: VecDeque::new(),
            history: qos.history,
            minimum_separation: qos.time_based_filter.filter(|d| !d.is_zero()),
            last_accepted: BTreeMap::new(),
            refused: 0,
            seen: std::collections::BTreeSet::new(),
        }
    }

    /// Takes one sample in. KEEP_LAST(n) replaces the instance's oldest
    /// sample once it holds `n` (the policy's own semantics, §2.2.3.18);
    /// a sample within the minimum separation of the instance's last one
    /// is not delivered (§2.2.3.12). Returns `false` when the queue is at
    /// its cap and the sample could not be kept — the caller reports it.
    fn push(&mut self, mut sample: BufferedSample, now: Instant) -> bool {
        // A dispose or unregister marker is a state change of the instance,
        // not a sample of the history: no separation filter, no replacement.
        let marker = sample.state & 0b11 != 0;
        if self.seen.insert(sample.instance) {
            sample.state |= zerodds_xrce::sample_info::SampleInfoFlags::VIEW_STATE_NEW;
        }
        if marker {
            if self.samples.len() >= READER_QUEUE_CAP {
                self.refused += 1;
                return false;
            }
            self.samples.push_back(sample);
            return true;
        }
        if let Some(min) = self.minimum_separation {
            if let Some(last) = self.last_accepted.get(&sample.instance) {
                if now.duration_since(*last) < min {
                    return true;
                }
            }
        }
        if let History::KeepLast(depth) = self.history {
            let depth = usize::try_from(depth).unwrap_or(usize::MAX);
            let held = self
                .samples
                .iter()
                .filter(|s| s.instance == sample.instance)
                .count();
            if held >= depth {
                if let Some(pos) = self
                    .samples
                    .iter()
                    .position(|s| s.instance == sample.instance)
                {
                    self.samples.remove(pos);
                }
            }
        }
        if self.samples.len() >= READER_QUEUE_CAP {
            self.refused += 1;
            return false;
        }
        self.last_accepted.insert(sample.instance, now);
        self.samples.push_back(sample);
        true
    }
}

/// A parsed `content_filter_expression` (spec §7.7.14), bound to the
/// topic's [`SerializedTypeSupport`] it is evaluated against — READ_DATA
/// carries no separate parameter list (unlike `ContentFilterProperty`'s
/// `%N` parameters, which are a CREATE(reader)-time, SEDP-announced
/// concept this is not), so `Expr::evaluate` always gets an empty slice.
struct CompiledReadFilter {
    expr: zerodds_sql_filter::Expr,
    type_support: Arc<SerializedTypeSupport>,
}

impl CompiledReadFilter {
    /// Parses `expr_str` against the topic's type; `what` names the
    /// filter's origin in the error.
    fn compile(
        expr_str: &str,
        type_support: Option<Arc<SerializedTypeSupport>>,
        what: &str,
    ) -> Result<Self, DdsPortError> {
        let type_support = type_support.ok_or_else(|| {
            DdsPortError::InvalidType(format!(
                "{what} `{expr_str}`: the topic's type is unknown — no decoder to evaluate field \
                 paths against"
            ))
        })?;
        let expr = zerodds_sql_filter::parse(expr_str).map_err(|e| {
            DdsPortError::InvalidType(format!("{what} `{expr_str}` does not parse: {e:?}"))
        })?;
        Ok(Self { expr, type_support })
    }

    /// A sample the filter denies, or that does not decode or evaluate,
    /// is not delivered — the filter is the reader's request to narrow
    /// delivery.
    fn admits(&self, s: &BufferedSample) -> bool {
        // A marker has no data to evaluate; it is a state change of an
        // instance the reader has been told of.
        if s.state & 0b11 != 0 {
            return true;
        }
        let Ok(sample) = self.type_support.read_body(
            &s.payload,
            zerodds_dcps::serialized_type::read_body_representation(s.representation),
            s.big_endian,
        ) else {
            return false;
        };
        let row = zerodds_dcps::dds_type::DdsTypeRow::new(&sample);
        self.expr.evaluate(&row, &[]).unwrap_or(false)
    }
}

/// A QoS duration; `None` is DDS `DURATION_INFINITE`, and so is a finite
/// one past the 32-bit seconds range (more than 68 years).
fn qos_duration(d: Option<std::time::Duration>) -> zerodds_qos::Duration {
    let Some(d) = d else {
        return zerodds_qos::Duration::INFINITE;
    };
    match i32::try_from(d.as_secs()) {
        // 2^-32 s units, rounded to the nearest.
        Ok(seconds) if seconds < i32::MAX => {
            zerodds_qos::Duration::from_sec_nanosec(seconds, d.subsec_nanos())
        }
        _ => zerodds_qos::Duration::INFINITE,
    }
}

fn durability_kind(d: Durability) -> zerodds_qos::DurabilityKind {
    match d {
        Durability::Volatile => zerodds_qos::DurabilityKind::Volatile,
        Durability::TransientLocal => zerodds_qos::DurabilityKind::TransientLocal,
        Durability::Transient => zerodds_qos::DurabilityKind::Transient,
        Durability::Persistent => zerodds_qos::DurabilityKind::Persistent,
    }
}

fn ownership_kind(exclusive: bool) -> zerodds_qos::OwnershipKind {
    if exclusive {
        zerodds_qos::OwnershipKind::Exclusive
    } else {
        zerodds_qos::OwnershipKind::Shared
    }
}

/// The RTPS `Time_t` of a source time.
fn he_timestamp(t: SourceTime) -> zerodds_rtps::header_extension::HeTimestamp {
    let d = zerodds_qos::Duration::from_sec_nanosec(t.seconds, t.nanoseconds);
    zerodds_rtps::header_extension::HeTimestamp {
        seconds: u32::from_ne_bytes(d.seconds.to_ne_bytes()),
        fraction: d.fraction,
    }
}

/// The `StatusInfo` bits (RTPS 2.5 §9.6.3.9) of a dispose and/or unregister
/// marker.
fn lifecycle_status_bits(dispose: bool, unregister: bool) -> u32 {
    use zerodds_rtps::inline_qos::status_info::{DISPOSED, UNREGISTERED};
    let mut bits = 0u32;
    if dispose {
        bits |= DISPOSED;
    }
    if unregister {
        bits |= UNREGISTERED;
    }
    bits
}

/// The real DCPS bridge.
pub struct RealDdsPort {
    runtimes: BTreeMap<i32, Arc<DcpsRuntime>>,
    /// The runtimes of participants whose policies need one of their own
    /// (USER_DATA is part of the discovery data of a runtime's participant):
    /// slot (negative, the key in `runtimes`) -> (domain, USER_DATA).
    own_runtimes: BTreeMap<i32, (i32, Vec<u8>)>,
    runtime_config_template: RuntimeConfig,
    participants: BTreeMap<ObjectKey, ParticipantState>,
    containers: BTreeMap<ObjectKey, ContainerState>,
    topics: BTreeMap<ObjectKey, TopicState>,
    /// The participant each topic was created under.
    topic_participants: BTreeMap<ObjectKey, ObjectKey>,
    endpoints: BTreeMap<ObjectKey, EndpointKind>,
    /// The endpoints in `endpoints` that [`DdsPort::enable`] has not
    /// registered yet.
    pending: BTreeMap<ObjectKey, PendingEndpoint>,
    /// Buffered samples per reader, drained by `poll_samples` — filled by
    /// a background pump the daemon spawns per reader (see
    /// `crate::daemon::server`), because `mpsc::Receiver::try_recv` on
    /// every `poll_samples` call would work too but this keeps the
    /// non-blocking contract explicit at the type level.
    reader_buffers: Arc<Mutex<BTreeMap<ObjectKey, ReaderQueue>>>,
    /// The content filter, if any, the last `set_read_filter` call
    /// installed for a reader — applied in [`Self::drain_buffer`].
    read_filters: BTreeMap<ObjectKey, CompiledReadFilter>,
    /// The content filter a reader was created with (its QoS,
    /// `OBJK_DataReader_Binary_Qos.contentbased_filter`) — applied in
    /// [`Self::drain_buffer`] together with, not replaced by, a
    /// READ_DATA's own.
    reader_filters: BTreeMap<ObjectKey, CompiledReadFilter>,
}

impl RealDdsPort {
    /// Constructs a port with no runtimes yet — one is created lazily per
    /// distinct domain id the first time a participant names it.
    /// `runtime_config_template` is cloned per domain (only its
    /// `discovery`/security/transport settings matter across domains; the
    /// domain id itself is threaded through by `DcpsRuntime::start`).
    #[must_use]
    pub fn new(runtime_config_template: RuntimeConfig) -> Self {
        Self {
            runtimes: BTreeMap::new(),
            own_runtimes: BTreeMap::new(),
            runtime_config_template,
            participants: BTreeMap::new(),
            containers: BTreeMap::new(),
            topics: BTreeMap::new(),
            topic_participants: BTreeMap::new(),
            endpoints: BTreeMap::new(),
            pending: BTreeMap::new(),
            reader_buffers: Arc::new(Mutex::new(BTreeMap::new())),
            read_filters: BTreeMap::new(),
            reader_filters: BTreeMap::new(),
        }
    }

    /// The runtime slot of a participant with USER_DATA: one runtime per
    /// distinct (domain, USER_DATA); the slot is allocated here, the runtime is
    /// started when the participant is enabled (`runtime_for`).
    fn own_runtime_slot(&mut self, domain_id: i32, user_data: &[u8]) -> Result<i32, DdsPortError> {
        if let Some((slot, _)) = self
            .own_runtimes
            .iter()
            .find(|(_, (d, u))| *d == domain_id && u == user_data)
        {
            return Ok(*slot);
        }
        let slot = self
            .own_runtimes
            .keys()
            .next()
            .map_or(-1, |lowest| lowest - 1);
        self.own_runtimes
            .insert(slot, (domain_id, user_data.to_vec()));
        Ok(slot)
    }

    fn runtime_for(&mut self, domain_id: i32) -> Result<Arc<DcpsRuntime>, DdsPortError> {
        if let Some(rt) = self.runtimes.get(&domain_id) {
            return Ok(Arc::clone(rt));
        }
        if domain_id < 0 {
            // The runtime of a participant with USER_DATA, started now.
            let Some((domain, user_data)) = self.own_runtimes.get(&domain_id).cloned() else {
                return Err(DdsPortError::UnknownReference(format!(
                    "runtime slot {domain_id} of a participant does not exist"
                )));
            };
            let hex: String = user_data.iter().map(|b| format!("{b:02x}")).collect();
            let prefix = bridge_guid_prefix(&format!(
                "zerodds-xrce-agentd-domain-{domain}-participant-user-data-{hex}-{}",
                runtime_instance()
            ));
            let mut config = self.runtime_config_template.clone();
            config.user_data = user_data;
            let rt = DcpsRuntime::start(domain, prefix, config).map_err(|e| {
                DdsPortError::DdsError(format!("DcpsRuntime::start(domain {domain}): {e:?}"))
            })?;
            self.runtimes.insert(domain_id, Arc::clone(&rt));
            return Ok(rt);
        }
        let prefix = bridge_guid_prefix(&format!(
            "zerodds-xrce-agentd-domain-{domain_id}-{}",
            runtime_instance()
        ));
        let rt = DcpsRuntime::start(domain_id, prefix, self.runtime_config_template.clone())
            .map_err(|e| {
                DdsPortError::DdsError(format!("DcpsRuntime::start(domain {domain_id}): {e:?}"))
            })?;
        self.runtimes.insert(domain_id, Arc::clone(&rt));
        Ok(rt)
    }

    /// The Publisher or Subscriber an endpoint is created with, checked
    /// against the topic: same participant, and its ENTITY_FACTORY enables
    /// the entities it creates (DDS 1.4 §2.2.3.20; XRCE has no operation
    /// that enables a disabled entity later).
    fn container_for(
        &self,
        what: &str,
        container: ObjectKey,
        topic: &TopicState,
        topic_key: ObjectKey,
    ) -> Result<&ContainerState, DdsPortError> {
        let state = self.containers.get(&container).ok_or_else(|| {
            DdsPortError::UnknownReference(format!("{what}: Publisher/Subscriber not created"))
        })?;
        if state.domain_id != topic.domain_id
            || self.topic_participants.get(&topic_key) != Some(&state.participant)
        {
            return Err(DdsPortError::UnknownReference(format!(
                "{what}: the topic and the Publisher/Subscriber belong to different participants"
            )));
        }
        if !self
            .participants
            .get(&state.participant)
            .is_none_or(|p| p.autoenable_created_entities)
        {
            return Err(DdsPortError::DdsError(format!(
                "{what}: the ENTITY_FACTORY QoS of the participant does not enable the \
                 entities it creates, and XRCE has no operation that enables them"
            )));
        }
        if !state.spec.autoenable_created_entities {
            return Err(DdsPortError::DdsError(format!(
                "{what}: the ENTITY_FACTORY QoS of the Publisher/Subscriber does not enable the \
                 entities it creates, and XRCE has no operation that enables them"
            )));
        }
        Ok(state)
    }

    fn create_container(
        &mut self,
        what: &str,
        kind: zerodds_rtps::wire_types::EntityKind,
        key: ObjectKey,
        participant: ObjectKey,
        spec: ContainerSpec,
    ) -> Result<(), DdsPortError> {
        let domain_id = self
            .participants
            .get(&participant)
            .ok_or_else(|| {
                DdsPortError::UnknownReference(format!("{what}: participant not created"))
            })?
            .domain_id;
        let rt = self.runtime_for(domain_id)?;
        let group = rt.new_group_entity_id(kind);
        self.containers.insert(
            key,
            ContainerState {
                participant,
                domain_id,
                group,
                spec,
            },
        );
        Ok(())
    }

    /// Drains samples a reader's pump thread buffered (see
    /// [`Self::spawn_reader_pump`]), applying whatever content filter
    /// [`DdsPort::set_read_filter`] last installed for `key` — spec
    /// §7.7.14: "Only samples for which the filter evaluates to TRUE
    /// shall be returned to the XRCE Client." A sample the filter denies
    /// (or that errors evaluating, e.g. an unknown field path) is dropped
    /// from the reader's stream entirely, same as
    /// `crates/routing-service`'s `CompiledFilter::matches`: a filter
    /// expression is the caller's explicit request to narrow delivery, so
    /// "denied" and "unevaluable" both mean "do not deliver", not
    /// "deliver anyway".
    fn drain_buffer(&self, key: ObjectKey, max: usize) -> Vec<PolledSample> {
        let Ok(mut guard) = self.reader_buffers.lock() else {
            return Vec::new();
        };
        let Some(buf) = guard.get_mut(&key) else {
            return Vec::new();
        };
        let take = max.min(buf.samples.len());
        let drained: Vec<BufferedSample> = buf.samples.drain(0..take).collect();
        drop(guard);

        let filters: Vec<&CompiledReadFilter> =
            [self.reader_filters.get(&key), self.read_filters.get(&key)]
                .into_iter()
                .flatten()
                .collect();
        drained
            .into_iter()
            .filter(|s| filters.iter().all(|f| f.admits(s)))
            .map(|s| PolledSample {
                bytes: s.payload,
                state: s.state,
                source_time: s.source_time,
            })
            .collect()
    }

    fn spawn_reader_pump(&self, key: ObjectKey, rx: Receiver<UserSample>, qos: &EndpointQos) {
        let buffers = Arc::clone(&self.reader_buffers);
        if let Ok(mut guard) = buffers.lock() {
            guard.insert(key, ReaderQueue::new(qos));
        }
        std::thread::spawn(move || {
            while let Ok(sample) = rx.recv() {
                let (key_hash, payload, representation, big_endian, state, source_time) =
                    match sample {
                        UserSample::Alive {
                            key_hash,
                            payload,
                            representation,
                            big_endian,
                            source_timestamp,
                            ..
                        } => (
                            key_hash,
                            payload.to_vec(),
                            representation,
                            big_endian,
                            0u8,
                            source_timestamp
                                .map(|t| (i32::from_ne_bytes(t.seconds.to_ne_bytes()), t.fraction)),
                        ),
                        // DDS-XRCE 1.0 Annex A: `SampleInfoFlags` carries the
                        // instance state, so a dispose or unregister goes to
                        // the client as a sample of the formats that have
                        // the info (empty when the marker holds no key).
                        UserSample::Lifecycle {
                            key_hash,
                            kind,
                            key_holder,
                            source_timestamp,
                            ..
                        } => {
                            use zerodds_rtps::history_cache::ChangeKind;
                            let state = match kind {
                                ChangeKind::NotAliveDisposed => 0b10,
                                ChangeKind::NotAliveUnregistered => 0b01,
                                ChangeKind::NotAliveDisposedUnregistered => 0b11,
                                _ => continue,
                            };
                            (
                                Some(key_hash),
                                key_holder.unwrap_or_default(),
                                1,
                                true,
                                state,
                                source_timestamp.map(|t| {
                                    (i32::from_ne_bytes(t.seconds.to_ne_bytes()), t.fraction)
                                }),
                            )
                        }
                        _ => continue, // coherent-set markers: nothing to emit
                    };
                let Ok(mut guard) = buffers.lock() else {
                    break;
                };
                // A reader deleted meanwhile has no queue: the pump ends.
                let Some(queue) = guard.get_mut(&key) else {
                    break;
                };
                let kept = queue.push(
                    BufferedSample {
                        payload,
                        representation,
                        big_endian,
                        instance: key_hash,
                        state,
                        source_time,
                    },
                    Instant::now(),
                );
                if !kept {
                    eprintln!(
                        "[zerodds-xrce-agentd] reader {:04x} (client {:02x?}): KEEP_ALL queue at \
                         {READER_QUEUE_CAP} samples, the client is not reading — sample not kept \
                         ({} so far)",
                        key.1.raw(),
                        key.0,
                        queue.refused
                    );
                }
            }
        });
    }
}

impl DdsPort for RealDdsPort {
    fn create_participant(&mut self, key: ObjectKey, domain_id: i32) -> Result<(), DdsPortError> {
        self.create_participant_with(key, domain_id, &ParticipantQos::default())
    }

    /// Applies the policies of a DomainParticipant: ENTITY_FACTORY is held by
    /// the participant here (what its Publishers, Subscribers and Topics may
    /// create); USER_DATA is part of the discovery data of the DCPS
    /// participant, so a participant with USER_DATA gets a runtime of its own.
    fn create_participant_with(
        &mut self,
        key: ObjectKey,
        domain_id: i32,
        qos: &ParticipantQos,
    ) -> Result<(), DdsPortError> {
        // RTPS 9.6.1.1: the ports of a domain exist for the ids 0 to 232. The
        // participant is created disabled (DDS-XRCE 1.0 §7.8.3.1): no runtime
        // starts, and nothing is announced, before `enable`.
        if !(0..=232).contains(&domain_id) {
            return Err(DdsPortError::DdsError(format!(
                "domain id {domain_id} is outside 0..=232"
            )));
        }
        let slot = if qos.user_data.is_empty() {
            domain_id
        } else {
            self.own_runtime_slot(domain_id, &qos.user_data)?
        };
        self.participants.insert(
            key,
            ParticipantState {
                domain_id: slot,
                autoenable_created_entities: qos.autoenable_created_entities,
            },
        );
        Ok(())
    }

    fn create_topic(
        &mut self,
        key: ObjectKey,
        participant: ObjectKey,
        spec: TopicSpec,
    ) -> Result<(), DdsPortError> {
        let domain_id = self
            .participants
            .get(&participant)
            .ok_or_else(|| {
                DdsPortError::UnknownReference("create_topic: participant not created".into())
            })?
            .domain_id;
        let type_support = Self::type_support_for(&spec)?;
        let keyed = type_support.as_ref().is_some_and(|ts| ts.is_keyed());
        self.topic_participants.insert(key, participant);
        self.topics.insert(
            key,
            TopicState {
                domain_id,
                spec,
                type_support,
                keyed,
            },
        );
        Ok(())
    }

    fn create_publisher(
        &mut self,
        key: ObjectKey,
        participant: ObjectKey,
        spec: ContainerSpec,
    ) -> Result<(), DdsPortError> {
        self.create_container(
            "create_publisher",
            zerodds_rtps::wire_types::EntityKind::UserWriterGroup,
            key,
            participant,
            spec,
        )
    }

    fn create_subscriber(
        &mut self,
        key: ObjectKey,
        participant: ObjectKey,
        spec: ContainerSpec,
    ) -> Result<(), DdsPortError> {
        self.create_container(
            "create_subscriber",
            zerodds_rtps::wire_types::EntityKind::UserReaderGroup,
            key,
            participant,
            spec,
        )
    }

    fn create_writer(
        &mut self,
        key: ObjectKey,
        topic: ObjectKey,
        publisher: ObjectKey,
        qos: EndpointQos,
    ) -> Result<(), DdsPortError> {
        let topic_state = self.topics.get(&topic).ok_or_else(|| {
            DdsPortError::UnknownReference("create_writer: topic not created".into())
        })?;
        let container = self.container_for("create_writer", publisher, topic_state, topic)?;
        let (group, presentation) = (container.group, container.spec.presentation);
        let (domain_id, topic_name, type_name, keyed) = (
            topic_state.domain_id,
            topic_state.spec.topic_name.clone(),
            topic_state.spec.type_name.clone(),
            topic_state.keyed,
        );
        let rt = self.runtime_for(domain_id)?;
        let cfg = UserWriterConfig {
            topic_name,
            group_entity_id: Some(group),
            presentation,
            type_name,
            reliable: qos.reliable,
            durability: durability_kind(qos.durability),
            deadline: zerodds_qos::DeadlineQosPolicy {
                period: qos_duration(qos.deadline),
            },
            lifespan: zerodds_qos::LifespanQosPolicy {
                duration: qos_duration(qos.lifespan),
            },
            ownership: ownership_kind(qos.exclusive_ownership),
            ownership_strength: qos.ownership_strength,
            partition: qos.partitions,
            user_data: qos.user_data,
            group_data: qos.group_data,
            topic_data: qos.topic_data,
            liveliness: qos.liveliness,
            latency_budget: qos.latency_budget,
            destination_order: qos.destination_order,
            transport_priority: qos.transport_priority,
            durability_service: qos.durability_service,
            ..Default::default()
        };
        // §7.8.3.1: created disabled, registered (and so announced) by
        // `enable`, once the XRCE object is complete.
        let eid = rt.reserve_user_writer_id(keyed);
        let history_depth = match qos.history {
            History::KeepLast(d) => usize::try_from(d).unwrap_or(usize::MAX),
            History::KeepAll => usize::MAX,
        };
        self.pending.insert(
            key,
            PendingEndpoint::Writer {
                eid,
                cfg,
                domain_id,
                history_depth,
                resource_limits: qos.resource_limits,
                max_blocking_time: qos_duration(qos.max_blocking_time),
            },
        );
        self.endpoints
            .insert(key, EndpointKind::Writer(eid, topic, domain_id));
        Ok(())
    }

    fn create_reader(
        &mut self,
        key: ObjectKey,
        topic: ObjectKey,
        subscriber: ObjectKey,
        qos: EndpointQos,
    ) -> Result<(), DdsPortError> {
        let topic_state = self.topics.get(&topic).ok_or_else(|| {
            DdsPortError::UnknownReference("create_reader: topic not created".into())
        })?;
        let container = self.container_for("create_reader", subscriber, topic_state, topic)?;
        let (group, presentation) = (container.group, container.spec.presentation);
        let (domain_id, topic_name, type_name, keyed) = (
            topic_state.domain_id,
            topic_state.spec.topic_name.clone(),
            topic_state.spec.type_name.clone(),
            topic_state.keyed,
        );
        // The reader's own content filter compiles before anything is
        // registered: a filter no decoder can evaluate refuses the CREATE.
        let own_filter = match qos.content_filter.as_deref() {
            Some(expr) => Some(CompiledReadFilter::compile(
                expr,
                topic_state.type_support.clone(),
                "the reader's content filter",
            )?),
            None => None,
        };
        let rt = self.runtime_for(domain_id)?;
        let cfg = UserReaderConfig {
            topic_name,
            group_entity_id: Some(group),
            presentation,
            type_name,
            reliable: qos.reliable,
            durability: durability_kind(qos.durability),
            deadline: zerodds_qos::DeadlineQosPolicy {
                period: qos_duration(qos.deadline),
            },
            ownership: ownership_kind(qos.exclusive_ownership),
            partition: qos.partitions.clone(),
            user_data: qos.user_data.clone(),
            group_data: qos.group_data.clone(),
            topic_data: qos.topic_data.clone(),
            liveliness: qos.liveliness,
            latency_budget: qos.latency_budget,
            destination_order: qos.destination_order,
            resource_limits: qos.resource_limits,
            ..Default::default()
        };
        let eid = rt.reserve_user_reader_id(keyed);
        let (samples, rx) = std::sync::mpsc::channel();
        self.spawn_reader_pump(key, rx, &qos);
        if let Some(filter) = own_filter {
            self.reader_filters.insert(key, filter);
        }
        self.pending.insert(
            key,
            PendingEndpoint::Reader {
                eid,
                cfg,
                domain_id,
                samples,
            },
        );
        self.endpoints
            .insert(key, EndpointKind::Reader(eid, topic, domain_id));
        Ok(())
    }

    fn enable(&mut self, key: ObjectKey) -> Result<(), DdsPortError> {
        // A participant is enabled now that its CREATE has succeeded: only
        // now its runtime starts and announces it.
        if let Some(slot) = self.participants.get(&key).map(|p| p.domain_id) {
            self.runtime_for(slot)?;
            return Ok(());
        }
        let Some(pending) = self.pending.remove(&key) else {
            return Ok(());
        };
        let registered = match pending {
            PendingEndpoint::Writer {
                eid,
                cfg,
                domain_id,
                history_depth,
                resource_limits,
                max_blocking_time,
            } => {
                let rt = self.runtime_for(domain_id)?;
                rt.register_user_writer_as(eid, cfg, GroupTopicQos::default())
                    .map_err(|e| DdsPortError::DdsError(format!("register_user_writer: {e:?}")))
                    .and_then(|()| {
                        rt.set_user_writer_history_depth(eid, history_depth)
                            .and_then(|()| {
                                rt.set_user_writer_resource_limits(
                                    eid,
                                    resource_limits,
                                    max_blocking_time,
                                )
                            })
                            .map_err(|e| {
                                rt.remove_user_writer(eid);
                                DdsPortError::DdsError(format!("user writer QoS: {e:?}"))
                            })
                    })
            }
            PendingEndpoint::Reader {
                eid,
                cfg,
                domain_id,
                samples,
            } => {
                let rt = self.runtime_for(domain_id)?;
                rt.register_user_reader_as(eid, cfg, GroupTopicQos::default(), samples)
                    .map_err(|e| DdsPortError::DdsError(format!("register_user_reader: {e:?}")))
            }
        };
        if registered.is_err() {
            self.delete(key);
        }
        registered
    }

    fn write(
        &mut self,
        writer: ObjectKey,
        payload: &[u8],
        big_endian: bool,
        dispose: bool,
        unregister: bool,
        source_time: Option<SourceTime>,
    ) -> Result<WriteOutcome, DdsPortError> {
        let Some(EndpointKind::Writer(eid, topic, domain_id)) =
            self.endpoints.get(&writer).copied()
        else {
            return Err(DdsPortError::UnknownReference(
                "write: writer not created".into(),
            ));
        };
        let Some(rt) = self.runtimes.get(&domain_id) else {
            return Err(DdsPortError::DdsError(
                "write: no runtime for writer's domain".into(),
            ));
        };
        let type_support = self.topics.get(&topic).and_then(|t| t.type_support.clone());
        let stamp = source_time.map(he_timestamp);

        // `key_material` carries both the `KeyHash` and the raw key-holder
        // bytes (`SerializedTypeSupport::key_material`), so a dispose/
        // unregister on a known, keyed type can send `write_user_lifecycle`
        // a real `serializedKey` (RTPS 2.5 §9.4.2.11) instead of only the
        // `PID_KEY_HASH`-only marker Fast DDS/Cyclone/OpenDDS don't all
        // read as a dispose.
        let key_material = match &type_support {
            Some(ts) => match ts.read_body(payload, 2, big_endian) {
                Ok(sample) => ts.key_material(&sample),
                Err(e) => {
                    return Err(DdsPortError::InvalidType(format!(
                        "write: payload does not decode as the topic's known type: {e}"
                    )));
                }
            },
            None => None,
        };

        if dispose || unregister {
            // DDS-XRCE 1.0 §7.8.4.1: "the XRCE Agent shall call the dispose
            // [unregister] operation on the corresponding DDS DataWriter for
            // the instance identified in the associated data." The instance
            // is identified by its key; without a keyed type there is none,
            // and a plain update would be something else than the client
            // asked for, so nothing is written.
            let Some((key_hash, key_holder)) = &key_material else {
                return Err(DdsPortError::InvalidType(
                    "dispose/unregister names an instance by its key; the topic's type is \
                     unknown or has no key"
                        .into(),
                ));
            };
            rt.write_user_lifecycle(
                eid,
                *key_hash,
                Some(key_holder.as_slice()),
                lifecycle_status_bits(dispose, unregister),
                stamp,
            )
            .map_err(|e| DdsPortError::DdsError(format!("write_user_lifecycle: {e:?}")))?;
            return Ok(WriteOutcome::Disposed);
        }

        match (key_material, stamp) {
            (Some((key_hash, _)), stamp) => rt
                .write_user_sample_keyed(eid, payload, key_hash, stamp)
                .map_err(|e| DdsPortError::DdsError(format!("write_user_sample_keyed: {e:?}"))),
            (None, Some(stamp)) => rt
                .write_user_sample_stamped(eid, payload, stamp)
                .map_err(|e| DdsPortError::DdsError(format!("write_user_sample_stamped: {e:?}"))),
            (None, None) => rt
                .write_user_sample_borrowed(eid, payload)
                .map_err(|e| DdsPortError::DdsError(format!("write_user_sample: {e:?}"))),
        }?;
        Ok(WriteOutcome::Written)
    }

    fn sample_length(
        &mut self,
        writer: ObjectKey,
        bytes: &[u8],
        big_endian: bool,
    ) -> Result<usize, DdsPortError> {
        let Some(EndpointKind::Writer(_, topic, _)) = self.endpoints.get(&writer).copied() else {
            return Err(DdsPortError::UnknownReference(
                "sample_length: writer not created".into(),
            ));
        };
        let Some(ts) = self.topics.get(&topic).and_then(|t| t.type_support.clone()) else {
            return Err(DdsPortError::InvalidType(
                "the topic's type is unknown: the samples of a sequence cannot be told apart"
                    .into(),
            ));
        };
        ts.read_body_prefix(bytes, 2, big_endian)
            .map(|(_, taken)| taken)
            .map_err(|e| {
                DdsPortError::InvalidType(format!(
                    "the bytes do not start with a sample of the topic's type: {e}"
                ))
            })
    }

    fn set_read_filter(
        &mut self,
        reader: ObjectKey,
        filter: Option<&str>,
    ) -> Result<(), DdsPortError> {
        let Some(kind) = self.endpoints.get(&reader).copied() else {
            return Err(DdsPortError::UnknownReference(
                "set_read_filter: reader not created".into(),
            ));
        };
        let EndpointKind::Reader(_, topic, _) = kind else {
            return Err(DdsPortError::UnknownReference(
                "set_read_filter: not a reader".into(),
            ));
        };
        let Some(expr_str) = filter else {
            self.read_filters.remove(&reader);
            return Ok(());
        };
        let filter = CompiledReadFilter::compile(
            expr_str,
            self.topics.get(&topic).and_then(|t| t.type_support.clone()),
            "READ_DATA content_filter_expression",
        )?;
        self.read_filters.insert(reader, filter);
        Ok(())
    }

    fn poll_samples(&mut self, reader: ObjectKey, max: usize) -> Vec<PolledSample> {
        self.drain_buffer(reader, max)
    }

    fn delete(&mut self, key: ObjectKey) {
        self.participants.remove(&key);
        self.containers.remove(&key);
        self.topics.remove(&key);
        self.topic_participants.remove(&key);
        let pending = self.pending.remove(&key).is_some();
        if let Some(kind) = self.endpoints.remove(&key) {
            // An endpoint that was never enabled is not in the runtime.
            if !pending {
                self.tear_down_endpoint(key, kind);
            }
        }
        if let Ok(mut guard) = self.reader_buffers.lock() {
            guard.remove(&key);
        }
        self.read_filters.remove(&key);
        self.reader_filters.remove(&key);
    }

    fn unregister_client(&mut self, client_key: [u8; zerodds_xrce::header::CLIENT_KEY_LEN]) {
        self.participants.retain(|(ck, _), _| *ck != client_key);
        self.containers.retain(|(ck, _), _| *ck != client_key);
        self.topics.retain(|(ck, _), _| *ck != client_key);
        self.topic_participants
            .retain(|(ck, _), _| *ck != client_key);
        let removed: Vec<(ObjectKey, EndpointKind)> = {
            let mut removed = Vec::new();
            self.endpoints.retain(|k, kind| {
                if k.0 != client_key {
                    return true;
                }
                removed.push((*k, *kind));
                false
            });
            removed
        };
        for (key, kind) in removed {
            self.tear_down_endpoint(key, kind);
        }
        if let Ok(mut guard) = self.reader_buffers.lock() {
            guard.retain(|(ck, _), _| *ck != client_key);
        }
        self.read_filters.retain(|(ck, _), _| *ck != client_key);
        self.reader_filters.retain(|(ck, _), _| *ck != client_key);
    }
}

impl RealDdsPort {
    /// Tears down one endpoint's DCPS-side state: `remove_user_writer`/
    /// `remove_user_reader` (drops the RTPS writer/reader slot, rebuilds
    /// intra-runtime routes, and — the part that matters for a remote
    /// peer — sends an SEDP dispose so it drops the matched endpoint at
    /// once instead of waiting out a liveliness timeout). Without this a
    /// deleted XRCE DataWriter/DataReader left a "ghost" DCPS endpoint
    /// registered in the runtime forever: still discoverable, still
    /// counted, never disposed. Called from `delete()` and
    /// `unregister_client()`, after the entry is already gone from
    /// `self.endpoints` — a runtime lookup failure here (the domain's
    /// `DcpsRuntime` no longer present) is logged, not silently dropped.
    fn tear_down_endpoint(&self, key: ObjectKey, kind: EndpointKind) {
        let domain_id = kind.domain_id();
        let Some(rt) = self.runtimes.get(&domain_id) else {
            eprintln!(
                "[zerodds-xrce-agentd] delete {:04x} (client {:02x?}): domain {domain_id} has no \
                 live runtime, DCPS endpoint {:?} leaks until process exit",
                key.1.raw(),
                key.0,
                kind.entity_id()
            );
            return;
        };
        match kind {
            EndpointKind::Writer(eid, _, _) => rt.remove_user_writer(eid),
            EndpointKind::Reader(eid, _, _) => rt.remove_user_reader(eid),
        }
    }

    /// Builds the [`SerializedTypeSupport`] of a topic whose type is
    /// known — spec item 5's "type is known" path. Keyed-ness and
    /// extensibility are read off the `CompleteTypeObject`
    /// ([`SerializedTypeSupport::from_complete_type_objects`]).
    ///
    /// # Errors
    /// [`DdsPortError::InvalidType`] if the type does not build — a root
    /// that is not a topic type, a member type that is not among the
    /// dependencies; the CREATE(topic) is refused rather than forwarded
    /// under a type the agent could not read.
    fn type_support_for(
        spec: &TopicSpec,
    ) -> Result<Option<Arc<SerializedTypeSupport>>, DdsPortError> {
        let Some(known) = &spec.known_type else {
            return Ok(None);
        };
        SerializedTypeSupport::from_complete_type_objects(
            &spec.type_name,
            known.root.clone(),
            known.dependencies.clone(),
        )
        .map(Some)
        .map_err(|e| {
            DdsPortError::InvalidType(format!(
                "type `{}` of topic `{}` does not build: {e}",
                spec.type_name, spec.topic_name
            ))
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_xrce::object_id::ObjectId;
    use zerodds_xrce::object_kind::ObjectKind;

    fn ck(b: u8) -> [u8; zerodds_xrce::header::CLIENT_KEY_LEN] {
        [b; zerodds_xrce::header::CLIENT_KEY_LEN]
    }

    fn oid(raw: u16, kind: ObjectKind) -> ObjectId {
        ObjectId::new(raw, kind).unwrap()
    }

    #[test]
    fn create_writer_before_topic_rejected() {
        let mut p = RealDdsPort::new(RuntimeConfig::default());
        let writer = (ck(1), oid(1, ObjectKind::DataWriter));
        let topic = (ck(1), oid(2, ObjectKind::Topic));
        let publisher = (ck(1), oid(3, ObjectKind::Publisher));
        let res = p.create_writer(writer, topic, publisher, EndpointQos::writer_default());
        assert!(matches!(res, Err(DdsPortError::UnknownReference(_))));
    }

    /// Full chain against a real, leased DDS domain: participant → topic
    /// (opaque type) → writer, then a write actually reaches the wire
    /// (verified indirectly: no error, and the runtime accepted the
    /// registration — the cross-runtime delivery itself is covered by
    /// `tests/e2e_udp_loopback.rs`).
    #[test]
    fn full_chain_against_a_real_leased_domain() {
        let lease = zerodds_test_domains::lease_domain();
        let domain = lease.id();
        let mut p = RealDdsPort::new(RuntimeConfig::default());
        let client = ck(1);
        let participant = (client, oid(1, ObjectKind::Participant));
        let topic = (client, oid(2, ObjectKind::Topic));
        let writer = (client, oid(3, ObjectKind::DataWriter));
        p.create_participant(participant, domain)
            .expect("participant");
        p.create_topic(
            topic,
            participant,
            TopicSpec {
                topic_name: "DcpsBridgeTest".into(),
                type_name: "Opaque".into(),
                known_type: None,
            },
        )
        .expect("topic");
        let publisher = (client, oid(4, ObjectKind::Publisher));
        p.create_publisher(publisher, participant, ContainerSpec::default())
            .expect("publisher");
        p.create_writer(
            writer,
            topic,
            publisher,
            EndpointQos {
                reliable: false,
                ..EndpointQos::writer_default()
            },
        )
        .expect("writer");
        p.enable(writer).expect("enable writer");
        let outcome = p
            .write(writer, &[1, 2, 3, 4], false, false, false, None)
            .expect("write");
        assert_eq!(outcome, WriteOutcome::Written);
        p.delete(writer);
        assert!(matches!(
            p.write(writer, &[], false, false, false, None),
            Err(DdsPortError::UnknownReference(_))
        ));
    }

    /// DDS-XRCE 1.0 §7.8.3.1: the DDS entities are created disabled, so a
    /// CREATE that fails leaves no discovery traffic. An endpoint made by
    /// `create_writer`/`create_reader` is not announced and cannot be used;
    /// `enable` announces it; deleting it takes it away again, and an
    /// endpoint deleted before `enable` never was announced.
    #[test]
    fn an_endpoint_is_announced_only_when_it_is_enabled() {
        let lease = zerodds_test_domains::lease_domain();
        let mut p = RealDdsPort::new(RuntimeConfig::default());
        let client = ck(7);
        let participant = (client, oid(1, ObjectKind::Participant));
        let topic = (client, oid(2, ObjectKind::Topic));
        let publisher = (client, oid(3, ObjectKind::Publisher));
        let subscriber = (client, oid(4, ObjectKind::Subscriber));
        let writer = (client, oid(5, ObjectKind::DataWriter));
        let reader = (client, oid(6, ObjectKind::DataReader));
        let discarded = (client, oid(7, ObjectKind::DataWriter));
        p.create_participant(participant, lease.id()).unwrap();
        p.create_topic(
            topic,
            participant,
            TopicSpec {
                topic_name: "DcpsBridgeEnable".into(),
                type_name: "Opaque".into(),
                known_type: None,
            },
        )
        .unwrap();
        p.create_publisher(publisher, participant, ContainerSpec::default())
            .unwrap();
        p.create_subscriber(subscriber, participant, ContainerSpec::default())
            .unwrap();
        p.create_writer(writer, topic, publisher, EndpointQos::writer_default())
            .unwrap();
        p.create_reader(reader, topic, subscriber, EndpointQos::reader_default())
            .unwrap();
        p.create_writer(discarded, topic, publisher, EndpointQos::writer_default())
            .unwrap();
        let rt = Arc::clone(p.runtimes.get(&lease.id()).unwrap());
        let writer_id = p.endpoints.get(&writer).unwrap().entity_id();
        let reader_id = p.endpoints.get(&reader).unwrap().entity_id();
        let discarded_id = p.endpoints.get(&discarded).unwrap().entity_id();

        assert!(rt.latest_announced_publication(writer_id).is_none());
        assert!(rt.latest_announced_subscription(reader_id).is_none());
        assert!(
            p.write(writer, &[1, 2, 3, 4], false, false, false, None)
                .is_err()
        );

        p.enable(writer).unwrap();
        p.enable(reader).unwrap();
        assert!(rt.latest_announced_publication(writer_id).is_some());
        assert!(rt.latest_announced_subscription(reader_id).is_some());
        assert!(rt.latest_announced_publication(discarded_id).is_none());
        p.write(writer, &[1, 2, 3, 4], false, false, false, None)
            .unwrap();

        // Deleted before it was enabled: never announced, nothing to dispose.
        p.delete(discarded);
        assert!(rt.latest_announced_publication(discarded_id).is_none());
        assert!(!p.endpoints.contains_key(&discarded));
        assert!(!p.pending.contains_key(&discarded));
    }

    /// A `TypeObject` offered at CREATE(topic) makes `write()` compute a
    /// real `KeyHash` and honor dispose — spec item 5's "type is known"
    /// path, exercised against a real leased domain end to end (not
    /// `NullDdsPort`, which can never have a `KeyHash` to work with).
    #[test]
    fn known_keyed_type_gets_a_real_keyhash_and_honors_dispose() {
        let ast = Arc::new(
            zerodds_wire_probe::encode::model_from_idl(
                "@final struct Point { @key long id; long value; };",
                "dcps-bridge-known-type.idl",
            )
            .unwrap(),
        );
        let type_ = zerodds_types::dynamic::DynamicType::find(&ast, "Point").unwrap();
        let root = zerodds_types::from_ast::complete_type_object(&ast, type_.id()).unwrap();

        let lease = zerodds_test_domains::lease_domain();
        let domain = lease.id();
        let mut p = RealDdsPort::new(RuntimeConfig::default());
        let client = ck(3);
        let participant = (client, oid(1, ObjectKind::Participant));
        let topic = (client, oid(2, ObjectKind::Topic));
        let writer = (client, oid(3, ObjectKind::DataWriter));
        p.create_participant(participant, domain)
            .expect("participant");
        p.create_topic(
            topic,
            participant,
            TopicSpec {
                topic_name: "DcpsBridgeKnownType".into(),
                type_name: "Point".into(),
                known_type: Some(crate::daemon::dds_port::KnownType {
                    root,
                    dependencies: Vec::new(),
                }),
            },
        )
        .expect("topic");
        assert!(
            p.topics.get(&topic).unwrap().keyed,
            "keyed-ness is the type's own"
        );
        assert!(
            p.topics.get(&topic).unwrap().type_support.is_some(),
            "a valid keyed Final CompleteTypeObject must resolve to a SerializedTypeSupport"
        );
        let publisher = (client, oid(4, ObjectKind::Publisher));
        p.create_publisher(publisher, participant, ContainerSpec::default())
            .expect("publisher");
        p.create_writer(
            writer,
            topic,
            publisher,
            EndpointQos {
                reliable: false,
                ..EndpointQos::writer_default()
            },
        )
        .expect("writer");
        p.enable(writer).expect("enable writer");

        let mut body = 7i32.to_le_bytes().to_vec(); // id (key)
        body.extend_from_slice(&99i32.to_le_bytes()); // value
        let outcome = p
            .write(writer, &body, false, false, false, None)
            .expect("keyed write");
        assert_eq!(outcome, WriteOutcome::Written);

        let dispose_outcome = p
            .write(writer, &body, false, true, false, None)
            .expect("dispose");
        assert_eq!(dispose_outcome, WriteOutcome::Disposed);

        // §7.8.4.1: unregister is the same call with the other marker, and a
        // source time stamps a sample.
        let unregister_outcome = p
            .write(writer, &body, false, false, true, None)
            .expect("unregister");
        assert_eq!(unregister_outcome, WriteOutcome::Disposed);
        let both = p
            .write(writer, &body, false, true, true, None)
            .expect("dispose and unregister");
        assert_eq!(both, WriteOutcome::Disposed);
        let stamped = p
            .write(
                writer,
                &body,
                false,
                false,
                false,
                Some(SourceTime {
                    seconds: 1_700_000_000,
                    nanoseconds: 500_000_000,
                }),
            )
            .expect("stamped write");
        assert_eq!(stamped, WriteOutcome::Written);

        // Annex A `sequence<SampleData>`: the type tells where a sample ends.
        let mut two = body.clone();
        two.extend_from_slice(&body);
        assert_eq!(p.sample_length(writer, &two, false).unwrap(), body.len());
        assert!(matches!(
            p.sample_length(writer, &two[..6], false),
            Err(DdsPortError::InvalidType(_))
        ));
    }

    /// §7.8.4.1: a dispose or unregister names its instance by the key. A
    /// topic of unknown type has none, so nothing is written (no plain update
    /// in its place), and its samples cannot be told apart either.
    #[test]
    fn dispose_on_a_topic_of_unknown_type_is_refused() {
        let lease = zerodds_test_domains::lease_domain();
        let mut p = RealDdsPort::new(RuntimeConfig::default());
        let client = ck(9);
        let participant = (client, oid(1, ObjectKind::Participant));
        let topic = (client, oid(2, ObjectKind::Topic));
        let writer = (client, oid(3, ObjectKind::DataWriter));
        let publisher = (client, oid(4, ObjectKind::Publisher));
        p.create_participant(participant, lease.id())
            .expect("participant");
        p.create_topic(
            topic,
            participant,
            TopicSpec {
                topic_name: "DcpsBridgeOpaqueDispose".into(),
                type_name: "Opaque".into(),
                known_type: None,
            },
        )
        .expect("topic");
        p.create_publisher(publisher, participant, ContainerSpec::default())
            .expect("publisher");
        p.create_writer(writer, topic, publisher, EndpointQos::writer_default())
            .expect("writer");
        p.enable(writer).expect("enable writer");
        for (dispose, unregister) in [(true, false), (false, true)] {
            assert!(matches!(
                p.write(writer, &[1, 2, 3, 4], false, dispose, unregister, None),
                Err(DdsPortError::InvalidType(_))
            ));
        }
        assert!(matches!(
            p.sample_length(writer, &[1, 2, 3, 4], false),
            Err(DdsPortError::InvalidType(_))
        ));
        // A plain write still goes through.
        assert_eq!(
            p.write(writer, &[1, 2, 3, 4], false, false, false, None)
                .expect("write"),
            WriteOutcome::Written
        );
    }

    /// RTPS 2.5 §9.6.3.9 StatusInfo: dispose is bit 0, unregister bit 1, and
    /// a call with both sets both.
    #[test]
    fn the_lifecycle_marker_carries_the_bit_of_each_request() {
        use zerodds_rtps::inline_qos::status_info::{DISPOSED, UNREGISTERED};
        assert_eq!(lifecycle_status_bits(true, false), DISPOSED);
        assert_eq!(lifecycle_status_bits(false, true), UNREGISTERED);
        assert_eq!(lifecycle_status_bits(true, true), DISPOSED | UNREGISTERED);
        assert_ne!(DISPOSED, UNREGISTERED);
    }

    /// DDS-XRCE 1.0 §7.5, Table 6: a Publisher and a Subscriber are DDS
    /// entities of the participant's domain, each with a group entity of its
    /// own, and a Publisher whose ENTITY_FACTORY does not enable what it
    /// creates refuses its DataWriters.
    #[test]
    fn a_publisher_is_a_dds_entity_of_its_participants_domain() {
        let lease = zerodds_test_domains::lease_domain();
        let mut p = RealDdsPort::new(RuntimeConfig::default());
        let client = ck(8);
        let participant = (client, oid(1, ObjectKind::Participant));
        let (pub_a, pub_b) = (
            (client, oid(2, ObjectKind::Publisher)),
            (client, oid(3, ObjectKind::Publisher)),
        );
        let subscriber = (client, oid(4, ObjectKind::Subscriber));
        assert!(matches!(
            p.create_publisher(pub_a, participant, ContainerSpec::default()),
            Err(DdsPortError::UnknownReference(_))
        ));
        p.create_participant(participant, lease.id())
            .expect("participant");
        p.create_publisher(pub_a, participant, ContainerSpec::default())
            .expect("publisher");
        p.create_publisher(
            pub_b,
            participant,
            ContainerSpec {
                autoenable_created_entities: false,
                ..ContainerSpec::default()
            },
        )
        .expect("publisher without autoenable");
        p.create_subscriber(subscriber, participant, ContainerSpec::default())
            .expect("subscriber");
        assert_ne!(
            p.containers.get(&pub_a).unwrap().group,
            p.containers.get(&subscriber).unwrap().group
        );
        assert_eq!(p.containers.get(&pub_a).unwrap().domain_id, lease.id());
        let topic = (client, oid(5, ObjectKind::Topic));
        p.create_topic(
            topic,
            participant,
            TopicSpec {
                topic_name: "DcpsBridgePublisher".into(),
                type_name: "Opaque".into(),
                known_type: None,
            },
        )
        .expect("topic");
        assert!(matches!(
            p.create_writer(
                (client, oid(6, ObjectKind::DataWriter)),
                topic,
                pub_b,
                EndpointQos::writer_default()
            ),
            Err(DdsPortError::DdsError(_))
        ));
        p.create_writer(
            (client, oid(6, ObjectKind::DataWriter)),
            topic,
            pub_a,
            EndpointQos::writer_default(),
        )
        .expect("writer");
        p.enable((client, oid(6, ObjectKind::DataWriter)))
            .expect("enable writer");
        p.delete(pub_a);
        assert!(!p.containers.contains_key(&pub_a));
    }

    /// DDS-XRCE 1.0 §7.7.3.6.3 `qos_profile_reference`: ENTITY_FACTORY of the
    /// participant is held by the participant (its Publishers refuse their
    /// DataWriters when it does not enable what it creates); USER_DATA is part
    /// of the discovery data of a runtime, so a participant with USER_DATA has
    /// a runtime of its own, shared only by participants with the same value.
    #[test]
    fn a_participant_qos_is_applied_to_the_participant() {
        let lease = zerodds_test_domains::lease_domain();
        let mut p = RealDdsPort::new(RuntimeConfig::default());
        let client = ck(9);
        let (plain, factory_off, with_data, with_data_too, other_data) = (
            (client, oid(1, ObjectKind::Participant)),
            (client, oid(2, ObjectKind::Participant)),
            (client, oid(3, ObjectKind::Participant)),
            (client, oid(4, ObjectKind::Participant)),
            (client, oid(5, ObjectKind::Participant)),
        );
        p.create_participant(plain, lease.id()).expect("plain");
        p.create_participant_with(
            factory_off,
            lease.id(),
            &ParticipantQos {
                autoenable_created_entities: false,
                ..ParticipantQos::default()
            },
        )
        .expect("factory off");
        let user_data = |v: &[u8]| ParticipantQos {
            user_data: v.to_vec(),
            ..ParticipantQos::default()
        };
        p.create_participant_with(with_data, lease.id(), &user_data(&[1, 2]))
            .expect("with user data");
        p.create_participant_with(with_data_too, lease.id(), &user_data(&[1, 2]))
            .expect("same user data");
        p.create_participant_with(other_data, lease.id(), &user_data(&[9]))
            .expect("other user data");
        let slots: Vec<i32> = [plain, factory_off, with_data, with_data_too, other_data]
            .iter()
            .map(|k| p.participants.get(k).unwrap().domain_id)
            .collect();
        let slot = |k: ObjectKey| {
            slots[[plain, factory_off, with_data, with_data_too, other_data]
                .iter()
                .position(|x| *x == k)
                .unwrap()]
        };
        assert_eq!(slot(plain), lease.id(), "the shared runtime of the domain");
        assert_eq!(slot(factory_off), lease.id());
        assert!(slot(with_data) < 0, "a runtime of its own");
        assert_eq!(slot(with_data), slot(with_data_too));
        assert_ne!(slot(with_data), slot(other_data));
        assert_eq!(p.own_runtimes.len(), 2);
        // Created disabled: no runtime has started, nothing is announced.
        assert!(p.runtimes.is_empty(), "no runtime before enable");
        p.enable(plain).expect("enable");
        p.enable(with_data).expect("enable with user data");
        assert!(p.runtimes.contains_key(&lease.id()));
        assert!(p.runtimes.contains_key(&slot(with_data)));
        assert!(!p.runtimes.contains_key(&slot(other_data)));
        // A domain id the ports of RTPS do not have is refused at CREATE.
        assert!(
            p.create_participant((client, oid(9, ObjectKind::Participant)), 233)
                .is_err()
        );
        // The Publisher of the participant with ENTITY_FACTORY off is
        // disabled; its DataWriter is refused.
        let publisher = (client, oid(6, ObjectKind::Publisher));
        p.create_publisher(publisher, factory_off, ContainerSpec::default())
            .expect("publisher");
        let topic = (client, oid(7, ObjectKind::Topic));
        p.create_topic(
            topic,
            factory_off,
            TopicSpec {
                topic_name: "DcpsBridgeParticipantQos".into(),
                type_name: "Opaque".into(),
                known_type: None,
            },
        )
        .expect("topic");
        assert!(matches!(
            p.create_writer(
                (client, oid(8, ObjectKind::DataWriter)),
                topic,
                publisher,
                EndpointQos::writer_default()
            ),
            Err(DdsPortError::DdsError(_))
        ));
    }

    /// A content filter decodes a sample in the representation it arrived
    /// in. `UserSample` tags XCDR2 as 1; passed to `read_body` unchanged
    /// (it takes 2) every XCDR2 sample failed to decode and the filter
    /// dropped it.
    #[test]
    fn a_content_filter_admits_an_xcdr2_sample() {
        let ast = Arc::new(
            zerodds_wire_probe::encode::model_from_idl(
                "@final struct Point { @key long id; long value; };",
                "dcps-bridge-filter-xcdr2.idl",
            )
            .unwrap(),
        );
        let ty = zerodds_types::dynamic::DynamicType::find(&ast, "Point").unwrap();
        let root = zerodds_types::from_ast::complete_type_object(&ast, ty.id()).unwrap();
        let ts =
            SerializedTypeSupport::from_complete_type_objects("Point", root, Vec::new()).unwrap();
        let filter = CompiledReadFilter::compile("id = 7", Some(ts), "test filter").unwrap();
        let sample = |id: i32, representation: u8| {
            let mut payload = id.to_le_bytes().to_vec();
            payload.extend_from_slice(&99i32.to_le_bytes());
            BufferedSample {
                payload,
                representation,
                big_endian: false,
                instance: None,
                state: 0,
                source_time: None,
            }
        };
        assert!(filter.admits(&sample(7, 1)), "XCDR2");
        assert!(filter.admits(&sample(7, 0)), "XCDR1");
        assert!(!filter.admits(&sample(8, 1)));
    }

    /// The BigHelloWorld measurement: two 50 kB samples one second apart,
    /// written by an XRCE client's DataWriter (DDS defaults: RELIABLE,
    /// KEEP_LAST 1) and read by another client's DataReader (BEST_EFFORT,
    /// KEEP_LAST 1) on the same agent. Both must reach the reader queue.
    #[test]
    fn two_large_samples_reach_a_default_reader_on_the_same_agent() {
        let lease = zerodds_test_domains::lease_domain();
        let mut p = RealDdsPort::new(RuntimeConfig::default());
        let (a, b) = (ck(5), ck(6));
        for c in [a, b] {
            p.create_participant((c, oid(1, ObjectKind::Participant)), lease.id())
                .expect("participant");
            p.create_topic(
                (c, oid(2, ObjectKind::Topic)),
                (c, oid(1, ObjectKind::Participant)),
                TopicSpec {
                    topic_name: "BigHelloWorldTopic".into(),
                    type_name: "BigHelloWorld".into(),
                    known_type: None,
                },
            )
            .expect("topic");
        }
        let writer = (a, oid(3, ObjectKind::DataWriter));
        let reader = (b, oid(3, ObjectKind::DataReader));
        let publisher = (a, oid(4, ObjectKind::Publisher));
        let subscriber = (b, oid(4, ObjectKind::Subscriber));
        p.create_publisher(
            publisher,
            (a, oid(1, ObjectKind::Participant)),
            ContainerSpec::default(),
        )
        .expect("publisher");
        p.create_subscriber(
            subscriber,
            (b, oid(1, ObjectKind::Participant)),
            ContainerSpec::default(),
        )
        .expect("subscriber");
        p.create_reader(
            reader,
            (b, oid(2, ObjectKind::Topic)),
            subscriber,
            EndpointQos::reader_default(),
        )
        .expect("reader");
        p.enable(reader).expect("enable reader");
        p.create_writer(
            writer,
            (a, oid(2, ObjectKind::Topic)),
            publisher,
            EndpointQos::writer_default(),
        )
        .expect("writer");
        p.enable(writer).expect("enable writer");
        let mut got = Vec::new();
        for index in 0u32..2 {
            let mut body = index.to_le_bytes().to_vec();
            body.extend_from_slice(&50_000u32.to_le_bytes());
            body.extend(core::iter::repeat_n(b'A', 49_999));
            body.push(0);
            p.write(writer, &body, false, false, false, None)
                .expect("write");
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while std::time::Instant::now() < deadline {
                let polled = p.poll_samples(reader, 16);
                if !polled.is_empty() {
                    got.extend(polled);
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            std::thread::sleep(std::time::Duration::from_millis(900));
        }
        let indices: Vec<u32> = got
            .iter()
            .map(|s| u32::from_le_bytes([s.bytes[0], s.bytes[1], s.bytes[2], s.bytes[3]]))
            .collect();
        assert_eq!(indices, vec![0, 1]);
    }
}
