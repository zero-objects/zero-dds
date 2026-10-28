// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS 1.4 §2.2.3.18 HISTORY (`KEEP_LAST(depth)`) and §2.2.3.19
//! RESOURCE_LIMITS (`max_samples`, `max_samples_per_instance`,
//! `max_instances`) on the reader's own cache — `ingest_into_cache`
//! (`crates/dcps/src/subscriber.rs`) never enforced either before this file
//! existed: an unbounded number of not-yet-taken samples per instance (or
//! total, or instances) would simply accumulate.
//!
//! Design, sourced (not assumed) from RTI Connext's documented behaviour
//! (community.rti.com, RESOURCE_LIMITS QosPolicy / Missing Samples) and the
//! OMG DDS spec's own SAMPLE_REJECTED_STATUS wording ("a rejected sample may
//! be accepted in the future" — i.e. rejection never destroys data the
//! reader already holds):
//!
//! - `HISTORY = KEEP_LAST(depth)`: silently evicts the OLDEST not-yet-taken
//!   sample of an instance once it already holds `depth` — ordinary
//!   QoS-directed retention, not a failure, so neither status changes.
//! - RESOURCE_LIMITS ceiling exceeded (`max_samples`,
//!   `max_samples_per_instance`, `max_instances`) on a **RELIABLE** reader:
//!   the incoming sample is refused outright (`SAMPLE_REJECTED`); nothing
//!   already cached is touched.
//! - `max_samples` or `max_samples_per_instance` on a **BEST_EFFORT**
//!   reader: the incoming sample is admitted and the oldest one discarded
//!   (§2.2.3.18 KEEP_ALL: "If the reliability kind is BEST_EFFORT, then the
//!   old values will be discarded"). No status changes, as under KEEP_LAST:
//!   the discarded sample was received, and SAMPLE_LOST is a sample "never
//!   received" (§2.2.4.1). These used to count as SAMPLE_LOST, as RTI
//!   Connext reports them.
//! - `max_instances` has no existing instance to sacrifice for a
//!   best-effort admission, so the new instance's sample is refused —
//!   `SAMPLE_REJECTED`, "A (received) sample has been rejected", nothing
//!   created. It used to count as SAMPLE_LOST.
//!
//! All rigs are single-participant (same-runtime intra-process dispatch, as
//! `time_source_destination_order.rs`'s `intra_runtime_rig` and this
//! crate's other same-runtime tests use) — no UDP loopback, no discovery
//! wait.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::listener::DataReaderListener;
use zerodds_dcps::psm_constants::status as bits;
use zerodds_dcps::qos::{
    HistoryKind, HistoryQosPolicy, ReliabilityKind, ReliabilityQosPolicy, ResourceLimitsQosPolicy,
};
use zerodds_dcps::status::{SampleLostStatus, SampleRejectedStatus, SampleRejectedStatusKind};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
    HANDLE_NIL, InstanceHandle, PublisherQos, SubscriberQos, TopicQos,
};

use common::isolated_cfg;

/// `id` is the instance key; `seq` distinguishes writes to the same
/// instance in arrival order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Reading {
    id: u32,
    seq: u32,
}

impl DdsType for Reading {
    const TYPE_NAME: &'static str = "test::ReaderLimitsReading";
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

/// The participant and the domain lease of one test: bound first
/// (`let (_rig, writer, reader)`), so it drops last — the participant is
/// deleted, then the domain goes back. A lease held for the whole process
/// (`unique_domain`) made this binary need one domain per test, more than
/// the 26 the Linux host has off its ephemeral port range.
struct Rig {
    _participant: common::ParticipantGuard,
    _lease: common::DomainLease,
}

fn rig(
    history: HistoryQosPolicy,
    resource_limits: ResourceLimitsQosPolicy,
    reliable: bool,
) -> (
    Rig,
    zerodds_dcps::publisher::DataWriter<Reading>,
    zerodds_dcps::subscriber::DataReader<Reading>,
) {
    rig_with(history, resource_limits, reliable, true)
}

/// `rig` with the writer's `autodispose_unregistered_instances`
/// (§2.2.3.21) named.
fn rig_with(
    history: HistoryQosPolicy,
    resource_limits: ResourceLimitsQosPolicy,
    reliable: bool,
    autodispose_unregistered_instances: bool,
) -> (
    Rig,
    zerodds_dcps::publisher::DataWriter<Reading>,
    zerodds_dcps::subscriber::DataReader<Reading>,
) {
    let lease = common::lease_domain();
    let factory = DomainParticipantFactory::instance();
    let p = factory
        .create_participant_with_config(lease.id(), DomainParticipantQos::default(), isolated_cfg())
        .expect("participant");
    let topic = p
        .create_topic::<Reading>("ReaderLimitsReadings", TopicQos::default())
        .expect("topic");
    let publisher = p.create_publisher(PublisherQos::default());
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reliability = ReliabilityQosPolicy {
        kind: if reliable {
            ReliabilityKind::Reliable
        } else {
            ReliabilityKind::BestEffort
        },
        ..Default::default()
    };
    let writer = publisher
        .create_datawriter::<Reading>(
            &topic,
            DataWriterQos {
                history,
                // The limits under test are the reader's. A writer holds its
                // own (§2.2.2.4.2.11): with `max_instances = 1` it refuses the
                // second instance before the reader could.
                resource_limits: ResourceLimitsQosPolicy::default(),
                reliability,
                writer_data_lifecycle: zerodds_dcps::qos::WriterDataLifecycleQosPolicy {
                    autodispose_unregistered_instances,
                },
                ..Default::default()
            },
        )
        .expect("writer");
    let reader = subscriber
        .create_datareader::<Reading>(
            &topic,
            DataReaderQos {
                history,
                resource_limits,
                reliability,
                ..Default::default()
            },
        )
        .expect("reader");
    (
        Rig {
            _participant: common::ParticipantGuard::new(p),
            _lease: lease,
        },
        writer,
        reader,
    )
}

fn seqs(samples: &[zerodds_dcps::Sample<Reading>]) -> Vec<u32> {
    let mut v: Vec<u32> = samples.iter().map(|s| s.data.seq).collect();
    v.sort_unstable();
    v
}

/// Same as `seqs`, for the plain `take()`/`read()` return type (bare
/// `Reading`, no `SampleInfo`).
fn plain_seqs(samples: &[Reading]) -> Vec<u32> {
    let mut v: Vec<u32> = samples.iter().map(|s| s.seq).collect();
    v.sort_unstable();
    v
}

// ============================================================================
// HISTORY = KEEP_LAST(depth)
// ============================================================================

#[test]
fn keep_last_1_keeps_only_the_newest_per_instance() {
    let (_rig, writer, reader) = rig(
        HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 1,
        },
        ResourceLimitsQosPolicy::default(),
        true,
    );
    for seq in 1..=3 {
        writer.write(&Reading { id: 1, seq }).expect("write");
    }
    let samples = reader.take_with_info().expect("take_with_info");
    assert_eq!(
        seqs(&samples),
        vec![3],
        "KEEP_LAST(1) must keep only the newest not-yet-taken sample"
    );
}

#[test]
fn keep_last_n_keeps_the_newest_n_per_instance() {
    let (_rig, writer, reader) = rig(
        HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 3,
        },
        ResourceLimitsQosPolicy::default(),
        true,
    );
    for seq in 1..=5 {
        writer.write(&Reading { id: 1, seq }).expect("write");
    }
    let samples = reader.take_with_info().expect("take_with_info");
    assert_eq!(
        seqs(&samples),
        vec![3, 4, 5],
        "KEEP_LAST(3) must keep exactly the 3 newest, evicting seq 1 and 2"
    );
}

#[test]
fn keep_last_is_independent_per_instance() {
    let (_rig, writer, reader) = rig(
        HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 1,
        },
        ResourceLimitsQosPolicy::default(),
        true,
    );
    writer.write(&Reading { id: 1, seq: 1 }).expect("write");
    writer.write(&Reading { id: 2, seq: 1 }).expect("write");
    writer.write(&Reading { id: 1, seq: 2 }).expect("write");
    writer.write(&Reading { id: 2, seq: 2 }).expect("write");
    let samples = reader.take_with_info().expect("take_with_info");
    let mut by_id: Vec<(u32, u32)> = samples.iter().map(|s| (s.data.id, s.data.seq)).collect();
    by_id.sort_unstable();
    assert_eq!(
        by_id,
        vec![(1, 2), (2, 2)],
        "KEEP_LAST(1) bounds each instance on its own -- instance 2's \
         arrivals must not evict instance 1's, and vice versa"
    );
}

#[test]
fn keep_last_eviction_does_not_bump_any_status() {
    let (_rig, writer, reader) = rig(
        HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 1,
        },
        ResourceLimitsQosPolicy::default(),
        true,
    );
    for seq in 1..=3 {
        writer.write(&Reading { id: 1, seq }).expect("write");
    }
    let _ = reader.take_with_info().expect("take_with_info");
    assert_eq!(
        reader.sample_rejected_status().total_count,
        0,
        "ordinary HISTORY eviction is not a rejection"
    );
    assert_eq!(
        reader.sample_lost_count(),
        0,
        "ordinary HISTORY eviction is not a loss"
    );
}

// ============================================================================
// RESOURCE_LIMITS.max_samples_per_instance (HISTORY = KEEP_ALL, so nothing
// evicts on its own -- the resource limit is the only ceiling in play).
// ============================================================================

fn keep_all_qos() -> HistoryQosPolicy {
    HistoryQosPolicy {
        kind: HistoryKind::KeepAll,
        depth: 1,
    }
}

#[test]
fn keep_all_max_samples_per_instance_rejects_when_reliable() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_samples_per_instance: 2,
            ..Default::default()
        },
        true,
    );
    for seq in 1..=3 {
        writer.write(&Reading { id: 1, seq }).expect("write");
    }
    let samples = reader.take_with_info().expect("take_with_info");
    assert_eq!(
        seqs(&samples),
        vec![1, 2],
        "the 3rd sample must be rejected outright -- the first two, already \
         admitted, must be untouched"
    );
    let status = reader.sample_rejected_status();
    assert_eq!(status.total_count, 1);
    assert_eq!(
        status.last_reason,
        SampleRejectedStatusKind::RejectedBySamplesPerInstanceLimit
    );
    assert_ne!(status.last_instance_handle, HANDLE_NIL);
    assert_eq!(reader.sample_lost_count(), 0);
}

#[test]
fn keep_all_max_samples_per_instance_discards_the_oldest_when_best_effort() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_samples_per_instance: 2,
            ..Default::default()
        },
        false,
    );
    for seq in 1..=3 {
        writer.write(&Reading { id: 1, seq }).expect("write");
    }
    let samples = reader.take_with_info().expect("take_with_info");
    assert_eq!(
        seqs(&samples),
        vec![2, 3],
        "best-effort admits the 3rd sample and evicts the instance's own \
         oldest (seq 1) to make room"
    );
    assert_eq!(
        reader.sample_lost_count(),
        0,
        "the discarded sample was received"
    );
    assert_eq!(reader.sample_rejected_status().total_count, 0);
}

// ============================================================================
// RESOURCE_LIMITS.max_samples (total, across every instance).
// ============================================================================

#[test]
fn keep_all_max_samples_total_rejects_when_reliable() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_samples: 2,
            ..Default::default()
        },
        true,
    );
    writer.write(&Reading { id: 1, seq: 1 }).expect("write");
    writer.write(&Reading { id: 2, seq: 1 }).expect("write");
    writer.write(&Reading { id: 3, seq: 1 }).expect("write");
    let samples = reader.take_with_info().expect("take_with_info");
    let mut ids: Vec<u32> = samples.iter().map(|s| s.data.id).collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        vec![1, 2],
        "the 3rd instance's sample must be rejected -- the first two \
         instances, already admitted, must be untouched"
    );
    let status = reader.sample_rejected_status();
    assert_eq!(status.total_count, 1);
    assert_eq!(
        status.last_reason,
        SampleRejectedStatusKind::RejectedBySamplesLimit
    );
}

/// DDS 1.4 §2.2.4.1: "a rejected sample may be accepted in the future".
/// A RELIABLE sample RESOURCE_LIMITS refuses is held back with everything
/// after it and delivered, in order, once the application has made room.
/// It used to be dropped after the RTPS layer had acknowledged it, so no
/// writer sent it again.
///
/// The writer is in the reader's participant, so it waits for the reader's
/// room itself (§2.2.2.4.2.11): the reader takes its limit and one sample
/// more, which it refuses and holds, and the write after that times out.
#[test]
fn a_reliable_sample_refused_for_room_arrives_once_there_is_room() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_samples: 2,
            ..Default::default()
        },
        true,
    );
    for seq in 1..=3 {
        writer.write(&Reading { id: 1, seq }).expect("write");
    }
    assert!(
        matches!(
            writer.write(&Reading { id: 1, seq: 4 }),
            Err(zerodds_dcps::error::DdsError::Timeout)
        ),
        "a full reader holds its writer back"
    );
    let mut got = seqs(&reader.take_with_info().expect("take_with_info"));
    assert_eq!(got, vec![1, 2]);
    for seq in 4..=5 {
        writer
            .write(&Reading { id: 1, seq })
            .expect("write after a take");
    }
    for _ in 0..2 {
        got.extend(seqs(&reader.take_with_info().expect("take_with_info")));
    }
    assert_eq!(got, vec![1, 2, 3, 4, 5], "every sample, in order");
    // Two refusals — the 3rd sample at the first take, the 5th at the
    // second — and none for a held sample refused a second time.
    assert_eq!(reader.sample_rejected_status().total_count, 2);
    assert_eq!(reader.sample_lost_count(), 0);
}

/// DDS 1.4 §2.2.2.4.2.13: dispose "may block and return TIMEOUT" where
/// `write` would (§2.2.2.4.2.11). A full RELIABLE reader holds its writer
/// back: dispose returns TIMEOUT and the instance stays ALIVE at the
/// writer; once the reader has room, dispose goes through and the reader
/// gets the marker. dispose returned OK and disposed the instance at the
/// writer with no marker sent.
#[test]
fn a_dispose_a_full_reader_holds_back_times_out() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_samples: 2,
            ..Default::default()
        },
        true,
    );
    for seq in 1..=3 {
        writer.write(&Reading { id: 1, seq }).expect("write");
    }
    assert!(matches!(
        writer.write(&Reading { id: 1, seq: 4 }),
        Err(zerodds_dcps::error::DdsError::Timeout)
    ));
    let instance = Reading { id: 1, seq: 0 };
    assert!(
        matches!(
            writer.dispose(&instance, HANDLE_NIL),
            Err(zerodds_dcps::error::DdsError::Timeout)
        ),
        "a dispose the reader has no room for returned"
    );
    let handle = writer.lookup_instance(&instance);
    assert_eq!(
        writer
            .instance_tracker()
            .get_by_handle(handle)
            .map(|state| state.kind),
        Some(zerodds_dcps::sample_info::InstanceStateKind::Alive),
        "the dispose that timed out changed the instance"
    );

    let mut taken = Vec::new();
    for _ in 0..2 {
        taken.extend(reader.take_with_info().expect("take"));
    }
    assert_eq!(seqs(&taken), vec![1, 2, 3]);
    writer
        .dispose(&instance, HANDLE_NIL)
        .expect("dispose with room");
    let marker = reader
        .take_with_info()
        .expect("take the marker")
        .into_iter()
        .find(|s| !s.info.valid_data)
        .expect("the dispose marker");
    assert_eq!(
        marker.info.instance_state,
        zerodds_dcps::sample_info::InstanceStateKind::NotAliveDisposed
    );
}

/// DDS 1.4 §2.2.2.4.2.7: unregister_instance "may block and return TIMEOUT
/// under the same circumstances described for the write operation". As
/// `a_dispose_a_full_reader_holds_back_times_out`: the instance stays
/// registered at the writer until the reader has room for the marker.
#[test]
fn an_unregister_a_full_reader_holds_back_times_out() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_samples: 2,
            ..Default::default()
        },
        true,
    );
    for seq in 1..=3 {
        writer.write(&Reading { id: 1, seq }).expect("write");
    }
    assert!(matches!(
        writer.write(&Reading { id: 1, seq: 4 }),
        Err(zerodds_dcps::error::DdsError::Timeout)
    ));
    let instance = Reading { id: 1, seq: 0 };
    assert!(
        matches!(
            writer.unregister_instance(&instance, HANDLE_NIL),
            Err(zerodds_dcps::error::DdsError::Timeout)
        ),
        "an unregister the reader has no room for returned"
    );
    let handle = writer.lookup_instance(&instance);
    assert_eq!(
        writer
            .instance_tracker()
            .get_by_handle(handle)
            .map(|state| state.kind),
        Some(zerodds_dcps::sample_info::InstanceStateKind::Alive),
        "the unregister that timed out changed the instance"
    );

    let mut taken = Vec::new();
    for _ in 0..2 {
        taken.extend(reader.take_with_info().expect("take"));
    }
    assert_eq!(seqs(&taken), vec![1, 2, 3]);
    writer
        .unregister_instance(&instance, HANDLE_NIL)
        .expect("unregister with room");
    let marker = reader
        .take_with_info()
        .expect("take the marker")
        .into_iter()
        .find(|s| !s.info.valid_data)
        .expect("the unregister marker");
    assert_ne!(
        marker.info.instance_state,
        zerodds_dcps::sample_info::InstanceStateKind::Alive
    );
}

#[test]
fn keep_all_max_samples_total_discards_the_oldest_when_best_effort() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_samples: 2,
            ..Default::default()
        },
        false,
    );
    writer.write(&Reading { id: 1, seq: 1 }).expect("write");
    writer.write(&Reading { id: 2, seq: 1 }).expect("write");
    writer.write(&Reading { id: 3, seq: 1 }).expect("write");
    let samples = reader.take_with_info().expect("take_with_info");
    let mut ids: Vec<u32> = samples.iter().map(|s| s.data.id).collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        vec![2, 3],
        "best-effort admits instance 3's sample and evicts the globally \
         oldest cached sample (instance 1's) to make room"
    );
    assert_eq!(
        reader.sample_lost_count(),
        0,
        "the discarded sample was received"
    );
    assert_eq!(reader.sample_rejected_status().total_count, 0);
}

// ============================================================================
// RESOURCE_LIMITS.max_instances.
// ============================================================================

#[test]
fn max_instances_rejects_when_reliable() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_instances: 1,
            ..Default::default()
        },
        true,
    );
    writer.write(&Reading { id: 1, seq: 1 }).expect("write");
    writer.write(&Reading { id: 2, seq: 1 }).expect("write");
    let samples = reader.take_with_info().expect("take_with_info");
    let ids: Vec<u32> = samples.iter().map(|s| s.data.id).collect();
    assert_eq!(
        ids,
        vec![1],
        "the 2nd, brand-new instance must be rejected outright"
    );
    let status = reader.sample_rejected_status();
    assert_eq!(status.total_count, 1);
    assert_eq!(
        status.last_reason,
        SampleRejectedStatusKind::RejectedByInstancesLimit
    );
    assert_eq!(
        status.last_instance_handle, HANDLE_NIL,
        "no instance was ever created for the refused sample"
    );
}

#[test]
fn max_instances_rejects_when_best_effort() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_instances: 1,
            ..Default::default()
        },
        false,
    );
    writer.write(&Reading { id: 1, seq: 1 }).expect("write");
    writer.write(&Reading { id: 2, seq: 1 }).expect("write");
    let samples = reader.take_with_info().expect("take_with_info");
    let ids: Vec<u32> = samples.iter().map(|s| s.data.id).collect();
    assert_eq!(
        ids,
        vec![1],
        "there is no existing instance to sacrifice for the 2nd -- it is \
         refused, not admitted"
    );
    let status = reader.sample_rejected_status();
    assert_eq!(status.total_count, 1);
    assert_eq!(
        status.last_reason,
        SampleRejectedStatusKind::RejectedByInstancesLimit
    );
    assert_eq!(status.last_instance_handle, HANDLE_NIL);
    assert_eq!(reader.sample_lost_count(), 0, "the sample was received");
}

// ============================================================================
// Listener callbacks (DDS 1.4 §2.2.4.2.6.2 / .3) -- the delta-poll dispatch
// convention this crate already uses for every other status
// (`poll_sample_lost`/`poll_sample_rejected`, `subscriber.rs`): the
// listener fires the next time the corresponding status accessor is
// called, same as `on_requested_deadline_missed` etc.
// ============================================================================

/// How long a test waits for a listener call it expects.
const CALL: std::time::Duration = std::time::Duration::from_secs(5);
/// How long a test watches for a second call it does not expect.
const NO_SECOND_CALL: std::time::Duration = std::time::Duration::from_millis(500);

/// The calls a listener received, and a way to wait for them: the
/// runtime's listener thread may make the call the accessor asked for
/// (DDS 1.4 §2.2.4.1 calls a listener when the status changes, not when
/// the application reads it), so a test waits for the call instead of
/// assuming it ran on its own thread.
#[derive(Default)]
struct Counters {
    calls: std::sync::Mutex<(u32, u32)>,
    moved: std::sync::Condvar,
}

impl Counters {
    /// `(rejected, lost)` once `done` holds for them, or as they stand
    /// after `within`.
    fn wait_for(
        &self,
        within: std::time::Duration,
        done: impl Fn((u32, u32)) -> bool,
    ) -> (u32, u32) {
        let deadline = std::time::Instant::now() + within;
        let mut calls = self.calls.lock().expect("calls");
        while !done(*calls) {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                break;
            }
            calls = self.moved.wait_timeout(calls, left).expect("calls").0;
        }
        *calls
    }
}

impl DataReaderListener for Counters {
    fn on_sample_rejected(&self, _reader: InstanceHandle, _status: SampleRejectedStatus) {
        self.calls.lock().expect("calls").0 += 1;
        self.moved.notify_all();
    }
    fn on_sample_lost(&self, _reader: InstanceHandle, _status: SampleLostStatus) {
        self.calls.lock().expect("calls").1 += 1;
        self.moved.notify_all();
    }
}

#[test]
fn on_sample_rejected_listener_fires() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_samples_per_instance: 1,
            ..Default::default()
        },
        true,
    );
    let counters = Arc::new(Counters::default());
    reader.set_listener(Some(counters.clone()), bits::ANY);
    writer.write(&Reading { id: 1, seq: 1 }).expect("write");
    writer.write(&Reading { id: 1, seq: 2 }).expect("write");
    let _ = reader.take_with_info().expect("take_with_info");
    // The accessor asks for the listener's look; the listener thread may
    // be the one that makes the call.
    let _ = reader.sample_rejected_status();
    assert_eq!(counters.wait_for(CALL, |(rejected, _)| rejected >= 1).0, 1);
    // One rejection is one call: a second look finds nothing unread. The
    // double call this guards against came within microseconds.
    let _ = reader.sample_rejected_status();
    assert_eq!(
        counters
            .wait_for(NO_SECOND_CALL, |(rejected, _)| rejected >= 2)
            .0,
        1
    );
}

/// A BEST_EFFORT reader refuses the sample of an instance past
/// `max_instances`: `on_sample_rejected`, once, and no `on_sample_lost`.
/// The SAMPLE_LOST of a BEST_EFFORT stream with a gap is
/// `runtime::tests::samples_a_best_effort_reader_steps_over_are_sample_lost`.
#[test]
fn on_sample_rejected_listener_fires_for_a_best_effort_reader() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_instances: 1,
            ..Default::default()
        },
        false,
    );
    let counters = Arc::new(Counters::default());
    reader.set_listener(Some(counters.clone()), bits::ANY);
    writer.write(&Reading { id: 1, seq: 1 }).expect("write");
    writer.write(&Reading { id: 2, seq: 1 }).expect("write");
    let _ = reader.take_with_info().expect("take_with_info");
    let _ = reader.sample_rejected_status();
    assert_eq!(
        counters.wait_for(CALL, |(rejected, _)| rejected >= 1),
        (1, 0)
    );
    let _ = reader.sample_rejected_status();
    let _ = reader.sample_lost_count();
    assert_eq!(
        counters.wait_for(NO_SECOND_CALL, |(rejected, lost)| rejected >= 2
            || lost >= 1),
        (1, 0)
    );
}

// ============================================================================
// Round 3 regression guard: `take()`/`read()` must observe the identical
// HISTORY/RESOURCE_LIMITS outcome as `take_with_info()` above -- both now
// route through the same `ingest_into_cache` admission instead of a second,
// unenforced bypass. One representative case per HISTORY/RESOURCE_LIMITS
// category through `take()` (the direct, consuming counterpart of
// `take_with_info()`), plus `read()` idempotency checks for the two
// mechanisms: the underlying admission decision is made once, in
// `ingest_into_cache`, regardless of which accessor triggers it, so this is
// not a mechanical re-run of every case above -- it is the specific claim
// this round's fix makes, that was previously false for plain take()/read().
// ============================================================================

#[test]
fn plain_take_keep_last_1_keeps_only_the_newest_per_instance() {
    let (_rig, writer, reader) = rig(
        HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 1,
        },
        ResourceLimitsQosPolicy::default(),
        true,
    );
    for seq in 1..=3 {
        writer.write(&Reading { id: 1, seq }).expect("write");
    }
    let samples = reader.take().expect("take");
    assert_eq!(
        plain_seqs(&samples),
        vec![3],
        "plain take() must enforce KEEP_LAST(1) exactly like take_with_info()"
    );
}

#[test]
fn plain_take_keep_last_n_keeps_the_newest_n_per_instance() {
    let (_rig, writer, reader) = rig(
        HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 3,
        },
        ResourceLimitsQosPolicy::default(),
        true,
    );
    for seq in 1..=5 {
        writer.write(&Reading { id: 1, seq }).expect("write");
    }
    let samples = reader.take().expect("take");
    assert_eq!(
        plain_seqs(&samples),
        vec![3, 4, 5],
        "plain take() must enforce KEEP_LAST(3) exactly like take_with_info()"
    );
}

#[test]
fn plain_take_keep_last_is_independent_per_instance() {
    let (_rig, writer, reader) = rig(
        HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 1,
        },
        ResourceLimitsQosPolicy::default(),
        true,
    );
    writer.write(&Reading { id: 1, seq: 1 }).expect("write");
    writer.write(&Reading { id: 2, seq: 1 }).expect("write");
    writer.write(&Reading { id: 1, seq: 2 }).expect("write");
    writer.write(&Reading { id: 2, seq: 2 }).expect("write");
    let samples = reader.take().expect("take");
    let mut by_id: Vec<(u32, u32)> = samples.iter().map(|s| (s.id, s.seq)).collect();
    by_id.sort_unstable();
    assert_eq!(
        by_id,
        vec![(1, 2), (2, 2)],
        "plain take()'s KEEP_LAST(1) bounds each instance on its own, same \
         as take_with_info()"
    );
}

#[test]
fn plain_take_max_samples_per_instance_rejects_when_reliable() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_samples_per_instance: 2,
            ..Default::default()
        },
        true,
    );
    for seq in 1..=3 {
        writer.write(&Reading { id: 1, seq }).expect("write");
    }
    let samples = reader.take().expect("take");
    assert_eq!(
        plain_seqs(&samples),
        vec![1, 2],
        "plain take() must reject the 3rd sample exactly like take_with_info()"
    );
}

#[test]
fn plain_take_max_samples_per_instance_discards_the_oldest_when_best_effort() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_samples_per_instance: 2,
            ..Default::default()
        },
        false,
    );
    for seq in 1..=3 {
        writer.write(&Reading { id: 1, seq }).expect("write");
    }
    let samples = reader.take().expect("take");
    assert_eq!(
        plain_seqs(&samples),
        vec![2, 3],
        "plain take() must admit the 3rd sample and discard seq 1, exactly \
         like take_with_info()"
    );
}

#[test]
fn plain_take_max_samples_total_rejects_when_reliable() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_samples: 2,
            ..Default::default()
        },
        true,
    );
    writer.write(&Reading { id: 1, seq: 1 }).expect("write");
    writer.write(&Reading { id: 2, seq: 1 }).expect("write");
    writer.write(&Reading { id: 3, seq: 1 }).expect("write");
    let samples = reader.take().expect("take");
    let mut ids: Vec<u32> = samples.iter().map(|s| s.id).collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        vec![1, 2],
        "plain take() must reject the 3rd instance's sample exactly like \
         take_with_info()"
    );
}

#[test]
fn plain_take_max_samples_total_discards_the_oldest_when_best_effort() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_samples: 2,
            ..Default::default()
        },
        false,
    );
    writer.write(&Reading { id: 1, seq: 1 }).expect("write");
    writer.write(&Reading { id: 2, seq: 1 }).expect("write");
    writer.write(&Reading { id: 3, seq: 1 }).expect("write");
    let samples = reader.take().expect("take");
    let mut ids: Vec<u32> = samples.iter().map(|s| s.id).collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        vec![2, 3],
        "plain take() must admit instance 3 and discard instance 1's sample, \
         exactly like take_with_info()"
    );
}

#[test]
fn plain_take_max_instances_rejects_when_reliable() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_instances: 1,
            ..Default::default()
        },
        true,
    );
    writer.write(&Reading { id: 1, seq: 1 }).expect("write");
    writer.write(&Reading { id: 2, seq: 1 }).expect("write");
    let samples = reader.take().expect("take");
    let ids: Vec<u32> = samples.iter().map(|s| s.id).collect();
    assert_eq!(
        ids,
        vec![1],
        "plain take() must reject the 2nd, brand-new instance exactly like \
         take_with_info()"
    );
}

#[test]
fn plain_take_max_instances_rejects_when_best_effort() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_instances: 1,
            ..Default::default()
        },
        false,
    );
    writer.write(&Reading { id: 1, seq: 1 }).expect("write");
    writer.write(&Reading { id: 2, seq: 1 }).expect("write");
    let samples = reader.take().expect("take");
    let ids: Vec<u32> = samples.iter().map(|s| s.id).collect();
    assert_eq!(
        ids,
        vec![1],
        "plain take() has no existing instance to sacrifice for the 2nd, \
         exactly like take_with_info()"
    );
}

#[test]
fn plain_read_keep_last_1_keeps_only_the_newest_per_instance_and_is_idempotent() {
    let (_rig, writer, reader) = rig(
        HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 1,
        },
        ResourceLimitsQosPolicy::default(),
        true,
    );
    for seq in 1..=3 {
        writer.write(&Reading { id: 1, seq }).expect("write");
    }
    let first = reader.read().expect("read #1");
    assert_eq!(
        plain_seqs(&first),
        vec![3],
        "plain read() must enforce KEEP_LAST(1) exactly like take_with_info()"
    );
    let second = reader.read().expect("read #2");
    assert_eq!(
        plain_seqs(&second),
        vec![3],
        "read() does not consume -- a second call over the same \
         still-unread data must see the identical verdict"
    );
}

#[test]
fn plain_read_max_samples_per_instance_rejects_when_reliable_and_is_idempotent() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_samples_per_instance: 2,
            ..Default::default()
        },
        true,
    );
    for seq in 1..=3 {
        writer.write(&Reading { id: 1, seq }).expect("write");
    }
    let first = reader.read().expect("read #1");
    assert_eq!(
        plain_seqs(&first),
        vec![1, 2],
        "plain read() must reject the 3rd sample exactly like take_with_info()"
    );
    let second = reader.read().expect("read #2");
    assert_eq!(
        plain_seqs(&second),
        vec![1, 2],
        "read() does not consume -- a second call must see the identical \
         verdict, and must not itself trigger a further rejection"
    );
    assert_eq!(
        reader.sample_rejected_status().total_count,
        1,
        "the non-consuming read() calls above must not re-run admission \
         on samples already in the cache"
    );
}

// ============================================================================
// Lifecycle markers keep their place in arrival order
// ============================================================================

/// Writes one sample per instance, then unregisters (or disposes) every
/// instance, and returns what the reader holds. `stage` lets the reader
/// call `wait_for_data` first, as a waiting application does.
fn last_sample_then_lifecycle(unregister: bool, stage: bool) -> Vec<zerodds_dcps::Sample<Reading>> {
    // An unregister without autodispose, so the reader sees
    // NOT_ALIVE_NO_WRITERS rather than NOT_ALIVE_DISPOSED (§2.2.3.21).
    let (_rig, writer, reader) = rig_with(
        HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 1,
        },
        ResourceLimitsQosPolicy::default(),
        true,
        !unregister,
    );
    // Registered explicitly: the lifecycle calls below resolve the instance
    // through the writer's registration.
    for id in 1..=4 {
        writer
            .register_instance(&Reading { id, seq: 1 })
            .expect("register");
    }
    for id in 1..=4 {
        writer.write(&Reading { id, seq: 1 }).expect("write");
    }
    for id in 1..=4 {
        let r = Reading { id, seq: 1 };
        if unregister {
            writer
                .unregister_instance(&r, HANDLE_NIL)
                .expect("unregister");
        } else {
            writer.dispose(&r, HANDLE_NIL).expect("dispose");
        }
    }
    if stage {
        // `wait_for_data` parks the first delivery in the staging inbox;
        // `take` then ingests the inbox ahead of the channel.
        reader
            .wait_for_data(std::time::Duration::from_millis(500))
            .expect("data");
    }
    reader.take_with_info().expect("take_with_info")
}

fn marker_states(
    samples: &[zerodds_dcps::Sample<Reading>],
) -> Vec<zerodds_dcps::InstanceStateKind> {
    samples
        .iter()
        .filter(|s| !s.info.valid_data)
        .map(|s| s.info.instance_state)
        .collect()
}

/// DDS 1.4 §2.2.3.18 KEEP_LAST(1) keeps the newest entry of an instance.
/// An unregister that arrives after an instance's last sample is newer than
/// that sample: it must survive the sample's admission, one
/// NOT_ALIVE_NO_WRITERS per instance (§2.2.2.5.1). Before, the markers of a
/// batch were applied ahead of its data samples, and each sample then evicted
/// its instance's marker.
#[test]
fn an_unregister_after_the_last_sample_survives_keep_last_1() {
    for stage in [false, true] {
        let samples = last_sample_then_lifecycle(true, stage);
        assert_eq!(
            marker_states(&samples),
            vec![zerodds_dcps::InstanceStateKind::NotAliveNoWriters; 4],
            "stage={stage}: {samples:?}"
        );
    }
}

/// The same for a dispose: one NOT_ALIVE_DISPOSED per instance.
#[test]
fn a_dispose_after_the_last_sample_survives_keep_last_1() {
    for stage in [false, true] {
        let samples = last_sample_then_lifecycle(false, stage);
        assert_eq!(
            marker_states(&samples),
            vec![zerodds_dcps::InstanceStateKind::NotAliveDisposed; 4],
            "stage={stage}: {samples:?}"
        );
    }
}

/// A reader on default RESOURCE_LIMITS (§2.2.3.19, all LENGTH_UNLIMITED)
/// under KEEP_ALL + RELIABLE keeps everything not yet taken: 20 instances of
/// 150 samples each. With the former 1000 / 10 / 100 defaults, instances
/// beyond the tenth and samples beyond the hundredth were refused.
#[test]
fn default_resource_limits_keep_everything_under_keep_all() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        DataReaderQos::default().resource_limits,
        true,
    );
    for id in 1..=20 {
        for seq in 1..=150 {
            writer.write(&Reading { id, seq }).expect("write");
        }
    }
    let samples = reader.take_with_info().expect("take_with_info");
    assert_eq!(samples.len(), 20 * 150);
    assert_eq!(reader.sample_rejected_status().total_count, 0);
}

/// The writer's own RESOURCE_LIMITS (§2.2.2.4.2.11): a RELIABLE writer
/// with `max_instances = 1` refuses a sample of a second instance with
/// OUT_OF_RESOURCES at once, as no wait can free an instance, and keeps
/// writing the first.
#[test]
fn a_writer_refuses_an_instance_beyond_its_own_max_instances() {
    let lease = common::lease_domain();
    let p = DomainParticipantFactory::instance()
        .create_participant_with_config(lease.id(), DomainParticipantQos::default(), isolated_cfg())
        .expect("participant");
    let _guard = common::ParticipantGuard::new(p.clone());
    let topic = p
        .create_topic::<Reading>("WriterLimitsReadings", TopicQos::default())
        .expect("topic");
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Reading>(
            &topic,
            DataWriterQos {
                resource_limits: ResourceLimitsQosPolicy {
                    max_instances: 1,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .expect("writer");
    writer
        .write(&Reading { id: 1, seq: 1 })
        .expect("first instance");
    let second = writer.write(&Reading { id: 2, seq: 1 });
    assert!(
        matches!(second, Err(zerodds_dcps::DdsError::OutOfResources { .. })),
        "{second:?}"
    );
    writer
        .write(&Reading { id: 1, seq: 2 })
        .expect("the first instance again");
}

/// §2.2.2.5.3.25 `get_sample_rejected_status` and §2.2.4.1: the read gives
/// `total_count_change` and takes it off, `total_count` stays.
#[test]
fn get_sample_rejected_status_reports_the_change_once() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_samples_per_instance: 2,
            ..Default::default()
        },
        true,
    );
    for seq in 1..=3 {
        writer.write(&Reading { id: 1, seq }).expect("write");
    }
    let _ = reader.take_with_info().expect("take_with_info");
    let first = reader.get_sample_rejected_status().expect("status");
    assert_eq!((first.total_count, first.total_count_change), (1, 1));
    assert_eq!(
        first.last_reason,
        SampleRejectedStatusKind::RejectedBySamplesPerInstanceLimit
    );
    let second = reader.get_sample_rejected_status().expect("status");
    assert_eq!((second.total_count, second.total_count_change), (1, 0));
}

/// §2.2.3.19 and §2.2.2.5.3.8: a sample without data (the NOT_ALIVE marker
/// of a dispose) holds no room in `max_samples_per_instance`: it is
/// delivered at the limit and nothing is rejected.
#[test]
fn a_dispose_marker_does_not_count_against_the_resource_limits() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_samples_per_instance: 1,
            ..Default::default()
        },
        true,
    );
    let sample = Reading { id: 1, seq: 1 };
    writer.write(&sample).expect("write");
    writer.dispose(&sample, HANDLE_NIL).expect("dispose");
    let samples = reader.take_with_info().expect("take_with_info");
    assert_eq!(samples.len(), 2, "the sample and the marker");
    assert!(samples[0].info.valid_data);
    assert!(!samples[1].info.valid_data);
    assert_eq!(reader.sample_rejected_status().total_count, 0);
}

/// §2.2.2.5.3.8: "Samples that contain no data do not count towards the
/// limits imposed by the RESOURCE_LIMITS QoS policy" — also the total of
/// `max_samples`: a dispose marker holds no room, and a later sample of
/// another instance is admitted at the limit of two.
#[test]
fn a_dispose_marker_does_not_count_against_max_samples() {
    let (_rig, writer, reader) = rig(
        keep_all_qos(),
        ResourceLimitsQosPolicy {
            max_samples: 2,
            ..Default::default()
        },
        true,
    );
    let first = Reading { id: 1, seq: 1 };
    writer.write(&first).expect("write");
    writer.dispose(&first, HANDLE_NIL).expect("dispose");
    writer.write(&Reading { id: 2, seq: 1 }).expect("write");
    let samples = reader.take_with_info().expect("take_with_info");
    let data: Vec<u32> = samples
        .iter()
        .filter(|s| s.info.valid_data)
        .map(|s| s.data.id)
        .collect();
    assert_eq!(data, vec![1, 2], "{}", samples.len());
    assert_eq!(
        samples.iter().filter(|s| !s.info.valid_data).count(),
        1,
        "the dispose marker is delivered"
    );
    assert_eq!(reader.sample_rejected_status().total_count, 0);
}
