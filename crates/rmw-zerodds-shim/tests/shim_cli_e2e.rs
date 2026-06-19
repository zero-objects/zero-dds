// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// E2E tests for the `zerodds-ros2-shim` CLI.
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

// ---------------------------------------------------------------------------
// C8 — `doctor` / `graph` diagnostic CLI
// ---------------------------------------------------------------------------

#[test]
fn doctor_clean_ros_env_passes() {
    let out = Command::new(shim_binary())
        .arg("doctor")
        .env_clear()
        .env("RMW_IMPLEMENTATION", "rmw_zerodds_cpp")
        .env("ROS_DISTRO", "jazzy")
        .env("ROS_DOMAIN_ID", "0")
        .output()
        .expect("spawn doctor");
    assert!(out.status.success(), "doctor exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("[  ok] RMW_IMPLEMENTATION"));
    assert!(stdout.contains("0 failure(s)"));
}

#[test]
fn doctor_multicast_free_without_peers_fails() {
    // Multicast off + no unicast peers = unreachable discovery → hard fail
    // (exit 5). The classic ROS-on-WiFi misconfiguration `doctor` exists for.
    let out = Command::new(shim_binary())
        .arg("doctor")
        .env_clear()
        .env("RMW_IMPLEMENTATION", "rmw_zerodds_cpp")
        .env("ZERODDS_NO_MULTICAST", "1")
        .output()
        .expect("spawn doctor");
    assert_eq!(out.status.code(), Some(5), "expected hard-fail exit 5");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("[fail] discovery"));
}

#[test]
fn doctor_multicast_free_with_peers_ok() {
    let out = Command::new(shim_binary())
        .arg("doctor")
        .env_clear()
        .env("RMW_IMPLEMENTATION", "rmw_zerodds_cpp")
        .env("ROS_DISTRO", "humble")
        .env("ZERODDS_NO_MULTICAST", "1")
        .env("ZERODDS_PEERS", "10.0.0.2,10.0.0.3")
        .output()
        .expect("spawn doctor");
    assert!(out.status.success(), "doctor exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("multicast-free, 2 unicast peer(s)"));
}

#[test]
fn doctor_bad_security_dir_fails() {
    let out = Command::new(shim_binary())
        .arg("doctor")
        .env_clear()
        .env("RMW_IMPLEMENTATION", "rmw_zerodds_cpp")
        .env("ZERODDS_SECURITY_DIR", "/tmp/_zerodds_not_an_enclave_xyz")
        .output()
        .expect("spawn doctor");
    assert_eq!(out.status.code(), Some(5));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("[fail] security"));
}

#[test]
fn graph_shows_participant_and_discovery_mode() {
    let out = Command::new(shim_binary())
        .arg("graph")
        .arg("--domain")
        .arg("7")
        .env_clear()
        .env("ZERODDS_NO_MULTICAST", "1")
        .output()
        .expect("spawn graph");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("rmw_zerodds_cpp"));
    assert!(stdout.contains("domain_id             7"));
    assert!(stdout.contains("discovery             unicast"));
}
