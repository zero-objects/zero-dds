// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! C2(6) — a deterministic [`TestClock`] driving DESTINATION_ORDER=
//! BY_SOURCE_TIMESTAMP (DDS 1.4 §2.2.3.18) through the real write path,
//! not just the tracker hook `destination_order_qos.rs` exercises
//! directly.
//!
//! `RuntimeConfig::time_source` is what `write_user_sample_pooled`
//! (`crates/dcps/src/runtime.rs`) reads for the SOURCE_TIMESTAMP a
//! plain `write()` stamps (RTPS-F1); a [`TestClock`] installed on the
//! **writer's** participant controls that timestamp exactly, without
//! racing a wall clock or sleeping for milliseconds to get two
//! distinguishable instants. `write()` bakes the timestamp into the
//! staged sample synchronously, so setting the clock between two writes
//! is race-free even though the actual UDP send happens later.
//!
//! DESTINATION_ORDER is wired onto the reader's cache-ingest path
//! (`subscriber.rs`'s `ingest_into_cache`), reached only by the
//! `_filtered`/`_w_condition` family (`take_with_info`/`read_with_info`
//! among them) — **not** by the plain `take()`/`read()`, whose live-mode
//! drain applies LIFESPAN and TIME_BASED_FILTER inline but has no
//! DESTINATION_ORDER check at all. That gap is pre-existing, unrelated
//! to the pluggable time source, and not fixed here (out of scope for
//! C2(6)) — `take_with_info()` below is the spec-conformant call that
//! does enforce it.
//!
//! The same-runtime direct channel handoff `ordered_access_e2e.rs` uses
//! bypasses `ingest_into_cache` too, so this needs two real
//! participants — same-host SPDP is unreliable on the macOS loopback
//! (as with `discovery_server_e2e.rs` and the other multi-participant
//! e2e tests in this crate), hence `ignore` there; runs on Linux / the
//! CI bench.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::Duration;

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::qos::{
    DestinationOrderKind, DestinationOrderQosPolicy, HistoryKind, HistoryQosPolicy,
};
use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::time::Time;
use zerodds_dcps::time_source::TestClock;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
    PublisherQos, SubscriberQos, TopicQos,
};

use common::unique_domain;

/// A single-instance, keyed test type: `encode_key_holder_be` writes a
/// fixed instance key regardless of `seq`, so every write lands on the
/// same instance and DESTINATION_ORDER's per-instance `last_delivered_ts`
/// tie-break actually engages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Reading {
    seq: u32,
}

impl DdsType for Reading {
    const TYPE_NAME: &'static str = "test::TimeSourceReading";
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
                what: "truncated Reading",
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

fn fast_cfg() -> RuntimeConfig {
    RuntimeConfig {
        tick_period: Duration::from_millis(20),
        spdp_period: Duration::from_millis(100),
        ..RuntimeConfig::default()
    }
}

fn by_source_timestamp() -> DestinationOrderQosPolicy {
    DestinationOrderQosPolicy {
        kind: DestinationOrderKind::BySourceTimestamp,
    }
}

/// KEEP_ALL: every test in this file writes several samples to the one
/// instance `Reading` always keys to and then asserts on more than the
/// single newest of them — the default KEEP_LAST(1) would (correctly,
/// since `take()`/`read()` now route through the same HISTORY-enforcing
/// cache `take_with_info()` always used) evict everything the
/// DESTINATION_ORDER admission check did not already drop.
fn unbounded_history() -> HistoryQosPolicy {
    HistoryQosPolicy {
        kind: HistoryKind::KeepAll,
        depth: 1,
    }
}

/// Two real participants, one writer (its runtime's `time_source` a
/// [`TestClock`], still controllable via the returned handle) and one
/// reader, both DESTINATION_ORDER=BY_SOURCE_TIMESTAMP, matched before
/// return.
fn rig() -> (
    TestClock,
    zerodds_dcps::publisher::DataWriter<Reading>,
    zerodds_dcps::subscriber::DataReader<Reading>,
) {
    let clock = TestClock::new(Time::new(1_000, 0));
    let pub_cfg = RuntimeConfig {
        time_source: Arc::new(clock.clone()),
        ..fast_cfg()
    };
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let pub_p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), pub_cfg)
        .expect("pub participant");
    let sub_p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), fast_cfg())
        .expect("sub participant");

    let pub_topic = pub_p
        .create_topic::<Reading>("TimeSourceReadings", TopicQos::default())
        .expect("pub topic");
    let sub_topic = sub_p
        .create_topic::<Reading>("TimeSourceReadings", TopicQos::default())
        .expect("sub topic");
    let publisher = pub_p.create_publisher(PublisherQos::default());
    let subscriber = sub_p.create_subscriber(SubscriberQos::default());

    let writer = publisher
        .create_datawriter::<Reading>(
            &pub_topic,
            DataWriterQos {
                destination_order: by_source_timestamp(),
                ..DataWriterQos::default()
            },
        )
        .expect("writer");
    let reader = subscriber
        .create_datareader::<Reading>(
            &sub_topic,
            DataReaderQos {
                destination_order: by_source_timestamp(),
                history: unbounded_history(),
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

    (clock, writer, reader)
}

/// The core claim: a `write()` with no explicit timestamp is stamped
/// with exactly what the writer's runtime `time_source` reads *at that
/// call* (not the platform clock) — set the clock backwards between two
/// writes and the DESTINATION_ORDER=BY_SOURCE_TIMESTAMP reader drops
/// the one carrying the earlier SOURCE_TIMESTAMP, deterministically, no
/// sleep/race required to produce two distinguishable instants.
#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn test_clock_drives_destination_order_deterministically() {
    let (clock, writer, reader) = rig();

    // t=1000: first sample, always delivered (no prior delivery yet).
    writer.write(&Reading { seq: 1 }).expect("write seq=1");

    // t=999: earlier than what was just delivered -> DESTINATION_ORDER
    // must drop it. A real wall clock could not be driven backwards to
    // prove this deterministically; the TestClock can.
    clock.set(Time::new(999, 0));
    writer.write(&Reading { seq: 2 }).expect("write seq=2");

    // t=1001: later -> delivered.
    clock.set(Time::new(1_001, 0));
    writer.write(&Reading { seq: 3 }).expect("write seq=3");

    reader
        .wait_for_data(Duration::from_secs(5))
        .expect("data arrives");
    // A second, generous wait: three separate DATA submessages over
    // real UDP, not a single batch.
    std::thread::sleep(Duration::from_millis(300));

    // `take_with_info()` (spec §2.2.2.5.3.6 `take_w_condition` with ANY
    // masks) demonstrates C2(6)'s actual claim directly. Plain `take()`
    // enforces the identical DESTINATION_ORDER admission now (see
    // `plain_take_enforces_destination_order` in this file) — both route
    // through the same `ingest_into_cache`.
    let delivered: Vec<u32> = reader
        .take_with_info()
        .expect("take_with_info")
        .into_iter()
        .map(|s| s.data.seq)
        .collect();
    assert_eq!(
        delivered,
        vec![1, 3],
        "seq=2's out-of-order (earlier) SOURCE_TIMESTAMP must be dropped, \
         not delivered between seq=1 and seq=3"
    );
}

/// Without advancing the clock, two writes carry the *same*
/// SOURCE_TIMESTAMP; DESTINATION_ORDER=BY_SOURCE_TIMESTAMP requires a
/// strictly greater timestamp to deliver (§2.2.3.18), so the second is
/// dropped even though nothing here raced a wall clock.
#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn same_test_clock_instant_drops_the_second_write() {
    let (_clock, writer, reader) = rig();

    writer.write(&Reading { seq: 1 }).expect("write seq=1");
    writer
        .write(&Reading { seq: 2 })
        .expect("write seq=2 (same instant)");

    reader
        .wait_for_data(Duration::from_secs(5))
        .expect("data arrives");
    std::thread::sleep(Duration::from_millis(300));

    let delivered: Vec<u32> = reader
        .take_with_info()
        .expect("take_with_info")
        .into_iter()
        .map(|s| s.data.seq)
        .collect();
    assert_eq!(delivered, vec![1]);
}

/// Single-participant rig: writer + reader share one `DomainParticipant`,
/// so delivery goes through `DcpsRuntime::intra_runtime_dispatch_alive`
/// directly into the reader's mpsc channel — the exact same channel a real
/// RTPS receive feeds — with no SPDP/UDP discovery involved. Reliable on
/// every host (see `ordered_access_e2e.rs`'s doc comment for the same
/// pattern); `writer.write()` returns only after the dispatch has already
/// happened, so no wait/sleep is needed before reading back.
fn intra_runtime_rig() -> (
    TestClock,
    zerodds_dcps::publisher::DataWriter<Reading>,
    zerodds_dcps::subscriber::DataReader<Reading>,
) {
    let clock = TestClock::new(Time::new(1_000, 0));
    let cfg = RuntimeConfig {
        time_source: Arc::new(clock.clone()),
        ..fast_cfg()
    };
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
        .expect("participant");
    let topic = p
        .create_topic::<Reading>("TimeSourceReadingsIntra", TopicQos::default())
        .expect("topic");
    let publisher = p.create_publisher(PublisherQos::default());
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let writer = publisher
        .create_datawriter::<Reading>(
            &topic,
            DataWriterQos {
                destination_order: by_source_timestamp(),
                ..DataWriterQos::default()
            },
        )
        .expect("writer");
    let reader = subscriber
        .create_datareader::<Reading>(
            &topic,
            DataReaderQos {
                destination_order: by_source_timestamp(),
                history: unbounded_history(),
                ..DataReaderQos::default()
            },
        )
        .expect("reader");
    (clock, writer, reader)
}

/// Regression guard for the gap this file's other tests document inline:
/// the plain `take()` live-mode drain must reject exactly what
/// `take_with_info()` rejects (DDS 1.4 §2.2.3.18) — found while wiring
/// `spatial_bridge.rs` / the time-service DCPS example. Before the fix,
/// `take()` never called `should_deliver_under_destination_order` at all.
#[test]
fn plain_take_enforces_destination_order() {
    let (clock, writer, reader) = intra_runtime_rig();

    writer.write(&Reading { seq: 1 }).expect("write seq=1");
    clock.set(Time::new(999, 0));
    writer
        .write(&Reading { seq: 2 })
        .expect("write seq=2 (earlier ts)");
    clock.set(Time::new(1_001, 0));
    writer.write(&Reading { seq: 3 }).expect("write seq=3");

    let delivered: Vec<u32> = reader
        .take()
        .expect("take")
        .into_iter()
        .map(|s| s.seq)
        .collect();
    assert_eq!(
        delivered,
        vec![1, 3],
        "plain take() must drop seq=2's out-of-order SOURCE_TIMESTAMP, \
         exactly like take_with_info()"
    );
}

/// Same claim through `read()` (non-consuming) — `read()`'s live-mode drain
/// had the identical gap.
#[test]
fn plain_read_enforces_destination_order() {
    let (clock, writer, reader) = intra_runtime_rig();

    writer.write(&Reading { seq: 1 }).expect("write seq=1");
    clock.set(Time::new(999, 0));
    writer
        .write(&Reading { seq: 2 })
        .expect("write seq=2 (earlier ts)");
    clock.set(Time::new(1_001, 0));
    writer.write(&Reading { seq: 3 }).expect("write seq=3");

    let delivered: Vec<u32> = reader
        .read()
        .expect("read")
        .into_iter()
        .map(|s| s.seq)
        .collect();
    assert_eq!(
        delivered,
        vec![1, 3],
        "plain read() must drop seq=2's out-of-order SOURCE_TIMESTAMP, \
         exactly like read_with_info()"
    );
}

/// `read()` is non-consuming: a second call over the exact same
/// still-unread samples must see the identical verdict as the first — the
/// DESTINATION_ORDER/TIME_BASED_FILTER admission state it consults must not
/// be mutated by a call that never took anything.
#[test]
fn plain_read_is_idempotent_under_destination_order() {
    let (clock, writer, reader) = intra_runtime_rig();

    writer.write(&Reading { seq: 1 }).expect("write seq=1");
    clock.set(Time::new(999, 0));
    writer
        .write(&Reading { seq: 2 })
        .expect("write seq=2 (earlier ts)");
    clock.set(Time::new(1_001, 0));
    writer.write(&Reading { seq: 3 }).expect("write seq=3");

    let first: Vec<u32> = reader
        .read()
        .expect("read #1")
        .into_iter()
        .map(|s| s.seq)
        .collect();
    let second: Vec<u32> = reader
        .read()
        .expect("read #2")
        .into_iter()
        .map(|s| s.seq)
        .collect();
    assert_eq!(first, vec![1, 3]);
    assert_eq!(
        second, first,
        "a non-consuming read() must not change the admission verdict \
         for samples it did not take"
    );
}
