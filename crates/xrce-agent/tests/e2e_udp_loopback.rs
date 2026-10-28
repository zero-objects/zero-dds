// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! In-process end-to-end test: a real `zerodds-xrce-client` talks UDP
//! loopback to a real `zerodds-xrce-agentd` (`daemon::server::start`, the
//! same entry point the binary uses), which bridges into a real DDS
//! domain a plain `zerodds-dcps` `DomainParticipant` (no XRCE involved)
//! sits on as the other side.
//!
//! Four phases, all against one keyed type (`@key long id; sequence<octet>
//! payload;`, chosen specifically so `payload` can be small or large
//! enough to force `FRAGMENT` with the same type/topics throughout):
//!
//! 1. **Reliable, keyed, with dispose.** The XRCE client writes an
//!    instance; the plain DCPS reader on the "Forward" topic sees it
//!    `Alive` with the right bytes. The XRCE client then disposes the
//!    same instance (`FORMAT_SAMPLE`, `request_write_sample`); the DCPS
//!    reader sees `NotAliveDisposed` for the same key — proof
//!    `RealDdsPort::write` computed a real `KeyHash` from the
//!    `SerializedTypeSupport` this daemon built from the XRCE client's
//!    own `CREATE(OBJK_TYPE)`, not a stub.
//! 2. **Best-effort.** A second writer/reader pair
//!    (`reliability="BEST_EFFORT"` in the XRCE `CREATE`, matching
//!    `DataReaderQos` on the DCPS side) round-trips a sample the same
//!    way, without the reliable stream's ACKNACK machinery.
//! 3. **A payload above the MTU, forward direction.** The XRCE client
//!    writes a sample whose body is larger than `DEFAULT_MTU` (508
//!    bytes) on the reliable writer — this client's own outbound
//!    `FRAGMENT` split (`send_maybe_fragmented`) and the daemon's inbound
//!    reassembly (`SessionEngine::handle_message`'s `recv_data`/
//!    `drain_in_order` + `FragmentAssembler` gate) both have to work for
//!    the DCPS reader to see the whole thing.
//! 4. **A payload above the MTU, return direction.** The plain DCPS
//!    participant writes an oversized sample on the "Return" topic; the
//!    XRCE client reads it back via `READ_DATA` — this exercises the
//!    daemon's outbound `FRAGMENT` split (`pump_read::
//!    emit_possibly_fragmented`) and this client's own inbound
//!    reassembly.

#![cfg(feature = "daemon")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod udp_client;

use std::net::UdpSocket;
use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::serialized_type::SerializedTypeSupport;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
    PublisherQos, SubscriberQos, TopicQos,
};
use zerodds_qos::{ReliabilityKind, ReliabilityQosPolicy};
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

fn decode_keyed_point_id(bytes: &[u8]) -> i32 {
    i32::from_le_bytes(bytes[0..4].try_into().unwrap())
}

/// Waits for SPDP/SEDP to match a DCPS writer/reader pair (VOLATILE
/// durability, the QoS default, replays nothing to a reader that matches
/// only after the write already happened) before any test phase writes
/// through it.
fn wait_for_match(budget: Duration, mut matched_count: impl FnMut() -> usize, what: &str) {
    let start = Instant::now();
    loop {
        if matched_count() > 0 {
            return;
        }
        if start.elapsed() > budget {
            panic!("timed out after {budget:?} waiting for discovery match: {what}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn start_test_daemon() -> (server::DaemonHandle, Vec<u8>) {
    let ast = Arc::new(
        zerodds_wire_probe::encode::model_from_idl(
            "@final struct KeyedPoint { @key long id; sequence<octet> payload; };",
            "e2e-udp-loopback.idl",
        )
        .unwrap(),
    );
    let ty = zerodds_types::dynamic::DynamicType::find(&ast, "KeyedPoint").unwrap();
    let type_object_bytes = zerodds_types::TypeObject::Complete(
        zerodds_types::from_ast::complete_type_object(&ast, ty.id()).unwrap(),
    )
    .to_bytes_le()
    .unwrap();

    let cfg = DaemonConfig {
        discovery_enabled: false,
        listeners: vec![ListenerConfig::Udp {
            bind: "127.0.0.1:0".to_string(),
        }],
        // DDS-XRCE 1.0 §7.7.3.3 and Annex A: an OBJK_TYPE_Representation is
        // by reference or XML; it has no binary form. This test creates its
        // type as a binary TypeObject (an extension of this agent), so it
        // names the setting that admits the format.
        allow_any_representation_format: true,
        ..DaemonConfig::default()
    };
    let handle = server::start(cfg, RuntimeConfig::default(), None).expect("daemon start");
    (handle, type_object_bytes)
}

fn connect_client(daemon_addr: std::net::SocketAddr, start: Instant) -> XrceClient<UdpTransport> {
    let socket = UdpSocket::bind("127.0.0.1:0").expect("bind client socket");
    socket.set_nonblocking(true).expect("nonblocking");
    socket.connect(daemon_addr).expect("connect to daemon");
    let mut client = XrceClient::new(
        ClientKey([0xE2, 0xE2, 0x00, 0x01]),
        0x81,
        UdpTransport(socket),
    );
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

/// Sends a request and waits for its `Status` event, asserting success.
fn create_and_wait(
    client: &mut XrceClient<UdpTransport>,
    start: Instant,
    request_id: u16,
    what: &str,
) {
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

#[test]
fn e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments() {
    let start = Instant::now();
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id();
    let (daemon, type_object_bytes) = start_test_daemon();
    let daemon_addr = daemon
        .udp_local_addr()
        .expect("daemon bound a UDP listener");

    let mut client = connect_client(daemon_addr, start);

    // ---- CREATE(participant), CREATE(type), CREATE(topic) x2 ----
    let participant = ObjectId::new(0x001, ObjectKind::Participant).unwrap();
    let rid = client
        .create_object(
            participant,
            ObjectVariant::ByXmlString(format!(
                "<dds><participant><domain_id>{domain}</domain_id></participant></dds>"
            )),
            ObjectVariantExtra {
                domain_id: Some(i16::try_from(domain).unwrap()),
                parent: None,
            },
            false,
            false,
            true,
        )
        .unwrap();
    create_and_wait(&mut client, start, rid, "CREATE(participant)");

    let type_oid = ObjectId::new(0x002, ObjectKind::Type).unwrap();
    let rid = client
        .create_object(
            type_oid,
            ObjectVariant::InBinary(type_object_bytes.clone()),
            ObjectVariantExtra::default(),
            false,
            false,
            true,
        )
        .unwrap();
    create_and_wait(&mut client, start, rid, "CREATE(type)");

    let forward_topic = ObjectId::new(0x003, ObjectKind::Topic).unwrap();
    let rid = client
        .create_object(
            forward_topic,
            ObjectVariant::ByXmlString(
                "<dds><topic name=\"Forward\" type_name=\"KeyedPoint\"/></dds>".to_string(),
            ),
            ObjectVariantExtra {
                domain_id: None,
                parent: Some(participant),
            },
            false,
            false,
            true,
        )
        .unwrap();
    create_and_wait(&mut client, start, rid, "CREATE(Forward topic)");

    let return_topic = ObjectId::new(0x004, ObjectKind::Topic).unwrap();
    let rid = client
        .create_object(
            return_topic,
            ObjectVariant::ByXmlString(
                "<dds><topic name=\"Return\" type_name=\"KeyedPoint\"/></dds>".to_string(),
            ),
            ObjectVariantExtra {
                domain_id: None,
                parent: Some(participant),
            },
            false,
            false,
            true,
        )
        .unwrap();
    create_and_wait(&mut client, start, rid, "CREATE(Return topic)");

    // ---- CREATE(publisher), CREATE(subscriber): the endpoints' containers ----
    let publisher = ObjectId::new(0x001, ObjectKind::Publisher).unwrap();
    let subscriber = ObjectId::new(0x001, ObjectKind::Subscriber).unwrap();
    for (container, what) in [
        (publisher, "CREATE(publisher)"),
        (subscriber, "CREATE(subscriber)"),
    ] {
        let rid = client
            .create_object(
                container,
                ObjectVariant::ByXmlString(String::new()),
                ObjectVariantExtra {
                    domain_id: None,
                    parent: Some(participant),
                },
                false,
                false,
                true,
            )
            .unwrap();
        create_and_wait(&mut client, start, rid, what);
    }

    // ---- CREATE(writer on Forward, reliable), CREATE(reader on Return, reliable) ----
    let fwd_writer = ObjectId::new(0x005, ObjectKind::DataWriter).unwrap();
    let rid = client
        .create_object(
            fwd_writer,
            ObjectVariant::ByXmlString(format!(
                "<dds><data_writer topic_ref=\"0x{:04x}\" reliability=\"RELIABLE\"/></dds>",
                forward_topic.raw()
            )),
            ObjectVariantExtra {
                domain_id: None,
                parent: Some(publisher),
            },
            false,
            false,
            true,
        )
        .unwrap();
    create_and_wait(&mut client, start, rid, "CREATE(Forward writer)");

    let ret_reader = ObjectId::new(0x006, ObjectKind::DataReader).unwrap();
    let rid = client
        .create_object(
            ret_reader,
            ObjectVariant::ByXmlString(format!(
                "<dds><data_reader topic_ref=\"0x{:04x}\" reliability=\"RELIABLE\"/></dds>",
                return_topic.raw()
            )),
            ObjectVariantExtra {
                domain_id: None,
                parent: Some(subscriber),
            },
            false,
            false,
            true,
        )
        .unwrap();
    create_and_wait(&mut client, start, rid, "CREATE(Return reader)");

    // ---- Best-effort writer/reader pair on the same two topics ----
    let fwd_writer_be = ObjectId::new(0x007, ObjectKind::DataWriter).unwrap();
    let rid = client
        .create_object(
            fwd_writer_be,
            ObjectVariant::ByXmlString(format!(
                "<dds><data_writer topic_ref=\"0x{:04x}\" reliability=\"BEST_EFFORT\"/></dds>",
                forward_topic.raw()
            )),
            ObjectVariantExtra {
                domain_id: None,
                parent: Some(publisher),
            },
            false,
            false,
            true,
        )
        .unwrap();
    create_and_wait(
        &mut client,
        start,
        rid,
        "CREATE(Forward best-effort writer)",
    );

    // ---- Plain DCPS side (no XRCE) on the same domain ----
    let support =
        SerializedTypeSupport::from_type_object("KeyedPoint", &type_object_bytes, true, 0)
            .expect("type support");
    let native = DomainParticipantFactory::instance()
        .create_participant_with_config(
            domain,
            DomainParticipantQos::default(),
            RuntimeConfig::default(),
        )
        .expect("native participant");
    let forward_native_topic = native
        .create_serialized_topic("Forward", TopicQos::default(), Arc::clone(&support))
        .expect("native Forward topic");
    let return_native_topic = native
        .create_serialized_topic("Return", TopicQos::default(), Arc::clone(&support))
        .expect("native Return topic");
    let sub = native.create_subscriber(SubscriberQos::default());
    let fwd_reader = sub
        .create_datareader(&forward_native_topic, DataReaderQos::default())
        .expect("native Forward reader (default = best-effort, compatible with a reliable writer)");
    let fwd_reader_be = sub
        .create_datareader(
            &forward_native_topic,
            DataReaderQos {
                reliability: ReliabilityQosPolicy {
                    kind: ReliabilityKind::BestEffort,
                    ..DataReaderQos::default().reliability
                },
                ..DataReaderQos::default()
            },
        )
        .expect("native Forward best-effort reader");
    let publ = native.create_publisher(PublisherQos::default());
    let ret_writer = publ
        .create_datawriter(&return_native_topic, DataWriterQos::default())
        .expect("native Return writer");

    // ================= Phase 1: reliable, keyed, with dispose =================
    wait_for_match(
        Duration::from_secs(15),
        || fwd_reader.matched_publication_count(),
        "reliable Forward writer <-> native Forward reader",
    );
    let sample1 = encode_keyed_point(42, b"hello from xrce");
    let rid = client
        .request_write(fwd_writer, &sample1, true)
        .expect("write sample1");
    let _ = rid; // WRITE_DATA gets no STATUS reply (spec) -- verified via the DCPS reader below.

    let mut phase1_buffer = Vec::new();
    let alive = wait_until(
        &mut client,
        start,
        Duration::from_secs(10),
        |_c| {
            phase1_buffer.extend(fwd_reader.take_with_info().unwrap_or_default());
            phase1_buffer
                .iter()
                .filter(|s| {
                    s.info.valid_data
                        && s.data.field_value("id") == Some(zerodds_sql_filter::Value::Int(42))
                })
                .cloned()
                .collect::<Vec<_>>()
        },
        "phase 1: native reader sees the XRCE-written instance",
    );
    assert_eq!(alive.len(), 1, "exactly one live sample for id=42 expected");
    assert_eq!(alive[0].data.body(), sample1.as_slice());

    client
        .request_write_sample(fwd_writer, &sample1, true, false, true)
        .expect("dispose instance 42");
    let disposed = wait_until(
        &mut client,
        start,
        Duration::from_secs(10),
        |_c| {
            phase1_buffer.extend(fwd_reader.take_with_info().unwrap_or_default());
            phase1_buffer
                .iter()
                .filter(|s| {
                    !s.info.valid_data
                        && s.info.instance_state
                            == zerodds_dcps::InstanceStateKind::NotAliveDisposed
                        && s.data.field_value("id") == Some(zerodds_sql_filter::Value::Int(42))
                })
                .cloned()
                .collect::<Vec<_>>()
        },
        "phase 1: native reader sees the dispose",
    );
    assert_eq!(
        disposed.len(),
        1,
        "exactly one dispose notification for id=42 expected"
    );

    // DDS-XRCE 1.0 §7.8.4.1: a Sample whose SampleInfo has the unregister
    // flag set makes the agent call unregister on the DDS DataWriter for the
    // instance in the data: the native reader sees NOT_ALIVE_NO_WRITERS.
    let sample_43 = encode_keyed_point(43, b"to be unregistered");
    client
        .request_write(fwd_writer, &sample_43, true)
        .expect("write sample for id=43");
    client
        .request_write_sample(fwd_writer, &sample_43, false, true, true)
        .expect("unregister instance 43");
    let unregistered = wait_until(
        &mut client,
        start,
        Duration::from_secs(10),
        |_c| {
            phase1_buffer.extend(fwd_reader.take_with_info().unwrap_or_default());
            phase1_buffer
                .iter()
                .filter(|s| {
                    !s.info.valid_data
                        && s.info.instance_state
                            == zerodds_dcps::InstanceStateKind::NotAliveNoWriters
                        && s.data.field_value("id") == Some(zerodds_sql_filter::Value::Int(43))
                })
                .cloned()
                .collect::<Vec<_>>()
        },
        "phase 1: native reader sees the unregister",
    );
    assert_eq!(
        unregistered.len(),
        1,
        "exactly one unregister notification for id=43 expected"
    );

    // ================= Phase 2: best-effort =================
    wait_for_match(
        Duration::from_secs(15),
        || fwd_reader_be.matched_publication_count(),
        "best-effort Forward writer <-> native best-effort Forward reader",
    );
    let sample2 = encode_keyed_point(7, b"best effort payload");
    client
        .request_write(fwd_writer_be, &sample2, false)
        .expect("write sample2 (best-effort)");
    let mut phase2_buffer = Vec::new();
    let received_be = wait_until(
        &mut client,
        start,
        Duration::from_secs(10),
        |_c| {
            phase2_buffer.extend(fwd_reader_be.take_with_info().unwrap_or_default());
            phase2_buffer
                .iter()
                .filter(|s| {
                    s.info.valid_data
                        && s.data.field_value("id") == Some(zerodds_sql_filter::Value::Int(7))
                })
                .cloned()
                .collect::<Vec<_>>()
        },
        "phase 2: native best-effort reader sees the sample",
    );
    assert_eq!(received_be.len(), 1);
    assert_eq!(received_be[0].data.body(), sample2.as_slice());

    // ================= Phase 3: above-MTU, forward direction =================
    let big_payload = vec![0xABu8; 700]; // body > zerodds_xrce::handshake::DEFAULT_MTU (508)
    let sample3 = encode_keyed_point(99, &big_payload);
    client
        .request_write(fwd_writer, &sample3, true)
        .expect("write oversized sample");
    let received_big = wait_until(
        &mut client,
        start,
        Duration::from_secs(10),
        |_c| {
            phase1_buffer.extend(fwd_reader.take_with_info().unwrap_or_default());
            phase1_buffer
                .iter()
                .filter(|s| {
                    s.info.valid_data
                        && s.data.field_value("id") == Some(zerodds_sql_filter::Value::Int(99))
                })
                .cloned()
                .collect::<Vec<_>>()
        },
        "phase 3: native reader sees the reassembled oversized sample",
    );
    assert_eq!(received_big.len(), 1);
    assert_eq!(received_big[0].data.body(), sample3.as_slice());
    assert_eq!(decode_keyed_point_id(received_big[0].data.body()), 99);

    // ================= Phase 4: above-MTU, return direction =================
    wait_for_match(
        Duration::from_secs(15),
        || ret_writer.matched_subscription_count(),
        "native Return writer <-> reliable XRCE Return reader",
    );
    let big_return_payload = vec![0xCDu8; 700];
    let sample4 = encode_keyed_point(123, &big_return_payload);
    let sample4_typed = support
        .read_body(&sample4, 2, false)
        .expect("decode sample4");
    ret_writer
        .write(&sample4_typed)
        .expect("native write oversized");

    // The next sample, within 10 s: a READ_DATA without delivery_control
    // returns only what the agent's reader has at that moment (DDS-XRCE 1.0
    // §8.3.5.9.2), and the sample may still be on its way.
    let next_sample = DataDeliveryControl {
        max_samples: 1,
        max_elapsed_time: 10,
        max_bytes_per_second: 0,
        min_pace_period: 0,
    };
    let rid = client
        .request_read(
            ret_reader,
            0x80,
            DataFormat::Data,
            None,
            Some(next_sample),
            true,
        )
        .expect("READ_DATA");
    let data_events = wait_until(
        &mut client,
        start,
        Duration::from_secs(10),
        |c| {
            c.poll(Duration::ZERO)
                .unwrap()
                .into_iter()
                .filter(|e| matches!(e, ClientEvent::Data { request_id: r, .. } if *r == rid))
                .collect::<Vec<_>>()
        },
        "phase 4: XRCE client reads back the reassembled oversized sample",
    );
    assert_eq!(data_events.len(), 1);
    let ClientEvent::Data { payload, .. } = &data_events[0] else {
        panic!("filtered for ClientEvent::Data above");
    };
    assert_eq!(
        payload, &sample4,
        "the reassembled bytes must match exactly"
    );

    client.disconnect();
}
