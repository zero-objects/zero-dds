// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-coap-bridge` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-dcps`, all behind its `daemon` feature:
//! - `src/bin/zerodds-coap-bridged.rs`: `cli::Configured::from_command_line`
//!   with the flags `zerodds_config::CommandLine::extract` took out and the
//!   daemon's convenience flags pushed as `--set` entries, the base name
//!   `config::CONFIG_STEM` and the key predicate `config::is_key`;
//!   `configured.source.tree()` for `DaemonConfig::from_config`,
//!   `checking`, `check_config` and `configured.runtime`; `cli::HELP` in the
//!   usage text; `cli::notify_service_manager("READY=1")` and
//!   `("STOPPING=1")`.
//! - `src/daemon/server.rs`: `runtime::RuntimeConfig::from_standard_paths`
//!   (`start`), `participant::bridge_guid_prefix` of the CoAP bind address,
//!   `DcpsRuntime::start` and `shutdown`, per topic `register_user_reader`
//!   and `register_user_writer` with a `UserReaderConfig` /
//!   `UserWriterConfig` that sets topic, type, `reliable` and `durability`
//!   over `Default`, `write_user_sample` of a CoAP payload, and the
//!   `UserSample` of a reader channel (`Alive`: its payload goes to the
//!   observers; `Lifecycle` and `CoherentSetEnd` are dropped).
//!
//! What it relies on and is checked here without a socket: the layers of
//! the daemon's own file and keys, the runtime keys in that file, the rank
//! of a `--set`, the refusal of an unknown `--set` path and of an invalid
//! runtime value, the warning on an unknown file key, `--check-config` and
//! its report without the secrets; the GUID prefix of the bridge; the
//! policies the bridge leaves at `Default`.
//!
//! Oracles: `docs/specs/zerodds-deployment-1.0.md` §2.1 ("Runtime
//! configuration from RC7 on": the four layers, `--config PATH` must exist,
//! a `--set` no setting reads is rejected, unknown file keys warn, the exit
//! codes of `--check-config`, a bridge daemon's own file carries the runtime
//! keys), the rustdoc of `zerodds_config::is_secret_path` and
//! `zerodds_config::Origin`, DDS-XTypes 1.3 §7.6.3.1.1
//! (`XCDR_DATA_REPRESENTATION = 0`, `XCDR2_DATA_REPRESENTATION = 2`, the
//! default `[XCDR]`), DDSI-RTPS 2.5 §9.3.1.5 (the first two octets of a GUID
//! prefix are the vendor id), the rustdoc of `bridge_guid_prefix` and of
//! `UserWriterConfig` / `UserReaderConfig` (every member is the DDS default of
//! its policy), DDS 1.4 §2.2.3 (the defaults).
//!
//! Left to the integration pillar, because only a bound runtime shows it: that
//! `write_user_sample` frames the payload under the encapsulation header of
//! the runtime's offer, what a reader channel delivers, and
//! `notify_service_manager`, which sends a datagram to `$NOTIFY_SOCKET`.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use zerodds_coap_bridge::daemon::config::{CONFIG_STEM, is_key};
use zerodds_config::{CommandLine, ConfigError, Origin};
use zerodds_dcps::cli::{Configured, HELP};
use zerodds_dcps::participant::{bridge_guid_prefix, host_id_bytes};
use zerodds_dcps::runtime::{UserReaderConfig, UserWriterConfig};
use zerodds_qos::{DestinationOrderKind, LivelinessKind, OwnershipKind, PresentationAccessScope};
use zerodds_rtps::wire_types::VendorId;

/// DDS-XTypes 1.3 §7.6.3.1.1: `XCDR_DATA_REPRESENTATION`.
const XCDR: i16 = 0;
/// DDS-XTypes 1.3 §7.6.3.1.1: `XCDR2_DATA_REPRESENTATION`.
const XCDR2: i16 = 2;

/// A daemon file of this test alone, so that no system file takes part.
fn daemon_file(name: &str, text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "zerodds-coap-contract-dcps-{}-{name}.yaml",
        std::process::id()
    ));
    std::fs::write(&path, text).unwrap();
    path
}

/// What the daemon's `main` does with its arguments: the configuration
/// flags out, then the layers of its own file and keys.
fn configured(file: &Path, flags: &[&str]) -> Result<Configured, ConfigError> {
    let mut args = vec![String::from("--config"), file.display().to_string()];
    args.extend(flags.iter().map(|flag| (*flag).to_string()));
    let (command_line, rest) = CommandLine::extract(args)?;
    assert!(rest.is_empty(), "{rest:?}");
    Configured::from_command_line(command_line, Vec::new(), CONFIG_STEM, is_key, &[])
}

#[test]
fn the_daemon_file_holds_its_own_keys_and_the_runtime_keys() {
    let file = daemon_file(
        "keys",
        "domain: 7\ncoap:\n  max_retransmit: 4\nwire:\n  data_representation_offer: XCDR2\n",
    );
    let configured = configured(&file, &[]).unwrap();
    let tree = configured.source.tree().unwrap();
    assert_eq!(tree.get_str("domain"), Some("7"));
    assert_eq!(tree.get_str("coap.max_retransmit"), Some("4"));
    // The runtime key of the daemon's file reaches the DDS side.
    assert_eq!(configured.runtime.data_representation_offer, [XCDR2]);
    assert!(!configured.checking());
    assert!(configured.check_config_valid().is_none());
    let _ = std::fs::remove_file(file);
}

#[test]
fn a_convenience_flag_pushed_as_set_ranks_above_the_daemon_file() {
    let file = daemon_file("precedence", "coap:\n  max_retransmit: 4\n");
    let (mut command_line, _) =
        CommandLine::extract([String::from("--config"), file.display().to_string()]).unwrap();
    command_line.push_set("coap.max_retransmit=9").unwrap();
    let configured =
        Configured::from_command_line(command_line, Vec::new(), CONFIG_STEM, is_key, &[]).unwrap();
    let tree = configured.source.tree().unwrap();
    assert_eq!(tree.get_str("coap.max_retransmit"), Some("9"));
    assert_eq!(
        tree.origin_of("coap.max_retransmit"),
        Some(&Origin::Cli(String::from("--set coap.max_retransmit=9")))
    );
    let _ = std::fs::remove_file(file);
}

#[test]
fn a_set_flag_no_setting_reads_is_refused() {
    let file = daemon_file("unknown-set", "domain: 0\n");
    assert!(matches!(
        configured(&file, &["--set", "coap.no_such_key=1"]),
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
    let file = daemon_file("unknown-key", "coap:\n  no_such_key: 1\n");
    let started = configured(&file, &[]).unwrap();
    let unknown = started.source.unknown_keys(&is_key);
    assert_eq!(unknown.len(), 1, "{unknown:?}");
    assert_eq!(unknown[0].0, "coap.no_such_key");
    let checked = configured(&file, &["--check-config"]).unwrap();
    assert_eq!(checked.check_config_valid(), Some(false));
    let _ = std::fs::remove_file(file);
}

#[test]
fn the_file_the_config_flag_names_must_exist() {
    let missing = std::env::temp_dir().join(format!(
        "zerodds-coap-contract-dcps-{}-missing.yaml",
        std::process::id()
    ));
    assert!(matches!(
        configured(&missing, &[]),
        Err(ConfigError::Io(..))
    ));
}

#[test]
fn the_check_config_report_hides_the_secrets() {
    let file = daemon_file(
        "secret",
        "auth:\n  bearer_token: tok-4d2f81\noscore:\n  master_secret_hex: 0a0b0c0d\n",
    );
    let configured = configured(&file, &[]).unwrap();
    // `Ok` holds the report of a valid configuration, `Err` the one with
    // its findings; either is printed.
    let (Ok(report) | Err(report)) = configured.source.check_report(&is_key);
    assert!(!report.contains("tok-4d2f81"), "{report}");
    assert!(!report.contains("0a0b0c0d"), "{report}");
    assert!(report.contains("auth.bearer_token = ***"), "{report}");
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
    let prefix = bridge_guid_prefix("0.0.0.0:5683");
    assert_eq!(&prefix.to_bytes()[..2], &VendorId::ZERODDS.0[..]);
    // Octets 2..6: the host id every participant of this stack carries.
    assert_eq!(prefix.host_id(), host_id_bytes());
}

#[test]
fn the_bridge_prefix_is_stable_per_bind_address_and_differs_between_them() {
    assert_eq!(
        bridge_guid_prefix("0.0.0.0:5683"),
        bridge_guid_prefix("0.0.0.0:5683")
    );
    assert_ne!(
        bridge_guid_prefix("127.0.0.1:5683"),
        bridge_guid_prefix("127.0.0.1:5684")
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
    // No offer of its own: the writer takes the runtime's.
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
    // opened, with an encapsulation header the runtime read.
    assert!(!r.relay);
}
