// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS 1.4 §2.2.5: the built-in topics carry data "about the entities of
//! other DomainParticipants", not about the participant's own topics,
//! DataWriters and DataReaders.

#![cfg(target_os = "linux")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

#[path = "common/mod.rs"]
mod common;

use std::time::Duration;

use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
    RawBytes, SubscriberQos, TopicQos,
};

#[test]
fn the_own_entities_are_in_no_built_in_reader() {
    let lease = common::lease_domain();
    let p = common::ParticipantGuard::new(
        DomainParticipantFactory::instance()
            .create_participant_with_config(
                lease.id(),
                DomainParticipantQos::default(),
                common::isolated_cfg(),
            )
            .unwrap(),
    );
    let topic = p
        .create_topic::<RawBytes>("OwnEntities", TopicQos::default())
        .unwrap();
    let _writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let _reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    // The discovery path has had time to announce them (SPDP every 100 ms).
    std::thread::sleep(Duration::from_millis(600));

    let builtin = p.get_builtin_subscriber();
    assert!(builtin.participant_reader().take().unwrap().is_empty());
    assert!(builtin.topic_reader().take().unwrap().is_empty());
    assert!(builtin.publication_reader().take().unwrap().is_empty());
    assert!(builtin.subscription_reader().take().unwrap().is_empty());
}
