// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `DomainParticipant::delete_topic` (DDS 1.4 §2.2.2.2.1.6).
//!
//! The clause under test: "The deletion of a Topic is not allowed if there
//! are any existing DataReader, DataWriter, ContentFilteredTopic, or
//! MultiTopic objects that are using the Topic. If the delete_topic
//! operation is called on a Topic with any of these existing objects
//! attached to it, it will return PRECONDITION_NOT_MET. The delete_topic
//! operation must be called on the same DomainParticipant object used to
//! create the Topic. If delete_topic is called on a different
//! DomainParticipant, the operation will have no effect and it will return
//! PRECONDITION_NOT_MET."
//!
//! Every participant here is offline (`create_participant_offline`): the
//! precondition, the registry and the return codes are local state, so
//! nothing in this file needs a socket.

#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use zerodds_dcps::topic::TopicDescription as _;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DdsType, DomainParticipant, DomainParticipantFactory,
    DomainParticipantQos, Entity, PublisherQos, RawBytes, SubscriberQos, TopicQos, TopicUserCounts,
};

mod common;

fn offline() -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default())
}

/// §2.2.2.2.1.6 with nothing attached to the topic: the deletion happens.
#[test]
fn delete_topic_succeeds_when_nothing_uses_it() {
    let p = offline();
    let topic = p
        .create_topic::<RawBytes>("Free", TopicQos::default())
        .unwrap();
    assert_eq!(p.topics_len(), 1);
    assert_eq!(p.topic_users("Free").unwrap(), TopicUserCounts::default());
    p.delete_topic(&topic)
        .expect("no user, so the delete is allowed");
    assert_eq!(p.topics_len(), 0);
}

/// §2.2.2.2.1.12 `lookup_topicdescription` and §2.2.2.2.1.10
/// `contains_entity` no longer see a deleted topic; `create_topic` with the
/// same name then builds a new one rather than handing the old state back.
#[test]
fn a_deleted_topic_is_gone_from_the_registry_and_its_name_is_free_again() {
    let p = offline();
    let topic = p
        .create_topic::<RawBytes>("Cycling", TopicQos::default())
        .unwrap();
    let handle = topic.get_instance_handle();
    assert!(p.contains_entity(handle));
    assert!(p.lookup_topicdescription("Cycling").is_some());

    p.delete_topic(&topic).unwrap();

    assert!(
        p.lookup_topicdescription("Cycling").is_none(),
        "§2.2.2.2.1.12 searches the participant's local topics"
    );
    assert!(
        !p.contains_entity(handle),
        "§2.2.2.2.1.10 contains_entity walks the participant's topics"
    );

    let again = p
        .create_topic::<RawBytes>("Cycling", TopicQos::default())
        .unwrap();
    assert_eq!(again.name(), "Cycling");
    assert_eq!(p.topics_len(), 1);
    assert!(p.lookup_topicdescription("Cycling").is_some());
}

/// DDS 1.4 §2.2.2.2.1.11: "If a Topic is obtained multiple times by means
/// of find_topic or create_topic, it must also be deleted that same number
/// of times using delete_topic." Two `create_topic` calls of the same name
/// and type give one entity; the first `delete_topic` leaves it, the
/// second deletes it, and a third is ALREADY_DELETED. The first deleted
/// it.
#[test]
fn a_topic_created_twice_goes_with_the_second_delete() {
    let p = offline();
    let first = p
        .create_topic::<RawBytes>("Shared", TopicQos::default())
        .unwrap();
    let second = p
        .create_topic::<RawBytes>("Shared", TopicQos::default())
        .unwrap();
    p.delete_topic(&first).unwrap();
    assert!(p.lookup_topicdescription("Shared").is_some(), "still there");
    let reader = p
        .create_subscriber(zerodds_dcps::SubscriberQos::default())
        .create_datareader::<RawBytes>(&second, zerodds_dcps::DataReaderQos::default());
    assert!(reader.is_ok(), "the topic is usable after the first delete");
    drop(reader);
    p.delete_topic(&second).unwrap();
    assert!(p.lookup_topicdescription("Shared").is_none());
    assert!(matches!(
        p.delete_topic(&second).unwrap_err(),
        DdsError::AlreadyDeleted
    ));
    // Both handles still answer for themselves.
    assert_eq!(second.name(), "Shared");
    assert_eq!(second.type_name(), RawBytes::TYPE_NAME);
    assert_eq!(second.get_participant().domain_id(), 0);
}

/// §2.2.2.2.1.6, second rule: "The delete_topic operation must be called on
/// the same DomainParticipant object used to create the Topic. If
/// delete_topic is called on a different DomainParticipant, the operation
/// will have no effect and it will return PRECONDITION_NOT_MET."
#[test]
fn delete_topic_from_another_participant_is_refused_and_changes_nothing() {
    let owner = offline();
    let stranger = offline();
    let topic = owner
        .create_topic::<RawBytes>("Owned", TopicQos::default())
        .unwrap();

    assert!(matches!(
        stranger.delete_topic(&topic).unwrap_err(),
        DdsError::PreconditionNotMet { .. }
    ));
    assert_eq!(owner.topics_len(), 1, "the topic is untouched");
    assert!(owner.lookup_topicdescription("Owned").is_some());

    owner
        .delete_topic(&topic)
        .expect("the owning participant may delete it");
    assert_eq!(owner.topics_len(), 0);
}

/// §2.2.2.2.1.6, first rule, DataWriter clause.
#[test]
fn delete_topic_is_refused_while_a_datawriter_uses_it() {
    let p = offline();
    let topic = p
        .create_topic::<RawBytes>("Written", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let writer = publisher
        .create_datawriter(&topic, DataWriterQos::default())
        .unwrap();
    assert_eq!(p.topic_users("Written").unwrap().data_writers, 1);

    assert!(matches!(
        p.delete_topic(&topic).unwrap_err(),
        DdsError::PreconditionNotMet { .. }
    ));
    assert_eq!(p.topics_len(), 1, "a refused delete removes nothing");

    drop(writer);
    assert_eq!(p.topic_users("Written").unwrap().data_writers, 0);
    p.delete_topic(&topic)
        .expect("the writer is gone, so the delete is allowed");
    assert_eq!(p.topics_len(), 0);
}

/// §2.2.2.2.1.6, first rule, DataReader clause.
#[test]
fn delete_topic_is_refused_while_a_datareader_uses_it() {
    let p = offline();
    let topic = p
        .create_topic::<RawBytes>("Read", TopicQos::default())
        .unwrap();
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader(&topic, DataReaderQos::default())
        .unwrap();
    assert_eq!(p.topic_users("Read").unwrap().data_readers, 1);

    assert!(matches!(
        p.delete_topic(&topic).unwrap_err(),
        DdsError::PreconditionNotMet { .. }
    ));

    drop(reader);
    assert_eq!(p.topic_users("Read").unwrap().data_readers, 0);
    p.delete_topic(&topic)
        .expect("the reader is gone, so the delete is allowed");
}

/// §2.2.2.2.1.6, first rule, ContentFilteredTopic clause. The related topic
/// is held for as long as the ContentFilteredTopic's handle exists — this API
/// deletes a ContentFilteredTopic with its last handle, as
/// `delete_contentfilteredtopic` documents — so the count follows the handle.
#[test]
fn delete_topic_is_refused_while_a_content_filtered_topic_uses_it() {
    let p = offline();
    let topic = p
        .create_topic::<RawBytes>("Filtered", TopicQos::default())
        .unwrap();
    let cft = p
        .create_contentfilteredtopic("Filtered::CFT", &topic, "id > %0", vec!["0".to_string()])
        .unwrap();
    assert_eq!(
        p.topic_users("Filtered").unwrap().content_filtered_topics,
        1
    );

    assert!(matches!(
        p.delete_topic(&topic).unwrap_err(),
        DdsError::PreconditionNotMet { .. }
    ));

    p.delete_contentfilteredtopic(&cft).unwrap();
    drop(cft);
    assert_eq!(
        p.topic_users("Filtered").unwrap().content_filtered_topics,
        0
    );
    p.delete_topic(&topic)
        .expect("no filtered topic left, so the delete is allowed");
}

/// §2.2.2.2.1.6, first rule, MultiTopic clause. A MultiTopic names its
/// related topics, and each of them is held while the MultiTopic exists.
#[test]
fn delete_topic_is_refused_while_a_multitopic_uses_it() {
    let p = offline();
    let topic = common::order_topic(&p, "Joined");
    let multitopic = common::order_copy_multitopic(&p, "Joined::Multi", "Joined");
    assert_eq!(p.topic_users("Joined").unwrap().multi_topics, 1);

    assert!(matches!(
        p.delete_topic(&topic).unwrap_err(),
        DdsError::PreconditionNotMet { .. }
    ));

    p.delete_multitopic(&multitopic).unwrap();
    drop(multitopic);
    assert_eq!(p.topic_users("Joined").unwrap().multi_topics, 0);
    p.delete_topic(&topic)
        .expect("no MultiTopic left, so the delete is allowed");
}

/// Every kind at once, then none: the precondition is "any", so the count
/// is per kind and the topic is freed only when all four are empty.
#[test]
fn every_kind_of_user_holds_the_topic_and_releasing_all_of_them_frees_it() {
    let p = offline();
    let topic = common::order_topic(&p, "Busy");
    let publisher = p.create_publisher(PublisherQos::default());
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let writer = publisher
        .create_datawriter(&topic, DataWriterQos::default())
        .unwrap();
    let reader = subscriber
        .create_datareader(&topic, DataReaderQos::default())
        .unwrap();
    let cft = p
        .create_contentfilteredtopic("Busy::CFT", &topic, "id > 0", Vec::new())
        .unwrap();
    let multitopic = common::order_copy_multitopic(&p, "Busy::Multi", "Busy");
    let users = p.topic_users("Busy").unwrap();
    assert_eq!(users.data_writers, 1);
    assert_eq!(users.data_readers, 1);
    assert_eq!(users.content_filtered_topics, 1);
    assert_eq!(users.multi_topics, 1);

    // One kind released at a time; the topic stays undeletable until the last
    // one is gone, and each release is visible on its own.
    drop(writer);
    assert_eq!(p.topic_users("Busy").unwrap().data_writers, 0);
    assert!(matches!(
        p.delete_topic(&topic).unwrap_err(),
        DdsError::PreconditionNotMet { .. }
    ));

    drop(reader);
    assert_eq!(p.topic_users("Busy").unwrap().data_readers, 0);
    assert!(matches!(
        p.delete_topic(&topic).unwrap_err(),
        DdsError::PreconditionNotMet { .. }
    ));

    drop(cft);
    assert_eq!(p.topic_users("Busy").unwrap().content_filtered_topics, 0);
    assert!(matches!(
        p.delete_topic(&topic).unwrap_err(),
        DdsError::PreconditionNotMet { .. }
    ));

    drop(multitopic);
    assert_eq!(p.topic_users("Busy").unwrap(), Default::default());
    p.delete_topic(&topic).expect("all users are gone");
}

/// DDS 1.4 §2.2.2.4.1.5 and §2.2.2.5.2.5: "If the Topic was created from
/// a different DomainParticipant, the operation will fail and return a nil
/// result." Neither endpoint is created, and the owning participant counts
/// no user of its topic.
#[test]
fn an_endpoint_cannot_use_the_topic_of_another_participant() {
    let owner = offline();
    let stranger = offline();
    let topic = owner
        .create_topic::<RawBytes>("Shared", TopicQos::default())
        .unwrap();
    let writer = stranger
        .create_publisher(PublisherQos::default())
        .create_datawriter(&topic, DataWriterQos::default());
    assert!(matches!(writer, Err(DdsError::PreconditionNotMet { .. })));
    let reader = stranger
        .create_subscriber(SubscriberQos::default())
        .create_datareader(&topic, DataReaderQos::default());
    assert!(matches!(reader, Err(DdsError::PreconditionNotMet { .. })));
    assert_eq!(owner.topic_users("Shared").unwrap().data_writers, 0);
    owner
        .delete_topic(&topic)
        .expect("no endpoint holds the topic");
}

/// A `Topic::new` built outside `create_topic` owns state this participant
/// never registered; deleting it would dispose somebody else's registry
/// entry, so it is refused with a message rather than half-done. A builtin
/// `Topic::new_orphan` belongs to no participant at all and is refused the
/// same way.
#[test]
fn delete_topic_refuses_a_topic_that_was_never_registered() {
    let p = offline();
    let stray = zerodds_dcps::topic::Topic::<RawBytes>::new(
        "Stray".to_string(),
        TopicQos::default(),
        p.clone(),
    );
    assert!(p.lookup_topicdescription("Stray").is_none());
    assert!(matches!(
        p.delete_topic(&stray).unwrap_err(),
        DdsError::PreconditionNotMet { .. }
    ));
    let builtin = zerodds_dcps::topic::Topic::<RawBytes>::new_orphan(
        "DCPSBuiltin".to_string(),
        TopicQos::default(),
    );
    assert!(matches!(
        p.delete_topic(&builtin).unwrap_err(),
        DdsError::PreconditionNotMet { .. }
    ));
}

/// The untyped form the C-FFI uses: a name, not a typed handle. It follows
/// the same precondition and the same ALREADY_DELETED answer.
#[test]
fn delete_topic_by_name_follows_the_same_rules() {
    let p = offline();
    let topic = p
        .create_topic::<RawBytes>("ByName", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let writer = publisher
        .create_datawriter(&topic, DataWriterQos::default())
        .unwrap();

    assert!(matches!(
        p.delete_topic_by_name("ByName").unwrap_err(),
        DdsError::PreconditionNotMet { .. }
    ));
    assert_eq!(p.topics_len(), 1);

    drop(writer);
    p.delete_topic_by_name("ByName").expect("no user left");
    assert_eq!(p.topics_len(), 0);
    assert!(matches!(
        p.delete_topic_by_name("ByName").unwrap_err(),
        DdsError::AlreadyDeleted
    ));
    assert!(matches!(
        p.delete_topic_by_name("NeverCreated").unwrap_err(),
        DdsError::AlreadyDeleted
    ));
}
