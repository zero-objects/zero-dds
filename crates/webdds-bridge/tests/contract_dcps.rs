// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-webdds-bridge` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-dcps`:
//! - `src/bin/zerodds-webdds-bridged.rs`: `cli::Configured::from_command_line`
//!   with the flags `zerodds_config::CommandLine::extract` took out and the
//!   daemon's convenience flags added to the command line, the base name
//!   `config::CONFIG_STEM` and the key predicate `config::is_key`;
//!   `configured.source.tree()`, `checking`, `check_config` and
//!   `configured.runtime`; `cli::HELP` in the usage text;
//!   `cli::notify_service_manager("READY=1")` and `("STOPPING=1")`.
//! - `src/gateway.rs`: `DomainParticipantFactory::instance()
//!   .create_participant`, `create_serialized_topic` with
//!   `TopicQos::default()`, `create_publisher` / `create_subscriber`,
//!   `create_datawriter` / `create_datareader` of `SerializedSample` with
//!   `DataWriterQos::default()` / `DataReaderQos::default()` or a merged
//!   QoS, the `Entity` `get_qos` / `set_qos` of every entity (merged onto
//!   the live QoS, `src/qos_update.rs`), `DataWriter::write`,
//!   `DataReader::read`; a `SerializedSample` from
//!   `SerializedTypeSupport::read_body(bytes, 2, false)` and, on the way out,
//!   its `body`, `representation` (0 read as XCDR1, else XCDR2) and
//!   `big_endian`; `WaitSet` with one `ReadCondition::new(0, 0, 0, …)` per
//!   reader, `attach_condition`, `wait` (`DdsError::Timeout` is "nothing
//!   triggered"), the triggered conditions told apart with `Arc::ptr_eq`.
//! - `src/types_registry.rs`: the runtime's `type_lookup_server` and
//!   `send_type_lookup_request`, `SerializedTypeSupport::from_type_objects`
//!   with the root, its dependency closure, `keyed` and the extensibility
//!   code (0 final, 1 appendable, 2 mutable).
//! - `src/qos_update.rs`: `qos::{DataReaderQos, DataWriterQos,
//!   DomainParticipantQos, PublisherQos, SubscriberQos, TopicQos}` and the
//!   policy types of `zerodds_dcps::qos`.
//! - `src/dynamic_rpc.rs`, `src/rpc_dispatch.rs`: a `dds_type::DdsType` for
//!   DDS-RPC samples (`encode_continuing`, `decode_continuing`,
//!   `field_value` with `FilterValue`) on a `participant::DomainParticipant`.
//!
//! What it relies on and is checked here without a socket: the layers of
//! the daemon's own file and keys, the runtime keys in that file, the rank
//! of a `--set`, the refusal of an unknown `--set` path and of an invalid
//! runtime value, the warning on an unknown file key, `--check-config` and
//! its report without the bearer token; the default QoS sets the gateway
//! creates entities with; `set_qos` that changes a changeable policy,
//! refuses an immutable one or an inconsistent set and then leaves the QoS
//! as it was; `wait` that times out or names the triggered condition; a
//! type support built from a root and its dependencies; the form a
//! `SerializedSample` names for its body.
//!
//! Oracles: `docs/specs/zerodds-deployment-1.0.md` §2.1 ("Runtime
//! configuration from RC7 on"), the rustdoc of `zerodds_config::is_secret_path`
//! and `zerodds_config::Origin`, DDS-XTypes 1.3 §7.6.3.1.1
//! (`XCDR_DATA_REPRESENTATION = 0`, `XCDR2_DATA_REPRESENTATION = 2`, the
//! default `[XCDR]`), DDS 1.4 §2.2.3 (the defaults and the "Changeable"
//! column of the QoS table; §2.2.3.12 DEADLINE period not below the
//! TIME_BASED_FILTER minimum separation, §2.2.3.18 HISTORY depth not above
//! RESOURCE_LIMITS max_samples_per_instance), §2.2.2.1.1.1 (`set_qos`:
//! IMMUTABLE_POLICY, INCONSISTENT_POLICY, nothing changed on a failure),
//! §2.2.2.1.6 (`wait` returns the triggered conditions or TIMEOUT), the
//! rustdoc of `from_type_objects` (a missing dependency refuses the type)
//! and the octets of `tests/interop-corpus/expected-bytes.tsv` that the
//! `specification` rows name (case `annotation-key-single`, sample `values`).
//!
//! Left to the integration pillar, because only a bound runtime shows it:
//! writing and reading samples, the TypeLookup round trip, the DDS-RPC
//! samples on the wire, and `notify_service_manager`, which sends a datagram
//! to `$NOTIFY_SOCKET`.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use zerodds_config::{CommandLine, ConfigError, Origin};
use zerodds_dcps::cli::{Configured, HELP};
use zerodds_dcps::qos::{
    DurabilityKind, HistoryKind, PresentationAccessScope, PresentationQosPolicy, ReliabilityKind,
};
use zerodds_dcps::serialized_type::SerializedTypeSupport;
use zerodds_dcps::{
    Condition, DataReaderQos, DataWriterQos, DdsError, DomainParticipant, DomainParticipantFactory,
    DomainParticipantQos, Entity, GuardCondition, PublisherQos, ReadCondition, SubscriberQos,
    TopicQos, WaitSet,
};
use zerodds_types::dynamic::DynamicType;
use zerodds_types::type_object::TypeObject;
use zerodds_webdds_bridge::daemon::config::{CONFIG_STEM, is_key};

/// DDS-XTypes 1.3 §7.6.3.1.1: `XCDR_DATA_REPRESENTATION`.
const XCDR: i16 = 0;
/// DDS-XTypes 1.3 §7.6.3.1.1: `XCDR2_DATA_REPRESENTATION`.
const XCDR2: i16 = 2;

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

fn type_object_bytes(idl: &str, name: &str) -> Vec<u8> {
    let ast = Arc::new(zerodds_wire_probe::encode::model_from_idl(idl, "contract.idl").unwrap());
    let type_ = DynamicType::find(&ast, name).unwrap();
    TypeObject::Complete(zerodds_types::from_ast::complete_type_object(&ast, type_.id()).unwrap())
        .to_bytes_le()
        .unwrap()
}

/// What `src/types_registry.rs` builds for a resolved type.
fn keyed_support() -> Arc<SerializedTypeSupport> {
    SerializedTypeSupport::from_type_objects(
        "Sample",
        &type_object_bytes(KEYED_IDL, "Sample"),
        &[],
        true,
        0,
    )
    .unwrap()
}

/// A participant without a network: what the gateway's entities are on,
/// minus the runtime.
fn participant() -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default())
}

/// A daemon file of this test alone, so that no system file takes part.
fn daemon_file(name: &str, text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "zerodds-webdds-contract-dcps-{}-{name}.yaml",
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
        "domain: 7\nmax_body_bytes: 65536\nwire:\n  data_representation_offer: XCDR2\n",
    );
    let configured = configured(&file, &[]).unwrap();
    let tree = configured.source.tree().unwrap();
    assert_eq!(tree.get_str("domain"), Some("7"));
    assert_eq!(tree.get_str("max_body_bytes"), Some("65536"));
    // The runtime key of the daemon's file reaches the DDS side.
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
        configured(&file, &["--set", "tls.no_such_key=1"]),
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
    let file = daemon_file("unknown-key", "tls:\n  no_such_key: 1\n");
    let started = configured(&file, &[]).unwrap();
    let unknown = started.source.unknown_keys(&is_key);
    assert_eq!(unknown.len(), 1, "{unknown:?}");
    assert_eq!(unknown[0].0, "tls.no_such_key");
    let checked = configured(&file, &["--check-config"]).unwrap();
    assert_eq!(checked.check_config_valid(), Some(false));
    let _ = std::fs::remove_file(file);
}

#[test]
fn the_file_the_config_flag_names_must_exist() {
    let missing = std::env::temp_dir().join(format!(
        "zerodds-webdds-contract-dcps-{}-missing.yaml",
        std::process::id()
    ));
    assert!(matches!(
        configured(&missing, &[]),
        Err(ConfigError::Io(..))
    ));
}

#[test]
fn the_check_config_report_hides_the_bearer_token() {
    let file = daemon_file("secret", "auth:\n  bearer_token: tok-5a17e0\n");
    let configured = configured(&file, &[]).unwrap();
    // `Ok` holds the report of a valid configuration, `Err` the one with
    // its findings; either is printed.
    let (Ok(report) | Err(report)) = configured.source.check_report(&is_key);
    assert!(!report.contains("tok-5a17e0"), "{report}");
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
fn dcps_2_2_3_the_default_qos_sets_are_the_dds_defaults() {
    let w = DataWriterQos::default();
    assert_eq!(w.reliability.kind, ReliabilityKind::Reliable, "§2.2.3.14");
    assert_eq!(
        w.reliability.max_blocking_time,
        zerodds_qos::Duration::from_millis(100),
        "§2.2.3.14"
    );
    assert_eq!(w.durability.kind, DurabilityKind::Volatile, "§2.2.3.4");
    assert_eq!(
        (w.history.kind, w.history.depth),
        (HistoryKind::KeepLast, 1),
        "§2.2.3.18"
    );
    assert_eq!(w.resource_limits.max_samples, -1, "§2.2.3.19");
    assert_eq!(w.resource_limits.max_instances, -1, "§2.2.3.19");
    assert_eq!(w.resource_limits.max_samples_per_instance, -1, "§2.2.3.19");
    assert!(
        w.writer_data_lifecycle.autodispose_unregistered_instances,
        "§2.2.3.21"
    );
    assert!(w.data_representation.is_none());

    let r = DataReaderQos::default();
    assert_eq!(r.reliability.kind, ReliabilityKind::BestEffort, "§2.2.3.14");
    assert_eq!(r.durability.kind, DurabilityKind::Volatile, "§2.2.3.4");
    assert_eq!(
        (r.history.kind, r.history.depth),
        (HistoryKind::KeepLast, 1),
        "§2.2.3.18"
    );
    assert_eq!(r.resource_limits.max_samples, -1, "§2.2.3.19");
    assert!(
        r.time_based_filter.minimum_separation.is_zero(),
        "§2.2.3.12"
    );
    assert!(r.deadline.period.is_infinite(), "§2.2.3.7");

    let t = TopicQos::default();
    assert_eq!(t.reliability.kind, ReliabilityKind::BestEffort, "§2.2.3.14");
    assert_eq!(
        (t.history.kind, t.history.depth),
        (HistoryKind::KeepLast, 1),
        "§2.2.3.18"
    );
    assert!(w.check_consistency().is_ok());
    assert!(r.check_consistency().is_ok());
    assert!(t.check_consistency().is_ok());
}

#[test]
fn dcps_2_2_2_1_1_1_a_writer_changes_deadline_and_keeps_its_qos_on_a_refusal() {
    let p = participant();
    let topic = p
        .create_serialized_topic("WebddsWriterQos", TopicQos::default(), keyed_support())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let writer = publisher
        .create_datawriter(&topic, DataWriterQos::default())
        .unwrap();
    assert!(writer.is_enabled(), "§2.2.3.20: created enabled");
    // DEADLINE: changeable.
    let mut changed = writer.get_qos();
    changed.deadline.period = zerodds_qos::Duration::from_secs(2);
    writer.set_qos(changed.clone()).unwrap();
    assert_eq!(writer.get_qos(), changed);
    // RELIABILITY: not changeable once enabled.
    let mut immutable = changed.clone();
    immutable.reliability.kind = ReliabilityKind::BestEffort;
    assert!(matches!(
        writer.set_qos(immutable),
        Err(DdsError::ImmutablePolicy { .. })
    ));
    // §2.2.3.18: a KEEP_LAST history keeps at least one sample.
    let mut inconsistent = changed.clone();
    inconsistent.history.depth = 0;
    assert!(matches!(
        writer.set_qos(inconsistent),
        Err(DdsError::InconsistentPolicy { .. })
    ));
    assert_eq!(writer.get_qos(), changed, "nothing changed on a refusal");
}

#[test]
fn dcps_2_2_2_1_1_1_a_reader_refuses_a_filter_wider_than_its_deadline() {
    let p = participant();
    let topic = p
        .create_serialized_topic("WebddsReaderQos", TopicQos::default(), keyed_support())
        .unwrap();
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader(&topic, DataReaderQos::default())
        .unwrap();
    let before = reader.get_qos();
    // §2.2.3.12: DEADLINE period >= TIME_BASED_FILTER minimum_separation.
    let mut inconsistent = before.clone();
    inconsistent.deadline.period = zerodds_qos::Duration::from_secs(1);
    inconsistent.time_based_filter.minimum_separation = zerodds_qos::Duration::from_secs(2);
    assert!(matches!(
        reader.set_qos(inconsistent),
        Err(DdsError::InconsistentPolicy { .. })
    ));
    // DURABILITY: not changeable once enabled.
    let mut immutable = before.clone();
    immutable.durability.kind = DurabilityKind::TransientLocal;
    assert!(matches!(
        reader.set_qos(immutable),
        Err(DdsError::ImmutablePolicy { .. })
    ));
    assert_eq!(reader.get_qos(), before);
    // TIME_BASED_FILTER within the deadline: changeable.
    let mut changed = before;
    changed.time_based_filter.minimum_separation = zerodds_qos::Duration::from_millis(10);
    reader.set_qos(changed.clone()).unwrap();
    assert_eq!(reader.get_qos(), changed);
}

#[test]
fn dcps_2_2_2_1_1_1_a_topic_and_a_publisher_take_their_changeable_policies_only() {
    let p = participant();
    let topic = p
        .create_serialized_topic("WebddsTopicQos", TopicQos::default(), keyed_support())
        .unwrap();
    // §2.2.2.1.1.7: a second `enable` is a no-op; the gateway's topics are
    // enabled.
    topic.enable().unwrap();
    assert!(topic.is_enabled());
    let before = topic.get_qos();
    assert_eq!(before, TopicQos::default());
    let mut changed = before.clone();
    changed.deadline.period = zerodds_qos::Duration::from_secs(3);
    topic.set_qos(changed.clone()).unwrap();
    assert_eq!(topic.get_qos(), changed);
    let mut immutable = changed.clone();
    immutable.reliability.kind = ReliabilityKind::Reliable;
    assert!(matches!(
        topic.set_qos(immutable),
        Err(DdsError::ImmutablePolicy { .. })
    ));
    // §2.2.3.18: depth above max_samples_per_instance.
    let mut inconsistent = changed.clone();
    inconsistent.history.depth = 5;
    inconsistent.resource_limits.max_samples_per_instance = 2;
    assert!(matches!(
        topic.set_qos(inconsistent),
        Err(DdsError::InconsistentPolicy { .. })
    ));
    assert_eq!(topic.get_qos(), changed);

    let publisher = p.create_publisher(PublisherQos::default());
    let mut partition = publisher.get_qos();
    partition.partition.names = vec![String::from("a")];
    publisher.set_qos(partition.clone()).unwrap();
    assert_eq!(publisher.get_qos(), partition);
    let mut presentation = partition.clone();
    presentation.presentation = PresentationQosPolicy {
        access_scope: PresentationAccessScope::Topic,
        coherent_access: true,
        ordered_access: false,
    };
    assert!(matches!(
        publisher.set_qos(presentation),
        Err(DdsError::ImmutablePolicy { .. })
    ));
    assert_eq!(publisher.get_qos(), partition);
}

#[test]
fn dcps_2_2_2_1_6_wait_names_the_triggered_condition_or_times_out() {
    let ws = WaitSet::new();
    let guard = GuardCondition::new();
    let attached: Arc<dyn Condition> = Arc::clone(&guard) as Arc<dyn Condition>;
    ws.attach_condition(Arc::clone(&attached)).unwrap();
    assert!(matches!(
        ws.wait(Duration::from_millis(20)),
        Err(DdsError::Timeout)
    ));
    guard.set_trigger_value(true);
    let triggered = ws.wait(Duration::from_millis(20)).unwrap();
    assert_eq!(triggered.len(), 1);
    assert!(Arc::ptr_eq(&triggered[0], &attached));

    // The gateway's read conditions: one closure per reader.
    let ws = WaitSet::new();
    let empty = ReadCondition::new(0, 0, 0, |_, _, _| false);
    let ready = ReadCondition::new(0, 0, 0, |_, _, _| true);
    let empty_attached: Arc<dyn Condition> = Arc::clone(&empty) as Arc<dyn Condition>;
    let ready_attached: Arc<dyn Condition> = Arc::clone(&ready) as Arc<dyn Condition>;
    ws.attach_condition(Arc::clone(&empty_attached)).unwrap();
    assert!(matches!(
        ws.wait(Duration::from_millis(20)),
        Err(DdsError::Timeout)
    ));
    ws.attach_condition(Arc::clone(&ready_attached)).unwrap();
    let triggered = ws.wait(Duration::from_millis(20)).unwrap();
    assert_eq!(triggered.len(), 1);
    assert!(Arc::ptr_eq(&triggered[0], &ready_attached));
    assert!(!Arc::ptr_eq(&triggered[0], &empty_attached));
}

#[test]
fn a_type_with_a_nested_type_needs_its_dependency() {
    let idl = "@final struct Inner { long v; }; @final struct Outer { Inner i; long w; };";
    let root = type_object_bytes(idl, "Outer");
    let inner = type_object_bytes(idl, "Inner");
    assert!(SerializedTypeSupport::from_type_objects("Outer", &root, &[], false, 0).is_err());
    let support =
        SerializedTypeSupport::from_type_objects("Outer", &root, &[inner], false, 0).unwrap();
    assert!(!support.is_keyed());
    // The extensibility code has to name the type's: 0 final.
    assert!(
        SerializedTypeSupport::from_type_objects(
            "Sample",
            &type_object_bytes(KEYED_IDL, "Sample"),
            &[],
            true,
            2
        )
        .is_err()
    );
}

#[test]
fn a_serialized_sample_names_the_form_of_its_body() {
    let support = keyed_support();
    for (representation, big_endian, octets) in [
        (2, false, KEYED_LE),
        (2, true, KEYED_BE),
        (0, false, KEYED_LE),
        (0, true, KEYED_BE),
    ] {
        let body = unhex(octets);
        let sample = support
            .read_body(&body, representation, big_endian)
            .unwrap();
        assert_eq!(sample.representation(), representation);
        assert_eq!(sample.big_endian(), big_endian);
        assert_eq!(sample.body(), body);
    }
}
