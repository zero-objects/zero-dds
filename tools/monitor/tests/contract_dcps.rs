// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-monitor-cli` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-dcps` (`src/main.rs`):
//! - `cli::Configured::from_args` over its arguments, `check_config`,
//!   `runtime` and `args` (its own subcommand and options, parsed by
//!   `zerodds_monitor_cli::parse_args`), and `cli::HELP` in `print_help`. A
//!   configuration error is its exit code 2.
//! - `runtime::DcpsRuntime::start(domain, prefix, config)` for `snapshot`
//!   and `serve`; it then reads `zerodds_monitor::default_registry()`, the
//!   registry the DCPS entities of the process record into (feature
//!   `metrics`, a default feature of `zerodds-dcps`).
//!
//! What it relies on and is checked here:
//! - The configuration flags are taken out and everything else comes back
//!   in order, `--set` ranks above the file, an unknown `--set` path, a
//!   missing `--config` file and an invalid value are errors,
//!   `--check-config` reports instead of starting, the help text names the
//!   three flags. Oracles: `docs/specs/zerodds-deployment-1.0.md` §2.1
//!   "Runtime configuration from RC7 on", the rustdoc of
//!   `zerodds_dcps::cli`.
//! - A DataWriter's writes reach the default registry as
//!   `dds_dcps_samples_written_total` and `dds_dcps_sample_size_bytes`
//!   with the topic name as the `topic` label. Oracles:
//!   `docs/specs/zerodds-monitor-1.1.md` §2.3 (the DCPS metrics and their
//!   labels) and §9 (`TopicLabelPolicy`, default `Full`: the topic name
//!   unchanged); the rustdoc of `RawBytes` (its octets go out as they are,
//!   so a sample of n octets has n serialized octets).
//!
//! `DcpsRuntime::start` opens sockets; its signature is checked at compile
//! time. The DataWriter here belongs to a participant without a runtime.
#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::sync::Arc;

use zerodds_dcps::cli::{Configured, HELP};
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig};
use zerodds_dcps::{
    DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos, RawBytes, TopicQos,
};
use zerodds_monitor_lib::{Labels, default_registry, metric_names};

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| (*a).to_string()).collect()
}

/// A configuration file of this test alone; `--config` replaces the system
/// directory, so no file of the host takes part.
fn config_file(name: &str, text: &str) -> String {
    let path =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("contract-dcps-monitor-{name}.yaml"));
    std::fs::write(&path, text).unwrap();
    path.to_str().unwrap().to_string()
}

#[test]
fn the_configuration_flags_leave_the_monitor_arguments_in_order() {
    let config = config_file("args", "\n");
    let configured = Configured::from_args(strings(&[
        "snapshot",
        "--config",
        &config,
        "--domain",
        "4",
        "--set",
        "discovery.domain_tag=monitor-run",
        "--format",
        "prometheus",
    ]))
    .unwrap();
    assert_eq!(
        configured.args,
        strings(&["snapshot", "--domain", "4", "--format", "prometheus"])
    );
    assert_eq!(configured.runtime.domain_tag, "monitor-run");
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
    let missing = Path::new(env!("CARGO_TARGET_TMPDIR")).join("contract-dcps-monitor-absent.yaml");
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
fn writes_reach_the_registry_the_monitor_reads() {
    const TOPIC: &str = "contract-dcps-monitor-written";
    assert_eq!(
        metric_names::DDS_DCPS_SAMPLES_WRITTEN_TOTAL,
        "dds_dcps_samples_written_total"
    );
    assert_eq!(
        metric_names::DDS_DCPS_SAMPLE_SIZE_BYTES,
        "dds_dcps_sample_size_bytes"
    );
    let participant = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let topic = participant
        .create_topic::<RawBytes>(TOPIC, TopicQos::default())
        .unwrap();
    let writer = participant
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    for len in [3_usize, 5, 8] {
        writer.write(&RawBytes::new(vec![0xAB; len])).unwrap();
    }

    let snapshot = default_registry().snapshot();
    let of_topic = |labels: &Labels| labels.iter().any(|(k, v)| k == "topic" && v == TOPIC);
    let written = snapshot
        .counters
        .iter()
        .find(|(key, _)| {
            key.name == metric_names::DDS_DCPS_SAMPLES_WRITTEN_TOTAL && of_topic(&key.labels)
        })
        .map(|(_, value)| *value);
    assert_eq!(written, Some(3));
    let sizes = snapshot
        .histograms
        .iter()
        .find(|(key, _)| {
            key.name == metric_names::DDS_DCPS_SAMPLE_SIZE_BYTES && of_topic(&key.labels)
        })
        .map(|(_, histogram)| (histogram.count, histogram.sum_ns));
    assert_eq!(sizes, Some((3, 16)), "three samples, 3 + 5 + 8 octets");
}

#[test]
fn the_runtime_start_keeps_its_shape() {
    // `start_runtime`: the prefix of `zerodds_cli_common::stable_prefix`.
    let _ = |config: &RuntimeConfig| -> zerodds_dcps::Result<Arc<DcpsRuntime>> {
        DcpsRuntime::start(4, zerodds_cli_common::stable_prefix(0xFC), config.clone())
    };
}
