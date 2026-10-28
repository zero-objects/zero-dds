// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A TRANSIENT_LOCAL late joiner in another participant receives what the
//! writer's HISTORY keeps (DDS 1.4 §2.2.3.4 + §2.2.3.18): the newest
//! `depth` samples per instance under KEEP_LAST, every sample under
//! KEEP_ALL.
//!
//! macOS: ignored — multicast loopback is unreliable there, as for the other
//! same-host SPDP e2e tests; runs on Linux.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::time::{Duration, Instant};

use zerodds_dcps::dds_type::{DecodeError, EncodeError};
use zerodds_dcps::runtime::RuntimeConfig;

#[path = "common/mod.rs"]
mod common;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
    PublisherQos, SubscriberQos, TopicQos,
};
use zerodds_qos::{
    DurabilityKind, DurabilityQosPolicy, HistoryKind, HistoryQosPolicy, ReliabilityKind,
    ReliabilityQosPolicy, ResourceLimitsQosPolicy,
};

/// `@key id`, one value.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Keyed {
    id: u32,
    n: u32,
}

impl DdsType for Keyed {
    const TYPE_NAME: &'static str = "test::KeyedDepth";
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.id.to_le_bytes());
        out.extend_from_slice(&self.n.to_le_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < 8 {
            return Err(DecodeError::Invalid {
                what: "truncated Keyed",
            });
        }
        Ok(Self {
            id: u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            n: u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        })
    }

    fn encode_key_holder_be(&self, holder: &mut zerodds_dcps::dds_type::PlainCdr2BeKeyHolder) {
        holder.write_u32(self.id);
    }
}

fn cfg() -> RuntimeConfig {
    RuntimeConfig {
        tick_period: Duration::from_millis(20),
        spdp_period: Duration::from_millis(100),
        ..RuntimeConfig::default()
    }
}

fn reliable() -> ReliabilityQosPolicy {
    ReliabilityQosPolicy {
        kind: ReliabilityKind::Reliable,
        max_blocking_time: zerodds_qos::Duration::from_millis(500),
    }
}

fn tl() -> DurabilityQosPolicy {
    DurabilityQosPolicy {
        kind: DurabilityKind::TransientLocal,
    }
}

/// Writes `samples` before any reader exists, then starts a late joiner in a
/// second participant and returns what it receives within the window.
fn late_joiner_receives(
    topic: &str,
    history: HistoryQosPolicy,
    samples: &[Keyed],
    expect: usize,
) -> Vec<Keyed> {
    let domain = common::unique_domain();
    let factory = DomainParticipantFactory::instance();
    let pub_p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg())
        .expect("pub participant");
    let t = pub_p
        .create_topic::<Keyed>(topic, TopicQos::default())
        .expect("topic");
    let writer = pub_p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Keyed>(
            &t,
            DataWriterQos {
                reliability: reliable(),
                durability: tl(),
                history,
                resource_limits: ResourceLimitsQosPolicy::default(),
                ..Default::default()
            },
        )
        .expect("writer");
    for s in samples {
        writer.write(s).expect("write");
    }
    std::thread::sleep(Duration::from_millis(200));

    let sub_p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg())
        .expect("sub participant");
    let t = sub_p
        .create_topic::<Keyed>(topic, TopicQos::default())
        .expect("topic");
    let reader = sub_p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Keyed>(
            &t,
            DataReaderQos {
                reliability: reliable(),
                durability: tl(),
                history: HistoryQosPolicy {
                    kind: HistoryKind::KeepAll,
                    depth: 1,
                },
                ..Default::default()
            },
        )
        .expect("reader");
    reader
        .wait_for_matched_publication(1, common::match_timeout())
        .expect("match");
    let mut got = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        let _ = reader.wait_for_data(Duration::from_millis(100));
        if let Ok(taken) = reader.take() {
            got.extend(taken);
        }
        // Past the expected count, keep listening briefly: a surplus is
        // what this test is about.
        if got.len() >= expect {
            std::thread::sleep(Duration::from_millis(500));
            if let Ok(taken) = reader.take() {
                got.extend(taken);
            }
            break;
        }
    }
    drop(writer);
    got
}

#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn keep_last_one_replays_only_the_newest_sample() {
    let samples: Vec<Keyed> = (1..=5).map(|n| Keyed { id: 0, n }).collect();
    let got = late_joiner_receives(
        "TlDepthOne",
        HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 1,
        },
        &samples,
        1,
    );
    assert_eq!(
        got,
        vec![Keyed { id: 0, n: 5 }],
        "KEEP_LAST(1) keeps the newest"
    );
}

#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn keep_last_one_replays_the_newest_sample_of_every_instance() {
    let mut samples = Vec::new();
    for n in 1..=4 {
        for id in 0..3 {
            samples.push(Keyed { id, n });
        }
    }
    let mut got = late_joiner_receives(
        "TlDepthPerInstance",
        HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 1,
        },
        &samples,
        3,
    );
    got.sort_by_key(|k| k.id);
    assert_eq!(
        got,
        vec![
            Keyed { id: 0, n: 4 },
            Keyed { id: 1, n: 4 },
            Keyed { id: 2, n: 4 }
        ],
        "KEEP_LAST(1) is per instance"
    );
}

#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn keep_all_replays_every_sample() {
    let samples: Vec<Keyed> = (1..=100).map(|n| Keyed { id: 0, n }).collect();
    let got = late_joiner_receives(
        "TlKeepAll",
        HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        &samples,
        100,
    );
    assert_eq!(
        got.len(),
        100,
        "KEEP_ALL keeps every sample: {:?}",
        got.first()
    );
    assert_eq!(got.last(), Some(&Keyed { id: 0, n: 100 }));
}

/// VOLATILE + RELIABLE + KEEP_ALL: a burst to a matched reader arrives
/// whole. KEEP_ALL may not drop a sample a reliable reader has not
/// acknowledged (DDS 1.4 §2.2.3.18). Both participants share this process,
/// so the in-process peer path may carry the burst; the RTPS writer's
/// repair window between two processes is not what this measures.
#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn a_keep_all_reliable_burst_arrives_whole() {
    let domain = common::unique_domain();
    let factory = DomainParticipantFactory::instance();
    let pub_p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg())
        .expect("pub participant");
    let sub_p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg())
        .expect("sub participant");
    let keep_all = HistoryQosPolicy {
        kind: HistoryKind::KeepAll,
        depth: 1,
    };
    let t = pub_p
        .create_topic::<Keyed>("KeepAllBurst", TopicQos::default())
        .expect("topic");
    let writer = pub_p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Keyed>(
            &t,
            DataWriterQos {
                reliability: reliable(),
                history: keep_all,
                ..Default::default()
            },
        )
        .expect("writer");
    let t = sub_p
        .create_topic::<Keyed>("KeepAllBurst", TopicQos::default())
        .expect("topic");
    let reader = sub_p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Keyed>(
            &t,
            DataReaderQos {
                reliability: reliable(),
                history: keep_all,
                ..Default::default()
            },
        )
        .expect("reader");
    writer
        .wait_for_matched_subscription(1, common::match_timeout())
        .expect("match");
    const N: u32 = 1000;
    for n in 1..=N {
        writer.write(&Keyed { id: 0, n }).expect("write");
    }
    let mut got: Vec<u32> = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline && got.len() < N as usize {
        let _ = reader.wait_for_data(Duration::from_millis(100));
        if let Ok(taken) = reader.take() {
            got.extend(taken.into_iter().map(|k| k.n));
        }
    }
    let missing: Vec<u32> = (1..=N).filter(|n| !got.contains(n)).collect();
    assert!(
        missing.is_empty(),
        "{} of {N} samples missing, first {:?}",
        missing.len(),
        missing.iter().take(10).collect::<Vec<_>>()
    );
}
