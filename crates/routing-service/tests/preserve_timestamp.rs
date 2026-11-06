// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A route with `preserve_source_timestamp: true` writes each sample with
//! the source timestamp it came in with.
//!
//! The writer on the input domain stamps its samples with `write_w_timestamp`
//! (DDS 1.4 §2.2.2.4.2.12) at a time far before the run; the reader on the
//! output domain gets that time in `SampleInfo.source_timestamp`
//! (§2.2.2.5.5). Without the option the router writes with its own clock,
//! which cannot be that time. The time is a whole second plus one half: the
//! RTPS `Time_t` carries the fraction in units of 2^-32 s (DDSI-RTPS 2.5
//! §9.3.2), and a half second is exact in both forms.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::time::{Duration, Instant};

use zerodds_dcps::{
    DomainParticipantFactory, DomainParticipantQos, PublisherQos, RawBytes, SubscriberQos, Time,
    TopicQos,
};
use zerodds_routing_service::{Router, RouterConfig};

/// 2001-09-09T01:46:40.5Z.
const STAMP: Time = Time {
    sec: 1_000_000_000,
    nanosec: 500_000_000,
};

#[test]
fn dcps_2_2_2_5_5_a_preserving_route_keeps_the_source_timestamp_of_the_input() {
    let input = zerodds_test_domains::unique_domain();
    let output = zerodds_test_domains::unique_domain();
    let cfg = RouterConfig::from_json(&format!(
        r#"{{ "name": "preserve-ts", "routes": [ {{
              "name": "sensor",
              "input":  {{ "domain": {input}, "topic": "SensorIn", "type_name": "zerodds::RawBytes" }},
              "output": {{ "domain": {output}, "topic": "SensorOut", "type_name": "zerodds::RawBytes" }},
              "preserve_source_timestamp": true }} ] }}"#
    ))
    .unwrap();
    let router = Router::start(&cfg).unwrap();

    let factory = DomainParticipantFactory::instance();
    let downstream = factory
        .create_participant(output, DomainParticipantQos::default())
        .unwrap();
    let topic = downstream
        .create_topic::<RawBytes>("SensorOut", TopicQos::default())
        .unwrap();
    let reader = downstream
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, common::reader_qos())
        .unwrap();
    let upstream = factory
        .create_participant(input, DomainParticipantQos::default())
        .unwrap();
    let topic = upstream
        .create_topic::<RawBytes>("SensorIn", TopicQos::default())
        .unwrap();
    let writer = upstream
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, common::writer_qos())
        .unwrap();
    writer
        .wait_for_matched_subscription(1, Duration::from_secs(15))
        .expect("the router's input reader matches");
    reader
        .wait_for_matched_publication(1, Duration::from_secs(15))
        .expect("the router's output writer matches");

    let bodies: Vec<Vec<u8>> = (0u8..3).map(|i| vec![i, 0xC0, 0xDE]).collect();
    for body in &bodies {
        writer
            .write_w_timestamp(&RawBytes::new(body.clone()), STAMP)
            .unwrap();
    }

    let mut got = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(15);
    while got.len() < bodies.len() {
        let left = deadline.saturating_duration_since(Instant::now());
        assert!(
            reader.wait_for_data(left).is_ok(),
            "the routed samples did not arrive: {got:?}"
        );
        for s in reader.take_with_info().unwrap() {
            if s.info.valid_data {
                got.push((s.data.data, s.info.source_timestamp));
            }
        }
    }
    let want: Vec<(Vec<u8>, Time)> = bodies.into_iter().map(|b| (b, STAMP)).collect();
    assert_eq!(
        got, want,
        "each body once, in order, with the writer's time"
    );
    assert_eq!(router.route_metrics("sensor").unwrap().errors, 0);
}
