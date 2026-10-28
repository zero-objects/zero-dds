//! DDS 1.4 §2.2.2.5.3: `create_readcondition`, `create_querycondition`
//! and `delete_readcondition` on the DataReader, and the reader's
//! `delete_contained_entities`.
//!
//! A ReadCondition a reader creates triggers while the reader holds a sample
//! in the condition's three state masks (§2.2.2.5.8); a QueryCondition while
//! it holds one in the masks that its expression holds for (§2.2.2.5.9), as
//! `read_w_condition` evaluates it.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::sync::Arc;
use std::time::Duration;

use zerodds_dcps::qos::{HistoryKind, HistoryQosPolicy};
use zerodds_dcps::{
    Condition, DataReaderQos, DdsError, DdsType, DomainParticipantFactory, DomainParticipantQos,
    SubscriberQos, TopicQos, WaitSet, instance_state_mask, sample_state_mask, view_state_mask,
};
use zerodds_dcps::{DecodeError, EncodeError};
use zerodds_sql_filter::Value;

/// A keyless topic is one implicit instance; KEEP_ALL keeps every sample a
/// test pushes.
fn qos_keep_all() -> DataReaderQos {
    DataReaderQos {
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        ..DataReaderQos::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Score {
    id: u32,
    score: i32,
}

impl DdsType for Score {
    const TYPE_NAME: &'static str = "test::ReaderConditionScore";
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = false;

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.id.to_le_bytes());
        out.extend_from_slice(&self.score.to_le_bytes());
        Ok(())
    }

    fn decode(b: &[u8]) -> Result<Self, DecodeError> {
        if b.len() < 8 {
            return Err(DecodeError::Invalid {
                what: "truncated Score",
            });
        }
        Ok(Self {
            id: u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            score: i32::from_le_bytes([b[4], b[5], b[6], b[7]]),
        })
    }

    fn field_value(&self, path: &str) -> Option<Value> {
        match path {
            "id" => Some(Value::Int(i64::from(self.id))),
            "score" => Some(Value::Int(i64::from(self.score))),
            _ => None,
        }
    }
}

fn encode(s: &Score) -> Vec<u8> {
    let mut buf = Vec::new();
    s.encode(&mut buf).unwrap();
    buf
}

fn reader_on(
    domain: i32,
    topic_name: &str,
) -> (
    zerodds_dcps::DomainParticipant,
    zerodds_dcps::DataReader<Score>,
) {
    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant_offline(domain, DomainParticipantQos::default());
    let topic = participant
        .create_topic::<Score>(topic_name, TopicQos::default())
        .unwrap();
    let reader = participant
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Score>(&topic, qos_keep_all())
        .unwrap();
    (participant, reader)
}

#[test]
fn a_read_condition_follows_the_states_of_the_readers_samples() {
    let (_p, reader) = reader_on(193, "ReadConditionStates");
    let not_read = reader
        .create_readcondition(
            sample_state_mask::NOT_READ,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap();
    let read = reader
        .create_readcondition(
            sample_state_mask::READ,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap();
    assert_eq!(
        not_read.get_sample_state_mask(),
        sample_state_mask::NOT_READ
    );
    assert!(!not_read.get_trigger_value(), "nothing received");

    reader
        .__push_raw(encode(&Score { id: 1, score: 5 }))
        .unwrap();
    assert!(
        not_read.get_trigger_value(),
        "a sample received and not yet read is NOT_READ"
    );
    assert!(!read.get_trigger_value());

    assert_eq!(reader.read().unwrap().len(), 1);
    assert!(!not_read.get_trigger_value(), "the one sample is READ now");
    assert!(read.get_trigger_value());

    assert_eq!(reader.take().unwrap().len(), 1);
    assert!(!read.get_trigger_value(), "the reader holds nothing");
}

#[test]
fn a_query_condition_triggers_for_the_samples_its_expression_holds_for() {
    let (_p, reader) = reader_on(194, "QueryConditionTrigger");
    let high = reader
        .create_querycondition(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
            "score > %0",
            vec!["9".into()],
        )
        .unwrap();
    assert_eq!(high.get_query_expression(), "score > %0");

    reader
        .__push_raw(encode(&Score { id: 1, score: 5 }))
        .unwrap();
    // Not yet in the cache: its fields are not known, it counts.
    assert!(high.get_trigger_value());
    assert!(reader.read_w_condition(&high).unwrap().is_empty());
    assert!(
        !high.get_trigger_value(),
        "in the cache now, and 5 is not above 9"
    );

    reader
        .__push_raw(encode(&Score { id: 2, score: 50 }))
        .unwrap();
    let out = reader.read_w_condition(&high).unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].data.score, 50);
    assert!(high.get_trigger_value(), "50 is above 9");

    high.set_query_parameters(vec!["60".into()]).unwrap();
    assert!(!high.get_trigger_value(), "nothing above 60");

    let taken = reader.take_w_condition(&high).unwrap();
    assert!(taken.is_empty());
    assert_eq!(reader.take().unwrap().len(), 2, "both samples stayed");
}

#[test]
fn delete_readcondition_takes_only_the_readers_own_conditions() {
    let (_p, reader) = reader_on(195, "DeleteReadCondition");
    let (_q, other) = reader_on(196, "DeleteReadConditionOther");
    let mine = reader
        .create_readcondition(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap();
    let foreign = other
        .create_readcondition(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap();
    let query = reader
        .create_querycondition(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
            "score > 0",
            Vec::new(),
        )
        .unwrap();

    assert!(matches!(
        reader.delete_readcondition(&foreign),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    reader
        .__push_raw(encode(&Score { id: 1, score: 5 }))
        .unwrap();
    assert!(mine.get_trigger_value());

    reader.delete_readcondition(&mine).unwrap();
    assert!(
        !mine.get_trigger_value(),
        "a deleted condition does not trigger"
    );
    assert!(matches!(
        reader.delete_readcondition(&mine),
        Err(DdsError::PreconditionNotMet { .. })
    ));

    // A QueryCondition goes by its ReadCondition.
    assert!(query.get_trigger_value());
    reader.delete_readcondition(query.base()).unwrap();
    assert!(!query.get_trigger_value());
}

#[test]
fn delete_contained_entities_deletes_every_condition_the_reader_made() {
    let (_p, reader) = reader_on(197, "DeleteContainedConditions");
    let read = reader
        .create_readcondition(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap();
    let query = reader
        .create_querycondition(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
            "score > 0",
            Vec::new(),
        )
        .unwrap();
    reader
        .__push_raw(encode(&Score { id: 1, score: 5 }))
        .unwrap();
    assert!(read.get_trigger_value() && query.get_trigger_value());

    reader.delete_contained_entities().unwrap();
    assert!(!read.get_trigger_value());
    assert!(!query.get_trigger_value());
    assert!(
        reader.delete_readcondition(&read).is_err(),
        "deleted already"
    );
}

#[test]
fn a_waitset_returns_a_read_condition_of_a_reader_with_data() {
    let (_p, reader) = reader_on(198, "ReadConditionWaitSet");
    let condition = reader
        .create_readcondition(
            sample_state_mask::NOT_READ,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap();
    let waitset = WaitSet::new();
    let attached: Arc<dyn Condition> = condition.clone();
    waitset.attach_condition(Arc::clone(&attached)).unwrap();
    assert!(waitset.wait(Duration::from_millis(20)).is_err(), "no data");

    reader
        .__push_raw(encode(&Score { id: 1, score: 5 }))
        .unwrap();
    let ready = waitset.wait(Duration::from_secs(1)).unwrap();
    assert_eq!(ready.len(), 1);
    assert!(Arc::ptr_eq(&ready[0], &attached));
}
