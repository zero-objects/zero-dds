// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Regression for the durability-service late-joiner duplicate: a
//! TRANSIENT_LOCAL writer's `KEEP_LAST(2)` history, replayed to a late
//! joiner in a **second participant of the same process** (two distinct
//! `DcpsRuntime`s, discovered via the in-process fastpath — see
//! `crates/dcps/src/inproc.rs`), must be exactly the 2 retained samples,
//! once each — not the un-capped history of some other writer, and not
//! duplicated by any second delivery path. The same across two OS
//! *processes* is covered on the Linux host separately; this file only
//! exercises the same-process, two-runtime case.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig};
use zerodds_dcps::{DomainParticipantFactory, DomainParticipantQos};
use zerodds_qos::policies::durability::DurabilityKind;
use zerodds_qos::{LivelinessKind, LivelinessQosPolicy};

#[path = "common/mod.rs"]
mod common;

const TYPE_NAME: &str = "test::LateJoinerKeepLast";

fn writer_cfg(topic: &str) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic.to_string(),
        type_name: TYPE_NAME.to_string(),
        durability: DurabilityKind::TransientLocal,
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn reader_cfg(topic: &str) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic.to_string(),
        type_name: TYPE_NAME.to_string(),
        durability: DurabilityKind::TransientLocal,
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn instance_a() -> [u8; 16] {
    let mut k = [0u8; 16];
    k[0] = 0xAA;
    k
}

/// Collects `Alive` payloads' second byte (the write index) for `timeout`,
/// stopping early only after `expect` have arrived AND a quiet period
/// follows — an early stop at exactly `expect` could hide a duplicate that
/// arrives moments later via a second delivery path (this is what exposed
/// the durability-service bug: without the quiet window the test would have
/// seen only the first `expect` arrivals and missed the duplicates).
fn collect_indices(
    rx: &std::sync::mpsc::Receiver<UserSample>,
    expect: usize,
    timeout: Duration,
) -> Vec<u8> {
    let deadline = Instant::now() + timeout;
    let mut out = Vec::new();
    let mut quiet_since: Option<Instant> = None;
    let quiet_window = Duration::from_millis(500);
    loop {
        if Instant::now() >= deadline {
            break;
        }
        if out.len() >= expect {
            match quiet_since {
                Some(since) if since.elapsed() >= quiet_window => break,
                Some(_) => {}
                None => quiet_since = Some(Instant::now()),
            }
        }
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(UserSample::Alive { payload, .. }) => {
                out.push(payload.as_ref()[1]);
                quiet_since = None;
            }
            Ok(UserSample::Lifecycle { .. }) | Ok(UserSample::CoherentSetEnd { .. }) => {}
            Err(_) => {}
        }
    }
    out
}

/// Three writes to one instance, all before any reader exists. A late
/// joiner in a second participant of the same process must replay exactly
/// the 2 `KEEP_LAST(2)`-retained samples (indices 2 and 3), once each — not
/// index 1 (which the depth must have evicted), and not any index twice.
#[test]
fn late_joiner_in_second_participant_replays_keep_last_2_once() {
    let domain = common::unique_domain();
    let topic = "LateJoinKeepLast";
    let factory = DomainParticipantFactory::instance();

    let writer_p = factory
        .create_participant_with_config(
            domain,
            DomainParticipantQos::default(),
            RuntimeConfig::default(),
        )
        .expect("writer participant");
    let wrt = writer_p.runtime().expect("writer runtime").clone();
    let weid = wrt
        .register_user_writer_kind(writer_cfg(topic), true)
        .expect("register writer");
    wrt.set_user_writer_history_depth(weid, 2)
        .expect("set depth");

    for i in 1..=3u8 {
        wrt.write_user_sample_keyed(weid, &[b'A', i], instance_a(), None)
            .expect("write");
    }
    assert_eq!(
        wrt.user_writer_retained_len(weid),
        2,
        "KEEP_LAST(2) must retain only the 2 newest samples of the one instance written"
    );

    let reader_p = factory
        .create_participant_with_config(
            domain,
            DomainParticipantQos::default(),
            RuntimeConfig::default(),
        )
        .expect("reader participant");
    let rrt = reader_p.runtime().expect("reader runtime").clone();
    let (_reid, rx) = rrt
        .register_user_reader_kind(reader_cfg(topic), true)
        .expect("register reader");

    let got = collect_indices(&rx, 2, Duration::from_secs(10));
    let mut sorted = got.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        vec![2, 3],
        "late joiner should replay exactly the 2 KEEP_LAST(2)-retained samples, once each, got {got:?}"
    );
}

/// Same as above, but the reader matches FIRST (live delivery), and the 3
/// writes stream in afterward — the other direction the task calls out:
/// live samples after the join must not be duplicated either.
#[test]
fn live_samples_after_join_are_not_duplicated() {
    let domain = common::unique_domain();
    let topic = "LateJoinKeepLastLive";
    let factory = DomainParticipantFactory::instance();

    let writer_p = factory
        .create_participant_with_config(
            domain,
            DomainParticipantQos::default(),
            RuntimeConfig::default(),
        )
        .expect("writer participant");
    let wrt = writer_p.runtime().expect("writer runtime").clone();
    let weid = wrt
        .register_user_writer_kind(writer_cfg(topic), true)
        .expect("register writer");
    wrt.set_user_writer_history_depth(weid, 2)
        .expect("set depth");

    let reader_p = factory
        .create_participant_with_config(
            domain,
            DomainParticipantQos::default(),
            RuntimeConfig::default(),
        )
        .expect("reader participant");
    let rrt = reader_p.runtime().expect("reader runtime").clone();
    let (_reid, rx) = rrt
        .register_user_reader_kind(reader_cfg(topic), true)
        .expect("register reader");

    let deadline = Instant::now() + Duration::from_secs(10);
    while wrt.user_writer_matched_count(weid) == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        wrt.user_writer_matched_count(weid) >= 1,
        "reader must match the writer"
    );

    for i in 1..=3u8 {
        wrt.write_user_sample_keyed(weid, &[b'A', i], instance_a(), None)
            .expect("write");
    }

    let got = collect_indices(&rx, 3, Duration::from_secs(10));
    let mut sorted = got.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        vec![1, 2, 3],
        "live delivery after the join must carry every write exactly once, got {got:?}"
    );
}
