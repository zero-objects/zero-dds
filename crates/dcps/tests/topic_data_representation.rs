//! DDS-XTypes 1.3 §7.6.3.1.1: the TopicQos carries a
//! DataRepresentationQosPolicy; "the first element of the sequence applies
//! to writers of the Topic, and the whole sequence applies to readers of
//! the Topic". The policy cannot be changed once its entity is enabled.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DomainParticipantFactory, DomainParticipantQos, Entity,
    Publisher, RawBytes, Subscriber, TopicQos, psm_constants,
};

#[test]
fn the_topic_qos_names_a_representation_for_its_writers_and_readers() {
    let topic = TopicQos {
        data_representation: Some(vec![2, 0]),
        ..TopicQos::default()
    };
    let mut writer = DataWriterQos::default();
    Publisher::copy_from_topic_qos(&mut writer, &topic).unwrap();
    assert_eq!(writer.data_representation, Some(vec![2]));

    let mut reader = DataReaderQos::default();
    Subscriber::copy_from_topic_qos(&mut reader, &topic).unwrap();
    assert_eq!(reader.data_representation, Some(vec![2, 0]));
}

#[test]
fn an_empty_list_is_the_single_element_xcdr() {
    let topic = TopicQos {
        data_representation: Some(Vec::new()),
        ..TopicQos::default()
    };
    let mut writer = DataWriterQos::default();
    Publisher::copy_from_topic_qos(&mut writer, &topic).unwrap();
    assert_eq!(
        writer.data_representation,
        Some(vec![psm_constants::data_representation_id::XCDR])
    );
    let mut reader = DataReaderQos::default();
    Subscriber::copy_from_topic_qos(&mut reader, &topic).unwrap();
    assert_eq!(reader.data_representation, Some(vec![0]));
}

#[test]
fn the_default_topic_leaves_the_default_representation() {
    assert_eq!(TopicQos::default().data_representation, None);
    let mut writer = DataWriterQos::default();
    Publisher::copy_from_topic_qos(&mut writer, &TopicQos::default()).unwrap();
    assert_eq!(writer.data_representation, None);
}

#[test]
fn an_enabled_topic_refuses_a_new_representation() {
    let participant = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let topic = participant
        .create_topic::<RawBytes>("Representation", TopicQos::default())
        .unwrap();
    topic.enable().unwrap();
    let err = topic
        .set_qos(TopicQos {
            data_representation: Some(vec![2]),
            ..TopicQos::default()
        })
        .unwrap_err();
    assert!(matches!(
        err,
        DdsError::ImmutablePolicy {
            policy: "DATA_REPRESENTATION"
        }
    ));
}
