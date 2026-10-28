// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! C2(6) — in-process: a SpatialDDS writer -> `spatial_bridge`
//! translation -> a plain DCPS reader of the ROS 2 type, and back.
//!
//! Everything (the bridge and both test endpoints) shares **one**
//! participant, so delivery is the same-runtime direct channel handoff
//! `zerodds-dcps`'s `ordered_access_e2e.rs` uses for the same reason: no
//! UDP loopback, no platform gate, no discovery wait. `spatial_bridge`'s
//! own field-fidelity observability event is checked too, via a `VecSink`
//! — the "not silent" half of C2(6)'s spatial-bridge requirement.
//!
//! What this does **not** cover (per the crate's own README): a real
//! `ros2 topic echo` against a native ROS 2 node. That is a manual,
//! human-run check — the exact commands are in
//! `crates/rmw-zerodds-shim/README.md`.
//!
//! The two tests at the end pin the bridge's durability (DDS 1.4
//! §2.2.3.4), the fix for a sample the bridge forwarded before its own
//! writer had discovered the reader on the other side: its writers offer
//! TRANSIENT_LOCAL, so a TRANSIENT_LOCAL reader created afterwards still
//! receives that sample — and a VOLATILE reader created afterwards receives
//! none of it. The second half is what keeps the first honest: a reader
//! created before the pump's write would have received the sample on the
//! live path whatever durability it asked for, and the VOLATILE test would
//! say so.

#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use std::time::Duration;

use rmw_zerodds::spatial_bridge::{
    BridgeFramedPose, BridgePoseStamped, Direction, SpatialBridge, SpatialBridgeConfig,
    SpatialMessageKind, TopicMapping,
};
use zerodds_dcps::qos::{
    DurabilityKind, DurabilityQosPolicy, HistoryKind, HistoryQosPolicy, ReliabilityKind,
    ReliabilityQosPolicy,
};
use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipant, DomainParticipantFactory,
    DomainParticipantQos, PublisherQos, SubscriberQos, Topic, TopicQos,
};
use zerodds_foundation::observability::VecSink;
use zerodds_spatial_dds::builtin::Time as SpatialTime;
use zerodds_spatial_dds::spatial::common::FrameRef;
use zerodds_spatial_dds::spatial::core::{FramedPose, PoseSE3};

/// A test publisher: RELIABLE + KeepLast(10), VOLATILE durability.
///
/// The bridge's own readers request VOLATILE, and a VOLATILE publisher
/// satisfies a requested VOLATILE (DDS 1.4 §2.2.3, table "QoS
/// compatibility", DURABILITY "RxO: Yes" — offered >= requested), so this
/// writer matches both the bridge's SpatialDDS-side reader and its
/// ROS-side one.
fn qos_writer() -> DataWriterQos {
    DataWriterQos {
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

/// A test subscriber: RELIABLE + KeepLast(10), durability as asked.
///
/// The bridge's writers offer TRANSIENT_LOCAL (`ros_qos_writer`), which
/// satisfies a requested VOLATILE *and* a requested TRANSIENT_LOCAL — the
/// same clause the other way round is why the bridge's own readers stay
/// VOLATILE.
fn qos_reader(durability: DurabilityKind) -> DataReaderQos {
    DataReaderQos {
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            ..Default::default()
        },
        durability: DurabilityQosPolicy { kind: durability },
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 10,
        },
        ..Default::default()
    }
}

/// How long a pump is given to forward a sample before a reader is created
/// on its output side. `pump` blocks in `wait_for_data(200 ms)` and writes
/// in the same pass, so this is ten times that poll period — and the
/// VOLATILE-reader test waits the same time, which is what makes the pair
/// below a real check: a reader created before the pump's write would have
/// been served the sample live, VOLATILE or not, and the second test would
/// have failed.
const FORWARD_SETTLE: Duration = Duration::from_secs(2);

fn sample(x: f64, frame: &str, sec: i32) -> BridgeFramedPose {
    BridgeFramedPose(FramedPose {
        pose: PoseSE3 {
            t: [x, 2.0, 3.0],
            q: [0.0, 0.0, 0.0, 1.0],
        },
        frame_ref: FrameRef {
            fqn: frame.to_string(),
            ..Default::default()
        },
        cov: Default::default(),
        stamp: SpatialTime { sec, nanosec: 0 },
    })
}

#[test]
fn spatial_dds_writer_reaches_a_plain_dcps_reader_of_the_ros_type_and_back() {
    let domain = zerodds_test_domains::unique_domain();
    // The generated SpatialDDS/ROS 2 codecs only emit XCDR2 (see
    // spatial_bridge.rs's own `start()`); this participant offers only
    // that, same as the bridge's own.
    let runtime_config = RuntimeConfig {
        data_representation_offer: vec![zerodds_rtps::publication_data::data_representation::XCDR2],
        ..RuntimeConfig::default()
    };
    let participant = DomainParticipantFactory::instance()
        .create_participant_with_config(domain, DomainParticipantQos::default(), runtime_config)
        .expect("participant");

    let spatial_topic_name = "sdds/test/FramedPose";
    let ros_topic_name = "/robot/pose";

    let config = SpatialBridgeConfig {
        enabled: true,
        mappings: vec![TopicMapping {
            message: SpatialMessageKind::PoseStamped,
            spatial_topic: spatial_topic_name.to_string(),
            ros_topic: ros_topic_name.to_string(),
            direction: Direction::Both,
        }],
    };
    let sink = std::sync::Arc::new(VecSink::new());
    let bridge = SpatialBridge::start_on_participant(participant.clone(), &config, sink.clone())
        .expect("bridge starts")
        .expect("enabled -> Some");

    // ---- dds_to_ros: a SpatialDDS FramedPose write reaches a plain DCPS
    // reader of the ROS type. ----
    let spatial_topic = participant
        .create_topic::<BridgeFramedPose>(spatial_topic_name, TopicQos::default())
        .expect("spatial topic");
    let ros_dds_topic_name = zerodds_spatial_ros2::naming::ros_topic(ros_topic_name);
    let ros_topic = participant
        .create_topic::<BridgePoseStamped>(&ros_dds_topic_name, TopicQos::default())
        .expect("ros topic");

    let spatial_publisher = participant.create_publisher(PublisherQos::default());
    let spatial_writer = spatial_publisher
        .create_datawriter::<BridgeFramedPose>(&spatial_topic, qos_writer())
        .expect("spatial writer");
    let ros_subscriber = participant.create_subscriber(SubscriberQos::default());
    let ros_reader = ros_subscriber
        .create_datareader::<BridgePoseStamped>(&ros_topic, qos_reader(DurabilityKind::Volatile))
        .expect("ros reader");

    let sent = sample(1.0, "map", 5);
    spatial_writer.write(&sent).expect("write spatial sample");

    ros_reader
        .wait_for_data(Duration::from_secs(5))
        .expect("ros reader sees data");
    let received = ros_reader.take().expect("take");
    assert_eq!(received.len(), 1, "{received:?}");
    assert_eq!(received[0].0.header.frame_id, "map");
    assert_eq!(received[0].0.pose.position.x, 1.0);
    assert_eq!(received[0].0.header.stamp.sec, 5);

    // ---- ros_to_dds: a plain DCPS write of the ROS type reaches a
    // SpatialDDS reader, converted back. ----
    let ros_publisher = participant.create_publisher(PublisherQos::default());
    let ros_writer = ros_publisher
        .create_datawriter::<BridgePoseStamped>(&ros_topic, qos_writer())
        .expect("ros writer");
    let spatial_subscriber = participant.create_subscriber(SubscriberQos::default());
    let spatial_reader = spatial_subscriber
        .create_datareader::<BridgeFramedPose>(&spatial_topic, qos_reader(DurabilityKind::Volatile))
        .expect("spatial reader");

    let ros_sample = BridgePoseStamped(zerodds_spatial_ros2::convert::framed_pose_to_ros(
        &sample(9.0, "odom", 7).0,
    ));
    ros_writer.write(&ros_sample).expect("write ros sample");

    spatial_reader
        .wait_for_data(Duration::from_secs(5))
        .expect("spatial reader sees data");
    let back = spatial_reader.take().expect("take");
    assert_eq!(back.len(), 1, "{back:?}");
    assert_eq!(back[0].0.frame_ref.fqn, "odom");
    assert_eq!(back[0].0.pose.t[0], 9.0);
    assert_eq!(back[0].0.stamp.sec, 7);

    // ---- observability: the field-fidelity loss is reported, not silent.
    let events = sink.snapshot();
    assert!(
        events
            .iter()
            .any(|e| e.name == "spatial_bridge.field_fidelity"
                && e.attrs
                    .iter()
                    .any(|a| a.key == "direction" && a.value == "dds_to_ros")),
        "{events:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| e.name == "spatial_bridge.field_fidelity"
                && e.attrs
                    .iter()
                    .any(|a| a.key == "direction" && a.value == "ros_to_dds")),
        "{events:?}"
    );

    drop(bridge);
}

/// A bridge on a participant of its own, over two single-direction
/// mappings — everything the two durability tests below need before they
/// write anything. One direction per mapping, and a topic pair each: an
/// output topic with no reader at all is the window the measurement is
/// about, and a `both` mapping would put the sibling pump's own reader on
/// it.
struct Fixture {
    participant: DomainParticipant,
    /// `dds_to_ros`: the SpatialDDS topic the bridge reads, and the ROS
    /// topic it publishes the converted `PoseStamped` on.
    spatial_in: Topic<BridgeFramedPose>,
    ros_out: Topic<BridgePoseStamped>,
    /// `ros_to_dds`: the ROS topic the bridge reads, and the SpatialDDS
    /// topic it publishes the converted `FramedPose` on.
    ros_in: Topic<BridgePoseStamped>,
    spatial_out: Topic<BridgeFramedPose>,
    /// Declared last so it drops last: the pumps are stopped and joined
    /// while the participant and its topics are still alive.
    _bridge: SpatialBridge,
}

impl Fixture {
    /// The two mappings above, over topic names carrying `suffix`, on a
    /// leased domain of this participant's own
    /// (`zerodds_test_domains::unique_domain` hands out a different one
    /// per call, so the tests in this file do not share a DDS domain).
    /// The participant offers only the XCDR2 the generated SpatialDDS /
    /// ROS 2 codecs emit, as `spatial_bridge.rs`'s own `start()` does.
    fn start(suffix: &str) -> Self {
        let runtime_config = RuntimeConfig {
            data_representation_offer: vec![
                zerodds_rtps::publication_data::data_representation::XCDR2,
            ],
            ..RuntimeConfig::default()
        };
        let participant = DomainParticipantFactory::instance()
            .create_participant_with_config(
                zerodds_test_domains::unique_domain(),
                DomainParticipantQos::default(),
                runtime_config,
            )
            .expect("participant");

        let mapping = |spatial_topic: &str, ros_topic: &str, direction| TopicMapping {
            message: SpatialMessageKind::PoseStamped,
            spatial_topic: spatial_topic.to_string(),
            ros_topic: ros_topic.to_string(),
            direction,
        };
        let config = SpatialBridgeConfig {
            enabled: true,
            mappings: vec![
                mapping(
                    &format!("sdds/{suffix}/FramedPose"),
                    &format!("/robot/{suffix}/pose"),
                    Direction::DdsToRos,
                ),
                mapping(
                    &format!("sdds/{suffix}/OdomPose"),
                    &format!("/robot/{suffix}/odom"),
                    Direction::RosToDds,
                ),
            ],
        };
        let sink = std::sync::Arc::new(VecSink::new());
        let bridge =
            SpatialBridge::start_on_participant(participant.clone(), &config, sink.clone())
                .expect("bridge starts")
                .expect("enabled -> Some");

        let ros_topic = |logical: &str| {
            participant
                .create_topic::<BridgePoseStamped>(
                    &zerodds_spatial_ros2::naming::ros_topic(logical),
                    TopicQos::default(),
                )
                .expect("ros topic")
        };
        let spatial_topic = |name: &str| {
            participant
                .create_topic::<BridgeFramedPose>(name, TopicQos::default())
                .expect("spatial topic")
        };
        let spatial_in = spatial_topic(&config.mappings[0].spatial_topic);
        let ros_out = ros_topic(&config.mappings[0].ros_topic);
        let ros_in = ros_topic(&config.mappings[1].ros_topic);
        let spatial_out = spatial_topic(&config.mappings[1].spatial_topic);
        Self {
            participant,
            spatial_in,
            ros_out,
            ros_in,
            spatial_out,
            _bridge: bridge,
        }
    }

    /// A SpatialDDS publisher on the input side of the `dds_to_ros`
    /// mapping — matched by the bridge's VOLATILE reader.
    fn spatial_writer(&self) -> zerodds_dcps::DataWriter<BridgeFramedPose> {
        self.participant
            .create_publisher(PublisherQos::default())
            .create_datawriter::<BridgeFramedPose>(&self.spatial_in, qos_writer())
            .expect("spatial writer")
    }

    /// A ROS publisher on the input side of the `ros_to_dds` mapping —
    /// matched by the bridge's VOLATILE reader.
    fn ros_writer(&self) -> zerodds_dcps::DataWriter<BridgePoseStamped> {
        self.participant
            .create_publisher(PublisherQos::default())
            .create_datawriter::<BridgePoseStamped>(&self.ros_in, qos_writer())
            .expect("ros writer")
    }
}

/// The sample a `FramedPose` writes into the ROS type it is bridged as.
fn as_ros(s: &BridgeFramedPose) -> BridgePoseStamped {
    BridgePoseStamped(zerodds_spatial_ros2::convert::framed_pose_to_ros(&s.0))
}

/// A TRANSIENT_LOCAL reader created **after** the bridge forwarded a
/// sample onto a topic nobody was reading yet still receives it: DDS 1.4
/// §2.2.3.4, a TRANSIENT_LOCAL writer delivers the samples its HISTORY
/// holds to a reader that matches later. Both directions — this is the
/// pose that was reaching nobody on the Linux test host (59 of 60 `ros_to_dds`, 57 of
/// 58 `dds_to_ros`), because the bridge's writers were VOLATILE.
#[test]
fn a_late_transient_local_reader_gets_what_the_bridge_forwarded_before_it_existed() {
    let fx = Fixture::start("late-tl");
    let subscriber = fx.participant.create_subscriber(SubscriberQos::default());

    // ---- dds_to_ros ----
    let spatial_writer = fx.spatial_writer();
    spatial_writer
        .write(&sample(1.0, "map", 5))
        .expect("write spatial sample");
    // Nothing reads the ROS topic yet: this is the window in which the
    // forwarded pose used to be lost.
    std::thread::sleep(FORWARD_SETTLE);
    let ros_reader = subscriber
        .create_datareader::<BridgePoseStamped>(
            &fx.ros_out,
            qos_reader(DurabilityKind::TransientLocal),
        )
        .expect("late ros reader");
    ros_reader
        .wait_for_data(Duration::from_secs(5))
        .expect("the late TRANSIENT_LOCAL reader gets the forwarded sample");
    let received = ros_reader.take().expect("take");
    assert_eq!(received.len(), 1, "{received:?}");
    assert_eq!(received[0].0.header.frame_id, "map");
    assert_eq!(received[0].0.pose.position.x, 1.0);
    assert_eq!(received[0].0.header.stamp.sec, 5);

    // ---- ros_to_dds ----
    let ros_writer = fx.ros_writer();
    ros_writer
        .write(&as_ros(&sample(9.0, "odom", 7)))
        .expect("write ros sample");
    std::thread::sleep(FORWARD_SETTLE);
    let spatial_reader = subscriber
        .create_datareader::<BridgeFramedPose>(
            &fx.spatial_out,
            qos_reader(DurabilityKind::TransientLocal),
        )
        .expect("late spatial reader");
    spatial_reader
        .wait_for_data(Duration::from_secs(5))
        .expect("the late TRANSIENT_LOCAL reader gets the forwarded sample");
    let back = spatial_reader.take().expect("take");
    assert_eq!(back.len(), 1, "{back:?}");
    assert_eq!(back[0].0.frame_ref.fqn, "odom");
    assert_eq!(back[0].0.pose.t[0], 9.0);
    assert_eq!(back[0].0.stamp.sec, 7);
}

/// The same two samples into a **VOLATILE** reader created after the same
/// delay: a VOLATILE reader never receives samples written before it
/// matched (DDS 1.4 §2.2.3.4), although it does match the bridge's
/// TRANSIENT_LOCAL writer — offered TRANSIENT_LOCAL satisfies a requested
/// VOLATILE (§2.2.3, table "QoS compatibility", DURABILITY "RxO: Yes").
/// The match and the post-match delivery are both asserted, so an empty
/// reader here cannot be a reader that never matched.
#[test]
fn a_late_volatile_reader_gets_none_of_what_the_bridge_forwarded_before_it_existed() {
    let fx = Fixture::start("late-volatile");
    let subscriber = fx.participant.create_subscriber(SubscriberQos::default());

    // ---- dds_to_ros ----
    let spatial_writer = fx.spatial_writer();
    spatial_writer
        .write(&sample(1.0, "map", 5))
        .expect("write spatial sample");
    std::thread::sleep(FORWARD_SETTLE);
    let ros_reader = subscriber
        .create_datareader::<BridgePoseStamped>(&fx.ros_out, qos_reader(DurabilityKind::Volatile))
        .expect("late ros reader");
    ros_reader
        .wait_for_matched_publication(1, Duration::from_secs(5))
        .expect("a VOLATILE reader still matches the bridge's TRANSIENT_LOCAL writer");
    std::thread::sleep(FORWARD_SETTLE);
    let pre_match = ros_reader.take().expect("take");
    assert!(
        pre_match.is_empty(),
        "a VOLATILE reader must not be served what the bridge wrote before it matched, got \
         {pre_match:?}"
    );
    // Live delivery works, so the empty take above was the durability and
    // not a reader that never matched: the next sample does arrive.
    spatial_writer
        .write(&sample(2.0, "map", 6))
        .expect("write spatial sample");
    ros_reader
        .wait_for_data(Duration::from_secs(5))
        .expect("the live sample arrives at the VOLATILE reader");
    let live = ros_reader.take().expect("take");
    assert_eq!(live.len(), 1, "{live:?}");
    assert_eq!(live[0].0.pose.position.x, 2.0);

    // ---- ros_to_dds ----
    let ros_writer = fx.ros_writer();
    ros_writer
        .write(&as_ros(&sample(9.0, "odom", 7)))
        .expect("write ros sample");
    std::thread::sleep(FORWARD_SETTLE);
    let spatial_reader = subscriber
        .create_datareader::<BridgeFramedPose>(
            &fx.spatial_out,
            qos_reader(DurabilityKind::Volatile),
        )
        .expect("late spatial reader");
    spatial_reader
        .wait_for_matched_publication(1, Duration::from_secs(5))
        .expect("a VOLATILE reader still matches the bridge's TRANSIENT_LOCAL writer");
    std::thread::sleep(FORWARD_SETTLE);
    let pre_match = spatial_reader.take().expect("take");
    assert!(
        pre_match.is_empty(),
        "a VOLATILE reader must not be served what the bridge wrote before it matched, got \
         {pre_match:?}"
    );
    ros_writer
        .write(&as_ros(&sample(8.0, "odom", 8)))
        .expect("write ros sample");
    spatial_reader
        .wait_for_data(Duration::from_secs(5))
        .expect("the live sample arrives at the VOLATILE reader");
    let live = spatial_reader.take().expect("take");
    assert_eq!(live.len(), 1, "{live:?}");
    assert_eq!(live[0].0.pose.t[0], 8.0);
}
