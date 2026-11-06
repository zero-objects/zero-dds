// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-rpc` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-dcps`:
//! - `src/requester.rs`, `src/replier.rs`: `DomainParticipant::{
//!   create_publisher, create_subscriber}`, `Publisher::create_datawriter`
//!   and `Subscriber::create_datareader` of `dds_type::RawBytes`;
//!   `DataWriter::{write, write_related, rtps_guid, next_sequence_number,
//!   set_rpc_discovery, matched_rpc_readers, matched_subscription_count,
//!   topic, qos, __drain_pending}`; `DataReader::{take_with_wire,
//!   set_listener, rtps_guid, set_rpc_discovery, matched_rpc_writers, topic,
//!   qos, __push_raw, __push_raw_related}`; `entity::Entity::set_qos` on
//!   both; `listener::DataReaderListener::on_data_available` with
//!   `psm_constants::status::DATA_AVAILABLE`; `sample_bytes::SampleWire`
//!   (`writer_guid`, `sequence_number`, `related_sample_identity`);
//!   `runtime::RpcDiscoveryExt`; `DdsType::{encode, decode,
//!   encode_continuing, decode_continuing}` of the user types, and
//!   `DecodeError::Invalid`.
//! - `src/endpoint_options.rs`: `DomainParticipant::{
//!   create_topic_with_type_name, create_topic_with_type_information}`,
//!   `TopicTypeInformation { object, dependencies }`, `qos::TopicQos`.
//! - `src/qos_profile.rs`: `qos::{DataWriterQos, DataReaderQos}`, built
//!   over their defaults.
//! - `src/discovery_ext.rs`: `runtime::{RpcDiscoveryExt, RpcPeer}`.
//! - `src/header_pair.rs`, `src/test_types.rs`: `dds_type::{DdsType,
//!   DecodeError, EncodeError}`.
//!
//! What it relies on: a request or reply frame goes out as the `RawBytes`
//! sample unchanged; offline an endpoint has no RTPS identity (no GUID, no
//! sequence number, no RPC peers), so the requester synthesizes one, and the
//! writer queues the frames for `__drain_pending`; `take_with_wire` hands
//! over the writer GUID, the sequence number and the related sample identity
//! of each sample; `on_data_available` is called for an arriving sample; the
//! changeable QoS policies change after `enable` and the others are refused.
//!
//! Oracles: DDS-RPC 1.0 §7.5.1.1.6/7 (the request and reply topic types
//! named `${I}_Request` / `${I}_Reply`, the sample a header followed by the
//! `Call` or `Return` union), §7.6.2.1 (`string<256>` in the discovery
//! extension), §7.8.1/§7.8.2 (request identity and reply correlation); DDS 1.4
//! §2.2.3 (default and Changeable column of the policies), §2.2.4.2.6.1
//! (`on_data_available`), §2.3.3 (`DATA_AVAILABLE_STATUS = 0x0001 << 10`);
//! DDS-XTypes 1.3 §7.4.3.2.2 Table 37 (MAXALIGN 8 under XCDR1, 4 under
//! XCDR2); the COMPLETE TypeObject CycloneDDS 11.0.1 `idlc` writes for
//! `module to { @final struct Plain { long a; short b; }; }`
//! (`crates/types/tests/typeobject_vendor_dheader.rs`); and the rustdoc of
//! `RawBytes`, `DdsType::encode_continuing`, `DataWriter::rtps_guid`,
//! `DataWriter::next_sequence_number`, `DataWriter::write_related`,
//! `DataWriter::set_rpc_discovery` and `SampleWire`. No socket is opened.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::time::Duration as StdDuration;

use zerodds_dcps::dds_type::{DdsType, DecodeError, EncodeError, RawBytes};
use zerodds_dcps::entity::Entity;
use zerodds_dcps::factory::DomainParticipantFactory;
use zerodds_dcps::instance_handle::InstanceHandle;
use zerodds_dcps::listener::DataReaderListener;
use zerodds_dcps::participant::DomainParticipant;
use zerodds_dcps::psm_constants::status;
use zerodds_dcps::qos::{
    DataReaderQos, DataWriterQos, DeadlineQosPolicy, DestinationOrderKind, DomainParticipantQos,
    DurabilityKind, DurabilityQosPolicy, HistoryKind, HistoryQosPolicy, LivelinessKind,
    OwnershipKind, PresentationAccessScope, PublisherQos, ReliabilityKind, SubscriberQos, TopicQos,
};
use zerodds_dcps::runtime::RpcDiscoveryExt;
use zerodds_dcps::subscriber::DataReader;
use zerodds_dcps::{DdsError, TopicTypeInformation};
use zerodds_qos::Duration;
use zerodds_rtps::inline_qos::SampleIdentityBytes;
use zerodds_types::TypeObject;

/// CycloneDDS's COMPLETE `TypeObject` of `to::Plain`.
const PLAIN_COMPLETE: &str = "4c000000f251010012000000000000000a000000746f3a3a506c61696e0000002c0000000200000010000000000000000100040002000000610000001000000001000000010003000200000062000000";

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn participant() -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default())
}

/// A reply reader that keeps every sample: under the default KEEP_LAST(1)
/// the second sample of the one instance replaces the first (DDS 1.4
/// §2.2.3.18).
fn keep_all_reader(p: &DomainParticipant, topic: &str) -> DataReader<RawBytes> {
    let topic = p
        .create_topic_with_type_name::<RawBytes>(topic, "Calculator_Reply", TopicQos::default())
        .unwrap();
    p.create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(
            &topic,
            DataReaderQos {
                history: HistoryQosPolicy {
                    kind: HistoryKind::KeepAll,
                    depth: 1,
                },
                ..DataReaderQos::default()
            },
        )
        .unwrap()
}

/// A payload type that gives only `encode` and `decode`, as a hand-written
/// `TIn`/`TOut` may: a `u32` and a `u64`, little-endian, the `u64` aligned to
/// four as XCDR2 does.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Wide {
    a: u32,
    b: u64,
}

impl DdsType for Wide {
    const TYPE_NAME: &'static str = "contract::Wide";

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.a.to_le_bytes());
        out.extend_from_slice(&self.b.to_le_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let a: [u8; 4] = bytes
            .get(0..4)
            .and_then(|b| b.try_into().ok())
            .ok_or(DecodeError::Invalid { what: "short" })?;
        let b: [u8; 8] = bytes
            .get(4..12)
            .and_then(|b| b.try_into().ok())
            .ok_or(DecodeError::Invalid { what: "short" })?;
        Ok(Self {
            a: u32::from_le_bytes(a),
            b: u64::from_le_bytes(b),
        })
    }
}

/// `on_data_available` as an event the test can wait for.
struct Arrivals(SyncSender<InstanceHandle>);

impl DataReaderListener for Arrivals {
    fn on_data_available(&self, reader: InstanceHandle) {
        let _ = self.0.try_send(reader);
    }
}

#[test]
fn rpc_7_5_1_1_6_a_frame_goes_out_as_the_raw_bytes_sample_unchanged() {
    // `RawBytes`: "Payload bytes (placed on the wire as-is, no CDR framing)",
    // in every representation and byte order the writer may pick.
    let frame = vec![0xaa, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
    let sample = RawBytes::new(frame.clone());
    let mut xcdr2 = Vec::new();
    sample.encode(&mut xcdr2).unwrap();
    assert_eq!(xcdr2, frame);
    let mut xcdr1 = Vec::new();
    sample.encode_xcdr1(&mut xcdr1).unwrap();
    assert_eq!(xcdr1, frame);
    let mut big_endian = Vec::new();
    sample.encode_be(&mut big_endian).unwrap();
    assert_eq!(big_endian, frame);
    assert_eq!(RawBytes::decode(&frame).unwrap(), sample);
    assert_eq!(RawBytes::decode_xcdr1(&frame).unwrap(), sample);
    assert_eq!(RawBytes::decode_be(&frame).unwrap(), sample);
    // Opaque octets have no alignment: they continue a header of any
    // length, and are read back from where the header ends.
    let mut continued = vec![0xee; 5];
    sample.encode_continuing(&mut continued, true).unwrap();
    assert_eq!(&continued[5..], frame.as_slice());
    assert_eq!(
        RawBytes::decode_continuing(&continued, 5, false).unwrap(),
        sample
    );
    assert!(RawBytes::decode_continuing(&continued, continued.len() + 1, false).is_err());
}

#[test]
fn xtypes_7_4_3_2_2_a_type_without_its_own_continuing_codec_keeps_to_maxalign() {
    // `DdsType::encode_continuing`: the default "refuses when prefix.len() is
    // not a multiple of the stream's alignment cap (8 for XCDR1, 4 for
    // XCDR2)" — Table 37, MAXALIGN.
    let wide = Wide { a: 1, b: 2 };
    let mut alone = Vec::new();
    wide.encode(&mut alone).unwrap();

    let mut behind_eight = vec![0xee; 8];
    wide.encode_continuing(&mut behind_eight, false).unwrap();
    assert_eq!(&behind_eight[8..], alone.as_slice());
    assert_eq!(
        Wide::decode_continuing(&behind_eight, 8, false).unwrap(),
        wide
    );

    let mut behind_six = vec![0xee; 6];
    assert!(matches!(
        wide.encode_continuing(&mut behind_six, false),
        Err(EncodeError::Invalid { .. })
    ));
    assert!(matches!(
        Wide::decode_continuing(&behind_eight, 6, false),
        Err(DecodeError::Invalid { .. })
    ));
    // Under XCDR1 the type declares no classic-CDR codec: refused even at an
    // aligned offset, rather than written as XCDR2.
    let mut behind_sixteen = vec![0xee; 16];
    assert!(matches!(
        wide.encode_continuing(&mut behind_sixteen, true),
        Err(EncodeError::Invalid { .. })
    ));
}

#[test]
fn rpc_7_8_1_an_offline_endpoint_has_no_rtps_identity_and_no_rpc_peers() {
    let p = participant();
    let topic = p
        .create_topic_with_type_name::<RawBytes>(
            "Calculator_Request",
            "Calculator_Request",
            TopicQos::default(),
        )
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    // `rtps_guid`: "`None` in offline mode"; `next_sequence_number`: "`None`
    // in offline mode". The requester then stands in its own GUID and
    // counter for the identity of §7.8.1.
    assert_eq!(writer.rtps_guid(), None);
    assert_eq!(writer.next_sequence_number(), None);
    assert_eq!(reader.rtps_guid(), None);
    assert_eq!(writer.matched_subscription_count(), 0);
    assert!(writer.matched_rpc_readers().is_empty());
    assert!(reader.matched_rpc_writers().is_empty());
    // "A writer without a runtime is discovered by nobody and has nothing
    // to announce": accepted, not an error.
    let ext = RpcDiscoveryExt {
        service_instance_name: Some("calc".into()),
        related_entity_guid: Some([1; 16]),
        topic_aliases: None,
    };
    writer.set_rpc_discovery(&ext).unwrap();
    reader.set_rpc_discovery(&ext).unwrap();
    // `write_related`: "`PreconditionNotMet` for a writer without a runtime,
    // which has no inline QoS to carry the identity."
    assert!(matches!(
        writer.write_related(
            &RawBytes::new(vec![1]),
            SampleIdentityBytes::new([2; 16], 3)
        ),
        Err(DdsError::PreconditionNotMet { .. })
    ));
}

#[test]
fn rpc_7_6_2_1_the_discovery_extension_is_unannounced_by_default_and_bounded_at_256() {
    // `None` is "not announced", which for the related entity reads as
    // GUID_UNKNOWN; the strings are `string<256>`.
    let ext = RpcDiscoveryExt::default();
    assert_eq!(ext.service_instance_name, None);
    assert_eq!(ext.related_entity_guid, None);
    assert_eq!(ext.topic_aliases, None);
    assert_eq!(RpcDiscoveryExt::MAX_STRING, 256);
}

#[test]
fn an_offline_writer_queues_each_frame_and_a_reader_takes_it_back() {
    // The offline path of the crate's tests and helpers: the request writer's
    // queue is drained into the replier's reader.
    let p = participant();
    let topic = p
        .create_topic_with_type_name::<RawBytes>(
            "Calc_Request",
            "Calc_Request",
            TopicQos::default(),
        )
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let first = vec![0, 1, 2, 3, 4];
    let second = vec![5, 6];
    writer.write(&RawBytes::new(first.clone())).unwrap();
    writer.write(&RawBytes::new(second.clone())).unwrap();
    let drained = writer.__drain_pending();
    assert_eq!(drained, vec![first.clone(), second.clone()]);
    assert!(writer.__drain_pending().is_empty());

    let reader = keep_all_reader(&p, "Calc_Reply");
    for frame in drained {
        reader.__push_raw(frame).unwrap();
    }
    let taken = reader.take_with_wire().unwrap();
    let frames: Vec<Vec<u8>> = taken.iter().map(|(raw, _)| raw.data.clone()).collect();
    assert_eq!(frames, vec![first, second]);
    // `SampleWire::sequence_number`: "-1 where the delivery path has none";
    // the Enhanced replier counts such a request as unidentified.
    for (_, wire) in &taken {
        assert_eq!(wire.expect("a wire").sequence_number, -1);
    }
    assert!(reader.take_with_wire().unwrap().is_empty());
}

#[test]
fn rpc_7_8_2_take_with_wire_hands_over_the_request_identity_and_the_related_identity() {
    // "the request is identified by the writer GUID and sequence number of
    // its DATA and the reply carries them in PID_RELATED_SAMPLE_IDENTITY".
    let p = participant();
    let reader = keep_all_reader(&p, "Calc_Reply_Related");
    let request = SampleIdentityBytes::new([9; 16], 42);
    reader
        .__push_raw_related(vec![1, 2, 3], [7; 16], 5, Some(request))
        .unwrap();
    reader
        .__push_raw_related(vec![4], [7; 16], 6, None)
        .unwrap();
    let taken = reader.take_with_wire().unwrap();
    assert_eq!(taken.len(), 2);
    let (raw, wire) = &taken[0];
    let wire = wire.expect("a wire");
    assert_eq!(raw.data, vec![1, 2, 3]);
    assert_eq!(wire.writer_guid, [7; 16]);
    assert_eq!(wire.sequence_number, 5);
    let related = wire.related_sample_identity.expect("a related identity");
    assert_eq!(related.writer_guid, [9; 16]);
    assert_eq!(related.sequence_number, 42);
    let wire = taken[1].1.expect("a wire");
    assert_eq!(wire.sequence_number, 6);
    assert_eq!(wire.related_sample_identity, None);
}

#[test]
fn dcps_2_2_4_2_6_1_on_data_available_is_called_until_the_listener_is_removed() {
    // DDS 1.4 §2.3.3: DATA_AVAILABLE_STATUS = 0x0001 << 10.
    assert_eq!(status::DATA_AVAILABLE, 0x0400);
    let p = participant();
    let reader = keep_all_reader(&p, "Calc_Reply_Listener");
    let (tx, rx) = sync_channel::<InstanceHandle>(16);
    reader.set_listener(Some(Arc::new(Arrivals(tx))), status::DATA_AVAILABLE);
    reader.__push_raw(vec![1]).unwrap();
    let handle = rx
        .recv_timeout(StdDuration::from_secs(5))
        .expect("on_data_available");
    assert_eq!(handle, reader.instance_handle());
    // The requester's `close` clears the slot with the same mask.
    reader.set_listener(None, status::DATA_AVAILABLE);
    reader.__push_raw(vec![2]).unwrap();
    assert!(rx.try_recv().is_err(), "a removed listener is not called");
    assert_eq!(reader.take_with_wire().unwrap().len(), 2);
}

#[test]
fn rpc_7_5_1_1_6_the_topics_carry_the_service_type_names_not_the_buffer_type() {
    let p = participant();
    let topic = p
        .create_topic_with_type_name::<RawBytes>(
            "Robot_Request",
            "Robot_Request",
            TopicQos::default(),
        )
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    assert_eq!(writer.topic().name(), "Robot_Request");
    assert_eq!(writer.topic().type_name(), "Robot_Request");
    assert_ne!(writer.topic().type_name(), RawBytes::TYPE_NAME);
    assert!(matches!(
        p.create_topic_with_type_name::<RawBytes>("Robot_Reply", "", TopicQos::default()),
        Err(DdsError::BadParameter { .. })
    ));
    // With the XTypes description of the type (§7.5.1.2): the vendor's
    // COMPLETE object, decoded as `endpoint_options.rs` decodes it.
    let object = TypeObject::from_bytes_le(&unhex(PLAIN_COMPLETE)).unwrap();
    let described = p
        .create_topic_with_type_information::<RawBytes>(
            "Plain_Request",
            "to::Plain",
            TopicTypeInformation {
                object,
                dependencies: Vec::new(),
            },
            TopicQos::default(),
        )
        .unwrap();
    assert_eq!(described.type_name(), "to::Plain");
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&described, DataReaderQos::default())
        .unwrap();
    assert_eq!(reader.topic().type_name(), "to::Plain");
}

#[test]
fn dcps_2_2_3_what_an_rpc_endpoint_leaves_to_the_defaults_is_the_dds_default() {
    // `qos_profile.rs` sets reliability, durability, history, resource
    // limits, lifespan and deadline and takes the rest from here.
    let w = DataWriterQos::default();
    assert_eq!(w.reliability.kind, ReliabilityKind::Reliable); // §2.2.3.14
    assert_eq!(w.reliability.max_blocking_time, Duration::from_millis(100));
    assert_eq!(w.durability.kind, DurabilityKind::Volatile); // §2.2.3.4
    assert_eq!(w.ownership.kind, OwnershipKind::Shared); // §2.2.3.9
    assert_eq!(w.ownership_strength.value, 0); // §2.2.3.10
    assert_eq!(w.liveliness.kind, LivelinessKind::Automatic); // §2.2.3.11
    assert_eq!(w.liveliness.lease_duration, Duration::INFINITE);
    assert_eq!(
        w.destination_order.kind,
        DestinationOrderKind::ByReceptionTimestamp
    ); // §2.2.3.17
    assert!(w.partition.names.is_empty()); // §2.2.3.13
    assert_eq!(
        w.presentation.access_scope,
        PresentationAccessScope::Instance
    ); // §2.2.3.6
    assert!(!w.presentation.coherent_access);
    assert!(!w.presentation.ordered_access);
    assert!(w.writer_data_lifecycle.autodispose_unregistered_instances); // §2.2.3.21
    assert_eq!(w.data_representation, None);
    let r = DataReaderQos::default();
    assert_eq!(r.reliability.kind, ReliabilityKind::BestEffort); // §2.2.3.14
    assert_eq!(r.history.kind, HistoryKind::KeepLast); // §2.2.3.18
    assert_eq!(r.history.depth, 1);
    assert_eq!(r.time_based_filter.minimum_separation, Duration::ZERO); // §2.2.3.12
    assert_eq!(
        r.reader_data_lifecycle.autopurge_nowriter_samples_delay,
        Duration::INFINITE
    ); // §2.2.3.22
    assert_eq!(r.data_representation, None);
}

#[test]
fn dcps_2_2_3_after_enable_changeable_policies_change_and_the_others_are_refused() {
    // `Requester::set_qos` and `Replier::set_qos`: §7.10.2 "The users may
    // modify the mutable QoS policies at run-time."
    let p = participant();
    let topic = p
        .create_topic_with_type_name::<RawBytes>("Calc_Qos", "Calc_Qos", TopicQos::default())
        .unwrap();
    let created = DataWriterQos {
        data_representation: Some(vec![0]),
        ..DataWriterQos::default()
    };
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, created.clone())
        .unwrap();
    // DEADLINE: Changeable Yes.
    let deadline = DeadlineQosPolicy {
        period: Duration::from_secs(2),
    };
    writer
        .set_qos(DataWriterQos {
            deadline,
            ..created
        })
        .unwrap();
    assert_eq!(writer.qos().deadline, deadline);
    // RELIABILITY: Changeable No; DATA_REPRESENTATION neither (XTypes 1.3
    // §7.6.3.1.2).
    let mut best_effort = writer.qos();
    best_effort.reliability.kind = ReliabilityKind::BestEffort;
    assert!(matches!(
        writer.set_qos(best_effort),
        Err(DdsError::ImmutablePolicy { .. })
    ));
    assert!(matches!(
        writer.set_qos(DataWriterQos {
            data_representation: Some(vec![2]),
            ..writer.qos()
        }),
        Err(DdsError::ImmutablePolicy { .. })
    ));
    assert_eq!(writer.qos().reliability.kind, ReliabilityKind::Reliable);

    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    reader
        .set_qos(DataReaderQos {
            deadline,
            ..DataReaderQos::default()
        })
        .unwrap();
    assert_eq!(reader.qos().deadline, deadline);
    // DURABILITY: Changeable No.
    assert!(matches!(
        reader.set_qos(DataReaderQos {
            durability: DurabilityQosPolicy {
                kind: DurabilityKind::TransientLocal,
            },
            ..reader.qos()
        }),
        Err(DdsError::ImmutablePolicy { .. })
    ));
}
