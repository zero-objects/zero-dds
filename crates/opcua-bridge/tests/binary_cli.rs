// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `zerodds-opcua-bridged` as a process (spec §2, §9.1, §9.2): its flags,
//! its exit codes and its end on SIGTERM and SIGINT.

#![cfg(all(unix, feature = "daemon"))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

const IDL: &str = "@final struct Telemetry { double value; };\n";

const GATEWAY_XML: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<zerodds_opcua_gateway>
  <bridge name=\"cli\">
    <domain_id>0</domain_id>
    <dds_to_ua_connection>
      <dds_topic>Telemetry</dds_topic>
      <dds_type>Telemetry</dds_type>
    </dds_to_ua_connection>
  </bridge>
</zerodds_opcua_gateway>
";

fn daemon() -> Command {
    Command::new(env!("CARGO_BIN_EXE_zerodds-opcua-bridged"))
}

fn fixture(name: &str, content: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("zerodds-opcua-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("fixture dir");
    let path = dir.join(name);
    std::fs::write(&path, content).expect("write fixture");
    path
}

fn code(command: &mut Command) -> (Option<i32>, String, String) {
    let output = command.output().expect("run the daemon");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Starts the daemon with `extra` beside the IDL, the gateway file and a
/// leased domain, and reads its stderr up to the line that names the
/// metrics endpoint. Returns the process, the lines read and the lease.
fn started(extra: &[&str], tag: &str) -> (Child, Vec<String>, zerodds_test_domains::DomainLease) {
    let idl = fixture(&format!("{tag}.idl"), IDL);
    let gateway = fixture(&format!("{tag}.xml"), GATEWAY_XML);
    let lease = zerodds_test_domains::lease_domain();
    let mut child = daemon()
        .args(["--domain", &lease.id().to_string()])
        .args(["--idl", idl.to_str().unwrap()])
        .args(["--gateway-config", gateway.to_str().unwrap()])
        .args(["--metrics", "127.0.0.1:0"])
        .args(extra)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the daemon");
    let stderr = child.stderr.take().expect("stderr");
    let mut lines = Vec::new();
    for line in BufReader::new(stderr).lines() {
        let line = line.expect("stderr line");
        let done = line.contains("metrics on ");
        lines.push(line);
        if done {
            break;
        }
    }
    (child, lines, lease)
}

fn signal(child: &Child, name: &str) {
    let status = Command::new("kill")
        .args([name, &child.id().to_string()])
        .status()
        .expect("kill");
    assert!(status.success());
}

/// §2: `--version` prints the name and the version and exits 0.
#[test]
fn opcuabridge_2_version_prints_the_name_and_exits_zero() {
    let (status, stdout, _) = code(daemon().arg("--version"));
    assert_eq!(status, Some(0));
    assert!(stdout.starts_with("zerodds-opcua-bridged "), "{stdout}");
}

/// §2: `--check-config` exits 0 for a configuration that loads, 1 for a
/// value the daemon does not take and 2 for a file that cannot be read.
#[test]
fn opcuabridge_2_check_config_answers_with_its_exit_codes() {
    let good = fixture("good.yaml", "domain: 0\nlog_level: warn\n");
    let (status, _, stderr) = code(daemon().arg("--config").arg(&good).arg("--check-config"));
    assert_eq!(status, Some(0), "{stderr}");

    let bad_value = fixture("bad-value.yaml", "domain: 0\nlog_level: loud\n");
    let (status, stdout, stderr) = code(
        daemon()
            .arg("--config")
            .arg(&bad_value)
            .arg("--check-config"),
    );
    assert_eq!(status, Some(1), "{stdout}{stderr}");
    assert!(
        format!("{stdout}{stderr}").contains("log_level"),
        "{stdout}{stderr}"
    );

    let missing = std::env::temp_dir().join("zerodds-opcua-cli-no-such-file.yaml");
    let (status, _, stderr) = code(daemon().arg("--config").arg(&missing).arg("--check-config"));
    assert_eq!(status, Some(2), "{stderr}");
}

/// §2: `--log-level` takes the five names and no other; an unknown flag
/// is refused.
#[test]
fn opcuabridge_2_a_log_level_outside_the_five_names_is_refused() {
    let (status, _, stderr) = code(daemon().args(["--log-level", "loud", "--check-config"]));
    assert_eq!(status, Some(1), "{stderr}");
    let (status, _, _) = code(daemon().args(["--log-level", "error", "--check-config"]));
    assert_eq!(status, Some(0));
    let (status, _, stderr) = code(daemon().arg("--no-such-flag"));
    assert_eq!(status, Some(1));
    assert!(stderr.contains("--no-such-flag"), "{stderr}");
}

/// §2, §9.1, §9.2: with `--idl`, `--gateway-config`, `--domain`, `--bind`
/// and `--metrics` the daemon runs and names its two endpoints; SIGTERM
/// ends it with exit code 0.
#[test]
fn opcuabridge_9_2_sigterm_ends_the_running_daemon_with_exit_code_zero() {
    let (mut child, lines, _lease) = started(&["--bind", "127.0.0.1:0"], "sigterm");
    assert!(
        lines.iter().any(|l| l.contains("running on 127.0.0.1:")),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("metrics on 127.0.0.1:")),
        "{lines:?}"
    );
    signal(&child, "-TERM");
    assert_eq!(child.wait().expect("wait").code(), Some(0));
}

/// §9.2: SIGINT ends the daemon with exit code 0 as well.
#[test]
fn opcuabridge_9_2_sigint_ends_the_running_daemon_with_exit_code_zero() {
    let (mut child, lines, _lease) = started(&["--bind", "127.0.0.1:0"], "sigint");
    assert!(lines.iter().any(|l| l.contains("running on ")), "{lines:?}");
    signal(&child, "-INT");
    assert_eq!(child.wait().expect("wait").code(), Some(0));
}

/// §2: a `--set` assignment wins over the configuration file. The file
/// names an address no socket binds, so the daemon does not start with the
/// file alone; with the assignment it runs on the address assigned.
#[test]
fn opcuabridge_2_a_set_assignment_wins_over_the_file() {
    let file = fixture(
        "precedence.yaml",
        "opcua:\n  server:\n    bind: \"203.0.113.1:4840\"\n",
    );
    let file = file.to_str().unwrap().to_string();
    let (mut child, lines, _lease) = started(&["--config", &file], "file-only");
    assert!(
        !lines.iter().any(|l| l.contains("running on ")),
        "the file's address is not one of this host: {lines:?}"
    );
    assert_ne!(child.wait().expect("wait").code(), Some(0));

    let (mut child, lines, _lease) = started(
        &["--config", &file, "--set", "opcua.server.bind=127.0.0.1:0"],
        "file-and-set",
    );
    assert!(
        lines.iter().any(|l| l.contains("running on 127.0.0.1:")),
        "{lines:?}"
    );
    signal(&child, "-TERM");
    assert_eq!(child.wait().expect("wait").code(), Some(0));
}

/// §11: `zerodds-opcua-bridged(1)` names every flag the binary takes, and
/// the binary's own help names the same ones.
#[test]
fn opcuabridge_11_the_manual_page_names_every_flag_of_the_binary() {
    let page = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../man/man1/zerodds-opcua-bridged.1"),
    )
    .expect("the manual page")
    .replace("\\-", "-");
    let (status, help, _) = code(daemon().arg("--help"));
    assert_eq!(status, Some(0));
    for flag in [
        "--config",
        "--check-config",
        "--set",
        "--bind",
        "--domain",
        "--idl",
        "--gateway-config",
        "--log-level",
        "--metrics",
        "--version",
        "--help",
    ] {
        assert!(page.contains(flag), "zerodds-opcua-bridged(1) lacks {flag}");
        assert!(help.contains(flag), "--help lacks {flag}:\n{help}");
    }
}
