// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-admin` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-dcps` (`src/main.rs`):
//! - `cli::Configured::from_args` over its arguments, `check_config`,
//!   `runtime` (handed to every live probe) and `args` (the commands
//!   `domain inspect`, `config inspect`, ... and its own flags `--json`,
//!   `--timeout-ms`, `--skip-unknown`, `--vendor-forms`), and `cli::HELP`
//!   in its usage text. A configuration error is its exit code 2.
//! - `runtime::DcpsRuntime::start` with that `RuntimeConfig`, then
//!   `discovered_participants` (`sender_prefix.0`, `sender_vendor.0`),
//!   `discovered_publication_endpoints`, `discovered_subscription_endpoints`,
//!   `discovered_publications_count`, `discovered_subscriptions_count` and
//!   `shutdown`; of `runtime::DiscoveredEndpointInfo` it reads `topic_name`,
//!   `type_name`, `reliable`, `transient_local` and the first twelve octets
//!   of `endpoint_guid` as the owning participant's prefix.
//!
//! What it relies on and is checked here: the configuration flags are taken
//! out and everything else comes back in order, `--set` ranks above the
//! file, an unknown `--set` path, a missing `--config` file and an invalid
//! value are errors, `--check-config` reports instead of starting, and the
//! help text names the three flags. Oracles: `docs/specs/zerodds-deployment-1.0.md`
//! §2.1 "Runtime configuration from RC7 on" (the four layers, `--config`
//! must exist, an unknown `--set` path is rejected, invalid values prevent
//! the start, the `--check-config` exit codes), the rustdoc of
//! `zerodds_dcps::cli` (`args`: "The arguments that were not configuration
//! flags, in order"; `runtime`: compiled-in defaults under
//! `--check-config`), the rustdoc of `DiscoveredEndpointInfo::endpoint_guid`
//! (12-octet participant prefix, then the 4-octet entity id) and DDSI-RTPS
//! 2.5 §8.2.4.1 and §9.3.1 (a GUID is the prefix followed by the entity
//! id). The live probe needs a bound runtime; its calls are checked here
//! for the types the tool reads, at compile time. No socket is opened.
#![allow(clippy::unwrap_used)]

use std::path::Path;

use zerodds_dcps::cli::{Configured, HELP};
use zerodds_dcps::runtime::{DcpsRuntime, DiscoveredEndpointInfo, RuntimeConfig};

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| (*a).to_string()).collect()
}

/// A configuration file of this test alone; `--config` replaces the system
/// directory, so no file of the host takes part.
fn config_file(name: &str, text: &str) -> String {
    let path =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("contract-dcps-admin-{name}.yaml"));
    std::fs::write(&path, text).unwrap();
    path.to_str().unwrap().to_string()
}

#[test]
fn the_configuration_flags_leave_the_admin_arguments_in_order() {
    let config = config_file("args", "\n");
    let configured = Configured::from_args(strings(&[
        "--json",
        "--config",
        &config,
        "domain",
        "--set",
        "discovery.domain_tag=admin-probe",
        "inspect",
        "7",
        "--timeout-ms",
        "500",
    ]))
    .unwrap();
    assert_eq!(
        configured.args,
        strings(&["--json", "domain", "inspect", "7", "--timeout-ms", "500"])
    );
    assert_eq!(configured.runtime.domain_tag, "admin-probe");
    assert!(!configured.checking());
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
    let missing = Path::new(env!("CARGO_TARGET_TMPDIR")).join("contract-dcps-admin-absent.yaml");
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
    assert!(checking.checking());
    assert!(checking.check_config().is_some());
    assert_eq!(checking.check_config_valid(), Some(false));
    // Under `--check-config` the runtime holds the compiled-in defaults.
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

/// What `domain inspect` and `discovery snapshot` read of an endpoint: the
/// participant prefix is the first twelve of the sixteen GUID octets.
fn admin_view(e: &DiscoveredEndpointInfo) -> (&str, &str, [u8; 12], bool, bool) {
    let guid: &[u8; 16] = &e.endpoint_guid;
    let mut prefix = [0u8; 12];
    prefix.copy_from_slice(&guid[..12]);
    (
        e.topic_name.as_str(),
        e.type_name.as_str(),
        prefix,
        e.reliable,
        e.transient_local,
    )
}

/// What admin prints of an endpoint: topic, type, participant prefix,
/// reliable, transient local.
type AdminView<'a> = (&'a str, &'a str, [u8; 12], bool, bool);

#[test]
fn the_probe_queries_return_what_admin_reads() {
    let _: fn(&DiscoveredEndpointInfo) -> AdminView<'_> = admin_view;
    let _: fn(&DcpsRuntime) -> Vec<DiscoveredEndpointInfo> =
        DcpsRuntime::discovered_publication_endpoints;
    let _: fn(&DcpsRuntime) -> Vec<DiscoveredEndpointInfo> =
        DcpsRuntime::discovered_subscription_endpoints;
    let _: fn(&DcpsRuntime) -> usize = DcpsRuntime::discovered_publications_count;
    let _: fn(&DcpsRuntime) -> usize = DcpsRuntime::discovered_subscriptions_count;
    let _: fn(&DcpsRuntime) = DcpsRuntime::shutdown;
    let _ = |rt: &DcpsRuntime| -> Vec<([u8; 12], [u8; 2])> {
        rt.discovered_participants()
            .iter()
            .map(|p| (p.sender_prefix.0, p.sender_vendor.0))
            .collect()
    };
}
