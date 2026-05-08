// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::process::Command;
use std::time::{Duration, Instant};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_zerodds-spy")
}

#[test]
fn help_exits_zero() {
    let out = Command::new(bin()).arg("--help").output().expect("spawn");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("zerodds-spy"));
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
fn missing_topic_exits_two() {
    // subscribe ohne --topic muss exit 2 vor jeder DCPS-Init geben.
    let out = Command::new(bin())
        .args(["subscribe", "-d", "5"])
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn unknown_subcommand_exits_two() {
    let out = Command::new(bin()).arg("publish").output().expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn bad_count_value_rejected() {
    let out = Command::new(bin())
        .args(["-t", "Foo", "-n", "abc"])
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn subscribe_short_duration_terminates() {
    let start = Instant::now();
    let out = Command::new(bin())
        .args(["-t", "ZerodDsSpyTest", "--duration", "1s"])
        .output()
        .expect("spawn");
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_secs(10), "binary hung > 10s");
    let code = out.status.code().unwrap_or(-1);
    assert!(code == 0 || code == 3, "unexpected exit {code}");
}

#[test]
fn subscribe_count_zero_terminates_immediately() {
    // -n 0 → max_samples=0 → break sofort, kein Sample warten.
    let start = Instant::now();
    let out = Command::new(bin())
        .args(["-t", "Foo", "-n", "0", "--duration", "5s"])
        .output()
        .expect("spawn");
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(5),
        "binary did not honor -n 0"
    );
    let code = out.status.code().unwrap_or(-1);
    assert!(code == 0 || code == 3);
}
