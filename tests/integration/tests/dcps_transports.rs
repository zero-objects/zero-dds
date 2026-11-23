// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DCPS over UDPv4, TCPv4, Unix domain sockets and shared memory: a writer
//! process and a reader process, each a ZeroDDS participant made through
//! the public API (`DomainParticipantFactory`), exchange a keyed topic over
//! the configured user transport. Shared memory comes twice: the same-host
//! channel under the UDPv4 user transport (`transport.same_host_shm`), and
//! the SHM user transport (`transport.user_transport = SHM`). TCPv4 comes
//! twice as well: in the default dialect (handshake and length-prefixed
//! frames, `zerodds-tcp-transport-1.0` §3 and §2.1) and in the dialect
//! `rtps` (`transport.tcp_dialect = rtps`, §2.5), where a connection
//! carries RTPS messages back to back and each states its length in its
//! HeaderExtension (DDSI-RTPS 2.5 §8.3.3). The second run shows that the
//! reliable protocol, in both directions, works on messages that begin
//! with a HeaderExtension; that the octets on the connection are those of
//! §2.5 is tested where a plain socket reads them
//! (`crates/dcps/src/runtime.rs`,
//! `tcp_2_5_the_tcp_leg_of_the_rtps_dialect_states_the_length_of_each_message`).
//!
//! The oracle has two halves, neither of them ZeroDDS code:
//!
//! 1. **The delivery the specifications promise.** RELIABLE and KEEP_ALL on
//!    both ends (DDS 1.4 §2.2.3.14, §2.2.3.18): every one of the `SAMPLES`
//!    samples written arrives, once. A writer sends in the order of its
//!    HistoryCache (DDSI-RTPS 2.5 §8.4.2.2) and a reliable reader makes a
//!    change available only when the ones before it are (§8.4.12.2), so the
//!    samples of each instance arrive in the order written. A disposed
//!    instance reads NOT_ALIVE_DISPOSED (DDS 1.4 §2.2.2.5.1.3). The expected
//!    values are fixed before the run: the sequence numbers 0..SAMPLES, the
//!    instance of each, the one disposed instance.
//!
//!    The SHM user transport runs BEST_EFFORT on both ends instead. A
//!    best-effort reader sends nothing (DDSI-RTPS 2.5 §8.4.2.3), and on that
//!    path the SHM user transport delivered nothing: its adapter opened the
//!    ring it reads a peer from at its own first send to the peer, and the
//!    reader's process never sent one (`bench-regression`, job 229228). A
//!    RELIABLE reader sends ACKNACKs, which opened the ring too, so a
//!    reliable run over the SHM user transport does not reach that path. Best
//!    effort promises no delivery, so the expectation here is the
//!    transport's own: one writer writes into one single-producer
//!    single-consumer ring, in order (ZeroDDS SHM Transport 1.0 §4.1), and a
//!    full ring is an error, not a silent drop (§4.5); 200 samples of a few
//!    octets do not fill the 1 MiB ring, so every one arrives, once and in
//!    order. The writer starts when the reader has matched it: a best-effort
//!    reader drops what an unmatched writer sends, and nothing repeats it.
//! 2. **The kernel's view of the transport** (proc(5)), read while both
//!    processes still hold their sockets: for TCPv4 an ESTABLISHED
//!    connection between the two processes; for UDS a bound Unix socket in
//!    each under the transport's directory, and for UDS in a configured
//!    directory (`transport.uds.base_dir`, `transport.uds.socket_mode`,
//!    `transport.uds.base_dir_mode`, `zerodds-uds-transport-1.0` §5.1) the
//!    sockets under that directory with its modes; for the same-host channel a
//!    file under `/dev/shm` both have mapped. For every transport but UDPv4,
//!    no UDP socket on the user-traffic ports of the domain (DDSI-RTPS 2.5
//!    §9.6.2.2 Table 9.16, defaults §9.6.2.3), so the samples cannot have
//!    gone over UDP; for UDPv4, the reader holds its user unicast port.
//!
//! A UDS user transport whose bind fails stops the participant's start,
//! and the transport's own error reaches the observability sink
//! (`dcps_over_uds_a_refused_bind_is_reported_with_its_cause`); the
//! refusal comes from the transport's check of a `max_datagram` against
//! the socket buffer the kernel grants (`zerodds-uds-transport-1.0` §3.4).
//!
//! Discovery is SPDP/SEDP over UDPv4 for every transport (DDSI-RTPS 2.5
//! §8.5) and only the user traffic moves, except in the run `TCPv4Only`
//! (`zerodds-tcp-transport-1.0` §4.3). There the two participants have no
//! UDP path to each other: neither sends SPDP multicast
//! (`discovery.no_multicast`), each has an SPDP group of its own, neither
//! announces a UDP metatraffic locator (`discovery.unicast_transports =
//! tcp`), and the user transport is TCPv4. The reader's TCP listener has a
//! port the scenario chose (`transport.tcp_port`), and the writer knows it
//! as its one initial peer (`discovery.peers = tcp://127.0.0.1:port`). A
//! participant sends unicast only to a locator it was told, and the only
//! locators these two are told are TCP ones: SPDP, SEDP, the reliable
//! protocol and the samples go over TCP or not at all. That the writer
//! matches the reader and every sample arrives is the proof. `TCPv6Only`
//! is the same over TCPv6, with the peer entry `tcp://[::1]:port`.

#![cfg(target_os = "linux")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    missing_docs
)]

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use zerodds_dcps::qos::{HistoryKind, HistoryQosPolicy, ReliabilityKind, ReliabilityQosPolicy};
use zerodds_dcps::runtime::{RuntimeConfig, TcpDialect, UdsMode, UdsUserConfig, UserTransportKind};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DomainParticipantFactory, DomainParticipantQos,
    HANDLE_NIL, InstanceStateKind, PublisherQos, SubscriberQos, TopicQos,
};
use zerodds_integration::dds::Reading;
use zerodds_integration::{Peer, event, fields, kernel, lease_domain, role, role_command};

/// Samples per run, over [`INSTANCES`] instances.
const SAMPLES: u32 = 200;
/// Instances the samples are spread over; the last one is disposed.
const INSTANCES: u32 = 4;
/// What one wait of a role may take. Discovery over loopback with a 100 ms
/// SPDP period matches in well under a second.
const STEP: Duration = Duration::from_secs(10);

/// The transports of this file, by the name `transport.user_transport`
/// takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Transport {
    UdpV4,
    TcpV4,
    /// TCPv4 in the dialect `rtps` (`transport.tcp_dialect = rtps`,
    /// `zerodds-tcp-transport-1.0` §2.5).
    TcpV4Rtps,
    /// TCPv4 for discovery and user traffic, with no UDP path between the
    /// participants (`zerodds-tcp-transport-1.0` §4.3); see the module
    /// comment.
    TcpV4Only,
    /// The same over TCPv6: the reader's listener is on `::`, the writer
    /// names it as `tcp://[::1]:port`.
    TcpV6Only,
    Uds,
    /// UDS in the directory the scenario names (`transport.uds.base_dir`),
    /// with the modes of a directory shared by the UIDs of one group:
    /// socket files `0660`, the directory `3770` when the first
    /// participant creates it (`zerodds-uds-transport-1.0` §5.1).
    UdsShared,
    /// UDS in stream mode (`transport.uds.mode = stream`,
    /// `zerodds-uds-transport-1.0` §3.5) in the directory the scenario
    /// names: one connection per peer, each message a length-prefixed frame.
    UdsStream,
    /// The same-host shared-memory channel (`transport.same_host_shm`,
    /// `zerodds-zero-copy-1.0` §6) under the default UDPv4 user transport.
    SameHostShm,
    /// The SHM user transport (`transport.user_transport = SHM`), best
    /// effort on both ends; see the module comment.
    ShmUser,
}

impl Transport {
    fn name(self) -> &'static str {
        match self {
            Self::UdpV4 => "UDPv4",
            Self::TcpV4 => "TCPv4",
            Self::TcpV4Rtps => "TCPv4Rtps",
            Self::TcpV4Only => "TCPv4Only",
            Self::TcpV6Only => "TCPv6Only",
            Self::Uds => "UDS",
            Self::UdsShared => "UDSShared",
            Self::UdsStream => "UDSStream",
            Self::SameHostShm => "SHM",
            Self::ShmUser => "ShmUser",
        }
    }

    fn parse(name: &str) -> Self {
        match name {
            "UDPv4" => Self::UdpV4,
            "TCPv4" => Self::TcpV4,
            "TCPv4Rtps" => Self::TcpV4Rtps,
            "TCPv4Only" => Self::TcpV4Only,
            "TCPv6Only" => Self::TcpV6Only,
            "UDS" => Self::Uds,
            "UDSShared" => Self::UdsShared,
            "UDSStream" => Self::UdsStream,
            "SHM" => Self::SameHostShm,
            "ShmUser" => Self::ShmUser,
            other => panic!("unknown transport {other}"),
        }
    }

    fn config(self) -> RuntimeConfig {
        let mut cfg = RuntimeConfig {
            tick_period: Duration::from_millis(20),
            spdp_period: Duration::from_millis(100),
            ..RuntimeConfig::default()
        };
        // One path for user traffic and no other: the user multicast port
        // and the same-host channel stay off unless they are the subject.
        cfg.user_multicast = false;
        cfg.same_host_shm = false;
        match self {
            Self::UdpV4 => cfg.user_transport = Some(UserTransportKind::UdpV4),
            Self::TcpV4 => cfg.user_transport = Some(UserTransportKind::TcpV4),
            Self::TcpV4Rtps => {
                cfg.user_transport = Some(UserTransportKind::TcpV4);
                cfg.tcp_dialect = TcpDialect::Rtps;
            }
            Self::TcpV4Only | Self::TcpV6Only => {
                let v6 = self == Self::TcpV6Only;
                cfg.user_transport = Some(if v6 {
                    UserTransportKind::TcpV6
                } else {
                    UserTransportKind::TcpV4
                });
                cfg.spdp_multicast_send = false;
                cfg.metatraffic_over_udp = false;
                let port: u16 = env("IT_TCP_PORT").parse().unwrap();
                // The reader listens on the port, the writer names it as
                // its initial peer; each has an SPDP group of its own.
                if role().as_deref() == Some("reader") {
                    cfg.tcp_port = port;
                    cfg.spdp_multicast_group = std::net::Ipv4Addr::new(239, 255, 9, 21);
                } else {
                    cfg.initial_peer_specs = vec![if v6 {
                        format!("tcp://[::1]:{port}")
                    } else {
                        format!("tcp://127.0.0.1:{port}")
                    }];
                    cfg.spdp_multicast_group = std::net::Ipv4Addr::new(239, 255, 9, 22);
                }
            }
            Self::Uds => cfg.user_transport = Some(UserTransportKind::Uds),
            Self::UdsShared => {
                cfg.user_transport = Some(UserTransportKind::Uds);
                cfg.uds = UdsUserConfig {
                    base_dir: std::path::PathBuf::from(env("IT_UDS_DIR")),
                    socket_mode: Some(0o660),
                    base_dir_mode: 0o3770,
                    ..UdsUserConfig::default()
                };
            }
            Self::UdsStream => {
                cfg.user_transport = Some(UserTransportKind::Uds);
                cfg.uds = UdsUserConfig {
                    base_dir: std::path::PathBuf::from(env("IT_UDS_DIR")),
                    mode: UdsMode::Stream,
                    ..UdsUserConfig::default()
                };
            }
            Self::SameHostShm => {
                cfg.user_transport = Some(UserTransportKind::UdpV4);
                cfg.same_host_shm = true;
            }
            Self::ShmUser => cfg.user_transport = Some(UserTransportKind::Shm),
        }
        cfg
    }

    /// RELIABLE, except for the SHM user transport; see the module comment.
    fn reliability(self) -> ReliabilityKind {
        match self {
            Self::ShmUser => ReliabilityKind::BestEffort,
            _ => ReliabilityKind::Reliable,
        }
    }

    /// What the writer role reports when it has handed everything over: the
    /// acknowledgment of every sample, or for best effort the last write.
    fn handed_over(self) -> &'static str {
        match self.reliability() {
            ReliabilityKind::Reliable => "acknowledged",
            ReliabilityKind::BestEffort => "written",
        }
    }
}

fn keep_all_writer(kind: ReliabilityKind) -> DataWriterQos {
    DataWriterQos {
        reliability: ReliabilityQosPolicy {
            kind,
            ..ReliabilityQosPolicy::default()
        },
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        ..DataWriterQos::default()
    }
}

fn keep_all_reader(kind: ReliabilityKind) -> DataReaderQos {
    DataReaderQos {
        reliability: ReliabilityQosPolicy {
            kind,
            ..ReliabilityQosPolicy::default()
        },
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        ..DataReaderQos::default()
    }
}

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set for this role"))
}

/// The writer role: matches, for best effort waits for the scenario's go
/// (the reader has matched it too), writes `SAMPLES` samples round-robin over
/// the instances, disposes the last instance, waits until the reader has
/// acknowledged everything (DDS 1.4 §2.2.2.4.2.23; a best-effort writer has
/// nothing to wait for), and holds its participant until the scenario
/// releases it.
fn writer_role() {
    let transport = Transport::parse(&env("IT_TRANSPORT"));
    let domain: i32 = env("IT_DOMAIN").parse().unwrap();
    let participant = DomainParticipantFactory::instance()
        .create_participant_with_config(domain, DomainParticipantQos::default(), transport.config())
        .expect("writer participant");
    let topic = participant
        .create_topic::<Reading>(&env("IT_TOPIC"), TopicQos::default())
        .expect("topic");
    let writer = participant
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Reading>(&topic, keep_all_writer(transport.reliability()))
        .expect("writer");
    writer
        .wait_for_matched_subscription(1, STEP)
        .expect("writer matches the reader");
    event("matched");
    if transport.reliability() == ReliabilityKind::BestEffort {
        // A best-effort reader drops the samples of a writer it has not
        // matched yet, and nothing sends them again: the writer starts
        // once the scenario has seen the reader match it.
        zerodds_integration::wait_for_line_on_stdin("go");
    }
    for seq in 0..SAMPLES {
        writer
            .write(&Reading {
                id: seq % INSTANCES,
                seq,
            })
            .expect("write");
    }
    writer
        .dispose(
            &Reading {
                id: INSTANCES - 1,
                seq: 0,
            },
            HANDLE_NIL,
        )
        .expect("dispose");
    if transport.reliability() == ReliabilityKind::Reliable {
        writer
            .wait_for_acknowledgments(STEP)
            .expect("the reader acknowledges every sample");
    }
    event(transport.handed_over());
    zerodds_integration::wait_for_release();
}

/// The reader role: takes until it has `SAMPLES` valid samples and the
/// dispose of the last instance, and prints each as an event line.
fn reader_role() {
    let transport = Transport::parse(&env("IT_TRANSPORT"));
    let domain: i32 = env("IT_DOMAIN").parse().unwrap();
    let participant = DomainParticipantFactory::instance()
        .create_participant_with_config(domain, DomainParticipantQos::default(), transport.config())
        .expect("reader participant");
    let topic = participant
        .create_topic::<Reading>(&env("IT_TOPIC"), TopicQos::default())
        .expect("topic");
    let reader = participant
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Reading>(&topic, keep_all_reader(transport.reliability()))
        .expect("reader");
    reader
        .wait_for_matched_publication(1, STEP)
        .expect("reader matches the writer");
    event("matched");
    let deadline = std::time::Instant::now() + STEP;
    let (mut valid, mut disposed) = (0u32, false);
    while valid < SAMPLES || !disposed {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if reader.wait_for_data(left).is_err() {
            event(format!("incomplete valid={valid} disposed={disposed}"));
            break;
        }
        for sample in reader.take_with_info().expect("take") {
            if sample.info.valid_data {
                valid += 1;
                event(format!(
                    "sample id={} seq={}",
                    sample.data.id, sample.data.seq
                ));
            } else if sample.info.instance_state == InstanceStateKind::NotAliveDisposed {
                disposed = true;
                event(format!("disposed id={}", sample.data.id));
            }
        }
    }
    event("done");
    zerodds_integration::wait_for_release();
}

/// The ports DDSI-RTPS 2.5 §9.6.2.2 gives user traffic on `domain`: the
/// multicast port `PB + DG·d + d2` and the unicast ports
/// `PB + DG·d + d3 + PG·p` of the 120 participant ids, with the defaults
/// of §9.6.2.3.
fn user_traffic_ports(domain: i32) -> BTreeSet<u16> {
    const PB: i32 = 7400;
    const DG: i32 = 250;
    const PG: i32 = 2;
    const D2: i32 = 1;
    const D3: i32 = 11;
    let base = PB + DG * domain;
    let mut ports: BTreeSet<u16> = (0..120)
        .map(|p| u16::try_from(base + D3 + PG * p).unwrap())
        .collect();
    ports.insert(u16::try_from(base + D2).unwrap());
    ports
}

fn run(test: &str, transport: Transport) {
    let lease = lease_domain();
    let domain = lease.id();
    let topic = format!("IntegrationReadings{}", transport.name());
    // A port for the reader's TCP listener (`TCPv4Only`): one the system
    // just gave out and that is free again.
    let tcp_port = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .and_then(|listener| listener.local_addr())
        .expect("a free TCP port")
        .port();
    // The directory of `UDSShared`, which the first participant creates
    // with `transport.uds.base_dir_mode`.
    let uds_parent =
        std::env::temp_dir().join(format!("zerodds-it-uds-{}-{domain}", std::process::id()));
    let _ = std::fs::remove_dir_all(&uds_parent);
    std::fs::create_dir_all(&uds_parent).expect("the parent of the UDS directory");
    let uds_dir = uds_parent.join("volume");
    let start = |name: &str| {
        let mut command = role_command(test, name);
        command
            .env("IT_TRANSPORT", transport.name())
            .env("IT_DOMAIN", domain.to_string())
            .env("IT_TOPIC", &topic)
            .env("IT_TCP_PORT", tcp_port.to_string())
            .env("IT_UDS_DIR", &uds_dir);
        Peer::spawn(name, command)
    };
    let mut reader = start("reader");
    let mut writer = start("writer");
    writer.wait_for_event("matched", STEP);
    reader.wait_for_event("matched", STEP);
    if transport.reliability() == ReliabilityKind::BestEffort {
        writer.send_line("go");
    }
    writer.wait_for_event(transport.handed_over(), STEP);
    reader.wait_for_event("done", STEP);

    // The kernel's view, while both participants still hold their sockets.
    let (w, r) = (writer.pid(), reader.pid());
    let user_ports = user_traffic_ports(domain);
    let udp_user = |pid: u32| -> Vec<u16> {
        kernel::udp_sockets(pid)
            .into_iter()
            .map(|s| s.local_port)
            .filter(|p| user_ports.contains(p))
            .collect()
    };
    let shared_shm: BTreeSet<String> = kernel::shm_mappings(w)
        .intersection(&kernel::shm_mappings(r))
        .cloned()
        .collect();
    match transport {
        Transport::UdpV4 => {
            assert!(
                !udp_user(r).is_empty(),
                "UDPv4: the reader holds no UDP socket on a user-traffic port of domain {domain} \
                 (DDSI-RTPS 2.5 Table 9.16); its UDP ports: {:?}",
                kernel::udp_sockets(r)
            );
            assert!(
                shared_shm.is_empty(),
                "UDPv4 with the same-host channel off: both map {shared_shm:?}"
            );
        }
        Transport::TcpV4 | Transport::TcpV4Rtps | Transport::TcpV4Only | Transport::TcpV6Only => {
            let connections = kernel::tcp_connections_between(w, r);
            assert!(
                !connections.is_empty(),
                "TCPv4: no ESTABLISHED TCP connection between writer {w} and reader {r}; \
                 writer TCP {:?}, reader TCP {:?}",
                kernel::tcp_sockets(w),
                kernel::tcp_sockets(r)
            );
        }
        Transport::Uds => {
            for (role, pid) in [("writer", w), ("reader", r)] {
                let paths = kernel::unix_socket_paths(pid);
                assert!(
                    paths
                        .iter()
                        .any(|p| p.starts_with(zerodds_transport_uds::DEFAULT_BASE_DIR)),
                    "UDS: the {role} holds no Unix socket under {}: {paths:?}",
                    zerodds_transport_uds::DEFAULT_BASE_DIR
                );
            }
        }
        Transport::UdsShared => {
            use std::os::unix::fs::PermissionsExt;
            let mode = |path: &std::path::Path| {
                std::fs::metadata(path)
                    .expect("the path exists")
                    .permissions()
                    .mode()
                    & 0o7777
            };
            assert_eq!(
                mode(&uds_dir),
                0o3770,
                "UDSShared: the first participant creates the directory with \
                 transport.uds.base_dir_mode"
            );
            for (role, pid) in [("writer", w), ("reader", r)] {
                let paths: Vec<String> = kernel::unix_socket_paths(pid)
                    .into_iter()
                    .filter(|p| std::path::Path::new(p).parent() == Some(uds_dir.as_path()))
                    .collect();
                assert_eq!(
                    paths.len(),
                    1,
                    "UDSShared: the {role} holds no Unix socket in {uds_dir:?}: {:?}",
                    kernel::unix_socket_paths(pid)
                );
                assert_eq!(
                    mode(std::path::Path::new(&paths[0])),
                    0o660,
                    "UDSShared: the {role}'s socket file carries transport.uds.socket_mode"
                );
            }
        }
        Transport::UdsStream => {
            // Each participant listens at its socket path in the directory
            // and holds a connection the other opened to it: the writer's
            // samples and heartbeats go to the reader, the reader's
            // acknowledgments to the writer.
            for (role, pid) in [("writer", w), ("reader", r)] {
                let streams: Vec<kernel::UnixStreamSocket> = kernel::unix_stream_sockets(pid)
                    .into_iter()
                    .filter(|s| std::path::Path::new(&s.path).parent() == Some(uds_dir.as_path()))
                    .collect();
                assert!(
                    streams.iter().any(|s| s.listening),
                    "UDSStream: the {role} holds no listening stream socket in {uds_dir:?}: \
                     {streams:?}"
                );
                assert!(
                    streams.iter().any(|s| s.connected && !s.listening),
                    "UDSStream: the {role} holds no accepted connection in {uds_dir:?}: \
                     {streams:?}"
                );
                assert!(
                    !kernel::unix_socket_paths(pid)
                        .iter()
                        .any(|p| p.starts_with(zerodds_transport_uds::DEFAULT_BASE_DIR)),
                    "UDSStream: the {role} holds a socket in the default directory"
                );
            }
        }
        Transport::SameHostShm | Transport::ShmUser => {
            assert!(
                !shared_shm.is_empty(),
                "{}: no file under /dev/shm is mapped by both writer {w} and reader {r}; \
                 writer {:?}, reader {:?}",
                transport.name(),
                kernel::shm_mappings(w),
                kernel::shm_mappings(r)
            );
        }
    }
    if matches!(transport, Transport::TcpV4Only | Transport::TcpV6Only) {
        let listening: Vec<u16> = kernel::tcp_sockets(r)
            .into_iter()
            .map(|s| s.local_port)
            .collect();
        assert!(
            listening.contains(&tcp_port),
            "{}: the reader holds no TCP socket on the port {tcp_port} of \
             transport.tcp_port; its TCP ports: {listening:?}",
            transport.name()
        );
    }
    if matches!(
        transport,
        Transport::TcpV4
            | Transport::TcpV4Rtps
            | Transport::TcpV4Only
            | Transport::TcpV6Only
            | Transport::Uds
            | Transport::UdsShared
            | Transport::UdsStream
            | Transport::ShmUser
    ) {
        for (role, pid) in [("writer", w), ("reader", r)] {
            let found = udp_user(pid);
            assert!(
                found.is_empty(),
                "{}: the {role} holds UDP sockets on the user-traffic ports {found:?} of \
                 domain {domain}, so samples could have gone over UDP",
                transport.name()
            );
        }
    }

    let lines: Vec<String> = reader.lines().iter().map(|l| l.text.clone()).collect();
    assert!(writer.release(STEP).success(), "writer role failed");
    assert!(reader.release(STEP).success(), "reader role failed");
    check_delivery(transport, &lines);
    let _ = std::fs::remove_dir_all(&uds_parent);
    drop(lease);
}

/// The delivery half of the oracle; see the module comment.
fn check_delivery(transport: Transport, lines: &[String]) {
    let name = transport.name();
    let mut per_instance: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    let mut disposed = Vec::new();
    let mut incomplete = None;
    for line in lines {
        if let Some(rest) = line.strip_prefix("EVENT sample ") {
            let f = fields(rest);
            per_instance
                .entry(f["id"].parse().unwrap())
                .or_default()
                .push(f["seq"].parse().unwrap());
        } else if let Some(rest) = line.strip_prefix("EVENT disposed ") {
            disposed.push(fields(rest)["id"].parse::<u32>().unwrap());
        } else if line.starts_with("EVENT incomplete") {
            incomplete = Some(line.clone());
        }
    }
    let mut all: Vec<u32> = per_instance.values().flatten().copied().collect();
    all.sort_unstable();
    let expected: Vec<u32> = (0..SAMPLES).collect();
    if let Some(line) = incomplete {
        let missing: Vec<u32> = expected
            .iter()
            .copied()
            .filter(|s| all.binary_search(s).is_err())
            .collect();
        panic!("{name}: the reader timed out: {line}; missing sequence numbers {missing:?}");
    }
    let promise = match transport.reliability() {
        ReliabilityKind::Reliable => {
            "RELIABLE + KEEP_ALL delivers every sample once \
                                      (DDS 1.4 §2.2.3.14, §2.2.3.18)"
        }
        ReliabilityKind::BestEffort => {
            "a ring that does not fill loses nothing \
                                        (ZeroDDS SHM Transport 1.0 §4.1, §4.5)"
        }
    };
    assert_eq!(all, expected, "{name}: {promise}");
    for (id, seqs) in &per_instance {
        assert!(
            seqs.iter().all(|s| s % INSTANCES == *id),
            "{name}: instance {id} received samples of another instance: {seqs:?}"
        );
        assert!(
            seqs.windows(2).all(|w| w[0] < w[1]),
            "{name}: instance {id} out of order (DDSI-RTPS 2.5 §8.4.2.2, §8.4.12.2): {seqs:?}"
        );
    }
    assert_eq!(
        disposed,
        vec![INSTANCES - 1],
        "{name}: exactly the disposed instance reads NOT_ALIVE_DISPOSED (DDS 1.4 §2.2.2.5.1.3)"
    );
}

fn dispatch(test: &str, transport: Transport) {
    match role().as_deref() {
        Some("writer") => writer_role(),
        Some("reader") => reader_role(),
        Some(other) => panic!("unknown role {other}"),
        None => run(test, transport),
    }
}

#[test]
fn dcps_over_udpv4() {
    dispatch("dcps_over_udpv4", Transport::UdpV4);
}

#[test]
fn dcps_over_tcpv4() {
    dispatch("dcps_over_tcpv4", Transport::TcpV4);
}

#[test]
fn dcps_over_tcpv4_rtps_dialect() {
    dispatch("dcps_over_tcpv4_rtps_dialect", Transport::TcpV4Rtps);
}

#[test]
fn dcps_over_tcpv4_without_udp_discovery() {
    dispatch(
        "dcps_over_tcpv4_without_udp_discovery",
        Transport::TcpV4Only,
    );
}

#[test]
fn dcps_over_tcpv6_without_udp_discovery() {
    dispatch(
        "dcps_over_tcpv6_without_udp_discovery",
        Transport::TcpV6Only,
    );
}

#[test]
fn dcps_over_uds() {
    dispatch("dcps_over_uds", Transport::Uds);
}

/// `zerodds-uds-transport-1.0` §5.1: the UDS user transport in a
/// directory the configuration names, with the modes of a directory shared
/// by the UIDs of one group.
#[test]
fn dcps_over_uds_in_a_configured_base_dir() {
    dispatch(
        "dcps_over_uds_in_a_configured_base_dir",
        Transport::UdsShared,
    );
}

/// `zerodds-uds-transport-1.0` §3.5: the UDS user transport in stream mode
/// (`transport.uds.mode = stream`, the mode Windows has, §6.2) carries the
/// samples, the heartbeats and the acknowledgments between writer and
/// reader processes over one connection each way.
#[test]
fn dcps_over_uds_in_stream_mode() {
    dispatch("dcps_over_uds_in_stream_mode", Transport::UdsStream);
}

/// `zerodds-uds-transport-1.0` §5.1: a UDS user transport whose bind fails
/// stops the participant's start with `TransportError`, and the bind's own
/// error reaches the observability sink as `transport.uds.bind_failed`
/// with the directory and the mode `datagram`. The bind is refused by the transport's check of
/// §3.4: a `max_datagram` of 1 GiB is above what any kernel grants as a
/// socket buffer (Linux caps it at `net.core.wmem_max`).
#[test]
fn dcps_over_uds_a_refused_bind_is_reported_with_its_cause() {
    use zerodds_foundation::observability::{Level, VecSink};
    let lease = lease_domain();
    let dir = std::env::temp_dir().join(format!(
        "zerodds-it-uds-refused-{}-{}",
        std::process::id(),
        lease.id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let sink = std::sync::Arc::new(VecSink::new());
    let mut cfg = Transport::Uds.config();
    cfg.uds = UdsUserConfig {
        base_dir: dir.clone(),
        max_datagram: 1 << 30,
        mode: UdsMode::Datagram,
        ..UdsUserConfig::default()
    };
    cfg.observability = sink.clone();
    let started = DomainParticipantFactory::instance().create_participant_with_config(
        lease.id(),
        DomainParticipantQos::default(),
        cfg,
    );
    let error = match started {
        Ok(_) => panic!("a participant started with a UDS max_datagram of 1 GiB"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        DdsError::TransportError {
            label: "user unicast bind (UDS)"
        }
    );
    let reported: Vec<_> = sink
        .snapshot()
        .into_iter()
        .filter(|e| e.name == "transport.uds.bind_failed")
        .collect();
    assert_eq!(reported.len(), 1, "{reported:?}");
    assert_eq!(reported[0].level, Level::Error);
    let attr = |key: &str| {
        reported[0]
            .attrs
            .iter()
            .find(|a| a.key == key)
            .map(|a| a.value.clone())
            .unwrap_or_else(|| panic!("no attribute {key}: {reported:?}"))
    };
    assert_eq!(attr("base_dir"), dir.display().to_string());
    assert_eq!(attr("mode"), "datagram");
    let cause = attr("error");
    assert!(
        cause.starts_with("uds max_datagram 1073741824 exceeds what the kernel grants: SO_SNDBUF "),
        "{cause}"
    );
    assert!(cause.contains("(capped by net.core.wmem_max)"), "{cause}");
    let _ = std::fs::remove_dir_all(&dir);
    drop(lease);
}

#[test]
fn dcps_over_same_host_shm() {
    dispatch("dcps_over_same_host_shm", Transport::SameHostShm);
}

#[test]
fn dcps_over_shm_user_transport_best_effort() {
    dispatch(
        "dcps_over_shm_user_transport_best_effort",
        Transport::ShmUser,
    );
}
