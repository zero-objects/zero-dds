// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-mq` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-dcps` (`src/main.rs`):
//! - `cli::Configured::from_args` over its arguments, `check_config`,
//!   `runtime` (the configuration of both bridged runtimes) and `args` (its
//!   own subcommand and options, parsed by `zerodds_mq::parse_args`), and
//!   `cli::HELP` in `print_help`. A configuration error is its exit code 2.
//! - `runtime::DcpsRuntime::start` once per domain, `register_user_reader`
//!   with `zerodds_cli_common::raw_reader_config`, `register_user_writer`
//!   with the `UserWriterConfig` of
//!   `zerodds_cli_common::reliable_raw_writer_config`, and
//!   `write_user_sample_borrowed(eid, payload.as_slice())` for every
//!   `UserSample::Alive { payload, .. }` it receives: the bridge forwards
//!   the body unchanged.
//!
//! What it relies on and is checked here:
//! - The configuration flags are taken out and everything else comes back
//!   in order, `--set` ranks above the file, an unknown `--set` path, a
//!   missing `--config` file and an invalid value are errors,
//!   `--check-config` reports instead of starting, the help text names the
//!   three flags. Oracles: `docs/specs/zerodds-deployment-1.0.md` §2.1
//!   "Runtime configuration from RC7 on", the rustdoc of `zerodds_dcps::cli`.
//! - `payload.as_slice()` is the body the source writer wrote and nothing
//!   around it, so the destination writer frames it once. Oracles: the
//!   rustdoc of `UserSample::Alive::payload` (the CDR payload without the
//!   encapsulation header) and of `sample_bytes::SampleBytes` (a view range
//!   over shared octets; `as_slice` and `Deref` give the view).
//!
//! The bridge needs two bound runtimes; the calls are checked at compile
//! time. No socket is opened.
#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use zerodds_dcps::cli::{Configured, HELP};
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserSample};
use zerodds_dcps::sample_bytes::SampleBytes;

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| (*a).to_string()).collect()
}

/// A configuration file of this test alone; `--config` replaces the system
/// directory, so no file of the host takes part.
fn config_file(name: &str, text: &str) -> String {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("contract-dcps-mq-{name}.yaml"));
    std::fs::write(&path, text).unwrap();
    path.to_str().unwrap().to_string()
}

#[test]
fn the_configuration_flags_leave_the_bridge_arguments_in_order() {
    let config = config_file("args", "\n");
    let configured = Configured::from_args(strings(&[
        "bridge",
        "--src-domain",
        "1",
        "--config",
        &config,
        "--dst-domain",
        "2",
        "--set",
        "discovery.domain_tag=mq-bridge",
        "--topic",
        "chatter",
    ]))
    .unwrap();
    assert_eq!(
        configured.args,
        strings(&[
            "bridge",
            "--src-domain",
            "1",
            "--dst-domain",
            "2",
            "--topic",
            "chatter"
        ])
    );
    assert_eq!(configured.runtime.domain_tag, "mq-bridge");
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
    let missing = Path::new(env!("CARGO_TARGET_TMPDIR")).join("contract-dcps-mq-absent.yaml");
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
fn the_forwarded_payload_is_the_body_and_nothing_around_it() {
    // A received datagram: a four-octet encapsulation header, then the body.
    let body = [0x10, 0x20, 0x30, 0x40, 0x50];
    let mut datagram = vec![0x00, 0x01, 0x00, 0x00];
    datagram.extend_from_slice(&body);
    let datagram: Arc<[u8]> = Arc::from(datagram.into_boxed_slice());
    let payload = SampleBytes::from_arc_slice(Arc::clone(&datagram), 4..datagram.len());
    assert_eq!(payload.as_slice(), body);
    assert_eq!(&payload[..], &body[..]);
    assert_eq!(payload.to_vec(), body.to_vec());
    // A sub-view stays a view of the same octets.
    assert_eq!(payload.slice(1..3).as_slice(), &body[1..3]);
}

#[test]
fn the_bridge_calls_keep_the_shape_mq_uses() {
    let _ = |config: &RuntimeConfig| -> zerodds_dcps::Result<()> {
        let src = DcpsRuntime::start(1, zerodds_cli_common::stable_prefix(0xF9), config.clone())?;
        let dst = DcpsRuntime::start(2, zerodds_cli_common::stable_prefix(0xF8), config.clone())?;
        let (_reader, rx) = src.register_user_reader(zerodds_cli_common::raw_reader_config("t"))?;
        let writer = dst.register_user_writer(zerodds_cli_common::reliable_raw_writer_config(
            "t",
            "zerodds::RawBytes",
        ))?;
        if let Ok(UserSample::Alive { payload, .. }) = rx.recv_timeout(Duration::from_millis(50)) {
            dst.write_user_sample_borrowed(writer, payload.as_slice())?;
        }
        Ok(())
    };
}
