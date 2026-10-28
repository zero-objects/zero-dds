// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! TRANSPORT_PRIORITY (DDS 1.4 §2.2.3.15) reaches the runtime: the writer's
//! datagrams go out with it, and a `set_qos` changes it (the policy is
//! changeable). The value never left the DCPS QoS before: `UserWriterConfig`
//! had no member for it.

#![cfg(feature = "std")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

#[path = "common/mod.rs"]
mod common;

use common::{ParticipantGuard, isolated_cfg, unique_domain};
use zerodds_dcps::{
    DataWriterQos, DomainParticipantFactory, DomainParticipantQos, Entity, PublisherQos, RawBytes,
    TopicQos,
};

#[test]
fn a_writers_transport_priority_reaches_the_runtime_and_follows_a_change() {
    let p = ParticipantGuard::new(
        DomainParticipantFactory::instance()
            .create_participant_with_config(
                unique_domain(),
                DomainParticipantQos::default(),
                isolated_cfg(),
            )
            .expect("participant"),
    );
    let topic = p
        .create_topic::<RawBytes>("TransportPriority", TopicQos::default())
        .unwrap();
    let mut wq = DataWriterQos::default();
    wq.transport_priority.value = 0x28;
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, wq)
        .unwrap();
    let (rt, eid) = writer.runtime_handle().expect("live writer");
    assert_eq!(rt.user_writer_transport_priority(eid), 0x28);

    let mut changed = writer.get_qos();
    changed.transport_priority.value = 0x10;
    writer.set_qos(changed).unwrap();
    assert_eq!(rt.user_writer_transport_priority(eid), 0x10);
    // A write goes out of the priority socket and does not fail on it.
    writer.write(&RawBytes::new(vec![1])).unwrap();
}
