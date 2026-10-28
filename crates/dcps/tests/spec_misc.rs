// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Small items of DDS 1.4 that held no test of their own: the enabled
//! statuses of a StatusCondition (§2.2.2.1.9), `get_inconsistent_topic_status`
//! (§2.2.2.3.2.1), the timestamps of `dispose_w_timestamp` and
//! `unregister_instance_w_timestamp` (§2.2.2.4.2.8, §2.2.2.4.2.14), the
//! error of a handle that matches nothing (§2.2.2.4.2.23, §2.2.2.5.3.33),
//! `no_writers_generation_count` and the view state of a reborn instance
//! (§2.2.2.5.1.5, §2.2.2.5.1.8), `wait_for_historical_data` (§2.2.2.5.3.32)
//! and the policies that cannot change after `enable` (§2.2.3).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

#[path = "common/mod.rs"]
mod common;

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::psm_constants::status;
use zerodds_dcps::qos::{
    DurabilityKind, DurabilityQosPolicy, ReliabilityKind, ReliabilityQosPolicy,
};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DdsType, DomainParticipantFactory,
    DomainParticipantQos, Entity, HANDLE_NIL, InstanceHandle, InstanceStateKind, PublisherQos,
    RawBytes, SubscriberQos, Time, TopicQos, ViewStateKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Reading {
    id: u32,
    seq: u32,
}

impl DdsType for Reading {
    const TYPE_NAME: &'static str = "test::SpecMiscReading";
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.id.to_le_bytes());
        out.extend_from_slice(&self.seq.to_le_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < 8 {
            return Err(DecodeError::Invalid { what: "short" });
        }
        Ok(Self {
            id: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            seq: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        })
    }

    fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
        holder.write_u32(self.id);
    }

    fn decode_key_holder_be(bytes: &[u8]) -> Result<Self, DecodeError> {
        let head: [u8; 4] = bytes
            .get(..4)
            .and_then(|b| b.try_into().ok())
            .ok_or(DecodeError::Invalid { what: "short key" })?;
        Ok(Self {
            id: u32::from_be_bytes(head),
            ..Self::default()
        })
    }
}

struct Other;

impl DdsType for Other {
    const TYPE_NAME: &'static str = "test::SpecMiscOther";
    const XCDR1_MATCHES_XCDR2: bool = true;

    fn encode(&self, _out: &mut Vec<u8>) -> Result<(), EncodeError> {
        Ok(())
    }

    fn decode(_bytes: &[u8]) -> Result<Self, DecodeError> {
        Ok(Self)
    }
}

struct Rig {
    _participant: common::ParticipantGuard,
    _lease: common::DomainLease,
}

fn rig(
    autodispose: bool,
) -> (
    Rig,
    zerodds_dcps::publisher::DataWriter<Reading>,
    zerodds_dcps::subscriber::DataReader<Reading>,
) {
    let lease = common::lease_domain();
    let p = DomainParticipantFactory::instance()
        .create_participant_with_config(
            lease.id(),
            DomainParticipantQos::default(),
            common::isolated_cfg(),
        )
        .expect("participant");
    let topic = p
        .create_topic::<Reading>("SpecMiscReadings", TopicQos::default())
        .expect("topic");
    let reliability = ReliabilityQosPolicy {
        kind: ReliabilityKind::Reliable,
        ..Default::default()
    };
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Reading>(
            &topic,
            DataWriterQos {
                reliability,
                writer_data_lifecycle: zerodds_dcps::qos::WriterDataLifecycleQosPolicy {
                    autodispose_unregistered_instances: autodispose,
                },
                history: zerodds_dcps::qos::HistoryQosPolicy {
                    kind: zerodds_dcps::qos::HistoryKind::KeepAll,
                    depth: 1,
                },
                ..Default::default()
            },
        )
        .expect("writer");
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Reading>(
            &topic,
            DataReaderQos {
                reliability,
                history: zerodds_dcps::qos::HistoryQosPolicy {
                    kind: zerodds_dcps::qos::HistoryKind::KeepAll,
                    depth: 1,
                },
                ..Default::default()
            },
        )
        .expect("reader");
    (
        Rig {
            _participant: common::ParticipantGuard::new(p),
            _lease: lease,
        },
        writer,
        reader,
    )
}

/// §2.2.2.1.9.1: "the default value of enabled_statuses is the set of all
/// statuses"; §2.2.2.1.9.2 reads back "the statuses that were explicitly
/// set on the last call to set_enabled_statuses".
#[test]
fn a_status_condition_starts_with_every_status_enabled() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let condition = p.get_status_condition();
    assert_eq!(condition.enabled_statuses(), status::ANY);
    condition.set_enabled_statuses(status::PUBLICATION_MATCHED | status::LIVELINESS_LOST);
    assert_eq!(
        condition.enabled_statuses(),
        status::PUBLICATION_MATCHED | status::LIVELINESS_LOST
    );
    // The one StatusCondition of the entity.
    assert_eq!(
        p.get_status_condition().enabled_statuses(),
        status::PUBLICATION_MATCHED | status::LIVELINESS_LOST
    );
}

/// §2.2.2.3.2.1 `get_inconsistent_topic_status` and §2.2.4.1: a second
/// topic of the name with another type is inconsistent; the status counts it
/// and the read takes the change off.
#[test]
fn get_inconsistent_topic_status_reports_the_change_once() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<RawBytes>("SpecMiscInconsistent", TopicQos::default())
        .unwrap();
    // The same type name under another type is refused and counted.
    assert!(
        p.create_topic_with_type_name::<Other>(
            "SpecMiscInconsistent",
            "test::OtherName",
            TopicQos::default()
        )
        .is_ok(),
        "another type name under the name is a second topic, not inconsistent"
    );
    // The type name of the first topic under a keyed type: one name for two
    // types.
    let inconsistent = p.create_topic_with_type_name::<Reading>(
        "SpecMiscInconsistent",
        RawBytes::TYPE_NAME,
        TopicQos::default(),
    );
    assert!(matches!(
        inconsistent,
        Err(DdsError::InconsistentPolicy { .. })
    ));
    let first = topic.get_inconsistent_topic_status().unwrap();
    assert_eq!((first.total_count, first.total_count_change), (1, 1));
    let second = topic.get_inconsistent_topic_status().unwrap();
    assert_eq!((second.total_count, second.total_count_change), (1, 0));
}

/// §2.2.2.4.2.23 and §2.2.2.5.3.33: a handle of no matched endpoint is
/// BAD_PARAMETER.
#[test]
fn a_handle_that_matches_nothing_is_bad_parameter() {
    let (_rig, writer, reader) = rig(true);
    let foreign = InstanceHandle::from_raw(0xBAD0_BAD0);
    assert!(matches!(
        writer.matched_subscription_data(foreign),
        Err(DdsError::BadParameter { .. })
    ));
    assert!(matches!(
        reader.matched_publication_data(foreign),
        Err(DdsError::BadParameter { .. })
    ));
}

/// §2.2.2.4.2.14 and §2.2.2.4.2.8: the timestamp of `dispose_w_timestamp`
/// and of `unregister_instance_w_timestamp` is the source timestamp of the
/// marker at the reader.
#[test]
fn the_timestamps_of_dispose_and_unregister_reach_the_reader() {
    let (_rig, writer, reader) = rig(false);
    let disposed = Reading { id: 1, seq: 1 };
    let unregistered = Reading { id: 2, seq: 1 };
    writer.write(&disposed).unwrap();
    writer.write(&unregistered).unwrap();
    let dispose_time = Time::new(1_000, 5);
    let unregister_time = Time::new(2_000, 7);
    writer
        .dispose_w_timestamp(&disposed, HANDLE_NIL, dispose_time)
        .unwrap();
    writer
        .unregister_instance_w_timestamp(&unregistered, HANDLE_NIL, unregister_time)
        .unwrap();

    let mut markers = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while markers.len() < 2 && std::time::Instant::now() < deadline {
        markers.extend(
            reader
                .take_with_info()
                .unwrap()
                .into_iter()
                .filter(|s| !s.info.valid_data),
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(markers.len(), 2);
    let of = |state| {
        markers
            .iter()
            .find(|s| s.info.instance_state == state)
            .unwrap_or_else(|| panic!("no marker in {state:?}"))
            .info
            .source_timestamp
    };
    assert_eq!(of(InstanceStateKind::NotAliveDisposed), dispose_time);
    assert_eq!(of(InstanceStateKind::NotAliveNoWriters), unregister_time);
}

/// §2.2.2.5.1.5: the instance that goes NOT_ALIVE_NO_WRITERS and comes back
/// counts `no_writers_generation_count`; §2.2.2.5.1.8: it is NEW again.
#[test]
fn a_reborn_instance_counts_a_generation_and_is_new_again() {
    let (_rig, writer, reader) = rig(false);
    let sample = Reading { id: 7, seq: 1 };
    writer.write(&sample).unwrap();
    let wait = |reader: &zerodds_dcps::subscriber::DataReader<Reading>, want: usize| {
        let mut got = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while got.len() < want && std::time::Instant::now() < deadline {
            got.extend(reader.take_with_info().unwrap());
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        got
    };
    let first = wait(&reader, 1);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].info.view_state, ViewStateKind::New);
    assert_eq!(first[0].info.no_writers_generation_count, 0);

    writer.unregister_instance(&sample, HANDLE_NIL).unwrap();
    let marker = wait(&reader, 1);
    assert_eq!(marker.len(), 1);
    assert_eq!(
        marker[0].info.instance_state,
        InstanceStateKind::NotAliveNoWriters
    );

    writer.write(&Reading { id: 7, seq: 2 }).unwrap();
    let again = wait(&reader, 1);
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].info.instance_state, InstanceStateKind::Alive);
    assert_eq!(again[0].info.no_writers_generation_count, 1);
    assert_eq!(again[0].info.view_state, ViewStateKind::New);
}

/// §2.2.2.5.3.32: a reader that no writer has to serve has no historical
/// data to wait for and returns at once.
#[test]
fn wait_for_historical_data_returns_when_no_writer_is_matched() {
    let lease = common::lease_domain();
    let p = common::ParticipantGuard::new(
        DomainParticipantFactory::instance()
            .create_participant_with_config(
                lease.id(),
                DomainParticipantQos::default(),
                common::isolated_cfg(),
            )
            .unwrap(),
    );
    let topic = p
        .create_topic::<Reading>("SpecMiscHistorical", TopicQos::default())
        .unwrap();
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Reading>(
            &topic,
            DataReaderQos {
                durability: DurabilityQosPolicy {
                    kind: DurabilityKind::TransientLocal,
                },
                reliability: ReliabilityQosPolicy {
                    kind: ReliabilityKind::Reliable,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
    let started = std::time::Instant::now();
    reader
        .wait_for_historical_data(std::time::Duration::from_secs(5))
        .expect("nothing to wait for");
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

/// §2.2.3: OWNERSHIP, LIVELINESS, RESOURCE_LIMITS and DURABILITY_SERVICE are
/// "Changeable: No"; WRITER_DATA_LIFECYCLE and READER_DATA_LIFECYCLE are
/// "Changeable: Yes".
#[test]
fn the_changeable_column_holds_after_enable() {
    use zerodds_dcps::qos::{
        DurabilityServiceQosPolicy, LivelinessKind, OwnershipKind, ReaderDataLifecycleQosPolicy,
        ResourceLimitsQosPolicy, WriterDataLifecycleQosPolicy,
    };
    let (_rig, writer, reader) = rig(true);

    let mut changed = writer.get_qos();
    changed.ownership.kind = OwnershipKind::Exclusive;
    assert!(matches!(
        writer.set_qos(changed),
        Err(DdsError::ImmutablePolicy { .. })
    ));
    let mut changed = writer.get_qos();
    changed.liveliness.kind = LivelinessKind::ManualByTopic;
    assert!(matches!(
        writer.set_qos(changed),
        Err(DdsError::ImmutablePolicy { .. })
    ));
    let mut changed = writer.get_qos();
    changed.resource_limits = ResourceLimitsQosPolicy {
        max_samples: 10,
        ..Default::default()
    };
    assert!(matches!(
        writer.set_qos(changed),
        Err(DdsError::ImmutablePolicy { .. })
    ));
    let mut changed = writer.get_qos();
    changed.durability_service = DurabilityServiceQosPolicy {
        history_depth: 5,
        ..Default::default()
    };
    assert!(matches!(
        writer.set_qos(changed),
        Err(DdsError::ImmutablePolicy { .. })
    ));
    let mut changed = writer.get_qos();
    changed.writer_data_lifecycle = WriterDataLifecycleQosPolicy {
        autodispose_unregistered_instances: false,
    };
    writer
        .set_qos(changed)
        .expect("WRITER_DATA_LIFECYCLE changes");

    let mut changed = reader.get_qos();
    changed.ownership.kind = OwnershipKind::Exclusive;
    assert!(matches!(
        reader.set_qos(changed),
        Err(DdsError::ImmutablePolicy { .. })
    ));
    let mut changed = reader.get_qos();
    changed.liveliness.kind = LivelinessKind::ManualByTopic;
    assert!(matches!(
        reader.set_qos(changed),
        Err(DdsError::ImmutablePolicy { .. })
    ));
    let mut changed = reader.get_qos();
    changed.resource_limits = ResourceLimitsQosPolicy {
        max_samples: 10,
        ..Default::default()
    };
    assert!(matches!(
        reader.set_qos(changed),
        Err(DdsError::ImmutablePolicy { .. })
    ));
    let mut changed = reader.get_qos();
    changed.reader_data_lifecycle = ReaderDataLifecycleQosPolicy {
        autopurge_nowriter_samples_delay: zerodds_qos::Duration::from_secs(5),
        ..Default::default()
    };
    reader
        .set_qos(changed)
        .expect("READER_DATA_LIFECYCLE changes");
}
