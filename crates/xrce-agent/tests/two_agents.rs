// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-XRCE 1.0 §10.2: two XRCE clients connected to two different agents
//! exchange data through DDS.
//!
//! Two real daemons (`daemon::server::start`, one UDP listener each) run in
//! this process on the same DDS domain. Client A creates a DataWriter on the
//! topic through agent 1, client B creates a DataReader on the same topic and
//! type through agent 2. A writes with `WRITE_DATA`, B reads with `READ_DATA`,
//! and the bytes B receives equal the bytes A wrote.
//!
//! The GUID prefix of the DDS runtime of an agent carries the process and a
//! counter of the runtimes (`dcps_bridge::runtime_instance`): derived from the
//! domain alone, two agents on one host announced the same GUID in one domain
//! and never discovered each other.
//!
//! The writer is reliable and VOLATILE. A sample written before the reader of
//! agent 2 is matched is lost, so the test writes the sample repeatedly until
//! the reader returns it, within one 20 second deadline.

#![cfg(feature = "daemon")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod udp_client;

use std::net::UdpSocket;
use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_xrce::header::ClientKey;
use zerodds_xrce::object_id::ObjectId;
use zerodds_xrce::object_kind::ObjectKind;
use zerodds_xrce::object_repr::{ObjectVariant, ObjectVariantExtra};
use zerodds_xrce::submessages::read_data::DataDeliveryControl;
use zerodds_xrce::submessages::write_data::DataFormat;

use udp_client::{UdpTransport, encode_keyed_point, wait_until};
use zerodds_xrce_agent::daemon::config::{DaemonConfig, ListenerConfig};
use zerodds_xrce_agent::daemon::server;
use zerodds_xrce_client::{ClientEvent, ClientState, XrceClient};

/// Deadline of the whole exchange (write loop and read loop).
const EXCHANGE_DEADLINE: Duration = Duration::from_secs(20);

/// One daemon with its own UDP listener on 127.0.0.1:0.
fn start_daemon() -> server::DaemonHandle {
    let cfg = DaemonConfig {
        discovery_enabled: false,
        listeners: vec![ListenerConfig::Udp {
            bind: "127.0.0.1:0".to_string(),
        }],
        // The binary TypeObject of CREATE(OBJK_TYPE) is an extension of this
        // agent (DDS-XRCE 1.0 §7.7.3.3 has no binary form); see
        // e2e_udp_loopback.rs.
        allow_any_representation_format: true,
        ..DaemonConfig::default()
    };
    server::start(cfg, RuntimeConfig::default(), None).expect("daemon start")
}

fn connect_client(
    daemon_addr: std::net::SocketAddr,
    key: [u8; 4],
    start: Instant,
) -> XrceClient<UdpTransport> {
    let socket = UdpSocket::bind("127.0.0.1:0").expect("bind client socket");
    socket.set_nonblocking(true).expect("nonblocking");
    socket.connect(daemon_addr).expect("connect to daemon");
    let mut client = XrceClient::new(ClientKey(key), 0x81, UdpTransport(socket));
    client.connect([0x01, 0x00]).expect("send CREATE_CLIENT");
    let events = wait_until(
        &mut client,
        start,
        Duration::from_secs(5),
        |c| c.poll(Duration::ZERO).unwrap(),
        "CREATE_CLIENT handshake",
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ClientEvent::Connected { .. })),
        "expected Connected, got {events:?}"
    );
    assert_eq!(client.state(), ClientState::Connected);
    client
}

/// Sends one CREATE and waits for its successful `Status`.
fn create(
    client: &mut XrceClient<UdpTransport>,
    start: Instant,
    object: ObjectId,
    variant: ObjectVariant,
    extra: ObjectVariantExtra,
    what: &str,
) {
    let request_id = client
        .create_object(object, variant, extra, false, false, true)
        .expect("send CREATE");
    let events = wait_until(
        client,
        start,
        Duration::from_secs(5),
        |c| c.poll(Duration::ZERO).unwrap(),
        what,
    );
    let status = events.iter().find_map(|e| match e {
        ClientEvent::Status {
            request_id: r,
            code,
            ..
        } if *r == request_id => Some(*code),
        _ => None,
    });
    assert!(
        status.is_some_and(zerodds_xrce::object_info::ResultStatusCode::is_success),
        "{what} (request_id {request_id}): expected a success STATUS, events were {events:?}"
    );
}

/// Creates participant, type, topic, publisher or subscriber, and the
/// reliable endpoint on one client. Returns the endpoint's `ObjectId`.
/// `user_data` is the base64 text of a USER_DATA value for the participant
/// (selects another runtime GUID prefix), or `None`.
fn build_side(
    client: &mut XrceClient<UdpTransport>,
    start: Instant,
    domain: i32,
    type_object_bytes: &[u8],
    user_data_b64: Option<&str>,
    writer: bool,
) -> ObjectId {
    let participant = ObjectId::new(0x001, ObjectKind::Participant).unwrap();
    let participant_xml = match user_data_b64 {
        Some(b64) => format!(
            "<dds><domain_participant name=\"P\" domain_id=\"{domain}\">\
             <domain_participant_qos><user_data><value>{b64}</value></user_data>\
             </domain_participant_qos></domain_participant></dds>"
        ),
        None => format!("<dds><participant><domain_id>{domain}</domain_id></participant></dds>"),
    };
    create(
        client,
        start,
        participant,
        ObjectVariant::ByXmlString(participant_xml),
        ObjectVariantExtra {
            domain_id: Some(i16::try_from(domain).unwrap()),
            parent: None,
        },
        "CREATE(participant)",
    );

    let type_oid = ObjectId::new(0x002, ObjectKind::Type).unwrap();
    create(
        client,
        start,
        type_oid,
        ObjectVariant::InBinary(type_object_bytes.to_vec()),
        ObjectVariantExtra::default(),
        "CREATE(type)",
    );

    let topic = ObjectId::new(0x003, ObjectKind::Topic).unwrap();
    create(
        client,
        start,
        topic,
        ObjectVariant::ByXmlString(
            "<dds><topic name=\"TwoAgents\" type_name=\"KeyedPoint\"/></dds>".to_string(),
        ),
        ObjectVariantExtra {
            domain_id: None,
            parent: Some(participant),
        },
        "CREATE(topic)",
    );

    let (container_kind, endpoint_kind, element) = if writer {
        (ObjectKind::Publisher, ObjectKind::DataWriter, "data_writer")
    } else {
        (
            ObjectKind::Subscriber,
            ObjectKind::DataReader,
            "data_reader",
        )
    };
    let container = ObjectId::new(0x001, container_kind).unwrap();
    create(
        client,
        start,
        container,
        ObjectVariant::ByXmlString(String::new()),
        ObjectVariantExtra {
            domain_id: None,
            parent: Some(participant),
        },
        "CREATE(publisher/subscriber)",
    );

    let endpoint = ObjectId::new(0x004, endpoint_kind).unwrap();
    create(
        client,
        start,
        endpoint,
        ObjectVariant::ByXmlString(format!(
            "<dds><{element} topic_ref=\"0x{:04x}\" reliability=\"RELIABLE\"/></dds>",
            topic.raw()
        )),
        ObjectVariantExtra {
            domain_id: None,
            parent: Some(container),
        },
        "CREATE(endpoint)",
    );
    endpoint
}

#[test]
fn two_clients_on_two_agents_exchange_a_sample_through_dds() {
    let start = Instant::now();
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id();

    let ast = Arc::new(
        zerodds_wire_probe::encode::model_from_idl(
            "@final struct KeyedPoint { @key long id; sequence<octet> payload; };",
            "two-agents.idl",
        )
        .unwrap(),
    );
    let ty = zerodds_types::dynamic::DynamicType::find(&ast, "KeyedPoint").unwrap();
    let type_object_bytes = zerodds_types::TypeObject::Complete(
        zerodds_types::from_ast::complete_type_object(&ast, ty.id()).unwrap(),
    )
    .to_bytes_le()
    .unwrap();

    let daemon1 = start_daemon();
    let daemon2 = start_daemon();
    let addr1 = daemon1.udp_local_addr().expect("agent 1 bound UDP");
    let addr2 = daemon2.udp_local_addr().expect("agent 2 bound UDP");
    assert_ne!(addr1, addr2, "the two agents need distinct listeners");

    let mut client_a = connect_client(addr1, [0xA1, 0xA1, 0x00, 0x01], start);
    let mut client_b = connect_client(addr2, [0xB2, 0xB2, 0x00, 0x01], start);

    let writer = build_side(&mut client_a, start, domain, &type_object_bytes, None, true);
    let reader = build_side(
        &mut client_b,
        start,
        domain,
        &type_object_bytes,
        None,
        false,
    );

    let sample = encode_keyed_point(17, b"through two agents");
    let next_sample = DataDeliveryControl {
        max_samples: 1,
        max_elapsed_time: 1,
        max_bytes_per_second: 0,
        min_pace_period: 0,
    };

    let loop_start = Instant::now();
    let mut writes = 0u32;
    let mut last_write: Option<Instant> = None;
    let mut last_read: Option<Instant> = None;
    let mut outstanding_reads: Vec<u16> = Vec::new();
    let received: Vec<u8> = loop {
        assert!(
            loop_start.elapsed() < EXCHANGE_DEADLINE,
            "timed out after {EXCHANGE_DEADLINE:?}: agent 2's reader returned no sample \
             after {writes} writes through agent 1"
        );
        client_a.tick(start.elapsed()).expect("tick A");
        client_b.tick(start.elapsed()).expect("tick B");
        client_a.poll(start.elapsed()).expect("poll A");

        // A: write again every 500 ms until B has the sample.
        if last_write.is_none_or(|t| t.elapsed() >= Duration::from_millis(500)) {
            client_a
                .request_write(writer, &sample, true)
                .expect("WRITE_DATA");
            writes += 1;
            last_write = Some(Instant::now());
        }
        // B: a READ_DATA returns what the reader has within one second.
        if last_read.is_none_or(|t| t.elapsed() >= Duration::from_millis(1_200)) {
            let rid = client_b
                .request_read(
                    reader,
                    0x80,
                    DataFormat::Data,
                    None,
                    Some(next_sample),
                    true,
                )
                .expect("READ_DATA");
            outstanding_reads.push(rid);
            last_read = Some(Instant::now());
        }
        let hit = client_b
            .poll(start.elapsed())
            .expect("poll B")
            .into_iter()
            .find_map(|e| match e {
                ClientEvent::Data {
                    request_id,
                    payload,
                    ..
                } if outstanding_reads.contains(&request_id) => Some(payload),
                _ => None,
            });
        if let Some(payload) = hit {
            break payload;
        }
        std::thread::sleep(Duration::from_millis(20));
    };

    assert_eq!(
        received, sample,
        "the bytes client B reads through agent 2 equal the bytes client A wrote through agent 1"
    );

    client_a.disconnect();
    client_b.disconnect();
    drop(daemon2);
    drop(daemon1);
    drop(lease);
}
