// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-omg-xtypes-test-main` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-dcps`:
//! - `src/carrier.rs`: a `DdsType` whose type is known only at run time.
//!   It keeps the constants at their defaults (`HAS_KEY` false,
//!   `KEY_HOLDER_MAX_SIZE` none) and answers the run-time accessors
//!   `type_name`, `type_identifier`, `extensibility`, `has_key` and
//!   `key_holder_max_size` (`Some(16)`, its holder being the finished
//!   sixteen-octet KeyHash), `type_object`, `encode*`/`decode*` over one
//!   stream, `encode_key_holder_be` with `PlainCdr2BeKeyHolder::write_bytes`,
//!   and `DecodeError::Discarded` for a sample the receiving type discards.
//! - `src/app.rs`: `create_topic::<Sample>`, `Topic::set_listener` with
//!   `psm_constants::status::ANY`, `create_publisher`/`create_subscriber`,
//!   `DataWriterQos`/`DataReaderQos { data_representation: Some([0] or
//!   [2]), .. }`, `ReliabilityKind::BestEffort` without `-r`, the reader's
//!   `type_consistency` set from `zerodds_types::qos::
//!   TypeConsistencyEnforcement::default()` for `d`, `DataWriter::write`,
//!   `take_with_info`, `drive_listeners`, `Topic::inconsistent_topic_status`,
//!   `DomainParticipant::register_type_object`.
//! - `src/main.rs`: `runtime::ConfigSource::from_path` and
//!   `RuntimeConfig::from_source` (or `from_standard_paths`), the struct
//!   update `RuntimeConfig { key_member_bounds, strong_assignability,
//!   ..base }`, `create_participant_with_config`,
//!   `set_minimal_type_object_camp`.
//! - `src/listeners.rs`: `listener::{TopicListener, DataWriterListener,
//!   DataReaderListener}` and their `status` structs.
//!
//! What it relies on and is checked here:
//! - The DCPS entities read a type through its run-time accessors, not the
//!   constants: the topic carries `type_name()`, the writer keys instances
//!   by `has_key()`, and the KeyHash follows `key_holder_max_size()` — the
//!   holder itself, zero padded, up to sixteen octets, its MD5 above.
//!   Oracles: the rustdoc of `DdsType::type_name` ("the five run-time
//!   accessors here ... are what the DCPS entities read"); DDSI-RTPS 2.5
//!   §9.6.4.8 Examples 1 and 2 and DDS-XTypes 1.3 §7.6.8 step 5.
//! - The reader default of TYPE_CONSISTENCY_ENFORCEMENT is the policy's
//!   default (DDS-XTypes 1.3 §7.6.3.4.1): ALLOW_TYPE_COERCION, sequence and
//!   string bounds ignored, member names not ignored, widening not
//!   prevented, no forced validation.
//! - The pinned representations (XCDR_DATA_REPRESENTATION 0 and
//!   XCDR2_DATA_REPRESENTATION 2, DDS-XTypes 1.3 Annex D) create their
//!   endpoints, reliable and best effort.
//! - A discarded sample is told apart from a decode failure (DDS-XTypes 1.3
//!   §7.2.4.1, the default of discarding a sample that does not fit).
//! - A runtime file reaches the runtime configuration, `xtypes.key_member_bounds`
//!   among it; a missing file and an invalid value are errors
//!   (`docs/specs/zerodds-deployment-1.0.md` §2.1; `man/man5/zerodds.yaml.5`,
//!   `xtypes.key_member_bounds`: "enforced or ignored", default enforced).
//! - Without a runtime, `set_minimal_type_object_camp` and
//!   `register_type_object` refuse with PRECONDITION_NOT_MET (their
//!   rustdoc).
//!
//! `create_participant_with_config` opens sockets; the participants here
//! have no runtime.
#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use zerodds_dcps::dds_type::{
    DdsType, DecodeError, EncodeError, Extensibility, PlainCdr2BeKeyHolder, compute_key_hash,
};
use zerodds_dcps::listener::TopicListener;
use zerodds_dcps::psm_constants::status;
use zerodds_dcps::qos::{
    DataReaderQos, DataWriterQos, PublisherQos, ReliabilityKind, SubscriberQos, TopicQos,
};
use zerodds_dcps::runtime::{ConfigSource, RuntimeConfig};
use zerodds_dcps::status::InconsistentTopicStatus;
use zerodds_dcps::{
    DdsError, DomainParticipant, DomainParticipantFactory, DomainParticipantQos, InstanceHandle,
};
use zerodds_types::assignability::KeyMemberBounds;
use zerodds_types::builder::TypeObjectBuilder;
use zerodds_types::qos::{TypeConsistencyEnforcement, TypeConsistencyKind};
use zerodds_types::type_object::{CompleteTypeObject, TypeObject};

/// A type described at run time, as the carrier is: the constants stay at
/// their defaults, the accessors answer. `MAX` is the key holder maximum.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RunTime<const MAX: usize> {
    holder: Vec<u8>,
}

impl<const MAX: usize> DdsType for RunTime<MAX> {
    const TYPE_NAME: &'static str = "contract::NotInstalled";

    fn type_name() -> &'static str {
        "contract::RunTimeType"
    }

    fn extensibility() -> Extensibility {
        Extensibility::Appendable
    }

    fn has_key() -> bool {
        true
    }

    fn key_holder_max_size() -> Option<usize> {
        Some(MAX)
    }

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.holder);
        Ok(())
    }

    fn encode_xcdr1(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.holder);
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        Ok(Self {
            holder: bytes.to_vec(),
        })
    }

    fn decode_xcdr1(bytes: &[u8]) -> Result<Self, DecodeError> {
        Self::decode(bytes)
    }

    fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
        holder.write_bytes(&self.holder);
    }
}

fn participant() -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default())
}

/// The KeyHash the writer derives for `holder`.
fn writer_key_hash<const MAX: usize>(topic_name: &str, holder: &[u8]) -> [u8; 16] {
    let p = participant();
    let topic = p
        .create_topic::<RunTime<MAX>>(topic_name, TopicQos::default())
        .unwrap();
    assert_eq!(topic.type_name(), "contract::RunTimeType");
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RunTime<MAX>>(&topic, DataWriterQos::default())
        .unwrap();
    let sample = RunTime::<MAX> {
        holder: holder.to_vec(),
    };
    let handle = writer.register_instance(&sample).unwrap();
    assert!(!handle.is_nil(), "keyed through has_key()");
    assert_eq!(writer.lookup_instance(&sample), handle);
    writer.write(&sample).unwrap();
    writer
        .instance_tracker()
        .key_hash_for_handle(handle)
        .unwrap()
}

#[test]
fn rtps_9_6_4_8_the_key_hash_follows_the_run_time_holder_maximum() {
    // Example 1: the holder {12 34 56 78}, at most 16 octets, padded.
    assert_eq!(
        writer_key_hash::<4>("contract-dcps-example-1", &[0x12, 0x34, 0x56, 0x78]),
        [
            0x12, 0x34, 0x56, 0x78, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00
        ]
    );
    // Example 2: the 20-octet holder of maximum 28 octets, hashed.
    let example_2 = [
        0x00, 0x00, 0x00, 0x05, 0x42, 0x4c, 0x55, 0x45, 0x00, 0x00, 0x00, 0x00, 0x12, 0x34, 0x56,
        0x78, 0x9a, 0xbc, 0xde, 0xf0,
    ];
    let md5 = [
        0xf9, 0x1a, 0x59, 0xe3, 0x2e, 0x45, 0x35, 0xd9, 0xa6, 0x9c, 0xd5, 0xd9, 0xf5, 0xb6, 0xe3,
        0x6e,
    ];
    assert_eq!(
        writer_key_hash::<28>("contract-dcps-example-2", &example_2),
        md5
    );
    assert_eq!(compute_key_hash(&example_2, 28), md5);
    // The carrier: a finished sixteen-octet KeyHash with maximum 16 is
    // taken as it is.
    let finished: [u8; 16] = core::array::from_fn(|i| u8::try_from(i * 17).unwrap());
    assert_eq!(
        writer_key_hash::<16>("contract-dcps-carrier", &finished),
        finished
    );
}

#[test]
fn xtypes_7_6_3_4_1_the_reader_default_is_the_policy_default() {
    let tce = DataReaderQos::default().type_consistency;
    assert_eq!(tce.kind, TypeConsistencyKind::AllowTypeCoercion);
    assert!(tce.ignore_sequence_bounds);
    assert!(tce.ignore_string_bounds);
    assert!(!tce.ignore_member_names);
    assert!(!tce.prevent_type_widening);
    assert!(!tce.force_type_validation);
    // What `d` puts back.
    assert_eq!(tce, TypeConsistencyEnforcement::default());
}

#[test]
fn xtypes_annex_d_the_pinned_representations_create_their_endpoints() {
    let p = participant();
    let topic = p
        .create_topic::<RunTime<16>>("contract-dcps-representations", TopicQos::default())
        .unwrap();
    let inconsistent = Arc::new(Inconsistent::default());
    topic.set_listener(Some(inconsistent.clone()), status::ANY);
    let publisher = p.create_publisher(PublisherQos::default());
    let subscriber = p.create_subscriber(SubscriberQos::default());
    for representation in [0_i16, 2] {
        for reliable in [true, false] {
            let mut wq = DataWriterQos {
                data_representation: Some(vec![representation]),
                ..DataWriterQos::default()
            };
            let mut rq = DataReaderQos {
                data_representation: Some(vec![representation]),
                ..DataReaderQos::default()
            };
            rq.type_consistency.ignore_member_names = true;
            if !reliable {
                wq.reliability.kind = ReliabilityKind::BestEffort;
                rq.reliability.kind = ReliabilityKind::BestEffort;
            }
            let writer = publisher
                .create_datawriter::<RunTime<16>>(&topic, wq)
                .unwrap();
            writer
                .write(&RunTime {
                    holder: vec![7; 16],
                })
                .unwrap();
            writer.drive_listeners();
            let reader = subscriber
                .create_datareader::<RunTime<16>>(&topic, rq)
                .unwrap();
            reader.drive_listeners();
            assert!(reader.take_with_info().unwrap().is_empty());
        }
    }
    assert_eq!(topic.inconsistent_topic_status().total_count, 0);
    assert_eq!(inconsistent.0.load(Ordering::Relaxed), 0);
}

/// Counts `on_inconsistent_topic`, the line the suite reads.
#[derive(Default)]
struct Inconsistent(AtomicU64);

impl TopicListener for Inconsistent {
    fn on_inconsistent_topic(&self, _topic: InstanceHandle, _status: InconsistentTopicStatus) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn xtypes_7_2_4_1_a_discarded_sample_is_not_a_decode_failure() {
    let discarded = DecodeError::Discarded {
        what: "the value does not fit this member",
    };
    let invalid = DecodeError::Invalid { what: "truncated" };
    assert!(discarded.is_discarded());
    assert!(!invalid.is_discarded());
    assert_ne!(discarded.to_string(), invalid.to_string());
}

/// A runtime file of this test alone.
fn config_file(name: &str, text: &str) -> std::path::PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("contract-dcps-xtypes-test-main-{name}.yaml"));
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn a_runtime_file_reaches_the_runtime_configuration() {
    let path = config_file(
        "runtime",
        "reader:\n  max_samples_per_proxy: 4096\nxtypes:\n  key_member_bounds: ignored\n",
    );
    let base = ConfigSource::from_path(path)
        .and_then(|source| RuntimeConfig::from_source(&source))
        .unwrap();
    assert_eq!(base.max_samples_per_proxy, 4096);
    assert_eq!(base.key_member_bounds, KeyMemberBounds::Ignored);
    assert_eq!(
        RuntimeConfig::default().key_member_bounds,
        KeyMemberBounds::Enforced
    );
    let strong = base.strong_assignability;
    let runtime = RuntimeConfig {
        key_member_bounds: KeyMemberBounds::Enforced,
        strong_assignability: strong,
        ..base
    };
    assert_eq!(runtime.key_member_bounds, KeyMemberBounds::Enforced);
    assert_eq!(runtime.max_samples_per_proxy, 4096);

    let invalid = config_file("invalid", "xtypes:\n  key_member_bounds: sometimes\n");
    assert!(
        ConfigSource::from_path(invalid)
            .and_then(|source| RuntimeConfig::from_source(&source))
            .is_err()
    );
    let missing =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join("contract-dcps-xtypes-test-main-absent.yaml");
    let _ = std::fs::remove_file(&missing);
    assert!(ConfigSource::from_path(missing).is_err());
}

#[test]
fn without_a_runtime_the_type_object_calls_refuse() {
    let p = participant();
    assert!(matches!(
        p.set_minimal_type_object_camp(false),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    let object = TypeObject::Complete(CompleteTypeObject::Struct(
        TypeObjectBuilder::struct_type("contract::Registered").build_complete(),
    ));
    assert!(matches!(
        p.register_type_object(object),
        Err(DdsError::PreconditionNotMet { .. })
    ));
}
