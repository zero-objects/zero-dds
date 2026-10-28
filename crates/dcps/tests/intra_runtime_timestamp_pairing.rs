// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Same-runtime intra-process delivery desyncs sample content and
//! `SampleInfo::source_timestamp` — found while wiring the time-service DCPS
//! example (a single participant, writer+reader on one keyed topic).
//!
//! Root cause: `DcpsRuntime::intra_runtime_dispatch_alive`
//! (`crates/dcps/src/runtime.rs`) always sent `source_timestamp: None`
//! (documented at the time as "bypasses the INFO_TS wire path"), instead of
//! the timestamp `write_user_sample_with_key` had just resolved (an
//! explicit override, or the configured `time_source`) and already stamped
//! into the writer's own HistoryCache / wire INFO_TS. `ingest_into_cache`
//! (`crates/dcps/src/subscriber.rs`) then falls back to the reception clock
//! for every sample with no source timestamp — captured **once** for the
//! whole batch it drains in one call. Three rapid writes drained together
//! therefore all received the *same* fallback timestamp, decoupled from
//! each sample's actual identity/content.
//!
//! The real RTPS wire-receive path is not affected: `delivered_to_user_sample`
//! threads `sample.source_timestamp` straight off the same `DeliveredSample`
//! that carries `sample.payload`, so content and timestamp are never taken
//! from different queue positions there. Exercised here too (`#[ignore]`
//! on macOS — real UDP loopback is unreliable on this host, verified by
//! force-running with `--ignored`).
//!
//! The same gap existed in two neighbouring spots, fixed alongside the
//! above: the listener callback's `SampleOrigin { source_timestamp: None,
//! .. }` in the same `intra_runtime_dispatch_alive`, and the TransientLocal
//! same-runtime replay path (`RetainedSample` /
//! `intra_runtime_replay_retained`), which never retained the writer's
//! source timestamp, RTPS sequence number or byte order at all — a late
//! joiner's replayed samples always reported reception order, `-1`, and
//! little-endian regardless of what the writer actually used.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::Duration;

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::qos::{DurabilityKind, DurabilityQosPolicy, HistoryKind, HistoryQosPolicy};
use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::time::Time;
use zerodds_dcps::time_source::TestClock;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
    PublisherQos, SubscriberQos, TopicQos,
};

use common::{isolated_cfg, unique_domain};

/// Single-instance keyed type (`encode_key_holder_be` writes a fixed key
/// regardless of `seq`) — every write lands on the SAME instance, matching
/// the reported scenario exactly ("one keyed instance").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Tick {
    seq: u32,
}

impl DdsType for Tick {
    const TYPE_NAME: &'static str = "test::IntraRuntimeTick";
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.seq.to_le_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let seq = bytes
            .get(..4)
            .and_then(|b| b.try_into().ok())
            .map(u32::from_le_bytes)
            .ok_or(DecodeError::Invalid {
                what: "truncated Tick",
            })?;
        Ok(Self { seq })
    }

    fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
        holder.write_u32(0);
    }

    fn decode_key_holder_be(_bytes: &[u8]) -> Result<Self, DecodeError> {
        Ok(Self::default())
    }
}

/// One participant, writer + reader on the same keyed topic — the exact
/// scenario reported. `history` lets the KeepLast-N / KeepAll neighbour
/// tests reuse this unchanged.
fn intra_runtime_rig(
    history: HistoryQosPolicy,
) -> (
    TestClock,
    zerodds_dcps::publisher::DataWriter<Tick>,
    zerodds_dcps::subscriber::DataReader<Tick>,
) {
    let clock = TestClock::new(Time::new(2_000, 0));
    let cfg = RuntimeConfig {
        time_source: Arc::new(clock.clone()),
        ..isolated_cfg()
    };
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
        .expect("participant");
    let topic = p
        .create_topic::<Tick>("IntraRuntimeTicks", TopicQos::default())
        .expect("topic");
    let publisher = p.create_publisher(PublisherQos::default());
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let writer = publisher
        .create_datawriter::<Tick>(
            &topic,
            DataWriterQos {
                history,
                ..DataWriterQos::default()
            },
        )
        .expect("writer");
    let reader = subscriber
        .create_datareader::<Tick>(
            &topic,
            DataReaderQos {
                history,
                ..DataReaderQos::default()
            },
        )
        .expect("reader");
    (clock, writer, reader)
}

/// Three rapid writes to one keyed instance, each stamped with a distinct
/// explicit instant via the pluggable time source (the same idiom
/// `time_source_destination_order.rs` uses); none are taken in between, so
/// `ingest_into_cache` drains and processes all three together. Asserts
/// each *surviving* taken sample's content matches its own source
/// timestamp — not the batch's, not another sample's.
///
/// `expected_len` is HISTORY's own doing, not this test's claim: under
/// KEEP_LAST(1) only the last of the three writes survives to be taken at
/// all (DDS 1.4 §2.2.3.18) — the pairing claim is about whichever samples
/// *do* survive, however many that is for the given `history`.
fn three_rapid_writes_pair_correctly(history: HistoryQosPolicy, expected_len: usize) {
    let (clock, writer, reader) = intra_runtime_rig(history);

    // Whole-second instants only: the RTPS `HeTimestamp` fraction field
    // truncates (not rounds) a sub-second nanosecond value on its
    // seconds+2^-32-fraction round trip (`time_to_he_timestamp` /
    // `he_timestamp_to_time`, `crates/dcps/src/time.rs`) — an unrelated,
    // pre-existing precision quirk this test must not trip over.
    let stamps = [
        Time::new(2_000, 0),
        Time::new(2_001, 0),
        Time::new(2_002, 0),
    ];
    for (i, ts) in stamps.iter().enumerate() {
        clock.set(*ts);
        writer
            .write(&Tick {
                seq: (i + 1) as u32,
            })
            .unwrap_or_else(|e| panic!("write seq={}: {e}", i + 1));
    }

    let samples = reader.take_with_info().expect("take_with_info");
    assert_eq!(
        samples.len(),
        expected_len,
        "HISTORY must keep exactly the newest {expected_len} of the three \
         rapid writes (DESTINATION_ORDER is BY_RECEPTION_TIMESTAMP by \
         default, so none are dropped for ordering — only HISTORY governs \
         survival here)"
    );
    for s in &samples {
        let seq = s.data.seq as usize;
        let expected = stamps[seq - 1];
        assert_eq!(
            s.info.source_timestamp, expected,
            "seq={seq}'s SampleInfo::source_timestamp must be the instant \
             that specific write was stamped with, not the reception batch's \
             clock or another write's instant (got {:?}, expected {:?})",
            s.info.source_timestamp, expected
        );
    }
}

/// `write_w_timestamp` (DDS 1.4 §2.2.2.4.2.16) must stamp the sample with
/// the *given* timestamp, not the configured `time_source`'s current
/// reading — found adjacent to the same-runtime timestamp gap above:
/// `DataWriter::write_w_timestamp` used the caller's `timestamp` only for
/// local instance-registration bookkeeping and then called plain
/// `write(sample)`, which resolves SOURCE_TIMESTAMP from `time_source`
/// and silently ignored the explicit value entirely.
#[test]
fn write_w_timestamp_stamps_the_sample_with_the_given_timestamp_not_the_clock() {
    let (clock, writer, reader) = intra_runtime_rig(HistoryQosPolicy {
        kind: HistoryKind::KeepLast,
        depth: 1,
    });
    // The ambient time source reads a DIFFERENT instant than the explicit
    // timestamp below, so a bug that falls back to it is caught, not
    // masked by both happening to agree.
    clock.set(Time::new(9_999, 0));

    let given = Time::new(1_234_567, 250_000_000);
    writer
        .write_w_timestamp(&Tick { seq: 1 }, given)
        .expect("write_w_timestamp");

    let samples = reader.take_with_info().expect("take_with_info");
    assert_eq!(samples.len(), 1);
    assert_eq!(
        samples[0].info.source_timestamp, given,
        "write_w_timestamp's explicit timestamp must reach SampleInfo, \
         not the ambient time_source's current reading"
    );
}

#[test]
fn intra_runtime_keep_last_1_pairs_content_with_its_own_timestamp() {
    // KEEP_LAST(1): only the last of the three writes (seq=3) survives to
    // be taken at all (see reader_history_resource_limits.rs for the
    // dedicated HISTORY-eviction tests) — the pairing claim here is about
    // that one survivor.
    three_rapid_writes_pair_correctly(
        HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 1,
        },
        1,
    );
}

/// Neighbour check: KeepLast with depth > 1 — deep enough that none of the
/// three writes are evicted, so all three survive.
#[test]
fn intra_runtime_keep_last_n_pairs_content_with_its_own_timestamp() {
    three_rapid_writes_pair_correctly(
        HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 5,
        },
        3,
    );
}

/// Neighbour check: KeepAll.
#[test]
fn intra_runtime_keep_all_pairs_content_with_its_own_timestamp() {
    three_rapid_writes_pair_correctly(
        HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        3,
    );
}

fn transient_local(history: HistoryQosPolicy) -> (DurabilityQosPolicy, HistoryQosPolicy) {
    (
        DurabilityQosPolicy {
            kind: DurabilityKind::TransientLocal,
        },
        history,
    )
}

/// Neighbour check: the TransientLocal same-runtime replay path
/// (`RetainedSample` / `DcpsRuntime::intra_runtime_replay_retained`). Three
/// writes to one keyed instance happen entirely BEFORE the reader exists —
/// they go into the writer's `retained` buffer, not the live mpsc path —
/// then a late-joining reader on the same participant triggers the replay.
/// Each replayed sample's content must still match its own write instant,
/// not the replay's own dispatch time.
#[test]
fn intra_runtime_replay_pairs_content_with_its_own_timestamp() {
    let clock = TestClock::new(Time::new(4_000, 0));
    let cfg = RuntimeConfig {
        time_source: Arc::new(clock.clone()),
        ..isolated_cfg()
    };
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
        .expect("participant");
    let topic = p
        .create_topic::<Tick>("IntraRuntimeTicksReplay", TopicQos::default())
        .expect("topic");
    let publisher = p.create_publisher(PublisherQos::default());
    let (durability, history) = transient_local(HistoryQosPolicy {
        kind: HistoryKind::KeepLast,
        depth: 3,
    });
    let writer = publisher
        .create_datawriter::<Tick>(
            &topic,
            DataWriterQos {
                durability,
                history,
                ..DataWriterQos::default()
            },
        )
        .expect("writer");

    // All three writes happen before any reader is created — they land in
    // the writer's `retained` buffer, never in a live mpsc channel.
    let stamps = [
        Time::new(4_000, 0),
        Time::new(4_001, 0),
        Time::new(4_002, 0),
    ];
    for (i, ts) in stamps.iter().enumerate() {
        clock.set(*ts);
        writer
            .write(&Tick {
                seq: (i + 1) as u32,
            })
            .unwrap_or_else(|e| panic!("write seq={}: {e}", i + 1));
    }

    // The late-joining reader: creating it triggers
    // `recompute_intra_runtime_routes`, which finds the new writer→reader
    // route and replays `retained` right away — no wait/sleep needed.
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<Tick>(
            &topic,
            DataReaderQos {
                durability,
                history,
                ..DataReaderQos::default()
            },
        )
        .expect("reader");

    let samples = reader.take_with_info().expect("take_with_info");
    assert_eq!(
        samples.len(),
        3,
        "the late joiner must receive all three retained samples"
    );
    for s in &samples {
        let seq = s.data.seq as usize;
        let expected = stamps[seq - 1];
        assert_eq!(
            s.info.source_timestamp, expected,
            "replayed seq={seq} must carry the source timestamp it was \
             written with, not the replay dispatch's own time (got {:?}, \
             expected {:?})",
            s.info.source_timestamp, expected
        );
    }
}

/// Neighbour check: the real RTPS wire-receive path (two participants, real
/// UDP loopback) must show the same correct pairing — it already threads
/// `DeliveredSample.source_timestamp` straight through, so this is a
/// non-regression guard, not expected to have been broken. macOS loopback
/// SPDP is unreliable (see other e2e tests in this crate); verified by
/// force-running with `--ignored`.
#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn remote_path_pairs_content_with_its_own_timestamp() {
    let clock = TestClock::new(Time::new(3_000, 0));
    let cfg = RuntimeConfig {
        time_source: Arc::new(clock.clone()),
        tick_period: Duration::from_millis(20),
        spdp_period: Duration::from_millis(100),
        ..RuntimeConfig::default()
    };
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let pub_p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
        .expect("pub participant");
    let sub_p = factory
        .create_participant_with_config(
            domain,
            DomainParticipantQos::default(),
            RuntimeConfig {
                tick_period: Duration::from_millis(20),
                spdp_period: Duration::from_millis(100),
                ..RuntimeConfig::default()
            },
        )
        .expect("sub participant");
    let pub_topic = pub_p
        .create_topic::<Tick>("IntraRuntimeTicksRemote", TopicQos::default())
        .expect("pub topic");
    let sub_topic = sub_p
        .create_topic::<Tick>("IntraRuntimeTicksRemote", TopicQos::default())
        .expect("sub topic");
    let publisher = pub_p.create_publisher(PublisherQos::default());
    let subscriber = sub_p.create_subscriber(SubscriberQos::default());
    // KEEP_ALL: this test is about content/timestamp pairing across the
    // remote wire path, not about HISTORY eviction (which
    // reader_history_resource_limits.rs covers) — the default
    // KEEP_LAST(1) would otherwise correctly evict seq 1 and 2 before
    // this test ever gets to look at them.
    let keep_all = HistoryQosPolicy {
        kind: HistoryKind::KeepAll,
        depth: 1,
    };
    let writer = publisher
        .create_datawriter::<Tick>(
            &pub_topic,
            DataWriterQos {
                history: keep_all,
                ..DataWriterQos::default()
            },
        )
        .expect("writer");
    let reader = subscriber
        .create_datareader::<Tick>(
            &sub_topic,
            DataReaderQos {
                history: keep_all,
                ..DataReaderQos::default()
            },
        )
        .expect("reader");
    writer
        .wait_for_matched_subscription(1, Duration::from_secs(15))
        .expect("writer sees reader");
    reader
        .wait_for_matched_publication(1, Duration::from_secs(15))
        .expect("reader sees writer");

    let stamps = [
        Time::new(3_000, 0),
        Time::new(3_001, 0),
        Time::new(3_002, 0),
    ];
    for (i, ts) in stamps.iter().enumerate() {
        clock.set(*ts);
        writer
            .write(&Tick {
                seq: (i + 1) as u32,
            })
            .unwrap_or_else(|e| panic!("write seq={}: {e}", i + 1));
    }

    reader
        .wait_for_data(Duration::from_secs(5))
        .expect("data arrives");
    std::thread::sleep(Duration::from_millis(300));

    let samples = reader.take_with_info().expect("take_with_info");
    assert_eq!(samples.len(), 3);
    for s in &samples {
        let seq = s.data.seq as usize;
        let expected = stamps[seq - 1];
        assert_eq!(
            s.info.source_timestamp, expected,
            "remote path: seq={seq}'s source_timestamp must match its own \
             write instant"
        );
    }
}

/// DDS 1.4 §2.2.2.2.1.32 `get_current_time`: "the current value of the
/// time that the service uses to time-stamp data-writes". With a test clock
/// as the runtime's time source the participant reads that clock, the one
/// `intra_runtime_rig`'s writes are stamped with. The participant had no
/// get_current_time; the free function reads the system clock.
#[test]
fn get_current_time_is_the_clock_writes_are_stamped_with() {
    let clock = TestClock::new(Time::new(6_000, 0));
    let cfg = RuntimeConfig {
        time_source: Arc::new(clock.clone()),
        ..isolated_cfg()
    };
    let p = DomainParticipantFactory::instance()
        .create_participant_with_config(unique_domain(), DomainParticipantQos::default(), cfg)
        .expect("participant");
    assert_eq!(p.get_current_time().unwrap(), Time::new(6_000, 0));
    clock.advance(zerodds_dcps::time::Duration::new(1, 500));
    assert_eq!(p.get_current_time().unwrap(), Time::new(6_001, 500));
    let _ = p.delete_contained_entities();
    let _ = DomainParticipantFactory::instance().delete_participant(&p);
}
