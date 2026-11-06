// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-c-api` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-dcps`, from its non-test sources:
//! - `runtime::DcpsRuntime` behind every C runtime handle (`lib.rs`,
//!   `entities.rs`, `publisher_ffi.rs`, `subscriber_ffi.rs`,
//!   `participant_ffi.rs`, `extra_ffi.rs`, `security_ffi.rs`,
//!   `shm_loan_ffi.rs`, `rpc_*_ffi.rs`), started with `DcpsRuntime::start`,
//!   with `UserWriterConfig`, `UserReaderConfig`, `UserSample` (`Alive`,
//!   `Lifecycle`, `CoherentSetEnd`), `UserReaderListener`, `SampleOrigin`,
//!   `StatusHook`, `GroupTopicQos`, `WriterQosUpdate`, `ReaderQosUpdate`,
//!   `RpcDiscoveryExt`, `RpcPeer`, `DeliveryMode` and the `PHASE_*` counters.
//! - `runtime::{ConfigSource, RuntimeConfig}` (`lib.rs`, `security_ffi.rs`):
//!   `ConfigSource::{from_standard_paths, from_path, from_command_line}`,
//!   `ConfigSource::var("wire.data_representation_offer")`,
//!   `RuntimeConfig::{from_source, ros_defaults}` and the field
//!   `data_representation_offer`.
//! - `participant::{host_id_bytes, next_process_unique}` for the GUID prefix
//!   of a C runtime (`lib.rs`), `participant::DomainParticipant` and its
//!   `ignore_*` operations (`participant_ffi.rs`).
//! - `qos::{DataWriterQos, DataReaderQos, TopicQos, PublisherQos,
//!   SubscriberQos, DomainParticipantQos}` and
//!   `factory::DomainParticipantFactoryQos` over their defaults
//!   (`entities.rs`, `qos_ffi.rs`, `qos_provider_ffi.rs`, `factory_ffi.rs`),
//!   `qos::{immutable_writer_change, immutable_reader_change}` for
//!   `set_qos` (`qos_ffi.rs`), `psm_constants::status` (`condition_ffi.rs`),
//!   `status::{QosPolicyCount, SampleRejectedStatusKind}` (`status_ffi.rs`,
//!   `subscriber_ffi.rs`).
//! - `instance_tracker::InstanceTracker` for the instances of a C writer and
//!   reader (`entities.rs`, `builtin_type_ffi.rs`, `subscriber_ffi.rs`):
//!   `new`, `register`, `lookup`, `len`, `get_by_handle`, `dispose`,
//!   `unregister`, `ensure_registered_limited`, `key_hash_for_handle`,
//!   `note_writer`, `drop_writer`, `mark_view_seen`.
//! - `instance_handle::InstanceHandle::{from_raw, as_raw, from_guid}` for the
//!   `u64` handles of C, `builtin_topics::TopicBuiltinTopicData::
//!   synthesize_key` (`entity_ffi.rs`, `builtin_ffi.rs`).
//! - `sample_info::{SampleInfo, SampleStateKind, ViewStateKind,
//!   InstanceStateKind, compute_ranks}` (`subscriber_ffi.rs`,
//!   `builtin_reader_ffi.rs`).
//! - `time::{Time, time_to_he_timestamp, he_timestamp_to_time}`
//!   (`publisher_ffi.rs`, `subscriber_ffi.rs`, `extra_ffi.rs`, `lib.rs`).
//! - `sample_bytes::{SampleBytes, SampleWire}` as the payload of a received
//!   sample and of a read loan (`lib.rs`, `subscriber_ffi.rs`).
//! - `builtin_types::{DdsString, DdsKeyedString, DdsBytes, DdsKeyedBytes,
//!   StringTypeSupport, KeyedStringTypeSupport, BytesTypeSupport,
//!   KeyedBytesTypeSupport, TypeSupport}`, `builtin_type_set::NAME_DDS_*` and
//!   `dds_type::PlainCdr2BeKeyHolder` (`builtin_type_ffi.rs`).
//! - `serialized_type::{SerializedTypeSupport::from_type_object(s),
//!   SerializedSample, read_body_representation}` with `DdsType::field_value`
//!   for content filters (`participant_ffi.rs`, `entities.rs`,
//!   `condition_ffi.rs`, `multitopic_ffi.rs`).
//! - `DdsError` variants it maps to C status codes.
//!
//! Every use of `DcpsRuntime` needs a started runtime, which binds sockets;
//! it is left to the integration pillar. The tests here cover what the C API
//! relies on without one: the QoS defaults and the policies `set_qos` may not
//! change after `enable`; the PSM values of the status bits and policy ids it
//! hands to C; the instance bookkeeping of its writers and readers; the time
//! conversion; the ranks of a returned collection; the octets, key holders,
//! key hashes and type objects of the four built-in types; the descriptor
//! support; the layered configuration it loads; the parts of the GUID prefix
//! it takes from `zerodds-dcps`.
//!
//! Oracles: DDS 1.4 §2.2.2.4.2.5/.7/.13 (instance handles), §2.2.2.5.1.3,
//! .5 and .6 (instance state, generation counts, ranks), §2.2.3 (defaults
//! and the Changeable column), §2.2.3.19 (`max_instances`), §2.2.4.1
//! (`SampleRejectedStatusKind`), §2.2.5 (one DCPSTopic entry per
//! participant), §2.3.2 (`nanosec` below 10^9), §2.3.3 (PSM values of status
//! bits, policy ids and `LENGTH_UNLIMITED`); DDSI-RTPS 2.5 §8.2.4.2 (a unique
//! GUID per participant), §9.3.2.1 (`Time_t`, its reserved values);
//! DDS-XTypes 1.3 §7.4.3 and Table 60 (PLAIN_CDR, PLAIN_CDR2,
//! DELIMITED_CDR2), §7.5.2 (`ExtensibilityKind` FINAL = 0, APPENDABLE = 1,
//! MUTABLE = 2), §7.6.3.1.1/.2 (DataRepresentation: default `[XCDR]`,
//! XCDR = 0, XCDR2 = 2), §7.6.5 and Annex E (the built-in types), §7.6.8
//! (key holder, KeyHash); the COMPLETE TypeObject CycloneDDS 11.0.1 `idlc`
//! writes for `module to { @final struct Plain { long a; short b; }; }`
//! (`crates/types/tests/typeobject_vendor_dheader.rs`); MD5 of key holders
//! computed with Python's `hashlib`; the rustdoc of `ConfigSource`,
//! `RuntimeConfig::ros_defaults`, `next_process_unique`, `host_id_bytes`,
//! `SampleBytes`, `read_body_representation` and `TypeSupport::register_type`.
//! No socket is opened.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use zerodds_dcps::builtin_topics::TopicBuiltinTopicData;
use zerodds_dcps::builtin_type_set::{
    NAME_DDS_BYTES, NAME_DDS_KEYED_BYTES, NAME_DDS_KEYED_STRING, NAME_DDS_STRING,
};
use zerodds_dcps::builtin_types::{
    BytesTypeSupport, DdsBytes, DdsKeyedBytes, DdsKeyedString, DdsString, KeyedBytesTypeSupport,
    KeyedStringTypeSupport, StringTypeSupport, TypeSupport,
};
use zerodds_dcps::dds_type::{DdsType, Extensibility, PlainCdr2BeKeyHolder};
use zerodds_dcps::factory::{DomainParticipantFactory, DomainParticipantFactoryQos};
use zerodds_dcps::instance_handle::{HANDLE_NIL, InstanceHandle};
use zerodds_dcps::instance_tracker::InstanceTracker;
use zerodds_dcps::participant::{
    host_id_bytes, namespaced_pid, next_process_unique, pid_namespace_inode,
};
use zerodds_dcps::psm_constants::{LENGTH_UNLIMITED, qos_policy_id, status};
use zerodds_dcps::qos::{
    DataReaderQos, DataWriterQos, DestinationOrderKind, DomainParticipantQos, DurabilityKind,
    HistoryKind, LivelinessKind, OwnershipKind, PresentationAccessScope, PublisherQos,
    ReliabilityKind, SubscriberQos, TopicQos, immutable_reader_change, immutable_writer_change,
};
use zerodds_dcps::runtime::{ConfigSource, RuntimeConfig};
use zerodds_dcps::sample_bytes::{SampleBytes, SampleWire};
use zerodds_dcps::sample_info::{
    InstanceStateKind, SampleInfo, SampleStateKind, ViewStateKind, compute_ranks,
};
use zerodds_dcps::serialized_type::{SerializedTypeSupport, read_body_representation};
use zerodds_dcps::status::{QosPolicyCount, SampleRejectedStatusKind};
use zerodds_dcps::time::{Time, he_timestamp_to_time, time_to_he_timestamp};
use zerodds_dcps::{DdsError, FilterValue};
use zerodds_qos::Duration;
use zerodds_rtps::header_extension::HeTimestamp;
use zerodds_rtps::inline_qos::SampleIdentityBytes;
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_types::type_object::flags::{StructMemberFlag, StructTypeFlag};
use zerodds_types::type_object::{CompleteTypeObject, TypeObject};

/// CycloneDDS's COMPLETE `TypeObject` of `to::Plain`.
const PLAIN_COMPLETE: &str = "4c000000f251010012000000000000000a000000746f3a3a506c61696e0000002c0000000200000010000000000000000100040002000000610000001000000001000000010003000200000062000000";

/// MD5 of the key holder `00000002 6b00` (the string "k", PLAIN_CDR2
/// big-endian), computed with Python's `hashlib`.
const MD5_OF_KEY_K: &str = "302cf0a69dae0d4732bd21ea86fec48d";

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn encoded<T: DdsType>(value: &T) -> Vec<u8> {
    let mut out = Vec::new();
    value.encode(&mut out).unwrap();
    out
}

/// A configuration file of its own for one test.
fn config_file(test: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "zerodds-c-api-contract-dcps-{}-{test}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("zerodds.yaml");
    std::fs::write(&path, text).unwrap();
    path
}

fn remove_config(path: &std::path::Path) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

// ---------------------------------------------------------------------------
// QoS
// ---------------------------------------------------------------------------

#[test]
fn dcps_2_2_3_the_defaults_the_c_api_starts_from_are_the_dds_defaults() {
    // `zerodds_endpoint_qos_default`: "RELIABLE, VOLATILE, KEEP_LAST(1),
    // infinite deadline, lifespan and lease, AUTOMATIC liveliness, no
    // RESOURCE_LIMITS, and a writer's max_blocking_time of 100 ms".
    let w = DataWriterQos::default();
    assert_eq!(w.reliability.kind, ReliabilityKind::Reliable); // §2.2.3.14
    assert_eq!(w.reliability.max_blocking_time, Duration::from_millis(100));
    assert_eq!(w.durability.kind, DurabilityKind::Volatile); // §2.2.3.4
    assert_eq!(w.history.kind, HistoryKind::KeepLast); // §2.2.3.18
    assert_eq!(w.history.depth, 1);
    assert_eq!(w.deadline.period, Duration::INFINITE); // §2.2.3.7
    assert_eq!(w.lifespan.duration, Duration::INFINITE); // §2.2.3.16
    assert_eq!(w.liveliness.kind, LivelinessKind::Automatic); // §2.2.3.11
    assert_eq!(w.liveliness.lease_duration, Duration::INFINITE);
    assert_eq!(w.ownership.kind, OwnershipKind::Shared); // §2.2.3.9
    assert_eq!(
        w.destination_order.kind,
        DestinationOrderKind::ByReceptionTimestamp
    ); // §2.2.3.17
    // §2.2.3.19 and §2.3.3: LENGTH_UNLIMITED = -1.
    assert_eq!(LENGTH_UNLIMITED, -1);
    assert_eq!(w.resource_limits.max_samples, LENGTH_UNLIMITED);
    assert_eq!(w.resource_limits.max_instances, LENGTH_UNLIMITED);
    assert_eq!(w.resource_limits.max_samples_per_instance, LENGTH_UNLIMITED);
    let r = DataReaderQos::default();
    assert_eq!(r.reliability.kind, ReliabilityKind::BestEffort); // §2.2.3.14
    assert_eq!(r.history.depth, 1);
    assert_eq!(r.resource_limits.max_samples, LENGTH_UNLIMITED);
    let t = TopicQos::default();
    assert_eq!(t.reliability.kind, ReliabilityKind::BestEffort); // "for DataReaders and Topics"
    assert_eq!(t.durability.kind, DurabilityKind::Volatile);
    assert_eq!(t.history.kind, HistoryKind::KeepLast);
    // §2.2.3.6 PRESENTATION: INSTANCE, no coherent or ordered access;
    // §2.2.3.13 PARTITION: none; §2.2.3.20 ENTITY_FACTORY: TRUE.
    for (presentation, partition, entity_factory) in [
        (
            PublisherQos::default().presentation,
            PublisherQos::default().partition,
            PublisherQos::default().entity_factory,
        ),
        (
            SubscriberQos::default().presentation,
            SubscriberQos::default().partition,
            SubscriberQos::default().entity_factory,
        ),
    ] {
        assert_eq!(presentation.access_scope, PresentationAccessScope::Instance);
        assert!(!presentation.coherent_access);
        assert!(!presentation.ordered_access);
        assert!(partition.names.is_empty());
        assert!(entity_factory.autoenable_created_entities);
    }
    assert!(
        DomainParticipantQos::default()
            .entity_factory
            .autoenable_created_entities
    );
    assert!(DomainParticipantFactoryQos::default().autoenable_created_entities);
}

#[test]
fn dcps_2_2_3_set_qos_after_enable_refuses_exactly_the_policies_marked_not_changeable() {
    // §2.2.3: "a policy with 'changeable' setting of 'NO' is considered
    // 'immutable' and can only be specified either at Entity creation time
    // or else prior to calling the enable operation"; XTypes 1.3 §7.6.3.1.2
    // and §7.6.3.4.4 for DATA_REPRESENTATION and TYPE_CONSISTENCY.
    let writer = DataWriterQos::default();
    let writer_change = |change: fn(&mut DataWriterQos)| {
        let mut next = writer.clone();
        change(&mut next);
        immutable_writer_change(&writer, &next)
    };
    let not_changeable: [fn(&mut DataWriterQos); 10] = [
        |q| q.durability.kind = DurabilityKind::TransientLocal,
        |q| q.reliability.kind = ReliabilityKind::BestEffort,
        |q| q.history.kind = HistoryKind::KeepAll,
        |q| q.resource_limits.max_samples = 10,
        |q| q.ownership.kind = OwnershipKind::Exclusive,
        |q| q.liveliness.kind = LivelinessKind::ManualByTopic,
        |q| q.destination_order.kind = DestinationOrderKind::BySourceTimestamp,
        |q| q.presentation.access_scope = PresentationAccessScope::Topic,
        |q| q.durability_service.history_depth = 5,
        |q| q.data_representation = Some(vec![2]),
    ];
    for (i, change) in not_changeable.into_iter().enumerate() {
        assert!(writer_change(change).is_some(), "writer policy {i}");
    }
    let changeable: [fn(&mut DataWriterQos); 9] = [
        |q| q.deadline.period = Duration::from_secs(1),
        |q| q.latency_budget.duration = Duration::from_millis(5),
        |q| q.ownership_strength.value = 3,
        |q| q.transport_priority.value = 7,
        |q| q.lifespan.duration = Duration::from_secs(10),
        |q| q.user_data.value = vec![1],
        |q| q.partition.names = vec!["a".into()],
        |q| q.writer_data_lifecycle.autodispose_unregistered_instances = false,
        |q| q.group_data.value = vec![2],
    ];
    for (i, change) in changeable.into_iter().enumerate() {
        assert_eq!(writer_change(change), None, "writer policy {i}");
    }

    let reader = DataReaderQos::default();
    let reader_change = |change: fn(&mut DataReaderQos)| {
        let mut next = reader.clone();
        change(&mut next);
        immutable_reader_change(&reader, &next)
    };
    let not_changeable: [fn(&mut DataReaderQos); 10] = [
        |q| q.durability.kind = DurabilityKind::TransientLocal,
        |q| q.reliability.kind = ReliabilityKind::Reliable,
        |q| q.history.kind = HistoryKind::KeepAll,
        |q| q.resource_limits.max_instances = 3,
        |q| q.ownership.kind = OwnershipKind::Exclusive,
        |q| q.liveliness.kind = LivelinessKind::ManualByParticipant,
        |q| q.destination_order.kind = DestinationOrderKind::BySourceTimestamp,
        |q| q.presentation.coherent_access = true,
        |q| q.data_representation = Some(vec![0]),
        |q| q.type_consistency.force_type_validation = true,
    ];
    for (i, change) in not_changeable.into_iter().enumerate() {
        assert!(reader_change(change).is_some(), "reader policy {i}");
    }
    let changeable: [fn(&mut DataReaderQos); 6] = [
        |q| q.deadline.period = Duration::from_secs(1),
        |q| q.latency_budget.duration = Duration::from_millis(5),
        |q| q.time_based_filter.minimum_separation = Duration::from_millis(10),
        |q| q.partition.names = vec!["a".into()],
        |q| q.reader_data_lifecycle.autopurge_disposed_samples_delay = Duration::from_secs(1),
        |q| q.user_data.value = vec![1],
    ];
    for (i, change) in changeable.into_iter().enumerate() {
        assert_eq!(reader_change(change), None, "reader policy {i}");
    }
}

// ---------------------------------------------------------------------------
// PSM values handed to C
// ---------------------------------------------------------------------------

#[test]
fn dcps_2_3_3_status_bits_and_policy_ids_have_the_psm_values() {
    // `*_STATUS = 0x0001 << n`, n = 0, 1, 2, 5 … 14.
    for (bit, value) in [
        (status::INCONSISTENT_TOPIC, 0x0001),
        (status::OFFERED_DEADLINE_MISSED, 0x0002),
        (status::REQUESTED_DEADLINE_MISSED, 0x0004),
        (status::OFFERED_INCOMPATIBLE_QOS, 0x0020),
        (status::REQUESTED_INCOMPATIBLE_QOS, 0x0040),
        (status::SAMPLE_LOST, 0x0080),
        (status::SAMPLE_REJECTED, 0x0100),
        (status::DATA_ON_READERS, 0x0200),
        (status::DATA_AVAILABLE, 0x0400),
        (status::LIVELINESS_LOST, 0x0800),
        (status::LIVELINESS_CHANGED, 0x1000),
        (status::PUBLICATION_MATCHED, 0x2000),
        (status::SUBSCRIPTION_MATCHED, 0x4000),
    ] {
        assert_eq!(bit, value);
    }
    // `*_QOS_POLICY_ID` of the IDL PSM; 23 and 24 from DDS-XTypes 1.3.
    let ids = [
        qos_policy_id::INVALID,
        qos_policy_id::USER_DATA,
        qos_policy_id::DURABILITY,
        qos_policy_id::PRESENTATION,
        qos_policy_id::DEADLINE,
        qos_policy_id::LATENCY_BUDGET,
        qos_policy_id::OWNERSHIP,
        qos_policy_id::OWNERSHIP_STRENGTH,
        qos_policy_id::LIVELINESS,
        qos_policy_id::TIME_BASED_FILTER,
        qos_policy_id::PARTITION,
        qos_policy_id::RELIABILITY,
        qos_policy_id::DESTINATION_ORDER,
        qos_policy_id::HISTORY,
        qos_policy_id::RESOURCE_LIMITS,
        qos_policy_id::ENTITY_FACTORY,
        qos_policy_id::WRITER_DATA_LIFECYCLE,
        qos_policy_id::READER_DATA_LIFECYCLE,
        qos_policy_id::TOPIC_DATA,
        qos_policy_id::GROUP_DATA,
        qos_policy_id::TRANSPORT_PRIORITY,
        qos_policy_id::LIFESPAN,
        qos_policy_id::DURABILITY_SERVICE,
        qos_policy_id::DATA_REPRESENTATION,
        qos_policy_id::TYPE_CONSISTENCY_ENFORCEMENT,
    ];
    for (expected, id) in (0u32..).zip(ids) {
        assert_eq!(id, expected);
    }
    let count = QosPolicyCount::new(qos_policy_id::RELIABILITY, 2);
    assert_eq!((count.policy_id, count.count), (11, 2));
    // §2.2.4.1: NOT_REJECTED is the reason before any rejection; the C
    // mapping 0..=3 follows the order of the enumeration.
    assert_eq!(
        SampleRejectedStatusKind::default(),
        SampleRejectedStatusKind::NotRejected
    );
    let c_value = |kind: SampleRejectedStatusKind| match kind {
        SampleRejectedStatusKind::NotRejected => 0,
        SampleRejectedStatusKind::RejectedByInstancesLimit => 1,
        SampleRejectedStatusKind::RejectedBySamplesLimit => 2,
        SampleRejectedStatusKind::RejectedBySamplesPerInstanceLimit => 3,
    };
    assert_eq!(
        c_value(SampleRejectedStatusKind::RejectedBySamplesPerInstanceLimit),
        3
    );
}

#[test]
fn dcps_2_3_3_a_c_handle_is_the_u64_and_zero_is_handle_nil() {
    for raw in [1, 0x0102_0304_0506_0708, u64::MAX] {
        let handle = InstanceHandle::from_raw(raw);
        assert_eq!(handle.as_raw(), raw);
        assert!(!handle.is_nil());
    }
    assert_eq!(HANDLE_NIL.as_raw(), 0);
    assert!(InstanceHandle::from_raw(0).is_nil());
}

// ---------------------------------------------------------------------------
// Instances
// ---------------------------------------------------------------------------

const KEY_A: [u8; 16] = [0xa; 16];
const KEY_B: [u8; 16] = [0xb; 16];

#[test]
fn dcps_2_2_2_4_2_5_a_registered_key_keeps_its_handle() {
    // "The operation register_instance is idempotent. If it is called for an
    // already registered instance, it just returns the already allocated
    // handle."
    let tracker = InstanceTracker::new();
    let a = tracker.register(KEY_A, vec![1], None);
    assert!(!a.is_nil());
    assert_eq!(tracker.register(KEY_A, vec![1], None), a);
    let b = tracker.register(KEY_B, vec![2], None);
    assert_ne!(a, b);
    assert_eq!(tracker.len(), 2);
    // §2.2.2.4.2.14 lookup_instance, and the key back from the handle.
    assert_eq!(tracker.lookup(&KEY_A), Some(a));
    assert_eq!(tracker.lookup(&[0; 16]), None);
    assert_eq!(tracker.key_hash_for_handle(a), Some(KEY_A));
    assert_eq!(tracker.key_hash_for_handle(HANDLE_NIL), None);
}

#[test]
fn dcps_2_2_2_5_1_5_an_instance_that_comes_back_counts_a_generation() {
    let tracker = InstanceTracker::new();
    let a = tracker.register(KEY_A, Vec::new(), None);
    let alive = |h| tracker.get_by_handle(h).unwrap();
    assert_eq!(alive(a).kind, InstanceStateKind::Alive);
    // §2.2.2.4.2.13: dispose makes the instance NOT_ALIVE_DISPOSED;
    // registering it again brings it back and counts a disposed generation.
    assert!(tracker.dispose(a, None));
    assert_eq!(alive(a).kind, InstanceStateKind::NotAliveDisposed);
    assert_eq!(tracker.register(KEY_A, Vec::new(), None), a);
    assert_eq!(alive(a).kind, InstanceStateKind::Alive);
    assert_eq!(alive(a).disposed_generation_count, 1);
    assert_eq!(alive(a).no_writers_generation_count, 0);
    // §2.2.2.4.2.7: the last writer that unregisters leaves it
    // NOT_ALIVE_NO_WRITERS; coming back counts a no-writers generation.
    let b = tracker.register(KEY_B, Vec::new(), None);
    assert!(tracker.unregister(b, None));
    assert_eq!(alive(b).kind, InstanceStateKind::NotAliveNoWriters);
    tracker.register(KEY_B, Vec::new(), None);
    assert_eq!(alive(b).no_writers_generation_count, 1);
    // A handle no instance has is refused, not invented.
    assert!(!tracker.dispose(InstanceHandle::from_raw(u64::MAX), None));
}

#[test]
fn dcps_2_2_3_19_max_instances_bounds_the_new_instances_only() {
    let tracker = InstanceTracker::new();
    let a = tracker.ensure_registered_limited(KEY_A, 1).unwrap();
    assert_eq!(tracker.ensure_registered_limited(KEY_B, 1), None);
    // A known instance is no new one.
    assert_eq!(tracker.ensure_registered_limited(KEY_A, 1), Some(a));
    // LENGTH_UNLIMITED.
    assert!(
        tracker
            .ensure_registered_limited(KEY_B, LENGTH_UNLIMITED)
            .is_some()
    );
    assert_eq!(tracker.len(), 2);
}

#[test]
fn dcps_2_2_2_5_1_3_an_instance_no_live_writer_writes_is_not_alive_no_writers() {
    // "NOT_ALIVE_NO_WRITERS … indicates the instance has been declared as
    // not-alive by the DataReader because it detected that there are no live
    // DataWriter objects writing that instance."
    let tracker = InstanceTracker::new();
    let a = tracker
        .ensure_registered_limited(KEY_A, LENGTH_UNLIMITED)
        .unwrap();
    tracker.note_writer(a, [1; 16]);
    tracker.note_writer(a, [2; 16]);
    tracker.mark_view_seen(a);
    assert!(tracker.drop_writer(&[1; 16], None).is_empty());
    assert_eq!(
        tracker.get_by_handle(a).unwrap().kind,
        InstanceStateKind::Alive
    );
    assert_eq!(tracker.drop_writer(&[2; 16], None), vec![a]);
    assert_eq!(
        tracker.get_by_handle(a).unwrap().kind,
        InstanceStateKind::NotAliveNoWriters
    );
}

#[test]
fn dcps_2_2_5_a_dcpstopic_entry_is_one_per_participant_topic_and_type() {
    let p1 = GuidPrefix::from_bytes([1; 12]);
    let p2 = GuidPrefix::from_bytes([2; 12]);
    let key = TopicBuiltinTopicData::synthesize_key(p1, "Square", "ShapeType");
    assert_eq!(
        key,
        TopicBuiltinTopicData::synthesize_key(p1, "Square", "ShapeType")
    );
    for other in [
        TopicBuiltinTopicData::synthesize_key(p2, "Square", "ShapeType"),
        TopicBuiltinTopicData::synthesize_key(p1, "Circle", "ShapeType"),
        TopicBuiltinTopicData::synthesize_key(p1, "Square", "Other"),
    ] {
        assert_ne!(key, other);
        assert_ne!(
            InstanceHandle::from_guid(key),
            InstanceHandle::from_guid(other)
        );
    }
}

// ---------------------------------------------------------------------------
// Samples
// ---------------------------------------------------------------------------

#[test]
fn rtps_9_3_2_1_a_source_timestamp_crosses_the_c_api_as_time_t() {
    // fraction in 2^-32 s: a quarter second is 2^30.
    let t = Time::new(2, 250_000_000);
    let ts = time_to_he_timestamp(t);
    assert_eq!((ts.seconds, ts.fraction), (2, 1 << 30));
    assert_eq!(he_timestamp_to_time(ts), t);
    // "TIME_INVALID: seconds = 0xffffffff, fraction = 0xffffffff;
    // TIME_INFINITE: seconds = 0xffffffff, fraction = 0xfffffffe".
    assert_eq!(
        time_to_he_timestamp(Time::INVALID),
        HeTimestamp::TIME_INVALID
    );
    assert_eq!(
        (
            HeTimestamp::TIME_INVALID.seconds,
            HeTimestamp::TIME_INVALID.fraction
        ),
        (0xFFFF_FFFF, 0xFFFF_FFFF)
    );
    assert_eq!(
        he_timestamp_to_time(HeTimestamp::TIME_INVALID),
        Time::INVALID
    );
    assert_eq!(
        time_to_he_timestamp(Time::INFINITE),
        HeTimestamp {
            seconds: 0xFFFF_FFFF,
            fraction: 0xFFFF_FFFE
        }
    );
    // DDS 1.4 §2.3.3: TIME_INVALID_SEC = -1, TIME_INVALID_NSEC = 0xffffffff.
    assert_eq!(
        (Time::INVALID.sec, Time::INVALID.nanosec),
        (-1, 0xFFFF_FFFF)
    );
    // DDS 1.4 §2.3.2: "0 <= nanosec < 1000000000": a C caller's surplus
    // nanoseconds carry into the seconds.
    let carried = Time::new(1, 1_500_000_000);
    assert_eq!((carried.sec, carried.nanosec), (2, 500_000_000));
}

#[test]
fn dcps_2_2_2_5_1_6_the_ranks_of_a_collection_follow_their_definitions() {
    let a = InstanceHandle::from_raw(1);
    let b = InstanceHandle::from_raw(2);
    let sample = |instance, disposed| SampleInfo {
        instance_handle: instance,
        disposed_generation_count: disposed,
        ..SampleInfo::default()
    };
    // A (generation 0), B, A and A after the instance came back once.
    let mut infos = [sample(a, 0), sample(b, 0), sample(a, 1), sample(a, 1)];
    // The reader's current counts: A came back twice by now.
    compute_ranks(&mut infos, |h| if h == a { (2, 0) } else { (0, 0) });
    // "The sample_rank indicates the number of samples of the same instance
    // that follow the current one in the collection."
    let ranks: Vec<i32> = infos.iter().map(|i| i.sample_rank).collect();
    assert_eq!(ranks, [2, 0, 1, 0]);
    // "generation_rank = (MRSIC.disposed_generation_count +
    // MRSIC.no_writers_generation_count) - (S.disposed_generation_count +
    // S.no_writers_generation_count)", MRSIC the most recent sample of the
    // instance in the collection.
    let generation: Vec<i32> = infos.iter().map(|i| i.generation_rank).collect();
    assert_eq!(generation, [1, 0, 0, 0]);
    // absolute_generation_rank: the same against MRS, the most recent sample
    // of the instance the reader received.
    let absolute: Vec<i32> = infos.iter().map(|i| i.absolute_generation_rank).collect();
    assert_eq!(absolute, [2, 0, 1, 1]);
}

#[test]
fn dcps_2_2_2_5_1_a_fresh_sample_info_is_not_read_new_alive_and_names_no_handle() {
    // The info `subscriber_ffi.rs` completes with `..SampleInfo::default()`:
    // a sample never read (§2.2.2.5.1.2), of a new instance (§2.2.2.5.1.8),
    // alive (§2.2.2.5.1.3), with data.
    let info = SampleInfo::default();
    assert_eq!(info.sample_state, SampleStateKind::NotRead);
    assert_eq!(info.view_state, ViewStateKind::New);
    assert_eq!(info.instance_state, InstanceStateKind::Alive);
    assert!(info.valid_data);
    assert_eq!(info.instance_handle, HANDLE_NIL);
    assert_eq!(info.publication_handle, HANDLE_NIL);
    assert_eq!(
        (
            info.disposed_generation_count,
            info.no_writers_generation_count
        ),
        (0, 0)
    );
}

#[test]
fn a_received_payload_keeps_its_octets_and_where_it_came_from() {
    // `SampleBytes`: refcounted, a clone or a slice is "Zero-copy"; the C
    // read loan hands out the pointer of the view.
    let wire = SampleWire {
        writer_guid: [7; 16],
        sequence_number: 9,
        related_sample_identity: Some(SampleIdentityBytes::new([3; 16], 4)),
        group: None,
    };
    let bytes = SampleBytes::from_vec(vec![1, 2, 3, 4]).with_wire(wire);
    assert_eq!(bytes.as_slice(), [1, 2, 3, 4]);
    assert_eq!(bytes.wire(), Some(&wire));
    let clone = bytes.clone();
    assert_eq!(clone.as_slice().as_ptr(), bytes.as_slice().as_ptr());
    let middle = bytes.slice(1..3);
    assert_eq!(middle.as_slice(), [2, 3]);
    assert_eq!(middle.as_slice().as_ptr(), bytes.as_slice()[1..].as_ptr());
    assert_eq!(middle.wire(), Some(&wire));
}

// ---------------------------------------------------------------------------
// Built-in types
// ---------------------------------------------------------------------------

#[test]
fn xtypes_7_6_5_the_built_in_types_have_their_spec_names() {
    assert_eq!(NAME_DDS_STRING, "DDS::String");
    assert_eq!(NAME_DDS_KEYED_STRING, "DDS::KeyedString");
    assert_eq!(NAME_DDS_BYTES, "DDS::Bytes");
    assert_eq!(NAME_DDS_KEYED_BYTES, "DDS::KeyedBytes");
    assert_eq!(DdsString::TYPE_NAME, NAME_DDS_STRING);
    assert_eq!(DdsKeyedString::TYPE_NAME, NAME_DDS_KEYED_STRING);
    assert_eq!(DdsBytes::TYPE_NAME, NAME_DDS_BYTES);
    assert_eq!(DdsKeyedBytes::TYPE_NAME, NAME_DDS_KEYED_BYTES);
    assert_eq!(StringTypeSupport.get_type_name(), NAME_DDS_STRING);
    assert_eq!(
        KeyedStringTypeSupport.get_type_name(),
        NAME_DDS_KEYED_STRING
    );
    assert_eq!(BytesTypeSupport.get_type_name(), NAME_DDS_BYTES);
    assert_eq!(KeyedBytesTypeSupport.get_type_name(), NAME_DDS_KEYED_BYTES);
    assert!(!DdsString::has_key());
    assert!(!DdsBytes::has_key());
    assert!(DdsKeyedString::has_key());
    assert!(DdsKeyedBytes::has_key());
}

#[test]
fn xtypes_7_6_5_the_built_in_types_are_appendable_on_the_wire() {
    // XCDR2 (DELIMITED_CDR2): a DHEADER with the length of the members; a
    // string is its length with the NUL, the octets and the NUL; members
    // aligned to four; a sequence of octets is its length and the octets.
    assert_eq!(
        encoded(&DdsString::new("hi")),
        [0x07, 0, 0, 0, 0x03, 0, 0, 0, b'h', b'i', 0x00]
    );
    let keyed = DdsKeyedString::new("k", "v");
    #[rustfmt::skip]
    let keyed_octets = [
        0x0e, 0, 0, 0,
        0x02, 0, 0, 0, b'k', 0x00, 0x00, 0x00,
        0x02, 0, 0, 0, b'v', 0x00,
    ];
    assert_eq!(encoded(&keyed), keyed_octets);
    assert_eq!(DdsKeyedString::decode(&keyed_octets).unwrap(), keyed);
    assert_eq!(
        encoded(&DdsBytes::new(vec![1, 2, 3])),
        [0x07, 0, 0, 0, 0x03, 0, 0, 0, 1, 2, 3]
    );
    #[rustfmt::skip]
    let keyed_bytes = [
        0x0d, 0, 0, 0,
        0x02, 0, 0, 0, b'k', 0x00, 0x00, 0x00,
        0x01, 0, 0, 0, 0x09,
    ];
    assert_eq!(encoded(&DdsKeyedBytes::new("k", vec![9])), keyed_bytes);
    // XCDR1 (Table 60, PLAIN_CDR for an appendable type): the members alone.
    let mut xcdr1 = Vec::new();
    DdsString::new("hi").encode_xcdr1(&mut xcdr1).unwrap();
    assert_eq!(xcdr1, [0x03, 0, 0, 0, b'h', b'i', 0x00]);
    // Big-endian XCDR2: every length reversed.
    let mut big = Vec::new();
    DdsString::new("hi").encode_be(&mut big).unwrap();
    assert_eq!(big, [0, 0, 0, 0x07, 0, 0, 0, 0x03, b'h', b'i', 0x00]);
}

#[test]
fn xtypes_7_6_8_the_key_of_a_keyed_built_in_type_is_its_key_string() {
    // §7.6.8: the key holder is the key member as PLAIN_CDR2 big-endian; an
    // unbounded string can exceed 16 octets, so the KeyHash is its MD5.
    let keyed = DdsKeyedString::new("k", "anything");
    let mut holder = PlainCdr2BeKeyHolder::new();
    keyed.encode_key_holder_be(&mut holder);
    assert_eq!(holder.as_bytes(), [0, 0, 0, 0x02, b'k', 0x00]);
    let md5 = unhex(MD5_OF_KEY_K);
    assert_eq!(
        keyed.compute_key_hash().map(|h| h.to_vec()),
        Some(md5.clone())
    );
    // The payload is not part of the key; the bytes type keys the same way.
    assert_eq!(
        DdsKeyedString::new("k", "else").compute_key_hash(),
        keyed.compute_key_hash()
    );
    assert_eq!(
        DdsKeyedBytes::new("k", vec![1])
            .compute_key_hash()
            .map(|h| h.to_vec()),
        Some(md5)
    );
    // `get_key_value`: the holder read back gives the key.
    assert_eq!(
        DdsKeyedString::decode_key_holder_be(holder.as_bytes())
            .unwrap()
            .key,
        "k"
    );
    assert_eq!(DdsString::new("x").compute_key_hash(), None);
}

#[test]
fn xtypes_7_6_5_the_type_objects_declare_appendable_structures_and_the_key() {
    for (object, key) in [
        (DdsString::type_object(), None),
        (DdsKeyedString::type_object(), Some("key")),
        (DdsBytes::type_object(), None),
        (DdsKeyedBytes::type_object(), Some("key")),
    ] {
        let Some(TypeObject::Complete(CompleteTypeObject::Struct(structure))) = &object else {
            panic!("a COMPLETE structure: {object:?}");
        };
        assert!(structure.struct_flags.has(StructTypeFlag::IS_APPENDABLE));
        let keys: Vec<&str> = structure
            .member_seq
            .iter()
            .filter(|m| m.common.member_flags.has(StructMemberFlag::IS_KEY))
            .map(|m| m.detail.name.as_str())
            .collect();
        assert_eq!(keys, key.into_iter().collect::<Vec<_>>());
    }
}

#[test]
fn a_built_in_type_registers_under_its_name_once_and_no_other_type_takes_the_name() {
    // `TypeSupport::register_type`: "registering one under its own name
    // again is not an error"; "PreconditionNotMet when another type is
    // registered under the name; BadParameter for a name that is no name".
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    StringTypeSupport.register_type(&p, None).unwrap();
    StringTypeSupport.register_type(&p, None).unwrap();
    StringTypeSupport
        .register_type(&p, Some(NAME_DDS_STRING))
        .unwrap();
    assert!(matches!(
        KeyedStringTypeSupport.register_type(&p, Some(NAME_DDS_STRING)),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    assert!(matches!(
        BytesTypeSupport.register_type(&p, Some("")),
        Err(DdsError::BadParameter { .. })
    ));
    KeyedBytesTypeSupport.register_type(&p, None).unwrap();
}

// ---------------------------------------------------------------------------
// Descriptor support
// ---------------------------------------------------------------------------

#[test]
fn xtypes_7_6_3_1_2_a_received_representation_tag_names_the_dds_representation() {
    // A `UserSample::Alive` carries 0 for XCDR1 and 1 for XCDR2;
    // `read_body` takes the DataRepresentationId, XCDR = 0, XCDR2 = 2.
    assert_eq!(read_body_representation(0), 0);
    assert_eq!(read_body_representation(1), 2);
}

#[test]
fn xtypes_7_4_3_a_descriptor_type_reads_the_body_and_answers_the_filter() {
    // `zerodds_cft_set_type_object`: the root TypeObject, keyless, @final
    // (ExtensibilityKind FINAL = 0).
    let object = unhex(PLAIN_COMPLETE);
    let support = SerializedTypeSupport::from_type_object("to::Plain", &object, false, 0).unwrap();
    assert_eq!(support.extensibility(), Extensibility::Final);
    assert!(!support.is_keyed());
    // PLAIN_CDR2 of `a = 1, b = -2`.
    let body = [0x01, 0x00, 0x00, 0x00, 0xfe, 0xff];
    let sample = support
        .read_body(&body, read_body_representation(1), false)
        .unwrap();
    assert_eq!(support.key_material(&sample), None);
    // The content filter of a C reader names the members.
    assert_eq!(sample.field_value("a"), Some(FilterValue::Int(1)));
    assert_eq!(sample.field_value("b"), Some(FilterValue::Int(-2)));
    assert_eq!(sample.field_value("c"), None);
    // Flags the object contradicts are a BadParameter to C.
    assert!(SerializedTypeSupport::from_type_object("to::Plain", &object, true, 0).is_err());
    assert!(SerializedTypeSupport::from_type_object("to::Plain", &object, false, 2).is_err());
    assert!(SerializedTypeSupport::from_type_object("to::Plain", &object[..40], false, 0).is_err());
}

// ---------------------------------------------------------------------------
// Configuration and the GUID prefix
// ---------------------------------------------------------------------------

#[test]
fn xtypes_7_6_3_1_1_the_profiles_offer_xcdr_first_and_default_to_xcdr() {
    // "the default value is an empty sequence which is equivalent to a
    // sequence containing the single element XCDR".
    assert_eq!(RuntimeConfig::default().data_representation_offer, [0]);
    // `ros_defaults`: "data_representation_offer = [XCDR1, XCDR2]".
    assert_eq!(
        RuntimeConfig::ros_defaults().data_representation_offer,
        [0, 2]
    );
}

#[test]
fn a_file_setting_reaches_the_offer_and_an_unknown_set_path_is_refused() {
    let path = config_file(
        "offer",
        "wire:\n  data_representation_offer:\n    - XCDR2\n",
    );
    // `ffi_config_from_source` asks the source for the setting by its path,
    // then reads the typed configuration.
    let source = ConfigSource::from_path(&path).unwrap();
    assert_eq!(
        source.var("wire.data_representation_offer").as_deref(),
        Some("XCDR2")
    );
    assert_eq!(
        RuntimeConfig::from_source(&source)
            .unwrap()
            .data_representation_offer,
        [2]
    );
    // `from_command_line`: "a `--set` path the runtime does not know — a
    // flag nobody reads is a typo, and starting anyway would drop it."
    let mut command_line = zerodds_config::CommandLine::default().with_config(&path);
    command_line.push_set("transport.recv_bacth=8").unwrap();
    assert!(ConfigSource::from_command_line(&command_line).is_err());
    remove_config(&path);
    // A known one is taken, as the comma list of the setting.
    let other = config_file("set", "transport:\n  recv_batch: 8\n");
    let mut command_line = zerodds_config::CommandLine::default().with_config(&other);
    command_line
        .push_set("wire.data_representation_offer=XCDR1,XCDR2")
        .unwrap();
    let source = ConfigSource::from_command_line(&command_line).unwrap();
    assert_eq!(
        RuntimeConfig::from_source(&source)
            .unwrap()
            .data_representation_offer,
        [0, 2]
    );
    remove_config(&other);
}

#[test]
fn rtps_8_2_4_2_the_prefix_parts_from_dcps_are_one_host_id_and_a_new_tail_per_call() {
    // `host_id_bytes`: "Deterministic 4-byte host identifier … Cached per
    // process".
    assert_eq!(host_id_bytes(), host_id_bytes());
    // `next_process_unique`: the process id (marked with the PID namespace)
    // and a counter that makes "the prefixes of one process pairwise
    // different" — "every Participant in the Domain has a unique GUID".
    let first = next_process_unique();
    let second = next_process_unique();
    let pid = namespaced_pid(std::process::id(), pid_namespace_inode()).to_le_bytes();
    assert_eq!(first[..4], pid);
    assert_eq!(second[..4], pid);
    assert_ne!(first[4..], second[4..]);
}
