// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-spy` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-dcps` (`src/main.rs`):
//! - `cli::Configured::from_args` over its arguments, `check_config`,
//!   `runtime` (moved into `DcpsRuntime::start`) and `args` (its own
//!   subcommand and options, parsed by `zerodds_spy::parse_args`), and
//!   `cli::HELP` in its usage text. A configuration error is its exit
//!   code 2.
//! - `runtime::DcpsRuntime::start(domain, prefix, config)`,
//!   `codec_dialect()` (the dialect of its `--decode` path unless
//!   `--dialect` names another), `register_user_reader` with
//!   `zerodds_cli_common::raw_reader_config_typed`, `shutdown`.
//! - `runtime::UserSample`: `Alive { payload, writer_guid, representation,
//!   big_endian, .. }` is decoded or printed as hex, `Lifecycle { kind, .. }`
//!   printed with `{kind:?}`, `CoherentSetEnd { set_first_sn, .. }` printed
//!   with `{set_first_sn:?}`.
//!
//! What it relies on and is checked here:
//! - The configuration flags are taken out and everything else comes back
//!   in order, `--set` ranks above the file, an unknown `--set` path, a
//!   missing `--config` file and an invalid value are errors,
//!   `--check-config` reports instead of starting, the help text names the
//!   three flags. Oracles: `docs/specs/zerodds-deployment-1.0.md` §2.1
//!   "Runtime configuration from RC7 on", the rustdoc of `zerodds_dcps::cli`.
//! - `wire.codec_dialect` reaches `RuntimeConfig::codec_dialect`, which
//!   `DcpsRuntime::codec_dialect` returns, default `specification`; a name
//!   that is no dialect is refused. Oracles: `man/man5/zerodds.yaml.5`,
//!   `wire.codec_dialect` ("the default of zerodds-spy and zerodds-record
//!   --dialect", "Default specification"), the rustdoc of
//!   `DcpsRuntime::codec_dialect`.
//!
//! The subscription needs a bound runtime; the calls and the sample
//! variants are checked at compile time. No socket is opened.
#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use zerodds_ast_layout::Dialect;
use zerodds_dcps::cli::{Configured, HELP};
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserSample};

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| (*a).to_string()).collect()
}

/// A configuration file of this test alone; `--config` replaces the system
/// directory, so no file of the host takes part.
fn config_file(name: &str, text: &str) -> String {
    let path =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("contract-dcps-spy-{name}.yaml"));
    std::fs::write(&path, text).unwrap();
    path.to_str().unwrap().to_string()
}

#[test]
fn the_configuration_flags_leave_the_spy_arguments_in_order() {
    let config = config_file("args", "\n");
    let configured = Configured::from_args(strings(&[
        "subscribe",
        "--topic",
        "tracks",
        "--config",
        &config,
        "--hex-bytes",
        "16",
        "--set",
        "discovery.domain_tag=spy",
    ]))
    .unwrap();
    assert_eq!(
        configured.args,
        strings(&["subscribe", "--topic", "tracks", "--hex-bytes", "16"])
    );
    assert_eq!(configured.runtime.domain_tag, "spy");
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
    let missing = Path::new(env!("CARGO_TARGET_TMPDIR")).join("contract-dcps-spy-absent.yaml");
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
fn the_codec_dialect_setting_is_the_decode_default() {
    assert_eq!(
        RuntimeConfig::default().codec_dialect,
        Dialect::Specification
    );
    let config = config_file("dialect", "wire:\n  codec_dialect: opendds\n");
    let from_file = Configured::from_args(strings(&["--config", &config])).unwrap();
    assert_eq!(from_file.runtime.codec_dialect, Dialect::OpenDds);
    let from_flag = Configured::from_args(strings(&[
        "--config",
        &config,
        "--set",
        "wire.codec_dialect=rti",
    ]))
    .unwrap();
    assert_eq!(from_flag.runtime.codec_dialect, Dialect::Rti);
    assert!(
        Configured::from_args(strings(&[
            "--config",
            &config,
            "--set",
            "wire.codec_dialect=no-such-dialect",
        ]))
        .is_err()
    );
}

/// The line `run_subscribe` prints for a sample that is not decoded.
fn printed(sample: &UserSample) -> String {
    match sample {
        UserSample::Alive {
            payload,
            writer_guid,
            representation,
            big_endian,
            ..
        } => format!(
            "writer={:02x}{:02x} bytes={} rep={representation} be={big_endian}",
            writer_guid[12],
            writer_guid[15],
            payload.len()
        ),
        UserSample::Lifecycle { kind, .. } => format!("[lifecycle] {kind:?}"),
        UserSample::CoherentSetEnd { set_first_sn, .. } => {
            let first: &Option<i64> = set_first_sn;
            format!("[coherent-set-end] first_sn={first:?}")
        }
    }
}

#[test]
fn the_subscription_calls_keep_the_shape_spy_uses() {
    let _: fn(&UserSample) -> String = printed;
    let _: fn(&DcpsRuntime) -> Dialect = DcpsRuntime::codec_dialect;
    let _ = |config: RuntimeConfig| -> zerodds_dcps::Result<Receiver<UserSample>> {
        let rt: Arc<DcpsRuntime> =
            DcpsRuntime::start(0, zerodds_cli_common::stable_prefix(0xFB), config)?;
        let (_reader, rx) = rt.register_user_reader(
            zerodds_cli_common::raw_reader_config_typed("tracks", "cuas::Track"),
        )?;
        rt.shutdown();
        Ok(rx)
    };
}
