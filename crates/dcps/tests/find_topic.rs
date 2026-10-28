// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `find_topic` (DDS 1.4 §2.2.2.2.1.11) and `lookup_topicdescription`
//! (§2.2.2.2.1.12): both give back a TopicDescription an application creates
//! DataWriters and DataReaders from; `lookup_topicdescription` finds the
//! Topics, ContentFilteredTopics and MultiTopics created here.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::time::{Duration, Instant};

use zerodds_dcps::dds_type::{DecodeError, EncodeError};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DdsType, DomainParticipant, DomainParticipantFactory,
    DomainParticipantQos, PublisherQos, RawBytes, SubscriberQos, TopicDescription,
    TopicDescriptionKind, TopicQos,
};

/// A second type, to be refused where `RawBytes` is meant.
struct Other;

impl DdsType for Other {
    const TYPE_NAME: &'static str = "test::OtherFind";
    const XCDR1_MATCHES_XCDR2: bool = true;

    fn encode(&self, _out: &mut Vec<u8>) -> Result<(), EncodeError> {
        Ok(())
    }

    fn decode(_bytes: &[u8]) -> Result<Self, DecodeError> {
        Ok(Self)
    }
}

fn offline() -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default())
}

/// The result of `find_topic` is a Topic a writer and a reader are created
/// from, not a name.
#[test]
fn find_topic_gives_a_topic_that_writers_and_readers_are_created_from() {
    let p = offline();
    p.create_topic::<RawBytes>("Found", TopicQos::default())
        .unwrap();
    let handle = p.find_topic("Found", Duration::from_millis(50)).unwrap();
    assert_eq!(handle.kind(), TopicDescriptionKind::Topic);
    assert_eq!(handle.get_name(), "Found");
    let topic = handle.topic::<RawBytes>().unwrap();
    p.create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .expect("a writer on the found topic");
    p.create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .expect("a reader on the found topic");
    // Another type is refused.
    assert!(matches!(
        handle.topic::<Other>(),
        Err(DdsError::BadParameter { .. })
    ));
}

/// "If the operation times-out, a 'nil' value is returned."
#[test]
fn find_topic_times_out_for_a_topic_nobody_creates() {
    let p = offline();
    let started = Instant::now();
    assert!(matches!(
        p.find_topic("Nobody", Duration::from_millis(100)),
        Err(DdsError::Timeout)
    ));
    assert!(started.elapsed() >= Duration::from_millis(100));
}

/// "Otherwise it waits (blocks the caller) until another mechanism creates it
/// (or the specified timeout occurs). This other mechanism can be another
/// thread".
#[test]
fn find_topic_waits_for_a_topic_another_thread_creates() {
    let p = offline();
    let creator = p.clone();
    let thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        creator
            .create_topic::<RawBytes>("Later", TopicQos::default())
            .unwrap()
    });
    let handle = p.find_topic("Later", Duration::from_secs(5)).unwrap();
    assert_eq!(handle.get_name(), "Later");
    let _kept = thread.join().unwrap();
}

/// "If a Topic is obtained multiple times by means of find_topic or
/// create_topic, it must also be deleted that same number of times using
/// delete_topic." `lookup_topicdescription` needs no extra deletion.
#[test]
fn a_topic_obtained_by_find_topic_is_deleted_as_often() {
    let p = offline();
    let created = p
        .create_topic::<RawBytes>("Counted", TopicQos::default())
        .unwrap();
    let found = p
        .find_topic("Counted", Duration::from_millis(50))
        .unwrap()
        .topic::<RawBytes>()
        .unwrap();
    // A lookup is not another topic to delete.
    let looked_up = p.lookup_topicdescription("Counted").unwrap();
    assert_eq!(looked_up.get_name(), "Counted");

    p.delete_topic(&found).unwrap();
    assert!(p.lookup_topicdescription("Counted").is_some(), "one left");
    p.delete_topic(&created).unwrap();
    assert!(p.lookup_topicdescription("Counted").is_none());
}

/// `lookup_topicdescription` "may be used to locate any locally-created
/// Topic, ContentFilteredTopic, and MultiTopic object", never creates one,
/// and a deleted one is not found.
#[test]
fn lookup_finds_a_content_filtered_topic_and_not_a_deleted_one() {
    let p = offline();
    let topic = p
        .create_topic::<RawBytes>("Related", TopicQos::default())
        .unwrap();
    let cft = p
        .create_contentfilteredtopic::<RawBytes>("Filtered", &topic, "id > 3", Vec::new())
        .unwrap();

    let handle = p.lookup_topicdescription("Filtered").expect("found");
    assert_eq!(handle.kind(), TopicDescriptionKind::ContentFilteredTopic);
    assert_eq!(handle.get_type_name(), topic.get_type_name());
    assert!(matches!(
        handle.topic::<RawBytes>(),
        Err(DdsError::BadParameter { .. })
    ));
    assert!(matches!(
        handle.content_filtered_topic::<Other>(),
        Err(DdsError::BadParameter { .. })
    ));

    // It names the same ContentFilteredTopic, and a reader is created on it.
    let again = handle.content_filtered_topic::<RawBytes>().unwrap();
    assert_eq!(again.get_filter_expression(), "id > 3");
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader_with_cft::<RawBytes>(&again, DataReaderQos::default())
        .expect("a reader on the found ContentFilteredTopic");
    // Nothing was created by the lookups.
    assert!(p.lookup_topicdescription("Absent").is_none());

    // A handle that exists uses the related topic.
    assert!(p.delete_topic(&topic).is_err());
    drop(again);
    drop(reader);
    p.delete_contentfilteredtopic(&cft).unwrap();
    assert!(p.lookup_topicdescription("Filtered").is_none());
    assert!(matches!(
        handle.content_filtered_topic::<RawBytes>(),
        Err(DdsError::AlreadyDeleted)
    ));
}

/// A ContentFilteredTopic nobody holds any more is not found.
#[test]
fn lookup_does_not_find_a_description_without_a_handle() {
    let p = offline();
    let topic = p
        .create_topic::<RawBytes>("Related2", TopicQos::default())
        .unwrap();
    let cft = p
        .create_contentfilteredtopic::<RawBytes>("Filtered2", &topic, "id > 3", Vec::new())
        .unwrap();
    assert!(p.lookup_topicdescription("Filtered2").is_some());
    drop(cft);
    assert!(p.lookup_topicdescription("Filtered2").is_none());
}
