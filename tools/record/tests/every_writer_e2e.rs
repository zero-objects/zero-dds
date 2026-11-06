// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS 1.4 §2.2.3.14 (RxO of RELIABILITY): a RELIABLE writer and a
//! BEST_EFFORT writer on one topic, and `zerodds-record` records both.
//!
//! A RELIABLE reader matches no BEST_EFFORT writer, a BEST_EFFORT reader
//! matches both and takes no repairs; the recorder attaches one of each and
//! keeps a sample of a RELIABLE writer from the RELIABLE reader only. The
//! oracle is the writers' own samples: each of the RELIABLE writer's twenty
//! is in the capture exactly once, each of the BEST_EFFORT writer's at most
//! once and at least one of them.
//!
//! Linux-only, as `live_decode_e2e.rs`: macOS multicast loopback is
//! unreliable for discovery between processes.

#![cfg(target_os = "linux")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserWriterConfig};
use zerodds_recorder::reader::RecordReader;
use zerodds_rtps::wire_types::GuidPrefix;

const TOPIC: &str = "EveryWriter";
const TYPE_NAME: &str = "demo::Bytes";
const SAMPLES: u8 = 20;

fn record_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_zerodds-record"))
}

fn writer_config(reliable: bool) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: TOPIC.into(),
        type_name: TYPE_NAME.into(),
        reliable,
        ..Default::default()
    }
}

#[test]
fn dcps_2_2_3_14_the_recorder_records_a_reliable_and_a_best_effort_writer() {
    let domain = zerodds_test_domains::unique_domain();
    let dir = tempfile::tempdir().unwrap();
    let capture = dir.path().join("every.zddsrec");

    let mut prefix = [0u8; 12];
    prefix[0..4].copy_from_slice(&std::process::id().to_le_bytes());
    prefix[11] = 0xEB;
    let runtime = DcpsRuntime::start(
        domain,
        GuidPrefix::from_bytes(prefix),
        RuntimeConfig::default(),
    )
    .expect("writer runtime");
    let reliable = runtime.register_user_writer(writer_config(true)).unwrap();
    let best_effort = runtime.register_user_writer(writer_config(false)).unwrap();

    let mut recorder = Command::new(record_bin())
        .args([
            "record",
            "-t",
            TOPIC,
            "-d",
            &domain.to_string(),
            "--duration",
            "10s",
            "-o",
            capture.to_str().unwrap(),
        ])
        .spawn()
        .expect("spawn record");

    // The recorder's two readers: both match the RELIABLE writer, the
    // BEST_EFFORT one matches the BEST_EFFORT writer.
    let deadline = Instant::now() + Duration::from_secs(10);
    while runtime.user_writer_matched_count(reliable) < 2
        || runtime.user_writer_matched_count(best_effort) < 1
    {
        assert!(
            Instant::now() < deadline,
            "the recorder's readers did not match (reliable {}, best effort {})",
            runtime.user_writer_matched_count(reliable),
            runtime.user_writer_matched_count(best_effort)
        );
        runtime.wait_match_event(Duration::from_millis(200));
    }
    for i in 0..SAMPLES {
        runtime.write_user_sample(reliable, vec![b'R', i]).unwrap();
        runtime
            .write_user_sample(best_effort, vec![b'B', i])
            .unwrap();
    }
    assert!(
        runtime.await_user_writer_acknowledged(reliable, Instant::now() + Duration::from_secs(10)),
        "the recorder did not acknowledge the RELIABLE writer's samples"
    );

    let status = recorder.wait().expect("record ran");
    assert!(status.success(), "record: {status:?}");
    runtime.shutdown();

    let bytes = std::fs::read(&capture).expect("the capture");
    let mut reader = RecordReader::new(&bytes);
    reader.parse_header().expect("the header");
    let mut seen: BTreeMap<Vec<u8>, usize> = BTreeMap::new();
    while let Some(frame) = reader.next_frame().expect("a frame") {
        if !frame.payload.is_empty() {
            *seen.entry(frame.payload).or_default() += 1;
        }
    }
    for i in 0..SAMPLES {
        assert_eq!(
            seen.get(&vec![b'R', i]),
            Some(&1),
            "the RELIABLE writer's sample {i} (all: {seen:?})"
        );
        assert!(
            seen.get(&vec![b'B', i]).is_none_or(|n| *n == 1),
            "the BEST_EFFORT writer's sample {i} twice (all: {seen:?})"
        );
    }
    assert!(
        (0..SAMPLES).any(|i| seen.contains_key(&vec![b'B', i])),
        "no sample of the BEST_EFFORT writer (all: {seen:?})"
    );
}
