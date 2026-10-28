//! DDS-XRCE 1.0 §8.4 logical actions, driven over the actual messages.
//!
//! Every test in this file exercises a spec sequence end to end on the
//! wire: the client message is encoded to bytes, decoded again, applied
//! to an [`ObjectStore`], and the agent reply is encoded and decoded in
//! turn. Nothing here asserts that a type merely compiles.
//!
//! Covered clauses:
//!
//! * §7.8.4 — the DataWriter proxy is operated by `WRITE_DATA` only.
//! * §8.4.2 — sending data using a pre-configured DataWriter.
//! * §8.4.3 — receiving data using a pre-configured DataReader.
//! * §8.4.7 — defining QoS via `CREATE(OBJK_QOSPROFILE)`.
//! * §8.4.8 — defining types via `CREATE(OBJK_TYPE)`.
//! * §8.4.10 — `CREATE(OBJK_DATAWRITER)` -> `STATUS` -> `WRITE_DATA`*N.
//! * §8.4.11 — `CREATE(OBJK_DATAREADER)` -> `STATUS` -> `READ_DATA` ->
//!   `DATA`*N.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::uninlined_format_args,
    missing_docs
)]

use zerodds_xrce::encoding::Endianness;
use zerodds_xrce::header::{ClientKey, MessageHeader, SessionId, StreamId};
use zerodds_xrce::object_id::ObjectId;
use zerodds_xrce::object_info::{BaseObjectReply, BaseObjectRequest, ResultStatus};
use zerodds_xrce::object_kind::ObjectKind;
use zerodds_xrce::object_repr::ObjectVariant;
use zerodds_xrce::object_store::{CreationMode, ObjectStore};
use zerodds_xrce::serial_number::SerialNumber16;
use zerodds_xrce::submessages::read_data::{DataDeliveryControl, ReadSpecification};
use zerodds_xrce::submessages::{
    CreatePayload, DataFormat, DataPayload, Message, ReadDataPayload, StatusPayload, Submessage,
    SubmessageId, WriteDataPayload,
};

const CLIENT_KEY: ClientKey = ClientKey([0xCA, 0xFE, 0xBA, 0xBE]);

/// Wraps submessages into a message with a reliable builtin stream.
fn message(seq: u16, submessages: Vec<Submessage>) -> Message {
    let header = MessageHeader::with_client_key(
        SessionId(0),
        StreamId::BUILTIN_RELIABLE,
        SerialNumber16::new(seq),
        CLIENT_KEY,
    )
    .expect("header");
    Message::new(header, submessages).expect("message")
}

/// Sends a message the way a transport would: encode, hand over the
/// bytes, decode on the far side.
fn over_the_wire(msg: &Message) -> Message {
    let bytes = msg.encode().expect("encode");
    Message::decode(&bytes).expect("decode")
}

/// Builds the `CREATE` body: `BaseObjectRequest` + `ObjectVariant`.
fn create_submessage(base: BaseObjectRequest, variant: &ObjectVariant) -> Submessage {
    let mut representation = Vec::from(base.encode());
    representation.extend_from_slice(&variant.encode(Endianness::Little).expect("variant"));
    CreatePayload {
        representation,
        reuse: false,
        replace: false,
    }
    .into_submessage()
    .expect("CREATE")
}

/// Splits a received `CREATE` body back into request + variant.
fn parse_create(sm: &Submessage) -> (BaseObjectRequest, ObjectVariant) {
    let payload = CreatePayload::try_from_submessage(sm).expect("CREATE payload");
    let (base, consumed) = BaseObjectRequest::decode(&payload.representation).expect("base");
    let (variant, _) =
        ObjectVariant::decode(&payload.representation[consumed..], Endianness::Little)
            .expect("variant");
    (base, variant)
}

/// The agent side of `CREATE`: registers the object and answers with a
/// `STATUS` submessage carrying the `BaseObjectReply` (§7.8.3, §8.4.x).
fn agent_apply_create(store: &mut ObjectStore, sm: &Submessage) -> Submessage {
    let (base, variant) = parse_create(sm);
    let kind = base.object_id.kind().expect("object kind");
    store
        .create(base.object_id, kind, variant, CreationMode::STRICT)
        .expect("create");
    StatusPayload {
        representation: BaseObjectReply {
            related_request: base,
            result: ResultStatus::ok(),
        }
        .encode(),
    }
    .into_submessage()
    .expect("STATUS")
}

/// Reads the `BaseObjectReply` out of a received `STATUS`.
fn parse_status(sm: &Submessage) -> BaseObjectReply {
    let payload = StatusPayload::try_from_submessage(sm).expect("STATUS payload");
    let (reply, _) = BaseObjectReply::decode(&payload.representation).expect("reply");
    reply
}

fn object_id(raw: u16, kind: ObjectKind) -> ObjectId {
    ObjectId::new(raw, kind).expect("object id")
}

// ============================================================================
// §8.4.10 Creating a DataWriter
// ============================================================================

/// Spec §8.4.10: the client creates a DataWriter with
/// `CREATE(OBJK_DATAWRITER)`, the agent replies with `STATUS`, and only
/// then does the client push samples with `WRITE_DATA`*N. The test walks
/// the whole sequence over encoded messages and checks each step.
#[test]
fn spec_8_4_10_create_datawriter_then_status_then_write_data() {
    let mut store = ObjectStore::new();
    let writer = object_id(0x0DA, ObjectKind::DataWriter);
    let request_id = [0x00, 0x01];

    // 1. CREATE(OBJK_DATAWRITER).
    let create = message(
        1,
        vec![create_submessage(
            BaseObjectRequest {
                request_id,
                object_id: writer,
            },
            &ObjectVariant::ByXmlString("<data_writer name=\"w\" topic_ref=\"Square\"/>".into()),
        )],
    );
    let received = over_the_wire(&create);
    assert_eq!(
        received.submessages[0].header.submessage_id,
        SubmessageId::Create
    );

    // 2. Agent applies it and answers STATUS.
    let status = agent_apply_create(&mut store, &received.submessages[0]);
    let status_seen = over_the_wire(&message(1, vec![status]));
    assert_eq!(
        status_seen.submessages[0].header.submessage_id,
        SubmessageId::Status
    );
    let reply = parse_status(&status_seen.submessages[0]);
    assert_eq!(
        reply.related_request.object_id, writer,
        "STATUS echoes the object id"
    );
    assert_eq!(
        reply.related_request.request_id, request_id,
        "STATUS echoes request id"
    );
    assert!(
        reply.result.status_code.is_success(),
        "STATUS reports success"
    );
    assert_eq!(
        store.get(writer).expect("stored").kind,
        ObjectKind::DataWriter,
        "the created object is a DataWriter"
    );

    // 3. WRITE_DATA*N against exactly that object id.
    let samples: [&[u8]; 3] = [b"sample-0", b"sample-1", b"sample-2"];
    for (i, payload) in samples.iter().enumerate() {
        let write = message(
            u16::try_from(i + 2).expect("seq"),
            vec![
                WriteDataPayload {
                    base: BaseObjectRequest {
                        request_id,
                        object_id: writer,
                    },
                    serialized_data: payload.to_vec(),
                }
                .into_submessage()
                .expect("WRITE_DATA"),
            ],
        );
        let seen = over_the_wire(&write);
        let decoded =
            WriteDataPayload::try_from_submessage(&seen.submessages[0]).expect("WRITE_DATA");
        assert_eq!(decoded.base.object_id, writer);
        assert_eq!(decoded.serialized_data, *payload);
        assert!(
            store.get(decoded.base.object_id).is_some(),
            "the agent resolves the DataWriter created in step 1"
        );
    }
}

// ============================================================================
// §8.4.11 Creating a DataReader
// ============================================================================

/// Spec §8.4.11: `CREATE(OBJK_DATAREADER)` -> `STATUS` -> `READ_DATA`
/// -> `DATA`*N. The `DATA` submessages must carry the `request_id` of
/// the `READ_DATA` that asked for them, so that the client can relate
/// the stream to its request (§7.7.8).
#[test]
fn spec_8_4_11_create_datareader_then_status_then_read_data_then_data_stream() {
    let mut store = ObjectStore::new();
    let reader = object_id(0x0DB, ObjectKind::DataReader);
    let request_id = [0x00, 0x2A];

    let create = over_the_wire(&message(
        1,
        vec![create_submessage(
            BaseObjectRequest {
                request_id,
                object_id: reader,
            },
            &ObjectVariant::ByXmlString("<data_reader name=\"r\" topic_ref=\"Square\"/>".into()),
        )],
    ));
    let status = over_the_wire(&message(
        1,
        vec![agent_apply_create(&mut store, &create.submessages[0])],
    ));
    let reply = parse_status(&status.submessages[0]);
    assert_eq!(reply.related_request.object_id, reader);
    assert!(reply.result.status_code.is_success());
    assert_eq!(
        store.get(reader).expect("stored").kind,
        ObjectKind::DataReader
    );

    // READ_DATA asking for at most three samples.
    let read = over_the_wire(&message(
        2,
        vec![
            ReadDataPayload {
                base: BaseObjectRequest {
                    request_id,
                    object_id: reader,
                },
                read_specification: ReadSpecification {
                    preferred_stream_id: StreamId::BUILTIN_RELIABLE.0,
                    data_format: DataFormat::Data,
                    content_filter_expression: None,
                    delivery_control: Some(DataDeliveryControl {
                        max_samples: 3,
                        max_elapsed_time: 0,
                        max_bytes_per_second: 0,
                        min_pace_period: 0,
                    }),
                },
            }
            .into_submessage()
            .expect("READ_DATA"),
        ],
    ));
    let read_back = ReadDataPayload::try_from_submessage(&read.submessages[0]).expect("READ_DATA");
    assert_eq!(read_back.base.object_id, reader);
    let control = read_back
        .read_specification
        .delivery_control
        .expect("delivery control");
    assert_eq!(control.max_samples, 3);

    // The agent answers with exactly `max_samples` DATA submessages.
    let payloads: [&[u8]; 3] = [b"s0", b"s1", b"s2"];
    let data_stream: Vec<Submessage> = payloads
        .iter()
        .map(|p| {
            DataPayload {
                base: read_back.base,
                serialized_data: p.to_vec(),
            }
            .into_submessage()
            .expect("DATA")
        })
        .collect();
    assert_eq!(data_stream.len(), usize::from(control.max_samples));
    let seen = over_the_wire(&message(3, data_stream));
    assert_eq!(seen.submessages.len(), 3);
    for (sm, expected) in seen.submessages.iter().zip(payloads.iter()) {
        let data = DataPayload::try_from_submessage(sm).expect("DATA");
        assert_eq!(data.base.object_id, reader, "DATA names the DataReader");
        assert_eq!(
            data.base.request_id, request_id,
            "DATA echoes the READ_DATA request id"
        );
        assert_eq!(data.serialized_data, *expected);
    }
}

// ============================================================================
// §7.8.4 + §8.4.2 Sending data with a pre-configured DataWriter
// ============================================================================

/// Spec §8.4.2: with a pre-configured DataWriter no `CREATE` is
/// exchanged — the client sends `WRITE_DATA` straight away, and the
/// agent resolves the object id in its store. An id that names no
/// object, or one whose kind is not DataWriter, must not be accepted as
/// a write target.
#[test]
fn spec_8_4_2_write_data_targets_a_preconfigured_datawriter() {
    let mut store = ObjectStore::new();
    let writer = object_id(0x101, ObjectKind::DataWriter);
    let reader = object_id(0x102, ObjectKind::DataReader);
    // Pre-configuration (§9.3 file config), not an exchanged CREATE.
    store
        .create(
            writer,
            ObjectKind::DataWriter,
            ObjectVariant::ByReference("Square_writer".into()),
            CreationMode::STRICT,
        )
        .expect("preconfigure writer");
    store
        .create(
            reader,
            ObjectKind::DataReader,
            ObjectVariant::ByReference("Square_reader".into()),
            CreationMode::STRICT,
        )
        .expect("preconfigure reader");

    let write = over_the_wire(&message(
        1,
        vec![
            WriteDataPayload {
                base: BaseObjectRequest {
                    request_id: [0, 3],
                    object_id: writer,
                },
                serialized_data: b"payload".to_vec(),
            }
            .into_submessage()
            .expect("WRITE_DATA"),
        ],
    ));
    let decoded = WriteDataPayload::try_from_submessage(&write.submessages[0]).expect("WRITE_DATA");
    assert_eq!(
        store.get(decoded.base.object_id).expect("resolved").kind,
        ObjectKind::DataWriter,
        "WRITE_DATA resolves to a pre-configured DataWriter"
    );
    assert_eq!(decoded.serialized_data, b"payload");

    // An unknown object id resolves to nothing.
    let unknown = object_id(0x1FF, ObjectKind::DataWriter);
    assert!(store.get(unknown).is_none());
    // A DataReader id is not a write target.
    assert_ne!(
        store.get(reader).expect("reader").kind,
        ObjectKind::DataWriter
    );
}

/// Spec §7.8.4: the DataWriter proxy has exactly one operation, and it
/// arrives as `WRITE_DATA` addressed to the DataWriter's `ObjectId`.
/// `WriteDataPayload` therefore refuses every other submessage id.
#[test]
fn spec_7_8_4_write_data_is_the_only_datawriter_operation() {
    let writer = object_id(0x0DA, ObjectKind::DataWriter);
    let sm = WriteDataPayload {
        base: BaseObjectRequest {
            request_id: [0, 1],
            object_id: writer,
        },
        serialized_data: b"x".to_vec(),
    }
    .into_submessage()
    .expect("WRITE_DATA");
    assert_eq!(sm.header.submessage_id, SubmessageId::WriteData);

    let seen = over_the_wire(&message(1, vec![sm]));
    let decoded = WriteDataPayload::try_from_submessage(&seen.submessages[0]).expect("WRITE_DATA");
    assert_eq!(
        decoded.base.object_id.kind().expect("kind"),
        ObjectKind::DataWriter
    );

    // Any other submessage is not a DataWriter operation.
    for id in [
        SubmessageId::ReadData,
        SubmessageId::Data,
        SubmessageId::GetInfo,
        SubmessageId::Delete,
    ] {
        let other = Submessage::new(id, 0x01, Vec::new()).expect("submessage");
        assert!(
            WriteDataPayload::try_from_submessage(&other).is_err(),
            "{:?} must not decode as a DataWriter operation",
            id
        );
    }
}

// ============================================================================
// §8.4.3 Receiving data with a pre-configured DataReader
// ============================================================================

/// Spec §8.4.3: with a pre-configured DataReader the client sends
/// `READ_DATA` and receives `DATA`*N; the samples arrive in the order
/// the agent produced them and carry the request id of the read.
#[test]
fn spec_8_4_3_read_data_from_a_preconfigured_datareader_yields_data_stream() {
    let mut store = ObjectStore::new();
    let reader = object_id(0x201, ObjectKind::DataReader);
    store
        .create(
            reader,
            ObjectKind::DataReader,
            ObjectVariant::ByReference("Square_reader".into()),
            CreationMode::STRICT,
        )
        .expect("preconfigure reader");

    let request_id = [0x07, 0x0E];
    let read = over_the_wire(&message(
        1,
        vec![
            ReadDataPayload {
                base: BaseObjectRequest {
                    request_id,
                    object_id: reader,
                },
                read_specification: ReadSpecification {
                    preferred_stream_id: StreamId::BUILTIN_BEST_EFFORT.0,
                    data_format: DataFormat::Data,
                    content_filter_expression: None,
                    delivery_control: None,
                },
            }
            .into_submessage()
            .expect("READ_DATA"),
        ],
    ));
    let read_back = ReadDataPayload::try_from_submessage(&read.submessages[0]).expect("READ_DATA");
    assert!(
        store.get(read_back.base.object_id).is_some(),
        "READ_DATA resolves to the pre-configured DataReader"
    );

    let payloads: [&[u8]; 2] = [b"first", b"second"];
    let stream = over_the_wire(&message(
        2,
        payloads
            .iter()
            .map(|p| {
                DataPayload {
                    base: read_back.base,
                    serialized_data: p.to_vec(),
                }
                .into_submessage()
                .expect("DATA")
            })
            .collect(),
    ));
    let decoded: Vec<DataPayload> = stream
        .submessages
        .iter()
        .map(|sm| DataPayload::try_from_submessage(sm).expect("DATA"))
        .collect();
    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded[0].serialized_data, b"first");
    assert_eq!(decoded[1].serialized_data, b"second");
    for d in &decoded {
        assert_eq!(d.base.request_id, request_id);
        assert_eq!(d.base.object_id, reader);
    }
}

// ============================================================================
// §8.4.7 Defining QoS / §8.4.8 Defining Types
// ============================================================================

/// Spec §8.4.7: a QoS profile is defined with `CREATE` on an object id
/// of kind `OBJK_QOSPROFILE`; the agent stores it under that kind and
/// answers `STATUS`. A DataWriter created afterwards may name the
/// profile by reference.
#[test]
fn spec_8_4_7_create_qosprofile_object_then_status() {
    let mut store = ObjectStore::new();
    let profile = object_id(0x00B, ObjectKind::QosProfile);
    let profile_xml = "<qos_profile name=\"reliable\"><reliability><kind>RELIABLE_RELIABILITY_QOS</kind>\
         </reliability></qos_profile>";

    let create = over_the_wire(&message(
        1,
        vec![create_submessage(
            BaseObjectRequest {
                request_id: [0, 1],
                object_id: profile,
            },
            &ObjectVariant::ByXmlString(profile_xml.into()),
        )],
    ));
    let (base, variant) = parse_create(&create.submessages[0]);
    assert_eq!(
        base.object_id.kind().expect("kind"),
        ObjectKind::QosProfile,
        "the CREATE addresses an OBJK_QOSPROFILE object id"
    );
    assert_eq!(variant, ObjectVariant::ByXmlString(profile_xml.into()));

    let status = over_the_wire(&message(
        1,
        vec![agent_apply_create(&mut store, &create.submessages[0])],
    ));
    assert!(
        parse_status(&status.submessages[0])
            .result
            .status_code
            .is_success()
    );
    let stored = store.get(profile).expect("stored profile");
    assert_eq!(stored.kind, ObjectKind::QosProfile);
    assert_eq!(
        stored.variant,
        ObjectVariant::ByXmlString(profile_xml.into())
    );

    // A DataWriter may then reference the profile by name.
    let writer = object_id(0x00C, ObjectKind::DataWriter);
    let writer_create = over_the_wire(&message(
        2,
        vec![create_submessage(
            BaseObjectRequest {
                request_id: [0, 2],
                object_id: writer,
            },
            &ObjectVariant::ByReference("reliable".into()),
        )],
    ));
    agent_apply_create(&mut store, &writer_create.submessages[0]);
    assert_eq!(
        store.get(writer).expect("writer").variant,
        ObjectVariant::ByReference("reliable".into())
    );
}

/// Spec §8.4.8: a data type is defined with `CREATE` on an object id of
/// kind `OBJK_TYPE`; the agent stores it under that kind and answers
/// `STATUS`. A Topic created afterwards may name the type by reference.
#[test]
fn spec_8_4_8_create_type_object_then_status() {
    let mut store = ObjectStore::new();
    let type_obj = object_id(0x00A, ObjectKind::Type);
    let type_xml = "<struct name=\"ShapeType\"><member name=\"color\" type=\"string\"/></struct>";

    let create = over_the_wire(&message(
        1,
        vec![create_submessage(
            BaseObjectRequest {
                request_id: [0, 1],
                object_id: type_obj,
            },
            &ObjectVariant::ByXmlString(type_xml.into()),
        )],
    ));
    let (base, variant) = parse_create(&create.submessages[0]);
    assert_eq!(
        base.object_id.kind().expect("kind"),
        ObjectKind::Type,
        "the CREATE addresses an OBJK_TYPE object id"
    );
    assert_eq!(variant, ObjectVariant::ByXmlString(type_xml.into()));

    let status = over_the_wire(&message(
        1,
        vec![agent_apply_create(&mut store, &create.submessages[0])],
    ));
    assert!(
        parse_status(&status.submessages[0])
            .result
            .status_code
            .is_success()
    );
    assert_eq!(
        store.get(type_obj).expect("stored type").kind,
        ObjectKind::Type
    );

    // A Topic may then reference the type by name.
    let topic = object_id(0x002, ObjectKind::Topic);
    let topic_create = over_the_wire(&message(
        2,
        vec![create_submessage(
            BaseObjectRequest {
                request_id: [0, 2],
                object_id: topic,
            },
            &ObjectVariant::ByReference("ShapeType".into()),
        )],
    ));
    agent_apply_create(&mut store, &topic_create.submessages[0]);
    assert_eq!(store.get(topic).expect("topic").kind, ObjectKind::Topic);
    assert_eq!(
        store.get(topic).expect("topic").variant,
        ObjectVariant::ByReference("ShapeType".into())
    );
}
