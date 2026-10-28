// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Session-level tests of DDS-XRCE 1.0 sentences that no other test reaches
//! through the public API of [`SessionEngine`]: the names of the types a
//! CREATE refers to, the type of a binary Topic, the XML of a Subscriber and
//! a DataReader, `STATUS_ERR_RESOURCES`, the DELETE of a missing object and
//! the stream of an ACKNACK and a TIMESTAMP_REPLY. Each test names the
//! sentence it covers.

#![cfg(feature = "daemon")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use zerodds_xrce::encoding::Endianness;
use zerodds_xrce::handshake::{ClientRepresentation, XRCE_VERSION};
use zerodds_xrce::header::{CLIENT_KEY_LEN, ClientKey, MessageHeader, SessionId, StreamId};
use zerodds_xrce::object_id::ObjectId;
use zerodds_xrce::object_info::{BaseObjectReply, BaseObjectRequest, ResultStatusCode};
use zerodds_xrce::object_kind::ObjectKind;
use zerodds_xrce::object_repr::{ObjectVariant, ObjectVariantExtra};
use zerodds_xrce::serial_number::SerialNumber16;
use zerodds_xrce::submessages::timestamp::TimePoint;
use zerodds_xrce::submessages::{
    AckNackPayload, CreateClientPayload, CreatePayload, DeletePayload, FragmentPayload,
    HeartbeatPayload, StatusAgentPayload, StatusPayload, Submessage, SubmessageId,
    TimestampPayload, TimestampReplyPayload,
};

use zerodds_xrce_agent::daemon::dds_port::{
    ContainerSpec, DdsPort, DdsPortError, EndpointQos, History, NullDdsPort, ObjectKey,
    PolledSample, SourceTime, TopicSpec, WriteOutcome,
};
use zerodds_xrce_agent::daemon::session::{
    Outbound, SessionEngine, SessionEngineConfig, SessionEvent,
};

type Key = [u8; CLIENT_KEY_LEN];

// ---------------------------------------------------------------------------
// A DdsPort that records what the engine asks of it.
// ---------------------------------------------------------------------------

#[derive(Default)]
struct RecordingPort {
    inner: NullDdsPort,
    /// Topic name and the name of the root type of its `known_type`.
    topics: Vec<(String, Option<String>)>,
    /// `true` = Publisher, `false` = Subscriber.
    containers: Vec<(bool, ContainerSpec)>,
    /// Topic name of each DataReader and the QoS it was created with.
    readers: Vec<(String, EndpointQos)>,
    topic_names: BTreeMap<ObjectKey, String>,
}

impl DdsPort for RecordingPort {
    fn create_participant(&mut self, key: ObjectKey, domain_id: i32) -> Result<(), DdsPortError> {
        self.inner.create_participant(key, domain_id)
    }
    fn create_topic(
        &mut self,
        key: ObjectKey,
        participant: ObjectKey,
        spec: TopicSpec,
    ) -> Result<(), DdsPortError> {
        let root_name = spec.known_type.as_ref().and_then(|k| match &k.root {
            zerodds_types::CompleteTypeObject::Struct(s) => Some(s.header.detail.type_name.clone()),
            _ => None,
        });
        self.topics.push((spec.topic_name.clone(), root_name));
        self.topic_names.insert(key, spec.topic_name.clone());
        self.inner.create_topic(key, participant, spec)
    }
    fn create_publisher(
        &mut self,
        key: ObjectKey,
        participant: ObjectKey,
        spec: ContainerSpec,
    ) -> Result<(), DdsPortError> {
        self.containers.push((true, spec.clone()));
        self.inner.create_publisher(key, participant, spec)
    }
    fn create_subscriber(
        &mut self,
        key: ObjectKey,
        participant: ObjectKey,
        spec: ContainerSpec,
    ) -> Result<(), DdsPortError> {
        self.containers.push((false, spec.clone()));
        self.inner.create_subscriber(key, participant, spec)
    }
    fn create_writer(
        &mut self,
        key: ObjectKey,
        topic: ObjectKey,
        publisher: ObjectKey,
        qos: EndpointQos,
    ) -> Result<(), DdsPortError> {
        self.inner.create_writer(key, topic, publisher, qos)
    }
    fn create_reader(
        &mut self,
        key: ObjectKey,
        topic: ObjectKey,
        subscriber: ObjectKey,
        qos: EndpointQos,
    ) -> Result<(), DdsPortError> {
        let name = self.topic_names.get(&topic).cloned().unwrap_or_default();
        self.readers.push((name, qos.clone()));
        self.inner.create_reader(key, topic, subscriber, qos)
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
        writer: ObjectKey,
        bytes: &[u8],
        big_endian: bool,
    ) -> Result<usize, DdsPortError> {
        self.inner.sample_length(writer, bytes, big_endian)
    }
    fn set_read_filter(
        &mut self,
        reader: ObjectKey,
        filter: Option<&str>,
    ) -> Result<(), DdsPortError> {
        self.inner.set_read_filter(reader, filter)
    }
    fn poll_samples(&mut self, reader: ObjectKey, max: usize) -> Vec<PolledSample> {
        self.inner.poll_samples(reader, max)
    }
    fn delete(&mut self, key: ObjectKey) {
        self.inner.delete(key);
    }
    fn unregister_client(&mut self, client_key: [u8; CLIENT_KEY_LEN]) {
        self.inner.unregister_client(client_key);
    }
}

// ---------------------------------------------------------------------------
// Helpers.
// ---------------------------------------------------------------------------

fn config() -> SessionEngineConfig {
    SessionEngineConfig {
        // A topic of a type the agent does not hold is refused by default
        // (§7.7.3.7.3); the tests that look at that say so.
        forward_unknown_topic_types: true,
        ..SessionEngineConfig::default()
    }
}

fn bootstrap_header() -> MessageHeader {
    MessageHeader::without_client_key(SessionId(0x80), StreamId::NONE, SerialNumber16::new(0))
        .unwrap()
}

/// A message of the session `connect` opens (session_id 0x81, no ClientKey,
/// STREAMID_NONE).
fn session_header() -> MessageHeader {
    MessageHeader::without_client_key(SessionId(0x81), StreamId::NONE, SerialNumber16::new(0))
        .unwrap()
}

/// A message of the same session on the reliable stream 0x80.
fn reliable_header(seq: u16) -> MessageHeader {
    MessageHeader {
        session_id: SessionId(0x81),
        stream_id: StreamId(0x80),
        sequence_nr: SerialNumber16::new(seq),
        client_key: None,
    }
}

fn create_client_sm(client_key: Key) -> Submessage {
    CreateClientPayload {
        representation: ClientRepresentation {
            timestamp: None,
            version: XRCE_VERSION,
            vendor_id: [0, 0],
            client_key: ClientKey(client_key),
            session_id: 0x81,
            properties: None,
            requested_mtu: None,
        },
    }
    .into_submessage()
    .unwrap()
}

fn connect<P: DdsPort>(e: &mut SessionEngine<P>, client: Key) -> Vec<Outbound> {
    e.handle_message(
        None,
        &bootstrap_header(),
        &[create_client_sm(client)],
        Duration::ZERO,
    )
}

fn create_sm(oid: ObjectId, variant: &ObjectVariant, extra: ObjectVariantExtra) -> Submessage {
    let base = BaseObjectRequest {
        request_id: [0, 1],
        object_id: oid,
    };
    let mut body = base.encode().to_vec();
    body.extend_from_slice(
        &variant
            .encode_for_kind(oid.kind().unwrap(), extra, Endianness::Little)
            .unwrap(),
    );
    CreatePayload {
        representation: body,
        reuse: false,
        replace: false,
    }
    .into_submessage()
    .unwrap()
}

fn delete_sm(oid: ObjectId) -> Submessage {
    DeletePayload {
        representation: BaseObjectRequest {
            request_id: [0, 2],
            object_id: oid,
        }
        .encode()
        .to_vec(),
    }
    .into_submessage()
    .unwrap()
}

fn status_of(out: &[Outbound]) -> ResultStatusCode {
    assert_eq!(out.len(), 1, "one reply expected: {out:?}");
    let reply = StatusPayload::try_from_submessage(&out[0].submessages[0]).unwrap();
    BaseObjectReply::decode(&reply.representation)
        .unwrap()
        .0
        .result
        .status_code
}

fn agent_status_of(out: &[Outbound]) -> ResultStatusCode {
    assert_eq!(out.len(), 1, "one reply expected: {out:?}");
    let reply = StatusAgentPayload::try_from_submessage(&out[0].submessages[0]).unwrap();
    reply
        .result_and_position(Endianness::Little)
        .unwrap()
        .0
        .status_code
}

fn parent(oid: ObjectId) -> ObjectVariantExtra {
    ObjectVariantExtra {
        domain_id: None,
        parent: Some(oid),
    }
}

fn participant_oid() -> ObjectId {
    ObjectId::new(0x001, ObjectKind::Participant).unwrap()
}

fn subscriber_oid() -> ObjectId {
    ObjectId::new(0x001, ObjectKind::Subscriber).unwrap()
}

fn xml(text: &str) -> ObjectVariant {
    ObjectVariant::ByXmlString(text.to_string())
}

/// CREATE of `oid` on the unreliable stream; the code of the STATUS.
fn create<P: DdsPort>(
    e: &mut SessionEngine<P>,
    client: Key,
    oid: ObjectId,
    variant: &ObjectVariant,
    extra: ObjectVariantExtra,
) -> ResultStatusCode {
    let out = e.handle_message(
        Some(client),
        &session_header(),
        &[create_sm(oid, variant, extra)],
        Duration::ZERO,
    );
    status_of(&out)
}

fn delete<P: DdsPort>(e: &mut SessionEngine<P>, client: Key, oid: ObjectId) -> ResultStatusCode {
    let out = e.handle_message(
        Some(client),
        &session_header(),
        &[delete_sm(oid)],
        Duration::ZERO,
    );
    status_of(&out)
}

fn create_participant<P: DdsPort>(e: &mut SessionEngine<P>, client: Key) {
    let code = create(
        e,
        client,
        participant_oid(),
        &xml("<dds><participant><domain_id>0</domain_id></participant></dds>"),
        ObjectVariantExtra {
            domain_id: Some(0),
            parent: None,
        },
    );
    assert_eq!(code, ResultStatusCode::Ok);
}

/// The CompleteTypeObject of `name` in the IDL text.
fn type_object(idl: &str, file: &str, name: &str) -> zerodds_types::CompleteTypeObject {
    let ast = Arc::new(zerodds_wire_probe::encode::model_from_idl(idl, file).unwrap());
    let ty = zerodds_types::dynamic::DynamicType::find(&ast, name).unwrap();
    zerodds_types::from_ast::complete_type_object(&ast, ty.id()).unwrap()
}

fn identifier_of(root: &zerodds_types::CompleteTypeObject) -> zerodds_types::TypeIdentifier {
    zerodds_types::hash::to_hashed_type_identifier(&zerodds_types::TypeObject::Complete(
        root.clone(),
    ))
    .unwrap()
}

// ---------------------------------------------------------------------------
// §7.7.3.3.1
// ---------------------------------------------------------------------------

/// §7.7.3.3.1, S. 30: "the object_reference field shall contain the fully
/// qualified name of an XRCE Type known to the XRCE Agent. The fully
/// qualified name is composed of the name of the type prepended by the names
/// of the enclosing modules."
///
/// The agent knows `MyModule::ShapeType` (`known_types`, the content of
/// `type_files`). CREATE(OBJK_TYPE) by reference with that name is STATUS_OK
/// and stores the object; a name it does not know, and the name without its
/// module, are STATUS_ERR_UNKNOWN_REFERENCE and store nothing.
#[test]
fn a_type_by_reference_names_a_known_type_by_its_fully_qualified_name() {
    let root = type_object(
        "module MyModule { @final struct ShapeType { @key string color; long x; }; };",
        "spec-items-session-module.idl",
        "MyModule::ShapeType",
    );
    let mut known = BTreeMap::new();
    known.insert("MyModule::ShapeType".to_string(), root);
    let mut e = SessionEngine::new(
        RecordingPort::default(),
        SessionEngineConfig {
            known_types: known,
            ..config()
        },
    );
    let client = [0x51; CLIENT_KEY_LEN];
    connect(&mut e, client);

    let by_reference =
        |e: &mut SessionEngine<RecordingPort>, raw: u16, name: &str| -> (ObjectId, _) {
            let oid = ObjectId::new(raw, ObjectKind::Type).unwrap();
            let code = create(
                e,
                client,
                oid,
                &ObjectVariant::ByReference(name.to_string()),
                ObjectVariantExtra::default(),
            );
            (oid, code)
        };

    let (unknown, code) = by_reference(&mut e, 0x001, "MyModule::Nobody");
    assert_eq!(code, ResultStatusCode::UnknownReference);
    let (unqualified, code) = by_reference(&mut e, 0x002, "ShapeType");
    assert_eq!(code, ResultStatusCode::UnknownReference);
    let (wrong_module, code) = by_reference(&mut e, 0x003, "OtherModule::ShapeType");
    assert_eq!(code, ResultStatusCode::UnknownReference);
    // Nothing of the three refused CREATEs is stored.
    for oid in [unknown, unqualified, wrong_module] {
        assert_eq!(
            delete(&mut e, client, oid),
            ResultStatusCode::UnknownReference
        );
    }

    let (known_oid, code) = by_reference(&mut e, 0x004, "MyModule::ShapeType");
    assert_eq!(code, ResultStatusCode::Ok);
    assert_eq!(delete(&mut e, client, known_oid), ResultStatusCode::Ok);
}

// ---------------------------------------------------------------------------
// §7.7.3.7.3
// ---------------------------------------------------------------------------

/// §7.7.3.7.3, S. 34: "Either type_reference or type_identifier may be used
/// to identify the XRCE Type associated with the Topic. Either member may be
/// omitted, but not both. If both are present the type_identifier shall take
/// precedence."
///
/// The agent knows `Alpha` and `Beta`; it is told to refuse a topic of an
/// unknown type (`forward_unknown_topic_types: false`).
/// - a type_reference of an unknown type and the type_identifier of `Alpha`:
///   STATUS_OK, the topic has the type `Alpha`;
/// - a type_reference of the known `Beta` and the type_identifier of `Alpha`:
///   STATUS_OK, the topic has the type `Alpha`;
/// - neither member: STATUS_ERR_INVALID_DATA, no topic is created.
#[test]
fn a_binary_topic_takes_its_type_from_the_type_identifier_and_needs_one_of_the_two() {
    use zerodds_wire::{Endianness as WireEndianness, Writer};
    let idl = "@final struct Alpha { @key long id; }; @final struct Beta { string text; };";
    let alpha = type_object(idl, "spec-items-session-topic.idl", "Alpha");
    let beta = type_object(idl, "spec-items-session-topic.idl", "Beta");
    let alpha_identifier = identifier_of(&alpha);
    assert_ne!(alpha_identifier, identifier_of(&beta));
    let mut known = BTreeMap::new();
    known.insert("Alpha".to_string(), alpha);
    known.insert("Beta".to_string(), beta);
    let mut e = SessionEngine::new(
        RecordingPort::default(),
        SessionEngineConfig {
            known_types: known,
            forward_unknown_topic_types: false,
            ..SessionEngineConfig::default()
        },
    );
    let client = [0x52; CLIENT_KEY_LEN];
    connect(&mut e, client);
    create_participant(&mut e, client);

    // OBJK_Topic_Binary of the clause: topic_name, optional type_reference,
    // optional type_identifier.
    let blob = |name: &str,
                reference: Option<&str>,
                identifier: Option<&zerodds_types::TypeIdentifier>| {
        let mut w = Writer::with_max_align(WireEndianness::Little, 4);
        w.write_string(name);
        w.write_bool(reference.is_some());
        if let Some(r) = reference {
            w.write_string(r);
        }
        w.write_bool(identifier.is_some());
        if let Some(i) = identifier {
            i.encode_into(&mut w).unwrap();
        }
        w.finish()
    };
    let topic = |e: &mut SessionEngine<RecordingPort>,
                 raw: u16,
                 bytes: Vec<u8>|
     -> (ObjectId, ResultStatusCode) {
        let oid = ObjectId::new(raw, ObjectKind::Topic).unwrap();
        let code = create(
            e,
            client,
            oid,
            &ObjectVariant::InBinary(bytes),
            parent(participant_oid()),
        );
        (oid, code)
    };

    let (_, code) = topic(
        &mut e,
        0x003,
        blob("T1", Some("Nobody"), Some(&alpha_identifier)),
    );
    assert_eq!(code, ResultStatusCode::Ok);
    let (_, code) = topic(
        &mut e,
        0x004,
        blob("T2", Some("Beta"), Some(&alpha_identifier)),
    );
    assert_eq!(code, ResultStatusCode::Ok);
    assert_eq!(
        e.dds_port_mut().topics,
        vec![
            ("T1".to_string(), Some("Alpha".to_string())),
            ("T2".to_string(), Some("Alpha".to_string())),
        ]
    );

    let (neither, code) = topic(&mut e, 0x005, blob("T3", None, None));
    assert_eq!(code, ResultStatusCode::InvalidData);
    assert_eq!(e.dds_port_mut().topics.len(), 2, "no topic for neither");
    assert_eq!(
        delete(&mut e, client, neither),
        ResultStatusCode::UnknownReference
    );
}

// ---------------------------------------------------------------------------
// §7.7.3.9.1
// ---------------------------------------------------------------------------

/// A client with a participant and a topic `Square`.
fn client_with_topic(e: &mut SessionEngine<RecordingPort>, client: Key) {
    connect(e, client);
    create_participant(e, client);
    let code = create(
        e,
        client,
        ObjectId::new(0x003, ObjectKind::Topic).unwrap(),
        &xml("<dds><topic name=\"Square\" type_name=\"ShapeType\"/></dds>"),
        parent(participant_oid()),
    );
    assert_eq!(code, ResultStatusCode::Ok);
}

/// §7.7.3.9.1, S. 35-36: "the string_representation field shall contain a
/// single <subscriber> top-level XML element [...]. Note that the XML
/// representation of a Subscriber allows specifying Qos policies and
/// including nested DataReader objects."
///
/// CREATE of a Subscriber from `<subscriber name=..>` with a
/// `<subscriber_qos>` (partition, presentation) and a nested `<data_reader>`
/// is STATUS_OK. The DDS side gets one Subscriber with the partition and the
/// presentation, the nested DataReader exists as an object of the client
/// (DELETE of it is STATUS_OK, a second one is STATUS_ERR_UNKNOWN_REFERENCE)
/// and it was created on the topic `Square` with the partition of its
/// Subscriber.
#[test]
fn a_subscriber_in_xml_has_its_qos_and_its_nested_data_reader() {
    let mut e = SessionEngine::new(RecordingPort::default(), config());
    let client = [0x53; CLIENT_KEY_LEN];
    client_with_topic(&mut e, client);

    let code = create(
        &mut e,
        client,
        subscriber_oid(),
        &xml(r#"<subscriber name="MySubscriber">
                  <subscriber_qos>
                    <partition><name><element>g1</element></name></partition>
                    <presentation>
                      <access_scope>GROUP_PRESENTATION_QOS</access_scope>
                      <coherent_access>true</coherent_access>
                      <ordered_access>true</ordered_access>
                    </presentation>
                  </subscriber_qos>
                  <data_reader name="MyReader" topic_ref="Square"/>
                </subscriber>"#),
        parent(participant_oid()),
    );
    assert_eq!(code, ResultStatusCode::Ok);

    let port = e.dds_port_mut();
    assert_eq!(port.containers.len(), 1);
    let (is_publisher, spec) = &port.containers[0];
    assert!(!is_publisher, "the container is a Subscriber");
    assert_eq!(spec.partitions, vec!["g1".to_string()]);
    assert_eq!(
        spec.presentation.access_scope,
        zerodds_qos::PresentationAccessScope::Group
    );
    assert!(spec.presentation.coherent_access);
    assert!(spec.presentation.ordered_access);
    assert_eq!(port.readers.len(), 1, "the nested DataReader was created");
    assert_eq!(port.readers[0].0, "Square");
    assert_eq!(port.readers[0].1.partitions, vec!["g1".to_string()]);

    let reader = ObjectId::from_reference("MyReader", ObjectKind::DataReader);
    assert_eq!(delete(&mut e, client, reader), ResultStatusCode::Ok);
    assert_eq!(
        delete(&mut e, client, reader),
        ResultStatusCode::UnknownReference
    );
}

// ---------------------------------------------------------------------------
// §7.7.3.11.1
// ---------------------------------------------------------------------------

/// §7.7.3.11.1, S. 38: "the string_representation field shall contain a
/// single <data_reader> top-level XML element [...]. The XML representation
/// of a DataReader allows specifying Qos policies. [...] The XML
/// representation of a DataReader may also contain time-based and
/// content-based filters." The example is `<content_filter name="MyFilter">
/// <expression> x &gt; 5 </expression> </content_filter>`.
///
/// CREATE of a DataReader from `<data_reader>` with a `<data_reader_qos>`
/// (reliability RELIABLE, history KEEP_LAST 7) and a `<content_filter>` is
/// STATUS_OK; the DDS side gets a reader on the topic `Square` that is
/// reliable, keeps the last 7 samples and has the filter expression `x > 100`.
#[test]
fn a_data_reader_in_xml_has_its_qos_and_its_content_filter() {
    let mut e = SessionEngine::new(RecordingPort::default(), config());
    let client = [0x54; CLIENT_KEY_LEN];
    client_with_topic(&mut e, client);
    let code = create(
        &mut e,
        client,
        subscriber_oid(),
        &xml("<subscriber name=\"MySubscriber\"/>"),
        parent(participant_oid()),
    );
    assert_eq!(code, ResultStatusCode::Ok);

    let reader = ObjectId::new(0x001, ObjectKind::DataReader).unwrap();
    let code = create(
        &mut e,
        client,
        reader,
        &xml(r#"<data_reader topic_ref="Square">
                  <data_reader_qos>
                    <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
                    <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>7</depth></history>
                  </data_reader_qos>
                  <content_filter name="MyFilter">
                    <expression> x &gt; 100 </expression>
                  </content_filter>
                </data_reader>"#),
        parent(subscriber_oid()),
    );
    assert_eq!(code, ResultStatusCode::Ok);

    let readers = &e.dds_port_mut().readers;
    assert_eq!(readers.len(), 1);
    assert_eq!(readers[0].0, "Square");
    let qos = &readers[0].1;
    assert!(qos.reliable);
    assert_eq!(qos.history, History::KeepLast(7));
    assert_eq!(qos.content_filter.as_deref(), Some("x > 100"));
}

// ---------------------------------------------------------------------------
// §7.7.7 Table 2
// ---------------------------------------------------------------------------

/// §7.7.7, Table 2, S. 41: STATUS_ERR_RESOURCES "Indicates a failure in the
/// execution of an operation due to a resource error on the Agent." Reached
/// by `max_clients`: with one client held, the CREATE_CLIENT of another
/// ClientKey is answered STATUS_AGENT with STATUS_ERR_RESOURCES; the agent
/// holds no second client and drops the messages of its key.
#[test]
fn create_client_over_max_clients_is_a_resource_error() {
    let mut e = SessionEngine::new(
        NullDdsPort::new(),
        SessionEngineConfig {
            max_clients: 1,
            ..config()
        },
    );
    let first = [0x61; CLIENT_KEY_LEN];
    let second = [0x62; CLIENT_KEY_LEN];
    assert!(agent_status_of(&connect(&mut e, first)).is_success());
    assert_eq!(e.client_count(), 1);

    assert_eq!(
        agent_status_of(&connect(&mut e, second)),
        ResultStatusCode::Resources
    );
    assert_eq!(e.client_count(), 1, "the second client is not stored");
    let out = e.handle_message(
        Some(second),
        &session_header(),
        &[delete_sm(participant_oid())],
        Duration::ZERO,
    );
    assert!(out.is_empty(), "a message of the refused client is dropped");
    assert!(
        e.drain_events()
            .iter()
            .any(|ev| matches!(ev, SessionEvent::NoClient { client } if *client == second))
    );

    // The client that is held can still be reconnected: it is not a new one.
    assert!(agent_status_of(&connect(&mut e, first)).is_success());
}

/// §7.7.7, Table 2, S. 41: STATUS_ERR_RESOURCES, reached by
/// `max_objects_per_client`: with two objects held, the third CREATE is
/// answered STATUS_ERR_RESOURCES and the object is not stored.
#[test]
fn create_over_max_objects_per_client_is_a_resource_error() {
    let mut e = SessionEngine::new(
        NullDdsPort::new(),
        SessionEngineConfig {
            max_objects_per_client: 2,
            ..config()
        },
    );
    let client = [0x63; CLIENT_KEY_LEN];
    connect(&mut e, client);
    create_participant(&mut e, client);
    let topic = |raw: u16| ObjectId::new(raw, ObjectKind::Topic).unwrap();
    let make = |e: &mut SessionEngine<NullDdsPort>, raw: u16, name: &str| {
        create(
            e,
            client,
            topic(raw),
            &xml(&format!(
                "<dds><topic name=\"{name}\" type_name=\"Ty\"/></dds>"
            )),
            parent(participant_oid()),
        )
    };
    assert_eq!(make(&mut e, 0x003, "A"), ResultStatusCode::Ok);
    assert_eq!(make(&mut e, 0x004, "B"), ResultStatusCode::Resources);
    assert_eq!(
        delete(&mut e, client, topic(0x004)),
        ResultStatusCode::UnknownReference,
        "the refused object is not stored"
    );
    // Room is made by deleting one; then the CREATE succeeds.
    assert_eq!(delete(&mut e, client, topic(0x003)), ResultStatusCode::Ok);
    assert_eq!(make(&mut e, 0x004, "B"), ResultStatusCode::Ok);
}

/// §7.7.7, Table 2, S. 41: STATUS_ERR_RESOURCES, reached by the reorder
/// buffer of the receiver of a reliable stream (64 messages): the sequence
/// numbers 1 to 64 arrive while 0 is missing and are held; the 65th message,
/// a CREATE, is answered STATUS_ERR_RESOURCES and is not stored: when 0
/// arrives the 65 messages 0 to 64 are released, and the participant of the
/// refused CREATE does not exist.
#[test]
fn a_full_reorder_buffer_of_a_reliable_stream_is_a_resource_error() {
    let mut e = SessionEngine::new(NullDdsPort::new(), config());
    let client = [0x64; CLIENT_KEY_LEN];
    connect(&mut e, client);
    let timestamp = || {
        TimestampPayload {
            transmit_timestamp: TimePoint {
                seconds: 1,
                nanoseconds: 2,
            },
        }
        .into_submessage()
        .unwrap()
    };
    for seq in 1..=64u16 {
        let out = e.handle_message(
            Some(client),
            &reliable_header(seq),
            &[timestamp()],
            Duration::ZERO,
        );
        assert!(out.is_empty(), "seq {seq} waits for seq 0");
    }
    let out = e.handle_message(
        Some(client),
        &reliable_header(65),
        &[create_sm(
            participant_oid(),
            &xml("<dds><participant><domain_id>0</domain_id></participant></dds>"),
            ObjectVariantExtra {
                domain_id: Some(0),
                parent: None,
            },
        )],
        Duration::ZERO,
    );
    assert_eq!(status_of(&out), ResultStatusCode::Resources);

    // Seq 0 releases the 64 held messages with it; the 65th was not held.
    let out = e.handle_message(
        Some(client),
        &reliable_header(0),
        &[timestamp()],
        Duration::ZERO,
    );
    let replies = out
        .iter()
        .flat_map(|o| &o.submessages)
        .filter(|sm| sm.header.submessage_id == SubmessageId::TimestampReply)
        .count();
    assert_eq!(replies, 65, "the TIMESTAMPs 0 to 64 were all processed");
    assert_eq!(
        delete(&mut e, client, participant_oid()),
        ResultStatusCode::UnknownReference,
        "the refused CREATE is not stored"
    );
}

/// §7.7.7, Table 2, S. 41: STATUS_ERR_RESOURCES, reached by a FRAGMENT
/// reassembly over its limit (256 fragments): the first fragment of a run
/// holds a CREATE of a participant, 255 more of four octets follow, the 257th
/// is answered STATUS_ERR_RESOURCES, and the participant is not created.
#[test]
fn a_fragment_run_over_its_limit_is_a_resource_error() {
    let mut e = SessionEngine::new(NullDdsPort::new(), config());
    let client = [0x65; CLIENT_KEY_LEN];
    connect(&mut e, client);
    let create_bytes = {
        let sm = create_sm(
            participant_oid(),
            &xml("<dds><participant><domain_id>0</domain_id></participant></dds>"),
            ObjectVariantExtra {
                domain_id: Some(0),
                parent: None,
            },
        );
        let mut bytes = sm.header.to_bytes().to_vec();
        bytes.extend_from_slice(&sm.body);
        bytes
    };
    let fragment = |data: Vec<u8>, last: bool| {
        FragmentPayload {
            data,
            last_fragment: last,
        }
        .into_submessage()
        .unwrap()
    };
    let send = |e: &mut SessionEngine<NullDdsPort>, seq: u16, sm: Submessage| {
        e.handle_message(Some(client), &reliable_header(seq), &[sm], Duration::ZERO)
    };
    let out = send(&mut e, 0, fragment(create_bytes, false));
    assert!(out.is_empty());
    for seq in 1..256u16 {
        let out = send(&mut e, seq, fragment(vec![0; 4], false));
        assert!(out.is_empty(), "fragment {seq} is within the limit");
    }
    let out = send(&mut e, 256, fragment(vec![0; 4], true));
    assert_eq!(status_of(&out), ResultStatusCode::Resources);
    assert_eq!(
        delete(&mut e, client, participant_oid()),
        ResultStatusCode::UnknownReference,
        "the run was not completed into a CREATE"
    );
}

// ---------------------------------------------------------------------------
// §7.8.3.4
// ---------------------------------------------------------------------------

/// §7.8.3.4, S. 53: "If the object does not exist
/// STATUS_ERR_UNKNOWN_REFERENCE shall be returned."
///
/// DELETE of an ordinary object the client never created is
/// STATUS_ERR_UNKNOWN_REFERENCE. After a DELETE of an object it did create
/// (STATUS_OK), a second DELETE of it is STATUS_ERR_UNKNOWN_REFERENCE too.
#[test]
fn delete_of_an_object_that_does_not_exist_is_an_unknown_reference() {
    let mut e = SessionEngine::new(NullDdsPort::new(), config());
    let client = [0x66; CLIENT_KEY_LEN];
    connect(&mut e, client);

    let never = ObjectId::new(0x007, ObjectKind::Topic).unwrap();
    assert_eq!(
        delete(&mut e, client, never),
        ResultStatusCode::UnknownReference
    );
    assert_eq!(
        delete(&mut e, client, participant_oid()),
        ResultStatusCode::UnknownReference
    );

    create_participant(&mut e, client);
    assert_eq!(
        delete(&mut e, client, participant_oid()),
        ResultStatusCode::Ok
    );
    assert_eq!(
        delete(&mut e, client, participant_oid()),
        ResultStatusCode::UnknownReference
    );
}

// ---------------------------------------------------------------------------
// §8.3.5.11, §8.3.5.16
// ---------------------------------------------------------------------------

/// §8.3.5.11, S. 74: "The ACKNACK submessage does not belong to any stream,
/// for this reason the MessageHeader shall have the streamId set to
/// STREAM_ID_NONE".
///
/// The agent answers a HEARTBEAT of the reliable stream 0x80 with an ACKNACK
/// whose MessageHeader has `StreamId::NONE`; the ACKNACK payload names the
/// reliable stream. The HEARTBEAT is sent in a message of STREAMID_NONE and
/// in a message of the reliable stream itself.
#[test]
fn the_acknack_of_the_agent_is_on_streamid_none() {
    let mut e = SessionEngine::new(NullDdsPort::new(), config());
    let client = [0x67; CLIENT_KEY_LEN];
    connect(&mut e, client);
    let heartbeat = || {
        HeartbeatPayload {
            first_unacked_seq_nr: 0,
            last_unacked_seq_nr: 0,
            stream_id: 0x80,
        }
        .into_submessage()
        .unwrap()
    };

    let out = e.handle_message(
        Some(client),
        &session_header(),
        &[heartbeat()],
        Duration::ZERO,
    );
    assert_eq!(out.len(), 1);
    assert_eq!(
        out[0].submessages[0].header.submessage_id,
        SubmessageId::AckNack
    );
    assert_eq!(out[0].header.stream_id, StreamId::NONE);
    assert_eq!(out[0].header.stream_id.0, 0x00);
    let ack = AckNackPayload::try_from_submessage(&out[0].submessages[0]).unwrap();
    assert_eq!(ack.stream_id, 0x80, "the payload names the reliable stream");

    let out = e.handle_message(
        Some(client),
        &reliable_header(0),
        &[heartbeat()],
        Duration::ZERO,
    );
    let acknacks: Vec<&Outbound> = out
        .iter()
        .filter(|o| {
            o.submessages
                .iter()
                .any(|sm| sm.header.submessage_id == SubmessageId::AckNack)
        })
        .collect();
    assert_eq!(acknacks.len(), 1);
    assert_eq!(acknacks[0].header.stream_id, StreamId::NONE);
}

/// §8.3.5.16, S. 76: "The TIMESTAMP_REPLY submessage does not belong to any
/// stream, for this reason the MessageHeader shall have the streamId set to
/// STREAM_ID_NONE".
///
/// The agent answers a TIMESTAMP with a TIMESTAMP_REPLY whose MessageHeader
/// has `StreamId::NONE`, whether the TIMESTAMP came in a message of
/// STREAMID_NONE or of a reliable stream.
#[test]
fn the_timestamp_reply_of_the_agent_is_on_streamid_none() {
    let mut e = SessionEngine::new(NullDdsPort::new(), config());
    let client = [0x68; CLIENT_KEY_LEN];
    connect(&mut e, client);
    let request = TimePoint {
        seconds: 12345,
        nanoseconds: 6789,
    };
    let timestamp = || {
        TimestampPayload {
            transmit_timestamp: request,
        }
        .into_submessage()
        .unwrap()
    };

    for header in [session_header(), reliable_header(0)] {
        let out = e.handle_message(Some(client), &header, &[timestamp()], Duration::ZERO);
        let replies: Vec<&Outbound> = out
            .iter()
            .filter(|o| {
                o.submessages
                    .iter()
                    .any(|sm| sm.header.submessage_id == SubmessageId::TimestampReply)
            })
            .collect();
        assert_eq!(replies.len(), 1, "stream {:#04x}", header.stream_id.0);
        assert_eq!(replies[0].header.stream_id, StreamId::NONE);
        let reply = replies[0]
            .submessages
            .iter()
            .find(|sm| sm.header.submessage_id == SubmessageId::TimestampReply)
            .unwrap();
        let reply = TimestampReplyPayload::try_from_submessage(reply).unwrap();
        assert_eq!(reply.originate_timestamp, request);
    }
}
