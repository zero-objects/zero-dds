// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! E2E test for `zerodds-mqtt-bridged`. Spec §12.2.
//!
//! Strategy: a tiny MQTT-5 broker in the test process (uses the
//! crate's own codec modules + `Broker` logic), and the daemon is
//! connected against it in the same process via `daemon::server::start()`.
//! We verify:
//!
//! * L1 — the daemon speaks MQTT-5 wire (CONNECT/CONNACK roundtrip
//!   visible in the mock broker).
//! * L3 — bidir pump: another MQTT client publishes, the daemon
//!   writes the sample into the DDS domain (internally); the reverse
//!   unfortunately with a multicast-loopback caveat (see note below).

#![cfg(feature = "daemon")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use zerodds_mqtt_bridge::codec::{PublishPacket, decode_publish, encode_publish};
use zerodds_mqtt_bridge::control_packets::{
    ConnackBody, decode_connect_body, decode_subscribe_body, encode_connack_body,
};
use zerodds_mqtt_bridge::daemon::config::{DaemonConfig, TopicConfig};
use zerodds_mqtt_bridge::daemon::server;
use zerodds_mqtt_bridge::packet::ControlPacketType;
use zerodds_mqtt_bridge::vbi::{decode_vbi, encode_vbi};

/// Minimal MQTT-5 mock broker for test purposes.
///
/// Accepts exactly one client (the daemon), answers CONNECT
/// with CONNACK, swallows SUBSCRIBE, forwards PUBLISH frames to an
/// `Arc<Mutex<Vec<...>>>` recorder and can inject additional
/// PUBLISH frames to the client.
pub struct MockBroker {
    pub bind_addr: String,
    pub stop: Arc<AtomicBool>,
    pub state: Arc<MockState>,
    _accept_thread: thread::JoinHandle<()>,
}

#[derive(Default)]
pub struct MockState {
    pub publishes: Mutex<Vec<(String, Vec<u8>)>>,
    pub subscribes: Mutex<Vec<String>>,
    pub client_stream: Mutex<Option<TcpStream>>,
    pub connect_seen: AtomicU8,
}

impl MockBroker {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind broker");
        let bind_addr = listener.local_addr().expect("addr").to_string();
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(MockState::default());
        let stop_c = Arc::clone(&stop);
        let state_c = Arc::clone(&state);
        let accept_thread = thread::spawn(move || {
            for incoming in listener.incoming() {
                if stop_c.load(Ordering::SeqCst) {
                    break;
                }
                let stream = match incoming {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                stream
                    .set_read_timeout(Some(Duration::from_millis(200)))
                    .ok();
                if let Ok(clone) = stream.try_clone() {
                    *state_c.client_stream.lock().unwrap() = Some(clone);
                }
                handle_client(stream, Arc::clone(&state_c), Arc::clone(&stop_c));
                break; // single-client-broker
            }
        });
        Self {
            bind_addr,
            stop,
            state,
            _accept_thread: accept_thread,
        }
    }

    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::SeqCst);
        // Self-connect to wake accept().
        if let Ok(addr) = self.bind_addr.parse::<std::net::SocketAddr>() {
            let _ = TcpStream::connect_timeout(&addr, Duration::from_millis(100));
        }
    }

    /// Sends a PUBLISH from the broker to the client.
    pub fn inject_publish(&self, topic: &str, payload: &[u8]) {
        let pkt = PublishPacket {
            dup: false,
            qos: 0,
            retain: false,
            topic: topic.to_string(),
            packet_id: None,
            properties: Vec::new(),
            payload: payload.to_vec(),
        };
        let bytes = encode_publish(&pkt).expect("encode");
        if let Some(stream) = self.state.client_stream.lock().unwrap().as_mut() {
            let _ = stream.write_all(&bytes);
        }
    }
}

fn handle_client(mut stream: TcpStream, state: Arc<MockState>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::SeqCst) {
        let mut hdr = [0u8; 1];
        match stream.read_exact(&mut hdr) {
            Ok(()) => {}
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                // The MockBroker has set_read_timeout(200 ms) broker-side
                // — on idle TCP, read_exact returns a WouldBlock,
                // which is NOT a connection drop. Keep looping, otherwise
                // handle_client ends the session prematurely (before the
                // daemon's first PUBLISH frame).
                continue;
            }
            Err(_) => break,
        }
        let mut vbi = Vec::new();
        loop {
            let mut b = [0u8; 1];
            if stream.read_exact(&mut b).is_err() {
                return;
            }
            vbi.push(b[0]);
            if b[0] & 0x80 == 0 {
                break;
            }
            if vbi.len() >= 4 {
                return;
            }
        }
        let (rem, _) = match decode_vbi(&vbi) {
            Ok(p) => p,
            Err(_) => return,
        };
        let mut body = vec![0u8; rem as usize];
        if !body.is_empty() && stream.read_exact(&mut body).is_err() {
            return;
        }
        let pt_bits = (hdr[0] >> 4) & 0x0F;
        let pt = match ControlPacketType::from_bits(pt_bits) {
            Some(p) => p,
            None => continue,
        };
        match pt {
            ControlPacketType::Connect => {
                state.connect_seen.fetch_add(1, Ordering::SeqCst);
                let _ = decode_connect_body(&body);
                // CONNACK 0x00.
                let connack = ConnackBody {
                    session_present: false,
                    reason_code: 0,
                    properties: Vec::new(),
                };
                let body_bytes = encode_connack_body(&connack).expect("connack");
                let mut frame = Vec::new();
                frame.push((ControlPacketType::ConnAck.to_bits()) << 4);
                let len_u32 = u32::try_from(body_bytes.len()).unwrap();
                frame.extend_from_slice(&encode_vbi(len_u32).expect("vbi"));
                frame.extend_from_slice(&body_bytes);
                if stream.write_all(&frame).is_err() {
                    return;
                }
            }
            ControlPacketType::Subscribe => {
                if let Ok(sub) = decode_subscribe_body(&body) {
                    let mut subs = state.subscribes.lock().unwrap();
                    for s in &sub.subscriptions {
                        subs.push(s.topic_filter.clone());
                    }
                    // Send SUBACK.
                    let mut suback_body = Vec::new();
                    suback_body.extend_from_slice(&sub.packet_id.to_be_bytes());
                    suback_body.push(0); // properties length VBI
                    suback_body.resize(suback_body.len() + sub.subscriptions.len(), 0); // success per subscription
                    let mut frame = Vec::new();
                    frame.push((ControlPacketType::SubAck.to_bits()) << 4);
                    let len_u32 = u32::try_from(suback_body.len()).unwrap();
                    frame.extend_from_slice(&encode_vbi(len_u32).expect("vbi"));
                    frame.extend_from_slice(&suback_body);
                    if stream.write_all(&frame).is_err() {
                        return;
                    }
                }
            }
            ControlPacketType::Publish => {
                // Daemon → Broker: rebuild + decode.
                let mut full = Vec::new();
                full.push(hdr[0]);
                full.extend_from_slice(&vbi);
                full.extend_from_slice(&body);
                if let Ok((_, pkt)) = decode_publish(&full) {
                    // A QoS 1 PUBLISH (the catalog of §5.2) gets its PUBACK,
                    // or the daemon's shutdown waits for it.
                    if let (1, Some(id)) = (pkt.qos, pkt.packet_id) {
                        let mut ack = vec![0x40, 0x03];
                        ack.extend_from_slice(&id.to_be_bytes());
                        ack.push(0);
                        if stream.write_all(&ack).is_err() {
                            return;
                        }
                    }
                    state
                        .publishes
                        .lock()
                        .unwrap()
                        .push((pkt.topic, pkt.payload));
                }
            }
            ControlPacketType::Disconnect => return,
            _ => {}
        }
    }
}

/// The daemon's configuration on a leased DDS domain (the host's share of
/// the band, `zerodds-test-domains`); the lease is held while the test runs.
fn make_test_config(broker_addr: &str) -> (DaemonConfig, zerodds_test_domains::DomainLease) {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.broker_url = format!("mqtt://{broker_addr}");
    cfg.client_id = "zerodds-test-bridge".to_string();
    let lease = zerodds_test_domains::lease_domain();
    cfg.domain = lease.id();
    cfg.topics.push(TopicConfig {
        dds_name: "Trade".to_string(),
        dds_type: "Trade".to_string(),
        mqtt_topic: "trade".to_string(),
        direction: "bidir".to_string(),
        mqtt_qos: Some(0),
        retain: Some(false),
        reliability: "best_effort".to_string(),
        durability: "volatile".to_string(),
        ..TopicConfig::default()
    });
    (cfg, lease)
}

#[test]
fn mqttbridge_2_daemon_connects_and_subscribes() {
    let broker = MockBroker::start();
    let (cfg, _lease) = make_test_config(&broker.bind_addr);
    let mut handle = server::start(cfg).expect("daemon start");

    // Wait until the CONNECT arrives + SUBSCRIBE is registered.
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if broker.state.connect_seen.load(Ordering::SeqCst) > 0
            && !broker.state.subscribes.lock().unwrap().is_empty()
        {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    assert!(
        broker.state.connect_seen.load(Ordering::SeqCst) > 0,
        "expected CONNECT to be observed"
    );
    let subs = broker.state.subscribes.lock().unwrap().clone();
    assert!(
        subs.iter().any(|f| f == "trade"),
        "expected subscribe to `trade`, got: {subs:?}"
    );
    handle.shutdown();
    broker.shutdown();
}

/// Spec §12.2: a PUBLISH of the broker on a subscribed topic is written into
/// DDS: a reader of the test on the daemon's participant receives the CDR
/// bytes after the encapsulation header, octet for octet. The test used to
/// send a payload without an encapsulation header, which the daemon
/// refuses, and checked only that nothing panicked.
#[test]
fn mqttbridge_12_2_an_mqtt_publish_is_written_to_dds() {
    use zerodds_dcps::runtime::{UserReaderConfig, UserSample};
    let broker = MockBroker::start();
    let (cfg, _lease) = make_test_config(&broker.bind_addr);
    let mut handle = server::start(cfg).expect("daemon start");
    let (_reader, rx) = handle
        .runtime
        .register_user_reader(UserReaderConfig {
            topic_name: "Trade".into(),
            type_name: "Trade".into(),
            reliable: false,
            ..Default::default()
        })
        .expect("the test's reader");
    let deadline = Instant::now() + Duration::from_secs(10);
    while broker.state.subscribes.lock().unwrap().is_empty() {
        assert!(Instant::now() < deadline, "no SUBSCRIBE");
        thread::sleep(Duration::from_millis(20));
    }
    // The encapsulation header of CDR little endian, then the CDR bytes.
    let body = b"AAPL@200.5";
    let mut publish = vec![0x00, 0x01, 0x00, 0x00];
    publish.extend_from_slice(body);
    let got = loop {
        broker.inject_publish("trade", &publish);
        if let Ok(UserSample::Alive { payload, .. }) = rx.recv_timeout(Duration::from_millis(300)) {
            break payload.to_vec();
        }
        assert!(Instant::now() < deadline, "nothing reached the DDS reader");
    };
    assert_eq!(got, body);
    handle.shutdown();
    broker.shutdown();
}

#[test]
fn mqttbridge_2_dds_publish_pumps_to_mqtt_broker() {
    // L3 outbound: we inject a synthetic alive sample
    // directly into the mpsc::Receiver channel that feeds the bridge
    // pump thread (via `DcpsRuntime::test_inject_user_alive`). This
    // bypasses the cross-participant RTPS path (multicast loopback,
    // which is unreliable in CI containers) but exercises the
    // full pump path in production form: reader channel → ACL read
    // check → MqttClient::publish → MQTT PUBLISH at the broker.

    let broker = MockBroker::start();
    let (cfg, _lease) = make_test_config(&broker.bind_addr);
    let mut handle = server::start(cfg).expect("daemon start");

    // Daemon bringup (MQTT CONNECT + reader-pump threads).
    thread::sleep(Duration::from_millis(500));

    // Synthetic sample injection directly onto the reader channel of the
    // daemon runtime — bypasses wire+discovery but exercises the
    // full pump path (mpsc::recv → ACL check → MQTT PUBLISH).
    let reader_eid = *handle
        .user_readers
        .get("Trade")
        .expect("daemon registered user_reader for Trade");
    assert!(
        handle
            .runtime
            .test_inject_user_alive(reader_eid, b"GOOG@2900".to_vec()),
        "test_inject_user_alive failed (reader slot missing or channel closed)"
    );

    // Wait for pump → MQTT PUBLISH. CI runners and coverage instrumentation
    // are 2-5x slower than a dev laptop, hence adaptive.
    let pump_timeout = if std::env::var_os("LLVM_PROFILE_FILE").is_some() {
        Duration::from_secs(30)
    } else if std::env::var_os("CI").is_some() {
        Duration::from_secs(15)
    } else {
        Duration::from_secs(5)
    };
    let deadline = Instant::now() + pump_timeout;
    // The catalog of §5.2 on `$zerodds/…` comes first.
    while Instant::now() < deadline {
        if broker
            .state
            .publishes
            .lock()
            .unwrap()
            .iter()
            .any(|(t, _)| t == "trade")
        {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    let pubs = broker.state.publishes.lock().unwrap().clone();
    assert!(
        pubs.iter().any(|(t, _)| t == "trade"),
        "expected mqtt-publish on `trade` (waited up to {pump_timeout:?}), got: {pubs:?}"
    );
    handle.shutdown();
    broker.shutdown();
}

/// §9.2 SIGHUP: a reload that closes the topic (default deny, no rule)
/// stops the samples to the broker at once; a configuration that does not
/// load changes nothing.
#[test]
fn mqttbridge_9_2_a_reload_that_withdraws_the_read_right_stops_the_samples() {
    let broker = MockBroker::start();
    let (cfg, _lease) = make_test_config(&broker.bind_addr);
    let next = std::sync::Arc::new(std::sync::Mutex::new(Ok::<DaemonConfig, String>(
        cfg.clone(),
    )));
    let n = std::sync::Arc::clone(&next);
    let mut handle = server::start_with_reload(
        cfg.clone(),
        zerodds_dcps::runtime::RuntimeConfig::default(),
        Some(Box::new(move || n.lock().unwrap().clone())),
    )
    .expect("daemon start");
    thread::sleep(Duration::from_millis(500));
    let reader = *handle.user_readers.get("Trade").unwrap();
    // Counted by payload: `trade` is bidirectional, so the broker may hand
    // the bridge's own publishes back to it. A PUBLISH carries the
    // encapsulation header and then the CDR bytes (zerodds-mqtt-bridge-1.0
    // §4.2); the injected sample is XCDR1, little endian: `00 01 00 00`.
    let arrived = |broker: &MockBroker, payload: &[u8]| {
        let message = [&[0x00, 0x01, 0x00, 0x00][..], payload].concat();
        broker
            .state
            .publishes
            .lock()
            .unwrap()
            .iter()
            .filter(|(t, p)| t == "trade" && *p == message)
            .count()
    };
    let send_and_wait = |payload: &[u8]| {
        assert!(
            handle
                .runtime
                .test_inject_user_alive(reader, payload.to_vec())
        );
        let t0 = Instant::now();
        let deadline = t0 + Duration::from_secs(8);
        while Instant::now() < deadline && arrived(&broker, payload) == 0 {
            thread::sleep(Duration::from_millis(10));
        }
        let n = arrived(&broker, payload);
        // The pump used to wait for the inbound thread's read timeout: 4.5 s
        // per sample (measured); now 15 ms.
        if n > 0 {
            assert!(
                t0.elapsed() < Duration::from_secs(2),
                "took {:?}",
                t0.elapsed()
            );
        }
        n
    };
    assert_eq!(send_and_wait(b"one"), 1, "before the reload");
    *next.lock().unwrap() = Err("a file that does not load".into());
    handle.reload_flag.store(true, Ordering::SeqCst);
    thread::sleep(Duration::from_millis(400));
    assert_eq!(send_and_wait(b"two"), 1, "a failed reload keeps the ACL");
    let mut closed = cfg;
    closed.acl_default_deny = Some(true);
    *next.lock().unwrap() = Ok(closed);
    handle.reload_flag.store(true, Ordering::SeqCst);
    thread::sleep(Duration::from_millis(400));
    assert_eq!(
        send_and_wait(b"three"),
        0,
        "after the reload the topic is closed"
    );
    handle.shutdown();
    broker.shutdown();
}

/// zerodds-mqtt-bridge-1.0 §4.2 with DDS-XTypes 1.3 §7.6.3.1.2, Table 60: a
/// PUBLISH carries the encapsulation header the sample came under. A
/// `@mutable` sample written big-endian (`PL_CDR2_BE`, `00 0a`) went out as
/// `CDR2_BE` (`00 06`): the header was built from the version and the byte
/// order alone (OPEN-ITEMS 60). The octets are the vendors' of
/// `struct Sample { long a; double b; short c; }` (`@mutable`, XCDR2 big
/// endian, `tests/interop-corpus/expected-bytes.tsv`).
#[test]
fn xtypes_7_6_3_1_2_a_publish_carries_the_header_the_sample_came_under() {
    use zerodds_dcps::runtime::UserWriterConfig;
    use zerodds_types::qos::ExtensibilityForRepr;
    let broker = MockBroker::start();
    let (cfg, _lease) = make_test_config(&broker.bind_addr);
    let mut handle = server::start(cfg).expect("daemon start");
    assert!(
        handle.user_readers.contains_key("Trade"),
        "the Trade reader"
    );
    let writer = handle
        .runtime
        .register_user_writer(UserWriterConfig {
            topic_name: "Trade".into(),
            type_name: "Trade".into(),
            data_representation_offer: Some(vec![2]),
            ..Default::default()
        })
        .expect("a writer beside the bridge");
    handle
        .runtime
        .set_user_writer_wire_extensibility(writer, ExtensibilityForRepr::Mutable)
        .unwrap();
    handle
        .runtime
        .set_user_writer_byte_order_override(writer, true)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while handle
        .runtime
        .user_writer_matched_reader_guids(writer)
        .is_empty()
        || broker.state.subscribes.lock().unwrap().is_empty()
    {
        assert!(
            Instant::now() < deadline,
            "the bridge did not subscribe or match"
        );
        thread::sleep(Duration::from_millis(20));
    }
    let octets: Vec<u8> = (0..30)
        .map(|i| {
            u8::from_str_radix(
                &"0000001a20000000ffed297930000001400921fb54442d1810000002fffe"[2 * i..2 * i + 2],
                16,
            )
            .unwrap()
        })
        .collect();
    handle
        .runtime
        .write_user_sample(writer, octets.clone())
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let published = loop {
        let found = broker
            .state
            .publishes
            .lock()
            .unwrap()
            .iter()
            .find(|(topic, _)| topic == "trade")
            .map(|(_, payload)| payload.clone());
        if let Some(payload) = found {
            break payload;
        }
        assert!(Instant::now() < deadline, "no PUBLISH on `trade`");
        thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(published[..4], [0x00, 0x0a, 0x00, 0x00], "PL_CDR2_BE");
    assert_eq!(published[4..], octets[..]);
    handle.shutdown();
    broker.shutdown();
}

/// A participant of the test on `domain`, beside the daemon's, reached over
/// RTPS.
fn foreign_participant(domain: i32) -> Arc<zerodds_dcps::runtime::DcpsRuntime> {
    let mut prefix = [0u8; 12];
    prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
    prefix[4..8].copy_from_slice(&domain.to_be_bytes());
    prefix[11] = 0x5c;
    zerodds_dcps::runtime::DcpsRuntime::start(
        domain,
        zerodds_rtps::wire_types::GuidPrefix::from_bytes(prefix),
        zerodds_dcps::runtime::RuntimeConfig::default(),
    )
    .expect("the test's participant")
}

/// zerodds-mqtt-bridge-1.0 §3, §4.2 with DDS-XTypes 1.3 §7.6.3.1: under
/// `data_representation: xcdr2` the daemon's DataWriter offers XCDR2 from
/// its creation, so a reader of another participant that accepts only XCDR2
/// matches it before the first PUBLISH; a PUBLISH under XCDR2 reaches that
/// reader, one under XCDR1 is refused and not written. A topic without the
/// key offers XCDR1 until a PUBLISH names another representation: such a
/// reader, discovered before it, does not match (the XCDR2 readers of
/// Cyclone DDS 0.10.5, Fast DDS 3.1 and RTI Connext DDS 7.7.0 matched no
/// writer of the daemon, measured with `tests/cross_vendor.rs`).
#[test]
fn mqttbridge_3_data_representation_xcdr2_matches_an_xcdr2_reader_before_the_first_publish() {
    use zerodds_dcps::runtime::{UserReaderConfig, UserSample};
    use zerodds_rtps::publication_data::data_representation as dr;
    let broker = MockBroker::start();
    let (mut cfg, lease) = make_test_config(&broker.bind_addr);
    cfg.topics[0].direction = "in".into();
    cfg.topics[0].reliability = "reliable".into();
    cfg.topics[0].data_representation = Some("xcdr2".into());
    cfg.topics.push(TopicConfig {
        dds_name: "Plain".to_string(),
        dds_type: "Trade".to_string(),
        mqtt_topic: "plain".to_string(),
        direction: "in".to_string(),
        reliability: "reliable".to_string(),
        ..TopicConfig::default()
    });
    let mut handle = server::start(cfg).expect("daemon start");
    let peer = foreign_participant(lease.id());
    let xcdr2_reader = |topic: &str| {
        peer.register_user_reader(UserReaderConfig {
            topic_name: topic.into(),
            type_name: "Trade".into(),
            reliable: true,
            data_representation_offer: Some(vec![dr::XCDR2]),
            ..Default::default()
        })
        .expect("the test's reader")
        .1
    };
    let rx = xcdr2_reader("Trade");
    let _plain = xcdr2_reader("Plain");
    let trade = handle.user_writers["Trade"];
    let plain = handle.user_writers["Plain"];
    let deadline = Instant::now() + Duration::from_secs(10);
    while handle
        .runtime
        .user_writer_matched_reader_guids(trade)
        .is_empty()
    {
        assert!(
            Instant::now() < deadline,
            "the XCDR2 writer matched no XCDR2 reader"
        );
        thread::sleep(Duration::from_millis(20));
    }
    assert!(
        handle
            .runtime
            .user_writer_matched_reader_guids(plain)
            .is_empty(),
        "a writer without the key offers XCDR1"
    );
    broker.inject_publish("trade", &[0x00, 0x01, 0x00, 0x00, 0x11]);
    broker.inject_publish("trade", &[0x00, 0x07, 0x00, 0x00, 0x22]);
    let mut got = Vec::new();
    let mut wait = Duration::from_secs(5);
    while let Ok(sample) = rx.recv_timeout(wait) {
        if let UserSample::Alive { payload, .. } = sample {
            got.push(payload.to_vec());
            wait = Duration::from_millis(800);
        }
    }
    assert_eq!(got, [vec![0x22]], "the XCDR1 PUBLISH is refused");
    handle.shutdown();
    broker.shutdown();
}

/// zerodds-mqtt-bridge-1.0 §3 with DDS-XTypes 1.3 §7.6.3.1.2: under
/// `data_representation: xcdr2` the DataReader of an `out` topic accepts
/// XCDR2 alone, so a DDS writer that offers only XCDR1 matches no reader of
/// the daemon, and one that offers XCDR2 does; the DataReader of a topic
/// without the key accepts both, and the XCDR1 writer matches it.
#[test]
fn mqttbridge_3_data_representation_xcdr2_reader_matches_no_xcdr1_writer() {
    use zerodds_dcps::runtime::UserWriterConfig;
    use zerodds_rtps::publication_data::data_representation as dr;
    let broker = MockBroker::start();
    let (mut cfg, lease) = make_test_config(&broker.bind_addr);
    cfg.topics[0].direction = "out".into();
    cfg.topics[0].reliability = "reliable".into();
    cfg.topics[0].data_representation = Some("xcdr2".into());
    cfg.topics.push(TopicConfig {
        dds_name: "Plain".to_string(),
        dds_type: "Trade".to_string(),
        mqtt_topic: "plain".to_string(),
        direction: "out".to_string(),
        reliability: "reliable".to_string(),
        ..TopicConfig::default()
    });
    let mut handle = server::start(cfg).expect("daemon start");
    let peer = foreign_participant(lease.id());
    let writer = |topic: &str, offer: i16| {
        peer.register_user_writer(UserWriterConfig {
            topic_name: topic.into(),
            type_name: "Trade".into(),
            reliable: true,
            data_representation_offer: Some(vec![offer]),
            ..Default::default()
        })
        .expect("the test's writer")
    };
    let xcdr2 = writer("Trade", dr::XCDR2);
    let xcdr1 = writer("Trade", dr::XCDR);
    let plain = writer("Plain", dr::XCDR);
    let matched = |w| !peer.user_writer_matched_reader_guids(w).is_empty();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !(matched(xcdr2) && matched(plain)) {
        assert!(
            Instant::now() < deadline,
            "the daemon's readers matched no writer of the test"
        );
        thread::sleep(Duration::from_millis(20));
    }
    // One discovery of each reader matched all of the test's writers.
    assert!(
        !matched(xcdr1),
        "a reader under xcdr2 matched a writer of XCDR1"
    );
    handle.shutdown();
    broker.shutdown();
}
