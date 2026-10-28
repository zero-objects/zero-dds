//! DDS-XTypes 1.3 §7.6.5 and Annex E: the type-specific writers of the
//! built-in types and the operations they add.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_dcps::builtin_types::{
    BytesDataWriter, BytesDataWriterOperations, DdsBytes, DdsKeyedBytes, DdsKeyedString, DdsString,
    KeyedBytesDataWriter, KeyedBytesDataWriterOperations, KeyedStringDataWriter,
    KeyedStringDataWriterOperations, StringDataWriter,
};
use zerodds_dcps::{
    DataWriterQos, DdsError, DomainParticipantFactory, DomainParticipantQos, HANDLE_NIL,
    PublisherQos, TopicQos,
};

fn participant() -> zerodds_dcps::DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default())
}

#[test]
fn the_four_types_are_topic_types_with_a_string_writer() {
    let p = participant();
    let publisher = p.create_publisher(PublisherQos::default());
    let topic = p
        .create_topic::<DdsString>("S", TopicQos::default())
        .unwrap();
    let writer: StringDataWriter = publisher
        .create_datawriter::<DdsString>(&topic, DataWriterQos::default())
        .unwrap();
    writer.write(&DdsString::new("hello")).unwrap();
}

#[test]
fn keyed_string_operations_work_on_the_key() {
    let p = participant();
    let publisher = p.create_publisher(PublisherQos::default());
    let topic = p
        .create_topic::<DdsKeyedString>("KS", TopicQos::default())
        .unwrap();
    let writer: KeyedStringDataWriter = publisher
        .create_datawriter::<DdsKeyedString>(&topic, DataWriterQos::default())
        .unwrap();

    assert_eq!(writer.lookup_instance_w_key("a"), HANDLE_NIL);
    let handle = writer.register_instance_w_key("a").unwrap();
    assert_ne!(handle, HANDLE_NIL);
    assert_eq!(writer.lookup_instance_w_key("a"), handle);

    let mut key = String::new();
    writer.get_key_value_w_key(&mut key, handle).unwrap();
    assert_eq!(key, "a");

    writer.write_string_w_key("a", "payload", handle).unwrap();
    // A handle of another instance is refused.
    let other = writer.register_instance_w_key("b").unwrap();
    assert!(matches!(
        writer.write_string_w_key("a", "payload", other),
        Err(DdsError::BadParameter { .. })
    ));
    writer
        .write_string_w_key("a", "payload", HANDLE_NIL)
        .unwrap();
    writer.dispose_w_key("a").unwrap();
    writer.unregister_instance_w_key("b").unwrap();
}

#[test]
fn bytes_are_written_from_an_offset_and_a_length() {
    let p = participant();
    let publisher = p.create_publisher(PublisherQos::default());
    let topic = p
        .create_topic::<DdsBytes>("B", TopicQos::default())
        .unwrap();
    let writer: BytesDataWriter = publisher
        .create_datawriter::<DdsBytes>(&topic, DataWriterQos::default())
        .unwrap();
    writer
        .write_w_bytes(&[1, 2, 3, 4], 1, 2, HANDLE_NIL)
        .unwrap();
    assert!(matches!(
        writer.write_w_bytes(&[1, 2, 3, 4], 3, 2, HANDLE_NIL),
        Err(DdsError::BadParameter { .. })
    ));
    assert!(matches!(
        writer.write_w_bytes(&[1, 2, 3, 4], -1, 2, HANDLE_NIL),
        Err(DdsError::BadParameter { .. })
    ));
}

#[test]
fn keyed_bytes_operations_work_on_the_key() {
    let p = participant();
    let publisher = p.create_publisher(PublisherQos::default());
    let topic = p
        .create_topic::<DdsKeyedBytes>("KB", TopicQos::default())
        .unwrap();
    let writer: KeyedBytesDataWriter = publisher
        .create_datawriter::<DdsKeyedBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let handle = writer.register_instance_w_key("k").unwrap();
    writer
        .write_bytes_w_key("k", &[9, 8, 7], 0, 3, handle)
        .unwrap();
    let mut key = String::new();
    writer.get_key_value_w_key(&mut key, handle).unwrap();
    assert_eq!(key, "k");
    assert!(
        writer
            .write_bytes_w_key("k", &[9], 0, 5, HANDLE_NIL)
            .is_err()
    );
}
