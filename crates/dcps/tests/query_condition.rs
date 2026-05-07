//! T7 — QueryCondition SQL-Per-Sample-Eval (Spec §2.2.2.5.9.6).
//!
//! Verifiziert, dass `read_w_condition` / `take_w_condition` den
//! gespeicherten SQL-Filter pro Sample auswerten und nur Samples
//! liefern, die `true` zurueckgeben.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::sync::Arc;

use zerodds_dcps::{
    DataReaderQos, DdsType, DomainParticipantFactory, DomainParticipantQos, QueryCondition,
    ReadCondition, SubscriberQos, TopicQos, instance_state_mask, sample_state_mask,
    view_state_mask,
};
use zerodds_dcps::{DecodeError, EncodeError};
use zerodds_sql_filter::Value;

/// Test-Fixture: keyed Topic mit Feldern `id: u32` und `score: i32`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Sample {
    id: u32,
    score: i32,
}

impl DdsType for Sample {
    const TYPE_NAME: &'static str = "test::SampleQC";
    const HAS_KEY: bool = false;

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.id.to_le_bytes());
        out.extend_from_slice(&self.score.to_le_bytes());
        Ok(())
    }
    fn decode(b: &[u8]) -> Result<Self, DecodeError> {
        if b.len() < 8 {
            return Err(DecodeError::Invalid {
                what: "truncated SampleQC",
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

fn make_qc(expr: &str, params: Vec<String>) -> Arc<QueryCondition> {
    let base = ReadCondition::new(
        sample_state_mask::ANY,
        view_state_mask::ANY,
        instance_state_mask::ANY,
        |_, _, _| true,
    );
    QueryCondition::new(base, expr, params).expect("parse")
}

fn encode(s: &Sample) -> Vec<u8> {
    let mut buf = Vec::new();
    s.encode(&mut buf).unwrap();
    buf
}

#[test]
fn read_w_condition_filters_per_sample() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(190, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Sample>("QcTopic", TopicQos::default())
        .unwrap();
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<Sample>(&topic, DataReaderQos::default())
        .unwrap();

    reader
        .__push_raw(encode(&Sample { id: 1, score: 5 }))
        .unwrap();
    reader
        .__push_raw(encode(&Sample { id: 2, score: 50 }))
        .unwrap();
    reader
        .__push_raw(encode(&Sample { id: 3, score: 10 }))
        .unwrap();

    let qc = make_qc("score > 9", Vec::new());
    let out = reader.read_w_condition(&qc).expect("read_w_condition");
    assert_eq!(out.len(), 2);
    let scores: Vec<i32> = out.iter().map(|s| s.data.score).collect();
    assert!(scores.contains(&50));
    assert!(scores.contains(&10));
    assert!(!scores.contains(&5));
}

#[test]
fn take_w_condition_consumes_only_matching_samples() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(191, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Sample>("QcTopic", TopicQos::default())
        .unwrap();
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<Sample>(&topic, DataReaderQos::default())
        .unwrap();

    reader
        .__push_raw(encode(&Sample { id: 1, score: 5 }))
        .unwrap();
    reader
        .__push_raw(encode(&Sample { id: 2, score: 50 }))
        .unwrap();
    reader
        .__push_raw(encode(&Sample { id: 3, score: 99 }))
        .unwrap();

    let qc = make_qc("score > 10", Vec::new());
    let out = reader.take_w_condition(&qc).expect("take_w_condition");
    assert_eq!(out.len(), 2);

    // Verbleibend: das score=5-Sample (matchte nicht) muss noch im
    // Cache stehen — gegen take_with_info pruefen, das aus dem Cache
    // (nicht aus der Inbox) konsumiert.
    let remaining = reader.take_with_info().unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].data.score, 5);
}

#[test]
fn read_w_condition_with_string_parameter() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(192, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Sample>("QcTopic", TopicQos::default())
        .unwrap();
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<Sample>(&topic, DataReaderQos::default())
        .unwrap();

    reader
        .__push_raw(encode(&Sample { id: 1, score: 5 }))
        .unwrap();
    reader
        .__push_raw(encode(&Sample { id: 2, score: 42 }))
        .unwrap();

    // String-Param "42" wird in Value::String gewrappt; gegen Int(42)
    // ist der Vergleich Type-Mismatch -> evaluate liefert Err, Sample
    // wird abgelehnt.
    let qc = make_qc("score = %0", vec!["42".into()]);
    let out = reader.read_w_condition(&qc).expect("read");
    assert!(out.is_empty(), "string-param != int → keine Matches");
}

#[test]
fn read_w_condition_preserves_cache_state() {
    // read_w_condition darf Samples nicht entfernen.
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(193, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Sample>("QcTopic", TopicQos::default())
        .unwrap();
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<Sample>(&topic, DataReaderQos::default())
        .unwrap();

    reader
        .__push_raw(encode(&Sample { id: 1, score: 5 }))
        .unwrap();
    reader
        .__push_raw(encode(&Sample { id: 2, score: 50 }))
        .unwrap();

    let qc = make_qc("score > 0", Vec::new());
    let r1 = reader.read_w_condition(&qc).unwrap();
    let r2 = reader.read_w_condition(&qc).unwrap();
    assert_eq!(r1.len(), 2);
    assert_eq!(r2.len(), 2, "second read sollte denselben Cache sehen");
}

#[test]
fn take_w_condition_empty_when_no_match() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(194, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Sample>("QcTopic", TopicQos::default())
        .unwrap();
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<Sample>(&topic, DataReaderQos::default())
        .unwrap();

    reader
        .__push_raw(encode(&Sample { id: 1, score: 5 }))
        .unwrap();

    let qc = make_qc("score > 99", Vec::new());
    let out = reader.take_w_condition(&qc).expect("take");
    assert!(out.is_empty());

    // Das Sample muss noch im Cache sein:
    let remaining = reader.take_with_info().unwrap();
    assert_eq!(remaining.len(), 1);
}

#[test]
fn read_w_condition_unknown_field_drops_sample() {
    // Wird ein Field referenziert, das DdsType::field_value nicht
    // kennt, evaluiert der Filter zu Err(UnknownField) -> Sample
    // wird abgelehnt (kein hard error nach oben).
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(195, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Sample>("QcTopic", TopicQos::default())
        .unwrap();
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<Sample>(&topic, DataReaderQos::default())
        .unwrap();

    reader
        .__push_raw(encode(&Sample { id: 1, score: 5 }))
        .unwrap();

    let qc = make_qc("nonexistent > 0", Vec::new());
    let out = reader.read_w_condition(&qc).expect("read");
    assert!(out.is_empty());
}
