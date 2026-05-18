// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! End-to-End-Test fuer Spec §2.2.3.5 DurabilityService Wire-Replay.
//!
//! Szenario: Writer mit Durability=Transient publisht Samples, bevor
//! ein Reader matched. Der Late-Joiner muss alle Samples via SEDP-Match-
//! getriggertem Backend-Replay sehen. Dies verifiziert den Wire-Pfad
//! aus `DcpsRuntime::wire_writer_to_remote_reader` — Backend-Samples
//! werden beim ersten Match in den HistoryCache injiziert und ueber
//! den existierenden Reliable-Reader-Pfad ausgeliefert.
//!
//! Linux-only: macOS-Loopback-Multicast ist fuer DDS-Discovery
//! unzuverlaessig (siehe `fastdds_qos_matrix.rs` mit gleichem Guard).

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
        // No-Key-Topic: alle Samples haben gleichen instance_key. Damit
        // Backend + Writer-Cache + Reader die volle Sample-History sehen,
        // brauchen wir KeepAll-History + DurabilityServiceQos mit
        // hinreichender history_depth.
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

/// Late-Joiner-Reader bekommt Backend-Replay beim ersten Match.
///
/// Setup: Writer mit `Transient`-Durability publisht 3 Samples vor dem
/// Reader-Start. Reader matched anschliessend; muss alle 3 Samples
/// sehen (ueber Backend-Replay-Pfad in `wire_writer_to_remote_reader`).
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

    // Backend muss da sein.
    assert!(
        writer.durability_backend().is_some(),
        "Transient-Writer muss DurabilityService-Backend haben"
    );

    // 3 Samples publishen, *bevor* der Reader existiert.
    for n in 1u32..=3 {
        writer.write(&Beep { n }).expect("write");
    }

    // 200 ms Verzoegerung — Writer-Discovery propagiert.
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

    // Match + Backend-Replay einlaufen lassen.
    reader
        .wait_for_matched_publication(1, Duration::from_secs(5))
        .expect("reader match");

    // Bis zu 3 Sekunden auf die replayed Samples warten.
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
        "Late-Joiner muss alle 3 Backend-Samples sehen (via Wire-Replay)"
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

/// Persistent-Backend (On-Disk) Late-Joiner-Replay.
///
/// Setup: Writer mit `Persistent`-Durability publisht 3 Samples vor dem
/// Reader-Start. Backend ist `OnDiskDurabilityBackend` (auto-build mit
/// `ZERODDS_DURABILITY_DIR` Env-Override). Reader matched anschliessend
/// und muss alle 3 Samples via Wire-Replay sehen.
///
/// Eigener Topic-Name verhindert Cross-Test-Kontamination im Backend.
#[test]
fn persistent_late_joiner_receives_backend_replay() {
    // Unique tmpdir pro Run, damit Backend nicht alte Samples re-played.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmpdir = std::env::temp_dir().join(format!(
        "zerodds-persistent-test-{}-{}",
        std::process::id(),
        nanos
    ));
    // SAFETY: Env-Var-Set ist Test-only, kein Cross-Thread-Race da
    // Cargo Tests pro Crate per default mit -j1 in CI laufen — und
    // wir nutzen einen unique Pfad pro Run.
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
        "Persistent-Writer muss DurabilityService-Backend (On-Disk) haben"
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
        .wait_for_matched_publication(1, Duration::from_secs(5))
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
        "Persistent-Late-Joiner muss alle 3 Backend-Samples sehen (via On-Disk-Wire-Replay)"
    );
}
