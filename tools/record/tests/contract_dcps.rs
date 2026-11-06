// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-record` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-dcps` (`src/main.rs`):
//! - `cli::Configured::from_args` over its arguments, `check_config`,
//!   `runtime` and `args` (its own subcommand and options, parsed by
//!   `zerodds_record::parse_args`), and `cli::HELP` in `print_help`. A
//!   configuration error is its exit code 2.
//! - `RuntimeConfig::codec_dialect`, the dialect its `--decode` sinks read
//!   the bodies in unless `--dialect` names another.
//! - `runtime::DcpsRuntime::start(domain, prefix, config)`,
//!   `discovered_publication_topics` (the writers' `(topic, type name)`
//!   pairs it follows), `register_user_reader` with a `UserReaderConfig`
//!   over `Default` that names the writer's topic and type name, and
//!   `shutdown`.
//! - `runtime::UserSample`: `Alive { payload, writer_guid, representation,
//!   big_endian, .. }` is recorded, `Lifecycle { kind, .. }` is recorded as
//!   disposed for `ChangeKind::NotAliveDisposed` and as unregistered
//!   otherwise, `CoherentSetEnd` is not recorded.
//!
//! What it relies on and is checked here:
//! - The configuration flags are taken out and everything else comes back
//!   in order, `--set` ranks above the file, an unknown `--set` path, a
//!   missing `--config` file and an invalid value are errors,
//!   `--check-config` reports instead of starting, the help text names the
//!   three flags. Oracles: `docs/specs/zerodds-deployment-1.0.md` §2.1
//!   "Runtime configuration from RC7 on", the rustdoc of `zerodds_dcps::cli`.
//! - `wire.codec_dialect` reaches `RuntimeConfig::codec_dialect`, default
//!   `specification`, and a name that is no dialect is refused. Oracle:
//!   `man/man5/zerodds.yaml.5`, `wire.codec_dialect` ("the default of
//!   zerodds-spy and zerodds-record --dialect", the names, "Default
//!   specification"), which the deployment specification names as the
//!   reference of the keys.
//! - A reader config over `Default` announces no TypeIdentifier, so it
//!   matches a writer by topic and type name (rustdoc of
//!   `UserReaderConfig`'s `Default`, `TypeIdentifier::None`).
//!
//! The capture needs a bound runtime; the calls and the sample variants are
//! checked at compile time. No socket is opened.
#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use zerodds_ast_layout::Dialect;
use zerodds_dcps::cli::{Configured, HELP};
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample};
use zerodds_rtps::history_cache::ChangeKind;
use zerodds_types::TypeIdentifier;

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| (*a).to_string()).collect()
}

/// A configuration file of this test alone; `--config` replaces the system
/// directory, so no file of the host takes part.
fn config_file(name: &str, text: &str) -> String {
    let path =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("contract-dcps-record-{name}.yaml"));
    std::fs::write(&path, text).unwrap();
    path.to_str().unwrap().to_string()
}

#[test]
fn the_configuration_flags_leave_the_record_arguments_in_order() {
    let config = config_file("args", "\n");
    let configured = Configured::from_args(strings(&[
        "record",
        "--topic",
        "tracks",
        "--config",
        &config,
        "--output",
        "run.zddsrec",
        "--set",
        "discovery.domain_tag=recorder",
    ]))
    .unwrap();
    assert_eq!(
        configured.args,
        strings(&["record", "--topic", "tracks", "--output", "run.zddsrec"])
    );
    assert_eq!(configured.runtime.domain_tag, "recorder");
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
    let missing = Path::new(env!("CARGO_TARGET_TMPDIR")).join("contract-dcps-record-absent.yaml");
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
    let config = config_file("dialect", "wire:\n  codec_dialect: cyclone\n");
    let from_file = Configured::from_args(strings(&["--config", &config])).unwrap();
    assert_eq!(from_file.runtime.codec_dialect, Dialect::Cyclone);
    let from_flag = Configured::from_args(strings(&[
        "--config",
        &config,
        "--set",
        "wire.codec_dialect=fastdds",
    ]))
    .unwrap();
    assert_eq!(from_flag.runtime.codec_dialect, Dialect::FastDds);
    assert_eq!(from_flag.runtime.codec_dialect.as_str(), "fastdds");
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

#[test]
fn a_reader_config_over_default_follows_the_type_name() {
    let cfg = UserReaderConfig {
        topic_name: "tracks".to_string(),
        type_name: "cuas::Track".to_string(),
        reliable: true,
        ..Default::default()
    };
    assert_eq!(cfg.type_identifier, TypeIdentifier::None);
    assert!(cfg.content_filter.is_none());
    assert!(cfg.partition.is_empty());
}

/// How `write_sample` files a runtime sample: `Some(Some(disposed))` for a
/// lifecycle marker, `Some(None)` for data, `None` for the end of a set.
fn filed_as(sample: &UserSample) -> Option<Option<bool>> {
    match sample {
        UserSample::Alive {
            payload,
            writer_guid,
            representation,
            big_endian,
            ..
        } => {
            let _: (&[u8], &[u8; 16], u8, bool) = (
                payload.as_slice(),
                writer_guid,
                *representation,
                *big_endian,
            );
            Some(None)
        }
        UserSample::Lifecycle { kind, .. } => {
            Some(Some(matches!(kind, ChangeKind::NotAliveDisposed)))
        }
        UserSample::CoherentSetEnd { .. } => None,
    }
}

#[test]
fn the_capture_calls_keep_the_shape_record_uses() {
    let _: fn(&UserSample) -> Option<Option<bool>> = filed_as;
    let _ = |config: &RuntimeConfig,
             cfg: UserReaderConfig|
     -> zerodds_dcps::Result<Receiver<UserSample>> {
        let rt: Arc<DcpsRuntime> =
            DcpsRuntime::start(0, zerodds_cli_common::stable_prefix(0xFE), config.clone())?;
        let _: Vec<(String, String)> = rt.discovered_publication_topics();
        let (_reader, rx) = rt.register_user_reader(cfg)?;
        let _: Dialect = config.codec_dialect;
        rt.shutdown();
        Ok(rx)
    };
}
