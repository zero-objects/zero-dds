// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-zenoh-bridge` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-dcps`, behind its `zenoh-runtime` feature
//! (the runtime) and its `daemon` feature (the daemon and its configuration):
//! - `src/bin/zerodds-zenoh-bridged.rs`: `cli::Configured::from_command_line`
//!   with the flags `zerodds_config::CommandLine::extract` took out and the
//!   daemon's convenience flags pushed as `--set` entries, the base name
//!   `config::CONFIG_STEM` and the key predicate `config::is_key`;
//!   `configured.source.tree()` for `DaemonConfig::from_config`,
//!   `checking`, `check_config` and `configured.runtime`; `cli::HELP` in the
//!   usage text; `cli::notify_service_manager("READY=1")` and
//!   `("STOPPING=1")`.
//! - `src/daemon/server.rs`: `participant::bridge_guid_prefix` of
//!   `"zenoh-bridge-<domain>-<pid>"`, `DcpsRuntime::start`; for a configured
//!   route `serialized_type::SerializedTypeSupport::from_type_object` with
//!   the route's `keyed` and extensibility code (0 final, 1 appendable, 2
//!   mutable); for a discovered topic `guid_prefix`, `discovery_generation`,
//!   `discovered_publication_endpoints` /
//!   `discovered_subscription_endpoints` (`DiscoveredEndpointInfo`), the
//!   `type_lookup_server` registry, `from_complete_type_objects`,
//!   `is_keyed` and `extensibility` (`dds_type::Extensibility` matched as
//!   `Final`, `Appendable`, `Mutable`).
//! - `src/runtime.rs`: `runtime::{DcpsRuntime, UserReaderConfig, UserSample,
//!   UserWriterConfig}`; `register_user_reader_kind` /
//!   `register_user_writer_kind` with a config that sets topic, type,
//!   `reliable`, `durability` and `partition` over `Default`;
//!   `set_user_writer_wire_extensibility`, `set_user_writer_key_support`,
//!   `set_user_writer_history_depth`; the `UserSample::Alive` fields
//!   `payload`, `representation` (`!= 0` is XCDR2), `big_endian`, `key_hash`
//!   with `remember_user_writer_instance`, `UserSample::Lifecycle`
//!   (`key_hash`), `CoherentSetEnd`; on the way into DDS
//!   `set_user_writer_data_rep_override` (`[0]` or `[2]`),
//!   `set_user_writer_byte_order_override`, `read_body(body, 0 | 2,
//!   big_endian)` with `key_material` for `write_user_sample_keyed`, else
//!   `write_user_sample_borrowed`; `write_user_lifecycle`.
//!
//! What it relies on and is checked here without a socket: the layers of
//! the daemon's own file and keys, the runtime keys in that file, the rank
//! of a `--set`, the refusal of an unknown `--set` path and of an invalid
//! runtime value, the warning on an unknown file key, `--check-config`; the
//! GUID prefix of the bridge; the policies the bridge leaves at `Default`;
//! the extensibility code of `from_type_object` and the flags
//! `from_complete_type_objects` reads off a type; `read_body` in the
//! representations 0 and 2 and both byte orders, its refusal of the sample
//! tag 1, and the key hash of a keyed sample.
//!
//! Oracles: `docs/specs/zerodds-deployment-1.0.md` §2.1 ("Runtime
//! configuration from RC7 on"), the rustdoc of `zerodds_config::Origin`,
//! DDS-XTypes 1.3 §7.6.3.1.1 (`XCDR_DATA_REPRESENTATION = 0`,
//! `XCDR2_DATA_REPRESENTATION = 2`, the default `[XCDR]`), §7.2.2.4.4 (the
//! three extensibility kinds), §7.4 (XCDR1 aligns an 8-octet member to 8,
//! XCDR2 to 4) and §7.6.8 (the key holder is the key members in big-endian
//! PLAIN_CDR2; one of at most 16 octets, zero padded, is the key hash),
//! DDSI-RTPS 2.5 §9.3.1.5 (the first two octets of a GUID prefix are the
//! vendor id), the rustdoc of `bridge_guid_prefix`, `from_type_objects`
//! (flags that disagree with the object are refused),
//! `read_body_representation` and of `UserWriterConfig` / `UserReaderConfig`
//! (every member is the DDS default of its policy), DDS 1.4 §2.2.3 (the
//! defaults), and the octets of `tests/interop-corpus/expected-bytes.tsv`
//! that the `specification` rows name (cases `evolution-final-tail` and
//! `annotation-key-single`, sample `values`).
//!
//! Left to the integration pillar, because only a bound runtime shows it: how
//! `representation` and `big_endian` of a delivered `UserSample::Alive`
//! follow the encapsulation header of the sample (DDSI-RTPS 2.5 §10.5), the
//! header a writer puts in front of a body after the overrides and
//! `set_user_writer_wire_extensibility`, discovery, the TypeLookup registry,
//! and `notify_service_manager`, which sends a datagram to `$NOTIFY_SOCKET`.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use zerodds_config::{CommandLine, ConfigError, Origin};
use zerodds_dcps::cli::{Configured, HELP};
use zerodds_dcps::dds_type::Extensibility;
use zerodds_dcps::participant::{bridge_guid_prefix, host_id_bytes};
use zerodds_dcps::runtime::{UserReaderConfig, UserWriterConfig};
use zerodds_dcps::serialized_type::{SerializedTypeSupport, read_body_representation};
use zerodds_qos::{DestinationOrderKind, LivelinessKind, OwnershipKind, PresentationAccessScope};
use zerodds_rtps::wire_types::VendorId;
use zerodds_types::CompleteTypeObject;
use zerodds_types::dynamic::{DynamicData, DynamicType};
use zerodds_types::type_object::TypeObject;
use zerodds_zenoh_bridge::daemon::config::{CONFIG_STEM, is_key};

/// DDS-XTypes 1.3 §7.6.3.1.1: `XCDR_DATA_REPRESENTATION`.
const XCDR: i16 = 0;
/// DDS-XTypes 1.3 §7.6.3.1.1: `XCDR2_DATA_REPRESENTATION`.
const XCDR2: i16 = 2;

/// Interop corpus `evolution-final-tail`: `a = 7, b = 1.5, c = -3`.
const EVO_IDL: &str = "@final struct Evo { long a; double b; short c; };";
const EVO_XCDR1_BE: &str = "00000007000000003ff8000000000000fffd";
const EVO_XCDR1_LE: &str = "0700000000000000000000000000f83ffdff";
const EVO_XCDR2_BE: &str = "000000073ff8000000000000fffd";
const EVO_XCDR2_LE: &str = "07000000000000000000f83ffdff";

/// Interop corpus `annotation-key-single`: `id = -1234567,
/// payload = 2147483647`; the same octets in XCDR1 and XCDR2.
const KEYED_IDL: &str = "@final struct Sample { @key long id; long payload; };";
const KEYED_BE: &str = "ffed29797fffffff";
const KEYED_LE: &str = "7929edffffffff7f";

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn complete(idl: &str, name: &str) -> CompleteTypeObject {
    let ast = Arc::new(zerodds_wire_probe::encode::model_from_idl(idl, "contract.idl").unwrap());
    let type_ = DynamicType::find(&ast, name).unwrap();
    zerodds_types::from_ast::complete_type_object(&ast, type_.id()).unwrap()
}

/// The octets of a route's `type_file`.
fn type_file(idl: &str, name: &str) -> Vec<u8> {
    TypeObject::Complete(complete(idl, name))
        .to_bytes_le()
        .unwrap()
}

fn member<'a>(data: &'a DynamicData, name: &str) -> &'a DynamicData {
    let DynamicData::Aggregate(members) = data else {
        panic!("{data:?}");
    };
    &members.iter().find(|entry| entry.0 == name).unwrap().1
}

fn integer(data: &DynamicData, name: &str) -> i128 {
    match member(data, name) {
        DynamicData::Integer { value, .. } => *value,
        other => panic!("{name}: {other:?}"),
    }
}

/// A daemon file of this test alone, so that no system file takes part.
fn daemon_file(name: &str, text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "zerodds-zenoh-contract-dcps-{}-{name}.yaml",
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
        "domain: 7\nzenoh:\n  mode: peer\nwire:\n  data_representation_offer: XCDR2\n",
    );
    let configured = configured(&file, &[]).unwrap();
    let tree = configured.source.tree().unwrap();
    assert_eq!(tree.get_str("domain"), Some("7"));
    assert_eq!(tree.get_str("zenoh.mode"), Some("peer"));
    // The runtime key of the daemon's file reaches the DDS side.
    assert_eq!(configured.runtime.data_representation_offer, [XCDR2]);
    assert!(!configured.checking());
    assert!(configured.check_config_valid().is_none());
    let _ = std::fs::remove_file(file);
}

#[test]
fn a_convenience_flag_pushed_as_set_ranks_above_the_daemon_file() {
    let file = daemon_file("precedence", "zenoh:\n  mode: peer\n");
    let (mut command_line, _) =
        CommandLine::extract([String::from("--config"), file.display().to_string()]).unwrap();
    command_line.push_set("zenoh.mode=client").unwrap();
    let configured =
        Configured::from_command_line(command_line, Vec::new(), CONFIG_STEM, is_key, &[]).unwrap();
    let tree = configured.source.tree().unwrap();
    assert_eq!(tree.get_str("zenoh.mode"), Some("client"));
    assert_eq!(
        tree.origin_of("zenoh.mode"),
        Some(&Origin::Cli(String::from("--set zenoh.mode=client")))
    );
    let _ = std::fs::remove_file(file);
}

#[test]
fn a_set_flag_no_setting_reads_is_refused() {
    let file = daemon_file("unknown-set", "domain: 0\n");
    assert!(matches!(
        configured(&file, &["--set", "zenoh.no_such_key=1"]),
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
    let file = daemon_file("unknown-key", "zenoh:\n  no_such_key: 1\n");
    let started = configured(&file, &[]).unwrap();
    let unknown = started.source.unknown_keys(&is_key);
    assert_eq!(unknown.len(), 1, "{unknown:?}");
    assert_eq!(unknown[0].0, "zenoh.no_such_key");
    let checked = configured(&file, &["--check-config"]).unwrap();
    assert_eq!(checked.check_config_valid(), Some(false));
    let _ = std::fs::remove_file(file);
}

#[test]
fn the_file_the_config_flag_names_must_exist() {
    let missing = std::env::temp_dir().join(format!(
        "zerodds-zenoh-contract-dcps-{}-missing.yaml",
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
    let prefix = bridge_guid_prefix("zenoh-bridge-0-4242");
    assert_eq!(&prefix.to_bytes()[..2], &VendorId::ZERODDS.0[..]);
    // Octets 2..6: the host id every participant of this stack carries.
    assert_eq!(prefix.host_id(), host_id_bytes());
}

#[test]
fn the_bridge_prefix_is_stable_per_domain_and_process_and_differs_otherwise() {
    assert_eq!(
        bridge_guid_prefix("zenoh-bridge-0-4242"),
        bridge_guid_prefix("zenoh-bridge-0-4242")
    );
    assert_ne!(
        bridge_guid_prefix("zenoh-bridge-0-4242"),
        bridge_guid_prefix("zenoh-bridge-1-4242")
    );
    assert_ne!(
        bridge_guid_prefix("zenoh-bridge-0-4242"),
        bridge_guid_prefix("zenoh-bridge-0-4243")
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
    // overrides it per sample.
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
    // §2.2.3.19: LENGTH_UNLIMITED (-1, DDS 1.4 §2.3.3).
    assert_eq!(r.resource_limits.max_samples, -1);
    assert!(r.user_data.is_empty() && r.topic_data.is_empty() && r.group_data.is_empty());
    assert!(r.data_representation_offer.is_none());
    assert!(r.content_filter.is_none());
    // Not a relay reader (DDS-Security 1.2 §7.1.1.4): its payloads arrive
    // opened, with the encapsulation header read into `representation` and
    // `big_endian`.
    assert!(!r.relay);
}

#[test]
fn xtypes_7_2_2_4_4_the_extensibility_code_names_final_appendable_and_mutable() {
    for (idl, name, code, kind) in [
        ("@final struct F { long x; };", "F", 0, Extensibility::Final),
        (
            "@appendable struct A { long x; };",
            "A",
            1,
            Extensibility::Appendable,
        ),
        (
            "@mutable struct M { long x; };",
            "M",
            2,
            Extensibility::Mutable,
        ),
    ] {
        let bytes = type_file(idl, name);
        let support = SerializedTypeSupport::from_type_object(name, &bytes, false, code).unwrap();
        assert_eq!(support.extensibility(), kind, "{idl}");
        assert!(!support.is_keyed());
        // A route that names another extensibility, or claims a key, is
        // refused rather than bridged with the wrong framing.
        for other in (0..=3).filter(|other| *other != code) {
            assert!(
                SerializedTypeSupport::from_type_object(name, &bytes, false, other).is_err(),
                "{idl} as {other}"
            );
        }
        assert!(SerializedTypeSupport::from_type_object(name, &bytes, true, code).is_err());
    }
}

#[test]
fn xtypes_7_6_8_a_discovered_type_gives_its_key_and_extensibility() {
    let support = SerializedTypeSupport::from_complete_type_objects(
        "Sample",
        complete(KEYED_IDL, "Sample"),
        Vec::new(),
    )
    .unwrap();
    assert!(support.is_keyed());
    assert_eq!(support.extensibility(), Extensibility::Final);
    let support = SerializedTypeSupport::from_complete_type_objects(
        "Evo",
        complete(EVO_IDL, "Evo"),
        Vec::new(),
    )
    .unwrap();
    assert!(!support.is_keyed());
}

#[test]
fn xtypes_7_4_read_body_takes_the_representations_0_and_2_in_both_byte_orders() {
    let evo = SerializedTypeSupport::from_type_object("Evo", &type_file(EVO_IDL, "Evo"), false, 0)
        .unwrap();
    for (representation, big_endian, octets) in [
        (0, true, EVO_XCDR1_BE),
        (0, false, EVO_XCDR1_LE),
        (2, true, EVO_XCDR2_BE),
        (2, false, EVO_XCDR2_LE),
    ] {
        let sample = evo
            .read_body(&unhex(octets), representation, big_endian)
            .unwrap();
        assert_eq!(integer(sample.data(), "a"), 7, "{octets}");
        assert_eq!(integer(sample.data(), "c"), -3, "{octets}");
    }
    // `UserSample::Alive::representation` tags XCDR2 as 1; `read_body`
    // refuses it, the bridge maps it to 2 as `read_body_representation` does.
    assert!(evo.read_body(&unhex(EVO_XCDR2_LE), 1, false).is_err());
    assert_eq!(read_body_representation(1), 2);
}

#[test]
fn xtypes_7_6_8_a_key_of_at_most_16_octets_is_the_zero_padded_key_hash() {
    let keyed =
        SerializedTypeSupport::from_type_object("Sample", &type_file(KEYED_IDL, "Sample"), true, 0)
            .unwrap();
    for (representation, big_endian, octets) in [
        (2, false, KEYED_LE),
        (2, true, KEYED_BE),
        (0, false, KEYED_LE),
        (0, true, KEYED_BE),
    ] {
        let sample = keyed
            .read_body(&unhex(octets), representation, big_endian)
            .unwrap();
        let (hash, holder) = keyed.key_material(&sample).unwrap();
        assert_eq!(holder, [0xff, 0xed, 0x29, 0x79]);
        assert_eq!(
            hash,
            [0xff, 0xed, 0x29, 0x79, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
    }
}
