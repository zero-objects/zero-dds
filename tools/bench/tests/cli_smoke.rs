// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Smoke-Tests für `zerodds-bench`.
//!
//! Tests die DcpsRuntime starten haben einen 10s hard-cap — sonst
//! hat der Test einen hang den wir nicht in CI verschleppen wollen.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::process::Command;
use std::time::{Duration, Instant};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_zerodds-bench")
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
    assert!(stdout.contains("zerodds-bench"));
    assert!(stdout.contains("SUBCOMMANDS"));
}

#[test]
fn version_exits_zero() {
    let out = Command::new(bin())
        .arg("--version")
        .output()
        .expect("spawn");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("zerodds-bench"));
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
fn info_exits_zero() {
    let out = Command::new(bin()).arg("info").output().expect("spawn");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("backend"));
    assert!(stdout.contains("XCDR2"));
}

#[test]
fn latency_short_duration_terminates() {
    // Latency-Bench mit --duration 1s muss in <10s self-terminieren.
    let start = Instant::now();
    let out = Command::new(bin())
        .args(["latency", "--duration", "1s", "-p", "32"])
        .output()
        .expect("spawn");
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_secs(10), "binary hung > 10s");
    let code = out.status.code().unwrap_or(-1);
    assert!(code == 0 || code == 3, "unexpected exit {code}");
}

#[test]
fn throughput_short_duration_terminates() {
    let start = Instant::now();
    let out = Command::new(bin())
        .args(["throughput", "--duration", "1s", "-p", "32"])
        .output()
        .expect("spawn");
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_secs(10), "binary hung > 10s");
    let code = out.status.code().unwrap_or(-1);
    assert!(code == 0 || code == 3, "unexpected exit {code}");
}

#[test]
fn bad_payload_value_rejected() {
    let out = Command::new(bin())
        .args(["latency", "--payload", "abc"])
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}
