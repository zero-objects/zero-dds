// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-xrce-agent` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-dcps`, all behind its `daemon` feature:
//! - `src/bin/zerodds-xrce-agentd.rs`: `cli::Configured::from_command_line`
//!   with the flags `zerodds_config::CommandLine::extract` took out and the
//!   daemon's convenience flags pushed as `--set` entries, the base name
//!   `config::CONFIG_STEM` and the key predicate `config::is_key`;
//!   `configured.source.tree()` for `DaemonConfig::from_config`,
//!   `checking`, `check_config` and `configured.runtime`; `cli::HELP` in the
//!   usage text; `cli::notify_service_manager("READY=1")` and
//!   `("STOPPING=1")`.
//! - `src/daemon/server.rs`: `runtime::RuntimeConfig` as the template of
//!   every runtime the agent starts.
//! - `src/daemon/dcps_bridge.rs`: `participant::bridge_guid_prefix` of
//!   `"zerodds-xrce-agentd-domain-<id>-pid<pid>-rt<n>"` (and of a
//!   participant with USER_DATA), `DcpsRuntime::start` with the template's
//!   `user_data` set; `new_group_entity_id`, `reserve_user_writer_id` /
//!   `reserve_user_reader_id`, `register_user_writer_as` /
//!   `register_user_reader_as` with a `UserWriterConfig` / `UserReaderConfig`
//!   that sets the policies of the XRCE request over `Default`, and
//!   `GroupTopicQos::default()`; `set_user_writer_history_depth`,
//!   `set_user_writer_resource_limits`, `remove_user_writer` /
//!   `remove_user_reader`; `write_user_sample_keyed`,
//!   `write_user_sample_stamped`, `write_user_sample_borrowed` and
//!   `write_user_lifecycle` with the key hash and key holder of
//!   `SerializedTypeSupport::read_body(payload, 2, big_endian)` and
//!   `key_material`; `read_body_prefix(bytes, 2, big_endian)` for samples
//!   packed back to back; a reader pump on `UserSample::Alive` (`key_hash`,
//!   `payload`, `representation`, `big_endian`, `source_timestamp`) and
//!   `UserSample::Lifecycle` (`key_hash`, `kind`, `key_holder`, handed on as
//!   XCDR2 big endian, `source_timestamp`); a content filter that decodes a
//!   buffered sample with `read_body` and
//!   `serialized_type::read_body_representation` and evaluates
//!   `dds_type::DdsTypeRow`; `time::he_timestamp_to_time` for a client's
//!   source time; `SerializedTypeSupport::from_complete_type_objects` and
//!   `is_keyed` for a topic's type.
//!
//! What it relies on and is checked here without a socket: the layers of
//! the daemon's own file and keys, the runtime keys in that file, the rank
//! of a `--set`, the refusal of an unknown `--set` path and of an invalid
//! runtime value, the warning on an unknown file key, `--check-config`; the
//! GUID prefix of an agent runtime; the policies the agent leaves at
//! `Default`; PARTITION, GROUP_DATA and TOPIC_DATA of an endpoint over an
//! empty `GroupTopicQos`; the mapping of the sample tag to the `read_body`
//! representation; a filter over the members of a decoded sample; samples
//! packed back to back; the key hash and key holder; the source time.
//!
//! Oracles: `docs/specs/zerodds-deployment-1.0.md` §2.1 ("Runtime
//! configuration from RC7 on"), the rustdoc of `zerodds_config::Origin`,
//! DDS-XTypes 1.3 §7.6.3.1.1 (`XCDR_DATA_REPRESENTATION = 0`,
//! `XCDR2_DATA_REPRESENTATION = 2`, the default `[XCDR]`), §7.6.3.4.1 (the
//! TypeConsistencyEnforcement defaults), §7.4 (XCDR1 aligns an 8-octet member
//! to 8, XCDR2 to 4), §7.6.7 (a filter names the members of the type) and
//! §7.6.8 (the key holder is the key members in big-endian PLAIN_CDR2; one of
//! at most 16 octets, zero padded, is the key hash), DDSI-RTPS 2.5 §9.3.1.5
//! (the first two octets of a GUID prefix are the vendor id) and §9.3.2
//! (`Time_t`: a fraction in units of 2^-32 s), DDS-XRCE 1.0 Annex A (samples
//! packed back to back), the rustdoc of `bridge_guid_prefix`,
//! `read_body_representation`, `read_body_prefix`, `GroupTopicQos::over`,
//! `he_timestamp_to_time` and of `UserWriterConfig` / `UserReaderConfig`
//! (every member is the DDS default of its policy), DDS 1.4 §2.2.3 (the
//! defaults), and the octets of `tests/interop-corpus/expected-bytes.tsv`
//! that the `specification` rows name (cases `evolution-final-tail` and
//! `annotation-key-single`, sample `values`).
//!
//! Left to the integration pillar, because only a bound runtime shows it:
//! registration and announcement of the reserved endpoints, the framing of
//! written samples, how `representation` and `big_endian` of a delivered
//! `UserSample::Alive` follow the encapsulation header of the sample, the
//! `key_holder` a delivered `UserSample::Lifecycle` carries, and
//! `notify_service_manager`, which sends a datagram to `$NOTIFY_SOCKET`.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use zerodds_config::{CommandLine, ConfigError, Origin};
use zerodds_dcps::cli::{Configured, HELP};
use zerodds_dcps::dds_type::DdsTypeRow;
use zerodds_dcps::participant::{bridge_guid_prefix, host_id_bytes};
use zerodds_dcps::runtime::{GroupTopicQos, UserReaderConfig, UserWriterConfig};
use zerodds_dcps::serialized_type::{SerializedTypeSupport, read_body_representation};
use zerodds_dcps::time::he_timestamp_to_time;
use zerodds_rtps::header_extension::HeTimestamp;
use zerodds_rtps::wire_types::VendorId;
use zerodds_types::dynamic::{DynamicData, DynamicType};
use zerodds_types::qos::TypeConsistencyKind;
use zerodds_types::type_object::TypeObject;
use zerodds_xrce_agent::daemon::config::{CONFIG_STEM, is_key};

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

fn support(idl: &str, name: &str, keyed: bool) -> Arc<SerializedTypeSupport> {
    let ast = Arc::new(zerodds_wire_probe::encode::model_from_idl(idl, "contract.idl").unwrap());
    let type_ = DynamicType::find(&ast, name).unwrap();
    let complete = zerodds_types::from_ast::complete_type_object(&ast, type_.id()).unwrap();
    // The agent reads keyed-ness and extensibility off the COMPLETE object.
    let support =
        SerializedTypeSupport::from_complete_type_objects(name, complete.clone(), Vec::new())
            .unwrap();
    assert_eq!(support.is_keyed(), keyed);
    let bytes = TypeObject::Complete(complete).to_bytes_le().unwrap();
    assert!(SerializedTypeSupport::from_type_object(name, &bytes, keyed, 0).is_ok());
    support
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
        "zerodds-xrce-contract-dcps-{}-{name}.yaml",
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
        "log_level: debug\nsession_timeout_secs: 30\nwire:\n  data_representation_offer: XCDR2\n",
    );
    let configured = configured(&file, &[]).unwrap();
    let tree = configured.source.tree().unwrap();
    assert_eq!(tree.get_str("log_level"), Some("debug"));
    assert_eq!(tree.get_str("session_timeout_secs"), Some("30"));
    // The runtime key of the daemon's file reaches the runtimes the agent
    // starts.
    assert_eq!(configured.runtime.data_representation_offer, [XCDR2]);
    assert!(!configured.checking());
    assert!(configured.check_config_valid().is_none());
    let _ = std::fs::remove_file(file);
}

#[test]
fn a_convenience_flag_pushed_as_set_ranks_above_the_daemon_file() {
    let file = daemon_file("precedence", "session_timeout_secs: 30\n");
    let (mut command_line, _) =
        CommandLine::extract([String::from("--config"), file.display().to_string()]).unwrap();
    command_line.push_set("session_timeout_secs=60").unwrap();
    let configured =
        Configured::from_command_line(command_line, Vec::new(), CONFIG_STEM, is_key, &[]).unwrap();
    let tree = configured.source.tree().unwrap();
    assert_eq!(tree.get_str("session_timeout_secs"), Some("60"));
    assert_eq!(
        tree.origin_of("session_timeout_secs"),
        Some(&Origin::Cli(String::from("--set session_timeout_secs=60")))
    );
    let _ = std::fs::remove_file(file);
}

#[test]
fn a_set_flag_no_setting_reads_is_refused() {
    let file = daemon_file("unknown-set", "log_level: info\n");
    assert!(matches!(
        configured(&file, &["--set", "limits.no_such_key=1"]),
        Err(ConfigError::Usage(_))
    ));
    let _ = std::fs::remove_file(file);
}

#[test]
fn an_invalid_runtime_value_stops_the_start_and_check_config_reports_it() {
    let file = daemon_file(
        "invalid",
        "log_level: info\nwire:\n  data_representation_offer: XCDR3\n",
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
    let file = daemon_file("unknown-key", "limits:\n  no_such_key: 1\n");
    let started = configured(&file, &[]).unwrap();
    let unknown = started.source.unknown_keys(&is_key);
    assert_eq!(unknown.len(), 1, "{unknown:?}");
    assert_eq!(unknown[0].0, "limits.no_such_key");
    let checked = configured(&file, &["--check-config"]).unwrap();
    assert_eq!(checked.check_config_valid(), Some(false));
    let _ = std::fs::remove_file(file);
}

#[test]
fn the_file_the_config_flag_names_must_exist() {
    let missing = std::env::temp_dir().join(format!(
        "zerodds-xrce-contract-dcps-{}-missing.yaml",
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
fn rtps_9_3_1_5_the_agent_prefix_starts_with_the_vendor_id() {
    let prefix = bridge_guid_prefix("zerodds-xrce-agentd-domain-0-pid1-rt0");
    assert_eq!(&prefix.to_bytes()[..2], &VendorId::ZERODDS.0[..]);
    // Octets 2..6: the host id every participant of this stack carries.
    assert_eq!(prefix.host_id(), host_id_bytes());
}

#[test]
fn two_agent_runtimes_of_one_domain_get_two_prefixes() {
    // DDS-XRCE 1.0 §10.2: two agents on one host have to discover each
    // other; the seed carries the process and the runtime counter.
    assert_eq!(
        bridge_guid_prefix("zerodds-xrce-agentd-domain-0-pid1-rt0"),
        bridge_guid_prefix("zerodds-xrce-agentd-domain-0-pid1-rt0")
    );
    assert_ne!(
        bridge_guid_prefix("zerodds-xrce-agentd-domain-0-pid1-rt0"),
        bridge_guid_prefix("zerodds-xrce-agentd-domain-0-pid1-rt1")
    );
    assert_ne!(
        bridge_guid_prefix("zerodds-xrce-agentd-domain-0-pid1-rt0"),
        bridge_guid_prefix("zerodds-xrce-agentd-domain-0-pid2-rt0")
    );
}

#[test]
fn dcps_2_2_3_what_the_agent_leaves_out_of_its_endpoints_is_the_default() {
    let w = UserWriterConfig::default();
    // The writer takes the runtime's offer and builds its own entity name.
    assert!(w.data_representation_offer.is_none());
    assert!(w.entity_name.is_none());
    assert_eq!(w.type_identifier, zerodds_types::TypeIdentifier::None);
    let r = UserReaderConfig::default();
    assert!(
        r.time_based_filter.minimum_separation.is_zero(),
        "§2.2.3.12"
    );
    assert!(r.data_representation_offer.is_none());
    assert!(r.entity_name.is_none());
    // The agent filters itself; the reader announces no ContentFilteredTopic.
    assert!(r.content_filter.is_none());
    // Not a relay reader (DDS-Security 1.2 §7.1.1.4): its payloads arrive
    // opened.
    assert!(!r.relay);
    // DDS-XTypes 1.3 §7.6.3.4.1.
    let tce = r.type_consistency;
    assert_eq!(tce.kind, TypeConsistencyKind::AllowTypeCoercion);
    assert!(tce.ignore_sequence_bounds && tce.ignore_string_bounds);
    assert!(!tce.ignore_member_names && !tce.prevent_type_widening && !tce.force_type_validation);
}

#[test]
fn an_endpoint_announces_its_own_partition_over_an_empty_group() {
    let inherited = GroupTopicQos::default();
    // DDS 1.4 §2.2.3.13: an empty PARTITION is the default partition.
    assert!(inherited.partition.is_empty());
    assert!(inherited.group_data.is_empty() && inherited.topic_data.is_empty());
    let own = GroupTopicQos {
        partition: vec![String::from("sensors")],
        group_data: vec![1, 2],
        topic_data: vec![3],
    };
    assert_eq!(own.over(&inherited), own);
    assert_eq!(GroupTopicQos::default().over(&inherited), inherited);
}

#[test]
fn xtypes_7_4_the_sample_tag_names_the_representation_read_body_takes() {
    // `UserSample::Alive::representation` is 0 for XCDR1 and 1 for XCDR2;
    // `read_body` takes 0 and 2.
    assert_eq!(read_body_representation(0), 0);
    assert_eq!(read_body_representation(1), 2);
    let evo = support(EVO_IDL, "Evo", false);
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
        assert_eq!(integer(sample.data(), "c"), -3, "{octets}");
    }
    // The tag passed on unchanged is refused, not read as something else.
    assert!(evo.read_body(&unhex(EVO_XCDR2_LE), 1, false).is_err());
}

#[test]
fn xtypes_7_6_7_a_filter_reads_the_members_of_a_decoded_sample() {
    let evo = support(EVO_IDL, "Evo", false);
    let sample = evo
        .read_body(&unhex(EVO_XCDR2_BE), read_body_representation(1), true)
        .unwrap();
    let row = DdsTypeRow::new(&sample);
    let admits = |expression: &str| {
        zerodds_sql_filter::parse(expression)
            .unwrap()
            .evaluate(&row, &[])
            .unwrap()
    };
    assert!(admits("a = 7 AND c < 0"));
    assert!(admits("b > 1.25"));
    assert!(!admits("a = 8"));
}

#[test]
fn samples_packed_back_to_back_are_read_one_at_a_time() {
    let keyed = support(KEYED_IDL, "Sample", true);
    let one = unhex(KEYED_LE);
    let mut packed = one.clone();
    packed.extend_from_slice(&one);
    let (first, taken) = keyed.read_body_prefix(&packed, 2, false).unwrap();
    assert_eq!(taken, one.len());
    let (second, rest) = keyed.read_body_prefix(&packed[taken..], 2, false).unwrap();
    assert_eq!(rest, one.len());
    for sample in [&first, &second] {
        assert_eq!(integer(sample.data(), "id"), -1_234_567);
        assert_eq!(integer(sample.data(), "payload"), 2_147_483_647);
    }
}

#[test]
fn xtypes_7_6_8_the_key_holder_is_big_endian_and_a_short_one_is_the_key_hash() {
    let keyed = support(KEYED_IDL, "Sample", true);
    for (big_endian, octets) in [(false, KEYED_LE), (true, KEYED_BE)] {
        let sample = keyed.read_body(&unhex(octets), 2, big_endian).unwrap();
        let (hash, holder) = keyed.key_material(&sample).unwrap();
        // What a dispose of this instance carries as its key holder, and the
        // agent hands on as XCDR2 big endian.
        assert_eq!(holder, [0xff, 0xed, 0x29, 0x79]);
        assert_eq!(
            hash,
            [0xff, 0xed, 0x29, 0x79, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
    }
}

#[test]
fn rtps_9_3_2_a_source_time_counts_the_fraction_in_two_to_the_minus_32_seconds() {
    let t = he_timestamp_to_time(HeTimestamp {
        seconds: 1_700_000_000,
        fraction: 0x4000_0000,
    });
    assert_eq!((t.sec, t.nanosec), (1_700_000_000, 250_000_000));
    let t = he_timestamp_to_time(HeTimestamp {
        seconds: 9,
        fraction: u32::MAX,
    });
    assert_eq!((t.sec, t.nanosec), (10, 0), "rounded, the second carries");
}
