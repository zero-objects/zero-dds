// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-bench-suite` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-dcps` (its library does not; the
//! binaries do):
//! - `src/bin/roundtrip_typed.rs`: the generated `RoundtripBench::Roundtrip`
//!   (`DdsType`, from `idl/roundtrip.idl` through the build script),
//!   `DomainParticipantFactory::create_participant`,
//!   `create_topic::<Roundtrip>` for the request and the echo topic,
//!   `create_publisher`/`create_subscriber` with their default QoS,
//!   `create_datawriter`/`create_datareader` with RELIABLE and
//!   KEEP_LAST(64) (`qos::{ReliabilityKind, HistoryKind}`),
//!   `wait_for_matched_subscription`/`wait_for_matched_publication`,
//!   `DataWriter::write`, `DataReader::take` and `clone`.
//! - `src/bin/roundtrip_1us.rs`: `runtime::UserTransportKind::{UdpV4,
//!   UdpV6, TcpV4, TcpV6, Shm}` for `--dcps-transport UDPv4|UDPv6|TCPv4|
//!   TCPv6|SHM`, put into the `RuntimeConfig` of `zerodds-c-api` as
//!   `user_transport` with `user_transports` cleared. `Shm` exists with the
//!   feature `same-host-shm` of `zerodds-dcps`, which `zerodds-c-api` turns
//!   on by default.
//!
//! What it relies on and is checked here:
//! - The bench QoS is consistent and creates the endpoints of both topics;
//!   a sample is written, a reader that received nothing takes nothing, a
//!   clone of the reader reads the same reader. Oracles: DDS 1.4 §2.2.3.18
//!   and §2.2.3.19 (KEEP_LAST depth within the RESOURCE_LIMITS, all
//!   unlimited by default), §2.2.2.4.1.5, §2.2.2.5.2.5, §2.2.2.5.3.8.
//! - `Roundtrip` has no key (the IDL's "single-instance stream"), so every
//!   sample is the one instance of the writer: DDS 1.4 §2.2.1.2.2 (a topic
//!   without a key has a single instance), §2.2.2.4.2.5 (`register_instance`
//!   is idempotent), §2.2.2.4.2.10 (`lookup_instance`).
//! - The five transport names the bench takes are the names of the
//!   setting `transport.user_transport` and select the same kinds. Oracle:
//!   `man/man5/zerodds.yaml.5`, `transport.user_transport`.
//!
//! `create_participant` opens sockets; the participant here has no
//! runtime. No socket is opened.
#![allow(clippy::unwrap_used)]

use std::path::Path;

use zerodds_dcps::qos::{HistoryKind, ReliabilityKind};
use zerodds_dcps::runtime::{ConfigSource, RuntimeConfig, UserTransportKind};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
    PublisherQos, SubscriberQos, TopicQos,
};

#[allow(dead_code, non_snake_case)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/roundtrip.rs"));
}

use generated::RoundtripBench::Roundtrip;

fn writer_qos() -> DataWriterQos {
    let mut q = DataWriterQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.history.kind = HistoryKind::KeepLast;
    q.history.depth = 64;
    q
}

fn reader_qos() -> DataReaderQos {
    let mut q = DataReaderQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.history.kind = HistoryKind::KeepLast;
    q.history.depth = 64;
    q
}

#[test]
fn dcps_2_2_3_19_the_bench_qos_creates_the_endpoints_of_both_topics() {
    assert!(writer_qos().check_consistency().is_ok());
    assert!(reader_qos().check_consistency().is_ok());
    let participant = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let request = participant
        .create_topic::<Roundtrip>("contract-dcps-RoundtripBench_Request", TopicQos::default())
        .unwrap();
    let echo = participant
        .create_topic::<Roundtrip>("contract-dcps-RoundtripBench_Echo", TopicQos::default())
        .unwrap();
    let writer = participant
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Roundtrip>(&echo, writer_qos())
        .unwrap();
    let reader = participant
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Roundtrip>(&request, reader_qos())
        .unwrap();
    let sample = Roundtrip {
        sequence_id: 1,
        t_send_ns: 2,
        payload: vec![0xAB; 64],
    };
    writer.write(&sample).unwrap();
    let clone = reader.clone();
    assert!(reader.take().unwrap().is_empty());
    assert!(clone.take().unwrap().is_empty());
    assert_eq!(clone.instance_handle(), reader.instance_handle());
}

#[test]
fn dcps_2_2_2_4_2_5_the_roundtrip_stream_is_one_instance() {
    assert!(!<Roundtrip as DdsType>::has_key());
    let participant = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let topic = participant
        .create_topic::<Roundtrip>("contract-dcps-RoundtripBench_Keyless", TopicQos::default())
        .unwrap();
    let writer = participant
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Roundtrip>(&topic, writer_qos())
        .unwrap();
    let first = Roundtrip {
        sequence_id: 1,
        t_send_ns: 10,
        payload: Vec::new(),
    };
    let second = Roundtrip {
        sequence_id: 2,
        t_send_ns: 20,
        payload: vec![1, 2, 3],
    };
    let instance = writer.register_instance(&first).unwrap();
    assert_eq!(writer.register_instance(&second).unwrap(), instance);
    assert_eq!(writer.lookup_instance(&second), instance);
    writer.write(&second).unwrap();
    assert_eq!(writer.lookup_instance(&first), instance);
}

/// `roundtrip_1us`'s `--dcps-transport` names.
fn bench_kind(name: &str) -> Option<UserTransportKind> {
    match name {
        "UDPv4" => Some(UserTransportKind::UdpV4),
        "UDPv6" => Some(UserTransportKind::UdpV6),
        "TCPv4" => Some(UserTransportKind::TcpV4),
        "TCPv6" => Some(UserTransportKind::TcpV6),
        "SHM" => Some(UserTransportKind::Shm),
        _ => None,
    }
}

#[test]
fn the_bench_transport_names_are_the_setting_names() {
    for name in ["UDPv4", "UDPv6", "TCPv4", "TCPv6", "SHM"] {
        let path = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("contract-dcps-bench-suite-{name}.yaml"));
        std::fs::write(&path, format!("transport:\n  user_transport: {name}\n")).unwrap();
        let mut config = ConfigSource::from_path(path)
            .and_then(|source| RuntimeConfig::from_source(&source))
            .unwrap();
        assert_eq!(config.user_transport, bench_kind(name), "{name}");
        assert!(bench_kind(name).is_some());
        // What the bench does before it creates the runtime.
        config.user_transport = bench_kind(name);
        config.user_transports.clear();
        assert!(config.user_transports.is_empty());
    }
    assert_eq!(bench_kind("udpv4"), None);
}
