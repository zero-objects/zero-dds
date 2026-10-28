// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Round 3 measurement: does routing `take()`/`read()` through
//! `ingest_into_cache`/`cache` (instead of a second, hand-rolled
//! enforcement loop over the raw mpsc/inbox) cost throughput? KEEP_ALL, so
//! every write survives to be taken under both the old and the new
//! implementation — this measures decode+admission cost per delivered
//! sample, not a HISTORY-driven difference in how many samples arrive.
//!
//! Not a permanent benchmark (no criterion harness, no CI job) --
//! `cargo test --release --test take_read_perf_probe -- --nocapture
//! --ignored`, read the printed numbers, compare by hand. `#[ignore]`:
//! timing numbers in a correctness test suite are not an assertion.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

#[path = "common/mod.rs"]
mod common;

use std::time::Instant;

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::qos::{HistoryKind, HistoryQosPolicy, ResourceLimitsQosPolicy};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
    PublisherQos, SubscriberQos, TopicQos,
};

use common::{isolated_cfg, unique_domain};

const N: u32 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Reading {
    id: u32,
    seq: u32,
}

impl DdsType for Reading {
    const TYPE_NAME: &'static str = "test::PerfProbeReading";
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.id.to_le_bytes());
        out.extend_from_slice(&self.seq.to_le_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < 8 {
            return Err(DecodeError::Invalid {
                what: "truncated Reading",
            });
        }
        Ok(Self {
            id: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            seq: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        })
    }

    fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
        holder.write_u32(self.id);
    }

    fn decode_key_holder_be(bytes: &[u8]) -> Result<Self, DecodeError> {
        let head: [u8; 4] =
            bytes
                .get(..4)
                .and_then(|b| b.try_into().ok())
                .ok_or(DecodeError::Invalid {
                    what: "truncated Reading key holder",
                })?;
        Ok(Self {
            id: u32::from_be_bytes(head),
            ..Self::default()
        })
    }
}

#[test]
#[ignore]
fn take_drains_n_rapid_writes() {
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), isolated_cfg())
        .expect("participant");
    let topic = p
        .create_topic::<Reading>("PerfProbeReadings", TopicQos::default())
        .expect("topic");
    let publisher = p.create_publisher(PublisherQos::default());
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let keep_all = HistoryQosPolicy {
        kind: HistoryKind::KeepAll,
        depth: 1,
    };
    let writer = publisher
        .create_datawriter::<Reading>(
            &topic,
            DataWriterQos {
                history: keep_all,
                // `DataReaderQos::default()`'s own resource_limits are
                // finite (max_instances=10) -- fine for a normal reader,
                // wrong for this probe's 10_000 distinct instances (each
                // write uses a unique `id`). Genuinely unlimited, so the
                // measurement is decode+admission cost, not a RESOURCE_
                // LIMITS rejection/loss count.
                resource_limits: ResourceLimitsQosPolicy::default(),
                ..DataWriterQos::default()
            },
        )
        .expect("writer");
    let reader = subscriber
        .create_datareader::<Reading>(
            &topic,
            DataReaderQos {
                history: keep_all,
                // `DataReaderQos::default()`'s own resource_limits are
                // finite (max_instances=10) -- fine for a normal reader,
                // wrong for this probe's 10_000 distinct instances (each
                // write uses a unique `id`). Genuinely unlimited, so the
                // measurement is decode+admission cost, not a RESOURCE_
                // LIMITS rejection/loss count.
                resource_limits: ResourceLimitsQosPolicy::default(),
                ..DataReaderQos::default()
            },
        )
        .expect("reader");

    for seq in 0..N {
        writer
            .write(&Reading { id: seq, seq })
            .unwrap_or_else(|e| panic!("write {seq}: {e}"));
    }

    let started = Instant::now();
    let out = reader.take().expect("take");
    let elapsed = started.elapsed();
    assert_eq!(out.len(), N as usize, "every write must survive KEEP_ALL");

    eprintln!(
        "take_drains_n_rapid_writes: N={N} elapsed={elapsed:?} \
         per_sample={:?}",
        elapsed / N
    );
}

#[test]
#[ignore]
fn read_then_take_n_rapid_writes() {
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), isolated_cfg())
        .expect("participant");
    let topic = p
        .create_topic::<Reading>("PerfProbeReadReadings", TopicQos::default())
        .expect("topic");
    let publisher = p.create_publisher(PublisherQos::default());
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let keep_all = HistoryQosPolicy {
        kind: HistoryKind::KeepAll,
        depth: 1,
    };
    let writer = publisher
        .create_datawriter::<Reading>(
            &topic,
            DataWriterQos {
                history: keep_all,
                // `DataReaderQos::default()`'s own resource_limits are
                // finite (max_instances=10) -- fine for a normal reader,
                // wrong for this probe's 10_000 distinct instances (each
                // write uses a unique `id`). Genuinely unlimited, so the
                // measurement is decode+admission cost, not a RESOURCE_
                // LIMITS rejection/loss count.
                resource_limits: ResourceLimitsQosPolicy::default(),
                ..DataWriterQos::default()
            },
        )
        .expect("writer");
    let reader = subscriber
        .create_datareader::<Reading>(
            &topic,
            DataReaderQos {
                history: keep_all,
                // `DataReaderQos::default()`'s own resource_limits are
                // finite (max_instances=10) -- fine for a normal reader,
                // wrong for this probe's 10_000 distinct instances (each
                // write uses a unique `id`). Genuinely unlimited, so the
                // measurement is decode+admission cost, not a RESOURCE_
                // LIMITS rejection/loss count.
                resource_limits: ResourceLimitsQosPolicy::default(),
                ..DataReaderQos::default()
            },
        )
        .expect("reader");

    for seq in 0..N {
        writer
            .write(&Reading { id: seq, seq })
            .unwrap_or_else(|e| panic!("write {seq}: {e}"));
    }

    let started = Instant::now();
    let peeked = reader.read().expect("read");
    let elapsed_read = started.elapsed();
    assert_eq!(peeked.len(), N as usize);

    let started2 = Instant::now();
    let taken = reader.take().expect("take");
    let elapsed_take = started2.elapsed();
    assert_eq!(taken.len(), N as usize);

    eprintln!(
        "read_then_take_n_rapid_writes: N={N} read_elapsed={elapsed_read:?} \
         take_elapsed={elapsed_take:?}"
    );
}
