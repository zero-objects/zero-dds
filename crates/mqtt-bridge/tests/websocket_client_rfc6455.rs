// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The WebSocket client of the `ws://` broker scheme
//! (`src/daemon/ws_transport.rs`) held against RFC 6455 §10.3, §10.4 and
//! §10.7: a masking key of its own for every frame it sends, a frame of the
//! server over its limit refused from the header, invalid frames of the
//! server refused, the Close frame of the server validated and answered
//! (§5.5.1, §7.4).
//!
//! The server is a stub in this process. It takes the opening handshake
//! with the handshake functions of `zerodds-websocket-bridge`, answers the
//! MQTT CONNECT with a CONNACK, and then writes the octets a test gives it,
//! invalid ones by hand. It records every frame the client sends with its
//! mask bit and masking key.
//!
//! Behind invalid data the client fails the connection (RFC 6455 §7.1.7):
//! the tests require its reason for the lost connection to name the
//! violation, one Close frame with the status code of the violation
//! (§7.4.1), and the network connection shut down by the client, all of it
//! before the stub would give up waiting.

#![cfg(feature = "daemon")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use zerodds_mqtt_bridge::daemon::client::{InboundEvent, MqttClient};
use zerodds_mqtt_bridge::daemon::config::{BrokerUrl, DaemonConfig, parse_broker_url};
use zerodds_websocket_bridge::codec::{decode, encode, peek_header};
use zerodds_websocket_bridge::frame::{Frame, Opcode};
use zerodds_websocket_bridge::handshake::{
    build_server_response, parse_client_request, render_server_response,
};

/// One frame the client sent.
#[derive(Debug)]
struct Seen {
    masked: bool,
    key: [u8; 4],
    frame: Frame,
}

/// How long the stub waits for octets of the client before it gives up.
const STUB_PATIENCE: Duration = Duration::from_secs(5);

/// The client reports invalid data well inside the stub's patience: the
/// report is then the client's own and not the answer to the stub ending
/// the connection.
const CLIENT_REPORTS_WITHIN: Duration = Duration::from_secs(3);

/// The stub's side of the connection.
struct Conn {
    stream: TcpStream,
    buf: Vec<u8>,
    seen: Vec<Seen>,
    /// The stub gave up waiting: the client neither sent octets nor ended
    /// the connection for [`STUB_PATIENCE`].
    gave_up: bool,
}

/// What the stub saw of one connection.
struct Report {
    /// Every frame of the client.
    seen: Vec<Seen>,
    /// How many of them came before the script of the test ended.
    before: usize,
    /// The stub ended the connection because the client sent nothing more
    /// and kept it open; `false`: the client ended it.
    gave_up: bool,
    /// The subprotocols the opening handshake of the client offered.
    protocols: Vec<String>,
}

impl Report {
    /// The frames the client sent behind the script of the test.
    fn after(&self) -> &[Seen] {
        &self.seen[self.before..]
    }
}

impl Conn {
    /// Reads the next frame of the client into `seen`; `false` when the
    /// client ended the connection or sent nothing for [`STUB_PATIENCE`]
    /// (`gave_up`).
    fn next(&mut self) -> bool {
        let mut chunk = [0u8; 4096];
        loop {
            match decode(&self.buf) {
                Ok((frame, used)) => {
                    let header = peek_header(&self.buf).expect("the header of a decoded frame");
                    let mut key = [0u8; 4];
                    if header.masked {
                        key.copy_from_slice(&self.buf[header.header_len - 4..header.header_len]);
                    }
                    self.buf.drain(..used);
                    self.seen.push(Seen {
                        masked: header.masked,
                        key,
                        frame,
                    });
                    return true;
                }
                Err(e) if e.is_incomplete() => match self.stream.read(&mut chunk) {
                    Ok(0) => return false,
                    Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
                    Err(e) => {
                        self.gave_up = matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                        );
                        return false;
                    }
                },
                Err(e) => panic!("the client sent an invalid frame: {e}"),
            }
        }
    }

    fn send(&mut self, wire: &[u8]) {
        self.stream.write_all(wire).expect("the stub sends");
    }

    /// Reads on until the client sent a frame with `opcode`.
    fn until(&mut self, opcode: Opcode) -> &Seen {
        loop {
            assert!(self.next(), "the client sent no {opcode:?} frame");
            if self.seen.last().is_some_and(|s| s.frame.opcode == opcode) {
                return self.seen.last().expect("a frame");
            }
        }
    }
}

/// Starts the stub: one connection, the opening handshake with the
/// subprotocol `mqtt`, the CONNECT answered with a CONNACK, then `script`,
/// then every frame the client still sends until it ends the connection.
/// Returns the URL and the thread, which hands back what it saw.
fn stub(script: impl FnOnce(&mut Conn) + Send + 'static) -> (BrokerUrl, JoinHandle<Report>) {
    stub_answering(
        |conn, level| {
            // CONNACK: session present 0, reason 0 (and no properties in
            // 5.0), in one binary frame.
            conn.send(&encode(&Frame::binary(connack(level))).expect("encode"));
        },
        script,
    )
}

/// The CONNACK of the stub for the protocol `level`: session present 0,
/// reason 0, and in 5.0 no properties.
fn connack(level: u8) -> Vec<u8> {
    if level == 5 {
        vec![0x20, 0x03, 0x00, 0x00, 0x00]
    } else {
        vec![0x20, 0x02, 0x00, 0x00]
    }
}

/// Like [`stub`], but `answer` sends the CONNACK, in whatever frames it
/// likes; it gets the protocol level of the CONNECT.
fn stub_answering(
    answer: impl FnOnce(&mut Conn, u8) + Send + 'static,
    script: impl FnOnce(&mut Conn) + Send + 'static,
) -> (BrokerUrl, JoinHandle<Report>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("local_addr").port();
    let join = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        stream
            .set_read_timeout(Some(STUB_PATIENCE))
            .expect("timeout");
        let mut head = Vec::new();
        let mut octet = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            assert_eq!(stream.read(&mut octet).expect("the request"), 1);
            head.push(octet[0]);
        }
        let request =
            parse_client_request(std::str::from_utf8(&head).expect("an HTTP head")).expect("valid");
        let protocols = request.protocols.clone();
        assert!(protocols.iter().any(|p| p == "mqtt"));
        let mut response = build_server_response(&request);
        response.protocol = Some("mqtt".to_string());
        stream
            .write_all(render_server_response(&response).as_bytes())
            .expect("the response");
        let mut conn = Conn {
            stream,
            buf: Vec::new(),
            seen: Vec::new(),
            gave_up: false,
        };
        // The CONNECT packet, over as many frames as the client uses:
        // type 1, the remaining length, `MQTT` and the protocol level.
        let mut connect = Vec::new();
        let level = loop {
            assert!(conn.next(), "the client sent no CONNECT");
            connect.extend_from_slice(&conn.seen.last().expect("a frame").frame.payload);
            if let Some(at) = connect.windows(4).position(|w| w == b"MQTT") {
                // The remaining length is a variable byte integer: the
                // CONNECT with its will (spec §6) is over 127 octets.
                let complete = connect.get(1..).is_some_and(|rest| {
                    zerodds_mqtt_bridge::vbi::decode_vbi(rest)
                        .is_ok_and(|(len, n)| connect.len() >= 1 + n + len as usize)
                });
                if complete {
                    break connect[at + 4];
                }
            }
        };
        assert_eq!(connect[0], 0x10, "a CONNECT packet");
        answer(&mut conn, level);
        script(&mut conn);
        let before = conn.seen.len();
        while conn.next() {}
        Report {
            seen: conn.seen,
            before,
            gave_up: conn.gave_up,
            protocols,
        }
    });
    let url = parse_broker_url(&format!("ws://127.0.0.1:{port}/mqtt")).expect("a ws URL");
    (url, join)
}

fn config(id: &str) -> DaemonConfig {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.client_id = id.to_string();
    cfg.clean_start = true;
    cfg
}

/// The reason the client reports for the lost connection, and how long it
/// took to report it.
fn disconnected(client: &mut MqttClient) -> (String, Duration) {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(10);
    while Instant::now() < deadline {
        match client.next_event().expect("next_event") {
            Some(InboundEvent::Disconnected(why)) => return (why, started.elapsed()),
            Some(other) => panic!("expected the end of the connection, got {other:?}"),
            None => {}
        }
    }
    panic!("the client kept the connection");
}

/// The body of the one frame the client sent behind the script of the
/// test, which is a masked Close frame.
fn the_close_frame<'a>(report: &'a Report, what: &str) -> &'a [u8] {
    let after = report.after();
    assert_eq!(after.len(), 1, "{what}: one Close frame, got {after:?}");
    let seen = &after[0];
    assert!(seen.masked, "{what}: an unmasked frame of the client");
    assert_eq!(seen.frame.opcode, Opcode::Close, "{what}: {seen:?}");
    &seen.frame.payload
}

/// One connection the server makes invalid with `wire`: a Ping the client
/// answers shows the connection works, then `wire` ends it. The client
/// fails the connection (RFC 6455 §7.1.7, §10.7): its reason names the
/// violation with `reason`, it sends one Close frame with `status`
/// (§7.4.1) and shuts the network connection down, and it does so on its
/// own, before the stub would give up.
fn the_client_fails_the_connection(what: &str, wire: Vec<u8>, reason: &str, status: u16) {
    let (url, stub) = stub(move |conn| {
        conn.send(&encode(&Frame::ping(b"alive".to_vec())).expect("encode"));
        let pong = conn.until(Opcode::Pong);
        assert!(pong.masked);
        assert_eq!(pong.frame.payload, b"alive");
        conn.send(&wire);
    });
    let mut client = MqttClient::connect_url(&url, &config("ws-invalid"), None)
        .unwrap_or_else(|e| panic!("{what}: CONNECT: {e}"));
    let (why, took) = disconnected(&mut client);
    assert!(why.contains(reason), "{what}: the client reports: {why}");
    assert!(
        !why.contains("connection closed"),
        "{what}: the client took the octets and saw the server leave: {why}"
    );
    assert!(
        took < CLIENT_REPORTS_WITHIN,
        "{what}: reported after {took:?}"
    );
    // The client object lives on: the connection ends because the client
    // shut it down, not because it was dropped.
    let report = stub.join().expect("the stub");
    assert!(
        !report.gave_up,
        "{what}: the client left the network connection open"
    );
    let body = the_close_frame(&report, what);
    assert!(body.len() >= 2, "{what}: a Close frame without status");
    assert_eq!(u16::from_be_bytes([body[0], body[1]]), status, "{what}");
    assert!(
        std::str::from_utf8(&body[2..]).is_ok(),
        "{what}: the reason of the Close frame is not UTF-8"
    );
    drop(client);
}

/// The wire octets of a Close frame of the server with `body`.
fn server_close(body: &[u8]) -> Vec<u8> {
    let mut wire = vec![0x88, u8::try_from(body.len()).expect("a control frame")];
    wire.extend_from_slice(body);
    wire
}

#[test]
fn ws_10_3_every_frame_of_the_mqtt_client_carries_a_key_of_its_own() {
    // "Clients MUST choose a new masking key for each frame" (§10.3), and
    // every frame of a client is masked (§5.1). The CONNECT, a hundred
    // PUBLISH packets, the DISCONNECT and the Close frame.
    let (url, stub) = stub(|_| {});
    let mut client = MqttClient::connect_url(&url, &config("ws-keys"), None).expect("CONNECT");
    for n in 0..100u32 {
        client
            .publish(
                "zerodds/ws/keys",
                &n.to_be_bytes(),
                0,
                false,
                &Default::default(),
            )
            .expect("publish");
    }
    client.graceful_disconnect();
    let seen = stub.join().expect("the stub").seen;
    assert!(seen.len() >= 102, "{} frames", seen.len());
    assert!(seen.iter().all(|s| s.masked), "an unmasked frame");
    let keys: HashSet<[u8; 4]> = seen.iter().map(|s| s.key).collect();
    assert_eq!(keys.len(), seen.len(), "a masking key was used twice");
    // The frames are what the client meant to send: MQTT packets in
    // binary frames, and the Close frame last.
    let publishes = seen
        .iter()
        .filter(|s| s.frame.opcode == Opcode::Binary && s.frame.payload.first() == Some(&0x30))
        .count();
    assert_eq!(publishes, 100);
    assert!(
        seen.iter()
            .any(|s| s.frame.opcode == Opcode::Binary && s.frame.payload.first() == Some(&0xe0)),
        "no DISCONNECT packet"
    );
    assert_eq!(seen.last().expect("a frame").frame.opcode, Opcode::Close);
}

#[test]
fn ws_10_4_the_mqtt_client_refuses_a_frame_over_its_limit_from_the_header() {
    // Ten octets: the header of a binary frame that announces 2^40 octets.
    // The client refuses it without waiting for payload that never comes,
    // with the status code 1009 (§7.4.1).
    let mut header = vec![0x82u8, 127];
    header.extend_from_slice(&(1u64 << 40).to_be_bytes());
    the_client_fails_the_connection(
        "a frame over the limit",
        header,
        "announces more than",
        1009,
    );
}

#[test]
fn ws_10_7_the_mqtt_client_refuses_invalid_frames_of_the_server() {
    // "Incoming data MUST always be validated by both clients and servers"
    // (§10.7). Each case is a connection of its own. The payloads are a
    // PINGRESP, which the client would read and pass over if it took the
    // frame: the connection would go on, and the case fails on the reason,
    // the time and the missing Close frame.
    let masked = encode(&Frame::binary(vec![0xd0, 0x00]).with_mask([1, 2, 3, 4])).expect("encode");
    let mut long_ping = vec![0x89u8, 126, 0x00, 126];
    long_ping.extend_from_slice(&[0u8; 126]);
    let cases: Vec<(&str, Vec<u8>, &str, u16)> = vec![
        (
            "a masked frame of the server (§5.1)",
            masked,
            "masked frame",
            1002,
        ),
        (
            "RSV1 without an extension (§5.2)",
            vec![0xc2, 0x02, 0xd0, 0x00],
            "RSV bit",
            1002,
        ),
        ("RSV2 (§5.2)", vec![0xa2, 0x02, 0xd0, 0x00], "RSV bit", 1002),
        ("RSV3 (§5.2)", vec![0x92, 0x02, 0xd0, 0x00], "RSV bit", 1002),
        (
            "a reserved opcode (§5.2)",
            vec![0x83, 0x00],
            "reserved opcode",
            1002,
        ),
        (
            "a length that is not in its shortest form (§5.2)",
            vec![0x82, 126, 0x00, 0x02, 0xd0, 0x00],
            "non-minimal",
            1002,
        ),
        (
            "a 64-bit length with its most significant bit set (§5.2)",
            vec![0x82, 127, 0x80, 0, 0, 0, 0, 0, 0, 0],
            "MSB",
            1002,
        ),
        (
            "a fragmented control frame (§5.5)",
            vec![0x09, 0x00],
            "FIN=0",
            1002,
        ),
        (
            "a control frame over 125 octets (§5.5)",
            long_ping,
            "125",
            1002,
        ),
        (
            "a continuation frame without a message (§5.4)",
            vec![0x80, 0x02, 0xd0, 0x00],
            "continuation frame outside a message",
            1002,
        ),
        (
            "a new data frame inside a fragmented message (§5.4)",
            vec![0x02, 0x00, 0x82, 0x02, 0xd0, 0x00],
            "inside an unfinished message",
            1002,
        ),
        // MQTT 5.0 §6 [MQTT-6.0.0-1]: any data frame that is not binary.
        ("a text frame", vec![0x81, 0x00], "a text frame", 1003),
    ];
    for (what, wire, reason, status) in cases {
        the_client_fails_the_connection(what, wire, reason, status);
    }
}

#[test]
fn ws_10_7_the_mqtt_client_refuses_an_invalid_close_frame_of_the_server() {
    // §5.5.1: "If there is a body, the first two bytes of the body MUST be
    // a 2-byte unsigned integer"; the reason is "UTF-8-encoded data".
    // §7.4.1: 1005, 1006 and 1015 "MUST NOT be set as a status code in a
    // Close control frame by an endpoint"; §7.4.2: 0 to 999 are not used,
    // 1000 to 2999 are the protocol's, and what nothing defines there
    // (1004, 2999) is no status code; nothing is defined from 5000 on.
    the_client_fails_the_connection(
        "a Close body of one octet",
        server_close(&[0x03]),
        "close body of one octet",
        1002,
    );
    for code in [0u16, 999, 1004, 1005, 1006, 1015, 1016, 2999, 5000, 65535] {
        the_client_fails_the_connection(
            &format!("the Close status code {code}"),
            server_close(&code.to_be_bytes()),
            &format!("close status code {code} is not allowed"),
            1002,
        );
    }
    // 1000 and a reason that ends inside a UTF-8 sequence.
    the_client_fails_the_connection(
        "a Close reason that is not UTF-8",
        server_close(&[0x03, 0xe8, 0xce, 0xba, 0xe1, 0xbd]),
        "close reason is not UTF-8",
        1007,
    );
}

#[test]
fn ws_5_5_1_the_mqtt_client_answers_a_close_frame_of_the_server_with_its_status() {
    // §5.5.1: "If an endpoint receives a Close frame and did not previously
    // send a Close frame, the endpoint MUST send a Close frame in
    // response", "typically echoing the status code it received". A Close
    // frame without a body is answered with one without a body. The client
    // reports the closed connection and not invalid data.
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("no body", Vec::new()),
        ("1000", 1000u16.to_be_bytes().to_vec()),
        ("1001 with a reason", b"\x03\xe9going away".to_vec()),
        ("1011", 1011u16.to_be_bytes().to_vec()),
        ("3000, a library code", 3000u16.to_be_bytes().to_vec()),
        ("4999, an application code", 4999u16.to_be_bytes().to_vec()),
    ];
    for (what, body) in cases {
        let wire = server_close(&body);
        let (url, stub) = stub(move |conn| conn.send(&wire));
        let mut client = MqttClient::connect_url(&url, &config("ws-close"), None)
            .unwrap_or_else(|e| panic!("{what}: CONNECT: {e}"));
        let (why, took) = disconnected(&mut client);
        assert!(why.contains("connection closed"), "{what}: {why}");
        assert!(took < CLIENT_REPORTS_WITHIN, "{what}: after {took:?}");
        // §7.1.1: the server closes the TCP connection first; the stub
        // reads until this end is gone.
        drop(client);
        let report = stub.join().expect("the stub");
        let answer = the_close_frame(&report, what);
        assert_eq!(answer, &body[..body.len().min(2)], "{what}");
    }
}

/// A PUBLISH at QoS 0 in MQTT 5.0 octets: `topic`, no properties,
/// `payload`.
fn publish5(topic: &str, payload: &[u8]) -> Vec<u8> {
    zerodds_mqtt_bridge::codec::encode_publish(&zerodds_mqtt_bridge::codec::PublishPacket {
        dup: false,
        qos: 0,
        retain: false,
        topic: topic.to_string(),
        packet_id: None,
        properties: Vec::new(),
        payload: payload.to_vec(),
    })
    .expect("encode PUBLISH")
}

/// One unmasked data frame of the server: `opcode`, FIN as `fin`.
fn server_frame(opcode: Opcode, fin: bool, payload: Vec<u8>) -> Vec<u8> {
    let mut frame = Frame::binary(payload);
    frame.opcode = opcode;
    frame.fin = fin;
    encode(&frame).expect("encode")
}

/// The next PUBLISH the client reports as `(topic, payload)`.
fn next_publish(client: &mut MqttClient) -> (String, Vec<u8>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        match client.next_event().expect("next_event") {
            Some(InboundEvent::Publish { topic, payload, .. }) => return (topic, payload),
            Some(other) => panic!("expected a PUBLISH, got {other:?}"),
            None => {}
        }
    }
    panic!("no PUBLISH arrived");
}

#[test]
fn mqtt5_6_0_0_1_a_text_frame_from_the_server_closes_the_network_connection() {
    // [MQTT-6.0.0-1]: "If any other type of data frame is received the
    // recipient MUST close the Network Connection." A text frame that
    // carries a valid PINGRESP, an empty one, and the first fragment of a
    // text message: each time the client reports the text frame, sends a
    // Close frame with 1003 and shuts the connection down.
    for (what, wire) in [
        (
            "a text frame carrying a PINGRESP",
            server_frame(Opcode::Text, true, vec![0xd0, 0x00]),
        ),
        ("an empty text frame", vec![0x81, 0x00]),
        (
            "the first fragment of a text message",
            server_frame(Opcode::Text, false, vec![0xd0]),
        ),
    ] {
        the_client_fails_the_connection(what, wire, "a text frame", 1003);
    }
}

#[test]
fn mqtt5_6_0_0_2_a_control_packet_split_over_frames_is_read_as_one() {
    // [MQTT-6.0.0-2]: "The receiver MUST NOT assume that MQTT Control
    // Packets are aligned on WebSocket frame boundaries." The CONNACK comes
    // in two binary frames, cut inside its fixed header; a PUBLISH comes as
    // a fragmented binary message with a Ping between the fragments, cut
    // inside the topic; the next PUBLISH is cut into one frame per octet.
    let first = publish5("split/topic", b"one");
    let second = publish5("bytes", b"two");
    let (url, stub) = stub_answering(
        |conn, level| {
            assert_eq!(level, 5);
            let ack = connack(level);
            conn.send(&server_frame(Opcode::Binary, true, ack[..1].to_vec()));
            conn.send(&server_frame(Opcode::Binary, true, ack[1..].to_vec()));
        },
        move |conn| {
            conn.send(&server_frame(Opcode::Binary, false, first[..6].to_vec()));
            conn.send(&encode(&Frame::ping(b"mid".to_vec())).expect("encode"));
            conn.send(&server_frame(
                Opcode::Continuation,
                true,
                first[6..].to_vec(),
            ));
            let pong = conn.until(Opcode::Pong);
            assert_eq!(pong.frame.payload, b"mid");
            for octet in &second {
                conn.send(&server_frame(Opcode::Binary, true, vec![*octet]));
            }
        },
    );
    let mut client = MqttClient::connect_url(&url, &config("ws-split"), None)
        .expect("a CONNACK over two frames");
    assert_eq!(
        next_publish(&mut client),
        ("split/topic".into(), b"one".to_vec())
    );
    assert_eq!(next_publish(&mut client), ("bytes".into(), b"two".to_vec()));
    drop(client);
    let report = stub.join().expect("the stub");
    assert!(!report.gave_up);
}

#[test]
fn mqtt5_6_0_0_2_two_control_packets_in_one_frame_are_both_read() {
    // [MQTT-6.0.0-2], the other way round: the CONNACK and a PUBLISH share
    // one binary frame, and the next frame carries two PUBLISH packets and
    // the first octets of a third, whose rest follows in a frame of its own.
    let (url, stub) = stub_answering(
        |conn, level| {
            let mut wire = connack(level);
            wire.extend_from_slice(&publish5("a", b"1"));
            conn.send(&server_frame(Opcode::Binary, true, wire));
        },
        |conn| {
            let mut wire = publish5("b", b"2");
            wire.extend_from_slice(&publish5("c", b"3"));
            let third = publish5("d", b"4");
            wire.extend_from_slice(&third[..3]);
            conn.send(&server_frame(Opcode::Binary, true, wire));
            conn.send(&server_frame(Opcode::Binary, true, third[3..].to_vec()));
        },
    );
    let mut client = MqttClient::connect_url(&url, &config("ws-packed"), None)
        .expect("a CONNACK sharing its frame");
    for (topic, payload) in [("a", b"1"), ("b", b"2"), ("c", b"3"), ("d", b"4")] {
        assert_eq!(next_publish(&mut client), (topic.into(), payload.to_vec()));
    }
    drop(client);
    stub.join().expect("the stub");
}

#[test]
fn mqtt5_6_0_0_3_the_client_offers_the_subprotocol_mqtt() {
    // [MQTT-6.0.0-3]: "The Client MUST include mqtt in the list of
    // WebSocket Sub Protocols it offers." The stub records the
    // Sec-WebSocket-Protocol list of the opening handshake.
    let (url, stub) = stub(|_| {});
    let client = MqttClient::connect_url(&url, &config("ws-offer"), None).expect("CONNECT");
    drop(client);
    let report = stub.join().expect("the stub");
    assert_eq!(report.protocols, vec!["mqtt".to_string()]);
}

#[test]
fn mqtt5_6_0_0_4_a_server_that_selects_no_or_another_subprotocol_is_refused() {
    // [MQTT-6.0.0-4]: "The WebSocket Subprotocol name selected and returned
    // by the Server MUST be mqtt." A 101 answer without a subprotocol and
    // one that selects `mqttv3.1` are refused: the connect fails naming the
    // subprotocol, and the client sends no CONNECT over the connection.
    for selected in [None, Some("mqttv3.1")] {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("local_addr").port();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            stream
                .set_read_timeout(Some(STUB_PATIENCE))
                .expect("timeout");
            let mut head = Vec::new();
            let mut octet = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n") {
                assert_eq!(stream.read(&mut octet).expect("the request"), 1);
                head.push(octet[0]);
            }
            let request =
                parse_client_request(std::str::from_utf8(&head).expect("HTTP")).expect("valid");
            let mut response = build_server_response(&request);
            response.protocol = selected.map(str::to_string);
            stream
                .write_all(render_server_response(&response).as_bytes())
                .expect("the response");
            // Everything the client sends behind the response, up to the
            // end of the connection.
            let mut rest = Vec::new();
            let _ = stream.read_to_end(&mut rest);
            rest
        });
        let url = parse_broker_url(&format!("ws://127.0.0.1:{port}/mqtt")).expect("a ws URL");
        let err = match MqttClient::connect_url(&url, &config("ws-select"), None) {
            Ok(_) => panic!("{selected:?}: the client took the handshake"),
            Err(e) => e.to_string(),
        };
        assert!(
            err.to_lowercase().contains("protocol"),
            "{selected:?}: {err}"
        );
        let rest = server.join().expect("the server");
        assert!(rest.is_empty(), "{selected:?}: the client sent {rest:?}");
    }
}
