// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! zerodds-tcp-transport-1.0 §6.1 — against a running Fast DDS over TCP.
//!
//! The shapes client of `tests/interop/ci/fastdds` (`fastdds_shapes`)
//! runs with its `tcp` option: Fast DDS's TCPv4 transport alone, no UDP
//! and no shared memory, so its SPDP, SEDP and samples reach a ZeroDDS
//! participant over TCP or not at all. The ZeroDDS participant sends no
//! SPDP multicast and announces no UDP metatraffic locator
//! (`discovery.unicast_transports = tcp`), with
//! `transport.tcp_dialect = fastdds`: its locators are of kind 4 and hold
//! the logical ports of the participant, and the connections it opens
//! speak RTCP, Fast DDS's TCP protocol. The connections it accepts it
//! detects (§4.2).
//!
//! Three topologies:
//!
//! 1. Fast DDS publishes and has a listener: ZeroDDS names it in its
//!    `discovery.peers` (`tcp://127.0.0.1:port`, asked at the metatraffic
//!    unicast ports of the first participant ids) and connects.
//! 2. Fast DDS publishes and has no listener: it connects to the listener
//!    of ZeroDDS (`transport.tcp_port`, its initial peer) and expects the
//!    answers on that connection.
//! 3. ZeroDDS publishes to a Fast DDS subscriber with a listener.
//!
//! The oracle is Fast DDS: `fastdds_shapes` writes and reads the
//! `ShapeType` of the OMG shapes demo with the type support Fast-DDS-Gen
//! generated. In 1 and 2 the samples Fast DDS wrote arrive at a ZeroDDS
//! reader; in 3 `fastdds_shapes sub` prints a `SAMPLE` line per sample it
//! took and checks its fields.
//!
//! `#[ignore]` in the crate tier: it needs `fastdds_shapes` (the
//! executable `FASTDDS_SHAPES` names) and Fast DDS's libraries. It runs in
//! the job `vendor-live-dcps` (`ci/interop-omg-weekly.yml`, the interop
//! image with Fast DDS 3.6.1).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::net::{Ipv4Addr, TcpListener};
use std::time::{Duration, Instant};

use zerodds_dcps::interop::ShapeType;
use zerodds_dcps::runtime::{RuntimeConfig, TcpDialect, UserTransportKind};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipant, DomainParticipantFactory,
    DomainParticipantQos, PublisherQos, SubscriberQos, TopicQos,
};
use zerodds_rtps::wire_types::VendorId;

#[path = "common/cross_vendor.rs"]
mod cross_vendor;

/// A TCP port the system just gave out and that is free again.
fn free_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .and_then(|listener| listener.local_addr())
        .expect("a free TCP port")
        .port()
}

/// A participant that has TCP alone towards another process, in the
/// dialect of Fast DDS; see the module comment.
fn tcp_only(domain: i32, tcp_port: u16, peers: Vec<String>) -> DomainParticipant {
    let config = RuntimeConfig {
        tick_period: Duration::from_millis(20),
        spdp_period: Duration::from_millis(200),
        spdp_multicast_send: false,
        user_multicast: false,
        same_host_shm: false,
        metatraffic_over_udp: false,
        user_transport: Some(UserTransportKind::TcpV4),
        tcp_dialect: TcpDialect::FastDds,
        tcp_port,
        initial_peer_specs: peers,
        ..RuntimeConfig::default()
    };
    DomainParticipantFactory::instance()
        .create_participant_with_config(domain, DomainParticipantQos::default(), config)
        .expect("participant")
}

fn reliable_reader_qos() -> DataReaderQos {
    let mut qos = DataReaderQos::default();
    qos.reliability.kind = zerodds_dcps::qos::ReliabilityKind::Reliable;
    qos
}

/// Takes from a reader of `Square` until five samples of Fast DDS have
/// arrived, checks their fields and that the Fast DDS participant is
/// known by TCP locators that hold a logical port.
fn five_samples_of_fastdds_arrive(
    participant: &DomainParticipant,
    fastdds: &mut cross_vendor::VendorChild,
) {
    let topic = participant
        .create_topic::<ShapeType>("Square", TopicQos::default())
        .expect("topic");
    let reader = participant
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<ShapeType>(&topic, reliable_reader_qos())
        .expect("reader");
    let mut samples = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(20);
    while samples.len() < 5 {
        fastdds.ensure_running().expect("fastdds_shapes runs");
        assert!(
            Instant::now() < deadline,
            "{} samples of Fast DDS arrived in 20 s; fastdds_shapes wrote:\n{}",
            samples.len(),
            fastdds.output().unwrap_or_default()
        );
        if reader.wait_for_data(Duration::from_millis(500)).is_ok() {
            samples.extend(reader.take().expect("take"));
        }
    }
    for sample in &samples {
        assert_eq!(sample.color, "RED");
        assert!((50..150).contains(&sample.x), "{sample:?}");
        assert!((60..160).contains(&sample.y), "{sample:?}");
        assert_eq!(sample.shapesize, 30);
    }
    let runtime = participant.runtime().expect("an online participant");
    let known = runtime.discovered_participants();
    let fastdds_peers: Vec<_> = known
        .iter()
        .filter(|p| p.sender_vendor == VendorId::FASTDDS)
        .collect();
    assert_eq!(fastdds_peers.len(), 1, "{known:?}");
    let data = &fastdds_peers[0].data;
    assert!(!data.metatraffic_unicast_locators.is_empty());
    for locator in data
        .metatraffic_unicast_locators
        .iter()
        .chain(&data.default_unicast_locators)
    {
        assert_eq!(locator.kind.as_i32(), 4, "{locator:?}");
        assert!(
            locator.port > u32::from(u16::MAX),
            "a Fast DDS TCP locator holds a logical port: {locator:?}"
        );
    }
}

#[test]
#[ignore = "needs Fast DDS's fastdds_shapes (vendor-live-dcps)"]
fn tcp_6_1_fastdds_tcp_publisher_reaches_a_zerodds_subscriber() {
    if !cross_vendor::require_or_skip("fastdds_live_tcp") {
        return;
    }
    let lease = zerodds_test_domains::lease_domain();
    let fastdds_port = free_port();
    let domain = lease.id().to_string();
    let listening = fastdds_port.to_string();
    let mut fastdds = cross_vendor::start_fastdds_shapes(
        "fastdds-tcp-listening-pub",
        &[
            "pub", "Square", &domain, "reliable", "volatile", "tcp", &listening,
        ],
    )
    .expect("start fastdds_shapes");
    let participant = tcp_only(
        lease.id(),
        0,
        vec![format!("tcp://127.0.0.1:{fastdds_port}")],
    );
    five_samples_of_fastdds_arrive(&participant, &mut fastdds);
    // Fast DDS announced its listener: the physical port of its locators.
    let runtime = participant.runtime().expect("an online participant");
    let known = runtime.discovered_participants();
    let data = &known
        .iter()
        .find(|p| p.sender_vendor == VendorId::FASTDDS)
        .expect("the Fast DDS participant")
        .data;
    assert_eq!(
        data.metatraffic_unicast_locators[0].port & 0xFFFF,
        u32::from(fastdds_port)
    );
}

#[test]
#[ignore = "needs Fast DDS's fastdds_shapes (vendor-live-dcps)"]
fn tcp_6_1_a_fastdds_client_without_a_listening_port_is_answered_on_its_connection() {
    if !cross_vendor::require_or_skip("fastdds_live_tcp") {
        return;
    }
    let lease = zerodds_test_domains::lease_domain();
    let port = free_port();
    let participant = tcp_only(lease.id(), port, Vec::new());
    let domain = lease.id().to_string();
    let peer = format!("127.0.0.1:{port}");
    let mut fastdds = cross_vendor::start_fastdds_shapes(
        "fastdds-tcp-client-pub",
        &[
            "pub", "Square", &domain, "reliable", "volatile", "tcp", "0", &peer,
        ],
    )
    .expect("start fastdds_shapes");
    // Fast DDS has no listener: the SPDP, SEDP and the acknowledgements
    // that let its reliable writer go on reached it on the connection it
    // opened, or the samples would not arrive.
    five_samples_of_fastdds_arrive(&participant, &mut fastdds);
}

#[test]
#[ignore = "needs Fast DDS's fastdds_shapes (vendor-live-dcps)"]
fn tcp_6_1_zerodds_publisher_reaches_a_fastdds_tcp_subscriber() {
    if !cross_vendor::require_or_skip("fastdds_live_tcp") {
        return;
    }
    let lease = zerodds_test_domains::lease_domain();
    let fastdds_port = free_port();
    let domain = lease.id().to_string();
    let listening = fastdds_port.to_string();
    let mut fastdds = cross_vendor::start_fastdds_shapes(
        "fastdds-tcp-sub",
        &[
            "sub", "Square", &domain, "reliable", "volatile", "tcp", &listening,
        ],
    )
    .expect("start fastdds_shapes");
    let participant = tcp_only(
        lease.id(),
        0,
        vec![format!("tcp://127.0.0.1:{fastdds_port}")],
    );
    let topic = participant
        .create_topic::<ShapeType>("Square", TopicQos::default())
        .expect("topic");
    let writer = participant
        .create_publisher(PublisherQos::default())
        .create_datawriter::<ShapeType>(&topic, DataWriterQos::default())
        .expect("writer");
    writer
        .wait_for_matched_subscription(1, Duration::from_secs(15))
        .unwrap_or_else(|e| {
            panic!(
                "the writer matches Fast DDS's reader: {e:?}; fastdds_shapes wrote:\n{}",
                fastdds.output().unwrap_or_default()
            )
        });
    let taken = |log: &str| log.lines().filter(|l| l.starts_with("SAMPLE ")).count();
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut i = 0i32;
    loop {
        fastdds.ensure_running().expect("fastdds_shapes runs");
        writer
            .write(&ShapeType::new("RED", 50 + i % 100, 60 + i % 100, 30))
            .expect("write");
        i += 1;
        std::thread::sleep(Duration::from_millis(20));
        let log = fastdds.output().unwrap_or_default();
        if taken(&log) >= 5 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "fastdds_shapes took {} samples of {i} written in 15 s; it wrote:\n{log}",
            taken(&log)
        );
    }
}
