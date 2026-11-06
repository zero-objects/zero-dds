// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-snitch` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-dcps` (`src/main.rs`):
//! - `cli::Configured::from_args` over its arguments, `check_config`,
//!   `runtime` and `args` (its own subcommand and options, parsed by
//!   `zerodds_snitch::parse_args`), and `cli::HELP` in `print_help`. A
//!   configuration error is its exit code 2.
//! - `runtime::DcpsRuntime::start(domain, prefix, config)`, then
//!   `discovered_participants` (as `zerodds_discovery::spdp::
//!   DiscoveredParticipant`, rendered by `render_text`/`render_json`),
//!   `discovered_publications_count`, `discovered_subscriptions_count` and
//!   `shutdown`.
//!
//! What it relies on and is checked here: the configuration flags are taken
//! out and everything else comes back in order, `--set` ranks above the
//! file, an unknown `--set` path, a missing `--config` file and an invalid
//! value are errors, `--check-config` reports instead of starting, and the
//! help text names the three flags. Oracles:
//! `docs/specs/zerodds-deployment-1.0.md` §2.1 "Runtime configuration from
//! RC7 on", the rustdoc of `zerodds_dcps::cli`. The probe needs a bound
//! runtime; the snapshot calls are checked at compile time for the types
//! the tool renders (`tests/contract_discovery.rs` holds what a
//! `DiscoveredParticipant` carries). No socket is opened.
#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::sync::Arc;

use zerodds_dcps::cli::{Configured, HELP};
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig};
use zerodds_discovery::spdp::DiscoveredParticipant;

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| (*a).to_string()).collect()
}

/// A configuration file of this test alone; `--config` replaces the system
/// directory, so no file of the host takes part.
fn config_file(name: &str, text: &str) -> String {
    let path =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("contract-dcps-snitch-{name}.yaml"));
    std::fs::write(&path, text).unwrap();
    path.to_str().unwrap().to_string()
}

#[test]
fn the_configuration_flags_leave_the_snitch_arguments_in_order() {
    let config = config_file("args", "\n");
    let configured = Configured::from_args(strings(&[
        "probe",
        "--config",
        &config,
        "--domain",
        "5",
        "--set",
        "discovery.domain_tag=snitch-probe",
        "--format",
        "json",
    ]))
    .unwrap();
    assert_eq!(
        configured.args,
        strings(&["probe", "--domain", "5", "--format", "json"])
    );
    assert_eq!(configured.runtime.domain_tag, "snitch-probe");
    assert!(configured.check_config().is_none());
}

#[test]
fn a_set_flag_ranks_above_the_configuration_file() {
    let config = config_file("precedence", "discovery:\n  domain_tag: from-file\n");
    let from_file = Configured::from_args(strings(&["--config", &config])).unwrap();
    assert_eq!(from_file.runtime.domain_tag, "from-file");
    let from_flag = Configured::from_args(strings(&[
        "--config",
        &config,
        "--set",
        "discovery.domain_tag=from-flag",
    ]))
    .unwrap();
    assert_eq!(from_flag.runtime.domain_tag, "from-flag");
}

#[test]
fn an_unknown_set_path_and_a_missing_file_are_configuration_errors() {
    let config = config_file("unknown", "\n");
    assert!(
        Configured::from_args(strings(&[
            "--config",
            &config,
            "--set",
            "discovery.no_such_setting=1",
        ]))
        .is_err()
    );
    let missing = Path::new(env!("CARGO_TARGET_TMPDIR")).join("contract-dcps-snitch-absent.yaml");
    let _ = std::fs::remove_file(&missing);
    assert!(Configured::from_args(strings(&["--config", missing.to_str().unwrap()])).is_err());
}

#[test]
fn an_invalid_value_stops_the_start_and_check_config_reports_it() {
    // `transport.recv_batch` takes a count above zero.
    let config = config_file("invalid", "transport:\n  recv_batch: 0\n");
    assert!(Configured::from_args(strings(&["--config", &config])).is_err());
    let checking =
        Configured::from_args(strings(&["--config", &config, "--check-config"])).unwrap();
    assert!(checking.check_config().is_some());
    assert_eq!(checking.check_config_valid(), Some(false));
    assert_eq!(
        checking.runtime.recv_batch_size,
        RuntimeConfig::default().recv_batch_size
    );

    let valid = config_file("valid", "discovery:\n  domain_tag: checked\n");
    let checking = Configured::from_args(strings(&["--config", &valid, "--check-config"])).unwrap();
    assert_eq!(checking.check_config_valid(), Some(true));
}

#[test]
fn the_help_text_names_the_configuration_flags() {
    for flag in ["--config", "--set", "--check-config"] {
        assert!(HELP.contains(flag), "{flag}");
    }
}

#[test]
fn the_snapshot_calls_keep_the_shape_snitch_renders() {
    let _: fn(&DcpsRuntime) -> Vec<DiscoveredParticipant> = DcpsRuntime::discovered_participants;
    let _: fn(&DcpsRuntime) -> usize = DcpsRuntime::discovered_publications_count;
    let _: fn(&DcpsRuntime) -> usize = DcpsRuntime::discovered_subscriptions_count;
    let _ = |config: &RuntimeConfig| -> zerodds_dcps::Result<Arc<DcpsRuntime>> {
        let rt = DcpsRuntime::start(0, zerodds_cli_common::stable_prefix(0xFA), config.clone())?;
        rt.shutdown();
        Ok(rt)
    };
}
