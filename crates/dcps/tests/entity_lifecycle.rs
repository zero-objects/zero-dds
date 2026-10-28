//! Integration tests for the Entity trait + lifecycle.
//!
//! Spec refs: DDS-DCPS 1.4 §2.2.2.1 (Entity base), §2.2.3 (QoS
//! changeability table).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DomainParticipantFactory, DomainParticipantQos, Entity,
    PublisherQos, RawBytes, SubscriberQos, TopicQos,
};

fn make_participant() -> zerodds_dcps::DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default())
}

/// A participant whose ENTITY_FACTORY QoS switches off auto-enable, so
/// its children are created disabled (DDS-DCPS 1.4 §2.2.3.20).
fn make_manual_participant() -> zerodds_dcps::DomainParticipant {
    let mut qos = DomainParticipantQos::default();
    qos.entity_factory.autoenable_created_entities = false;
    DomainParticipantFactory::instance().create_participant_offline(0, qos)
}

/// §2.2.3.20: the `DomainParticipantFactory` carries
/// `autoenable_created_entities = TRUE` by default, so a participant
/// comes back from `create_participant` already enabled and the explicit
/// `enable()` is the idempotent no-op §2.2.2.1.1.7 promises.
#[test]
fn participant_is_enabled_by_the_factory_and_enable_is_idempotent() {
    let p = make_participant();
    assert!(p.is_enabled());
    p.enable().unwrap();
    assert!(p.is_enabled());
}

#[test]
fn participant_qos_get_set_roundtrip() {
    let p = make_participant();
    let qos = p.get_qos();
    p.set_qos(qos.clone()).unwrap();
    assert_eq!(p.get_qos(), qos);
}

#[test]
fn participant_get_instance_handle_unique() {
    let p1 = make_participant();
    let p2 = make_participant();
    assert_ne!(p1.get_instance_handle(), p2.get_instance_handle());
}

/// §2.2.3.20: "the factory `create_<entity>` operation will
/// automatically enable the created entity" when
/// `autoenable_created_entities` is TRUE — and leaves it disabled when
/// it is FALSE, until the application calls `enable()` itself.
#[test]
fn publisher_follows_the_participants_entity_factory_policy() {
    let p = make_participant();
    let pub_ = p.create_publisher(PublisherQos::default());
    assert!(pub_.is_enabled(), "autoenable defaults to TRUE");

    let manual = make_manual_participant();
    let pub2 = manual.create_publisher(PublisherQos::default());
    assert!(!pub2.is_enabled(), "autoenable = FALSE leaves it disabled");
    pub2.enable().unwrap();
    assert!(pub2.is_enabled());
}

#[test]
fn publisher_set_qos_pre_and_post_enable_works() {
    let p = make_participant();
    let pub_ = p.create_publisher(PublisherQos::default());
    pub_.set_qos(PublisherQos::default()).unwrap();
    pub_.enable().unwrap();
    // The same QoS again changes no policy, immutable or not.
    pub_.set_qos(PublisherQos::default()).unwrap();
}

/// DDS 1.4 §2.2.3: PRESENTATION is Changeable=NO on the Publisher and the
/// Subscriber; PARTITION and GROUP_DATA are changeable.
#[test]
fn publisher_and_subscriber_refuse_a_presentation_change_once_enabled() {
    use zerodds_qos::PresentationAccessScope;
    let p = make_participant();
    let pub_ = p.create_publisher(PublisherQos::default());
    let sub = p.create_subscriber(SubscriberQos::default());
    let mut pq = PublisherQos::default();
    pq.presentation.access_scope = PresentationAccessScope::Topic;
    assert!(matches!(
        pub_.set_qos(pq).unwrap_err(),
        DdsError::ImmutablePolicy {
            policy: "PRESENTATION"
        }
    ));
    let mut sq = SubscriberQos::default();
    sq.presentation.access_scope = PresentationAccessScope::Topic;
    assert!(matches!(
        sub.set_qos(sq).unwrap_err(),
        DdsError::ImmutablePolicy {
            policy: "PRESENTATION"
        }
    ));
    let mut pq = PublisherQos::default();
    pq.partition.names = vec!["A".into()];
    pq.group_data.value = vec![1];
    pub_.set_qos(pq.clone()).unwrap();
    assert_eq!(pub_.get_qos(), pq);
}

/// DDS 1.4 §2.2.3: DESTINATION_ORDER is Changeable=NO on a DataWriter and
/// a DataReader; DEADLINE and LATENCY_BUDGET are changeable.
#[test]
fn endpoints_refuse_a_destination_order_change_once_enabled() {
    use zerodds_qos::DestinationOrderKind;
    let p = make_participant();
    let topic = p
        .create_topic::<RawBytes>("Chatter", TopicQos::default())
        .unwrap();
    let dw = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let dr = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    let mut wq = dw.get_qos();
    wq.destination_order.kind = DestinationOrderKind::BySourceTimestamp;
    assert!(matches!(
        dw.set_qos(wq).unwrap_err(),
        DdsError::ImmutablePolicy {
            policy: "DESTINATION_ORDER"
        }
    ));
    let mut rq = dr.get_qos();
    rq.destination_order.kind = DestinationOrderKind::BySourceTimestamp;
    assert!(matches!(
        dr.set_qos(rq).unwrap_err(),
        DdsError::ImmutablePolicy {
            policy: "DESTINATION_ORDER"
        }
    ));
    let mut wq = dw.get_qos();
    wq.deadline.period = zerodds_qos::Duration::from_millis(200);
    dw.set_qos(wq.clone()).unwrap();
    assert_eq!(dw.get_qos(), wq);
}

/// DDS 1.4 §2.2.3 for the Topic: HISTORY is Changeable=NO, DEADLINE and
/// TOPIC_DATA are changeable.
#[test]
fn topic_refuses_a_history_change_and_takes_a_deadline_change_once_enabled() {
    let p = make_participant();
    let topic = p
        .create_topic::<RawBytes>("Chatter", TopicQos::default())
        .unwrap();
    let mut q = topic.get_qos();
    q.history.depth += 1;
    assert!(matches!(
        topic.set_qos(q).unwrap_err(),
        DdsError::ImmutablePolicy { policy: "HISTORY" }
    ));
    let mut q = topic.get_qos();
    q.deadline.period = zerodds_qos::Duration::from_millis(200);
    q.topic_data.value = vec![4, 2];
    topic.set_qos(q.clone()).unwrap();
    assert_eq!(topic.get_qos(), q);
}

#[test]
fn subscriber_entity_lifecycle() {
    let p = make_participant();
    let sub = p.create_subscriber(SubscriberQos::default());
    assert!(sub.is_enabled(), "autoenable defaults to TRUE");
    sub.set_qos(SubscriberQos::default()).unwrap();

    let manual = make_manual_participant();
    let sub2 = manual.create_subscriber(SubscriberQos::default());
    assert!(!sub2.is_enabled());
    sub2.enable().unwrap();
    assert!(sub2.is_enabled());
    sub2.set_qos(SubscriberQos::default()).unwrap();
}

#[test]
fn topic_immutable_durability_post_enable() {
    use zerodds_qos::{DurabilityKind, DurabilityQosPolicy};
    let p = make_participant();
    let q = TopicQos {
        durability: DurabilityQosPolicy {
            kind: DurabilityKind::Volatile,
        },
        ..TopicQos::default()
    };
    let topic = p.create_topic::<RawBytes>("Chatter", q.clone()).unwrap();
    topic.enable().unwrap();
    // Durability must not be changed post-enable
    let mut q2 = q.clone();
    q2.durability = DurabilityQosPolicy {
        kind: DurabilityKind::TransientLocal,
    };
    let err = topic.set_qos(q2).unwrap_err();
    assert!(matches!(
        err,
        DdsError::ImmutablePolicy {
            policy: "DURABILITY"
        }
    ));
}

#[test]
fn topic_set_qos_pre_enable_allows_durability_change() {
    use zerodds_qos::{DurabilityKind, DurabilityQosPolicy};
    // Pre-enable is only reachable with auto-enable switched off
    // (§2.2.3.20); with the default TRUE the topic is enabled on
    // creation and DURABILITY is already immutable.
    let p = make_manual_participant();
    let topic = p
        .create_topic::<RawBytes>("Chatter", TopicQos::default())
        .unwrap();
    // pre-enable: all fields allowed
    let q = TopicQos {
        durability: DurabilityQosPolicy {
            kind: DurabilityKind::TransientLocal,
        },
        ..TopicQos::default()
    };
    topic.set_qos(q).unwrap();
    assert_eq!(topic.qos().durability.kind, DurabilityKind::TransientLocal);
}

/// §2.2.3.20 down the whole factory chain: the participant's
/// ENTITY_FACTORY governs publisher and subscriber, and each of those
/// governs its own writers and readers. With auto-enable off nothing in
/// the chain comes back enabled.
#[test]
fn datawriter_and_datareader_follow_their_factories_entity_factory_policy() {
    let p = make_participant();
    let topic = p
        .create_topic::<RawBytes>("Chatter", TopicQos::default())
        .unwrap();
    let pub_ = p.create_publisher(PublisherQos::default());
    let sub = p.create_subscriber(SubscriberQos::default());
    let dw = pub_
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let dr = sub
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    assert!(dw.is_enabled(), "autoenable defaults to TRUE");
    assert!(dr.is_enabled(), "autoenable defaults to TRUE");

    let manual = make_manual_participant();
    let topic2 = manual
        .create_topic::<RawBytes>("Chatter", TopicQos::default())
        .unwrap();
    let pub2 = manual.create_publisher(PublisherQos::default());
    let sub2 = manual.create_subscriber(SubscriberQos::default());
    assert!(!topic2.is_enabled());
    let dw2 = pub2
        .create_datawriter::<RawBytes>(&topic2, DataWriterQos::default())
        .unwrap();
    let dr2 = sub2
        .create_datareader::<RawBytes>(&topic2, DataReaderQos::default())
        .unwrap();
    assert!(
        !dw2.is_enabled(),
        "a disabled publisher creates disabled writers"
    );
    assert!(
        !dr2.is_enabled(),
        "a disabled subscriber creates disabled readers"
    );
    // §2.2.2.1.1.7: "Calling enable on an Entity whose factory is not
    // enabled will fail and return PRECONDITION_NOT_MET."
    assert!(matches!(
        dw2.enable().unwrap_err(),
        DdsError::PreconditionNotMet { .. }
    ));
    assert!(matches!(
        dr2.enable().unwrap_err(),
        DdsError::PreconditionNotMet { .. }
    ));
    pub2.enable().unwrap();
    sub2.enable().unwrap();
    dw2.enable().unwrap();
    dr2.enable().unwrap();
    assert!(dw2.is_enabled());
    assert!(dr2.is_enabled());
}

/// §2.2.2.1.1.7: "If the autoenable_created_entities field of
/// ENTITY_FACTORY is set to TRUE, the enable operation on the factory will
/// automatically enable all entities created from the factory."
#[test]
fn enabling_a_factory_enables_what_it_created_when_its_entity_factory_says_so() {
    let manual = make_manual_participant();
    let topic = manual
        .create_topic::<RawBytes>("Chatter", TopicQos::default())
        .unwrap();
    // A disabled publisher with autoenable TRUE creates a disabled writer;
    // the publisher's enable enables it.
    let pub_ = manual.create_publisher(PublisherQos::default());
    let dw = pub_
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    assert!(!dw.is_enabled());
    pub_.enable().unwrap();
    assert!(dw.is_enabled());

    // With autoenable FALSE the subscriber's enable leaves its reader as it is.
    let mut sq = SubscriberQos::default();
    sq.entity_factory.autoenable_created_entities = false;
    let sub = manual.create_subscriber(sq);
    sub.enable().unwrap();
    let dr = sub
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    assert!(!dr.is_enabled());
    sub.enable().unwrap();
    assert!(!dr.is_enabled());
}

/// §2.2.2.1.1.7: operations a disabled entity is not listed for return
/// NOT_ENABLED; get_qos and set_qos are listed.
#[test]
fn a_disabled_writer_and_reader_refuse_data_operations() {
    let manual = make_manual_participant();
    let topic = manual
        .create_topic::<RawBytes>("Chatter", TopicQos::default())
        .unwrap();
    let dw = manual
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let dr = manual
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    assert!(matches!(
        dw.write(&RawBytes::new(vec![1])).unwrap_err(),
        DdsError::NotEnabled
    ));
    assert!(matches!(dr.take().unwrap_err(), DdsError::NotEnabled));
    assert!(matches!(dr.read().unwrap_err(), DdsError::NotEnabled));
    let mut q = dw.get_qos();
    q.deadline.period = zerodds_qos::Duration::from_millis(100);
    dw.set_qos(q).unwrap();
}

#[test]
fn datawriter_immutable_reliability_post_enable() {
    use zerodds_qos::{ReliabilityKind, ReliabilityQosPolicy};
    let p = make_participant();
    let topic = p
        .create_topic::<RawBytes>("Chatter", TopicQos::default())
        .unwrap();
    let pub_ = p.create_publisher(PublisherQos::default());
    let dw = pub_
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    dw.enable().unwrap();
    let mut q = dw.get_qos();
    let opposite = match q.reliability.kind {
        ReliabilityKind::Reliable => ReliabilityKind::BestEffort,
        ReliabilityKind::BestEffort => ReliabilityKind::Reliable,
    };
    q.reliability = ReliabilityQosPolicy {
        kind: opposite,
        max_blocking_time: q.reliability.max_blocking_time,
    };
    let err = dw.set_qos(q).unwrap_err();
    assert!(matches!(
        err,
        DdsError::ImmutablePolicy {
            policy: "RELIABILITY"
        }
    ));
}

#[test]
fn datareader_status_changes_track() {
    let p = make_participant();
    let topic = p
        .create_topic::<RawBytes>("Chatter", TopicQos::default())
        .unwrap();
    let sub = p.create_subscriber(SubscriberQos::default());
    let dr = sub
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    assert_eq!(dr.get_status_changes(), 0);
    dr.entity_state().set_status_bits(0b0010);
    assert_eq!(dr.get_status_changes(), 0b0010);
    dr.entity_state().clear_status_changes(0b0010);
    assert_eq!(dr.get_status_changes(), 0);
}

#[test]
fn status_condition_trigger_on_relevant_status() {
    let p = make_participant();
    let cond = p.get_status_condition();
    cond.set_enabled_statuses(0b0001);
    assert!(!cond.trigger_value());
    p.entity_state().set_status_bits(0b0001);
    assert!(cond.trigger_value());
}

#[test]
fn entity_handles_unique_across_types() {
    let p = make_participant();
    let pub_ = p.create_publisher(PublisherQos::default());
    let sub = p.create_subscriber(SubscriberQos::default());
    let topic = p
        .create_topic::<RawBytes>("Chatter", TopicQos::default())
        .unwrap();
    let h_p = p.get_instance_handle();
    let h_pub = pub_.get_instance_handle();
    let h_sub = sub.get_instance_handle();
    let h_topic = topic.get_instance_handle();
    let mut seen = std::collections::HashSet::new();
    assert!(seen.insert(h_p));
    assert!(seen.insert(h_pub));
    assert!(seen.insert(h_sub));
    assert!(seen.insert(h_topic));
}

#[test]
fn topic_qos_get_returns_clone() {
    let p = make_participant();
    let topic = p
        .create_topic::<RawBytes>("Chatter", TopicQos::default())
        .unwrap();
    let q1 = topic.qos();
    let q2 = topic.qos();
    assert_eq!(q1, q2);
}

/// DDS 1.4 §2.2.2.1.1.7: on an entity not yet enabled, operations other
/// than QoS, listener, get_statuscondition, factory, get-status and lookup
/// ones "will return the error NOT_ENABLED". A disabled Publisher's
/// suspend/resume and coherent changes, a disabled Subscriber's
/// begin_access, end_access, get_datareaders and notify_datareaders return
/// it, and the same calls go through once the entities are enabled. They
/// ran on the disabled entities.
#[test]
fn group_operations_of_a_disabled_publisher_or_subscriber_are_not_enabled() {
    use zerodds_dcps::error::DdsError;
    let p = make_manual_participant();
    let publisher = p.create_publisher(PublisherQos::default());
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let not_enabled = |r: zerodds_dcps::Result<()>| matches!(r, Err(DdsError::NotEnabled));
    assert!(not_enabled(publisher.suspend_publications()));
    assert!(not_enabled(publisher.resume_publications()));
    assert!(not_enabled(publisher.begin_coherent_changes()));
    assert!(not_enabled(publisher.end_coherent_changes()));
    assert!(not_enabled(subscriber.begin_access()));
    assert!(not_enabled(subscriber.end_access()));
    assert!(not_enabled(subscriber.notify_datareaders()));
    assert!(matches!(
        subscriber.get_datareaders(0xffff, 0xffff, 0xffff),
        Err(DdsError::NotEnabled)
    ));

    p.enable().unwrap();
    publisher.enable().unwrap();
    subscriber.enable().unwrap();
    publisher.suspend_publications().unwrap();
    publisher.resume_publications().unwrap();
    publisher.begin_coherent_changes().unwrap();
    publisher.end_coherent_changes().unwrap();
    subscriber.begin_access().unwrap();
    subscriber.end_access().unwrap();
    subscriber.notify_datareaders().unwrap();
    assert!(subscriber.get_datareaders(0xffff, 0xffff, 0xffff).is_ok());
}
