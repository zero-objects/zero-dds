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

#[test]
fn participant_starts_disabled_until_enable() {
    let p = make_participant();
    assert!(!p.is_enabled());
    p.enable().unwrap();
    assert!(p.is_enabled());
    // idempotent
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

#[test]
fn publisher_starts_disabled() {
    let p = make_participant();
    let pub_ = p.create_publisher(PublisherQos::default());
    assert!(!pub_.is_enabled());
    pub_.enable().unwrap();
    assert!(pub_.is_enabled());
}

#[test]
fn publisher_set_qos_pre_and_post_enable_works() {
    let p = make_participant();
    let pub_ = p.create_publisher(PublisherQos::default());
    pub_.set_qos(PublisherQos::default()).unwrap();
    pub_.enable().unwrap();
    // PublisherQos has no immutable fields per spec → set_qos
    // post-enable stays successful.
    pub_.set_qos(PublisherQos::default()).unwrap();
}

#[test]
fn subscriber_entity_lifecycle() {
    let p = make_participant();
    let sub = p.create_subscriber(SubscriberQos::default());
    assert!(!sub.is_enabled());
    sub.enable().unwrap();
    assert!(sub.is_enabled());
    sub.set_qos(SubscriberQos::default()).unwrap();
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
    let p = make_participant();
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
