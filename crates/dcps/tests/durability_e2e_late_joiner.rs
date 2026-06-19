// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! End-to-end test for Spec §2.2.3.5 DurabilityService wire replay.
//!
//! Scenario: a writer with Durability=Transient publishes samples before
//! a reader matches. The late-joiner must see all samples via SEDP-match-
//! triggered backend replay. This verifies the wire path in
//! `DcpsRuntime::wire_writer_to_remote_reader` — backend samples are
//! injected into the HistoryCache on the first match and delivered over
//! the existing reliable-reader path.
//!
//! Linux-only: macOS loopback multicast is unreliable for DDS discovery
//! (see `fastdds_qos_matrix.rs` with the same guard).

#![cfg(target_os = "linux")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::uninlined_format_args,
    missing_docs
)]

use std::time::Duration;

use zerodds_dcps::dds_type::{DecodeError, EncodeError};
use zerodds_dcps::runtime::RuntimeConfig;

#[path = "common/mod.rs"]
mod common;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
    PublisherQos, SubscriberQos, TopicQos,
};
use zerodds_qos::{
    DurabilityKind, DurabilityQosPolicy, DurabilityServiceQosPolicy, HistoryKind, HistoryQosPolicy,
    ReliabilityKind, ReliabilityQosPolicy,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Beep {
    n: u32,
}

impl DdsType for Beep {
    const TYPE_NAME: &'static str = "test::Beep";
    const HAS_KEY: bool = false;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(0);

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.n.to_be_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < 4 {
            return Err(DecodeError::Invalid {
                what: "truncated Beep",
            });
        }
        let mut a = [0u8; 4];
        a.copy_from_slice(&bytes[..4]);
        Ok(Self {
            n: u32::from_be_bytes(a),
        })
    }
}

fn reliable_transient_writer() -> DataWriterQos {
    DataWriterQos {
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: zerodds_qos::Duration::from_millis(100),
        },
        durability: DurabilityQosPolicy {
            kind: DurabilityKind::Transient,
        },
        // No-key topic: all samples share the same instance_key. For the
        // backend + writer cache + reader to see the full sample history,
        // we need KeepAll history + DurabilityServiceQos with a
        // sufficient history_depth.
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        durability_service: DurabilityServiceQosPolicy {
            history_kind: HistoryKind::KeepAll,
            history_depth: 16,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn reliable_transient_reader() -> DataReaderQos {
    DataReaderQos {
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: zerodds_qos::Duration::from_millis(100),
        },
        durability: DurabilityQosPolicy {
            kind: DurabilityKind::Transient,
        },
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        ..Default::default()
    }
}

fn fast_runtime_cfg() -> RuntimeConfig {
    RuntimeConfig {
        tick_period: Duration::from_millis(20),
        spdp_period: Duration::from_millis(100),
        ..RuntimeConfig::default()
    }
}

/// Late-joiner reader gets a backend replay on the first match.
///
/// Setup: a writer with `Transient` durability publishes 3 samples before
/// the reader starts. The reader then matches and must see all 3 samples
/// (via the backend-replay path in `wire_writer_to_remote_reader`).
#[test]
fn transient_late_joiner_receives_backend_replay() {
    let factory = DomainParticipantFactory::instance();
    let cfg = fast_runtime_cfg();

    // Publisher-Participant
    let pub_p = factory
        .create_participant_with_config(7, DomainParticipantQos::default(), cfg.clone())
        .expect("create pub participant");
    let topic_p = pub_p
        .create_topic::<Beep>("LateJoinTopic", TopicQos::default())
        .expect("topic");
    let publisher = pub_p.create_publisher(PublisherQos::default());
    let writer = publisher
        .create_datawriter::<Beep>(&topic_p, reliable_transient_writer())
        .expect("writer");

    // Backend must exist.
    assert!(
        writer.durability_backend().is_some(),
        "transient writer must have a DurabilityService backend"
    );

    // Publish 3 samples *before* the reader exists.
    for n in 1u32..=3 {
        writer.write(&Beep { n }).expect("write");
    }

    // 200 ms delay — writer discovery propagates.
    std::thread::sleep(Duration::from_millis(200));

    // Subscriber-Participant (Late-Joiner)
    let sub_p = factory
        .create_participant_with_config(7, DomainParticipantQos::default(), cfg.clone())
        .expect("create sub participant");
    let topic_s = sub_p
        .create_topic::<Beep>("LateJoinTopic", TopicQos::default())
        .expect("topic");
    let subscriber = sub_p.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<Beep>(&topic_s, reliable_transient_reader())
        .expect("reader");

    // Let the match + backend replay settle in.
    reader
        .wait_for_matched_publication(1, common::match_timeout())
        .expect("reader match");

    // Wait up to 3 seconds for the replayed samples.
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut collected: Vec<u32> = Vec::new();
    while std::time::Instant::now() < deadline && collected.len() < 3 {
        let _ = reader.wait_for_data(Duration::from_millis(200));
        if let Ok(samples) = reader.take() {
            for s in samples {
                collected.push(s.n);
            }
        }
    }

    collected.sort_unstable();
    assert_eq!(
        collected,
        vec![1, 2, 3],
        "late-joiner must see all 3 backend samples (via wire replay)"
    );
}

fn reliable_persistent_writer() -> DataWriterQos {
    DataWriterQos {
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: zerodds_qos::Duration::from_millis(100),
        },
        durability: DurabilityQosPolicy {
            kind: DurabilityKind::Persistent,
        },
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        durability_service: zerodds_qos::DurabilityServiceQosPolicy {
            history_kind: HistoryKind::KeepAll,
            history_depth: 16,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn reliable_persistent_reader() -> DataReaderQos {
    DataReaderQos {
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: zerodds_qos::Duration::from_millis(100),
        },
        durability: DurabilityQosPolicy {
            kind: DurabilityKind::Persistent,
        },
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        ..Default::default()
    }
}

/// Persistent backend (on-disk) late-joiner replay.
///
/// Setup: a writer with `Persistent` durability publishes 3 samples before
/// the reader starts. The backend is `OnDiskDurabilityBackend` (auto-built
/// with the `ZERODDS_DURABILITY_DIR` env override). The reader then matches
/// and must see all 3 samples via wire replay.
///
/// A dedicated topic name prevents cross-test contamination in the backend.
#[test]
fn persistent_late_joiner_receives_backend_replay() {
    // Unique tmpdir per run so the backend does not replay old samples.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmpdir = std::env::temp_dir().join(format!(
        "zerodds-persistent-test-{}-{}",
        std::process::id(),
        nanos
    ));
    // SAFETY: setting the env-var is test-only, with no cross-thread race
    // since cargo tests run per crate with -j1 in CI by default — and we
    // use a unique path per run.
    // SAFETY: This is the only test-thread touching this env-var (Linux-only test,
    // tests in this file run serially per default).
    unsafe {
        std::env::set_var("ZERODDS_DURABILITY_DIR", &tmpdir);
    }

    let factory = DomainParticipantFactory::instance();
    let cfg = fast_runtime_cfg();

    let pub_p = factory
        .create_participant_with_config(8, DomainParticipantQos::default(), cfg.clone())
        .expect("create pub participant");
    let topic_p = pub_p
        .create_topic::<Beep>("PersistentLateJoinTopic", TopicQos::default())
        .expect("topic");
    let publisher = pub_p.create_publisher(PublisherQos::default());
    let writer = publisher
        .create_datawriter::<Beep>(&topic_p, reliable_persistent_writer())
        .expect("writer");

    assert!(
        writer.durability_backend().is_some(),
        "persistent writer must have a DurabilityService backend (on-disk)"
    );

    for n in 1u32..=3 {
        writer.write(&Beep { n }).expect("write");
    }

    std::thread::sleep(Duration::from_millis(200));

    let sub_p = factory
        .create_participant_with_config(8, DomainParticipantQos::default(), cfg.clone())
        .expect("create sub participant");
    let topic_s = sub_p
        .create_topic::<Beep>("PersistentLateJoinTopic", TopicQos::default())
        .expect("topic");
    let subscriber = sub_p.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<Beep>(&topic_s, reliable_persistent_reader())
        .expect("reader");

    reader
        .wait_for_matched_publication(1, common::match_timeout())
        .expect("reader match");

    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut collected: Vec<u32> = Vec::new();
    while std::time::Instant::now() < deadline && collected.len() < 3 {
        let _ = reader.wait_for_data(Duration::from_millis(200));
        if let Ok(samples) = reader.take() {
            for s in samples {
                collected.push(s.n);
            }
        }
    }

    // Cleanup tmpdir (best-effort, ignore errors).
    let _ = std::fs::remove_dir_all(&tmpdir);

    collected.sort_unstable();
    assert_eq!(
        collected,
        vec![1, 2, 3],
        "persistent late-joiner must see all 3 backend samples (via on-disk wire replay)"
    );
}
