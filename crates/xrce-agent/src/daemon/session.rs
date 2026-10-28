// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The pure DDS-XRCE session protocol engine.
//!
//! [`SessionEngine`] is a function from "inbound `Message`" to "outbound
//! `Message`s (+ DDS-side effects through [`DdsPort`])" — no sockets, no
//! clock of its own (`now: Duration` is passed in by the caller on every
//! call). That is deliberate: the same engine instance is what
//! `tests/replay_foreign_session.rs` drives with real eProsima Client 3.0.1
//! bytes and what `crate::daemon::transport` drives from a live UDP/TCP/
//! serial socket.
//!
//! ## What every request gets back
//!
//! Per this task's rule — "a sample or request the daemon cannot handle is
//! answered with the spec's STATUS error, or logged as an observability
//! event; never dropped silently" — every branch below either:
//!
//! 1. produces a spec submessage reply (`STATUS`/`STATUS_AGENT`/`INFO`/
//!    `DATA`/`TIMESTAMP_REPLY`) with a [`ResultStatusCode`] chosen from
//!    Table 7.7.7, or
//! 2. for the handful of cases the spec gives no reply slot for (a
//!    corrupt `FRAGMENT` reassembly, a `CREATE_CLIENT` whose cookie does
//!    not read `"XRCE"`), still answers with a `STATUS` against
//!    `OBJECTID_CLIENT` rather than staying silent — a documented
//!    extension, not a literal spec citation (see the inline comment at
//!    each such site).
//!
//! ## Reliability (§8.4.14)
//!
//! One [`ReliableStreamState`] per `(client, stream_id)` serves both
//! directions at once: its sender half tracks what *this agent* has sent
//! on that stream, its receiver half tracks what *the client* has sent —
//! the same split `reliable.rs`'s own end-to-end test uses across two
//! separate instances, collapsed into one because both roles belong to
//! the same agent here. Both halves are fully wired, not just present:
//!
//! - **Receiver** (client → agent): [`Self::handle_message`] gates every
//!   message on a reliable stream (`StreamId::is_reliable`) through
//!   `ReliableStreamState::recv_data`/`drain_in_order` *before* dispatch
//!   — an out-of-order message is buffered, not processed, until the gap
//!   ahead of it fills; a duplicate/already-delivered retransmission is
//!   silently absorbed by `recv_data` itself (the correct answer to a
//!   duplicate, not a dropped request). Only once `drain_in_order`
//!   releases a message, in order, does it reach
//!   [`Self::process_message_submessages`] (§8.3.5.14 FRAGMENT
//!   reassembly, then the real handlers). Best-effort/`StreamId::NONE`
//!   traffic skips this gate entirely and is dispatched as it arrives.
//! - **Sender** (agent → client): [`Self::finalize_reliable_sequencing`]
//!   assigns every reliable-stream `Outbound` a real, buffered sequence
//!   number via `ReliableStreamState::submit` (§8.4.14.1: "each time a
//!   message is sent the HighestSentSequenceNumber is increased") —
//!   before this, every reply on every stream carried `sequence_nr = 0`
//!   unconditionally. [`Self::handle_acknack`] answers a NACK'd sequence
//!   number (a bit set in the 16-entry bitmap) by resending the exact
//!   buffered bytes via `get_in_flight`, not just updating acknowledgment
//!   bookkeeping.
//!
//! `Vec<Submessage>` per message is bridged through `ReliableStreamState`'s
//! byte-oriented buffering via [`encode_submessages`]/[`decode_submessages`]
//! rather than reimplementing a submessage-aware reorder/retransmit buffer.

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::time::Duration;

use zerodds_xrce::encoding::Endianness;
use zerodds_xrce::fragment::{AssemblerKey, DEFAULT_GC_TTL, FragmentAssembler};
use zerodds_xrce::header::{CLIENT_KEY_LEN, ClientKey, MessageHeader, SessionId, StreamId};
use zerodds_xrce::object_id::{OBJECTID_CLIENT, OBJECTID_INVALID, ObjectId};
use zerodds_xrce::object_info::{
    ActivityInfoVariant, BaseObjectReply, BaseObjectRequest, InfoMask, ObjectConfig, ObjectInfo,
    ResultStatus, ResultStatusCode,
};
use zerodds_xrce::object_kind::ObjectKind;
use zerodds_xrce::object_repr::{ObjectVariant, ObjectVariantExtra};
use zerodds_xrce::object_store::{CreateOutcome, CreationMode};
use zerodds_xrce::reliable::{ReliableConfig, ReliableStreamState};
use zerodds_xrce::serial_number::SerialNumber16;
use zerodds_xrce::submessages::timestamp::TimePoint;
use zerodds_xrce::submessages::{
    AckNackPayload, CreateClientPayload, CreatePayload, DataPayload, DeletePayload,
    FragmentPayload, GetInfoPayload, HeartbeatPayload, InfoPayload, ReadDataPayload,
    StatusAgentPayload, StatusPayload, Submessage, SubmessageHeader, SubmessageId,
    TimestampPayload, TimestampReplyPayload,
};
use zerodds_xrce::transport_locator::TransportLocator;
use zerodds_xrce::{DeliveryControl, MAX_SAMPLES_ZERO, PendingSample, ReadStream};

use crate::daemon::access::{Operation, Subject};
use crate::daemon::dds_port::{
    ContainerSpec, DdsPort, DdsPortError, KnownType, ParticipantQos, SourceTime, TopicSpec,
};
use crate::daemon::definitions::ParticipantScope;
use crate::daemon::entity_repr::{
    self, ContainerDescription, EndpointDescription, EndpointRole, ParticipantSource, ReprError,
    TopicDescription, TopicRef,
};
use crate::daemon::handshake::{AgentRepresentation, ClientRepresentation};
use crate::daemon::profiles::AgentProfiles;
use crate::{AgentError, TraceSink, XrceAgent};

/// `[u8; CLIENT_KEY_LEN]` — the map-friendly form of [`ClientKey`], same
/// convention as `crate::ClientKeyOrd` (private to the crate root).
pub type ClientKeyOrd = [u8; CLIENT_KEY_LEN];

/// One outbound datagram the transport layer must deliver — addressed by
/// `ClientKeyOrd` rather than a socket address, which is the transport's
/// job to resolve (its own `(peer address) -> ClientKeyOrd` table, updated
/// from [`SessionEvent::ClientRegistered`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outbound {
    /// Destination client.
    pub client: ClientKeyOrd,
    /// The message header (session id, stream id, sequence number).
    pub header: MessageHeader,
    /// One or more submessages to send as a single `Message`.
    pub submessages: Vec<Submessage>,
}

/// The DDS domain and topic name behind an object, for the
/// AccessController (§7.4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ObjectScope {
    domain: Option<u16>,
    topic: Option<String>,
}

/// The GET_INFO an INFO answers.
struct InfoRequest<'a> {
    header: &'a MessageHeader,
    /// Whom the INFO is addressed to: the client, or `CLIENTKEY_INVALID`
    /// when the request came from no session.
    reply_to: ClientKeyOrd,
    client: Option<ClientKeyOrd>,
    base: BaseObjectRequest,
}

/// What the agent counts of a DataWriter (§7.7.12 `DATAWRITER_ActivityInfo`).
#[derive(Debug, Clone, Copy, Default)]
struct WriterActivity {
    /// Samples written through it.
    samples: u64,
    /// The `sequenceNr` of the message of the last write, on its stream.
    last_stream_seq: i16,
}

/// One sample of a WRITE_DATA with what its sample information says.
struct WriteItem {
    data: Vec<u8>,
    dispose: bool,
    unregister: bool,
    /// `session_time_offset` in milliseconds, for a sample that came with
    /// sample information.
    session_time_offset: Option<u32>,
}

/// What stands in front of each `SampleData` of a sequence format.
enum Front {
    /// FORMAT_DATA_SEQ: nothing.
    None,
    /// FORMAT_SAMPLE_SEQ: a `SampleInfo`.
    Info,
    /// FORMAT_PACKED_SAMPLES: a `SampleInfoDelta` against the common info.
    Delta(zerodds_xrce::sample_info::SampleInfo),
}

/// Why the samples of a WRITE_DATA could not be told apart.
struct WriteSplitError {
    status: ResultStatusCode,
    message: String,
}

/// An object as it was before a replace (`SessionEngine::handle_create`).
struct PreviousObject {
    variant: ObjectVariant,
    extra: Option<ObjectVariantExtra>,
    scope: Option<ObjectScope>,
    plan: Option<DdsCreate>,
}

/// The DDS side of one CREATE, read from its representation before the
/// object is stored (`SessionEngine::plan_dds_create`).
#[derive(Clone)]
enum DdsCreate {
    /// The kind has no DDS side.
    Nothing,
    /// A DomainParticipant in `domain_id`. `source` is the DDS-XML
    /// definition behind it, if it has one: the objects inside it are made
    /// with it.
    Participant {
        domain_id: i32,
        qos: ParticipantQos,
        /// The scope a binary `domain_reference` gives the participant.
        scope: Option<ParticipantScope>,
        not_applied: Vec<String>,
        source: Option<Box<ParticipantSource>>,
    },
    /// An Application: its DomainParticipants and what is inside them are
    /// made with it (Table 6).
    Application {
        participants: Vec<ParticipantSource>,
    },
    /// A topic in `participant`.
    Topic {
        participant: ObjectId,
        description: TopicDescription,
    },
    /// A Publisher or Subscriber of `participant`.
    Container {
        participant: ObjectId,
        description: ContainerDescription,
    },
    /// The types an `OBJK_TYPE` defines, by name.
    Types(Vec<(String, zerodds_types::CompleteTypeObject)>),
    /// A DataWriter or DataReader on `topic`.
    Endpoint {
        role: EndpointRole,
        topic: ObjectId,
        /// The Publisher (writer) or Subscriber (reader) it is created with.
        container: ObjectId,
        description: Box<EndpointDescription>,
    },
}

/// The time limit of a read for the `max_elapsed_time` of its
/// `DataDeliveryControl`: 0 is `MAX_ELAPSED_TIME_UNLIMITED`, else a number of
/// seconds (Table 9) or of milliseconds (the reading of Annex B.4.2).
fn elapsed_time_limit(value: u16, milliseconds: bool) -> Duration {
    match (value, milliseconds) {
        (0, _) => Duration::MAX,
        (v, false) => Duration::from_secs(u64::from(v)),
        (v, true) => Duration::from_millis(u64::from(v)),
    }
}

/// The elements of the representation a plan does not apply.
fn plan_not_applied(plan: &DdsCreate) -> Vec<String> {
    match plan {
        DdsCreate::Participant { not_applied, .. } => not_applied.clone(),
        DdsCreate::Topic { description, .. } => description.not_applied.clone(),
        DdsCreate::Container { description, .. } => description.not_applied.clone(),
        DdsCreate::Endpoint { description, .. } => description.not_applied.clone(),
        DdsCreate::Nothing | DdsCreate::Application { .. } | DdsCreate::Types(_) => Vec::new(),
    }
}

/// The DataWriter or DataReader elements a Publisher or Subscriber plan holds.
fn embedded_of(plan: &DdsCreate) -> Vec<zerodds_xml::XmlElement> {
    match plan {
        DdsCreate::Container { description, .. } => description.embedded.clone(),
        _ => Vec::new(),
    }
}

/// The `STATUS` of a `get_info` of the Root for the `client_info` it carries
/// (§7.8.2.2, §8.4.4 Figure 10): `STATUS_ERR_INCOMPATIBLE` when the
/// `CLIENT_Representation` names another major version of the protocol than
/// the agent's (as `create_client` does, §7.8.2.1), `STATUS_ERR_INVALID_DATA`
/// when the octets behind the mask are no `client_info`, else `STATUS_OK`. A
/// request without `client_info` is `STATUS_OK`.
pub(crate) fn client_info_status(payload: &GetInfoPayload, e: Endianness) -> ResultStatusCode {
    use zerodds_xrce::object_info::ObjectConfig;
    match payload.client_info(e) {
        Ok(None) => ResultStatusCode::Ok,
        Ok(Some(info)) => match info.config {
            Some(ObjectConfig::Client(rep))
                if rep.version[0] != crate::daemon::handshake::XRCE_VERSION[0] =>
            {
                ResultStatusCode::Incompatible
            }
            Some(ObjectConfig::Client(_)) | None => ResultStatusCode::Ok,
            // §7.8.2.2: "The ObjectVariant member within client_info shall
            // contain a CLIENT_Representation".
            Some(_) => ResultStatusCode::InvalidData,
        },
        Err(_) => ResultStatusCode::InvalidData,
    }
}

/// The predefined objects of `profiles` by `ObjectId`; where several have one
/// (the same name in two participants), the first.
fn predefined_index(
    profiles: &AgentProfiles,
) -> BTreeMap<ObjectId, crate::daemon::profiles::PredefinedObject> {
    let mut index = BTreeMap::new();
    for object in profiles.predefined_objects().unwrap_or_default() {
        index.entry(object.oid).or_insert(object);
    }
    index
}

/// Whether the object kind has a representation in the format of `variant`
/// (DDS-XRCE 1.0 §7.7.3.1, Annex A): `OBJK_Representation3_Base` for a
/// DomainParticipant and a Topic (all three), `OBJK_RepresentationRefAndXML_Base`
/// for a Type, QosProfile, Application and Domain (by reference or XML),
/// `OBJK_RepresentationBinAndXML_Base` for a Publisher, Subscriber, DataWriter
/// and DataReader (binary or XML). Other kinds have no representation to check.
fn format_is_allowed(kind: ObjectKind, variant: &ObjectVariant) -> bool {
    match kind {
        ObjectKind::Type | ObjectKind::QosProfile | ObjectKind::Application | ObjectKind::Other => {
            !matches!(variant, ObjectVariant::InBinary(_))
        }
        ObjectKind::Publisher
        | ObjectKind::Subscriber
        | ObjectKind::DataWriter
        | ObjectKind::DataReader => !matches!(variant, ObjectVariant::ByReference(_)),
        _ => true,
    }
}

/// The DomainParticipant definitions a plan makes objects for: the one behind
/// a DomainParticipant, those of an Application.
fn nested_sources_of(plan: &DdsCreate) -> Vec<ParticipantSource> {
    match plan {
        DdsCreate::Participant {
            source: Some(source),
            ..
        } => vec![(**source).clone()],
        DdsCreate::Application { participants } => participants.clone(),
        _ => Vec::new(),
    }
}

/// A side effect the transport/daemon layer should act on, beyond sending
/// bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    /// A message named a ClientKey the agent holds no ProxyClient for and was
    /// dropped without an answer (DDS-XRCE 1.0 §7.8.1: "the failure to locate
    /// a ClientKey may [...] silently drop the connection to the client"). The
    /// transport counts these per connection and closes it at the threshold.
    NoClient {
        /// The ClientKey the message named.
        client: ClientKeyOrd,
    },
    /// A new client completed its handshake with the given `session_id`
    /// and `ClientKey` — the transport layer should now bind the peer
    /// address this datagram arrived from to `client`, so future
    /// header-less (`session_id >= 128`) datagrams from that address
    /// resolve to it.
    ClientRegistered {
        /// The client's identity.
        client: ClientKeyOrd,
        /// The session id the agent accepted.
        session_id: u8,
    },
    /// A client's session ended (DELETE(CLIENT) or timeout) — the
    /// transport layer should drop its address mapping.
    ClientUnregistered {
        /// The client that is gone.
        client: ClientKeyOrd,
    },
    /// A request whose body cannot be read: a WRITE_DATA, READ_DATA or DELETE
    /// that is no `WRITE_DATA_Payload`, `READ_DATA_Payload` or
    /// `BaseObjectRequest`. It is answered with `STATUS_ERR_INVALID_DATA`
    /// (against the request_id and object_id when the first four octets are
    /// there), and this says so.
    RequestUnreadable {
        /// The client.
        client: ClientKeyOrd,
        /// The submessage.
        submessage: SubmessageId,
        /// The length of its body.
        body_len: usize,
    },
    /// A `TIMESTAMP_REPLY` answered a `TIMESTAMP` the agent sent
    /// (§8.3.5.15, §8.3.5.16): the clock of the client against the agent's,
    /// by the estimate of NTP from the four timestamps.
    ClockSynchronised {
        /// The client.
        client: ClientKeyOrd,
        /// The clock of the client minus the clock of the agent, in
        /// nanoseconds.
        offset_nanos: i64,
        /// The time the exchange took, without the time the client held the
        /// message, in nanoseconds.
        round_trip_nanos: i64,
    },
    /// A topic's type is unknown to this agent (no `TypeObject` was ever
    /// offered for it) — samples on it are forwarded opaque, with no
    /// `TypeInformation` announced and no key tracking. Emitted once per
    /// `(client, topic)`, per spec item 5's observability requirement.
    UnknownTypeForwarding {
        /// The client whose topic this is.
        client: ClientKeyOrd,
        /// The topic's `ObjectId`.
        topic: ObjectId,
        /// The DDS type name the client (or config) gave for it.
        type_name: String,
    },
    /// A DDS-side operation failed — surfaced to the client as a `STATUS`
    /// already, this event is the same failure for whatever structured
    /// log/metrics sink the daemon wires up (never the only place the
    /// failure is visible).
    DdsPortFailed {
        /// The client the operation was for.
        client: ClientKeyOrd,
        /// The object the operation targeted.
        object: ObjectId,
        /// What went wrong.
        error: String,
    },
    /// A CREATE was refused before anything was stored: its representation
    /// does not parse, or names a profile, topic or container the agent
    /// does not hold (`crate::daemon::entity_repr`). The client got the
    /// matching STATUS.
    CreateRefused {
        /// The client whose CREATE this was.
        client: ClientKeyOrd,
        /// The object it would have created.
        object: ObjectId,
        /// Why.
        reason: String,
    },
    /// A CREATE's representation carries elements the agent does not apply
    /// to the DDS entity (a Fast DDS profile element with no DDS policy
    /// behind it, e.g. `qos/publishMode`). The entity was created without
    /// them.
    RepresentationNotApplied {
        /// The client whose CREATE this was.
        client: ClientKeyOrd,
        /// The object created.
        object: ObjectId,
        /// The elements, by their path under the entity's XML element.
        elements: Vec<String>,
    },
    /// A client's FRAGMENT run stayed incomplete for longer than the
    /// reassembly TTL (`zerodds_xrce::fragment::DEFAULT_GC_TTL`) and was
    /// discarded: the message it carried is lost.
    FragmentsDiscarded {
        /// The client the fragments came from.
        client: ClientKeyOrd,
        /// How many runs were discarded.
        runs: usize,
    },
    /// A reliable-stream outbound message could not be sequenced/buffered
    /// (§8.4.14.1: "Each time a message is sent the
    /// HighestSentSequenceNumber is increased") — the reliable sender's
    /// window is full (the client has not acked enough of what this
    /// agent already sent) or the message body exceeds
    /// `RELIABLE_MAX_PAYLOAD`. The message is still sent, with
    /// `sequence_nr = 0`, rather than dropped — degrading to
    /// unsequenced, unretransmittable delivery for that one message
    /// only, not silence.
    ReliableSendOverflowed {
        /// The client the message was addressed to.
        client: ClientKeyOrd,
        /// The reliable stream that could not take the message.
        stream_id: u8,
        /// Why `ReliableStreamState::submit` refused it.
        reason: String,
    },
    /// A sample of a READ_DATA is longer than a DATA submessage can carry
    /// (`submessageLength` is 16 bits, §8.3.4.3); the client gets a STATUS
    /// `STATUS_ERR_RESOURCES` for the read instead.
    SampleTooLarge {
        /// The client of the read.
        client: ClientKeyOrd,
        /// The DataReader.
        reader: ObjectId,
        /// The length of the sample.
        len: usize,
    },
    /// The AccessController (§7.4) refused a `CREATE_CLIENT` or an object
    /// operation. The client got `STATUS_ERR_DENIED`.
    OperationDenied {
        /// The client the request came from.
        client: ClientKeyOrd,
        /// The object the request named (`OBJECTID_CLIENT` for a
        /// `CREATE_CLIENT`).
        object: ObjectId,
        /// The operation (`create_client`, `create`, `update`, `get_info`,
        /// `delete`, `write_data`, `read_data`) and what refused it.
        what: String,
    },
    /// A message was not processed: it carries no session
    /// (`SESSION_ID_NONE_*`) but something other than CREATE_CLIENT, or its
    /// `session_id` is not the client's active session (DDS-XRCE 1.0
    /// §8.2.2: "Creation of a new session closes any previous sessions";
    /// §8.3.2.1).
    MessageRefused {
        /// The client the message was resolved to, if any.
        client: Option<ClientKeyOrd>,
        /// The `session_id` of the header.
        session_id: u8,
        /// The `stream_id` of the header.
        stream_id: u8,
        /// Why.
        reason: String,
    },
    /// A message of a best-effort stream was discarded because it is a
    /// duplicate of the last one or older (DDS-XRCE 1.0 §8.3.2.3: "Messages
    /// within a stream shall not be delivered out of order. In addition,
    /// duplicate messages shall be discarded.").
    BestEffortMessageDiscarded {
        /// The client of the stream.
        client: ClientKeyOrd,
        /// The stream.
        stream_id: u8,
        /// The `sequenceNr` of the discarded message.
        sequence_nr: u16,
        /// The `sequenceNr` of the newest message taken.
        newest: u16,
    },
    /// A read ended by `max_elapsed_time` while samples taken from the
    /// DataReader waited for the rate or the pace; they are not delivered.
    ReadEndedUndelivered {
        /// The client of the read.
        client: ClientKeyOrd,
        /// The DataReader.
        reader: ObjectId,
        /// How many samples.
        samples: usize,
    },
}

/// What a READ_DATA without `delivery_control` reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReadWithoutDeliveryControl {
    /// DDS-XRCE 1.0 §8.3.5.9.2: one read call; "the only data returned will
    /// be the one already in the DDS DataReader cache".
    #[default]
    Once,
    /// One sample, the next there is, without a time limit: what
    /// Micro-XRCE-DDS 3.0.1 does (`DataReader::read`, `max_samples(1)`).
    /// Not the specification; opt-in for clients that rely on it.
    NextSample,
}

/// Configuration [`SessionEngine::new`] takes.
#[derive(Debug, Clone)]
pub struct SessionEngineConfig {
    /// This agent's `xrce_vendor_id` (Annex A `XrceVendorId`) —
    /// `[0, 0]` if this deployment has none registered.
    pub agent_vendor_id: [u8; 2],
    /// Maximum concurrently registered clients (`STATUS_ERR_RESOURCES`
    /// beyond this).
    pub max_clients: usize,
    /// Maximum objects one client's `ObjectStore` may hold
    /// (`STATUS_ERR_RESOURCES` beyond this).
    pub max_objects_per_client: usize,
    /// Maximum samples buffered per `(client, reader)` pull queue.
    pub max_pending_samples: usize,
    /// A session with no traffic for this long is torn down by
    /// [`SessionEngine::tick`] (spec §8.4.9-adjacent liveliness).
    pub session_timeout: Duration,
    /// The locators this agent listens on, for `GET_INFO(OBJECTID_AGENT)`
    /// → `INFO`'s `ActivityInfoVariant::Agent { address_seq, .. }`.
    pub listening_locators: Vec<TransportLocator>,
    /// Topic types the daemon knows for every client (`type_files:`), by
    /// fully qualified name. A client's own `OBJK_TYPE` of the same name
    /// takes precedence for that client.
    pub known_types: BTreeMap<String, zerodds_types::CompleteTypeObject>,
    /// The profiles `BY_REFERENCE` representations and `qos_profile`
    /// references resolve against (`xml_profile_files:`,
    /// `qos_library_files:`).
    pub profiles: crate::daemon::profiles::AgentProfiles,
    /// What a READ_DATA without `delivery_control` reads
    /// (`read_without_delivery_control:`).
    pub read_without_delivery_control: ReadWithoutDeliveryControl,
    /// Whether a topic whose type the agent does not know is created and its
    /// samples forwarded opaque, as the eProsima client expects for the types
    /// it names. By default (`false`) it is refused with
    /// `STATUS_ERR_UNKNOWN_REFERENCE` (DDS-XRCE 1.0 §7.7.3.7.3: "The
    /// referenced type shall be known to the XRCE Agent").
    pub forward_unknown_topic_types: bool,
    /// The unit of `max_elapsed_time` of a `DataDeliveryControl`. DDS-XRCE 1.0
    /// Table 9 defines it in seconds (the default, `false`); the example of
    /// Annex B.4.2 writes 30000 for 30 seconds, which reads as milliseconds
    /// (`true`, agent key `max_elapsed_time_unit: milliseconds`).
    pub max_elapsed_time_in_milliseconds: bool,
    /// Whether `STATUS_AGENT` always carries a `ResultStatus` before the
    /// `AGENT_Representation` (the text of §8.3.5.5.2, the capture of
    /// eProsima). By default (`false`) the payload is the `agent_info` of
    /// Annex A; a failure carries the `ResultStatus` in either case.
    pub status_agent_with_result_status: bool,
    /// Whether `DELETE(OBJECTID_CLIENT)` (`delete_client`) is answered with
    /// `STATUS_AGENT`, as Table 14 of DDS-XRCE 1.0 has it. By default
    /// (`false`) it is a `STATUS`, as §8.3.5.4 says and as the IDL of Annex A
    /// allows: a `STATUS_AGENT` has no `ResultStatus` of its own there.
    pub delete_client_reply_is_status_agent: bool,
    /// Whether a representation that carries QoS the agent cannot apply (a
    /// policy the DDS runtime does not take, an element of a vendor dialect)
    /// is created with that part ignored, reported as
    /// `SessionEvent::RepresentationNotApplied`. By default (`false`) it is
    /// refused with `STATUS_ERR_INCOMPATIBLE`: an object whose QoS is not the
    /// one asked for is not what the client asked to create.
    pub ignore_unsupported_qos: bool,
    /// Whether a `CREATE_CLIENT` with the `session_id` of the client's
    /// session restarts the streams of that session. DDS-XRCE 1.0 §7.8.2.1
    /// says "shall not perform any action" (`false`, the default). A client
    /// process that restarts with the same `ClientKey` and `session_id`
    /// starts its sequence numbers at 0 again, as the eProsima examples of
    /// the captured session do; `true` accepts that, and the agent then
    /// forgets the stream state of the client.
    pub restart_on_same_session: bool,
    /// Whether a CREATE with `reuse` and `replace` — the update of §8.4.13 —
    /// of an object that does not exist is `STATUS_ERR_UNKNOWN_REFERENCE`
    /// (§7.8.3.2) instead of creating it (Table 5).
    pub refuse_update_of_missing_object: bool,
    /// Whether a CREATE may use any `RepresentationFormat` for any kind.
    /// DDS-XRCE 1.0 §7.7.3.1 and Annex A give each kind the formats it has: a
    /// Type, QosProfile and Application are by reference or XML, a
    /// Publisher, Subscriber, DataWriter and DataReader binary or XML, a
    /// DomainParticipant and Topic all three; a format outside that is
    /// `STATUS_ERR_INVALID_DATA` (the default, `false`). `true` admits the
    /// others: the eProsima Micro XRCE-DDS client creates its Publishers,
    /// Subscribers, DataWriters and DataReaders by reference
    /// (`representation_formats: any`).
    pub allow_any_representation_format: bool,
    /// How often the agent sends a `TIMESTAMP` to each client (§8.3.5.15: "It
    /// may be used as part of a higher-level clock-synchronization
    /// mechanism"); `None` sends none.
    pub timestamp_interval: Option<Duration>,
    /// The Applications and DomainParticipants of the agent's definitions
    /// that are made for a client when its `CREATE_CLIENT` arrives
    /// (`client_applications:`), by `ClientKey`. DDS-XRCE 1.0 §8.4.2, §8.4.3:
    /// "An Agent has been pre-configured for a Client (identified by the
    /// ClientKey) [...]. The reception of the CREATE_CLIENT triggers the
    /// creation or reuse of the corresponding XRCE objects."
    pub client_applications: Vec<(ClientKeyOrd, String)>,
    /// The AccessController of DDS-XRCE 1.0 §7.4: the ClientKeys
    /// configured in the agent, the properties a `CREATE_CLIENT` must
    /// carry, and the per-client permissions for the object operations.
    /// The default restricts nothing.
    pub access: crate::daemon::access::AccessController,
}

impl Default for SessionEngineConfig {
    fn default() -> Self {
        Self {
            agent_vendor_id: [0, 0],
            max_clients: 1024,
            max_objects_per_client: 256,
            max_pending_samples: 256,
            session_timeout: Duration::from_secs(60),
            listening_locators: Vec::new(),
            known_types: BTreeMap::new(),
            profiles: crate::daemon::profiles::AgentProfiles::default(),
            read_without_delivery_control: ReadWithoutDeliveryControl::Once,
            forward_unknown_topic_types: false,
            ignore_unsupported_qos: false,
            status_agent_with_result_status: false,
            delete_client_reply_is_status_agent: false,
            max_elapsed_time_in_milliseconds: false,
            restart_on_same_session: false,
            refuse_update_of_missing_object: false,
            allow_any_representation_format: false,
            timestamp_interval: None,
            client_applications: Vec::new(),
            access: crate::daemon::access::AccessController::default(),
        }
    }
}

/// Continuous-read registration for one `(client, reader)`.
struct ActiveRead {
    stream: ReadStream,
    request_id: [u8; 2],
    reply_stream_id: u8,
    /// The `data_format` of the READ_DATA: the format of every DATA
    /// (§8.3.5.10.2).
    format: zerodds_xrce::submessages::write_data::DataFormat,
}

/// Bookkeeping for an in-flight inbound `FRAGMENT` reassembly on one
/// `(client, stream_id)`: the sequence number of the run's *first*
/// fragment (the [`AssemblerKey::base_seq`] every fragment of this run
/// must share) and how many bytes have been accumulated so far.
///
/// `FRAGMENT`'s wire format (`crate::submessages::fragment`) carries no
/// offset field — only an opaque body slice and a `LAST` flag — and the
/// spec restricts it to reliable streams (§8.3.5.14), whose ACKNACK-based
/// delivery guarantees a client's fragments of one run arrive at this
/// engine gap-free and in order. That makes "bytes accumulated so far"
/// exactly the true offset, and the first fragment's own sequence number
/// the run's `base_seq` — both of which have to be remembered here across
/// calls, since [`FragmentAssembler::add_fragment`] takes an explicit
/// `offset` and `AssemblerKey` rather than inferring either itself.
struct FragmentRun {
    base_seq: u16,
    next_offset: u32,
    /// Tick of the last fragment accepted into this run — GC'd in
    /// `tick()` on the same [`DEFAULT_GC_TTL`] as the underlying
    /// [`FragmentAssembler`]'s own `PendingAssembly`, so a run whose
    /// `FragmentAssembler` half was already discarded can never be
    /// "resumed" at a stale offset into a fresh, empty reassembly.
    last_arrival: Duration,
}

/// The pure protocol engine.
pub struct SessionEngine<P: DdsPort> {
    agent: XrceAgent,
    dds: P,
    config: SessionEngineConfig,
    reliable: BTreeMap<(ClientKeyOrd, u8), ReliableStreamState>,
    fragments: BTreeMap<ClientKeyOrd, FragmentAssembler>,
    /// Reliable-stream messages waiting for room in the sender window.
    reliable_backlog: BTreeMap<(ClientKeyOrd, u8), alloc::collections::VecDeque<Outbound>>,
    /// The next sequence number of each best-effort output stream.
    best_effort_seq: BTreeMap<(ClientKeyOrd, u8), SerialNumber16>,
    /// The `sequenceNr` of the newest message taken on each best-effort
    /// input stream (§8.3.2.3).
    best_effort_in: BTreeMap<(ClientKeyOrd, u8), SerialNumber16>,
    /// Per-`(client, stream_id)` state for the fragment run currently
    /// being reassembled on that stream — see [`FragmentRun`].
    fragment_runs: BTreeMap<(ClientKeyOrd, u8), FragmentRun>,
    reads: BTreeMap<(ClientKeyOrd, ObjectId), ActiveRead>,
    liveliness: BTreeMap<ClientKeyOrd, Duration>,
    session_ids: BTreeMap<ClientKeyOrd, u8>,
    /// Topics whose type is known — so `WRITE_DATA`/`READ_DATA` can tell
    /// [`DdsPort`] whether to key/track instances, and so
    /// `SessionEvent::UnknownTypeForwarding` fires only once.
    known_type_topics: BTreeMap<(ClientKeyOrd, ObjectId), bool>,
    /// Every topic with a DDS side, by the name and type it was created
    /// with — so an endpoint can name its topic by DDS name (the Fast DDS
    /// element form) and have its type checked against it.
    topics: BTreeMap<(ClientKeyOrd, ObjectId), TopicDescription>,
    /// Every Publisher/Subscriber, with the PARTITION/GROUP_DATA its
    /// endpoints inherit.
    containers: BTreeMap<(ClientKeyOrd, ObjectId), ContainerDescription>,
    /// The types each client's `OBJK_TYPE` objects define, by name.
    client_types:
        BTreeMap<(ClientKeyOrd, ObjectId), Vec<(String, zerodds_types::CompleteTypeObject)>>,
    /// The DDS domain and topic name behind each object that has them
    /// (participants and everything below; topics and endpoints): what the
    /// AccessController (§7.4) matches its rules against.
    scopes: BTreeMap<(ClientKeyOrd, ObjectId), ObjectScope>,
    /// The participant of each topic, Publisher and Subscriber.
    participants_of: BTreeMap<(ClientKeyOrd, ObjectId), ObjectId>,
    /// The trailing field (domain, container) each object was created with:
    /// what the `config` of a GET_INFO needs to write the object's
    /// `ObjectVariant` again.
    extras: BTreeMap<(ClientKeyOrd, ObjectId), ObjectVariantExtra>,
    /// What the agent counts of each DataWriter: `DATAWRITER_ActivityInfo`.
    writer_activity: BTreeMap<(ClientKeyOrd, ObjectId), WriterActivity>,
    /// The DDS side each object was created with, so that a replace whose
    /// new DDS side fails can create the previous one again (§7.8.3.2: "the
    /// referenced object shall return to its previous configuration").
    plans: BTreeMap<(ClientKeyOrd, ObjectId), DdsCreate>,
    /// The agent's definitions: those of its files, and the libraries the
    /// objects of the clients define (`dynamic_definitions`).
    profiles: AgentProfiles,
    /// The XML library (`<qos_library>`, `<types>`, `<application_library>`,
    /// `<domain_participant_library>`) each object defines, which other
    /// clients refer to while the object exists (§7.7.3.1.1, §9.2).
    dynamic_definitions: BTreeMap<(ClientKeyOrd, ObjectId), String>,
    /// The objects the definitions amount to, by `ObjectId` (§9.3, Table 16):
    /// a client that names one it has not made gets it made.
    predefined: BTreeMap<ObjectId, crate::daemon::profiles::PredefinedObject>,
    /// What each DomainParticipant of a client defines: its type
    /// registrations and topics, those of its Domain included.
    participant_scopes: BTreeMap<(ClientKeyOrd, ObjectId), ParticipantScope>,
    /// When the agent last sent a `TIMESTAMP` to each client (`now` of the
    /// engine).
    last_timestamp: BTreeMap<ClientKeyOrd, Duration>,
    /// The `request_id` of the `DELETE(OBJECTID_CLIENT)` that ended the session
    /// of each client the agent no longer holds: a retransmission of that
    /// request (the client did not see the answer) is answered `STATUS_OK`
    /// again, not `STATUS_ERR_UNKNOWN_REFERENCE`.
    deleted_clients: BTreeMap<ClientKeyOrd, [u8; 2]>,
    /// The sequence number of the last sample the agent delivered from each
    /// DataReader, for the `SampleInfo` of the formats that carry one.
    read_sequence: BTreeMap<(ClientKeyOrd, ObjectId), u32>,
    /// When each client's session was created, by the agent's clock: the
    /// session timestamp `session_time_offset` is relative to (§7.7.2).
    session_start: BTreeMap<ClientKeyOrd, TimePoint>,
    /// The `mtu` a client's `CLIENT_Representation` announced (or
    /// [`crate::daemon::handshake::DEFAULT_MTU`] if it sent none) — the
    /// outbound `DATA`/`FRAGMENT` split threshold in [`Self::pump_read`].
    client_mtu: BTreeMap<ClientKeyOrd, u16>,
    events: Vec<SessionEvent>,
}

impl<P: DdsPort> SessionEngine<P> {
    /// Constructs an engine over the given [`DdsPort`].
    pub fn new(dds: P, config: SessionEngineConfig) -> Self {
        let profiles = config.profiles.clone();
        let predefined = predefined_index(&profiles);
        Self {
            agent: XrceAgent::with_max_pending_samples(config.max_pending_samples),
            dds,
            config,
            reliable: BTreeMap::new(),
            fragments: BTreeMap::new(),
            fragment_runs: BTreeMap::new(),
            reliable_backlog: BTreeMap::new(),
            best_effort_seq: BTreeMap::new(),
            best_effort_in: BTreeMap::new(),
            reads: BTreeMap::new(),
            liveliness: BTreeMap::new(),
            session_ids: BTreeMap::new(),
            known_type_topics: BTreeMap::new(),
            topics: BTreeMap::new(),
            containers: BTreeMap::new(),
            client_types: BTreeMap::new(),
            scopes: BTreeMap::new(),
            participants_of: BTreeMap::new(),
            extras: BTreeMap::new(),
            writer_activity: BTreeMap::new(),
            plans: BTreeMap::new(),
            profiles,
            dynamic_definitions: BTreeMap::new(),
            predefined,
            participant_scopes: BTreeMap::new(),
            last_timestamp: BTreeMap::new(),
            deleted_clients: BTreeMap::new(),
            read_sequence: BTreeMap::new(),
            session_start: BTreeMap::new(),
            client_mtu: BTreeMap::new(),
            events: Vec::new(),
        }
    }

    /// Registers a trace sink on the underlying [`XrceAgent`] (spec §8.5).
    pub fn set_trace_sink(&mut self, sink: alloc::boxed::Box<dyn TraceSink + Send>) {
        self.agent.set_trace_sink(sink);
    }

    /// Drains accumulated [`SessionEvent`]s (call after every
    /// [`Self::handle_message`]/[`Self::tick`]).
    pub fn drain_events(&mut self) -> Vec<SessionEvent> {
        core::mem::take(&mut self.events)
    }

    /// Number of registered clients.
    #[must_use]
    pub fn client_count(&self) -> usize {
        self.agent.client_count()
    }

    /// Direct access to the [`DdsPort`], e.g. so a daemon can push a
    /// DDS-side sample into a reader's queue via
    /// `XrceAgent::submit_sample` — kept on `SessionEngine` instead
    /// because the pull queue lives on `self.agent`, not `self.dds`.
    pub fn dds_port_mut(&mut self) -> &mut P {
        &mut self.dds
    }

    /// Applies the subset of [`SessionEngineConfig`] a SIGHUP reload may
    /// change without a restart — see `crate::daemon::server`'s module
    /// doc for exactly which keys these are and why the others are not
    /// here. Already-registered clients keep their state; only the
    /// *limits* future operations are checked against change.
    pub fn set_reloadable_limits(
        &mut self,
        max_clients: usize,
        max_objects_per_client: usize,
        session_timeout: Duration,
    ) {
        self.config.max_clients = max_clients;
        self.config.max_objects_per_client = max_objects_per_client;
        self.config.session_timeout = session_timeout;
    }

    /// The DDS side pushed a sample for `reader` — buffers it for
    /// delivery on the next [`Self::tick`] (continuous read) or the next
    /// `READ_DATA` (single-shot pull). Spec §7.3 client-pull model +
    /// §8.4.14 continuous read.
    pub fn submit_sample(
        &mut self,
        client: ClientKeyOrd,
        reader: ObjectId,
        payload: Vec<u8>,
    ) -> Result<(), AgentError> {
        self.agent.submit_sample(ClientKey(client), reader, payload)
    }

    /// Processes one inbound `Message`.
    ///
    /// `resolved_client` is `None` only when the header carries no
    /// `client_key` and this is not yet a known client — the only
    /// submessage that can establish identity in that state is
    /// `CREATE_CLIENT` (its payload carries the `ClientKey` itself);
    /// anything else in that state is answered with `STATUS_ERR_DENIED`
    /// against `OBJECTID_CLIENT` and dropped, per §8.4.5 requiring a
    /// bound session before any other operation.
    pub fn handle_message(
        &mut self,
        resolved_client: Option<ClientKeyOrd>,
        header: &MessageHeader,
        submessages: &[Submessage],
        now: Duration,
    ) -> Vec<Outbound> {
        let mut out = Vec::new();
        let mut retransmits = Vec::new();
        // §7.8.1: "With the exception of the operations create_client and
        // get_info on the XRCE Root, all other operations expect that the
        // ClientKey references an already existing XRCE ProxyClient. If this
        // is not the case, the operation shall fail." The text gives two ways:
        // STATUS_ERR_NOCLIENT, which StatusValue (Annex A) does not have, or
        // dropping the message silently. The agent drops it and counts it
        // (`SessionEvent::NoClient`); the delete_client of a ProxyClient that
        // is not there is answered, §7.8.2.3 gives it its own status.
        if let Some(c) = resolved_client {
            if !self.agent.has_client(ClientKey(c)) && self.drops_for_unknown_client(submessages) {
                self.events.push(SessionEvent::NoClient { client: c });
                self.events.push(SessionEvent::MessageRefused {
                    client: resolved_client,
                    session_id: header.session_id.0,
                    stream_id: header.stream_id.0,
                    reason: "the ClientKey names no ProxyClient (DDS-XRCE 1.0 §7.8.1)".into(),
                });
                return out;
            }
        }
        if let Some(reason) = self.refusal(resolved_client, header, submessages) {
            self.events.push(SessionEvent::MessageRefused {
                client: resolved_client,
                session_id: header.session_id.0,
                stream_id: header.stream_id.0,
                reason,
            });
            return out;
        }
        if let Some(c) = resolved_client {
            self.liveliness.insert(c, now);
        }

        // §8.4.14.2: on a reliable stream, a message is not "received" in
        // the protocol sense (and so not dispatched) until it is in
        // order — `recv_data` buffers it (or recognizes a duplicate/
        // already-delivered retransmission and drops it, silently on
        // purpose: that *is* the correct answer to a duplicate, not a
        // dropped request), `drain_in_order` then releases every message,
        // starting at `expected_seq`, that is now contiguous. A gap
        // still open after that means nothing drains this call; the next
        // HEARTBEAT/ACKNACK round trip is what makes the client resend
        // it. Best-effort/`StreamId::NONE` traffic skips all of this —
        // the spec gives it no ordering or retransmission contract to
        // enforce.
        if header.stream_id.is_reliable() {
            if let Some(client) = resolved_client {
                let rs = self
                    .reliable
                    .entry((client, header.stream_id.0))
                    .or_insert_with(|| {
                        ReliableStreamState::new(header.stream_id, ReliableConfig::default())
                    });
                match rs.recv_data(header.sequence_nr, encode_submessages(submessages)) {
                    Ok(()) => {
                        for (seq, bytes) in rs.drain_in_order() {
                            let decoded = decode_submessages(&bytes);
                            let mut h = *header;
                            h.sequence_nr = seq;
                            self.process_message_submessages(
                                Some(client),
                                &h,
                                &decoded,
                                now,
                                &mut out,
                                &mut retransmits,
                            );
                        }
                    }
                    Err(_) => {
                        // DoS cap: receiver reorder buffer full — spec
                        // gives no reply slot for this either; same
                        // documented OBJECTID_CLIENT extension as a
                        // corrupt FRAGMENT reassembly.
                        out.push(status_reply(
                            client,
                            header,
                            [0, 0],
                            OBJECTID_CLIENT,
                            ResultStatusCode::Resources,
                        ));
                    }
                }
            }
            // `resolved_client` is `None` here only for a reliable-stream
            // CREATE_CLIENT — impossible per §8.3.2.1 (a session-less
            // handshake always travels on `STREAMID_NONE`), so there is
            // nothing meaningful to buffer without a client identity to
            // buffer it under.
        } else {
            // §8.3.2.3: on a stream other than STREAMID_NONE "the sequenceNr
            // imposes an order. Messages within a stream shall not be
            // delivered out of order. In addition, duplicate messages shall
            // be discarded." A best-effort stream has no retransmission, so
            // a message that is not newer than the last one is dropped.
            if header.stream_id.is_best_effort() {
                if let Some(client) = resolved_client {
                    if !self.take_best_effort(client, header) {
                        return out;
                    }
                }
            }
            self.process_message_submessages(
                resolved_client,
                header,
                submessages,
                now,
                &mut out,
                &mut retransmits,
            );
        }

        self.finalize_reliable_sequencing(&mut out);
        out.append(&mut retransmits);
        out
    }

    /// Whether a message to a ClientKey without a ProxyClient is dropped:
    /// every message but a CREATE_CLIENT, a GET_INFO (§7.8.1) and a
    /// `DELETE(OBJECTID_CLIENT)`, which §7.8.2.3 answers.
    fn drops_for_unknown_client(&self, submessages: &[Submessage]) -> bool {
        let deletes_client = |sm: &Submessage| {
            sm.header.submessage_id == SubmessageId::Delete
                && BaseObjectRequest::decode(&sm.body).is_ok_and(|(b, _)| {
                    b.object_id == OBJECTID_CLIENT || b.object_id.kind() == Ok(ObjectKind::Client)
                })
        };
        !submessages.iter().any(|sm| {
            matches!(
                sm.header.submessage_id,
                SubmessageId::CreateClient | SubmessageId::GetInfo
            ) || deletes_client(sm)
        })
    }

    /// Why a message is not processed, if it is not: DDS-XRCE 1.0 §8.3.2.1
    /// gives `SESSION_ID_NONE_*` for "the lack of a session" — the only
    /// submessage that belongs there is the CREATE_CLIENT that makes one —
    /// and §8.2.2 has at most one active session per client, so a message of
    /// a session that has been replaced is not the client's.
    fn refusal(
        &self,
        resolved_client: Option<ClientKeyOrd>,
        header: &MessageHeader,
        submessages: &[Submessage],
    ) -> Option<String> {
        if submessages
            .iter()
            .any(|sm| sm.header.submessage_id == SubmessageId::CreateClient)
        {
            return None;
        }
        // §7.8.2.2: get_info "may be used prior to establishing a Session".
        let only_get_info = submessages
            .iter()
            .all(|sm| sm.header.submessage_id == SubmessageId::GetInfo);
        if header.session_id.is_none() && only_get_info {
            return None;
        }
        if header.session_id.is_none() {
            return Some(format!(
                "the header carries no session (session_id {:#04x}) and the message is not a \
                 CREATE_CLIENT",
                header.session_id.0
            ));
        }
        let client = resolved_client?;
        match self.session_ids.get(&client) {
            Some(active) if *active != header.session_id.0 => Some(format!(
                "session_id {:#04x} is not the client's active session {active:#04x}",
                header.session_id.0
            )),
            _ => None,
        }
    }

    /// Whether a message of a best-effort input stream is newer than the last
    /// one taken (RFC 1982 serial number arithmetic, §8.3.2.3). If it is, it
    /// becomes the newest; if not it is reported and the caller drops it.
    fn take_best_effort(&mut self, client: ClientKeyOrd, header: &MessageHeader) -> bool {
        let key = (client, header.stream_id.0);
        match self.best_effort_in.get(&key).copied() {
            Some(newest) if !newest.wrapping_lt(header.sequence_nr) => {
                self.events.push(SessionEvent::BestEffortMessageDiscarded {
                    client,
                    stream_id: header.stream_id.0,
                    sequence_nr: header.sequence_nr.raw(),
                    newest: newest.raw(),
                });
                false
            }
            _ => {
                self.best_effort_in.insert(key, header.sequence_nr);
                true
            }
        }
    }

    /// The actual per-message pipeline (§8.3.5.14 FRAGMENT reassembly,
    /// then dispatch, then ACKNACK-triggered retransmission) — shared
    /// between best-effort/`StreamId::NONE` traffic (called directly from
    /// [`Self::handle_message`]) and reliable-stream traffic (called once
    /// per message [`Self::handle_message`]'s `drain_in_order` loop
    /// releases, in sequence order).
    fn process_message_submessages(
        &mut self,
        resolved_client: Option<ClientKeyOrd>,
        header: &MessageHeader,
        submessages: &[Submessage],
        now: Duration,
        out: &mut Vec<Outbound>,
        retransmits: &mut Vec<Outbound>,
    ) {
        // §8.3.5.14: FRAGMENT reassembly happens before anything else —
        // a reassembled submessage is processed exactly like one that
        // arrived whole.
        let mut effective: Vec<Submessage> = Vec::with_capacity(submessages.len());
        for sm in submessages {
            if sm.header.submessage_id == SubmessageId::Fragment {
                let Some(client) = resolved_client else {
                    continue; // no session yet — nothing to reassemble into
                };
                let Ok(frag) = FragmentPayload::try_from_submessage(sm) else {
                    continue;
                };
                // A run in progress on this (client, stream) keeps its
                // *first* fragment's sequence number as `base_seq` and
                // resumes at the byte offset accumulated so far; no run
                // in progress means this fragment starts a new one, at
                // this message's own sequence number and offset 0 (see
                // `FragmentRun`'s doc for why that offset is trustworthy).
                let run_key = (client, header.stream_id.0);
                let (assembler_key, offset) = match self.fragment_runs.get(&run_key) {
                    Some(run) => (
                        AssemblerKey {
                            stream_id: header.stream_id.0,
                            base_seq: run.base_seq,
                        },
                        run.next_offset,
                    ),
                    None => (
                        AssemblerKey {
                            stream_id: header.stream_id.0,
                            base_seq: header.sequence_nr.raw(),
                        },
                        0,
                    ),
                };
                let frag_len = frag.data.len() as u32;
                let assembler = self.fragments.entry(client).or_default();
                match assembler.add_fragment(
                    assembler_key,
                    offset,
                    frag.last_fragment,
                    frag.data,
                    now,
                ) {
                    Ok(Some(complete)) => {
                        self.fragment_runs.remove(&run_key);
                        // §8.3.5.14: "The concatenated payloads shall be
                        // interpreted as XRCE submessages" — all of them,
                        // 4-aligned (§8.3.3). Only the first was taken,
                        // the rest dropped without a word.
                        match zerodds_xrce::submessages::decode_submessages(&complete) {
                            // A run that holds no submessage carries nothing
                            // to interpret; it is refused like a corrupt one.
                            Ok(reassembled) if !reassembled.is_empty() => {
                                effective.extend(reassembled);
                            }
                            Ok(_) | Err(_) => out.push(status_reply(
                                client,
                                header,
                                [0, 0],
                                OBJECTID_CLIENT,
                                ResultStatusCode::InvalidData,
                            )),
                        }
                    }
                    Ok(None) => {
                        self.fragment_runs.insert(
                            run_key,
                            FragmentRun {
                                base_seq: assembler_key.base_seq,
                                next_offset: offset.saturating_add(frag_len),
                                last_arrival: now,
                            },
                        );
                    }
                    Err(_) => {
                        self.fragment_runs.remove(&run_key);
                        // DoS cap tripped or a corrupt reassembly — spec
                        // gives FRAGMENT no reply slot of its own; report
                        // it against OBJECTID_CLIENT rather than staying
                        // silent (see module doc).
                        out.push(status_reply(
                            client,
                            header,
                            [0, 0],
                            OBJECTID_CLIENT,
                            ResultStatusCode::Resources,
                        ));
                    }
                }
            } else {
                effective.push(sm.clone());
            }
        }

        for sm in &effective {
            self.handle_submessage(resolved_client, header, sm, now, out);
        }
        // ACKNACK retransmissions already carry a real, previously
        // `submit()`ted sequence number (the exact one being repaired) —
        // `finalize_reliable_sequencing` must not reassign it a fresh
        // one, so the caller collects them separately (`retransmits`) and
        // merges them in only after it has run over everything freshly
        // built this call.
        for sm in &effective {
            if sm.header.submessage_id == SubmessageId::AckNack {
                self.handle_acknack(resolved_client, header, sm, retransmits);
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn handle_submessage(
        &mut self,
        resolved_client: Option<ClientKeyOrd>,
        header: &MessageHeader,
        sm: &Submessage,
        now: Duration,
        out: &mut Vec<Outbound>,
    ) {
        match sm.header.submessage_id {
            SubmessageId::CreateClient => self.handle_create_client(header, sm, now, out),
            SubmessageId::Heartbeat => self.handle_heartbeat(resolved_client, header, sm, out),
            // handled in a dedicated pass in `handle_message`, after
            // `finalize_reliable_sequencing` — see the comment there.
            SubmessageId::AckNack => {}
            SubmessageId::Reset => self.handle_reset(resolved_client, out),
            SubmessageId::Timestamp => self.handle_timestamp(resolved_client, header, sm, out),
            SubmessageId::TimestampReply => self.handle_timestamp_reply(resolved_client, sm),
            SubmessageId::Fragment => {} // consumed in handle_message
            // §7.8.2.2: "get_info [...] may be used prior to establishing a
            // Session with the XRCE Agent".
            SubmessageId::GetInfo if resolved_client.is_none() || header.session_id.is_none() => {
                self.handle_get_info(None, header, sm, out);
            }
            _ => {
                let Some(client) = resolved_client else {
                    // Not yet a session — spec requires CREATE_CLIENT
                    // first (§8.4.5). No BaseObjectRequest to echo, so
                    // there is nothing to correlate a STATUS with; the
                    // datagram is simply not actionable. This is the one
                    // path this engine does not answer, because doing so
                    // would mean inventing a request_id the client never
                    // sent — logged, not silently dropped, via the
                    // caller's transport-level "unresolved peer" counter.
                    return;
                };
                match sm.header.submessage_id {
                    SubmessageId::Create => self.handle_create(client, header, sm, out),
                    SubmessageId::Delete => self.handle_delete(client, header, sm, out),
                    SubmessageId::GetInfo => self.handle_get_info(Some(client), header, sm, out),
                    SubmessageId::WriteData => self.handle_write_data(client, header, sm, out),
                    SubmessageId::ReadData => self.handle_read_data(client, header, sm, now, out),
                    _ => {}
                }
            }
        }
    }

    fn handle_create_client(
        &mut self,
        header: &MessageHeader,
        sm: &Submessage,
        now: Duration,
        out: &mut Vec<Outbound>,
    ) {
        let rep = match CreateClientPayload::try_from_submessage(sm) {
            Ok(payload) => payload.representation,
            Err(_) => {
                // §7.8.2.1: "Check the xrce_cookie to ensure it matches
                // XRCE_COOKIE. If it does not match, ... STATUS_ERR_INVALID_DATA."
                // §8.3.5.1: "The XRCE Agent shall send a STATUS_AGENT
                // message in response" to a CREATE_CLIENT, so the failure
                // is a STATUS_AGENT, not a STATUS. The ClientKey is the one
                // of the header when it has one: the payload is unreadable.
                out.push(agent_status_reply(
                    header.client_key.map_or([0; CLIENT_KEY_LEN], |k| k.0),
                    header,
                    header.session_id.0,
                    ResultStatusCode::InvalidData,
                    self.config.agent_vendor_id,
                    self.config.status_agent_with_result_status,
                ));
                return;
            }
        };
        let client = rep.client_key.0;
        // §8.3.2.1: 0x00 and 0x80 "indicate the lack of a session"; a
        // client cannot ask for a session that is none.
        if SessionId(rep.session_id).is_none() {
            out.push(agent_status_reply(
                client,
                header,
                rep.session_id,
                ResultStatusCode::InvalidData,
                self.config.agent_vendor_id,
                self.config.status_agent_with_result_status,
            ));
            return;
        }
        // §7.8.2.1: "Check that the major version (xrce_version[0])
        // matches XRCE_VERSION_MAJOR. If it does not match, ...
        // STATUS_ERR_INCOMPATIBLE." (Checked before the existing-ProxyClient
        // and resource checks below — an incompatible client must never
        // reset or count against an existing session's state.)
        if rep.version[0] != crate::daemon::handshake::XRCE_VERSION[0] {
            out.push(agent_status_reply(
                client,
                header,
                rep.session_id,
                ResultStatusCode::Incompatible,
                self.config.agent_vendor_id,
                self.config.status_agent_with_result_status,
            ));
            return;
        }
        // §7.8.2.1: "Check that the Client identified by the client_key is
        // authorized to connect to the XRCE Agent. If this check fails the
        // operation shall fail and set the returnValue StatusValue to
        // STATUS_ERR_DENIED." (§7.8.1: the ClientKey is configured in the
        // Agent.)
        if !self.config.access.client_authorized(client) {
            self.deny_create_client(
                client,
                header,
                rep.session_id,
                "the ClientKey is not configured",
                out,
            );
            return;
        }
        // §7.8.2.1: "Check the Client properties, if present. These may
        // contain vendor-specific information that may prevent the Agent
        // from accepting the connection from the Client."
        if !self
            .config
            .access
            .properties_accepted(rep.properties.as_deref())
        {
            self.deny_create_client(
                client,
                header,
                rep.session_id,
                "the client properties do not meet what the agent requires",
                out,
            );
            return;
        }
        let already_known = self.agent.has_client(rep.client_key);
        if !already_known && self.agent.client_count() >= self.config.max_clients {
            out.push(agent_status_reply(
                client,
                header,
                rep.session_id,
                ResultStatusCode::Resources,
                self.config.agent_vendor_id,
                self.config.status_agent_with_result_status,
            ));
            return;
        }
        // §7.8.2.1: "If a ProxyClient exists and has the same session_id,
        // ... shall not perform any action and ... STATUS_OK. If a
        // ProxyClient exists and has a different session_id then the
        // operation shall delete the existing XRCE ProxyClient object and
        // subsequently take the same actions as if there had not been a
        // ProxyClient ... Any messages that were cached pending
        // acknowledgments shall be discarded." A same-session_id
        // reconnect falls through to the idempotent register_client/
        // insert calls below, which leave the *object store* unchanged.
        let same_session = already_known && self.session_ids.get(&client) == Some(&rep.session_id);
        if already_known && !same_session {
            self.agent.unregister_client(rep.client_key);
            self.dds.unregister_client(client);
            self.reads.retain(|(c, _), _| *c != client);
            self.forget_client_objects(client);
            self.events
                .push(SessionEvent::ClientUnregistered { client });
        }
        // §7.8.2.1: a CREATE_CLIENT for the session that exists "shall not
        // perform any action"; the streams go on (a retransmitted
        // CREATE_CLIENT must not drop what is in flight). The eProsima
        // examples of `tools/wire-capture/xrce-sessions/client-to-agent.hex`
        // start several processes with the same ClientKey and session_id,
        // each with its sequence numbers at 0: `restart_on_same_session`
        // resets the streams for them (`replay_foreign_session.rs`).
        if !same_session || self.config.restart_on_same_session {
            self.reliable.retain(|(c, _), _| *c != client);
            self.fragments.remove(&client);
            self.fragment_runs.retain(|(c, _), _| *c != client);
            // The client's streams start over, its best-effort input
            // included: a sequence carried on from the previous process
            // would look older than the new receiver's starting point and
            // be discarded.
            self.reliable_backlog.retain(|(c, _), _| *c != client);
            self.best_effort_seq.retain(|(c, _), _| *c != client);
            self.best_effort_in.retain(|(c, _), _| *c != client);
        }
        self.agent.register_client(rep.client_key);
        self.deleted_clients.remove(&client);
        // §7.7.2: the session timestamp is the `client_timestamp` of the
        // CLIENT_Representation (Table 1: "the session timestamp corresponds
        // to the client_timestamp attribute"); a client that sends none (the
        // IDL of Annex A has no such member) gets the time the agent created
        // the session. A CREATE_CLIENT for the session that exists changes
        // nothing.
        if self.session_ids.get(&client) != Some(&rep.session_id) {
            let start = rep
                .timestamp
                .filter(|t| !(t.seconds == 0 && t.nanoseconds == 0))
                .unwrap_or_else(wall_clock_now);
            self.session_start.insert(client, start);
        }
        self.session_ids.insert(client, rep.session_id);
        // §7.8.2's non-normative MTU extension (see handshake.rs's module
        // doc) — remembered here so `pump_read` knows when an outbound
        // DATA must be split into FRAGMENT instead of sent whole.
        self.client_mtu.insert(
            client,
            rep.requested_mtu
                .unwrap_or(crate::daemon::handshake::DEFAULT_MTU),
        );
        self.liveliness.insert(client, now);
        self.events.push(SessionEvent::ClientRegistered {
            client,
            session_id: rep.session_id,
        });
        // §8.4.2, §8.4.3: the CREATE_CLIENT "triggers the creation or reuse of
        // the corresponding XRCE objects".
        self.make_client_applications(client);
        out.push(agent_status_reply(
            client,
            header,
            rep.session_id,
            ResultStatusCode::Ok,
            self.config.agent_vendor_id,
            self.config.status_agent_with_result_status,
        ));
    }

    /// Answers a `CREATE_CLIENT` the AccessController refused with
    /// `STATUS_ERR_DENIED` (§7.8.2.1); no session is created.
    fn deny_create_client(
        &mut self,
        client: ClientKeyOrd,
        header: &MessageHeader,
        session_id: u8,
        reason: &str,
        out: &mut Vec<Outbound>,
    ) {
        self.events.push(SessionEvent::OperationDenied {
            client,
            object: OBJECTID_CLIENT,
            what: format!("create_client: {reason}"),
        });
        out.push(agent_status_reply(
            client,
            header,
            session_id,
            ResultStatusCode::Denied,
            self.config.agent_vendor_id,
            self.config.status_agent_with_result_status,
        ));
    }

    /// The AccessController's (§7.4) decision on `op` for `client` on
    /// `subject`. A refusal is answered with `STATUS_ERR_DENIED` against
    /// `request_id` and `object` and reported as an event.
    #[allow(clippy::too_many_arguments)]
    fn permit(
        &mut self,
        client: ClientKeyOrd,
        header: &MessageHeader,
        request_id: [u8; 2],
        object: ObjectId,
        op: Operation,
        subject: &Subject,
        out: &mut Vec<Outbound>,
    ) -> bool {
        if self.permits(client, object, op, subject) {
            return true;
        }
        out.push(status_reply(
            client,
            header,
            request_id,
            object,
            ResultStatusCode::Denied,
        ));
        false
    }

    /// The AccessController's (§7.4) decision on `op` for `client` on
    /// `subject`; a refusal is reported as an event.
    fn permits(
        &mut self,
        client: ClientKeyOrd,
        object: ObjectId,
        op: Operation,
        subject: &Subject,
    ) -> bool {
        if self.config.access.permits(client, op, subject) {
            return true;
        }
        self.events.push(SessionEvent::OperationDenied {
            client,
            object,
            what: format!("{}: no access rule allows it", op.name()),
        });
        false
    }

    /// What an existing object stands for in the AccessController's rules.
    fn subject_of(&self, client: ClientKeyOrd, oid: ObjectId) -> Subject {
        let scope = self.scopes.get(&(client, oid)).cloned().unwrap_or_default();
        Subject {
            kind: oid.kind().unwrap_or(ObjectKind::Invalid),
            domain: scope.domain,
            topic: scope.topic,
        }
    }

    /// The domain and topic a CREATE's plan will give its object, taken
    /// from the objects it hangs under.
    fn scope_of_plan(&self, client: ClientKeyOrd, plan: &DdsCreate) -> ObjectScope {
        let parent_domain = |p: Option<ObjectId>| {
            p.and_then(|p| self.scopes.get(&(client, p)))
                .and_then(|s| s.domain)
        };
        match plan {
            DdsCreate::Participant { domain_id, .. } => ObjectScope {
                domain: u16::try_from(*domain_id).ok(),
                topic: None,
            },
            DdsCreate::Topic {
                participant,
                description,
            } => ObjectScope {
                domain: parent_domain(Some(*participant)),
                topic: Some(description.topic_name.clone()),
            },
            DdsCreate::Container { participant, .. } => ObjectScope {
                domain: parent_domain(Some(*participant)),
                topic: None,
            },
            DdsCreate::Endpoint { topic, .. } => ObjectScope {
                domain: parent_domain(Some(*topic)),
                topic: self
                    .topics
                    .get(&(client, *topic))
                    .map(|t| t.topic_name.clone()),
            },
            DdsCreate::Nothing | DdsCreate::Types(_) | DdsCreate::Application { .. } => {
                ObjectScope::default()
            }
        }
    }

    fn handle_create(
        &mut self,
        client: ClientKeyOrd,
        header: &MessageHeader,
        sm: &Submessage,
        out: &mut Vec<Outbound>,
    ) {
        let Ok(payload) = CreatePayload::try_from_submessage(sm) else {
            return;
        };
        let Ok((request, n)) = BaseObjectRequest::decode(&payload.representation) else {
            // §8.3.5.2: "The XRCE Agent shall send a STATUS submessage in
            // response". The request is too short to name an object; the
            // request_id is answered when its two octets are there.
            let request_id = match payload.representation.as_slice() {
                [a, b, ..] => [*a, *b],
                _ => [0, 0],
            };
            self.events.push(SessionEvent::CreateRefused {
                client,
                object: OBJECTID_INVALID,
                reason: format!(
                    "CREATE of {} bytes holds no BaseObjectRequest",
                    payload.representation.len()
                ),
            });
            out.push(status_reply(
                client,
                header,
                request_id,
                OBJECTID_INVALID,
                ResultStatusCode::InvalidData,
            ));
            return;
        };
        let e = sm.header.body_endianness();
        // §7.7.3: `ObjectVariant` is a union discriminated by `ObjectKind`
        // (an outer byte), wrapping the `RepresentationFormat`-discriminated
        // payload `ObjectVariant::decode` alone models -- byte-oracled
        // against a real eProsima CREATE(OBJK_SUBSCRIBER) (see
        // crates/xrce/src/object_repr.rs's module doc): the plain
        // single-level decode this used to call rejected every real
        // Publisher/Subscriber/DataWriter/DataReader CREATE outright.
        let Ok((kind, mut variant, extra, _)) =
            ObjectVariant::decode_for_kind(&payload.representation[n..], e)
        else {
            out.push(status_reply(
                client,
                header,
                request.request_id,
                request.object_id,
                ResultStatusCode::InvalidData,
            ));
            return;
        };
        // §7.7.3.1.1: the object_reference "shall be a string formatted as
        // defined by the XSD simpleType elementNameReference" of DDS-XML
        // (`whiteSpace collapse`, `((::)?[a-zA-Z0-9_.])+`). A string of
        // another shape is no reference: STATUS_ERR_INVALID_DATA, where a
        // well-formed one the agent does not hold is UNKNOWN_REFERENCE.
        if let ObjectVariant::ByReference(name) = &variant {
            match zerodds_xrce::object_repr::element_name_reference(name) {
                Some(collapsed) => variant = ObjectVariant::ByReference(collapsed),
                None => {
                    self.events.push(SessionEvent::CreateRefused {
                        client,
                        object: request.object_id,
                        reason: format!(
                            "object_reference {name:?} is no elementNameReference (DDS-XML \
                             XSD, §7.7.3.1.1)"
                        ),
                    });
                    out.push(status_reply(
                        client,
                        header,
                        request.request_id,
                        request.object_id,
                        ResultStatusCode::InvalidData,
                    ));
                    return;
                }
            }
        }
        // §7.7.3.1, Annex A: the formats the kind has.
        if !self.config.allow_any_representation_format && !format_is_allowed(kind, &variant) {
            self.events.push(SessionEvent::CreateRefused {
                client,
                object: request.object_id,
                reason: format!(
                    "{kind:?} has no representation in the format {variant:?}; DDS-XRCE 1.0 \
                     §7.7.3.1 (`representation_formats: any` admits it)"
                ),
            });
            out.push(status_reply(
                client,
                header,
                request.request_id,
                request.object_id,
                ResultStatusCode::InvalidData,
            ));
            return;
        }
        // §7.8.3.1: "The combination of the objectid_prefix and the
        // ObjectKind contained in the object_representation discriminator
        // shall be used to construct the 'input' ObjectId"; the object_id of
        // the returnValue "shall be derived from the object_prefix". The
        // kind in the low bits of the request's object_id is not read.
        let Ok(input_id) = ObjectId::new(request.object_id.raw_id_12(), kind) else {
            out.push(status_reply(
                client,
                header,
                request.request_id,
                request.object_id,
                ResultStatusCode::InvalidData,
            ));
            return;
        };
        let base = BaseObjectRequest {
            request_id: request.request_id,
            object_id: input_id,
        };
        let mode = CreationMode {
            reuse: payload.reuse,
            replace: payload.replace,
        };
        let big_endian = matches!(e, Endianness::Big);
        // The container a CREATE names may be an object of the agent's
        // definitions the client has not made (§7.3).
        if let Some(parent) = extra.parent {
            if let Err(code) = self.ensure_predefined(client, parent) {
                out.push(status_reply(
                    client,
                    header,
                    base.request_id,
                    base.object_id,
                    code,
                ));
                return;
            }
        }
        let code = self.create_core(client, base.object_id, variant, extra, big_endian, mode);
        out.push(status_reply(
            client,
            header,
            base.request_id,
            base.object_id,
            code,
        ));
    }

    /// Creates the object `oid` of `variant` for `client`: the checks of
    /// §7.8.3.1, the AccessController (§7.4), the store, the DDS side
    /// (Table 6), the objects inside a DomainParticipant or Application
    /// ("the creation of the XRCE object shall also create the nested
    /// objects and the failure to create any nested entity shall be
    /// considered a failure to create the contained entity as well"), and the
    /// definitions an XML library representation adds to the agent
    /// (§7.7.3.1.1, §9.2). Returns the `ResultStatus` code of the CREATE; a
    /// code that is no success leaves nothing created.
    fn create_core(
        &mut self,
        client: ClientKeyOrd,
        oid: ObjectId,
        variant: ObjectVariant,
        extra: ObjectVariantExtra,
        big_endian: bool,
        mode: CreationMode,
    ) -> ResultStatusCode {
        let Ok(kind) = oid.kind() else {
            return ResultStatusCode::InvalidData;
        };
        if let Ok(n) = self.agent.object_count(ClientKey(client)) {
            if n >= self.config.max_objects_per_client {
                return ResultStatusCode::Resources;
            }
        }
        // What the DDS side needs is read before anything is stored: a
        // representation no DDS entity can be made from is refused with
        // its STATUS and leaves the object store as it was.
        let plan = match self.plan_dds_create(client, kind, &variant, extra, big_endian) {
            Ok(plan) => plan,
            Err(err) => return self.refuse_create(client, oid, &err),
        };
        // QoS the agent cannot apply is not accepted and ignored: the object
        // would not be what the client asked for.
        if !self.config.ignore_unsupported_qos {
            let elements = plan_not_applied(&plan);
            if !elements.is_empty() {
                return self.refuse_create(
                    client,
                    oid,
                    &ReprError::Incompatible(format!(
                        "QoS the agent cannot apply: {} (unsupported_qos: ignore creates the \
                         object without it)",
                        elements.join(", ")
                    )),
                );
            }
        }
        let registration = match self.definition_to_register(kind, &variant) {
            Ok(xml) => xml,
            Err(err) => return self.refuse_create(client, oid, &err),
        };
        // §7.4: the AccessController decides whether this client may create
        // this object (the domain it joins, the topic it names). Replacing
        // an existing object deletes it first (§7.8.3.1, Table 5), which
        // needs the permission to delete it.
        let scope = self.scope_of_plan(client, &plan);
        let subject = Subject {
            kind,
            domain: scope.domain,
            topic: scope.topic.clone(),
        };
        if !self.permits(client, oid, Operation::Create, &subject) {
            return ResultStatusCode::Denied;
        }
        let existing_object = self
            .agent
            .get_object(ClientKey(client), oid)
            .map(|o| (o.kind, o.variant.clone()));
        if mode.replace && existing_object.is_some() {
            // Replacing deletes the object (Table 5); with `reuse` as well it
            // is the update of §7.8.3.2 / §8.4.13.
            let operation = if mode.reuse {
                Operation::Update
            } else {
                Operation::Delete
            };
            let existing = self.subject_of(client, oid);
            if !self.permits(client, oid, operation, &existing) {
                return ResultStatusCode::Denied;
            }
        }
        // §7.8.3.2: "If the object does not already exist
        // STATUS_ERR_UNKNOWN_REFERENCE shall be returned." Table 5 creates it.
        // An update is a CREATE with `reuse` and `replace` (§8.4.13, Table
        // 14); `update_of_missing_object: refuse` reads it as the update.
        if mode.reuse
            && mode.replace
            && existing_object.is_none()
            && self.config.refuse_update_of_missing_object
        {
            return ResultStatusCode::UnknownReference;
        }
        // What the object is now, kept in case the replacement cannot be
        // made (§7.8.3.2).
        let previous = if mode.replace {
            self.agent
                .get_object(ClientKey(client), oid)
                .map(|existing| PreviousObject {
                    variant: existing.variant.clone(),
                    extra: self.extras.get(&(client, oid)).copied(),
                    scope: self.scopes.get(&(client, oid)).cloned(),
                    plan: self.plans.get(&(client, oid)).cloned(),
                })
        } else {
            None
        };
        // Table 5: "Check if object_representation matches the existing
        // Object". The representation includes the container it is made
        // under (participant_id, publisher_id, subscriber_id) and the
        // domain_id, which the store does not hold. With `reuse` and
        // `replace`, one that does not match replaces the object.
        let same_extra = self.extras.get(&(client, oid)).copied().unwrap_or_default() == extra;
        let store_mode = match &existing_object {
            Some((existing_kind, existing_variant))
                if mode.reuse
                    && mode.replace
                    && !(*existing_kind == kind && *existing_variant == variant && same_extra) =>
            {
                CreationMode::REPLACE
            }
            _ => mode,
        };
        let Ok(outcome) =
            self.agent
                .create_object(ClientKey(client), oid, variant.clone(), store_mode)
        else {
            return ResultStatusCode::UnknownReference;
        };
        let code = match outcome {
            CreateOutcome::Created | CreateOutcome::Replaced => ResultStatusCode::Ok,
            CreateOutcome::Reused { equal: true } if same_extra => ResultStatusCode::OkMatched,
            CreateOutcome::Reused { .. } => ResultStatusCode::Mismatch,
            CreateOutcome::Conflict => ResultStatusCode::AlreadyExists,
        };
        // §7.8.3.2: a reused object with an equal representation is the
        // one that already exists, DDS side included; a replaced one
        // loses its old DDS entity before the new one is made.
        if !code.is_success() || matches!(outcome, CreateOutcome::Reused { .. }) {
            return code;
        }
        if matches!(outcome, CreateOutcome::Replaced) {
            self.dds.delete((client, oid));
        }

        let kept_plan = plan.clone();
        let nested_sources = nested_sources_of(&plan);
        let embedded = embedded_of(&plan);
        // §7.8.3.1: the DDS entities are made disabled and enabled "only
        // after it has been determined that the creation has succeeded";
        // that is here, with the object stored and nothing left to fail.
        let applied = self
            .apply_dds_create(client, oid, plan)
            .and_then(|()| self.dds.enable((client, oid)));
        if let Err(e) = applied {
            // The client is told the object does not exist, so the store
            // must not keep it either, and neither the DDS side.
            self.dds.delete((client, oid));
            let _ = self.agent.delete_object(ClientKey(client), oid);
            self.forget_object(client, oid);
            self.events.push(SessionEvent::DdsPortFailed {
                client,
                object: oid,
                error: dds_port_error_message(&e),
            });
            // §7.8.3.2: "If an update is unsuccessful the referenced
            // object shall return to its previous configuration."
            if let Some(previous) = previous {
                self.restore_previous(client, oid, previous);
            }
            return dds_port_error_status(&e);
        }
        self.plans.insert((client, oid), kept_plan);
        self.extras.insert((client, oid), extra);
        self.scopes.remove(&(client, oid));
        if scope != ObjectScope::default() {
            self.scopes.insert((client, oid), scope);
        }
        // §9.2: an XML library the object defines is known to the agent for
        // as long as the object exists; other clients refer to it.
        if let Some(xml) = registration {
            self.dynamic_definitions.insert((client, oid), xml);
            self.rebuild_profiles();
        }
        if nested_sources.is_empty() && embedded.is_empty() {
            return code;
        }
        let mut nested = self.create_nested(client, oid, kind, &nested_sources);
        if nested == ResultStatusCode::Ok {
            nested = self.create_embedded(client, oid, kind, &embedded);
        }
        if nested != ResultStatusCode::Ok {
            // A nested entity that cannot be made is a failure of the whole:
            // what was made of it goes, and the previous object comes back.
            self.remove_with_nested(client, oid);
            if let Some(previous) = previous {
                self.restore_previous(client, oid, previous);
            }
            return nested;
        }
        code
    }

    /// Answers a CREATE whose representation cannot be turned into an
    /// object: reports it and gives the code of the error.
    fn refuse_create(
        &mut self,
        client: ClientKeyOrd,
        oid: ObjectId,
        err: &ReprError,
    ) -> ResultStatusCode {
        self.events.push(SessionEvent::CreateRefused {
            client,
            object: oid,
            reason: err.message().to_string(),
        });
        match err {
            ReprError::Malformed(_) => ResultStatusCode::InvalidData,
            ReprError::Unresolved(_) => ResultStatusCode::UnknownReference,
            ReprError::Incompatible(_) => ResultStatusCode::Incompatible,
        }
    }

    /// The DataWriters of a Publisher or DataReaders of a Subscriber its XML
    /// holds (§7.7.3.8.1, §7.7.3.9.1), made under the ObjectIds of their names
    /// (§9.3). `Ok` when all were made.
    fn create_embedded(
        &mut self,
        client: ClientKeyOrd,
        container: ObjectId,
        container_kind: ObjectKind,
        elements: &[zerodds_xml::XmlElement],
    ) -> ResultStatusCode {
        let kind = if container_kind == ObjectKind::Publisher {
            ObjectKind::DataWriter
        } else {
            ObjectKind::DataReader
        };
        let mut made: Vec<ObjectId> = Vec::new();
        for element in elements {
            let Some(name) = element.attribute("name") else {
                self.events.push(SessionEvent::CreateRefused {
                    client,
                    object: container,
                    reason: format!("a <{}> inside the CREATE has no name", element.name),
                });
                self.remove_all(client, &made);
                return ResultStatusCode::InvalidData;
            };
            let oid = ObjectId::from_reference(name, kind);
            let code = self.create_core(
                client,
                oid,
                ObjectVariant::ByXmlString(crate::daemon::definitions::to_xml(element)),
                ObjectVariantExtra {
                    domain_id: None,
                    parent: Some(container),
                },
                false,
                CreationMode::STRICT,
            );
            if !code.is_success() {
                self.remove_all(client, &made);
                return code;
            }
            made.push(oid);
        }
        ResultStatusCode::Ok
    }

    /// The objects inside a DomainParticipant, or the DomainParticipants of
    /// an Application (§7.8.3.1, Table 6), made under the ObjectIds of their
    /// names (§9.3). `Ok` when all were made; the code of the first that was
    /// not otherwise, after the ones made before it were removed again.
    fn create_nested(
        &mut self,
        client: ClientKeyOrd,
        owner: ObjectId,
        owner_kind: ObjectKind,
        sources: &[ParticipantSource],
    ) -> ResultStatusCode {
        if owner_kind == ObjectKind::Application {
            // An Application's participants are objects of their own, with
            // the ObjectId of their reference (Table 16); each is made from
            // its definition like a CREATE by reference, nested objects
            // included.
            let mut made: Vec<ObjectId> = Vec::new();
            for source in sources {
                let oid = ObjectId::from_reference(&source.def.reference, ObjectKind::Participant);
                if self.agent.get_object(ClientKey(client), oid).is_some() {
                    // Reused: made by an earlier instantiation.
                    continue;
                }
                let code = self.create_core(
                    client,
                    oid,
                    ObjectVariant::ByReference(source.def.reference.clone()),
                    ObjectVariantExtra {
                        domain_id: source.domain_id().ok().and_then(|d| i16::try_from(d).ok()),
                        parent: None,
                    },
                    false,
                    CreationMode::STRICT,
                );
                if !code.is_success() {
                    for done in &made {
                        self.remove_with_nested(client, *done);
                    }
                    return code;
                }
                made.push(oid);
            }
            return ResultStatusCode::Ok;
        }
        let mut made: Vec<ObjectId> = Vec::new();
        for source in sources {
            let plan = match source
                .definitions
                .plan_participant(&source.def, owner, &|t| {
                    self.find_type_definition(client, t)
                }) {
                Ok(plan) => plan,
                Err(message) => {
                    self.events.push(SessionEvent::CreateRefused {
                        client,
                        object: owner,
                        reason: message,
                    });
                    return ResultStatusCode::InvalidData;
                }
            };
            for object in plan.nested {
                let code = self.create_core(
                    client,
                    object.oid,
                    object.variant,
                    object.extra,
                    false,
                    CreationMode::STRICT,
                );
                if !code.is_success() {
                    self.events.push(SessionEvent::CreateRefused {
                        client,
                        object: object.oid,
                        reason: format!(
                            "`{}` inside `{}` was not made: {code:?}",
                            object.name, source.def.reference
                        ),
                    });
                    self.remove_all(client, &made);
                    return code;
                }
                made.push(object.oid);
            }
        }
        ResultStatusCode::Ok
    }

    /// Removes the objects `made` of `client` (and their DDS side), newest
    /// first.
    fn remove_all(&mut self, client: ClientKeyOrd, made: &[ObjectId]) {
        for oid in made.iter().rev() {
            self.dds.delete((client, *oid));
            let _ = self.agent.delete_object(ClientKey(client), *oid);
            self.forget_object(client, *oid);
        }
    }

    /// Removes `oid` and the objects inside it: those created under it
    /// (their `participant_id`, `publisher_id` or `subscriber_id` is `oid`),
    /// and those under them.
    fn remove_with_nested(&mut self, client: ClientKeyOrd, oid: ObjectId) {
        let mut found: Vec<ObjectId> = vec![oid];
        let mut at = 0;
        while at < found.len() {
            let parent = found[at];
            at += 1;
            for ((c, child), extra) in &self.extras {
                if *c == client && extra.parent == Some(parent) && !found.contains(child) {
                    found.push(*child);
                }
            }
        }
        // Newest first: children go before their parents.
        self.remove_all(client, &found);
    }

    /// The XML of a library representation that defines something the agent
    /// holds from then on (§7.7.3.1.1: a QosProfile, Type, Application or
    /// DomainParticipant "known to the Agent"): a `<qos_library>`, `<types>`,
    /// `<application_library>` or `<domain_participant_library>`. Checked
    /// against the definitions the agent holds before anything is created.
    fn definition_to_register(
        &self,
        kind: ObjectKind,
        variant: &ObjectVariant,
    ) -> Result<Option<String>, ReprError> {
        let ObjectVariant::ByXmlString(xml) = variant else {
            return Ok(None);
        };
        let wanted = match kind {
            ObjectKind::QosProfile => "qos_library",
            ObjectKind::Type => "types",
            ObjectKind::Application => "application_library",
            ObjectKind::Participant => "domain_participant_library",
            _ => return Ok(None),
        };
        let trimmed = xml.trim_matches(|c: char| c.is_whitespace() || c == '\0');
        let Ok(doc) = zerodds_xml::parse_xml_tree(trimmed) else {
            return Ok(None); // the plan has already read it
        };
        if doc.root.name != wanted {
            return Ok(None);
        }
        // §7.7.3.2.2: the <qos_library> "shall contain a single
        // <qos_profile> child element".
        if kind == ObjectKind::QosProfile
            && (doc.root.children.len() != 1 || doc.root.children[0].name != "qos_profile")
        {
            return Err(ReprError::Malformed(
                "a QosProfile is a <qos_library> with a single <qos_profile> (§7.7.3.2.2)".into(),
            ));
        }
        let mut checked = self.profiles.clone();
        checked.refer_to(&self.config.known_types);
        checked
            .add_profiles(trimmed)
            .map_err(ReprError::Malformed)?;
        Ok(Some(trimmed.to_string()))
    }

    /// The agent's definitions: the files, then the libraries the objects of
    /// the clients define (§9.2), in the order they were made.
    fn rebuild_profiles(&mut self) {
        let mut profiles = self.config.profiles.clone();
        profiles.refer_to(&self.config.known_types);
        for xml in self.dynamic_definitions.values() {
            if let Err(e) = profiles.add_profiles(xml) {
                self.events.push(SessionEvent::CreateRefused {
                    client: [0; CLIENT_KEY_LEN],
                    object: OBJECTID_INVALID,
                    reason: format!(
                        "a definition of a client is not in the agent's definitions: {e}"
                    ),
                });
            }
        }
        self.predefined = predefined_index(&profiles);
        self.profiles = profiles;
    }

    /// Makes the object `oid` for `client` if the definitions of the agent
    /// define it and the client has not made it (DDS-XRCE 1.0 §7.3: "a Client
    /// that has access to this resource can write data using this resource
    /// simply by referring to the existing 'MySquareWriter'. The Client does
    /// not have to create a resource."). Its DomainParticipant is made with
    /// everything inside it, as a CREATE by reference does (§8.4.2, §8.4.3).
    /// An `ObjectId` no definition has is left alone: the operation then
    /// fails on the unknown object, as before. The code of a CREATE that fails
    /// (`STATUS_ERR_DENIED` for a client without the permission to create) is
    /// the error.
    fn ensure_predefined(
        &mut self,
        client: ClientKeyOrd,
        oid: ObjectId,
    ) -> Result<(), ResultStatusCode> {
        if self.agent.get_object(ClientKey(client), oid).is_some() {
            return Ok(());
        }
        let Some(found) = self.predefined.get(&oid).cloned() else {
            return Ok(());
        };
        // Everything inside a DomainParticipant is made with it.
        let (target, kind) = match &found.participant {
            Some(participant) => (participant.clone(), ObjectKind::Participant),
            None => (found.reference.clone(), found.kind),
        };
        let target_oid = ObjectId::from_reference(&target, kind);
        if self
            .agent
            .get_object(ClientKey(client), target_oid)
            .is_some()
        {
            return Ok(());
        }
        let code = self.create_core(
            client,
            target_oid,
            ObjectVariant::ByReference(target),
            ObjectVariantExtra::default(),
            false,
            CreationMode::STRICT,
        );
        if code.is_success() { Ok(()) } else { Err(code) }
    }

    /// Makes the objects the agent is configured to make for `client` when
    /// its `CREATE_CLIENT` arrives (`client_applications:`); one that already
    /// exists is reused.
    fn make_client_applications(&mut self, client: ClientKeyOrd) {
        let wanted: Vec<String> = self
            .config
            .client_applications
            .iter()
            .filter(|(key, _)| *key == client)
            .map(|(_, reference)| reference.clone())
            .collect();
        for reference in wanted {
            let kind = if self
                .profiles
                .definitions()
                .application(&reference)
                .is_some()
            {
                ObjectKind::Application
            } else {
                ObjectKind::Participant
            };
            let oid = ObjectId::from_reference(&reference, kind);
            if self.agent.get_object(ClientKey(client), oid).is_some() {
                continue;
            }
            let code = self.create_core(
                client,
                oid,
                ObjectVariant::ByReference(reference.clone()),
                ObjectVariantExtra::default(),
                false,
                CreationMode::STRICT,
            );
            if !code.is_success() {
                self.events.push(SessionEvent::CreateRefused {
                    client,
                    object: oid,
                    reason: format!("client_applications `{reference}`: {code:?}"),
                });
            }
        }
    }

    /// The type definition a `type_ref` names, among the types of the
    /// client, of the files and of the clients' libraries: that exact name,
    /// else the one type whose name ends in `::type_ref`.
    fn find_type_definition(&self, client: ClientKeyOrd, type_ref: &str) -> Option<String> {
        let names: Vec<&str> = self
            .client_types
            .iter()
            .filter(|((c, _), _)| *c == client)
            .flat_map(|(_, defs)| defs.iter().map(|(n, _)| n.as_str()))
            .chain(self.profiles.types().keys().map(String::as_str))
            .chain(self.config.known_types.keys().map(String::as_str))
            .collect();
        if names.contains(&type_ref) {
            return Some(type_ref.to_string());
        }
        let suffix = format!("::{type_ref}");
        let mut hits: Vec<&str> = names.into_iter().filter(|n| n.ends_with(&suffix)).collect();
        hits.sort_unstable();
        hits.dedup();
        match hits.as_slice() {
            [only] => Some((*only).to_string()),
            _ => None,
        }
    }

    /// Makes the object `oid` what it was before a replace that failed: the
    /// store entry, the session's records and the DDS side from the plan it
    /// was created with. A failure to do so is reported as
    /// [`SessionEvent::DdsPortFailed`]; the object is then gone.
    fn restore_previous(&mut self, client: ClientKeyOrd, oid: ObjectId, previous: PreviousObject) {
        let stored = self.agent.create_object(
            ClientKey(client),
            oid,
            previous.variant,
            CreationMode::default(),
        );
        if stored.is_err() {
            self.events.push(SessionEvent::DdsPortFailed {
                client,
                object: oid,
                error: "the previous object could not be stored again".to_string(),
            });
            return;
        }
        if let Some(plan) = previous.plan {
            let kept = plan.clone();
            if let Err(e) = self
                .apply_dds_create(client, oid, plan)
                .and_then(|()| self.dds.enable((client, oid)))
            {
                self.dds.delete((client, oid));
                let _ = self.agent.delete_object(ClientKey(client), oid);
                self.forget_object(client, oid);
                self.events.push(SessionEvent::DdsPortFailed {
                    client,
                    object: oid,
                    error: format!(
                        "the previous object could not be created again: {}",
                        dds_port_error_message(&e)
                    ),
                });
                return;
            }
            self.plans.insert((client, oid), kept);
        }
        if let Some(extra) = previous.extra {
            self.extras.insert((client, oid), extra);
        }
        if let Some(scope) = previous.scope {
            self.scopes.insert((client, oid), scope);
        }
    }

    /// Reads a CREATE's representation into what the DDS side is created
    /// from (`crate::daemon::entity_repr`), and resolves the objects it
    /// refers to: an endpoint's topic (by `ObjectId` or by DDS topic
    /// name), its Publisher/Subscriber. Only `OBJK_PARTICIPANT`,
    /// `OBJK_TOPIC`, `OBJK_PUBLISHER`/`OBJK_SUBSCRIBER` and the endpoints
    /// have a DDS side; QoS profiles, applications and types are object
    /// store bookkeeping.
    fn plan_dds_create(
        &self,
        client: ClientKeyOrd,
        kind: ObjectKind,
        variant: &ObjectVariant,
        extra: ObjectVariantExtra,
        big_endian: bool,
    ) -> Result<DdsCreate, ReprError> {
        match kind {
            ObjectKind::Participant => {
                let participant =
                    entity_repr::participant(variant, extra.domain_id, big_endian, &self.profiles)?;
                // §7.7.3.6.1, §7.7.3.6.2: a participant that is a DDS-XML
                // definition holds topics, publishers and endpoints; they are
                // made with it (Table 6).
                let source =
                    entity_repr::participant_definition(variant, &self.profiles)?.map(Box::new);
                Ok(DdsCreate::Participant {
                    domain_id: participant.domain_id,
                    qos: participant.qos,
                    scope: participant.scope,
                    not_applied: participant.not_applied,
                    source,
                })
            }
            ObjectKind::Application => Ok(DdsCreate::Application {
                participants: entity_repr::application_definition(variant, &self.profiles)?,
            }),
            ObjectKind::QosProfile => {
                // §7.7.3.2.1: "the fully qualified name of a QosProfile known
                // to the XRCE Agent"; §7.7.3.2.2: a single <qos_library>.
                match variant {
                    ObjectVariant::ByReference(name) => {
                        self.profiles
                            .qos_profile(name)
                            .map_err(ReprError::Unresolved)?;
                    }
                    ObjectVariant::ByXmlString(_) => {
                        if self.definition_to_register(kind, variant)?.is_none() {
                            return Err(ReprError::Malformed(
                                "a QosProfile is a <qos_library> with a <qos_profile> (§7.7.3.2.2)"
                                    .into(),
                            ));
                        }
                    }
                    ObjectVariant::InBinary(_) => {
                        return Err(ReprError::Malformed(
                            "a QosProfile has no binary representation (§7.7.3.2)".into(),
                        ));
                    }
                }
                Ok(DdsCreate::Nothing)
            }
            ObjectKind::Topic => {
                // OBJK_TOPIC_Representation.participant_id names the
                // topic's participant, which must exist (Table 6).
                let participant = self.participant_of_create(client, extra.parent, "topic")?;
                let mut description = entity_repr::topic(
                    variant,
                    big_endian,
                    &self.profiles,
                    self.participant_scopes.get(&(client, participant)),
                )?;
                if let Some(definition) = description.definition.take() {
                    description.definition = Some(
                        self.find_type_definition(client, &definition)
                            .unwrap_or(definition),
                    );
                }
                // §7.7.3.7.3: "If both are present the type_identifier shall
                // take precedence." The name of the type is the one of the
                // known type that has this TypeIdentifier.
                if let Some(identifier) = &description.type_identifier {
                    description.type_name =
                        self.type_name_of(client, identifier).ok_or_else(|| {
                            ReprError::Unresolved(format!(
                                "topic `{}`: no type known to the agent has the \
                                 type_identifier {identifier:?}",
                                description.topic_name
                            ))
                        })?;
                }
                Ok(DdsCreate::Topic {
                    participant,
                    description,
                })
            }
            ObjectKind::Publisher | ObjectKind::Subscriber => {
                let what = if kind == ObjectKind::Publisher {
                    "publisher"
                } else {
                    "subscriber"
                };
                // §7.7.3.8, §7.7.3.9: "the member participant_id shall
                // contain the ObjectId of an XRCE DomainParticipant
                // object"; Table 6: if it is not found the operation
                // "shall fail and return STATUS_ERR_UNKNOWN_REFERENCE".
                let participant = self.participant_of_create(client, extra.parent, what)?;
                Ok(DdsCreate::Container {
                    participant,
                    description: entity_repr::container(variant, what, big_endian, &self.profiles)?,
                })
            }
            ObjectKind::Type => {
                // §7.7.3.3.1: "the fully qualified name of a n XRCE Type known
                // to the XRCE Agent".
                if let ObjectVariant::ByReference(name) = variant {
                    return if self.find_type_definition(client, name).as_deref() == Some(name) {
                        Ok(DdsCreate::Nothing)
                    } else {
                        Err(ReprError::Unresolved(format!(
                            "no type named `{name}` is known to the agent"
                        )))
                    };
                }
                // The types it may reference: the client's own, the ones of
                // the files and of the profile files (§7.7.3.3.2).
                let known: BTreeMap<String, zerodds_types::CompleteTypeObject> = self
                    .config
                    .known_types
                    .iter()
                    .chain(self.profiles.types())
                    .map(|(n, d)| (n.clone(), d.clone()))
                    .chain(
                        self.client_types
                            .iter()
                            .filter(|((c, _), _)| *c == client)
                            .flat_map(|(_, defs)| defs.iter().cloned()),
                    )
                    .collect();
                let named = entity_repr::types(variant, &self.profiles, &known)?;
                // One name, one definition per client: a second object
                // defining a known name differently would make the topic's
                // type depend on creation order.
                for (name, def) in &named {
                    let clash = self
                        .client_types
                        .iter()
                        .filter(|((c, _), _)| *c == client)
                        .flat_map(|(_, defs)| defs.iter())
                        .any(|(n, d)| n == name && d != def);
                    if clash {
                        return Err(ReprError::Malformed(format!(
                            "type `{name}` is already defined differently by this client"
                        )));
                    }
                }
                Ok(DdsCreate::Types(named))
            }
            ObjectKind::DataWriter | ObjectKind::DataReader => {
                let role = if kind == ObjectKind::DataWriter {
                    EndpointRole::Writer
                } else {
                    EndpointRole::Reader
                };
                let mut description =
                    entity_repr::endpoint(variant, role, big_endian, &self.profiles)?;
                let container_participant = extra
                    .parent
                    .and_then(|p| self.participants_of.get(&(client, p)).copied());
                let topic =
                    self.resolve_topic(client, description.topic.as_ref(), container_participant)?;
                // The QoS of the Topic is what the endpoint takes where its
                // own representation states nothing.
                if let Some(known) = self.topics.get(&(client, topic)) {
                    if !known.qos.is_empty() {
                        description = entity_repr::endpoint_for_topic(
                            variant,
                            role,
                            big_endian,
                            &self.profiles,
                            Some(&known.qos),
                        )?;
                    }
                }
                if let (Some(named), Some(known)) = (
                    description.type_name.as_deref(),
                    self.topics.get(&(client, topic)),
                ) {
                    if named != known.type_name {
                        return Err(ReprError::Malformed(format!(
                            "the endpoint names type `{named}`, its topic `{}` has type `{}`",
                            known.topic_name, known.type_name
                        )));
                    }
                }
                // Table 6: the agent locates the XRCE Publisher (DataWriter)
                // or Subscriber (DataReader) identified by publisher_id /
                // subscriber_id; "If this object is not found the operation
                // shall fail and return STATUS_ERR_UNKNOWN_REFERENCE".
                let container_kind = if role == EndpointRole::Writer {
                    ObjectKind::Publisher
                } else {
                    ObjectKind::Subscriber
                };
                let parent = extra.parent.ok_or_else(|| {
                    ReprError::Malformed(format!(
                        "CREATE({container_kind:?}-owned endpoint) names no {container_kind:?}"
                    ))
                })?;
                if parent.kind() != Ok(container_kind) {
                    return Err(ReprError::Malformed(format!(
                        "an endpoint's parent {:04x} is not a {container_kind:?}",
                        parent.raw()
                    )));
                }
                let container = self.containers.get(&(client, parent)).ok_or_else(|| {
                    ReprError::Unresolved(format!(
                        "the endpoint's {container_kind:?} {:04x} does not exist",
                        parent.raw()
                    ))
                })?;
                // DDS: the entity and its topic belong to one participant.
                if self.participants_of.get(&(client, parent))
                    != self.participants_of.get(&(client, topic))
                {
                    return Err(ReprError::Malformed(format!(
                        "the endpoint's {container_kind:?} {:04x} and its topic {:04x} belong to \
                         different participants",
                        parent.raw(),
                        topic.raw()
                    )));
                }
                if description.qos.partitions.is_empty() {
                    description.qos.partitions = container.partitions.clone();
                }
                description.qos.group_data = container.group_data.clone();
                Ok(DdsCreate::Endpoint {
                    role,
                    topic,
                    container: parent,
                    description: Box::new(description),
                })
            }
            _ => Ok(DdsCreate::Nothing),
        }
    }

    /// The XRCE DomainParticipant a topic, Publisher or Subscriber CREATE
    /// names (`participant_id`, §7.7.3.7 to §7.7.3.9): it must be an
    /// `OBJK_PARTICIPANT` id and the client must hold that object.
    fn participant_of_create(
        &self,
        client: ClientKeyOrd,
        named: Option<ObjectId>,
        what: &str,
    ) -> Result<ObjectId, ReprError> {
        let participant = named
            .filter(|p| p.kind() == Ok(ObjectKind::Participant))
            .ok_or_else(|| {
                ReprError::Malformed(format!(
                    "CREATE({what}) names no OBJK_PARTICIPANT as its participant_id"
                ))
            })?;
        if self
            .agent
            .get_object(ClientKey(client), participant)
            .is_none()
        {
            return Err(ReprError::Unresolved(format!(
                "the {what}'s participant {:04x} does not exist",
                participant.raw()
            )));
        }
        Ok(participant)
    }

    /// The topic an endpoint names, among the client's topics that have a
    /// DDS side.
    /// `participant`, if known, limits a topic named by its DDS name to the
    /// ones of that DomainParticipant (the name is "referenceable from any
    /// objects in the same DomainParticipant", Table 15).
    fn resolve_topic(
        &self,
        client: ClientKeyOrd,
        named: Option<&TopicRef>,
        participant: Option<ObjectId>,
    ) -> Result<ObjectId, ReprError> {
        let mut own = self
            .topics
            .iter()
            .filter(|((c, _), _)| *c == client)
            .filter(|((_, oid), _)| match (participant, named) {
                (Some(p), Some(TopicRef::Name(_)) | None) => {
                    self.participants_of.get(&(client, *oid)) == Some(&p)
                }
                _ => true,
            })
            .map(|((_, oid), t)| (*oid, t));
        match named {
            Some(TopicRef::Object(id)) => own
                .find(|(oid, _)| oid == id)
                .map(|(oid, _)| oid)
                .ok_or_else(|| {
                    ReprError::Unresolved(format!("topic {:04x} does not exist", id.raw()))
                }),
            Some(TopicRef::Name(name)) => {
                let mut hits = own.filter(|(_, t)| &t.topic_name == name).map(|(o, _)| o);
                match (hits.next(), hits.next()) {
                    (Some(oid), None) => Ok(oid),
                    (None, _) => Err(ReprError::Unresolved(format!(
                        "no topic named `{name}` was created"
                    ))),
                    (Some(_), Some(_)) => Err(ReprError::Unresolved(format!(
                        "several topics are named `{name}`"
                    ))),
                }
            }
            None => match (own.next(), own.next()) {
                (Some((oid, _)), None) => Ok(oid),
                _ => Err(ReprError::Unresolved(
                    "the endpoint names no topic and the client has not exactly one".into(),
                )),
            },
        }
    }

    /// Creates the DDS side [`Self::plan_dds_create`] planned.
    fn apply_dds_create(
        &mut self,
        client: ClientKeyOrd,
        oid: ObjectId,
        plan: DdsCreate,
    ) -> Result<(), DdsPortError> {
        match plan {
            DdsCreate::Nothing => Ok(()),
            DdsCreate::Types(named) => {
                self.client_types.insert((client, oid), named);
                Ok(())
            }
            DdsCreate::Participant {
                domain_id,
                qos,
                scope,
                not_applied,
                source,
            } => {
                if !not_applied.is_empty() {
                    self.events.push(SessionEvent::RepresentationNotApplied {
                        client,
                        object: oid,
                        elements: not_applied,
                    });
                }
                if let Some(source) = source {
                    if let Ok(scope) = source.scope() {
                        self.participant_scopes.insert((client, oid), scope);
                    }
                } else if let Some(scope) = scope {
                    self.participant_scopes.insert((client, oid), scope);
                }
                self.dds
                    .create_participant_with((client, oid), domain_id, &qos)
            }
            DdsCreate::Application { .. } => Ok(()),
            DdsCreate::Container {
                participant,
                description,
            } => {
                if !description.not_applied.is_empty() {
                    self.events.push(SessionEvent::RepresentationNotApplied {
                        client,
                        object: oid,
                        elements: description.not_applied.clone(),
                    });
                }
                // §7.8.3.1, Table 6: the XRCE Publisher/Subscriber has an
                // associated DDS Publisher/Subscriber, created with the
                // DomainParticipant.
                let spec = ContainerSpec {
                    partitions: description.partitions.clone(),
                    group_data: description.group_data.clone(),
                    presentation: description.presentation,
                    autoenable_created_entities: description.autoenable_created_entities,
                };
                if oid.kind() == Ok(ObjectKind::Publisher) {
                    self.dds
                        .create_publisher((client, oid), (client, participant), spec)?;
                } else {
                    self.dds
                        .create_subscriber((client, oid), (client, participant), spec)?;
                }
                self.participants_of.insert((client, oid), participant);
                self.containers.insert((client, oid), description);
                Ok(())
            }
            DdsCreate::Topic {
                participant,
                description,
            } => {
                if !description.not_applied.is_empty() {
                    self.events.push(SessionEvent::RepresentationNotApplied {
                        client,
                        object: oid,
                        elements: description.not_applied.clone(),
                    });
                }
                let known_type = self.resolve_type(
                    client,
                    description
                        .definition
                        .as_deref()
                        .unwrap_or(&description.type_name),
                );
                if known_type.is_none() && !self.config.forward_unknown_topic_types {
                    return Err(DdsPortError::UnknownReference(format!(
                        "topic `{}`: the type `{}` is not known to the agent",
                        description.topic_name, description.type_name
                    )));
                }
                if known_type.is_none() {
                    let key = (client, oid);
                    if let alloc::collections::btree_map::Entry::Vacant(e) =
                        self.known_type_topics.entry(key)
                    {
                        e.insert(false);
                        self.events.push(SessionEvent::UnknownTypeForwarding {
                            client,
                            topic: oid,
                            type_name: description.type_name.clone(),
                        });
                    }
                } else {
                    self.known_type_topics.insert((client, oid), true);
                }
                self.participants_of.insert((client, oid), participant);
                self.dds.create_topic(
                    (client, oid),
                    (client, participant),
                    TopicSpec {
                        topic_name: description.topic_name.clone(),
                        type_name: description.type_name.clone(),
                        known_type,
                    },
                )?;
                self.topics.insert((client, oid), description);
                Ok(())
            }
            DdsCreate::Endpoint {
                role,
                topic,
                container,
                description,
            } => {
                if !description.not_applied.is_empty() {
                    self.events.push(SessionEvent::RepresentationNotApplied {
                        client,
                        object: oid,
                        elements: description.not_applied.clone(),
                    });
                }
                match role {
                    EndpointRole::Writer => self.dds.create_writer(
                        (client, oid),
                        (client, topic),
                        (client, container),
                        description.qos,
                    ),
                    EndpointRole::Reader => self.dds.create_reader(
                        (client, oid),
                        (client, topic),
                        (client, container),
                        description.qos,
                    ),
                }
            }
        }
    }

    /// Drops the session's own record of one object (its topic name, its
    /// container QoS, its type-known flag).
    fn forget_object(&mut self, client: ClientKeyOrd, oid: ObjectId) {
        self.client_types.remove(&(client, oid));
        self.topics.remove(&(client, oid));
        self.containers.remove(&(client, oid));
        self.known_type_topics.remove(&(client, oid));
        self.scopes.remove(&(client, oid));
        self.participants_of.remove(&(client, oid));
        self.extras.remove(&(client, oid));
        self.writer_activity.remove(&(client, oid));
        self.plans.remove(&(client, oid));
        self.read_sequence.remove(&(client, oid));
        self.participant_scopes.remove(&(client, oid));
        if self.dynamic_definitions.remove(&(client, oid)).is_some() {
            self.rebuild_profiles();
        }
    }

    /// Drops the session's own record of every object of `client`.
    fn forget_client_objects(&mut self, client: ClientKeyOrd) {
        self.reliable_backlog.retain(|(c, _), _| *c != client);
        self.best_effort_seq.retain(|(c, _), _| *c != client);
        self.best_effort_in.retain(|(c, _), _| *c != client);
        self.client_types.retain(|(c, _), _| *c != client);
        self.topics.retain(|(c, _), _| *c != client);
        self.containers.retain(|(c, _), _| *c != client);
        self.known_type_topics.retain(|(c, _), _| *c != client);
        self.scopes.retain(|(c, _), _| *c != client);
        self.participants_of.retain(|(c, _), _| *c != client);
        self.extras.retain(|(c, _), _| *c != client);
        self.writer_activity.retain(|(c, _), _| *c != client);
        self.plans.retain(|(c, _), _| *c != client);
        self.read_sequence.retain(|(c, _), _| *c != client);
        self.participant_scopes.retain(|(c, _), _| *c != client);
        let before = self.dynamic_definitions.len();
        self.dynamic_definitions.retain(|(c, _), _| *c != client);
        if self.dynamic_definitions.len() != before {
            self.rebuild_profiles();
        }
    }

    /// The type a topic names (spec item 5: "known from the XRCE type
    /// object"): the client's own `OBJK_TYPE` definitions first, then the
    /// daemon's `type_files:`, matched by name. The dependencies are every
    /// other type known in the same scope. `None` means the topic is
    /// forwarded opaque.
    fn type_name_of(
        &self,
        client: ClientKeyOrd,
        identifier: &zerodds_types::TypeIdentifier,
    ) -> Option<String> {
        let own = self
            .client_types
            .iter()
            .filter(|((c, _), _)| *c == client)
            .flat_map(|(_, defs)| defs.iter().map(|(n, d)| (n, d)));
        let configured = self.config.known_types.iter();
        own.chain(configured).find_map(|(name, def)| {
            let hashed = zerodds_types::hash::to_hashed_type_identifier(
                &zerodds_types::TypeObject::Complete(def.clone()),
            )
            .ok()?;
            (&hashed == identifier).then(|| name.clone())
        })
    }

    fn resolve_type(&self, client: ClientKeyOrd, type_name: &str) -> Option<KnownType> {
        let own: Vec<&(String, zerodds_types::CompleteTypeObject)> = self
            .client_types
            .iter()
            .filter(|((c, _), _)| *c == client)
            .flat_map(|(_, defs)| defs.iter())
            .collect();
        let root = own
            .iter()
            .find(|(n, _)| n == type_name)
            .map(|(_, d)| d.clone())
            .or_else(|| self.config.known_types.get(type_name).cloned())?;
        let dependencies = own
            .iter()
            .map(|(n, d)| (n.as_str(), d))
            .chain(self.config.known_types.iter().map(|(n, d)| (n.as_str(), d)))
            .filter(|(n, _)| *n != type_name)
            .map(|(_, d)| d.clone())
            .collect();
        Some(KnownType { root, dependencies })
    }

    /// Answers a request (WRITE_DATA, READ_DATA, DELETE) whose body cannot be
    /// read with `STATUS_ERR_INVALID_DATA`: against the `request_id` and
    /// `object_id` in its first four octets when they are there, else against
    /// `OBJECTID_INVALID`. The request is not dropped without a word.
    fn refuse_unreadable(
        &mut self,
        client: ClientKeyOrd,
        header: &MessageHeader,
        sm: &Submessage,
        out: &mut Vec<Outbound>,
    ) {
        self.events.push(SessionEvent::RequestUnreadable {
            client,
            submessage: sm.header.submessage_id,
            body_len: sm.body.len(),
        });
        let (request_id, object_id) = match sm.body.as_slice() {
            [a, b, c, d, ..] => ([*a, *b], ObjectId::from_raw(u16::from_be_bytes([*c, *d]))),
            [a, b] | [a, b, _] => ([*a, *b], OBJECTID_INVALID),
            _ => ([0, 0], OBJECTID_INVALID),
        };
        out.push(status_reply(
            client,
            header,
            request_id,
            object_id,
            ResultStatusCode::InvalidData,
        ));
    }

    fn handle_delete(
        &mut self,
        client: ClientKeyOrd,
        header: &MessageHeader,
        sm: &Submessage,
        out: &mut Vec<Outbound>,
    ) {
        let Ok(payload) = DeletePayload::try_from_submessage(sm) else {
            return;
        };
        let Ok((base, _)) = BaseObjectRequest::decode(&payload.representation) else {
            self.refuse_unreadable(client, header, sm, out);
            return;
        };
        // §7.8.2.3: "The XRCE Agent shall check the ClientKey to locate an
        // existing XRCE: ProxyClient. If the object is not found the
        // operation shall fail and returnValue StatusValue shall be set to
        // STATUS_ERR_UNKNOWN_REFERENCE."
        let deletes_client =
            base.object_id == OBJECTID_CLIENT || base.object_id.kind() == Ok(ObjectKind::Client);
        if deletes_client && !self.agent.has_client(ClientKey(client)) {
            // The same request again, for the session it already ended, is a
            // retransmission: it gets the answer the first one got.
            let code = if self.deleted_clients.get(&client) == Some(&base.request_id) {
                ResultStatusCode::Ok
            } else {
                ResultStatusCode::UnknownReference
            };
            out.push(self.delete_client_reply(client, header, base, code, None));
            return;
        }
        // §7.8.3.4: "If the object cannot be deleted due to permission
        // restrictions, STATUS_ERR_DENIED shall be returned."
        let subject = self.subject_of(client, base.object_id);
        if !self.permit(
            client,
            header,
            base.request_id,
            base.object_id,
            Operation::Delete,
            &subject,
            out,
        ) {
            return;
        }
        if deletes_client {
            let session_id = self.session_ids.get(&client).copied();
            self.agent.unregister_client(ClientKey(client));
            self.dds.unregister_client(client);
            self.reliable.retain(|(c, _), _| *c != client);
            self.fragments.remove(&client);
            self.fragment_runs.retain(|(c, _), _| *c != client);
            self.reads.retain(|(c, _), _| *c != client);
            // §7.8.2.3 and §7.8.2.1: deleting the ProxyClient resets the
            // communication state with the client.
            self.reliable_backlog.retain(|(c, _), _| *c != client);
            self.best_effort_seq.retain(|(c, _), _| *c != client);
            self.best_effort_in.retain(|(c, _), _| *c != client);
            self.liveliness.remove(&client);
            self.session_ids.remove(&client);
            self.session_start.remove(&client);
            self.client_mtu.remove(&client);
            self.forget_client_objects(client);
            self.events
                .push(SessionEvent::ClientUnregistered { client });
            self.deleted_clients.insert(client, base.request_id);
            out.push(self.delete_client_reply(
                client,
                header,
                base,
                ResultStatusCode::Ok,
                session_id,
            ));
            return;
        }
        let removed = self.agent.delete_object(ClientKey(client), base.object_id);
        let existed = matches!(removed, Ok(true));
        self.dds.delete((client, base.object_id));
        self.reads.remove(&(client, base.object_id));
        self.forget_object(client, base.object_id);
        let code = if existed {
            ResultStatusCode::Ok
        } else {
            ResultStatusCode::UnknownReference
        };
        out.push(status_reply(
            client,
            header,
            base.request_id,
            base.object_id,
            code,
        ));
    }

    /// The answer to `DELETE(OBJECTID_CLIENT)`: a `STATUS` (§8.3.5.4), or the
    /// `STATUS_AGENT` Table 14 names when `delete_client_reply:
    /// status_agent` is set.
    fn delete_client_reply(
        &self,
        client: ClientKeyOrd,
        header: &MessageHeader,
        base: BaseObjectRequest,
        code: ResultStatusCode,
        session_id: Option<u8>,
    ) -> Outbound {
        if self.config.delete_client_reply_is_status_agent {
            agent_status_reply(
                client,
                header,
                session_id.unwrap_or(header.session_id.0),
                code,
                self.config.agent_vendor_id,
                // The STATUS_AGENT of a delete_client says its result.
                true,
            )
        } else {
            status_reply(client, header, base.request_id, base.object_id, code)
        }
    }

    /// GET_INFO (§8.3.5.3): the ObjectKind in the last 4 bits of the object_id
    /// selects the target. `OBJK_AGENT` is the XRCE Root (§7.8.2.2), which
    /// answers without a session; `0x01` to `0x0C` is the ProxyClient
    /// (§7.8.3.3) and needs the client. `client` is `None` when the message
    /// came from no session. The answer is an INFO (§8.3.5.7), also for a
    /// failure: its `ResultStatus` carries the error.
    fn handle_get_info(
        &mut self,
        client: Option<ClientKeyOrd>,
        header: &MessageHeader,
        sm: &Submessage,
        out: &mut Vec<Outbound>,
    ) {
        let Ok(payload) = GetInfoPayload::try_from_submessage(sm) else {
            return;
        };
        let Ok((base, _)) = BaseObjectRequest::decode(&payload.representation) else {
            self.refuse_unreadable(client.unwrap_or([0; CLIENT_KEY_LEN]), header, sm, out);
            return;
        };
        let e = sm.header.body_endianness();
        let request = InfoRequest {
            header,
            reply_to: client.unwrap_or([0; CLIENT_KEY_LEN]),
            client,
            base,
        };
        // §8.3.5.3.2: the payload holds the InfoMask.
        let Ok((_, mask)) = payload.parse(e) else {
            self.answer_info(
                &request,
                ResultStatusCode::InvalidData,
                &ObjectInfo::default(),
                out,
            );
            return;
        };
        let kind = base.object_id.kind();
        if kind == Ok(ObjectKind::Agent) {
            let status = client_info_status(&payload, e);
            let info = if status.is_success() {
                agent_object_info(
                    mask,
                    self.config.agent_vendor_id,
                    &self.config.listening_locators,
                )
            } else {
                ObjectInfo::default()
            };
            self.answer_info(&request, status, &info, out);
            return;
        }
        let Some(client) = client else {
            // §7.8.2.2: before a session only the Root can answer.
            self.answer_info(
                &request,
                ResultStatusCode::UnknownReference,
                &ObjectInfo::default(),
                out,
            );
            return;
        };
        let Ok(kind) = kind else {
            self.answer_info(
                &request,
                ResultStatusCode::InvalidData,
                &ObjectInfo::default(),
                out,
            );
            return;
        };
        if matches!(kind, ObjectKind::Invalid | ObjectKind::Other) {
            self.answer_info(
                &request,
                ResultStatusCode::InvalidData,
                &ObjectInfo::default(),
                out,
            );
            return;
        }
        // §7.3: an object the agent is configured with is there for the
        // client without a CREATE.
        if let Err(code) = self.ensure_predefined(client, base.object_id) {
            self.answer_info(&request, code, &ObjectInfo::default(), out);
            return;
        }
        // §7.8.3.3: "If the object cannot be accessed due to permission
        // restrictions STATUS_ERR_DENIED shall be returned."
        let subject = self.subject_of(client, base.object_id);
        if !self
            .config
            .access
            .permits(client, Operation::GetInfo, &subject)
        {
            self.events.push(SessionEvent::OperationDenied {
                client,
                object: base.object_id,
                what: format!("{}: no access rule allows it", Operation::GetInfo.name()),
            });
            self.answer_info(
                &request,
                ResultStatusCode::Denied,
                &ObjectInfo::default(),
                out,
            );
            return;
        }
        let info = if kind == ObjectKind::Client {
            self.client_object_info(client, mask)
        } else {
            match self.object_info(client, base.object_id, kind, mask) {
                Some(info) => info,
                None => {
                    // §7.8.3.3: "If the object does not already exist
                    // STATUS_ERR_UNKNOWN_REFERENCE shall be returned."
                    self.answer_info(
                        &request,
                        ResultStatusCode::UnknownReference,
                        &ObjectInfo::default(),
                        out,
                    );
                    return;
                }
            }
        };
        self.answer_info(&request, ResultStatusCode::Ok, &info, out);
    }

    /// Answers a GET_INFO with an INFO carrying `status` and `info`
    /// (§8.3.5.7.2: the reply echoes `request_id` and `object_id`).
    fn answer_info(
        &self,
        request: &InfoRequest<'_>,
        status: ResultStatusCode,
        info: &ObjectInfo,
        out: &mut Vec<Outbound>,
    ) {
        let build = |status: ResultStatusCode, info: &ObjectInfo| {
            let reply = BaseObjectReply {
                related_request: request.base,
                result: ResultStatus {
                    status_code: status,
                    implementation_status: 0,
                },
            };
            InfoPayload::new(reply, info, Endianness::Little).and_then(InfoPayload::into_submessage)
        };
        // An answer that does not encode (a representation beyond the size
        // of a submessage) is replaced by one that says so.
        let sm = match build(status, info) {
            Ok(sm) => sm,
            Err(_) => match build(ResultStatusCode::Resources, &ObjectInfo::default()) {
                Ok(sm) => sm,
                Err(_) => return,
            },
        };
        let session = request
            .client
            .and_then(|c| self.session_ids.get(&c).copied());
        out.push(Outbound {
            client: request.reply_to,
            header: reply_header(request.reply_to, request.header, session),
            submessages: vec![sm],
        });
    }

    /// `ObjectInfo` of a ProxyClient object (§7.8.3.3): its configuration is
    /// the representation it was created with, its activity what the agent
    /// knows of a DataWriter or DataReader. `None` if the client has no such
    /// object.
    fn object_info(
        &self,
        client: ClientKeyOrd,
        oid: ObjectId,
        kind: ObjectKind,
        mask: InfoMask,
    ) -> Option<ObjectInfo> {
        let inst = self.agent.get_object(ClientKey(client), oid)?;
        let config = mask.configuration().then(|| ObjectConfig::Variant {
            kind: inst.kind,
            variant: inst.variant.clone(),
            extra: self.extras.get(&(client, oid)).copied().unwrap_or_default(),
        });
        let activity = if !mask.activity() {
            None
        } else {
            match kind {
                ObjectKind::DataWriter => {
                    let w = self
                        .writer_activity
                        .get(&(client, oid))
                        .copied()
                        .unwrap_or_default();
                    Some(ActivityInfoVariant::DataWriter {
                        sample_seq_num: w.samples,
                        stream_seq_num: w.last_stream_seq,
                    })
                }
                ObjectKind::DataReader => {
                    // The highest sequence number the client has
                    // acknowledged on the stream the read delivers on; 0
                    // when no read delivers on a reliable stream.
                    let acked = self
                        .reads
                        .get(&(client, oid))
                        .and_then(|r| self.reliable.get(&(client, r.reply_stream_id)))
                        .and_then(ReliableStreamState::highest_acked)
                        .map_or(0, |n| n.raw() as i16);
                    Some(ActivityInfoVariant::DataReader {
                        highest_acked_num: acked,
                    })
                }
                _ => None,
            }
        };
        Some(ObjectInfo { activity, config })
    }

    /// `ObjectInfo` of the client itself (`OBJK_CLIENT`): its
    /// `CLIENT_Representation` (§7.8.2.1) as the agent holds it.
    fn client_object_info(&self, client: ClientKeyOrd, mask: InfoMask) -> ObjectInfo {
        ObjectInfo {
            activity: None,
            config: mask.configuration().then(|| {
                ObjectConfig::Client(ClientRepresentation {
                    timestamp: None,
                    version: crate::daemon::handshake::XRCE_VERSION,
                    vendor_id: [0, 0],
                    client_key: ClientKey(client),
                    session_id: self.session_ids.get(&client).copied().unwrap_or(0),
                    properties: None,
                    requested_mtu: self.client_mtu.get(&client).copied(),
                })
            }),
        }
    }

    fn handle_write_data(
        &mut self,
        client: ClientKeyOrd,
        header: &MessageHeader,
        sm: &Submessage,
        out: &mut Vec<Outbound>,
    ) {
        use zerodds_xrce::submessages::write_data::AnyWriteData;
        let Ok(any) = AnyWriteData::try_from_submessage(sm) else {
            self.refuse_unreadable(client, header, sm, out);
            return;
        };
        let base = any.base();
        let writer = base.object_id;
        // §7.3: a resource the agent is configured with needs no CREATE.
        if let Err(code) = self.ensure_predefined(client, writer) {
            out.push(status_reply(client, header, base.request_id, writer, code));
            return;
        }
        // §7.8.4.1: "If the client is not allowed to write data using the
        // referenced object_id due to permission restrictions, the
        // ResultStatus STATUS_ERR_DENIED shall be returned."
        let subject = self.subject_of(client, writer);
        if !self.permit(
            client,
            header,
            base.request_id,
            writer,
            Operation::WriteData,
            &subject,
            out,
        ) {
            return;
        }
        let e = sm.header.body_endianness();
        let big_endian = !e.is_little();
        // §7.8.4.1: "This operation writes one or more samples". The samples
        // of a sequence are told apart first, all of them, so that a body
        // that does not split writes none.
        let items = match self.split_write_items(client, writer, any, e) {
            Ok(items) => items,
            Err(err) => {
                self.events.push(SessionEvent::DdsPortFailed {
                    client,
                    object: writer,
                    error: err.message.clone(),
                });
                out.push(status_reply(
                    client,
                    header,
                    base.request_id,
                    writer,
                    err.status,
                ));
                return;
            }
        };
        let mut written = true;
        for item in items {
            let source_time = item
                .session_time_offset
                .map(|ms| self.source_time_of(client, ms));
            if !self.dds_write_and_report(
                client,
                header,
                base,
                &item.data,
                big_endian,
                item.dispose,
                item.unregister,
                source_time,
                out,
            ) {
                // The first failure ends the request; its STATUS is out.
                written = false;
                break;
            }
        }
        // §7.8.4.1: "If the data is successfully written STATUS_OK shall be
        // returned." One STATUS answers the request, whatever the number of
        // samples in it (§8.3.5.8: the DataWriter of the request).
        if written {
            out.push(status_reply(
                client,
                header,
                base.request_id,
                writer,
                ResultStatusCode::Ok,
            ));
        }
    }

    /// The samples of a WRITE_DATA, one by one, with their sample
    /// information (§7.7.1, §7.7.2 Table 1). FORMAT_DATA and FORMAT_SAMPLE
    /// hold one. The three sequence formats hold the serialized samples back
    /// to back with no length each (Annex A `sequence<SampleData>`, Annex B.5.2
    /// to B.5.5): the topic's type tells where each ends
    /// ([`DdsPort::sample_length`]), after the `SampleInfo` (FORMAT_SAMPLE_SEQ)
    /// or `SampleInfoDelta` (FORMAT_PACKED_SAMPLES) in front of it.
    fn split_write_items(
        &mut self,
        client: ClientKeyOrd,
        writer: ObjectId,
        any: zerodds_xrce::submessages::write_data::AnyWriteData,
        e: Endianness,
    ) -> Result<Vec<WriteItem>, WriteSplitError> {
        use zerodds_xrce::sample_info::{SampleInfo, SampleInfoDelta};
        use zerodds_xrce::submessages::write_data::AnyWriteData;
        let big_endian = !e.is_little();
        let single = |data: Vec<u8>, info: Option<SampleInfo>| WriteItem {
            data,
            dispose: info.is_some_and(|i| i.state.is_disposed()),
            unregister: info.is_some_and(|i| i.state.is_unregistered()),
            session_time_offset: info.map(|i| i.session_time_offset),
        };
        let (count, elements, mut front): (u32, Vec<u8>, Front) = match any {
            AnyWriteData::Data(p) => return Ok(vec![single(p.serialized_data, None)]),
            AnyWriteData::Sample(p) => return Ok(vec![single(p.serialized_data, Some(p.info))]),
            AnyWriteData::DataSeq(p) => (p.sample_count, p.elements, Front::None),
            AnyWriteData::SampleSeq(p) => (p.sample_count, p.elements, Front::Info),
            AnyWriteData::PackedSamples(p) => {
                (p.sample_count, p.elements, Front::Delta(p.info_base))
            }
        };
        let bad = |message: String| WriteSplitError {
            status: ResultStatusCode::InvalidData,
            message,
        };
        // Every sample takes at least one octet, so a count above the length
        // of the bytes cannot be right (and must not size an allocation).
        if usize::try_from(count).map_or(true, |c| c > elements.len()) {
            return Err(bad(format!(
                "WRITE_DATA {:04x}: {count} samples announced in {} bytes",
                writer.raw(),
                elements.len()
            )));
        }
        let mut items = Vec::with_capacity(count as usize);
        let mut pos = 0usize;
        for index in 0..count {
            let (info, front_len) = match &mut front {
                Front::None => (None, 0),
                Front::Info => {
                    let (info, n) = SampleInfo::decode(&elements[pos..], e).map_err(|err| {
                        bad(format!("WRITE_DATA sample {index}: SampleInfo: {err}"))
                    })?;
                    (Some(info), n)
                }
                Front::Delta(base) => {
                    let (delta, n) =
                        SampleInfoDelta::decode(&elements[pos..], e).map_err(|err| {
                            bad(format!("WRITE_DATA sample {index}: SampleInfoDelta: {err}"))
                        })?;
                    (Some(delta.resolve(base)), n)
                }
            };
            let data_at = pos + front_len;
            let len = self
                .dds
                .sample_length((client, writer), &elements[data_at..], big_endian)
                .map_err(|err| WriteSplitError {
                    status: dds_port_error_status(&err),
                    message: format!(
                        "WRITE_DATA {:04x}: sample {index} of {count}: {}",
                        writer.raw(),
                        dds_port_error_message(&err)
                    ),
                })?;
            if len == 0 || data_at + len > elements.len() {
                return Err(bad(format!(
                    "WRITE_DATA {:04x}: sample {index} of {count} does not fit the body",
                    writer.raw()
                )));
            }
            items.push(single(elements[data_at..data_at + len].to_vec(), info));
            pos = data_at + len;
        }
        if pos != elements.len() {
            return Err(bad(format!(
                "WRITE_DATA {:04x}: {} bytes follow the {count} announced samples",
                writer.raw(),
                elements.len() - pos
            )));
        }
        Ok(items)
    }

    /// The source time a sample carries: `offset_ms` after the time of the
    /// session (§7.7.2 Table 1: "an offset relative to the session timestamp
    /// established when the session was created"). The `CLIENT_Representation`
    /// of Annex A has no `client_timestamp`, so the session's time is the
    /// agent's clock when it created the session.
    fn source_time_of(&self, client: ClientKeyOrd, offset_ms: u32) -> SourceTime {
        let start = self
            .session_start
            .get(&client)
            .copied()
            .unwrap_or_else(wall_clock_now);
        let nanos = u64::from(start.nanoseconds) + u64::from(offset_ms % 1000) * 1_000_000;
        let seconds = i64::from(start.seconds)
            + i64::from(offset_ms / 1000)
            + i64::try_from(nanos / 1_000_000_000).unwrap_or(0);
        SourceTime {
            seconds: i32::try_from(seconds).unwrap_or(i32::MAX),
            nanoseconds: u32::try_from(nanos % 1_000_000_000).unwrap_or(0),
        }
    }

    /// Writes one sample through [`DdsPort::write`]; a failure — including a
    /// dispose or unregister whose instance cannot be named — is a `STATUS`
    /// and a [`SessionEvent::DdsPortFailed`], never silence. Returns whether
    /// the sample was written.
    #[allow(clippy::too_many_arguments)]
    fn dds_write_and_report(
        &mut self,
        client: ClientKeyOrd,
        header: &MessageHeader,
        base: BaseObjectRequest,
        payload: &[u8],
        big_endian: bool,
        dispose: bool,
        unregister: bool,
        source_time: Option<SourceTime>,
        out: &mut Vec<Outbound>,
    ) -> bool {
        let writer = base.object_id;
        match self.dds.write(
            (client, writer),
            payload,
            big_endian,
            dispose,
            unregister,
            source_time,
        ) {
            Ok(_) => {
                let activity = self.writer_activity.entry((client, writer)).or_default();
                activity.samples += 1;
                activity.last_stream_seq = header.sequence_nr.raw() as i16;
                true
            }
            Err(e) => {
                self.events.push(SessionEvent::DdsPortFailed {
                    client,
                    object: writer,
                    error: dds_port_error_message(&e),
                });
                out.push(status_reply(
                    client,
                    header,
                    base.request_id,
                    writer,
                    dds_port_error_status(&e),
                ));
                false
            }
        }
    }

    fn handle_read_data(
        &mut self,
        client: ClientKeyOrd,
        header: &MessageHeader,
        sm: &Submessage,
        now: Duration,
        out: &mut Vec<Outbound>,
    ) {
        let Ok(payload) = ReadDataPayload::try_from_submessage(sm) else {
            self.refuse_unreadable(client, header, sm, out);
            return;
        };
        let reader = payload.base.object_id;
        // §7.3: a resource the agent is configured with needs no CREATE.
        if let Err(code) = self.ensure_predefined(client, reader) {
            out.push(status_reply(
                client,
                header,
                payload.base.request_id,
                reader,
                code,
            ));
            return;
        }
        // §7.8.5.1: "If the client is not allowed to read data using the
        // referenced object_id due to permission restrictions,
        // STATUS_ERR_DENIED shall be returned."
        let subject = self.subject_of(client, reader);
        if !self.permit(
            client,
            header,
            payload.base.request_id,
            reader,
            Operation::ReadData,
            &subject,
            out,
        ) {
            return;
        }
        let spec = &payload.read_specification;
        // §7.8.5.1 Table 9, §8.3.5.9.2: MAX_SAMPLES_ZERO "shall be used to
        // cancel the currently active read operation without sending any
        // more samples". It started a read without a limit.
        if spec
            .delivery_control
            .is_some_and(|d| d.max_samples == MAX_SAMPLES_ZERO)
        {
            self.reads.remove(&(client, reader));
            return;
        }
        // §7.7.14: "the setting of the content_filter_expression
        // configures a content filter... applied to the samples in the
        // DataReader cache." Installed (or cleared, if this READ_DATA
        // omits the optional member) before anything is pumped, so a
        // filter that fails to install (topic type unknown, expression
        // does not parse) never lets an unfiltered sample through — the
        // client gets a STATUS error instead, not silently-unfiltered
        // data.
        if let Err(e) = self.dds.set_read_filter(
            (client, reader),
            payload
                .read_specification
                .content_filter_expression
                .as_deref(),
        ) {
            self.events.push(SessionEvent::DdsPortFailed {
                client,
                object: reader,
                error: dds_port_error_message(&e),
            });
            out.push(status_reply(
                client,
                header,
                payload.base.request_id,
                reader,
                dds_port_error_status(&e),
            ));
            return;
        }
        // Table 9: `MAX_SAMPLES_UNLIMITED` = 0xFFFF, `MAX_ELAPSED_TIME_UNLIMITED`
        // = 0. Without `delivery_control` the read is called once
        // (§8.3.5.9.2) unless `read_without_delivery_control` says
        // otherwise.
        let spec = &payload.read_specification;
        let stream = match (
            spec.delivery_control,
            self.config.read_without_delivery_control,
        ) {
            (Some(d), _) => ReadStream::new(
                ObjectId::default(),
                reader,
                DeliveryControl {
                    max_samples: d.max_samples,
                    max_elapsed_time: elapsed_time_limit(
                        d.max_elapsed_time,
                        self.config.max_elapsed_time_in_milliseconds,
                    ),
                    max_bytes_per_second: u32::from(d.max_bytes_per_second),
                    min_pace_period: Duration::from_millis(u64::from(d.min_pace_period)),
                },
                now,
            ),
            (None, ReadWithoutDeliveryControl::Once) => {
                ReadStream::once(ObjectId::default(), reader, now)
            }
            (None, ReadWithoutDeliveryControl::NextSample) => ReadStream::new(
                ObjectId::default(),
                reader,
                DeliveryControl {
                    max_samples: 1,
                    ..DeliveryControl::default()
                },
                now,
            ),
        };
        // Table 9: "The special value STREAMID_NONE indicates that Agent can
        // choose the StreamId unconstrained." DATA went on STREAMID_NONE,
        // which carries no stream (§8.3.2.2); the agent takes the built-in
        // best-effort stream.
        let reply_stream_id = if spec.preferred_stream_id == StreamId::NONE.0 {
            StreamId::BUILTIN_BEST_EFFORT.0
        } else {
            spec.preferred_stream_id
        };
        self.reads.insert(
            (client, reader),
            ActiveRead {
                stream,
                request_id: payload.base.request_id,
                reply_stream_id,
                format: spec.data_format,
            },
        );
        // Immediately try to satisfy it from whatever the agent already
        // queued for this reader (both the XrceAgent pull-queue path and
        // any DdsPort-buffered samples), so a single-shot READ_DATA does
        // not have to wait for the next `tick`.
        self.pump_read(client, reader, out, now);
        let _ = header; // MessageHeader-level fields not otherwise needed here
    }

    /// Feeds newly available samples (from `XrceAgent`'s pull queue and
    /// from `DdsPort::poll_samples`) into the reader's [`ReadStream`] and
    /// emits `DATA` for whatever it releases.
    fn pump_read(
        &mut self,
        client: ClientKeyOrd,
        reader: ObjectId,
        out: &mut Vec<Outbound>,
        now: Duration,
    ) {
        // Only as many samples are taken from the DataReader as the read
        // still delivers (`ReadStream::capacity`): up to 64 were taken at
        // every pump, and what the read did not deliver before it ended was
        // gone.
        const BATCH: usize = 64;
        let mtu = self
            .client_mtu
            .get(&client)
            .copied()
            .unwrap_or(crate::daemon::handshake::DEFAULT_MTU);
        loop {
            let Some(active) = self.reads.get_mut(&(client, reader)) else {
                return;
            };
            let want = active.stream.capacity(now).min(BATCH);
            let mut taken = 0;
            while taken < want {
                match self.agent.pull_sample(ClientKey(client), reader) {
                    Ok(Some(sample)) => {
                        active.stream.push_sample(PendingSample::new(sample));
                        taken += 1;
                    }
                    _ => break,
                }
            }
            if taken < want {
                for sample in self.dds.poll_samples((client, reader), want - taken) {
                    active.stream.push_sample(PendingSample {
                        bytes: sample.bytes,
                        state: sample.state,
                        source_time: sample.source_time,
                    });
                    taken += 1;
                }
            }
            // A read without delivery_control takes the whole cache before
            // it returns it.
            if active.stream.is_once() && want > 0 && taken == want {
                continue;
            }
            let ready = active.stream.pull_pending_samples(now);
            let request_id = active.request_id;
            let stream_id = active.reply_stream_id;
            let format = active.format;
            let finalized = active.stream.is_finalized();
            let undelivered = active.stream.undelivered();
            let delivered_any = !ready.is_empty();
            let samples: Vec<PendingSample> = ready;
            // §8.3.5.10.2: the payload "shall match the one requested in the
            // READ_DATA message".
            for built in self.data_submessages(client, reader, request_id, format, samples) {
                let sm = match built {
                    Ok(sm) => sm,
                    Err(len) => {
                        self.events.push(SessionEvent::SampleTooLarge {
                            client,
                            reader,
                            len,
                        });
                        out.push(status_reply(
                            client,
                            &default_header(),
                            request_id,
                            reader,
                            ResultStatusCode::Resources,
                        ));
                        continue;
                    }
                };
                let mut header = reply_header(
                    client,
                    &default_header(),
                    self.session_ids.get(&client).copied(),
                );
                header.stream_id = StreamId(stream_id);
                self.emit_possibly_fragmented(client, header, sm, mtu, out);
            }
            if finalized {
                if undelivered > 0 {
                    self.events.push(SessionEvent::ReadEndedUndelivered {
                        client,
                        reader,
                        samples: undelivered,
                    });
                }
                self.reads.remove(&(client, reader));
                return;
            }
            if taken == 0 || !delivered_any {
                return;
            }
        }
    }

    /// The DATA submessages that carry `samples` of `reader` in `format`
    /// (§7.7.1, Table 1, §8.3.5.10.2, Annex B.5): FORMAT_DATA and
    /// FORMAT_SAMPLE one per sample, the three sequence formats as many
    /// samples as one submessage holds (its length is 16 bits; a packed one
    /// takes 256 at most, the range of `seq_number_delta`). A sample that
    /// does not fit a submessage alone is an `Err` with its length.
    ///
    /// The `SampleInfo` is what the DataReader gave: `state` carries the
    /// instance state of a dispose or unregister marker and `VIEW_STATE_NEW`
    /// for the first sample of an instance (`SAMPLE_STATE_READ` is never
    /// set, a sample is handed over once); `session_time_offset` is the
    /// source timestamp of the writer in milliseconds after the start of the
    /// session, or the time of delivery when the writer sent none;
    /// `sequence_number` counts the samples the agent delivered from the
    /// reader. FORMAT_DATA and FORMAT_DATA_SEQ have no `SampleInfo`, so a
    /// marker (a change of instance state, no data) cannot be sent in them.
    fn data_submessages(
        &mut self,
        client: ClientKeyOrd,
        reader: ObjectId,
        request_id: [u8; 2],
        format: zerodds_xrce::submessages::write_data::DataFormat,
        samples: Vec<PendingSample>,
    ) -> Vec<Result<Submessage, usize>> {
        use zerodds_xrce::sample_info::{SampleInfo, SampleInfoDelta, SampleInfoFlags};
        use zerodds_xrce::submessages::data::{
            DataPayloadDataSeq, DataPayloadPackedSamples, DataPayloadSample, DataPayloadSampleSeq,
        };
        use zerodds_xrce::submessages::write_data::DataFormat;
        let e = Endianness::Little;
        let base = BaseObjectRequest {
            request_id,
            object_id: reader,
        };
        let offset_ms = self.session_offset_ms(client);
        let source_offsets: Vec<Option<u32>> = samples
            .iter()
            .map(|s| s.source_time.map(|t| self.source_offset_ms(client, t)))
            .collect();
        let states: Vec<u8> = samples.iter().map(|s| s.state).collect();
        let carries_info = !matches!(format, DataFormat::Data | DataFormat::DataSeq);
        let samples: Vec<(usize, Vec<u8>)> = samples
            .into_iter()
            .enumerate()
            .filter(|(_, s)| carries_info || s.state & 0b11 == 0)
            .map(|(i, s)| (i, s.bytes))
            .collect();
        let counter = self.read_sequence.entry((client, reader)).or_insert(0);
        let mut next_info = |index: usize| {
            *counter = counter.wrapping_add(1);
            SampleInfo {
                state: SampleInfoFlags(states[index]),
                sequence_number: *counter,
                session_time_offset: source_offsets[index].unwrap_or(offset_ms),
            }
        };
        let mut built: Vec<Result<Submessage, usize>> = Vec::new();
        match format {
            DataFormat::Data => {
                for (_, bytes) in samples {
                    let len = bytes.len();
                    built.push(
                        DataPayload {
                            base,
                            serialized_data: bytes,
                        }
                        .into_submessage()
                        .map_err(|_| len),
                    );
                }
            }
            DataFormat::Sample => {
                for (index, bytes) in samples {
                    let len = bytes.len();
                    built.push(
                        DataPayloadSample {
                            base,
                            info: next_info(index),
                            serialized_data: bytes,
                        }
                        .into_submessage()
                        .map_err(|_| len),
                    );
                }
            }
            DataFormat::DataSeq | DataFormat::SampleSeq | DataFormat::PackedSamples => {
                let (per_sample, fixed) = match format {
                    DataFormat::DataSeq => (0, BaseObjectRequest::WIRE_SIZE + 4),
                    DataFormat::SampleSeq => {
                        (SampleInfo::WIRE_SIZE, BaseObjectRequest::WIRE_SIZE + 4)
                    }
                    _ => (
                        SampleInfoDelta::WIRE_SIZE,
                        BaseObjectRequest::WIRE_SIZE + SampleInfo::WIRE_SIZE + 4,
                    ),
                };
                let max_samples = if format == DataFormat::PackedSamples {
                    usize::from(u8::MAX) + 1
                } else {
                    usize::MAX
                };
                let mut chunk: Vec<(usize, Vec<u8>)> = Vec::new();
                let mut size = fixed;
                let mut pending: Vec<Vec<(usize, Vec<u8>)>> = Vec::new();
                for (index, bytes) in samples {
                    let need = per_sample + bytes.len();
                    if fixed + need > usize::from(u16::MAX) {
                        built.push(Err(bytes.len()));
                        continue;
                    }
                    if size + need > usize::from(u16::MAX) || chunk.len() == max_samples {
                        pending.push(core::mem::take(&mut chunk));
                        size = fixed;
                    }
                    size += need;
                    chunk.push((index, bytes));
                }
                if !chunk.is_empty() {
                    pending.push(chunk);
                }
                for chunk in pending {
                    let count = chunk.len() as u32;
                    let len: usize = chunk.iter().map(|(_, b)| b.len()).sum();
                    let submessage = match format {
                        DataFormat::DataSeq => DataPayloadDataSeq {
                            base,
                            sample_count: count,
                            elements: chunk.iter().flat_map(|(_, b)| b.iter().copied()).collect(),
                        }
                        .into_submessage(),
                        DataFormat::SampleSeq => {
                            let mut elements = Vec::new();
                            for (index, bytes) in &chunk {
                                let _ = next_info(*index).encode_into(e, &mut elements);
                                elements.extend_from_slice(bytes);
                            }
                            DataPayloadSampleSeq {
                                base,
                                sample_count: count,
                                elements,
                            }
                            .into_submessage()
                        }
                        _ => {
                            let base_index = chunk.first().map_or(0, |(i, _)| *i);
                            let info_base = next_info(base_index);
                            let mut elements = Vec::new();
                            for (position, (index, bytes)) in chunk.iter().enumerate() {
                                if position > 0 {
                                    let _ = next_info(*index);
                                }
                                // §7.7.1: the delta counts tenths of a second
                                // from the base; a sample before the base
                                // (the source clocks of two writers) is 0.
                                let offset = source_offsets[*index].unwrap_or(offset_ms);
                                let base_offset = source_offsets[base_index].unwrap_or(offset_ms);
                                let delta = SampleInfoDelta {
                                    state: SampleInfoFlags(states[*index]),
                                    seq_number_delta: position as u8,
                                    timestamp_delta: u16::try_from(
                                        offset.saturating_sub(base_offset) / 100,
                                    )
                                    .unwrap_or(u16::MAX),
                                };
                                let _ = delta.encode_into(e, &mut elements);
                                elements.extend_from_slice(bytes);
                            }
                            DataPayloadPackedSamples {
                                base,
                                info_base,
                                sample_count: count,
                                elements,
                            }
                            .into_submessage()
                        }
                    };
                    built.push(submessage.map_err(|_| len));
                }
            }
        }
        built
    }

    /// Milliseconds from the start of `client`'s session to now (§7.7.2:
    /// `session_time_offset`); saturates at the 32 bits of the field.
    fn session_offset_ms(&self, client: ClientKeyOrd) -> u32 {
        let now = wall_clock_now();
        let start = self.session_start.get(&client).copied().unwrap_or(now);
        let nanos = (i128::from(now.seconds) - i128::from(start.seconds)) * 1_000_000_000
            + i128::from(now.nanoseconds)
            - i128::from(start.nanoseconds);
        u32::try_from((nanos / 1_000_000).max(0)).unwrap_or(u32::MAX)
    }

    /// Milliseconds from the start of `client`'s session to the source time
    /// `source` of a sample (seconds since the Unix epoch, nanoseconds); 0 for
    /// a sample from before the session.
    fn source_offset_ms(&self, client: ClientKeyOrd, source: (i32, u32)) -> u32 {
        let now = wall_clock_now();
        let start = self.session_start.get(&client).copied().unwrap_or(now);
        let nanos = (i128::from(source.0) - i128::from(start.seconds)) * 1_000_000_000
            + i128::from(source.1)
            - i128::from(start.nanoseconds);
        u32::try_from((nanos / 1_000_000).max(0)).unwrap_or(u32::MAX)
    }

    /// Sends `sm` as a single `Outbound` if `header` + it fits within
    /// `mtu`; otherwise splits it into `FRAGMENT` submessages (spec
    /// §8.3.5.14) — one `Outbound` per fragment, each sequenced
    /// individually by [`Self::finalize_reliable_sequencing`], matching
    /// how [`Self::handle_message`]'s inbound reassembly interprets
    /// `base_seq` as the message carrying the *first* fragment.
    ///
    /// FRAGMENT is reliable-stream-only (§8.3.5.14: "Direction:
    /// bidirectional, only in reliable streams"), so an oversized `sm` on
    /// a best-effort/`StreamId::NONE` stream cannot legally be
    /// fragmented at all — it is sent whole anyway (this daemon's
    /// existing behavior before MTU-awareness existed) rather than
    /// silently dropped, with a [`SessionEvent::ReliableSendOverflowed`]
    /// naming the mismatch. The one truly unsendable case — an `mtu` too
    /// small to carry even one fragment's header — reports the same
    /// event and sends nothing, since "whole" would only be more
    /// oversized, not less.
    fn emit_possibly_fragmented(
        &mut self,
        client: ClientKeyOrd,
        header: MessageHeader,
        sm: Submessage,
        mtu: u16,
        out: &mut Vec<Outbound>,
    ) {
        let header_size = if header.client_key.is_some() {
            MessageHeader::WIRE_SIZE_WITH_KEY
        } else {
            MessageHeader::WIRE_SIZE_NO_KEY
        };
        let budget = usize::from(mtu);
        if header_size + sm.wire_size_unpadded() <= budget {
            out.push(Outbound {
                client,
                header,
                submessages: vec![sm],
            });
            return;
        }
        if !header.stream_id.is_reliable() {
            self.events.push(SessionEvent::ReliableSendOverflowed {
                client,
                stream_id: header.stream_id.0,
                reason: alloc::format!(
                    "DATA ({} bytes) exceeds the client's {mtu}-byte MTU on a best-effort \
                     stream -- FRAGMENT is reliable-stream-only (spec 8.3.5.14); sent whole",
                    header_size + sm.wire_size_unpadded()
                ),
            });
            out.push(Outbound {
                client,
                header,
                submessages: vec![sm],
            });
            return;
        }
        let Some(fragment_budget) = budget.checked_sub(header_size + SubmessageHeader::WIRE_SIZE)
        else {
            self.events.push(SessionEvent::ReliableSendOverflowed {
                client,
                stream_id: header.stream_id.0,
                reason: alloc::format!(
                    "client MTU {mtu} is too small to carry even one FRAGMENT submessage header"
                ),
            });
            return;
        };
        if fragment_budget == 0 {
            self.events.push(SessionEvent::ReliableSendOverflowed {
                client,
                stream_id: header.stream_id.0,
                reason: alloc::format!(
                    "client MTU {mtu} leaves no room for FRAGMENT payload bytes"
                ),
            });
            return;
        }
        let encoded = encode_submessages(core::slice::from_ref(&sm));
        let chunks: Vec<&[u8]> = encoded.chunks(fragment_budget).collect();
        let last = chunks.len().saturating_sub(1);
        for (i, chunk) in chunks.into_iter().enumerate() {
            let Ok(frag_sm) = (FragmentPayload {
                data: chunk.to_vec(),
                last_fragment: i == last,
            })
            .into_submessage() else {
                continue;
            };
            out.push(Outbound {
                client,
                header,
                submessages: vec![frag_sm],
            });
        }
    }

    /// Gives every outbound message on a stream its sequence number.
    ///
    /// Reliable streams (§8.4.14.1: "Each time a message is sent the
    /// HighestSentSequenceNumber is increased"): each message is
    /// `ReliableStreamState::submit`ted and buffered for repair. A message
    /// the sender window has no room for waits in the stream's backlog
    /// until the client's ACKNACK frees the window — sent without a
    /// sequence number, a reliable client drops it (measured: a
    /// fragmented BigHelloWorld sample never arrived). Backlogged messages
    /// go first, in order.
    ///
    /// Best-effort streams (§8.3.2): a receiver discards a message whose
    /// sequence number is not newer than the last one it took, so each
    /// message gets the next number of its stream (with 0 on every
    /// message, a subscriber took only the first sample of three).
    /// `STREAMID_NONE` carries no sequence.
    ///
    /// Must run after every other submessage handler has had a chance to
    /// push into `out`, and before ACKNACK retransmissions (which already
    /// carry a real, previously assigned sequence number) are merged in —
    /// see the call sites in [`Self::handle_message`]/[`Self::tick`].
    fn finalize_reliable_sequencing(&mut self, out: &mut Vec<Outbound>) {
        for mut ob in core::mem::take(out) {
            let stream = ob.header.stream_id;
            if stream.is_reliable() {
                self.reliable_backlog
                    .entry((ob.client, stream.0))
                    .or_default()
                    .push_back(ob);
            } else {
                if stream != StreamId::NONE {
                    let next = self
                        .best_effort_seq
                        .entry((ob.client, stream.0))
                        .or_insert_with(|| SerialNumber16::new(0));
                    ob.header.sequence_nr = *next;
                    *next = next.next();
                }
                out.push(ob);
            }
        }
        let streams: Vec<(ClientKeyOrd, u8)> = self.reliable_backlog.keys().copied().collect();
        for key in streams {
            let rs = self.reliable.entry(key).or_insert_with(|| {
                ReliableStreamState::new(StreamId(key.1), ReliableConfig::default())
            });
            let Some(queue) = self.reliable_backlog.get_mut(&key) else {
                continue;
            };
            while let Some(front) = queue.front() {
                match rs.submit(encode_submessages(&front.submessages)) {
                    Ok(seq) => {
                        if let Some(mut ob) = queue.pop_front() {
                            ob.header.sequence_nr = seq;
                            out.push(ob);
                        }
                    }
                    // The window is full: the rest waits for an ACKNACK.
                    Err(zerodds_xrce::XrceError::ValueOutOfRange { .. }) => break,
                    Err(e) => {
                        // Larger than a reliable message can be; no wait
                        // helps. Reported, and sent as it is.
                        self.events.push(SessionEvent::ReliableSendOverflowed {
                            client: key.0,
                            stream_id: key.1,
                            reason: alloc::format!("{e:?}"),
                        });
                        if let Some(ob) = queue.pop_front() {
                            out.push(ob);
                        }
                    }
                }
            }
            if queue.is_empty() {
                self.reliable_backlog.remove(&key);
            }
        }
    }

    fn handle_heartbeat(
        &mut self,
        resolved_client: Option<ClientKeyOrd>,
        header: &MessageHeader,
        sm: &Submessage,
        out: &mut Vec<Outbound>,
    ) {
        let Some(client) = resolved_client else {
            return;
        };
        let Ok(hb) = HeartbeatPayload::try_from_submessage(sm) else {
            return;
        };
        let rs = self
            .reliable
            .entry((client, hb.stream_id))
            .or_insert_with(|| {
                ReliableStreamState::new(StreamId(hb.stream_id.max(128)), ReliableConfig::default())
            });
        let last_seen = SerialNumber16::new(hb.last_unacked_seq_nr as u16);
        let ack = rs.pending_acknack(Some(last_seen));
        let Ok(sm) = ack.into_submessage() else {
            return;
        };
        let mut h = reply_header(client, header, self.session_ids.get(&client).copied());
        h.stream_id = StreamId::NONE;
        out.push(Outbound {
            client,
            header: h,
            submessages: vec![sm],
        });
    }

    /// §8.4.14.1's sender state machine: `receive(ACKNACK) / save
    /// HighestAcknowledgedSeqNumber, repair SequenceNumber in NACK` — the
    /// save half is `ReliableStreamState::recv_acknack`; the repair half
    /// (resending the exact buffered bytes for every sequence number the
    /// 16-bit NACK bitmap marks as still missing) was previously not
    /// implemented at all, so a client's NACK for a lost reliable message
    /// got acknowledgement bookkeeping updated but no retransmission —
    /// indistinguishable, from the client's side, from the agent having
    /// silently dropped it.
    fn handle_acknack(
        &mut self,
        resolved_client: Option<ClientKeyOrd>,
        header: &MessageHeader,
        sm: &Submessage,
        out: &mut Vec<Outbound>,
    ) {
        let Some(client) = resolved_client else {
            return;
        };
        let Ok(ack) = AckNackPayload::try_from_submessage(sm) else {
            return;
        };
        let Some(rs) = self.reliable.get_mut(&(client, ack.stream_id)) else {
            return;
        };
        let base = ack.first_unacked_seq_num as u16;
        let bitmap = u16::from_le_bytes(ack.nack_bitmap);
        let stream_id = ack.stream_id;
        rs.recv_acknack(ack);
        for i in 0u16..16 {
            if (bitmap >> i) & 1 == 0 {
                continue; // bit clear = acknowledged, nothing to repair
            }
            let seq = SerialNumber16::new(base.wrapping_add(i));
            let Some(bytes) = rs.get_in_flight(seq) else {
                continue; // never sent this seq, or it already left the window
            };
            let submessages = decode_submessages(bytes);
            if submessages.is_empty() {
                continue;
            }
            let mut h = reply_header(client, header, self.session_ids.get(&client).copied());
            h.stream_id = StreamId(stream_id);
            h.sequence_nr = seq;
            out.push(Outbound {
                client,
                header: h,
                submessages,
            });
        }
    }

    fn handle_reset(&mut self, resolved_client: Option<ClientKeyOrd>, out: &mut Vec<Outbound>) {
        let Some(client) = resolved_client else {
            return;
        };
        // §8.3.5.13: "It shall cause the XRCE Agent to reset all state
        // associated with the session_id indicated in the submessage
        // header": the reliable streams (stream 255 included), the
        // reassembly of FRAGMENTs, the running reads, the messages waiting
        // for room in a sender window, and the sequence numbers of the
        // best-effort streams, in and out.
        for ((c, _), rs) in &mut self.reliable {
            if *c == client {
                rs.reset();
            }
        }
        if let Some(fa) = self.fragments.get_mut(&client) {
            fa.reset();
        }
        self.fragment_runs.retain(|(c, _), _| *c != client);
        self.reads.retain(|(c, _), _| *c != client);
        self.reliable_backlog.retain(|(c, _), _| *c != client);
        self.best_effort_seq.retain(|(c, _), _| *c != client);
        self.best_effort_in.retain(|(c, _), _| *c != client);
        let _ = out; // RESET has no reply per §8.3.5.13
    }

    fn handle_timestamp(
        &mut self,
        resolved_client: Option<ClientKeyOrd>,
        header: &MessageHeader,
        sm: &Submessage,
        out: &mut Vec<Outbound>,
    ) {
        let Some(client) = resolved_client else {
            return;
        };
        let Ok(ts) = TimestampPayload::try_from_submessage(sm) else {
            return;
        };
        // §8.3.5.16: transmit_timestamp is "the timestamp of the sender
        // [...] at the time the [reply] message is sent"; receive_timestamp
        // is "the timestamp at which the related [TIMESTAMP] was received".
        // This handler runs synchronously between the two (no queuing), so
        // one wall-clock read for both is exact enough for the clock-offset
        // estimate TimeSync exists for — `now: Duration` is this engine's
        // *monotonic* clock (deliberately not wall time, so session-timeout/
        // pacing math never sees an NTP step); TimeSync needs real wall
        // time instead, which only this handler reads.
        let wall_clock = wall_clock_now();
        let reply = TimestampReplyPayload {
            transmit_timestamp: wall_clock,
            receive_timestamp: wall_clock,
            originate_timestamp: ts.transmit_timestamp,
        };
        let Ok(sm) = reply.into_submessage() else {
            return;
        };
        let mut h = reply_header(client, header, self.session_ids.get(&client).copied());
        h.stream_id = StreamId::NONE;
        out.push(Outbound {
            client,
            header: h,
            submessages: vec![sm],
        });
    }

    /// A `TIMESTAMP` for `client` (§8.3.5.15): "the timestamp of the sender of
    /// the message at the time the message is sent", on `STREAMID_NONE`. The
    /// client answers with a `TIMESTAMP_REPLY`, which becomes a
    /// [`SessionEvent::ClockSynchronised`]. `None` for a client without a
    /// session.
    pub fn request_timestamp(&mut self, client: ClientKeyOrd) -> Option<Outbound> {
        let session = self.session_ids.get(&client).copied()?;
        let sm = TimestampPayload {
            transmit_timestamp: wall_clock_now(),
        }
        .into_submessage()
        .ok()?;
        let mut header = reply_header(client, &default_header(), Some(session));
        header.stream_id = StreamId::NONE;
        Some(Outbound {
            client,
            header,
            submessages: vec![sm],
        })
    }

    /// The `TIMESTAMP_REPLY` of a client (§8.3.5.16) to a `TIMESTAMP` of the
    /// agent: `originate_timestamp` is the agent's `transmit_timestamp` (t0),
    /// `receive_timestamp` the client's time of receipt (t1),
    /// `transmit_timestamp` the client's time of sending the reply (t2); the
    /// agent reads the clock at receipt (t3). The offset of the client's clock
    /// is ((t1 − t0) + (t2 − t3)) / 2, the round trip (t3 − t0) − (t2 − t1).
    fn handle_timestamp_reply(&mut self, resolved_client: Option<ClientKeyOrd>, sm: &Submessage) {
        let Some(client) = resolved_client else {
            return;
        };
        let Ok(reply) = TimestampReplyPayload::try_from_submessage(sm) else {
            return;
        };
        let nanos =
            |t: TimePoint| i128::from(t.seconds) * 1_000_000_000 + i128::from(t.nanoseconds);
        let (t0, t1, t2, t3) = (
            nanos(reply.originate_timestamp),
            nanos(reply.receive_timestamp),
            nanos(reply.transmit_timestamp),
            nanos(wall_clock_now()),
        );
        let clamp = |v: i128| i64::try_from(v).unwrap_or(if v < 0 { i64::MIN } else { i64::MAX });
        self.events.push(SessionEvent::ClockSynchronised {
            client,
            offset_nanos: clamp(((t1 - t0) + (t2 - t3)) / 2),
            round_trip_nanos: clamp((t3 - t0) - (t2 - t1)),
        });
    }

    /// Drives time-based work: per-stream `HEARTBEAT` emission, continuous
    /// `READ_DATA` pumping, and session-timeout eviction. Call
    /// periodically (e.g. every 100 ms) from the transport's event loop.
    pub fn tick(&mut self, now: Duration) -> Vec<Outbound> {
        let mut out = Vec::new();
        let timed_out: Vec<ClientKeyOrd> = self
            .liveliness
            .iter()
            .filter(|&(_, &last)| now.saturating_sub(last) > self.config.session_timeout)
            .map(|(&c, _)| c)
            .collect();
        for client in timed_out {
            self.agent.unregister_client(ClientKey(client));
            self.dds.unregister_client(client);
            self.reliable.retain(|(c, _), _| *c != client);
            self.fragments.remove(&client);
            self.fragment_runs.retain(|(c, _), _| *c != client);
            self.reads.retain(|(c, _), _| *c != client);
            self.liveliness.remove(&client);
            self.session_ids.remove(&client);
            self.session_start.remove(&client);
            self.client_mtu.remove(&client);
            self.forget_client_objects(client);
            self.events
                .push(SessionEvent::ClientUnregistered { client });
        }

        // §8.3.5.15: a TIMESTAMP to each client every interval.
        self.last_timestamp
            .retain(|client, _| self.session_ids.contains_key(client));
        if let Some(interval) = self.config.timestamp_interval {
            let clients: Vec<ClientKeyOrd> = self.session_ids.keys().copied().collect();
            for client in clients {
                let due = self
                    .last_timestamp
                    .get(&client)
                    .is_none_or(|last| now.saturating_sub(*last) >= interval);
                if due {
                    if let Some(ob) = self.request_timestamp(client) {
                        out.push(ob);
                        self.last_timestamp.insert(client, now);
                    }
                }
            }
        }

        // §8.4.13: a fragment run a client abandoned mid-transmission
        // (sends fragment 1, never sends the rest) must not sit in memory
        // for the whole session — GC'd on its own TTL rather than only on
        // the much longer session timeout. `fragment_runs` uses the same
        // TTL/`now` so the two halves of one run never disagree about
        // whether it is still alive.
        for (client, fa) in &mut self.fragments {
            let discarded = fa.gc(now);
            if discarded > 0 {
                self.events.push(SessionEvent::FragmentsDiscarded {
                    client: *client,
                    runs: discarded,
                });
            }
        }
        self.fragment_runs
            .retain(|_, run| now.saturating_sub(run.last_arrival) <= DEFAULT_GC_TTL);

        let stream_keys: Vec<(ClientKeyOrd, u8)> = self.reliable.keys().copied().collect();
        for (client, stream_id) in stream_keys {
            if let Some(rs) = self.reliable.get_mut(&(client, stream_id)) {
                if let Some(hb) = rs.pending_heartbeat(now) {
                    if let Ok(sm) = hb.into_submessage() {
                        let mut h = reply_header(
                            client,
                            &default_header(),
                            self.session_ids.get(&client).copied(),
                        );
                        h.stream_id = StreamId::NONE;
                        out.push(Outbound {
                            client,
                            header: h,
                            submessages: vec![sm],
                        });
                    }
                }
            }
        }

        let read_keys: Vec<(ClientKeyOrd, ObjectId)> = self.reads.keys().copied().collect();
        for (client, reader) in read_keys {
            self.pump_read(client, reader, &mut out, now);
        }
        self.finalize_reliable_sequencing(&mut out);
        out
    }
}

/// Wall-clock time as a spec `Time_t` (§8.3.5.16 TIMESTAMP_REPLY — the one
/// place this engine needs real time; everywhere else `now: Duration` is
/// the caller's *monotonic* clock, on purpose, so session-timeout/pacing
/// math never sees an NTP step). `Time_t.seconds` is a signed 32-bit
/// count of seconds since the Unix epoch, so this — like the spec's own
/// `Time_t` — wraps in 2038; tracked as a known limitation, not silently
/// truncated (`as i32` on a value already reduced mod 2^32 by the `as
/// u32` cast is the same wrap `Time_t` itself has, not an extra one).
fn wall_clock_now() -> TimePoint {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    TimePoint {
        seconds: d.as_secs() as u32 as i32,
        nanoseconds: d.subsec_nanos(),
    }
}

fn default_header() -> MessageHeader {
    MessageHeader {
        session_id: SessionId(0x80),
        stream_id: StreamId::NONE,
        sequence_nr: SerialNumber16::new(0),
        client_key: None,
    }
}

/// `availability` of `AGENT_ActivityInfo` for an agent that serves.
const AGENT_AVAILABLE: i16 = 1;

/// The `ObjectInfo` of the XRCE Agent (§7.8.2.2, `agent_info`): the activity
/// (`AGENT_ActivityInfo`, with the transport addresses a client can reach the
/// agent at) and the configuration (`AGENT_Representation`, with version and
/// vendor), as far as `mask` asks for them.
pub(crate) fn agent_object_info(
    mask: InfoMask,
    vendor_id: [u8; 2],
    locators: &[TransportLocator],
) -> ObjectInfo {
    ObjectInfo {
        activity: mask.activity().then(|| ActivityInfoVariant::Agent {
            availability: AGENT_AVAILABLE,
            address_seq: locators.to_vec(),
        }),
        config: mask
            .configuration()
            .then_some(ObjectConfig::Agent(AgentRepresentation {
                timestamp: Some(wall_clock_now()),
                version: crate::daemon::handshake::XRCE_VERSION,
                vendor_id,
                properties: None,
            })),
    }
}

fn reply_header(
    client: ClientKeyOrd,
    request: &MessageHeader,
    session_id: Option<u8>,
) -> MessageHeader {
    let sid = session_id.unwrap_or(request.session_id.0);
    if sid <= 127 {
        MessageHeader {
            session_id: SessionId(sid),
            stream_id: request.stream_id,
            sequence_nr: SerialNumber16::new(0),
            client_key: Some(ClientKey(client)),
        }
    } else {
        MessageHeader {
            session_id: SessionId(sid),
            stream_id: request.stream_id,
            sequence_nr: SerialNumber16::new(0),
            client_key: None,
        }
    }
}

// `BaseObjectReply::encode()`/`ResultStatus::encode()` + a fixed-size
// `AGENT_Representation` are a handful of bytes, always far under
// `Submessage`'s `u16::MAX` body cap — `into_submessage()`'s only failure
// mode (`PayloadTooLarge`) cannot trigger on either reply, so the
// `expect()` below documents a real invariant rather than papering over
// an error path the caller should propagate instead.
#[allow(clippy::expect_used)]
fn status_reply(
    client: ClientKeyOrd,
    request: &MessageHeader,
    request_id: [u8; 2],
    object_id: ObjectId,
    code: ResultStatusCode,
) -> Outbound {
    let reply = BaseObjectReply {
        related_request: BaseObjectRequest {
            request_id,
            object_id,
        },
        result: ResultStatus {
            status_code: code,
            implementation_status: 0,
        },
    };
    let body = reply.encode().to_vec();
    let sm = StatusPayload {
        representation: body,
    }
    .into_submessage()
    .expect("STATUS body is always within the submessage size cap");
    Outbound {
        client,
        header: reply_header(client, request, None),
        submessages: vec![sm],
    }
}

/// `STATUS_AGENT_Payload`. Annex A (§8.3.5.5.2 IDL) has the `agent_info`
/// (`AGENT_Representation`) alone, which is the form of a success. The text of
/// §8.3.5.5.2 and the capture of eProsima (`agent-to-client.hex` line 1:
/// `submessage_length = 11` = `ResultStatus`(2) + `AGENT_Representation`(9))
/// have a `ResultStatus` in front; `with_result_status` (agent key
/// `status_agent: with_result_status`) writes that form always. The Annex A
/// form has nowhere to put a failure, so a failed CREATE_CLIENT is answered
/// with the `ResultStatus` the text gives it in either setting.
#[allow(clippy::expect_used)]
fn agent_status_reply(
    client: ClientKeyOrd,
    request: &MessageHeader,
    session_id: u8,
    code: ResultStatusCode,
    vendor_id: [u8; 2],
    with_result_status: bool,
) -> Outbound {
    let rep = AgentRepresentation {
        timestamp: Some(wall_clock_now()),
        version: crate::daemon::handshake::XRCE_VERSION,
        vendor_id,
        properties: None,
    };
    let mut body = Vec::new();
    if with_result_status || !code.is_success() {
        ResultStatus {
            status_code: code,
            implementation_status: 0,
        }
        .encode_into(&mut body, Endianness::Little);
    }
    // Behind the ResultStatus: the padding of the representation is counted
    // from the start of the payload.
    let status_len = body.len();
    if rep.encode_into(&mut body, Endianness::Little).is_err() {
        body.truncate(status_len);
    }
    let sm = StatusAgentPayload {
        representation: body,
    }
    .into_submessage()
    .expect("STATUS_AGENT body is always within the submessage size cap");
    let sid = if code.is_success() {
        session_id
    } else {
        request.session_id.0
    };
    Outbound {
        client,
        header: reply_header(client, request, Some(sid)),
        submessages: vec![sm],
    }
}

fn dds_port_error_status(e: &DdsPortError) -> ResultStatusCode {
    match e {
        DdsPortError::UnknownReference(_) => ResultStatusCode::UnknownReference,
        DdsPortError::DdsError(_) => ResultStatusCode::DdsError,
        // Table 7.7.7 has no dedicated "cannot split a sequence" code;
        // InvalidData ("wire body not parseable") is the closest fit —
        // without the element boundaries a type decoder would give, the
        // body genuinely cannot be parsed into individual samples.
        DdsPortError::InvalidType(_) | DdsPortError::UnsplittableSequence(_) => {
            ResultStatusCode::InvalidData
        }
    }
}

fn dds_port_error_message(e: &DdsPortError) -> String {
    match e {
        DdsPortError::UnknownReference(m)
        | DdsPortError::DdsError(m)
        | DdsPortError::InvalidType(m)
        | DdsPortError::UnsplittableSequence(m) => m.to_string(),
    }
}

/// Encodes a `Vec<Submessage>` as `[header(4) + body]*` with no outer
/// `MessageHeader` and no inter-submessage padding — an internal-only
/// bridge to [`ReliableStreamState`]'s byte-oriented buffering, never
/// written to a real transport.
fn encode_submessages(submessages: &[Submessage]) -> Vec<u8> {
    let mut out = Vec::new();
    for sm in submessages {
        out.extend_from_slice(&sm.header.to_bytes());
        out.extend_from_slice(&sm.body);
    }
    out
}

/// Inverse of [`encode_submessages`].
fn decode_submessages(bytes: &[u8]) -> Vec<Submessage> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while offset + SubmessageHeader::WIRE_SIZE <= bytes.len() {
        let Ok(h) = SubmessageHeader::from_bytes(&bytes[offset..]) else {
            break;
        };
        offset += SubmessageHeader::WIRE_SIZE;
        let len = usize::from(h.submessage_length);
        if offset + len > bytes.len() {
            break;
        }
        out.push(Submessage {
            header: h,
            body: bytes[offset..offset + len].to_vec(),
        });
        offset += len;
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::daemon::dds_port::NullDdsPort;
    use crate::daemon::handshake::ClientRepresentation;
    use crate::daemon::handshake::XRCE_VERSION;
    use zerodds_xrce::submessages::ResetPayload;
    use zerodds_xrce::submessages::read_data::{DataDeliveryControl, ReadSpecification};
    use zerodds_xrce::submessages::write_data::DataFormat;

    /// The configuration of the tests: the defaults of the agent, except that
    /// a topic of a type the agent does not hold is forwarded (the default is
    /// to refuse it, §7.7.3.7.3); the tests that look at that name it.
    fn test_config() -> SessionEngineConfig {
        SessionEngineConfig {
            forward_unknown_topic_types: true,
            ignore_unsupported_qos: true,
            ..SessionEngineConfig::default()
        }
    }

    fn engine() -> SessionEngine<NullDdsPort> {
        SessionEngine::new(NullDdsPort::new(), test_config())
    }

    fn create_client_sm(
        client_key: [u8; CLIENT_KEY_LEN],
        session_id: u8,
        version: [u8; 2],
    ) -> Submessage {
        create_client_sm_with_mtu(client_key, session_id, version, None)
    }

    fn create_client_sm_with_mtu(
        client_key: [u8; CLIENT_KEY_LEN],
        session_id: u8,
        version: [u8; 2],
        requested_mtu: Option<u16>,
    ) -> Submessage {
        let rep = ClientRepresentation {
            timestamp: None,
            version,
            vendor_id: [0, 0],
            client_key: ClientKey(client_key),
            session_id,
            properties: None,
            requested_mtu,
        };
        CreateClientPayload {
            representation: rep,
        }
        .into_submessage()
        .unwrap()
    }

    /// A CREATE_CLIENT with no client_key in the header — the state
    /// before any session exists, exactly how a real first datagram
    /// arrives (§8.3.2.1: session_id>=128 carries no ClientKey).
    fn bootstrap_header() -> MessageHeader {
        MessageHeader::without_client_key(SessionId(0x80), StreamId::NONE, SerialNumber16::new(0))
            .unwrap()
    }

    /// The header of a message in the session `create_client_sm` opens
    /// (session_id 0x81, no ClientKey, STREAMID_NONE).
    fn session_header() -> MessageHeader {
        MessageHeader::without_client_key(SessionId(0x81), StreamId::NONE, SerialNumber16::new(0))
            .unwrap()
    }

    fn create_object_sm(oid: ObjectId, xml: &str, mode: CreationMode) -> Submessage {
        create_variant_sm(oid, &ObjectVariant::ByXmlString(xml.to_string()), mode)
    }

    fn create_variant_sm(oid: ObjectId, variant: &ObjectVariant, mode: CreationMode) -> Submessage {
        let kind = oid.kind().unwrap();
        // §7.7.3's outer ObjectKind-discriminated union needs the
        // trailing field its own kind requires (see
        // crates/xrce/src/object_repr.rs's module doc): the domain for a
        // participant, the container for a publisher/subscriber
        // (`participant_oid`) and for an endpoint (`publisher_oid`/
        // `subscriber_oid`, which a test creates before its endpoints).
        let parent = |parent| ObjectVariantExtra {
            domain_id: None,
            parent: Some(parent),
        };
        let extra = match kind {
            ObjectKind::Participant => ObjectVariantExtra {
                domain_id: Some(0),
                parent: None,
            },
            ObjectKind::Topic | ObjectKind::Publisher | ObjectKind::Subscriber => {
                parent(participant_oid())
            }
            ObjectKind::DataWriter => parent(publisher_oid()),
            ObjectKind::DataReader => parent(subscriber_oid()),
            _ => ObjectVariantExtra::default(),
        };
        create_sm_with_extra(oid, variant, mode, extra)
    }

    /// A CREATE whose `participant_id` / `publisher_id` / `subscriber_id`
    /// is `parent`, not the one the other helpers take.
    fn create_under_sm(
        oid: ObjectId,
        variant: &ObjectVariant,
        mode: CreationMode,
        parent: ObjectId,
    ) -> Submessage {
        create_sm_with_extra(
            oid,
            variant,
            mode,
            ObjectVariantExtra {
                domain_id: None,
                parent: Some(parent),
            },
        )
    }

    fn create_sm_with_extra(
        oid: ObjectId,
        variant: &ObjectVariant,
        mode: CreationMode,
        extra: ObjectVariantExtra,
    ) -> Submessage {
        let base = BaseObjectRequest {
            request_id: [0, 1],
            object_id: oid,
        };
        let mut body = base.encode().to_vec();
        let kind = oid.kind().unwrap();
        body.extend_from_slice(
            &variant
                .encode_for_kind(kind, extra, zerodds_xrce::encoding::Endianness::Little)
                .unwrap(),
        );
        CreatePayload {
            representation: body,
            reuse: mode.reuse,
            replace: mode.replace,
        }
        .into_submessage()
        .unwrap()
    }

    fn participant_oid() -> ObjectId {
        ObjectId::new(0x001, ObjectKind::Participant).unwrap()
    }

    fn publisher_oid() -> ObjectId {
        ObjectId::new(0x001, ObjectKind::Publisher).unwrap()
    }

    fn subscriber_oid() -> ObjectId {
        ObjectId::new(0x001, ObjectKind::Subscriber).unwrap()
    }

    #[test]
    fn create_client_happy_path_registers_and_replies_ok() {
        let mut e = engine();
        let out = e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm([1; CLIENT_KEY_LEN], 0x81, XRCE_VERSION)],
            Duration::ZERO,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].submessages[0].header.submessage_id,
            SubmessageId::StatusAgent
        );
        let reply = StatusAgentPayload::try_from_submessage(&out[0].submessages[0]).unwrap();
        let (result, _) = reply.result_and_position(Endianness::Little).unwrap();
        assert!(result.status_code.is_success());
        assert_eq!(e.client_count(), 1);
    }

    /// §7.7.2, Table 1: "The session timestamp corresponds to the
    /// client_timestamp attribute in CLIENT_Representation." With
    /// `representation_timestamps: present` the `client_timestamp` of the
    /// CREATE_CLIENT is the reference of the `session_time_offset`; a client
    /// that sends none (Annex A) gets the time the agent made the session.
    #[test]
    fn the_client_timestamp_is_the_session_timestamp() {
        use zerodds_xrce::encoding::{RepresentationTimestamps, with_representation_timestamps};
        let client = [0x70; CLIENT_KEY_LEN];
        let stamp = TimePoint {
            seconds: 1_518_905_996,
            nanoseconds: 500_000_000,
        };
        let mut e = engine();
        with_representation_timestamps(RepresentationTimestamps::Present, || {
            let rep = ClientRepresentation {
                timestamp: Some(stamp),
                version: XRCE_VERSION,
                vendor_id: [0, 0],
                client_key: ClientKey(client),
                session_id: 0x81,
                properties: None,
                requested_mtu: None,
            };
            let sm = CreateClientPayload {
                representation: rep,
            }
            .into_submessage()
            .unwrap();
            let out = e.handle_message(None, &bootstrap_header(), &[sm], Duration::ZERO);
            assert_eq!(out.len(), 1);
        });
        assert_eq!(e.session_start[&client], stamp);
        // Annex A: no timestamp, the agent's clock.
        let other = [0x71; CLIENT_KEY_LEN];
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(other, 0x81, XRCE_VERSION)],
            Duration::ZERO,
        );
        assert_ne!(e.session_start[&other], stamp);
        assert!(e.session_start[&other].seconds > 1_600_000_000);
    }

    /// §7.8.1: an operation other than create_client and get_info for a
    /// ClientKey without a ProxyClient fails; the agent drops the message
    /// without an answer (StatusValue has no STATUS_ERR_NOCLIENT) and reports
    /// it as `SessionEvent::NoClient`. The `delete_client` of a ProxyClient
    /// that is not there is answered (§7.8.2.3), a GET_INFO is processed.
    #[test]
    fn a_message_to_an_unknown_client_key_is_dropped_and_counted() {
        let mut e = engine();
        let client = [0x72; CLIENT_KEY_LEN];
        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[participant_sm(1)],
            Duration::ZERO,
        );
        assert!(out.is_empty(), "no answer: {out:?}");
        let events = e.drain_events();
        assert!(
            events.contains(&SessionEvent::NoClient { client }),
            "{events:?}"
        );
        // The ClientKey is not held: nothing was created for it.
        assert!(!e.agent.has_client(ClientKey(client)));
        // A delete_client is answered with UNKNOWN_REFERENCE.
        let delete = DeletePayload {
            representation: BaseObjectRequest {
                request_id: [0, 1],
                object_id: OBJECTID_CLIENT,
            }
            .encode()
            .to_vec(),
        }
        .into_submessage()
        .unwrap();
        let out = e.handle_message(Some(client), &session_header(), &[delete], Duration::ZERO);
        assert_eq!(status_of(&out), ResultStatusCode::UnknownReference);
        assert!(
            !e.drain_events()
                .iter()
                .any(|ev| matches!(ev, SessionEvent::NoClient { .. }))
        );
    }

    /// Table 14 of DDS-XRCE 1.0 names `STATUS_AGENT` for `delete_client`;
    /// §8.3.5.4 says `STATUS` for a DELETE. The `STATUS` is the default;
    /// `delete_client_reply: status_agent` answers with a `STATUS_AGENT`.
    #[test]
    fn delete_client_is_answered_with_status_unless_status_agent_is_named() {
        for (status_agent, expected) in [
            (false, SubmessageId::Status),
            (true, SubmessageId::StatusAgent),
        ] {
            let mut e = SessionEngine::new(
                NullDdsPort::new(),
                SessionEngineConfig {
                    delete_client_reply_is_status_agent: status_agent,
                    ..test_config()
                },
            );
            let client = [0x65; CLIENT_KEY_LEN];
            connected(&mut e, client, 0x81);
            let delete = DeletePayload {
                representation: BaseObjectRequest {
                    request_id: [0, 9],
                    object_id: OBJECTID_CLIENT,
                }
                .encode()
                .to_vec(),
            }
            .into_submessage()
            .unwrap();
            let out = e.handle_message(Some(client), &session_header(), &[delete], Duration::ZERO);
            assert_eq!(out.len(), 1);
            assert_eq!(out[0].submessages[0].header.submessage_id, expected);
            if status_agent {
                let p = StatusAgentPayload::try_from_submessage(&out[0].submessages[0]).unwrap();
                assert!(
                    p.result_and_position(Endianness::Little)
                        .unwrap()
                        .0
                        .status_code
                        .is_success()
                );
            }
            assert!(!e.agent.has_client(ClientKey(client)));
        }
    }

    /// §8.3.5.5.2: the IDL of Annex A has `agent_info` alone in
    /// `STATUS_AGENT_Payload`; the text and the capture of eProsima have a
    /// `ResultStatus` in front. By default the success is the `AGENT_Representation`
    /// alone (9 octets, the cookie first); `status_agent: with_result_status`
    /// writes the `ResultStatus` too; a failure carries it in either form. The
    /// client reads all three.
    #[test]
    fn the_status_agent_payload_is_annex_a_unless_the_result_status_is_named() {
        let reply = |with: bool, version: [u8; 2]| {
            let mut e = SessionEngine::new(
                NullDdsPort::new(),
                SessionEngineConfig {
                    status_agent_with_result_status: with,
                    ..test_config()
                },
            );
            let out = e.handle_message(
                None,
                &bootstrap_header(),
                &[create_client_sm([5; CLIENT_KEY_LEN], 0x81, version)],
                Duration::ZERO,
            );
            StatusAgentPayload::try_from_submessage(&out[0].submessages[0]).unwrap()
        };
        let annex_a = reply(false, XRCE_VERSION);
        assert!(annex_a.representation.starts_with(b"XRCE"));
        assert_eq!(annex_a.representation.len(), 9);
        assert!(
            annex_a
                .result_and_position(Endianness::Little)
                .unwrap()
                .0
                .status_code
                .is_success()
        );
        let with = reply(true, XRCE_VERSION);
        assert!(!with.representation.starts_with(b"XRCE"));
        assert_eq!(
            with.representation.len(),
            6 + 9,
            "ResultStatus with its DHEADER, then the 9"
        );
        let (result, at) = with.result_and_position(Endianness::Little).unwrap();
        assert!(result.status_code.is_success());
        assert_eq!(at, 6);
        // A failure has the ResultStatus in either form.
        for with_result_status in [false, true] {
            let failed = reply(with_result_status, [0x02, 0x00]);
            assert_eq!(
                failed
                    .result_and_position(Endianness::Little)
                    .unwrap()
                    .0
                    .status_code,
                ResultStatusCode::Incompatible
            );
        }
    }

    /// §7.8.2.1: "Check that the major version (xrce_version[0]) matches
    /// XRCE_VERSION_MAJOR. If it does not match, ... STATUS_ERR_INCOMPATIBLE."
    #[test]
    fn create_client_rejects_wrong_major_version() {
        let mut e = engine();
        let bad_version = [0x02, 0x00]; // major=2, this agent only speaks major=1
        let out = e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm([2; CLIENT_KEY_LEN], 0x81, bad_version)],
            Duration::ZERO,
        );
        assert_eq!(out.len(), 1);
        let reply = StatusAgentPayload::try_from_submessage(&out[0].submessages[0]).unwrap();
        let (result, _) = reply.result_and_position(Endianness::Little).unwrap();
        assert_eq!(result.status_code, ResultStatusCode::Incompatible);
        assert_eq!(
            e.client_count(),
            0,
            "an incompatible client is not registered"
        );
    }

    /// §7.8.2.1: a reconnect with the *same* session_id "shall not
    /// perform any action" — the client's existing objects survive.
    #[test]
    fn create_client_same_session_id_reconnect_keeps_object_store() {
        let mut e = engine();
        let client = [3; CLIENT_KEY_LEN];
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            Duration::ZERO,
        );
        e.handle_message(
            Some(client),
            &session_header(),
            &[create_object_sm(
                participant_oid(),
                "<dds><participant><domain_id>0</domain_id></participant></dds>",
                CreationMode::default(),
            )],
            Duration::ZERO,
        );
        assert_eq!(e.agent.object_count(ClientKey(client)).unwrap(), 1);

        // Same client_key, same session_id, reconnects (e.g. a second
        // CREATE_CLIENT the client sends defensively) — no reset.
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            Duration::from_secs(10),
        );
        assert_eq!(
            e.agent.object_count(ClientKey(client)).unwrap(),
            1,
            "same session_id must not reset the ProxyClient"
        );
    }

    /// Opens the session 0x81 of `client`, creates one participant on the
    /// reliable stream (sequence number 0), asks again with the same
    /// session_id, then creates a second participant at sequence number 0.
    /// Returns the object count: 1 when the agent kept the stream (the
    /// second message is a duplicate), 2 when it restarted the stream.
    fn objects_after_a_second_create_client(
        mut e: SessionEngine<NullDdsPort>,
        client: ClientKeyOrd,
    ) -> usize {
        connected(&mut e, client, 0x81);
        let out = e.handle_message(
            Some(client),
            &reliable_header(0),
            &[participant_sm(1)],
            Duration::ZERO,
        );
        assert_eq!(status_of(&out), ResultStatusCode::Ok);
        connected(&mut e, client, 0x81);
        e.handle_message(
            Some(client),
            &reliable_header(0),
            &[participant_sm(2)],
            Duration::ZERO,
        );
        e.agent.object_count(ClientKey(client)).unwrap()
    }

    /// §7.8.2.1: a CREATE_CLIENT with the session_id of the existing
    /// ProxyClient "shall not perform any action": the reliable stream goes
    /// on, a message at sequence number 0 is the duplicate it is.
    #[test]
    fn a_create_client_for_the_same_session_leaves_the_streams_alone() {
        assert_eq!(
            objects_after_a_second_create_client(engine(), [0x61; CLIENT_KEY_LEN]),
            1
        );
    }

    /// With `restart_on_same_session` the streams start again (a client
    /// process that restarts with the same ClientKey and session_id).
    #[test]
    fn a_create_client_for_the_same_session_can_restart_the_streams() {
        let e = SessionEngine::new(
            NullDdsPort::new(),
            SessionEngineConfig {
                restart_on_same_session: true,
                ..test_config()
            },
        );
        assert_eq!(
            objects_after_a_second_create_client(e, [0x62; CLIENT_KEY_LEN]),
            2
        );
    }

    /// §7.8.2.1: a cookie that is not XRCE_COOKIE fails the creation with
    /// STATUS_ERR_INVALID_DATA, and §8.3.5.1 has the agent answer a
    /// CREATE_CLIENT with a STATUS_AGENT.
    #[test]
    fn a_wrong_cookie_is_answered_with_a_status_agent() {
        let mut sm = create_client_sm([0x63; CLIENT_KEY_LEN], 0x81, XRCE_VERSION);
        sm.body[0] ^= 0xFF;
        let mut e = engine();
        let out = e.handle_message(None, &bootstrap_header(), &[sm], Duration::ZERO);
        assert_eq!(agent_status_of(&out), ResultStatusCode::InvalidData);
        assert!(!e.agent.has_client(ClientKey([0x63; CLIENT_KEY_LEN])));
    }

    /// §7.8.2.3: DELETE of a client the agent does not hold fails with
    /// STATUS_ERR_UNKNOWN_REFERENCE; of one it holds, STATUS_OK, and the
    /// ProxyClient is gone (a second DELETE then fails).
    #[test]
    fn delete_client_needs_a_proxy_client() {
        let mut e = engine();
        let client = [0x64; CLIENT_KEY_LEN];
        let delete = |request: u8| {
            DeletePayload {
                representation: BaseObjectRequest {
                    request_id: [0, request],
                    object_id: OBJECTID_CLIENT,
                }
                .encode()
                .to_vec(),
            }
            .into_submessage()
            .unwrap()
        };
        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[delete(3)],
            Duration::ZERO,
        );
        assert_eq!(status_of(&out), ResultStatusCode::UnknownReference);
        connected(&mut e, client, 0x81);
        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[delete(3)],
            Duration::ZERO,
        );
        assert_eq!(status_of(&out), ResultStatusCode::Ok);
        assert!(!e.agent.has_client(ClientKey(client)));
        // The same request again is a retransmission of the one that ended the
        // session and gets the same answer; another request finds no
        // ProxyClient.
        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[delete(3)],
            Duration::ZERO,
        );
        assert_eq!(status_of(&out), ResultStatusCode::Ok);
        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[delete(4)],
            Duration::ZERO,
        );
        assert_eq!(status_of(&out), ResultStatusCode::UnknownReference);
    }

    /// §7.8.2.1: a reconnect with a *different* session_id "shall delete
    /// the existing XRCE ProxyClient object and subsequently take the
    /// same actions as if there had not been a ProxyClient" — the old
    /// object store is gone.
    #[test]
    fn create_client_different_session_id_resets_object_store() {
        let mut e = engine();
        let client = [4; CLIENT_KEY_LEN];
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            Duration::ZERO,
        );
        e.handle_message(
            Some(client),
            &session_header(),
            &[create_object_sm(
                participant_oid(),
                "<dds><participant><domain_id>0</domain_id></participant></dds>",
                CreationMode::default(),
            )],
            Duration::ZERO,
        );
        assert_eq!(e.agent.object_count(ClientKey(client)).unwrap(), 1);

        // Same client_key, a *different* session_id — a fresh ProxyClient.
        let out = e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x42, XRCE_VERSION)],
            Duration::from_secs(10),
        );
        let reply = StatusAgentPayload::try_from_submessage(&out[0].submessages[0]).unwrap();
        let (result, _) = reply.result_and_position(Endianness::Little).unwrap();
        assert!(result.status_code.is_success(), "still STATUS_OK");
        assert_eq!(
            e.agent.object_count(ClientKey(client)).unwrap(),
            0,
            "a different session_id must reset the ProxyClient's object store"
        );
    }

    /// §8.3.5.16: `originate_timestamp` echoes the request's own
    /// `transmit_timestamp`; `transmit_timestamp`/`receive_timestamp` on
    /// the reply must be real wall-clock time, not the all-zero
    /// `TimePoint` a stub would produce (which would make TimeSync
    /// unable to compute any clock offset at all).
    #[test]
    fn timestamp_reply_echoes_originate_and_uses_real_wall_clock() {
        let mut e = engine();
        let client = [5; CLIENT_KEY_LEN];
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            Duration::ZERO,
        );
        let request_ts = TimePoint {
            seconds: 12345,
            nanoseconds: 6789,
        };
        let ts_sm = TimestampPayload {
            transmit_timestamp: request_ts,
        }
        .into_submessage()
        .unwrap();
        let out = e.handle_message(Some(client), &session_header(), &[ts_sm], Duration::ZERO);
        assert_eq!(out.len(), 1);
        let reply = TimestampReplyPayload::try_from_submessage(&out[0].submessages[0]).unwrap();
        assert_eq!(
            reply.originate_timestamp, request_ts,
            "originate_timestamp must echo the request's transmit_timestamp"
        );
        assert_ne!(
            reply.transmit_timestamp,
            TimePoint {
                seconds: 0,
                nanoseconds: 0
            },
            "transmit_timestamp must be real wall-clock time, not a zero stub"
        );
        assert_eq!(
            reply.transmit_timestamp, reply.receive_timestamp,
            "this handler processes synchronously, so both reads are the same instant"
        );
    }

    /// CREATE(participant)/CREATE(topic)/CREATE(reader), so the reader
    /// this test reads from really exists on the `DdsPort` side — matters
    /// once `handle_read_data` starts by calling `DdsPort::
    /// set_read_filter`, which (correctly) answers a nonexistent reader
    /// with `STATUS_ERR_UNKNOWN_REFERENCE` rather than silently pumping
    /// nothing forever.
    fn create_participant_topic_reader(e: &mut SessionEngine<NullDdsPort>, client: ClientKeyOrd) {
        let now = Duration::ZERO;
        e.handle_message(
            Some(client),
            &session_header(),
            &[create_object_sm(
                participant_oid(),
                "<dds><participant><domain_id>0</domain_id></participant></dds>",
                CreationMode::default(),
            )],
            now,
        );
        let topic = ObjectId::new(0x003, ObjectKind::Topic).unwrap();
        e.handle_message(
            Some(client),
            &session_header(),
            &[create_object_sm(
                topic,
                "<dds><topic name=\"T\" type_name=\"Ty\"/></dds>",
                CreationMode::default(),
            )],
            now,
        );
        e.handle_message(
            Some(client),
            &session_header(),
            &[create_object_sm(
                subscriber_oid(),
                "",
                CreationMode::default(),
            )],
            now,
        );
        // `topic_ref` is the full encoded `ObjectId` (id << 4 | kind
        // nibble), not the bare `raw_id` `ObjectId::new`'s first argument
        // takes — the topic's own `raw()` must be used here, not the
        // `0x003` passed to `new` above.
        e.handle_message(
            Some(client),
            &session_header(),
            &[create_object_sm(
                ObjectId::new(0x002, ObjectKind::DataReader).unwrap(),
                &format!(
                    "<dds><data_reader topic_ref=\"0x{:04x}\"/></dds>",
                    topic.raw()
                ),
                CreationMode::default(),
            )],
            now,
        );
    }

    /// Regression for the `header_now()` bug: a continuous READ_DATA's
    /// elapsed-time budget used to be measured from `Duration::ZERO`
    /// regardless of the real clock, so in a daemon that had already been
    /// running longer than the requested budget, the read finalized on
    /// the very first tick — before any real time had passed since the
    /// request. Anchoring at the real `now` (simulated here as a daemon
    /// already 1000s into its run) must NOT finalize until the budget has
    /// actually elapsed relative to *that* start.
    #[test]
    fn read_data_elapsed_time_is_measured_from_the_real_request_time() {
        let mut e = engine();
        let client = [6; CLIENT_KEY_LEN];
        let process_uptime = Duration::from_secs(1000);
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            process_uptime,
        );
        create_participant_topic_reader(&mut e, client);
        let reader = ObjectId::new(0x002, ObjectKind::DataReader).unwrap();
        let read_sm = ReadDataPayload {
            base: BaseObjectRequest {
                request_id: [0, 2],
                object_id: reader,
            },
            read_specification: ReadSpecification {
                preferred_stream_id: 0x01,
                data_format: DataFormat::Data,
                content_filter_expression: None,
                delivery_control: Some(DataDeliveryControl {
                    max_samples: zerodds_xrce::MAX_SAMPLES_UNLIMITED,
                    max_elapsed_time: 2, // seconds
                    max_bytes_per_second: 0,
                    min_pace_period: 0,
                }),
            },
        }
        .into_submessage()
        .unwrap();
        e.handle_message(Some(client), &session_header(), &[read_sm], process_uptime);
        assert!(
            e.reads.contains_key(&(client, reader)),
            "the read must still be active immediately after the request \
             (had it been anchored at Duration::ZERO, elapsed=1000s would \
             already exceed the 2s budget and finalize it right away)"
        );

        // 1s after the real request time — still within the 2s budget.
        e.tick(process_uptime + Duration::from_secs(1));
        assert!(
            e.reads.contains_key(&(client, reader)),
            "1s after the request must still be within a 2s budget"
        );

        // 3s after the real request time — budget exceeded, finalized.
        e.tick(process_uptime + Duration::from_secs(3));
        assert!(
            !e.reads.contains_key(&(client, reader)),
            "3s after the request must have exceeded the 2s budget"
        );
    }

    /// §7.7.14: a `content_filter_expression` the agent cannot honor must
    /// not silently deliver unfiltered data — `NullDdsPort` never has a
    /// decoded type to evaluate a filter against, so `handle_read_data`
    /// must answer with a `STATUS` error and never register the read at
    /// all (a client that got no explicit rejection would otherwise have
    /// no way to know its filter was ignored).
    #[test]
    fn read_data_with_an_unhonorable_content_filter_is_rejected_not_silently_unfiltered() {
        let mut e = engine();
        let client = [7; CLIENT_KEY_LEN];
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            Duration::ZERO,
        );
        create_participant_topic_reader(&mut e, client);
        let reader = ObjectId::new(0x002, ObjectKind::DataReader).unwrap();
        let read_sm = ReadDataPayload {
            base: BaseObjectRequest {
                request_id: [0, 5],
                object_id: reader,
            },
            read_specification: ReadSpecification {
                preferred_stream_id: 0x01,
                data_format: DataFormat::Data,
                content_filter_expression: Some("temperature > 20".into()),
                delivery_control: None,
            },
        }
        .into_submessage()
        .unwrap();
        let out = e.handle_message(Some(client), &session_header(), &[read_sm], Duration::ZERO);
        assert!(
            !e.reads.contains_key(&(client, reader)),
            "a read whose filter could not be honored must not be registered as active"
        );
        assert_eq!(out.len(), 1, "the client must get an explicit STATUS reply");
        let reply = StatusPayload::try_from_submessage(&out[0].submessages[0]).unwrap();
        let (result, _) = BaseObjectReply::decode(&reply.representation).unwrap();
        assert_eq!(result.result.status_code, ResultStatusCode::InvalidData);
    }

    fn read_sm(
        reader: ObjectId,
        request: u8,
        stream: u8,
        delivery_control: Option<DataDeliveryControl>,
    ) -> Submessage {
        ReadDataPayload {
            base: BaseObjectRequest {
                request_id: [0, request],
                object_id: reader,
            },
            read_specification: ReadSpecification {
                preferred_stream_id: stream,
                data_format: DataFormat::Data,
                content_filter_expression: None,
                delivery_control,
            },
        }
        .into_submessage()
        .unwrap()
    }

    fn limited(max_samples: u16) -> Option<DataDeliveryControl> {
        Some(DataDeliveryControl {
            max_samples,
            max_elapsed_time: 0,
            max_bytes_per_second: 0,
            min_pace_period: 0,
        })
    }

    /// The samples of the DATA in `out`, with the stream they went on.
    fn data_of(out: &[Outbound]) -> Vec<(u8, Vec<u8>)> {
        out.iter()
            .filter(|ob| ob.submessages[0].header.submessage_id == SubmessageId::Data)
            .map(|ob| {
                let d = DataPayload::try_from_submessage(&ob.submessages[0]).unwrap();
                (ob.header.stream_id.0, d.serialized_data)
            })
            .collect()
    }

    fn reading_engine(
        rwdc: ReadWithoutDeliveryControl,
        client: ClientKeyOrd,
    ) -> (SessionEngine<NullDdsPort>, ObjectId) {
        let mut e = SessionEngine::new(
            NullDdsPort::new(),
            SessionEngineConfig {
                read_without_delivery_control: rwdc,
                ..test_config()
            },
        );
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            Duration::ZERO,
        );
        create_participant_topic_reader(&mut e, client);
        (e, ObjectId::new(0x002, ObjectKind::DataReader).unwrap())
    }

    /// DDS-XRCE 1.0 §7.8.5.1 Table 9, §8.3.5.9.2: MAX_SAMPLES_ZERO "shall be
    /// used to cancel the currently active read operation without sending
    /// any more samples". It started a read without a limit.
    #[test]
    fn max_samples_zero_cancels_the_active_read() {
        let client = [21; CLIENT_KEY_LEN];
        let (mut e, reader) = reading_engine(ReadWithoutDeliveryControl::Once, client);
        let unlimited = limited(zerodds_xrce::MAX_SAMPLES_UNLIMITED);
        e.handle_message(
            Some(client),
            &session_header(),
            &[read_sm(reader, 1, 0x01, unlimited)],
            Duration::ZERO,
        );
        e.submit_sample(client, reader, vec![1]).unwrap();
        assert_eq!(data_of(&e.tick(Duration::ZERO)), vec![(0x01, vec![1])]);

        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[read_sm(reader, 2, 0x01, limited(0))],
            Duration::ZERO,
        );
        assert!(data_of(&out).is_empty());
        assert!(!e.reads.contains_key(&(client, reader)));
        e.submit_sample(client, reader, vec![2]).unwrap();
        assert!(data_of(&e.tick(Duration::from_millis(100))).is_empty());
    }

    /// §8.3.5.9.2: without delivery_control "the Agent shall call the read
    /// operation just once. As a result the only data returned will be the
    /// one already in the DDS DataReader cache." It returned the first of
    /// three and later the next one that arrived.
    #[test]
    fn a_read_without_delivery_control_returns_the_cache_once() {
        let client = [22; CLIENT_KEY_LEN];
        let (mut e, reader) = reading_engine(ReadWithoutDeliveryControl::Once, client);
        for b in 1..=3u8 {
            e.submit_sample(client, reader, vec![b]).unwrap();
        }
        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[read_sm(reader, 1, 0x01, None)],
            Duration::ZERO,
        );
        assert_eq!(
            data_of(&out),
            vec![(0x01, vec![1]), (0x01, vec![2]), (0x01, vec![3])]
        );
        assert!(!e.reads.contains_key(&(client, reader)));
        e.submit_sample(client, reader, vec![4]).unwrap();
        assert!(data_of(&e.tick(Duration::from_millis(100))).is_empty());
    }

    /// A READ_DATA of `reader` for `format`, without delivery control.
    fn read_in_format(reader: ObjectId, format: DataFormat) -> Submessage {
        ReadDataPayload {
            base: BaseObjectRequest {
                request_id: [0, 9],
                object_id: reader,
            },
            read_specification: ReadSpecification {
                preferred_stream_id: 0x01,
                data_format: format,
                content_filter_expression: None,
                delivery_control: None,
            },
        }
        .into_submessage()
        .unwrap()
    }

    /// The DATA a read for `format` answers with, over three samples
    /// `[1; 4]`, `[2; 4]` and `[3; 4]` in the cache.
    fn data_for_format(format: DataFormat) -> Vec<zerodds_xrce::submessages::data::AnyData> {
        use zerodds_xrce::submessages::data::AnyData;
        let client = [24; CLIENT_KEY_LEN];
        let (mut e, reader) = reading_engine(ReadWithoutDeliveryControl::Once, client);
        for b in 1..=3u8 {
            e.submit_sample(client, reader, vec![b; 4]).unwrap();
        }
        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[read_in_format(reader, format)],
            Duration::ZERO,
        );
        out.iter()
            .filter(|ob| ob.submessages[0].header.submessage_id == SubmessageId::Data)
            .map(|ob| {
                let any = AnyData::try_from_submessage(&ob.submessages[0]).unwrap();
                assert_eq!(any.base().request_id, [0, 9]);
                assert_eq!(any.base().object_id, reader);
                any
            })
            .collect()
    }

    /// §7.8.5.1 Table 9, §8.3.5.10.2: the DATA is in the format the READ_DATA
    /// asked for. FORMAT_DATA_SEQ: one DATA with the count and the samples
    /// back to back (Annex B.5.2).
    #[test]
    fn a_read_for_data_seq_is_answered_with_one_data_seq() {
        use zerodds_xrce::submessages::data::AnyData;
        let data = data_for_format(DataFormat::DataSeq);
        assert_eq!(data.len(), 1);
        let AnyData::DataSeq(seq) = &data[0] else {
            panic!("not FORMAT_DATA_SEQ: {data:?}");
        };
        assert_eq!(seq.sample_count, 3);
        assert_eq!(seq.elements, [vec![1; 4], vec![2; 4], vec![3; 4]].concat());
    }

    /// FORMAT_SAMPLE: a SampleInfo with each sample; the sequence number
    /// counts the samples delivered from the reader (1, 2, 3), the state is
    /// zero.
    #[test]
    fn a_read_for_sample_is_answered_with_a_sample_each() {
        use zerodds_xrce::submessages::data::AnyData;
        let data = data_for_format(DataFormat::Sample);
        assert_eq!(data.len(), 3);
        for (index, any) in data.iter().enumerate() {
            let AnyData::Sample(sample) = any else {
                panic!("not FORMAT_SAMPLE: {any:?}");
            };
            assert_eq!(sample.info.sequence_number, index as u32 + 1);
            assert_eq!(sample.info.state.0, 0);
            assert_eq!(sample.serialized_data, vec![index as u8 + 1; 4]);
        }
    }

    /// FORMAT_SAMPLE_SEQ: the count, then SampleInfo (12 octets) and the
    /// data of each sample (Annex B.5.4).
    #[test]
    fn a_read_for_sample_seq_is_answered_with_one_sample_seq() {
        use zerodds_xrce::sample_info::SampleInfo;
        use zerodds_xrce::submessages::data::AnyData;
        let data = data_for_format(DataFormat::SampleSeq);
        assert_eq!(data.len(), 1);
        let AnyData::SampleSeq(seq) = &data[0] else {
            panic!("not FORMAT_SAMPLE_SEQ: {data:?}");
        };
        assert_eq!(seq.sample_count, 3);
        assert_eq!(seq.elements.len(), 3 * (SampleInfo::WIRE_SIZE + 4));
        for index in 0..3usize {
            let at = index * (SampleInfo::WIRE_SIZE + 4);
            let (info, n) = SampleInfo::decode(&seq.elements[at..], Endianness::Little).unwrap();
            assert_eq!(info.sequence_number, index as u32 + 1);
            assert_eq!(&seq.elements[at + n..at + n + 4], &[index as u8 + 1; 4][..]);
        }
    }

    /// FORMAT_PACKED_SAMPLES: a common SampleInfo, then SampleInfoDelta (4
    /// octets) and data of each sample (Annex B.5.5); the sequence number of
    /// a sample is `info_base.sequence_number + seq_number_delta`.
    #[test]
    fn a_read_for_packed_samples_is_answered_with_one_packed_samples() {
        use zerodds_xrce::sample_info::SampleInfoDelta;
        use zerodds_xrce::submessages::data::AnyData;
        let data = data_for_format(DataFormat::PackedSamples);
        assert_eq!(data.len(), 1);
        let AnyData::PackedSamples(packed) = &data[0] else {
            panic!("not FORMAT_PACKED_SAMPLES: {data:?}");
        };
        assert_eq!(packed.sample_count, 3);
        assert_eq!(packed.info_base.sequence_number, 1);
        assert_eq!(packed.elements.len(), 3 * (SampleInfoDelta::WIRE_SIZE + 4));
        for index in 0..3usize {
            let at = index * (SampleInfoDelta::WIRE_SIZE + 4);
            let (delta, n) =
                SampleInfoDelta::decode(&packed.elements[at..], Endianness::Little).unwrap();
            assert_eq!(usize::from(delta.seq_number_delta), index);
            assert_eq!(
                delta.resolve(&packed.info_base).sequence_number,
                index as u32 + 1
            );
            assert_eq!(
                &packed.elements[at + n..at + n + 4],
                &[index as u8 + 1; 4][..]
            );
        }
    }

    /// A sequence format holds as many samples as one submessage can (its
    /// length is 16 bits), a packed one 256 (the range of `seq_number_delta`);
    /// a sample that does not fit one alone is reported with its length.
    #[test]
    fn the_sequence_formats_split_at_the_size_of_a_submessage() {
        let mut e = engine();
        let client = [25; CLIENT_KEY_LEN];
        let reader = ObjectId::new(0x002, ObjectKind::DataReader).unwrap();
        let count = |built: &[Result<Submessage, usize>]| -> Vec<u32> {
            built
                .iter()
                .map(|b| {
                    match zerodds_xrce::submessages::data::AnyData::try_from_submessage(
                        b.as_ref().unwrap(),
                    )
                    .unwrap()
                    {
                        zerodds_xrce::submessages::data::AnyData::DataSeq(s) => s.sample_count,
                        zerodds_xrce::submessages::data::AnyData::PackedSamples(s) => {
                            s.sample_count
                        }
                        other => panic!("{other:?}"),
                    }
                })
                .collect()
        };
        let built = e.data_submessages(
            client,
            reader,
            [0, 1],
            DataFormat::PackedSamples,
            vec![vec![7u8]; 300]
                .into_iter()
                .map(PendingSample::new)
                .collect(),
        );
        assert_eq!(count(&built), vec![256, 44]);
        let built = e.data_submessages(
            client,
            reader,
            [0, 1],
            DataFormat::DataSeq,
            vec![vec![7u8; 40_000]; 3]
                .into_iter()
                .map(PendingSample::new)
                .collect(),
        );
        assert_eq!(count(&built), vec![1, 1, 1]);
        let built = e.data_submessages(
            client,
            reader,
            [0, 1],
            DataFormat::DataSeq,
            vec![vec![7u8; 30_000]; 3]
                .into_iter()
                .map(PendingSample::new)
                .collect(),
        );
        assert_eq!(count(&built), vec![2, 1]);
        let built = e.data_submessages(
            client,
            reader,
            [0, 1],
            DataFormat::SampleSeq,
            vec![PendingSample::new(vec![7u8; 70_000])],
        );
        assert_eq!(built.len(), 1);
        assert_eq!(built[0].as_ref().unwrap_err(), &70_000);
    }

    /// Annex A `SampleInfoFlags` and the source time of a sample are what the
    /// DataReader gave: a first sample of an instance has `VIEW_STATE_NEW`, a
    /// dispose marker `INSTANCE_STATE_DISPOSED`, the offset is the source
    /// time after the start of the session. FORMAT_SAMPLE sends them;
    /// FORMAT_DATA has no SampleInfo and sends no marker.
    #[test]
    fn the_sample_info_is_what_the_datareader_gave() {
        use zerodds_xrce::sample_info::SampleInfoFlags as F;
        use zerodds_xrce::submessages::data::AnyData;
        let mut e = engine();
        let client = [26; CLIENT_KEY_LEN];
        let reader = ObjectId::new(0x002, ObjectKind::DataReader).unwrap();
        let start = wall_clock_now();
        e.session_start.insert(client, start);
        let at = |ms: i32| (start.seconds, start.nanoseconds + 1_000_000 * ms as u32);
        let samples = || {
            vec![
                PendingSample {
                    bytes: vec![1, 1, 1, 1],
                    state: F::VIEW_STATE_NEW,
                    source_time: Some(at(250)),
                },
                PendingSample {
                    bytes: vec![2, 2, 2, 2],
                    state: 0,
                    source_time: Some(at(400)),
                },
                PendingSample {
                    bytes: Vec::new(),
                    state: F::INSTANCE_STATE_DISPOSED,
                    source_time: None,
                },
            ]
        };
        let built = e.data_submessages(client, reader, [0, 1], DataFormat::Sample, samples());
        let infos: Vec<_> = built
            .iter()
            .map(
                |b| match AnyData::try_from_submessage(b.as_ref().unwrap()).unwrap() {
                    AnyData::Sample(s) => (s.info.state.0, s.info.session_time_offset),
                    other => panic!("{other:?}"),
                },
            )
            .collect();
        assert_eq!(infos[0], (F::VIEW_STATE_NEW, 250));
        assert_eq!(infos[1], (0, 400));
        assert_eq!(infos[2].0, F::INSTANCE_STATE_DISPOSED);
        // Without a SampleInfo the marker has nowhere to go.
        let built = e.data_submessages(client, reader, [0, 1], DataFormat::Data, samples());
        assert_eq!(built.len(), 2);
        let built = e.data_submessages(client, reader, [0, 1], DataFormat::DataSeq, samples());
        assert_eq!(built.len(), 1);
    }

    /// `read_without_delivery_control: next-sample`: the next sample there
    /// is, without a time limit, as Micro-XRCE-DDS 3.0.1 reads.
    #[test]
    fn next_sample_waits_for_one_sample() {
        let client = [23; CLIENT_KEY_LEN];
        let (mut e, reader) = reading_engine(ReadWithoutDeliveryControl::NextSample, client);
        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[read_sm(reader, 1, 0x01, None)],
            Duration::ZERO,
        );
        assert!(data_of(&out).is_empty());
        e.submit_sample(client, reader, vec![7]).unwrap();
        e.submit_sample(client, reader, vec![8]).unwrap();
        assert_eq!(
            data_of(&e.tick(Duration::from_secs(30))),
            vec![(0x01, vec![7])]
        );
        assert!(!e.reads.contains_key(&(client, reader)));
        // The second sample stays for the next read.
        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[read_sm(reader, 2, 0x01, None)],
            Duration::from_secs(30),
        );
        assert_eq!(data_of(&out), vec![(0x01, vec![8])]);
    }

    /// §8.3.5.9: the agent sends DATA "until either the 'end criteria' [...]
    /// attained or else a new READ_DATA message for the same object_id is
    /// received": the new read replaces the active one.
    #[test]
    fn a_new_read_data_replaces_the_active_read() {
        let client = [27; CLIENT_KEY_LEN];
        let (mut e, reader) = reading_engine(ReadWithoutDeliveryControl::Once, client);
        let unlimited = limited(zerodds_xrce::MAX_SAMPLES_UNLIMITED);
        for request in [1, 2] {
            e.handle_message(
                Some(client),
                &session_header(),
                &[read_sm(reader, request, 0x01, unlimited)],
                Duration::ZERO,
            );
        }
        e.submit_sample(client, reader, vec![3]).unwrap();
        let requests: Vec<[u8; 2]> = e
            .tick(Duration::ZERO)
            .iter()
            .filter(|ob| ob.submessages[0].header.submessage_id == SubmessageId::Data)
            .map(|ob| {
                DataPayload::try_from_submessage(&ob.submessages[0])
                    .unwrap()
                    .base
                    .request_id
            })
            .collect();
        assert_eq!(requests, vec![[0, 2]]);
    }

    /// A read takes from the DataReader only what it delivers: up to 64
    /// samples were taken at every pump and what the read did not deliver
    /// before `max_samples` ended it was gone, also for the next read.
    #[test]
    fn a_read_takes_only_the_samples_it_delivers() {
        let client = [24; CLIENT_KEY_LEN];
        let (mut e, reader) = reading_engine(ReadWithoutDeliveryControl::Once, client);
        for b in 1..=5u8 {
            e.submit_sample(client, reader, vec![b]).unwrap();
        }
        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[read_sm(reader, 1, 0x01, limited(3))],
            Duration::ZERO,
        );
        assert_eq!(
            data_of(&out),
            vec![(0x01, vec![1]), (0x01, vec![2]), (0x01, vec![3])]
        );
        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[read_sm(reader, 2, 0x01, None)],
            Duration::ZERO,
        );
        assert_eq!(data_of(&out), vec![(0x01, vec![4]), (0x01, vec![5])]);
    }

    /// Table 9: "The special value STREAMID_NONE indicates that Agent can
    /// choose the StreamId unconstrained." DATA went on STREAMID_NONE; the
    /// agent takes the built-in best-effort stream.
    #[test]
    fn streamid_none_lets_the_agent_choose_the_stream() {
        let client = [25; CLIENT_KEY_LEN];
        let (mut e, reader) = reading_engine(ReadWithoutDeliveryControl::Once, client);
        e.submit_sample(client, reader, vec![9]).unwrap();
        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[read_sm(reader, 1, StreamId::NONE.0, None)],
            Duration::ZERO,
        );
        assert_eq!(
            data_of(&out),
            vec![(StreamId::BUILTIN_BEST_EFFORT.0, vec![9])]
        );
    }

    /// A sample longer than a DATA submessage can carry (16-bit
    /// submessageLength, §8.3.4.3) is reported and answered with STATUS
    /// STATUS_ERR_RESOURCES; it was skipped without a word.
    #[test]
    fn a_sample_too_large_for_data_is_reported() {
        let client = [26; CLIENT_KEY_LEN];
        let (mut e, reader) = reading_engine(ReadWithoutDeliveryControl::Once, client);
        let _ = e.drain_events();
        e.submit_sample(client, reader, vec![0; 70_000]).unwrap();
        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[read_sm(reader, 1, 0x01, None)],
            Duration::ZERO,
        );
        assert!(data_of(&out).is_empty());
        let status = out
            .iter()
            .find(|ob| ob.submessages[0].header.submessage_id == SubmessageId::Status)
            .expect("a STATUS for the read");
        let reply = StatusPayload::try_from_submessage(&status.submessages[0]).unwrap();
        let (result, _) = BaseObjectReply::decode(&reply.representation).unwrap();
        assert_eq!(result.result.status_code, ResultStatusCode::Resources);
        assert_eq!(result.related_request.request_id, [0, 1]);
        assert!(
            e.drain_events()
                .iter()
                .any(|ev| matches!(ev, SessionEvent::SampleTooLarge { len: 70_000, .. }))
        );
    }

    fn reliable_header(seq: u16) -> MessageHeader {
        MessageHeader {
            session_id: SessionId(0x81),
            stream_id: StreamId(0x80),
            sequence_nr: SerialNumber16::new(seq),
            client_key: None,
        }
    }

    /// Regression for the `AssemblerKey`/offset bug: every FRAGMENT
    /// submessage used to be reassembled with `offset` hardcoded to `0`
    /// and `base_seq` taken from *that fragment's own* sequence number
    /// (instead of the run's first fragment) — so a genuinely
    /// multi-fragment transmission never combined its pieces; only the
    /// last fragment's own bytes "completed" a (wrong) 1-fragment
    /// reassembly, silently discarding everything before it. Splits a
    /// real CREATE(participant) submessage into three FRAGMENT pieces and
    /// checks the participant actually gets created — this would have
    /// failed under the old code (either a decode failure on the last
    /// fragment's bytes alone, or nothing at all). Sequence numbers start
    /// at 0 (a fresh `ReliableStreamState`'s `expected_seq`) since this
    /// also has to clear the receiver-side in-order gate.
    #[test]
    fn multi_fragment_create_reassembles_correctly() {
        let mut e = engine();
        let client = [11; CLIENT_KEY_LEN];
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            Duration::ZERO,
        );
        let full_sm = create_object_sm(
            participant_oid(),
            "<dds><participant><domain_id>0</domain_id></participant></dds>",
            CreationMode::default(),
        );
        let encoded = encode_submessages(&[full_sm]);
        assert!(
            encoded.len() >= 6,
            "the CREATE submessage must be big enough to split into three non-trivial pieces"
        );
        let chunk_len = encoded.len().div_ceil(3);
        let chunks: Vec<Vec<u8>> = encoded.chunks(chunk_len).map(<[u8]>::to_vec).collect();
        assert!(
            chunks.len() >= 2,
            "must actually exercise multi-fragment reassembly"
        );
        let last = chunks.len() - 1;
        for (i, chunk) in chunks.into_iter().enumerate() {
            let frag_sm = FragmentPayload {
                data: chunk,
                last_fragment: i == last,
            }
            .into_submessage()
            .unwrap();
            e.handle_message(
                Some(client),
                &reliable_header(i as u16),
                &[frag_sm],
                Duration::ZERO,
            );
        }
        assert_eq!(
            e.agent.object_count(ClientKey(client)).unwrap(),
            1,
            "the reassembled CREATE(participant) must have actually run"
        );
    }

    /// DDS-XRCE 1.0 §8.3.5.14: "The concatenated payloads shall be
    /// interpreted as XRCE submessages" — a run that carries two CREATEs,
    /// each at a multiple of 4 (§8.3.3), runs both. Only the first ran,
    /// the second was dropped without a word.
    #[test]
    fn a_fragmented_run_of_two_submessages_runs_both() {
        let mut e = engine();
        let client = [12; CLIENT_KEY_LEN];
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            Duration::ZERO,
        );
        // A body whose length is no multiple of 4, so the run has padding.
        let mut xml =
            String::from("<dds><participant><domain_id>0</domain_id></participant></dds>");
        let sm_of = |id: u16, xml: &str| {
            create_object_sm(
                ObjectId::new(id, ObjectKind::Participant).unwrap(),
                xml,
                CreationMode::default(),
            )
        };
        while sm_of(1, &xml).body.len() % 4 == 0 {
            xml.push(' ');
        }
        let mut run = Vec::new();
        for sm in [sm_of(1, &xml), sm_of(2, &xml)] {
            run.extend_from_slice(&sm.header.to_bytes());
            run.extend_from_slice(&sm.body);
            while run.len() % 4 != 0 {
                run.push(0);
            }
        }
        let chunks: Vec<Vec<u8>> = run
            .chunks(run.len().div_ceil(3))
            .map(<[u8]>::to_vec)
            .collect();
        let last = chunks.len() - 1;
        for (i, chunk) in chunks.into_iter().enumerate() {
            let frag_sm = FragmentPayload {
                data: chunk,
                last_fragment: i == last,
            }
            .into_submessage()
            .unwrap();
            e.handle_message(
                Some(client),
                &reliable_header(i as u16),
                &[frag_sm],
                Duration::ZERO,
            );
        }
        assert_eq!(e.agent.object_count(ClientKey(client)).unwrap(), 2);
    }

    /// §8.3.5.14: the concatenated payloads "shall be interpreted as XRCE
    /// submessages". A run that holds none is answered with STATUS
    /// `InvalidData` against `OBJECTID_CLIENT`; it is not accepted silently.
    #[test]
    fn a_fragment_run_without_a_submessage_is_answered_invalid_data() {
        let mut e = engine();
        let client = [13; CLIENT_KEY_LEN];
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            Duration::ZERO,
        );
        let frag_sm = FragmentPayload {
            data: Vec::new(),
            last_fragment: true,
        }
        .into_submessage()
        .unwrap();
        let out = e.handle_message(
            Some(client),
            &reliable_header(0),
            &[frag_sm],
            Duration::ZERO,
        );
        assert_eq!(status_of(&out), ResultStatusCode::InvalidData);
    }

    /// Two fragmented messages one after the other on one reliable
    /// stream, as the eProsima BigHelloWorld publisher sends its samples:
    /// the second must be reassembled like the first (measured on the
    /// Linux host: the second 50 kB sample's 101 fragments all arrived in
    /// order, and the reassembly never completed).
    #[test]
    fn a_second_fragmented_message_reassembles_like_the_first() {
        let mut e = engine();
        let client = [15; CLIENT_KEY_LEN];
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            Duration::ZERO,
        );
        let mut seq = 0u16;
        for raw in [0x001u16, 0x002] {
            let full_sm = create_object_sm(
                ObjectId::new(raw, ObjectKind::Participant).unwrap(),
                "<dds><participant><domain_id>0</domain_id></participant></dds>",
                CreationMode::default(),
            );
            let encoded = encode_submessages(&[full_sm]);
            let chunks: Vec<Vec<u8>> = encoded.chunks(4).map(<[u8]>::to_vec).collect();
            let last = chunks.len() - 1;
            for (i, chunk) in chunks.into_iter().enumerate() {
                let frag_sm = FragmentPayload {
                    data: chunk,
                    last_fragment: i == last,
                }
                .into_submessage()
                .unwrap();
                e.handle_message(
                    Some(client),
                    &reliable_header(seq),
                    &[frag_sm],
                    Duration::ZERO,
                );
                seq += 1;
            }
        }
        assert_eq!(
            e.agent.object_count(ClientKey(client)).unwrap(),
            2,
            "both reassembled CREATEs must have run"
        );
    }

    /// The agent's tick runs every 100 ms, between the fragments of a
    /// message that is still arriving: the reassembly must survive it.
    /// With the assembler's derived Default (TTL 0) every tick discarded
    /// the run in progress.
    #[test]
    fn a_tick_between_fragments_keeps_the_reassembly() {
        let mut e = engine();
        let client = [16; CLIENT_KEY_LEN];
        let t0 = Duration::from_secs(3);
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            t0,
        );
        let full_sm = create_object_sm(
            participant_oid(),
            "<dds><participant><domain_id>0</domain_id></participant></dds>",
            CreationMode::default(),
        );
        let encoded = encode_submessages(&[full_sm]);
        let chunks: Vec<Vec<u8>> = encoded.chunks(16).map(<[u8]>::to_vec).collect();
        let last = chunks.len() - 1;
        for (i, chunk) in chunks.into_iter().enumerate() {
            let now = t0 + Duration::from_millis(50 * i as u64);
            let frag_sm = FragmentPayload {
                data: chunk,
                last_fragment: i == last,
            }
            .into_submessage()
            .unwrap();
            e.handle_message(Some(client), &reliable_header(i as u16), &[frag_sm], now);
            let _ = e.tick(now + Duration::from_millis(25));
        }
        assert_eq!(e.agent.object_count(ClientKey(client)).unwrap(), 1);
        assert!(
            e.drain_events()
                .iter()
                .all(|ev| !matches!(ev, SessionEvent::FragmentsDiscarded { .. }))
        );
    }

    /// Regression for two compounding gaps: (1) inbound reliable-stream
    /// submessages were dispatched the instant they arrived, with no
    /// gap/duplicate tracking at all (`ReliableStreamState::recv_data`/
    /// `drain_in_order` were never called) — an out-of-order message ran
    /// immediately instead of waiting for the gap to fill, and a
    /// retransmitted duplicate ran a second time instead of being
    /// recognized; (2) `pending_acknack`'s bitmap is computed entirely
    /// from `recv_data`'s bookkeeping, so with it never called the agent's
    /// ACKNACK to the client *always* reported "everything since sequence
    /// 0 is still missing", regardless of how much the client had
    /// actually gotten through — a spec-compliant sender would see its
    /// own already-delivered messages permanently NACK'd.
    #[test]
    fn out_of_order_reliable_messages_wait_for_the_gap_and_report_a_truthful_acknack() {
        let mut e = engine();
        let client = [14; CLIENT_KEY_LEN];
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            Duration::ZERO,
        );

        // seq 0: CREATE(participant) — in order, dispatched immediately.
        let sm0 = create_object_sm(
            participant_oid(),
            "<dds><participant><domain_id>0</domain_id></participant></dds>",
            CreationMode::default(),
        );
        e.handle_message(Some(client), &reliable_header(0), &[sm0], Duration::ZERO);
        assert_eq!(e.agent.object_count(ClientKey(client)).unwrap(), 1);

        // seq 2: CREATE(topic oid=3) — arrives before seq 1, must be
        // buffered, not dispatched (object count unchanged).
        let topic_a = ObjectId::new(0x003, ObjectKind::Topic).unwrap();
        let sm2 = create_object_sm(
            topic_a,
            "<dds><topic name=\"A\" type_name=\"Ty\"/></dds>",
            CreationMode::default(),
        );
        e.handle_message(
            Some(client),
            &reliable_header(2),
            &[sm2.clone()],
            Duration::ZERO,
        );
        assert_eq!(
            e.agent.object_count(ClientKey(client)).unwrap(),
            1,
            "an out-of-order message must not be dispatched before its gap is filled"
        );

        // Resending seq 2 again (the client retransmitting because it has
        // not seen an ACK yet) must not somehow double-apply it either.
        e.handle_message(Some(client), &reliable_header(2), &[sm2], Duration::ZERO);
        assert_eq!(e.agent.object_count(ClientKey(client)).unwrap(), 1);

        // A HEARTBEAT now must produce a truthful ACKNACK: seq 0 already
        // fully received (first_unacked advances past it), seq 1 still
        // missing (bit set), seq 2 already buffered (bit clear).
        let hb_sm = HeartbeatPayload {
            first_unacked_seq_nr: 0,
            last_unacked_seq_nr: 2,
            stream_id: 0x80,
        }
        .into_submessage()
        .unwrap();
        let out = e.handle_message(Some(client), &session_header(), &[hb_sm], Duration::ZERO);
        assert_eq!(out.len(), 1);
        let ack = AckNackPayload::try_from_submessage(&out[0].submessages[0]).unwrap();
        assert_eq!(
            ack.first_unacked_seq_num, 1,
            "seq 0 was fully received and dispatched, so the receiver now expects seq 1"
        );
        let bitmap = u16::from_le_bytes(ack.nack_bitmap);
        assert_eq!(bitmap & 1, 1, "seq 1 (bit 0) is genuinely still missing");
        assert_eq!(
            bitmap & 2,
            0,
            "seq 2 (bit 1) is already buffered, not missing -- must not be re-requested"
        );

        // seq 1 arrives, filling the gap: seq 1 AND the buffered seq 2
        // must both drain and dispatch now, in order.
        let topic_b = ObjectId::new(0x004, ObjectKind::Topic).unwrap();
        let sm1 = create_object_sm(
            topic_b,
            "<dds><topic name=\"B\" type_name=\"Ty\"/></dds>",
            CreationMode::default(),
        );
        e.handle_message(Some(client), &reliable_header(1), &[sm1], Duration::ZERO);
        assert_eq!(
            e.agent.object_count(ClientKey(client)).unwrap(),
            3,
            "filling the gap at seq 1 must release both seq 1 and the buffered seq 2"
        );
    }

    /// §8.4.14.1: "Each time a message is sent the
    /// HighestSentSequenceNumber is increased." Two replies sent back to
    /// back on the same reliable stream must carry two different,
    /// incrementing sequence numbers — previously every reply on every
    /// stream carried `sequence_nr = 0` unconditionally.
    #[test]
    fn reliable_outbound_messages_get_real_incrementing_sequence_numbers() {
        let mut e = engine();
        let client = [12; CLIENT_KEY_LEN];
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            Duration::ZERO,
        );
        let sm1 = create_object_sm(
            participant_oid(),
            "<dds><participant><domain_id>0</domain_id></participant></dds>",
            CreationMode::default(),
        );
        let out1 = e.handle_message(Some(client), &reliable_header(0), &[sm1], Duration::ZERO);
        assert_eq!(out1.len(), 1);
        let seq1 = out1[0].header.sequence_nr;

        let topic_oid = ObjectId::new(0x003, ObjectKind::Topic).unwrap();
        let sm2 = create_object_sm(
            topic_oid,
            "<dds><topic name=\"T\" type_name=\"Ty\"/></dds>",
            CreationMode::default(),
        );
        let out2 = e.handle_message(Some(client), &reliable_header(1), &[sm2], Duration::ZERO);
        assert_eq!(out2.len(), 1);
        let seq2 = out2[0].header.sequence_nr;

        assert_ne!(
            seq1, seq2,
            "two replies on the same reliable stream must not both carry sequence_nr=0"
        );
        assert_eq!(
            seq2,
            seq1.next(),
            "sequence numbers increment by one per message sent on that stream"
        );
    }

    /// §8.4.14.1's sender state machine: a NACK'd sequence number must be
    /// repaired by resending the *exact* buffered bytes for that
    /// sequence number, not just acknowledging the ACKNACK — previously
    /// `handle_acknack` only updated bookkeeping and never retransmitted
    /// anything.
    #[test]
    fn acknack_nack_triggers_retransmission_of_the_exact_buffered_bytes() {
        let mut e = engine();
        let client = [13; CLIENT_KEY_LEN];
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            Duration::ZERO,
        );
        let sm1 = create_object_sm(
            participant_oid(),
            "<dds><participant><domain_id>0</domain_id></participant></dds>",
            CreationMode::default(),
        );
        let out1 = e.handle_message(Some(client), &reliable_header(0), &[sm1], Duration::ZERO);
        assert_eq!(out1.len(), 1);
        let seq1 = out1[0].header.sequence_nr;

        // The client claims seq1 is still missing (bit 0 set, base = seq1).
        let ack = AckNackPayload {
            first_unacked_seq_num: seq1.raw() as i16,
            nack_bitmap: 1u16.to_le_bytes(),
            stream_id: 0x80,
        };
        let ack_sm = ack.into_submessage().unwrap();
        let out2 = e.handle_message(Some(client), &session_header(), &[ack_sm], Duration::ZERO);
        assert_eq!(
            out2.len(),
            1,
            "a NACK'd sequence number must produce exactly one retransmission"
        );
        assert_eq!(
            out2[0].header.sequence_nr, seq1,
            "the retransmission repeats the original, NACK'd sequence number"
        );
        assert_eq!(out2[0].header.stream_id, StreamId(0x80));
        assert_eq!(
            out2[0].submessages, out1[0].submessages,
            "retransmission resends the exact original submessage bytes"
        );
    }

    /// DDS-XRCE 1.0 §8.3.5.12: the HEARTBEAT of a reliable output stream
    /// goes on STREAMID_NONE and names the stream in its payload.
    #[test]
    fn a_heartbeat_of_the_agent_is_on_streamid_none() {
        let mut e = engine();
        let client = [16; CLIENT_KEY_LEN];
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm_with_mtu(client, 0x81, XRCE_VERSION, None)],
            Duration::ZERO,
        );
        create_participant_topic_reader(&mut e, client);
        let reader = ObjectId::new(0x002, ObjectKind::DataReader).unwrap();
        let read_sm = ReadDataPayload {
            base: BaseObjectRequest {
                request_id: [0, 9],
                object_id: reader,
            },
            read_specification: ReadSpecification {
                preferred_stream_id: 0x80,
                data_format: DataFormat::Data,
                content_filter_expression: None,
                delivery_control: Some(DataDeliveryControl {
                    max_samples: zerodds_xrce::MAX_SAMPLES_UNLIMITED,
                    max_elapsed_time: 0,
                    max_bytes_per_second: 0,
                    min_pace_period: 0,
                }),
            },
        }
        .into_submessage()
        .unwrap();
        e.handle_message(Some(client), &session_header(), &[read_sm], Duration::ZERO);
        e.submit_sample(client, reader, vec![1, 2, 3, 4]).unwrap();
        let data = e.tick(Duration::ZERO);
        assert!(
            data.iter().any(|ob| ob.header.stream_id == StreamId(0x80)),
            "the sample goes on the reliable stream: {data:?}"
        );
        let out = e.tick(Duration::from_secs(1));
        let hb = out
            .iter()
            .find(|ob| ob.submessages[0].header.submessage_id == SubmessageId::Heartbeat)
            .expect("a HEARTBEAT for the unacknowledged DATA");
        assert_eq!(hb.header.stream_id, StreamId::NONE);
        let payload = HeartbeatPayload::try_from_submessage(&hb.submessages[0]).unwrap();
        assert_eq!(payload.stream_id, 0x80);
    }

    /// A sample too big for the client's negotiated MTU (spec item 1's
    /// outbound half: WRITE_DATA/DATA above the MTU) used to be sent as
    /// one oversized `DATA` submessage regardless of size —
    /// `DataPayload::into_submessage`'s only failure mode is the 16-bit
    /// submessage-length cap (~65535 bytes), far above any real MTU, so
    /// it never actually caught this. On a reliable read stream it must
    /// now split into `FRAGMENT` submessages that reassemble back to the
    /// exact original bytes.
    #[test]
    fn a_sample_above_the_clients_mtu_on_a_reliable_stream_splits_into_fragments() {
        let mut e = engine();
        let client = [15; CLIENT_KEY_LEN];
        let mtu = 32u16; // header(4) + FRAGMENT header(4) = 8B overhead -> 24B/fragment
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm_with_mtu(
                client,
                0x81,
                XRCE_VERSION,
                Some(mtu),
            )],
            Duration::ZERO,
        );
        create_participant_topic_reader(&mut e, client);
        let reader = ObjectId::new(0x002, ObjectKind::DataReader).unwrap();
        let read_sm = ReadDataPayload {
            base: BaseObjectRequest {
                request_id: [0, 9],
                object_id: reader,
            },
            read_specification: ReadSpecification {
                preferred_stream_id: 0x80, // reliable -- FRAGMENT is reliable-stream-only
                data_format: DataFormat::Data,
                content_filter_expression: None,
                delivery_control: Some(DataDeliveryControl {
                    max_samples: zerodds_xrce::MAX_SAMPLES_UNLIMITED,
                    max_elapsed_time: 0,
                    max_bytes_per_second: 0,
                    min_pace_period: 0,
                }),
            },
        }
        .into_submessage()
        .unwrap();
        e.handle_message(Some(client), &session_header(), &[read_sm], Duration::ZERO);

        let big_payload = vec![0xABu8; 100];
        e.submit_sample(client, reader, big_payload.clone())
            .unwrap();
        let out = e.tick(Duration::ZERO);

        assert!(
            out.len() > 1,
            "a 100-byte sample must not fit a 32-byte MTU as one message"
        );
        for ob in &out {
            assert_eq!(ob.submessages.len(), 1);
            assert_eq!(
                ob.submessages[0].header.submessage_id,
                SubmessageId::Fragment,
                "every piece of an oversized DATA must be a FRAGMENT submessage"
            );
        }
        let (last, rest) = out.split_last().unwrap();
        for ob in rest {
            assert_eq!(
                ob.submessages[0].header.flags
                    & zerodds_xrce::submessages::fragment::FRAGMENT_FLAG_LAST,
                0,
                "only the final fragment may carry the LAST flag"
            );
        }
        assert_ne!(
            last.submessages[0].header.flags
                & zerodds_xrce::submessages::fragment::FRAGMENT_FLAG_LAST,
            0,
            "the final fragment must carry the LAST flag"
        );

        // Reassemble and check the original DATA payload round-trips
        // exactly.
        let mut reassembled = Vec::new();
        for ob in &out {
            let frag = FragmentPayload::try_from_submessage(&ob.submessages[0]).unwrap();
            reassembled.extend_from_slice(&frag.data);
        }
        let decoded = decode_submessages(&reassembled);
        assert_eq!(decoded.len(), 1);
        let data = DataPayload::try_from_submessage(&decoded[0]).unwrap();
        assert_eq!(data.serialized_data, big_payload);
    }

    /// A `DdsPort` that records which topics were created with a known
    /// type.
    #[derive(Default)]
    struct TypeRecordingPort {
        inner: NullDdsPort,
        topics: Vec<(String, Option<String>)>,
        /// Each Publisher (`true`) or Subscriber (`false`) created, with
        /// its participant and spec.
        containers: Vec<(bool, ClientKeyOrd, ObjectId, ContainerSpec)>,
        /// Every key `enable` was called for, in order.
        enabled: Vec<crate::daemon::dds_port::ObjectKey>,
        /// Whether `enable` fails.
        fail_enable: bool,
    }

    impl DdsPort for TypeRecordingPort {
        fn enable(&mut self, key: crate::daemon::dds_port::ObjectKey) -> Result<(), DdsPortError> {
            self.enabled.push(key);
            if self.fail_enable {
                return Err(DdsPortError::DdsError("enable refused".into()));
            }
            Ok(())
        }
        fn create_participant(
            &mut self,
            key: crate::daemon::dds_port::ObjectKey,
            domain_id: i32,
        ) -> Result<(), DdsPortError> {
            self.inner.create_participant(key, domain_id)
        }
        fn create_topic(
            &mut self,
            key: crate::daemon::dds_port::ObjectKey,
            participant: crate::daemon::dds_port::ObjectKey,
            spec: TopicSpec,
        ) -> Result<(), DdsPortError> {
            let root_name = spec.known_type.as_ref().and_then(|k| match &k.root {
                zerodds_types::CompleteTypeObject::Struct(s) => {
                    Some(s.header.detail.type_name.clone())
                }
                _ => None,
            });
            self.topics.push((spec.topic_name.clone(), root_name));
            self.inner.create_topic(key, participant, spec)
        }
        fn create_publisher(
            &mut self,
            key: crate::daemon::dds_port::ObjectKey,
            participant: crate::daemon::dds_port::ObjectKey,
            spec: ContainerSpec,
        ) -> Result<(), DdsPortError> {
            self.containers
                .push((true, key.0, participant.1, spec.clone()));
            self.inner.create_publisher(key, participant, spec)
        }
        fn create_subscriber(
            &mut self,
            key: crate::daemon::dds_port::ObjectKey,
            participant: crate::daemon::dds_port::ObjectKey,
            spec: ContainerSpec,
        ) -> Result<(), DdsPortError> {
            self.containers
                .push((false, key.0, participant.1, spec.clone()));
            self.inner.create_subscriber(key, participant, spec)
        }
        fn create_writer(
            &mut self,
            key: crate::daemon::dds_port::ObjectKey,
            topic: crate::daemon::dds_port::ObjectKey,
            publisher: crate::daemon::dds_port::ObjectKey,
            qos: crate::daemon::dds_port::EndpointQos,
        ) -> Result<(), DdsPortError> {
            self.inner.create_writer(key, topic, publisher, qos)
        }
        fn create_reader(
            &mut self,
            key: crate::daemon::dds_port::ObjectKey,
            topic: crate::daemon::dds_port::ObjectKey,
            subscriber: crate::daemon::dds_port::ObjectKey,
            qos: crate::daemon::dds_port::EndpointQos,
        ) -> Result<(), DdsPortError> {
            self.inner.create_reader(key, topic, subscriber, qos)
        }
        fn write(
            &mut self,
            writer: crate::daemon::dds_port::ObjectKey,
            payload: &[u8],
            big_endian: bool,
            dispose: bool,
            unregister: bool,
            source_time: Option<SourceTime>,
        ) -> Result<crate::daemon::dds_port::WriteOutcome, DdsPortError> {
            self.inner.write(
                writer,
                payload,
                big_endian,
                dispose,
                unregister,
                source_time,
            )
        }
        fn sample_length(
            &mut self,
            writer: crate::daemon::dds_port::ObjectKey,
            bytes: &[u8],
            big_endian: bool,
        ) -> Result<usize, DdsPortError> {
            self.inner.sample_length(writer, bytes, big_endian)
        }
        fn set_read_filter(
            &mut self,
            reader: crate::daemon::dds_port::ObjectKey,
            filter: Option<&str>,
        ) -> Result<(), DdsPortError> {
            self.inner.set_read_filter(reader, filter)
        }
        fn poll_samples(
            &mut self,
            reader: crate::daemon::dds_port::ObjectKey,
            max: usize,
        ) -> Vec<crate::daemon::dds_port::PolledSample> {
            self.inner.poll_samples(reader, max)
        }
        fn delete(&mut self, key: crate::daemon::dds_port::ObjectKey) {
            self.inner.delete(key);
        }
        fn unregister_client(&mut self, client_key: [u8; CLIENT_KEY_LEN]) {
            self.inner.unregister_client(client_key);
        }
    }

    /// A topic gets the type its own type name names, not whatever type
    /// the client happened to define: with one `OBJK_TYPE` for `A`, a
    /// topic of type `B` used to be created with `A`'s TypeObject.
    #[test]
    fn a_topic_gets_the_type_its_type_name_names() {
        let mut e = SessionEngine::new(TypeRecordingPort::default(), test_config());
        let client = [0x31; CLIENT_KEY_LEN];
        let now = Duration::ZERO;
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            now,
        );
        let creates = [
            (
                participant_oid(),
                "<dds><participant><domain_id>0</domain_id></participant></dds>".to_string(),
            ),
            (
                ObjectId::new(0x002, ObjectKind::Type).unwrap(),
                "<type><struct name=\"A\"><member name=\"id\" type=\"int32\" key=\"true\"/>\
                 </struct></type>"
                    .to_string(),
            ),
            (
                ObjectId::new(0x003, ObjectKind::Topic).unwrap(),
                "<dds><topic name=\"X\" type_name=\"A\"/></dds>".to_string(),
            ),
            (
                ObjectId::new(0x004, ObjectKind::Topic).unwrap(),
                "<dds><topic name=\"Y\" type_name=\"B\"/></dds>".to_string(),
            ),
        ];
        for (oid, xml) in &creates {
            e.handle_message(
                Some(client),
                &session_header(),
                &[create_object_sm(*oid, xml, CreationMode::default())],
                now,
            );
        }
        let refused: Vec<_> = e
            .drain_events()
            .into_iter()
            .filter(|ev| matches!(ev, SessionEvent::CreateRefused { .. }))
            .collect();
        assert!(refused.is_empty(), "{refused:?}");
        assert_eq!(
            e.dds_port_mut().topics,
            vec![
                ("X".to_string(), Some("A".to_string())),
                ("Y".to_string(), None),
            ]
        );
    }

    /// Annex B.2.3, byte for byte: the CREATE of a DataWriter in binary for
    /// the topic "Square" with default QoS, in the Publisher {0xBB, 0xB3},
    /// object_id {0xDD, 0xD5}. The topic is found by its name.
    #[test]
    fn annex_b_2_3_create_of_a_binary_data_writer() {
        let mut e = engine();
        let client = [0x36; CLIENT_KEY_LEN];
        connect(&mut e, client);
        let publisher = ObjectId::from_raw(0xBBB3);
        for sm in [
            create_object_sm(
                participant_oid(),
                "<dds><participant><domain_id>0</domain_id></participant></dds>",
                CreationMode::default(),
            ),
            create_object_sm(
                ObjectId::new(0x003, ObjectKind::Topic).unwrap(),
                "<dds><topic name=\"Square\" type_name=\"ShapeType\"/></dds>",
                CreationMode::default(),
            ),
            create_under_sm(
                publisher,
                &ObjectVariant::ByXmlString(String::new()),
                CreationMode::default(),
                participant_oid(),
            ),
        ] {
            let out = e.handle_message(Some(client), &session_header(), &[sm], Duration::ZERO);
            assert_eq!(status_of(&out), ResultStatusCode::Ok);
        }
        // request_id {0xAA, 0x01}, object_id {0xDD, 0xD5}, then the
        // ObjectVariant of the example: kind, format, padding, the DHEADER
        // 0x8000000C, the name, qos? = FALSE, and the publisher_id.
        let mut body = vec![0xAA, 0x01, 0xDD, 0xD5, 0x05, 0x03, 0x00, 0x00];
        body.extend_from_slice(&[0x0C, 0x00, 0x00, 0x80, 0x07, 0x00, 0x00, 0x00]);
        body.extend_from_slice(b"Square\0");
        body.extend_from_slice(&[0x00, 0xBB, 0xB3]);
        let sm = CreatePayload {
            representation: body,
            reuse: false,
            replace: false,
        }
        .into_submessage()
        .unwrap();
        let out = e.handle_message(Some(client), &session_header(), &[sm], Duration::ZERO);
        assert_eq!(status_of(&out), ResultStatusCode::Ok);
        assert!(
            e.agent
                .get_object(ClientKey(client), ObjectId::from_raw(0xDDD5))
                .is_some()
        );
    }

    /// Annex B.2.4, byte for byte: the CREATE of a DataWriter with a QoS in
    /// binary, object_id {0xDD, 0xD5}, Publisher {0xBB, 0xB3}. The padding
    /// octet before the `publisher_id` (byte 43) is outside the DHEADER (23)
    /// in this reading; the deadline's presence octet is byte 35, which the
    /// table calls padding.
    #[test]
    fn annex_b_2_4_create_of_a_binary_data_writer_with_qos() {
        let mut e = engine();
        let client = [0x37; CLIENT_KEY_LEN];
        connect(&mut e, client);
        let publisher = ObjectId::from_raw(0xBBB3);
        for sm in [
            create_object_sm(
                participant_oid(),
                "<dds><participant><domain_id>0</domain_id></participant></dds>",
                CreationMode::default(),
            ),
            create_object_sm(
                ObjectId::new(0x003, ObjectKind::Topic).unwrap(),
                "<dds><topic name=\"Square\" type_name=\"ShapeType\"/></dds>",
                CreationMode::default(),
            ),
            create_under_sm(
                publisher,
                &ObjectVariant::ByXmlString(String::new()),
                CreationMode::default(),
                participant_oid(),
            ),
        ] {
            let out = e.handle_message(Some(client), &session_header(), &[sm], Duration::ZERO);
            assert_eq!(status_of(&out), ResultStatusCode::Ok);
        }
        let mut body = vec![0xAA, 0x01, 0xDD, 0xD5, 0x05, 0x03, 0x00, 0x00];
        body.extend_from_slice(&[0x17, 0x00, 0x00, 0x80, 0x07, 0x00, 0x00, 0x00]);
        body.extend_from_slice(b"Square\0");
        body.extend_from_slice(&[
            0x01, 0x03, 0x00, 0x00, 0x01, 0xC0, 0xD4, 0x01, 0x00, 0x00, 0x00, 0x00,
        ]);
        body.extend_from_slice(&[0x00, 0xBB, 0xB3]); // padding, publisher_id
        assert_eq!(body.len(), 38, "bytes 8 to 45 of the example");
        let sm = CreatePayload {
            representation: body,
            reuse: false,
            replace: false,
        }
        .into_submessage()
        .unwrap();
        let out = e.handle_message(Some(client), &session_header(), &[sm], Duration::ZERO);
        assert_eq!(status_of(&out), ResultStatusCode::Ok);
        assert!(
            e.agent
                .get_object(ClientKey(client), ObjectId::from_raw(0xDDD5))
                .is_some()
        );
    }

    /// §7.7.3.7.3: a binary topic may name its type by `type_identifier`
    /// alone; the agent finds the type it knows with that TypeIdentifier. A
    /// TypeIdentifier no known type has is STATUS_ERR_UNKNOWN_REFERENCE.
    #[test]
    fn a_topic_can_name_its_type_by_type_identifier() {
        use zerodds_wire::{Endianness as WireEndianness, Writer};
        let ast = std::sync::Arc::new(
            zerodds_wire_probe::encode::model_from_idl(
                "@final struct Ident { @key long id; };",
                "session-type-identifier.idl",
            )
            .unwrap(),
        );
        let ty = zerodds_types::dynamic::DynamicType::find(&ast, "Ident").unwrap();
        let root = zerodds_types::from_ast::complete_type_object(&ast, ty.id()).unwrap();
        let identifier = zerodds_types::hash::to_hashed_type_identifier(
            &zerodds_types::TypeObject::Complete(root.clone()),
        )
        .unwrap();
        let topic_blob = |identifier: &zerodds_types::TypeIdentifier| {
            // OBJK_Topic_Binary of the clause: topic_name, no type_reference,
            // a type_identifier.
            let mut w = Writer::with_max_align(WireEndianness::Little, 4);
            w.write_string("T");
            w.write_bool(false);
            w.write_bool(true);
            identifier.encode_into(&mut w).unwrap();
            w.finish()
        };
        let mut known = BTreeMap::new();
        known.insert("Ident".to_string(), root);
        let config = SessionEngineConfig {
            known_types: known,
            ..test_config()
        };
        let mut e = SessionEngine::new(TypeRecordingPort::default(), config);
        let client = [0x34; CLIENT_KEY_LEN];
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            Duration::ZERO,
        );
        e.handle_message(
            Some(client),
            &session_header(),
            &[create_object_sm(
                participant_oid(),
                "<dds><participant><domain_id>0</domain_id></participant></dds>",
                CreationMode::default(),
            )],
            Duration::ZERO,
        );
        let topic = ObjectId::new(0x003, ObjectKind::Topic).unwrap();
        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[create_variant_sm(
                topic,
                &ObjectVariant::InBinary(topic_blob(&identifier)),
                CreationMode::default(),
            )],
            Duration::ZERO,
        );
        assert_eq!(status_of(&out), ResultStatusCode::Ok);
        assert_eq!(
            e.dds_port_mut().topics,
            vec![("T".to_string(), Some("Ident".to_string()))]
        );
        // A TypeIdentifier that no known type has.
        let other = ObjectId::new(0x004, ObjectKind::Topic).unwrap();
        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[create_variant_sm(
                other,
                &ObjectVariant::InBinary(topic_blob(
                    &zerodds_types::TypeIdentifier::String8Small { bound: 16 },
                )),
                CreationMode::default(),
            )],
            Duration::ZERO,
        );
        assert_eq!(status_of(&out), ResultStatusCode::UnknownReference);
    }

    fn reply_ids(out: &[Outbound]) -> ([u8; 2], ObjectId) {
        assert_eq!(out.len(), 1, "one reply expected");
        let reply = StatusPayload::try_from_submessage(&out[0].submessages[0]).unwrap();
        let (reply, _) = BaseObjectReply::decode(&reply.representation).unwrap();
        (
            reply.related_request.request_id,
            reply.related_request.object_id,
        )
    }

    /// §8.3.5.2: "The XRCE Agent shall send a STATUS submessage in
    /// response". A CREATE too short to hold a BaseObjectRequest is answered
    /// with STATUS_ERR_INVALID_DATA (the request_id when its two octets are
    /// there) and reported.
    #[test]
    fn a_create_without_a_base_object_request_is_answered() {
        let mut e = engine();
        let client = [0x65; CLIENT_KEY_LEN];
        connected(&mut e, client, 0x81);
        e.drain_events();
        for (representation, request_id) in [
            (vec![], [0, 0]),
            (vec![0x01], [0, 0]),
            (vec![0, 9, 7], [0, 9]),
        ] {
            let sm = CreatePayload {
                representation,
                reuse: false,
                replace: false,
            }
            .into_submessage()
            .unwrap();
            let out = e.handle_message(Some(client), &session_header(), &[sm], Duration::ZERO);
            assert_eq!(status_of(&out), ResultStatusCode::InvalidData);
            assert_eq!(reply_ids(&out), (request_id, OBJECTID_INVALID));
        }
        assert_eq!(
            e.drain_events()
                .iter()
                .filter(|ev| matches!(ev, SessionEvent::CreateRefused { .. }))
                .count(),
            3
        );
    }

    /// §7.8.3.1: "The combination of the objectid_prefix and the ObjectKind
    /// contained in the object_representation discriminator shall be used to
    /// construct the 'input' ObjectId." A request whose object_id has another
    /// kind in its low bits creates the object of the discriminator's kind
    /// under the same prefix; the STATUS names that object_id.
    #[test]
    fn the_object_id_is_the_prefix_and_the_kind_of_the_representation() {
        let mut e = engine();
        let client = [0x66; CLIENT_KEY_LEN];
        connected(&mut e, client, 0x81);
        let mut body = BaseObjectRequest {
            request_id: [0, 4],
            // Prefix 0x001, kind OBJK_PUBLISHER.
            object_id: publisher_oid(),
        }
        .encode()
        .to_vec();
        body.extend_from_slice(
            &ObjectVariant::ByXmlString(
                "<dds><participant><domain_id>0</domain_id></participant></dds>".to_string(),
            )
            .encode_for_kind(
                ObjectKind::Participant,
                ObjectVariantExtra {
                    domain_id: Some(0),
                    parent: None,
                },
                zerodds_xrce::encoding::Endianness::Little,
            )
            .unwrap(),
        );
        let sm = CreatePayload {
            representation: body,
            reuse: false,
            replace: false,
        }
        .into_submessage()
        .unwrap();
        let out = e.handle_message(Some(client), &session_header(), &[sm], Duration::ZERO);
        assert_eq!(status_of(&out), ResultStatusCode::Ok);
        assert_eq!(reply_ids(&out), ([0, 4], participant_oid()));
        assert!(
            e.agent
                .get_object(ClientKey(client), participant_oid())
                .is_some()
        );
        assert!(
            e.agent
                .get_object(ClientKey(client), publisher_oid())
                .is_none()
        );
    }

    /// Table 5: the representation that is compared includes the container
    /// the object is created under. The same XML under another participant
    /// is a mismatch, under the same one a match.
    #[test]
    fn reuse_compares_the_container_of_the_representation() {
        let mut e = engine();
        let client = [0x67; CLIENT_KEY_LEN];
        connected(&mut e, client, 0x81);
        let second = ObjectId::new(0x002, ObjectKind::Participant).unwrap();
        let topic = ObjectId::new(0x003, ObjectKind::Topic).unwrap();
        let xml =
            ObjectVariant::ByXmlString("<dds><topic name=\"T\" type_name=\"Ty\"/></dds>".into());
        for sm in [
            participant_sm(1),
            participant_sm(2),
            create_under_sm(topic, &xml, CreationMode::default(), participant_oid()),
        ] {
            let out = e.handle_message(Some(client), &session_header(), &[sm], Duration::ZERO);
            assert_eq!(status_of(&out), ResultStatusCode::Ok);
        }
        for (parent, expected) in [
            (participant_oid(), ResultStatusCode::OkMatched),
            (second, ResultStatusCode::Mismatch),
        ] {
            let out = e.handle_message(
                Some(client),
                &session_header(),
                &[create_under_sm(topic, &xml, CreationMode::REUSE, parent)],
                Duration::ZERO,
            );
            assert_eq!(status_of(&out), expected, "parent {parent:?}");
        }
    }

    /// §7.8.3.1: the DDS entities are enabled "only after it has been
    /// determined that the creation has succeeded". The session enables an
    /// object once it is created, and an object whose enabling fails is not
    /// created: STATUS_ERR_DDS_ERROR, nothing stored, the DDS side deleted.
    #[test]
    fn a_dds_entity_is_enabled_when_the_creation_has_succeeded() {
        let mut e = SessionEngine::new(TypeRecordingPort::default(), test_config());
        let client = [0x69; CLIENT_KEY_LEN];
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            Duration::ZERO,
        );
        let topic = ObjectId::new(0x003, ObjectKind::Topic).unwrap();
        for sm in [
            participant_sm(1),
            create_object_sm(
                topic,
                "<dds><topic name=\"T\" type_name=\"Ty\"/></dds>",
                CreationMode::default(),
            ),
            create_object_sm(publisher_oid(), "", CreationMode::default()),
            create_object_sm(
                writer_oid(),
                &format!(
                    "<dds><data_writer topic_ref=\"0x{:04x}\"/></dds>",
                    topic.raw()
                ),
                CreationMode::default(),
            ),
        ] {
            let out = e.handle_message(Some(client), &session_header(), &[sm], Duration::ZERO);
            assert_eq!(status_of(&out), ResultStatusCode::Ok);
        }
        assert_eq!(
            e.dds_port_mut().enabled,
            vec![
                (client, participant_oid()),
                (client, topic),
                (client, publisher_oid()),
                (client, writer_oid()),
            ]
        );

        e.dds_port_mut().fail_enable = true;
        let second = ObjectId::new(0x009, ObjectKind::DataWriter).unwrap();
        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[create_object_sm(
                second,
                &format!(
                    "<dds><data_writer topic_ref=\"0x{:04x}\"/></dds>",
                    topic.raw()
                ),
                CreationMode::default(),
            )],
            Duration::ZERO,
        );
        assert_eq!(status_of(&out), ResultStatusCode::DdsError);
        assert!(e.agent.get_object(ClientKey(client), second).is_none());
    }

    fn create_with_mode(
        e: &mut SessionEngine<TypeRecordingPort>,
        client: ClientKeyOrd,
        oid: ObjectId,
        xml_text: &str,
        mode: CreationMode,
    ) -> ResultStatusCode {
        let extra = ObjectVariantExtra {
            domain_id: None,
            parent: Some(participant_oid()),
        };
        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[create_sm_with_extra(oid, &xml(xml_text), mode, extra)],
            Duration::ZERO,
        );
        status_of(&out)
    }

    const UPDATE: CreationMode = CreationMode {
        reuse: true,
        replace: true,
    };

    /// Table 5, §8.4.13: a CREATE with `reuse` and `replace` is the update of
    /// an object. A representation that matches is `STATUS_OK_MATCHES` and
    /// changes nothing; one that does not match replaces the object
    /// (`STATUS_OK`), the DDS side included. The same XML under another
    /// participant does not match.
    #[test]
    fn a_create_with_reuse_and_replace_updates_the_object() {
        let mut e = recording_engine();
        let client = [0x91; CLIENT_KEY_LEN];
        open_session(&mut e, client);
        for n in [1u16, 2] {
            assert_eq!(
                create_as(
                    &mut e,
                    client,
                    ObjectId::new(n, ObjectKind::Participant).unwrap(),
                    xml("<dds><participant><domain_id>0</domain_id></participant></dds>"),
                    None
                ),
                ResultStatusCode::Ok
            );
        }
        let topic = ObjectId::new(0x0A1, ObjectKind::Topic).unwrap();
        let first = "<topic name=\"T\" type_name=\"One\"/>";
        assert_eq!(
            create_with_mode(&mut e, client, topic, first, CreationMode::default()),
            ResultStatusCode::Ok
        );
        assert_eq!(e.dds_port_mut().topics.len(), 1);
        // The same representation: matched, nothing done.
        assert_eq!(
            create_with_mode(&mut e, client, topic, first, UPDATE),
            ResultStatusCode::OkMatched
        );
        assert_eq!(e.dds_port_mut().topics.len(), 1);
        // Another type: the object is replaced.
        assert_eq!(
            create_with_mode(
                &mut e,
                client,
                topic,
                "<topic name=\"T\" type_name=\"Two\"/>",
                UPDATE
            ),
            ResultStatusCode::Ok
        );
        assert_eq!(e.topics[&(client, topic)].type_name, "Two");
        assert_eq!(e.dds_port_mut().topics.len(), 2);
        // Without `replace` the unequal one is a mismatch, as before.
        assert_eq!(
            create_with_mode(
                &mut e,
                client,
                topic,
                "<topic name=\"T\" type_name=\"Three\"/>",
                CreationMode::REUSE
            ),
            ResultStatusCode::Mismatch
        );
        assert_eq!(e.topics[&(client, topic)].type_name, "Two");
    }

    /// §7.8.3.2: the update of an object that does not exist is
    /// `STATUS_ERR_UNKNOWN_REFERENCE`; Table 5 creates it. The default is
    /// Table 5; `update_of_missing_object: refuse` the update.
    #[test]
    fn the_update_of_a_missing_object_creates_it_or_is_refused() {
        for (refuse, expected) in [
            (false, ResultStatusCode::Ok),
            (true, ResultStatusCode::UnknownReference),
        ] {
            let mut e = SessionEngine::new(
                TypeRecordingPort::default(),
                SessionEngineConfig {
                    refuse_update_of_missing_object: refuse,
                    ..test_config()
                },
            );
            let client = [0x92; CLIENT_KEY_LEN];
            open_session(&mut e, client);
            assert_eq!(
                create_as(
                    &mut e,
                    client,
                    participant_oid(),
                    xml("<dds><participant><domain_id>0</domain_id></participant></dds>"),
                    None
                ),
                ResultStatusCode::Ok
            );
            let topic = ObjectId::new(0x0A2, ObjectKind::Topic).unwrap();
            assert_eq!(
                create_with_mode(
                    &mut e,
                    client,
                    topic,
                    "<topic name=\"T\" type_name=\"One\"/>",
                    UPDATE
                ),
                expected,
                "refuse: {refuse}"
            );
            assert_eq!(
                e.agent.get_object(ClientKey(client), topic).is_some(),
                !refuse
            );
        }
    }

    /// §7.8.3.2: "If the object cannot be updated due to permission
    /// restrictions, STATUS_ERR_DENIED shall be returned." An update needs the
    /// `update` permission on the object; a replace without `reuse` needs
    /// `delete`.
    #[test]
    fn an_update_needs_the_permission_to_update() {
        for (granted, update_expected, replace_expected) in [
            (
                Operation::Update,
                ResultStatusCode::Ok,
                ResultStatusCode::Denied,
            ),
            (
                Operation::Delete,
                ResultStatusCode::Denied,
                ResultStatusCode::Ok,
            ),
        ] {
            let access = {
                let mut a = deny_all_but(KEY_A, vec![Operation::Create, granted]);
                a.authorized_clients.clear();
                a
            };
            let mut e = SessionEngine::new(
                TypeRecordingPort::default(),
                SessionEngineConfig {
                    access,
                    ..test_config()
                },
            );
            open_session(&mut e, KEY_A);
            assert_eq!(
                create_as(
                    &mut e,
                    KEY_A,
                    participant_oid(),
                    xml("<dds><participant><domain_id>0</domain_id></participant></dds>"),
                    None
                ),
                ResultStatusCode::Ok
            );
            let topic = ObjectId::new(0x0A3, ObjectKind::Topic).unwrap();
            assert_eq!(
                create_with_mode(
                    &mut e,
                    KEY_A,
                    topic,
                    "<topic name=\"T\" type_name=\"One\"/>",
                    CreationMode::default()
                ),
                ResultStatusCode::Ok
            );
            assert_eq!(
                create_with_mode(
                    &mut e,
                    KEY_A,
                    topic,
                    "<topic name=\"T\" type_name=\"Two\"/>",
                    UPDATE
                ),
                update_expected,
                "update with {granted:?}"
            );
            assert_eq!(
                create_with_mode(
                    &mut e,
                    KEY_A,
                    topic,
                    "<topic name=\"T\" type_name=\"Three\"/>",
                    CreationMode::REPLACE
                ),
                replace_expected,
                "replace with {granted:?}"
            );
        }
    }

    /// §7.8.3.2: "If an update is unsuccessful the referenced object shall
    /// return to its previous configuration." A replace whose new DDS side
    /// cannot be made leaves the old object, in the store and on the DDS
    /// side, and answers with the error.
    #[test]
    fn a_replace_that_fails_returns_the_object_to_its_previous_configuration() {
        let ast = alloc::sync::Arc::new(
            zerodds_wire_probe::encode::model_from_idl(
                "@final struct HelloWorld { unsigned long index; };",
                "session-replace.idl",
            )
            .unwrap(),
        );
        let ty = zerodds_types::dynamic::DynamicType::find(&ast, "HelloWorld").unwrap();
        let root = zerodds_types::from_ast::complete_type_object(&ast, ty.id()).unwrap();
        let mut config = SessionEngineConfig {
            forward_unknown_topic_types: false,
            ..test_config()
        };
        config.known_types.insert("HelloWorld".into(), root);
        let mut e = SessionEngine::new(TypeRecordingPort::default(), config);
        let client = [0x68; CLIENT_KEY_LEN];
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            Duration::ZERO,
        );
        let topic = ObjectId::new(0x003, ObjectKind::Topic).unwrap();
        let topic_xml =
            |type_name: &str| format!("<dds><topic name=\"T\" type_name=\"{type_name}\"/></dds>");
        for sm in [
            participant_sm(1),
            create_object_sm(topic, &topic_xml("HelloWorld"), CreationMode::default()),
        ] {
            let out = e.handle_message(Some(client), &session_header(), &[sm], Duration::ZERO);
            assert_eq!(status_of(&out), ResultStatusCode::Ok);
        }
        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[create_object_sm(
                topic,
                &topic_xml("Nobody"),
                CreationMode::REPLACE,
            )],
            Duration::ZERO,
        );
        assert_eq!(status_of(&out), ResultStatusCode::UnknownReference);
        let kept = e.agent.get_object(ClientKey(client), topic).unwrap();
        assert_eq!(
            kept.variant,
            ObjectVariant::ByXmlString(topic_xml("HelloWorld"))
        );
        assert_eq!(e.topics[&(client, topic)].type_name, "HelloWorld");
        // Created, deleted for the replace, created again.
        assert_eq!(e.dds_port_mut().topics.len(), 2);
        assert!(
            e.dds_port_mut()
                .topics
                .iter()
                .all(|(name, root)| name == "T" && root.as_deref() == Some("HelloWorld"))
        );
    }

    fn recording_engine() -> SessionEngine<TypeRecordingPort> {
        SessionEngine::new(TypeRecordingPort::default(), test_config())
    }

    fn open_session(e: &mut SessionEngine<TypeRecordingPort>, client: ClientKeyOrd) {
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            Duration::ZERO,
        );
    }

    /// Sends a CREATE of `oid` (under `parent` where the kind has one) with
    /// the representation `variant`, and returns the code of the STATUS.
    fn create_as(
        e: &mut SessionEngine<TypeRecordingPort>,
        client: ClientKeyOrd,
        oid: ObjectId,
        variant: ObjectVariant,
        parent: Option<ObjectId>,
    ) -> ResultStatusCode {
        let extra = match oid.kind().unwrap() {
            ObjectKind::Participant => ObjectVariantExtra {
                domain_id: Some(0),
                parent: None,
            },
            _ => ObjectVariantExtra {
                domain_id: None,
                parent,
            },
        };
        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[create_sm_with_extra(
                oid,
                &variant,
                CreationMode::default(),
                extra,
            )],
            Duration::ZERO,
        );
        status_of(&out)
    }

    fn xml(text: &str) -> ObjectVariant {
        ObjectVariant::ByXmlString(text.to_string())
    }

    const PARTICIPANT_LIBRARY: &str = r#"<domain_participant_library name="L">
        <domain_participant name="P" domain_id="3">
          <register_type name="T" type_ref="T"/>
          <topic name="Square" register_type_ref="T"/>
          <publisher name="Pub"><data_writer name="W" topic_ref="Square"/></publisher>
          <subscriber name="Sub"><data_reader name="R" topic_ref="Square"/></subscriber>
        </domain_participant></domain_participant_library>"#;

    /// §7.8.3.1: the creation of a DomainParticipant "shall also create the
    /// nested objects"; their ObjectIds are those of their names (§9.3), the
    /// DDS entities are made and enabled with it, and the domain is the one of
    /// the definition.
    #[test]
    fn a_participant_definition_creates_the_objects_inside_it() {
        let mut e = recording_engine();
        let client = [0x71; CLIENT_KEY_LEN];
        open_session(&mut e, client);
        let code = create_as(
            &mut e,
            client,
            participant_oid(),
            xml(PARTICIPANT_LIBRARY),
            None,
        );
        assert_eq!(code, ResultStatusCode::Ok);
        let derived = |name: &str, kind| ObjectId::from_reference(name, kind);
        for (name, kind) in [
            ("Square", ObjectKind::Topic),
            ("Pub", ObjectKind::Publisher),
            ("Sub", ObjectKind::Subscriber),
            ("W", ObjectKind::DataWriter),
            ("R", ObjectKind::DataReader),
        ] {
            assert!(
                e.agent
                    .get_object(ClientKey(client), derived(name, kind))
                    .is_some(),
                "{name}"
            );
        }
        assert_eq!(e.scopes[&(client, participant_oid())].domain, Some(3));
        let port = e.dds_port_mut();
        assert_eq!(port.topics, vec![("Square".to_string(), None)]);
        assert_eq!(port.containers.len(), 2);
        // Participant, topic, publisher, subscriber, writer, reader: all enabled.
        assert_eq!(port.enabled.len(), 6);
        assert!(
            port.enabled
                .contains(&(client, derived("W", ObjectKind::DataWriter)))
        );
    }

    /// A nested object that cannot be made is a failure of the whole: nothing
    /// of the participant is left, in the store or on the DDS side.
    #[test]
    fn a_participant_definition_with_a_broken_part_creates_nothing() {
        let mut e = recording_engine();
        let client = [0x72; CLIENT_KEY_LEN];
        open_session(&mut e, client);
        let code = create_as(
            &mut e,
            client,
            participant_oid(),
            xml(
                r#"<domain_participant_library name="B"><domain_participant name="P" domain_id="0">
                 <publisher name="Pub"><data_writer name="W" topic_ref="Missing"/></publisher>
               </domain_participant></domain_participant_library>"#,
            ),
            None,
        );
        assert_eq!(code, ResultStatusCode::InvalidData);
        assert_eq!(e.agent.object_count(ClientKey(client)).unwrap(), 0);
        assert!(e.participant_scopes.is_empty());
        assert!(e.dynamic_definitions.is_empty());
    }

    /// §7.7.3.7.1, §7.7.3.7.2: a topic is created by the bare name of one the
    /// DomainParticipant (or its Domain) defines, or from `<topic name=…
    /// register_type_ref=…>` with a registration of the participant.
    #[test]
    fn a_topic_is_created_by_name_or_by_register_type_ref() {
        let mut e = recording_engine();
        let client = [0x73; CLIENT_KEY_LEN];
        open_session(&mut e, client);
        assert_eq!(
            create_as(
                &mut e,
                client,
                participant_oid(),
                xml(PARTICIPANT_LIBRARY),
                None
            ),
            ResultStatusCode::Ok
        );
        let by_name = ObjectId::new(0x0A1, ObjectKind::Topic).unwrap();
        assert_eq!(
            create_as(
                &mut e,
                client,
                by_name,
                ObjectVariant::ByReference("Square".into()),
                Some(participant_oid())
            ),
            ResultStatusCode::Ok
        );
        assert_eq!(e.topics[&(client, by_name)].topic_name, "Square");
        assert_eq!(e.topics[&(client, by_name)].type_name, "T");
        let by_register = ObjectId::new(0x0A2, ObjectKind::Topic).unwrap();
        assert_eq!(
            create_as(
                &mut e,
                client,
                by_register,
                xml(r#"<topic name="Circle" register_type_ref="T"/>"#),
                Some(participant_oid())
            ),
            ResultStatusCode::Ok
        );
        assert_eq!(e.topics[&(client, by_register)].topic_name, "Circle");
        // A registration the participant does not hold, and a topic it does not
        // define, are refused.
        for variant in [
            xml(r#"<topic name="Triangle" register_type_ref="Nope"/>"#),
            ObjectVariant::ByReference("Triangle".into()),
        ] {
            let oid = ObjectId::new(0x0A3, ObjectKind::Topic).unwrap();
            assert_eq!(
                create_as(&mut e, client, oid, variant, Some(participant_oid())),
                ResultStatusCode::UnknownReference
            );
        }
    }

    /// §7.7.3.10.1: a DataWriter names its topic by name, and states its QoS
    /// as `<datawriter_qos base_name="…">` over a profile of the agent, with
    /// policies of its own on top.
    #[test]
    fn a_data_writer_states_its_qos_over_a_profile() {
        let mut config = test_config();
        config
            .profiles
            .add_profiles(
                r#"<qos_library name="Lib"><qos_profile name="P"><datawriter_qos>
                   <reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability>
                   <durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability>
                   </datawriter_qos></qos_profile></qos_library>"#,
            )
            .unwrap();
        let mut e = SessionEngine::new(TypeRecordingPort::default(), config);
        let client = [0x74; CLIENT_KEY_LEN];
        open_session(&mut e, client);
        assert_eq!(
            create_as(
                &mut e,
                client,
                participant_oid(),
                xml(PARTICIPANT_LIBRARY),
                None
            ),
            ResultStatusCode::Ok
        );
        let writer = ObjectId::new(0x0B1, ObjectKind::DataWriter).unwrap();
        let code = create_as(
            &mut e,
            client,
            writer,
            xml(r#"<data_writer name="W2" topic_ref="Square">
                     <datawriter_qos base_name="Lib::P">
                       <deadline><period><sec>120</sec></period></deadline>
                     </datawriter_qos></data_writer>"#),
            Some(ObjectId::from_reference("Pub", ObjectKind::Publisher)),
        );
        assert_eq!(code, ResultStatusCode::Ok);
        let DdsCreate::Endpoint { description, .. } = &e.plans[&(client, writer)] else {
            panic!("not an endpoint");
        };
        assert!(!description.qos.reliable, "from the profile");
        assert_eq!(
            description.qos.durability,
            crate::daemon::dds_port::Durability::TransientLocal
        );
        assert_eq!(description.qos.deadline, Some(Duration::from_secs(120)));
    }

    /// §7.7.3.8.1: the XML of a Publisher "allows specifying Qos policies and
    /// including nested DataWriter objects"; the CREATE of the Publisher makes
    /// them, and a DataWriter that fails fails the CREATE.
    #[test]
    fn a_publisher_with_nested_data_writers_creates_them() {
        let mut e = recording_engine();
        let client = [0x77; CLIENT_KEY_LEN];
        open_session(&mut e, client);
        assert_eq!(
            create_as(
                &mut e,
                client,
                participant_oid(),
                xml(r#"<domain_participant_library name="L">
                     <domain_participant name="P" domain_id="0">
                       <register_type name="T" type_ref="T"/>
                       <topic name="Square" register_type_ref="T"/>
                     </domain_participant></domain_participant_library>"#),
                None
            ),
            ResultStatusCode::Ok
        );
        let publisher = ObjectId::new(0x0F1, ObjectKind::Publisher).unwrap();
        let code = create_as(
            &mut e,
            client,
            publisher,
            xml(r#"<publisher name="MyPublisher">
                     <publisher_qos><partition><name><element>g1</element></name></partition></publisher_qos>
                     <data_writer name="MySquareWriter" topic_ref="Square"/>
                   </publisher>"#),
            Some(participant_oid()),
        );
        assert_eq!(code, ResultStatusCode::Ok);
        let writer = ObjectId::from_reference("MySquareWriter", ObjectKind::DataWriter);
        assert!(e.agent.get_object(ClientKey(client), writer).is_some());
        assert_eq!(
            e.containers[&(client, publisher)].partitions,
            vec!["g1".to_string()]
        );
        let DdsCreate::Endpoint {
            description,
            container,
            ..
        } = &e.plans[&(client, writer)]
        else {
            panic!("not an endpoint");
        };
        assert_eq!(*container, publisher);
        assert_eq!(description.qos.partitions, vec!["g1".to_string()]);

        // A nested DataWriter of a topic the participant does not hold fails
        // the whole CREATE.
        let broken = ObjectId::new(0x0F2, ObjectKind::Publisher).unwrap();
        let count = e.agent.object_count(ClientKey(client)).unwrap();
        let code = create_as(
            &mut e,
            client,
            broken,
            xml(
                r#"<publisher name="Other"><data_writer name="Lost" topic_ref="Missing"/></publisher>"#,
            ),
            Some(participant_oid()),
        );
        assert_eq!(code, ResultStatusCode::UnknownReference);
        assert_eq!(e.agent.object_count(ClientKey(client)).unwrap(), count);
        assert!(!e.containers.contains_key(&(client, broken)));
    }

    /// §7.7.3.3.2: the `string_representation` "may reference other Types
    /// already known to the Agent". A second `<types>` names the first one's
    /// type as a member; a name that is no type is STATUS_ERR_UNKNOWN_REFERENCE.
    #[test]
    fn a_types_string_references_a_type_created_before() {
        let mut e = recording_engine();
        let client = [0x79; CLIENT_KEY_LEN];
        open_session(&mut e, client);
        let ty = |n: u16| ObjectId::new(n, ObjectKind::Type).unwrap();
        let outer = |inner: &str| {
            xml(&format!(
                "<types><struct name=\"Outer\"><member name=\"b\" type=\"nonBasic\" \
                 nonBasicTypeName=\"{inner}\"/></struct></types>"
            ))
        };
        assert_eq!(
            create_as(&mut e, client, ty(1), outer("Base"), None),
            ResultStatusCode::UnknownReference,
            "Base is not known yet"
        );
        assert_eq!(
            create_as(
                &mut e,
                client,
                ty(2),
                xml(
                    "<types><struct name=\"Base\"><member name=\"x\" type=\"int32\"/></struct></types>"
                ),
                None
            ),
            ResultStatusCode::Ok
        );
        assert_eq!(
            create_as(&mut e, client, ty(1), outer("Base"), None),
            ResultStatusCode::Ok
        );
        assert_eq!(
            create_as(&mut e, client, ty(3), outer("Missing"), None),
            ResultStatusCode::UnknownReference
        );
    }

    /// §7.7.3.3.2: of several types in a `<types>` only one is not nested; it
    /// is the XRCE Type. §7.7.3.2.2: the `<qos_library>` of a QosProfile holds
    /// a single `<qos_profile>`, whose `base_name` may be written with one
    /// colon.
    #[test]
    fn type_and_qos_profile_xml_follow_their_clauses() {
        let mut e = recording_engine();
        let client = [0x78; CLIENT_KEY_LEN];
        open_session(&mut e, client);
        let ty = |n: u16| ObjectId::new(n, ObjectKind::Type).unwrap();
        let two = |second: &str| {
            xml(&format!(
                "<types><struct name=\"A\"><member name=\"x\" type=\"int32\"/></struct>\
                 <struct name=\"B\"{second}><member name=\"y\" type=\"int32\"/></struct></types>"
            ))
        };
        assert_eq!(
            create_as(&mut e, client, ty(1), two(""), None),
            ResultStatusCode::InvalidData,
            "two types that are not nested"
        );
        assert_eq!(
            create_as(&mut e, client, ty(2), two(" nested=\"true\""), None),
            ResultStatusCode::Ok
        );
        let profile = |n: u16| ObjectId::new(n, ObjectKind::QosProfile).unwrap();
        assert_eq!(
            create_as(
                &mut e,
                client,
                profile(1),
                xml(
                    r#"<qos_library name="Q"><qos_profile name="One"><datareader_qos>
                       <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
                       </datareader_qos></qos_profile></qos_library>"#
                ),
                None
            ),
            ResultStatusCode::Ok
        );
        // A second profile, based on the first with the spelling of the
        // clause (one colon), and a library of two profiles.
        assert_eq!(
            create_as(
                &mut e,
                client,
                profile(2),
                xml(
                    r#"<qos_library name="Q"><qos_profile name="Two" base_name="Q:One">
                       <datareader_qos><deadline><period><sec>5</sec></period></deadline>
                       </datareader_qos></qos_profile></qos_library>"#
                ),
                None
            ),
            ResultStatusCode::Ok
        );
        let resolved = e.profiles.qos_profile("Q::Two").unwrap();
        let reader = zerodds_xml::default_qos(&resolved.datareader_qos).unwrap();
        assert!(reader.reliability.is_some() && reader.deadline.is_some());
        assert_eq!(
            create_as(
                &mut e,
                client,
                profile(3),
                xml(
                    r#"<qos_library name="R"><qos_profile name="A"/><qos_profile name="B"/>
                       </qos_library>"#
                ),
                None
            ),
            ResultStatusCode::InvalidData
        );
    }

    /// §9.2: the QosProfile, Application and DomainParticipant a
    /// ConfigurationClient creates with XML are known to the agent, and
    /// another client refers to them with REPRESENTATION_BY_REFERENCE, for as
    /// long as the object of the ConfigurationClient exists.
    #[test]
    fn what_a_configuration_client_defines_is_referenced_by_another_client() {
        let mut e = recording_engine();
        let (config, user) = ([0x75; CLIENT_KEY_LEN], [0x76; CLIENT_KEY_LEN]);
        open_session(&mut e, config);
        open_session(&mut e, user);
        let profile = ObjectId::new(0x0C1, ObjectKind::QosProfile).unwrap();
        let application = ObjectId::new(0x0C2, ObjectKind::Application).unwrap();
        assert_eq!(
            create_as(
                &mut e,
                config,
                profile,
                xml(
                    r#"<qos_library name="Lib"><qos_profile name="P"><datawriter_qos>
                       <reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability>
                       </datawriter_qos></qos_profile></qos_library>"#
                ),
                None
            ),
            ResultStatusCode::Ok
        );
        assert_eq!(
            create_as(
                &mut e,
                config,
                application,
                xml(r#"<application_library name="Apps"><application name="App">
                       <domain_participant name="Part" domain_id="2">
                         <register_type name="T" type_ref="T"/>
                         <topic name="Square" register_type_ref="T"/>
                         <publisher name="Pub"><data_writer name="W" topic_ref="Square">
                           <datawriter_qos base_name="Lib::P"/></data_writer></publisher>
                       </domain_participant></application></application_library>"#),
                None
            ),
            ResultStatusCode::Ok
        );
        // The Application made its participant with the id of its reference.
        let participant = ObjectId::from_reference("Apps::App::Part", ObjectKind::Participant);
        assert!(e.agent.get_object(ClientKey(config), participant).is_some());

        // The other client has neither, and finds both by reference.
        let user_profile = ObjectId::new(0x0D1, ObjectKind::QosProfile).unwrap();
        let user_application = ObjectId::new(0x0D2, ObjectKind::Application).unwrap();
        assert_eq!(
            create_as(
                &mut e,
                user,
                user_profile,
                ObjectVariant::ByReference("Lib::P".into()),
                None
            ),
            ResultStatusCode::Ok
        );
        assert_eq!(
            create_as(
                &mut e,
                user,
                user_application,
                ObjectVariant::ByReference("Apps::App".into()),
                None
            ),
            ResultStatusCode::Ok
        );
        let writer = ObjectId::from_reference("W", ObjectKind::DataWriter);
        assert!(e.agent.get_object(ClientKey(user), participant).is_some());
        assert!(e.agent.get_object(ClientKey(user), writer).is_some());
        let DdsCreate::Endpoint { description, .. } = &e.plans[&(user, writer)] else {
            panic!("not an endpoint");
        };
        assert!(!description.qos.reliable, "the profile of the other client");

        // The ConfigurationClient deletes its objects: the definitions go.
        for oid in [profile, application] {
            let out = e.handle_message(
                Some(config),
                &session_header(),
                &[DeletePayload {
                    representation: BaseObjectRequest {
                        request_id: [0, 9],
                        object_id: oid,
                    }
                    .encode()
                    .to_vec(),
                }
                .into_submessage()
                .unwrap()],
                Duration::ZERO,
            );
            assert_eq!(status_of(&out), ResultStatusCode::Ok);
        }
        let third = ObjectId::new(0x0E1, ObjectKind::QosProfile).unwrap();
        assert_eq!(
            create_as(
                &mut e,
                user,
                third,
                ObjectVariant::ByReference("Lib::P".into()),
                None
            ),
            ResultStatusCode::UnknownReference
        );
        assert!(e.profiles.definitions().application("Apps::App").is_none());
    }

    /// A configuration file with an Application whose participant holds a
    /// topic, a DataWriter and a DataReader (DDS-XRCE 1.0 §9.3.1).
    const SYSTEM_FILE: &str = r#"<dds><application_library name="Apps">
        <application name="App">
          <domain_participant name="Part" domain_id="0">
            <register_type name="T" type_ref="T"/>
            <topic name="Square" register_type_ref="T"/>
            <publisher name="MyPublisher">
              <data_writer name="MySquareWriter" topic_ref="Square"/></publisher>
            <subscriber name="MySubscriber">
              <data_reader name="MySquareReader" topic_ref="Square"/></subscriber>
          </domain_participant></application></application_library></dds>"#;

    fn system_engine(
        client_applications: Vec<(ClientKeyOrd, String)>,
        access: crate::daemon::access::AccessController,
    ) -> SessionEngine<TypeRecordingPort> {
        let mut profiles = AgentProfiles::default();
        profiles.add_profiles(SYSTEM_FILE).unwrap();
        SessionEngine::new(
            TypeRecordingPort::default(),
            SessionEngineConfig {
                profiles,
                client_applications,
                access,
                ..test_config()
            },
        )
    }

    fn write_to(
        e: &mut SessionEngine<TypeRecordingPort>,
        client: ClientKeyOrd,
        object: ObjectId,
    ) -> Vec<Outbound> {
        let write = zerodds_xrce::submessages::write_data::WriteDataPayload {
            base: BaseObjectRequest {
                request_id: [0, 1],
                object_id: object,
            },
            serialized_data: vec![1, 2, 3, 4],
        }
        .into_submessage()
        .unwrap();
        e.handle_message(Some(client), &session_header(), &[write], Duration::ZERO)
    }

    /// §7.3, §8.4.2: "a Client that has access to this resource can write data
    /// using this resource simply by referring to the existing
    /// 'MySquareWriter'. The Client does not have to create a resource." The
    /// WRITE_DATA names the ObjectId of the name (§9.3); the agent makes the
    /// DomainParticipant with everything inside it, and the write succeeds.
    #[test]
    fn a_client_writes_through_a_preconfigured_data_writer_without_a_create() {
        let mut e = system_engine(
            Vec::new(),
            crate::daemon::access::AccessController::default(),
        );
        let client = [0x81; CLIENT_KEY_LEN];
        open_session(&mut e, client);
        assert_eq!(e.agent.object_count(ClientKey(client)).unwrap(), 0);
        let writer = ObjectId::from_reference("MySquareWriter", ObjectKind::DataWriter);
        let out = write_to(&mut e, client, writer);
        assert_eq!(status_of(&out), ResultStatusCode::Ok);
        for (name, kind) in [
            ("Apps::App::Part", ObjectKind::Participant),
            ("Square", ObjectKind::Topic),
            ("MyPublisher", ObjectKind::Publisher),
            ("MySquareWriter", ObjectKind::DataWriter),
            ("MySubscriber", ObjectKind::Subscriber),
            ("MySquareReader", ObjectKind::DataReader),
        ] {
            assert!(
                e.agent
                    .get_object(ClientKey(client), ObjectId::from_reference(name, kind))
                    .is_some(),
                "{name}"
            );
        }
        // A second write reuses them: one participant, one writer.
        assert_eq!(
            status_of(&write_to(&mut e, client, writer)),
            ResultStatusCode::Ok
        );
        assert_eq!(e.agent.object_count(ClientKey(client)).unwrap(), 6);
        // What no definition has is still an unknown object.
        let unknown = ObjectId::from_reference("NotConfigured", ObjectKind::DataWriter);
        assert_eq!(
            status_of(&write_to(&mut e, client, unknown)),
            ResultStatusCode::UnknownReference
        );
    }

    /// §8.4.3: a client reads through a preconfigured DataReader with
    /// READ_DATA, and DATA comes back with the request_id of the READ_DATA.
    #[test]
    fn a_client_reads_through_a_preconfigured_data_reader_without_a_create() {
        let mut e = system_engine(
            Vec::new(),
            crate::daemon::access::AccessController::default(),
        );
        let client = [0x82; CLIENT_KEY_LEN];
        open_session(&mut e, client);
        let reader = ObjectId::from_reference("MySquareReader", ObjectKind::DataReader);
        let first = e.handle_message(
            Some(client),
            &session_header(),
            &[read_sm(reader, 1, 0x01, None)],
            Duration::ZERO,
        );
        assert!(first.is_empty(), "nothing to read yet: {first:?}");
        assert!(e.agent.get_object(ClientKey(client), reader).is_some());
        e.submit_sample(client, reader, vec![9, 8, 7, 6]).unwrap();
        let second = e.handle_message(
            Some(client),
            &session_header(),
            &[read_sm(reader, 2, 0x01, None)],
            Duration::ZERO,
        );
        assert_eq!(data_of(&second), vec![(0x01, vec![9, 8, 7, 6])]);
    }

    /// §8.4.2: "An Agent has been pre-configured for a Client (identified by
    /// the ClientKey) [...]. The reception of the CREATE_CLIENT triggers the
    /// creation or reuse of the corresponding XRCE objects." A second
    /// CREATE_CLIENT of the same session reuses them.
    #[test]
    fn a_create_client_makes_the_objects_configured_for_the_client() {
        let (client, other) = ([0x83; CLIENT_KEY_LEN], [0x84; CLIENT_KEY_LEN]);
        let mut e = system_engine(
            vec![(client, "Apps::App".to_string())],
            crate::daemon::access::AccessController::default(),
        );
        open_session(&mut e, client);
        open_session(&mut e, other);
        let writer = ObjectId::from_reference("MySquareWriter", ObjectKind::DataWriter);
        // The Application and its participant, and what is inside.
        assert!(
            e.agent
                .get_object(
                    ClientKey(client),
                    ObjectId::from_reference("Apps::App", ObjectKind::Application)
                )
                .is_some()
        );
        assert!(e.agent.get_object(ClientKey(client), writer).is_some());
        let count = e.agent.object_count(ClientKey(client)).unwrap();
        assert_eq!(count, 7);
        // The other client was configured for nothing.
        assert_eq!(e.agent.object_count(ClientKey(other)).unwrap(), 0);
        // CREATE_CLIENT again, same session: reuse, nothing more.
        open_session(&mut e, client);
        assert_eq!(e.agent.object_count(ClientKey(client)).unwrap(), count);
        assert_eq!(
            e.dds_port_mut()
                .enabled
                .iter()
                .filter(|(c, o)| *c == client && *o == writer)
                .count(),
            1
        );
    }

    /// The AccessController (§7.4) decides on the objects made for a client
    /// that names them: without the permission to create, the WRITE_DATA is
    /// `STATUS_ERR_DENIED`.
    #[test]
    fn a_preconfigured_object_is_made_only_for_a_client_allowed_to_create_it() {
        let access = deny_all_but(KEY_A, vec![Operation::WriteData]);
        let mut e = system_engine(Vec::new(), access);
        open_session(&mut e, KEY_A);
        let writer = ObjectId::from_reference("MySquareWriter", ObjectKind::DataWriter);
        let out = write_to(&mut e, KEY_A, writer);
        assert_eq!(status_of(&out), ResultStatusCode::Denied);
        assert_eq!(e.agent.object_count(ClientKey(KEY_A)).unwrap(), 0);
    }

    /// §7.7.3.7.3: "The referenced type shall be known to the XRCE Agent".
    /// A topic whose type the agent does not know is STATUS_ERR_UNKNOWN_REFERENCE
    /// and nothing is stored (the default); with `forward_unknown_topic_types`
    /// it is created and its samples are forwarded opaque.
    #[test]
    fn a_topic_of_an_unknown_type_is_refused_when_the_agent_is_told_to() {
        for (refuse, expected) in [
            (true, ResultStatusCode::UnknownReference),
            (false, ResultStatusCode::Ok),
        ] {
            let mut e = SessionEngine::new(
                TypeRecordingPort::default(),
                SessionEngineConfig {
                    forward_unknown_topic_types: !refuse,
                    ..test_config()
                },
            );
            let client = [0x35; CLIENT_KEY_LEN];
            e.handle_message(
                None,
                &bootstrap_header(),
                &[create_client_sm(client, 0x81, XRCE_VERSION)],
                Duration::ZERO,
            );
            e.handle_message(
                Some(client),
                &session_header(),
                &[create_object_sm(
                    participant_oid(),
                    "<dds><participant><domain_id>0</domain_id></participant></dds>",
                    CreationMode::default(),
                )],
                Duration::ZERO,
            );
            let topic = ObjectId::new(0x003, ObjectKind::Topic).unwrap();
            let out = e.handle_message(
                Some(client),
                &session_header(),
                &[create_object_sm(
                    topic,
                    "<dds><topic name=\"X\" type_name=\"Nobody\"/></dds>",
                    CreationMode::default(),
                )],
                Duration::ZERO,
            );
            assert_eq!(status_of(&out), expected, "refuse: {refuse}");
            assert_eq!(
                e.agent.get_object(ClientKey(client), topic).is_some(),
                !refuse
            );
        }
    }

    /// QoS the agent cannot apply (here an element of a vendor dialect in a
    /// `<topic>`) is not accepted and ignored: the CREATE is
    /// STATUS_ERR_INCOMPATIBLE and nothing is stored; `unsupported_qos: ignore`
    /// creates the object without it.
    #[test]
    fn qos_the_agent_cannot_apply_is_refused_unless_ignoring_is_named() {
        for (ignore, expected) in [
            (false, ResultStatusCode::Incompatible),
            (true, ResultStatusCode::Ok),
        ] {
            let mut e = SessionEngine::new(
                TypeRecordingPort::default(),
                SessionEngineConfig {
                    ignore_unsupported_qos: ignore,
                    ..test_config()
                },
            );
            let client = [0x36; CLIENT_KEY_LEN];
            e.handle_message(
                None,
                &bootstrap_header(),
                &[create_client_sm(client, 0x81, XRCE_VERSION)],
                Duration::ZERO,
            );
            e.handle_message(
                Some(client),
                &session_header(),
                &[create_object_sm(
                    participant_oid(),
                    "<dds><participant><domain_id>0</domain_id></participant></dds>",
                    CreationMode::default(),
                )],
                Duration::ZERO,
            );
            let topic = ObjectId::new(0x003, ObjectKind::Topic).unwrap();
            let out = e.handle_message(
                Some(client),
                &session_header(),
                &[create_object_sm(
                    topic,
                    "<dds><topic name=\"X\" type_name=\"Nobody\">\
                     <historyMemoryPolicy>PREALLOCATED</historyMemoryPolicy></topic></dds>",
                    CreationMode::default(),
                )],
                Duration::ZERO,
            );
            assert_eq!(status_of(&out), expected, "ignore: {ignore}");
            assert_eq!(
                e.agent.get_object(ClientKey(client), topic).is_some(),
                ignore
            );
        }
    }

    /// `max_elapsed_time` of a DataDeliveryControl: Table 9 gives seconds, 0 is
    /// unlimited; the example of Annex B.4.2 (30000 for 30 seconds) reads as
    /// milliseconds, which is the setting `max_elapsed_time_unit: milliseconds`.
    #[test]
    fn max_elapsed_time_is_in_seconds_or_in_the_milliseconds_of_the_example() {
        assert_eq!(elapsed_time_limit(0, false), Duration::MAX);
        assert_eq!(elapsed_time_limit(0, true), Duration::MAX);
        assert_eq!(elapsed_time_limit(30, false), Duration::from_secs(30));
        assert_eq!(elapsed_time_limit(30_000, true), Duration::from_secs(30));
        assert_eq!(
            elapsed_time_limit(30_000, false),
            Duration::from_secs(30_000)
        );
        assert!(!SessionEngineConfig::default().max_elapsed_time_in_milliseconds);
    }

    /// §7.7.3.1.1: the object_reference is an `elementNameReference` of
    /// DDS-XML. One of another shape is STATUS_ERR_INVALID_DATA; a well-formed
    /// one the agent does not hold is STATUS_ERR_UNKNOWN_REFERENCE; white space
    /// at the edges is collapsed away.
    #[test]
    fn an_object_reference_must_be_an_element_name_reference() {
        let mut e = engine();
        let client = [0x73; CLIENT_KEY_LEN];
        connected(&mut e, client, 0x81);
        let create = |e: &mut SessionEngine<NullDdsPort>, name: &str| {
            let oid = ObjectId::new(0x00A, ObjectKind::Type).unwrap();
            let out = e.handle_message(
                Some(client),
                &session_header(),
                &[create_variant_sm(
                    oid,
                    &ObjectVariant::ByReference(name.into()),
                    CreationMode::default(),
                )],
                Duration::ZERO,
            );
            status_of(&out)
        };
        assert_eq!(create(&mut e, "My-Type"), ResultStatusCode::InvalidData);
        assert_eq!(create(&mut e, "a b"), ResultStatusCode::InvalidData);
        assert_eq!(create(&mut e, "Lib::"), ResultStatusCode::InvalidData);
        assert_eq!(create(&mut e, ""), ResultStatusCode::InvalidData);
        assert_eq!(
            create(&mut e, "Lib::Nobody"),
            ResultStatusCode::UnknownReference
        );
        assert_eq!(
            create(&mut e, "  Lib::Nobody \n"),
            ResultStatusCode::UnknownReference
        );
    }

    /// `type_files:` types are known to every client.
    #[test]
    fn a_configured_type_is_known_to_every_client() {
        let ast = alloc::sync::Arc::new(
            zerodds_wire_probe::encode::model_from_idl(
                "@final struct HelloWorld { unsigned long index; string message; };",
                "session-known-types.idl",
            )
            .unwrap(),
        );
        let ty = zerodds_types::dynamic::DynamicType::find(&ast, "HelloWorld").unwrap();
        let root = zerodds_types::from_ast::complete_type_object(&ast, ty.id()).unwrap();
        let mut config = test_config();
        config.known_types.insert("HelloWorld".into(), root);
        let mut e = SessionEngine::new(TypeRecordingPort::default(), config);
        let client = [0x32; CLIENT_KEY_LEN];
        let now = Duration::ZERO;
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            now,
        );
        for (oid, xml) in [
            (
                participant_oid(),
                "<dds><participant><domain_id>0</domain_id></participant></dds>",
            ),
            (
                ObjectId::new(0x003, ObjectKind::Topic).unwrap(),
                "<dds><topic><name>HelloWorldTopic</name><dataType>HelloWorld</dataType>\
                 </topic></dds>",
            ),
        ] {
            e.handle_message(
                Some(client),
                &session_header(),
                &[create_object_sm(oid, xml, CreationMode::default())],
                now,
            );
        }
        assert_eq!(
            e.dds_port_mut().topics,
            vec![(
                "HelloWorldTopic".to_string(),
                Some("HelloWorld".to_string())
            )]
        );
    }

    /// The eProsima ShapesDemo client creates everything by reference;
    /// `xml_profile_files:` holds the profiles (`agent.refs`).
    #[test]
    fn references_resolve_against_the_profile_files() {
        // A DataWriter by reference is not a format the specification has for
        // it (§7.7.3.1); the eProsima client's way needs the setting.
        let mut config = SessionEngineConfig {
            allow_any_representation_format: true,
            ..test_config()
        };
        config
            .profiles
            .add_profiles(
                "<profiles><participant profile_name=\"default_xrce_participant\">\
                 <rtps><name>default_xrce_participant</name></rtps></participant>\
                 <topic profile_name=\"shapetype_topic\"><kind>WITH_KEY</kind><name>Square</name>\
                 <dataType>ShapeType</dataType></topic>\
                 <data_writer profile_name=\"shapetype_data_writer\"><topic><kind>WITH_KEY</kind>\
                 <name>Square</name><dataType>ShapeType</dataType></topic></data_writer>\
                 </profiles>",
            )
            .unwrap();
        let mut e = SessionEngine::new(TypeRecordingPort::default(), config);
        let client = [0x33; CLIENT_KEY_LEN];
        let now = Duration::ZERO;
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            now,
        );
        let reference = |name: &str| ObjectVariant::ByReference(name.to_string());
        for (oid, variant) in [
            (participant_oid(), reference("default_xrce_participant")),
            (
                ObjectId::new(0x001, ObjectKind::Topic).unwrap(),
                reference("shapetype_topic"),
            ),
            (publisher_oid(), ObjectVariant::ByXmlString(String::new())),
            (
                ObjectId::new(0x001, ObjectKind::DataWriter).unwrap(),
                reference("shapetype_data_writer"),
            ),
        ] {
            e.handle_message(
                Some(client),
                &session_header(),
                &[create_variant_sm(oid, &variant, CreationMode::default())],
                now,
            );
        }
        let refused: Vec<_> = e
            .drain_events()
            .into_iter()
            .filter(|ev| matches!(ev, SessionEvent::CreateRefused { .. }))
            .collect();
        assert!(refused.is_empty(), "{refused:?}");
        assert_eq!(e.dds_port_mut().topics, vec![("Square".to_string(), None)]);
    }

    /// §7.7.3.1, Annex A: a Type, QosProfile and Application are by reference
    /// or XML (no binary), a Publisher, Subscriber, DataWriter and DataReader
    /// binary or XML (no reference), a DomainParticipant and Topic all three.
    /// A format outside that is STATUS_ERR_INVALID_DATA; `representation_formats:
    /// any` admits it.
    #[test]
    fn the_formats_of_each_kind_are_enforced() {
        use ObjectKind as K;
        let by_ref = ObjectVariant::ByReference("x".into());
        let xml = ObjectVariant::ByXmlString(String::new());
        let bin = ObjectVariant::InBinary(vec![0, 0, 0, 0]);
        for (kind, variant, allowed) in [
            (K::Participant, &by_ref, true),
            (K::Participant, &bin, true),
            (K::Topic, &by_ref, true),
            (K::Topic, &bin, true),
            (K::Type, &by_ref, true),
            (K::Type, &xml, true),
            (K::Type, &bin, false),
            (K::QosProfile, &bin, false),
            (K::Application, &bin, false),
            (K::Application, &by_ref, true),
            (K::Publisher, &by_ref, false),
            (K::Publisher, &xml, true),
            (K::Subscriber, &by_ref, false),
            (K::DataWriter, &by_ref, false),
            (K::DataWriter, &bin, true),
            (K::DataReader, &by_ref, false),
            (K::DataReader, &xml, true),
        ] {
            assert_eq!(
                format_is_allowed(kind, variant),
                allowed,
                "{kind:?} {variant:?}"
            );
        }
        for (any, expected) in [
            (false, ResultStatusCode::InvalidData),
            (true, ResultStatusCode::Ok),
        ] {
            let mut e = SessionEngine::new(
                TypeRecordingPort::default(),
                SessionEngineConfig {
                    allow_any_representation_format: any,
                    ..test_config()
                },
            );
            let client = [0x95; CLIENT_KEY_LEN];
            open_session(&mut e, client);
            let writer = ObjectId::new(0x001, ObjectKind::DataWriter).unwrap();
            let out = e.handle_message(
                Some(client),
                &session_header(),
                &[create_variant_sm(
                    writer,
                    &ObjectVariant::ByReference("w".into()),
                    CreationMode::default(),
                )],
                Duration::ZERO,
            );
            let code = status_of(&out);
            let format_refusals = e
                .drain_events()
                .into_iter()
                .filter(|ev| {
                    matches!(ev, SessionEvent::CreateRefused { reason, .. }
                        if reason.contains("has no representation in the format"))
                })
                .count();
            if any {
                // The format is admitted; whatever the lookup then says.
                assert_eq!(format_refusals, 0, "{code:?}");
            } else {
                assert_eq!(format_refusals, 1);
                assert_eq!(code, expected);
            }
        }
    }

    fn outbound_on(client: ClientKeyOrd, stream: u8) -> Outbound {
        let mut header = bootstrap_header();
        header.stream_id = StreamId(stream);
        Outbound {
            client,
            header,
            submessages: vec![
                AckNackPayload {
                    first_unacked_seq_num: 0,
                    nack_bitmap: [0, 0],
                    stream_id: 0,
                }
                .into_submessage()
                .unwrap(),
            ],
        }
    }

    /// §8.3.2: a best-effort receiver takes only a message newer than the
    /// last; every message on a best-effort stream gets the next number.
    #[test]
    fn best_effort_messages_get_increasing_sequence_numbers() {
        let mut e = engine();
        let client = [0x41; CLIENT_KEY_LEN];
        let mut out: Vec<Outbound> = (0..3).map(|_| outbound_on(client, 0x01)).collect();
        out.push(outbound_on(client, 0x00));
        e.finalize_reliable_sequencing(&mut out);
        let seqs: Vec<u16> = out.iter().map(|o| o.header.sequence_nr.raw()).collect();
        assert_eq!(seqs, vec![0, 1, 2, 0]);
        let mut next = vec![outbound_on(client, 0x01)];
        e.finalize_reliable_sequencing(&mut next);
        assert_eq!(next[0].header.sequence_nr.raw(), 3);
    }

    /// A reliable message the sender window has no room for waits for the
    /// client's ACKNACK, and then goes out with its sequence number — not
    /// unsequenced, which a reliable client drops.
    #[test]
    fn reliable_messages_beyond_the_window_wait_for_the_acknack() {
        let mut e = engine();
        let client = [0x42; CLIENT_KEY_LEN];
        let mut out: Vec<Outbound> = (0..20).map(|_| outbound_on(client, 0x80)).collect();
        e.finalize_reliable_sequencing(&mut out);
        let sent: Vec<u16> = out.iter().map(|o| o.header.sequence_nr.raw()).collect();
        assert_eq!(sent, (0..16).collect::<Vec<u16>>());
        assert!(
            e.drain_events()
                .iter()
                .all(|ev| !matches!(ev, SessionEvent::ReliableSendOverflowed { .. }))
        );
        e.reliable
            .get_mut(&(client, 0x80))
            .unwrap()
            .recv_acknack(AckNackPayload {
                first_unacked_seq_num: 16,
                nack_bitmap: [0, 0],
                stream_id: 0x80,
            });
        let mut later = Vec::new();
        e.finalize_reliable_sequencing(&mut later);
        let sent: Vec<u16> = later.iter().map(|o| o.header.sequence_nr.raw()).collect();
        assert_eq!(sent, vec![16, 17, 18, 19]);
    }

    fn agent_status_of(out: &[Outbound]) -> ResultStatusCode {
        assert_eq!(out.len(), 1, "one reply expected");
        let reply = StatusAgentPayload::try_from_submessage(&out[0].submessages[0]).unwrap();
        reply
            .result_and_position(Endianness::Little)
            .unwrap()
            .0
            .status_code
    }

    fn status_of(out: &[Outbound]) -> ResultStatusCode {
        assert_eq!(out.len(), 1, "one reply expected");
        let reply = StatusPayload::try_from_submessage(&out[0].submessages[0]).unwrap();
        BaseObjectReply::decode(&reply.representation)
            .unwrap()
            .0
            .result
            .status_code
    }

    const KEY_A: ClientKeyOrd = [0xAA, 0xBB, 0xCC, 0xDD];
    const KEY_B: ClientKeyOrd = [0x01, 0x02, 0x03, 0x04];

    fn engine_with(access: crate::daemon::access::AccessController) -> SessionEngine<NullDdsPort> {
        SessionEngine::new(
            NullDdsPort::new(),
            SessionEngineConfig {
                access,
                ..test_config()
            },
        )
    }

    fn connect(e: &mut SessionEngine<NullDdsPort>, client: ClientKeyOrd) -> Vec<Outbound> {
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            Duration::ZERO,
        )
    }

    fn connect_with_properties(
        e: &mut SessionEngine<NullDdsPort>,
        client: ClientKeyOrd,
        properties: Option<Vec<crate::daemon::handshake::Property>>,
    ) -> Vec<Outbound> {
        let sm = CreateClientPayload {
            representation: ClientRepresentation {
                timestamp: None,
                version: XRCE_VERSION,
                vendor_id: [0, 0],
                client_key: ClientKey(client),
                session_id: 0x81,
                properties,
                requested_mtu: None,
            },
        }
        .into_submessage()
        .unwrap();
        e.handle_message(None, &bootstrap_header(), &[sm], Duration::ZERO)
    }

    /// DDS-XRCE 1.0 §7.8.2.1: a client whose ClientKey is not configured in
    /// the agent is answered with STATUS_ERR_DENIED and gets no session.
    #[test]
    fn a_client_key_not_configured_in_the_agent_is_denied() {
        let mut e = engine_with(crate::daemon::access::AccessController {
            authorized_clients: vec![KEY_A],
            ..Default::default()
        });
        assert_eq!(
            agent_status_of(&connect(&mut e, KEY_B)),
            ResultStatusCode::Denied
        );
        assert_eq!(e.client_count(), 0);
        assert!(e.drain_events().iter().any(|ev| matches!(
            ev,
            SessionEvent::OperationDenied { client, .. } if *client == KEY_B
        )));
        assert_eq!(
            agent_status_of(&connect(&mut e, KEY_A)),
            ResultStatusCode::Ok
        );
        assert_eq!(e.client_count(), 1);
    }

    /// Without configured ClientKeys the agent accepts every key.
    #[test]
    fn without_configured_client_keys_every_key_is_accepted() {
        let mut e = engine();
        assert_eq!(
            agent_status_of(&connect(&mut e, KEY_B)),
            ResultStatusCode::Ok
        );
    }

    /// DDS-XRCE 1.0 §7.8.2.1: "Check the Client properties, if present."
    /// An agent that requires a property denies a client without it, or
    /// with another value, and accepts one that carries it.
    #[test]
    fn client_properties_are_checked_against_what_the_agent_requires() {
        use crate::daemon::handshake::Property;
        let prop = |v: &str| Property {
            name: "auth".to_string(),
            value: v.to_string(),
        };
        let mut e = engine_with(crate::daemon::access::AccessController {
            required_properties: vec![("auth".to_string(), "token".to_string())],
            ..Default::default()
        });
        assert_eq!(
            agent_status_of(&connect_with_properties(&mut e, KEY_A, None)),
            ResultStatusCode::Denied
        );
        assert_eq!(
            agent_status_of(&connect_with_properties(
                &mut e,
                KEY_A,
                Some(vec![prop("x")])
            )),
            ResultStatusCode::Denied
        );
        assert_eq!(e.client_count(), 0);
        assert_eq!(
            agent_status_of(&connect_with_properties(
                &mut e,
                KEY_A,
                Some(vec![prop("token")])
            )),
            ResultStatusCode::Ok
        );
        assert_eq!(e.client_count(), 1);
    }

    /// DDS-XRCE 1.0 §8.3.4.2: a CREATE_CLIENT with the E flag clear carries
    /// its properties big-endian; the agent reads them as such and checks
    /// the required one against them.
    #[test]
    fn a_big_endian_create_client_with_properties_is_read_right() {
        use crate::daemon::handshake::Property;
        let mut e = engine_with(crate::daemon::access::AccessController {
            required_properties: vec![("auth".to_string(), "token".to_string())],
            ..Default::default()
        });
        let sm = CreateClientPayload {
            representation: ClientRepresentation {
                timestamp: None,
                version: XRCE_VERSION,
                vendor_id: [0, 0],
                client_key: ClientKey(KEY_A),
                session_id: 0x81,
                properties: Some(vec![Property {
                    name: "auth".to_string(),
                    value: "token".to_string(),
                }]),
                requested_mtu: None,
            },
        }
        .into_submessage_with(Endianness::Big)
        .unwrap();
        assert_eq!(sm.header.flags & 1, 0, "big-endian");
        let out = e.handle_message(None, &bootstrap_header(), &[sm], Duration::ZERO);
        assert_eq!(agent_status_of(&out), ResultStatusCode::Ok);
        assert_eq!(e.client_count(), 1);
    }

    /// DDS-XRCE 1.0 §7.8.2.1: a PropertySeq the agent cannot read fails
    /// the check; the session is not created and the client is told.
    #[test]
    fn an_unreadable_property_seq_is_refused_with_invalid_data() {
        let mut body = Vec::new();
        body.extend_from_slice(b"XRCE");
        body.extend_from_slice(&XRCE_VERSION);
        body.extend_from_slice(&[0, 0]);
        body.extend_from_slice(&KEY_A);
        body.push(0x81);
        body.push(1);
        body.extend_from_slice(&3u32.to_le_bytes());
        let sm = Submessage::new(
            SubmessageId::CreateClient,
            zerodds_xrce::submessages::FLAG_E_LITTLE_ENDIAN,
            body,
        )
        .unwrap();
        let mut e = engine();
        let out = e.handle_message(None, &bootstrap_header(), &[sm], Duration::ZERO);
        // §8.3.5.1: "The XRCE Agent shall send a STATUS_AGENT message in
        // response" to a CREATE_CLIENT.
        assert_eq!(agent_status_of(&out), ResultStatusCode::InvalidData);
        assert_eq!(e.client_count(), 0);
    }

    fn participant_create(
        e: &mut SessionEngine<NullDdsPort>,
        client: ClientKeyOrd,
    ) -> Vec<Outbound> {
        e.handle_message(
            Some(client),
            &session_header(),
            &[create_object_sm(
                participant_oid(),
                "<dds><participant><domain_id>0</domain_id></participant></dds>",
                CreationMode::default(),
            )],
            Duration::ZERO,
        )
    }

    fn deny_all_but(
        client: ClientKeyOrd,
        operations: Vec<Operation>,
    ) -> crate::daemon::access::AccessController {
        use crate::daemon::access::{AccessController, Effect, Rule};
        AccessController {
            rules: vec![Rule {
                client: Some(client),
                effect: Effect::Allow,
                operations,
                kinds: vec![],
                domains: vec![],
                topics: vec![],
            }],
            ..AccessController::default()
        }
    }

    /// DDS-XRCE 1.0 §7.4, §7.8.3.1: a CREATE the client has no permission
    /// for is answered with STATUS_ERR_DENIED and nothing is stored.
    #[test]
    fn create_without_permission_is_denied_and_stores_nothing() {
        let mut e = engine_with(deny_all_but(KEY_A, vec![Operation::GetInfo]));
        connect(&mut e, KEY_A);
        let out = participant_create(&mut e, KEY_A);
        assert_eq!(status_of(&out), ResultStatusCode::Denied);
        assert_eq!(e.agent.object_count(ClientKey(KEY_A)).unwrap(), 0);
        assert!(e.drain_events().iter().any(|ev| matches!(
            ev,
            SessionEvent::OperationDenied { object, .. } if *object == participant_oid()
        )));
    }

    /// §7.4: the rules name the DDS domain an application may join.
    #[test]
    fn a_rule_can_limit_the_domains_a_client_joins() {
        use crate::daemon::access::{AccessController, Effect, Rule};
        let mut e = engine_with(AccessController {
            rules: vec![Rule {
                client: None,
                effect: Effect::Allow,
                operations: vec![Operation::Create],
                kinds: vec![],
                domains: vec![5],
                topics: vec![],
            }],
            ..AccessController::default()
        });
        connect(&mut e, KEY_A);
        // The participant is created in domain 0.
        assert_eq!(
            status_of(&participant_create(&mut e, KEY_A)),
            ResultStatusCode::Denied
        );
    }

    /// §7.4: the rules name the topics an application may read and write;
    /// the topic's domain comes from its participant.
    #[test]
    fn a_rule_can_limit_the_topics_a_client_creates() {
        use crate::daemon::access::{AccessController, Effect, Rule};
        let mut e = engine_with(AccessController {
            rules: vec![
                Rule {
                    client: None,
                    effect: Effect::Allow,
                    operations: vec![Operation::Create],
                    kinds: vec![ObjectKind::Participant],
                    domains: vec![],
                    topics: vec![],
                },
                Rule {
                    client: None,
                    effect: Effect::Allow,
                    operations: vec![Operation::Create],
                    kinds: vec![ObjectKind::Topic],
                    domains: vec![0],
                    topics: vec!["Allowed*".to_string()],
                },
            ],
            ..AccessController::default()
        });
        connect(&mut e, KEY_A);
        assert_eq!(
            status_of(&participant_create(&mut e, KEY_A)),
            ResultStatusCode::Ok
        );
        let topic_create = |e: &mut SessionEngine<NullDdsPort>, id: u16, name: &str| {
            e.handle_message(
                Some(KEY_A),
                &session_header(),
                &[create_object_sm(
                    ObjectId::new(id, ObjectKind::Topic).unwrap(),
                    &format!("<dds><topic name=\"{name}\" type_name=\"Ty\"/></dds>"),
                    CreationMode::default(),
                )],
                Duration::ZERO,
            )
        };
        assert_eq!(
            status_of(&topic_create(&mut e, 3, "AllowedChat")),
            ResultStatusCode::Ok
        );
        assert_eq!(
            status_of(&topic_create(&mut e, 4, "Secret")),
            ResultStatusCode::Denied
        );
    }

    /// §7.8.3.3, §7.8.3.4: GET_INFO and DELETE of an object the client has
    /// no permission for are answered with STATUS_ERR_DENIED; the object
    /// stays.
    #[test]
    fn get_info_and_delete_without_permission_are_denied() {
        let mut e = engine_with(deny_all_but(KEY_A, vec![Operation::Create]));
        connect(&mut e, KEY_A);
        assert_eq!(
            status_of(&participant_create(&mut e, KEY_A)),
            ResultStatusCode::Ok
        );
        let base = BaseObjectRequest {
            request_id: [0, 9],
            object_id: participant_oid(),
        };
        let info = GetInfoPayload::new(base, InfoMask::ALL, Endianness::Little)
            .into_submessage()
            .unwrap();
        let out = e.handle_message(Some(KEY_A), &session_header(), &[info], Duration::ZERO);
        // §8.3.5.7.2: a failed GET_INFO is an INFO whose status is the error.
        assert_eq!(info_of(&out).0.result.status_code, ResultStatusCode::Denied);
        let delete = DeletePayload {
            representation: base.encode().to_vec(),
        }
        .into_submessage()
        .unwrap();
        let out = e.handle_message(Some(KEY_A), &session_header(), &[delete], Duration::ZERO);
        assert_eq!(status_of(&out), ResultStatusCode::Denied);
        assert_eq!(e.agent.object_count(ClientKey(KEY_A)).unwrap(), 1);
    }

    /// §7.8.4.1: WRITE_DATA on a writer the client may not write is
    /// answered with STATUS_ERR_DENIED and nothing reaches the DDS side;
    /// §7.8.5.1: the same for READ_DATA, and no read becomes active.
    #[test]
    fn write_and_read_without_permission_are_denied() {
        let mut e = engine_with(deny_all_but(KEY_A, vec![Operation::Create]));
        connect(&mut e, KEY_A);
        create_participant_topic_reader(&mut e, KEY_A);
        let reader = ObjectId::new(0x002, ObjectKind::DataReader).unwrap();
        let out = e.handle_message(
            Some(KEY_A),
            &session_header(),
            &[read_sm(reader, 7, 0x01, None)],
            Duration::ZERO,
        );
        assert_eq!(status_of(&out), ResultStatusCode::Denied);
        assert!(!e.reads.contains_key(&(KEY_A, reader)));

        let write = zerodds_xrce::submessages::write_data::WriteDataPayload {
            base: BaseObjectRequest {
                request_id: [0, 8],
                object_id: reader,
            },
            serialized_data: vec![1, 2, 3, 4],
        }
        .into_submessage()
        .unwrap();
        let out = e.handle_message(Some(KEY_A), &session_header(), &[write], Duration::ZERO);
        assert_eq!(status_of(&out), ResultStatusCode::Denied);
    }

    /// With the permission the same READ_DATA is not denied (no STATUS
    /// ERR_DENIED, the read becomes active).
    #[test]
    fn read_with_permission_is_not_denied() {
        let mut e = engine_with(deny_all_but(
            KEY_A,
            vec![Operation::Create, Operation::ReadData],
        ));
        connect(&mut e, KEY_A);
        create_participant_topic_reader(&mut e, KEY_A);
        let reader = ObjectId::new(0x002, ObjectKind::DataReader).unwrap();
        let out = e.handle_message(
            Some(KEY_A),
            &session_header(),
            &[read_sm(reader, 7, 0x01, limited(1))],
            Duration::ZERO,
        );
        assert!(
            out.iter().all(|o| o
                .submessages
                .iter()
                .all(|sm| { sm.header.submessage_id != SubmessageId::Status })),
            "no STATUS"
        );
        assert!(e.reads.contains_key(&(KEY_A, reader)));
    }

    /// §7.8.3.1, Table 5: replacing an existing object deletes it first,
    /// which needs the permission to delete.
    #[test]
    fn replacing_an_object_needs_the_permission_to_delete_it() {
        let mut e = engine_with(deny_all_but(KEY_A, vec![Operation::Create]));
        connect(&mut e, KEY_A);
        assert_eq!(
            status_of(&participant_create(&mut e, KEY_A)),
            ResultStatusCode::Ok
        );
        let out = e.handle_message(
            Some(KEY_A),
            &session_header(),
            &[create_object_sm(
                participant_oid(),
                "<dds><participant><domain_id>0</domain_id></participant></dds>",
                CreationMode {
                    reuse: false,
                    replace: true,
                },
            )],
            Duration::ZERO,
        );
        assert_eq!(status_of(&out), ResultStatusCode::Denied);
    }

    fn client_with_participant(e: &mut SessionEngine<TypeRecordingPort>, client: ClientKeyOrd) {
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            Duration::ZERO,
        );
        let out = e.handle_message(
            Some(client),
            &session_header(),
            &[create_object_sm(
                participant_oid(),
                "<dds><participant><domain_id>0</domain_id></participant></dds>",
                CreationMode::default(),
            )],
            Duration::ZERO,
        );
        assert_eq!(status_of(&out), ResultStatusCode::Ok);
    }

    fn create_one(
        e: &mut SessionEngine<TypeRecordingPort>,
        client: ClientKeyOrd,
        sm: Submessage,
    ) -> ResultStatusCode {
        status_of(&e.handle_message(Some(client), &session_header(), &[sm], Duration::ZERO))
    }

    /// DDS-XRCE 1.0 §7.7.3.8, §7.7.3.9, Table 6: the participant_id of a
    /// Publisher or Subscriber names an XRCE DomainParticipant. An id of
    /// another kind is invalid data; an id of a participant the client does
    /// not hold is STATUS_ERR_UNKNOWN_REFERENCE. Nothing is stored.
    #[test]
    fn a_publisher_or_subscriber_needs_an_existing_participant() {
        let mut e = SessionEngine::new(TypeRecordingPort::default(), test_config());
        let client = [0x41; CLIENT_KEY_LEN];
        client_with_participant(&mut e, client);
        let empty = ObjectVariant::ByXmlString(String::new());
        for (oid, kind) in [
            (publisher_oid(), ObjectKind::Publisher),
            (subscriber_oid(), ObjectKind::Subscriber),
        ] {
            let missing = ObjectId::new(0x00F, ObjectKind::Participant).unwrap();
            assert_eq!(
                create_one(
                    &mut e,
                    client,
                    create_under_sm(oid, &empty, CreationMode::default(), missing)
                ),
                ResultStatusCode::UnknownReference,
                "{kind:?} under a participant that does not exist"
            );
            let not_a_participant = ObjectId::new(0x001, ObjectKind::Topic).unwrap();
            assert_eq!(
                create_one(
                    &mut e,
                    client,
                    create_under_sm(oid, &empty, CreationMode::default(), not_a_participant)
                ),
                ResultStatusCode::InvalidData,
                "{kind:?} under an id that is not a participant"
            );
            assert!(e.agent.get_object(ClientKey(client), oid).is_none());
        }
        assert!(e.dds_port_mut().containers.is_empty());
        assert_eq!(
            create_one(
                &mut e,
                client,
                create_object_sm(publisher_oid(), "", CreationMode::default())
            ),
            ResultStatusCode::Ok
        );
        assert_eq!(
            create_one(
                &mut e,
                client,
                create_object_sm(subscriber_oid(), "", CreationMode::default())
            ),
            ResultStatusCode::Ok
        );
    }

    /// DDS-XRCE 1.0 §7.5, §7.8.3.1 Table 6: the XRCE Publisher and
    /// Subscriber have a DDS Publisher and Subscriber, created under the
    /// DomainParticipant, with the policies of their representation
    /// (PRESENTATION, ENTITY_FACTORY, PARTITION).
    #[test]
    fn a_publisher_and_a_subscriber_get_a_dds_entity_with_their_policies() {
        let mut e = SessionEngine::new(TypeRecordingPort::default(), test_config());
        let client = [0x42; CLIENT_KEY_LEN];
        client_with_participant(&mut e, client);
        assert_eq!(
            create_one(
                &mut e,
                client,
                create_object_sm(
                    publisher_oid(),
                    "<dds><publisher><qos><partition><names><name>p1</name></names></partition>\
                     <presentation><access_scope>TOPIC</access_scope><coherent_access>true\
                     </coherent_access></presentation></qos></publisher></dds>",
                    CreationMode::default()
                )
            ),
            ResultStatusCode::Ok
        );
        assert_eq!(
            create_one(
                &mut e,
                client,
                create_object_sm(subscriber_oid(), "", CreationMode::default())
            ),
            ResultStatusCode::Ok
        );
        let seen = &e.dds_port_mut().containers;
        assert_eq!(seen.len(), 2);
        let (is_publisher, key, participant, spec) = &seen[0];
        assert!(is_publisher);
        assert_eq!(*key, client);
        assert_eq!(*participant, participant_oid());
        assert_eq!(spec.partitions, vec!["p1".to_string()]);
        assert_eq!(
            spec.presentation.access_scope,
            zerodds_qos::PresentationAccessScope::Topic
        );
        assert!(spec.presentation.coherent_access);
        assert!(spec.autoenable_created_entities);
        assert!(!seen[1].0, "the second is the Subscriber");
        assert_eq!(seen[1].3, ContainerSpec::default());
    }

    /// DDS: a DataWriter and its topic belong to the participant of the
    /// Publisher. A Publisher of another participant is refused.
    #[test]
    fn an_endpoint_and_its_topic_belong_to_one_participant() {
        let mut e = SessionEngine::new(TypeRecordingPort::default(), test_config());
        let client = [0x43; CLIENT_KEY_LEN];
        client_with_participant(&mut e, client);
        let second = ObjectId::new(0x002, ObjectKind::Participant).unwrap();
        let xml = ObjectVariant::ByXmlString(
            "<dds><participant><domain_id>0</domain_id></participant></dds>".to_string(),
        );
        assert_eq!(
            create_one(
                &mut e,
                client,
                create_variant_sm(second, &xml, CreationMode::default())
            ),
            ResultStatusCode::Ok
        );
        // The topic is under the first participant, the Publisher under the second.
        let topic = ObjectId::new(0x001, ObjectKind::Topic).unwrap();
        assert_eq!(
            create_one(
                &mut e,
                client,
                create_object_sm(
                    topic,
                    "<dds><topic name=\"T\" type_name=\"Ty\"/></dds>",
                    CreationMode::default()
                )
            ),
            ResultStatusCode::Ok
        );
        let empty = ObjectVariant::ByXmlString(String::new());
        assert_eq!(
            create_one(
                &mut e,
                client,
                create_under_sm(publisher_oid(), &empty, CreationMode::default(), second)
            ),
            ResultStatusCode::Ok
        );
        let writer = ObjectId::new(0x001, ObjectKind::DataWriter).unwrap();
        let out = create_one(
            &mut e,
            client,
            create_object_sm(
                writer,
                &format!(
                    "<dds><data_writer topic_ref=\"0x{:04x}\"/></dds>",
                    topic.raw()
                ),
                CreationMode::default(),
            ),
        );
        assert_eq!(out, ResultStatusCode::InvalidData);
        assert!(e.agent.get_object(ClientKey(client), writer).is_none());
    }

    fn header_on(session: u8, stream: u8, seq: u16) -> MessageHeader {
        MessageHeader {
            session_id: SessionId(session),
            stream_id: StreamId(stream),
            sequence_nr: SerialNumber16::new(seq),
            client_key: None,
        }
    }

    fn participant_sm(id: u16) -> Submessage {
        create_variant_sm(
            ObjectId::new(id, ObjectKind::Participant).unwrap(),
            &ObjectVariant::ByXmlString(
                "<dds><participant><domain_id>0</domain_id></participant></dds>".to_string(),
            ),
            CreationMode::default(),
        )
    }

    fn connected(e: &mut SessionEngine<NullDdsPort>, client: ClientKeyOrd, session: u8) {
        let out = e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, session, XRCE_VERSION)],
            Duration::ZERO,
        );
        assert_eq!(agent_status_of(&out), ResultStatusCode::Ok);
    }

    fn refusals(e: &mut SessionEngine<NullDdsPort>) -> usize {
        e.drain_events()
            .iter()
            .filter(|ev| matches!(ev, SessionEvent::MessageRefused { .. }))
            .count()
    }

    /// DDS-XRCE 1.0 §8.2.2: "Creation of a new session closes any previous
    /// sessions." A message with the session_id of the replaced session is
    /// not the client's: nothing is processed, nothing is answered, and the
    /// agent reports it.
    #[test]
    fn a_message_of_a_replaced_session_is_not_processed() {
        let mut e = engine();
        let client = [0x51; CLIENT_KEY_LEN];
        connected(&mut e, client, 0x81);
        // The client starts a new session.
        connected(&mut e, client, 0x82);
        assert_eq!(refusals(&mut e), 0);
        let out = e.handle_message(
            Some(client),
            &header_on(0x81, 0, 0),
            &[participant_sm(1)],
            Duration::ZERO,
        );
        assert!(out.is_empty(), "no answer to the old session: {out:?}");
        assert_eq!(refusals(&mut e), 1);
        assert_eq!(e.agent.object_count(ClientKey(client)).unwrap(), 0);
        let out = e.handle_message(
            Some(client),
            &header_on(0x82, 0, 0),
            &[participant_sm(1)],
            Duration::ZERO,
        );
        assert_eq!(status_of(&out), ResultStatusCode::Ok);
        assert_eq!(e.agent.object_count(ClientKey(client)).unwrap(), 1);
    }

    /// §8.3.2.1: 0x00 and 0x80 "indicate the lack of a session". A registered
    /// client's CREATE in such a header belongs to no session.
    #[test]
    fn a_message_without_a_session_is_not_processed_unless_it_is_a_create_client() {
        let mut e = engine();
        let client = [0x52; CLIENT_KEY_LEN];
        connected(&mut e, client, 0x81);
        for none in [0x80u8, 0x00] {
            let out = e.handle_message(
                Some(client),
                &header_on(none, 0, 0),
                &[participant_sm(1)],
                Duration::ZERO,
            );
            assert!(out.is_empty(), "session_id {none:#x}: {out:?}");
        }
        assert_eq!(refusals(&mut e), 2);
        assert_eq!(e.agent.object_count(ClientKey(client)).unwrap(), 0);
        // The CREATE_CLIENT itself travels there.
        connected(&mut e, [0x53; CLIENT_KEY_LEN], 0x81);
        assert_eq!(refusals(&mut e), 0);
    }

    /// §8.3.2.1: a client cannot ask for a session that is none.
    #[test]
    fn a_create_client_for_a_session_id_of_no_session_is_refused() {
        let mut e = engine();
        for none in [0x00u8, 0x80] {
            let out = e.handle_message(
                None,
                &bootstrap_header(),
                &[create_client_sm([0x54; CLIENT_KEY_LEN], none, XRCE_VERSION)],
                Duration::ZERO,
            );
            assert_eq!(agent_status_of(&out), ResultStatusCode::InvalidData);
        }
        assert_eq!(e.client_count(), 0);
    }

    /// §8.3.2.3: on a best-effort stream "Messages within a stream shall not
    /// be delivered out of order. In addition, duplicate messages shall be
    /// discarded." STREAMID_NONE has no order.
    #[test]
    fn best_effort_streams_drop_duplicates_and_older_messages() {
        let mut e = engine();
        let client = [0x55; CLIENT_KEY_LEN];
        connected(&mut e, client, 0x81);
        let send = |e: &mut SessionEngine<NullDdsPort>, stream: u8, seq: u16, id: u16| {
            e.handle_message(
                Some(client),
                &header_on(0x81, stream, seq),
                &[participant_sm(id)],
                Duration::ZERO,
            )
        };
        assert_eq!(status_of(&send(&mut e, 1, 5, 1)), ResultStatusCode::Ok);
        // The same sequence number again, and an older one.
        assert!(send(&mut e, 1, 5, 2).is_empty());
        assert!(send(&mut e, 1, 4, 2).is_empty());
        assert_eq!(e.agent.object_count(ClientKey(client)).unwrap(), 1);
        let discarded: Vec<_> = e
            .drain_events()
            .into_iter()
            .filter_map(|ev| match ev {
                SessionEvent::BestEffortMessageDiscarded {
                    stream_id,
                    sequence_nr,
                    newest,
                    ..
                } => Some((stream_id, sequence_nr, newest)),
                _ => None,
            })
            .collect();
        assert_eq!(discarded, vec![(1, 5, 5), (1, 4, 5)]);
        // A newer one is taken, also across the wrap of the 16-bit number
        // (RFC 1982: up to 32767 ahead is newer; 0xFFFF after 6 is 7 behind).
        assert_eq!(status_of(&send(&mut e, 1, 6, 2)), ResultStatusCode::Ok);
        assert_eq!(status_of(&send(&mut e, 1, 0x7FFF, 3)), ResultStatusCode::Ok);
        assert_eq!(status_of(&send(&mut e, 1, 0xFFFE, 4)), ResultStatusCode::Ok);
        assert_eq!(status_of(&send(&mut e, 1, 0, 8)), ResultStatusCode::Ok);
        assert!(send(&mut e, 1, 0xFFFF, 9).is_empty(), "older than 0xFFFE+2");
        // Another stream counts for itself.
        assert_eq!(status_of(&send(&mut e, 2, 0, 5)), ResultStatusCode::Ok);
        // STREAMID_NONE imposes no order.
        assert_eq!(status_of(&send(&mut e, 0, 9, 6)), ResultStatusCode::Ok);
        assert_eq!(status_of(&send(&mut e, 0, 9, 7)), ResultStatusCode::Ok);
    }

    /// §8.3.5.13: RESET "shall cause the XRCE Agent to reset all state
    /// associated with the session_id": the running read ends, the messages
    /// waiting for room in a sender window go, the sequence numbers of the
    /// best-effort streams start over, and so do all reliable streams —
    /// stream 255 as well.
    #[test]
    fn reset_clears_reads_backlog_best_effort_counters_and_every_reliable_stream() {
        let mut e = engine();
        let client = [0x56; CLIENT_KEY_LEN];
        connected(&mut e, client, 0x81);
        create_participant_topic_reader(&mut e, client);
        let reader = ObjectId::new(0x002, ObjectKind::DataReader).unwrap();
        e.handle_message(
            Some(client),
            &session_header(),
            &[read_sm(reader, 7, 0x01, limited(0xFFFF))],
            Duration::ZERO,
        );
        assert!(e.reads.contains_key(&(client, reader)));
        // Best-effort stream 1 has taken message 7; reliable stream 255 has
        // taken message 0.
        let send = |e: &mut SessionEngine<NullDdsPort>, stream: u8, seq: u16, id: u16| {
            e.handle_message(
                Some(client),
                &header_on(0x81, stream, seq),
                &[participant_sm(id)],
                Duration::ZERO,
            )
        };
        let statuses = |out: &[Outbound]| {
            out.iter()
                .flat_map(|o| &o.submessages)
                .filter(|sm| sm.header.submessage_id == SubmessageId::Status)
                .count()
        };
        assert_eq!(status_of(&send(&mut e, 1, 7, 10)), ResultStatusCode::Ok);
        assert_eq!(statuses(&send(&mut e, 0xFF, 0, 11)), 1);
        assert_eq!(statuses(&send(&mut e, 0xFF, 0, 12)), 0, "a duplicate");
        // A message waiting for room in a sender window, and a counter of the
        // best-effort output (put in after the sends: any message handled
        // moves the waiting message into the window).
        e.reliable_backlog
            .entry((client, 0x80))
            .or_default()
            .push_back(outbound_on(client, 0x80));
        e.best_effort_seq
            .insert((client, 0x01), SerialNumber16::new(9));

        let reset = ResetPayload.into_submessage().unwrap();
        let out = e.handle_message(Some(client), &session_header(), &[reset], Duration::ZERO);
        assert!(out.is_empty(), "RESET has no reply");
        assert!(!e.reads.contains_key(&(client, reader)), "the read ended");
        assert!(e.reliable_backlog.is_empty(), "the backlog is gone");
        assert!(e.best_effort_seq.is_empty(), "the output counters restart");
        // The same sequence numbers are new again.
        assert_eq!(status_of(&send(&mut e, 1, 7, 13)), ResultStatusCode::Ok);
        assert_eq!(
            statuses(&send(&mut e, 0xFF, 0, 14)),
            1,
            "stream 255 starts over"
        );
    }

    /// A `DdsPort` whose samples are `size` bytes long, that records every
    /// write: what the session hands to the DDS side.
    struct SplitPort {
        inner: NullDdsPort,
        size: usize,
        writes: Vec<RecordedWrite>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RecordedWrite {
        payload: Vec<u8>,
        dispose: bool,
        unregister: bool,
        source_time: Option<SourceTime>,
    }

    impl SplitPort {
        fn new(size: usize) -> Self {
            Self {
                inner: NullDdsPort::new(),
                size,
                writes: Vec::new(),
            }
        }
    }

    impl DdsPort for SplitPort {
        fn create_participant(
            &mut self,
            key: crate::daemon::dds_port::ObjectKey,
            domain_id: i32,
        ) -> Result<(), DdsPortError> {
            self.inner.create_participant(key, domain_id)
        }
        fn create_topic(
            &mut self,
            key: crate::daemon::dds_port::ObjectKey,
            participant: crate::daemon::dds_port::ObjectKey,
            spec: TopicSpec,
        ) -> Result<(), DdsPortError> {
            self.inner.create_topic(key, participant, spec)
        }
        fn create_publisher(
            &mut self,
            key: crate::daemon::dds_port::ObjectKey,
            participant: crate::daemon::dds_port::ObjectKey,
            spec: ContainerSpec,
        ) -> Result<(), DdsPortError> {
            self.inner.create_publisher(key, participant, spec)
        }
        fn create_subscriber(
            &mut self,
            key: crate::daemon::dds_port::ObjectKey,
            participant: crate::daemon::dds_port::ObjectKey,
            spec: ContainerSpec,
        ) -> Result<(), DdsPortError> {
            self.inner.create_subscriber(key, participant, spec)
        }
        fn create_writer(
            &mut self,
            key: crate::daemon::dds_port::ObjectKey,
            topic: crate::daemon::dds_port::ObjectKey,
            publisher: crate::daemon::dds_port::ObjectKey,
            qos: crate::daemon::dds_port::EndpointQos,
        ) -> Result<(), DdsPortError> {
            self.inner.create_writer(key, topic, publisher, qos)
        }
        fn create_reader(
            &mut self,
            key: crate::daemon::dds_port::ObjectKey,
            topic: crate::daemon::dds_port::ObjectKey,
            subscriber: crate::daemon::dds_port::ObjectKey,
            qos: crate::daemon::dds_port::EndpointQos,
        ) -> Result<(), DdsPortError> {
            self.inner.create_reader(key, topic, subscriber, qos)
        }
        fn write(
            &mut self,
            _writer: crate::daemon::dds_port::ObjectKey,
            payload: &[u8],
            _big_endian: bool,
            dispose: bool,
            unregister: bool,
            source_time: Option<SourceTime>,
        ) -> Result<crate::daemon::dds_port::WriteOutcome, DdsPortError> {
            self.writes.push(RecordedWrite {
                payload: payload.to_vec(),
                dispose,
                unregister,
                source_time,
            });
            Ok(crate::daemon::dds_port::WriteOutcome::Written)
        }
        fn sample_length(
            &mut self,
            _writer: crate::daemon::dds_port::ObjectKey,
            bytes: &[u8],
            _big_endian: bool,
        ) -> Result<usize, DdsPortError> {
            if bytes.len() < self.size {
                return Err(DdsPortError::InvalidType("truncated sample".to_string()));
            }
            Ok(self.size)
        }
        fn set_read_filter(
            &mut self,
            reader: crate::daemon::dds_port::ObjectKey,
            filter: Option<&str>,
        ) -> Result<(), DdsPortError> {
            self.inner.set_read_filter(reader, filter)
        }
        fn poll_samples(
            &mut self,
            reader: crate::daemon::dds_port::ObjectKey,
            max: usize,
        ) -> Vec<crate::daemon::dds_port::PolledSample> {
            self.inner.poll_samples(reader, max)
        }
        fn delete(&mut self, key: crate::daemon::dds_port::ObjectKey) {
            self.inner.delete(key);
        }
        fn unregister_client(&mut self, client_key: [u8; CLIENT_KEY_LEN]) {
            self.inner.unregister_client(client_key);
        }
    }

    const WRITER_CLIENT: ClientKeyOrd = [0x61; CLIENT_KEY_LEN];

    fn writer_oid() -> ObjectId {
        ObjectId::new(0x002, ObjectKind::DataWriter).unwrap()
    }

    /// A connected client with a participant, topic, publisher and DataWriter.
    fn engine_with_writer<P: DdsPort>(port: P) -> SessionEngine<P> {
        let mut e = SessionEngine::new(port, test_config());
        let client = WRITER_CLIENT;
        e.handle_message(
            None,
            &bootstrap_header(),
            &[create_client_sm(client, 0x81, XRCE_VERSION)],
            Duration::ZERO,
        );
        let topic = ObjectId::new(0x003, ObjectKind::Topic).unwrap();
        for sm in [
            create_object_sm(
                participant_oid(),
                "<dds><participant><domain_id>0</domain_id></participant></dds>",
                CreationMode::default(),
            ),
            create_object_sm(
                topic,
                "<dds><topic name=\"T\" type_name=\"Ty\"/></dds>",
                CreationMode::default(),
            ),
            create_object_sm(publisher_oid(), "", CreationMode::default()),
            create_object_sm(
                writer_oid(),
                &format!(
                    "<dds><data_writer topic_ref=\"0x{:04x}\"/></dds>",
                    topic.raw()
                ),
                CreationMode::default(),
            ),
        ] {
            let out = e.handle_message(Some(client), &session_header(), &[sm], Duration::ZERO);
            assert_eq!(status_of(&out), ResultStatusCode::Ok);
        }
        e
    }

    fn write_base(request: u8) -> BaseObjectRequest {
        BaseObjectRequest {
            request_id: [0, request],
            object_id: writer_oid(),
        }
    }

    fn sample_info(flags: u8, offset_ms: u32) -> zerodds_xrce::sample_info::SampleInfo {
        zerodds_xrce::sample_info::SampleInfo {
            state: zerodds_xrce::sample_info::SampleInfoFlags(flags),
            sequence_number: 7,
            session_time_offset: offset_ms,
        }
    }

    fn send_write<P: DdsPort>(e: &mut SessionEngine<P>, sm: Submessage) -> Vec<Outbound> {
        e.handle_message(
            Some(WRITER_CLIENT),
            &session_header(),
            &[sm],
            Duration::ZERO,
        )
    }

    /// §7.8.4.1: "If the data is successfully written STATUS_OK shall be
    /// returned." The STATUS names the request and the DataWriter
    /// (§8.3.5.1 BaseObjectReply), and one request with several samples gets
    /// one STATUS.
    #[test]
    fn a_successful_write_is_answered_with_status_ok() {
        use zerodds_xrce::submessages::write_data::WriteDataPayload;
        let mut e = engine_with_writer(SplitPort::new(4));
        let out = send_write(
            &mut e,
            WriteDataPayload {
                base: write_base(5),
                serialized_data: vec![1, 2, 3, 4],
            }
            .into_submessage()
            .unwrap(),
        );
        assert_eq!(out.len(), 1);
        let reply = StatusPayload::try_from_submessage(&out[0].submessages[0]).unwrap();
        let (reply, _) = BaseObjectReply::decode(&reply.representation).unwrap();
        assert_eq!(reply.related_request.request_id, [0, 5]);
        assert_eq!(reply.related_request.object_id, writer_oid());
        assert_eq!(reply.result.status_code, ResultStatusCode::Ok);
        assert_eq!(e.dds_port_mut().writes.len(), 1);
    }

    /// DDS-XRCE 1.0 §7.8.4.1, Annex B.5.2: FORMAT_DATA_SEQ is a count and the
    /// serialized samples back to back with no length; the topic's type says
    /// where each ends. Each sample is written, in order.
    #[test]
    fn a_data_seq_is_split_by_the_type_and_written_sample_by_sample() {
        use zerodds_xrce::submessages::write_data::WriteDataPayloadDataSeq;
        let mut e = engine_with_writer(SplitPort::new(4));
        let out = send_write(
            &mut e,
            WriteDataPayloadDataSeq {
                base: write_base(1),
                sample_count: 3,
                elements: vec![1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3],
            }
            .into_submessage()
            .unwrap(),
        );
        assert_eq!(status_of(&out), ResultStatusCode::Ok);
        let writes = &e.dds_port_mut().writes;
        assert_eq!(
            writes.iter().map(|w| w.payload.clone()).collect::<Vec<_>>(),
            vec![vec![1; 4], vec![2; 4], vec![3; 4]]
        );
        assert!(
            writes
                .iter()
                .all(|w| !w.dispose && !w.unregister && w.source_time.is_none())
        );
    }

    /// Annex B.5.4: FORMAT_SAMPLE_SEQ puts a 12-octet SampleInfo before each
    /// sample. Its dispose and unregister flags and its session_time_offset
    /// apply to that sample; the source time is the session time plus the
    /// offset (§7.7.2).
    #[test]
    fn a_sample_seq_carries_state_and_time_per_sample() {
        use zerodds_xrce::sample_info::SampleInfoFlags as F;
        use zerodds_xrce::submessages::write_data::WriteDataPayloadSampleSeq;
        let mut e = engine_with_writer(SplitPort::new(4));
        let mut elements = Vec::new();
        for (info, data) in [
            (sample_info(0, 0), [1u8; 4]),
            (sample_info(F::INSTANCE_STATE_DISPOSED, 1500), [2; 4]),
            (sample_info(F::INSTANCE_STATE_UNREGISTERED, 90_000), [3; 4]),
        ] {
            info.encode_into(Endianness::Little, &mut elements).unwrap();
            elements.extend_from_slice(&data);
        }
        let out = send_write(
            &mut e,
            WriteDataPayloadSampleSeq {
                base: write_base(2),
                sample_count: 3,
                elements,
            }
            .into_submessage()
            .unwrap(),
        );
        assert_eq!(status_of(&out), ResultStatusCode::Ok);
        let start = e.session_start[&WRITER_CLIENT];
        let at = |ms: u64| {
            let total = u64::from(start.nanoseconds) + (ms % 1000) * 1_000_000;
            Some(SourceTime {
                seconds: start.seconds + (ms / 1000) as i32 + (total / 1_000_000_000) as i32,
                nanoseconds: (total % 1_000_000_000) as u32,
            })
        };
        let writes = &e.dds_port_mut().writes;
        assert_eq!(writes.len(), 3);
        assert_eq!(
            (
                writes[0].dispose,
                writes[0].unregister,
                writes[0].source_time
            ),
            (false, false, at(0))
        );
        assert_eq!(
            (
                writes[1].dispose,
                writes[1].unregister,
                writes[1].source_time
            ),
            (true, false, at(1500))
        );
        assert_eq!(
            (
                writes[2].dispose,
                writes[2].unregister,
                writes[2].source_time
            ),
            (false, true, at(90_000))
        );
        assert_eq!(writes[1].payload, vec![2; 4]);
    }

    /// Annex B.5.5: FORMAT_PACKED_SAMPLES has a common SampleInfo and a
    /// 4-octet SampleInfoDelta before each sample; §7.7.2: the offset of a
    /// sample is the base offset plus its delta (tenths of a second).
    #[test]
    fn packed_samples_resolve_their_deltas_against_the_base() {
        use zerodds_xrce::sample_info::{SampleInfoDelta, SampleInfoFlags as F};
        use zerodds_xrce::submessages::write_data::WriteDataPayloadPackedSamples;
        let mut e = engine_with_writer(SplitPort::new(4));
        let mut elements = Vec::new();
        for (delta, data) in [
            (
                SampleInfoDelta {
                    state: F(0),
                    seq_number_delta: 1,
                    timestamp_delta: 10,
                },
                [5u8; 4],
            ),
            (
                SampleInfoDelta {
                    state: F(F::INSTANCE_STATE_DISPOSED),
                    seq_number_delta: 2,
                    timestamp_delta: 25,
                },
                [6; 4],
            ),
        ] {
            delta
                .encode_into(Endianness::Little, &mut elements)
                .unwrap();
            elements.extend_from_slice(&data);
        }
        let out = send_write(
            &mut e,
            WriteDataPayloadPackedSamples {
                base: write_base(3),
                info_base: sample_info(0, 1000),
                sample_count: 2,
                elements,
            }
            .into_submessage()
            .unwrap(),
        );
        assert_eq!(status_of(&out), ResultStatusCode::Ok);
        let start = e.session_start[&WRITER_CLIENT];
        let writes = &e.dds_port_mut().writes;
        assert_eq!(writes.len(), 2);
        // 1000 ms + 10 * 100 ms = 2 s; 1000 ms + 25 * 100 ms = 3.5 s.
        let first = writes[0].source_time.unwrap();
        let second = writes[1].source_time.unwrap();
        let ns = |t: SourceTime| i64::from(t.seconds) * 1_000_000_000 + i64::from(t.nanoseconds);
        let base_ns = i64::from(start.seconds) * 1_000_000_000 + i64::from(start.nanoseconds);
        assert_eq!(ns(first) - base_ns, 2_000_000_000);
        assert_eq!(ns(second) - base_ns, 3_500_000_000);
        assert!(writes[1].dispose && !writes[0].dispose);
    }

    /// FORMAT_SAMPLE: the session_time_offset stamps the sample (§7.8.4.1:
    /// "allowing sending timestamps"); FORMAT_DATA has none and is not stamped.
    #[test]
    fn a_sample_is_stamped_with_the_session_time_plus_its_offset() {
        use zerodds_xrce::submessages::write_data::{WriteDataPayload, WriteDataPayloadSample};
        let mut e = engine_with_writer(SplitPort::new(4));
        let out = send_write(
            &mut e,
            WriteDataPayloadSample {
                base: write_base(4),
                info: sample_info(0, 2500),
                serialized_data: vec![9; 4],
            }
            .into_submessage()
            .unwrap(),
        );
        assert_eq!(status_of(&out), ResultStatusCode::Ok);
        send_write(
            &mut e,
            WriteDataPayload {
                base: write_base(5),
                serialized_data: vec![8; 4],
            }
            .into_submessage()
            .unwrap(),
        );
        let start = e.session_start[&WRITER_CLIENT];
        let writes = &e.dds_port_mut().writes;
        let t = writes[0].source_time.unwrap();
        let ns = |s: i32, n: u32| i64::from(s) * 1_000_000_000 + i64::from(n);
        assert_eq!(
            ns(t.seconds, t.nanoseconds) - ns(start.seconds, start.nanoseconds),
            2_500_000_000
        );
        assert_eq!(writes[1].source_time, None);
    }

    /// A sequence that does not split writes nothing and says why: a count
    /// the bytes cannot hold, bytes left behind the last sample, a truncated
    /// sample.
    #[test]
    fn a_sequence_that_does_not_split_writes_nothing() {
        use zerodds_xrce::submessages::write_data::WriteDataPayloadDataSeq;
        let mut e = engine_with_writer(SplitPort::new(4));
        for (count, elements) in [
            (1_000_000u32, vec![1u8; 8]),
            (2, vec![1; 9]),
            (2, vec![1; 6]),
        ] {
            let out = send_write(
                &mut e,
                WriteDataPayloadDataSeq {
                    base: write_base(6),
                    sample_count: count,
                    elements,
                }
                .into_submessage()
                .unwrap(),
            );
            assert_eq!(
                status_of(&out),
                ResultStatusCode::InvalidData,
                "count {count}"
            );
        }
        assert!(e.dds_port_mut().writes.is_empty());
        assert!(e.drain_events().iter().any(|ev| matches!(
            ev,
            SessionEvent::DdsPortFailed { object, .. } if *object == writer_oid()
        )));
    }

    /// Without a type that tells the samples apart (NullDdsPort knows none) a
    /// sequence format is answered with STATUS_ERR_INVALID_DATA, not
    /// guessed at.
    #[test]
    fn a_sequence_of_a_topic_of_unknown_type_is_refused() {
        use zerodds_xrce::submessages::write_data::WriteDataPayloadDataSeq;
        let mut e = engine_with_writer(NullDdsPort::new());
        let out = send_write(
            &mut e,
            WriteDataPayloadDataSeq {
                base: write_base(7),
                sample_count: 2,
                elements: vec![0; 8],
            }
            .into_submessage()
            .unwrap(),
        );
        assert_eq!(status_of(&out), ResultStatusCode::InvalidData);
    }

    /// §7.8.4.1: a dispose or unregister whose instance the agent cannot name
    /// (the topic's type is unknown) is answered with a STATUS and written
    /// nowhere; it used to be written as a plain update with only an event.
    #[test]
    fn a_dispose_the_agent_cannot_identify_is_answered_with_a_status() {
        use zerodds_xrce::sample_info::SampleInfoFlags as F;
        use zerodds_xrce::submessages::write_data::WriteDataPayloadSample;
        let mut e = engine_with_writer(NullDdsPort::new());
        for flag in [F::INSTANCE_STATE_DISPOSED, F::INSTANCE_STATE_UNREGISTERED] {
            let out = send_write(
                &mut e,
                WriteDataPayloadSample {
                    base: write_base(8),
                    info: sample_info(flag, 0),
                    serialized_data: vec![1, 2, 3, 4],
                }
                .into_submessage()
                .unwrap(),
            );
            assert_eq!(status_of(&out), ResultStatusCode::InvalidData);
        }
    }

    fn info_of(out: &[Outbound]) -> (BaseObjectReply, ObjectInfo) {
        assert_eq!(out.len(), 1, "one reply expected");
        let sm = &out[0].submessages[0];
        assert_eq!(sm.header.submessage_id, SubmessageId::Info, "an INFO");
        InfoPayload::try_from_submessage(sm)
            .unwrap()
            .parse(sm.header.body_endianness())
            .unwrap()
    }

    fn get_info_sm(object_id: ObjectId, request: u8, mask: InfoMask) -> Submessage {
        GetInfoPayload::new(
            BaseObjectRequest {
                request_id: [0, request],
                object_id,
            },
            mask,
            Endianness::Little,
        )
        .into_submessage()
        .unwrap()
    }

    /// §7.8.2.2, §8.3.5.7: GET_INFO(OBJK_AGENT) in a session is answered with
    /// an INFO that echoes request_id and object_id, with STATUS_OK, the
    /// AGENT_ActivityInfo (the addresses) and the AGENT_Representation.
    #[test]
    fn get_info_of_the_agent_in_a_session_returns_activity_and_representation() {
        use zerodds_xrce::object_id::OBJECTID_AGENT;
        use zerodds_xrce::transport_locator::TransportLocatorMedium;
        let locator = TransportLocator::Medium(TransportLocatorMedium {
            address: [10, 0, 0, 9],
            port: 2018,
        });
        let mut e = SessionEngine::new(
            NullDdsPort::new(),
            SessionEngineConfig {
                agent_vendor_id: [1, 15],
                listening_locators: vec![locator.clone()],
                ..test_config()
            },
        );
        connect(&mut e, KEY_A);
        let out = e.handle_message(
            Some(KEY_A),
            &session_header(),
            &[get_info_sm(OBJECTID_AGENT, 4, InfoMask::ALL)],
            Duration::ZERO,
        );
        let (reply, info) = info_of(&out);
        assert_eq!(reply.related_request.request_id, [0, 4]);
        assert_eq!(reply.related_request.object_id, OBJECTID_AGENT);
        assert_eq!(reply.result.status_code, ResultStatusCode::Ok);
        assert_eq!(
            info.activity,
            Some(ActivityInfoVariant::Agent {
                availability: 1,
                address_seq: vec![locator],
            })
        );
        assert_eq!(
            info.config,
            Some(ObjectConfig::Agent(AgentRepresentation {
                timestamp: None,
                version: XRCE_VERSION,
                vendor_id: [1, 15],
                properties: None,
            }))
        );
    }

    /// §7.8.2.2: get_info "may be used prior to establishing a Session": a
    /// GET_INFO(OBJK_AGENT) in a header without a session reaches the Root and
    /// is answered; any other object has no ProxyClient to ask.
    #[test]
    fn get_info_without_a_session_is_answered_for_the_agent_only() {
        use zerodds_xrce::object_id::OBJECTID_AGENT;
        let mut e = engine();
        let out = e.handle_message(
            None,
            &bootstrap_header(),
            &[get_info_sm(OBJECTID_AGENT, 1, InfoMask::ALL)],
            Duration::ZERO,
        );
        let (reply, info) = info_of(&out);
        assert_eq!(reply.result.status_code, ResultStatusCode::Ok);
        assert!(info.activity.is_some() && info.config.is_some());
        // Not a session: no client was registered by asking.
        assert_eq!(e.client_count(), 0);
        let out = e.handle_message(
            None,
            &bootstrap_header(),
            &[get_info_sm(participant_oid(), 2, InfoMask::ALL)],
            Duration::ZERO,
        );
        let (reply, info) = info_of(&out);
        assert_eq!(reply.result.status_code, ResultStatusCode::UnknownReference);
        assert_eq!(reply.related_request.object_id, participant_oid());
        assert_eq!(info, ObjectInfo::default());
    }

    /// §7.8.2.2, §8.4.4 Figure 10: the `client_info` of a GET_INFO(OBJK_AGENT)
    /// names the version of the client; another major version is answered
    /// with an INFO of STATUS_ERR_INCOMPATIBLE, with or without a session.
    #[test]
    fn get_info_of_the_agent_checks_the_version_in_client_info() {
        use zerodds_xrce::handshake::ClientRepresentation;
        use zerodds_xrce::object_id::OBJECTID_AGENT;
        use zerodds_xrce::object_info::ObjectConfig;
        let from_version = |version: [u8; 2]| {
            let client_info = ObjectInfo {
                activity: None,
                config: Some(ObjectConfig::Client(ClientRepresentation {
                    timestamp: None,
                    version,
                    vendor_id: [0, 0],
                    client_key: ClientKey([0; CLIENT_KEY_LEN]),
                    session_id: 0x81,
                    properties: None,
                    requested_mtu: None,
                })),
            };
            GetInfoPayload::with_client_info(
                BaseObjectRequest {
                    request_id: [0, 2],
                    object_id: OBJECTID_AGENT,
                },
                InfoMask::ALL,
                &client_info,
                Endianness::Little,
            )
            .unwrap()
            .into_submessage()
            .unwrap()
        };
        let mut e = engine();
        connect(&mut e, KEY_A);
        for (version, expected) in [
            ([9, 0], ResultStatusCode::Incompatible),
            (XRCE_VERSION, ResultStatusCode::Ok),
        ] {
            let in_session = e.handle_message(
                Some(KEY_A),
                &session_header(),
                &[from_version(version)],
                Duration::ZERO,
            );
            let (reply, info) = info_of(&in_session);
            assert_eq!(reply.result.status_code, expected);
            assert_eq!(info.config.is_some(), expected == ResultStatusCode::Ok);
            let without = e.handle_message(
                None,
                &bootstrap_header(),
                &[from_version(version)],
                Duration::ZERO,
            );
            assert_eq!(info_of(&without).0.result.status_code, expected);
        }
    }

    /// A WRITE_DATA, READ_DATA or DELETE whose body cannot be read is answered
    /// with STATUS_ERR_INVALID_DATA (against the request_id and object_id in
    /// its first four octets when they are there) and reported, not dropped
    /// without a word.
    #[test]
    fn a_request_that_cannot_be_read_is_answered_invalid_data() {
        let mut e = engine();
        connect(&mut e, KEY_A);
        e.drain_events();
        for (id, body) in [
            (SubmessageId::ReadData, vec![0, 8, 0x12, 0x35]),
            (SubmessageId::WriteData, vec![0, 5, 0x12]),
            (SubmessageId::ReadData, vec![0, 6]),
            (SubmessageId::Delete, vec![0, 7, 0x00]),
            (SubmessageId::GetInfo, vec![0, 9, 0x00]),
            (SubmessageId::WriteData, Vec::new()),
        ] {
            let sm = Submessage::new(
                id,
                zerodds_xrce::submessages::FLAG_E_LITTLE_ENDIAN,
                body.clone(),
            )
            .unwrap();
            let out = e.handle_message(Some(KEY_A), &session_header(), &[sm], Duration::ZERO);
            assert_eq!(
                status_of(&out),
                ResultStatusCode::InvalidData,
                "{id:?} {body:?}"
            );
            let (request_id, object_id) = reply_ids(&out);
            if body.len() >= 4 {
                assert_eq!(request_id, [0, 8]);
                assert_eq!(object_id.raw(), 0x1235);
            } else if body.len() >= 2 {
                assert_eq!(request_id, [body[0], body[1]]);
                assert_eq!(object_id, OBJECTID_INVALID);
            } else {
                assert_eq!((request_id, object_id), ([0, 0], OBJECTID_INVALID));
            }
        }
        let refused = e
            .drain_events()
            .iter()
            .filter(|ev| matches!(ev, SessionEvent::RequestUnreadable { .. }))
            .count();
        assert_eq!(refused, 6);
    }

    /// §8.3.5.15, §8.3.5.16: the agent sends a TIMESTAMP (on STREAMID_NONE,
    /// with its own time) and reads the TIMESTAMP_REPLY of the client: the
    /// four timestamps give the offset of the client's clock and the round
    /// trip.
    #[test]
    fn the_agent_sends_a_timestamp_and_reads_the_reply() {
        let mut e = engine();
        connect(&mut e, KEY_A);
        assert!(e.request_timestamp(KEY_B).is_none(), "no session");
        let ob = e.request_timestamp(KEY_A).expect("a TIMESTAMP");
        assert_eq!(ob.header.stream_id, StreamId::NONE);
        let sent = TimestampPayload::try_from_submessage(&ob.submessages[0]).unwrap();
        assert_ne!(sent.transmit_timestamp, TimePoint::default());
        // The client's clock is 5 s ahead; it answers at once.
        let ahead = TimePoint {
            seconds: sent.transmit_timestamp.seconds + 5,
            nanoseconds: sent.transmit_timestamp.nanoseconds,
        };
        let reply = TimestampReplyPayload {
            transmit_timestamp: ahead,
            receive_timestamp: ahead,
            originate_timestamp: sent.transmit_timestamp,
        }
        .into_submessage()
        .unwrap();
        e.drain_events();
        let out = e.handle_message(Some(KEY_A), &session_header(), &[reply], Duration::ZERO);
        assert!(out.is_empty());
        let events = e.drain_events();
        let [
            SessionEvent::ClockSynchronised {
                client,
                offset_nanos,
                round_trip_nanos,
            },
        ] = events.as_slice()
        else {
            panic!("expected one ClockSynchronised: {events:?}");
        };
        assert_eq!(*client, KEY_A);
        // t1 = t2 = t0 + 5 s, t3 a moment after t0: offset 5 s less half the
        // moment, round trip the moment.
        assert!(
            (4_900_000_000..=5_000_000_000).contains(offset_nanos),
            "{offset_nanos}"
        );
        assert!(
            (0..1_000_000_000).contains(round_trip_nanos),
            "{round_trip_nanos}"
        );
    }

    /// `timestamp_interval`: a TIMESTAMP to each client at the first tick and
    /// then every interval; none without an interval.
    #[test]
    fn tick_sends_a_timestamp_every_interval() {
        let timestamps = |out: &[Outbound]| {
            out.iter()
                .filter(|ob| ob.submessages[0].header.submessage_id == SubmessageId::Timestamp)
                .count()
        };
        let mut e = SessionEngine::new(
            NullDdsPort::new(),
            SessionEngineConfig {
                timestamp_interval: Some(Duration::from_secs(10)),
                ..test_config()
            },
        );
        connect(&mut e, KEY_A);
        assert_eq!(timestamps(&e.tick(Duration::ZERO)), 1);
        assert_eq!(timestamps(&e.tick(Duration::from_secs(5))), 0);
        assert_eq!(timestamps(&e.tick(Duration::from_secs(10))), 1);
        let mut silent = engine();
        connect(&mut silent, KEY_A);
        assert_eq!(timestamps(&silent.tick(Duration::ZERO)), 0);
    }

    /// §8.3.5.3: the ObjectKind of the object_id selects the Root, not the
    /// whole id 0xFFFD.
    #[test]
    fn any_object_id_of_kind_agent_is_the_root_in_a_session() {
        let mut e = engine();
        connect(&mut e, KEY_A);
        let other = ObjectId::new(0x010, ObjectKind::Agent).unwrap();
        let out = e.handle_message(
            Some(KEY_A),
            &session_header(),
            &[get_info_sm(other, 3, InfoMask::ALL)],
            Duration::ZERO,
        );
        let (reply, info) = info_of(&out);
        assert_eq!(reply.result.status_code, ResultStatusCode::Ok);
        assert_eq!(reply.related_request.object_id, other);
        assert!(info.activity.is_some());
    }

    /// §7.8.3.3, §8.4.12: GET_INFO of an object returns the ObjectVariant it
    /// was created with (kind, representation, trailing field) as `config`, and
    /// nothing as `activity` for a kind that has none; the mask selects.
    #[test]
    fn get_info_of_an_object_returns_the_object_variant_it_was_created_with() {
        let mut e = engine();
        connect(&mut e, KEY_A);
        let xml = "<dds><participant><domain_id>7</domain_id></participant></dds>";
        let out = e.handle_message(
            Some(KEY_A),
            &session_header(),
            &[create_variant_sm(
                participant_oid(),
                &ObjectVariant::ByXmlString(xml.to_string()),
                CreationMode::default(),
            )],
            Duration::ZERO,
        );
        assert_eq!(status_of(&out), ResultStatusCode::Ok);
        let out = e.handle_message(
            Some(KEY_A),
            &session_header(),
            &[get_info_sm(participant_oid(), 5, InfoMask::ALL)],
            Duration::ZERO,
        );
        let (reply, info) = info_of(&out);
        assert_eq!(reply.result.status_code, ResultStatusCode::Ok);
        assert_eq!(reply.related_request.request_id, [0, 5]);
        assert_eq!(info.activity, None);
        assert_eq!(
            info.config,
            Some(ObjectConfig::Variant {
                kind: ObjectKind::Participant,
                variant: ObjectVariant::ByXmlString(xml.to_string()),
                extra: ObjectVariantExtra {
                    domain_id: Some(0),
                    parent: None,
                },
            })
        );
        // Only the activity asked for: a participant has none.
        let out = e.handle_message(
            Some(KEY_A),
            &session_header(),
            &[get_info_sm(
                participant_oid(),
                6,
                InfoMask(InfoMask::INFO_ACTIVITY),
            )],
            Duration::ZERO,
        );
        assert_eq!(info_of(&out).1, ObjectInfo::default());
    }

    /// §7.8.3.3: an object that does not exist is STATUS_ERR_UNKNOWN_REFERENCE
    /// — in the INFO (§8.3.5.7.2), echoing the request.
    #[test]
    fn get_info_of_a_missing_object_is_an_info_with_unknown_reference() {
        let mut e = engine();
        connect(&mut e, KEY_A);
        let out = e.handle_message(
            Some(KEY_A),
            &session_header(),
            &[get_info_sm(participant_oid(), 8, InfoMask::ALL)],
            Duration::ZERO,
        );
        let (reply, info) = info_of(&out);
        assert_eq!(reply.result.status_code, ResultStatusCode::UnknownReference);
        assert_eq!(reply.related_request.request_id, [0, 8]);
        assert_eq!(reply.related_request.object_id, participant_oid());
        assert_eq!(info, ObjectInfo::default());
    }

    /// A GET_INFO without the InfoMask is invalid data, answered as an INFO.
    #[test]
    fn get_info_without_an_info_mask_is_invalid_data() {
        let mut e = engine();
        connect(&mut e, KEY_A);
        let short = GetInfoPayload {
            representation: BaseObjectRequest {
                request_id: [0, 3],
                object_id: participant_oid(),
            }
            .encode()
            .to_vec(),
        }
        .into_submessage()
        .unwrap();
        let out = e.handle_message(Some(KEY_A), &session_header(), &[short], Duration::ZERO);
        assert_eq!(
            info_of(&out).0.result.status_code,
            ResultStatusCode::InvalidData
        );
    }

    /// §7.7.12 DATAWRITER_ActivityInfo: the agent counts the samples written
    /// through a writer and keeps the sequence number of the last message.
    #[test]
    fn get_info_of_a_data_writer_reports_its_activity() {
        use zerodds_xrce::submessages::write_data::WriteDataPayload;
        let mut e = engine_with_writer(SplitPort::new(4));
        for seq in [3u16, 9] {
            e.handle_message(
                Some(WRITER_CLIENT),
                &header_on(0x81, 1, seq),
                &[WriteDataPayload {
                    base: write_base(1),
                    serialized_data: vec![1, 2, 3, 4],
                }
                .into_submessage()
                .unwrap()],
                Duration::ZERO,
            );
        }
        let out = e.handle_message(
            Some(WRITER_CLIENT),
            &session_header(),
            &[get_info_sm(writer_oid(), 2, InfoMask::ALL)],
            Duration::ZERO,
        );
        let (reply, info) = info_of(&out);
        assert_eq!(reply.result.status_code, ResultStatusCode::Ok);
        assert_eq!(
            info.activity,
            Some(ActivityInfoVariant::DataWriter {
                sample_seq_num: 2,
                stream_seq_num: 9,
            })
        );
        assert!(matches!(
            info.config,
            Some(ObjectConfig::Variant {
                kind: ObjectKind::DataWriter,
                ..
            })
        ));
    }

    /// DATAREADER_ActivityInfo: without a read delivering on a reliable
    /// stream the client has acknowledged nothing.
    #[test]
    fn get_info_of_a_data_reader_reports_its_activity() {
        let mut e = engine();
        connect(&mut e, KEY_A);
        create_participant_topic_reader(&mut e, KEY_A);
        let reader = ObjectId::new(0x002, ObjectKind::DataReader).unwrap();
        let out = e.handle_message(
            Some(KEY_A),
            &session_header(),
            &[get_info_sm(reader, 2, InfoMask::ALL)],
            Duration::ZERO,
        );
        let (reply, info) = info_of(&out);
        assert_eq!(reply.result.status_code, ResultStatusCode::Ok);
        assert_eq!(
            info.activity,
            Some(ActivityInfoVariant::DataReader {
                highest_acked_num: 0
            })
        );
    }
}
