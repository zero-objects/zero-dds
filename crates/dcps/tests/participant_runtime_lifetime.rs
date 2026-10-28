// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! How long a participant's runtime runs: while the participant or any
//! publisher or subscriber it created is still held (DDS-PSM-Cxx: a child
//! keeps its parent alive), and no longer. Two participants of this process
//! on one leased domain.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

#[path = "common/mod.rs"]
mod common;

use std::time::{Duration, Instant};

use zerodds_dcps::qos::{
    DurabilityKind, DurabilityQosPolicy, HistoryKind, HistoryQosPolicy, ReliabilityKind,
    ReliabilityQosPolicy,
};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
    RawBytes, SubscriberQos, TopicQos,
};

/// TRANSIENT_LOCAL, RELIABLE, KEEP_ALL on both sides: `RawBytes` has no
/// key, so under the default KEEP_LAST(1) a reader keeps one sample.
fn transient_local_reliable() -> (DataWriterQos, DataReaderQos) {
    let history = HistoryQosPolicy {
        kind: HistoryKind::KeepAll,
        depth: 1,
    };
    let reliability = ReliabilityQosPolicy {
        kind: ReliabilityKind::Reliable,
        ..Default::default()
    };
    let durability = DurabilityQosPolicy {
        kind: DurabilityKind::TransientLocal,
    };
    (
        DataWriterQos {
            reliability,
            durability,
            history,
            ..Default::default()
        },
        DataReaderQos {
            reliability,
            durability,
            history,
            ..Default::default()
        },
    )
}

fn take_for(reader: &zerodds_dcps::subscriber::DataReader<RawBytes>, window: Duration) -> Vec<u8> {
    let mut got = Vec::new();
    let until = Instant::now() + window;
    while Instant::now() < until {
        let _ = reader.wait_for_data(Duration::from_millis(100));
        if let Ok(samples) = reader.take() {
            got.extend(samples.into_iter().map(|s| s.data[0]));
        }
    }
    got
}

/// The application drops its participant handle but keeps the writer: the
/// runtime keeps running and the writer's samples arrive.
#[test]
fn a_writer_keeps_its_runtime_after_the_participant_handle_drops() {
    let lease = common::lease_domain();
    let cfg = common::isolated_cfg();
    let factory = DomainParticipantFactory::instance();
    let (wq, rq) = transient_local_reliable();

    let writer = {
        let pa = factory
            .create_participant_with_config(
                lease.id(),
                DomainParticipantQos::default(),
                cfg.clone(),
            )
            .expect("participant A");
        let topic = pa
            .create_topic::<RawBytes>("RuntimeLifetimeKept", TopicQos::default())
            .expect("topic A");
        pa.create_publisher(PublisherQos::default())
            .create_datawriter::<RawBytes>(&topic, wq)
            .expect("writer")
        // `pa` drops here; the writer is all the application keeps.
    };

    let pb = common::ParticipantGuard::new(
        factory
            .create_participant_with_config(lease.id(), DomainParticipantQos::default(), cfg)
            .expect("participant B"),
    );
    let topic = pb
        .create_topic::<RawBytes>("RuntimeLifetimeKept", TopicQos::default())
        .expect("topic B");
    let reader = pb
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, rq)
        .expect("reader");
    writer
        .wait_for_matched_subscription(1, common::match_timeout())
        .expect("the writer's runtime still discovers and matches");
    for n in 1..=5u8 {
        writer.write(&RawBytes::new(vec![n])).expect("write");
    }
    let mut got = take_for(&reader, Duration::from_secs(3));
    got.sort_unstable();
    assert_eq!(got, vec![1, 2, 3, 4, 5]);
}

/// The participant and everything it created are dropped: its runtime stops
/// announcing it. A participant created afterwards discovers only the one
/// that is still alive (C, the control that discovery works here), not A.
#[test]
fn dropping_a_participant_with_its_entities_stops_its_runtime() {
    let lease = common::lease_domain();
    let cfg = common::isolated_cfg();
    let factory = DomainParticipantFactory::instance();
    let (wq, _) = transient_local_reliable();

    let pc = common::ParticipantGuard::new(
        factory
            .create_participant_with_config(
                lease.id(),
                DomainParticipantQos::default(),
                cfg.clone(),
            )
            .expect("participant C"),
    );
    {
        let pa = factory
            .create_participant_with_config(
                lease.id(),
                DomainParticipantQos::default(),
                cfg.clone(),
            )
            .expect("participant A");
        let topic = pa
            .create_topic::<RawBytes>("RuntimeLifetimeGone", TopicQos::default())
            .expect("topic A");
        let publisher = pa.create_publisher(PublisherQos::default());
        let writer = publisher
            .create_datawriter::<RawBytes>(&topic, wq)
            .expect("writer");
        writer.write(&RawBytes::new(vec![1])).expect("write");
        // writer, publisher, topic and pa drop here.
    }

    let pb = common::ParticipantGuard::new(
        factory
            .create_participant_with_config(lease.id(), DomainParticipantQos::default(), cfg)
            .expect("participant B"),
    );
    let mut seen = 0;
    let until = Instant::now() + Duration::from_secs(3);
    while Instant::now() < until {
        seen = pb.get_discovered_participants().len();
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(
        seen, 1,
        "B must discover C and only C; a second participant is A's runtime still running"
    );
    drop(pc);
}
