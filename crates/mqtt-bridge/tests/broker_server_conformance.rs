// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The broker server against the server statements of MQTT 5.0 chapters 3
//! and 4, packet by packet over loopback TCP: a raw client writes the
//! octets of each packet and reads the server's answer.

#![cfg(feature = "std")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{self, ErrorKind, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use zerodds_mqtt_bridge::codec::{PublishPacket, decode_publish_v, encode_publish_v};
use zerodds_mqtt_bridge::control_packets::{
    ConnectBody, SubscribeBody, Subscription, UnsubscribeBody, connect_flags,
    decode_connack_body_v, decode_suback_body_v, encode_connect_body_v, encode_subscribe_body_v,
    encode_unsubscribe_body_v,
};
use zerodds_mqtt_bridge::net::{byte0, frame_packet, read_packet};
use zerodds_mqtt_bridge::packet::ControlPacketType as T;
use zerodds_mqtt_bridge::property_set::{PropertyContext, PropertySet};
use zerodds_mqtt_bridge::server::{
    AuthExchange, AuthStep, Authenticator, ClientStream, ClientWriter, MqttBrokerServer, Redirect,
    ServerConfig, ServerHandle,
};
use zerodds_mqtt_bridge::version::ProtocolVersion::{self, V5, V311};
use zerodds_mqtt_bridge::{QoS, net::MAX_MQTT_PACKET_SIZE};

const TO: Duration = Duration::from_secs(3);
const QUIET: Duration = Duration::from_millis(300);

fn start(cfg: ServerConfig) -> (ServerHandle, String) {
    let h = MqttBrokerServer::bind_with("127.0.0.1:0", cfg)
        .unwrap()
        .spawn()
        .unwrap();
    let addr = h.local_addr().to_string();
    (h, addr)
}

/// A CONNECT to build.
#[derive(Clone)]
struct Conn {
    v: ProtocolVersion,
    id: String,
    clean: bool,
    keep_alive: u16,
    props: PropertySet,
    will: Option<(String, Vec<u8>, u8, bool, PropertySet)>,
    user: Option<String>,
    pass: Option<Vec<u8>>,
}

fn conn(id: &str) -> Conn {
    Conn {
        v: V5,
        id: id.into(),
        clean: true,
        keep_alive: 0,
        props: PropertySet::default(),
        will: None,
        user: None,
        pass: None,
    }
}

impl Conn {
    fn bytes(&self) -> Vec<u8> {
        let mut flags = 0;
        if self.clean {
            flags |= connect_flags::CLEAN_START;
        }
        if let Some((_, _, q, r, _)) = &self.will {
            flags |= connect_flags::WILL | (q << 3);
            if *r {
                flags |= connect_flags::WILL_RETAIN;
            }
        }
        if self.user.is_some() {
            flags |= connect_flags::USER_NAME;
        }
        if self.pass.is_some() {
            flags |= connect_flags::PASSWORD;
        }
        let body = ConnectBody {
            protocol_name: "MQTT".into(),
            protocol_version: self.v.level(),
            connect_flags: flags,
            keep_alive: self.keep_alive,
            properties: self.props.encode().unwrap(),
            client_id: self.id.clone(),
            will_properties: self
                .will
                .as_ref()
                .map(|w| w.4.encode().unwrap())
                .unwrap_or_default(),
            will_topic: self.will.as_ref().map(|w| w.0.clone()),
            will_payload: self.will.as_ref().map(|w| w.1.clone()).unwrap_or_default(),
            user_name: self.user.clone(),
            password: self.pass.clone().unwrap_or_default(),
        };
        frame_packet(
            byte0(T::Connect, 0),
            &encode_connect_body_v(&body, self.v).unwrap(),
        )
        .unwrap()
    }

    /// Connects and returns the client and its CONNACK.
    fn open(&self, addr: &str) -> (Raw, Connack) {
        let mut raw = Raw::tcp(addr, self.v);
        raw.write(&self.bytes());
        let ack = raw.connack();
        (raw, ack)
    }

    /// Connects and expects Reason Code 0.
    fn ok(&self, addr: &str) -> Raw {
        let (raw, ack) = self.open(addr);
        assert_eq!(ack.code, 0, "CONNACK of {}", self.id);
        raw
    }
}

#[derive(Debug)]
struct Connack {
    session_present: bool,
    code: u8,
    props: PropertySet,
}

/// A received PUBLISH.
#[derive(Debug)]
struct Pub {
    p: PublishPacket,
    props: PropertySet,
}

struct Raw {
    s: TcpStream,
    v: ProtocolVersion,
}

impl Raw {
    fn tcp(addr: &str, v: ProtocolVersion) -> Self {
        let s = TcpStream::connect(addr).unwrap();
        s.set_read_timeout(Some(TO)).unwrap();
        s.set_nodelay(true).unwrap();
        Self { s, v }
    }

    fn write(&mut self, bytes: &[u8]) {
        self.s.write_all(bytes).unwrap();
    }

    fn send(&mut self, kind: T, flags: u8, body: &[u8]) {
        let b = frame_packet(byte0(kind, flags), body).unwrap();
        self.write(&b);
    }

    fn read(&mut self) -> (u8, Vec<u8>) {
        self.s.set_read_timeout(Some(TO)).unwrap();
        read_packet(&mut self.s).expect("a packet from the server")
    }

    fn read_kind(&mut self, kind: T) -> Vec<u8> {
        let (b0, body) = self.read();
        assert_eq!(
            b0 >> 4,
            kind.to_bits(),
            "expected {kind:?}, got type {}",
            b0 >> 4
        );
        body
    }

    fn connack(&mut self) -> Connack {
        let body = self.read_kind(T::ConnAck);
        let c = decode_connack_body_v(&body, self.v).unwrap();
        Connack {
            session_present: c.session_present,
            code: c.reason_code,
            props: PropertySet::parse(&c.properties, PropertyContext::Packet(T::ConnAck)).unwrap(),
        }
    }

    /// Nothing arrives within `QUIET`.
    fn quiet(&mut self) {
        self.s.set_read_timeout(Some(QUIET)).unwrap();
        match read_packet(&mut self.s) {
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            other => panic!("expected silence, got {other:?}"),
        }
    }

    /// The server closed the Network Connection.
    fn closed(&mut self) {
        self.s.set_read_timeout(Some(TO)).unwrap();
        match read_packet(&mut self.s) {
            Err(e) => assert!(
                !matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut),
                "connection still open: {e}"
            ),
            Ok(p) => panic!("expected the close, got {p:?}"),
        }
    }

    /// A DISCONNECT with `code`, then the close.
    fn disconnected(&mut self, code: u8) -> PropertySet {
        let body = self.read_kind(T::Disconnect);
        assert_eq!(
            body.first().copied().unwrap_or(0),
            code,
            "DISCONNECT reason"
        );
        let props = if body.len() > 1 {
            PropertySet::parse(&body[2..], PropertyContext::Packet(T::Disconnect)).unwrap()
        } else {
            PropertySet::default()
        };
        self.closed();
        props
    }

    fn publish(
        &mut self,
        topic: &str,
        payload: &[u8],
        qos: u8,
        retain: bool,
        pid: u16,
        props: &PropertySet,
    ) {
        let p = PublishPacket {
            dup: false,
            qos,
            retain,
            topic: topic.into(),
            packet_id: (qos > 0).then_some(pid),
            properties: props.encode().unwrap(),
            payload: payload.to_vec(),
        };
        let b = encode_publish_v(&p, self.v).unwrap();
        self.write(&b);
    }

    /// PUBLISH QoS 1 and its PUBACK reason code.
    fn publish1(&mut self, topic: &str, payload: &[u8], pid: u16, props: &PropertySet) -> u8 {
        self.publish(topic, payload, 1, false, pid, props);
        let body = self.read_kind(T::PubAck);
        assert_eq!(u16::from_be_bytes([body[0], body[1]]), pid);
        body.get(2).copied().unwrap_or(0)
    }

    fn recv(&mut self) -> Pub {
        let (b0, body) = self.read();
        assert_eq!(
            b0 >> 4,
            T::Publish.to_bits(),
            "expected PUBLISH, got type {}",
            b0 >> 4
        );
        let full = frame_packet(b0, &body).unwrap();
        let (_, p) = decode_publish_v(&full, self.v).unwrap();
        let props = PropertySet::parse(&p.properties, PropertyContext::Packet(T::Publish)).unwrap();
        Pub { p, props }
    }

    fn ack(&mut self, kind: T, pid: u16, code: u8) {
        let flags = if kind == T::PubRel { 0b0010 } else { 0 };
        let [hi, lo] = pid.to_be_bytes();
        let body = if self.v == V5 {
            vec![hi, lo, code, 0]
        } else {
            vec![hi, lo]
        };
        self.send(kind, flags, &body);
    }

    /// SUBSCRIBE and the SUBACK's Reason Codes and properties.
    fn subscribe_with(
        &mut self,
        pid: u16,
        entries: &[(&str, u8)],
        props: &PropertySet,
    ) -> (Vec<u8>, PropertySet) {
        let body = SubscribeBody {
            packet_id: pid,
            properties: props.encode().unwrap(),
            subscriptions: entries
                .iter()
                .map(|(f, o)| Subscription {
                    topic_filter: (*f).into(),
                    options: *o,
                })
                .collect(),
        };
        self.send(
            T::Subscribe,
            0b0010,
            &encode_subscribe_body_v(&body, self.v).unwrap(),
        );
        let sb = self.read_kind(T::SubAck);
        let s = decode_suback_body_v(&sb, self.v).unwrap();
        assert_eq!(s.packet_id, pid, "[MQTT-3.8.4-2]");
        let props = PropertySet::parse(&s.properties, PropertyContext::Packet(T::SubAck)).unwrap();
        (s.reason_codes, props)
    }

    fn subscribe(&mut self, filter: &str, options: u8) -> u8 {
        self.subscribe_with(1, &[(filter, options)], &PropertySet::default())
            .0[0]
    }

    fn disconnect(mut self, code: u8, props: &PropertySet) {
        let mut body = vec![code];
        let p = props.encode().unwrap();
        body.push(u8::try_from(p.len()).unwrap());
        body.extend_from_slice(&p);
        self.send(T::Disconnect, 0, &body);
        self.closed();
    }
}

fn expiry(secs: u32) -> PropertySet {
    PropertySet {
        session_expiry_interval: Some(secs),
        ..PropertySet::default()
    }
}

fn with_props(mut c: Conn, props: PropertySet) -> Conn {
    c.props = props;
    c
}

fn persistent(id: &str, secs: u32) -> Conn {
    let mut c = conn(id);
    c.clean = false;
    c.props = expiry(secs);
    c
}

fn will(
    topic: &str,
    payload: &[u8],
    props: PropertySet,
) -> Option<(String, Vec<u8>, u8, bool, PropertySet)> {
    Some((topic.into(), payload.to_vec(), 0, false, props))
}

// ---------------------------------------------------------------- §3.1 / §3.2

#[test]
fn mqtt5_3_2_2_connack_carries_the_server_limits_and_session_present_follows_clean_start() {
    let cfg = ServerConfig {
        receive_maximum: 10,
        topic_alias_maximum: 5,
        ..ServerConfig::default()
    };
    let (h, addr) = start(cfg);
    let (raw, ack) = persistent("sp", 60).open(&addr);
    assert_eq!(
        (ack.code, ack.session_present),
        (0, false),
        "[MQTT-3.1.2-6]"
    );
    assert_eq!(ack.props.receive_maximum, Some(10));
    assert_eq!(ack.props.topic_alias_maximum, Some(5));
    assert_eq!(ack.props.maximum_packet_size, Some(MAX_MQTT_PACKET_SIZE));
    assert_eq!(ack.props.assigned_client_identifier, None);
    raw.disconnect(0, &PropertySet::default());
    let (raw, ack) = persistent("sp", 60).open(&addr);
    assert!(ack.session_present, "[MQTT-3.1.2-5], [MQTT-3.2.2-3]");
    raw.disconnect(0, &PropertySet::default());
    let mut clean = persistent("sp", 60);
    clean.clean = true;
    let (_raw, ack) = clean.open(&addr);
    assert!(!ack.session_present, "[MQTT-3.1.2-4], [MQTT-3.2.2-2]");
    h.shutdown();
}

#[test]
fn mqtt5_3_1_3_1_an_empty_client_id_gets_an_assigned_client_identifier() {
    let (h, addr) = start(ServerConfig::default());
    let (_a, ack_a) = conn("").open(&addr);
    let (_b, ack_b) = conn("").open(&addr);
    let a = ack_a
        .props
        .assigned_client_identifier
        .expect("[MQTT-3.1.3-7]");
    let b = ack_b
        .props
        .assigned_client_identifier
        .expect("[MQTT-3.1.3-7]");
    assert!(!a.is_empty());
    assert_ne!(a, b, "[MQTT-3.2.2-16]");
    // 3.1.1: an empty ClientID needs Clean Session 1 (3.1.1 [MQTT-3.1.3-8]).
    let mut old = conn("");
    old.v = V311;
    old.clean = false;
    let (mut raw, ack) = old.open(&addr);
    assert_eq!(ack.code, 0x02);
    raw.closed();
    old.clean = true;
    assert_eq!(old.open(&addr).1.code, 0x00);
    h.shutdown();
}

#[test]
fn mqtt5_3_1_4_3_a_second_connection_takes_the_session_over_with_0x8e() {
    let (h, addr) = start(ServerConfig::default());
    let mut watcher = conn("watch").ok(&addr);
    watcher.subscribe("lwt", 0);
    let mut first = conn("same");
    first.will = will("lwt", b"gone", PropertySet::default());
    let mut old = first.ok(&addr);
    let mut new = conn("same").ok(&addr);
    old.disconnected(0x8E);
    // The old connection's Will (Will Delay 0) is published.
    assert_eq!(watcher.recv().p.payload, b"gone");
    new.subscribe("t", 0);
    watcher.publish("t", b"x", 0, false, 0, &PropertySet::default());
    assert_eq!(new.recv().p.payload, b"x");
    assert_eq!(h.connected_clients(), ["same", "watch"]);
    h.shutdown();
}

/// A TCP connection served through [`ClientStream`] whose close ends only
/// the server's direction: the server still reads what the client sends
/// after the close, as it reads a packet its reader buffered before it.
/// The drop of the stream at the end of the connection's thread is noted.
struct ReadsOn {
    s: TcpStream,
    dropped: Arc<AtomicBool>,
}

impl Read for ReadsOn {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.s.read(buf)
    }
}

impl Drop for ReadsOn {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

struct WriteSide(TcpStream);

impl Write for WriteSide {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl ClientWriter for WriteSide {
    fn close(&mut self) {
        let _ = self.0.shutdown(Shutdown::Write);
    }
}

impl ClientStream for ReadsOn {
    fn writer(&self) -> io::Result<Box<dyn ClientWriter>> {
        Ok(Box::new(WriteSide(self.s.try_clone()?)))
    }

    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.s.set_read_timeout(timeout)
    }
}

/// [MQTT-3.1.4-3]: what the connection a takeover replaced still reads
/// (a SUBSCRIBE, then a DISCONNECT with Session Expiry Interval 0, both
/// sent after its DISCONNECT 0x8E) does not reach the session, which is
/// the new connection's: it gets no subscription of the old connection
/// and keeps the Session Expiry Interval of the new CONNECT.
#[test]
fn mqtt5_3_1_4_3_what_the_replaced_connection_still_reads_does_not_reach_the_session() {
    let (h, addr) = start(ServerConfig::default());
    let mut watcher = conn("watch").ok(&addr);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut old = Raw::tcp(&listener.local_addr().unwrap().to_string(), V5);
    let (stream, _) = listener.accept().unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    h.serve(ReadsOn {
        s: stream,
        dropped: Arc::clone(&dropped),
    });
    old.write(&persistent("same", 600).bytes());
    assert_eq!(old.connack().code, 0);
    let (mut new, ack) = persistent("same", 600).open(&addr);
    assert_eq!((ack.code, ack.session_present), (0, true));
    old.disconnected(0x8E);
    let subscribe = SubscribeBody {
        packet_id: 1,
        properties: Vec::new(),
        subscriptions: vec![Subscription {
            topic_filter: "old/t".into(),
            options: 0,
        }],
    };
    old.send(
        T::Subscribe,
        0b0010,
        &encode_subscribe_body_v(&subscribe, V5).unwrap(),
    );
    let props = expiry(0).encode().unwrap();
    let mut body = vec![0x00, u8::try_from(props.len()).unwrap()];
    body.extend_from_slice(&props);
    // The SUBSCRIBE may have ended the old connection already.
    let _ = old
        .s
        .write_all(&frame_packet(byte0(T::Disconnect, 0), &body).unwrap());
    let t0 = Instant::now();
    while !dropped.load(Ordering::SeqCst) {
        assert!(t0.elapsed() < TO, "the old connection's thread did not end");
        std::thread::sleep(Duration::from_millis(10));
    }
    watcher.publish("old/t", b"x", 0, false, 0, &PropertySet::default());
    new.quiet();
    new.disconnect(0, &PropertySet::default());
    assert!(
        h.has_session("same"),
        "the session keeps the Session Expiry Interval 600 of the new CONNECT"
    );
    h.shutdown();
}

#[test]
fn mqtt5_3_1_2_10_keep_alive_closes_after_one_and_a_half_periods_and_publishes_the_will() {
    let (h, addr) = start(ServerConfig::default());
    let mut watcher = conn("watch").ok(&addr);
    watcher.subscribe("lwt", 0);
    let mut c = conn("idle");
    c.keep_alive = 1;
    c.will = will("lwt", b"timeout", PropertySet::default());
    let mut idle = c.ok(&addr);
    let t0 = Instant::now();
    idle.closed();
    let took = t0.elapsed();
    assert!(took >= Duration::from_millis(1400), "closed after {took:?}");
    assert!(took <= Duration::from_millis(2500), "closed after {took:?}");
    assert_eq!(
        watcher.recv().p.payload,
        b"timeout",
        "[MQTT-3.1.2-22] as if the network failed"
    );
    h.shutdown();
}

#[test]
fn mqtt5_3_2_2_3_14_the_server_keep_alive_replaces_the_clients() {
    let cfg = ServerConfig {
        server_keep_alive: Some(1),
        ..ServerConfig::default()
    };
    let (h, addr) = start(cfg);
    let (mut raw, ack) = conn("ska").open(&addr);
    assert_eq!(ack.props.server_keep_alive, Some(1));
    let t0 = Instant::now();
    raw.closed();
    assert!(
        t0.elapsed() >= Duration::from_millis(1400),
        "[MQTT-3.2.2-21]"
    );
    h.shutdown();
}

#[test]
fn mqtt5_3_2_2_22_a_3_1_1_client_keeps_its_own_keep_alive_beside_a_server_keep_alive() {
    let cfg = ServerConfig {
        server_keep_alive: Some(1),
        ..ServerConfig::default()
    };
    let (h, addr) = start(cfg);
    // A 3.1.1 CONNACK carries no Server Keep Alive: the client's Keep
    // Alive 0 holds ([MQTT-3.2.2-22]; 3.1.1 [MQTT-3.1.2-24]).
    let mut old = conn("ka311");
    old.v = V311;
    let mut raw = old.ok(&addr);
    std::thread::sleep(Duration::from_millis(2000));
    raw.send(T::PingReq, 0, &[]);
    raw.read_kind(T::PingResp);
    h.shutdown();
}

#[test]
fn mqtt5_3_1_0_2_a_second_connect_is_a_protocol_error() {
    let (h, addr) = start(ServerConfig::default());
    let c = conn("twice");
    let mut raw = c.ok(&addr);
    raw.write(&c.bytes());
    raw.disconnected(0x82);
    h.shutdown();
}

#[test]
fn mqtt5_3_1_2_2_an_unsupported_protocol_version_gets_0x84_and_the_close() {
    let (h, addr) = start(ServerConfig::default());
    // Level 6, "MQTT", flags 0x02, keep alive 0, properties 0, id "x".
    let body = [0, 4, b'M', b'Q', b'T', b'T', 6, 0x02, 0, 0, 0, 0, 1, b'x'];
    let mut raw = Raw::tcp(&addr, V5);
    raw.send(T::Connect, 0, &body);
    let ack = raw.connack();
    assert_eq!(
        (ack.code, ack.session_present),
        (0x84, false),
        "[MQTT-3.2.2-6]"
    );
    raw.closed();
    // MQTT 3.1: the 3.1.1 Return Code 0x01.
    let old = [
        0, 6, b'M', b'Q', b'I', b's', b'd', b'p', 3, 0x02, 0, 0, 0, 1, b'x',
    ];
    let mut raw = Raw::tcp(&addr, V311);
    raw.send(T::Connect, 0, &old);
    assert_eq!(raw.connack().code, 0x01);
    raw.closed();
    h.shutdown();
}

#[test]
fn mqtt5_3_1_2_3_the_reserved_connect_flag_gets_0x81() {
    let (h, addr) = start(ServerConfig::default());
    let body = [0, 4, b'M', b'Q', b'T', b'T', 5, 0x03, 0, 0, 0, 0, 1, b'x'];
    let mut raw = Raw::tcp(&addr, V5);
    raw.send(T::Connect, 0, &body);
    assert_eq!(raw.connack().code, 0x81);
    raw.closed();
    // [MQTT-3.1.0-1]: another packet first is closed without an answer.
    let mut raw = Raw::tcp(&addr, V5);
    raw.send(T::PingReq, 0, &[]);
    raw.closed();
    h.shutdown();
}

#[test]
fn mqtt5_3_2_0_1_session_present_and_clean_session_of_3_1_1_clients() {
    let (h, addr) = start(ServerConfig::default());
    let mut c = conn("old");
    c.v = V311;
    c.clean = false;
    let (raw, ack) = c.open(&addr);
    assert!(!ack.session_present);
    drop(raw);
    std::thread::sleep(Duration::from_millis(100));
    let (_raw, ack) = c.open(&addr);
    assert!(
        ack.session_present,
        "3.1.1 Clean Session 0 keeps the session"
    );
    h.shutdown();
}

// ---------------------------------------------------------------- §4.13

#[test]
fn mqtt5_4_13_1_a_malformed_packet_gets_0x81_the_will_and_the_end_of_the_session() {
    let (h, addr) = start(ServerConfig::default());
    let mut watcher = conn("watch").ok(&addr);
    watcher.subscribe("lwt", 0);
    let mut c = conn("bad");
    c.will = will("lwt", b"broken", PropertySet::default());
    let mut raw = c.ok(&addr);
    // A PUBLISH with both QoS bits set ([MQTT-3.3.1-4]).
    raw.send(T::Publish, 0b0110, &[0, 1, b't', 0, 1, 0]);
    raw.disconnected(0x81);
    assert_eq!(watcher.recv().p.payload, b"broken", "[MQTT-3.1.2-8]");
    assert!(!h.has_session("bad"), "Session Expiry Interval 0");
    h.shutdown();
}

#[test]
fn mqtt5_3_1_2_11_4_a_packet_above_the_servers_maximum_packet_size_gets_0x95() {
    let cfg = ServerConfig {
        maximum_packet_size: 128,
        ..ServerConfig::default()
    };
    let (h, addr) = start(cfg);
    let (mut raw, ack) = conn("big").open(&addr);
    assert_eq!(ack.props.maximum_packet_size, Some(128));
    raw.publish("t", &[0u8; 200], 0, false, 0, &PropertySet::default());
    raw.disconnected(0x95);
    h.shutdown();
}

#[test]
fn mqtt5_3_3_2_2_a_topic_name_with_a_wildcard_gets_0x90() {
    let (h, addr) = start(ServerConfig::default());
    let mut raw = conn("wild").ok(&addr);
    raw.publish("a/+", b"x", 0, false, 0, &PropertySet::default());
    raw.disconnected(0x90);
    h.shutdown();
}

#[test]
fn mqtt5_3_3_4_6_a_subscription_identifier_from_the_client_is_a_protocol_error() {
    let (h, addr) = start(ServerConfig::default());
    let mut raw = conn("sid").ok(&addr);
    let props = PropertySet {
        subscription_identifiers: vec![4],
        ..PropertySet::default()
    };
    raw.publish("t", b"x", 0, false, 0, &props);
    raw.disconnected(0x82);
    h.shutdown();
}

#[test]
fn mqtt5_3_8_3_5_reserved_subscription_option_bits_and_a_shared_no_local() {
    let (h, addr) = start(ServerConfig::default());
    let mut raw = conn("opt").ok(&addr);
    raw.send(T::Subscribe, 0b0010, &[0, 1, 0, 0, 1, b't', 0x40]);
    raw.disconnected(0x81);
    let mut raw = conn("nl").ok(&addr);
    // No Local on a Shared Subscription ([MQTT-3.8.3-4]).
    raw.send(
        T::Subscribe,
        0b0010,
        &[
            0, 1, 0, 0, 10, b'$', b's', b'h', b'a', b'r', b'e', b'/', b'g', b'/', b't', 0x04,
        ],
    );
    raw.disconnected(0x82);
    // SUBSCRIBE with flags 0 ([MQTT-3.8.1-1]).
    let mut raw = conn("fl").ok(&addr);
    raw.send(T::Subscribe, 0, &[0, 1, 0, 0, 1, b't', 0]);
    raw.disconnected(0x81);
    h.shutdown();
}

#[test]
fn mqtt5_3_6_1_a_pubrel_with_flags_0_is_malformed() {
    let (h, addr) = start(ServerConfig::default());
    let mut raw = conn("rel").ok(&addr);
    raw.send(T::PubRel, 0, &[0, 1]);
    raw.disconnected(0x81);
    h.shutdown();
}

// ---------------------------------------------------------------- §3.3 PUBLISH

#[test]
fn mqtt5_3_3_2_3_the_properties_of_a_message_reach_the_subscriber_unaltered() {
    let (h, addr) = start(ServerConfig::default());
    let mut sub = conn("sub").ok(&addr);
    sub.subscribe("p/t", 1);
    let mut publ = conn("pub").ok(&addr);
    let props = PropertySet {
        payload_format_indicator: Some(1),
        message_expiry_interval: Some(60),
        content_type: Some("text/plain".into()),
        response_topic: Some("reply/here".into()),
        correlation_data: Some(vec![9, 8, 7]),
        user_properties: vec![
            ("z".into(), "1".into()),
            ("a".into(), "2".into()),
            ("z".into(), "3".into()),
        ],
        ..PropertySet::default()
    };
    assert_eq!(publ.publish1("p/t", b"hello", 1, &props), 0);
    let got = sub.recv();
    assert_eq!(got.p.payload, b"hello");
    assert_eq!(
        got.props.payload_format_indicator,
        Some(1),
        "[MQTT-3.3.2-4]"
    );
    assert_eq!(
        got.props.content_type.as_deref(),
        Some("text/plain"),
        "[MQTT-3.3.2-20]"
    );
    assert_eq!(
        got.props.response_topic.as_deref(),
        Some("reply/here"),
        "[MQTT-3.3.2-15]"
    );
    assert_eq!(
        got.props.correlation_data.as_deref(),
        Some(&[9u8, 8, 7][..]),
        "[MQTT-3.3.2-16]"
    );
    assert_eq!(
        got.props.user_properties, props.user_properties,
        "[MQTT-3.3.2-17], [MQTT-3.3.2-18]"
    );
    let left = got.props.message_expiry_interval.unwrap();
    assert!((58..=60).contains(&left), "[MQTT-3.3.2-6]: {left}");
    h.shutdown();
}

#[test]
fn mqtt5_3_3_2_3_3_an_expired_message_is_deleted_and_the_rest_carries_what_is_left() {
    let (h, addr) = start(ServerConfig::default());
    let mut sub = persistent("late", 60).ok(&addr);
    sub.subscribe("e", 1);
    sub.disconnect(0, &PropertySet::default());
    let mut publ = conn("pub").ok(&addr);
    let exp = |s| PropertySet {
        message_expiry_interval: Some(s),
        ..PropertySet::default()
    };
    publ.publish1("e", b"short", 1, &exp(1));
    publ.publish1("e", b"long", 2, &exp(30));
    std::thread::sleep(Duration::from_millis(1300));
    let (mut sub, ack) = persistent("late", 60).open(&addr);
    assert!(ack.session_present);
    let got = sub.recv();
    assert_eq!(got.p.payload, b"long", "[MQTT-3.3.2-5]");
    let left = got.props.message_expiry_interval.unwrap();
    assert!((27..=29).contains(&left), "[MQTT-3.3.2-6]: {left}");
    sub.ack(T::PubAck, got.p.packet_id.unwrap(), 0);
    sub.quiet();
    h.shutdown();
}

#[test]
fn mqtt5_3_3_1_3_retain_handling_and_retain_as_published() {
    let (h, addr) = start(ServerConfig::default());
    let mut publ = conn("pub").ok(&addr);
    publ.publish("r/t", b"kept", 0, true, 0, &PropertySet::default());
    std::thread::sleep(Duration::from_millis(100));
    let mut sub = conn("sub").ok(&addr);
    // Retain Handling 2: no retained message ([MQTT-3.3.1-11]).
    assert_eq!(sub.subscribe("r/#", 0x20), 0);
    sub.quiet();
    // Retain Handling 0 on the same filter: sent ([MQTT-3.3.1-9], [MQTT-3.8.4-4]).
    assert_eq!(sub.subscribe("r/#", 0x00), 0);
    let got = sub.recv();
    assert!(got.p.retain);
    assert_eq!(got.p.payload, b"kept");
    // Retain Handling 1, the subscription exists: not sent ([MQTT-3.3.1-10]).
    assert_eq!(sub.subscribe("r/#", 0x10), 0);
    sub.quiet();
    // Retain As Published 0 forwards RETAIN 0 ([MQTT-3.3.1-12]) ...
    publ.publish("r/t", b"live", 0, true, 0, &PropertySet::default());
    assert!(!sub.recv().p.retain);
    // ... Retain As Published 1 keeps it ([MQTT-3.3.1-13]).
    assert_eq!(sub.subscribe("r/#", 0x08 | 0x20), 0);
    publ.publish("r/t", b"live2", 0, true, 0, &PropertySet::default());
    assert!(sub.recv().p.retain);
    // An empty retained payload removes it ([MQTT-3.3.1-6]).
    publ.publish("r/t", b"", 0, true, 0, &PropertySet::default());
    sub.recv();
    let mut fresh = conn("fresh").ok(&addr);
    fresh.subscribe("r/#", 0);
    fresh.quiet();
    h.shutdown();
}

#[test]
fn mqtt5_3_8_3_3_no_local_keeps_a_message_from_its_publisher() {
    let (h, addr) = start(ServerConfig::default());
    let mut me = conn("me").ok(&addr);
    me.subscribe("nl", 0x04);
    me.publish("nl", b"mine", 0, false, 0, &PropertySet::default());
    me.quiet();
    let mut other = conn("other").ok(&addr);
    other.publish("nl", b"theirs", 0, false, 0, &PropertySet::default());
    assert_eq!(me.recv().p.payload, b"theirs");
    h.shutdown();
}

#[test]
fn mqtt5_3_3_4_overlapping_subscriptions_give_one_copy_with_every_identifier() {
    let (h, addr) = start(ServerConfig::default());
    let mut sub = conn("ov").ok(&addr);
    let id = |n| PropertySet {
        subscription_identifiers: vec![n],
        ..PropertySet::default()
    };
    assert_eq!(sub.subscribe_with(1, &[("a/+", 1)], &id(1)).0, [1]);
    assert_eq!(sub.subscribe_with(2, &[("a/#", 2)], &id(2)).0, [2]);
    let mut publ = conn("pub").ok(&addr);
    publ.publish("a/b", b"x", 2, false, 1, &PropertySet::default());
    let got = sub.recv();
    assert_eq!(got.p.qos, 2, "[MQTT-3.3.4-2]");
    let mut ids = got.props.subscription_identifiers.clone();
    ids.sort_unstable();
    assert_eq!(ids, [1, 2], "[MQTT-3.3.4-3], [MQTT-3.3.4-4]");
    sub.ack(T::PubRec, got.p.packet_id.unwrap(), 0);
    sub.read_kind(T::PubRel);
    sub.quiet();
    h.shutdown();
}

#[test]
fn mqtt5_4_8_2_a_shared_subscription_alternates_and_sends_no_retained_message() {
    let (h, addr) = start(ServerConfig::default());
    let mut publ = conn("pub").ok(&addr);
    publ.publish("s/r", b"old", 0, true, 0, &PropertySet::default());
    let mut a = conn("a").ok(&addr);
    let mut b = conn("b").ok(&addr);
    assert_eq!(a.subscribe("$share/g/s/+", 1), 1);
    assert_eq!(b.subscribe("$share/g/s/+", 1), 1);
    a.quiet();
    b.quiet();
    for i in 0..4u8 {
        publ.publish("s/x", &[i], 0, false, 0, &PropertySet::default());
    }
    let mut got = Vec::new();
    for _ in 0..2 {
        got.push(a.recv().p.payload[0]);
        got.push(b.recv().p.payload[0]);
    }
    got.sort_unstable();
    assert_eq!(got, [0, 1, 2, 3]);
    a.quiet();
    b.quiet();
    h.shutdown();
}

#[test]
fn mqtt5_3_3_2_3_2_a_payload_that_is_not_utf8_gets_0x99() {
    let (h, addr) = start(ServerConfig::default());
    let mut sub = conn("sub").ok(&addr);
    sub.subscribe("u", 1);
    let mut publ = conn("pub").ok(&addr);
    let utf8 = PropertySet {
        payload_format_indicator: Some(1),
        ..PropertySet::default()
    };
    assert_eq!(publ.publish1("u", &[0xC3, 0x28], 1, &utf8), 0x99);
    sub.quiet();
    publ.publish("u", &[0xFF], 0, false, 0, &utf8);
    publ.disconnected(0x99);
    h.shutdown();
}

#[test]
fn mqtt5_3_4_2_1_a_puback_says_0x10_without_matching_subscribers() {
    let (h, addr) = start(ServerConfig::default());
    let mut publ = conn("pub").ok(&addr);
    assert_eq!(
        publ.publish1("nobody", b"x", 1, &PropertySet::default()),
        0x10
    );
    let mut sub = conn("sub").ok(&addr);
    sub.subscribe("nobody", 0);
    assert_eq!(
        publ.publish1("nobody", b"x", 2, &PropertySet::default()),
        0x00
    );
    h.shutdown();
}

#[test]
fn mqtt5_4_3_3_a_duplicate_qos2_publish_is_delivered_once() {
    let (h, addr) = start(ServerConfig::default());
    let mut sub = conn("sub").ok(&addr);
    sub.subscribe("q2", 0);
    let mut publ = conn("pub").ok(&addr);
    publ.publish("q2", b"once", 2, false, 5, &PropertySet::default());
    publ.read_kind(T::PubRec);
    // The same message again with DUP before the PUBREL.
    let again = PublishPacket {
        dup: true,
        qos: 2,
        retain: false,
        topic: "q2".into(),
        packet_id: Some(5),
        properties: Vec::new(),
        payload: b"once".to_vec(),
    };
    publ.write(&encode_publish_v(&again, V5).unwrap());
    publ.read_kind(T::PubRec);
    publ.ack(T::PubRel, 5, 0);
    let comp = publ.read_kind(T::PubComp);
    assert_eq!(comp[2], 0x00, "[MQTT-4.3.3-11]");
    assert_eq!(sub.recv().p.payload, b"once");
    sub.quiet();
    // A PUBREL for an identifier not held: PUBCOMP 0x92.
    publ.ack(T::PubRel, 5, 0);
    assert_eq!(publ.read_kind(T::PubComp)[2], 0x92);
    h.shutdown();
}

// ---------------------------------------------------------------- §4.9 / §3.3.4

#[test]
fn mqtt5_4_9_the_server_sends_no_more_than_the_clients_receive_maximum() {
    let (h, addr) = start(ServerConfig::default());
    let mut sub = with_props(
        conn("rm"),
        PropertySet {
            receive_maximum: Some(1),
            ..PropertySet::default()
        },
    )
    .ok(&addr);
    sub.subscribe("q", 1);
    let mut publ = conn("pub").ok(&addr);
    publ.publish1("q", b"1", 1, &PropertySet::default());
    publ.publish1("q", b"2", 2, &PropertySet::default());
    let first = sub.recv();
    assert_eq!(first.p.payload, b"1");
    sub.quiet();
    // [MQTT-4.9.0-3]: other packets are answered meanwhile.
    sub.send(T::PingReq, 0, &[]);
    sub.read_kind(T::PingResp);
    sub.ack(T::PubAck, first.p.packet_id.unwrap(), 0);
    assert_eq!(sub.recv().p.payload, b"2", "[MQTT-3.3.4-9]");
    h.shutdown();
}

#[test]
fn mqtt5_3_3_4_a_client_over_the_servers_receive_maximum_gets_0x93() {
    let cfg = ServerConfig {
        receive_maximum: 1,
        ..ServerConfig::default()
    };
    let (h, addr) = start(cfg);
    let mut publ = conn("pub").ok(&addr);
    publ.publish("t", b"1", 2, false, 1, &PropertySet::default());
    publ.read_kind(T::PubRec);
    publ.publish("t", b"2", 2, false, 2, &PropertySet::default());
    publ.disconnected(0x93);
    h.shutdown();
}

#[test]
fn mqtt5_3_1_2_24_a_message_over_the_clients_maximum_packet_size_is_not_sent() {
    let (h, addr) = start(ServerConfig::default());
    let mut sub = with_props(
        conn("small"),
        PropertySet {
            maximum_packet_size: Some(100),
            receive_maximum: Some(1),
            ..PropertySet::default()
        },
    )
    .ok(&addr);
    sub.subscribe("m", 1);
    let mut publ = conn("pub").ok(&addr);
    publ.publish1("m", &[7u8; 500], 1, &PropertySet::default());
    publ.publish1("m", b"fits", 2, &PropertySet::default());
    // [MQTT-3.1.2-25]: the large one counts as sent, the quota of 1 is free.
    assert_eq!(sub.recv().p.payload, b"fits");
    h.shutdown();
}

// ---------------------------------------------------------------- Topic Alias

#[test]
fn mqtt5_3_3_2_3_4_topic_aliases_from_the_client() {
    let cfg = ServerConfig {
        topic_alias_maximum: 2,
        ..ServerConfig::default()
    };
    let (h, addr) = start(cfg);
    let mut sub = conn("sub").ok(&addr);
    sub.subscribe("x/y", 0);
    let mut publ = conn("pub").ok(&addr);
    let alias = |a| PropertySet {
        topic_alias: Some(a),
        ..PropertySet::default()
    };
    publ.publish("x/y", b"1", 0, false, 0, &alias(1));
    publ.publish("", b"2", 0, false, 0, &alias(1));
    assert_eq!(sub.recv().p.topic, "x/y");
    assert_eq!(sub.recv().p.topic, "x/y", "[MQTT-3.3.2-12]");
    publ.publish("x/y", b"3", 0, false, 0, &alias(3));
    publ.disconnected(0x94);
    // A Topic Alias of 0: 0x94 as well, not 0x82 (§3.3.2.3.4).
    let mut zero = conn("pub").ok(&addr);
    zero.publish("x/y", b"0", 0, false, 0, &alias(0));
    zero.disconnected(0x94);
    // An alias of another connection is unknown here ([MQTT-3.3.2-7]).
    let mut fresh = conn("pub").ok(&addr);
    fresh.publish("", b"4", 0, false, 0, &alias(1));
    fresh.disconnected(0x82);
    h.shutdown();
}

#[test]
fn mqtt5_3_1_2_26_topic_aliases_to_the_client_stay_within_its_maximum() {
    let (h, addr) = start(ServerConfig::default());
    let mut sub = with_props(
        conn("ta"),
        PropertySet {
            topic_alias_maximum: Some(1),
            ..PropertySet::default()
        },
    )
    .ok(&addr);
    sub.subscribe("t/#", 0);
    let mut plain = conn("plain").ok(&addr);
    plain.subscribe("t/#", 0);
    let mut publ = conn("pub").ok(&addr);
    for t in ["t/1", "t/1", "t/2"] {
        publ.publish(t, b"x", 0, false, 0, &PropertySet::default());
    }
    let a = sub.recv();
    assert_eq!((a.p.topic.as_str(), a.props.topic_alias), ("t/1", Some(1)));
    let b = sub.recv();
    assert_eq!((b.p.topic.as_str(), b.props.topic_alias), ("", Some(1)));
    let c = sub.recv();
    assert_eq!(
        (c.p.topic.as_str(), c.props.topic_alias),
        ("t/2", None),
        "[MQTT-3.1.2-26]"
    );
    for _ in 0..3 {
        assert_eq!(plain.recv().props.topic_alias, None, "[MQTT-3.1.2-27]");
    }
    h.shutdown();
}

// ---------------------------------------------------------------- §4.4 / §4.1

#[test]
fn mqtt5_4_4_unacknowledged_messages_are_sent_again_at_reconnect_with_dup() {
    let (h, addr) = start(ServerConfig::default());
    let mut sub = persistent("re", 60).ok(&addr);
    sub.subscribe("q1", 1);
    sub.subscribe("q2", 2);
    let mut publ = conn("pub").ok(&addr);
    publ.publish1("q1", b"one", 1, &PropertySet::default());
    publ.publish("q2", b"two", 2, false, 2, &PropertySet::default());
    publ.read_kind(T::PubRec);
    let one = sub.recv();
    assert!(!one.p.dup, "[MQTT-4.3.2-2]");
    let two = sub.recv();
    let pid2 = two.p.packet_id.unwrap();
    sub.ack(T::PubRec, pid2, 0);
    assert_eq!(
        &sub.read_kind(T::PubRel)[..2],
        &pid2.to_be_bytes(),
        "[MQTT-4.3.3-4]"
    );
    drop(sub);
    std::thread::sleep(Duration::from_millis(100));
    let (mut sub, ack) = persistent("re", 60).open(&addr);
    assert!(ack.session_present);
    let again = sub.recv();
    assert!(again.p.dup, "[MQTT-3.3.1-1]");
    assert_eq!(
        (again.p.packet_id, again.p.payload.as_slice()),
        (one.p.packet_id, &b"one"[..])
    );
    assert_eq!(
        &sub.read_kind(T::PubRel)[..2],
        &pid2.to_be_bytes(),
        "[MQTT-4.3.3-6]: PUBREL, not PUBLISH"
    );
    sub.ack(T::PubAck, one.p.packet_id.unwrap(), 0);
    sub.ack(T::PubComp, pid2, 0);
    sub.quiet();
    h.shutdown();
}

#[test]
fn mqtt5_4_9_a_reconnect_resends_no_more_than_the_new_receive_maximum() {
    let (h, addr) = start(ServerConfig::default());
    let mut sub = persistent("rq", 60).ok(&addr);
    sub.subscribe("q", 1);
    let mut publ = conn("pub").ok(&addr);
    publ.publish1("q", b"1", 1, &PropertySet::default());
    publ.publish1("q", b"2", 2, &PropertySet::default());
    let a = sub.recv();
    let b = sub.recv();
    drop(sub);
    std::thread::sleep(Duration::from_millis(100));
    // §4.9: the send quota starts again with the new connection, here 1.
    let mut sub = with_props(
        persistent("rq", 60),
        PropertySet {
            session_expiry_interval: Some(60),
            receive_maximum: Some(1),
            ..PropertySet::default()
        },
    )
    .ok(&addr);
    let again = sub.recv();
    assert!(again.p.dup, "[MQTT-3.3.1-1]");
    assert_eq!(
        (again.p.packet_id, again.p.payload.as_slice()),
        (a.p.packet_id, &b"1"[..])
    );
    sub.quiet();
    sub.ack(T::PubAck, a.p.packet_id.unwrap(), 0);
    let next = sub.recv();
    assert!(next.p.dup, "[MQTT-4.4.0-1]");
    assert_eq!(
        (next.p.packet_id, next.p.payload.as_slice()),
        (b.p.packet_id, &b"2"[..]),
        "[MQTT-3.3.4-9]: after the PUBACK freed the quota"
    );
    sub.ack(T::PubAck, b.p.packet_id.unwrap(), 0);
    sub.quiet();
    h.shutdown();
}

#[test]
fn mqtt5_4_1_a_session_ends_after_its_expiry_interval() {
    let (h, addr) = start(ServerConfig::default());
    let mut c = persistent("short", 1).ok(&addr);
    c.subscribe("t", 0);
    drop(c);
    std::thread::sleep(Duration::from_millis(200));
    assert!(h.has_session("short"), "[MQTT-3.1.2-23]");
    std::thread::sleep(Duration::from_millis(1300));
    assert!(!h.has_session("short"), "[MQTT-4.1.0-2]");
    let (_c, ack) = persistent("short", 1).open(&addr);
    assert!(!ack.session_present);
    h.shutdown();
}

#[test]
fn mqtt5_4_1_a_full_queue_ends_the_session_with_0x97_and_drops_no_message_from_it() {
    let cfg = ServerConfig {
        max_queued_messages: Some(1),
        ..ServerConfig::default()
    };
    let (h, addr) = start(cfg);
    let mut watcher = conn("watch").ok(&addr);
    watcher.subscribe("lwt", 0);
    let mut c = with_props(
        persistent("full", 60),
        PropertySet {
            session_expiry_interval: Some(60),
            receive_maximum: Some(1),
            ..PropertySet::default()
        },
    );
    c.will = will("lwt", b"quota", PropertySet::default());
    let mut sub = c.ok(&addr);
    sub.subscribe("q", 1);
    let mut publ = conn("pub").ok(&addr);
    // In flight (the quota of 1), then queued: the queue is full.
    publ.publish1("q", b"1", 1, &PropertySet::default());
    publ.publish1("q", b"2", 2, &PropertySet::default());
    assert_eq!(sub.recv().p.payload, b"1");
    // QoS 0 at the full queue is not kept (§4.1: optional state).
    publ.publish("q", b"0", 0, false, 0, &PropertySet::default());
    publ.send(T::PingReq, 0, &[]);
    publ.read_kind(T::PingResp);
    assert_eq!(h.queue_overflows(), 1);
    sub.quiet();
    // QoS 1 at the full queue ends the session (§4.1.1, [MQTT-4.1.0-1]).
    publ.publish1("q", b"3", 3, &PropertySet::default());
    sub.disconnected(0x97);
    assert_eq!(
        watcher.recv().p.payload,
        b"quota",
        "[MQTT-3.1.2-8]: the session ended"
    );
    assert!(!h.has_session("full"));
    assert_eq!(h.queue_overflows(), 2);
    let (_s, ack) = persistent("full", 60).open(&addr);
    assert!(!ack.session_present);
    h.shutdown();
}

#[test]
fn mqtt5_3_2_2_3_2_a_session_expiry_above_the_servers_maximum_is_answered() {
    let cfg = ServerConfig {
        maximum_session_expiry: Some(30),
        ..ServerConfig::default()
    };
    let (h, addr) = start(cfg);
    let (_c, ack) = persistent("long", 3600).open(&addr);
    assert_eq!(ack.props.session_expiry_interval, Some(30));
    let (_c, ack) = persistent("fits", 10).open(&addr);
    assert_eq!(ack.props.session_expiry_interval, None);
    h.shutdown();
}

#[test]
fn mqtt5_3_1_3_2_2_a_delayed_will_waits_and_a_reconnect_cancels_it() {
    let (h, addr) = start(ServerConfig::default());
    let mut watcher = conn("watch").ok(&addr);
    watcher.subscribe("lwt", 0);
    let delay = |s| PropertySet {
        will_delay_interval: Some(s),
        user_properties: vec![("b".into(), "1".into()), ("a".into(), "2".into())],
        ..PropertySet::default()
    };
    let mut c = persistent("d1", 60);
    c.will = will("lwt", b"late", delay(1));
    drop(c.ok(&addr));
    watcher.quiet();
    let got = watcher.recv();
    assert_eq!(got.p.payload, b"late", "[MQTT-3.1.2-8]");
    assert_eq!(
        got.props.user_properties,
        delay(1).user_properties,
        "[MQTT-3.1.3-10]"
    );
    let mut c = persistent("d2", 60);
    c.will = will("lwt", b"never", delay(2));
    drop(c.ok(&addr));
    std::thread::sleep(Duration::from_millis(300));
    let _back = persistent("d2", 60).ok(&addr);
    std::thread::sleep(Duration::from_millis(2000));
    watcher.quiet();
    h.shutdown();
}

#[test]
fn mqtt5_3_14_4_disconnect_0x04_publishes_the_will_0x00_does_not() {
    let (h, addr) = start(ServerConfig::default());
    let mut watcher = conn("watch").ok(&addr);
    watcher.subscribe("lwt", 0);
    let mut c = conn("w");
    c.will = will("lwt", b"bye", PropertySet::default());
    c.ok(&addr).disconnect(0x00, &PropertySet::default());
    watcher.quiet();
    c.ok(&addr).disconnect(0x04, &PropertySet::default());
    assert_eq!(watcher.recv().p.payload, b"bye");
    h.shutdown();
}

#[test]
fn mqtt5_3_14_2_2_2_a_session_expiry_in_disconnect_after_0_in_connect_is_a_protocol_error() {
    let (h, addr) = start(ServerConfig::default());
    let mut raw = conn("z").ok(&addr);
    raw.send(T::Disconnect, 0, &[0x00, 5, 0x11, 0, 0, 0, 10]);
    raw.disconnected(0x82);
    // Expiry 10 at CONNECT, 0 at DISCONNECT: the session ends.
    persistent("p", 10).ok(&addr).disconnect(0x00, &expiry(0));
    std::thread::sleep(Duration::from_millis(100));
    assert!(!h.has_session("p"));
    h.shutdown();
    // The rule looks at the CONNECT, not at what the server granted: 60
    // asked for, 0 granted, 60 in DISCONNECT is no Protocol Error.
    let cfg = ServerConfig {
        maximum_session_expiry: Some(0),
        ..ServerConfig::default()
    };
    let (h, addr) = start(cfg);
    let (raw, ack) = persistent("g", 60).open(&addr);
    assert_eq!(ack.props.session_expiry_interval, Some(0));
    raw.disconnect(0x00, &expiry(60));
    std::thread::sleep(Duration::from_millis(100));
    assert!(!h.has_session("g"), "the server's maximum 0 still holds");
    h.shutdown();
}

// ---------------------------------------------------------------- §3.8 to §3.11

#[test]
fn mqtt5_3_9_3_suback_and_unsuback_reason_codes_and_request_problem_information() {
    let (h, addr) = start(ServerConfig::default());
    let mut raw = conn("codes").ok(&addr);
    let (codes, props) = raw.subscribe_with(
        1,
        &[("ok/+", 1), ("a/#/b", 0), ("$share/g", 0)],
        &PropertySet::default(),
    );
    assert_eq!(codes, [1, 0x8F, 0x8F], "[MQTT-3.8.4-6], [MQTT-3.9.3-1]");
    assert!(props.reason_string.is_some());
    let body = UnsubscribeBody {
        packet_id: 3,
        properties: Vec::new(),
        topic_filters: vec!["ok/+".into(), "none".into(), "a/#/b".into()],
    };
    raw.send(
        T::Unsubscribe,
        0b0010,
        &encode_unsubscribe_body_v(&body, V5).unwrap(),
    );
    let ub = raw.read_kind(T::UnsubAck);
    let u = decode_suback_body_v(&ub, V5).unwrap();
    assert_eq!(u.packet_id, 3);
    assert_eq!(u.reason_codes, [0x00, 0x11, 0x8F], "[MQTT-3.11.3-1]");
    // Request Problem Information 0: no Reason String ([MQTT-3.1.2-29]).
    let mut quiet = with_props(
        conn("rpi"),
        PropertySet {
            request_problem_information: Some(0),
            ..PropertySet::default()
        },
    )
    .ok(&addr);
    let (codes, props) = quiet.subscribe_with(1, &[("a/#/b", 0)], &PropertySet::default());
    assert_eq!(codes, [0x8F]);
    assert_eq!(props.reason_string, None);
    h.shutdown();
}

#[test]
fn mqtt5_3_2_2_3_features_the_server_does_not_offer() {
    let cfg = ServerConfig {
        maximum_qos: QoS::AtLeastOnce,
        retain_available: false,
        wildcard_subscription_available: false,
        subscription_identifiers_available: false,
        shared_subscription_available: false,
        ..ServerConfig::default()
    };
    let (h, addr) = start(cfg);
    let (mut raw, ack) = conn("f").open(&addr);
    assert_eq!(ack.props.maximum_qos, Some(1));
    assert_eq!(ack.props.retain_available, Some(0));
    assert_eq!(ack.props.wildcard_subscription_available, Some(0));
    assert_eq!(ack.props.subscription_identifier_available, Some(0));
    assert_eq!(ack.props.shared_subscription_available, Some(0));
    let (codes, _) = raw.subscribe_with(1, &[("t", 2)], &PropertySet::default());
    assert_eq!(codes, [1], "[MQTT-3.2.2-10]");
    raw.publish("t", b"x", 2, false, 1, &PropertySet::default());
    raw.disconnected(0x9B);
    // §3.2.2.3.11, §3.2.2.3.13: a Wildcard or Shared Subscription the
    // CONNACK announced as not available is a Protocol Error, DISCONNECT
    // and the close ([MQTT-4.13.1-1]), each on its own connection.
    for (filter, code) in [("a/+", 0xA2), ("a/#", 0xA2), ("$share/g/t", 0x9E)] {
        let mut raw = conn("w").ok(&addr);
        let body = SubscribeBody {
            packet_id: 1,
            properties: Vec::new(),
            subscriptions: vec![
                Subscription {
                    topic_filter: "t".into(),
                    options: 0,
                },
                Subscription {
                    topic_filter: filter.into(),
                    options: 0,
                },
            ],
        };
        raw.send(
            T::Subscribe,
            0b0010,
            &encode_subscribe_body_v(&body, V5).unwrap(),
        );
        raw.disconnected(code);
    }
    // A 3.1.1 client got no such announcement: 0x80 for the entry, the
    // connection stays.
    let mut old = conn("old");
    old.v = V311;
    let mut raw = old.ok(&addr);
    let (codes, _) = raw.subscribe_with(
        1,
        &[("a/+", 0), ("$share/g/t", 0), ("t", 1)],
        &PropertySet::default(),
    );
    assert_eq!(codes, [0x80, 0x80, 1]);
    raw.send(T::PingReq, 0, &[]);
    raw.read_kind(T::PingResp);
    let mut raw = conn("r").ok(&addr);
    raw.publish("t", b"x", 0, true, 0, &PropertySet::default());
    raw.disconnected(0x9A);
    let mut raw = conn("s").ok(&addr);
    let sid = PropertySet {
        subscription_identifiers: vec![1],
        ..PropertySet::default()
    };
    raw.send(T::Subscribe, 0b0010, &{
        let mut b = vec![0, 1];
        let p = sid.encode().unwrap();
        b.push(u8::try_from(p.len()).unwrap());
        b.extend_from_slice(&p);
        b.extend_from_slice(&[0, 1, b't', 0]);
        b
    });
    raw.disconnected(0xA1);
    // A Will QoS above the Maximum QoS ([MQTT-3.2.2-12]).
    let mut c = conn("wq");
    c.will = Some((
        "lwt".into(),
        b"x".to_vec(),
        2,
        false,
        PropertySet::default(),
    ));
    let (mut raw, ack) = c.open(&addr);
    assert_eq!(ack.code, 0x9B);
    raw.closed();
    // A Will Retain 1 without Retain Available ([MQTT-3.2.2-13]).
    let mut c = conn("wr");
    c.will = Some(("lwt".into(), b"x".to_vec(), 0, true, PropertySet::default()));
    let (mut raw, ack) = c.open(&addr);
    assert_eq!(ack.code, 0x9A);
    raw.closed();
    h.shutdown();
}

// ---------------------------------------------------------------- §4.12 / §4.11

/// Challenge and response: the client sends "hello", the server
/// challenges with "challenge", the client answers "response".
struct Challenge;

struct ChallengeExchange {
    challenged: bool,
}

impl AuthExchange for ChallengeExchange {
    fn step(&mut self, data: Option<&[u8]>) -> AuthStep {
        match (self.challenged, data) {
            (false, Some(b"hello")) => {
                self.challenged = true;
                AuthStep::Continue(b"challenge".to_vec())
            }
            (true, Some(b"response")) => AuthStep::Success(Some(b"welcome".to_vec())),
            _ => AuthStep::Failure(0x87),
        }
    }
}

impl Authenticator for Challenge {
    fn authenticate(&self, _: &str, user: Option<&str>, password: Option<&[u8]>) -> Result<(), u8> {
        match (user, password) {
            (Some("u"), Some(b"right")) => Ok(()),
            _ => Err(0x86),
        }
    }

    fn start(&self, _: &str, method: &str) -> Option<Box<dyn AuthExchange>> {
        (method == "TEST-CHALLENGE")
            .then(|| Box::new(ChallengeExchange { challenged: false }) as Box<dyn AuthExchange>)
    }
}

fn auth_body(code: u8, method: &str, data: &[u8]) -> Vec<u8> {
    let p = PropertySet {
        authentication_method: Some(method.into()),
        authentication_data: Some(data.to_vec()),
        ..PropertySet::default()
    }
    .encode()
    .unwrap();
    let mut b = vec![code, u8::try_from(p.len()).unwrap()];
    b.extend_from_slice(&p);
    b
}

fn read_auth(raw: &mut Raw) -> (u8, PropertySet) {
    let body = raw.read_kind(T::Auth);
    (
        body[0],
        PropertySet::parse(&body[2..], PropertyContext::Packet(T::Auth)).unwrap(),
    )
}

#[test]
fn mqtt5_4_12_enhanced_authentication_and_re_authentication() {
    let cfg = ServerConfig {
        authenticator: Some(Arc::new(Challenge)),
        ..ServerConfig::default()
    };
    let (h, addr) = start(cfg);
    let c = with_props(
        conn("auth"),
        PropertySet {
            authentication_method: Some("TEST-CHALLENGE".into()),
            authentication_data: Some(b"hello".to_vec()),
            ..PropertySet::default()
        },
    );
    let mut raw = Raw::tcp(&addr, V5);
    raw.write(&c.bytes());
    let (code, p) = read_auth(&mut raw);
    assert_eq!(code, 0x18, "[MQTT-4.12.0-2]");
    assert_eq!(
        p.authentication_method.as_deref(),
        Some("TEST-CHALLENGE"),
        "[MQTT-4.12.0-5]"
    );
    assert_eq!(p.authentication_data.as_deref(), Some(&b"challenge"[..]));
    raw.send(T::Auth, 0, &auth_body(0x18, "TEST-CHALLENGE", b"response"));
    let ack = raw.connack();
    assert_eq!(ack.code, 0);
    assert_eq!(
        ack.props.authentication_method.as_deref(),
        Some("TEST-CHALLENGE")
    );
    assert_eq!(
        ack.props.authentication_data.as_deref(),
        Some(&b"welcome"[..])
    );
    // Re-authentication (§4.12.1).
    raw.send(T::Auth, 0, &auth_body(0x19, "TEST-CHALLENGE", b"hello"));
    assert_eq!(read_auth(&mut raw).0, 0x18);
    raw.send(T::Auth, 0, &auth_body(0x18, "TEST-CHALLENGE", b"response"));
    let (code, p) = read_auth(&mut raw);
    assert_eq!(code, 0x00);
    assert_eq!(p.authentication_method.as_deref(), Some("TEST-CHALLENGE"));
    // A failed re-authentication: DISCONNECT and close ([MQTT-4.12.1-2]).
    raw.send(T::Auth, 0, &auth_body(0x19, "TEST-CHALLENGE", b"wrong"));
    raw.disconnected(0x87);
    // An unknown method: 0x8C ([MQTT-4.12.0-1]).
    let other = with_props(
        conn("other"),
        PropertySet {
            authentication_method: Some("OTHER".into()),
            ..PropertySet::default()
        },
    );
    let (mut raw, ack) = other.open(&addr);
    assert_eq!(ack.code, 0x8C);
    raw.closed();
    // AUTH without an Authentication Method in the CONNECT ([MQTT-4.12.0-7]).
    let mut plain = conn("plain");
    plain.user = Some("u".into());
    plain.pass = Some(b"right".to_vec());
    let mut raw = plain.ok(&addr);
    raw.send(T::Auth, 0, &auth_body(0x19, "TEST-CHALLENGE", b"hello"));
    raw.disconnected(0x82);
    h.shutdown();
}

#[test]
fn mqtt5_4_12_simple_authentication_refuses_a_wrong_password() {
    let cfg = ServerConfig {
        authenticator: Some(Arc::new(Challenge)),
        ..ServerConfig::default()
    };
    let (h, addr) = start(cfg);
    let mut c = conn("pw");
    c.user = Some("u".into());
    c.pass = Some(b"wrong".to_vec());
    let (mut raw, ack) = c.open(&addr);
    assert_eq!(ack.code, 0x86);
    raw.closed();
    c.v = V311;
    let (mut raw, ack) = c.open(&addr);
    assert_eq!(ack.code, 0x04, "3.1.1 Bad user name or password");
    raw.closed();
    h.shutdown();
}

#[test]
fn mqtt5_3_1_2_28_response_information_only_on_request() {
    let cfg = ServerConfig {
        response_information: Some("resp/".into()),
        ..ServerConfig::default()
    };
    let (h, addr) = start(cfg);
    let asks = with_props(
        conn("asks"),
        PropertySet {
            request_response_information: Some(1),
            ..PropertySet::default()
        },
    );
    assert_eq!(
        asks.open(&addr).1.props.response_information.as_deref(),
        Some("resp/")
    );
    assert_eq!(
        conn("silent").open(&addr).1.props.response_information,
        None
    );
    h.shutdown();
}

#[test]
fn mqtt5_4_11_server_redirection_with_a_server_reference() {
    let cfg = ServerConfig {
        redirect: Some(Redirect {
            permanent: false,
            server_reference: "other.example:1883".into(),
        }),
        ..ServerConfig::default()
    };
    let (h, addr) = start(cfg);
    let (mut raw, ack) = conn("r").open(&addr);
    assert_eq!(ack.code, 0x9C);
    assert_eq!(
        ack.props.server_reference.as_deref(),
        Some("other.example:1883")
    );
    raw.closed();
    h.shutdown();
    let (h, addr) = start(ServerConfig::default());
    let mut raw = conn("moved").ok(&addr);
    h.redirect_clients(&Redirect {
        permanent: true,
        server_reference: "new.example".into(),
    });
    let props = raw.disconnected(0x9D);
    assert_eq!(props.server_reference.as_deref(), Some("new.example"));
    h.shutdown();
}

#[test]
fn mqtt5_3_14_0_1_a_server_shutdown_sends_0x8b() {
    let (h, addr) = start(ServerConfig::default());
    let mut raw = conn("down").ok(&addr);
    h.shutdown();
    raw.disconnected(0x8B);
}

/// [MQTT-3.2.2-8]: a connection the server accepted before its shutdown
/// whose CONNECT comes after it is refused with a CONNACK Reason Code
/// (0x88 Server unavailable; 0x8B is none), MQTT 3.1.1 with Return Code
/// 0x03, and closed; it used to get CONNACK 0x00 and stay open.
#[test]
fn mqtt5_3_2_2_2_a_connect_after_the_shutdown_gets_0x88_and_the_close() {
    let (h, addr) = start(ServerConfig::default());
    let mut v5 = Raw::tcp(&addr, V5);
    let mut v311 = Raw::tcp(&addr, V311);
    // The accept loop hands both connections to their threads, which wait
    // for the CONNECT.
    std::thread::sleep(QUIET);
    h.shutdown();
    let late = conn("late");
    v5.write(&late.bytes());
    let ack = v5.connack();
    assert_eq!((ack.code, ack.session_present), (0x88, false));
    v5.closed();
    let mut old = conn("late-311");
    old.v = V311;
    v311.write(&old.bytes());
    let ack = v311.connack();
    assert_eq!((ack.code, ack.session_present), (0x03, false));
    v311.closed();
}

// ---------------------------------------------------------------- §4.6 / §4.7

#[test]
fn mqtt5_4_6_messages_of_a_topic_arrive_in_the_order_they_were_published() {
    let (h, addr) = start(ServerConfig::default());
    let mut sub = conn("ord-sub").ok(&addr);
    sub.subscribe("ord", 1);
    let mut publ = conn("ord-pub").ok(&addr);
    for i in 1..=50u16 {
        publ.publish(
            "ord",
            &i.to_be_bytes(),
            1,
            false,
            i,
            &PropertySet::default(),
        );
    }
    for i in 1..=50u16 {
        let got = sub.recv();
        assert_eq!(
            got.p.payload,
            i.to_be_bytes(),
            "[MQTT-4.6.0-5], [MQTT-4.6.0-6]"
        );
        sub.ack(T::PubAck, got.p.packet_id.unwrap(), 0);
    }
    h.shutdown();
}

#[test]
fn mqtt5_4_7_2_a_wildcard_filter_does_not_match_a_topic_beginning_with_dollar() {
    let (h, addr) = start(ServerConfig::default());
    let mut all = conn("all").ok(&addr);
    all.subscribe("#", 0);
    all.subscribe("+/x", 0);
    let mut sys = conn("sys").ok(&addr);
    sys.subscribe("$SYS/#", 0);
    let mut publ = conn("pub").ok(&addr);
    publ.publish("$SYS/x", b"s", 0, false, 0, &PropertySet::default());
    assert_eq!(sys.recv().p.topic, "$SYS/x");
    all.quiet();
    h.shutdown();
}

#[test]
fn mqtt5_3_6_2_2_a_reason_string_beyond_the_clients_maximum_packet_size_is_left_out() {
    let (h, addr) = start(ServerConfig::default());
    // PUBREC for a Packet Identifier the server never used: PUBREL 0x92.
    let mut wide = conn("wide").ok(&addr);
    wide.ack(T::PubRec, 9, 0);
    let body = wide.read_kind(T::PubRel);
    assert_eq!(body[2], 0x92);
    let props = PropertySet::parse(&body[4..], PropertyContext::Packet(T::PubRel)).unwrap();
    assert!(props.reason_string.is_some());
    let mut narrow = with_props(
        conn("narrow"),
        PropertySet {
            maximum_packet_size: Some(30),
            ..PropertySet::default()
        },
    )
    .ok(&addr);
    narrow.ack(T::PubRec, 9, 0);
    let body = narrow.read_kind(T::PubRel);
    assert_eq!(body, [0, 9, 0x92, 0], "[MQTT-3.6.2-2]");
    h.shutdown();
}

// ---------------------------------------------------------------- ClientStream

/// A transport other than the server's own listener: a TCP stream that
/// counts what passes through it.
struct Counted {
    s: TcpStream,
    read: Arc<AtomicUsize>,
    written: Arc<AtomicUsize>,
}

impl Read for Counted {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.s.read(buf)?;
        self.read.fetch_add(n, Ordering::SeqCst);
        Ok(n)
    }
}

struct CountedWriter {
    s: TcpStream,
    written: Arc<AtomicUsize>,
}

impl Write for CountedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.s.write(buf)?;
        self.written.fetch_add(n, Ordering::SeqCst);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.s.flush()
    }
}

impl ClientWriter for CountedWriter {
    fn close(&mut self) {
        let _ = self.s.shutdown(Shutdown::Both);
    }
}

impl ClientStream for Counted {
    fn writer(&self) -> io::Result<Box<dyn ClientWriter>> {
        Ok(Box::new(CountedWriter {
            s: self.s.try_clone()?,
            written: Arc::clone(&self.written),
        }))
    }

    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.s.set_read_timeout(timeout)
    }
}

#[test]
fn mqtt5_4_2_a_connection_of_another_transport_is_served_through_client_stream() {
    let (h, addr) = start(ServerConfig::default());
    let mut sub = conn("tcp").ok(&addr);
    sub.subscribe("other/t", 1);
    let side = TcpListener::bind("127.0.0.1:0").unwrap();
    let side_addr = side.local_addr().unwrap().to_string();
    let mut raw = Raw::tcp(&side_addr, V5);
    let (stream, _) = side.accept().unwrap();
    let read = Arc::new(AtomicUsize::new(0));
    let written = Arc::new(AtomicUsize::new(0));
    h.serve(Counted {
        s: stream,
        read: Arc::clone(&read),
        written: Arc::clone(&written),
    });
    raw.write(&conn("other").bytes());
    assert_eq!(raw.connack().code, 0);
    assert_eq!(
        raw.publish1("other/t", b"via", 1, &PropertySet::default()),
        0
    );
    assert_eq!(sub.recv().p.payload, b"via");
    assert_eq!(h.connected_clients(), ["other", "tcp"]);
    raw.disconnect(0, &PropertySet::default());
    assert!(read.load(Ordering::SeqCst) > 0);
    assert!(written.load(Ordering::SeqCst) > 0);
    h.shutdown();
}
