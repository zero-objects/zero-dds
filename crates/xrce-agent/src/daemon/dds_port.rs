// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The DDS side of the agent, abstracted behind [`DdsPort`].
//!
//! [`crate::daemon::session::SessionEngine`] never touches `zerodds-dcps`
//! directly — every effect that reaches the DDS data space (participant
//! creation, topic/endpoint creation, writes, reads) goes through this
//! trait. That keeps the protocol engine testable without a live
//! `DcpsRuntime`: [`NullDdsPort`] backs the unit tests and the
//! `tests/replay_foreign_session.rs` conformance test, and
//! [`crate::daemon::dcps_bridge::RealDdsPort`] (in the `dcps_bridge`
//! module, built only with the real `zerodds-dcps` dependency present) is
//! what the daemon binary runs.

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_xrce::header::CLIENT_KEY_LEN;
use zerodds_xrce::object_id::ObjectId;

/// `(ClientKey bytes, ObjectId)` — the session-scoped identity of an
/// object, as the session engine names it. `DdsPort` implementations map
/// this to whatever process-global entity id their DDS stack uses.
pub type ObjectKey = ([u8; CLIENT_KEY_LEN], ObjectId);

/// A topic type the agent knows: its `CompleteTypeObject` and the ones
/// its members refer to (XTypes 1.3 §7.3.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownType {
    /// The type itself.
    pub root: zerodds_types::CompleteTypeObject,
    /// Every other type the agent knows in the same scope — the types
    /// `root`'s members can refer to by hash.
    pub dependencies: Vec<zerodds_types::CompleteTypeObject>,
}

/// Everything a `CREATE` needs to tell the DDS side about a topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicSpec {
    /// DDS topic name.
    pub topic_name: String,
    /// DDS type name.
    pub type_name: String,
    /// The type, if the client (an `OBJK_TYPE` object) or the daemon's
    /// `type_files:` defines one under `type_name`. With it the DDS side
    /// computes key hashes, honors dispose/unregister and announces the
    /// type; keyed-ness and extensibility are the type's own. `None`
    /// means samples on the topic are forwarded opaque, unkeyed.
    pub known_type: Option<KnownType>,
}

/// What a Publisher or Subscriber is created with (DDS-XRCE 1.0 §7.5,
/// Table 6): the policies of the entity itself. PARTITION and GROUP_DATA
/// also reach its endpoints through [`EndpointQos`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerSpec {
    /// PARTITION names (§2.2.3.13).
    pub partitions: Vec<String>,
    /// GROUP_DATA (§2.2.3.2).
    pub group_data: Vec<u8>,
    /// PRESENTATION (§2.2.3.6); its endpoints offer or request it.
    pub presentation: zerodds_qos::PresentationQosPolicy,
    /// ENTITY_FACTORY `autoenable_created_entities` (§2.2.3.20). When
    /// `false`, the endpoints created under the entity stay disabled and
    /// XRCE has no operation that enables them, so their creation is
    /// refused.
    pub autoenable_created_entities: bool,
}

impl Default for ContainerSpec {
    fn default() -> Self {
        Self {
            partitions: Vec::new(),
            group_data: Vec::new(),
            presentation: zerodds_qos::PresentationQosPolicy::default(),
            autoenable_created_entities: true,
        }
    }
}

/// DURABILITY kind, DDS 1.4 §2.2.3.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Durability {
    /// `VOLATILE` — the default.
    #[default]
    Volatile,
    /// `TRANSIENT_LOCAL`.
    TransientLocal,
    /// `TRANSIENT`.
    Transient,
    /// `PERSISTENT`.
    Persistent,
}

/// HISTORY, DDS 1.4 §2.2.3.18.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum History {
    /// `KEEP_LAST` with its depth (≥ 1) — the default is depth 1.
    KeepLast(u32),
    /// `KEEP_ALL`.
    KeepAll,
}

impl Default for History {
    fn default() -> Self {
        Self::KeepLast(1)
    }
}

/// QoS the DDS side creates an endpoint with — every policy the CREATE
/// representations (`crate::daemon::entity_repr`: XML attribute and
/// element form, eProsima binary form) can carry, starting from the DDS
/// 1.4 §2.2.3 defaults of the endpoint's role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointQos {
    /// `true` = RELIABLE, `false` = BEST_EFFORT (§2.2.3.14).
    pub reliable: bool,
    /// DURABILITY (§2.2.3.4).
    pub durability: Durability,
    /// HISTORY (§2.2.3.18).
    pub history: History,
    /// OWNERSHIP EXCLUSIVE (`true`) or SHARED (§2.2.3.9).
    pub exclusive_ownership: bool,
    /// OWNERSHIP_STRENGTH (§2.2.3.10); writers only.
    pub ownership_strength: i32,
    /// DEADLINE period (§2.2.3.7); `None` = infinite.
    pub deadline: Option<core::time::Duration>,
    /// LIFESPAN (§2.2.3.16); writers only, `None` = infinite.
    pub lifespan: Option<core::time::Duration>,
    /// TIME_BASED_FILTER minimum separation (§2.2.3.12); readers only,
    /// `None` = 0.
    pub time_based_filter: Option<core::time::Duration>,
    /// A content filter expression (§2.2.2.3.3 grammar) the reader was
    /// created with; readers only.
    pub content_filter: Option<String>,
    /// USER_DATA (§2.2.3.1).
    pub user_data: Vec<u8>,
    /// PARTITION of the endpoint's Publisher/Subscriber (§2.2.3.13).
    pub partitions: Vec<String>,
    /// GROUP_DATA of the endpoint's Publisher/Subscriber (§2.2.3.2).
    pub group_data: Vec<u8>,
    /// RESOURCE_LIMITS (§2.2.3.19); unlimited by default.
    pub resource_limits: zerodds_qos::ResourceLimitsQosPolicy,
    /// RELIABILITY `max_blocking_time` (§2.2.3.14): how long a writer's
    /// write waits for room under its RESOURCE_LIMITS; `None` = infinite.
    pub max_blocking_time: Option<core::time::Duration>,
    /// LIVELINESS (§2.2.3.11): offered by a writer, requested by a reader.
    pub liveliness: zerodds_qos::LivelinessQosPolicy,
    /// LATENCY_BUDGET (§2.2.3.10).
    pub latency_budget: zerodds_qos::LatencyBudgetQosPolicy,
    /// DESTINATION_ORDER (§2.2.3.18).
    pub destination_order: zerodds_qos::DestinationOrderQosPolicy,
    /// TRANSPORT_PRIORITY (§2.2.3.15); writers only.
    pub transport_priority: i32,
    /// DURABILITY_SERVICE (§2.2.3.5); writers only.
    pub durability_service: zerodds_qos::DurabilityServiceQosPolicy,
    /// TOPIC_DATA (§2.2.3.2) of the endpoint's Topic.
    pub topic_data: Vec<u8>,
}

impl EndpointQos {
    /// The DataWriter defaults: RELIABLE, VOLATILE, KEEP_LAST 1, SHARED.
    #[must_use]
    pub fn writer_default() -> Self {
        Self {
            reliable: true,
            ..Self::reader_default()
        }
    }

    /// The DataReader defaults: BEST_EFFORT, VOLATILE, KEEP_LAST 1, SHARED.
    #[must_use]
    pub fn reader_default() -> Self {
        Self {
            reliable: false,
            durability: Durability::Volatile,
            history: History::KeepLast(1),
            exclusive_ownership: false,
            ownership_strength: 0,
            deadline: None,
            lifespan: None,
            time_based_filter: None,
            content_filter: None,
            user_data: Vec::new(),
            partitions: Vec::new(),
            group_data: Vec::new(),
            resource_limits: zerodds_qos::ResourceLimitsQosPolicy::default(),
            // The DDS 1.4 default of both roles (§2.2.3.14).
            max_blocking_time: Some(core::time::Duration::from_millis(100)),
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            transport_priority: 0,
            durability_service: zerodds_qos::DurabilityServiceQosPolicy::default(),
            topic_data: Vec::new(),
        }
    }
}

/// One error a `DdsPort` operation reports. Always surfaced to the client
/// as a `STATUS` submessage with a spec result code, or logged as an
/// observability event when there is nothing more specific to say —
/// never dropped silently (see `session.rs`'s module doc for how each
/// variant maps to a `ResultStatusCode`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DdsPortError {
    /// The requested domain, participant, topic or endpoint does not
    /// exist on the DDS side (e.g. a writer CREATE before its topic).
    UnknownReference(String),
    /// The DDS runtime itself refused the operation (bind failure,
    /// resource exhaustion, a rejected `TypeObject`).
    DdsError(String),
    /// A `TypeObject` decode/validate failure — the type the client
    /// offered does not parse or is inconsistent with its own `keyed`/
    /// extensibility flags.
    InvalidType(String),
    /// A `WRITE_DATA` body could not be split into individual samples —
    /// FORMAT_DATA_SEQ/FORMAT_SAMPLE_SEQ/FORMAT_PACKED_SAMPLES carry no
    /// per-element length (confirmed against Annex B.5.2/B.5.4/B.5.5 of
    /// the spec text), so recovering element boundaries needs the topic's
    /// own type decoder to tell the caller how many bytes each element
    /// consumed — a capability `zerodds_dcps::serialized_type::
    /// SerializedTypeSupport` does not expose today (`read_body` decodes
    /// exactly one sample from a whole slice, never a prefix of a longer
    /// one). Tracked as a real, named limitation rather than a guessed
    /// split that could silently misdecode.
    UnsplittableSequence(String),
}

/// A point in time as DDS stamps a sample with: seconds since the Unix
/// epoch and nanoseconds. The source timestamp of a sample a client wrote
/// with sample information (DDS-XRCE 1.0 §7.8.4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceTime {
    /// Seconds since 1970-01-01 UTC.
    pub seconds: i32,
    /// Nanoseconds within the second.
    pub nanoseconds: u32,
}

/// What [`DdsPort::write`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteOutcome {
    /// A sample was written.
    Written,
    /// The dispose and/or unregister was carried out: the DDS side computed a
    /// real `KeyHash` for the instance and sent the lifecycle marker instead
    /// of a sample.
    Disposed,
}

/// A sample taken from a DataReader with its `SampleInfo` (DDS-XRCE 1.0
/// §7.7.1, Annex A `SampleInfoFlags`): what `FORMAT_SAMPLE` and the formats
/// built on it send, and `FORMAT_DATA` and `FORMAT_DATA_SEQ` leave out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolledSample {
    /// The serialized sample; empty for a dispose or unregister marker that
    /// carries no key.
    pub bytes: Vec<u8>,
    /// `SampleInfoFlags`: `INSTANCE_STATE_UNREGISTERED` (bit 0),
    /// `INSTANCE_STATE_DISPOSED` (bit 1), `VIEW_STATE_NEW` (bit 2, the first
    /// sample of the instance the reader sees), `SAMPLE_STATE_READ` (bit 3,
    /// never set: the agent hands a sample over once).
    pub state: u8,
    /// The source timestamp of the writer (seconds since the Unix epoch,
    /// nanoseconds), if it sent one.
    pub source_time: Option<(i32, u32)>,
}

impl PolledSample {
    /// An alive sample with no source timestamp and the state of a sample
    /// that is read for the first time.
    #[must_use]
    pub fn alive(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            state: 0,
            source_time: None,
        }
    }

    /// Whether the sample is a dispose or unregister marker: a state change
    /// of the instance, which `FORMAT_DATA` cannot carry.
    #[must_use]
    pub fn is_lifecycle(&self) -> bool {
        self.state & 0b11 != 0
    }
}

/// The policies of a DDS `DomainParticipant` (DDS 1.4 §2.2.2.2.1.1
/// `DomainParticipantQos`) an XRCE DomainParticipant carries
/// (`qos_profile_reference`, DDS-XRCE 1.0 §7.7.3.6.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantQos {
    /// USER_DATA (§2.2.3.1), announced in the participant's discovery data.
    /// A non-empty value needs a participant (a DCPS runtime) of its own: the
    /// participants of a domain share one runtime, whose discovery data has
    /// one USER_DATA.
    pub user_data: Vec<u8>,
    /// ENTITY_FACTORY `autoenable_created_entities` (§2.2.3.20). When
    /// `false`, the Publishers, Subscribers and Topics the participant
    /// creates stay disabled, and XRCE has no operation that enables them, so
    /// the endpoints created under them are refused.
    pub autoenable_created_entities: bool,
}

impl Default for ParticipantQos {
    fn default() -> Self {
        Self {
            user_data: Vec::new(),
            autoenable_created_entities: true,
        }
    }
}

/// The DDS-side effects the session engine drives.
pub trait DdsPort {
    /// Binds (or reuses) a DDS `DomainParticipant` in `domain_id` for this
    /// client's `OBJK_PARTICIPANT` object. Idempotent per `ObjectKey`.
    ///
    /// # Errors
    /// [`DdsPortError`].
    fn create_participant(&mut self, key: ObjectKey, domain_id: i32) -> Result<(), DdsPortError>;

    /// As [`Self::create_participant`], with the policies of the participant.
    /// A port that cannot apply a policy refuses; the default implementation
    /// takes the default policies only.
    ///
    /// # Errors
    /// [`DdsPortError`].
    fn create_participant_with(
        &mut self,
        key: ObjectKey,
        domain_id: i32,
        qos: &ParticipantQos,
    ) -> Result<(), DdsPortError> {
        if *qos == ParticipantQos::default() {
            self.create_participant(key, domain_id)
        } else {
            Err(DdsPortError::DdsError(
                "this DDS port cannot apply a participant QoS".into(),
            ))
        }
    }

    /// Declares a topic under `participant` (already created via
    /// [`Self::create_participant`]).
    ///
    /// # Errors
    /// [`DdsPortError::UnknownReference`] if `participant` was not
    /// created first; [`DdsPortError::InvalidType`] if `spec.type_object`
    /// is present but does not decode.
    fn create_topic(
        &mut self,
        key: ObjectKey,
        participant: ObjectKey,
        spec: TopicSpec,
    ) -> Result<(), DdsPortError>;

    /// Creates the DDS Publisher of an `OBJK_PUBLISHER` object under
    /// `participant` (DDS-XRCE 1.0 §7.8.3.1, Table 6: "The DDS Publisher
    /// shall be created using the DomainParticipant identified by the
    /// participant_id").
    ///
    /// # Errors
    /// [`DdsPortError::UnknownReference`] if `participant` was not created
    /// first.
    fn create_publisher(
        &mut self,
        key: ObjectKey,
        participant: ObjectKey,
        spec: ContainerSpec,
    ) -> Result<(), DdsPortError>;

    /// Creates the DDS Subscriber of an `OBJK_SUBSCRIBER` object under
    /// `participant` (Table 6, like [`Self::create_publisher`]).
    ///
    /// # Errors
    /// [`DdsPortError::UnknownReference`] if `participant` was not created
    /// first.
    fn create_subscriber(
        &mut self,
        key: ObjectKey,
        participant: ObjectKey,
        spec: ContainerSpec,
    ) -> Result<(), DdsPortError>;

    /// Creates a DataWriter on `topic` with `publisher` (Table 6: "The DDS
    /// DataWriter shall be created using the Publisher identified by the
    /// publisher_id").
    ///
    /// # Errors
    /// [`DdsPortError::UnknownReference`] if `topic` or `publisher` was
    /// not created, or they belong to different participants;
    /// [`DdsPortError::DdsError`] if the Publisher's ENTITY_FACTORY does
    /// not enable the entities it creates.
    fn create_writer(
        &mut self,
        key: ObjectKey,
        topic: ObjectKey,
        publisher: ObjectKey,
        qos: EndpointQos,
    ) -> Result<(), DdsPortError>;

    /// Creates a DataReader on `topic` with `subscriber`. Samples arriving
    /// afterward are retrieved via [`Self::poll_samples`].
    ///
    /// # Errors
    /// As [`Self::create_writer`], for the Subscriber.
    fn create_reader(
        &mut self,
        key: ObjectKey,
        topic: ObjectKey,
        subscriber: ObjectKey,
        qos: EndpointQos,
    ) -> Result<(), DdsPortError>;

    /// Writes one sample through `writer`. `big_endian` is the XCDR2 byte
    /// order `payload` was serialized in (spec §7.7.1: `SampleData` is
    /// always XCDR2; the submessage's own E-flag gives the byte order).
    /// `dispose`/`unregister`, from a `Sample`'s `SampleInfoFlags`
    /// (FORMAT_SAMPLE and richer — always `false` for plain FORMAT_DATA,
    /// which carries no `SampleInfo`), ask for a lifecycle marker instead
    /// of an ordinary sample; the returned [`WriteOutcome`] says whether
    /// that could actually be honored (it needs a `KeyHash`, which needs
    /// the topic's type to be known and keyed). `source_time` stamps the
    /// sample or marker; `None` leaves the stamp to the DDS side.
    ///
    /// # Errors
    /// [`DdsPortError`]; [`DdsPortError::InvalidType`] for a dispose or
    /// unregister whose instance cannot be identified because the topic's
    /// type is unknown or has no key — nothing is written then, as a plain
    /// update would not be what the client asked for.
    fn write(
        &mut self,
        writer: ObjectKey,
        payload: &[u8],
        big_endian: bool,
        dispose: bool,
        unregister: bool,
        source_time: Option<SourceTime>,
    ) -> Result<WriteOutcome, DdsPortError>;

    /// The length of the first serialized sample of `bytes`, which may hold
    /// more behind it, as `writer`'s topic type reads it (DDS-XRCE 1.0
    /// Annex A: `sequence<SampleData>` has no length for its elements, so the
    /// type is what tells them apart).
    ///
    /// # Errors
    /// [`DdsPortError::UnknownReference`] if `writer` was not created;
    /// [`DdsPortError::InvalidType`] if the topic's type is unknown, or the
    /// bytes do not start with a sample of it.
    fn sample_length(
        &mut self,
        writer: ObjectKey,
        bytes: &[u8],
        big_endian: bool,
    ) -> Result<usize, DdsPortError>;

    /// Installs (`Some`) or clears (`None`) the content filter a
    /// `READ_DATA`'s `ReadSpecification.content_filter_expression` names
    /// for `reader` (spec §7.7.14): "applied to the samples in the
    /// DataReader cache. Only samples for which the filter evaluates to
    /// TRUE shall be returned to the XRCE Client." Persists across
    /// [`Self::poll_samples`] calls — including a continuous read's
    /// repeated polls — until the next `set_read_filter` call; a
    /// `READ_DATA` that omits the optional member calls this with `None`,
    /// clearing a filter a prior request on the same reader may have set
    /// (never silently keeping it).
    ///
    /// # Errors
    /// [`DdsPortError::InvalidType`] if `filter` is `Some` and either the
    /// reader's topic's type is unknown (no decoder exists to evaluate
    /// field paths against) or the expression fails to parse.
    /// [`DdsPortError::UnknownReference`] if `reader` was not created.
    fn set_read_filter(
        &mut self,
        reader: ObjectKey,
        filter: Option<&str>,
    ) -> Result<(), DdsPortError>;

    /// Drains up to `max` samples buffered for `reader` since the last
    /// call, already filtered per the last [`Self::set_read_filter`] call
    /// on it. Never blocks. Each sample carries what the DataReader knew of
    /// it ([`PolledSample`]).
    fn poll_samples(&mut self, reader: ObjectKey, max: usize) -> Vec<PolledSample>;

    /// Enables the endpoint `key` that [`Self::create_writer`] or
    /// [`Self::create_reader`] made: only now is it discovered. DDS-XRCE 1.0
    /// §7.8.3.1: "If the creation of the XRCE object fails then there should
    /// be no associated DDS-RTPS discovery traffic generated by the Agent.
    /// This means that all DDS entities shall be created disabled [...] and
    /// enabled (if so configured by their QoS) only after it has been
    /// determined that the creation has succeeded." The session calls it
    /// when the XRCE object is complete. An endpoint that is not enabled
    /// reads and writes nothing. A key that needs no enabling (every
    /// non-endpoint) is `Ok`.
    ///
    /// # Errors
    /// [`DdsPortError::DdsError`] if the DDS runtime refuses to register the
    /// endpoint; the endpoint is then gone, as after [`Self::delete`].
    fn enable(&mut self, _key: ObjectKey) -> Result<(), DdsPortError> {
        Ok(())
    }

    /// Removes whatever DDS-side state `key` owns (participant, topic or
    /// endpoint — the implementation infers which from the last
    /// `create_*` call, or does nothing if `key` was never created).
    fn delete(&mut self, key: ObjectKey);

    /// Removes every DDS-side object a client created — the DDS-side
    /// counterpart of `XrceAgent::unregister_client`, driven by the same
    /// session-timeout/DELETE(CLIENT) paths.
    fn unregister_client(&mut self, client_key: [u8; CLIENT_KEY_LEN]);
}

/// A `DdsPort` that accepts every CREATE and silently drops every write —
/// the DDS side is not exercised at all. Used by:
///
/// - unit tests of `session.rs` that only check protocol-level behavior
///   (STATUS codes, object ids, ACKNACK bitmaps) and do not care what the
///   DDS side did with a WRITE_DATA;
/// - `tests/replay_foreign_session.rs`, which replays real client
///   requests and checks the session engine's *replies* match the real
///   agent's — the real agent's DDS backend (a running eProsima example
///   subscriber) is not part of that byte oracle, only its protocol
///   replies are.
///
/// `poll_samples` always returns empty: nothing pushes samples into it,
/// there is no DDS backend delivering them.
#[derive(Debug, Default)]
pub struct NullDdsPort {
    /// Objects this port has "created" (for `delete`/idempotency
    /// bookkeeping only — no real resource is held).
    known: BTreeMap<ObjectKey, ()>,
    /// Each Publisher/Subscriber with its participant and spec.
    containers: BTreeMap<ObjectKey, (ObjectKey, ContainerSpec)>,
    /// Each topic's participant.
    topic_participants: BTreeMap<ObjectKey, ObjectKey>,
    /// The policies each participant was created with.
    participant_qos: BTreeMap<ObjectKey, ParticipantQos>,
}

impl NullDdsPort {
    /// Constructs an empty port.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The policies the participant `key` was created with.
    #[must_use]
    pub fn participant_qos(&self, key: ObjectKey) -> Option<&ParticipantQos> {
        self.participant_qos.get(&key)
    }
}

impl NullDdsPort {
    fn create_container(
        &mut self,
        what: &str,
        key: ObjectKey,
        participant: ObjectKey,
        spec: ContainerSpec,
    ) -> Result<(), DdsPortError> {
        if !self.known.contains_key(&participant) {
            return Err(DdsPortError::UnknownReference(alloc::format!(
                "{what}: participant not created"
            )));
        }
        self.known.insert(key, ());
        self.containers.insert(key, (participant, spec));
        Ok(())
    }

    fn create_endpoint(
        &mut self,
        what: &str,
        key: ObjectKey,
        topic: ObjectKey,
        container: ObjectKey,
    ) -> Result<(), DdsPortError> {
        if !self.known.contains_key(&topic) {
            return Err(DdsPortError::UnknownReference(alloc::format!(
                "{what}: topic not created"
            )));
        }
        let Some((participant, spec)) = self.containers.get(&container) else {
            return Err(DdsPortError::UnknownReference(alloc::format!(
                "{what}: Publisher/Subscriber not created"
            )));
        };
        if self.topic_participants.get(&topic) != Some(participant) {
            return Err(DdsPortError::UnknownReference(alloc::format!(
                "{what}: the topic and the Publisher/Subscriber belong to different participants"
            )));
        }
        if !spec.autoenable_created_entities {
            return Err(DdsPortError::DdsError(alloc::format!(
                "{what}: the ENTITY_FACTORY QoS of the Publisher/Subscriber does not enable the \
                 entities it creates, and XRCE has no operation that enables them"
            )));
        }
        self.known.insert(key, ());
        Ok(())
    }
}

impl DdsPort for NullDdsPort {
    fn create_participant_with(
        &mut self,
        key: ObjectKey,
        domain_id: i32,
        qos: &ParticipantQos,
    ) -> Result<(), DdsPortError> {
        // The null port keeps no DDS entities; the policies are accepted and
        // have nothing to apply to, and the group-enable rule is the one of
        // the real port.
        self.participant_qos.insert(key, qos.clone());
        self.create_participant(key, domain_id)
    }

    fn create_participant(&mut self, key: ObjectKey, _domain_id: i32) -> Result<(), DdsPortError> {
        self.known.insert(key, ());
        Ok(())
    }

    fn create_topic(
        &mut self,
        key: ObjectKey,
        participant: ObjectKey,
        _spec: TopicSpec,
    ) -> Result<(), DdsPortError> {
        if !self.known.contains_key(&participant) {
            return Err(DdsPortError::UnknownReference(
                "create_topic: participant not created".into(),
            ));
        }
        self.known.insert(key, ());
        self.topic_participants.insert(key, participant);
        Ok(())
    }

    fn create_publisher(
        &mut self,
        key: ObjectKey,
        participant: ObjectKey,
        spec: ContainerSpec,
    ) -> Result<(), DdsPortError> {
        self.create_container("create_publisher", key, participant, spec)
    }

    fn create_subscriber(
        &mut self,
        key: ObjectKey,
        participant: ObjectKey,
        spec: ContainerSpec,
    ) -> Result<(), DdsPortError> {
        self.create_container("create_subscriber", key, participant, spec)
    }

    fn create_writer(
        &mut self,
        key: ObjectKey,
        topic: ObjectKey,
        publisher: ObjectKey,
        _qos: EndpointQos,
    ) -> Result<(), DdsPortError> {
        self.create_endpoint("create_writer", key, topic, publisher)
    }

    fn create_reader(
        &mut self,
        key: ObjectKey,
        topic: ObjectKey,
        subscriber: ObjectKey,
        _qos: EndpointQos,
    ) -> Result<(), DdsPortError> {
        self.create_endpoint("create_reader", key, topic, subscriber)
    }

    fn write(
        &mut self,
        writer: ObjectKey,
        _payload: &[u8],
        _big_endian: bool,
        dispose: bool,
        unregister: bool,
        _source_time: Option<SourceTime>,
    ) -> Result<WriteOutcome, DdsPortError> {
        if !self.known.contains_key(&writer) {
            return Err(DdsPortError::UnknownReference(
                "write: writer not created".into(),
            ));
        }
        // The DDS side is not exercised at all here (see the type doc): no
        // type, so no key to name the instance of a dispose or unregister.
        if dispose || unregister {
            return Err(DdsPortError::InvalidType(
                "dispose/unregister names an instance by its key and NullDdsPort knows no type"
                    .into(),
            ));
        }
        Ok(WriteOutcome::Written)
    }

    fn sample_length(
        &mut self,
        writer: ObjectKey,
        _bytes: &[u8],
        _big_endian: bool,
    ) -> Result<usize, DdsPortError> {
        if !self.known.contains_key(&writer) {
            return Err(DdsPortError::UnknownReference(
                "sample_length: writer not created".into(),
            ));
        }
        Err(DdsPortError::InvalidType(
            "NullDdsPort knows no type to tell the samples of a sequence apart".into(),
        ))
    }

    fn set_read_filter(
        &mut self,
        reader: ObjectKey,
        filter: Option<&str>,
    ) -> Result<(), DdsPortError> {
        if !self.known.contains_key(&reader) {
            return Err(DdsPortError::UnknownReference(
                "set_read_filter: reader not created".into(),
            ));
        }
        match filter {
            // The DDS side is never exercised here (see the type doc), so
            // there is no decoded field to evaluate a filter against —
            // same honest refusal as a dispose/unregister request.
            Some(_) => Err(DdsPortError::InvalidType(
                "NullDdsPort has no decoded samples to evaluate a content filter against".into(),
            )),
            None => Ok(()),
        }
    }

    fn poll_samples(&mut self, _reader: ObjectKey, _max: usize) -> Vec<PolledSample> {
        Vec::new()
    }

    fn delete(&mut self, key: ObjectKey) {
        self.known.remove(&key);
        self.containers.remove(&key);
        self.topic_participants.remove(&key);
    }

    fn unregister_client(&mut self, client_key: [u8; CLIENT_KEY_LEN]) {
        self.known.retain(|(ck, _), ()| *ck != client_key);
        self.containers.retain(|(ck, _), _| *ck != client_key);
        self.topic_participants
            .retain(|(ck, _), _| *ck != client_key);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_xrce::object_kind::ObjectKind;

    fn ck(b: u8) -> [u8; CLIENT_KEY_LEN] {
        [b; CLIENT_KEY_LEN]
    }

    fn oid(raw: u16, kind: ObjectKind) -> ObjectId {
        ObjectId::new(raw, kind).unwrap()
    }

    #[test]
    fn null_port_records_the_participant_qos() {
        let mut p = NullDdsPort::new();
        let participant = (ck(5), oid(1, ObjectKind::Participant));
        let qos = ParticipantQos {
            user_data: vec![7],
            autoenable_created_entities: false,
        };
        p.create_participant_with(participant, 0, &qos).unwrap();
        assert_eq!(p.participant_qos(participant), Some(&qos));
    }

    #[test]
    fn null_port_rejects_topic_before_participant() {
        let mut p = NullDdsPort::new();
        let topic = (ck(1), oid(1, ObjectKind::Topic));
        let participant = (ck(1), oid(2, ObjectKind::Participant));
        let res = p.create_topic(
            topic,
            participant,
            TopicSpec {
                topic_name: "T".into(),
                type_name: "Ty".into(),
                known_type: None,
            },
        );
        assert!(matches!(res, Err(DdsPortError::UnknownReference(_))));
    }

    #[test]
    fn null_port_full_chain_and_delete() {
        let mut p = NullDdsPort::new();
        let client = ck(1);
        let participant = (client, oid(1, ObjectKind::Participant));
        let topic = (client, oid(2, ObjectKind::Topic));
        let writer = (client, oid(3, ObjectKind::DataWriter));
        p.create_participant(participant, 0).unwrap();
        p.create_topic(
            topic,
            participant,
            TopicSpec {
                topic_name: "T".into(),
                type_name: "Ty".into(),
                known_type: None,
            },
        )
        .unwrap();
        let publisher = (client, oid(9, ObjectKind::Publisher));
        p.create_publisher(publisher, participant, ContainerSpec::default())
            .unwrap();
        p.create_writer(writer, topic, publisher, EndpointQos::writer_default())
            .unwrap();
        let outcome = p
            .write(writer, &[1, 2, 3], false, false, false, None)
            .unwrap();
        assert_eq!(outcome, WriteOutcome::Written);
        p.delete(writer);
        assert!(matches!(
            p.write(writer, &[], false, false, false, None),
            Err(DdsPortError::UnknownReference(_))
        ));
    }

    #[test]
    fn null_port_refuses_a_dispose_it_cannot_identify() {
        let mut p = NullDdsPort::new();
        let client = ck(2);
        let participant = (client, oid(1, ObjectKind::Participant));
        let topic = (client, oid(2, ObjectKind::Topic));
        let writer = (client, oid(3, ObjectKind::DataWriter));
        p.create_participant(participant, 0).unwrap();
        p.create_topic(
            topic,
            participant,
            TopicSpec {
                topic_name: "T".into(),
                type_name: "Ty".into(),
                known_type: None,
            },
        )
        .unwrap();
        let publisher = (client, oid(9, ObjectKind::Publisher));
        p.create_publisher(publisher, participant, ContainerSpec::default())
            .unwrap();
        p.create_writer(writer, topic, publisher, EndpointQos::writer_default())
            .unwrap();
        for (dispose, unregister) in [(true, false), (false, true), (true, true)] {
            assert!(matches!(
                p.write(writer, &[1], false, dispose, unregister, None),
                Err(DdsPortError::InvalidType(_))
            ));
        }
        assert!(matches!(
            p.sample_length(writer, &[1, 2, 3, 4], false),
            Err(DdsPortError::InvalidType(_))
        ));
    }

    #[test]
    fn null_port_rejects_content_filter_but_accepts_clearing_it() {
        let mut p = NullDdsPort::new();
        let client = ck(4);
        let participant = (client, oid(1, ObjectKind::Participant));
        let topic = (client, oid(2, ObjectKind::Topic));
        let reader = (client, oid(3, ObjectKind::DataReader));
        p.create_participant(participant, 0).unwrap();
        p.create_topic(
            topic,
            participant,
            TopicSpec {
                topic_name: "T".into(),
                type_name: "Ty".into(),
                known_type: None,
            },
        )
        .unwrap();
        let subscriber = (client, oid(9, ObjectKind::Subscriber));
        p.create_subscriber(subscriber, participant, ContainerSpec::default())
            .unwrap();
        p.create_reader(reader, topic, subscriber, EndpointQos::writer_default())
            .unwrap();
        assert!(matches!(
            p.set_read_filter(reader, Some("x > 1")),
            Err(DdsPortError::InvalidType(_))
        ));
        assert!(p.set_read_filter(reader, None).is_ok());
        assert!(matches!(
            p.set_read_filter((ck(9), oid(1, ObjectKind::DataReader)), None),
            Err(DdsPortError::UnknownReference(_))
        ));
    }

    /// DDS-XRCE 1.0 §7.8.3.1, Table 6: the Publisher and Subscriber are
    /// created with the participant, which must exist; a DataWriter and a
    /// DataReader are created with their Publisher or Subscriber.
    #[test]
    fn publisher_and_subscriber_need_their_participant_and_endpoints_need_them() {
        let mut p = NullDdsPort::new();
        let client = ck(5);
        let participant = (client, oid(1, ObjectKind::Participant));
        let publisher = (client, oid(2, ObjectKind::Publisher));
        let subscriber = (client, oid(2, ObjectKind::Subscriber));
        let topic = (client, oid(3, ObjectKind::Topic));
        let writer = (client, oid(4, ObjectKind::DataWriter));
        let reader = (client, oid(4, ObjectKind::DataReader));
        assert!(matches!(
            p.create_publisher(publisher, participant, ContainerSpec::default()),
            Err(DdsPortError::UnknownReference(_))
        ));
        assert!(matches!(
            p.create_subscriber(subscriber, participant, ContainerSpec::default()),
            Err(DdsPortError::UnknownReference(_))
        ));
        p.create_participant(participant, 0).unwrap();
        p.create_topic(
            topic,
            participant,
            TopicSpec {
                topic_name: "T".into(),
                type_name: "Ty".into(),
                known_type: None,
            },
        )
        .unwrap();
        assert!(matches!(
            p.create_writer(writer, topic, publisher, EndpointQos::writer_default()),
            Err(DdsPortError::UnknownReference(_))
        ));
        assert!(matches!(
            p.create_reader(reader, topic, subscriber, EndpointQos::reader_default()),
            Err(DdsPortError::UnknownReference(_))
        ));
        p.create_publisher(publisher, participant, ContainerSpec::default())
            .unwrap();
        p.create_subscriber(subscriber, participant, ContainerSpec::default())
            .unwrap();
        p.create_writer(writer, topic, publisher, EndpointQos::writer_default())
            .unwrap();
        p.create_reader(reader, topic, subscriber, EndpointQos::reader_default())
            .unwrap();
        // Deleted, the Publisher is gone for the next writer.
        p.delete(publisher);
        assert!(matches!(
            p.create_writer(
                (client, oid(5, ObjectKind::DataWriter)),
                topic,
                publisher,
                EndpointQos::writer_default()
            ),
            Err(DdsPortError::UnknownReference(_))
        ));
    }

    /// A DataWriter and the topic it writes belong to one participant.
    #[test]
    fn a_publisher_and_a_topic_of_different_participants_do_not_match() {
        let mut p = NullDdsPort::new();
        let client = ck(6);
        let (pa, pb) = (
            (client, oid(1, ObjectKind::Participant)),
            (client, oid(2, ObjectKind::Participant)),
        );
        let publisher = (client, oid(3, ObjectKind::Publisher));
        let topic = (client, oid(4, ObjectKind::Topic));
        p.create_participant(pa, 0).unwrap();
        p.create_participant(pb, 0).unwrap();
        p.create_publisher(publisher, pa, ContainerSpec::default())
            .unwrap();
        p.create_topic(
            topic,
            pb,
            TopicSpec {
                topic_name: "T".into(),
                type_name: "Ty".into(),
                known_type: None,
            },
        )
        .unwrap();
        assert!(matches!(
            p.create_writer(
                (client, oid(5, ObjectKind::DataWriter)),
                topic,
                publisher,
                EndpointQos::writer_default()
            ),
            Err(DdsPortError::UnknownReference(_))
        ));
    }

    /// DDS 1.4 §2.2.3.20: a Publisher whose ENTITY_FACTORY does not enable
    /// what it creates leaves its DataWriters disabled; XRCE cannot enable
    /// them, so the creation is refused.
    #[test]
    fn a_publisher_that_does_not_autoenable_refuses_its_writers() {
        let mut p = NullDdsPort::new();
        let client = ck(7);
        let participant = (client, oid(1, ObjectKind::Participant));
        let publisher = (client, oid(2, ObjectKind::Publisher));
        let topic = (client, oid(3, ObjectKind::Topic));
        p.create_participant(participant, 0).unwrap();
        p.create_publisher(
            publisher,
            participant,
            ContainerSpec {
                autoenable_created_entities: false,
                ..ContainerSpec::default()
            },
        )
        .unwrap();
        p.create_topic(
            topic,
            participant,
            TopicSpec {
                topic_name: "T".into(),
                type_name: "Ty".into(),
                known_type: None,
            },
        )
        .unwrap();
        assert!(matches!(
            p.create_writer(
                (client, oid(4, ObjectKind::DataWriter)),
                topic,
                publisher,
                EndpointQos::writer_default()
            ),
            Err(DdsPortError::DdsError(_))
        ));
    }

    #[test]
    fn null_port_poll_samples_always_empty() {
        let mut p = NullDdsPort::new();
        let reader = (ck(1), oid(1, ObjectKind::DataReader));
        assert!(p.poll_samples(reader, 10).is_empty());
    }

    #[test]
    fn unregister_client_drops_all_its_objects() {
        let mut p = NullDdsPort::new();
        let a = ck(1);
        let b = ck(2);
        p.create_participant((a, oid(1, ObjectKind::Participant)), 0)
            .unwrap();
        p.create_participant((b, oid(1, ObjectKind::Participant)), 0)
            .unwrap();
        p.unregister_client(a);
        assert!(!p.known.contains_key(&(a, oid(1, ObjectKind::Participant))));
        assert!(p.known.contains_key(&(b, oid(1, ObjectKind::Participant))));
    }
}
