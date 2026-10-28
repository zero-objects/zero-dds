//! T7 — QueryCondition SQL per-sample eval (Spec §2.2.2.5.9.6).
//!
//! Verifies that `read_w_condition` / `take_w_condition` evaluate the
//! stored SQL filter per sample and only return samples that
//! return `true`.

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

use zerodds_dcps::qos::{HistoryKind, HistoryQosPolicy};
use zerodds_dcps::{
    DataReaderQos, DdsType, DomainParticipantFactory, DomainParticipantQos, QueryCondition,
    ReadCondition, SubscriberQos, TopicQos, instance_state_mask, sample_state_mask,
    view_state_mask,
};
use zerodds_dcps::{DecodeError, EncodeError};
use zerodds_sql_filter::Value;

/// This file pushes several independent keyless samples per test and takes
/// none of them until the assertions run — DDS 1.4 §2.2.2.5.1.10 treats a
/// keyless topic as one implicit instance, so the default HISTORY
/// (`KEEP_LAST(1)`, §2.2.3.18) would keep only the very last one and starve
/// every assertion here that expects more than one. None of these tests are
/// about HISTORY, so `KEEP_ALL` takes it out of the way entirely.
fn qos_keep_all() -> DataReaderQos {
    DataReaderQos {
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        ..DataReaderQos::default()
    }
}

/// Test fixture: keyed topic with fields `id: u32` and `score: i32`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Sample {
    id: u32,
    score: i32,
}

impl DdsType for Sample {
    const TYPE_NAME: &'static str = "test::SampleQC";
    /// @final with no member that XCDR1 and XCDR2 frame differently: one stream.
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

fn make_qc(
    reader: &zerodds_dcps::subscriber::DataReader<Sample>,
    expr: &str,
    params: Vec<String>,
) -> Arc<QueryCondition> {
    reader
        .create_querycondition(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
            expr,
            params,
        )
        .expect("parse")
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
        .create_datareader::<Sample>(&topic, qos_keep_all())
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

    let qc = make_qc(&reader, "score > 9", Vec::new());
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
        .create_datareader::<Sample>(&topic, qos_keep_all())
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

    let qc = make_qc(&reader, "score > 10", Vec::new());
    let out = reader.take_w_condition(&qc).expect("take_w_condition");
    assert_eq!(out.len(), 2);

    // Remaining: the score=5 sample (did not match) must still be in
    // the cache — check against take_with_info, which consumes from the
    // cache (not from the inbox).
    let remaining = reader.take_with_info().unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].data.score, 5);
}

#[test]
fn read_w_condition_reads_a_parameter_as_its_literal() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(192, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Sample>("QcTopic", TopicQos::default())
        .unwrap();
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<Sample>(&topic, qos_keep_all())
        .unwrap();

    reader
        .__push_raw(encode(&Sample { id: 1, score: 5 }))
        .unwrap();
    reader
        .__push_raw(encode(&Sample { id: 2, score: 42 }))
        .unwrap();

    // DDS 1.4 Annex B: a parameter is a string holding a literal. "42" is
    // the integer 42 and matches; "'42'" is the text 42, which an integer
    // field does not compare with, so nothing matches.
    let qc = make_qc(&reader, "score = %0", vec!["42".into()]);
    let out = reader.read_w_condition(&qc).expect("read");
    let ids: Vec<u32> = out.iter().map(|s| s.data.id).collect();
    assert_eq!(ids, vec![2]);
    let qc = make_qc(&reader, "score = %0", vec!["'42'".into()]);
    assert!(reader.read_w_condition(&qc).expect("read").is_empty());
}

#[test]
fn read_w_condition_preserves_cache_state() {
    // read_w_condition must not remove samples.
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(193, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Sample>("QcTopic", TopicQos::default())
        .unwrap();
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<Sample>(&topic, qos_keep_all())
        .unwrap();

    reader
        .__push_raw(encode(&Sample { id: 1, score: 5 }))
        .unwrap();
    reader
        .__push_raw(encode(&Sample { id: 2, score: 50 }))
        .unwrap();

    let qc = make_qc(&reader, "score > 0", Vec::new());
    let r1 = reader.read_w_condition(&qc).unwrap();
    let r2 = reader.read_w_condition(&qc).unwrap();
    assert_eq!(r1.len(), 2);
    assert_eq!(r2.len(), 2, "second read should see the same cache");
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
        .create_datareader::<Sample>(&topic, qos_keep_all())
        .unwrap();

    reader
        .__push_raw(encode(&Sample { id: 1, score: 5 }))
        .unwrap();

    let qc = make_qc(&reader, "score > 99", Vec::new());
    let out = reader.take_w_condition(&qc).expect("take");
    assert!(out.is_empty());

    // The sample must still be in the cache:
    let remaining = reader.take_with_info().unwrap();
    assert_eq!(remaining.len(), 1);
}

#[test]
fn read_w_condition_unknown_field_drops_sample() {
    // If a field is referenced that DdsType::field_value does not
    // know, the filter evaluates to Err(UnknownField) -> the sample
    // is rejected (no hard error propagated up).
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(195, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Sample>("QcTopic", TopicQos::default())
        .unwrap();
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<Sample>(&topic, qos_keep_all())
        .unwrap();

    reader
        .__push_raw(encode(&Sample { id: 1, score: 5 }))
        .unwrap();

    let qc = make_qc(&reader, "nonexistent > 0", Vec::new());
    let out = reader.read_w_condition(&qc).expect("read");
    assert!(out.is_empty());
}

/// Annex B.2 `QueryExpression ::= {Condition} {'ORDER BY' (FIELDNAME // ',')}`:
/// the samples a read or take returns are ordered by the fields, ascending,
/// the first field first; a condition before `ORDER BY` still selects.
#[test]
fn order_by_orders_what_a_read_or_take_returns() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(191, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Sample>("QcOrderBy", TopicQos::default())
        .unwrap();
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<Sample>(&topic, qos_keep_all())
        .unwrap();
    for (id, score) in [(1, 30), (2, 10), (3, 20), (4, 10)] {
        reader.__push_raw(encode(&Sample { id, score })).unwrap();
    }

    // Only ordering: by score, then by id.
    let qc = make_qc(&reader, "ORDER BY score, id", Vec::new());
    assert_eq!(qc.order_by(), ["score".to_string(), "id".to_string()]);
    let read: Vec<u32> = reader
        .read_w_condition(&qc)
        .unwrap()
        .iter()
        .map(|s| s.data.id)
        .collect();
    assert_eq!(read, vec![2, 4, 3, 1]);

    // A condition and an order; take removes what it returned.
    let qc = make_qc(&reader, "score >= %0 ORDER BY id", vec!["20".into()]);
    let taken: Vec<u32> = reader
        .take_w_condition(&qc)
        .unwrap()
        .iter()
        .map(|s| s.data.id)
        .collect();
    assert_eq!(taken, vec![1, 3]);
    let left: Vec<u32> = reader.take().unwrap().iter().map(|s| s.id).collect();
    assert_eq!(left.len(), 2, "the two with score 10 stay");
}

/// An `ORDER BY` without a field is not a QueryExpression.
#[test]
fn an_order_by_without_a_field_is_refused() {
    let base = ReadCondition::new(
        sample_state_mask::ANY,
        view_state_mask::ANY,
        instance_state_mask::ANY,
        |_, _, _| true,
    );
    assert!(QueryCondition::new(base, "score > 1 ORDER BY", Vec::new()).is_err());
}

/// §2.2.2.5.3.8: `read_w_condition` and `take_w_condition` return at most
/// max_samples. The limit comes after the `ORDER BY`, and only the samples
/// returned become READ or are taken; none left is NO_DATA.
#[test]
fn max_samples_limits_what_a_condition_read_or_take_returns() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(192, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Sample>("QcMaxSamples", TopicQos::default())
        .unwrap();
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<Sample>(&topic, qos_keep_all())
        .unwrap();
    for (id, score) in [(1, 30), (2, 10), (3, 20), (4, 10)] {
        reader.__push_raw(encode(&Sample { id, score })).unwrap();
    }
    let qc = make_qc(&reader, "score >= 10 ORDER BY score, id", Vec::new());

    // The first two of the order: 2 and 4 (score 10).
    let first: Vec<u32> = reader
        .read_w_condition_selected(&qc, 2)
        .unwrap()
        .iter()
        .map(|s| s.data.id)
        .collect();
    assert_eq!(first, vec![2, 4]);
    // Only those two became READ.
    let not_read = reader
        .read_filtered(
            sample_state_mask::NOT_READ,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap();
    let mut ids: Vec<u32> = not_read.iter().map(|s| s.data.id).collect();
    ids.sort_unstable();
    assert_eq!(ids, vec![1, 3]);

    // take takes the first one only.
    let taken = reader.take_w_condition_selected(&qc, 1).unwrap();
    assert_eq!(taken.len(), 1);
    assert_eq!(taken[0].data.id, 2);
    let rest = reader.take_w_condition_selected(&qc, -1).unwrap();
    assert_eq!(rest.len(), 3, "the other three stay until taken");

    assert!(matches!(
        reader.take_w_condition_selected(&qc, -1),
        Err(zerodds_dcps::DdsError::NoData)
    ));
    assert!(matches!(
        reader.read_w_condition_selected(&qc, -2),
        Err(zerodds_dcps::DdsError::BadParameter { .. })
    ));
}
