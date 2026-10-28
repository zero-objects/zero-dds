// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A DataReader on a ContentFilteredTopic (DDS 1.4 §2.2.2.3.3,
//! §2.2.2.5.2.5): only samples the expression accepts are delivered, with
//! the parameters current at delivery.

#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use zerodds_dcps::qos::{HistoryKind, HistoryQosPolicy};
use zerodds_dcps::{
    DataReaderQos, DdsType, DecodeError, DomainParticipantFactory, DomainParticipantQos,
    EncodeError, SubscriberQos, TopicQos,
};
use zerodds_sql_filter::Value;

/// KEEP_ALL: `a_reader_on_a_content_filtered_topic_delivers_what_the_filter_accepts`
/// pushes several samples to this keyless (single-instance) topic and
/// asserts on more than the single newest of them — the default
/// KEEP_LAST(1) would (correctly) evict the rest.
fn unbounded_history() -> DataReaderQos {
    DataReaderQos {
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        ..DataReaderQos::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Reading {
    id: u32,
    score: i32,
}

impl DdsType for Reading {
    const TYPE_NAME: &'static str = "test::CftReading";
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = false;

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.id.to_le_bytes());
        out.extend_from_slice(&self.score.to_le_bytes());
        Ok(())
    }
    fn decode(b: &[u8]) -> Result<Self, DecodeError> {
        if b.len() < 8 {
            return Err(DecodeError::Invalid { what: "truncated" });
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

fn raw(id: u32, score: i32) -> Vec<u8> {
    let mut out = Vec::new();
    Reading { id, score }.encode(&mut out).unwrap();
    out
}

#[test]
fn a_reader_on_a_content_filtered_topic_delivers_what_the_filter_accepts() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(192, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Reading>("CftReadings", TopicQos::default())
        .unwrap();
    let cft = p
        .create_contentfilteredtopic("HighScores", &topic, "score > %0", vec!["9".into()])
        .unwrap();
    let property = cft.content_filter_property();
    assert_eq!(property.related_topic_name, "CftReadings");
    assert_eq!(property.filter_class_name, "DDSSQL");
    assert_eq!(property.expression_parameters, vec!["9".to_string()]);

    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader_with_cft(&cft, unbounded_history())
        .unwrap();
    for (id, score) in [(1, 5), (2, 50), (3, 10)] {
        reader.__push_raw(raw(id, score)).unwrap();
    }
    let ids: Vec<u32> = reader.take().unwrap().iter().map(|s| s.id).collect();
    assert_eq!(ids, vec![2, 3]);

    // New parameters reach the reader that already exists.
    cft.set_filter_parameters(vec!["40".into()]).unwrap();
    for (id, score) in [(4, 45), (5, 20)] {
        reader.__push_raw(raw(id, score)).unwrap();
    }
    let ids: Vec<u32> = reader.take().unwrap().iter().map(|s| s.id).collect();
    assert_eq!(ids, vec![4]);
}

#[test]
fn a_field_the_type_does_not_answer_delivers_nothing() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(193, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Reading>("CftReadings", TopicQos::default())
        .unwrap();
    let cft = p
        .create_contentfilteredtopic("Unknown", &topic, "missing = 1", Vec::new())
        .unwrap();
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader_with_cft(&cft, DataReaderQos::default())
        .unwrap();
    reader.__push_raw(raw(1, 1)).unwrap();
    assert!(reader.take().unwrap().is_empty());
}

/// DDS 1.4 §2.2.2.5.2.5: a TopicDescription of another DomainParticipant gives
/// no reader ("the operation will fail and return a nil result"); the
/// ContentFilteredTopic keeps no hold from the failed call.
#[test]
fn a_content_filtered_topic_of_another_participant_creates_no_reader() {
    let owner = DomainParticipantFactory::instance()
        .create_participant_offline(193, DomainParticipantQos::default());
    let stranger = DomainParticipantFactory::instance()
        .create_participant_offline(194, DomainParticipantQos::default());
    let topic = owner
        .create_topic::<Reading>("CftForeign", TopicQos::default())
        .unwrap();
    let cft = owner
        .create_contentfilteredtopic("ForeignScores", &topic, "score > 1", Vec::new())
        .unwrap();
    let result = stranger
        .create_subscriber(SubscriberQos::default())
        .create_datareader_with_cft(&cft, unbounded_history());
    assert!(
        matches!(
            result,
            Err(zerodds_dcps::DdsError::PreconditionNotMet { .. })
        ),
        "{:?}",
        result.err()
    );
    // no reader holds the description: the owner may delete it
    owner
        .delete_contentfilteredtopic(&cft)
        .expect("nothing holds the ContentFilteredTopic");
}
