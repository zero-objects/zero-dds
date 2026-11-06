// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-omg-shape-main` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-dcps`:
//! - `src/qos_map.rs`: `qos::{DataWriterQos, DataReaderQos, PublisherQos,
//!   SubscriberQos}` built over their `Default`, with `ReliabilityQosPolicy`,
//!   `DurabilityQosPolicy`, `HistoryQosPolicy` (KEEP_ALL for a depth `<= 0`),
//!   DEADLINE, LIFESPAN, `OwnershipQosPolicy`/`OwnershipStrengthQosPolicy`,
//!   `PartitionQosPolicy`, `TimeBasedFilterQosPolicy`,
//!   `WriterDataLifecycleQosPolicy` (autodispose off for `--final-instance-state
//!   u`), `PresentationQosPolicy` and `data_representation` (`[0]`, `[2]`).
//! - `src/main.rs`: `RuntimeConfig::from_standard_paths`, the fields it sets
//!   for the suite (`reader_heartbeat_response_delay`,
//!   `reader_heartbeat_suppression_duration`, `nack_response_delay`,
//!   `spdp_period`, `accept_empty_acknack_base_zero`, `pl_cdr1_reading`,
//!   `writer_unregister_on_delete`, `reader_honor_data_max_size_serialized`,
//!   `reader_dispose_with_unregister`, `key_member_bounds`) unless the
//!   environment names the setting's variable, `DdsError::Configuration`,
//!   `create_participant_with_config`, `runtime()` with
//!   `user_writer_behaviour` and `set_vendor_rtps_behaviour`;
//!   `create_publisher`, `create_subscriber`, `create_topic`,
//!   `create_datawriter`, `create_datareader`; on the writer
//!   `set_listener`, `register_instance`, `write`, `drive_listeners`,
//!   `unregister_instance`, `dispose`, `wait_for_acknowledgments`; on the
//!   publisher `begin_coherent_changes`/`end_coherent_changes`; on the
//!   reader `with_filter`, `set_listener`, `read_with_info`,
//!   `take_with_info`, `read_next_instance`/`take_next_instance` from
//!   `HANDLE_NIL`; on the subscriber `begin_access`/`end_access`;
//!   `Sample`, `sample_info::SampleInfo` (`valid_data`, `instance_state`,
//!   `instance_handle`), `InstanceStateKind`, `psm_constants::status`.
//! - `src/listeners.rs`: `listener::{DataWriterListener,
//!   DataReaderListener}` on one struct, the `status` structs, and
//!   `psm_constants::qos_policy_id` to name the last incompatible policy.
//!
//! What it relies on and is checked here, with the generated `ShapeType`
//! of `interop/shapes/omg-dds-rtps.idl` (the tool's own type, from the same
//! build script):
//! - The status kinds and QoS policy ids are the PSM values (DDS 1.4
//!   §2.3.3; DATA_REPRESENTATION 23 of DDS-XTypes 1.3 Annex D).
//! - The flags change the DDS defaults, so the defaults are the spec's (DDS
//!   1.4 §2.2.3, the QoS table; §2.2.3.21 autodispose TRUE).
//! - The suite's QoS creates its entities; a TIME_BASED_FILTER longer than
//!   the DEADLINE does not (DDS 1.4 §2.2.3.12, §2.2.2.5.2.5).
//! - Instances: `register_instance` is idempotent per key (§2.2.2.4.2.5),
//!   `lookup_instance` finds it (§2.2.2.4.2.10), `dispose` and
//!   `unregister_instance` take it (§2.2.2.4.2.13, §2.2.2.4.2.7).
//! - Coherent sets and group access nest and need their begin
//!   (§2.2.2.4.1.10, §2.2.2.4.1.11, §2.2.2.5.2.8, §2.2.2.5.2.9);
//!   `wait_for_acknowledgments` with no reliable reader matched returns OK
//!   (§2.2.2.4.2.15).
//! - The reading paths return nothing when nothing arrived, also from
//!   `HANDLE_NIL` (§2.2.2.5.3.16).
//! - Each variable the tool tests before it sets a value is one the runtime
//!   reads, for that setting, and the values it replaces are the
//!   specifications' (`man/man5/zerodds.yaml.5`; DDSI-RTPS 2.5 §8.4.7.1
//!   `nackResponseDelay` 200 ms, §8.4.10.1 `heartbeatResponseDelay` 500 ms
//!   and `heartbeatSuppressionDuration` 0).
//!
//! `create_participant_with_config` opens sockets; the participants here
//! have no runtime, and the runtime calls are checked at compile time.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration as StdDuration;

use zerodds_ast_layout::PlCdr1Reading;
use zerodds_dcps::psm_constants::{qos_policy_id, status};
use zerodds_dcps::qos::{
    DataReaderQos, DataWriterQos, DurabilityKind, HistoryKind, HistoryQosPolicy, OwnershipKind,
    PresentationAccessScope, PublisherQos, ReliabilityKind, SubscriberQos,
};
use zerodds_dcps::runtime::{RuntimeConfig, setting_path};
use zerodds_dcps::status::PublicationMatchedStatus;
use zerodds_dcps::{
    DdsError, DomainParticipant, DomainParticipantFactory, DomainParticipantQos, HANDLE_NIL,
    InstanceHandle, InstanceStateKind, TopicQos,
};
use zerodds_qos::Duration;
use zerodds_rtps::dialect::{EndOfSetForm, GroupWireForm};
use zerodds_rtps::wire_types::VendorId;
use zerodds_types::assignability::KeyMemberBounds;

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/omg-dds-rtps.rs"));
}

use generated::ShapeType;

fn shape(color: &str) -> ShapeType {
    ShapeType {
        color: color.to_string(),
        x: 10,
        y: 20,
        shapesize: 30,
        additional_payload_size: Vec::new(),
    }
}

fn participant() -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default())
}

#[test]
fn dcps_2_3_3_the_status_kinds_are_the_psm_bits() {
    let bits = [
        (status::INCONSISTENT_TOPIC, 0),
        (status::OFFERED_DEADLINE_MISSED, 1),
        (status::REQUESTED_DEADLINE_MISSED, 2),
        (status::OFFERED_INCOMPATIBLE_QOS, 5),
        (status::REQUESTED_INCOMPATIBLE_QOS, 6),
        (status::SAMPLE_LOST, 7),
        (status::SAMPLE_REJECTED, 8),
        (status::DATA_ON_READERS, 9),
        (status::DATA_AVAILABLE, 10),
        (status::LIVELINESS_LOST, 11),
        (status::LIVELINESS_CHANGED, 12),
        (status::PUBLICATION_MATCHED, 13),
        (status::SUBSCRIPTION_MATCHED, 14),
    ];
    for (kind, shift) in bits {
        assert_eq!(kind, 0x0001 << shift);
        assert_eq!(
            status::ANY & kind,
            kind,
            "the mask the listeners are set with"
        );
    }
}

#[test]
fn dcps_2_3_3_the_qos_policy_ids_are_the_psm_values() {
    let ids = [
        (qos_policy_id::USER_DATA, 1),
        (qos_policy_id::DURABILITY, 2),
        (qos_policy_id::PRESENTATION, 3),
        (qos_policy_id::DEADLINE, 4),
        (qos_policy_id::LATENCY_BUDGET, 5),
        (qos_policy_id::OWNERSHIP, 6),
        (qos_policy_id::OWNERSHIP_STRENGTH, 7),
        (qos_policy_id::LIVELINESS, 8),
        (qos_policy_id::TIME_BASED_FILTER, 9),
        (qos_policy_id::PARTITION, 10),
        (qos_policy_id::RELIABILITY, 11),
        (qos_policy_id::DESTINATION_ORDER, 12),
        (qos_policy_id::HISTORY, 13),
        (qos_policy_id::RESOURCE_LIMITS, 14),
    ];
    for (id, value) in ids {
        assert_eq!(id, value);
    }
}

#[test]
fn xtypes_annex_d_data_representation_is_policy_23() {
    assert_eq!(qos_policy_id::DATA_REPRESENTATION, 23);
}

#[test]
fn dcps_2_2_3_the_flags_start_from_the_spec_defaults() {
    let w = DataWriterQos::default();
    assert_eq!(w.reliability.kind, ReliabilityKind::Reliable);
    assert_eq!(w.reliability.max_blocking_time, Duration::from_millis(100));
    assert_eq!(w.durability.kind, DurabilityKind::Volatile);
    assert_eq!(w.history.kind, HistoryKind::KeepLast);
    assert_eq!(w.history.depth, 1);
    assert_eq!(w.deadline.period, Duration::INFINITE);
    assert_eq!(w.lifespan.duration, Duration::INFINITE);
    assert_eq!(w.ownership.kind, OwnershipKind::Shared);
    assert_eq!(w.ownership_strength.value, 0);
    assert!(w.partition.names.is_empty());
    assert!(w.writer_data_lifecycle.autodispose_unregistered_instances);
    assert_eq!(w.data_representation, None);

    let r = DataReaderQos::default();
    assert_eq!(r.reliability.kind, ReliabilityKind::BestEffort);
    assert_eq!(r.durability.kind, DurabilityKind::Volatile);
    assert_eq!(r.history.kind, HistoryKind::KeepLast);
    assert_eq!(r.history.depth, 1);
    assert_eq!(r.deadline.period, Duration::INFINITE);
    assert_eq!(r.time_based_filter.minimum_separation, Duration::ZERO);
    assert_eq!(r.ownership.kind, OwnershipKind::Shared);

    let p = PublisherQos::default().presentation;
    assert_eq!(p.access_scope, PresentationAccessScope::Instance);
    assert!(!p.coherent_access);
    assert!(!p.ordered_access);
    assert_eq!(SubscriberQos::default().presentation, p);
}

/// The QoS `-P -r -k 0 -s 5 --deadline 500 --lifespan 2000 -p A -x 2
/// --final-instance-state u` maps to.
fn suite_writer_qos() -> DataWriterQos {
    let mut q = DataWriterQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.durability.kind = DurabilityKind::TransientLocal;
    q.history = HistoryQosPolicy {
        kind: HistoryKind::KeepAll,
        depth: i32::MAX,
    };
    q.deadline.period = Duration::from_millis(500);
    q.lifespan.duration = Duration::from_millis(2000);
    q.ownership.kind = OwnershipKind::Exclusive;
    q.ownership_strength.value = 5;
    q.partition.names = vec!["A".to_string()];
    q.writer_data_lifecycle.autodispose_unregistered_instances = false;
    q.data_representation = Some(vec![2]);
    q
}

#[test]
fn dcps_2_2_3_12_the_suite_qos_creates_its_entities() {
    let p = participant();
    let topic = p
        .create_topic::<ShapeType>("contract-dcps-Square", TopicQos::default())
        .unwrap();
    let mut group = PublisherQos::default();
    group.presentation.access_scope = PresentationAccessScope::Group;
    group.presentation.coherent_access = true;
    group.presentation.ordered_access = true;
    let publisher = p.create_publisher(group);
    let writer = publisher
        .create_datawriter::<ShapeType>(&topic, suite_writer_qos())
        .unwrap();
    writer.write(&shape("RED")).unwrap();

    let mut reader_qos = DataReaderQos::default();
    reader_qos.reliability.kind = ReliabilityKind::Reliable;
    reader_qos.history = HistoryQosPolicy {
        kind: HistoryKind::KeepLast,
        depth: 3,
    };
    reader_qos.deadline.period = Duration::from_millis(500);
    reader_qos.time_based_filter.minimum_separation = Duration::from_millis(100);
    reader_qos.data_representation = Some(vec![0]);
    let subscriber = p.create_subscriber(SubscriberQos::default());
    assert!(
        subscriber
            .create_datareader::<ShapeType>(&topic, reader_qos.clone())
            .is_ok()
    );

    // A minimum separation above the deadline period is inconsistent.
    reader_qos.time_based_filter.minimum_separation = Duration::from_millis(600);
    assert!(matches!(
        subscriber.create_datareader::<ShapeType>(&topic, reader_qos),
        Err(DdsError::InconsistentPolicy { .. })
    ));
}

#[test]
fn dcps_2_2_2_4_2_5_register_instance_is_idempotent_per_key() {
    let p = participant();
    let topic = p
        .create_topic::<ShapeType>("contract-dcps-Circle", TopicQos::default())
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<ShapeType>(&topic, DataWriterQos::default())
        .unwrap();
    let red = writer.register_instance(&shape("RED")).unwrap();
    assert_ne!(red, HANDLE_NIL, "the topic is keyed by color");
    let mut moved = shape("RED");
    moved.x = 99;
    assert_eq!(writer.register_instance(&moved).unwrap(), red);
    let blue = writer.register_instance(&shape("BLUE")).unwrap();
    assert_ne!(blue, red);
    assert_eq!(writer.lookup_instance(&shape("BLUE")), blue);
    assert_eq!(writer.lookup_instance(&shape("GREEN")), HANDLE_NIL);
    writer.write(&shape("RED")).unwrap();
    writer.dispose(&shape("RED"), red).unwrap();
    writer.unregister_instance(&shape("BLUE"), blue).unwrap();
    assert!(
        writer
            .wait_for_acknowledgments(StdDuration::from_secs(1))
            .is_ok(),
        "no reliable reader is matched"
    );
}

#[test]
fn dcps_2_2_2_4_1_11_coherent_sets_and_group_access_need_their_begin() {
    let p = participant();
    let publisher = p.create_publisher(PublisherQos::default());
    assert!(matches!(
        publisher.end_coherent_changes(),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    publisher.begin_coherent_changes().unwrap();
    publisher.begin_coherent_changes().unwrap();
    publisher.end_coherent_changes().unwrap();
    publisher.end_coherent_changes().unwrap();
    assert!(matches!(
        publisher.end_coherent_changes(),
        Err(DdsError::PreconditionNotMet { .. })
    ));

    let subscriber = p.create_subscriber(SubscriberQos::default());
    assert!(matches!(
        subscriber.end_access(),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    subscriber.begin_access().unwrap();
    subscriber.begin_access().unwrap();
    subscriber.end_access().unwrap();
    subscriber.end_access().unwrap();
    assert!(matches!(
        subscriber.end_access(),
        Err(DdsError::PreconditionNotMet { .. })
    ));
}

/// One listener for a writer and a reader, as `ShapeListener` is.
#[derive(Default)]
struct Matches {
    publication: AtomicU64,
}

impl zerodds_dcps::listener::DataWriterListener for Matches {
    fn on_publication_matched(&self, _writer: InstanceHandle, status: PublicationMatchedStatus) {
        self.publication.store(
            u64::try_from(status.total_count).unwrap_or(0),
            Ordering::Relaxed,
        );
    }
}

impl zerodds_dcps::listener::DataReaderListener for Matches {}

#[test]
fn dcps_2_2_2_5_3_16_the_reading_paths_return_nothing_when_nothing_arrived() {
    let p = participant();
    let topic = p
        .create_topic::<ShapeType>("contract-dcps-Triangle", TopicQos::default())
        .unwrap();
    let listener = Arc::new(Matches::default());
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<ShapeType>(&topic, DataWriterQos::default())
        .unwrap();
    writer.set_listener(Some(listener.clone()), status::ANY);
    writer.drive_listeners();
    // No reader matched, so no match was counted.
    assert_eq!(listener.publication.load(Ordering::Relaxed), 0);

    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<ShapeType>(&topic, DataReaderQos::default())
        .unwrap()
        .with_filter(|s: &ShapeType| s.color == "BLUE");
    reader.set_listener(Some(listener), status::ANY);
    reader.drive_listeners();
    assert!(reader.read_with_info().unwrap().is_empty());
    assert!(reader.take_with_info().unwrap().is_empty());
    assert!(reader.take_next_instance(HANDLE_NIL).unwrap().is_empty());
    assert!(reader.read_next_instance(HANDLE_NIL).unwrap().is_empty());
}

/// `sample_line`'s reading of a sample without data.
fn state_name(state: InstanceStateKind) -> &'static str {
    match state {
        InstanceStateKind::NotAliveDisposed => "NOT_ALIVE_DISPOSED_INSTANCE_STATE",
        InstanceStateKind::NotAliveNoWriters => "NOT_ALIVE_NO_WRITERS_INSTANCE_STATE",
        InstanceStateKind::Alive => "ALIVE_INSTANCE_STATE",
    }
}

#[test]
fn the_suite_settings_name_the_variables_the_runtime_reads() {
    let _: fn(InstanceStateKind) -> &'static str = state_name;
    let settings = [
        (
            "ZERODDS_WIRE_ACCEPT_EMPTY_ACKNACK_BASE_ZERO",
            "wire.accept_empty_acknack_base_zero",
        ),
        ("ZERODDS_WIRE_PL_CDR1_READING", "wire.pl_cdr1_reading"),
        (
            "ZERODDS_WRITER_UNREGISTER_ON_DELETE",
            "writer.unregister_on_delete",
        ),
        (
            "ZERODDS_READER_HONOR_DATA_MAX_SIZE_SERIALIZED",
            "reader.honor_data_max_size_serialized",
        ),
        (
            "ZERODDS_READER_DISPOSE_WITH_UNREGISTER",
            "reader.dispose_with_unregister",
        ),
        ("ZERODDS_KEY_MEMBER_BOUNDS", "xtypes.key_member_bounds"),
        (
            "ZERODDS_WRITER_GROUP_INFO_ON_WIRE",
            "writer.group_info_on_wire",
        ),
    ];
    for (variable, path) in settings {
        assert_eq!(setting_path(variable), Some(path), "{variable}");
    }
}

#[test]
fn rtps_8_4_7_1_the_settings_the_suite_changes_default_to_the_specifications() {
    let defaults = RuntimeConfig::default();
    // DDSI-RTPS 2.5 §8.4.7.1 and §8.4.10.1, "Default Timing-Related Values".
    assert_eq!(defaults.nack_response_delay, StdDuration::from_millis(200));
    assert_eq!(
        defaults.reader_heartbeat_response_delay,
        StdDuration::from_millis(500)
    );
    assert_eq!(
        defaults.reader_heartbeat_suppression_duration,
        StdDuration::ZERO
    );
    // `man/man5/zerodds.yaml.5`, each with the clause it reads.
    assert!(!defaults.accept_empty_acknack_base_zero);
    assert_eq!(defaults.pl_cdr1_reading, PlCdr1Reading::Strict);
    assert!(defaults.writer_unregister_on_delete);
    assert!(defaults.reader_honor_data_max_size_serialized);
    assert!(defaults.reader_dispose_with_unregister);
    assert_eq!(defaults.key_member_bounds, KeyMemberBounds::Enforced);
}

#[test]
fn the_runtime_calls_keep_the_shape_shape_main_uses() {
    assert!(participant().runtime().is_none(), "vendor_forms returns");
    let _ = |p: &DomainParticipant| {
        let Some(runtime) = p.runtime() else {
            return;
        };
        let form = runtime
            .user_writer_behaviour()
            .with_group_wire_form(GroupWireForm::CoherentSetsOnly);
        for vendor in [VendorId::RTI_CONNEXT, VendorId::HDDS, VendorId::ZZDDS] {
            runtime.set_vendor_rtps_behaviour(vendor, form);
        }
        runtime.set_vendor_rtps_behaviour(
            VendorId::INTERCOM,
            runtime
                .user_writer_behaviour()
                .with_end_of_set_form(EndOfSetForm::Gap),
        );
    };
    let _ = |config: RuntimeConfig| -> zerodds_dcps::Result<DomainParticipant> {
        DomainParticipantFactory::instance().create_participant_with_config(
            0,
            DomainParticipantQos::default(),
            config,
        )
    };
    let _ = || -> zerodds_dcps::Result<RuntimeConfig> {
        RuntimeConfig::from_standard_paths().map_err(|error| DdsError::Configuration {
            message: error.to_string(),
        })
    };
}
