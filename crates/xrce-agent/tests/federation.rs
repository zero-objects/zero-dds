// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-XRCE 1.0 §10.4 with daemons over UDP loopback, as Figure 26: outer
//! agents that are clients of an inner agent ("XRCE Agents appear as
//! Clients to other XRCE Agents"), and the store-and-forward dataflow of the
//! figure's text. Every agent is `daemon::server::start`, the entry point of
//! the binary; the clients are `zerodds-xrce-client` over UDP. One upstream
//! link runs over TCP through a proxy that stops reading.

#![cfg(feature = "daemon")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod udp_client;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_xrce::object_id::ObjectId;
use zerodds_xrce::object_info::ResultStatusCode;
use zerodds_xrce::object_kind::ObjectKind;
use zerodds_xrce::object_repr::{ObjectVariant, ObjectVariantExtra};
use zerodds_xrce::submessages::read_data::DataDeliveryControl;
use zerodds_xrce::submessages::write_data::DataFormat;
use zerodds_xrce_agent::daemon::config::{DaemonConfig, ListenerConfig};
use zerodds_xrce_agent::daemon::federation::{UpstreamConfig, UpstreamLocator};
use zerodds_xrce_agent::daemon::server;
use zerodds_xrce_client::{ClientEvent, XrceClient};

use udp_client::{UdpTransport, connect_udp, create_endpoints, status_of, wait_until};

fn agent(
    bind: &str,
    upstream: Option<std::net::SocketAddr>,
    runtime: RuntimeConfig,
) -> server::DaemonHandle {
    let cfg = DaemonConfig {
        discovery_enabled: false,
        listeners: vec![ListenerConfig::Udp {
            bind: bind.to_string(),
        }],
        forward_unknown_topic_types: true,
        upstream: upstream.map(|address| UpstreamConfig {
            keepalive: Duration::from_millis(200),
            link_timeout: Duration::from_millis(1000),
            reconnect: Duration::from_millis(200),
            ..UpstreamConfig::new(UpstreamLocator::Udp(address))
        }),
        ..DaemonConfig::default()
    };
    server::start(cfg, runtime, None).expect("daemon start")
}

/// The DDS runtime of the outer agents: it cannot start a `DcpsRuntime`,
/// because its multicast allowlist does not hold the SPDP group
/// (`DcpsRuntime::start` refuses). An outer agent that made a client's
/// objects itself, instead of relaying them, answers its CREATEs with an
/// error, and no sample can travel by DDS between the agents.
fn no_dds() -> RuntimeConfig {
    RuntimeConfig {
        multicast_allowlist: vec![std::net::Ipv4Addr::new(239, 255, 0, 99)],
        ..RuntimeConfig::default()
    }
}

/// CREATEs `oid` by XML and returns the request id, without waiting.
fn create(
    client: &mut XrceClient<UdpTransport>,
    oid: ObjectId,
    xml: String,
    domain_id: Option<i16>,
    parent: Option<ObjectId>,
) -> u16 {
    client
        .create_object(
            oid,
            ObjectVariant::ByXmlString(xml),
            ObjectVariantExtra { domain_id, parent },
            false,
            false,
            true,
        )
        .unwrap()
}

fn continuous() -> DataDeliveryControl {
    DataDeliveryControl {
        max_samples: zerodds_xrce::MAX_SAMPLES_UNLIMITED,
        max_elapsed_time: 0,
        max_bytes_per_second: 0,
        min_pace_period: 0,
    }
}

/// The payloads of the DATA `client` receives within `budget`.
fn data_of(
    client: &mut XrceClient<UdpTransport>,
    start: Instant,
    budget: Duration,
    what: &str,
) -> Vec<Vec<u8>> {
    wait_until(
        client,
        start,
        budget,
        |c| {
            c.poll(start.elapsed())
                .unwrap()
                .into_iter()
                .filter_map(|e| match e {
                    ClientEvent::Data { payload, .. } => Some(payload),
                    _ => None,
                })
                .collect()
        },
        what,
    )
}

/// Figure 26: clients of the outer agents R1 and R3 exchange a sample
/// through the inner agent U, which alone is a DDS participant; R3 is a
/// client of R1, so the sample of its client crosses two federated agents.
/// The writer's agent answers its CREATEs with the STATUS of U. The outer
/// agents cannot start a DDS runtime ([`no_dds`]): an outer agent with the
/// same runtime and no upstream refuses the client's participant, so the
/// sample can only have travelled through U.
#[test]
fn xrce_10_4_clients_of_federated_agents_exchange_a_sample_through_the_inner_agent() {
    let start = Instant::now();
    let lease = zerodds_test_domains::lease_domain();
    // The control: without an upstream, such an agent makes no participant.
    let alone = agent("127.0.0.1:0", None, no_dds());
    let mut on_alone = connect_udp(alone.udp_local_addr().unwrap(), [0x10, 0x4F, 0, 4], start);
    let participant = ObjectId::new(0x001, ObjectKind::Participant).unwrap();
    let rid = create(
        &mut on_alone,
        participant,
        format!(
            "<dds><participant><domain_id>{}</domain_id></participant></dds>",
            lease.id()
        ),
        Some(i16::try_from(lease.id()).unwrap()),
        None,
    );
    let code = status_of(&mut on_alone, start, rid, "participant at the agent alone");
    assert!(
        !code.is_success(),
        "an outer agent without upstream must not reach DDS: {code:?}"
    );
    drop((on_alone, alone));

    let inner = agent("127.0.0.1:0", None, RuntimeConfig::default());
    let u = inner.udp_local_addr().unwrap();
    let r1 = agent("127.0.0.1:0", Some(u), no_dds());
    let r1_addr = r1.udp_local_addr().unwrap();
    let r2 = agent("127.0.0.1:0", Some(u), no_dds());
    let r3 = agent("127.0.0.1:0", Some(r1_addr), no_dds());

    let topic = "FederatedAgents";
    let mut reader_on_r2 = connect_udp(r2.udp_local_addr().unwrap(), [0x10, 0x4F, 0, 2], start);
    let r = create_endpoints(&mut reader_on_r2, start, lease.id(), topic, "Opaque");
    reader_on_r2
        .request_read(
            r.reader,
            0x80,
            DataFormat::Data,
            None,
            Some(continuous()),
            true,
        )
        .unwrap();
    let mut reader_on_r3 = connect_udp(r3.udp_local_addr().unwrap(), [0x10, 0x4F, 0, 3], start);
    let r3_endpoints = create_endpoints(&mut reader_on_r3, start, lease.id(), topic, "Opaque");
    reader_on_r3
        .request_read(
            r3_endpoints.reader,
            0x80,
            DataFormat::Data,
            None,
            Some(continuous()),
            true,
        )
        .unwrap();
    let mut writer_on_r1 = connect_udp(r1_addr, [0x10, 0x4F, 0, 1], start);
    let w = create_endpoints(&mut writer_on_r1, start, lease.id(), topic, "Opaque");
    // Give the READ_DATAs time to reach U.
    for _ in 0..25 {
        for c in [&mut reader_on_r2, &mut reader_on_r3] {
            c.tick(start.elapsed()).unwrap();
            let _ = c.poll(start.elapsed()).unwrap();
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let sample = [0x5A, 0x10, 0x04, 0x00];
    let rid = writer_on_r1.request_write(w.writer, &sample, true).unwrap();
    // A successful WRITE_DATA has no STATUS (§8.3.5.8); keep the writer
    // ticking so its reliable stream is acknowledged.
    let _ = rid;
    for _ in 0..10 {
        writer_on_r1.tick(start.elapsed()).unwrap();
        let _ = writer_on_r1.poll(start.elapsed()).unwrap();
        std::thread::sleep(Duration::from_millis(20));
    }
    let on_r2 = data_of(
        &mut reader_on_r2,
        start,
        Duration::from_secs(10),
        "DATA at R2",
    );
    assert_eq!(on_r2, vec![sample.to_vec()]);
    let on_r3 = data_of(
        &mut reader_on_r3,
        start,
        Duration::from_secs(10),
        "DATA at R3",
    );
    assert_eq!(on_r3, vec![sample.to_vec()]);
    drop((r3, r2, r1, inner));
}

/// Figure 26 "store-and-forward": the inner agent starts after the client of
/// the outer agent has created its participant, topic, subscriber, reader,
/// publisher and writer, started a READ_DATA and written a sample. All of it
/// waits at the outer agent and reaches the inner agent when it is there:
/// the STATUS of each CREATE and of the WRITE_DATA reaches the client once,
/// in order, and then the sample it wrote before (writer and reader
/// TRANSIENT_LOCAL, so the sample does not depend on when U matched them).
#[test]
fn xrce_10_4_requests_sent_before_the_inner_agent_starts_are_carried_out_after_it_starts() {
    let start = Instant::now();
    let lease = zerodds_test_domains::lease_domain();
    // A port for U that nothing listens on yet.
    let u_addr = {
        let probe = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        probe.local_addr().unwrap()
    };
    let r1 = agent("127.0.0.1:0", Some(u_addr), no_dds());
    let mut client = connect_udp(r1.udp_local_addr().unwrap(), [0x10, 0x4F, 0, 9], start);
    let participant = ObjectId::new(0x001, ObjectKind::Participant).unwrap();
    let topic = ObjectId::new(0x002, ObjectKind::Topic).unwrap();
    let subscriber = ObjectId::new(0x001, ObjectKind::Subscriber).unwrap();
    let reader = ObjectId::new(0x003, ObjectKind::DataReader).unwrap();
    let publisher = ObjectId::new(0x001, ObjectKind::Publisher).unwrap();
    let writer = ObjectId::new(0x003, ObjectKind::DataWriter).unwrap();
    let endpoint = |kind: &str| {
        format!(
            "<dds><{kind} topic_ref=\"0x{:04x}\" reliability=\"RELIABLE\"><qos><durability>\
             <kind>TRANSIENT_LOCAL</kind></durability></qos></{kind}></dds>",
            topic.raw()
        )
    };
    let mut rids = Vec::new();
    rids.push(create(
        &mut client,
        participant,
        format!(
            "<dds><participant><domain_id>{}</domain_id></participant></dds>",
            lease.id()
        ),
        Some(i16::try_from(lease.id()).unwrap()),
        None,
    ));
    rids.push(create(
        &mut client,
        topic,
        "<dds><topic name=\"StoreAndForward\" type_name=\"Opaque\"/></dds>".to_string(),
        None,
        Some(participant),
    ));
    rids.push(create(
        &mut client,
        subscriber,
        String::new(),
        None,
        Some(participant),
    ));
    rids.push(create(
        &mut client,
        reader,
        endpoint("data_reader"),
        None,
        Some(subscriber),
    ));
    client
        .request_read(
            reader,
            0x80,
            DataFormat::Data,
            None,
            Some(continuous()),
            true,
        )
        .unwrap();
    rids.push(create(
        &mut client,
        publisher,
        String::new(),
        None,
        Some(participant),
    ));
    rids.push(create(
        &mut client,
        writer,
        endpoint("data_writer"),
        None,
        Some(publisher),
    ));
    let sample = [0x5A, 0x10, 0x04, 0x01];
    rids.push(client.request_write(writer, &sample, true).unwrap());
    // U is away for a second: no answer and no sample arrives.
    let quiet = Instant::now();
    while quiet.elapsed() < Duration::from_secs(1) {
        client.tick(start.elapsed()).unwrap();
        let events = client.poll(start.elapsed()).unwrap();
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, ClientEvent::Status { .. } | ClientEvent::Data { .. })),
            "nothing before U is there: {events:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let inner = agent(&u_addr.to_string(), None, RuntimeConfig::default());
    let mut answers = Vec::new();
    let mut samples = Vec::new();
    let wait = Instant::now();
    while (answers.len() < rids.len() || samples.is_empty())
        && wait.elapsed() < Duration::from_secs(10)
    {
        client.tick(start.elapsed()).unwrap();
        for e in client.poll(start.elapsed()).unwrap() {
            match e {
                ClientEvent::Status {
                    request_id, code, ..
                } => answers.push((request_id, code)),
                ClientEvent::Data { payload, .. } => samples.push(payload),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        answers,
        rids.iter()
            .map(|rid| (*rid, ResultStatusCode::Ok))
            .collect::<Vec<_>>(),
        "each CREATE and the WRITE_DATA answered once, in order, by U"
    );
    assert_eq!(
        samples,
        vec![sample.to_vec()],
        "the sample written before U started"
    );
    // Nothing more arrives: the answers and the sample are not repeated.
    let more = Instant::now();
    while more.elapsed() < Duration::from_millis(600) {
        client.tick(start.elapsed()).unwrap();
        let events = client.poll(start.elapsed()).unwrap();
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, ClientEvent::Status { .. } | ClientEvent::Data { .. })),
            "{events:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    drop((r1, inner));
}

/// §10.4 with §7.8.2.1: the outer agent R1 stops while the inner agent U
/// still holds the session of its client (no DELETE reached U), with its
/// objects and its streams past sequence number 0. A new outer agent R2
/// takes the client, which opens the same session (ClientKey and
/// session_id) again: its CREATEs are answered, and its sample reaches a
/// reader at U. R2's first session at U replaced the one U held.
#[test]
fn xrce_10_4_a_restarted_relay_replaces_the_session_the_inner_agent_still_holds() {
    let start = Instant::now();
    let lease = zerodds_test_domains::lease_domain();
    let inner = agent("127.0.0.1:0", None, RuntimeConfig::default());
    let u = inner.udp_local_addr().unwrap();
    let topic = "RestartedRelay";
    let mut reader_on_u = connect_udp(u, [0x10, 0x4F, 0, 0x21], start);
    let r = create_endpoints(&mut reader_on_u, start, lease.id(), topic, "Opaque");
    reader_on_u
        .request_read(
            r.reader,
            0x80,
            DataFormat::Data,
            None,
            Some(continuous()),
            true,
        )
        .unwrap();

    let key = [0x10, 0x4F, 0, 0x20];
    let r1 = agent("127.0.0.1:0", Some(u), no_dds());
    let mut writer = connect_udp(r1.udp_local_addr().unwrap(), key, start);
    create_endpoints(&mut writer, start, lease.id(), topic, "Opaque");
    // R1 stops; a stopping agent sends no DELETE upstream.
    drop((writer, r1));

    let r2 = agent("127.0.0.1:0", Some(u), no_dds());
    let mut writer = connect_udp(r2.udp_local_addr().unwrap(), key, start);
    let w = create_endpoints(&mut writer, start, lease.id(), topic, "Opaque");
    let sample = [0x5A, 0x10, 0x04, 0x02];
    let mut got: Vec<Vec<u8>> = Vec::new();
    let wait = Instant::now();
    // The writer and the reader at U match after a moment: the sample goes
    // again until the reader has it.
    while got.is_empty() && wait.elapsed() < Duration::from_secs(10) {
        writer.request_write(w.writer, &sample, true).unwrap();
        for _ in 0..10 {
            writer.tick(start.elapsed()).unwrap();
            let _ = writer.poll(start.elapsed()).unwrap();
            reader_on_u.tick(start.elapsed()).unwrap();
            got.extend(
                reader_on_u
                    .poll(start.elapsed())
                    .unwrap()
                    .into_iter()
                    .filter_map(|e| match e {
                        ClientEvent::Data { payload, .. } => Some(payload),
                        _ => None,
                    }),
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    assert!(!got.is_empty(), "no sample reached the reader at U");
    assert!(got.iter().all(|p| *p == sample), "{got:?}");
    drop((r2, inner));
}

/// A TCP proxy in front of an agent: it copies both ways until `stall` is
/// set; from then on it reads nothing more from the agent below it, as an
/// upstream agent that stops reading, and holds the connections. `accepted`
/// counts the connections the agent below opened.
struct StallingProxy {
    address: SocketAddr,
    accepted: Arc<AtomicUsize>,
    stall: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
}

impl StallingProxy {
    fn start(upstream: SocketAddr) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let accepted = Arc::new(AtomicUsize::new(0));
        let stall = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let (count, stalled, stopped) =
            (Arc::clone(&accepted), Arc::clone(&stall), Arc::clone(&stop));
        std::thread::spawn(move || {
            while !stopped.load(Ordering::SeqCst) {
                let Ok((below, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                };
                count.fetch_add(1, Ordering::SeqCst);
                below.set_nonblocking(false).unwrap();
                let above = TcpStream::connect(upstream).unwrap();
                let (b, a) = (below.try_clone().unwrap(), above.try_clone().unwrap());
                let (st, sp) = (Arc::clone(&stalled), Arc::clone(&stopped));
                std::thread::spawn(move || copy(b, a, Some(st), &sp));
                let sp = Arc::clone(&stopped);
                std::thread::spawn(move || copy(above, below, None, &sp));
            }
        });
        Self {
            address,
            accepted,
            stall,
            stop,
        }
    }
}

impl Drop for StallingProxy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Copies `from` to `to` until `stop`; while `stall` is set it does not read.
fn copy(mut from: TcpStream, mut to: TcpStream, stall: Option<Arc<AtomicBool>>, stop: &AtomicBool) {
    from.set_read_timeout(Some(Duration::from_millis(20)))
        .unwrap();
    let mut buf = vec![0u8; 65_536];
    while !stop.load(Ordering::SeqCst) {
        if stall.as_ref().is_some_and(|s| s.load(Ordering::SeqCst)) {
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }
        match from.read(&mut buf) {
            Ok(0) => return,
            Ok(n) => {
                if to.write_all(&buf[..n]).is_err() {
                    return;
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return,
        }
    }
}

/// §10.4 over a TCP link (§11.3): the upstream agent stops reading while a
/// routed client writes best-effort samples of 60 000 octets, far more than
/// the socket buffers hold. A client of the same agent that is not routed
/// gets every answer within 3 s during and after that, because no call of
/// the link waits for the socket; and the link, silent, is opened again on
/// a new connection.
#[test]
fn xrce_10_4_an_upstream_agent_that_stops_reading_over_tcp_holds_up_no_other_client() {
    let start = Instant::now();
    let lease = zerodds_test_domains::lease_domain();
    let u_addr = {
        let probe = TcpListener::bind("127.0.0.1:0").unwrap();
        probe.local_addr().unwrap()
    };
    let inner = server::start(
        DaemonConfig {
            discovery_enabled: false,
            listeners: vec![ListenerConfig::Tcp {
                bind: u_addr.to_string(),
            }],
            forward_unknown_topic_types: true,
            ..DaemonConfig::default()
        },
        no_dds(),
        None,
    )
    .expect("inner agent");
    let proxy = StallingProxy::start(u_addr);
    let routed_key = [0x10, 0x4F, 0, 0x0A];
    let relay = server::start(
        DaemonConfig {
            discovery_enabled: false,
            listeners: vec![ListenerConfig::Udp {
                bind: "127.0.0.1:0".to_string(),
            }],
            upstream: Some(UpstreamConfig {
                keepalive: Duration::from_millis(200),
                link_timeout: Duration::from_millis(1000),
                reconnect: Duration::from_millis(200),
                clients: Some(vec![routed_key]),
                ..UpstreamConfig::new(UpstreamLocator::Tcp(proxy.address))
            }),
            ..DaemonConfig::default()
        },
        no_dds(),
        None,
    )
    .expect("relay");
    let r_addr = relay.udp_local_addr().unwrap();
    let mut routed = connect_udp(r_addr, routed_key, start);
    let mut local = connect_udp(r_addr, [0x10, 0x4F, 0, 0x0B], start);
    let writer = ObjectId::new(0x0F0, ObjectKind::DataWriter).unwrap();
    // The link is up: the inner agent answers (it has no such writer).
    let rid = routed.request_write(writer, &[1, 2, 3, 4], true).unwrap();
    assert_eq!(
        status_of(&mut routed, start, rid, "WRITE_DATA through the link"),
        ResultStatusCode::UnknownReference
    );
    assert_eq!(proxy.accepted.load(Ordering::SeqCst), 1);

    let mut next_participant = 0x100u16;
    let mut local_answers_within = |local: &mut XrceClient<UdpTransport>, what: &str| {
        let participant = ObjectId::new(next_participant, ObjectKind::Participant).unwrap();
        next_participant += 1;
        let asked = Instant::now();
        let rid = create(
            local,
            participant,
            format!(
                "<dds><participant><domain_id>{}</domain_id></participant></dds>",
                lease.id()
            ),
            Some(i16::try_from(lease.id()).unwrap()),
            None,
        );
        let _ = status_of(local, start, rid, what);
        let took = asked.elapsed();
        assert!(took < Duration::from_secs(3), "{what}: {took:?}");
    };
    proxy.stall.store(true, Ordering::SeqCst);
    let sample = vec![0xA5; 60_000];
    for i in 0..1000 {
        routed.request_write(writer, &sample, false).unwrap();
        if i % 100 == 99 {
            local_answers_within(&mut local, "an answer during the flood");
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    local_answers_within(&mut local, "an answer after the flood");
    // The inner agent hears nothing more and falls silent; after
    // link_timeout the relay connects again.
    let wait = Instant::now();
    while proxy.accepted.load(Ordering::SeqCst) < 2 && wait.elapsed() < Duration::from_secs(10) {
        local_answers_within(&mut local, "an answer while the link is opened again");
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        proxy.accepted.load(Ordering::SeqCst) >= 2,
        "the silent link is opened again on a new connection"
    );
    drop((routed, local, relay, proxy, inner));
}
