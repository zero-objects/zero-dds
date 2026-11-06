// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The command line of `zerodds-record` against its manual page
//! (`man/man1/zerodds-record.1`): the exit statuses, `--duration`, and
//! `info` on a capture.
//!
//! The binary is the one cargo builds for the test
//! (`CARGO_BIN_EXE_zerodds-record`).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_zerodds-record")
}

fn status(args: &[&str]) -> (Option<i32>, String, String) {
    let out = Command::new(bin()).args(args).output().expect("spawn");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// EXIT STATUS: 0 success (`--help`, `--version`), 2 a parse error (no
/// subcommand, an unknown one, `record` without `--topic`, checked before
/// any DDS state exists), 3 an I/O error (`info` on a missing file).
#[test]
fn the_exit_status_is_the_manuals() {
    let (code, stdout, _) = status(&["--help"]);
    assert_eq!(code, Some(0));
    assert!(stdout.contains("zerodds-record") && stdout.contains("SUBCOMMANDS"));
    let (code, stdout, _) = status(&["--version"]);
    assert_eq!(code, Some(0));
    assert!(stdout.contains("zerodds-record"));
    let (code, _, stderr) = status(&[]);
    assert_eq!(code, Some(2));
    assert!(stderr.contains("error"));
    assert_eq!(status(&["frobnicate"]).0, Some(2));
    let (code, _, stderr) = status(&["record", "--domain", "5"]);
    assert_eq!(code, Some(2));
    assert!(stderr.contains("at least one --topic"));
    assert_eq!(status(&["info", "/nonexistent/path.zddsrec"]).0, Some(3));
}

/// `--duration` ends a run by itself, with exit status 0, and the capture
/// it leaves is a `.zddsrec` that `info` reads. The domain is leased.
#[test]
fn a_run_with_duration_ends_by_itself_and_leaves_a_capture() {
    use std::time::{Duration, Instant};
    let domain = zerodds_test_domains::unique_domain().to_string();
    let path = std::env::temp_dir().join(format!(
        "zds-record-duration-{}.zddsrec",
        std::process::id()
    ));
    let start = Instant::now();
    let out = Command::new(bin())
        .args([
            "record",
            "-t",
            "TestTopic",
            "-d",
            &domain,
            "--duration",
            "1s",
            "-o",
        ])
        .arg(&path)
        .output()
        .expect("spawn");
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "the run did not end"
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let (code, stdout, _) = status(&["info", path.to_str().unwrap()]);
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, Some(0));
    assert!(stdout.contains("zddsrec header"), "{stdout}");
}

/// `info` prints the header of a capture: its topics and participants.
#[test]
fn info_prints_the_header_of_a_capture() {
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
