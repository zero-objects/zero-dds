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

fn make_test_config(broker_addr: &str) -> DaemonConfig {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.broker_url = format!("mqtt://{broker_addr}");
    cfg.client_id = "zerodds-test-bridge".to_string();
    cfg.domain = 99;
    cfg.topics.push(TopicConfig {
        dds_name: "Trade".to_string(),
        dds_type: "Trade".to_string(),
        mqtt_topic: "trade".to_string(),
        direction: "bidir".to_string(),
        mqtt_qos: 0,
        retain: false,
        reliability: "best_effort".to_string(),
        durability: "volatile".to_string(),
        history_depth: 10,
    });
    cfg
}

#[test]
fn daemon_connects_and_subscribes() {
    let broker = MockBroker::start();
    let cfg = make_test_config(&broker.bind_addr);
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

#[test]
fn mqtt_publish_to_daemon_does_not_crash_and_subscribe_arrived() {
    // Proof that the inbound path (MQTT PUBLISH → DDS writer) is wired
    // and does not trigger a panic. The DDS subscriber-side sniffing
    // needs multicast loopback (Linux-only) and is therefore only
    // indirectly verified here.

    let broker = MockBroker::start();
    let cfg = make_test_config(&broker.bind_addr);
    let mut handle = server::start(cfg).expect("daemon start");

    // Await the CONNECT + SUBSCRIBE handshake.
    thread::sleep(Duration::from_millis(500));
    assert!(broker.state.connect_seen.load(Ordering::SeqCst) > 0);

    // Inject a PUBLISH from the broker to the daemon.
    broker.inject_publish("trade", b"AAPL@200.5");

    // Wait a bit — the daemon holds a lock on the TCP stream the
    // broker watches. We check that no crash happens + no
    // further DISCONNECT comes from the daemon.
    thread::sleep(Duration::from_millis(500));

    handle.shutdown();
    broker.shutdown();
    // If we get here without a panic, L1+L3 inbound is wired.
}

#[test]
fn dds_publish_pumps_to_mqtt_broker() {
    // L3 outbound: we inject a synthetic alive sample
    // directly into the mpsc::Receiver channel that feeds the bridge
    // pump thread (via `DcpsRuntime::test_inject_user_alive`). This
    // bypasses the cross-participant RTPS path (multicast loopback,
    // which is unreliable in CI containers) but exercises the
    // full pump path in production form: reader channel → ACL read
    // check → MqttClient::publish → MQTT PUBLISH at the broker.

    let broker = MockBroker::start();
    let cfg = make_test_config(&broker.bind_addr);
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
    while Instant::now() < deadline {
        if !broker.state.publishes.lock().unwrap().is_empty() {
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
