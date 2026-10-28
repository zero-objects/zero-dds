//! DDS 1.4 §2.2.3: "Some QosPolicy values may not be consistent with other
//! ones … If the resulting QoS is inconsistent, the change of QoS operation
//! fails and the previous values are retained." `create_topic`,
//! `create_datawriter`, `create_datareader` and `set_qos` refuse a set whose
//! HISTORY, RESOURCE_LIMITS, DEADLINE and TIME_BASED_FILTER disagree
//! (§2.2.3.12, §2.2.3.18, §2.2.3.19).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_dcps::qos::{
    DeadlineQosPolicy, HistoryKind, HistoryQosPolicy, ResourceLimitsQosPolicy,
    TimeBasedFilterQosPolicy,
};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DomainParticipantFactory, DomainParticipantQos, Entity,
    PublisherQos, RawBytes, SubscriberQos, TopicQos,
};
use zerodds_qos::Duration as QosDuration;

fn keep_last(depth: i32) -> HistoryQosPolicy {
    HistoryQosPolicy {
        kind: HistoryKind::KeepLast,
        depth,
    }
}

fn limits(max_samples: i32, per_instance: i32) -> ResourceLimitsQosPolicy {
    ResourceLimitsQosPolicy {
        max_samples,
        max_instances: -1,
        max_samples_per_instance: per_instance,
    }
}

fn inconsistent<T: core::fmt::Debug>(r: Result<T, DdsError>) -> bool {
    matches!(r, Err(DdsError::InconsistentPolicy { .. }))
}

/// §2.2.3.18: "depth <= max_samples_per_instance"; §2.2.3.19: "max_samples
/// >= max_samples_per_instance".
#[test]
fn an_inconsistent_history_or_resource_limit_refuses_the_entity() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let deep = TopicQos {
        history: keep_last(5),
        resource_limits: limits(-1, 2),
        ..TopicQos::default()
    };
    assert!(inconsistent(p.create_topic::<RawBytes>("Deep", deep)));
    let topic = p
        .create_topic::<RawBytes>("Consistent", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let wq = DataWriterQos {
        resource_limits: limits(2, 3),
        ..DataWriterQos::default()
    };
    assert!(inconsistent(publisher.create_datawriter(&topic, wq)));
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let rq = DataReaderQos {
        history: keep_last(4),
        resource_limits: limits(-1, 3),
        ..DataReaderQos::default()
    };
    assert!(inconsistent(subscriber.create_datareader(&topic, rq)));
    let depth_zero = DataReaderQos {
        history: keep_last(0),
        ..DataReaderQos::default()
    };
    assert!(inconsistent(
        subscriber.create_datareader(&topic, depth_zero)
    ));
}

/// §2.2.3.12: "period >= minimum_separation" for DEADLINE and
/// TIME_BASED_FILTER of a DataReader.
#[test]
fn a_time_based_filter_longer_than_the_deadline_refuses_the_reader() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<RawBytes>("Filtered", TopicQos::default())
        .unwrap();
    let rq = DataReaderQos {
        deadline: DeadlineQosPolicy {
            period: QosDuration::from_millis(100),
        },
        time_based_filter: TimeBasedFilterQosPolicy {
            minimum_separation: QosDuration::from_millis(500),
        },
        ..DataReaderQos::default()
    };
    assert!(inconsistent(
        p.create_subscriber(SubscriberQos::default())
            .create_datareader(&topic, rq)
    ));
}

/// §2.2.3: an inconsistent `set_qos` "fails and the previous values are
/// retained".
#[test]
fn an_inconsistent_set_qos_keeps_the_previous_values() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<RawBytes>("Retained", TopicQos::default())
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter(&topic, DataWriterQos::default())
        .unwrap();
    let before = writer.get_qos();
    let bad = DataWriterQos {
        resource_limits: limits(1, 2),
        ..before.clone()
    };
    assert!(inconsistent(writer.set_qos(bad)));
    assert_eq!(writer.get_qos().resource_limits, before.resource_limits);
}
