// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-corba-dds-bridge` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-dcps`, all behind its `std` feature:
//! - `src/bin/zerodds-corba-bridged.rs`: `cli::Configured::from_command_line`
//!   twice (start and SIGHUP reload) with the flags
//!   `zerodds_config::CommandLine::extract` took out and the daemon's
//!   convenience flags pushed as `--set` entries, the base name
//!   `corba-bridged` (a constant of the binary), the key predicate
//!   `bridge_daemon::is_key` (its `KEYS` and every entry of the map
//!   `corba.ior.initial_references`) and the daemon's own environment name
//!   `bridge_daemon::ENV` (`ZERODDS_CORBA_ADVERTISE_HOST` for
//!   `corba.iiop.advertise_host`); `configured.source.tree()` for
//!   `DaemonConfig::from_config`, `checking`, `check_config` and
//!   `configured.runtime`.
//! - `src/bridge_daemon.rs`: `runtime::{DcpsRuntime, RuntimeConfig,
//!   UserReaderConfig, UserSample, UserWriterConfig}`;
//!   `participant::bridge_guid_prefix` of `"<orb_id>@<iiop_bind>"`,
//!   `DcpsRuntime::start`, per mapping `register_user_writer_kind` (request
//!   topic) and `register_user_reader_kind` (reply and notify topics) with a
//!   `UserWriterConfig` / `UserReaderConfig` that sets topic, type,
//!   `reliable` and `durability` over `Default`; per request
//!   `set_user_writer_data_rep_override(writer, Some(vec![XCDR]))`,
//!   `set_user_writer_byte_order_override` with the GIOP byte order and
//!   `write_user_sample_borrowed` of the GIOP body; the `UserSample::Alive`
//!   of a reply or notify channel with `payload`, `big_endian` and
//!   `representation` (a notify sample with `representation != 0` is refused
//!   as XCDR2); `cli::notify_service_manager("READY=1")` and
//!   `("STOPPING=1")`.
//!
//! What it relies on and is checked here without a socket: the layers of
//! the daemon's own file, keys and environment name, the runtime keys in that
//! file, the rank of a `--set`, the refusal of an unknown `--set` path and of
//! an invalid runtime value, the warning on an unknown file key, the map
//! `corba.ior.initial_references` passing the unknown-key check,
//! `--check-config`; the GUID prefix of the bridge; the policies the bridge
//! leaves at `Default`.
//!
//! Oracles: `docs/specs/zerodds-deployment-1.0.md` §2.1 ("Runtime
//! configuration from RC7 on": the four layers, the environment beneath the
//! files, `--config PATH` must exist, a `--set` no setting reads is rejected,
//! unknown file keys warn, the exit codes of `--check-config`, a bridge
//! daemon's own file carries the runtime keys), the rustdoc of
//! `ConfigSource::for_program` (`app_env` beneath the files) and of
//! `zerodds_config::Origin`, DDS-XTypes 1.3 §7.6.3.1.1
//! (`XCDR_DATA_REPRESENTATION = 0`, `XCDR2_DATA_REPRESENTATION = 2`, the
//! default `[XCDR]`), DDSI-RTPS 2.5 §9.3.1.5 (the first two octets of a GUID
//! prefix are the vendor id), the rustdoc of `bridge_guid_prefix` and of
//! `UserWriterConfig` / `UserReaderConfig` (every member is the DDS default of
//! its policy), DDS 1.4 §2.2.3 (the defaults).
//!
//! Left to the integration pillar, because only a bound runtime shows it: that
//! the request writer frames the GIOP body as `CDR_BE` / `CDR_LE` after the
//! two overrides (DDSI-RTPS 2.5 §10.5), what a reply or notify channel
//! delivers (`representation` 0 for XCDR1, `big_endian` from the
//! representation identifier), and `notify_service_manager`, which sends a
//! datagram to `$NOTIFY_SOCKET`.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use zerodds_config::{CommandLine, ConfigError, Origin};
use zerodds_corba_dds_bridge::bridge_daemon::{ENV, is_key};
use zerodds_dcps::cli::{Configured, HELP};
use zerodds_dcps::participant::{bridge_guid_prefix, host_id_bytes};
use zerodds_dcps::runtime::{ConfigSource, UserReaderConfig, UserWriterConfig};
use zerodds_qos::{DestinationOrderKind, LivelinessKind, OwnershipKind, PresentationAccessScope};
use zerodds_rtps::wire_types::VendorId;

/// The base name of the daemon's file (`src/bin/zerodds-corba-bridged.rs`).
const CONFIG_STEM: &str = "corba-bridged";
/// DDS-XTypes 1.3 §7.6.3.1.1: `XCDR_DATA_REPRESENTATION`.
const XCDR: i16 = 0;
/// DDS-XTypes 1.3 §7.6.3.1.1: `XCDR2_DATA_REPRESENTATION`.
const XCDR2: i16 = 2;

/// A daemon file of this test alone, so that no system file takes part.
fn daemon_file(name: &str, text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "zerodds-corba-contract-dcps-{}-{name}.yaml",
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
        "domain: 7\ncorba:\n  orb_id: orb-a\nwire:\n  data_representation_offer: XCDR2\n",
    );
    let configured = configured(&file, &[]).unwrap();
    let tree = configured.source.tree().unwrap();
    assert_eq!(tree.get_str("domain"), Some("7"));
    assert_eq!(tree.get_str("corba.orb_id"), Some("orb-a"));
    // The runtime key of the daemon's file reaches the DDS side.
    assert_eq!(configured.runtime.data_representation_offer, [XCDR2]);
    assert!(!configured.checking());
    assert!(configured.check_config_valid().is_none());
    let _ = std::fs::remove_file(file);
}

#[test]
fn a_convenience_flag_pushed_as_set_ranks_above_the_daemon_file() {
    let file = daemon_file("precedence", "corba:\n  ssliop:\n    enabled: false\n");
    let (mut command_line, _) =
        CommandLine::extract([String::from("--config"), file.display().to_string()]).unwrap();
    // `--tls-cert`/`--tls-key` become these entries.
    command_line.push_set("corba.ssliop.enabled=true").unwrap();
    let configured =
        Configured::from_command_line(command_line, Vec::new(), CONFIG_STEM, is_key, ENV).unwrap();
    let tree = configured.source.tree().unwrap();
    assert_eq!(tree.get_str("corba.ssliop.enabled"), Some("true"));
    assert_eq!(
        tree.origin_of("corba.ssliop.enabled"),
        Some(&Origin::Cli(String::from(
            "--set corba.ssliop.enabled=true"
        )))
    );
    let _ = std::fs::remove_file(file);
}

#[test]
fn the_daemon_environment_name_sits_beneath_the_daemon_file() {
    // `Configured::from_command_line` reads the process environment through
    // `ConfigSource::for_program`; here the environment is given.
    let file = daemon_file("env", "domain: 0\n");
    let command_line = CommandLine::default().with_config(file.clone());
    let env = |name: &str| {
        (name == "ZERODDS_CORBA_ADVERTISE_HOST").then(|| String::from("host-from-env"))
    };
    let source = ConfigSource::for_program(&command_line, CONFIG_STEM, &is_key, ENV, env).unwrap();
    let tree = source.tree().unwrap();
    assert_eq!(
        tree.get_str("corba.iiop.advertise_host"),
        Some("host-from-env")
    );
    assert_eq!(
        tree.origin_of("corba.iiop.advertise_host"),
        Some(&Origin::Env(String::from("ZERODDS_CORBA_ADVERTISE_HOST")))
    );
    std::fs::write(
        &file,
        "corba:\n  iiop:\n    advertise_host: host-from-file\n",
    )
    .unwrap();
    let source = ConfigSource::for_program(&command_line, CONFIG_STEM, &is_key, ENV, env).unwrap();
    assert_eq!(
        source.tree().unwrap().get_str("corba.iiop.advertise_host"),
        Some("host-from-file")
    );
    let _ = std::fs::remove_file(file);
}

#[test]
fn a_set_flag_no_setting_reads_is_refused() {
    let file = daemon_file("unknown-set", "domain: 0\n");
    assert!(matches!(
        configured(&file, &["--set", "corba.no_such_key=1"]),
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
    let file = daemon_file("unknown-key", "corba:\n  no_such_key: 1\n");
    let started = configured(&file, &[]).unwrap();
    let unknown = started.source.unknown_keys(&is_key);
    assert_eq!(unknown.len(), 1, "{unknown:?}");
    assert_eq!(unknown[0].0, "corba.no_such_key");
    let checked = configured(&file, &["--check-config"]).unwrap();
    assert_eq!(checked.check_config_valid(), Some(false));
    let _ = std::fs::remove_file(file);
}

#[test]
fn the_initial_references_map_passes_the_unknown_key_check() {
    // The entries of a map with operator-chosen keys come back as dotted
    // paths (`corba.ior.initial_references.NameService`), which `is_key`
    // accepts by their prefix.
    let file = daemon_file(
        "initial-references",
        "corba:\n  ior:\n    initial_references:\n      NameService: ns-ior\n      TradingService: ts-ior\n",
    );
    let configured = configured(&file, &[]).unwrap();
    assert!(configured.source.unknown_keys(&is_key).is_empty());
    assert!(configured.check_config_valid().is_none());
    let _ = std::fs::remove_file(file);
}

#[test]
fn the_file_the_config_flag_names_must_exist() {
    let missing = std::env::temp_dir().join(format!(
        "zerodds-corba-contract-dcps-{}-missing.yaml",
        std::process::id()
    ));
    assert!(matches!(
        configured(&missing, &[]),
        Err(ConfigError::Io(..))
    ));
}

#[test]
fn the_usage_text_names_the_configuration_flags() {
    for flag in ["--config", "--set", "--check-config"] {
        assert!(HELP.contains(flag), "{flag}");
    }
}

#[test]
fn rtps_9_3_1_5_the_bridge_prefix_starts_with_the_vendor_id() {
    let prefix = bridge_guid_prefix("zerodds@0.0.0.0:2809");
    assert_eq!(&prefix.to_bytes()[..2], &VendorId::ZERODDS.0[..]);
    // Octets 2..6: the host id every participant of this stack carries.
    assert_eq!(prefix.host_id(), host_id_bytes());
}

#[test]
fn the_bridge_prefix_is_stable_per_orb_and_differs_between_them() {
    assert_eq!(
        bridge_guid_prefix("zerodds@0.0.0.0:2809"),
        bridge_guid_prefix("zerodds@0.0.0.0:2809")
    );
    assert_ne!(
        bridge_guid_prefix("zerodds@0.0.0.0:2809"),
        bridge_guid_prefix("zerodds@0.0.0.0:2810")
    );
    assert_ne!(
        bridge_guid_prefix("orb-a@0.0.0.0:2809"),
        bridge_guid_prefix("orb-b@0.0.0.0:2809")
    );
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
    // No offer of its own; the request writer sets `[XCDR]` per request.
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
