// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-durability-service` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-dcps` (`src/lib.rs`):
//! - `runtime::RuntimeConfig::from_standard_paths` (`start`) and
//!   `DomainParticipantFactory::create_participant_with_config` for the two
//!   participants (ingest and replay), `participant_handle` and
//!   `ignore_participant` so that they ignore each other, `runtime()`.
//! - The typed path of `serve`: `create_topic::<RawBytes>`,
//!   `create_publisher`/`create_subscriber`, `create_datawriter` with the
//!   replay QoS (RELIABLE, TRANSIENT_LOCAL, KEEP_ALL or KEEP_LAST(n),
//!   `max_samples` LENGTH_UNLIMITED), `create_datareader` with the ingest
//!   QoS (RELIABLE, TRANSIENT_LOCAL, KEEP_ALL, `max_samples`
//!   LENGTH_UNLIMITED), `DataWriter::write`, `instance_handle`,
//!   `DataReader::wait_for_data`, `take_with_info` (`info.valid_data`,
//!   `info.instance_state`, `info.publication_handle`, `data.data`),
//!   `RawBytes::TYPE_NAME` persisted as the topic's type name.
//! - Auto-discovery: `get_builtin_subscriber().publication_reader()`,
//!   `wait_for_data`, `take_with_info`, and of
//!   `builtin_topics::PublicationBuiltinTopicData` the members `topic_name`,
//!   `type_name`, `key` (its entity kind says keyed or not),
//!   `durability.kind` (TRANSIENT or PERSISTENT is served) and
//!   `durability_service` (the retention contract; the DDS default means
//!   "not set" and takes the daemon's own).
//! - The runtime path of `serve_typed`: `UserReaderConfig` over `Default`,
//!   `UserWriterConfig` written out, `register_user_writer_kind`,
//!   `register_user_reader_kind`, `set_user_writer_history_depth`,
//!   `set_user_writer_data_rep_override`, `set_user_writer_byte_order_override`,
//!   `write_user_sample_on_behalf`, `write_user_sample_keyed`,
//!   `write_user_lifecycle`, and `UserSample` (`Alive` with key hash,
//!   payload, representation, byte order, writer GUID, source sequence number
//!   and timestamp; `Lifecycle` with key hash, kind, key holder, timestamp;
//!   `CoherentSetEnd`).
//! - `DdsError` (wrapped in `ServiceError::Dcps`), `Result`, `InstanceHandle`.
//!
//! What it relies on and is checked here:
//! - A discovered writer's DURABILITY and DURABILITY_SERVICE reach
//!   `PublicationBuiltinTopicData` as announced, and an absent
//!   `PID_DURABILITY_SERVICE` is the DDS default policy. Oracles: DDSI-RTPS
//!   2.5 §9.6.3.2 Tables 9.18 and 9.19 (`PID_ENDPOINT_GUID` 0x005a,
//!   `PID_TOPIC_NAME` 0x0005, `PID_TYPE_NAME` 0x0007, `PID_DURABILITY`
//!   0x001d, `PID_DURABILITY_SERVICE` 0x001e; an absent parameter takes the
//!   DDS default), §9.3.1.2 Table 9.1 (entity kinds 0x02 and 0x03), §9.3.2
//!   (`Duration_t`), §10.5 (PL_CDR_LE); DDS 1.4 §2.2.3, the QoS table
//!   (DURABILITY_SERVICE defaults: cleanup delay zero, KEEP_LAST, depth 1,
//!   all limits LENGTH_UNLIMITED) and §2.3.3 (DURABILITY kinds 0 to 3).
//!   Parameter lists are written by hand.
//! - `RawBytes` is `zerodds::RawBytes`, keyless, and carries its octets
//!   unchanged (its rustdoc: "placed on the wire as-is, no CDR framing").
//! - The service's QoS is consistent and its endpoints are created; a
//!   KEEP_LAST depth above `max_samples_per_instance` would not be. Oracle:
//!   DDS 1.4 §2.2.3.18 and §2.2.3.19 ("depth <= max_samples_per_instance"),
//!   §2.2.2.4.1.5 and §2.2.2.5.2.5 (no entity with an inconsistent QoS).
//! - An ignored participant stays ignored (DDS 1.4 §2.2.2.2.1.14), and the
//!   handle the service passes is `InstanceHandle::from_guid` of the
//!   participant GUID, `HANDLE_NIL` without a runtime (rustdoc of
//!   `participant_handle`).
//! - The built-in publication reader returns after its timeout when nothing
//!   arrived, so the discovery thread sees its stop flag (rustdoc of
//!   `DataReader::wait_for_data`: `Timeout` on no data).
//!
//! The live participants and the runtime path need a bound runtime; the
//! calls of `serve_typed` are checked at compile time. No socket is opened.
#![allow(clippy::unwrap_used)]

use std::sync::mpsc::Receiver;

use zerodds_dcps::builtin_topics::PublicationBuiltinTopicData;
use zerodds_dcps::runtime::{DcpsRuntime, UserReaderConfig, UserSample, UserWriterConfig};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DdsType, DomainParticipantFactory,
    DomainParticipantQos, HANDLE_NIL, InstanceHandle, PublisherQos, RawBytes, Sample,
    SubscriberQos, TopicQos,
};
use zerodds_qos::Duration;
use zerodds_qos::policies::durability::DurabilityKind;
use zerodds_qos::policies::durability_service::DurabilityServiceQosPolicy;
use zerodds_qos::policies::history::HistoryKind;
use zerodds_qos::policies::reliability::ReliabilityKind;
use zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED;
use zerodds_rtps::header_extension::HeTimestamp;
use zerodds_rtps::history_cache::ChangeKind;
use zerodds_rtps::publication_data::PublicationBuiltinTopicData as WirePublication;
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, SequenceNumber, TopicKind};

// ---------------------------------------------------------------------
// A writer's announcement, written by hand
// ---------------------------------------------------------------------

/// A PL_CDR_LE payload: `{0x00, 0x03}`, options, each parameter padded to
/// four octets, `PID_SENTINEL`.
fn pl_cdr_le(params: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut out = vec![0x00, 0x03, 0x00, 0x00];
    for (pid, value) in params {
        let mut value = value.clone();
        while value.len() % 4 != 0 {
            value.push(0);
        }
        out.extend_from_slice(&pid.to_le_bytes());
        out.extend_from_slice(&u16::try_from(value.len()).unwrap().to_le_bytes());
        out.extend_from_slice(&value);
    }
    out.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
    out
}

fn cdr_string(s: &str) -> Vec<u8> {
    let mut out = u32::try_from(s.len() + 1).unwrap().to_le_bytes().to_vec();
    out.extend_from_slice(s.as_bytes());
    out.push(0);
    out
}

/// A user writer's GUID; `kind` is the entity kind octet (Table 9.1).
fn writer_guid(kind: u8) -> [u8; 16] {
    [
        0x01, 0xF0, 7, 7, 7, 7, 1, 2, 3, 4, 5, 6, 0x00, 0x00, 0x01, kind,
    ]
}

fn publication(guid: [u8; 16], durability: u32, service: Option<Vec<u8>>) -> WirePublication {
    let mut params: Vec<(u16, Vec<u8>)> = vec![
        (0x005a, guid.to_vec()),
        (0x0005, cdr_string("Track")),
        (0x0007, cdr_string("cuas::Track")),
        (0x001d, durability.to_le_bytes().to_vec()),
    ];
    if let Some(service) = service {
        params.push((0x001e, service));
    }
    WirePublication::from_pl_cdr_le(&pl_cdr_le(&params)).unwrap()
}

#[test]
fn rtps_9_6_3_2_a_writers_durability_service_reaches_the_builtin_data() {
    // DurabilityServiceQosPolicy: service_cleanup_delay {5 s, 0},
    // KEEP_LAST_HISTORY_QOS (0), depth 5, max_samples 100,
    // max_instances 10, max_samples_per_instance 10.
    let mut service = Vec::new();
    for word in [5_u32, 0, 0, 5, 100, 10, 10] {
        service.extend_from_slice(&word.to_le_bytes());
    }
    let guid = writer_guid(0x02);
    // TRANSIENT_DURABILITY_QOS = 2.
    let data = PublicationBuiltinTopicData::from_wire(&publication(guid, 2, Some(service)));
    assert_eq!(data.topic_name, "Track");
    assert_eq!(data.type_name, "cuas::Track");
    assert_eq!(data.key, Guid::from_bytes(guid));
    assert_eq!(
        data.key.entity_id.entity_kind.topic_kind(),
        Some(TopicKind::WithKey)
    );
    assert_eq!(data.durability.kind, DurabilityKind::Transient);
    assert!(data.durability.kind >= DurabilityKind::Transient);
    assert_eq!(
        data.durability_service,
        DurabilityServiceQosPolicy {
            service_cleanup_delay: Duration::from_secs(5),
            history_kind: HistoryKind::KeepLast,
            history_depth: 5,
            max_samples: 100,
            max_instances: 10,
            max_samples_per_instance: 10,
        }
    );
    assert_ne!(
        data.durability_service,
        DurabilityServiceQosPolicy::default()
    );
}

#[test]
fn dcps_2_2_3_an_absent_durability_service_is_the_default_policy() {
    let guid = writer_guid(0x03);
    // PERSISTENT_DURABILITY_QOS = 3, no PID_DURABILITY_SERVICE.
    let data = PublicationBuiltinTopicData::from_wire(&publication(guid, 3, None));
    assert_eq!(data.durability.kind, DurabilityKind::Persistent);
    assert_eq!(
        data.key.entity_id.entity_kind.topic_kind(),
        Some(TopicKind::NoKey)
    );
    let spec_default = DurabilityServiceQosPolicy {
        service_cleanup_delay: Duration::ZERO,
        history_kind: HistoryKind::KeepLast,
        history_depth: 1,
        max_samples: LENGTH_UNLIMITED,
        max_instances: LENGTH_UNLIMITED,
        max_samples_per_instance: LENGTH_UNLIMITED,
    };
    assert_eq!(data.durability_service, spec_default);
    // What the service compares against to tell "not set" apart.
    assert_eq!(
        data.durability_service,
        DurabilityServiceQosPolicy::default()
    );
}

#[test]
fn the_raw_bytes_type_the_service_persists_is_keyless_and_passes_octets_unchanged() {
    assert_eq!(<RawBytes as DdsType>::TYPE_NAME, "zerodds::RawBytes");
    assert_eq!(<RawBytes as DdsType>::type_name(), "zerodds::RawBytes");
    assert!(!<RawBytes as DdsType>::has_key());
    let octets = vec![0x00, 0x01, 0xFE, 0xFF, 0x80];
    let mut encoded = Vec::new();
    RawBytes::new(octets.clone()).encode(&mut encoded).unwrap();
    assert_eq!(encoded, octets);
    assert_eq!(RawBytes::decode(&octets).unwrap().data, octets);
}

fn replay_writer_qos(history: HistoryKind, depth: i32) -> DataWriterQos {
    let mut q = DataWriterQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.durability.kind = DurabilityKind::TransientLocal;
    q.history.kind = history;
    q.history.depth = depth;
    q.resource_limits.max_samples = LENGTH_UNLIMITED;
    q
}

fn ingest_reader_qos() -> DataReaderQos {
    let mut q = DataReaderQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.durability.kind = DurabilityKind::TransientLocal;
    q.history.kind = HistoryKind::KeepAll;
    q.resource_limits.max_samples = LENGTH_UNLIMITED;
    q
}

#[test]
fn dcps_2_2_3_19_the_service_qos_is_consistent_and_creates_its_endpoints() {
    let factory = DomainParticipantFactory::instance();
    let ingest = factory.create_participant_offline(0, DomainParticipantQos::default());
    let replay = factory.create_participant_offline(0, DomainParticipantQos::default());
    let rtopic = ingest
        .create_topic::<RawBytes>("contract-dcps-durability", TopicQos::default())
        .unwrap();
    let wtopic = replay
        .create_topic::<RawBytes>("contract-dcps-durability", TopicQos::default())
        .unwrap();
    let publisher = replay.create_publisher(PublisherQos::default());
    let subscriber = ingest.create_subscriber(SubscriberQos::default());

    for qos in [
        replay_writer_qos(HistoryKind::KeepAll, 0),
        replay_writer_qos(HistoryKind::KeepLast, 3),
    ] {
        assert!(qos.check_consistency().is_ok());
        let writer = publisher
            .create_datawriter::<RawBytes>(&wtopic, qos)
            .unwrap();
        // Startup-sync writes the stored samples before any reader exists.
        writer.write(&RawBytes::new(vec![1, 2, 3])).unwrap();
    }
    let reader = subscriber
        .create_datareader::<RawBytes>(&rtopic, ingest_reader_qos())
        .unwrap();
    assert!(reader.take_with_info().unwrap().is_empty());

    // KEEP_LAST deeper than max_samples_per_instance is inconsistent.
    let mut inconsistent = replay_writer_qos(HistoryKind::KeepLast, 5);
    inconsistent.resource_limits.max_samples_per_instance = 2;
    assert!(matches!(
        inconsistent.check_consistency(),
        Err(DdsError::InconsistentPolicy { .. })
    ));
    assert!(matches!(
        publisher.create_datawriter::<RawBytes>(&wtopic, inconsistent),
        Err(DdsError::InconsistentPolicy { .. })
    ));
}

#[test]
fn dcps_2_2_2_2_1_14_an_ignored_participant_stays_ignored() {
    let factory = DomainParticipantFactory::instance();
    let ingest = factory.create_participant_offline(0, DomainParticipantQos::default());
    let replay = factory.create_participant_offline(0, DomainParticipantQos::default());
    // Without a runtime there is no participant GUID.
    assert_eq!(replay.participant_handle(), HANDLE_NIL);
    assert!(replay.runtime().is_none());
    // The handle of a live participant: its GUID with ENTITYID_PARTICIPANT.
    let prefix = GuidPrefix::from_bytes([0x01, 0xF0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    let handle = InstanceHandle::from_guid(Guid::new(prefix, EntityId::PARTICIPANT));
    let other = InstanceHandle::from_guid(Guid::new(
        GuidPrefix::from_bytes([0x01, 0xF0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11]),
        EntityId::PARTICIPANT,
    ));
    assert_ne!(handle, other);
    ingest.ignore_participant(handle).unwrap();
    assert!(ingest.is_participant_ignored(handle));
    assert!(!ingest.is_participant_ignored(other));
    ingest.ignore_participant(handle).unwrap();
    assert!(ingest.is_participant_ignored(handle));
}

#[test]
fn the_builtin_publication_reader_returns_when_nothing_arrived() {
    let participant = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let reader = participant.get_builtin_subscriber().publication_reader();
    assert!(matches!(
        reader.wait_for_data(core::time::Duration::from_millis(20)),
        Err(DdsError::Timeout)
    ));
    let samples: Vec<Sample<PublicationBuiltinTopicData>> = reader.take_with_info().unwrap();
    assert!(samples.is_empty());
}

/// What the ingest pump takes out of a runtime sample.
type Ingested = (
    [u8; 16],
    Vec<u8>,
    u8,
    bool,
    [u8; 16],
    i64,
    Option<HeTimestamp>,
);

fn ingested(sample: UserSample) -> Option<Ingested> {
    match sample {
        UserSample::Alive {
            key_hash,
            payload,
            representation,
            big_endian,
            writer_guid,
            source_sequence_number,
            source_timestamp,
            ..
        } => Some((
            key_hash.unwrap_or([0u8; 16]),
            payload.to_vec(),
            representation,
            big_endian,
            writer_guid,
            source_sequence_number,
            source_timestamp,
        )),
        UserSample::Lifecycle {
            key_hash,
            kind,
            key_holder,
            source_timestamp,
            ..
        } => {
            let _: ([u8; 16], ChangeKind, Option<Vec<u8>>, Option<HeTimestamp>) =
                (key_hash, kind, key_holder, source_timestamp);
            None
        }
        UserSample::CoherentSetEnd { .. } => None,
    }
}

#[test]
fn the_runtime_path_keeps_the_shape_serve_typed_uses() {
    let _: fn(UserSample) -> Option<Ingested> = ingested;
    let _ = |rt: &DcpsRuntime,
             writer: UserWriterConfig,
             reader: UserReaderConfig,
             eid: EntityId,
             ts: Option<HeTimestamp>|
     -> zerodds_dcps::Result<Receiver<UserSample>> {
        let _: EntityId = rt.register_user_writer_kind(writer, true)?;
        let (_, rx) = rt.register_user_reader_kind(reader, true)?;
        rt.set_user_writer_history_depth(eid, usize::MAX)?;
        rt.set_user_writer_data_rep_override(eid, Some(vec![2]))?;
        rt.set_user_writer_byte_order_override(eid, false)?;
        let _: i64 = rt.write_user_sample_on_behalf(
            eid,
            &[1, 2, 3],
            Some([0; 16]),
            ts,
            Guid::from_bytes([1; 16]),
            SequenceNumber(7),
        )?;
        rt.write_user_sample_keyed(eid, &[1, 2, 3], [0; 16], ts)?;
        rt.write_user_lifecycle(eid, [0; 16], None, 0, ts)?;
        Ok(rx)
    };
}
