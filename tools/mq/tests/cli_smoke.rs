// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::process::Command;
use std::time::{Duration, Instant};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_zerodds-mq")
}

#[test]
fn help_exits_zero() {
    let out = Command::new(bin()).arg("--help").output().expect("spawn");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("zerodds-mq"));
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
    let out = Command::new(bin())
        .args(["bridge", "--src-domain", "0", "--dst-domain", "1"])
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn same_domain_rejected() {
    let out = Command::new(bin())
        .args([
            "bridge",
            "--src-domain",
            "5",
            "--dst-domain",
            "5",
            "-t",
            "Foo",
        ])
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("must differ"));
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
fn bad_domain_value_rejected() {
    let out = Command::new(bin())
        .args(["bridge", "--src-domain", "abc", "-t", "Foo"])
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn bridge_short_duration_terminates() {
    let start = Instant::now();
    let out = Command::new(bin())
        .args([
            "bridge",
            "--src-domain",
            "0",
            "--dst-domain",
            "1",
            "-t",
            "MqBridgeTest",
            "--duration",
            "1s",
        ])
        .output()
        .expect("spawn");
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_secs(15), "binary hung > 15s");
    let code = out.status.code().unwrap_or(-1);
    assert!(code == 0 || code == 3, "unexpected exit {code}");
}

#[test]
fn bridge_bidirectional_short_duration_terminates() {
    let start = Instant::now();
    let out = Command::new(bin())
        .args([
            "bridge",
            "--src-domain",
            "0",
            "--dst-domain",
            "1",
            "-t",
            "MqBridgeBiTest",
            "--bidirectional",
            "--duration",
            "1s",
        ])
        .output()
        .expect("spawn");
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_secs(15), "binary hung > 15s");
    let code = out.status.code().unwrap_or(-1);
    assert!(code == 0 || code == 3);
}
