// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `wait_for_historical_data` (DDS 1.4 §2.2.2.5.3): a TRANSIENT_LOCAL
//! reader that joins after a writer wrote blocks until the writer's
//! history has arrived, and then holds it. It returned OK at once for
//! every reader, before any history had arrived.
//!
//! Linux only: multicast loopback on macOS is unreliable for SPDP.

#![cfg(all(target_os = "linux", feature = "std"))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

#[path = "common/mod.rs"]
mod common;

use std::time::Duration;

use common::{ParticipantGuard, isolated_cfg, match_timeout, unique_domain};
use zerodds_dcps::qos::{DurabilityKind, HistoryKind, ReliabilityKind};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
    RawBytes, SubscriberQos, TopicQos,
};

fn pair() -> (ParticipantGuard, ParticipantGuard) {
    let cfg = isolated_cfg();
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let a = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
        .expect("participant a");
    let b = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
        .expect("participant b");
    (ParticipantGuard::new(a), ParticipantGuard::new(b))
}

#[test]
fn a_late_transient_local_reader_waits_for_the_history_and_then_holds_it() {
    let (a, b) = pair();
    let topic_a = a
        .create_topic::<RawBytes>("HistoricalData", TopicQos::default())
        .unwrap();
    let topic_b = b
        .create_topic::<RawBytes>("HistoricalData", TopicQos::default())
        .unwrap();
    let mut wq = DataWriterQos::default();
    wq.durability.kind = DurabilityKind::TransientLocal;
    wq.history.kind = HistoryKind::KeepAll;
    let writer = a
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic_a, wq)
        .unwrap();
    for i in 0..5u8 {
        writer.write(&RawBytes::new(vec![i])).unwrap();
    }

    let mut rq = DataReaderQos::default();
    rq.durability.kind = DurabilityKind::TransientLocal;
    rq.reliability.kind = ReliabilityKind::Reliable;
    rq.history.kind = HistoryKind::KeepAll;
    let reader = b
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic_b, rq)
        .unwrap();
    reader
        .wait_for_matched_publication(1, match_timeout())
        .expect("match");
    reader
        .wait_for_historical_data(match_timeout())
        .expect("the history arrived");
    let taken = reader.take().unwrap();
    assert_eq!(
        taken.len(),
        5,
        "the history was not there when the wait returned"
    );
}

#[test]
fn a_volatile_reader_expects_no_history() {
    let (_a, b) = pair();
    let topic = b
        .create_topic::<RawBytes>("HistoricalDataVolatile", TopicQos::default())
        .unwrap();
    let reader = b
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    reader
        .wait_for_historical_data(Duration::ZERO)
        .expect("nothing to wait for");
}
