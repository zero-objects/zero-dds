// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What DDS 1.4 §2.2.2.1.1.7 and §2.2.2.2.1 say about an entity that is not
//! enabled: the operations that stay legal, NOT_ENABLED for the others, no
//! statuses, no listener calls, inactive conditions, and that such an entity
//! can be deleted. One test: it switches the factory to
//! `autoenable_created_entities = FALSE` and back.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use zerodds_dcps::listener::DataReaderListener;
use zerodds_dcps::psm_constants::status;
use zerodds_dcps::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DomainParticipantFactory, DomainParticipantQos, Entity,
    HANDLE_NIL, InstanceHandle, PublisherQos, RawBytes, SubscriberQos, TopicQos,
};

#[derive(Default)]
struct Heard(AtomicU32);

impl DataReaderListener for Heard {
    fn on_data_available(&self, _reader: InstanceHandle) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn a_disabled_entity_is_inert_and_can_be_deleted() {
    let factory = DomainParticipantFactory::instance();
    let before = factory.get_qos();
    factory
        .set_qos(zerodds_dcps::factory::DomainParticipantFactoryQos {
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
