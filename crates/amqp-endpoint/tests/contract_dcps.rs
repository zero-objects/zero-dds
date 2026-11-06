// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-amqp-endpoint` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-dcps`, all behind its `std` feature:
//! - `src/bin/zerodds-amqp-bridged.rs`: `cli::Configured::from_command_line`
//!   twice (start and SIGHUP reload) with the flags
//!   `zerodds_config::CommandLine::extract` took out and the daemon's
//!   convenience flags pushed as `--set` entries, the base name
//!   `amqp-bridged` (a constant of the binary), the key predicate
//!   `bridge_daemon::is_key` and the daemon's own environment name
//!   `bridge_daemon::ENV` (`AMQP_PASSWORD` for `amqp.sasl.password`);
//!   `configured.source.tree()` for `DaemonConfig::from_config`,
//!   `checking`, `check_config` and `configured.runtime`;
//!   `cli::notify_service_manager("STOPPING=1")`.
//! - `src/bridge_daemon.rs`: `runtime::{DcpsRuntime, RuntimeConfig,
//!   UserReaderConfig, UserSample, UserWriterConfig}`;
//!   `participant::bridge_guid_prefix` of the container id, whose octets the
//!   bridge also keeps to drop the samples it wrote itself;
//!   `DcpsRuntime::start`; per topic `register_user_reader_kind` and
//!   `register_user_writer_kind` with a `UserReaderConfig` /
//!   `UserWriterConfig` that sets topic, type, `reliable` and `durability`
//!   over `Default`, `set_user_writer_history_depth`,
//!   `set_user_writer_key_support` with a
//!   `serialized_type::SerializedTypeSupport::from_complete_type_objects` of
//!   the topic's `type_file` (refused unless `is_keyed()`); the
//!   `UserSample::Alive` fields `key_hash`, `payload`, `writer_guid`,
//!   `representation`, `big_endian`, `source_timestamp` (through
//!   `time::he_timestamp_to_time` into nanoseconds), `source_sequence_number`
//!   and `lifespan_nanos`, `remember_user_writer_instance`,
//!   `UserSample::Lifecycle` (`key_hash`, `kind`), `CoherentSetEnd`; on the
//!   way into DDS `set_user_writer_data_rep_override` (`[XCDR]` or
//!   `[XCDR2]` from the encapsulation header of the AMQP body),
//!   `set_user_writer_byte_order_override`, `write_user_sample_keyed` /
//!   `write_user_sample_borrowed` and `write_user_lifecycle`;
//!   `cli::notify_service_manager("READY=1")` and `("STOPPING=1")`.
//!
//! What it relies on and is checked here without a socket: the layers of
//! the daemon's own file, keys and environment name, the runtime keys in that
//! file, the rank of a `--set`, the refusal of an unknown `--set` path and of
//! an invalid runtime value, the warning on an unknown file key,
//! `--check-config` and its report without the SASL password; the GUID prefix
//! of the bridge; the policies the bridge leaves at `Default`; the source
//! timestamp in seconds and nanoseconds; a type support that says whether
//! its type is keyed.
//!
//! Oracles: `docs/specs/zerodds-deployment-1.0.md` §2.1 ("Runtime
//! configuration from RC7 on": the four layers, the environment beneath the
//! files, `--config PATH` must exist, a `--set` no setting reads is rejected,
//! unknown file keys warn, the exit codes of `--check-config`, a bridge
//! daemon's own file carries the runtime keys), the rustdoc of
//! `ConfigSource::for_program` (`app_env` beneath the files),
//! `zerodds_config::is_secret_path` and `zerodds_config::Origin`, DDS-XTypes
//! 1.3 §7.6.3.1.1 (`XCDR_DATA_REPRESENTATION = 0`,
//! `XCDR2_DATA_REPRESENTATION = 2`, the default `[XCDR]`), DDSI-RTPS 2.5
//! §9.3.1.5 (the first two octets of a GUID prefix are the vendor id) and
//! §9.3.2 (`Time_t`: seconds and a fraction in units of 2^-32 s, the reserved
//! `TIME_INVALID` and `TIME_INFINITE`), the rustdoc of `bridge_guid_prefix`,
//! of `he_timestamp_to_time` (rounded to the nearest nanosecond) and of
//! `UserWriterConfig` / `UserReaderConfig` (every member is the DDS default of
//! its policy), DDS 1.4 §2.2.3 (the defaults), and the COMPLETE TypeObject
//! Cyclone DDS 11.0.1 `idlc` writes for `module to { @final struct Plain {
//! long a; short b; }; }` (recorded in
//! `crates/types/tests/typeobject_vendor_dheader.rs`).
//!
//! Left to the integration pillar, because only a bound runtime shows it: how
//! `representation`, `big_endian` and `representation_identifier` of a
//! delivered `UserSample::Alive` follow the encapsulation header of the
//! sample (DDSI-RTPS 2.5 §10.5, DDS-XTypes 1.3 Table 60), the header the
//! writer puts in front of a body after the two overrides, the key a
//! dispose carries after `remember_user_writer_instance`, and
//! `notify_service_manager`, which sends a datagram to `$NOTIFY_SOCKET`.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use zerodds_amqp_endpoint::bridge_daemon::{ENV, is_key};
use zerodds_config::{CommandLine, ConfigError, Origin};
use zerodds_dcps::cli::{Configured, HELP};
use zerodds_dcps::participant::{bridge_guid_prefix, host_id_bytes};
use zerodds_dcps::runtime::{ConfigSource, UserReaderConfig, UserWriterConfig};
use zerodds_dcps::serialized_type::SerializedTypeSupport;
use zerodds_dcps::time::{Time, he_timestamp_to_time};
use zerodds_qos::{DestinationOrderKind, LivelinessKind, OwnershipKind, PresentationAccessScope};
use zerodds_rtps::header_extension::HeTimestamp;
use zerodds_rtps::wire_types::VendorId;
use zerodds_types::type_object::TypeObject;

/// The base name of the daemon's file (`src/bin/zerodds-amqp-bridged.rs`).
const CONFIG_STEM: &str = "amqp-bridged";
/// DDS-XTypes 1.3 §7.6.3.1.1: `XCDR_DATA_REPRESENTATION`.
const XCDR: i16 = 0;
/// DDS-XTypes 1.3 §7.6.3.1.1: `XCDR2_DATA_REPRESENTATION`.
const XCDR2: i16 = 2;
/// Cyclone DDS 11.0.1 `idlc`: `module to { @final struct Plain { long a;
/// short b; }; }`, COMPLETE, XCDR2 little endian.
const PLAIN_COMPLETE: &str = "4c000000f251010012000000000000000a000000746f3a3a506c61696e0000002c0000000200000010000000000000000100040002000000610000001000000001000000010003000200000062000000";

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// A daemon file of this test alone, so that no system file takes part.
fn daemon_file(name: &str, text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "zerodds-amqp-contract-dcps-{}-{name}.yaml",
        std::process::id()
    ));
    std::fs::write(&path, text).unwrap();
    path
}

/// What the daemon's `main` does with its arguments: the configuration
/// flags out, then the layers of its own file, keys and environment names.
fn configured(file: &Path, flags: &[&str]) -> Result<Configured, ConfigError> {
    let mut args = vec![String::from("--config"), file.display().to_string()];
    args.extend(flags.iter().map(|flag| (*flag).to_string()));
    let (command_line, rest) = CommandLine::extract(args)?;
    assert!(rest.is_empty(), "{rest:?}");
    Configured::from_command_line(command_line, Vec::new(), CONFIG_STEM, is_key, ENV)
}

#[test]
fn the_daemon_file_holds_its_own_keys_and_the_runtime_keys() {
    let file = daemon_file(
        "keys",
        "domain: 7\namqp:\n  container_id: bridge-a\nwire:\n  data_representation_offer: XCDR2\n",
    );
    let configured = configured(&file, &[]).unwrap();
    let tree = configured.source.tree().unwrap();
    assert_eq!(tree.get_str("domain"), Some("7"));
    assert_eq!(tree.get_str("amqp.container_id"), Some("bridge-a"));
    // The runtime key of the daemon's file reaches the DDS side.
    assert_eq!(configured.runtime.data_representation_offer, [XCDR2]);
    assert!(!configured.checking());
    assert!(configured.check_config_valid().is_none());
    let _ = std::fs::remove_file(file);
}

#[test]
fn a_convenience_flag_pushed_as_set_ranks_above_the_daemon_file() {
    let file = daemon_file("precedence", "amqp:\n  tls:\n    enabled: false\n");
    let (mut command_line, _) =
        CommandLine::extract([String::from("--config"), file.display().to_string()]).unwrap();
    // `--tls` becomes this entry.
    command_line.push_set("amqp.tls.enabled=true").unwrap();
    let configured =
        Configured::from_command_line(command_line, Vec::new(), CONFIG_STEM, is_key, ENV).unwrap();
    let tree = configured.source.tree().unwrap();
    assert_eq!(tree.get_str("amqp.tls.enabled"), Some("true"));
    assert_eq!(
        tree.origin_of("amqp.tls.enabled"),
        Some(&Origin::Cli(String::from("--set amqp.tls.enabled=true")))
    );
    let _ = std::fs::remove_file(file);
}

#[test]
fn the_daemon_environment_name_sits_beneath_the_daemon_file() {
    // `Configured::from_command_line` reads the process environment through
    // `ConfigSource::for_program`; here the environment is given.
    let file = daemon_file("env", "domain: 0\n");
    let command_line = CommandLine::default().with_config(file.clone());
    let env = |name: &str| (name == "AMQP_PASSWORD").then(|| String::from("from-env"));
    let source = ConfigSource::for_program(&command_line, CONFIG_STEM, &is_key, ENV, env).unwrap();
    let tree = source.tree().unwrap();
    assert_eq!(tree.get_str("amqp.sasl.password"), Some("from-env"));
    assert_eq!(
        tree.origin_of("amqp.sasl.password"),
        Some(&Origin::Env(String::from("AMQP_PASSWORD")))
    );
    std::fs::write(&file, "amqp:\n  sasl:\n    password: from-file\n").unwrap();
    let source = ConfigSource::for_program(&command_line, CONFIG_STEM, &is_key, ENV, env).unwrap();
    assert_eq!(
        source.tree().unwrap().get_str("amqp.sasl.password"),
        Some("from-file")
    );
    let _ = std::fs::remove_file(file);
}

#[test]
fn a_set_flag_no_setting_reads_is_refused() {
    let file = daemon_file("unknown-set", "domain: 0\n");
    assert!(matches!(
        configured(&file, &["--set", "amqp.no_such_key=1"]),
        Err(ConfigError::Usage(_))
    ));
    let _ = std::fs::remove_file(file);
}

#[test]
fn an_invalid_runtime_value_stops_the_start_and_check_config_reports_it() {
    let file = daemon_file(
        "invalid",
        "domain: 0\nwire:\n  data_representation_offer: XCDR3\n",
    );
    match configured(&file, &[]) {
        Err(ConfigError::Invalid {
            setting, origin, ..
        }) => {
            assert_eq!(setting, "wire.data_representation_offer");
            assert!(matches!(origin, Origin::File { line: 3, .. }), "{origin:?}");
        }
        other => panic!("{other:?}"),
    }
    // Under `--check-config` the daemon validates instead of failing: the
    // compiled-in defaults, and a report that says invalid (exit 1).
    let checked = configured(&file, &["--check-config"]).unwrap();
    assert!(checked.checking());
    assert_eq!(checked.runtime.data_representation_offer, [XCDR]);
    assert_eq!(checked.check_config_valid(), Some(false));
    let _ = std::fs::remove_file(file);
}

#[test]
fn an_unknown_key_in_the_daemon_file_warns_at_start_and_fails_the_check() {
    let file = daemon_file("unknown-key", "amqp:\n  no_such_key: 1\n");
    let started = configured(&file, &[]).unwrap();
    let unknown = started.source.unknown_keys(&is_key);
    assert_eq!(unknown.len(), 1, "{unknown:?}");
    assert_eq!(unknown[0].0, "amqp.no_such_key");
    let checked = configured(&file, &["--check-config"]).unwrap();
    assert_eq!(checked.check_config_valid(), Some(false));
    let _ = std::fs::remove_file(file);
}

#[test]
fn the_file_the_config_flag_names_must_exist() {
    let missing = std::env::temp_dir().join(format!(
        "zerodds-amqp-contract-dcps-{}-missing.yaml",
        std::process::id()
    ));
    assert!(matches!(
        configured(&missing, &[]),
        Err(ConfigError::Io(..))
    ));
}

#[test]
fn the_check_config_report_hides_the_sasl_password() {
    let file = daemon_file("secret", "amqp:\n  sasl:\n    password: pw-7e19d4\n");
    let configured = configured(&file, &[]).unwrap();
    // `Ok` holds the report of a valid configuration, `Err` the one with
    // its findings; either is printed.
    let (Ok(report) | Err(report)) = configured.source.check_report(&is_key);
    assert!(!report.contains("pw-7e19d4"), "{report}");
    assert!(report.contains("amqp.sasl.password = ***"), "{report}");
    let _ = std::fs::remove_file(file);
}

#[test]
fn the_usage_text_names_the_configuration_flags() {
    for flag in ["--config", "--set", "--check-config"] {
        assert!(HELP.contains(flag), "{flag}");
    }
}

#[test]
fn rtps_9_3_1_5_the_bridge_prefix_starts_with_the_vendor_id() {
    let prefix = bridge_guid_prefix("zerodds-amqp-bridge");
    assert_eq!(&prefix.to_bytes()[..2], &VendorId::ZERODDS.0[..]);
    // Octets 2..6: the host id every participant of this stack carries.
    assert_eq!(prefix.host_id(), host_id_bytes());
}

#[test]
fn the_bridge_prefix_is_stable_per_container_id_and_differs_between_them() {
    // The bridge recognises its own samples by these twelve octets, so the
    // same container id has to give them again.
    assert_eq!(
        bridge_guid_prefix("zerodds-amqp-bridge").to_bytes(),
        bridge_guid_prefix("zerodds-amqp-bridge").to_bytes()
    );
    assert_ne!(
        bridge_guid_prefix("zerodds-bridge-a"),
        bridge_guid_prefix("zerodds-bridge-b")
    );
}

#[test]
fn rtps_9_3_2_a_source_timestamp_counts_the_fraction_in_two_to_the_minus_32_seconds() {
    // time = seconds + fraction / 2^32.
    let t = he_timestamp_to_time(HeTimestamp {
        seconds: 1_700_000_000,
        fraction: 0x8000_0000,
    });
    assert_eq!((t.sec, t.nanosec), (1_700_000_000, 500_000_000));
    let t = he_timestamp_to_time(HeTimestamp {
        seconds: 5,
        fraction: 0x4000_0000,
    });
    assert_eq!((t.sec, t.nanosec), (5, 250_000_000));
    // Rounded to the nearest nanosecond: 3 * 2^-32 s is 0.70 ns.
    let t = he_timestamp_to_time(HeTimestamp {
        seconds: 0,
        fraction: 3,
    });
    assert_eq!((t.sec, t.nanosec), (0, 1));
    // A fraction that rounds to a whole second carries; the bridge's
    // `sec * 1e9 + nanosec` needs `nanosec` below one second.
    let t = he_timestamp_to_time(HeTimestamp {
        seconds: 9,
        fraction: u32::MAX,
    });
    assert_eq!((t.sec, t.nanosec), (10, 0));
}

#[test]
fn rtps_9_3_2_the_reserved_time_t_values_are_no_point_in_time() {
    assert_eq!(
        he_timestamp_to_time(HeTimestamp::TIME_INVALID),
        Time::INVALID
    );
    assert_eq!(
        he_timestamp_to_time(HeTimestamp::TIME_INFINITE),
        Time::INFINITE
    );
}

#[test]
fn xtypes_7_6_8_a_complete_type_without_a_key_member_is_not_keyed() {
    let TypeObject::Complete(complete) = TypeObject::from_bytes_le(&unhex(PLAIN_COMPLETE)).unwrap()
    else {
        panic!("EK_COMPLETE");
    };
    let support =
        SerializedTypeSupport::from_complete_type_objects("to::Plain", complete, Vec::new())
            .unwrap();
    // A keyed topic's `type_file` with this type is refused by the bridge.
    assert!(!support.is_keyed());
    assert_eq!(support.get_type_name(), "to::Plain");
}

#[test]
fn dcps_2_2_3_the_writer_policies_the_bridge_leaves_out_are_the_dds_defaults() {
    let w = UserWriterConfig::default();
    assert!(w.deadline.period.is_infinite(), "§2.2.3.7");
    assert!(w.latency_budget.duration.is_zero(), "§2.2.3.8");
    assert_eq!(w.ownership, OwnershipKind::Shared, "§2.2.3.9");
    assert_eq!(w.ownership_strength, 0, "§2.2.3.10");
    assert_eq!(w.liveliness.kind, LivelinessKind::Automatic, "§2.2.3.11");
    assert!(w.liveliness.lease_duration.is_infinite(), "§2.2.3.11");
    assert!(w.partition.is_empty(), "§2.2.3.13: the default partition");
    assert_eq!(w.transport_priority, 0, "§2.2.3.15");
    assert!(w.lifespan.duration.is_infinite(), "§2.2.3.16");
    assert_eq!(
        w.destination_order.kind,
        DestinationOrderKind::ByReceptionTimestamp,
        "§2.2.3.17"
    );
    assert_eq!(
        w.presentation.access_scope,
        PresentationAccessScope::Instance,
        "§2.2.3.6"
    );
    assert!(!w.presentation.coherent_access && !w.presentation.ordered_access);
    assert!(w.user_data.is_empty() && w.topic_data.is_empty() && w.group_data.is_empty());
    // No offer of its own: the writer takes the runtime's until the bridge
    // overrides it per body.
    assert!(w.data_representation_offer.is_none());
}

#[test]
fn dcps_2_2_3_the_reader_policies_the_bridge_leaves_out_are_the_dds_defaults() {
    let r = UserReaderConfig::default();
    assert!(r.deadline.period.is_infinite(), "§2.2.3.7");
    assert!(r.latency_budget.duration.is_zero(), "§2.2.3.8");
    assert_eq!(r.ownership, OwnershipKind::Shared, "§2.2.3.9");
    assert_eq!(r.liveliness.kind, LivelinessKind::Automatic, "§2.2.3.11");
    assert!(r.liveliness.lease_duration.is_infinite(), "§2.2.3.11");
    assert!(
        r.time_based_filter.minimum_separation.is_zero(),
        "§2.2.3.12"
    );
    assert!(r.partition.is_empty(), "§2.2.3.13");
    assert_eq!(
        r.destination_order.kind,
        DestinationOrderKind::ByReceptionTimestamp,
        "§2.2.3.17"
    );
    assert_eq!(
        r.presentation.access_scope,
        PresentationAccessScope::Instance,
        "§2.2.3.6"
    );
    assert!(!r.presentation.coherent_access && !r.presentation.ordered_access);
    // §2.2.3.19: LENGTH_UNLIMITED (-1, DDS 1.4 §2.3.3).
    assert_eq!(r.resource_limits.max_samples, -1);
    assert_eq!(r.resource_limits.max_instances, -1);
    assert_eq!(r.resource_limits.max_samples_per_instance, -1);
    assert!(r.user_data.is_empty() && r.topic_data.is_empty() && r.group_data.is_empty());
    assert!(r.data_representation_offer.is_none());
    assert!(r.content_filter.is_none());
    // Not a relay reader (DDS-Security 1.2 §7.1.1.4): its payloads arrive
    // opened, with the encapsulation header read into `representation` and
    // `big_endian`.
    assert!(!r.relay);
}
