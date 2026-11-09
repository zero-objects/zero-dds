// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS 1.4 §2.2.3.4 (DURABILITY): `zerodds-record` records the history a
//! TRANSIENT_LOCAL writer keeps for late joiners.
//!
//! Only a reader that requests TRANSIENT_LOCAL is sent the samples written
//! before it matched; a VOLATILE reader takes what is written after. The
//! recorder attaches a RELIABLE TRANSIENT_LOCAL reader besides its two
//! VOLATILE ones (`zerodds_cli_common::monitor_reader_configs`). The oracle
//! is the writer's own samples: the five written before the recorder
//! started and the five written after are each in the capture exactly once.
//!
//! Linux-only, as `every_writer_e2e.rs`: macOS multicast loopback is
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

const TOPIC: &str = "KeptHistory";
const TYPE_NAME: &str = "demo::Bytes";
const SAMPLES: u8 = 5;

fn record_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_zerodds-record"))
}

#[test]
fn dcps_2_2_3_4_the_recorder_records_the_history_of_a_transient_local_writer() {
    let domain = zerodds_test_domains::unique_domain();
    let dir = tempfile::tempdir().unwrap();
    let capture = dir.path().join("history.zddsrec");

    let mut prefix = [0u8; 12];
    prefix[0..4].copy_from_slice(&std::process::id().to_le_bytes());
    prefix[11] = 0x7C;
    let runtime = DcpsRuntime::start(
        domain,
        GuidPrefix::from_bytes(prefix),
        RuntimeConfig::default(),
    )
    .expect("writer runtime");
    let writer = runtime
        .register_user_writer(UserWriterConfig {
            topic_name: TOPIC.into(),
            type_name: TYPE_NAME.into(),
            reliable: true,
            durability: zerodds_qos::DurabilityKind::TransientLocal,
            ..Default::default()
        })
        .unwrap();
    // KEEP_LAST deep enough for all ten: the samples written after the
    // match can be out before the history reaches the recorder, and with a
    // depth of five they would push the first ones out of the writer's
    // cache (DDS 1.4 §2.2.3.18), which the writer then sends as a GAP.
    runtime
        .set_user_writer_history_depth(writer, 2 * usize::from(SAMPLES))
        .expect("the writer keeps its history");
    for i in 0..SAMPLES {
        runtime.write_user_sample(writer, vec![b'H', i]).unwrap();
    }

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

    // All three of the recorder's readers match: the writer is RELIABLE and
    // offers TRANSIENT_LOCAL.
    let deadline = Instant::now() + Duration::from_secs(10);
    while runtime.user_writer_matched_count(writer) < 3 {
        assert!(
            Instant::now() < deadline,
            "the recorder's readers did not match ({})",
            runtime.user_writer_matched_count(writer)
        );
        runtime.wait_match_event(Duration::from_millis(200));
    }
    for i in 0..SAMPLES {
        runtime.write_user_sample(writer, vec![b'L', i]).unwrap();
    }
    assert!(
        runtime.await_user_writer_acknowledged(writer, Instant::now() + Duration::from_secs(10)),
        "the recorder did not acknowledge the writer's samples"
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
            seen.get(&vec![b'H', i]),
            Some(&1),
            "sample {i} of the history (all: {seen:?})"
        );
        assert_eq!(
            seen.get(&vec![b'L', i]),
            Some(&1),
            "sample {i} written after the match (all: {seen:?})"
        );
    }
}
