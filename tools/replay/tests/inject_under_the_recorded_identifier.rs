// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `zerodds-replay replay --inject` sends each sample under the
//! representation identifier it was recorded with (DDSI-RTPS 2.5 §10.5,
//! DDS-XTypes 1.3 §7.6.3.1.2 Table 60).
//!
//! A `.zddsrec` of version 3 holds three samples of one topic under
//! `PL_CDR_LE` (`00 03`), `D_CDR2_BE` (`00 08`) and `CDR2_LE` (`00 07`);
//! the replay injects it into a leased domain, and a reader there reads off
//! each sample the identifier of its encapsulation header. Before, every
//! sample went out under the replay writer's own `CDR2_LE`. A capture of
//! version 2, which records no identifier, is refused unless
//! `--representation` names one.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::process::Command;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample};
use zerodds_recorder::{Frame, Header, ParticipantEntry, RecordWriter, SampleKind, TopicEntry};
use zerodds_rtps::wire_types::GuidPrefix;

const TOPIC: &str = "ReplayIdentifierTopic";
const TYPE: &str = "ReplayIdentifierType";
const IDENTIFIERS: [[u8; 2]; 3] = [[0x00, 0x03], [0x00, 0x08], [0x00, 0x07]];

fn capture(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("three.zddsrec");
    let mut w = RecordWriter::new(std::fs::File::create(&path).unwrap());
    w.write_header(&Header {
        time_base_unix_ns: 0,
        participants: vec![ParticipantEntry {
            guid: [1; 16],
            name: "test".into(),
        }],
        topics: vec![TopicEntry {
            name: TOPIC.into(),
            type_name: TYPE.into(),
        }],
    })
    .unwrap();
    for (i, id) in IDENTIFIERS.iter().enumerate() {
        w.write_frame(&Frame {
            timestamp_delta_ns: i as i64 * 1_000_000,
            participant_idx: 0,
            topic_idx: 0,
            sample_kind: SampleKind::Alive,
            representation_identifier: Some(*id),
            payload: vec![i as u8, 0, 0, 0],
        })
        .unwrap();
    }
    path
}

#[test]
fn rtps_10_5_the_injected_samples_keep_their_representation_identifiers() {
    let lease = zerodds_test_domains::lease_domain();
    let dir = std::env::temp_dir().join(format!("zerodds-replay-id-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = capture(&dir);

    let rt = DcpsRuntime::start(
        lease.id(),
        GuidPrefix::from_bytes([0x5e; 12]),
        RuntimeConfig::default(),
    )
    .unwrap();
    let (_reader, rx) = rt
        .register_user_reader(UserReaderConfig {
            topic_name: TOPIC.into(),
            type_name: TYPE.into(),
            reliable: true,
            data_representation_offer: Some(vec![0, 2]),
            ..Default::default()
        })
        .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_zerodds-replay"))
        .args(["replay", path.to_str().unwrap(), "--inject-domain"])
        .arg(lease.id().to_string())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "replay failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let mut got = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while got.len() < IDENTIFIERS.len() {
        let left = deadline.saturating_duration_since(Instant::now());
        assert!(!left.is_zero(), "the reader got {got:?}");
        if let Ok(UserSample::Alive {
            representation_identifier,
            payload,
            ..
        }) = rx.recv_timeout(left)
        {
            got.push((payload.as_ref()[0], representation_identifier));
        }
    }
    got.sort_unstable();
    assert_eq!(
        got,
        [
            (0, IDENTIFIERS[0]),
            (1, IDENTIFIERS[1]),
            (2, IDENTIFIERS[2])
        ]
    );
    rt.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_capture_without_identifiers_is_not_injected_unless_one_is_named() {
    let dir = std::env::temp_dir().join(format!("zerodds-replay-v2-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // Version 2 (docs/specs/zddsrec-1.0.md before version 3): no ReprPresent
    // and ReprId in the frame.
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"ZDDS");
    bytes.extend_from_slice(&2u32.to_le_bytes());
    bytes.extend_from_slice(&0i64.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&[1u8; 16]);
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.push(b'p');
    bytes.extend_from_slice(&(TYPE.len() as u32).to_le_bytes());
    bytes.extend_from_slice(TYPE.as_bytes());
    bytes.extend_from_slice(&(TOPIC.len() as u32).to_le_bytes());
    bytes.extend_from_slice(TOPIC.as_bytes());
    bytes.push(b'F');
    bytes.extend_from_slice(&0i64.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&4u32.to_le_bytes());
    bytes.extend_from_slice(&[9, 0, 0, 0]);
    let path = dir.join("v2.zddsrec");
    std::fs::write(&path, bytes).unwrap();

    let lease = zerodds_test_domains::lease_domain();
    let out = Command::new(env!("CARGO_BIN_EXE_zerodds-replay"))
        .args(["replay", path.to_str().unwrap(), "--inject-domain"])
        .arg(lease.id().to_string())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("--representation"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
}
