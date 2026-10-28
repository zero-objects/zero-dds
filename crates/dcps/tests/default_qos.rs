// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The default QoS of the factories and the placeholders `*_QOS_DEFAULT`
//! and `*_QOS_USE_TOPIC_QOS` (DDS 1.4 §2.2.2.1.1.1, §2.2.2.2.1.20–25,
//! §2.2.2.2.2.1, §2.2.2.2.2.5, §2.2.2.4.1.5, §2.2.2.4.1.15–16,
//! §2.2.2.5.2.5, §2.2.2.5.2.15–16, §2.3.3), on offline participants.
//!
//! The participant factory is one per process: only
//! `the_participant_factory_default` changes its default, and no other
//! test here creates a participant with `PARTICIPANT_QOS_DEFAULT`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_dcps::qos::{
    DurabilityKind, HistoryKind, HistoryQosPolicy, LifespanQosPolicy, OwnershipStrengthQosPolicy,
    PartitionQosPolicy, PresentationAccessScope, ReliabilityKind, ResourceLimitsQosPolicy,
};
use zerodds_dcps::{
    DATAREADER_QOS_DEFAULT, DATAREADER_QOS_USE_TOPIC_QOS, DATAWRITER_QOS_DEFAULT,
    DATAWRITER_QOS_USE_TOPIC_QOS, DataReaderQos, DataWriterQos, DdsError, DomainParticipant,
    DomainParticipantFactory, DomainParticipantQos, Entity, PARTICIPANT_QOS_DEFAULT,
    PUBLISHER_QOS_DEFAULT, PublisherQos, RawBytes, SUBSCRIBER_QOS_DEFAULT, SubscriberQos,
    THE_PARTICIPANT_FACTORY, TOPIC_QOS_DEFAULT, TopicQos,
};
use zerodds_qos::Duration;

fn offline() -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default())
}

fn partition(name: &str) -> PartitionQosPolicy {
    PartitionQosPolicy {
        names: vec![name.into()],
    }
}

/// §2.2.2.2.2.3: "TheParticipantFactory can also be used as an alias for
/// the singleton factory returned by the operation get_instance."
#[test]
fn the_participant_factory_is_the_instance() {
    assert!(core::ptr::eq(
        &*THE_PARTICIPANT_FACTORY,
        DomainParticipantFactory::instance()
    ));
}

/// §2.2.2.2.2.1, §2.2.2.2.2.5: `create_participant` with
/// PARTICIPANT_QOS_DEFAULT takes the factory's default; a QoS given as a
/// value is taken as it is; PARTICIPANT_QOS_DEFAULT passed to
/// `set_default_participant_qos` resets the default. `set_qos` with
/// PARTICIPANT_QOS_DEFAULT takes the factory's current default
/// (§2.2.2.1.1.1). The stored default was read by no `create_participant*`.
#[test]
fn the_participant_factory_default() {
    let factory = DomainParticipantFactory::instance();
    let mut qos = DomainParticipantQos::default();
    qos.user_data.value = vec![1, 2, 3];
    factory.set_default_participant_qos(qos.clone()).unwrap();

    let defaulted = factory.create_participant_offline(0, PARTICIPANT_QOS_DEFAULT);
    assert_eq!(defaulted.get_qos().user_data.value, vec![1, 2, 3]);
    let given = factory.create_participant_offline(0, DomainParticipantQos::default());
    assert!(given.get_qos().user_data.value.is_empty());

    given.set_qos(PARTICIPANT_QOS_DEFAULT).unwrap();
    assert_eq!(given.get_qos().user_data.value, vec![1, 2, 3]);

    factory
        .set_default_participant_qos(PARTICIPANT_QOS_DEFAULT)
        .unwrap();
    assert_eq!(
        factory.get_default_participant_qos(),
        DomainParticipantQos::default()
    );
    let after_reset = factory.create_participant_offline(0, PARTICIPANT_QOS_DEFAULT);
    assert!(after_reset.get_qos().user_data.value.is_empty());
}

/// §2.2.2.2.1.20–23: the default of `create_publisher` and
/// `create_subscriber`. A publisher created before the change keeps its
/// QoS. There were no such operations.
#[test]
fn publisher_and_subscriber_defaults() {
    let p = offline();
    let before = p.create_publisher(PUBLISHER_QOS_DEFAULT);

    let pub_qos = PublisherQos {
        partition: partition("A"),
        ..PublisherQos::default()
    };
    p.set_default_publisher_qos(pub_qos.clone()).unwrap();
    assert_eq!(p.get_default_publisher_qos(), pub_qos);
    assert_eq!(p.create_publisher(PUBLISHER_QOS_DEFAULT).get_qos(), pub_qos);
    assert_eq!(
        p.create_publisher(PublisherQos::default()).get_qos(),
        PublisherQos::default()
    );
    assert_eq!(before.get_qos(), PublisherQos::default());

    let sub_qos = SubscriberQos {
        partition: partition("B"),
        ..SubscriberQos::default()
    };
    p.set_default_subscriber_qos(sub_qos.clone()).unwrap();
    assert_eq!(
        p.create_subscriber(SUBSCRIBER_QOS_DEFAULT).get_qos(),
        sub_qos
    );

    p.set_default_publisher_qos(PUBLISHER_QOS_DEFAULT).unwrap();
    assert_eq!(p.get_default_publisher_qos(), PublisherQos::default());
}

/// §2.2.2.2.1.24: "This operation will check that the resulting policies
/// are self consistent; if they are not, the operation will have no effect
/// and return INCONSISTENT_POLICY." `create_topic` with TOPIC_QOS_DEFAULT
/// takes the default.
#[test]
fn topic_default_is_checked_and_used() {
    let p = offline();
    let mut good = TopicQos::default();
    good.durability.kind = DurabilityKind::TransientLocal;
    p.set_default_topic_qos(good.clone()).unwrap();

    let bad = TopicQos {
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 10,
        },
        resource_limits: ResourceLimitsQosPolicy {
            max_samples_per_instance: 5,
            ..ResourceLimitsQosPolicy::default()
        },
        ..TopicQos::default()
    };
    assert!(matches!(
        p.set_default_topic_qos(bad),
        Err(DdsError::InconsistentPolicy { .. })
    ));
    assert_eq!(
        p.get_default_topic_qos(),
        good,
        "the refused default stayed"
    );

    let topic = p
        .create_topic::<RawBytes>("DefaultTopicQos", TOPIC_QOS_DEFAULT)
        .unwrap();
    assert_eq!(
        topic.get_qos().durability.kind,
        DurabilityKind::TransientLocal
    );
}

/// §2.2.2.4.1.5, §2.2.2.4.1.15: DATAWRITER_QOS_DEFAULT takes the
/// publisher's default; DATAWRITER_QOS_USE_TOPIC_QOS takes that default
/// with the Topic QoS copied over it, so a writer-only policy of the
/// default stays and a policy the topic carries comes from the topic.
#[test]
fn datawriter_default_and_use_topic_qos() {
    let p = offline();
    let mut topic_qos = TopicQos::default();
    topic_qos.reliability.kind = ReliabilityKind::BestEffort;
    topic_qos.lifespan = LifespanQosPolicy {
        duration: Duration::from_secs(3),
    };
    let topic = p
        .create_topic::<RawBytes>("UseTopicQosW", topic_qos)
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());

    let default = DataWriterQos {
        ownership_strength: OwnershipStrengthQosPolicy { value: 7 },
        ..DataWriterQos::default()
    };
    publisher
        .set_default_datawriter_qos(default.clone())
        .unwrap();
    assert_eq!(publisher.get_default_datawriter_qos(), default);

    let defaulted = publisher
        .create_datawriter(&topic, DATAWRITER_QOS_DEFAULT)
        .unwrap();
    assert_eq!(defaulted.get_qos(), default);

    let combined = publisher
        .create_datawriter(&topic, DATAWRITER_QOS_USE_TOPIC_QOS)
        .unwrap()
        .get_qos();
    assert_eq!(combined.ownership_strength.value, 7);
    assert_eq!(combined.reliability.kind, ReliabilityKind::BestEffort);
    assert_eq!(combined.lifespan.duration, Duration::from_secs(3));

    // §2.2.2.1.1.1: set_qos with DATAWRITER_QOS_DEFAULT takes the
    // publisher's current default.
    let mut changed = default.clone();
    changed.ownership_strength = OwnershipStrengthQosPolicy { value: 9 };
    publisher.set_default_datawriter_qos(changed).unwrap();
    defaulted.set_qos(DATAWRITER_QOS_DEFAULT).unwrap();
    assert_eq!(defaulted.get_qos().ownership_strength.value, 9);

    publisher
        .set_default_datawriter_qos(DATAWRITER_QOS_DEFAULT)
        .unwrap();
    assert_eq!(
        publisher.get_default_datawriter_qos(),
        DataWriterQos::default()
    );
}

/// The reader half: DATAREADER_QOS_DEFAULT, DATAREADER_QOS_USE_TOPIC_QOS,
/// also on a ContentFilteredTopic, whose "Topic in question is the
/// ContentFilteredTopic's 'related Topic'" (§2.2.2.5.2.5).
#[test]
fn datareader_default_and_use_topic_qos() {
    let p = offline();
    let mut topic_qos = TopicQos::default();
    topic_qos.durability.kind = DurabilityKind::TransientLocal;
    let topic = p
        .create_topic::<RawBytes>("UseTopicQosR", topic_qos)
        .unwrap();
    let subscriber = p.create_subscriber(SubscriberQos::default());

    let default = DataReaderQos {
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 4,
        },
        ..DataReaderQos::default()
    };
    subscriber
        .set_default_datareader_qos(default.clone())
        .unwrap();
    let defaulted = subscriber
        .create_datareader(&topic, DATAREADER_QOS_DEFAULT)
        .unwrap();
    assert_eq!(defaulted.get_qos(), default);

    let combined = subscriber
        .create_datareader(&topic, DATAREADER_QOS_USE_TOPIC_QOS)
        .unwrap()
        .get_qos();
    assert_eq!(combined.durability.kind, DurabilityKind::TransientLocal);
    // HISTORY is a policy the Topic QoS carries: the topic's KEEP_LAST 1
    // overrides the default's depth 4.
    assert_eq!(combined.history, TopicQos::default().history);

    let cft = p
        .create_contentfilteredtopic::<RawBytes>("UseTopicQosCft", &topic, "x = 1", Vec::new())
        .unwrap();
    let filtered = subscriber
        .create_datareader_with_cft(&cft, DATAREADER_QOS_USE_TOPIC_QOS)
        .unwrap()
        .get_qos();
    assert_eq!(filtered.durability.kind, DurabilityKind::TransientLocal);

    subscriber
        .set_default_datareader_qos(DATAREADER_QOS_DEFAULT)
        .unwrap();
    assert_eq!(
        subscriber.get_default_datareader_qos(),
        DataReaderQos::default()
    );
}

/// §2.2.2.1.1.1: `set_qos` with `*_QOS_DEFAULT` "cannot modify the
/// immutable QoS": once enabled, a default whose PRESENTATION differs is
/// refused as that QoS given as a value would be, and nothing changes.
#[test]
fn set_qos_with_the_default_keeps_the_immutable_rule() {
    let p = offline();
    let publisher = p.create_publisher(PublisherQos::default());
    publisher.enable().unwrap();
    let mut default = PublisherQos::default();
    default.presentation.access_scope = PresentationAccessScope::Topic;
    default.partition = partition("C");
    p.set_default_publisher_qos(default).unwrap();
    assert!(matches!(
        publisher.set_qos(PUBLISHER_QOS_DEFAULT),
        Err(DdsError::ImmutablePolicy { .. })
    ));
    assert_eq!(publisher.get_qos(), PublisherQos::default());

    let mutable_only = PublisherQos {
        partition: partition("C"),
        ..PublisherQos::default()
    };
    p.set_default_publisher_qos(mutable_only.clone()).unwrap();
    publisher.set_qos(PUBLISHER_QOS_DEFAULT).unwrap();
    assert_eq!(publisher.get_qos(), mutable_only);
}

/// §2.2.2.1.1.1 on a Topic, a Subscriber and a DataReader: `set_qos` with
/// `*_QOS_DEFAULT` takes the current default of the participant or the
/// subscriber.
#[test]
fn set_qos_with_the_default_on_topic_subscriber_and_reader() {
    let p = offline();
    let topic = p
        .create_topic::<RawBytes>("SetQosDefault", TopicQos::default())
        .unwrap();
    let mut topic_default = TopicQos::default();
    topic_default.transport_priority.value = 5;
    p.set_default_topic_qos(topic_default.clone()).unwrap();
    topic.set_qos(TOPIC_QOS_DEFAULT).unwrap();
    assert_eq!(topic.get_qos(), topic_default);

    let subscriber = p.create_subscriber(SubscriberQos::default());
    let sub_default = SubscriberQos {
        partition: partition("D"),
        ..SubscriberQos::default()
    };
    p.set_default_subscriber_qos(sub_default.clone()).unwrap();
    subscriber.set_qos(SUBSCRIBER_QOS_DEFAULT).unwrap();
    assert_eq!(subscriber.get_qos(), sub_default);

    let reader = subscriber
        .create_datareader(&topic, DataReaderQos::default())
        .unwrap();
    let mut reader_default = DataReaderQos::default();
    reader_default.latency_budget.duration = Duration::from_millis(20);
    subscriber
        .set_default_datareader_qos(reader_default.clone())
        .unwrap();
    reader.set_qos(DATAREADER_QOS_DEFAULT).unwrap();
    assert_eq!(reader.get_qos(), reader_default);
}

/// §2.2.2.4.1.13 and §2.2.2.5.2.13 `get_participant`.
#[test]
fn publisher_and_subscriber_know_their_participant() {
    let p = offline();
    let publisher = p.create_publisher(PublisherQos::default());
    let subscriber = p.create_subscriber(SubscriberQos::default());
    assert_eq!(
        publisher.get_participant().map(|q| q.instance_handle()),
        Some(p.instance_handle())
    );
    assert_eq!(
        subscriber.get_participant().map(|q| q.instance_handle()),
        Some(p.instance_handle())
    );
}
