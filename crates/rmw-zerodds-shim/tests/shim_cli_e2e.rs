// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// E2E-Tests fuer `zerodds-ros2-shim` CLI.
//
// Spec: `docs/specs/zerodds-ros2-bridge-1.0.md` §12.2.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::path::PathBuf;
use std::process::Command;

fn shim_binary() -> PathBuf {
    let exe = env!("CARGO_BIN_EXE_zerodds-ros2-shim");
    PathBuf::from(exe)
}

#[test]
fn selftest_succeeds() {
    let out = Command::new(shim_binary())
        .arg("selftest")
        .output()
        .expect("spawn shim selftest");
    assert!(out.status.success(), "selftest exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("selftest OK"));
    assert!(stdout.contains("rt/chatter"));
}

#[test]
fn topic_mangle_emits_rt_prefix() {
    let out = Command::new(shim_binary())
        .args(["topic", "/cmd_vel"])
        .output()
        .expect("spawn shim topic");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("rt/cmd_vel"), "stdout: {stdout}");
}

#[test]
fn qos_sensor_data_is_best_effort() {
    let out = Command::new(shim_binary())
        .args(["qos", "sensor_data"])
        .output()
        .expect("spawn shim qos");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("BEST_EFFORT"));
    assert!(stdout.contains("KEEP_LAST(5)"));
}

#[test]
fn qos_unknown_profile_exits_nonzero() {
    let out = Command::new(shim_binary())
        .args(["qos", "no_such_profile"])
        .output()
        .expect("spawn shim qos bad");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("unknown profile"));
}

#[test]
fn version_emits_one_line() {
    let out = Command::new(shim_binary())
        .arg("--version")
        .output()
        .expect("spawn shim version");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("zerodds-ros2-shim"));
    assert!(stdout.contains("1.0"));
}

#[test]
fn validate_with_minimal_yaml_succeeds() {
    let tmp = std::env::temp_dir().join("zerodds-ros2-shim-test.yaml");
    let yaml = "ros2:\n  namespace: \"/foo\"\n  ros_domain_id: 3\n";
    std::fs::write(&tmp, yaml).expect("write tmp yaml");
    let out = Command::new(shim_binary())
        .args(["validate", tmp.to_str().expect("utf8 tmp path")])
        .output()
        .expect("spawn shim validate");
    let _ = std::fs::remove_file(&tmp);
    assert!(out.status.success(), "validate failed: {out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("config OK"));
}

#[test]
fn info_includes_rmw_compat_marker() {
    let out = Command::new(shim_binary())
        .arg("info")
        .output()
        .expect("spawn shim info");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("rmw_zerodds_cpp"));
    assert!(stdout.contains("REP-2007"));
}
