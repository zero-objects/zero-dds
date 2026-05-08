// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::process::Command;
use std::time::{Duration, Instant};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_zerodds-snitch")
}

#[test]
fn help_exits_zero() {
    let out = Command::new(bin()).arg("--help").output().expect("spawn");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("zerodds-snitch"));
}

#[test]
fn version_exits_zero() {
    let out = Command::new(bin())
        .arg("--version")
        .output()
        .expect("spawn");
    assert!(out.status.success());
}

#[test]
fn no_args_exits_two() {
    let out = Command::new(bin()).output().expect("spawn");
    assert_eq!(out.status.code(), Some(2));
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
fn bad_format_rejected() {
    let out = Command::new(bin())
        .args(["probe", "--format", "xml"])
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn probe_short_duration_terminates_text() {
    let start = Instant::now();
    let out = Command::new(bin())
        .args(["probe", "--duration", "1s"])
        .output()
        .expect("spawn");
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_secs(10), "binary hung > 10s");
    let code = out.status.code().unwrap_or(-1);
    assert!(code == 0 || code == 3);
    if code == 0 {
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("Discovery snapshot"));
    }
}

#[test]
fn probe_short_duration_terminates_json() {
    let start = Instant::now();
    let out = Command::new(bin())
        .args(["probe", "--duration", "1s", "--format", "json"])
        .output()
        .expect("spawn");
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_secs(10));
    let code = out.status.code().unwrap_or(-1);
    assert!(code == 0 || code == 3);
    if code == 0 {
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("\"participants\":"));
        assert!(stdout.contains("\"entries\":"));
    }
}

#[test]
fn probe_implicit_subcommand() {
    // -d 5 ohne explizites "probe" → muss als probe geparst werden.
    let start = Instant::now();
    let out = Command::new(bin())
        .args(["-d", "0", "--duration", "1s"])
        .output()
        .expect("spawn");
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_secs(10));
    let code = out.status.code().unwrap_or(-1);
    assert!(code == 0 || code == 3);
}
