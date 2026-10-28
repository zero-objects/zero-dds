//! DDS-XTypes 1.3 §7.6.4.1 and §7.6.4.2: a participant may hold several
//! topics of one name, "type-based subsets of 'the same' network topic",
//! with consistent QoS; `lookup_topicdescription` takes an index that
//! iterates over them.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_dcps::dds_type::{DecodeError, EncodeError};
use zerodds_dcps::{
    DdsError, DdsType, DomainParticipantFactory, DomainParticipantQos, RawBytes, TopicDescription,
    TopicQos,
};

/// A second type, under another type name, for the name `RawBytes` uses.
struct Other;

impl DdsType for Other {
    const TYPE_NAME: &'static str = "test::Other";
    const XCDR1_MATCHES_XCDR2: bool = true;

    fn encode(&self, _out: &mut Vec<u8>) -> Result<(), EncodeError> {
        Ok(())
    }

    fn decode(_bytes: &[u8]) -> Result<Self, DecodeError> {
        Ok(Self)
    }
}

fn participant() -> zerodds_dcps::DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default())
}

#[test]
fn a_second_type_under_one_name_is_a_second_topic() {
    let p = participant();
    let first = p
        .create_topic::<RawBytes>("Shared", TopicQos::default())
        .unwrap();
    let second = p
        .create_topic::<Other>("Shared", TopicQos::default())
        .unwrap();
    assert_eq!(p.topics_len(), 2);
    assert_eq!(first.get_name(), second.get_name());
    assert_ne!(first.get_type_name(), second.get_type_name());
}

#[test]
fn lookup_iterates_over_the_topics_of_a_name() {
    let p = participant();
    p.create_topic::<RawBytes>("Shared", TopicQos::default())
        .unwrap();
    p.create_topic::<Other>("Shared", TopicQos::default())
        .unwrap();

    let at = |i| p.lookup_topicdescription_at("Shared", i);
    let zero = at(0).expect("index 0");
    let one = at(1).expect("index 1");
    assert_ne!(zero.get_type_name(), one.get_type_name());
    assert!(at(2).is_none(), "an index outside the range is nil");
    // The index-less form is index 0.
    assert_eq!(
        p.lookup_topicdescription("Shared").unwrap().get_type_name(),
        zero.get_type_name()
    );
    assert!(p.lookup_topicdescription_at("Absent", 0).is_none());
}

#[test]
fn an_inconsistent_qos_is_refused_and_counted() {
    let p = participant();
    p.create_topic::<RawBytes>("Shared", TopicQos::default())
        .unwrap();
    let mut other = TopicQos::default();
    other.topic_data.value = vec![1, 2, 3];
    let err = p
        .create_topic::<Other>("Shared", other)
        .err()
        .expect("refused");
    assert!(matches!(err, DdsError::InconsistentPolicy { .. }));
    assert_eq!(p.topics_len(), 1);
}

#[test]
fn deleting_one_topic_leaves_the_other_of_its_name() {
    let p = participant();
    let first = p
        .create_topic::<RawBytes>("Shared", TopicQos::default())
        .unwrap();
    p.create_topic::<Other>("Shared", TopicQos::default())
        .unwrap();
    p.delete_topic(&first).unwrap();
    assert_eq!(p.topics_len(), 1);
    assert_eq!(
        p.lookup_topicdescription_at("Shared", 0)
            .unwrap()
            .get_type_name(),
        "test::Other"
    );
    assert!(p.lookup_topicdescription_at("Shared", 1).is_none());
}

/// §7.6.4.1: the topics of one name are objects of their own. A DataWriter
/// on the second one is a user of the second one: the first is deleted
/// without it, the second not before it is gone.
#[test]
fn a_topic_waits_for_its_own_endpoints_and_not_for_those_of_the_other_of_its_name() {
    use zerodds_dcps::{DataWriterQos, PublisherQos};
    let p = participant();
    let first = p
        .create_topic::<RawBytes>("Shared", TopicQos::default())
        .unwrap();
    let second = p
        .create_topic::<Other>("Shared", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let writer = publisher
        .create_datawriter::<Other>(&second, DataWriterQos::default())
        .unwrap();
    assert_eq!(p.topic_users("Shared").unwrap().data_writers, 1);

    p.delete_topic(&first)
        .expect("the writer is on the other topic of the name");
    assert!(matches!(
        p.delete_topic(&second),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    drop(writer);
    p.delete_topic(&second).expect("nothing uses it any more");
    assert_eq!(p.topics_len(), 0);
}
