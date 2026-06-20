// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! E2E: a participant configured with `RuntimeConfig.user_transports` runs its
//! user traffic over a `LayeredUserTransport`. Two such participants discover
//! each other and exchange a sample end-to-end — proving the layered transport
//! path works in the live runtime, not just in isolation.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::time::Duration;

use zerodds_dcps::runtime::{RuntimeConfig, UserTransportKind};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
    RawBytes, SubscriberQos, TopicQos,
};

fn layered_cfg() -> RuntimeConfig {
    RuntimeConfig {
        tick_period: Duration::from_millis(20),
        spdp_period: Duration::from_millis(100),
        // Preference-ordered multi-transport. Both legs are UDPv4 kinds here so
        // the test needs no extra OS setup; the layering machinery (build,
        // route, multiplex, advertise) is exercised regardless.
        user_transports: vec![UserTransportKind::UdpV4],
        ..RuntimeConfig::default()
    }
}

#[test]
fn two_layered_participants_exchange_a_sample() {
    let factory = DomainParticipantFactory::instance();
    // Unique domain to avoid colliding with other concurrent tests.
    let domain = 41;

    let pub_p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), layered_cfg())
        .expect("pub participant over layered transport");
    let sub_p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), layered_cfg())
        .expect("sub participant over layered transport");

    let topic_p = pub_p
        .create_topic::<RawBytes>("LayeredChatter", TopicQos::default())
        .unwrap();
    let topic_s = sub_p
        .create_topic::<RawBytes>("LayeredChatter", TopicQos::default())
        .unwrap();

    let writer = pub_p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic_p, DataWriterQos::default())
        .unwrap();
    let reader = sub_p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic_s, DataReaderQos::default())
        .unwrap();

    writer
        .wait_for_matched_subscription(1, Duration::from_secs(5))
        .expect("writer matches subscriber over layered transport");
    reader
        .wait_for_matched_publication(1, Duration::from_secs(5))
        .expect("reader matches publisher over layered transport");

    writer
        .write(&RawBytes::new(b"layered-hello".to_vec()))
        .unwrap();
    reader
        .wait_for_data(Duration::from_secs(3))
        .expect("sample arrives over layered transport");

    let samples = reader.take().unwrap();
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].data, b"layered-hello");
}
