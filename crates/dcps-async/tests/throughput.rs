// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Spec §9.2 — `take_stream` throughput / no-sample-loss.
//!
//! Target: no sample is lost through polling latency; a reliable reader draining
//! `take_stream` receives 100 % of what a reliable writer published. Uses a live
//! (loopback) participant with Reliable + KeepAll QoS so the transport itself
//! guarantees delivery — the test asserts the async stream surfaces every one.

#![allow(clippy::expect_used, clippy::print_stderr, missing_docs)]

use std::time::{Duration, Instant};

use futures::StreamExt;
use tokio::time::timeout;
use zerodds_dcps::qos::{HistoryKind, HistoryQosPolicy, ReliabilityKind, ReliabilityQosPolicy};
use zerodds_dcps::{DataReaderQos, DataWriterQos, RawBytes, TopicQos};
use zerodds_dcps_async::{AsyncDomainParticipantFactory, PublisherQos, SubscriberQos};

const T: Duration = Duration::from_secs(5);
const N: usize = 500;

fn reliable_keepall_writer() -> DataWriterQos {
    DataWriterQos {
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: zerodds_qos::Duration::from_millis(500),
        },
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        ..Default::default()
    }
}

fn reliable_keepall_reader() -> DataReaderQos {
    DataReaderQos {
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: zerodds_qos::Duration::from_millis(500),
        },
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        ..Default::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn take_stream_delivers_all_samples_no_loss() {
    let factory = AsyncDomainParticipantFactory::instance();
    let p = factory.create_participant(0).expect("participant");
    let topic = p
        .create_topic::<RawBytes>("AsyncThroughput", TopicQos::default())
        .expect("topic");
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, reliable_keepall_writer())
        .expect("writer");
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, reliable_keepall_reader())
        .expect("reader");

    writer
        .wait_for_matched_subscription(1, T)
        .await
        .expect("w match");
    reader
        .wait_for_matched_publication(1, T)
        .await
        .expect("r match");

    let start = Instant::now();
    for i in 0..N {
        writer
            .write(&RawBytes::new(vec![(i & 0xff) as u8; 16]))
            .await
            .expect("write");
    }

    let mut stream = reader.take_stream();
    let mut received = 0usize;
    while received < N {
        match timeout(T, stream.next()).await {
            Ok(Some(_)) => received += 1,
            _ => break,
        }
    }
    let elapsed = start.elapsed();

    assert_eq!(
        received, N,
        "take_stream must surface 100% of reliably-published samples (no loss)"
    );
    eprintln!(
        "§9.2 take_stream throughput: {N} samples in {elapsed:?} = {:.0} samples/s",
        N as f64 / elapsed.as_secs_f64()
    );
}
