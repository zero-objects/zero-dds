// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-durability-service-bin` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this daemon uses of `zerodds-dcps` (`src/main.rs`; `src/cli.rs` and
//! `src/config.rs` name `zerodds_dcps::cli` in their documentation):
//! - `cli::Configured::from_command_line(command_line, Vec::new(),
//!   "durability-svc", config::is_key, &[])`, after it has turned its own
//!   convenience flags into `--set` entries with
//!   `zerodds_config::CommandLine::push_set`; then `source.tree()` (its own
//!   keys `domain`, `store`, `path`, `auto`, `durability.topics`),
//!   `checking()`, `check_config()` and `runtime`, which it hands to
//!   `DurabilityService::start_with_config` as a `runtime::RuntimeConfig`.
//!   A configuration that does not load is exit code 2 under
//!   `--check-config` and 1 at start.
//! - `cli::HELP` in its help and usage text.
//! - `cli::notify_service_manager("READY=1")` once it serves and
//!   `("STOPPING=1")` when the shutdown begins.
//!
//! What it relies on and is checked here:
//! - Its own keys sit in the same files and flag layer as the runtime's: a
//!   `--set` of one of them ranks above the file, a `--set` of a path that
//!   neither it nor the runtime reads is refused, an unknown key in a file is
//!   a warning at start and a finding under `--check-config`, a missing
//!   `--config` file is an error also under `--check-config`. Oracles:
//!   `docs/specs/zerodds-deployment-1.0.md` §2.1 "Runtime configuration from
//!   RC7 on" (the layers, `--config` must exist, "A `--set` on a path that
//!   no setting reads is rejected", "Unknown keys in files produce a warning
//!   at startup", the `--check-config` exit codes, a daemon's own file with
//!   the runtime keys in it) and the rustdoc of
//!   `Configured::from_command_line` (convenience flags go into the flag
//!   layer first).
//! - The help text names the three configuration flags (same section).
//! - The service manager gets `READY=1` and `STOPPING=1` as one datagram
//!   each on the socket `$NOTIFY_SOCKET` names. Oracle: systemd
//!   `sd_notify(3)` and `docs/specs/zerodds-deployment-1.0.md` §2.3 (the
//!   units are `Type=notify`). The test runs itself as a child process with
//!   the variable set, so the environment of this process is not changed.
//!
//! The daemon is a binary without a library; `is_key` below is its
//! `config::KEYS`. `DurabilityService::start_with_config` opens sockets and
//! is not called here.
#![allow(clippy::unwrap_used)]

use std::path::Path;

use zerodds_config::CommandLine;
use zerodds_dcps::cli::{Configured, HELP};
use zerodds_dcps::runtime::RuntimeConfig;

/// `src/config.rs` `CONFIG_STEM`.
const CONFIG_STEM: &str = "durability-svc";

/// `src/config.rs` `is_key` over `KEYS`.
fn is_key(path: &str) -> bool {
    ["domain", "store", "path", "auto", "durability.topics"].contains(&path)
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| (*a).to_string()).collect()
}

/// A configuration file of this test alone; `--config` replaces the system
/// directory, so no file of the host takes part.
fn config_file(name: &str, text: &str) -> String {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("contract-dcps-durability-svc-{name}.yaml"));
    std::fs::write(&path, text).unwrap();
    path.to_str().unwrap().to_string()
}

/// What `main` does: take the configuration flags out, add the convenience
/// flags as `--set` entries, load.
fn load(args: &[&str], convenience: &[&str]) -> Result<Configured, zerodds_config::ConfigError> {
    let (mut command_line, rest) = CommandLine::extract(strings(args))?;
    assert!(rest.is_empty(), "the daemon passes no arguments of its own");
    for assignment in convenience {
        command_line.push_set(assignment)?;
    }
    Configured::from_command_line(command_line, Vec::new(), CONFIG_STEM, is_key, &[])
}

#[test]
fn the_daemon_keys_sit_in_the_flag_layer_beside_the_runtime_keys() {
    let config = config_file(
        "keys",
        "store: sqlite\ndomain: 3\ndiscovery:\n  domain_tag: from-file\n",
    );
    let configured = load(
        &[
            "--config",
            &config,
            "--set",
            "discovery.domain_tag=from-flag",
        ],
        &["store=file", "path=/var/lib/zerodds/durability"],
    )
    .unwrap();
    let tree = configured.source.tree().unwrap();
    assert_eq!(
        tree.get_str("store"),
        Some("file"),
        "the flag over the file"
    );
    assert_eq!(tree.get_str("domain"), Some("3"));
    assert_eq!(tree.get_str("path"), Some("/var/lib/zerodds/durability"));
    let runtime: &RuntimeConfig = &configured.runtime;
    assert_eq!(runtime.domain_tag, "from-flag");
    assert!(configured.args.is_empty());
    assert!(!configured.checking());
    assert!(configured.check_config().is_none());
}

#[test]
fn a_set_of_a_path_nobody_reads_is_refused() {
    let config = config_file("refused", "store: sqlite\n");
    assert!(load(&["--config", &config], &["no_such_section.key=1"]).is_err());
    assert!(load(&["--config", &config], &["store=file"]).is_ok());
}

#[test]
fn an_unknown_file_key_warns_at_start_and_fails_the_check() {
    let unknown = config_file("unknown", "store: sqlite\nno_such_section:\n  key: 1\n");
    assert!(load(&["--config", &unknown], &[]).is_ok());
    let checking = load(&["--config", &unknown, "--check-config"], &[]).unwrap();
    assert!(checking.checking());
    assert_eq!(checking.check_config_valid(), Some(false));

    let valid = config_file("valid", "store: sqlite\ndomain: 3\nauto: true\n");
    let checking = load(&["--config", &valid, "--check-config"], &[]).unwrap();
    assert!(checking.check_config().is_some());
    assert_eq!(checking.check_config_valid(), Some(true));
}

#[test]
fn a_missing_configuration_file_is_an_error_also_under_check_config() {
    let missing =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join("contract-dcps-durability-svc-absent.yaml");
    let _ = std::fs::remove_file(&missing);
    let missing = missing.to_str().unwrap();
    assert!(load(&["--config", missing], &[]).is_err());
    assert!(load(&["--config", missing, "--check-config"], &[]).is_err());
}

#[test]
fn the_help_text_names_the_configuration_flags() {
    for flag in ["--config", "--set", "--check-config"] {
        assert!(HELP.contains(flag), "{flag}");
    }
}

#[cfg(unix)]
#[test]
fn the_daemon_reports_ready_and_stopping_on_the_notify_socket() {
    const CHILD: &str = "ZERODDS_CONTRACT_DCPS_NOTIFY_CHILD";
    if std::env::var_os(CHILD).is_some() {
        // The child: what the daemon sends around its run.
        zerodds_dcps::cli::notify_service_manager("READY=1");
        zerodds_dcps::cli::notify_service_manager("STOPPING=1");
        return;
    }
    let path = std::env::temp_dir().join(format!("zd-notify-contract-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let socket = std::os::unix::net::UnixDatagram::bind(&path).unwrap();
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "the_daemon_reports_ready_and_stopping_on_the_notify_socket",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .env("NOTIFY_SOCKET", &path)
        .status()
        .unwrap();
    assert!(status.success());
    let mut buf = [0u8; 64];
    let n = socket.recv(&mut buf).unwrap();
    assert_eq!(&buf[..n], b"READY=1");
    let n = socket.recv(&mut buf).unwrap();
    assert_eq!(&buf[..n], b"STOPPING=1");
    let _ = std::fs::remove_file(path);
}
