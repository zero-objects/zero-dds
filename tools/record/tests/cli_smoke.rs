// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Smoke tests for the CLI frontend of the `zerodds-record` bin.
//!
//! We build the bin via `env!("CARGO_BIN_EXE_zerodds-record")` (cargo
//! provides the path to the built bin as an env var during `cargo test`).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_zerodds-record")
}

#[test]
fn help_exits_zero() {
    let out = Command::new(bin()).arg("--help").output().expect("spawn");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("zerodds-record"));
    assert!(stdout.contains("SUBCOMMANDS"));
}

#[test]
fn version_exits_zero() {
    let out = Command::new(bin())
        .arg("--version")
        .output()
        .expect("spawn");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("zerodds-record"));
}

#[test]
fn no_args_exits_two() {
    let out = Command::new(bin()).output().expect("spawn");
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("error"));
}

#[test]
fn unknown_subcommand_exits_two() {
    let out = Command::new(bin())
        .arg("frobnicate")
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn record_rejects_missing_topic() {
    // Without --topic, exit 2 must occur (parse validation), before
    // the DCPS runtime is even started. Important: this
    // test must NEVER start a DcpsRuntime — otherwise the
    // binary runs endlessly and blocks the test suite.
    let out = Command::new(bin())
        .args(["record", "--domain", "5"])
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("at least one --topic"));
}

#[test]
fn record_short_duration_terminates() {
    // With --duration 1s the binary must self-terminate after ~1s.
    // We do NOT test-double on network multicast — if DcpsRuntime
    // fails, exit 3 comes out (which is also ok; important: no hang).
    use std::time::{Duration, Instant};
    let start = Instant::now();
    let out = Command::new(bin())
        .args([
            "record",
            "-t",
            "TestTopic",
            "--duration",
            "1s",
            "-o",
            "/tmp/zds-record-test.zddsrec",
        ])
        .output()
        .expect("spawn");
    let elapsed = start.elapsed();
    // Hard cap: nothing here may be > 10s — otherwise the test has
    // a hang that we did not want to reproduce locally.
    assert!(elapsed < Duration::from_secs(10), "binary hung > 10s");
    // Exit code may be 0 (success) OR 3 (DDS-init fail in
    // network-restricted CI) — neither is a test situation.
    let code = out.status.code().unwrap_or(-1);
    assert!(code == 0 || code == 3, "unexpected exit {code}");
    let _ = std::fs::remove_file("/tmp/zds-record-test.zddsrec");
}

#[test]
fn info_missing_file_exits_three() {
    let out = Command::new(bin())
        .args(["info", "/nonexistent/path.zddsrec"])
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(3));
}

#[test]
fn info_reads_synthetic_zddsrec() {
    use zerodds_recorder::format::{Header, ParticipantEntry, TopicEntry};

    // Write a tiny, valid .zddsrec.
    let mut buf = Vec::new();
    let h = Header {
        time_base_unix_ns: 0,
        participants: vec![ParticipantEntry {
            guid: [9u8; 16],
            name: "test-participant".into(),
        }],
        topics: vec![TopicEntry {
            name: "T1".into(),
            type_name: "TT".into(),
        }],
    };
    h.write(&mut buf);

    let path = std::env::temp_dir().join(format!(
        "zds-record-cli-test-{}.zddsrec",
        std::process::id()
    ));
    std::fs::write(&path, &buf).unwrap();

    let out = Command::new(bin())
        .args(["info"])
        .arg(&path)
        .output()
        .expect("spawn");
    let _ = std::fs::remove_file(&path);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("zddsrec header"));
    assert!(stdout.contains("- T1"));
    assert!(stdout.contains("participants:        1"));
}
