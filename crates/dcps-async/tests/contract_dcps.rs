// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-dcps-async` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-dcps`:
//! - `src/factory.rs`: `DomainParticipantFactory::instance`,
//!   `create_participant_offline`, `create_participant` and, with
//!   `tokio-glue`, `create_participant_with_config` with a
//!   `runtime::RuntimeConfig` built over `RuntimeConfig::default()`
//!   (`user_data`, `external_tick`), `DomainParticipant::runtime` and the
//!   tick driver of the runtime (`tick_driver`, `tick_period`, `is_stopped`,
//!   `tick`).
//! - `src/participant.rs`: `DomainParticipant::{domain_id, create_topic,
//!   create_contentfilteredtopic, create_publisher, create_subscriber}`.
//! - `src/publisher.rs`, `src/subscriber.rs`: `Publisher::create_datawriter`,
//!   `Subscriber::create_datareader`, `DataReader::with_filter`, and for a
//!   ContentFilteredTopic `get_related_topic` and `evaluate` over
//!   `DdsTypeRow::new`, an `Err` taken as "filtered out".
//! - `src/writer.rs`: `DataWriter::{write, qos, register_instance, dispose,
//!   unregister_instance, matched_subscription_count}` and
//!   `runtime::without_waiting_for_room`. `write`, `dispose` and
//!   `unregister_instance` run under it and are retried on
//!   `DdsError::Timeout` until `qos().reliability.max_blocking_time` (read
//!   through `to_nanos`) has passed, then `DdsError::Timeout`.
//!   `status::PublicationMatchedStatus` is built field by field with
//!   `HANDLE_NIL` as the last handle.
//! - `src/reader.rs`: `DataReader::{take, read, take_with_info,
//!   read_with_info, matched_publication_count, qos, runtime_handle}`,
//!   `DcpsRuntime::register_user_reader_waker`, `Sample<T>`,
//!   `status::SubscriptionMatchedStatus` built field by field.
//! - `src/lib.rs`: re-exports of `DataReaderQos`, `DataWriterQos`,
//!   `DdsError`, `DdsType`, `DomainParticipantQos`, `InstanceHandle`,
//!   `PublisherQos`, `Result`, `SubscriberQos`, `Topic`, `TopicQos` and the
//!   two matched statuses.
//!
//! What it relies on: an offline participant and its endpoints have no
//! runtime, so the streams fall back to polling; an empty reader answers
//! `take`/`read` with an empty `Vec`, not an error; `read` leaves the samples
//! in the reader and `take` removes them; a full offline queue refuses with
//! `OutOfResources` where waiting cannot help; matched counts are 0 offline.
//!
//! Oracles: DDS 1.4 §2.2.2.2.2.3 (the factory singleton), §2.2.2.2.1.7
//! (ContentFilteredTopic) with Annex B (`%N` parameters), §2.2.2.4.2.5,
//! .7 and .13 (instance handles), §2.2.2.4.2.11 (OUT_OF_RESOURCES instead of
//! blocking), §2.2.2.5.1.2 and §2.2.2.5.3.9 (read and take), §2.2.3.14
//! (RELIABILITY defaults), §2.2.4.1 (matched statuses), and the rustdoc of
//! `DataReader::take`, `DataReader::runtime_handle`,
//! `DataWriter::matched_subscription_count` and
//! `DataReader::matched_publication_count`. Samples reach an offline reader
//! through `DataReader::__push_raw`, the hook the dcps tests use for the
//! receive path; no socket is opened.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use core::any::TypeId;

use zerodds_dcps::builtin_types::DdsKeyedString;
use zerodds_dcps::qos::{
    HistoryKind, HistoryQosPolicy, ReliabilityKind, ReliabilityQosPolicy, ResourceLimitsQosPolicy,
};
use zerodds_dcps::sample_info::SampleStateKind;
use zerodds_dcps::status::{PublicationMatchedStatus, SubscriptionMatchedStatus};
use zerodds_dcps::{
    DataReader, DataReaderQos, DataWriterQos, DdsError, DdsType, DdsTypeRow, DecodeError,
    DomainParticipant, DomainParticipantFactory, DomainParticipantQos, EncodeError, FilterValue,
    HANDLE_NIL, InstanceHandle, PublisherQos, RawBytes, SubscriberQos, TopicQos,
};
use zerodds_qos::Duration;

fn participant() -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default())
}

/// A reader of `RawBytes` that keeps every sample (KEEP_ALL): under the
/// default KEEP_LAST(1) a second sample of the one instance replaces the
/// first (DDS 1.4 §2.2.3.18).
fn keep_all_reader(p: &DomainParticipant, topic: &str) -> DataReader<RawBytes> {
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

/// A sample type whose member `x` a filter expression can name.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Reading {
    x: i32,
}

impl DdsType for Reading {
    const TYPE_NAME: &'static str = "contract::Reading";

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.x.to_le_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let octets: [u8; 4] = bytes
            .get(..4)
            .and_then(|b| b.try_into().ok())
            .ok_or(DecodeError::Invalid { what: "short" })?;
        Ok(Self {
            x: i32::from_le_bytes(octets),
        })
    }

    fn field_value(&self, path: &str) -> Option<FilterValue> {
        if path == "x" {
            Some(FilterValue::Int(i64::from(self.x)))
        } else {
            None
        }
    }
}

#[test]
fn dcps_2_2_2_2_2_3_the_factory_is_one_instance_and_an_offline_participant_has_no_runtime() {
    // "This operation returns the DomainParticipantFactory singleton." The
    // async factory holds the `&'static` it is handed and shares it with
    // the sync API.
    assert!(core::ptr::eq(
        DomainParticipantFactory::instance(),
        DomainParticipantFactory::instance()
    ));
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(7, DomainParticipantQos::default());
    assert_eq!(p.domain_id(), 7);
    // `spawn_in_tokio` drives a tick loop only where there is a runtime.
    assert!(p.runtime().is_none());
}

#[test]
fn offline_endpoints_have_no_runtime_handle_and_no_matches() {
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("AsyncOffline", TopicQos::default())
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    // The streams register a waker only on a live reader; offline they poll.
    assert!(reader.runtime_handle().is_none());
    // "Always 0 in offline mode": `wait_for_matched_*` time out on it.
    assert_eq!(writer.matched_subscription_count(), 0);
    assert_eq!(reader.matched_publication_count(), 0);
}

#[test]
fn an_empty_reader_answers_every_access_with_an_empty_collection() {
    // `DataReader::take`: "Returns an empty Vec if there is nothing." The
    // async `take(timeout)` resolves to that empty `Vec` on timeout.
    let p = participant();
    let reader = keep_all_reader(&p, "AsyncEmpty");
    assert!(reader.take().unwrap().is_empty());
    assert!(reader.read().unwrap().is_empty());
    assert!(reader.take_with_info().unwrap().is_empty());
    assert!(reader.read_with_info().unwrap().is_empty());
}

#[test]
fn dcps_2_2_2_5_3_9_read_leaves_the_samples_and_take_removes_them() {
    let p = participant();
    let reader = keep_all_reader(&p, "AsyncReadTake");
    reader.__push_raw(vec![1]).unwrap();
    // `DataAvailableStream` counts what `read` returns: it must not shrink
    // when a sample has been read before, and must grow with each arrival.
    assert_eq!(reader.read().unwrap(), vec![RawBytes::new(vec![1])]);
    reader.__push_raw(vec![2]).unwrap();
    assert_eq!(reader.read().unwrap().len(), 2);
    // §2.2.2.5.1.2: a sample accessed by `read` is READ.
    let infos = reader.read_with_info().unwrap();
    assert_eq!(infos.len(), 2);
    assert!(
        infos
            .iter()
            .all(|s| s.info.sample_state == SampleStateKind::Read)
    );
    // §2.2.2.5.3.9: "The act of taking a sample removes it from the
    // DataReader so it cannot be 'read' or 'taken' again."
    let taken = reader.take_with_info().unwrap();
    let data: Vec<Vec<u8>> = taken.into_iter().map(|s| s.data.data).collect();
    assert_eq!(data, vec![vec![1], vec![2]]);
    assert!(reader.take().unwrap().is_empty());
    assert!(reader.read().unwrap().is_empty());
}

#[test]
fn a_reader_with_a_filter_delivers_only_what_the_filter_accepts() {
    // `create_datareader_filtered` is `create_datareader(..).with_filter(f)`.
    let p = participant();
    let reader = keep_all_reader(&p, "AsyncFiltered").with_filter(|s: &RawBytes| s.data[0] > 1);
    reader.__push_raw(vec![1]).unwrap();
    reader.__push_raw(vec![2]).unwrap();
    reader.__push_raw(vec![3]).unwrap();
    let taken: Vec<Vec<u8>> = reader.take().unwrap().into_iter().map(|s| s.data).collect();
    assert_eq!(taken, vec![vec![2], vec![3]]);
}

#[test]
fn dcps_2_2_2_2_1_7_a_content_filtered_topic_evaluates_its_expression_with_its_parameters() {
    let p = participant();
    let topic = p
        .create_topic::<Reading>("AsyncReadings", TopicQos::default())
        .unwrap();
    // Annex B: `%0` is the first filter parameter, "5" an integer literal.
    let cft = p
        .create_contentfilteredtopic(
            "AsyncReadingsAboveFive",
            &topic,
            "x > %0",
            vec!["5".to_string()],
        )
        .unwrap();
    assert_eq!(cft.get_related_topic().name(), "AsyncReadings");
    assert!(cft.evaluate(&DdsTypeRow::new(&Reading { x: 7 })).unwrap());
    assert!(!cft.evaluate(&DdsTypeRow::new(&Reading { x: 3 })).unwrap());
    // A member the type does not answer is an error, which the async
    // reader takes as "filtered out".
    let unknown = p
        .create_contentfilteredtopic("AsyncReadingsByY", &topic, "y > 1", Vec::new())
        .unwrap();
    assert!(matches!(
        unknown.evaluate(&DdsTypeRow::new(&Reading { x: 7 })),
        Err(DdsError::BadParameter { .. })
    ));
}

#[test]
fn dcps_2_2_2_2_1_7_a_content_filtered_topic_needs_a_name_and_every_named_parameter() {
    let p = participant();
    let topic = p
        .create_topic::<Reading>("AsyncReadingsRefused", TopicQos::default())
        .unwrap();
    assert!(matches!(
        p.create_contentfilteredtopic("", &topic, "x > 1", Vec::new()),
        Err(DdsError::BadParameter { .. })
    ));
    assert!(matches!(
        p.create_contentfilteredtopic("AsyncMissingParam", &topic, "x > %1", vec!["1".into()]),
        Err(DdsError::BadParameter { .. })
    ));
}

#[test]
fn dcps_2_2_3_14_a_writer_offers_reliable_with_a_max_blocking_time_of_100_ms() {
    // The retry loop of `AsyncDataWriter::write` reads its budget from here.
    let defaults = DataWriterQos::default().reliability;
    assert_eq!(defaults.kind, ReliabilityKind::Reliable);
    assert_eq!(defaults.max_blocking_time.to_nanos(), 100_000_000);
    // `qos()` returns the QoS the writer was created with.
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("AsyncWriterQos", TopicQos::default())
        .unwrap();
    let reliability = ReliabilityQosPolicy {
        kind: ReliabilityKind::Reliable,
        max_blocking_time: Duration::from_millis(250),
    };
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(
            &topic,
            DataWriterQos {
                reliability,
                ..DataWriterQos::default()
            },
        )
        .unwrap();
    assert_eq!(writer.qos().reliability, reliability);
    assert_eq!(
        writer.qos().reliability.max_blocking_time.to_nanos(),
        250_000_000
    );
}

#[test]
fn dcps_2_2_2_4_2_11_a_full_queue_refuses_with_out_of_resources_where_waiting_cannot_help() {
    // "Instead of blocking, the write operation is allowed to return
    // immediately with the error code OUT_OF_RESOURCES" when waiting has no
    // chance of freeing resources: a BEST_EFFORT writer, or one that may
    // not wait at all. The async writer passes this error on.
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("AsyncBackpressure", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let limits = ResourceLimitsQosPolicy {
        max_samples: 1,
        max_instances: zerodds_dcps::psm_constants::LENGTH_UNLIMITED,
        max_samples_per_instance: 1,
    };
    for reliability in [
        ReliabilityQosPolicy {
            kind: ReliabilityKind::BestEffort,
            max_blocking_time: Duration::from_millis(100),
        },
        ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: Duration::ZERO,
        },
    ] {
        let writer = publisher
            .create_datawriter::<RawBytes>(
                &topic,
                DataWriterQos {
                    reliability,
                    resource_limits: limits,
                    ..DataWriterQos::default()
                },
            )
            .unwrap();
        writer.write(&RawBytes::new(vec![1])).unwrap();
        assert!(
            matches!(
                writer.write(&RawBytes::new(vec![2])),
                Err(DdsError::OutOfResources { .. })
            ),
            "{reliability:?}"
        );
    }
}

/// `runtime::without_waiting_for_room`: where the write would wait for room
/// (RELIABLE, `max_blocking_time` 5 s, the one sample of `max_samples`
/// written), it returns TIMEOUT at once; the async writer suspends on it.
#[test]
fn dcps_2_2_2_4_2_11_a_write_that_may_not_wait_returns_timeout_at_once() {
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("AsyncNoWait", TopicQos::default())
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(
            &topic,
            DataWriterQos {
                reliability: ReliabilityQosPolicy {
                    kind: ReliabilityKind::Reliable,
                    max_blocking_time: Duration::from_secs(5),
                },
                resource_limits: ResourceLimitsQosPolicy {
                    max_samples: 1,
                    max_instances: 1,
                    max_samples_per_instance: 1,
                },
                history: HistoryQosPolicy {
                    kind: HistoryKind::KeepAll,
                    depth: 1,
                },
                ..DataWriterQos::default()
            },
        )
        .unwrap();
    writer.write(&RawBytes::new(vec![1])).unwrap();
    let began = std::time::Instant::now();
    let second =
        zerodds_dcps::runtime::without_waiting_for_room(|| writer.write(&RawBytes::new(vec![2])));
    assert!(matches!(second, Err(DdsError::Timeout)), "{second:?}");
    assert!(began.elapsed() < std::time::Duration::from_secs(1));
}

#[test]
fn dcps_2_2_2_4_2_5_register_dispose_and_unregister_follow_the_handle_rules() {
    let p = participant();
    let topic = p
        .create_topic::<DdsKeyedString>("AsyncInstances", TopicQos::default())
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<DdsKeyedString>(&topic, DataWriterQos::default())
        .unwrap();
    let red = DdsKeyedString::new("red", "1");
    let blue = DdsKeyedString::new("blue", "2");
    // §2.2.2.4.2.5: "The operation register_instance is idempotent. If it is
    // called for an already registered instance, it just returns the already
    // allocated handle."
    let h_red = writer.register_instance(&red).unwrap();
    assert!(!h_red.is_nil());
    assert_eq!(writer.register_instance(&red).unwrap(), h_red);
    let h_blue = writer.register_instance(&blue).unwrap();
    assert_ne!(h_blue, h_red);
    // §2.2.2.4.2.7: a handle of another existing instance is
    // PRECONDITION_NOT_MET, one of no instance BAD_PARAMETER (dispose
    // follows the same handle rules, §2.2.2.4.2.13).
    assert!(matches!(
        writer.dispose(&red, h_blue),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    assert!(matches!(
        writer.dispose(&red, InstanceHandle::from_raw(u64::MAX)),
        Err(DdsError::BadParameter { .. })
    ));
    writer.dispose(&red, h_red).unwrap();
    // HANDLE_NIL: the instance is deduced from the key.
    writer.unregister_instance(&blue, HANDLE_NIL).unwrap();
}

#[test]
fn dcps_2_2_4_1_an_offline_endpoint_reports_the_matched_statuses_the_async_streams_build() {
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("AsyncMatched", TopicQos::default())
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    // The five fields of PUBLICATION_MATCHED and SUBSCRIPTION_MATCHED, as the
    // streams fill them for a count of zero: no match, no handle.
    assert_eq!(
        writer.get_publication_matched_status().unwrap(),
        PublicationMatchedStatus {
            total_count: 0,
            total_count_change: 0,
            current_count: 0,
            current_count_change: 0,
            last_subscription_handle: HANDLE_NIL,
        }
    );
    assert_eq!(
        reader.get_subscription_matched_status().unwrap(),
        SubscriptionMatchedStatus {
            total_count: 0,
            total_count_change: 0,
            current_count: 0,
            current_count_change: 0,
            last_publication_handle: HANDLE_NIL,
        }
    );
    assert!(HANDLE_NIL.is_nil());
}

#[test]
fn the_re_exported_types_are_the_dcps_types() {
    assert_eq!(
        TypeId::of::<zerodds_dcps_async::DdsError>(),
        TypeId::of::<DdsError>()
    );
    assert_eq!(
        TypeId::of::<zerodds_dcps_async::InstanceHandle>(),
        TypeId::of::<InstanceHandle>()
    );
    assert_eq!(
        TypeId::of::<zerodds_dcps_async::DataWriterQos>(),
        TypeId::of::<DataWriterQos>()
    );
    assert_eq!(
        TypeId::of::<zerodds_dcps_async::Topic<RawBytes>>(),
        TypeId::of::<zerodds_dcps::Topic<RawBytes>>()
    );
    assert_eq!(
        TypeId::of::<zerodds_dcps_async::PublicationMatchedStatus>(),
        TypeId::of::<PublicationMatchedStatus>()
    );
    assert_eq!(
        TypeId::of::<zerodds_dcps_async::SubscriptionMatchedStatus>(),
        TypeId::of::<SubscriptionMatchedStatus>()
    );
}
