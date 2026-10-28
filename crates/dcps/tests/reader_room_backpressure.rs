// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A RELIABLE KEEP_ALL DataReader whose RESOURCE_LIMITS are reached holds
//! its writer back (DDS 1.4 §2.2.3.14, §2.2.2.4.2.11) instead of queueing
//! whatever the writer sends.
//!
//! The reader is in a second participant, so the samples travel over RTPS
//! and the writer learns of the reader's room only through what the reader
//! acknowledges. Before the reader had a room, every sample went onto the
//! reader's channel and was acknowledged at once: a writer with finite
//! RESOURCE_LIMITS never blocked, and a reader that took every 3 s held
//! 523 273 samples.
//!
//! What holds a writer back is the RESOURCE_LIMITS: a full flow-control
//! window alone does not make its write fail.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

#[path = "common/mod.rs"]
mod common;

use std::time::{Duration, Instant};

use zerodds_dcps::dds_type::{DecodeError, EncodeError};
use zerodds_dcps::error::DdsError;
use zerodds_dcps::qos::{
    HistoryKind, HistoryQosPolicy, ReliabilityKind, ReliabilityQosPolicy, ResourceLimitsQosPolicy,
};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
    PublisherQos, SubscriberQos, TopicQos,
};

use common::isolated_cfg;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Numbered {
    seq: u32,
}

impl DdsType for Numbered {
    const TYPE_NAME: &'static str = "test::ReaderRoomNumbered";
    const XCDR1_MATCHES_XCDR2: bool = true;

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.seq.to_le_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let head: [u8; 4] =
            bytes
                .get(..4)
                .and_then(|b| b.try_into().ok())
                .ok_or(DecodeError::Invalid {
                    what: "truncated Numbered",
                })?;
        Ok(Self {
            seq: u32::from_le_bytes(head),
        })
    }
}

const READER_MAX_SAMPLES: i32 = 4;
const WRITER_MAX_SAMPLES: i32 = 8;

fn keep_all() -> HistoryQosPolicy {
    HistoryQosPolicy {
        kind: HistoryKind::KeepAll,
        depth: 1,
    }
}

fn reliable(max_blocking_time: Duration) -> ReliabilityQosPolicy {
    ReliabilityQosPolicy {
        kind: ReliabilityKind::Reliable,
        max_blocking_time: zerodds_qos::Duration::from_millis(
            i32::try_from(max_blocking_time.as_millis()).expect("ms"),
        ),
    }
}

#[test]
fn a_full_keep_all_reader_blocks_its_writer_and_loses_nothing() {
    let lease = common::lease_domain();
    let factory = DomainParticipantFactory::instance();
    let cfg = isolated_cfg();
    let writer_p = common::ParticipantGuard::new(
        factory
            .create_participant_with_config(
                lease.id(),
                DomainParticipantQos::default(),
                cfg.clone(),
            )
            .expect("writer participant"),
    );
    let reader_p = common::ParticipantGuard::new(
        factory
            .create_participant_with_config(lease.id(), DomainParticipantQos::default(), cfg)
            .expect("reader participant"),
    );
    let topic_name = "ReaderRoomBackpressure";
    let writer_topic = writer_p
        .create_topic::<Numbered>(topic_name, TopicQos::default())
        .expect("writer topic");
    let reader_topic = reader_p
        .create_topic::<Numbered>(topic_name, TopicQos::default())
        .expect("reader topic");
    let writer = writer_p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Numbered>(
            &writer_topic,
            DataWriterQos {
                history: keep_all(),
                resource_limits: ResourceLimitsQosPolicy {
                    max_samples: WRITER_MAX_SAMPLES,
                    ..Default::default()
                },
                reliability: reliable(Duration::from_millis(200)),
                ..Default::default()
            },
        )
        .expect("writer");
    let reader = reader_p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Numbered>(
            &reader_topic,
            DataReaderQos {
                history: keep_all(),
                resource_limits: ResourceLimitsQosPolicy {
                    max_samples: READER_MAX_SAMPLES,
                    ..Default::default()
                },
                reliability: reliable(Duration::from_millis(100)),
                ..Default::default()
            },
        )
        .expect("reader");
    writer
        .wait_for_matched_subscription(1, common::match_timeout())
        .expect("the reader matches");

    // The reader takes nothing. What the writer can place is bounded by
    // the reader's cache and the one sample it holds back (5), the RTPS
    // receive cache capped at the reader's limit (4) and the writer's own
    // history (8): 17, and some writes in flight. A writer that never
    // blocks writes all 200.
    let mut written = 0u32;
    let mut blocked = false;
    for seq in 1..=200u32 {
        match writer.write(&Numbered { seq }) {
            Ok(()) => written = seq,
            Err(DdsError::Timeout) => {
                blocked = true;
                break;
            }
            Err(other) => panic!("write {seq}: {other:?}"),
        }
    }
    assert!(
        blocked,
        "the writer wrote {written} samples without blocking"
    );
    assert!(
        written <= 64,
        "the writer placed {written} samples before it blocked"
    );

    // Taking makes room; every sample the writer placed arrives, once and
    // in order.
    let mut got = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(20);
    while got.len() < written as usize && Instant::now() < deadline {
        let taken = reader.take().expect("take");
        if taken.is_empty() {
            std::thread::sleep(Duration::from_millis(10));
        }
        got.extend(taken.iter().map(|s| s.seq));
    }
    assert_eq!(got, (1..=written).collect::<Vec<_>>());
    assert_eq!(reader.sample_lost_count(), 0);

    // The writer has room again.
    writer
        .write(&Numbered { seq: written + 1 })
        .expect("a write after the reader took");
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut last = Vec::new();
    while last.is_empty() && Instant::now() < deadline {
        last = reader.take().expect("take");
        if last.is_empty() {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    assert_eq!(
        last.iter().map(|s| s.seq).collect::<Vec<_>>(),
        [written + 1]
    );
}

/// A RELIABLE KEEP_ALL writer without RESOURCE_LIMITS whose flow-control
/// window is full writes on once `max_blocking_time` has passed: DDS 1.4
/// §2.2.2.4.2.11 lets a write fail with TIMEOUT only while the
/// RESOURCE_LIMITS have no room or data would be lost, and the window
/// (`writer.unacked_window_bytes`) is neither. With a one-byte window every
/// write after the first finds it full, and the reader, at the spec's
/// heartbeatResponseDelay of 500 ms, acknowledges later than the writer's
/// 20 ms. Every write returned TIMEOUT before.
#[test]
fn a_full_window_alone_does_not_make_a_write_fail() {
    let lease = common::lease_domain();
    let factory = DomainParticipantFactory::instance();
    let mut writer_cfg = isolated_cfg();
    writer_cfg.writer_unacked_window = 1;
    let mut reader_cfg = writer_cfg.clone();
    reader_cfg.reader_heartbeat_response_delay =
        zerodds_dcps::runtime::RuntimeConfig::default().reader_heartbeat_response_delay;
    let writer_p = common::ParticipantGuard::new(
        factory
            .create_participant_with_config(lease.id(), DomainParticipantQos::default(), writer_cfg)
            .expect("writer participant"),
    );
    let reader_p = common::ParticipantGuard::new(
        factory
            .create_participant_with_config(lease.id(), DomainParticipantQos::default(), reader_cfg)
            .expect("reader participant"),
    );
    let topic_name = "WriterWindowNoTimeout";
    let writer_topic = writer_p
        .create_topic::<Numbered>(topic_name, TopicQos::default())
        .expect("writer topic");
    let reader_topic = reader_p
        .create_topic::<Numbered>(topic_name, TopicQos::default())
        .expect("reader topic");
    let writer = writer_p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Numbered>(
            &writer_topic,
            DataWriterQos {
                history: keep_all(),
                reliability: reliable(Duration::from_millis(20)),
                ..Default::default()
            },
        )
        .expect("writer");
    let reader = reader_p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Numbered>(
            &reader_topic,
            DataReaderQos {
                history: keep_all(),
                reliability: reliable(Duration::from_millis(100)),
                ..Default::default()
            },
        )
        .expect("reader");
    writer
        .wait_for_matched_subscription(1, common::match_timeout())
        .expect("the reader matches");

    for seq in 1..=10u32 {
        writer
            .write(&Numbered { seq })
            .unwrap_or_else(|e| panic!("write {seq}: {e:?}"));
    }

    let mut got = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(20);
    while got.len() < 10 && Instant::now() < deadline {
        let taken = reader.take().expect("take");
        if taken.is_empty() {
            std::thread::sleep(Duration::from_millis(10));
        }
        got.extend(taken.iter().map(|s| s.seq));
    }
    assert_eq!(got, (1..=10).collect::<Vec<_>>());
}
