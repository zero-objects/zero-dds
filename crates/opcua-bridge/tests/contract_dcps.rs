// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-opcua-bridge` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-dcps`, all behind its `daemon` feature:
//! - `src/bin/zerodds-opcua-bridged.rs`: `cli::Configured::from_command_line`
//!   with the flags `zerodds_config::CommandLine::extract` took out and the
//!   daemon's convenience flags pushed as `--set` entries, the base name
//!   `config::CONFIG_STEM` and the key predicate `config::is_key`;
//!   `configured.source.tree()` for `DaemonConfig::from_config`,
//!   `checking`, `check_config` and `configured.runtime`; `cli::HELP` in the
//!   usage text; `cli::notify_service_manager("READY=1")` and
//!   `("STOPPING=1")`.
//! - `src/dds_bridge.rs`: `participant::bridge_guid_prefix` of
//!   `"<process>-domain-<id>"`, `DcpsRuntime::start`;
//!   `register_user_reader_kind` with a `UserReaderConfig` that sets topic,
//!   type and `reliable` over `Default`; `register_user_writer_kind` with a
//!   `UserWriterConfig` that sets topic, type and `reliable` over `Default`;
//!   `write_user_sample_keyed` with the key hash of
//!   `SerializedTypeSupport::read_body(body, 2, false)` and `key_material`,
//!   else `write_user_sample_borrowed`; the reader pump takes `payload`,
//!   `representation` (through `serialized_type::read_body_representation`),
//!   `big_endian` and `source_timestamp` of `UserSample::Alive`;
//!   `discovered_publication_endpoints` / `discovered_subscription_endpoints`
//!   (`DiscoveredEndpointInfo`); `resolve_discovered` reads the runtime's
//!   `type_lookup_server` registry and builds
//!   `SerializedTypeSupport::from_complete_type_objects`.
//! - `src/idl.rs`: `SerializedTypeSupport::from_type_object` with `keyed`
//!   and the extensibility code (0 final, 1 appendable, 2 mutable) read off
//!   the model.
//! - `src/ua_client.rs`, `src/pubsub_glue.rs`, `src/daemon/server.rs`:
//!   `from_dynamic_data`, `write_body(&sample, 2, false)` for the bodies the
//!   bridge writes to DDS, `read_body(body, representation, big_endian)` for
//!   the bodies it reads; `runtime::RuntimeConfig` (`default` in tests, the
//!   configured one in the daemon).
//!
//! What it relies on and is checked here without a socket: the layers of
//! the daemon's own file and keys, the runtime keys in that file, the rank
//! of a `--set`, the refusal of an unknown `--set` path and of an invalid
//! runtime value, the warning on an unknown file key, `--check-config`; the
//! GUID prefix of the bridge; the policies the bridge leaves at `Default`;
//! that `representation` 0 and 1 of a `UserSample` map to the `read_body`
//! representations 0 and 2 and decode what other vendors write in each form;
//! that a sample built from values is XCDR2 little endian and rewrites into
//! the other three forms as other vendors write them; the key hash of a
//! keyed sample; the extensibility code and `keyed` of a type support.
//!
//! Oracles: `docs/specs/zerodds-deployment-1.0.md` §2.1 ("Runtime
//! configuration from RC7 on"), the rustdoc of `zerodds_config::Origin`,
//! DDS-XTypes 1.3 §7.6.3.1.1 (`XCDR_DATA_REPRESENTATION = 0`,
//! `XCDR2_DATA_REPRESENTATION = 2`, the default `[XCDR]`), §7.4 (XCDR1 aligns
//! an 8-octet member to 8, XCDR2 to 4) and §7.6.8 (the key holder is the key
//! members in big-endian PLAIN_CDR2; one of at most 16 octets, zero padded, is
//! the key hash), DDSI-RTPS 2.5 §9.3.1.5 (the first two octets of a GUID
//! prefix are the vendor id), the rustdoc of `bridge_guid_prefix`,
//! `read_body_representation`, `from_dynamic_data` (XCDR2 little endian),
//! `from_type_objects` (flags that disagree with the object are refused) and
//! of `UserWriterConfig` / `UserReaderConfig` (every member is the DDS default
//! of its policy), DDS 1.4 §2.2.3 (the defaults), and the octets of
//! `tests/interop-corpus/expected-bytes.tsv` that the `specification` rows
//! name (cases `evolution-final-tail` and `annotation-key-single`, sample
//! `values`).
//!
//! Left to the integration pillar, because only a bound runtime shows it: the
//! encapsulation header a written body goes out under (DDSI-RTPS 2.5 §10.5;
//! the writer takes the runtime's offer, `[XCDR]` unless configured), how
//! `representation` and `big_endian` of a delivered `UserSample::Alive`
//! follow the header of the sample, the discovered endpoints, the TypeLookup
//! registry, and `notify_service_manager`, which sends a datagram to
//! `$NOTIFY_SOCKET`.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use zerodds_config::{CommandLine, ConfigError, Origin};
use zerodds_dcps::cli::{Configured, HELP};
use zerodds_dcps::dds_type::Extensibility;
use zerodds_dcps::participant::{bridge_guid_prefix, host_id_bytes};
use zerodds_dcps::runtime::{UserReaderConfig, UserWriterConfig};
use zerodds_dcps::serialized_type::{SerializedTypeSupport, read_body_representation};
use zerodds_opcua_bridge::daemon::config::{CONFIG_STEM, is_key};
use zerodds_qos::{DestinationOrderKind, LivelinessKind, OwnershipKind, PresentationAccessScope};
use zerodds_rtps::wire_types::VendorId;
use zerodds_types::CompleteTypeObject;
use zerodds_types::dynamic::{DynamicData, DynamicType};
use zerodds_types::type_object::TypeObject;

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

fn type_object_bytes(idl: &str, name: &str) -> Vec<u8> {
    TypeObject::Complete(complete(idl, name))
        .to_bytes_le()
        .unwrap()
}

/// What `src/idl.rs` builds: the support with `keyed` and the extensibility
/// code it read off the model.
fn support(idl: &str, name: &str, keyed: bool, extensibility: u8) -> Arc<SerializedTypeSupport> {
    SerializedTypeSupport::from_type_object(
        name,
        &type_object_bytes(idl, name),
        keyed,
        extensibility,
    )
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

fn float_bits(data: &DynamicData, name: &str) -> u64 {
    match member(data, name) {
        DynamicData::Float { value, .. } => value.to_bits(),
        other => panic!("{name}: {other:?}"),
    }
}

/// A daemon file of this test alone, so that no system file takes part.
fn daemon_file(name: &str, text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "zerodds-opcua-contract-dcps-{}-{name}.yaml",
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
        "domain: 7\nlog_level: debug\nwire:\n  data_representation_offer: XCDR2\n",
    );
    let configured = configured(&file, &[]).unwrap();
    let tree = configured.source.tree().unwrap();
    assert_eq!(tree.get_str("domain"), Some("7"));
    assert_eq!(tree.get_str("log_level"), Some("debug"));
    // The runtime key of the daemon's file reaches the DDS side; with it the
    // writers announce the XCDR2 the bridge writes.
    assert_eq!(configured.runtime.data_representation_offer, [XCDR2]);
    assert!(!configured.checking());
    assert!(configured.check_config_valid().is_none());
    let _ = std::fs::remove_file(file);
}

#[test]
fn a_convenience_flag_pushed_as_set_ranks_above_the_daemon_file() {
    let file = daemon_file("precedence", "domain: 3\n");
    let (mut command_line, _) =
        CommandLine::extract([String::from("--config"), file.display().to_string()]).unwrap();
    command_line.push_set("domain=9").unwrap();
    let configured =
        Configured::from_command_line(command_line, Vec::new(), CONFIG_STEM, is_key, &[]).unwrap();
    let tree = configured.source.tree().unwrap();
    assert_eq!(tree.get_str("domain"), Some("9"));
    assert_eq!(
        tree.origin_of("domain"),
        Some(&Origin::Cli(String::from("--set domain=9")))
    );
    let _ = std::fs::remove_file(file);
}

#[test]
fn a_set_flag_no_setting_reads_is_refused() {
    let file = daemon_file("unknown-set", "domain: 0\n");
    assert!(matches!(
        configured(&file, &["--set", "opcua.no_such_key=1"]),
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
    let file = daemon_file("unknown-key", "opcua:\n  no_such_key: 1\n");
    let started = configured(&file, &[]).unwrap();
    let unknown = started.source.unknown_keys(&is_key);
    assert_eq!(unknown.len(), 1, "{unknown:?}");
    assert_eq!(unknown[0].0, "opcua.no_such_key");
    let checked = configured(&file, &["--check-config"]).unwrap();
    assert_eq!(checked.check_config_valid(), Some(false));
    let _ = std::fs::remove_file(file);
}

#[test]
fn the_file_the_config_flag_names_must_exist() {
    let missing = std::env::temp_dir().join(format!(
        "zerodds-opcua-contract-dcps-{}-missing.yaml",
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
    let prefix = bridge_guid_prefix("zerodds-opcua-bridged-domain-0");
    assert_eq!(&prefix.to_bytes()[..2], &VendorId::ZERODDS.0[..]);
    // Octets 2..6: the host id every participant of this stack carries.
    assert_eq!(prefix.host_id(), host_id_bytes());
}

#[test]
fn the_bridge_prefix_is_stable_per_domain_and_differs_between_domains() {
    assert_eq!(
        bridge_guid_prefix("zerodds-opcua-bridged-domain-0"),
        bridge_guid_prefix("zerodds-opcua-bridged-domain-0")
    );
    assert_ne!(
        bridge_guid_prefix("zerodds-opcua-bridged-domain-0"),
        bridge_guid_prefix("zerodds-opcua-bridged-domain-1")
    );
}

#[test]
fn dcps_2_2_3_the_writer_policies_the_bridge_leaves_out_are_the_dds_defaults() {
    let w = UserWriterConfig::default();
    assert_eq!(
        w.durability,
        zerodds_qos::DurabilityKind::Volatile,
        "§2.2.3.4"
    );
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
    assert_eq!(
        r.durability,
        zerodds_qos::DurabilityKind::Volatile,
        "§2.2.3.4"
    );
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
    // §2.2.3.19: LENGTH_UNLIMITED (-1, DDS 1.4 §2.3.3).
    assert_eq!(r.resource_limits.max_samples, -1);
    assert!(r.data_representation_offer.is_none());
    assert!(r.content_filter.is_none());
    // Not a relay reader (DDS-Security 1.2 §7.1.1.4): its payloads arrive
    // opened, with the encapsulation header read into `representation` and
    // `big_endian`.
    assert!(!r.relay);
}

#[test]
fn xtypes_7_4_the_user_sample_tag_names_the_representation_read_body_takes() {
    // `UserSample::Alive::representation` is 0 for XCDR1 and 1 for XCDR2;
    // `read_body` takes 0 and 2.
    assert_eq!(read_body_representation(0), 0);
    assert_eq!(read_body_representation(1), 2);
    let evo = support(EVO_IDL, "Evo", false, 0);
    for (tag, big_endian, octets) in [
        (0, true, EVO_XCDR1_BE),
        (0, false, EVO_XCDR1_LE),
        (1, true, EVO_XCDR2_BE),
        (1, false, EVO_XCDR2_LE),
    ] {
        let sample = evo
            .read_body(&unhex(octets), read_body_representation(tag), big_endian)
            .unwrap();
        assert_eq!(integer(sample.data(), "a"), 7, "{octets}");
        assert_eq!(
            float_bits(sample.data(), "b"),
            1.5_f64.to_bits(),
            "{octets}"
        );
        assert_eq!(integer(sample.data(), "c"), -3, "{octets}");
        assert_eq!(sample.big_endian(), big_endian);
    }
    // The tag passed on unchanged is refused, not read as something else.
    assert!(evo.read_body(&unhex(EVO_XCDR2_LE), 1, false).is_err());
    // XCDR2 octets read as XCDR1 do not decode: XCDR1 puts the double at 8.
    assert!(evo.read_body(&unhex(EVO_XCDR2_LE), 0, false).is_err());
}

#[test]
fn xtypes_7_4_a_sample_built_from_values_is_xcdr2_little_endian_and_rewrites_to_each_form() {
    let evo = support(EVO_IDL, "Evo", false, 0);
    let read = evo.read_body(&unhex(EVO_XCDR1_BE), 0, true).unwrap();
    let built = evo.from_dynamic_data(read.data().clone()).unwrap();
    assert_eq!(built.representation(), 2);
    assert!(!built.big_endian());
    assert_eq!(built.body(), unhex(EVO_XCDR2_LE));
    assert_eq!(
        evo.write_body(&built, 2, false).unwrap(),
        unhex(EVO_XCDR2_LE)
    );
    assert_eq!(
        evo.write_body(&built, 2, true).unwrap(),
        unhex(EVO_XCDR2_BE)
    );
    assert_eq!(
        evo.write_body(&built, 0, false).unwrap(),
        unhex(EVO_XCDR1_LE)
    );
    assert_eq!(
        evo.write_body(&built, 0, true).unwrap(),
        unhex(EVO_XCDR1_BE)
    );
}

#[test]
fn xtypes_7_6_8_a_key_of_at_most_16_octets_is_the_zero_padded_key_hash() {
    let keyed = support(KEYED_IDL, "Sample", true, 0);
    assert!(keyed.is_keyed());
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
        // `id` in big-endian PLAIN_CDR2, then zeros.
        assert_eq!(holder, [0xff, 0xed, 0x29, 0x79]);
        assert_eq!(
            hash,
            [0xff, 0xed, 0x29, 0x79, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(sample.key_hash(), Some(hash));
    }
    // A keyless type has no key material.
    let evo = support(EVO_IDL, "Evo", false, 0);
    let sample = evo.read_body(&unhex(EVO_XCDR2_LE), 2, false).unwrap();
    assert!(evo.key_material(&sample).is_none());
}

#[test]
fn xtypes_7_2_2_a_type_support_takes_its_flags_from_the_type_and_refuses_others() {
    // `resolve_discovered`: read off the COMPLETE object.
    let resolved = SerializedTypeSupport::from_complete_type_objects(
        "Sample",
        complete(KEYED_IDL, "Sample"),
        Vec::new(),
    )
    .unwrap();
    assert!(resolved.is_keyed());
    assert_eq!(resolved.extensibility(), Extensibility::Final);
    // `src/idl.rs`: asserted, 0 final, 1 appendable, 2 mutable.
    let bytes = type_object_bytes(KEYED_IDL, "Sample");
    assert!(SerializedTypeSupport::from_type_object("Sample", &bytes, true, 0).is_ok());
    assert!(SerializedTypeSupport::from_type_object("Sample", &bytes, false, 0).is_err());
    assert!(SerializedTypeSupport::from_type_object("Sample", &bytes, true, 1).is_err());
    let appendable = type_object_bytes("@appendable struct A { long x; };", "A");
    let support = SerializedTypeSupport::from_type_object("A", &appendable, false, 1).unwrap();
    assert_eq!(support.extensibility(), Extensibility::Appendable);
    let mutable = type_object_bytes("@mutable struct M { long x; };", "M");
    let support = SerializedTypeSupport::from_type_object("M", &mutable, false, 2).unwrap();
    assert_eq!(support.extensibility(), Extensibility::Mutable);
}
