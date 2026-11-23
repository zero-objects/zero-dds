// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What DDS 1.4 §2.2.2.1.1.7 and §2.2.2.2.1 say about an entity that is not
//! enabled: the operations that stay legal, NOT_ENABLED for the others, no
//! statuses, no listener calls, inactive conditions, and that such an entity
//! can be deleted; that a change of the factory's ENTITY_FACTORY
//! (§2.2.3.20) reaches only the participants created after it; and that
//! the `enable` of a participant created disabled follows the
//! ENTITY_FACTORY it has then. Every test switches the process's factory to
//! `autoenable_created_entities = FALSE` and back, one at a time.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use zerodds_dcps::factory::DomainParticipantFactoryQos;
use zerodds_dcps::listener::DataReaderListener;
use zerodds_dcps::psm_constants::status;
use zerodds_dcps::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DomainParticipantFactory, DomainParticipantQos, Entity,
    HANDLE_NIL, InstanceHandle, PublisherQos, RawBytes, SubscriberQos, TopicQos,
};

/// The factory's QoS is the process's: each test that changes it holds
/// this lock.
static FACTORY_QOS: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Default)]
struct Heard(AtomicU32);

impl DataReaderListener for Heard {
    fn on_data_available(&self, _reader: InstanceHandle) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn dcps_2_2_2_1_1_6_a_disabled_entity_is_inert_and_can_be_deleted() {
    let _serial = FACTORY_QOS.lock();
    let factory = DomainParticipantFactory::instance();
    let before = factory.get_qos();
    factory
        .set_qos(DomainParticipantFactoryQos {
            autoenable_created_entities: false,
        })
        .unwrap();
    let p = factory.create_participant_offline(0, DomainParticipantQos::default());
    factory.set_qos(before).unwrap();
    assert!(!p.is_enabled(), "the factory did not enable it");

    // §2.2.2.2.1: the operations that may be called on a disabled participant.
    let _ = p.get_qos();
    p.set_qos(p.get_qos()).unwrap();
    p.set_listener(None, status::NONE);
    assert!(p.get_listener().is_none());
    let _ = p.get_status_condition();
    p.set_default_topic_qos(TopicQos::default()).unwrap();
    assert!(p.lookup_topicdescription("Nothing").is_none());
    let topic = p
        .create_topic::<RawBytes>("DisabledTopic", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let subscriber = p.create_subscriber(SubscriberQos::default());
    assert!(!publisher.is_enabled());
    assert!(!subscriber.is_enabled());

    // The others are NOT_ENABLED.
    assert!(matches!(p.assert_liveliness(), Err(DdsError::NotEnabled)));
    assert!(matches!(
        p.try_get_discovered_participants(),
        Err(DdsError::NotEnabled)
    ));
    assert!(matches!(
        p.try_get_discovered_topics(),
        Err(DdsError::NotEnabled)
    ));
    assert!(matches!(
        p.try_contains_entity(HANDLE_NIL),
        Err(DdsError::NotEnabled)
    ));
    assert!(matches!(
        p.get_discovered_participant_data(HANDLE_NIL),
        Err(DdsError::NotEnabled)
    ));
    assert!(matches!(
        p.get_discovered_topic_data(HANDLE_NIL),
        Err(DdsError::NotEnabled)
    ));
    assert!(matches!(p.get_current_time(), Err(DdsError::NotEnabled)));

    // §2.2.2.1.1.6: no status is triggered.
    assert_eq!(p.get_status_changes(), 0);

    // §2.2.2.1.1.7: "It is legal to delete an Entity that has not been
    // enabled by calling the proper operation on its factory."
    let disabled_writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    assert!(!disabled_writer.is_enabled());
    publisher.delete_datawriter(&disabled_writer).unwrap();
    let disabled_reader = subscriber
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    subscriber.delete_datareader(&disabled_reader).unwrap();

    // The reader of a disabled subscriber: its listener is not called, its
    // conditions are inactive, until it is enabled.
    let heard = Arc::new(Heard::default());
    let reader = subscriber
        .create_datareader_with_listener(
            &topic,
            DataReaderQos::default(),
            Some(heard.clone()),
            status::DATA_AVAILABLE,
        )
        .unwrap();
    assert!(!reader.is_enabled());
    let read_condition = reader
        .create_readcondition(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap();
    reader.__push_raw(vec![1]).unwrap();
    assert_eq!(heard.0.load(Ordering::SeqCst), 0, "no call before enable");
    assert_eq!(reader.get_status_changes(), 0);
    assert!(
        !zerodds_dcps::Condition::get_trigger_value(&*read_condition),
        "a Condition of a disabled entity has trigger_value FALSE"
    );
    assert!(
        !zerodds_dcps::Condition::get_trigger_value(&*reader.get_status_condition()),
        "the StatusCondition of a disabled entity is inactive"
    );

    p.enable().unwrap();
    subscriber.enable().unwrap();
    reader.enable().unwrap();
    reader.__push_raw(vec![2]).unwrap();
    assert!(
        heard.0.load(Ordering::SeqCst) >= 1,
        "the listener is called once the reader is enabled"
    );
    assert!(zerodds_dcps::Condition::get_trigger_value(&*read_condition));

    // A reader that holds a ReadCondition is not deleted (§2.2.2.5.2.6).
    assert!(matches!(
        subscriber.delete_datareader(&reader),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    reader.delete_contained_entities().unwrap();
    subscriber.delete_datareader(&reader).unwrap();
    p.delete_publisher(&publisher).unwrap();
    p.delete_subscriber(&subscriber).unwrap();
    p.delete_topic(&topic).unwrap();
}

/// Puts the factory's QoS back when the test ends, also when it fails.
struct RestoreFactoryQos(DomainParticipantFactoryQos);

impl Drop for RestoreFactoryQos {
    fn drop(&mut self) {
        let _ = DomainParticipantFactory::instance().set_qos(self.0);
    }
}

/// DDS 1.4 §2.2.3.20 on the DomainParticipantFactory: ENTITY_FACTORY "is
/// mutable. A change in the policy affects only the entities created after
/// the change; not the previously created entities." The switch to TRUE
/// leaves a participant created under FALSE disabled and enables the next
/// one; the switch back to FALSE leaves that one enabled and creates the
/// next one disabled.
#[test]
fn dcps_2_2_3_20_a_changed_factory_qos_affects_only_participants_created_afterwards() {
    let _serial = FACTORY_QOS.lock();
    let factory = DomainParticipantFactory::instance();
    let _restore = RestoreFactoryQos(factory.get_qos());
    let switch = |on: bool| {
        factory
            .set_qos(DomainParticipantFactoryQos {
                autoenable_created_entities: on,
            })
            .unwrap();
        assert_eq!(factory.get_qos().autoenable_created_entities, on);
    };
    let create = || factory.create_participant_offline(0, DomainParticipantQos::default());

    switch(false);
    let p1 = create();
    assert!(!p1.is_enabled());
    switch(true);
    assert!(
        !p1.is_enabled(),
        "the change to TRUE enabled an older participant"
    );
    let p2 = create();
    assert!(
        p2.is_enabled(),
        "a participant created after the change to TRUE"
    );
    switch(false);
    assert!(p2.is_enabled());
    let p3 = create();
    assert!(
        !p3.is_enabled(),
        "a participant created after the change to FALSE"
    );
    assert!(!p1.is_enabled());
}

/// DDS 1.4 §2.2.2.1.1.7 on a DomainParticipant its factory created
/// disabled: "If the ENTITY_FACTORY Qos policy has
/// autoenable_created_entities set to TRUE, the enable operation on the
/// factory will automatically enable all entities created from the
/// factory." The value the participant has at its `enable` decides, also
/// for the topic, publisher and subscriber it created before a `set_qos`
/// of ENTITY_FACTORY (§2.2.3.20); the `set_qos` itself enabled none of
/// them.
#[test]
fn dcps_2_2_3_20_the_enable_of_a_participant_follows_the_entity_factory_it_has_then() {
    let _serial = FACTORY_QOS.lock();
    let factory = DomainParticipantFactory::instance();
    let _restore = RestoreFactoryQos(factory.get_qos());
    factory
        .set_qos(DomainParticipantFactoryQos {
            autoenable_created_entities: false,
        })
        .unwrap();
    let with_factory = |on: bool| {
        let mut qos = DomainParticipantQos::default();
        qos.entity_factory.autoenable_created_entities = on;
        qos
    };
    let children = |p: &zerodds_dcps::DomainParticipant, name: &str| {
        let topic = p
            .create_topic::<RawBytes>(name, TopicQos::default())
            .unwrap();
        (
            topic,
            p.create_publisher(PublisherQos::default()),
            p.create_subscriber(SubscriberQos::default()),
        )
    };

    // Created under FALSE, enabled by the participant's enable under TRUE.
    let up = factory.create_participant_offline(0, with_factory(false));
    assert!(!up.is_enabled());
    let (topic, publisher, subscriber) = children(&up, "EnableUnderTrue");
    assert!(!topic.is_enabled() && !publisher.is_enabled() && !subscriber.is_enabled());
    up.set_qos(with_factory(true)).unwrap();
    assert!(
        !topic.is_enabled() && !publisher.is_enabled() && !subscriber.is_enabled(),
        "the set_qos of ENTITY_FACTORY enabled a child"
    );
    up.enable().unwrap();
    assert!(up.is_enabled());
    assert!(topic.is_enabled(), "the topic stayed disabled");
    assert!(publisher.is_enabled(), "the publisher stayed disabled");
    assert!(subscriber.is_enabled(), "the subscriber stayed disabled");

    // Created under TRUE, left disabled by the participant's enable under
    // FALSE; each child is enabled by hand.
    let down = factory.create_participant_offline(0, with_factory(true));
    assert!(!down.is_enabled());
    let (topic, publisher, subscriber) = children(&down, "EnableUnderFalse");
    down.set_qos(with_factory(false)).unwrap();
    down.enable().unwrap();
    assert!(down.is_enabled());
    assert!(
        !topic.is_enabled() && !publisher.is_enabled() && !subscriber.is_enabled(),
        "the enable under FALSE enabled a child"
    );
    topic.enable().unwrap();
    publisher.enable().unwrap();
    subscriber.enable().unwrap();
    assert!(topic.is_enabled() && publisher.is_enabled() && subscriber.is_enabled());
}
