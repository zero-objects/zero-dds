// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `ros2.spatial_bridge` — an opt-in content-translation mode (C2(6)).
//!
//! `zerodds-spatial-ros2` converts SpatialDDS types to and from ROS 2
//! common-interfaces messages, but nothing in the tree calls it: this RMW
//! shim (the natural place) does purely transport-level, message-type-
//! agnostic DDS<->ROS bridging and never touches sample *content*
//! (`docs/doc-map/ros2.md`'s own words). This module is the missing call:
//! for each configured topic pairing it runs a small DDS<->DDS republisher
//! — a SpatialDDS topic on one side, the matching ROS 2 topic/type
//! (`spatial_ros2::naming`) on the other, `spatial_ros2::convert` in
//! between — turning the crate's own stated gap ("no live communication
//! demonstrated") into a wired one.
//!
//! It runs on its **own** `DomainParticipant` (the plain `zerodds_dcps`
//! typed API, not the RMW's C-API runtime pointer, and not `rmw_c/`): a
//! second participant on the same domain is ordinary DDS, needs no access
//! to the RMW's internal FFI runtime, and is what makes the in-process test
//! below possible (a SpatialDDS writer and a plain DCPS reader of the ROS
//! type, both against this bridge's participant, no real network needed).
//!
//! ## Field fidelity
//!
//! `spatial_ros2::convert`'s own doc comment names, per message kind, which
//! SpatialDDS-only fields have no ROS destination (dropped on `dds_to_ros`,
//! defaulted on `ros_to_dds`). This module does not repeat that loss
//! silently: `SpatialMessageKind::dropped_fields` is emitted once per
//! configured `(topic, direction)` as an observability event
//! (`Component::User`, name `spatial_bridge.field_fidelity`) when the pump
//! for it starts, through the same sink `diagnostics.observability` already
//! configures for the rest of the runtime
//! (`zerodds_dcps::runtime::observability_sink_from_source`).
//!
//! ## Durability
//!
//! Both bridge writers offer TRANSIENT_LOCAL and both bridge readers
//! request VOLATILE (`ros_qos_writer` / `ros_qos_reader` give the measured
//! reason and the clause). The consequence for anyone consuming a bridged
//! topic: a reader that joins after the bridge's writer has already
//! forwarded gets the writer's retained samples **only if it asks for
//! TRANSIENT_LOCAL**; a VOLATILE reader (the ROS default) sees what is
//! written from the moment it matched, and nothing before.
//!
//! ## Type names
//!
//! The ROS side uses the rmw-compatible names `spatial_ros2::naming` gives
//! (`pkg::msg::dds_::Type_`) — matches a native `rmw_fastrtps_cpp`/
//! `rmw_cyclonedds_cpp` node by construction. The SpatialDDS side has no
//! such registry (it is ZeroDDS's own profile, not an external wire
//! standard with a fixed type-name convention); this module uses
//! `spatial::<module>::<Type>` (mirroring the Rust module path in
//! `zerodds-spatial-dds`), documented in `zerodds-ros2.yaml(5)` — a
//! SpatialDDS publisher/subscriber this bridge talks to must use the same
//! type name string (DDS matches publications/subscriptions by topic name
//! **and** type name; this is not XTypes-hash-checked, only string-checked,
//! same as every hand-written `DdsType` in this tree that does not go
//! through the `idl-rust` codegen).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use zerodds_config::ConfigError;
use zerodds_config::read::{Section, first_origin};
use zerodds_dcps::qos::{
    DataReaderQos, DataWriterQos, DurabilityKind, DurabilityQosPolicy, HistoryKind,
    HistoryQosPolicy, PublisherQos, ReliabilityKind, ReliabilityQosPolicy, SubscriberQos, TopicQos,
};
use zerodds_dcps::{
    DdsType, DomainParticipant, DomainParticipantFactory, DomainParticipantQos, InstanceHandle,
};
use zerodds_foundation::observability::{Component, Event, Level, SharedSink};
use zerodds_spatial_ros2::naming::RosMessage;
use zerodds_spatial_ros2::{convert, geographic_msgs, geometry_msgs, sensor_msgs, vision_msgs};

// ============================================================================
// Wrapper `DdsType`s — orphan-rule newtypes over the generated SpatialDDS /
// ROS 2 message structs. `encode`/`decode` delegate to each type's own
// generated inherent `encode(&mut Writer)`/`decode(&mut Reader)` (the same
// ones `crates/spatial-ros2/tests/mapping.rs` round-trips); this module
// writes no wire format of its own.
// ============================================================================

/// XCDR2 `encode`/`decode` of a wrapper over a generated type. The
/// generated types declare no extensibility, so they are `@appendable` and
/// their XCDR2 codec writes a DHEADER; `EXTENSIBILITY` says so. Declared
/// `@final`, the encapsulation said PLAIN_CDR2 over a delimited body, and
/// `rmw_zerodds_cpp`, believing the header, read every member four octets
/// early (measured: a `PoseStamped` of position (1, 2, 3) and frame `map`
/// arrived as (1.5e-306, 5.9e-313, 0) and an empty frame).
macro_rules! bridge_xcdr2_codec {
    ($inner:path, $type_name:literal) => {
        const TYPE_NAME: &'static str = $type_name;
        const EXTENSIBILITY: zerodds_dcps::dds_type::Extensibility =
            zerodds_dcps::dds_type::Extensibility::Appendable;

        fn encode(
            &self,
            out: &mut Vec<u8>,
        ) -> core::result::Result<(), zerodds_dcps::dds_type::EncodeError> {
            let mut w = zerodds_wire::Writer::new(zerodds_wire::Endianness::Little);
            self.0
                .encode(&mut w)
                .map_err(|_| zerodds_dcps::dds_type::EncodeError::Invalid {
                    what: concat!("spatial_bridge: ", $type_name, " encode failed"),
                })?;
            out.extend_from_slice(&w.finish());
            Ok(())
        }

        fn decode(bytes: &[u8]) -> core::result::Result<Self, zerodds_dcps::dds_type::DecodeError> {
            let mut r = zerodds_wire::Reader::new(bytes, zerodds_wire::Endianness::Little);
            <$inner>::decode(&mut r).map(Self).map_err(|_| {
                zerodds_dcps::dds_type::DecodeError::Invalid {
                    what: concat!("spatial_bridge: ", $type_name, " decode failed"),
                }
            })
        }
    };
}

/// A SpatialDDS type: XCDR2 only, the one form its generated codec has.
macro_rules! bridge_dds_type {
    ($wrapper:ident, $inner:path, $type_name:literal) => {
        /// Orphan-rule newtype over
        #[doc = concat!("`", stringify!($inner), "`")]
        /// (see the module doc comment) — a bridge-internal `DdsType` for
        #[doc = concat!("`", $type_name, "`.")]
        #[derive(Debug, Clone, PartialEq, Default)]
        #[allow(missing_docs)]
        pub struct $wrapper(pub $inner);

        impl DdsType for $wrapper {
            bridge_xcdr2_codec!($inner, $type_name);
        }
    };
}

/// A ROS 2 message type: XCDR2, and XCDR1 (`CDR_LE`), the form ROS 2
/// middlewares — `rmw_zerodds_cpp` among them — write and read ROS
/// messages in. `zerodds-spatial-ros2` generates both codecs.
macro_rules! bridge_ros_type {
    ($wrapper:ident, $inner:path, $type_name:literal) => {
        /// Orphan-rule newtype over
        #[doc = concat!("`", stringify!($inner), "`")]
        /// (see the module doc comment) — a bridge-internal `DdsType` for
        #[doc = concat!("`", $type_name, "`.")]
        #[derive(Debug, Clone, PartialEq, Default)]
        #[allow(missing_docs)]
        pub struct $wrapper(pub $inner);

        impl DdsType for $wrapper {
            bridge_xcdr2_codec!($inner, $type_name);

            fn encode_xcdr1(
                &self,
                out: &mut Vec<u8>,
            ) -> core::result::Result<(), zerodds_dcps::dds_type::EncodeError> {
                let mut w =
                    zerodds_wire::Writer::with_max_align(zerodds_wire::Endianness::Little, 8);
                self.0.encode_xcdr1(&mut w).map_err(|_| {
                    zerodds_dcps::dds_type::EncodeError::Invalid {
                        what: concat!("spatial_bridge: ", $type_name, " XCDR1 encode failed"),
                    }
                })?;
                out.extend_from_slice(&w.finish());
                Ok(())
            }

            fn decode_xcdr1(
                bytes: &[u8],
            ) -> core::result::Result<Self, zerodds_dcps::dds_type::DecodeError> {
                let mut r = zerodds_wire::Reader::with_max_align(
                    bytes,
                    zerodds_wire::Endianness::Little,
                    8,
                );
                <$inner>::decode_xcdr1(&mut r).map(Self).map_err(|_| {
                    zerodds_dcps::dds_type::DecodeError::Invalid {
                        what: concat!("spatial_bridge: ", $type_name, " XCDR1 decode failed"),
                    }
                })
            }
        }
    };
}

bridge_dds_type!(
    BridgeFramedPose,
    zerodds_spatial_dds::spatial::core::FramedPose,
    "spatial::core::FramedPose"
);
bridge_dds_type!(
    BridgeFrameTransform,
    zerodds_spatial_dds::spatial::core::FrameTransform,
    "spatial::core::FrameTransform"
);
bridge_dds_type!(
    BridgeGeoPose,
    zerodds_spatial_dds::spatial::core::GeoPose,
    "spatial::core::GeoPose"
);
bridge_dds_type!(
    BridgeDetection3D,
    zerodds_spatial_dds::spatial::semantics::Detection3D,
    "spatial::semantics::Detection3D"
);
bridge_dds_type!(
    BridgeDetection3DSet,
    zerodds_spatial_dds::spatial::semantics::Detection3DSet,
    "spatial::semantics::Detection3DSet"
);
bridge_dds_type!(
    BridgeImuSample,
    zerodds_spatial_dds::spatial::vio::ImuSample,
    "spatial::vio::ImuSample"
);

bridge_ros_type!(
    BridgePoseStamped,
    geometry_msgs::msg::PoseStamped,
    "geometry_msgs::msg::dds_::PoseStamped_"
);
bridge_ros_type!(
    BridgeTransformStamped,
    geometry_msgs::msg::TransformStamped,
    "geometry_msgs::msg::dds_::TransformStamped_"
);
bridge_ros_type!(
    BridgeGeoPoseStamped,
    geographic_msgs::msg::GeoPoseStamped,
    "geographic_msgs::msg::dds_::GeoPoseStamped_"
);
bridge_ros_type!(
    BridgeVisionDetection3D,
    vision_msgs::msg::Detection3D,
    "vision_msgs::msg::dds_::Detection3D_"
);
bridge_ros_type!(
    BridgeVisionDetection3DArray,
    vision_msgs::msg::Detection3DArray,
    "vision_msgs::msg::dds_::Detection3DArray_"
);
bridge_ros_type!(
    BridgeImu,
    sensor_msgs::msg::Imu,
    "sensor_msgs::msg::dds_::Imu_"
);

// ============================================================================
// Message kinds
// ============================================================================

/// A configured `(spatial_topic, ros_topic)` pairing's message kind.
/// `tf_message` is deliberately absent: `spatial_ros2::convert` has no
/// `tf_message_from_ros` (no reverse conversion at all) and its
/// `tf_message_from` bundles a *slice* of `FrameTransform`, not one sample
/// at a time — neither shape fits this module's per-sample pump, so a
/// config naming it is refused (see [`SpatialMessageKind::parse`]) rather
/// than silently only doing half the job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialMessageKind {
    /// `spatial::core::FramedPose` <-> `geometry_msgs/PoseStamped`.
    PoseStamped,
    /// `spatial::core::FrameTransform` <-> `geometry_msgs/TransformStamped`.
    TransformStamped,
    /// `spatial::core::GeoPose` <-> `geographic_msgs/GeoPoseStamped`.
    GeoPoseStamped,
    /// `spatial::semantics::Detection3D` <-> `vision_msgs/Detection3D`.
    Detection3D,
    /// `spatial::semantics::Detection3DSet` <-> `vision_msgs/Detection3DArray`.
    Detection3DArray,
    /// `spatial::vio::ImuSample` <-> `sensor_msgs/Imu`.
    Imu,
}

impl SpatialMessageKind {
    /// The config spelling (`message: <this>`), or `None` for an unknown
    /// or deliberately-unsupported (`tf_message`) name.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "pose_stamped" => Self::PoseStamped,
            "transform_stamped" => Self::TransformStamped,
            "geo_pose_stamped" => Self::GeoPoseStamped,
            "detection3d" => Self::Detection3D,
            "detection3d_array" => Self::Detection3DArray,
            "imu" => Self::Imu,
            _ => return None,
        })
    }

    /// The [`RosMessage`] this kind maps to (its DDS topic/type name).
    #[must_use]
    pub fn ros_message(self) -> RosMessage {
        match self {
            Self::PoseStamped => RosMessage::PoseStamped,
            Self::TransformStamped => RosMessage::TransformStamped,
            Self::GeoPoseStamped => RosMessage::GeoPoseStamped,
            Self::Detection3D => RosMessage::Detection3D,
            Self::Detection3DArray => RosMessage::Detection3DArray,
            Self::Imu => RosMessage::Imu,
        }
    }

    /// SpatialDDS-only fields with no ROS destination — dropped going
    /// `dds_to_ros`, defaulted coming back `ros_to_dds` — verbatim from
    /// `spatial_ros2::convert`'s own field-fidelity doc comment.
    #[must_use]
    fn dropped_fields(self) -> &'static [&'static str] {
        match self {
            Self::PoseStamped => &["cov (CovMatrix)"],
            Self::TransformStamped => &["transform_id", "cov (CovMatrix)"],
            Self::GeoPoseStamped => &["frame_kind", "cov (CovMatrix)"],
            Self::Detection3D | Self::Detection3DArray => &[
                "track_id",
                "attributes",
                "visibility",
                "point-count evidence",
                "tile_key",
                "source_id",
            ],
            Self::Imu => &[
                "source_id",
                "seq",
                "orientation (ROS convention: orientation_covariance[0] = -1, unknown)",
            ],
        }
    }
}

// ============================================================================
// Config — `ros2.spatial_bridge`
// ============================================================================

/// Direction a topic pairing bridges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// SpatialDDS -> ROS 2 only.
    DdsToRos,
    /// ROS 2 -> SpatialDDS only.
    RosToDds,
    /// Both ways (two independent pumps).
    Both,
}

impl Direction {
    fn dds_to_ros(self) -> bool {
        matches!(self, Self::DdsToRos | Self::Both)
    }

    fn ros_to_dds(self) -> bool {
        matches!(self, Self::RosToDds | Self::Both)
    }
}

/// One `ros2.spatial_bridge.mappings` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicMapping {
    /// Message kind — decides the conversion and the ROS type/topic name.
    pub message: SpatialMessageKind,
    /// The SpatialDDS DCPS topic name (e.g. `sdds/default/FramedPose`,
    /// `zerodds_spatial_dds::topic(operator, kind)`'s convention).
    pub spatial_topic: String,
    /// The ROS logical topic name (before `rt/` mangling), e.g. `/robot1/pose`.
    pub ros_topic: String,
    /// Which way(s) this pairing bridges.
    pub direction: Direction,
}

/// The `ros2.spatial_bridge` settings.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SpatialBridgeConfig {
    /// Whether the bridge starts at all. Default `false` — opt-in.
    pub enabled: bool,
    /// The configured topic pairings.
    pub mappings: Vec<TopicMapping>,
}

impl SpatialBridgeConfig {
    /// Reads `ros2.spatial_bridge` out of a merged runtime tree.
    ///
    /// # Errors
    /// [`ConfigError::Invalid`] naming the setting, its origin and what it
    /// accepts — including a `message: tf_message` entry, refused by name
    /// (see [`SpatialMessageKind`]'s doc comment for why).
    pub fn from_config(config: Option<&zerodds_config::Config>) -> Result<Self, ConfigError> {
        let tree = config.and_then(zerodds_config::Config::root);
        let root = Section::root(tree)?;
        let ros2 = root.section("ros2")?;
        let sb = ros2.section("spatial_bridge")?;
        sb.only(&["enabled", "mappings"])?;
        let enabled = sb.boolean("enabled")?.unwrap_or(false);
        let mut mappings = Vec::new();
        for (path, item) in sb.items("mappings")? {
            let entry = Section::of(Some(item), path.clone())?;
            entry.only(&["message", "spatial_topic", "ros_topic", "direction"])?;
            let origin = first_origin(item);
            let message_str = entry
                .string("message")?
                .ok_or_else(|| ConfigError::Invalid {
                    setting: format!("{path}.message"),
                    origin: origin.clone(),
                    message: "a mapping needs a message kind".to_string(),
                })?;
            let message =
                SpatialMessageKind::parse(&message_str).ok_or_else(|| ConfigError::Invalid {
                    setting: format!("{path}.message"),
                    origin: origin.clone(),
                    message: if message_str == "tf_message" {
                        "tf_message is not bridged: spatial_ros2::convert has no reverse \
                         conversion and its forward one bundles a slice, not one sample; \
                         use transform_stamped for a 1:1 FrameTransform bridge"
                            .to_string()
                    } else {
                        "expected pose_stamped, transform_stamped, geo_pose_stamped, \
                         detection3d, detection3d_array or imu"
                            .to_string()
                    },
                })?;
            let spatial_topic =
                entry
                    .string("spatial_topic")?
                    .ok_or_else(|| ConfigError::Invalid {
                        setting: format!("{path}.spatial_topic"),
                        origin: origin.clone(),
                        message: "a mapping needs a spatial_topic".to_string(),
                    })?;
            let ros_topic = entry
                .string("ros_topic")?
                .ok_or_else(|| ConfigError::Invalid {
                    setting: format!("{path}.ros_topic"),
                    origin: origin.clone(),
                    message: "a mapping needs a ros_topic".to_string(),
                })?;
            let direction = match entry
                .choice("direction", &["dds_to_ros", "ros_to_dds", "both"])?
                .as_deref()
            {
                Some("dds_to_ros") => Direction::DdsToRos,
                Some("ros_to_dds") => Direction::RosToDds,
                _ => Direction::Both,
            };
            mappings.push(TopicMapping {
                message,
                spatial_topic,
                ros_topic,
                direction,
            });
        }
        Ok(Self { enabled, mappings })
    }
}

// ============================================================================
// Runtime
// ============================================================================

/// What both bridge writers offer: RELIABLE, TRANSIENT_LOCAL,
/// KEEP_LAST(10).
///
/// TRANSIENT_LOCAL is what keeps a forwarded sample from dying on the way
/// out. Measured on the Linux test host against a ROS 2 Jazzy node
/// (`zerodds-examples/rmw-spatial-bridge`, `rmw_zerodds_cpp`): the ROS node
/// published as soon as the bridge's *reader* had matched it, so the bridge
/// could forward the first pose before its own writer had discovered the
/// SpatialDDS reader — and the pose reached nobody. 59 of 60
/// `ros_to_dds`, 57 of 58 `dds_to_ros`, the first pose missing in the runs
/// that lost one. DDS 1.4 §2.2.3.4 (DURABILITY): a VOLATILE writer keeps
/// no sample for a reader that matches later, while a TRANSIENT_LOCAL
/// writer delivers the samples its HISTORY holds to such a reader.
///
/// The HISTORY stays the KEEP_LAST(10) the bridge already offered,
/// unchanged: that depth is what bounds what a reader joining late is
/// served (its last ten samples), and nothing else about the writer's
/// QoS changes.
fn ros_qos_writer() -> DataWriterQos {
    DataWriterQos {
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            ..Default::default()
        },
        durability: DurabilityQosPolicy {
            kind: DurabilityKind::TransientLocal,
        },
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 10,
        },
        ..Default::default()
    }
}

/// The ROS-side writer offers XCDR1: ROS 2 middlewares put ROS messages on
/// the wire as `CDR_LE` (`rmw_zerodds_cpp` does), and a ROS reader that
/// takes nothing else then still gets the sample. The participant's own
/// offer is `[XCDR2]`, for the SpatialDDS side. Its TRANSIENT_LOCAL offer
/// comes from `ros_qos_writer` — this is that same writer plus a
/// per-writer DataRepresentation.
fn ros_type_writer_qos() -> DataWriterQos {
    DataWriterQos {
        data_representation: Some(vec![
            zerodds_rtps::publication_data::data_representation::XCDR,
        ]),
        ..ros_qos_writer()
    }
}

/// What both bridge readers request: RELIABLE, VOLATILE, KEEP_LAST(10).
///
/// VOLATILE is deliberate, and the compatibility table is why in both
/// directions:
///
/// * A reader requesting TRANSIENT_LOCAL does not match a VOLATILE
///   publisher at all — DDS 1.4 §2.2.3, table "QoS compatibility",
///   DURABILITY "RxO: Yes": an offered VOLATILE does not satisfy a
///   requested TRANSIENT_LOCAL (the reverse holds: offered TRANSIENT_LOCAL
///   *does* satisfy a requested VOLATILE). ROS publishers are VOLATILE
///   unless the application asks otherwise, so requesting TRANSIENT_LOCAL
///   would leave the `dds_to_ros` pump silently unmatched against a
///   default-profile ROS node.
/// * Because offered TRANSIENT_LOCAL satisfies a requested VOLATILE, this
///   reader still matches the bridge's own TRANSIENT_LOCAL writer (and a
///   SpatialDDS publisher on either side of the bridge), and receives every
///   sample written from the match on. It only declines the history it did
///   not ask for: DDS 1.4 §2.2.3.4, a VOLATILE reader never receives
///   samples written before it matched.
fn ros_qos_reader() -> DataReaderQos {
    DataReaderQos {
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            ..Default::default()
        },
        durability: DurabilityQosPolicy {
            kind: DurabilityKind::Volatile,
        },
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 10,
        },
        ..Default::default()
    }
}

/// Runs one direction's pump until `stop` is set: `wait_for_data` +
/// `take_with_info()` on `reader`, `convert`, `write()` on `writer`.
/// Errors from a single `write()`/`take_with_info()` are not fatal — the
/// loop just tries again next pass (matches every other background poller
/// in this crate, e.g. `events.rs`'s dispatcher thread).
///
/// A `direction: both` mapping puts pump A's writer and pump B's reader
/// on the *same* topic (pump A's output topic is pump B's input topic
/// and vice versa): without echo suppression it bounces forever — pump A
/// converts and republishes, pump B reads that republish right back off
/// the same topic, converts and republishes again, ad infinitum.
/// `SampleInfo::publication_handle` (DDS 1.4 §2.2.2.5.1 "may be compared
/// to `DataWriter::get_instance_handle()`") is the principled fix: this
/// pump's caller passes `sibling_writer_handle`, the *other* direction's
/// `DataWriter::publication_handle()` — the pump skips any incoming
/// sample whose `SampleInfo::publication_handle` equals it, i.e. anything
/// its own sibling wrote, and converts+republishes everything else,
/// including content-identical traffic from a genuine external
/// publisher. `None` for a direction with no sibling (not `direction:
/// both`). `zerodds-dcps`'s live/mpsc delivery path used to never
/// populate the field (confirmed empirically, always `HANDLE_NIL`, not
/// assumed) — fixed at the root there; this pump no longer needs the
/// content-match fallback that gap once forced.
fn pump<S, R, F>(
    label: String,
    reader: zerodds_dcps::subscriber::DataReader<S>,
    writer: zerodds_dcps::publisher::DataWriter<R>,
    sibling_writer_handle: Option<InstanceHandle>,
    convert: F,
    stop: Arc<AtomicBool>,
) -> Result<JoinHandle<()>, zerodds_dcps::error::DdsError>
where
    S: DdsType + Send + Sync + 'static,
    R: DdsType + Send + Sync + 'static,
    F: Fn(&S) -> R + Send + 'static,
{
    std::thread::Builder::new()
        .name(format!("spatial-bridge-{label}"))
        .spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                let _ = reader.wait_for_data(Duration::from_millis(200));
                if let Ok(samples) = reader.take_with_info() {
                    for sample in &samples {
                        if !sample.info.valid_data {
                            continue;
                        }
                        if sibling_writer_handle == Some(sample.info.publication_handle) {
                            continue;
                        }
                        let converted = convert(&sample.data);
                        let _ = writer.write(&converted);
                    }
                }
            }
        })
        .map_err(|_| zerodds_dcps::error::DdsError::TransportError {
            label: "spatial-bridge pump thread spawn failed",
        })
}

fn emit_field_fidelity(
    sink: &SharedSink,
    ros_topic: &str,
    direction: &str,
    kind: SpatialMessageKind,
) {
    let dropped = kind.dropped_fields();
    if dropped.is_empty() {
        return;
    }
    let event = Event::new(
        Level::Info,
        Component::User,
        "spatial_bridge.field_fidelity",
    )
    .with_attr("topic", ros_topic)
    .with_attr("direction", direction)
    .with_attr("fields", dropped.join(", "));
    sink.record(&event);
}

/// A running bridge: the pump threads and the internal participant that
/// owns their readers/writers. Dropping it stops every pump and tears the
/// participant down (RAII, same as every other DCPS entity in this tree).
pub struct SpatialBridge {
    _participant: DomainParticipant,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl Drop for SpatialBridge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

impl SpatialBridge {
    /// Starts the bridge on its own new participant on `domain_id` (the
    /// same domain the RMW context runs on) if `config.enabled`; `None`
    /// when it is not.
    ///
    /// # Errors
    /// A DDS entity failed to create (transport bind failure, an invalid
    /// QoS combination).
    pub fn start(
        domain_id: i32,
        config: &SpatialBridgeConfig,
        sink: SharedSink,
    ) -> Result<Option<Self>, zerodds_dcps::error::DdsError> {
        if !config.enabled || config.mappings.is_empty() {
            return Ok(None);
        }
        // The generated SpatialDDS/ROS 2 message codecs (`rust_codec` in
        // `zerodds-lang-rules`) only ever emit XCDR2 (`encode`/`decode` in
        // `crates/spatial-ros2/tests/mapping.rs`'s own words) — there is no
        // PLAIN_CDR (XCDR1) form to fall back to, unlike a hand-written
        // `DdsType` that can assert `XCDR1_MATCHES_XCDR2`. The runtime
        // default offer is `[XCDR1, XCDR2]` (legacy-first); this
        // participant overrides it to `[XCDR2]` so every bridge writer
        // announces and emits the only representation these types have.
        let mut runtime_config = zerodds_dcps::runtime::RuntimeConfig::from_standard_paths()
            .map_err(|e| zerodds_dcps::error::DdsError::Configuration {
                message: e.to_string(),
            })?;
        runtime_config.data_representation_offer =
            vec![zerodds_rtps::publication_data::data_representation::XCDR2];
        let participant = DomainParticipantFactory::instance().create_participant_with_config(
            domain_id,
            DomainParticipantQos::default(),
            runtime_config,
        )?;
        Self::start_on_participant(participant, config, sink)
    }

    /// Like [`Self::start`], but on an existing `participant` instead of a
    /// new one of its own — the hook the in-process tests use: a
    /// SpatialDDS writer and a plain DCPS reader of the ROS type, created
    /// on the **same** participant as the bridge, exchange samples through
    /// it without a second process or real network (same-runtime direct
    /// channel handoff, as `ordered_access_e2e.rs` in `zerodds-dcps` uses
    /// for exactly this reason).
    ///
    /// # Errors
    /// As [`Self::start`].
    pub fn start_on_participant(
        participant: DomainParticipant,
        config: &SpatialBridgeConfig,
        sink: SharedSink,
    ) -> Result<Option<Self>, zerodds_dcps::error::DdsError> {
        if !config.enabled || config.mappings.is_empty() {
            return Ok(None);
        }
        let stop = Arc::new(AtomicBool::new(false));
        let mut threads = Vec::new();

        macro_rules! wire_pair {
            ($mapping:expr, $spatial_ty:ty, $ros_ty:ty, $to_ros:expr, $to_dds:expr) => {{
                let m = $mapping;
                let spatial_topic = participant
                    .create_topic::<$spatial_ty>(&m.spatial_topic, TopicQos::default())?;
                let ros_dds_topic = zerodds_spatial_ros2::naming::ros_topic(&m.ros_topic);
                let ros_topic =
                    participant.create_topic::<$ros_ty>(&ros_dds_topic, TopicQos::default())?;

                // Both writers are created before either pump thread starts
                // (see `pump`'s doc comment): each direction's `sibling_*_
                // handle` is the OTHER direction's writer's
                // `publication_handle()`, captured here — a `Copy` value,
                // not a borrow — before that writer is moved into its own
                // pump. A direction with no sibling (not `direction: both`)
                // simply gets `None`.
                let dds_to_ros_writer = if m.direction.dds_to_ros() {
                    Some(
                        participant
                            .create_publisher(PublisherQos::default())
                            .create_datawriter::<$ros_ty>(&ros_topic, ros_type_writer_qos())?,
                    )
                } else {
                    None
                };
                let dds_to_ros_writer_handle =
                    dds_to_ros_writer.as_ref().map(|w| w.publication_handle());
                let ros_to_dds_writer = if m.direction.ros_to_dds() {
                    Some(
                        participant
                            .create_publisher(PublisherQos::default())
                            .create_datawriter::<$spatial_ty>(&spatial_topic, ros_qos_writer())?,
                    )
                } else {
                    None
                };
                let ros_to_dds_writer_handle =
                    ros_to_dds_writer.as_ref().map(|w| w.publication_handle());

                if let Some(writer) = dds_to_ros_writer {
                    let subscriber = participant.create_subscriber(SubscriberQos::default());
                    let reader = subscriber
                        .create_datareader::<$spatial_ty>(&spatial_topic, ros_qos_reader())?;
                    emit_field_fidelity(&sink, &m.ros_topic, "dds_to_ros", m.message);
                    threads.push(pump(
                        format!("{}-dds-to-ros", m.ros_topic),
                        reader,
                        writer,
                        ros_to_dds_writer_handle,
                        $to_ros,
                        Arc::clone(&stop),
                    )?);
                }
                if let Some(writer) = ros_to_dds_writer {
                    let subscriber = participant.create_subscriber(SubscriberQos::default());
                    let reader =
                        subscriber.create_datareader::<$ros_ty>(&ros_topic, ros_qos_reader())?;
                    emit_field_fidelity(&sink, &m.ros_topic, "ros_to_dds", m.message);
                    threads.push(pump(
                        format!("{}-ros-to-dds", m.ros_topic),
                        reader,
                        writer,
                        dds_to_ros_writer_handle,
                        $to_dds,
                        Arc::clone(&stop),
                    )?);
                }
            }};
        }

        for mapping in &config.mappings {
            match mapping.message {
                SpatialMessageKind::PoseStamped => wire_pair!(
                    mapping,
                    BridgeFramedPose,
                    BridgePoseStamped,
                    |s: &BridgeFramedPose| BridgePoseStamped(convert::framed_pose_to_ros(&s.0)),
                    |r: &BridgePoseStamped| BridgeFramedPose(convert::framed_pose_from_ros(&r.0))
                ),
                SpatialMessageKind::TransformStamped => wire_pair!(
                    mapping,
                    BridgeFrameTransform,
                    BridgeTransformStamped,
                    |s: &BridgeFrameTransform| BridgeTransformStamped(
                        convert::frame_transform_to_ros(&s.0)
                    ),
                    |r: &BridgeTransformStamped| BridgeFrameTransform(
                        convert::frame_transform_from_ros(&r.0)
                    )
                ),
                SpatialMessageKind::GeoPoseStamped => wire_pair!(
                    mapping,
                    BridgeGeoPose,
                    BridgeGeoPoseStamped,
                    |s: &BridgeGeoPose| BridgeGeoPoseStamped(convert::geo_pose_to_ros(&s.0)),
                    |r: &BridgeGeoPoseStamped| BridgeGeoPose(convert::geo_pose_from_ros(&r.0))
                ),
                SpatialMessageKind::Detection3D => wire_pair!(
                    mapping,
                    BridgeDetection3D,
                    BridgeVisionDetection3D,
                    |s: &BridgeDetection3D| BridgeVisionDetection3D(convert::detection3d_to_ros(
                        &s.0
                    )),
                    |r: &BridgeVisionDetection3D| BridgeDetection3D(convert::detection3d_from_ros(
                        &r.0
                    ))
                ),
                SpatialMessageKind::Detection3DArray => wire_pair!(
                    mapping,
                    BridgeDetection3DSet,
                    BridgeVisionDetection3DArray,
                    |s: &BridgeDetection3DSet| BridgeVisionDetection3DArray(
                        convert::detection3d_set_to_ros(&s.0)
                    ),
                    |r: &BridgeVisionDetection3DArray| BridgeDetection3DSet(
                        convert::detection3d_set_from_ros(&r.0)
                    )
                ),
                SpatialMessageKind::Imu => wire_pair!(
                    mapping,
                    BridgeImuSample,
                    BridgeImu,
                    |s: &BridgeImuSample| BridgeImu(convert::imu_to_ros(&s.0)),
                    |r: &BridgeImu| BridgeImuSample(convert::imu_from_ros(&r.0))
                ),
            }
        }

        Ok(Some(Self {
            _participant: participant,
            stop,
            threads,
        }))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn load(yaml: &str) -> Result<SpatialBridgeConfig, ConfigError> {
        let mut c = zerodds_config::Config::default();
        c.push(
            zerodds_config::parse_str(yaml, "<string>").unwrap(),
            "<string>".into(),
        )
        .unwrap();
        SpatialBridgeConfig::from_config(Some(&c))
    }

    #[test]
    fn disabled_by_default() {
        let c = SpatialBridgeConfig::from_config(None).unwrap();
        assert!(!c.enabled);
        assert!(c.mappings.is_empty());
    }

    #[test]
    fn a_full_mapping_parses() {
        let c = load(
            "ros2:\n  spatial_bridge:\n    enabled: true\n    mappings:\n      \
             - message: pose_stamped\n        spatial_topic: sdds/default/FramedPose\n        \
             ros_topic: /robot1/pose\n        direction: both\n",
        )
        .unwrap();
        assert!(c.enabled);
        assert_eq!(c.mappings.len(), 1);
        let m = &c.mappings[0];
        assert_eq!(m.message, SpatialMessageKind::PoseStamped);
        assert_eq!(m.spatial_topic, "sdds/default/FramedPose");
        assert_eq!(m.ros_topic, "/robot1/pose");
        assert_eq!(m.direction, Direction::Both);
    }

    #[test]
    fn direction_defaults_to_both() {
        let c = load(
            "ros2:\n  spatial_bridge:\n    mappings:\n      - message: imu\n        \
             spatial_topic: sdds/default/Imu\n        ros_topic: /imu\n",
        )
        .unwrap();
        assert_eq!(c.mappings[0].direction, Direction::Both);
    }

    #[test]
    fn tf_message_is_refused_by_name() {
        let err = load(
            "ros2:\n  spatial_bridge:\n    mappings:\n      - message: tf_message\n        \
             spatial_topic: t\n        ros_topic: /tf\n",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("not bridged"), "{err}");
    }

    #[test]
    fn unknown_message_kind_is_refused() {
        let err = load(
            "ros2:\n  spatial_bridge:\n    mappings:\n      - message: bogus\n        \
             spatial_topic: t\n        ros_topic: /r\n",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("pose_stamped"), "{err}");
    }

    #[test]
    fn a_mapping_missing_a_required_field_is_refused() {
        for yaml in [
            "ros2:\n  spatial_bridge:\n    mappings:\n      - spatial_topic: t\n        ros_topic: /r\n",
            "ros2:\n  spatial_bridge:\n    mappings:\n      - message: imu\n        ros_topic: /r\n",
            "ros2:\n  spatial_bridge:\n    mappings:\n      - message: imu\n        spatial_topic: t\n",
        ] {
            load(yaml).expect_err(yaml);
        }
    }

    #[test]
    fn an_unknown_key_is_refused() {
        let err = load("ros2:\n  spatial_bridge:\n    enable: true\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("enable"), "{err}");
    }

    #[test]
    fn message_kind_round_trips_through_its_config_spelling() {
        for (name, kind) in [
            ("pose_stamped", SpatialMessageKind::PoseStamped),
            ("transform_stamped", SpatialMessageKind::TransformStamped),
            ("geo_pose_stamped", SpatialMessageKind::GeoPoseStamped),
            ("detection3d", SpatialMessageKind::Detection3D),
            ("detection3d_array", SpatialMessageKind::Detection3DArray),
            ("imu", SpatialMessageKind::Imu),
        ] {
            assert_eq!(SpatialMessageKind::parse(name), Some(kind));
        }
        assert_eq!(SpatialMessageKind::parse("tf_message"), None);
        assert_eq!(SpatialMessageKind::parse("nope"), None);
    }

    /// Every mapped kind's ROS type name matches
    /// `spatial_ros2::naming::ros_type_name` exactly — the two are written
    /// independently (one as this module's compile-time `TYPE_NAME`
    /// constants, one as the naming module's runtime formatter), and
    /// nothing else catches them drifting apart.
    #[test]
    fn wrapper_type_names_match_the_naming_module() {
        for (kind, expected) in [
            (
                SpatialMessageKind::PoseStamped,
                BridgePoseStamped::TYPE_NAME,
            ),
            (
                SpatialMessageKind::TransformStamped,
                BridgeTransformStamped::TYPE_NAME,
            ),
            (
                SpatialMessageKind::GeoPoseStamped,
                BridgeGeoPoseStamped::TYPE_NAME,
            ),
            (
                SpatialMessageKind::Detection3D,
                BridgeVisionDetection3D::TYPE_NAME,
            ),
            (
                SpatialMessageKind::Detection3DArray,
                BridgeVisionDetection3DArray::TYPE_NAME,
            ),
            (SpatialMessageKind::Imu, BridgeImu::TYPE_NAME),
        ] {
            assert_eq!(kind.ros_message().dds_type(), expected);
        }
    }

    #[test]
    fn wrapper_types_round_trip_the_dds_type_wire_format() {
        let sample = BridgeFramedPose(zerodds_spatial_dds::spatial::core::FramedPose {
            pose: zerodds_spatial_dds::spatial::core::PoseSE3 {
                t: [1.0, 2.0, 3.0],
                q: [0.0, 0.0, 0.0, 1.0],
            },
            frame_ref: zerodds_spatial_dds::spatial::common::FrameRef {
                fqn: "map".to_string(),
                ..Default::default()
            },
            cov: Default::default(),
            stamp: zerodds_spatial_dds::builtin::Time { sec: 1, nanosec: 2 },
        });
        let mut bytes = Vec::new();
        sample.encode(&mut bytes).expect("encode");
        let back = BridgeFramedPose::decode(&bytes).expect("decode");
        assert_eq!(back, sample);
    }
}
