// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DCPSTopic against Cyclone DDS's SEDP topics endpoints (DDSI-RTPS 2.5
//! §8.5.4.2), both directions. Ignored: each needs a Cyclone DDS built with
//! `DDS_HAS_TOPIC_DISCOVERY` and run with
//! `Discovery/EnableTopicDiscoveryEndpoints` on the domain named by
//! `ZERODDS_TOPIC_PROBE_DOMAIN`.
//!
//! * [`a_cyclone_topic_reaches_dcps_topic`] reads the topic the Cyclone
//!   side creates with the QoS below.
//! * [`a_zerodds_topic_is_announced`] creates a topic and holds it for
//!   `ZERODDS_TOPIC_PROBE_SECS` seconds, for a Cyclone `DCPSTopic` reader to
//!   look for.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stdout,
    missing_docs
)]

use std::time::{Duration, Instant};

use zerodds_dcps::{
    DcpsTopicBuiltinTopicData, DomainParticipantFactory, DomainParticipantQos, RawBytes,
    TOPIC_NAME_DCPS_TOPIC, TopicQos,
};
use zerodds_qos::{
    DurabilityKind, Duration as QosDuration, HistoryKind, HistoryQosPolicy, OwnershipKind,
    ReliabilityKind,
};

fn probe_domain() -> i32 {
    std::env::var("ZERODDS_TOPIC_PROBE_DOMAIN")
        .expect("ZERODDS_TOPIC_PROBE_DOMAIN names the domain the Cyclone side runs on")
        .parse()
        .expect("ZERODDS_TOPIC_PROBE_DOMAIN is a domain id")
}

/// The Cyclone side creates `TopicProbe` of type `tp::Probe` with
/// TRANSIENT_LOCAL, KEEP_LAST(7), RESOURCE_LIMITS 100/10/10, DEADLINE 2 s,
/// LATENCY_BUDGET 5 ms, TRANSPORT_PRIORITY 3, LIFESPAN 30 s,
/// RELIABLE(100 ms), EXCLUSIVE and TOPIC_DATA `abc`, and no endpoint: every
/// member below can only have come over the topics endpoints.
#[test]
#[ignore = "needs a Cyclone DDS with topic discovery on ZERODDS_TOPIC_PROBE_DOMAIN"]
fn a_cyclone_topic_reaches_dcps_topic() {
    let participant = DomainParticipantFactory::instance()
        .create_participant(probe_domain(), DomainParticipantQos::default())
        .expect("participant");
    let reader = participant
        .get_builtin_subscriber()
        .lookup_datareader::<DcpsTopicBuiltinTopicData>(TOPIC_NAME_DCPS_TOPIC)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let sample = loop {
        if let Some(s) = reader
            .read_with_info()
            .unwrap()
            .into_iter()
            .find(|s| s.info.valid_data && s.data.name == "TopicProbe")
        {
            break s.data;
        }
        assert!(
            Instant::now() < deadline,
            "no DCPSTopic sample for Cyclone's TopicProbe within 15 s"
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    println!("{sample:?}");
    assert_eq!(sample.type_name, "tp::Probe");
    assert_eq!(sample.durability.kind, DurabilityKind::TransientLocal);
    assert_eq!(sample.history.kind, HistoryKind::KeepLast);
    assert_eq!(sample.history.depth, 7);
    assert_eq!(sample.resource_limits.max_samples, 100);
    assert_eq!(sample.resource_limits.max_instances, 10);
    assert_eq!(sample.resource_limits.max_samples_per_instance, 10);
    assert_eq!(sample.deadline.period, QosDuration::from_secs(2));
    assert_eq!(sample.transport_priority.value, 3);
    assert_eq!(sample.lifespan.duration, QosDuration::from_secs(30));
    assert_eq!(sample.reliability.kind, ReliabilityKind::Reliable);
    assert_eq!(sample.ownership.kind, OwnershipKind::Exclusive);
    assert_eq!(sample.topic_data.value, b"abc");
}

/// Creates `ZeroTopic` with KEEP_LAST(5) and holds it.
#[test]
#[ignore = "holds a topic for a Cyclone DCPSTopic reader on ZERODDS_TOPIC_PROBE_DOMAIN"]
fn a_zerodds_topic_is_announced() {
    let secs: u64 = std::env::var("ZERODDS_TOPIC_PROBE_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    let participant = DomainParticipantFactory::instance()
        .create_participant(probe_domain(), DomainParticipantQos::default())
        .expect("participant");
    let qos = TopicQos {
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 5,
        },
        ..TopicQos::default()
    };
    let _topic = participant
        .create_topic::<RawBytes>("ZeroTopic", qos)
        .unwrap();
    std::thread::sleep(Duration::from_secs(secs));
}
