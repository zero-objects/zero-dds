// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `delete_contentfilteredtopic` (DDS 1.4 §2.2.2.2.1.8) and
//! `delete_multitopic` (§2.2.2.2.1.10).
//!
//! §2.2.2.2.1.8: "If the delete_contentfilteredtopic operation is called on
//! a ContentFilteredTopic with existing DataReader objects attached to it,
//! it will return PRECONDITION_NOT_MET." A deleted object answers
//! ALREADY_DELETED (§2.2.1.1). Offline participants: the state is local.

#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use zerodds_dcps::{
    DataReaderQos, DdsError, DomainParticipant, DomainParticipantFactory, DomainParticipantQos,
    RawBytes, SubscriberQos, TopicQos,
};

mod common;

fn offline() -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default())
}

#[test]
fn a_content_filtered_topic_with_a_reader_is_not_deleted() {
    let p = offline();
    let topic = p
        .create_topic::<RawBytes>("Base", TopicQos::default())
        .unwrap();
    let cft = p
        .create_contentfilteredtopic("Base::CFT", &topic, "id > 0", Vec::new())
        .unwrap();
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader_with_cft(&cft, DataReaderQos::default())
        .unwrap();

    assert!(matches!(
        p.delete_contentfilteredtopic(&cft).unwrap_err(),
        DdsError::PreconditionNotMet { .. }
    ));
    // Refused: the topic is as it was, the topic of the CFT still held.
    assert_eq!(p.topic_users("Base").unwrap().content_filtered_topics, 1);
    cft.set_filter_parameters(Vec::new()).unwrap();

    sub.delete_datareader(&reader).unwrap();
    p.delete_contentfilteredtopic(&cft)
        .expect("no reader is attached any more");
}

#[test]
fn a_dropped_reader_lets_the_content_filtered_topic_go() {
    let p = offline();
    let topic = p
        .create_topic::<RawBytes>("Base", TopicQos::default())
        .unwrap();
    let cft = p
        .create_contentfilteredtopic("Base::CFT", &topic, "id > 0", Vec::new())
        .unwrap();
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader_with_cft(&cft, DataReaderQos::default())
        .unwrap();
    drop(reader);
    p.delete_contentfilteredtopic(&cft).unwrap();
}

#[test]
fn a_deleted_content_filtered_topic_is_deleted_for_every_handle() {
    let p = offline();
    let topic = p
        .create_topic::<RawBytes>("Base", TopicQos::default())
        .unwrap();
    let cft = p
        .create_contentfilteredtopic("Base::CFT", &topic, "id > %0", vec!["1".to_string()])
        .unwrap();
    let other_handle = cft.clone();
    assert_eq!(p.topic_users("Base").unwrap().content_filtered_topics, 2);

    p.delete_contentfilteredtopic(&cft).unwrap();

    // Both handles stop using the topic at once; the topic can go.
    assert_eq!(p.topic_users("Base").unwrap().content_filtered_topics, 0);
    p.delete_topic(&topic).unwrap();

    assert!(matches!(
        p.delete_contentfilteredtopic(&other_handle).unwrap_err(),
        DdsError::AlreadyDeleted
    ));
    assert!(matches!(
        other_handle
            .set_filter_parameters(vec!["2".to_string()])
            .unwrap_err(),
        DdsError::AlreadyDeleted
    ));
    // Dropping the handles after the delete does not take a use back twice.
    drop(cft);
    drop(other_handle);
}

#[test]
fn a_reader_cannot_be_created_on_a_deleted_content_filtered_topic() {
    let p = offline();
    let topic = p
        .create_topic::<RawBytes>("Base", TopicQos::default())
        .unwrap();
    let cft = p
        .create_contentfilteredtopic("Base::CFT", &topic, "id > 0", Vec::new())
        .unwrap();
    p.delete_contentfilteredtopic(&cft).unwrap();
    let sub = p.create_subscriber(SubscriberQos::default());
    assert!(matches!(
        sub.create_datareader_with_cft(&cft, DataReaderQos::default())
            .map(|_| ())
            .unwrap_err(),
        DdsError::AlreadyDeleted
    ));
}

#[test]
fn a_deleted_multitopic_releases_its_related_topics_and_answers_already_deleted() {
    let p = offline();
    let topic = common::order_topic(&p, "Joined");
    let multitopic = {
        common::dynamic_support(common::ORDER_COPY_IDL, "OrderCopy", true)
            .register_type(&p, None)
            .unwrap();
        p.create_dynamic_multitopic(
            "Joined::Multi",
            "OrderCopy",
            "SELECT * FROM Joined WHERE amount > %0",
            vec!["0".to_string()],
        )
        .unwrap()
    };
    let other_handle = multitopic.clone();
    assert_eq!(p.topic_users("Joined").unwrap().multi_topics, 2);

    p.delete_multitopic(&multitopic).unwrap();

    assert_eq!(p.topic_users("Joined").unwrap().multi_topics, 0);
    p.delete_topic(&topic).unwrap();
    assert!(matches!(
        p.delete_multitopic(&other_handle).unwrap_err(),
        DdsError::AlreadyDeleted
    ));
    assert!(matches!(
        other_handle
            .set_expression_parameters(vec!["1".to_string()])
            .unwrap_err(),
        DdsError::AlreadyDeleted
    ));
    drop(multitopic);
    drop(other_handle);
}
