// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DCPS over UDPv4, TCPv4, Unix domain sockets and shared memory: a writer
//! process and a reader process, each a ZeroDDS participant made through
//! the public API (`DomainParticipantFactory`), exchange a keyed topic over
//! the configured user transport. Shared memory comes twice: the same-host
//! channel under the UDPv4 user transport (`transport.same_host_shm`), and
//! the SHM user transport (`transport.user_transport = SHM`).
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
//!    each under the transport's directory; for the same-host channel a
//!    file under `/dev/shm` both have mapped. For every transport but UDPv4,
//!    no UDP socket on the user-traffic ports of the domain (DDSI-RTPS 2.5
//!    §9.6.2.2 Table 9.16, defaults §9.6.2.3), so the samples cannot have
//!    gone over UDP; for UDPv4, the reader holds its user unicast port.
//!
//! Discovery stays SPDP/SEDP over UDPv4 for every transport (DDSI-RTPS 2.5
//! §8.5); only the user traffic moves.

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
use zerodds_dcps::runtime::{RuntimeConfig, UserTransportKind};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, HANDLE_NIL,
    InstanceStateKind, PublisherQos, SubscriberQos, TopicQos,
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
    Uds,
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
            Self::Uds => "UDS",
            Self::SameHostShm => "SHM",
            Self::ShmUser => "ShmUser",
        }
    }

    fn parse(name: &str) -> Self {
        match name {
            "UDPv4" => Self::UdpV4,
            "TCPv4" => Self::TcpV4,
            "UDS" => Self::Uds,
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
            Self::Uds => cfg.user_transport = Some(UserTransportKind::Uds),
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
    let start = |name: &str| {
        let mut command = role_command(test, name);
        command
            .env("IT_TRANSPORT", transport.name())
            .env("IT_DOMAIN", domain.to_string())
            .env("IT_TOPIC", &topic);
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
        Transport::TcpV4 => {
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
    if matches!(
        transport,
        Transport::TcpV4 | Transport::Uds | Transport::ShmUser
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
fn dcps_over_uds() {
    dispatch("dcps_over_uds", Transport::Uds);
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
