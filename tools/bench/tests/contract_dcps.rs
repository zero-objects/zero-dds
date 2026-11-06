// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-bench` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-dcps` (`src/main.rs`):
//! - `cli::Configured::from_args` over its arguments, `check_config`,
//!   `runtime` and `args` (its own subcommand and options, parsed by
//!   `zerodds_bench::parse_args`), and `cli::HELP` in `print_help`. A
//!   configuration error is its exit code 2.
//! - `runtime::DcpsRuntime::start(domain, prefix, config)`,
//!   `register_user_writer` with the `UserWriterConfig` of
//!   `zerodds_cli_common::reliable_raw_writer_config`, `register_user_reader`
//!   (the receiver of `UserSample`s), `write_user_sample` and `shutdown`.
//! - `runtime::UserSample::Alive { payload, .. }`: the latency mode writes
//!   its send time into the first eight octets of the payload and reads
//!   them back from `payload[0..8]` of the echo.
//!
//! What it relies on and is checked here: the configuration flags are taken
//! out and everything else comes back in order, `--set` ranks above the
//! file, an unknown `--set` path, a missing `--config` file and an invalid
//! value are errors, `--check-config` reports instead of starting, the help
//! text names the three flags, and the payload of a sample is a view of the
//! body that starts at its first octet. Oracles:
//! `docs/specs/zerodds-deployment-1.0.md` §2.1 "Runtime configuration from
//! RC7 on", the rustdoc of `zerodds_dcps::cli` (`args` in order, `runtime`
//! the compiled-in defaults under `--check-config`), the rustdoc of
//! `UserSample::Alive::payload` (the CDR payload without the encapsulation
//! header) and of `sample_bytes::SampleBytes` (a view range over shared
//! octets, dereferenced to the view). The runtime calls need a bound
//! runtime; their signatures are checked at compile time. No socket is
//! opened.
#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use zerodds_dcps::cli::{Configured, HELP};
use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_dcps::sample_bytes::SampleBytes;
use zerodds_rtps::wire_types::{EntityId, GuidPrefix};

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| (*a).to_string()).collect()
}

/// A configuration file of this test alone; `--config` replaces the system
/// directory, so no file of the host takes part.
fn config_file(name: &str, text: &str) -> String {
    let path =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("contract-dcps-bench-{name}.yaml"));
    std::fs::write(&path, text).unwrap();
    path.to_str().unwrap().to_string()
}

#[test]
fn the_configuration_flags_leave_the_bench_arguments_in_order() {
    let config = config_file("args", "\n");
    let configured = Configured::from_args(strings(&[
        "latency",
        "--config",
        &config,
        "-d",
        "3",
        "--set",
        "discovery.domain_tag=bench-run",
        "--payload",
        "64",
    ]))
    .unwrap();
    assert_eq!(
        configured.args,
        strings(&["latency", "-d", "3", "--payload", "64"])
    );
    assert_eq!(configured.runtime.domain_tag, "bench-run");
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
    let missing = Path::new(env!("CARGO_TARGET_TMPDIR")).join("contract-dcps-bench-absent.yaml");
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
fn the_payload_is_a_view_that_starts_at_the_first_octet_of_the_body() {
    // A received datagram: a four-octet encapsulation header, then the body
    // the latency mode wrote (send time, little endian, then filler).
    let sent: u64 = 0x0102_0304_0506_0708;
    let mut datagram = vec![0x00, 0x01, 0x00, 0x00];
    datagram.extend_from_slice(&sent.to_le_bytes());
    datagram.extend_from_slice(&[0xAA; 56]);
    let datagram: Arc<[u8]> = Arc::from(datagram.into_boxed_slice());
    let payload = SampleBytes::from_arc_slice(Arc::clone(&datagram), 4..datagram.len());
    assert_eq!(payload.len(), 64);
    assert_eq!(&payload[..], &datagram[4..]);
    let first: [u8; 8] = payload[0..8].try_into().unwrap();
    assert_eq!(u64::from_le_bytes(first), sent);

    let owned = SampleBytes::from_vec(sent.to_le_bytes().to_vec());
    assert_eq!(owned.as_slice(), sent.to_le_bytes());
}

/// The latency mode's reading of an echo.
fn sent_at(sample: &UserSample) -> Option<u64> {
    match sample {
        UserSample::Alive { payload, .. } if payload.len() >= 8 => {
            let first: [u8; 8] = payload[0..8].try_into().ok()?;
            Some(u64::from_le_bytes(first))
        }
        _ => None,
    }
}

/// A registered reader and the channel of its samples.
type ReaderAndSamples = (EntityId, Receiver<UserSample>);

#[test]
fn the_runtime_calls_keep_the_shape_bench_uses() {
    let _: fn(&UserSample) -> Option<u64> = sent_at;
    let _: fn(i32, GuidPrefix, RuntimeConfig) -> zerodds_dcps::Result<Arc<DcpsRuntime>> =
        DcpsRuntime::start;
    let _: fn(&DcpsRuntime, UserWriterConfig) -> zerodds_dcps::Result<EntityId> =
        DcpsRuntime::register_user_writer;
    let _: fn(&DcpsRuntime, UserReaderConfig) -> zerodds_dcps::Result<ReaderAndSamples> =
        DcpsRuntime::register_user_reader;
    let _: fn(&DcpsRuntime, EntityId, Vec<u8>) -> zerodds_dcps::Result<()> =
        DcpsRuntime::write_user_sample;
    let _: fn(&DcpsRuntime) = DcpsRuntime::shutdown;
}
