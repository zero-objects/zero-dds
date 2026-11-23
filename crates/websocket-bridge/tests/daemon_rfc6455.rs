// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-ws-bridged` as an RFC 6455 server: what the daemon answers to a
//! client that does not keep to the protocol, and to one that does in the
//! ways the daemon has to follow (fragments, the closing handshake,
//! `permessage-deflate`), and how the daemon closes its connections when it
//! shuts down.
//!
//! The daemon runs in this process (`daemon::server::start`), in one test
//! as the binary that gets a SIGTERM; the client side writes its octets by
//! hand where they are invalid and reads the daemon's frames through the
//! library's client receiver, which refuses a masked or otherwise invalid
//! frame of the server.

#![cfg(feature = "daemon")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::uninlined_format_args,
    missing_docs
)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use zerodds_websocket_bridge::codec::encode;
use zerodds_websocket_bridge::connection::{Event, Limits, Receiver, Sender};
use zerodds_websocket_bridge::daemon::config::{DaemonConfig, TopicConfig};
use zerodds_websocket_bridge::daemon::runtime_common::{BridgeMetrics, family};
use zerodds_websocket_bridge::daemon::server::{self, DaemonHandle};
use zerodds_websocket_bridge::frame::{Frame, Opcode};
use zerodds_websocket_bridge::masking::Role;
use zerodds_websocket_bridge::message::Message;
use zerodds_websocket_bridge::permessage_deflate::{DeflateOffer, accept_response, render_offer};

const KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";
const MASK: [u8; 4] = [0x37, 0xfa, 0x21, 0x3d];

/// The daemon on a leased DDS domain with one topic, `TradeRfc`, whose
/// type the JSON text frames know.
fn start() -> (DaemonHandle, zerodds_test_domains::DomainLease) {
    start_with(|_| {})
}

/// [`start`] with a change to the configuration.
fn start_with(
    change: impl FnOnce(&mut DaemonConfig),
) -> (DaemonHandle, zerodds_test_domains::DomainLease) {
    let idl = std::env::temp_dir().join(format!("zerodds-ws-rfc-{}.idl", std::process::id()));
    std::fs::write(&idl, "@final struct TradeRfc { string sym; long seq; };")
        .expect("the IDL file");
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.listen = "127.0.0.1:0".to_string();
    let lease = zerodds_test_domains::lease_domain();
    cfg.domain = lease.id();
    cfg.metrics_enabled = true;
    cfg.metrics_addr = "127.0.0.1:0".to_string();
    cfg.topics.push(TopicConfig {
        name: "TradeRfc".to_string(),
        type_name: "TradeRfc".to_string(),
        direction: "bidir".to_string(),
        ws_path: "/topics/trade".to_string(),
        reliability: "reliable".to_string(),
        durability: "volatile".to_string(),
        history_depth: 10,
        idl_file: idl.display().to_string(),
        ..TopicConfig::default()
    });
    change(&mut cfg);
    (server::start(cfg).expect("daemon start"), lease)
}

/// The rest of an HTTP error response behind its head: the body, as long
/// as `Content-Length` says, and then the end of the connection.
fn body_and_end(stream: &mut TcpStream, head: &str) -> String {
    let length: usize = head
        .lines()
        .find_map(|l| l.strip_prefix("Content-Length: "))
        .expect("Content-Length")
        .parse()
        .expect("a length");
    let mut rest = Vec::new();
    stream.read_to_end(&mut rest).expect("the daemon closes");
    assert_eq!(rest.len(), length, "{head}");
    String::from_utf8(rest).expect("a text body")
}

/// The ping of §4.4 as JSON text of `len` octets.
fn ping_of(len: usize) -> Vec<u8> {
    let mut text = b"{\"op\":\"ping\"".to_vec();
    text.resize(len - 1, b' ');
    text.push(b'}');
    text
}

fn request(extra: &str) -> String {
    format!(
        "GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Key: {KEY}\r\nSec-WebSocket-Version: 13\r\n\
         Sec-WebSocket-Protocol: zerodds-ws-bridge-1.0+json\r\n{extra}\r\n"
    )
}

/// Sends `request` and returns the response head, read octet by octet so
/// that no frame behind it is taken.
fn handshake(addr: &str, request: &[u8]) -> (TcpStream, String) {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    stream.write_all(request).expect("send the request");
    let mut head = Vec::new();
    let mut octet = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        match stream.read(&mut octet) {
            Ok(1) => head.push(octet[0]),
            other => panic!("the response ended early ({other:?}): {head:?}"),
        }
    }
    (stream, String::from_utf8(head).expect("an HTTP head"))
}

/// A connection after a 101, without extensions.
fn open(addr: &str) -> Client {
    let (stream, head) = handshake(addr, request("").as_bytes());
    assert!(head.starts_with("HTTP/1.1 101 "), "{head}");
    assert!(
        !head
            .to_ascii_lowercase()
            .contains("sec-websocket-extensions")
    );
    Client {
        stream,
        receiver: Receiver::new(Role::Client, Limits::default()),
        buf: Vec::new(),
    }
}

/// The reading side of the test's client.
struct Client {
    stream: TcpStream,
    receiver: Receiver,
    buf: Vec<u8>,
}

impl Client {
    fn send(&mut self, wire: &[u8]) {
        self.stream.write_all(wire).expect("send");
    }

    /// The next event the daemon sent; `None` when it closed the
    /// connection.
    fn next(&mut self) -> Option<Event> {
        let mut chunk = [0u8; 4096];
        loop {
            if let Some(event) = self
                .receiver
                .feed(&mut self.buf)
                .expect("the daemon's frames are valid")
            {
                return Some(event);
            }
            match self.stream.read(&mut chunk) {
                Ok(0) => return None,
                Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
                Err(e) => panic!("no frame from the daemon: {e}"),
            }
        }
    }

    fn text(&mut self) -> String {
        match self.next() {
            Some(Event::Message(Message {
                is_text: true,
                payload,
            })) => String::from_utf8(payload).expect("UTF-8"),
            other => panic!("expected a text message, got {other:?}"),
        }
    }

    /// The daemon's next frame is a Close with `status` (`None`: without
    /// body), and the daemon closes the TCP connection behind it (RFC 6455
    /// §7.1.1).
    fn expect_close(&mut self, status: Option<u16>) {
        match self.next() {
            Some(Event::Close(body)) => assert_eq!(body.map(|(code, _)| code), status),
            other => panic!("expected a Close with {status:?}, got {other:?}"),
        }
        let mut chunk = [0u8; 64];
        match self.stream.read(&mut chunk) {
            Ok(0) => {}
            other => panic!("the daemon did not close the connection: {other:?}"),
        }
    }
}

fn masked(frame: Frame) -> Vec<u8> {
    encode(&frame.with_mask(MASK)).expect("encode")
}

fn part(opcode: Opcode, fin: bool, payload: &[u8]) -> Vec<u8> {
    masked(Frame {
        fin,
        opcode,
        ..Frame::binary(payload.to_vec())
    })
}

const PING: &str = "{\"op\":\"ping\"}";
/// The message of a shutdown (zerodds-ws-bridge-1.0 §9.2).
const SHUTDOWN: &str = "{\"op\":\"shutdown\"}";

/// zerodds-ws-bridge-1.0 §4.1, RFC 6455 §4.2.2: a client of raw TCP and a
/// hand-written request, as a browser sends it, gets 101 and the
/// `Sec-WebSocket-Accept` of its key (RFC 6455 §1.3, the example key and
/// its hash written out), on the path of a topic.
#[test]
fn wsbridge_4_1_a_raw_tcp_client_gets_101_and_the_accept_hash() {
    let (mut handle, _lease) = start();
    let req = format!(
        "GET /topics/trade HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\n\
         Connection: Upgrade\r\nSec-WebSocket-Key: {KEY}\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
    let (_stream, head) = handshake(&handle.local_addr, req.as_bytes());
    assert!(head.starts_with("HTTP/1.1 101 "), "{head}");
    assert!(
        head.contains("\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n"),
        "{head}"
    );
    handle.shutdown();
}

#[test]
fn ws_4_2_2_the_daemon_answers_an_unsupported_version_with_426_and_names_13() {
    let (mut handle, _lease) = start();
    let req = request("").replace("Sec-WebSocket-Version: 13", "Sec-WebSocket-Version: 8");
    let (_stream, head) = handshake(&handle.local_addr, req.as_bytes());
    assert!(
        head.starts_with("HTTP/1.1 426 Upgrade Required\r\n"),
        "{head}"
    );
    assert!(head.contains("\r\nSec-WebSocket-Version: 13\r\n"), "{head}");
    handle.shutdown();
}

#[test]
fn ws_4_2_1_the_daemon_answers_an_invalid_handshake_with_its_http_status() {
    let (mut handle, _lease) = start();
    let errors = handle.metrics.clone().expect("metrics").errors_total.get();
    let valid = request("");
    let cases: [(Vec<u8>, &str); 5] = [
        // Not a GET.
        (
            valid.replace("GET /", "POST /").into_bytes(),
            "405 Method Not Allowed",
        ),
        // Below HTTP/1.1.
        (
            valid.replace("HTTP/1.1", "HTTP/1.0").into_bytes(),
            "400 Bad Request",
        ),
        // A key that is not 16 octets.
        (
            valid.replace(KEY, "c2hvcnQ=").into_bytes(),
            "400 Bad Request",
        ),
        // No Upgrade.
        (
            valid.replace("Upgrade: websocket\r\n", "").into_bytes(),
            "400 Bad Request",
        ),
        // Octets that are no UTF-8.
        (
            {
                let mut req = valid.clone().into_bytes();
                let host = valid.find("localhost").expect("the Host value");
                req[host] = 0xff;
                req
            },
            "400 Bad Request",
        ),
    ];
    for (req, status) in &cases {
        let (mut stream, head) = handshake(&handle.local_addr, req);
        assert!(
            head.starts_with(&format!("HTTP/1.1 {status}\r\n")),
            "{status}: {head}"
        );
        if status.starts_with("405") {
            assert!(head.contains("\r\nAllow: GET\r\n"), "{head}");
        }
        // The daemon closes the connection behind the response's body.
        let mut rest = Vec::new();
        stream
            .read_to_end(&mut rest)
            .expect("the body, then the end");
    }
    let after = handle.metrics.clone().expect("metrics").errors_total.get();
    assert_eq!(after - errors, cases.len() as u64);
    handle.shutdown();
}

#[test]
fn ws_5_1_the_daemon_closes_an_unmasked_frame_with_1002() {
    // §5.1: "The server MUST close the connection upon receiving a frame
    // that is not masked." The frame is a ping of §4.4, which the daemon
    // would answer if it processed it.
    let (mut handle, _lease) = start();
    let mut client = open(&handle.local_addr);
    client.send(&encode(&Frame::text(PING)).expect("encode"));
    client.expect_close(Some(1002));
    let metrics = handle.metrics.clone().expect("metrics");
    assert_eq!(metrics.total(family::FRAMES_IN), 0);
    // The same frame masked is answered.
    let mut client = open(&handle.local_addr);
    client.send(&masked(Frame::text(PING)));
    assert_eq!(client.text(), "{\"op\":\"pong\"}");
    handle.shutdown();
}

#[test]
fn ws_5_2_the_daemon_closes_a_reserved_bit_a_reserved_opcode_and_a_wrong_length_with_1002() {
    let (mut handle, _lease) = start();
    let rsv = |bit: u8| {
        let mut wire = masked(Frame::text(PING));
        wire[0] |= bit;
        wire
    };
    let mut not_minimal = vec![0x81u8, 0xfe, 0x00, 0x0d];
    not_minimal.extend_from_slice(&masked(Frame::text(PING))[2..]);
    let cases: [(&str, Vec<u8>); 7] = [
        ("RSV1 without an extension", rsv(0x40)),
        ("RSV2", rsv(0x20)),
        ("RSV3", rsv(0x10)),
        ("opcode 0x3", part(Opcode::Reserved(0x3), true, b"")),
        ("opcode 0xb", part(Opcode::Reserved(0xb), true, b"")),
        // §5.2: "the minimal number of bytes MUST be used to encode the
        // length": 13 octets in the 16-bit form.
        ("a length that is not minimal", not_minimal),
        // §5.5: a control frame is not fragmented.
        ("a Ping without FIN", {
            let mut wire = masked(Frame::ping(Vec::new()));
            wire[0] &= 0x7f;
            wire
        }),
    ];
    for (what, wire) in cases {
        let mut client = open(&handle.local_addr);
        client.send(&wire);
        match client.next() {
            Some(Event::Close(Some((1002, _)))) => {}
            other => panic!("{what}: expected a Close with 1002, got {other:?}"),
        }
    }
    assert_eq!(
        handle
            .metrics
            .clone()
            .expect("metrics")
            .total(family::FRAMES_IN),
        0
    );
    handle.shutdown();
}

#[test]
fn ws_5_4_the_daemon_reassembles_a_fragmented_message() {
    let (mut handle, _lease) = start();
    let metrics = handle.metrics.clone().expect("metrics");
    let mut client = open(&handle.local_addr);
    // The ping of §4.4 in three fragments, a Ping frame between them
    // (§5.4: control frames may be injected).
    let mut wire = part(Opcode::Text, false, &PING.as_bytes()[..3]);
    wire.extend(part(Opcode::Continuation, false, &PING.as_bytes()[3..8]));
    wire.extend(masked(Frame::ping(b"between".to_vec())));
    wire.extend(part(Opcode::Continuation, true, &PING.as_bytes()[8..]));
    client.send(&wire);
    assert_eq!(client.next(), Some(Event::Pong(b"between".to_vec())));
    assert_eq!(client.text(), "{\"op\":\"pong\"}");
    // A sample in two fragments is one sample in DDS.
    let publish =
        "{\"op\":\"publish\",\"topic\":\"TradeRfc\",\"data\":{\"sym\":\"AAPL\",\"seq\":7}}";
    let (head, tail) = publish.as_bytes().split_at(20);
    let mut wire = part(Opcode::Text, false, head);
    wire.extend(part(Opcode::Continuation, true, tail));
    // The ping behind it shows the publish was processed without an error
    // frame before it.
    wire.extend(masked(Frame::text(PING)));
    client.send(&wire);
    assert_eq!(client.text(), "{\"op\":\"pong\"}");
    assert_eq!(metrics.total(family::DDS_PUBLISHED), 1);
    assert_eq!(metrics.total(family::FRAMES_IN), 3);
    assert_eq!(metrics.errors_total.get(), 0);
    handle.shutdown();
}

#[test]
fn ws_5_4_the_daemon_closes_a_broken_fragment_sequence_with_1002() {
    let (mut handle, _lease) = start();
    // A continuation frame without a message to continue.
    let mut client = open(&handle.local_addr);
    client.send(&part(Opcode::Continuation, true, PING.as_bytes()));
    client.expect_close(Some(1002));
    // A new text frame inside a fragmented message.
    let mut client = open(&handle.local_addr);
    let mut wire = part(Opcode::Text, false, b"{\"op\":");
    wire.extend(masked(Frame::text(PING)));
    client.send(&wire);
    client.expect_close(Some(1002));
    handle.shutdown();
}

#[test]
fn ws_5_5_1_the_daemon_answers_a_close_with_a_close_of_the_same_status() {
    // §5.5.1: "If an endpoint receives a Close frame and did not
    // previously send a Close frame, the endpoint MUST send a Close frame
    // in response."
    let (mut handle, _lease) = start();
    for status in [1000u16, 1001, 3000, 4999] {
        let mut client = open(&handle.local_addr);
        client.send(&masked(Frame::close(status, "bye")));
        client.expect_close(Some(status));
    }
    // A Close without status code.
    let mut client = open(&handle.local_addr);
    client.send(&part(Opcode::Close, true, b""));
    client.expect_close(None);
    // Frames behind the Close are not processed: no pong comes.
    let mut client = open(&handle.local_addr);
    let mut wire = masked(Frame::close(1000, ""));
    wire.extend(masked(Frame::text(PING)));
    client.send(&wire);
    client.expect_close(Some(1000));
    handle.shutdown();
}

#[test]
fn ws_5_5_1_the_daemon_closes_an_invalid_close_body_with_1002_or_1007() {
    let (mut handle, _lease) = start();
    // §7.4.1: 1005 and 1006 MUST NOT be set in a Close frame; §7.4.2: 999
    // and 5000 are in no range; one octet is no status code.
    for body in [
        &1005u16.to_be_bytes()[..],
        &1006u16.to_be_bytes()[..],
        &999u16.to_be_bytes()[..],
        &5000u16.to_be_bytes()[..],
        &[0x03][..],
    ] {
        let mut client = open(&handle.local_addr);
        client.send(&part(Opcode::Close, true, body));
        client.expect_close(Some(1002));
    }
    // A reason that is not UTF-8.
    let mut client = open(&handle.local_addr);
    client.send(&part(Opcode::Close, true, &[0x03, 0xe8, 0xff, 0xfe]));
    client.expect_close(Some(1007));
    handle.shutdown();
}

#[test]
fn ws_8_2_the_daemon_closes_malformed_utf8_in_a_text_frame_with_1007() {
    // §8.2: "the server MUST fail the WebSocket Connection". The first
    // frame the daemon sends is the Close, not an error frame of §4.4.
    let (mut handle, _lease) = start();
    let mut client = open(&handle.local_addr);
    client.send(&part(
        Opcode::Text,
        true,
        b"{\"op\":\"ping\",\"x\":\"\xc3\x28\"}",
    ));
    client.expect_close(Some(1007));
    // Over fragments: the fragment that makes the text invalid fails the
    // connection, the final one is not waited for.
    let mut client = open(&handle.local_addr);
    let mut wire = part(Opcode::Text, false, b"{\"op\":\"");
    wire.extend(part(Opcode::Continuation, false, &[0xed, 0xa0, 0x80]));
    client.send(&wire);
    client.expect_close(Some(1007));
    // Inside one frame: the frame announces 20 octets, the client sends
    // them up to the second octet of f4 90, which no code point begins
    // with, and waits. The daemon fails the connection on that octet; the
    // rest of the frame never comes.
    let mut client = open(&handle.local_addr);
    let wire = part(Opcode::Text, true, b"{\"op\":\"ping\"} \xf4\x90\x80\x80");
    client.send(&wire[..wire.len() - 2]);
    client.expect_close(Some(1007));
    handle.shutdown();
}

#[test]
fn ws_10_4_the_daemon_closes_a_frame_over_its_limit_with_1009_from_the_header() {
    // Fourteen octets: a header that announces 2^40 octets of payload.
    let (mut handle, _lease) = start();
    let mut client = open(&handle.local_addr);
    let mut wire = vec![0x82u8, 0xff];
    wire.extend_from_slice(&(1u64 << 40).to_be_bytes());
    wire.extend_from_slice(&MASK);
    client.send(&wire);
    client.expect_close(Some(1009));
    handle.shutdown();
}

#[test]
fn ws_10_4_the_daemon_closes_a_message_of_many_frames_over_its_limit_with_1009() {
    // No frame is over the limit; the message is: seventeen fragments of
    // 1 MiB against the 16 MiB of a message. Sixteen of them are a message
    // in progress, the seventeenth fails the connection.
    let (mut handle, _lease) = start();
    let metrics = handle.metrics.clone().expect("metrics");
    let mut client = open(&handle.local_addr);
    let chunk = vec![0x5au8; 1024 * 1024];
    client.send(&part(Opcode::Binary, false, &chunk));
    let more = part(Opcode::Continuation, false, &chunk);
    for _ in 0..15 {
        client.send(&more);
    }
    // The sixteen fragments are no message yet and no error: a ping
    // between them is answered.
    client.send(&masked(Frame::ping(b"at the limit".to_vec())));
    assert_eq!(client.next(), Some(Event::Pong(b"at the limit".to_vec())));
    assert_eq!(metrics.errors_total.get(), 0);
    client.send(&more);
    client.expect_close(Some(1009));
    assert_eq!(metrics.total(family::FRAMES_IN), 0);
    handle.shutdown();
}

#[test]
fn ws_10_4_the_daemon_holds_the_frame_limit_of_its_configuration() {
    // `limits.max_frame_size: 1024`. A frame of 1024 octets is a message;
    // the header of one with 1025 fails the connection before any of its
    // payload is sent.
    let (mut handle, _lease) = start_with(|cfg| cfg.max_frame_size = 1024);
    let metrics = handle.metrics.clone().expect("metrics");
    let mut client = open(&handle.local_addr);
    client.send(&part(Opcode::Text, true, &ping_of(1024)));
    assert_eq!(client.text(), "{\"op\":\"pong\"}");
    assert_eq!(metrics.errors_total.get(), 0);
    let mut header = vec![0x81u8, 0x80 | 126];
    header.extend_from_slice(&1025u16.to_be_bytes());
    header.extend_from_slice(&MASK);
    client.send(&header);
    client.expect_close(Some(1009));
    assert_eq!(metrics.errors_total.get(), 1);
    handle.shutdown();
}

#[test]
fn ws_10_4_the_daemon_holds_the_message_limit_of_its_configuration() {
    // `limits.max_message_size: 4096` under the default frame limit. Four
    // fragments of 1024 octets are a message; one octet more is not.
    let (mut handle, _lease) = start_with(|cfg| cfg.max_message_size = 4096);
    let metrics = handle.metrics.clone().expect("metrics");
    let mut client = open(&handle.local_addr);
    let ping = ping_of(4096);
    let mut wire = part(Opcode::Text, false, &ping[..1024]);
    wire.extend(part(Opcode::Continuation, false, &ping[1024..2048]));
    wire.extend(part(Opcode::Continuation, false, &ping[2048..3072]));
    wire.extend(part(Opcode::Continuation, true, &ping[3072..]));
    client.send(&wire);
    assert_eq!(client.text(), "{\"op\":\"pong\"}");
    assert_eq!(metrics.errors_total.get(), 0);
    let ping = ping_of(4097);
    let mut wire = part(Opcode::Text, false, &ping[..4096]);
    wire.extend(part(Opcode::Continuation, true, &ping[4096..]));
    client.send(&wire);
    client.expect_close(Some(1009));
    // An unfragmented message over the limit, in a frame under the frame
    // limit, is refused as well.
    let mut client = open(&handle.local_addr);
    client.send(&part(Opcode::Text, true, &ping));
    client.expect_close(Some(1009));
    handle.shutdown();
}

#[test]
fn ws_10_4_the_daemon_does_not_start_with_a_limit_it_cannot_serve() {
    for change in [
        (|cfg: &mut DaemonConfig| cfg.max_frame_size = 0) as fn(&mut DaemonConfig),
        |cfg| cfg.max_frame_size = 124,
        |cfg| cfg.max_message_size = 0,
    ] {
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.listen = "127.0.0.1:0".to_string();
        change(&mut cfg);
        match server::start(cfg) {
            Err(server::ServerError::Config(text)) => assert!(text.contains("limits"), "{text}"),
            other => panic!(
                "expected a configuration error, got {:?}",
                other.map(|_| ())
            ),
        }
    }
}

const EXPECTED_ORIGIN: &str = "https://app.example.org";

#[test]
fn ws_10_2_the_daemon_answers_an_origin_it_does_not_expect_with_403() {
    let (mut handle, _lease) = start_with(|cfg| {
        cfg.origin_allowed = vec![EXPECTED_ORIGIN.to_string(), "http://localhost:3000".into()];
    });
    let metrics = handle.metrics.clone().expect("metrics");
    for origin in [
        "https://evil.example",
        // The expected host under another scheme and on another port, and
        // a host that ends like it.
        "http://app.example.org",
        "https://app.example.org:8443",
        "https://app.example.org.evil.example",
        "null",
        // Two origins, one of them not expected.
        "https://app.example.org https://evil.example",
    ] {
        let errors = metrics.errors_total.get();
        let req = request(&format!("Origin: {origin}\r\n"));
        let (mut stream, head) = handshake(&handle.local_addr, req.as_bytes());
        assert!(
            head.starts_with("HTTP/1.1 403 Forbidden\r\n"),
            "{origin}: {head}"
        );
        assert!(head.contains("\r\nConnection: close\r\n"), "{head}");
        assert!(!head.contains("Sec-WebSocket-Accept"), "{head}");
        let body = body_and_end(&mut stream, &head);
        assert!(
            !body.contains("evil"),
            "the body repeats the origin: {body}"
        );
        assert_eq!(metrics.errors_total.get(), errors + 1, "{origin}");
    }
    // A second Origin field does not get an origin past the check.
    let req = request(&format!(
        "Origin: {EXPECTED_ORIGIN}\r\nOrigin: https://evil.example\r\n"
    ));
    let (_stream, head) = handshake(&handle.local_addr, req.as_bytes());
    assert!(head.starts_with("HTTP/1.1 403 Forbidden\r\n"), "{head}");
    assert_eq!(metrics.total(family::FRAMES_IN), 0);
    handle.shutdown();
}

#[test]
fn ws_10_2_the_daemon_accepts_an_origin_it_expects_and_a_client_without_one() {
    let (mut handle, _lease) = start_with(|cfg| {
        cfg.origin_allowed = vec![EXPECTED_ORIGIN.to_string(), "http://localhost:3000".into()];
    });
    let metrics = handle.metrics.clone().expect("metrics");
    for extra in [
        format!("Origin: {EXPECTED_ORIGIN}\r\n"),
        // Scheme and host have no case; 443 is the port of https.
        "origin: HTTPS://APP.Example.org:443\r\n".to_string(),
        "Origin: http://localhost:3000\r\n".to_string(),
        // A client outside a browser sends no Origin (RFC 6455 §4.1 item
        // 8) and `origin.required` is not set.
        String::new(),
    ] {
        let (stream, head) = handshake(&handle.local_addr, request(&extra).as_bytes());
        assert!(head.starts_with("HTTP/1.1 101 "), "{extra}: {head}");
        let mut client = Client {
            stream,
            receiver: Receiver::new(Role::Client, Limits::default()),
            buf: Vec::new(),
        };
        client.send(&masked(Frame::text(PING)));
        assert_eq!(client.text(), "{\"op\":\"pong\"}");
    }
    assert_eq!(metrics.errors_total.get(), 0);
    handle.shutdown();
}

#[test]
fn ws_10_2_the_daemon_refuses_a_handshake_without_origin_when_one_is_required() {
    let (mut handle, _lease) = start_with(|cfg| {
        cfg.origin_allowed = vec![EXPECTED_ORIGIN.to_string()];
        cfg.origin_required = true;
    });
    let (mut stream, head) = handshake(&handle.local_addr, request("").as_bytes());
    assert!(head.starts_with("HTTP/1.1 403 Forbidden\r\n"), "{head}");
    assert_eq!(
        body_and_end(&mut stream, &head),
        "this server requires an Origin\n"
    );
    let req = request(&format!("Origin: {EXPECTED_ORIGIN}\r\n"));
    let (_stream, head) = handshake(&handle.local_addr, req.as_bytes());
    assert!(head.starts_with("HTTP/1.1 101 "), "{head}");
    handle.shutdown();
}

#[test]
fn ws_10_2_without_a_list_the_daemon_accepts_every_origin() {
    // RFC 6455 §4.2.2 /origin/: "If the server does not validate the
    // origin, it will accept connections from anywhere."
    let (mut handle, _lease) = start();
    for origin in ["https://anything.example", "null", "not an origin"] {
        let req = request(&format!("Origin: {origin}\r\n"));
        let (_stream, head) = handshake(&handle.local_addr, req.as_bytes());
        assert!(head.starts_with("HTTP/1.1 101 "), "{origin}: {head}");
    }
    handle.shutdown();
}

#[test]
fn ws_10_2_the_daemon_checks_the_origin_before_the_credentials() {
    // With `auth.mode: bearer` a request without a token gets 401. From an
    // origin the daemon does not expect it gets 403, with and without the
    // token: the script of a foreign page is refused whatever the browser
    // sends along.
    let (mut handle, _lease) = start_with(|cfg| {
        cfg.origin_allowed = vec![EXPECTED_ORIGIN.to_string()];
        cfg.auth_mode = "bearer".to_string();
        cfg.auth_bearer_token = Some("the-token".to_string());
        cfg.auth_bearer_subject = Some("alice".to_string());
    });
    let cases = [
        (format!("Origin: {EXPECTED_ORIGIN}\r\n"), "401"),
        ("Origin: https://evil.example\r\n".to_string(), "403"),
        (
            "Origin: https://evil.example\r\nAuthorization: Bearer the-token\r\n".to_string(),
            "403",
        ),
        (
            format!("Origin: {EXPECTED_ORIGIN}\r\nAuthorization: Bearer the-token\r\n"),
            "101",
        ),
    ];
    for (extra, status) in cases {
        let (_stream, head) = handshake(&handle.local_addr, request(&extra).as_bytes());
        assert!(
            head.starts_with(&format!("HTTP/1.1 {status} ")),
            "{extra}: {head}"
        );
    }
    handle.shutdown();
}

#[test]
fn ws_10_2_the_daemon_does_not_start_with_an_origin_setting_it_cannot_serve() {
    for (allowed, required) in [
        (vec!["https://app.example.org/".to_string()], false),
        (vec!["app.example.org".to_string()], false),
        (Vec::new(), true),
    ] {
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.listen = "127.0.0.1:0".to_string();
        cfg.origin_allowed = allowed;
        cfg.origin_required = required;
        match server::start(cfg) {
            Err(server::ServerError::Config(text)) => assert!(text.contains("origin"), "{text}"),
            other => panic!(
                "expected a configuration error, got {:?}",
                other.map(|_| ())
            ),
        }
    }
}

/// zerodds-ws-bridge-1.0 §4.5, both sides of the bound: behind its Close
/// frame and the end of its sending direction (which `expect_close` has
/// read) the daemon reads on, so the connection is held and takes octets;
/// after 500 ms it has closed the connection, also on a client that goes on
/// sending and never closes.
fn expect_the_daemon_reads_on_for_500_ms(client: &mut Client, metrics: &BridgeMetrics) {
    let closed_at = Instant::now();
    while closed_at.elapsed() < Duration::from_millis(250) {
        // A daemon that had closed its socket would answer the first of
        // these with a reset, and a later one would fail.
        client
            .stream
            .write_all(&masked(Frame::ping(Vec::new())))
            .expect("the daemon reads on behind its Close");
        assert_eq!(
            metrics.connections_active.get(),
            1,
            "the daemon gave the connection up {:?} after its Close",
            closed_at.elapsed()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    while metrics.connections_active.get() != 0 {
        let held = closed_at.elapsed();
        assert!(
            held < Duration::from_secs(1),
            "the daemon still holds the connection {held:?} after its Close"
        );
        // A write may fail once the daemon has closed.
        let _ = client.stream.write_all(&masked(Frame::ping(Vec::new())));
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn ws_7_1_1_the_daemon_closes_the_connection_when_the_client_does_not() {
    // zerodds-ws-bridge-1.0 §4.5: after its Close frame the daemon ends its
    // sending direction and reads on for at most 500 ms. A client that
    // answers nothing, keeps its socket open and goes on sending is read
    // for that time and does not hold the connection longer.
    let (mut handle, _lease) = start();
    let metrics = handle.metrics.clone().expect("metrics");
    // The daemon answers the client's Close, and the daemon fails the
    // connection for an unmasked frame.
    let endings: [(Vec<u8>, u16); 2] = [
        (masked(Frame::close(1000, "")), 1000),
        (encode(&Frame::text(PING)).expect("encode"), 1002),
    ];
    for (ending, status) in endings {
        let frames_in = metrics.total(family::FRAMES_IN);
        let mut client = open(&handle.local_addr);
        client.send(&masked(Frame::text(PING)));
        assert_eq!(client.text(), "{\"op\":\"pong\"}");
        assert_eq!(metrics.connections_active.get(), 1);
        client.send(&ending);
        client.expect_close(Some(status));
        expect_the_daemon_reads_on_for_500_ms(&mut client, &metrics);
        // The ping before the Close was the only frame processed.
        assert_eq!(metrics.total(family::FRAMES_IN) - frames_in, 1);
    }
    handle.shutdown();
}

#[test]
fn ws_7_1_the_daemon_closes_every_connection_with_1001_when_it_shuts_down() {
    // zerodds-ws-bridge-1.0 §9.2, RFC 6455 §7.4.1: 1001, "an endpoint is
    // 'going away', such as a server going down". Every open connection
    // gets the Close frame and the end of the daemon's sending direction,
    // and the shutdown returns when the connections have ended.
    let (mut handle, _lease) = start();
    let metrics = handle.metrics.clone().expect("metrics");
    let mut clients: Vec<Client> = (0..16).map(|_| open(&handle.local_addr)).collect();
    for client in &mut clients {
        client.send(&masked(Frame::text(PING)));
        assert_eq!(client.text(), "{\"op\":\"pong\"}");
    }
    assert_eq!(metrics.connections_active.get(), 16);
    let shutdown = std::thread::spawn(move || {
        let started = Instant::now();
        handle.shutdown();
        (handle, started.elapsed())
    });
    for (nr, client) in clients.iter_mut().enumerate() {
        // zerodds-ws-bridge-1.0 §9.2: `{"op":"shutdown"}`, then the Close.
        assert_eq!(client.text(), SHUTDOWN, "connection {nr}");
        match client.next() {
            Some(Event::Close(Some((1001, reason)))) => assert_eq!(reason, "going away"),
            other => panic!("connection {nr}: expected a Close with 1001, got {other:?}"),
        }
        // The client answers with its Close (§5.5.1) and reads the end of
        // the connection, not a reset.
        client.send(&masked(Frame::close(1001, "")));
        let mut rest = [0u8; 16];
        match client.stream.read(&mut rest) {
            Ok(0) => {}
            other => panic!("connection {nr}: expected the end of the connection, got {other:?}"),
        }
    }
    drop(clients);
    let (_handle, took) = shutdown.join().expect("the shutdown");
    assert_eq!(metrics.connections_active.get(), 0);
    assert!(
        took < server::SHUTDOWN_CLOSE_WAIT,
        "the shutdown gave up waiting for the connections after {took:?}"
    );
}

#[test]
fn ws_7_1_1_the_daemon_reads_on_behind_the_close_of_its_shutdown() {
    // zerodds-ws-bridge-1.0 §4.5 on the shutdown: behind the Close 1001 the
    // daemon ends its sending direction and reads on for at most 500 ms;
    // the shutdown waits for it.
    let (mut handle, _lease) = start();
    let metrics = handle.metrics.clone().expect("metrics");
    let mut client = open(&handle.local_addr);
    client.send(&masked(Frame::text(PING)));
    assert_eq!(client.text(), "{\"op\":\"pong\"}");
    let shutdown = std::thread::spawn(move || handle.shutdown());
    assert_eq!(client.text(), SHUTDOWN);
    client.expect_close(Some(1001));
    assert!(
        !shutdown.is_finished(),
        "the shutdown did not wait for the connection"
    );
    expect_the_daemon_reads_on_for_500_ms(&mut client, &metrics);
    shutdown.join().expect("the shutdown");
}

/// Kills and reaps the daemon process when a test leaves it behind.
#[cfg(unix)]
struct Reaped(std::process::Child);

#[cfg(unix)]
impl Drop for Reaped {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(unix)]
#[test]
fn ws_7_1_the_binary_closes_every_connection_with_1001_on_sigterm() {
    // The same through `zerodds-ws-bridged` and its signal: SIGTERM sets
    // the stop flag before the daemon's main thread starts the shutdown,
    // and the process ends after it. Every connection gets its Close 1001
    // and the end of the TCP connection before the process is gone.
    use std::io::BufRead;
    use std::process::{Command, Stdio};
    let lease = zerodds_test_domains::lease_domain();
    let mut child = Reaped(
        Command::new(env!("CARGO_BIN_EXE_zerodds-ws-bridged"))
            .args(["--set", "listen=127.0.0.1:0", "--set"])
            .arg(format!("domain={}", lease.id()))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn"),
    );
    // The signal handler is installed before the daemon says `running`
    // (its JSON log, zerodds-ws-bridge-1.0 §8.1).
    let mut lines = std::io::BufReader::new(child.0.stdout.take().expect("stdout")).lines();
    let addr = lines
        .by_ref()
        .map_while(Result::ok)
        .find_map(|l| {
            let v: serde_json::Value = serde_json::from_str(&l).ok()?;
            (v["event"] == "running").then(|| v["peer"].as_str().unwrap().to_string())
        })
        .expect("the daemon reports the address it runs on");
    std::thread::spawn(move || lines.for_each(drop));
    // Forty connections opened over 200 ms: the two threads of a connection
    // look at the stop flag every 200 ms and every 500 ms, each from the
    // moment its connection began, so the signal meets them at every point
    // of that cycle.
    let mut clients = Vec::new();
    for _ in 0..40 {
        let mut client = open(&addr);
        client.send(&masked(Frame::text(PING)));
        assert_eq!(client.text(), "{\"op\":\"pong\"}");
        clients.push(client);
        std::thread::sleep(Duration::from_millis(5));
    }
    let killed = Command::new("kill")
        .args(["-TERM", &child.0.id().to_string()])
        .status()
        .expect("kill");
    assert!(killed.success());
    let without: Vec<String> = clients
        .iter_mut()
        .enumerate()
        .filter_map(|(nr, client)| {
            let mut wire = Vec::new();
            let end = client.stream.read_to_end(&mut wire);
            // The `shutdown` message (§9.2), then FIN, Close, two octets
            // of status code, the reason.
            let mut close = vec![0x81, 17];
            close.extend_from_slice(SHUTDOWN.as_bytes());
            close.extend_from_slice(&[0x88, 12, 0x03, 0xe9]);
            close.extend_from_slice(b"going away");
            (end.is_err() || wire != close)
                .then(|| format!("connection {nr}: {end:?}, octets {wire:02x?}"))
        })
        .collect();
    assert!(
        without.is_empty(),
        "{} of {} connections without the Close 1001 and the end of the connection: {without:?}",
        without.len(),
        clients.len()
    );
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.0.try_wait().expect("wait") {
            break status;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "still running 5 s after SIGTERM"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(0));
}

#[test]
fn ws_9_the_daemon_agrees_to_permessage_deflate_and_compresses_both_directions() {
    let (mut handle, _lease) = start();
    let offer = DeflateOffer {
        client_max_window_bits: Some(None),
        ..DeflateOffer::default()
    };
    let header = format!("Sec-WebSocket-Extensions: {}\r\n", render_offer(&offer));
    let (mut stream, head) = handshake(&handle.local_addr, request(&header).as_bytes());
    assert!(head.starts_with("HTTP/1.1 101 "), "{head}");
    let response = head
        .lines()
        .find_map(|l| l.strip_prefix("Sec-WebSocket-Extensions: "))
        .expect("the daemon names the extension it agreed to");
    // RFC 7692 §7: the response is one the client's offer allows.
    let params = accept_response(&offer, response).expect("a valid response");
    let mut sender = Sender::client().with_permessage_deflate(&params);
    let receiver = Receiver::new(Role::Client, Limits::default()).with_permessage_deflate(&params);
    let mut client = Client {
        stream: stream.try_clone().expect("clone"),
        receiver,
        buf: Vec::new(),
    };
    // A compressed ping (RSV1 on the client's frame) is answered with a
    // compressed pong (RSV1 on the daemon's frame), three times over the
    // same context.
    for _ in 0..3 {
        let wire = sender.message(true, PING.as_bytes()).expect("compress");
        assert_eq!(wire[0], 0xc1, "FIN, RSV1, text");
        stream.write_all(&wire).expect("send");
        let mut first = [0u8; 1];
        client.stream.peek(&mut first).expect("the answer");
        if client.buf.is_empty() {
            assert_eq!(first[0], 0xc1, "the daemon's frame is compressed");
        }
        assert_eq!(client.text(), "{\"op\":\"pong\"}");
    }
    // An uncompressed message on the same connection is one too (RFC 7692
    // §6: the bit says which it is).
    stream.write_all(&masked(Frame::text(PING))).expect("send");
    assert_eq!(client.text(), "{\"op\":\"pong\"}");
    // RSV1 on a control frame fails the connection (RFC 7692 §6.1).
    let mut wire = masked(Frame::ping(Vec::new()));
    wire[0] |= 0x40;
    stream.write_all(&wire).expect("send");
    client.expect_close(Some(1002));
    handle.shutdown();
}

#[test]
fn ws_9_the_daemon_declines_an_offer_that_is_invalid() {
    // RFC 7692 §7: an offer with an undefined parameter or an invalid
    // value is declined: the response names no extension (RFC 6455 §9.1),
    // the connection is not compressed.
    let (mut handle, _lease) = start();
    for offer in [
        "permessage-deflate; x_unknown",
        "permessage-deflate; server_max_window_bits=7",
        "x-webkit-deflate-frame",
    ] {
        let header = format!("Sec-WebSocket-Extensions: {offer}\r\n");
        let (stream, head) = handshake(&handle.local_addr, request(&header).as_bytes());
        assert!(head.starts_with("HTTP/1.1 101 "), "{offer}: {head}");
        assert!(
            !head
                .to_ascii_lowercase()
                .contains("sec-websocket-extensions"),
            "{offer}: {head}"
        );
        let mut client = Client {
            stream,
            receiver: Receiver::new(Role::Client, Limits::default()),
            buf: Vec::new(),
        };
        client.send(&masked(Frame::text(PING)));
        assert_eq!(client.text(), "{\"op\":\"pong\"}");
    }
    // The fallback offer behind a declined one is taken (RFC 7692 §5.1).
    let header = "Sec-WebSocket-Extensions: permessage-deflate; server_max_window_bits=16, \
                  permessage-deflate; server_max_window_bits=10\r\n";
    let (_stream, head) = handshake(&handle.local_addr, request(header).as_bytes());
    assert!(
        head.contains(
            "\r\nSec-WebSocket-Extensions: permessage-deflate; server_max_window_bits=10\r\n"
        ),
        "{head}"
    );
    handle.shutdown();
}

/// zerodds-ws-bridge-1.0 §4.1: a request head of more than
/// `MAX_HANDSHAKE_SIZE` octets is an invalid opening handshake, answered
/// with 400 and counted as an error; with and without its end, the empty
/// line.
#[test]
fn wsbridge_4_1_a_request_head_over_64_kib_is_answered_with_400() {
    let (mut handle, _lease) = start();
    let metrics = handle.metrics.clone().expect("metrics");
    let errors = metrics.errors_total.get();
    let valid = request("");
    let long_field = |len: usize| format!("X-Long: {}\r\n", "a".repeat(len));
    // The head with its empty line, one octet over the bound.
    let filler = server::MAX_HANDSHAKE_SIZE + 1 - valid.len() - "X-Long: \r\n".len();
    let complete = valid.replacen("\r\n\r\n", &format!("\r\n{}\r\n", long_field(filler)), 1);
    assert_eq!(complete.len(), server::MAX_HANDSHAKE_SIZE + 1);
    // A head without an end: one octet over the bound, then nothing.
    let endless = format!(
        "GET / HTTP/1.1\r\n{}",
        "a".repeat(server::MAX_HANDSHAKE_SIZE + 1 - "GET / HTTP/1.1\r\n".len())
    );
    for req in [complete, endless] {
        let (mut stream, head) = handshake(&handle.local_addr, req.as_bytes());
        assert!(head.starts_with("HTTP/1.1 400 Bad Request\r\n"), "{head}");
        body_and_end(&mut stream, &head);
    }
    assert_eq!(metrics.errors_total.get() - errors, 2);
    // A head of exactly the bound is read and served.
    let filler = server::MAX_HANDSHAKE_SIZE - valid.len() - "X-Long: \r\n".len();
    let at_bound = valid.replacen("\r\n\r\n", &format!("\r\n{}\r\n", long_field(filler)), 1);
    assert_eq!(at_bound.len(), server::MAX_HANDSHAKE_SIZE);
    let (_stream, head) = handshake(&handle.local_addr, at_bound.as_bytes());
    assert!(head.starts_with("HTTP/1.1 101 "), "{head}");
    handle.shutdown();
}

/// zerodds-ws-bridge-1.0 §4.1: the request head of the opening handshake
/// has `HANDSHAKE_TIMEOUT` after the TCP connection to arrive; a client
/// that sends it an octet at a time and never ends it gets 408 and the
/// connection ends, counted as an error.
#[test]
fn wsbridge_4_1_a_handshake_that_does_not_arrive_in_time_gets_408() {
    let (mut handle, _lease) = start();
    let metrics = handle.metrics.clone().expect("metrics");
    let errors = metrics.errors_total.get();
    let mut stream = TcpStream::connect(&handle.local_addr).expect("connect");
    let opened = Instant::now();
    let mut dribble = stream.try_clone().expect("a second handle");
    let writer = std::thread::spawn(move || {
        // An octet every 400 ms, less than the daemon's read poll, so that
        // each of its reads returns data; at most until the daemon closes.
        for octet in b"GET / HTTP/1.1\r\nHost: localhost\r\nX-Slow: abcdefghijkl".iter() {
            if dribble.write_all(&[*octet]).is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(400));
        }
    });
    stream
        .set_read_timeout(Some(server::HANDSHAKE_TIMEOUT + Duration::from_secs(5)))
        .expect("timeout");
    let mut response = Vec::new();
    let _ = stream.read_to_end(&mut response);
    let waited = opened.elapsed();
    let response = String::from_utf8_lossy(&response).into_owned();
    assert!(
        response.starts_with("HTTP/1.1 408 Request Timeout\r\n"),
        "{response:?} after {waited:?}"
    );
    assert!(
        waited >= server::HANDSHAKE_TIMEOUT
            && waited < server::HANDSHAKE_TIMEOUT + Duration::from_secs(2),
        "{waited:?}"
    );
    assert_eq!(metrics.errors_total.get() - errors, 1);
    writer.join().expect("the writer");
    handle.shutdown();
}
