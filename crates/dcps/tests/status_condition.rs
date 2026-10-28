// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `StatusCondition` (DDS 1.4 §2.2.2.1.9), `ReadCondition::get_datareader`
//! (§2.2.2.5.8.1) and `WaitSet::detach_condition` (§2.2.2.1.6.2).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::sync::Arc;

use zerodds_dcps::entity::EntityRef;
use zerodds_dcps::psm_constants::status;
use zerodds_dcps::{
    Condition, DataReaderQos, DataWriterQos, DdsError, DomainParticipant, DomainParticipantFactory,
    DomainParticipantQos, Entity, PublisherQos, RawBytes, SubscriberQos, TopicQos, WaitSet,
    instance_state_mask, sample_state_mask, view_state_mask,
};

fn offline() -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default())
}

/// §2.2.2.1.9: "A StatusCondition object is a specific Condition that is
/// associated with each Entity": every call returns that one, so the
/// `enabled_statuses` of §2.2.2.1.9.1 set through one is the entity's.
#[test]
fn an_entity_has_one_status_condition() {
    let p = offline();
    let topic = p
        .create_topic::<RawBytes>("OneCondition", TopicQos::default())
        .unwrap();
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    let first = reader.get_status_condition();
    let second = reader.get_status_condition();
    assert!(Arc::ptr_eq(&first, &second));
    first.set_enabled_statuses(status::DATA_AVAILABLE);
    assert_eq!(second.enabled_statuses(), status::DATA_AVAILABLE);
    assert!(!Arc::ptr_eq(&first, &p.get_status_condition()));
}

/// §2.2.2.1.9.3: `get_entity` returns the Entity the condition belongs to,
/// of every kind.
#[test]
fn get_entity_returns_the_entity_of_each_kind() {
    let p = offline();
    let topic = p
        .create_topic::<RawBytes>("GetEntity", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();

    let Some(EntityRef::DomainParticipant(found)) = p.get_status_condition().get_entity() else {
        panic!("the participant");
    };
    assert_eq!(found.get_instance_handle(), p.get_instance_handle());
    let Some(EntityRef::Publisher(found)) = publisher.get_status_condition().get_entity() else {
        panic!("the publisher");
    };
    assert_eq!(found.get_instance_handle(), publisher.get_instance_handle());
    let Some(EntityRef::Subscriber(found)) = subscriber.get_status_condition().get_entity() else {
        panic!("the subscriber");
    };
    assert_eq!(
        found.get_instance_handle(),
        subscriber.get_instance_handle()
    );
    let Some(EntityRef::Topic(found)) = topic.get_status_condition().get_entity() else {
        panic!("the topic");
    };
    assert_eq!(
        zerodds_dcps::TopicDescription::get_name(&found),
        "GetEntity"
    );
    let Some(EntityRef::DataWriter(found)) = writer.get_status_condition().get_entity() else {
        panic!("the writer");
    };
    assert_eq!(found.instance_handle(), writer.get_instance_handle());
    assert!(found.typed::<RawBytes>().is_some());
    let Some(EntityRef::DataReader(found)) = reader.get_status_condition().get_entity() else {
        panic!("the reader");
    };
    assert_eq!(found.instance_handle(), reader.get_instance_handle());
    assert!(found.typed::<RawBytes>().is_some());
}

/// The condition does not keep its entity alive: once the last handle of the
/// entity is gone, `get_entity` is `None`, and the handle stays readable.
#[test]
fn get_entity_is_none_once_the_entity_is_gone() {
    let p = offline();
    let topic = p
        .create_topic::<RawBytes>("EntityGone", TopicQos::default())
        .unwrap();
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    let condition = reader.get_status_condition();
    let handle = reader.get_instance_handle();
    assert!(condition.get_entity().is_some());
    drop(reader);
    assert!(condition.get_entity().is_none());
    assert_eq!(condition.get_entity_handle(), handle);
    assert!(!condition.trigger_value());
}

/// §2.2.2.5.8.1: `get_datareader` returns the DataReader the condition was
/// created on; a QueryCondition is a ReadCondition.
#[test]
fn a_read_condition_names_its_reader() {
    let p = offline();
    let topic = p
        .create_topic::<RawBytes>("ConditionReader", TopicQos::default())
        .unwrap();
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    let read = reader
        .create_readcondition(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap();
    assert_eq!(
        read.get_datareader().unwrap().instance_handle(),
        reader.get_instance_handle()
    );
    let query = reader
        .create_querycondition(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
            "id > 0",
            Vec::new(),
        )
        .unwrap();
    assert_eq!(
        query.base().get_datareader().unwrap().instance_handle(),
        reader.get_instance_handle()
    );
    // A condition nobody made on a reader has none.
    let loose = zerodds_dcps::ReadCondition::new(0, 0, 0, |_, _, _| false);
    assert!(loose.get_datareader().is_none());
}

/// §2.2.2.1.6.2: detaching a condition that is not attached is
/// PRECONDITION_NOT_MET; the StatusCondition of an entity detaches by the
/// object it was attached as.
#[test]
fn detach_condition_wants_an_attached_condition() {
    let p = offline();
    let waitset = WaitSet::new();
    let attached: Arc<dyn Condition> = p.get_status_condition();
    waitset.attach_condition(Arc::clone(&attached)).unwrap();

    let other: Arc<dyn Condition> = zerodds_dcps::GuardCondition::new();
    assert!(matches!(
        waitset.detach_condition(&other),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    // The condition the entity gives again is the attached one.
    let again: Arc<dyn Condition> = p.get_status_condition();
    waitset.detach_condition(&again).unwrap();
    assert!(matches!(
        waitset.detach_condition(&attached),
        Err(DdsError::PreconditionNotMet { .. })
    ));
}
