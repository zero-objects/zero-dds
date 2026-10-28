// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-RPC 1.0 §7.4.3: "a topic-alias is an alternative name for a topic".
//! A reader on a topic matches a writer that announced that topic as one of
//! its `topic_aliases` (and the other way round), over the wire and inside
//! one participant. Endpoints without aliases match as before.
//!
//! Linux only: multicast loopback on macOS is unreliable for SPDP.

#![cfg(all(target_os = "linux", feature = "std"))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

#[path = "common/mod.rs"]
mod common;

use std::time::Duration;

use common::{ParticipantGuard, isolated_cfg, match_timeout, unique_domain};
use zerodds_dcps::runtime::RpcDiscoveryExt;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
    RawBytes, SubscriberQos, TopicQos,
};

fn eventually(cond: impl Fn() -> bool) -> bool {
    let deadline = std::time::Instant::now() + match_timeout();
    while std::time::Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    cond()
}

fn aliases(names: &[&str]) -> RpcDiscoveryExt {
    RpcDiscoveryExt {
        topic_aliases: Some(names.iter().map(|n| (*n).to_string()).collect()),
        ..RpcDiscoveryExt::default()
    }
}

#[test]
fn a_reader_on_an_alias_matches_the_writer_over_the_wire() {
    let cfg = isolated_cfg();
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let a = ParticipantGuard::new(
        factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("participant a"),
    );
    let b = ParticipantGuard::new(
        factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("participant b"),
    );
    let derived = a
        .create_topic::<RawBytes>("Derived_Service_Request", TopicQos::default())
        .unwrap();
    let base = b
        .create_topic::<RawBytes>("Adder_Service_Request", TopicQos::default())
        .unwrap();
    let other = b
        .create_topic::<RawBytes>("Unrelated_Request", TopicQos::default())
        .unwrap();
    let writer = a
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&derived, DataWriterQos::default())
        .unwrap();
    let subscriber = b.create_subscriber(SubscriberQos::default());
    let on_alias = subscriber
        .create_datareader::<RawBytes>(&base, DataReaderQos::default())
        .unwrap();
    let on_other = subscriber
        .create_datareader::<RawBytes>(&other, DataReaderQos::default())
        .unwrap();

    // Without the announcement the topics differ and nothing matches.
    std::thread::sleep(Duration::from_millis(800));
    assert_eq!(on_alias.matched_publication_count(), 0);

    writer
        .set_rpc_discovery(&aliases(&["Adder_Service_Request"]))
        .unwrap();
    assert!(
        eventually(|| on_alias.matched_publication_count() == 1),
        "the reader on the alias matches"
    );
    assert_eq!(on_other.matched_publication_count(), 0);
    assert!(eventually(|| writer.matched_subscription_count() == 1));
}

#[test]
fn an_alias_announced_by_the_reader_matches_the_writer_on_that_topic() {
    let cfg = isolated_cfg();
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let a = ParticipantGuard::new(
        factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("participant a"),
    );
    let b = ParticipantGuard::new(
        factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("participant b"),
    );
    let base = a
        .create_topic::<RawBytes>("Adder_Service_Reply", TopicQos::default())
        .unwrap();
    let derived = b
        .create_topic::<RawBytes>("Derived_Service_Reply", TopicQos::default())
        .unwrap();
    let writer = a
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&base, DataWriterQos::default())
        .unwrap();
    let reader = b
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&derived, DataReaderQos::default())
        .unwrap();
    reader
        .set_rpc_discovery(&aliases(&["Adder_Service_Reply"]))
        .unwrap();
    assert!(eventually(|| reader.matched_publication_count() == 1));
    writer
        .wait_for_matched_subscription(1, match_timeout())
        .unwrap();
}

#[test]
fn aliases_match_inside_one_participant_too() {
    let cfg = isolated_cfg();
    let domain = unique_domain();
    let p = ParticipantGuard::new(
        DomainParticipantFactory::instance()
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("participant"),
    );
    let derived = p
        .create_topic::<RawBytes>("Derived_Service_Request", TopicQos::default())
        .unwrap();
    let base = p
        .create_topic::<RawBytes>("Adder_Service_Request", TopicQos::default())
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&derived, DataWriterQos::default())
        .unwrap();
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&base, DataReaderQos::default())
        .unwrap();
    writer
        .set_rpc_discovery(&aliases(&["Adder_Service_Request"]))
        .unwrap();
    assert!(eventually(|| reader.matched_publication_count() == 1));
    writer.write(&RawBytes::new(vec![1, 2, 3])).unwrap();
    assert!(eventually(|| {
        reader.take().is_ok_and(|samples| samples.len() == 1)
    }));
}
