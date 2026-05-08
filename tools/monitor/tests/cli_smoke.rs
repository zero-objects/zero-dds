// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::process::Command;
use std::time::{Duration, Instant};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_zerodds-monitor")
}

#[test]
fn help_exits_zero() {
    let out = Command::new(bin()).arg("--help").output().expect("spawn");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("zerodds-monitor"));
    assert!(stdout.contains("SUBCOMMANDS"));
}

#[test]
fn version_exits_zero() {
    let out = Command::new(bin())
        .arg("--version")
        .output()
        .expect("spawn");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("zerodds-monitor"));
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
fn names_exits_zero() {
    let out = Command::new(bin()).arg("names").output().expect("spawn");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("dds_transport_packets_sent_total"));
}

#[test]
fn snapshot_short_duration_terminates() {
    let start = Instant::now();
    let out = Command::new(bin())
        .args(["snapshot", "--duration", "1s"])
        .output()
        .expect("spawn");
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_secs(10), "binary hung > 10s");
    let code = out.status.code().unwrap_or(-1);
    assert!(code == 0 || code == 3, "unexpected exit {code}");
}

#[test]
fn snapshot_prometheus_format_terminates() {
    let start = Instant::now();
    let out = Command::new(bin())
        .args(["snapshot", "--duration", "1s", "--format", "prometheus"])
        .output()
        .expect("spawn");
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_secs(10), "binary hung > 10s");
    let code = out.status.code().unwrap_or(-1);
    assert!(code == 0 || code == 3, "unexpected exit {code}");
}

#[test]
fn serve_short_duration_terminates() {
    let start = Instant::now();
    // Random ephemeral port damit parallel runs nicht kollidieren.
    let port = 19000 + (std::process::id() % 1000);
    let out = Command::new(bin())
        .args([
            "serve",
            "--addr",
            &format!("127.0.0.1:{port}"),
            "--duration",
            "1s",
        ])
        .output()
        .expect("spawn");
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_secs(10), "binary hung > 10s");
    let code = out.status.code().unwrap_or(-1);
    assert!(code == 0 || code == 3, "unexpected exit {code}");
}

#[test]
fn bad_format_rejected() {
    let out = Command::new(bin())
        .args(["snapshot", "--format", "bogus"])
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}
