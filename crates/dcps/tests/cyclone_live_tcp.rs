// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! zerodds-tcp-transport-1.0 §4.1 and §4.3 — against a running Cyclone
//! DDS over TCP.
//!
//! Cyclone's own `ddsperf` runs with `General/Transport` = `tcp`: it has
//! no UDP socket, so its SPDP, SEDP and samples reach a ZeroDDS
//! participant over TCP or not at all. The ZeroDDS participant sends no
//! SPDP multicast and announces no UDP metatraffic locator
//! (`discovery.unicast_transports = tcp`), with
//! `transport.tcp_dialect = cyclone`: its locators carry the kinds 4 and
//! 8 Cyclone DDS reads, and the connections it opens have Cyclone's stream
//! framing. The connections it accepts it detects (§4.2).
//!
//! Three topologies:
//!
//! 1. Cyclone publishes and has no listener (`TCP/Port` unset): it
//!    connects to the listener of ZeroDDS (`transport.tcp_port`, named in
//!    Cyclone's `Discovery/Peers`), announces no unicast locator and
//!    expects its answers on that connection.
//! 2. Cyclone publishes and has a listener (`TCP/Port`): ZeroDDS connects
//!    to it (`discovery.peers = tcp://127.0.0.1:port`).
//! 3. ZeroDDS publishes to a Cyclone subscriber without a listener.
//!
//! The oracle is Cyclone: `ddsperf -T OU` writes and reads its `OneULong`
//! samples (`struct OneULong { unsigned long seq; }`) on the topic
//! `DDSPerfRDataOU`. In 1 and 2 the samples Cyclone wrote arrive at a
//! ZeroDDS reader with rising sequence numbers; in 3 `ddsperf sub` counts
//! the samples it received.
//!
//! `#[ignore]` in the crate tier: it needs `ddsperf` (on `PATH`, or the
//! executable `DDSPERF` names). It runs in the job `vendor-live-dcps`
//! (`ci/interop-omg-weekly.yml`, the interop image with Cyclone).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::net::{Ipv4Addr, TcpListener};
use std::time::{Duration, Instant};

use zerodds_dcps::dds_type::{DdsType, DecodeError, EncodeError};
use zerodds_dcps::runtime::{RuntimeConfig, TcpDialect, UserTransportKind};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipant, DomainParticipantFactory,
    DomainParticipantQos, PublisherQos, SubscriberQos, TopicQos,
};
use zerodds_rtps::wire_types::VendorId;

#[path = "common/cross_vendor.rs"]
mod cross_vendor;

/// `struct OneULong { unsigned long seq; };` of Cyclone's
/// `ddsperf_types.idl`, `@final`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OneULong {
    seq: u32,
}

impl DdsType for OneULong {
    const TYPE_NAME: &'static str = "OneULong";
    /// One `unsigned long`: XCDR1 and XCDR2 write it the same way.
    const XCDR1_MATCHES_XCDR2: bool = true;

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.seq.to_le_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let seq = bytes.get(..4).ok_or(DecodeError::Invalid {
            what: "OneULong: fewer than four octets",
        })?;
        Ok(Self {
            seq: u32::from_le_bytes(seq.try_into().unwrap()),
        })
    }

    fn decode_be(bytes: &[u8]) -> Result<Self, DecodeError> {
        let seq = bytes.get(..4).ok_or(DecodeError::Invalid {
            what: "OneULong: fewer than four octets",
        })?;
        Ok(Self {
            seq: u32::from_be_bytes(seq.try_into().unwrap()),
        })
    }
}

/// A TCP port the system just gave out and that is free again.
fn free_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .and_then(|listener| listener.local_addr())
        .expect("a free TCP port")
        .port()
}

/// A participant that has TCP alone towards another process, in the
/// dialect of Cyclone DDS; see the module comment.
fn tcp_only(domain: i32, tcp_port: u16, peers: Vec<String>) -> DomainParticipant {
    let config = RuntimeConfig {
        tick_period: Duration::from_millis(20),
        spdp_period: Duration::from_millis(200),
        spdp_multicast_send: false,
        user_multicast: false,
        same_host_shm: false,
        metatraffic_over_udp: false,
        user_transport: Some(UserTransportKind::TcpV4),
        tcp_dialect: TcpDialect::Cyclone,
        tcp_port,
        initial_peer_specs: peers,
        ..RuntimeConfig::default()
    };
    DomainParticipantFactory::instance()
        .create_participant_with_config(domain, DomainParticipantQos::default(), config)
        .expect("participant")
}

/// The configuration of a Cyclone DDS participant with TCP alone:
/// `tcp_port` is its `TCP/Port` (a listener) and `peer` an entry of its
/// `Discovery/Peers`.
fn cyclone_uri(tcp_port: Option<u16>, peer: Option<u16>) -> String {
    let listener = tcp_port.map_or_else(String::new, |port| {
        format!("<TCP><Port>{port}</Port></TCP>")
    });
    let peers = peer.map_or_else(String::new, |port| {
        format!("<Discovery><Peers><Peer address=\"127.0.0.1:{port}\"/></Peers></Discovery>")
    });
    format!("<General><Transport>tcp</Transport></General>{listener}{peers}")
}

/// Takes from a reader of `DDSPerfRDataOU` until five samples of Cyclone
/// have arrived, and checks that their sequence numbers rise and that the
/// Cyclone participant is known by TCP locators alone.
fn five_samples_of_cyclone_arrive(
    participant: &DomainParticipant,
    cyclone: &mut cross_vendor::VendorChild,
) {
    let topic = participant
        .create_topic::<OneULong>("DDSPerfRDataOU", TopicQos::default())
        .expect("topic");
    let reader = participant
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<OneULong>(&topic, DataReaderQos::default())
        .expect("reader");
    let mut seqs = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(15);
    while seqs.len() < 5 {
        cyclone.ensure_running().expect("ddsperf runs");
        assert!(
            Instant::now() < deadline,
            "{} samples of ddsperf arrived in 15 s: {seqs:?}; ddsperf wrote:\n{}",
            seqs.len(),
            cyclone.output().unwrap_or_default()
        );
        if reader.wait_for_data(Duration::from_millis(500)).is_ok() {
            seqs.extend(reader.take().expect("take").into_iter().map(|s| s.seq));
        }
    }
    assert!(
        seqs.windows(2).all(|w| w[0] < w[1]),
        "the sequence numbers ddsperf wrote rise: {seqs:?}"
    );
    let runtime = participant.runtime().expect("an online participant");
    let known = runtime.discovered_participants();
    let cyclones: Vec<_> = known
        .iter()
        .filter(|p| p.sender_vendor == VendorId::CYCLONE)
        .collect();
    assert_eq!(cyclones.len(), 1, "{known:?}");
    let data = &cyclones[0].data;
    for locator in data
        .metatraffic_unicast_locators
        .iter()
        .chain(&data.default_unicast_locators)
    {
        assert!(
            locator.kind.is_tcp_v4() || locator.kind.is_tcp_v6(),
            "Cyclone is known by a locator that is not TCP: {locator:?}"
        );
    }
    assert!(!data.metatraffic_unicast_locators.is_empty());
}

#[test]
#[ignore = "needs Cyclone DDS's ddsperf (vendor-live-dcps)"]
fn tcp_4_1_a_cyclone_publisher_without_a_listener_reaches_a_zerodds_reader() {
    if !cross_vendor::require_or_skip("cyclone_live_tcp") {
        return;
    }
    let lease = zerodds_test_domains::lease_domain();
    let port = free_port();
    let participant = tcp_only(lease.id(), port, Vec::new());
    let mut cyclone = cross_vendor::start_cyclone_ddsperf_with_uri(
        "cyclone-tcp-pub",
        &cyclone_uri(None, Some(port)),
        &[
            "-i",
            &lease.id().to_string(),
            "-T",
            "OU",
            "-D",
            "20",
            "pub",
            "20Hz",
        ],
    )
    .expect("start ddsperf");
    five_samples_of_cyclone_arrive(&participant, &mut cyclone);
    // Cyclone announced no unicast locator: it is known by the address
    // it connected from, and was answered on its connection.
    let runtime = participant.runtime().expect("an online participant");
    let known = runtime.discovered_participants();
    let cyclone_data = &known
        .iter()
        .find(|p| p.sender_vendor == VendorId::CYCLONE)
        .expect("the Cyclone participant")
        .data;
    assert_ne!(
        cyclone_data.metatraffic_unicast_locators[0].port,
        u32::from(port)
    );
}

#[test]
#[ignore = "needs Cyclone DDS's ddsperf (vendor-live-dcps)"]
fn tcp_4_1_a_cyclone_publisher_with_a_listener_reaches_a_zerodds_reader() {
    if !cross_vendor::require_or_skip("cyclone_live_tcp") {
        return;
    }
    let lease = zerodds_test_domains::lease_domain();
    let cyclone_port = free_port();
    let mut cyclone = cross_vendor::start_cyclone_ddsperf_with_uri(
        "cyclone-tcp-listening-pub",
        &cyclone_uri(Some(cyclone_port), None),
        &[
            "-i",
            &lease.id().to_string(),
            "-T",
            "OU",
            "-D",
            "20",
            "pub",
            "20Hz",
        ],
    )
    .expect("start ddsperf");
    let participant = tcp_only(
        lease.id(),
        0,
        vec![format!("tcp://127.0.0.1:{cyclone_port}")],
    );
    five_samples_of_cyclone_arrive(&participant, &mut cyclone);
    // Cyclone announced its listener, with the port of its `TCP/Port`.
    let runtime = participant.runtime().expect("an online participant");
    let known = runtime.discovered_participants();
    let cyclone_data = &known
        .iter()
        .find(|p| p.sender_vendor == VendorId::CYCLONE)
        .expect("the Cyclone participant")
        .data;
    assert_eq!(
        cyclone_data.metatraffic_unicast_locators[0].port,
        u32::from(cyclone_port)
    );
}

#[test]
#[ignore = "needs Cyclone DDS's ddsperf (vendor-live-dcps)"]
fn tcp_4_1_a_zerodds_writer_reaches_a_cyclone_subscriber_without_a_listener() {
    if !cross_vendor::require_or_skip("cyclone_live_tcp") {
        return;
    }
    let lease = zerodds_test_domains::lease_domain();
    let port = free_port();
    let participant = tcp_only(lease.id(), port, Vec::new());
    let topic = participant
        .create_topic::<OneULong>("DDSPerfRDataOU", TopicQos::default())
        .expect("topic");
    let writer = participant
        .create_publisher(PublisherQos::default())
        .create_datawriter::<OneULong>(&topic, DataWriterQos::default())
        .expect("writer");
    let mut cyclone = cross_vendor::start_cyclone_ddsperf_with_uri(
        "cyclone-tcp-sub",
        &cyclone_uri(None, Some(port)),
        &["-i", &lease.id().to_string(), "-T", "OU", "-D", "20", "sub"],
    )
    .expect("start ddsperf");
    writer
        .wait_for_matched_subscription(1, Duration::from_secs(10))
        .unwrap_or_else(|e| {
            panic!(
                "the writer matches Cyclone's reader: {e:?}; ddsperf wrote:\n{}",
                cyclone.output().unwrap_or_default()
            )
        });
    // ddsperf prints a line per second with the samples it has received
    // in all ("total N").
    let received = |log: &str| -> u64 {
        log.lines()
            .filter_map(|line| {
                let rest = line.split("total ").nth(1)?;
                rest.split_whitespace().next()?.parse::<u64>().ok()
            })
            .max()
            .unwrap_or(0)
    };
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut seq = 0u32;
    loop {
        cyclone.ensure_running().expect("ddsperf runs");
        writer.write(&OneULong { seq }).expect("write");
        seq += 1;
        std::thread::sleep(Duration::from_millis(10));
        let log = cyclone.output().unwrap_or_default();
        if received(&log) >= 5 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "ddsperf counted {} samples of {seq} written in 15 s; it wrote:\n{log}",
            received(&log)
        );
    }
}
