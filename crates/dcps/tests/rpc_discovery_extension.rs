// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-RPC 1.0 §7.6.2.1: an endpoint announces `service_instance_name`
//! (PID 0x0080), the related endpoint's GUID (0x0081) and `topic_aliases`
//! (0x0082), and its peer reads them from the matched endpoint's data.
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

#[test]
fn the_rpc_extension_of_a_writer_and_a_reader_reaches_the_matched_peer() {
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
    let topic_a = a
        .create_topic::<RawBytes>("RpcDiscoveryExt", TopicQos::default())
        .unwrap();
    let topic_b = b
        .create_topic::<RawBytes>("RpcDiscoveryExt", TopicQos::default())
        .unwrap();
    let writer = a
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic_a, DataWriterQos::default())
        .unwrap();
    let reader = b
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic_b, DataReaderQos::default())
        .unwrap();
    let reader_guid = reader.rtps_guid().expect("live reader");

    let writer_ext = RpcDiscoveryExt {
        service_instance_name: Some("calc-1".into()),
        related_entity_guid: Some([7; 16]),
        topic_aliases: Some(vec!["Base_Service_Request".into()]),
    };
    writer.set_rpc_discovery(&writer_ext).unwrap();
    let reader_ext = RpcDiscoveryExt {
        service_instance_name: None,
        related_entity_guid: Some(writer.rtps_guid().expect("live writer")),
        topic_aliases: None,
    };
    reader.set_rpc_discovery(&reader_ext).unwrap();

    assert!(
        eventually(|| {
            reader
                .matched_rpc_writers()
                .iter()
                .any(|peer| peer.ext == writer_ext)
        }),
        "the reader sees the writer's extension: {:?}",
        reader.matched_rpc_writers()
    );
    assert!(
        eventually(|| {
            writer
                .matched_rpc_readers()
                .iter()
                .any(|peer| peer.key == reader_guid && peer.ext == reader_ext)
        }),
        "the writer sees the reader's extension: {:?}",
        writer.matched_rpc_readers()
    );
}

#[test]
fn a_discovery_string_longer_than_256_is_refused() {
    let cfg = isolated_cfg();
    let domain = unique_domain();
    let p = ParticipantGuard::new(
        DomainParticipantFactory::instance()
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("participant"),
    );
    let topic = p
        .create_topic::<RawBytes>("RpcDiscoveryLong", TopicQos::default())
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let at_limit = RpcDiscoveryExt {
        service_instance_name: Some("x".repeat(256)),
        ..RpcDiscoveryExt::default()
    };
    writer.set_rpc_discovery(&at_limit).unwrap();
    let over = RpcDiscoveryExt {
        service_instance_name: Some("x".repeat(257)),
        ..RpcDiscoveryExt::default()
    };
    assert!(writer.set_rpc_discovery(&over).is_err());
}
