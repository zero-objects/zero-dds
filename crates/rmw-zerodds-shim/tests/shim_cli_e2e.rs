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

/// Each child gets an explicit file, never process-global env or /etc mutation.
struct RuntimeFixture(PathBuf);

impl RuntimeFixture {
    fn new(yaml: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        loop {
            let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir()
                .join(format!("zerodds-shim-runtime-{}-{id}", std::process::id()));
            match std::fs::create_dir(&dir) {
                Ok(()) => {
                    let fixture = Self(dir);
                    std::fs::write(fixture.path(), yaml).expect("write runtime config");
                    return fixture;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create runtime fixture: {error}"),
            }
        }
    }

    fn path(&self) -> PathBuf {
        self.0.join("runtime.yaml")
    }
}

impl Drop for RuntimeFixture {
    fn drop(&mut self) {
        std::fs::remove_file(self.path()).expect("remove fixture file");
        std::fs::remove_dir(&self.0).expect("remove fixture directory");
    }
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
    let yaml = "ros2:\n  topic_mangling: none\n";
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
    let fixture = RuntimeFixture::new("discovery:\n  no_multicast: true\n");
    let out = Command::new(shim_binary())
        .arg("doctor")
        .arg("--config")
        .arg(fixture.path())
        .env_clear()
        .env("RMW_IMPLEMENTATION", "rmw_zerodds_cpp")
        .output()
        .expect("spawn doctor");
    assert_eq!(out.status.code(), Some(5), "expected hard-fail exit 5");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("[fail] discovery"));
}

#[test]
fn doctor_multicast_free_with_peers_ok() {
    let fixture = RuntimeFixture::new(
        "discovery:\n  no_multicast: true\n  peers:\n    - 10.0.0.2\n    - 10.0.0.3\n",
    );
    let out = Command::new(shim_binary())
        .arg("doctor")
        .arg("--config")
        .arg(fixture.path())
        .env_clear()
        .env("RMW_IMPLEMENTATION", "rmw_zerodds_cpp")
        .env("ROS_DISTRO", "humble")
        .output()
        .expect("spawn doctor");
    assert!(out.status.success(), "doctor exit: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("multicast-free, 2 configured unicast peer(s)"));
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
    let fixture = RuntimeFixture::new("discovery:\n  no_multicast: true\n");
    let out = Command::new(shim_binary())
        .arg("graph")
        .arg("--config")
        .arg(fixture.path())
        .arg("--domain")
        .arg("7")
        .env_clear()
        .output()
        .expect("spawn graph");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("rmw_zerodds_cpp"));
    assert!(stdout.contains("domain_id             7"));
    assert!(stdout.contains("discovery             unicast"));
}

#[test]
fn diagnostics_refuse_invalid_explicit_runtime_files() {
    let fixture = RuntimeFixture::new("discovery:\n  peers:\n    - not-an-address\n");
    for path in [fixture.path(), fixture.0.join("missing.yaml")] {
        for command in ["doctor", "graph"] {
            let out = Command::new(shim_binary())
                .arg(command)
                .arg("--config")
                .arg(&path)
                .env_clear()
                .env("RMW_IMPLEMENTATION", "rmw_zerodds_cpp")
                .output()
                .expect("spawn diagnostics");
            assert!(!out.status.success(), "{command} accepted {path:?}");
            let diagnostic = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            assert!(diagnostic.contains("runtime configuration"), "{diagnostic}");
        }
    }
}

/// The diagnostics read the `ros2` section the RMW reads: mangling and the
/// override of a topic come out of the same file.
#[test]
fn topic_shows_the_configured_mangling_and_override() {
    let fixture = RuntimeFixture::new(
        "ros2:\n  topic_mangling: none\n  topic_overrides:\n    - ros_topic: /map\n      qos:\n        durability: transient_local\n        depth: 1\n",
    );
    let out = Command::new(shim_binary())
        .args(["topic", "/map", "--config"])
        .arg(fixture.path())
        .env_clear()
        .output()
        .expect("spawn topic");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{out:?}");
    assert!(stdout.contains("dds_topic               map\n"), "{stdout}");
    assert!(
        stdout.contains("durability              TRANSIENT_LOCAL"),
        "{stdout}"
    );
    assert!(
        stdout.contains("reliability             (as requested)"),
        "{stdout}"
    );
    assert!(
        stdout.contains("history                 (as requested) depth 1"),
        "{stdout}"
    );
}

/// A setting rcl owns is refused with where it belongs, by --check-config
/// and by validate.
#[test]
fn check_config_refuses_what_rcl_owns() {
    let fixture = RuntimeFixture::new("ros2:\n  namespace: /robot\n");
    let out = Command::new(shim_binary())
        .arg("--check-config")
        .arg("--config")
        .arg(fixture.path())
        .env_clear()
        .output()
        .expect("spawn check-config");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!out.status.success(), "{stdout}");
    assert!(stdout.contains("ros2.namespace"), "{stdout}");
    assert!(stdout.contains("__ns"), "{stdout}");
    assert!(stdout.contains("configuration invalid"), "{stdout}");
    let out = Command::new(shim_binary())
        .arg("validate")
        .arg(fixture.path())
        .env_clear()
        .output()
        .expect("spawn validate");
    assert!(!out.status.success());
}

#[test]
fn flags_of_the_old_shim_file_are_refused_with_their_replacement() {
    for (flag, needle) in [
        ("--runtime-config", "--config"),
        ("--log-level", "rcutils"),
        ("--enclave", "security.enclave_dir"),
    ] {
        let out = Command::new(shim_binary())
            .args(["info", flag, "x"])
            .env_clear()
            .output()
            .expect("spawn info");
        assert!(!out.status.success(), "{flag} accepted");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains(needle), "{flag}: {stderr}");
    }
}
