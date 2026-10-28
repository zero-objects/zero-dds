// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Custom content filter classes (DDSI-RTPS 2.5 §8.7.3.4): registered with the
//! participant, named by a ContentFilteredTopic, announced as the filter class
//! of the reader and applied to the samples of a DataReader on the topic.

#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use std::sync::Arc;
use zerodds_dcps::qos::{HistoryKind, HistoryQosPolicy};
use zerodds_dcps::{
    ContentFilter, DataReaderQos, DdsError, DdsType, DecodeError, DomainParticipantFactory,
    DomainParticipantQos, EncodeError, SubscriberQos, TopicQos,
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
    const TYPE_NAME: &'static str = "test::CustomFilterReading";
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

/// A filter class of the application: `"<parameter 0> divides id"`.
struct DividesId;

impl ContentFilter<Reading> for DividesId {
    fn compile(&self, expression: &str, parameters: &[String]) -> Result<(), DdsError> {
        if expression != "divides" {
            return Err(DdsError::BadParameter {
                what: "expression must be `divides`",
            });
        }
        match parameters.first().and_then(|p| p.parse::<u32>().ok()) {
            Some(n) if n > 0 => Ok(()),
            _ => Err(DdsError::BadParameter {
                what: "parameter 0 must be a positive integer",
            }),
        }
    }

    fn evaluate(&self, sample: &Reading, _expression: &str, parameters: &[String]) -> bool {
        parameters
            .first()
            .and_then(|p| p.parse::<u32>().ok())
            .is_some_and(|n| n > 0 && sample.id % n == 0)
    }
}

fn raw(id: u32, score: i32) -> Vec<u8> {
    let mut out = Vec::new();
    Reading { id, score }.encode(&mut out).unwrap();
    out
}

#[test]
fn a_registered_filter_class_is_named_announced_and_applied() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(194, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Reading>("CustomFilterReadings", TopicQos::default())
        .unwrap();
    assert!(!p.lookup_contentfilter("com.example.divides"));
    p.register_contentfilter::<Reading>("com.example.divides", Arc::new(DividesId))
        .unwrap();
    assert!(p.lookup_contentfilter("com.example.divides"));
    let cft = p
        .create_contentfilteredtopic_with_class(
            "Multiples",
            &topic,
            "com.example.divides",
            "divides",
            vec!["3".into()],
        )
        .unwrap();
    let property = cft.content_filter_property();
    assert_eq!(property.filter_class_name, "com.example.divides");
    assert_eq!(property.filter_expression, "divides");
    assert_eq!(property.expression_parameters, vec!["3".to_string()]);

    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader_with_cft(&cft, unbounded_history())
        .unwrap();
    for id in 1..=7 {
        reader.__push_raw(raw(id, 0)).unwrap();
    }
    let ids: Vec<u32> = reader.take().unwrap().iter().map(|s| s.id).collect();
    assert_eq!(ids, vec![3, 6]);

    // New parameters are checked by the class and reach the reader.
    assert!(cft.set_filter_parameters(vec!["0".into()]).is_err());
    cft.set_filter_parameters(vec!["2".into()]).unwrap();
    for id in 8..=11 {
        reader.__push_raw(raw(id, 0)).unwrap();
    }
    let ids: Vec<u32> = reader.take().unwrap().iter().map(|s| s.id).collect();
    assert_eq!(ids, vec![8, 10]);
    // The class checks the expression when the topic is created.
    assert!(
        p.create_contentfilteredtopic_with_class(
            "Bad",
            &topic,
            "com.example.divides",
            "something else",
            vec!["3".into()],
        )
        .is_err()
    );
}

#[test]
fn the_registry_refuses_what_it_cannot_serve() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(195, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Reading>("CustomFilterReadings", TopicQos::default())
        .unwrap();
    // DDSSQL is the implementation's own; an empty or over-long name is no class.
    assert!(
        p.register_contentfilter::<Reading>("DDSSQL", Arc::new(DividesId))
            .is_err()
    );
    assert!(
        p.register_contentfilter::<Reading>("", Arc::new(DividesId))
            .is_err()
    );
    assert!(
        p.register_contentfilter::<Reading>(&"c".repeat(257), Arc::new(DividesId))
            .is_err()
    );
    p.register_contentfilter::<Reading>("com.example.divides", Arc::new(DividesId))
        .unwrap();
    assert!(
        p.register_contentfilter::<Reading>("com.example.divides", Arc::new(DividesId))
            .is_err(),
        "registered twice"
    );
    // A class nobody registered, and DDSSQL by name, which stays the SQL class.
    assert!(
        p.create_contentfilteredtopic_with_class("A", &topic, "com.example.none", "x", Vec::new())
            .is_err()
    );
    let sql = p
        .create_contentfilteredtopic_with_class(
            "B",
            &topic,
            "DDSSQL",
            "score > %0",
            vec!["1".into()],
        )
        .unwrap();
    assert_eq!(sql.content_filter_property().filter_class_name, "DDSSQL");
    // Registered for `Reading`: not usable for another type.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Other;
    impl DdsType for Other {
        const TYPE_NAME: &'static str = "test::CustomFilterOther";
        const XCDR1_MATCHES_XCDR2: bool = true;
        const HAS_KEY: bool = false;
        fn encode(&self, _out: &mut Vec<u8>) -> Result<(), EncodeError> {
            Ok(())
        }
        fn decode(_b: &[u8]) -> Result<Self, DecodeError> {
            Ok(Self)
        }
    }
    let other = p
        .create_topic::<Other>("CustomFilterOther", TopicQos::default())
        .unwrap();
    assert!(
        p.create_contentfilteredtopic_with_class(
            "C",
            &other,
            "com.example.divides",
            "divides",
            vec!["3".into()],
        )
        .is_err()
    );
    // Unregistering ends the name, not the topics created with it.
    let cft = p
        .create_contentfilteredtopic_with_class(
            "D",
            &topic,
            "com.example.divides",
            "divides",
            vec!["3".into()],
        )
        .unwrap();
    p.unregister_contentfilter("com.example.divides").unwrap();
    assert!(!p.lookup_contentfilter("com.example.divides"));
    assert!(p.unregister_contentfilter("com.example.divides").is_err());
    assert_eq!(
        cft.content_filter_property().filter_class_name,
        "com.example.divides"
    );
}
