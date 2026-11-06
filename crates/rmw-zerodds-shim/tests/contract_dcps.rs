// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `rmw-zerodds-shim` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-dcps`:
//! - `src/ros2_config.rs` (`process_source`) and
//!   `src/bin/zerodds-ros2-shim.rs` (`load`, `check_config`):
//!   `runtime::ConfigSource::for_program` over the command line, the base
//!   name `zerodds`, the key predicate `ros2_config::is_key`, no environment
//!   names of its own and the process environment; `source.tree()` for
//!   `Ros2Config::from_config`, `source.check_report(&is_key)`;
//!   `runtime::RuntimeConfig::from_source`.
//! - `src/dynamic.rs`: `RuntimeConfig::from_source(..).codec_dialect`, the
//!   CDR dialect of the dynamic type support.
//! - `src/lib.rs`: `runtime::observability_sink_from_source` for the spatial
//!   bridge.
//! - `src/spatial_bridge.rs`: `DdsType` implementations of wrappers over the
//!   generated SpatialDDS and ROS 2 types (`TYPE_NAME`, `EXTENSIBILITY =
//!   Appendable`, `encode` / `decode`; the ROS types also `encode_xcdr1` /
//!   `decode_xcdr1`; `dds_type::{EncodeError, DecodeError}::Invalid`);
//!   `RuntimeConfig::from_standard_paths` with `data_representation_offer`
//!   set to `[XCDR2]`; `DomainParticipantFactory::instance()
//!   .create_participant_with_config`, `create_topic`, `create_publisher`,
//!   `create_subscriber`, `create_datawriter` / `create_datareader` with the
//!   QoS of `qos::{DataReaderQos, DataWriterQos, DurabilityKind,
//!   DurabilityQosPolicy, HistoryKind, HistoryQosPolicy, ReliabilityKind,
//!   ReliabilityQosPolicy, ...}` and a per-writer `data_representation` of
//!   `[XCDR]` for the ROS side; `DataReader::wait_for_data`,
//!   `take_with_info`, `DataWriter::write`, `publication_handle`,
//!   `InstanceHandle`; `error::DdsError::{TransportError, Configuration}`.
//! - `src/services.rs`: `runtime::{DcpsRuntime, UserSample,
//!   UserWriterConfig}`; `register_user_writer_kind` with a
//!   `UserWriterConfig` whose `user_data` is set over the C API's config,
//!   `set_user_writer_wire_extensibility(Final)`,
//!   `register_user_reader_kind`, `set_user_reader_arrival_hook`,
//!   `remove_user_writer` / `remove_user_reader`, `guid_prefix`,
//!   `write_user_sample_borrowed`, `write_user_sample_related`; the
//!   `UserSample::Alive` fields `payload`, `representation`, `big_endian` and
//!   `related_sample_identity`, from which the request or reply is handed to
//!   rcl behind an encapsulation header the shim builds again.
//!
//! What it relies on and is checked here without a socket: the `ros2.*`
//! keys taken by `--set` beside the runtime's, the refusal of a `--set` no
//! one reads, the runtime leaving the `ros2.*` section of a file to the RMW,
//! the runtime settings and the codec dialect a file names, the defaults
//! without one, an invalid value refused with its line, the `--check-config`
//! report; the `DdsType` defaults the bridged types run on (the declared
//! extensibility, no XCDR1 form for a SpatialDDS type, no big-endian form);
//! the type name a topic of a bridged type announces; the consistency of
//! the bridge's QoS.
//!
//! Oracles: `docs/specs/zerodds-deployment-1.0.md` §2.1 ("Runtime
//! configuration from RC7 on": the four layers, a `--set` no setting reads is
//! rejected, invalid values name their layer), the rustdoc of
//! `ConfigSource::unknown_keys` (`ros2.*` is left to the RMW), of
//! `RuntimeConfig::codec_dialect` (`wire.codec_dialect`, default
//! `specification`) and of `zerodds_config::Origin`, DDS-XTypes 1.3
//! §7.6.3.1.1 (`XCDR_DATA_REPRESENTATION = 0`, `XCDR2_DATA_REPRESENTATION =
//! 2`, the default `[XCDR]`), Table 60 (APPENDABLE is DELIMITED_CDR in
//! version 2, PLAIN_CDR in version 1) and §7.4.2 (DELIMITED_CDR: a DHEADER
//! that counts the octets after it), the rustdoc of `DdsType` (`type_name`
//! and `extensibility` default to the constants; `encode_xcdr1` /
//! `decode_xcdr1` refuse without `XCDR1_MATCHES_XCDR2`, `encode_be` /
//! `decode_be` without `BYTE_ORDER_AGNOSTIC`), DDS 1.4 §2.2.2.3.1.2
//! (`type_name`) and §2.2.3.18 / §2.2.3.19 (HISTORY depth within
//! RESOURCE_LIMITS).
//!
//! Left to the integration pillar, because only a bound runtime shows it: the
//! service endpoints and their samples (`related_sample_identity`, DDS-RPC
//! 1.0 §7.8.2; the encapsulation header of a delivered sample), the spatial
//! bridge's pumps, and the sink `observability_sink_from_source` selects.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use rmw_zerodds::ros2_config::is_key;
use rmw_zerodds::spatial_bridge::{BridgeGeoPose, BridgeGeoPoseStamped, BridgePoseStamped};
use zerodds_ast_layout::Dialect;
use zerodds_config::{CommandLine, ConfigError, Origin};
use zerodds_dcps::dds_type::{DecodeError, EncodeError, Extensibility};
use zerodds_dcps::qos::{
    DurabilityKind, DurabilityQosPolicy, HistoryKind, HistoryQosPolicy, ReliabilityKind,
    ReliabilityQosPolicy,
};
use zerodds_dcps::runtime::{ConfigSource, RuntimeConfig};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos, TopicQos,
};

/// DDS-XTypes 1.3 §7.6.3.1.1: `XCDR_DATA_REPRESENTATION`.
const XCDR: i16 = 0;
/// DDS-XTypes 1.3 §7.6.3.1.1: `XCDR2_DATA_REPRESENTATION`.
const XCDR2: i16 = 2;

/// A runtime file of this test alone, so that no system file takes part.
fn runtime_file(name: &str, text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "zerodds-rmw-contract-dcps-{}-{name}.yaml",
        std::process::id()
    ));
    std::fs::write(&path, text).unwrap();
    path
}

/// What `process_source` and the diagnostics CLI read, with an empty
/// environment in place of the process's.
fn source(args: &[&str]) -> Result<ConfigSource, ConfigError> {
    let (command_line, _) = CommandLine::extract(args.iter().map(|arg| (*arg).to_string()))?;
    ConfigSource::for_program(&command_line, "zerodds", &is_key, &[], |_| None)
}

fn no_keys(_: &str) -> bool {
    false
}

#[test]
fn the_rmw_keys_are_taken_by_set_beside_the_runtime_keys() {
    let file = runtime_file("set", "\n");
    let path = file.display().to_string();
    let source = source(&[
        "--config",
        &path,
        "--set",
        "ros2.topic_mangling=none",
        "--set",
        "wire.data_representation_offer=XCDR1,XCDR2",
    ])
    .unwrap();
    let tree = source.tree().unwrap();
    assert_eq!(tree.get_str("ros2.topic_mangling"), Some("none"));
    assert_eq!(
        tree.origin_of("ros2.topic_mangling"),
        Some(&Origin::Cli(String::from("--set ros2.topic_mangling=none")))
    );
    let runtime = RuntimeConfig::from_source(&source).unwrap();
    assert_eq!(runtime.data_representation_offer, [XCDR, XCDR2]);
    let _ = std::fs::remove_file(file);
}

#[test]
fn a_set_flag_no_setting_reads_is_refused() {
    let file = runtime_file("unknown-set", "\n");
    let path = file.display().to_string();
    assert!(matches!(
        source(&["--config", &path, "--set", "ros2.no_such_key=1"]),
        Err(ConfigError::Usage(_))
    ));
    let _ = std::fs::remove_file(file);
}

#[test]
fn the_runtime_leaves_the_ros2_section_of_its_files_to_the_rmw() {
    let file = runtime_file(
        "ros2-section",
        "ros2:\n  no_such_key: 1\nwire:\n  no_such_key: 1\n",
    );
    let path = file.display().to_string();
    let source = source(&["--config", &path]).unwrap();
    // A program without `ros2.*` keys of its own does not report the
    // RMW's section; the RMW checks it when it reads it.
    let unknown = source.unknown_keys(&no_keys);
    assert_eq!(unknown.len(), 1, "{unknown:?}");
    assert_eq!(unknown[0].0, "wire.no_such_key");
    let unknown = source.unknown_keys(&is_key);
    assert_eq!(unknown.len(), 1, "{unknown:?}");
    let _ = std::fs::remove_file(file);
}

#[test]
fn the_runtime_settings_and_the_codec_dialect_of_a_file_reach_the_runtime() {
    let file = runtime_file(
        "settings",
        "wire:\n  data_representation_offer: XCDR2\n  codec_dialect: ros-fastrtps\n",
    );
    let path = file.display().to_string();
    let runtime = RuntimeConfig::from_source(&source(&["--config", &path]).unwrap()).unwrap();
    assert_eq!(runtime.data_representation_offer, [XCDR2]);
    assert_eq!(runtime.codec_dialect, Dialect::RosFastRtps);
    // Without a setting: `[XCDR]` and the specification's dialect.
    std::fs::write(&file, "\n").unwrap();
    let runtime = RuntimeConfig::from_source(&source(&["--config", &path]).unwrap()).unwrap();
    assert_eq!(runtime.data_representation_offer, [XCDR]);
    assert_eq!(runtime.codec_dialect, Dialect::Specification);
    let _ = std::fs::remove_file(file);
}

#[test]
fn an_invalid_runtime_value_is_refused_with_its_line() {
    let file = runtime_file(
        "invalid",
        "ros2:\n  topic_mangling: rmw\nwire:\n  data_representation_offer: XCDR3\n",
    );
    let path = file.display().to_string();
    let source = source(&["--config", &path]).unwrap();
    match RuntimeConfig::from_source(&source) {
        Err(ConfigError::Invalid {
            setting, origin, ..
        }) => {
            assert_eq!(setting, "wire.data_representation_offer");
            assert!(matches!(origin, Origin::File { line: 4, .. }), "{origin:?}");
        }
        other => panic!("{other:?}"),
    }
    // The diagnostics CLI's `--check-config` report says invalid.
    assert!(source.check_report(&is_key).is_err());
    let _ = std::fs::remove_file(file);
}

#[test]
fn xtypes_7_4_2_a_spatial_type_declares_appendable_and_frames_a_dheader() {
    assert_eq!(BridgeGeoPose::extensibility(), Extensibility::Appendable);
    assert_eq!(BridgeGeoPose::type_name(), BridgeGeoPose::TYPE_NAME);
    let mut out = Vec::new();
    BridgeGeoPose::default().encode(&mut out).unwrap();
    // DELIMITED_CDR: the DHEADER is the length of what follows it.
    let dheader = u32::from_le_bytes([out[0], out[1], out[2], out[3]]);
    assert_eq!(usize::try_from(dheader).unwrap(), out.len() - 4);
}

#[test]
fn xtypes_7_6_3_1_2_a_spatial_type_without_an_xcdr1_codec_refuses_xcdr1() {
    // Table 60: APPENDABLE is PLAIN_CDR in version 1, no DHEADER. A type that has no
    // XCDR1 codec and does not claim `XCDR1_MATCHES_XCDR2` refuses.
    let mut out = Vec::new();
    assert!(matches!(
        BridgeGeoPose::default().encode_xcdr1(&mut out),
        Err(EncodeError::Invalid { .. })
    ));
    assert!(out.is_empty(), "nothing written under a CDR_LE header");
    let mut xcdr2 = Vec::new();
    BridgeGeoPose::default().encode(&mut xcdr2).unwrap();
    assert!(matches!(
        BridgeGeoPose::decode_xcdr1(&xcdr2[4..]),
        Err(DecodeError::Invalid { .. })
    ));
}

#[test]
fn the_bridged_types_refuse_big_endian_without_a_big_endian_codec() {
    let mut out = Vec::new();
    assert!(matches!(
        BridgeGeoPose::default().encode_be(&mut out),
        Err(EncodeError::Invalid { .. })
    ));
    assert!(matches!(
        BridgePoseStamped::default().encode_be(&mut out),
        Err(EncodeError::Invalid { .. })
    ));
    assert!(out.is_empty());
    let mut le = Vec::new();
    BridgePoseStamped::default().encode(&mut le).unwrap();
    assert!(matches!(
        BridgePoseStamped::decode_be(&le),
        Err(DecodeError::Invalid { .. })
    ));
}

#[test]
fn dcps_2_2_2_3_1_2_a_topic_of_a_bridged_type_names_its_type() {
    let participant = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let spatial = participant
        .create_topic::<BridgeGeoPose>("SpatialGeoPose", TopicQos::default())
        .unwrap();
    assert_eq!(spatial.name(), "SpatialGeoPose");
    assert_eq!(spatial.type_name(), "spatial::core::GeoPose");
    let ros = participant
        .create_topic::<BridgeGeoPoseStamped>("rt/geo_pose", TopicQos::default())
        .unwrap();
    assert_eq!(
        ros.type_name(),
        "geographic_msgs::msg::dds_::GeoPoseStamped_"
    );
}

#[test]
fn dcps_2_2_3_18_the_bridge_qos_keeps_its_history_within_the_default_limits() {
    let history = HistoryQosPolicy {
        kind: HistoryKind::KeepLast,
        depth: 10,
    };
    let reliable = ReliabilityQosPolicy {
        kind: ReliabilityKind::Reliable,
        ..Default::default()
    };
    let writer = DataWriterQos {
        reliability: reliable,
        durability: DurabilityQosPolicy {
            kind: DurabilityKind::TransientLocal,
        },
        history,
        data_representation: Some(vec![XCDR]),
        ..Default::default()
    };
    assert!(writer.check_consistency().is_ok());
    let reader = DataReaderQos {
        reliability: reliable,
        durability: DurabilityQosPolicy {
            kind: DurabilityKind::Volatile,
        },
        history,
        ..Default::default()
    };
    assert!(reader.check_consistency().is_ok());
    // §2.2.3.19: the limits left at their default are LENGTH_UNLIMITED.
    assert_eq!(writer.resource_limits.max_samples_per_instance, -1);
    assert_eq!(reader.resource_limits.max_samples_per_instance, -1);
}
