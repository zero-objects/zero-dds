// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Content filtering on the writer side (DDSI-RTPS 2.5 §8.7.3): a
//! DataWriter applies the DDSSQL filter a matched remote reader announces
//! in its subscription and does not send that reader the samples that do
//! not pass.
//!
//! The reader here is a runtime reader that announces a filter and has
//! none of its own, so what reaches it is what the writer sent as data.

#![cfg(all(target_os = "linux", feature = "std"))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

#[path = "common/mod.rs"]
mod common;

use std::time::{Duration, Instant};

use zerodds_dcps::qos::{HistoryKind, HistoryQosPolicy, ReliabilityKind, ReliabilityQosPolicy};
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample};
use zerodds_dcps::{
    DataWriterQos, DdsType, DecodeError, DomainParticipantFactory, DomainParticipantQos,
    EncodeError, PublisherQos, TopicQos,
};
use zerodds_rtps::subscription_data::ContentFilterProperty;
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_sql_filter::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Reading {
    id: u32,
    score: i32,
}

impl DdsType for Reading {
    const TYPE_NAME: &'static str = "test::WriterFilteredReading";
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

fn filter(expression: &str, parameters: &[&str]) -> ContentFilterProperty {
    ContentFilterProperty {
        content_filtered_topic_name: "HighReadings".into(),
        related_topic_name: "WriterFilteredReadings".into(),
        filter_class_name: "DDSSQL".into(),
        filter_expression: expression.into(),
        expression_parameters: parameters.iter().map(|p| (*p).into()).collect(),
    }
}

/// A typed DataWriter in one participant and, in a runtime of its own, a
/// reader that announces `announced` and filters nothing itself.
struct Pair {
    writer: zerodds_dcps::DataWriter<Reading>,
    reader_runtime: common::ShutsDown,
    reader: zerodds_rtps::wire_types::EntityId,
    rx: std::sync::mpsc::Receiver<UserSample>,
    _participant: zerodds_dcps::DomainParticipant,
}

fn pair(announced: Option<ContentFilterProperty>) -> Pair {
    let domain = common::unique_domain();
    let participant = DomainParticipantFactory::instance()
        .create_participant(domain, DomainParticipantQos::default())
        .unwrap();
    let topic = participant
        .create_topic::<Reading>("WriterFilteredReadings", TopicQos::default())
        .unwrap();
    let writer = participant
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Reading>(
            &topic,
            DataWriterQos {
                reliability: ReliabilityQosPolicy {
                    kind: ReliabilityKind::Reliable,
                    ..DataWriterQos::default().reliability
                },
                history: HistoryQosPolicy {
                    kind: HistoryKind::KeepAll,
                    depth: 1,
                },
                ..DataWriterQos::default()
            },
        )
        .unwrap();
    let reader_runtime = common::ShutsDown(
        DcpsRuntime::start(
            domain,
            GuidPrefix::from_bytes([0xC7; 12]),
            RuntimeConfig::default(),
        )
        .unwrap(),
    );
    let (reader, rx) = reader_runtime
        .register_user_reader_kind(
            UserReaderConfig {
                topic_name: "WriterFilteredReadings".into(),
                type_name: Reading::TYPE_NAME.into(),
                content_filter: announced,
                reliable: true,
                ..Default::default()
            },
            false,
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while writer
        .get_publication_matched_status()
        .unwrap()
        .current_count
        == 0
    {
        assert!(Instant::now() < deadline, "no match");
        std::thread::sleep(Duration::from_millis(20));
    }
    Pair {
        writer,
        reader_runtime,
        reader,
        rx,
        _participant: participant,
    }
}

impl Pair {
    /// Writes the (id, score) of `written` and gives the ids that arrive
    /// until the last one does; the last passes every filter of these
    /// tests.
    fn ids_received(&self, written: &[(u32, i32)]) -> Vec<u32> {
        for &(id, score) in written {
            self.writer.write(&Reading { id, score }).unwrap();
        }
        let last = written.last().unwrap().0;
        let mut ids = Vec::new();
        let end = Instant::now() + Duration::from_secs(10);
        while Instant::now() < end && ids.last() != Some(&last) {
            if let Ok(UserSample::Alive { payload, .. }) =
                self.rx.recv_timeout(Duration::from_millis(100))
            {
                ids.push(Reading::decode(payload.as_ref()).unwrap().id);
            }
        }
        ids
    }
}

/// The ids of the samples that reach a runtime reader announcing
/// `announced`, of the `written` (id, score) a typed DataWriter writes.
fn ids_received(announced: Option<ContentFilterProperty>, written: &[(u32, i32)]) -> Vec<u32> {
    pair(announced).ids_received(written)
}

const WRITTEN: &[(u32, i32)] = &[(1, 5), (2, 50), (3, 3), (4, 4), (5, 10), (6, 1), (7, 99)];

/// §8.7.3.3: "a sample is not sent if it does not pass the remote Reader's
/// filter"; the samples that pass arrive in order, the reader misses none.
#[test]
fn rtps_8_7_3_3_a_writer_sends_a_filtered_reader_only_what_passes() {
    assert_eq!(
        ids_received(Some(filter("score > %0", &["9"])), WRITTEN),
        vec![2, 5, 7]
    );
}

/// §8.7.3.4: without a filter in the subscription the writer filters
/// nothing.
#[test]
fn rtps_8_7_3_4_a_reader_without_a_filter_gets_every_sample() {
    assert_eq!(ids_received(None, WRITTEN), vec![1, 2, 3, 4, 5, 6, 7]);
}

/// §8.7.3.1: "In order for the Writer to apply the filter, it must have
/// been configured to handle filters of the specified filter class. If
/// not, the Writer will simply ignore the filter information and not
/// filter any data samples." The same holds for an expression about a
/// field the writer's type does not have.
#[test]
fn rtps_8_7_3_1_a_filter_the_writer_cannot_apply_is_left_to_the_reader() {
    let mut other_class = filter("score > %0", &["9"]);
    other_class.filter_class_name = "Vendor".into();
    assert_eq!(
        ids_received(Some(other_class), WRITTEN),
        vec![1, 2, 3, 4, 5, 6, 7]
    );
    assert_eq!(
        ids_received(Some(filter("missing > 9", &[])), WRITTEN),
        vec![1, 2, 3, 4, 5, 6, 7]
    );
}

/// §8.7.3.1: "Each time the parameters are modified, the updated
/// information is exchanged using the Endpoint discovery protocol"; the
/// writer then applies the filter with the new parameters.
#[test]
fn rtps_8_7_3_1_changed_parameters_reach_the_writers_filter() {
    let pair = pair(Some(filter("score > %0", &["9"])));
    assert_eq!(pair.ids_received(&[(1, 20), (2, 5), (3, 99)]), vec![1, 3]);
    pair.reader_runtime
        .update_user_reader_content_filter(pair.reader, Some(filter("score > %0", &["40"])))
        .unwrap();
    // The announcement is on its way: a score of 20 passes until it is in.
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut id = 10;
    loop {
        let got = pair.ids_received(&[(id, 20), (id + 1, 99)]);
        if got == vec![id + 1] {
            break;
        }
        assert_eq!(got, vec![id, id + 1], "the filter with the old parameter");
        assert!(Instant::now() < deadline, "the new parameter never applied");
        id += 2;
        std::thread::sleep(Duration::from_millis(20));
    }
}
