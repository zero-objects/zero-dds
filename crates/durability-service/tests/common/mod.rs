// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Shared helpers for the durability-service e2e tests. Each e2e test lives in
//! its OWN `tests/*.rs` file so cargo runs it in a SEPARATE process — full
//! isolation from the process-global DDS state (factory singleton, in-process
//! discovery registry, multicast), which otherwise cross-contaminates DDS
//! tests sharing one binary.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, dead_code)]

use std::time::{Duration, Instant, SystemTime};

use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, RawBytes,
    SubscriberQos, TopicQos,
};
use zerodds_durability_store::{Contract, DurabilityStore};
use zerodds_qos::policies::durability::DurabilityKind;
use zerodds_qos::policies::history::HistoryKind;
use zerodds_qos::policies::reliability::ReliabilityKind;
use zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED;

pub fn keep_all() -> Contract {
    Contract {
        history_kind: HistoryKind::KeepAll,
        history_depth: 0,
        max_samples: LENGTH_UNLIMITED,
        max_instances: LENGTH_UNLIMITED,
        max_samples_per_instance: LENGTH_UNLIMITED,
        cleanup_delay: Duration::ZERO,
    }
}

/// Writer QoS, durability = TRANSIENT_LOCAL (dies with the writer; no
/// writer-embedded backend to overlap the service on late-join).
pub fn transient_local_writer_qos() -> DataWriterQos {
    let mut q = DataWriterQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.durability.kind = DurabilityKind::TransientLocal;
    q.history.kind = HistoryKind::KeepAll;
    q.resource_limits.max_samples = LENGTH_UNLIMITED;
    q
}

/// Writer QoS, durability = TRANSIENT — the service level that auto-discovery
/// picks up (TransientLocal would not trigger it).
pub fn transient_service_writer_qos() -> DataWriterQos {
    let mut q = DataWriterQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.durability.kind = DurabilityKind::Transient;
    q.history.kind = HistoryKind::KeepAll;
    q.resource_limits.max_samples = LENGTH_UNLIMITED;
    q
}

pub fn transient_reader_qos() -> DataReaderQos {
    let mut q = DataReaderQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.durability.kind = DurabilityKind::TransientLocal;
    q.history.kind = HistoryKind::KeepAll;
    q.resource_limits.max_samples = LENGTH_UNLIMITED;
    q
}

/// Polls `store.stats(topic).samples` until it reaches `n` or times out.
pub fn wait_for_stored(
    store: &dyn DurabilityStore,
    topic: &str,
    n: usize,
    timeout: Duration,
) -> usize {
    let deadline = Instant::now() + timeout;
    loop {
        let got = store.stats(topic).map(|s| s.samples).unwrap_or(0);
        if got >= n || Instant::now() >= deadline {
            return got;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Collects up to `expect` samples from a late-joiner reader on `topic`.
pub fn late_joiner_collect(
    domain: i32,
    topic: &str,
    expect: usize,
    timeout: Duration,
) -> Vec<Vec<u8>> {
    let factory = DomainParticipantFactory::instance();
    let participant = factory
        .create_participant(domain, DomainParticipantQos::default())
        .unwrap();
    let sub = participant.create_subscriber(SubscriberQos::default());
    let topic_h = participant
        .create_topic::<RawBytes>(topic, TopicQos::default())
        .unwrap();
    let reader = sub
        .create_datareader::<RawBytes>(&topic_h, transient_reader_qos())
        .unwrap();
    let mut out = Vec::new();
    let deadline = Instant::now() + timeout;
    // Stop on `expect` DISTINCT payloads (not raw count). A not-yet-torn-down
    // overlapping writer can briefly re-deliver identical transient_local
    // history (duplicates of identical data, not extra samples); counting
    // distinct payloads keeps the stop condition from tripping early on such
    // duplicates, so the late-joiner deterministically observes every distinct
    // sample the daemon serves. Callers dedup / assert the distinct set.
    let mut distinct: std::collections::BTreeSet<Vec<u8>> = std::collections::BTreeSet::new();
    while distinct.len() < expect && Instant::now() < deadline {
        let _ = reader.wait_for_data(Duration::from_millis(200));
        if let Ok(samples) = reader.take_with_info() {
            for s in samples {
                if s.info.valid_data {
                    distinct.insert(s.data.data.clone());
                    out.push(s.data.data);
                }
            }
        }
    }
    out
}

pub fn tmp_db(tag: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    p.push(format!("zerodds-dursvc-{tag}-{nanos}.db"));
    p
}
