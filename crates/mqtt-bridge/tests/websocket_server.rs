// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The WebSocket listeners of the broker server (OASIS MQTT 5.0 §6,
//! RFC 6455): `ws` on TCP and `wss` on TLS, beside the TCP and the TLS
//! listener.
//!
//! A raw client in this process writes the opening handshake and the frames
//! by hand, masked with a key of its own, and reads every frame of the
//! server with its mask bit: the subprotocol the server selects
//! ([MQTT-6.0.0-4]), the refusal of a client that does not offer `mqtt`,
//! a text frame that closes the Network Connection ([MQTT-6.0.0-1]),
//! control packets split over frames and several in one frame
//! ([MQTT-6.0.0-2]), one unmasked binary frame per control packet of the
//! server, the frames RFC 6455 refuses, the closing handshake and the close
//! of the TCP connection behind it, and requests on other paths than
//! `/mqtt` (the URI has no impact on MQTT, §6). Messages
//! cross between the listeners with the ZeroDDS clients of `net.rs` (TCP)
//! and `daemon::client` (`ws://`, `wss://`, `mqtts://`).

#![cfg(feature = "daemon")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use zerodds_mqtt_bridge::codec::{PublishPacket, decode_publish_v, encode_publish_v};
use zerodds_mqtt_bridge::control_packets::{
    ConnectBody, SubscribeBody, Subscription, connect_flags, decode_connack_body_v,
    decode_suback_body_v, encode_connect_body_v, encode_subscribe_body_v,
};
use zerodds_mqtt_bridge::daemon::client::{InboundEvent, MqttClient as UrlClient, SubscribeEntry};
use zerodds_mqtt_bridge::daemon::config::{DaemonConfig, parse_broker_url};
use zerodds_mqtt_bridge::net::{MqttClient, byte0, frame_packet, read_packet};
use zerodds_mqtt_bridge::packet::ControlPacketType as T;
use zerodds_mqtt_bridge::server::{
    CLOSE_WAIT, MqttBrokerServer, ServerConfig, ServerHandle, Transport,
};
use zerodds_mqtt_bridge::version::ProtocolVersion::{V5, V311};
use zerodds_websocket_bridge::codec::{decode, encode, peek_header};
use zerodds_websocket_bridge::frame::{Frame, Opcode};
use zerodds_websocket_bridge::handshake::{client_key, validate_server_response};

const TO: Duration = Duration::from_secs(5);
/// The masking key of the raw client's frames.
const KEY: [u8; 4] = [0x37, 0xfa, 0x21, 0x3d];

fn start_with(cfg: ServerConfig) -> ServerHandle {
    MqttBrokerServer::bind_with("127.0.0.1:0", cfg)
        .unwrap()
        .with_websocket("127.0.0.1:0")
        .unwrap()
        .spawn()
        .unwrap()
}

fn start() -> ServerHandle {
    start_with(ServerConfig::default())
}

fn ws_addr(h: &ServerHandle) -> SocketAddr {
    h.transport_addr(Transport::WebSocket)
        .expect("a WebSocket listener")
}

/// An opening handshake on `/mqtt` offering `protocols` (no header with
/// `None`) and the key it carries.
fn request(protocols: Option<&str>) -> (String, String) {
    request_at("/mqtt", protocols)
}

/// [`request`] with the resource name `path` (RFC 6455 §4.1).
fn request_at(path: &str, protocols: Option<&str>) -> (String, String) {
    let key = client_key(&[7u8; 16]);
    let mut r = format!(
        "GET {path} HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n"
    );
    if let Some(p) = protocols {
        r.push_str(&format!("Sec-WebSocket-Protocol: {p}\r\n"));
    }
    r.push_str("\r\n");
    (r, key)
}

/// Sends `request` and reads the response head; the octets behind it stay
/// with the stream.
fn exchange(addr: SocketAddr, request: &str) -> (String, TcpStream, Vec<u8>) {
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(TO)).unwrap();
    s.write_all(request.as_bytes()).unwrap();
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let rest = buf.split_off(i + 4);
            return (String::from_utf8(buf).unwrap(), s, rest);
        }
        let n = s.read(&mut chunk).expect("the response of the server");
        assert!(n > 0, "the server closed before the end of its response");
        buf.extend_from_slice(&chunk[..n]);
    }
}

/// Reads until the server closes the connection; panics when it keeps it
/// open for [`TO`].
fn until_closed(s: &mut TcpStream) -> Vec<u8> {
    let mut all = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match s.read(&mut chunk) {
            Ok(0) => return all,
            Ok(n) => all.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == ErrorKind::ConnectionReset => return all,
            Err(e) => panic!("the server kept the connection open: {e}"),
        }
    }
}

/// The raw WebSocket client.
struct Ws {
    s: TcpStream,
    buf: Vec<u8>,
}

impl Ws {
    /// The opening handshake on `/mqtt` offering `mqtt`; the response must
    /// be valid (RFC 6455 §4.1) and select `mqtt`.
    fn open(addr: SocketAddr) -> Self {
        Self::open_at(addr, "/mqtt")
    }

    /// [`Ws::open`] with the resource name `path`.
    fn open_at(addr: SocketAddr, path: &str) -> Self {
        let (req, key) = request_at(path, Some("mqtt"));
        let (head, s, buf) = exchange(addr, &req);
        assert!(head.starts_with("HTTP/1.1 101 "), "{path}: {head}");
        validate_server_response(&head, &key, &["mqtt"], true)
            .unwrap_or_else(|e| panic!("{path}: {e}: {head}"));
        Self { s, buf }
    }

    /// `open`, then CONNECT and its CONNACK with Reason Code 0.
    fn connected(addr: SocketAddr, id: &str) -> Self {
        let mut ws = Self::open(addr);
        ws.binary(&connect(id, None));
        ws.connack();
        ws
    }

    fn raw(&mut self, frame: &Frame) {
        self.s.write_all(&encode(frame).unwrap()).unwrap();
    }

    /// A masked frame.
    fn send(&mut self, opcode: Opcode, fin: bool, payload: &[u8]) {
        self.raw(&Frame {
            fin,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode,
            masking_key: Some(KEY),
            payload: payload.to_vec(),
        });
    }

    fn binary(&mut self, payload: &[u8]) {
        self.send(Opcode::Binary, true, payload);
    }

    /// The next frame of the server and whether it was masked; `None` when
    /// the server closed the connection.
    fn frame(&mut self) -> Option<(Frame, bool)> {
        let mut chunk = [0u8; 16 * 1024];
        loop {
            match decode(&self.buf) {
                Ok((f, used)) => {
                    let masked = peek_header(&self.buf).unwrap().masked;
                    self.buf.drain(..used);
                    return Some((f, masked));
                }
                Err(e) if e.is_incomplete() => {}
                Err(e) => panic!("the server sent an invalid frame: {e}"),
            }
            match self.s.read(&mut chunk) {
                Ok(0) => return None,
                Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == ErrorKind::ConnectionReset => return None,
                Err(e) => panic!("the server sent nothing within {TO:?}: {e}"),
            }
        }
    }

    /// The next control packet: one unmasked binary frame with FIN that
    /// carries exactly one packet (MQTT 5.0 §6, RFC 6455 §5.1).
    fn packet(&mut self) -> (u8, Vec<u8>) {
        let (f, masked) = self.frame().expect("a frame of the server");
        assert_eq!(f.opcode, Opcode::Binary, "[MQTT-6.0.0-1]: {f:?}");
        assert!(f.fin, "one frame per control packet");
        assert!(!masked, "RFC 6455 §5.1: the server masks no frame");
        let mut rest = f.payload.as_slice();
        let packet = read_packet(&mut rest).unwrap();
        assert!(rest.is_empty(), "one control packet per frame");
        packet
    }

    fn packet_of(&mut self, kind: T) -> Vec<u8> {
        let (b0, body) = self.packet();
        assert_eq!(b0 >> 4, kind.to_bits(), "expected {kind:?}, got {b0:#x}");
        body
    }

    fn connack(&mut self) {
        let body = self.packet_of(T::ConnAck);
        assert_eq!(decode_connack_body_v(&body, V5).unwrap().reason_code, 0);
    }

    fn suback(&mut self, pid: u16) {
        let s = decode_suback_body_v(&self.packet_of(T::SubAck), V5).unwrap();
        assert_eq!(s.packet_id, pid);
        assert!(s.reason_codes.iter().all(|c| *c < 0x80), "{s:?}");
    }

    fn publish(&mut self) -> PublishPacket {
        let (b0, body) = self.packet();
        assert_eq!(b0 >> 4, T::Publish.to_bits());
        decode_publish_v(&frame_packet(b0, &body).unwrap(), V5)
            .unwrap()
            .1
    }

    /// Whether the server's socket no longer reads: two masked Pong frames
    /// 100 ms apart (a Pong answers nothing, RFC 6455 §5.5.3). Linux
    /// answers data that reaches a socket closed for reading with a RST
    /// (RFC 1122 §4.2.2.13), and the write after it fails.
    fn server_stopped_reading(&mut self) -> bool {
        let pong = encode(&Frame {
            fin: true,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode: Opcode::Pong,
            masking_key: Some(KEY),
            payload: b"probe".to_vec(),
        })
        .unwrap();
        for i in 0..2 {
            match self.s.write_all(&pong) {
                Ok(()) => {}
                Err(e)
                    if matches!(e.kind(), ErrorKind::ConnectionReset | ErrorKind::BrokenPipe) =>
                {
                    return true;
                }
                Err(e) => panic!("the write failed otherwise: {e}"),
            }
            if i == 0 {
                thread::sleep(Duration::from_millis(100));
            }
        }
        false
    }

    /// The next frame is a Close frame with `body`, and the server closes
    /// the connection behind it.
    fn closed_with(&mut self, body: &[u8], what: &str) {
        let (f, masked) = self
            .frame()
            .unwrap_or_else(|| panic!("{what}: no Close frame"));
        assert_eq!(f.opcode, Opcode::Close, "{what}: {f:?}");
        assert!(!masked, "{what}: RFC 6455 §5.1");
        assert_eq!(f.payload.get(..body.len()), Some(body), "{what}: {f:?}");
        assert!(
            self.frame().is_none(),
            "{what}: the server closes the connection after its Close frame"
        );
    }
}

fn connect(id: &str, will: Option<(&str, &[u8])>) -> Vec<u8> {
    let mut flags = connect_flags::CLEAN_START;
    if will.is_some() {
        flags |= connect_flags::WILL;
    }
    let body = ConnectBody {
        protocol_name: "MQTT".into(),
        protocol_version: V5.level(),
        connect_flags: flags,
        keep_alive: 0,
        properties: Vec::new(),
        client_id: id.into(),
        will_properties: Vec::new(),
        will_topic: will.map(|w| w.0.into()),
        will_payload: will.map(|w| w.1.to_vec()).unwrap_or_default(),
        user_name: None,
        password: Vec::new(),
    };
    frame_packet(
        byte0(T::Connect, 0),
        &encode_connect_body_v(&body, V5).unwrap(),
    )
    .unwrap()
}

fn subscribe(pid: u16, filter: &str) -> Vec<u8> {
    let body = SubscribeBody {
        packet_id: pid,
        properties: Vec::new(),
        subscriptions: vec![Subscription {
            topic_filter: filter.into(),
            options: 0,
        }],
    };
    frame_packet(
        byte0(T::Subscribe, 0b0010),
        &encode_subscribe_body_v(&body, V5).unwrap(),
    )
    .unwrap()
}

fn publish(topic: &str, payload: &[u8]) -> Vec<u8> {
    encode_publish_v(
        &PublishPacket {
            dup: false,
            qos: 0,
            retain: false,
            topic: topic.into(),
            packet_id: None,
            properties: Vec::new(),
            payload: payload.to_vec(),
        },
        V5,
    )
    .unwrap()
}

fn pingreq() -> Vec<u8> {
    frame_packet(byte0(T::PingReq, 0), &[]).unwrap()
}

/// A TCP subscriber of `filter`.
fn tcp_subscriber(h: &ServerHandle, id: &str, filter: &str) -> MqttClient {
    let mut c = MqttClient::connect(h.local_addr(), id, V5).unwrap();
    c.subscribe(filter, 0).unwrap();
    c
}

/// Waits until the server counts `n` failed handshakes; the count follows
/// the close the client sees.
fn handshake_failures_reach(h: &ServerHandle, n: u64) {
    let deadline = Instant::now() + TO;
    while h.handshake_failures() < n && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(h.handshake_failures(), n);
}

fn big_payload() -> Vec<u8> {
    // 70 000 octets: the frame carries a 64-bit length (RFC 6455 §5.2).
    (0..70_000u32).map(|i| (i % 251) as u8).collect()
}

#[test]
fn mqtt5_6_0_0_4_the_server_selects_the_subprotocol_mqtt() {
    let h = start();
    let (req, key) = request(Some("mqttv3.1, mqtt"));
    let (head, s, buf) = exchange(ws_addr(&h), &req);
    assert!(head.starts_with("HTTP/1.1 101 "), "{head}");
    assert!(
        head.contains("\r\nSec-WebSocket-Protocol: mqtt\r\n"),
        "[MQTT-6.0.0-4]: {head}"
    );
    assert!(!head.contains("Sec-WebSocket-Extensions"), "{head}");
    validate_server_response(&head, &key, &["mqtt"], true).unwrap();
    let mut ws = Ws { s, buf };
    ws.binary(&connect("ws-proto", None));
    ws.connack();
    assert_eq!(h.connected_clients(), ["ws-proto"]);
    assert_eq!(h.handshake_failures(), 0);
    h.shutdown();
}

#[test]
fn mqtt5_6_the_request_path_has_no_impact_on_mqtt() {
    let h = start();
    let paths = ["/", "/x/y?z=1", "/mqtt/other", "/%E2%9C%93"];
    let mut conns: Vec<Ws> = paths
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let mut ws = Ws::open_at(ws_addr(&h), path);
            ws.binary(&connect(&format!("ws-path-{i}"), None));
            ws.connack();
            ws
        })
        .collect();
    assert_eq!(h.connected_clients().len(), paths.len());
    conns[0].binary(&subscribe(1, "path/t"));
    conns[0].suback(1);
    conns[paths.len() - 1].binary(&publish("path/t", b"any path"));
    let p = conns[0].publish();
    assert_eq!(
        (p.topic.as_str(), p.payload.as_slice()),
        ("path/t", &b"any path"[..])
    );
    assert_eq!(h.handshake_failures(), 0);
    h.shutdown();
}

#[test]
fn mqtt5_6_0_0_4_a_client_that_does_not_offer_mqtt_is_refused() {
    let h = start();
    for (offer, what) in [(None, "no subprotocol"), (Some("mqttv3.1"), "another one")] {
        let (req, _) = request(offer);
        let (head, mut s, mut rest) = exchange(ws_addr(&h), &req);
        assert!(head.starts_with("HTTP/1.1 400 "), "{what}: {head}");
        assert!(!head.contains("Sec-WebSocket-Accept"), "{what}: {head}");
        rest.extend(until_closed(&mut s));
        assert!(
            String::from_utf8_lossy(&rest).contains("subprotocol mqtt"),
            "{what}: the body names the reason"
        );
    }
    handshake_failures_reach(&h, 2);
    assert!(h.connected_clients().is_empty());
    // The listener goes on serving.
    Ws::connected(ws_addr(&h), "after-refusals");
    h.shutdown();
}

#[test]
fn mqtt5_6_an_invalid_opening_handshake_gets_an_http_error_response() {
    let h = start();
    let (good, _) = request(Some("mqtt"));
    for (req, status, what) in [
        (
            good.replace("Version: 13", "Version: 8"),
            "426",
            "another version (RFC 6455 §4.2.2)",
        ),
        (good.replacen("GET", "POST", 1), "405", "not GET"),
        (
            good.replace("Upgrade: websocket\r\n", ""),
            "400",
            "no Upgrade",
        ),
        (
            good.replace(&request(None).1, "c2hvcnQ="),
            "400",
            "a key that is not 16 octets",
        ),
    ] {
        let (head, mut s, _) = exchange(ws_addr(&h), &req);
        assert!(
            head.starts_with(&format!("HTTP/1.1 {status} ")),
            "{what}: {head}"
        );
        until_closed(&mut s);
    }
    handshake_failures_reach(&h, 4);
    h.shutdown();
}

#[test]
fn mqtt5_6_the_opening_handshake_has_a_deadline() {
    let h = start_with(ServerConfig {
        connect_timeout: Duration::from_millis(300),
        ..ServerConfig::default()
    });
    let mut s = TcpStream::connect(ws_addr(&h)).unwrap();
    s.set_read_timeout(Some(TO)).unwrap();
    s.write_all(b"GET /mqtt HTTP/1.1\r\nHost: x\r\n").unwrap();
    let t = Instant::now();
    assert!(until_closed(&mut s).is_empty());
    assert!(t.elapsed() < Duration::from_secs(3), "{:?}", t.elapsed());
    handshake_failures_reach(&h, 1);
    h.shutdown();
}

#[test]
fn mqtt5_6_0_0_1_a_text_frame_closes_the_network_connection() {
    let h = start();
    let mut sub = tcp_subscriber(&h, "will-sub", "will/text");
    let mut ws = Ws::open(ws_addr(&h));
    ws.binary(&connect("ws-text", Some(("will/text", b"gone"))));
    ws.connack();
    ws.send(Opcode::Text, true, b"{\"not\":\"mqtt\"}");
    ws.closed_with(&1003u16.to_be_bytes(), "a text frame");
    // The Network Connection closed without DISCONNECT: the Will goes out
    // (§3.1.2.5).
    let (topic, payload) = sub.recv_publish(TO).unwrap();
    assert_eq!(
        (topic.as_str(), payload.as_slice()),
        ("will/text", &b"gone"[..])
    );
    assert!(h.connected_clients() == ["will-sub"]);
    h.shutdown();
}

#[test]
fn mqtt5_6_0_0_2_a_control_packet_split_over_frames_is_read_as_one() {
    let h = start();
    let mut pubr = MqttClient::connect(h.local_addr(), "tcp-pub", V5).unwrap();
    let mut ws = Ws::open(ws_addr(&h));
    // CONNECT in three messages, cut inside the fixed header and inside the
    // protocol name.
    let c = connect("ws-split", None);
    ws.binary(&c[..1]);
    ws.binary(&c[1..6]);
    ws.binary(&[]);
    ws.binary(&c[6..]);
    ws.connack();
    // SUBSCRIBE in one fragmented message with a Ping between the
    // fragments (RFC 6455 §5.4).
    let s = subscribe(7, "split/t");
    ws.send(Opcode::Binary, false, &s[..3]);
    ws.send(Opcode::Ping, true, b"between");
    ws.send(Opcode::Continuation, false, &s[3..5]);
    ws.send(Opcode::Continuation, true, &s[5..]);
    let (pong, masked) = ws.frame().unwrap();
    assert_eq!(
        (pong.opcode, pong.payload.as_slice()),
        (Opcode::Pong, &b"between"[..])
    );
    assert!(!masked);
    ws.suback(7);
    // A PUBLISH one octet per frame, then one that starts in the frame
    // where a PINGREQ ends.
    for b in publish("split/t", b"octets") {
        ws.binary(&[b]);
    }
    assert_eq!(ws.publish().payload, b"octets");
    let mut two = pingreq();
    let p = publish("split/t", b"second");
    two.extend_from_slice(&p[..4]);
    ws.binary(&two);
    ws.binary(&p[4..]);
    ws.packet_of(T::PingResp);
    assert_eq!(ws.publish().payload, b"second");
    // And a message from the TCP listener arrives whole.
    pubr.publish("split/t", b"from-tcp", 0, false).unwrap();
    assert_eq!(ws.publish().payload, b"from-tcp");
    h.shutdown();
}

#[test]
fn mqtt5_6_0_0_2_several_control_packets_in_one_frame_are_all_handled() {
    let h = start();
    let mut ws = Ws::open(ws_addr(&h));
    let mut all = connect("ws-many", None);
    all.extend(subscribe(3, "many/t"));
    all.extend(pingreq());
    all.extend(publish("many/t", b"self"));
    ws.binary(&all);
    ws.connack();
    ws.suback(3);
    ws.packet_of(T::PingResp);
    assert_eq!(ws.publish().payload, b"self");
    h.shutdown();
}

#[test]
fn mqtt5_6_0_0_1_the_server_sends_each_control_packet_in_one_unmasked_binary_frame() {
    let h = start();
    let mut ws = Ws::connected(ws_addr(&h), "ws-frames");
    ws.binary(&subscribe(1, "frames/#"));
    ws.suback(1);
    let mut pubr = MqttClient::connect(h.local_addr(), "tcp-frames", V5).unwrap();
    let big = big_payload();
    for (topic, payload) in [
        ("frames/a", &b"short"[..]),
        ("frames/b", &big[..]),
        ("frames/c", &[][..]),
    ] {
        pubr.publish(topic, payload, 0, false).unwrap();
        // `publish` asserts one unmasked binary frame per packet.
        let p = ws.publish();
        assert_eq!(p.topic, topic);
        assert!(p.payload == payload, "{topic}");
    }
    // The 70 000 octets of a client in one frame, too.
    ws.binary(&publish("frames/d", &big));
    let p = ws.publish();
    assert_eq!(p.topic, "frames/d");
    assert!(p.payload == big);
    h.shutdown();
}

#[test]
fn mqtt5_6_invalid_frames_of_the_client_fail_the_connection() {
    let h = start();
    let addr = ws_addr(&h);
    let plain = |opcode, fin, payload: &[u8]| Frame {
        fin,
        rsv1: false,
        rsv2: false,
        rsv3: false,
        opcode,
        masking_key: Some(KEY),
        payload: payload.to_vec(),
    };
    let cases: Vec<(&str, Vec<Frame>, u16)> = vec![
        (
            "an unmasked frame (RFC 6455 §5.1)",
            vec![Frame::binary(pingreq())],
            1002,
        ),
        (
            "an RSV bit (§5.2)",
            vec![Frame {
                rsv1: true,
                ..plain(Opcode::Binary, true, &pingreq())
            }],
            1002,
        ),
        (
            "a reserved opcode (§5.2)",
            vec![plain(Opcode::Reserved(0x3), true, b"x")],
            1002,
        ),
        (
            "a continuation frame outside a message (§5.4)",
            vec![plain(Opcode::Continuation, true, &pingreq())],
            1002,
        ),
        (
            "a binary frame inside an unfinished message (§5.4)",
            vec![
                plain(Opcode::Binary, false, &pingreq()[..1]),
                plain(Opcode::Binary, true, &pingreq()[1..]),
            ],
            1002,
        ),
        (
            "a Close frame with one octet (§5.5.1)",
            vec![plain(Opcode::Close, true, &[0x03])],
            1002,
        ),
        (
            "a Close frame with status 1005 (§7.4.1)",
            vec![plain(Opcode::Close, true, &1005u16.to_be_bytes())],
            1002,
        ),
        (
            "a Close reason that is not UTF-8 (§5.5.1)",
            vec![plain(Opcode::Close, true, &[0x03, 0xe8, 0xff, 0xfe])],
            1007,
        ),
    ];
    for (i, (what, frames, status)) in cases.into_iter().enumerate() {
        let mut ws = Ws::connected(addr, &format!("ws-invalid-{i}"));
        for f in &frames {
            ws.raw(f);
        }
        ws.closed_with(&status.to_be_bytes(), what);
    }
    h.shutdown();
}

#[test]
fn mqtt5_6_a_close_frame_of_the_client_is_answered_and_its_will_published() {
    let h = start();
    let mut sub = tcp_subscriber(&h, "close-sub", "will/close");
    let mut ws = Ws::open(ws_addr(&h));
    ws.binary(&connect("ws-close", Some(("will/close", b"closed"))));
    ws.connack();
    ws.send(Opcode::Close, true, &Frame::close(1001, "bye").payload);
    // RFC 6455 §5.5.1: the answer echoes the status code.
    ws.closed_with(&1001u16.to_be_bytes(), "the answer to a Close frame");
    let (topic, payload) = sub.recv_publish(TO).unwrap();
    assert_eq!(
        (topic.as_str(), payload.as_slice()),
        ("will/close", &b"closed"[..])
    );

    // A Close frame without body is answered with one without body.
    let mut ws = Ws::connected(ws_addr(&h), "ws-close-empty");
    ws.send(Opcode::Close, true, &[]);
    let (f, _) = ws.frame().unwrap();
    assert_eq!((f.opcode, f.payload.len()), (Opcode::Close, 0));
    assert!(ws.frame().is_none());
    h.shutdown();
}

#[test]
fn mqtt5_6_the_server_ends_a_connection_with_disconnect_then_a_close_frame() {
    let h = start();
    let mut ws = Ws::connected(ws_addr(&h), "ws-taken");
    // A second connection with the same ClientID on the TCP listener takes
    // the session over (§3.1.4): DISCONNECT 0x8E in a binary frame, then
    // the Close frame 1000 and the end of the server's direction.
    let _tcp = MqttClient::connect(h.local_addr(), "ws-taken", V5).unwrap();
    assert_eq!(ws.packet_of(T::Disconnect)[0], 0x8E);
    let (f, masked) = ws.frame().unwrap();
    assert_eq!(f.opcode, Opcode::Close);
    assert!(!masked);
    assert_eq!(&f.payload[..2], &1000u16.to_be_bytes());
    assert!(ws.frame().is_none(), "the server's direction ends");
    // The server still reads, so the client's Close frame can reach it
    // (RFC 6455 §7.1.1).
    assert!(
        !ws.server_stopped_reading(),
        "the server reads on after its Close frame"
    );
    // The client's Close frame comes: the server closes the TCP connection
    // then, well before CLOSE_WAIT.
    let sent = Instant::now();
    ws.send(Opcode::Close, true, &Frame::close(1000, "").payload);
    thread::sleep(Duration::from_millis(100));
    assert!(
        ws.server_stopped_reading(),
        "the server closes the TCP connection after the client's Close frame"
    );
    assert!(sent.elapsed() < CLOSE_WAIT / 2, "{:?}", sent.elapsed());
    h.shutdown();
}

fn sleep_until(t: Instant) {
    if let Some(d) = t.checked_duration_since(Instant::now()) {
        thread::sleep(d);
    }
}

#[test]
fn mqtt5_6_without_a_close_frame_of_the_client_the_server_closes_after_close_wait() {
    let h = start();
    let mut ws = Ws::connected(ws_addr(&h), "ws-silent");
    let _tcp = MqttClient::connect(h.local_addr(), "ws-silent", V5).unwrap();
    assert_eq!(ws.packet_of(T::Disconnect)[0], 0x8E);
    let (f, _) = ws.frame().unwrap();
    assert_eq!(f.opcode, Opcode::Close);
    // The server set its deadline before this point.
    let t0 = Instant::now();
    assert!(ws.frame().is_none(), "the server's direction ends");
    // The client sends no Close frame: the server waits for it up to
    // CLOSE_WAIT (RFC 6455 §7.1.1) ...
    sleep_until(t0 + CLOSE_WAIT - Duration::from_millis(600));
    assert!(
        !ws.server_stopped_reading(),
        "the server waits for the client's Close frame until CLOSE_WAIT"
    );
    // ... and closes the TCP connection then.
    sleep_until(t0 + CLOSE_WAIT + Duration::from_millis(400));
    assert!(
        ws.server_stopped_reading(),
        "the server closes the TCP connection when CLOSE_WAIT has passed"
    );
    h.shutdown();
}

#[test]
fn mqtt5_6_the_server_answers_a_ping_with_a_pong_of_the_same_data() {
    let h = start();
    let mut ws = Ws::connected(ws_addr(&h), "ws-ping");
    ws.send(Opcode::Ping, true, b"are you there");
    let (f, masked) = ws.frame().unwrap();
    assert_eq!(
        (f.opcode, f.payload.as_slice()),
        (Opcode::Pong, &b"are you there"[..])
    );
    assert!(!masked);
    // An unsolicited Pong is no error (RFC 6455 §5.5.3).
    ws.send(Opcode::Pong, true, b"heartbeat");
    ws.binary(&pingreq());
    ws.packet_of(T::PingResp);
    h.shutdown();
}

fn url_cfg(id: &str) -> DaemonConfig {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.client_id = id.to_string();
    cfg.clean_start = true;
    cfg.broker_tls_server_name = "localhost".to_string();
    cfg
}

/// The next PUBLISH on `topic` within [`TO`].
fn next_publish(client: &mut UrlClient, topic: &str) -> Vec<u8> {
    let deadline = Instant::now() + TO;
    while Instant::now() < deadline {
        match client.next_event().expect("next_event") {
            Some(InboundEvent::Publish {
                topic: t, payload, ..
            }) if t == topic => return payload,
            Some(InboundEvent::Disconnected(why)) => panic!("disconnected: {why}"),
            _ => {}
        }
    }
    panic!("no PUBLISH on {topic} within {TO:?}");
}

/// A message from `a` to a TCP subscriber and one from a TCP publisher to
/// `a`, each of them short and of 70 000 octets.
fn round_trip(h: &ServerHandle, a: &mut UrlClient, tag: &str) {
    let inbound = format!("{tag}/in");
    let outbound = format!("{tag}/out");
    a.subscribe(&[SubscribeEntry {
        filter: inbound.clone(),
        qos: 0,
        id: 1,
    }])
    .unwrap();
    let mut sub = tcp_subscriber(h, &format!("{tag}-tcp-sub"), &outbound);
    let mut pubr = MqttClient::connect(h.local_addr(), &format!("{tag}-tcp-pub"), V311).unwrap();
    // The SUBACK of `a` is taken by `next_event`; wait for the subscription.
    thread::sleep(Duration::from_millis(200));
    for payload in [b"hello".to_vec(), big_payload()] {
        pubr.publish(&inbound, &payload, 0, false).unwrap();
        assert!(next_publish(a, &inbound) == payload, "{tag}: TCP -> client");
        a.publish(&outbound, &payload, 0, false, &Default::default())
            .unwrap();
        let (topic, got) = sub.recv_publish(TO).unwrap();
        assert_eq!(topic, outbound);
        assert!(got == payload, "{tag}: client -> TCP");
    }
}

#[test]
fn mqtt5_6_a_message_crosses_between_the_tcp_and_the_websocket_listener() {
    let h = start();
    let url = parse_broker_url(&format!("ws://{}/mqtt", ws_addr(&h))).unwrap();
    let mut a = UrlClient::connect_url(&url, &url_cfg("url-ws"), None).expect("CONNECT over ws");
    round_trip(&h, &mut a, "ws");
    a.graceful_disconnect();
    h.shutdown();
}

fn tls_server(ck: &rcgen::CertifiedKey, alpn: &[u8]) -> Arc<rustls::ServerConfig> {
    let mut cfg = rustls::ServerConfig::builder_with_provider(Arc::new(
        zerodds_bridge_security::tls_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        vec![ck.cert.der().clone()],
        PrivatePkcs8KeyDer::from(ck.key_pair.serialize_der()).into(),
    )
    .unwrap();
    cfg.alpn_protocols = vec![alpn.to_vec()];
    Arc::new(cfg)
}

fn trusting(cert: &CertificateDer<'static>) -> Arc<rustls::ClientConfig> {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(cert.clone()).unwrap();
    Arc::new(
        rustls::ClientConfig::builder_with_provider(Arc::new(
            zerodds_bridge_security::tls_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth(),
    )
}

#[test]
fn mqtt5_6_the_zerodds_client_publishes_and_subscribes_over_wss_and_mqtts() {
    let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let h = MqttBrokerServer::bind_with("127.0.0.1:0", ServerConfig::default())
        .unwrap()
        .with_secure_websocket("127.0.0.1:0", tls_server(&ck, b"http/1.1"))
        .unwrap()
        .with_tls("127.0.0.1:0", tls_server(&ck, b"mqtt"))
        .unwrap()
        .spawn()
        .unwrap();
    let wss = h.transport_addr(Transport::SecureWebSocket).unwrap();
    let mqtts = h.transport_addr(Transport::Tls).unwrap();
    assert_eq!(h.transport_addr(Transport::WebSocket), None);
    let trust = trusting(ck.cert.der());

    let url = parse_broker_url(&format!("wss://127.0.0.1:{}/mqtt", wss.port())).unwrap();
    let mut a = UrlClient::connect_url(&url, &url_cfg("url-wss"), Some(Arc::clone(&trust)))
        .expect("CONNECT over wss");
    round_trip(&h, &mut a, "wss");
    let url = parse_broker_url(&format!("mqtts://127.0.0.1:{}", mqtts.port())).unwrap();
    let mut b = UrlClient::connect_url(&url, &url_cfg("url-mqtts"), Some(Arc::clone(&trust)))
        .expect("CONNECT over mqtts");
    round_trip(&h, &mut b, "mqtts");
    // wss to mqtts directly.
    b.subscribe(&[SubscribeEntry {
        filter: "cross/t".to_string(),
        qos: 0,
        id: 1,
    }])
    .unwrap();
    thread::sleep(Duration::from_millis(200));
    a.publish("cross/t", b"wss to mqtts", 0, false, &Default::default())
        .unwrap();
    assert_eq!(next_publish(&mut b, "cross/t"), b"wss to mqtts");
    a.graceful_disconnect();
    b.graceful_disconnect();
    assert_eq!(h.handshake_failures(), 0);

    // Gegenprobe: a client that does not trust the certificate gets no
    // connection, and the server counts the failed handshake.
    let other = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let url = parse_broker_url(&format!("wss://127.0.0.1:{}/mqtt", wss.port())).unwrap();
    assert!(
        UrlClient::connect_url(
            &url,
            &url_cfg("url-untrusted"),
            Some(trusting(other.cert.der()))
        )
        .is_err()
    );
    handshake_failures_reach(&h, 1);
    h.shutdown();
}
