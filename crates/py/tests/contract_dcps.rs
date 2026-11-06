// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-py` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-dcps`, all behind its
//! `extension-module` feature:
//! - `src/ffi.rs`: `DomainParticipantFactory::{instance,
//!   create_participant_offline, create_participant,
//!   create_participant_with_config}` with a `runtime::RuntimeConfig` over
//!   `RuntimeConfig::from_standard_paths`; `DomainParticipant::{domain_id,
//!   create_topic, create_serialized_topic, create_publisher,
//!   create_subscriber, topics_len, discovered_participants_count,
//!   ignore_participant, ignore_topic, ignore_publication,
//!   ignore_subscription, contains_entity}`; `DataWriter::{write,
//!   register_instance, lookup_instance, dispose, unregister_instance,
//!   wait_for_matched_subscription, wait_for_acknowledgments,
//!   matched_subscription_count, set_listener}` and the status getters;
//!   `DataReader::{take, read, take_with_info, read_with_info, with_filter,
//!   wait_for_data, wait_for_matched_publication, matched_publication_count,
//!   liveliness_changed_status, set_listener}`; `sample::Sample` and
//!   `sample_info::SampleInfo` with an exhaustive match over
//!   `InstanceStateKind`, `SampleStateKind`, `ViewStateKind`; the
//!   `sample_state_mask`, `view_state_mask` and `instance_state_mask`
//!   constants it exports to Python; `serialized_type::{
//!   SerializedTypeSupport::from_type_objects, read_body, write_body,
//!   SerializedSample}`; `interop::ShapeType`; `RawBytes`; `DdsType` for its
//!   own `KeyedReading` (`HAS_KEY`, `KEY_HOLDER_MAX_SIZE = Some(4)`,
//!   `dds_type::PlainCdr2BeKeyHolder::write_u32`);
//!   `instance_handle::InstanceHandle::{from_raw, as_raw}`;
//!   `DdsError::Timeout`, mapped to Python's `TimeoutError`;
//!   `condition::{GuardCondition, WaitSet, Condition}`.
//! - `src/conditions.rs`: `condition::{ReadCondition, QueryCondition,
//!   Condition}`.
//! - `src/listener.rs`: `listener::{DataReaderListener,
//!   DataWriterListener}` and the `status` structures they receive.
//! - `src/qos.rs`: the `qos` policies and kinds, over
//!   `DataWriterQos::default()` and `DataReaderQos::default()`.
//!
//! What it relies on: the QoS builders start from the DDS defaults; the
//! state masks it exports are the PSM values; a descriptor topic reads and
//! writes the octets of its `TypeObject`; the key hash of a key holder of at
//! most 16 octets is the holder zero-padded; a WaitSet returns the triggered
//! guard and times out with `DdsError::Timeout`; offline the wait
//! operations answer at once; a listener is invoked for the statuses in its
//! mask.
//!
//! `ffi.rs` compiles only with `extension-module`, which links against the
//! Python interpreter, and its items are private; the tests call
//! `zerodds-dcps` directly with the arguments `ffi.rs` passes, and a type
//! that declares its key the way `KeyedReading` does.
//!
//! Oracles: DDS 1.4 §2.2.2.1.1.3 (the listener mask), §2.2.2.1.6.3 and
//! §2.2.2.1.8 (WaitSet, GuardCondition), §2.2.2.2.1.14 and §2.2.2.2.1.31
//! (ignore, contains_entity), §2.2.2.5.3.6 and Annex B (QueryCondition),
//! §2.2.3 (policy defaults), §2.3.3 (the PSM values of the state masks);
//! DDS-XTypes 1.3 §7.4.3 and Table 60 (PLAIN_CDR and PLAIN_CDR2 of a `@final`
//! structure, DELIMITED_CDR of an `@appendable` one), §7.6.3.1.2
//! (DataRepresentationId XCDR = 0, XCDR2 = 2), §7.6.8 (key holder, KeyHash);
//! OMG dds-rtps `shape.idl` (`ShapeType`); the COMPLETE TypeObject
//! CycloneDDS 11.0.1 `idlc` writes for `module to { @final struct Plain {
//! long a; short b; }; }` (`crates/types/tests/typeobject_vendor_dheader.rs`);
//! MD5 of key holders computed with Python's `hashlib`; and the rustdoc of
//! `DataReader::wait_for_data`, `DataWriter::wait_for_acknowledgments`,
//! `DataReader::liveliness_changed_status` and
//! `DomainParticipant::discovered_participants_count`. No socket is opened.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::time::Duration as StdDuration;

use zerodds_dcps::condition::{Condition, GuardCondition, QueryCondition, ReadCondition, WaitSet};
use zerodds_dcps::dds_type::{DecodeError, EncodeError, Extensibility, PlainCdr2BeKeyHolder};
use zerodds_dcps::instance_handle::InstanceHandle;
use zerodds_dcps::interop::ShapeType;
use zerodds_dcps::listener::DataReaderListener;
use zerodds_dcps::psm_constants::status;
use zerodds_dcps::qos::{
    DestinationOrderKind, DurabilityKind, HistoryKind, HistoryQosPolicy, LivelinessKind,
    OwnershipKind, ReliabilityKind,
};
use zerodds_dcps::sample_info::{
    InstanceStateKind, SampleInfo, SampleStateKind, ViewStateKind, instance_state_mask,
    sample_state_mask, view_state_mask,
};
use zerodds_dcps::serialized_type::SerializedTypeSupport;
use zerodds_dcps::status::SampleRejectedStatusKind;
use zerodds_dcps::{
    DataReader, DataReaderQos, DataWriterQos, DdsError, DdsType, DomainParticipant,
    DomainParticipantFactory, DomainParticipantQos, HANDLE_NIL, PublisherQos, RawBytes,
    SubscriberQos, TopicQos,
};
use zerodds_qos::Duration;

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

fn bytes_reader(p: &DomainParticipant, topic: &str) -> DataReader<RawBytes> {
    let topic = p
        .create_topic::<RawBytes>(topic, TopicQos::default())
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

/// `struct Reading { @key long id; long seq; }`, keyed the way `ffi.rs`
/// declares `conformance::Reading`: a four-octet key holder.
#[derive(Debug, Clone, PartialEq, Eq)]
struct KeyedId {
    id: i32,
    seq: i32,
}

impl DdsType for KeyedId {
    const TYPE_NAME: &'static str = "contract::KeyedId";
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);
    const EXTENSIBILITY: Extensibility = Extensibility::Final;

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.id.to_le_bytes());
        out.extend_from_slice(&self.seq.to_le_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let word = |at: usize| -> Result<i32, DecodeError> {
            bytes
                .get(at..at + 4)
                .and_then(|b| b.try_into().ok())
                .map(i32::from_le_bytes)
                .ok_or(DecodeError::Invalid { what: "short" })
        };
        Ok(Self {
            id: word(0)?,
            seq: word(4)?,
        })
    }

    fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
        holder.write_u32(u32::from_ne_bytes(self.id.to_ne_bytes()));
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
fn dcps_2_2_3_the_qos_builders_start_from_the_dds_defaults() {
    let w = DataWriterQos::default();
    assert_eq!(w.reliability.kind, ReliabilityKind::Reliable); // §2.2.3.14
    assert_eq!(w.reliability.max_blocking_time, Duration::from_millis(100));
    assert_eq!(w.durability.kind, DurabilityKind::Volatile); // §2.2.3.4
    assert_eq!(w.deadline.period, Duration::INFINITE); // §2.2.3.7
    assert_eq!(w.latency_budget.duration, Duration::ZERO); // §2.2.3.8
    assert_eq!(w.ownership.kind, OwnershipKind::Shared); // §2.2.3.9
    assert_eq!(w.ownership_strength.value, 0); // §2.2.3.10
    assert_eq!(w.liveliness.kind, LivelinessKind::Automatic); // §2.2.3.11
    assert_eq!(w.liveliness.lease_duration, Duration::INFINITE);
    assert_eq!(w.transport_priority.value, 0); // §2.2.3.15
    assert_eq!(w.lifespan.duration, Duration::INFINITE); // §2.2.3.16
    assert_eq!(
        w.destination_order.kind,
        DestinationOrderKind::ByReceptionTimestamp
    ); // §2.2.3.17
    assert_eq!(w.history.kind, HistoryKind::KeepLast); // §2.2.3.18
    assert_eq!(w.history.depth, 1);
    // §2.2.3.19: LENGTH_UNLIMITED (-1).
    assert_eq!(w.resource_limits.max_samples, -1);
    assert_eq!(w.resource_limits.max_instances, -1);
    assert_eq!(w.resource_limits.max_samples_per_instance, -1);
    assert!(w.writer_data_lifecycle.autodispose_unregistered_instances); // §2.2.3.21
    assert_eq!(w.durability_service.service_cleanup_delay, Duration::ZERO); // §2.2.3.5
    assert_eq!(w.durability_service.history_kind, HistoryKind::KeepLast);
    assert_eq!(w.durability_service.history_depth, 1);
    let r = DataReaderQos::default();
    assert_eq!(r.reliability.kind, ReliabilityKind::BestEffort); // §2.2.3.14
    assert_eq!(r.time_based_filter.minimum_separation, Duration::ZERO); // §2.2.3.12
    assert_eq!(
        r.reader_data_lifecycle.autopurge_nowriter_samples_delay,
        Duration::INFINITE
    ); // §2.2.3.22
    assert_eq!(
        r.reader_data_lifecycle.autopurge_disposed_samples_delay,
        Duration::INFINITE
    );
    assert!(r.partition.names.is_empty()); // §2.2.3.13
}

#[test]
fn dcps_2_3_3_the_state_masks_python_receives_have_the_psm_values() {
    // READ_SAMPLE_STATE = 0x0001 << 0, NOT_READ_SAMPLE_STATE = 0x0001 << 1,
    // NEW_VIEW_STATE = 0x0001 << 0, NOT_NEW_VIEW_STATE = 0x0001 << 1,
    // ALIVE_INSTANCE_STATE = 0x0001 << 0, NOT_ALIVE_DISPOSED = 0x0001 << 1,
    // NOT_ALIVE_NO_WRITERS = 0x0001 << 2, NOT_ALIVE = 0x006, ANY_* = 0xffff.
    assert_eq!(sample_state_mask::READ, 0x0001);
    assert_eq!(sample_state_mask::NOT_READ, 0x0002);
    assert_eq!(sample_state_mask::ANY, 0xffff);
    assert_eq!(view_state_mask::NEW, 0x0001);
    assert_eq!(view_state_mask::NOT_NEW, 0x0002);
    assert_eq!(view_state_mask::ANY, 0xffff);
    assert_eq!(instance_state_mask::ALIVE, 0x0001);
    assert_eq!(instance_state_mask::NOT_ALIVE_DISPOSED, 0x0002);
    assert_eq!(instance_state_mask::NOT_ALIVE_NO_WRITERS, 0x0004);
    assert_eq!(instance_state_mask::NOT_ALIVE, 0x0006);
    assert_eq!(instance_state_mask::ANY, 0xffff);
    // `PySampleInfo::from_info` matches each kind exhaustively; a sample
    // state matches the mask bit of its own kind and no other.
    let info = |sample, view, instance| SampleInfo {
        sample_state: sample,
        view_state: view,
        instance_state: instance,
        ..SampleInfo::default()
    };
    for (kind, bit) in [
        (SampleStateKind::Read, sample_state_mask::READ),
        (SampleStateKind::NotRead, sample_state_mask::NOT_READ),
    ] {
        let i = info(kind, ViewStateKind::New, InstanceStateKind::Alive);
        assert!(i.matches_states(bit, view_state_mask::ANY, instance_state_mask::ANY));
        assert!(!i.matches_states(!bit & 0x3, view_state_mask::ANY, instance_state_mask::ANY));
    }
    for (kind, bit) in [
        (InstanceStateKind::Alive, instance_state_mask::ALIVE),
        (
            InstanceStateKind::NotAliveDisposed,
            instance_state_mask::NOT_ALIVE_DISPOSED,
        ),
        (
            InstanceStateKind::NotAliveNoWriters,
            instance_state_mask::NOT_ALIVE_NO_WRITERS,
        ),
    ] {
        let i = info(SampleStateKind::NotRead, ViewStateKind::NotNew, kind);
        assert!(i.matches_states(sample_state_mask::ANY, view_state_mask::NOT_NEW, bit));
        assert!(!i.matches_states(sample_state_mask::ANY, view_state_mask::NEW, bit));
    }
}

#[test]
fn shapes_demo_shape_type_is_the_appendable_keyed_type_of_shape_idl() {
    // OMG dds-rtps `shape.idl`: `@appendable struct ShapeType { @key
    // string<128> color; int32 x; int32 y; int32 shapesize;
    // sequence<uint8> additional_payload_size; };`
    assert_eq!(ShapeType::type_name(), "ShapeType");
    assert!(ShapeType::has_key());
    assert_eq!(ShapeType::extensibility(), Extensibility::Appendable);
    let shape = ShapeType::new("BLUE", 10, 20, 30);
    // XTypes 1.3 §7.4.3: DELIMITED_CDR2 — a DHEADER with the length of the
    // members, the string as length (with the NUL) + octets + NUL, padding
    // to four, three int32, an empty sequence as its length.
    #[rustfmt::skip]
    let little: [u8; 32] = [
        0x1c, 0x00, 0x00, 0x00, // DHEADER 28
        0x05, 0x00, 0x00, 0x00, b'B', b'L', b'U', b'E', 0x00, 0x00, 0x00, 0x00,
        0x0a, 0x00, 0x00, 0x00, // x
        0x14, 0x00, 0x00, 0x00, // y
        0x1e, 0x00, 0x00, 0x00, // shapesize
        0x00, 0x00, 0x00, 0x00, // additional_payload_size: none
    ];
    #[rustfmt::skip]
    let big: [u8; 32] = [
        0x00, 0x00, 0x00, 0x1c,
        0x00, 0x00, 0x00, 0x05, b'B', b'L', b'U', b'E', 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x0a,
        0x00, 0x00, 0x00, 0x14,
        0x00, 0x00, 0x00, 0x1e,
        0x00, 0x00, 0x00, 0x00,
    ];
    let mut out = Vec::new();
    shape.encode(&mut out).unwrap();
    assert_eq!(out, little);
    let mut out = Vec::new();
    shape.encode_be(&mut out).unwrap();
    assert_eq!(out, big);
    assert_eq!(ShapeType::decode(&little).unwrap(), shape);
    // §7.6.8: the key holder is `color` as PLAIN_CDR2 big-endian
    // (00000005 424c554500); `string<128>` can exceed 16 octets, so the
    // KeyHash is its MD5.
    let md5_of_holder = unhex("cac217c318363f8ef1160eeedef9e886");
    assert_eq!(
        shape.compute_key_hash().map(|h| h.to_vec()),
        Some(md5_of_holder)
    );
}

#[test]
fn xtypes_7_6_8_a_key_holder_of_at_most_16_octets_is_the_key_hash_zero_padded() {
    // `KeyedReading`: `@key long id`, `KEY_HOLDER_MAX_SIZE = Some(4)`, the
    // holder written with `write_u32`, big-endian (§7.6.8 step 4).
    let sample = KeyedId { id: 7, seq: 1 };
    let mut holder = PlainCdr2BeKeyHolder::new();
    sample.encode_key_holder_be(&mut holder);
    assert_eq!(holder.as_bytes(), [0x00, 0x00, 0x00, 0x07]);
    // Step 5.1: the holder itself, zero-padded to 16 octets.
    assert_eq!(
        sample.compute_key_hash(),
        Some([0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
    );
    // The instance is the key: a second sample of id 7 is the same instance.
    let p = participant();
    let topic = p
        .create_topic::<KeyedId>("PyKeyedReading", TopicQos::default())
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<KeyedId>(&topic, DataWriterQos::default())
        .unwrap();
    let handle = writer.register_instance(&sample).unwrap();
    assert_eq!(writer.lookup_instance(&KeyedId { id: 7, seq: 2 }), handle);
    assert_eq!(
        writer.lookup_instance(&KeyedId { id: 8, seq: 1 }),
        HANDLE_NIL
    );
}

#[test]
fn xtypes_7_4_3_a_descriptor_topic_reads_and_writes_the_octets_of_its_type_object() {
    // `create_descriptor_topic`: the root TypeObject, no dependencies, the
    // flags the Python descriptor states (keyless, @final = 0).
    let object = unhex(PLAIN_COMPLETE);
    let support =
        SerializedTypeSupport::from_type_objects("to::Plain", &object, &[], false, 0).unwrap();
    assert_eq!(support.get_type_name(), "to::Plain");
    assert!(!support.is_keyed());
    assert_eq!(support.extensibility(), Extensibility::Final);
    // `a = 1, b = 2`: PLAIN_CDR2 little-endian, `long` then `short`.
    let body = [0x01, 0x00, 0x00, 0x00, 0x02, 0x00];
    let sample = support.read_body(&body, 2, false).unwrap();
    // Table 60: a @final structure is PLAIN_CDR under XCDR1 too, and no
    // member here aligns beyond four, so the octets are the same.
    assert_eq!(support.write_body(&sample, 0, false).unwrap(), body);
    assert_eq!(support.write_body(&sample, 2, false).unwrap(), body);
    assert_eq!(
        support.write_body(&sample, 2, true).unwrap(),
        [0x00, 0x00, 0x00, 0x01, 0x00, 0x02]
    );
    // §7.6.3.1.2: 1 is XML, not a binary representation; the descriptor
    // refuses it as `ffi.rs` does before it gets here.
    assert!(support.read_body(&body, 1, false).is_err());
    assert!(support.read_body(&body[..5], 2, false).is_err());
    // Flags that disagree with the object are refused.
    assert!(SerializedTypeSupport::from_type_objects("to::Plain", &object, &[], true, 0).is_err());
    assert!(SerializedTypeSupport::from_type_objects("to::Plain", &object, &[], false, 1).is_err());
    assert!(SerializedTypeSupport::from_type_objects("to::Other", &object, &[], false, 0).is_err());

    // The writer of a descriptor topic writes the body it was handed.
    let p = participant();
    let topic = p
        .create_serialized_topic("PyPlain", TopicQos::default(), Arc::clone(&support))
        .unwrap();
    assert_eq!(topic.type_name(), "to::Plain");
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter(&topic, DataWriterQos::default())
        .unwrap();
    writer.write(&sample).unwrap();
    assert_eq!(writer.__drain_pending(), vec![body.to_vec()]);
}

#[test]
fn dcps_2_2_2_1_6_3_a_waitset_returns_the_triggered_guard_and_times_out_without_one() {
    // §2.2.2.1.8: a GuardCondition starts with trigger_value FALSE.
    let guard = GuardCondition::new();
    assert!(!guard.get_trigger_value());
    let waitset = WaitSet::new();
    waitset
        .attach_condition(Arc::clone(&guard) as Arc<dyn Condition>)
        .unwrap();
    // "If the duration is exceeded, wait will also return with the return
    // code TIMEOUT" — Python's `TimeoutError`.
    assert!(matches!(
        waitset.wait(StdDuration::from_millis(10)),
        Err(DdsError::Timeout)
    ));
    guard.set_trigger_value(true);
    let triggered = waitset.wait(StdDuration::from_secs(5)).unwrap();
    assert_eq!(triggered.len(), 1);
    assert!(triggered[0].get_trigger_value());
}

#[test]
fn dcps_2_2_2_5_3_6_read_and_query_conditions_keep_their_masks_and_check_the_expression() {
    let seen = Arc::new(std::sync::Mutex::new(None));
    let record = Arc::clone(&seen);
    let rc = ReadCondition::new(
        sample_state_mask::NOT_READ,
        view_state_mask::ANY,
        instance_state_mask::ALIVE,
        move |ss, vs, is| {
            *record.lock().unwrap() = Some((ss, vs, is));
            ss != 0 && vs != 0 && is != 0
        },
    );
    assert_eq!(rc.get_sample_state_mask(), sample_state_mask::NOT_READ);
    assert_eq!(rc.get_view_state_mask(), view_state_mask::ANY);
    assert_eq!(rc.get_instance_state_mask(), instance_state_mask::ALIVE);
    // The trigger `conditions.rs` builds is called with the three masks.
    assert!(rc.get_trigger_value());
    assert_eq!(*seen.lock().unwrap(), Some((0x0002, 0xffff, 0x0001)));
    // Annex B: `x > %0` is a FilterExpression; `x >` is none, and
    // create_querycondition refuses it.
    let base = || {
        ReadCondition::new(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
            |ss, vs, is| ss != 0 && vs != 0 && is != 0,
        )
    };
    let qc = QueryCondition::new(base(), "x > %0", vec!["5".into()]).unwrap();
    assert_eq!(qc.get_query_expression(), "x > %0");
    assert_eq!(qc.get_query_parameters(), vec!["5".to_string()]);
    assert!(matches!(
        QueryCondition::new(base(), "x >", Vec::new()),
        Err(DdsError::BadParameter { .. })
    ));
}

#[test]
fn dcps_2_2_2_2_1_14_an_offline_participant_ignores_by_handle_and_knows_its_entities() {
    let p = participant();
    assert_eq!(p.discovered_participants_count(), 0, "0 in offline mode");
    // `ffi.rs` passes the handle Python holds as a u64.
    let peer = InstanceHandle::from_raw(0x0102_0304_0506_0708);
    assert_eq!(peer.as_raw(), 0x0102_0304_0506_0708);
    p.ignore_participant(peer).unwrap();
    assert!(p.is_participant_ignored(peer), "ignoring is not reversible");
    assert!(!p.is_participant_ignored(InstanceHandle::from_raw(5)));
    let before = p.topics_len();
    let topic = p
        .create_topic::<RawBytes>("PyContains", TopicQos::default())
        .unwrap();
    assert_eq!(p.topics_len(), before + 1);
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    // §2.2.2.2.1.31 contains_entity: the participant's own, recursively.
    assert!(p.contains_entity(writer.instance_handle()));
    assert!(!p.contains_entity(HANDLE_NIL));
}

#[test]
fn the_offline_wait_operations_answer_at_once() {
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("PyWaits", TopicQos::default())
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let reader = bytes_reader(&p, "PyWaitsReader");
    // `wait_for_data`: "Offline mode: `Ok` iff the inbox is non-empty",
    // `Timeout` otherwise — Python's `TimeoutError`.
    assert!(matches!(
        reader.wait_for_data(StdDuration::from_millis(10)),
        Err(DdsError::Timeout)
    ));
    reader.__push_raw(vec![1]).unwrap();
    reader.wait_for_data(StdDuration::from_millis(10)).unwrap();
    // `wait_for_acknowledgments`: "Returns `Ok(())` immediately in offline
    // mode".
    writer.write(&RawBytes::new(vec![1])).unwrap();
    writer
        .wait_for_acknowledgments(StdDuration::from_millis(10))
        .unwrap();
    // No match offline: a wait for one reader or writer times out.
    assert!(matches!(
        writer.wait_for_matched_subscription(1, StdDuration::ZERO),
        Err(DdsError::Timeout)
    ));
    assert!(matches!(
        reader.wait_for_matched_publication(1, StdDuration::ZERO),
        Err(DdsError::Timeout)
    ));
    // `liveliness_changed_status`: "Offline: `(false, 0, 0)`."
    assert_eq!(reader.liveliness_changed_status(), (false, 0, 0));
    let rejected = reader.get_sample_rejected_status().unwrap();
    assert_eq!(rejected.total_count, 0);
    assert_eq!(rejected.last_reason, SampleRejectedStatusKind::NotRejected);
}

#[test]
fn dcps_2_2_2_1_1_3_a_listener_is_invoked_only_for_the_statuses_in_its_mask() {
    // "The listener will only be invoked on the changes of communication
    // status indicated by the specified mask." A mask of 0 selects none.
    let p = participant();
    let reader = bytes_reader(&p, "PyListener");
    let (tx, rx) = sync_channel::<InstanceHandle>(16);
    reader.set_listener(Some(Arc::new(Arrivals(tx.clone()))), 0);
    reader.__push_raw(vec![1]).unwrap();
    assert!(rx.try_recv().is_err(), "mask 0 selects no status");
    reader.set_listener(Some(Arc::new(Arrivals(tx))), status::DATA_AVAILABLE);
    reader.__push_raw(vec![2]).unwrap();
    let handle = rx
        .recv_timeout(StdDuration::from_secs(5))
        .expect("on_data_available");
    assert_eq!(handle, reader.instance_handle());
}
