// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! E2E test for `zerodds-ws-bridged`.
//!
//! Spec: `docs/specs/zerodds-ws-bridge-1.0.md` §12.2.
//!
//! Most tests start the daemon in this process (`daemon::server::start()`)
//! and talk to it as a WebSocket client; the tests of the binary
//! (`CARGO_BIN_EXE_zerodds-ws-bridged`) run it as a subprocess: its exit
//! codes and flags (§2), its JSON log (§8.1), its signals (§9.2) and the
//! byte-exact round-trip of §12.2 against a DDS participant of the test
//! process.

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

#[cfg(unix)]
#[path = "support/otlp_collector.rs"]
mod otlp_collector;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

#[cfg(unix)]
use otlp_collector::{otlp_collector, span_attribute};

#[allow(unused_imports)]
use zerodds_websocket_bridge::codec::{decode, encode};
use zerodds_websocket_bridge::daemon::config::{DaemonConfig, TopicConfig};
use zerodds_websocket_bridge::daemon::runtime_common::family;
use zerodds_websocket_bridge::daemon::server;
#[allow(unused_imports)]
use zerodds_websocket_bridge::frame::Frame;
use zerodds_websocket_bridge::handshake::compute_accept;

/// The daemon child, killed and reaped when the test ends, also when an
/// assertion fails before the test's own SIGTERM: a bare `Child` is not
/// killed on drop, and a failed run left the daemon running for hours.
struct Reaped(std::process::Child);

impl Drop for Reaped {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The tests here speak the JSON text frames, which a client asks for with
/// the `+json` subprotocol (zerodds-ws-bridge-1.0 §4.3); without it the
/// daemon sends binary frames.
fn build_handshake_request(host: &str, path: &str, key: &str) -> String {
    format!(
        "GET {path} HTTP/1.1\r\n\
         Host: {host}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {key}\r\n\
         Sec-WebSocket-Version: 13\r\n\
         Sec-WebSocket-Protocol: zerodds-ws-bridge-1.0+json\r\n\
         \r\n"
    )
}

fn read_until_double_crlf(stream: &mut TcpStream) -> String {
    let mut buf = [0u8; 4096];
    let mut out = Vec::new();
    loop {
        let n = stream.read(&mut buf).expect("read response");
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
        if out.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

fn ws_client_connect(addr: &str, path: &str) -> TcpStream {
    let mut stream = TcpStream::connect(addr).expect("connect to daemon");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set timeout");
    let request = build_handshake_request(addr, path, "dGhlIHNhbXBsZSBub25jZQ==");
    stream.write_all(request.as_bytes()).expect("send req");
    let response = read_until_double_crlf(&mut stream);
    assert!(
        response.contains("101 Switching Protocols"),
        "expected 101, got: {response}"
    );
    let expected = compute_accept("dGhlIHNhbXBsZSBub25jZQ==");
    assert!(
        response.contains(&expected),
        "expected accept hash {expected} in response: {response}"
    );
    stream
}

#[allow(dead_code)]
fn read_frame_from(stream: &mut TcpStream) -> Frame {
    let mut buf = [0u8; 4096];
    let mut acc = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        if std::time::Instant::now() > deadline {
            panic!("timeout waiting for frame; got {} bytes so far", acc.len());
        }
        match stream.read(&mut buf) {
            Ok(0) => panic!("eof before frame"),
            Ok(n) => acc.extend_from_slice(&buf[..n]),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20));
                continue;
            }
            Err(e) => panic!("read error: {e}"),
        }
        if let Ok((frame, _used)) = decode(&acc) {
            return frame;
        }
    }
}

/// `TradeE2E`'s IDL, in a file of this test process: the JSON text frames
/// (§4.3) read and write its samples.
const TRADE_IDL: &str = "@final struct TradeE2E { string sym; long seq; };";

fn trade_idl_file() -> String {
    let path = std::env::temp_dir().join(format!("zerodds-ws-trade-{}.idl", std::process::id()));
    std::fs::write(&path, TRADE_IDL).expect("write the IDL file");
    path.display().to_string()
}

/// A `TradeE2E` sample as XCDR1 little endian (`CDR_LE`, the header
/// `test_inject_user_alive` gives an injected sample): the string's length
/// with its NUL, the characters and the NUL, padding to four, the long
/// (DDS-XTypes 1.3 §7.4.1.1).
fn trade(sym: &str, seq: i32) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&u32::try_from(sym.len() + 1).unwrap().to_le_bytes());
    out.extend_from_slice(sym.as_bytes());
    out.push(0);
    while out.len() % 4 != 0 {
        out.push(0);
    }
    out.extend_from_slice(&seq.to_le_bytes());
    out
}

/// The daemon's configuration on a leased DDS domain (the host's share of
/// the band, `zerodds-test-domains`); the lease is held while the test runs.
fn make_test_config(port_hint: u16) -> (DaemonConfig, zerodds_test_domains::DomainLease) {
    let listen = format!("127.0.0.1:{port_hint}");
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.listen = listen;
    let lease = zerodds_test_domains::lease_domain();
    cfg.domain = lease.id();
    cfg.topics.push(TopicConfig {
        name: "TradeE2E".to_string(),
        type_name: "TradeE2E".to_string(),
        direction: "bidir".to_string(),
        ws_path: "/topics/trade".to_string(),
        reliability: "reliable".to_string(),
        durability: "volatile".to_string(),
        history_depth: 10,
        idl_file: trade_idl_file(),
        ..TopicConfig::default()
    });
    (cfg, lease)
}

#[test]
fn wsbridge_2_handshake_roundtrip_succeeds() {
    let (cfg, _lease) = make_test_config(0);
    let mut handle = server::start(cfg).expect("daemon start");
    let addr = handle.local_addr.clone();

    let stream = ws_client_connect(&addr, "/topics/trade");
    drop(stream);
    handle.shutdown();
}

#[test]
fn wsbridge_2_rejects_non_upgrade_request() {
    let (cfg, _lease) = make_test_config(0);
    let mut handle = server::start(cfg).expect("daemon start");
    let addr = handle.local_addr.clone();

    let mut stream = TcpStream::connect(&addr).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("timeout");
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n")
        .expect("write");
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).unwrap_or(0);
    let resp = String::from_utf8_lossy(&buf[..n]);
    assert!(
        resp.contains("400") || n == 0,
        "expected 400 Bad Request or close, got: {resp}"
    );
    handle.shutdown();
}

// Single-daemon pump test with synthetic reader injection — no
// multicast needed, hence stable on macOS and in CI containers too.
#[test]
fn wsbridge_2_cross_daemon_publish_pump_delivers_to_subscriber() {
    // L3 outbound pump proof (daemon reader → WS notify):
    // * Single-daemon setup, one WS client connects and sends
    //   `subscribe` for TradeE2E.
    // * Instead of spawning a second DDS participant and relying on
    //   multicast SPDP discovery (unreliable in CI containers and
    //   on macOS — analogous to mqtt-bridge daemon_e2e),
    //   we inject a synthetic `UserSample::Alive` directly
    //   into the `mpsc::Receiver` channel of the daemon reader slot via
    //   `DcpsRuntime::test_inject_user_alive`. This bypasses the
    //   wire+discovery path and exercises the full pump path in
    //   production form: reader channel → Router::dispatch →
    //   WS notify frame to all subscribed streams.
    //
    // Cross-daemon discovery (two DcpsRuntimes, SPDP multicast) is
    // verified separately via the Cyclone interop harness; in
    // CI containers the multicast loopback is not available.

    let (cfg, _lease) = make_test_config(0);

    let mut handle = server::start(cfg).expect("daemon start");

    let mut sub_stream = ws_client_connect(&handle.local_addr, "/topics/trade");
    sub_stream
        .set_read_timeout(Some(Duration::from_millis(50)))
        .expect("set read-timeout");

    // The daemon has auto-subscribe for TradeE2E via `/topics/trade`
    // (TopicConfig.ws_path), but subscribe + connection registration
    // runs ASYNCHRONOUSLY after the 101 response — a short pause before the
    // first inject, so the connection handler thread has gotten through
    // `router.register_connection` + `router.subscribe(auto_topic)`.
    std::thread::sleep(Duration::from_millis(150));

    let reader_eid = *handle
        .user_readers
        .get("TradeE2E")
        .expect("daemon registered user_reader for TradeE2E");
    let payload = trade("AAPL", 0);

    // Inject loop: there is a gap between frame-decode (counter++) and the
    // Router::subscribe mutation. We inject the sample repeatedly with
    // short pauses, until a notify frame arrives on the subscribed
    // connection (3 s budget).
    let frame = {
        let mut buf = [0u8; 4096];
        let mut acc = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut frame_opt = None;
        let mut last_inject = std::time::Instant::now() - Duration::from_secs(1);
        while std::time::Instant::now() < deadline {
            if last_inject.elapsed() >= Duration::from_millis(100) {
                let _ = handle
                    .runtime
                    .test_inject_user_alive(reader_eid, payload.clone());
                last_inject = std::time::Instant::now();
            }
            match sub_stream.read(&mut buf) {
                Ok(0) => panic!("eof on subscribe stream"),
                Ok(n) => {
                    acc.extend_from_slice(&buf[..n]);
                    if let Ok((f, _used)) = decode(&acc) {
                        frame_opt = Some(f);
                        break;
                    }
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) => panic!("read err on subscribe stream: {e}"),
            }
        }
        frame_opt.expect("no notify frame within 3 s")
    };
    // §4.3: a publish frame with the sample, as JSON, in `data`.
    let text = std::str::from_utf8(&frame.payload).unwrap_or("<bin>");
    for part in [
        "\"op\":\"publish\"",
        "\"topic\":\"TradeE2E\"",
        "\"data\":{\"seq\":0,\"sym\":\"AAPL\"}",
    ] {
        assert!(text.contains(part), "{part} in {text}");
    }
    handle.shutdown();
}

// ============================================================================
// Cross-Cutting Daemon-Runtime — E2E (Signal-Watcher / Admin-Endpoint /
// Metrics-Counter).
// ============================================================================

/// Graceful shutdown via DaemonHandle::shutdown() lets Drop-on-Exit
/// join all threads cleanly (§9.2).
#[test]
fn wsbridge_9_2_graceful_shutdown_completes_within_5s() {
    let (cfg, _lease) = make_test_config(0);
    let mut handle = server::start(cfg).expect("daemon start");
    // Connect a single client to ensure connection-tracking is exercised.
    let _stream = ws_client_connect(&handle.local_addr, "/topics/trade");

    let started = std::time::Instant::now();
    handle.shutdown();
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(5),
        "shutdown took {elapsed:?}, expected <5s"
    );
}

/// The handshake of a binary-mode client on the topic path of `TradeE2E`.
const TRADE_HANDSHAKE: &str = "GET /topics/trade HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\n\
     Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
     Sec-WebSocket-Version: 13\r\n\r\n";

/// §9.2: the samples the DataReader received before the shutdown reach
/// the client, all of them and in order, before `{"op":"shutdown"}`; the
/// Close 1001 follows. The RELIABLE topic's send queue holds two samples,
/// so most of them still wait in the daemon when the shutdown begins.
#[test]
fn wsbridge_9_2_samples_queued_before_the_signal_arrive_before_the_shutdown_frame() {
    const SAMPLES: i32 = 300;
    let (mut cfg, _lease) = make_test_config(0);
    cfg.max_send_queue_per_connection = 2;
    cfg.ping_interval_ms = 0;
    let mut handle = server::start(cfg).expect("daemon start");
    let (mut client, head) = Frames::handshake(&handle.local_addr, TRADE_HANDSHAKE);
    assert!(head.contains("101"), "{head}");
    std::thread::sleep(Duration::from_millis(200));
    let reader = *handle.user_readers.get("TradeE2E").unwrap();
    let payload = "s".repeat(1024);
    for seq in 0..SAMPLES {
        assert!(
            handle
                .runtime
                .test_inject_user_alive(reader, trade(&payload, seq))
        );
    }
    let shutdown = std::thread::spawn(move || {
        handle.shutdown();
        handle
    });
    let mut seqs = Vec::new();
    let text = loop {
        let frame = client
            .next_within(Duration::from_secs(10))
            .expect("a frame before the shutdown message");
        match frame.opcode {
            zerodds_websocket_bridge::frame::Opcode::Binary => {
                assert_eq!(&frame.payload[..4], b"ZDB1");
                let n = frame.payload.len();
                seqs.push(i32::from_le_bytes(
                    frame.payload[n - 4..].try_into().unwrap(),
                ));
            }
            zerodds_websocket_bridge::frame::Opcode::Text => {
                break String::from_utf8(frame.payload).unwrap();
            }
            other => panic!("unexpected {other:?} before the shutdown message"),
        }
    };
    assert_eq!(text, r#"{"op":"shutdown"}"#);
    assert_eq!(seqs, (0..SAMPLES).collect::<Vec<_>>());
    let close = client
        .next_within(Duration::from_secs(2))
        .expect("the Close");
    assert_eq!(close.opcode, zerodds_websocket_bridge::frame::Opcode::Close);
    assert_eq!(&close.payload[..2], &1001u16.to_be_bytes());
    drop(client);
    let _handle = shutdown.join().expect("the shutdown");
}

/// §9.2: the drain waits at most `shutdown_drain_ms`. A client that reads
/// nothing holds a RELIABLE topic's samples in the daemon; the shutdown
/// gives up on them when the bound passes and returns within the bound
/// plus the wait for the connections (`SHUTDOWN_CLOSE_WAIT`).
#[test]
fn wsbridge_9_2_the_drain_is_bounded_by_shutdown_drain_ms() {
    let (mut cfg, _lease) = make_test_config(0);
    cfg.max_send_queue_per_connection = 1;
    cfg.ping_interval_ms = 0;
    cfg.shutdown_drain_ms = 400;
    let mut handle = server::start(cfg).expect("daemon start");
    let (client, head) = Frames::handshake(&handle.local_addr, TRADE_HANDSHAKE);
    assert!(head.contains("101"), "{head}");
    std::thread::sleep(Duration::from_millis(200));
    let reader = *handle.user_readers.get("TradeE2E").unwrap();
    let payload = "d".repeat(64 * 1024);
    for seq in 0..400 {
        assert!(
            handle
                .runtime
                .test_inject_user_alive(reader, trade(&payload, seq))
        );
    }
    // Long enough for the client's socket and the daemon's queue to fill.
    std::thread::sleep(Duration::from_millis(300));
    let started = std::time::Instant::now();
    handle.shutdown();
    let took = started.elapsed();
    assert!(
        took >= Duration::from_millis(400),
        "the drain ended early: {took:?}"
    );
    assert!(
        took < Duration::from_millis(400) + server::SHUTDOWN_CLOSE_WAIT + Duration::from_secs(1),
        "the drain was not bounded: {took:?}"
    );
    drop(client);
}

/// The catalog/healthz/metrics endpoint returns a JSON body with the
/// configured topics (§5.2 + §8.2).
#[test]
fn wsbridge_5_2_admin_endpoint_serves_catalog_metrics_healthz() {
    let (mut cfg, _lease) = make_test_config(0);
    cfg.metrics_enabled = true;
    cfg.metrics_addr = "127.0.0.1:0".to_string();
    let mut handle = server::start(cfg).expect("daemon start");

    let admin = handle
        .admin_addr
        .clone()
        .expect("admin endpoint should be bound when metrics_enabled=true");
    let admin_sa: std::net::SocketAddr = admin.parse().expect("admin addr");

    // /catalog
    let mut s = TcpStream::connect_timeout(&admin_sa, Duration::from_secs(2))
        .expect("connect admin /catalog");
    s.set_read_timeout(Some(Duration::from_secs(2))).ok();
    s.write_all(b"GET /catalog HTTP/1.1\r\nHost: x\r\n\r\n")
        .expect("write");
    let mut body = String::new();
    s.read_to_string(&mut body).ok();
    assert!(body.contains("HTTP/1.1 200"), "got: {body}");
    assert!(body.contains("\"name\":\"TradeE2E\""), "got: {body}");
    assert!(
        body.contains("\"service\":\"zerodds-ws-bridged\""),
        "got: {body}"
    );

    // /healthz (200 — daemon healthy)
    let mut s = TcpStream::connect_timeout(&admin_sa, Duration::from_secs(2))
        .expect("connect admin /healthz");
    s.set_read_timeout(Some(Duration::from_secs(2))).ok();
    s.write_all(b"GET /healthz HTTP/1.1\r\nHost: x\r\n\r\n")
        .expect("write");
    let mut body = String::new();
    s.read_to_string(&mut body).ok();
    assert!(body.contains("HTTP/1.1 200"), "got: {body}");

    // /metrics
    let mut s = TcpStream::connect_timeout(&admin_sa, Duration::from_secs(2))
        .expect("connect admin /metrics");
    s.set_read_timeout(Some(Duration::from_secs(2))).ok();
    s.write_all(b"GET /metrics HTTP/1.1\r\nHost: x\r\n\r\n")
        .expect("write");
    let mut body = String::new();
    s.read_to_string(&mut body).ok();
    assert!(body.contains("HTTP/1.1 200"), "got: {body}");
    assert!(
        body.contains("zerodds_ws_bridge_connections_total"),
        "got: {body}"
    );

    handle.shutdown();
}

/// The Prometheus text of the admin endpoint at `admin`.
fn scrape(admin: &str) -> String {
    let mut s = TcpStream::connect(admin).expect("connect admin");
    s.set_read_timeout(Some(Duration::from_secs(2))).ok();
    s.write_all(b"GET /metrics HTTP/1.1\r\nHost: x\r\n\r\n")
        .expect("write");
    let mut body = String::new();
    s.read_to_string(&mut body).ok();
    body
}

/// §8.2: a message each way is counted under its topic and the topic's
/// `direction`, with its payload octets; a control message and its answer
/// under no topic; the DDS sides under `dds_samples_published` and
/// `dds_samples_received`. Read from the admin endpoint.
#[test]
fn wsbridge_8_2_a_frame_each_way_counts_under_its_topic() {
    let (mut cfg, _lease) = make_test_config(0);
    cfg.metrics_enabled = true;
    cfg.metrics_addr = "127.0.0.1:0".to_string();
    cfg.ping_interval_ms = 0;
    // In through `TradeE2E` (a DataWriter only), out through `TradeOut` (a
    // DataReader only): no sample of the test comes back to it through DDS.
    cfg.topics[0].direction = "in".into();
    cfg.topics.push(TopicConfig {
        name: "TradeOut".into(),
        direction: "out".into(),
        ws_path: "/topics/trade-out".into(),
        ..cfg.topics[0].clone()
    });
    let mut handle = server::start(cfg).expect("daemon start");
    let admin = handle.admin_addr.clone().expect("admin endpoint");
    let metrics = handle.metrics.clone().expect("metrics set");
    let (mut client, head) = Frames::handshake(
        &handle.local_addr,
        "GET /topics/trade-out HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\n\
         Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n",
    );
    assert!(head.contains("101"), "{head}");
    assert_eq!(metrics.connections_total.get(), 1);
    assert_eq!(metrics.connections_active.get(), 1);
    // In: a publish of TradeE2E.
    let publish = r#"{"op":"publish","topic":"TradeE2E","data":{"sym":"hi","seq":1}}"#;
    client.send_text(publish);
    // A control message and its answer: no topic.
    client.send_text(r#"{"op":"ping"}"#);
    let pong = client.next_text(Duration::from_secs(2)).expect("pong");
    assert!(pong.contains("\"op\":\"pong\""), "{pong}");
    // Out: a sample of TradeOut from DDS.
    let reader = *handle.user_readers.get("TradeOut").unwrap();
    assert!(
        handle
            .runtime
            .test_inject_user_alive(reader, trade("out", 2))
    );
    let sample = loop {
        let frame = client
            .next_within(Duration::from_secs(3))
            .expect("the sample frame");
        if frame.payload.starts_with(b"ZDB1") {
            break frame;
        }
    };
    let until = std::time::Instant::now() + Duration::from_secs(2);
    while metrics.total(family::FRAMES_OUT) < 2 && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    let text = scrape(&admin);
    let topic_in = r#"{direction="in",topic="TradeE2E"}"#;
    let topic_out = r#"{direction="out",topic="TradeOut"}"#;
    let none = r#"{direction="",topic=""}"#;
    for line in [
        format!("zerodds_ws_bridge_frames_in_total{topic_in} 1"),
        format!(
            "zerodds_ws_bridge_bytes_in_total{topic_in} {}",
            publish.len()
        ),
        format!("zerodds_ws_bridge_frames_in_total{none} 1"),
        format!("zerodds_ws_bridge_frames_out_total{topic_out} 1"),
        format!(
            "zerodds_ws_bridge_bytes_out_total{topic_out} {}",
            sample.payload.len()
        ),
        format!("zerodds_ws_bridge_frames_out_total{none} 1"),
        r#"zerodds_ws_bridge_dds_samples_published{topic="TradeE2E"} 1"#.to_string(),
        r#"zerodds_ws_bridge_dds_samples_received{topic="TradeOut"} 1"#.to_string(),
        "zerodds_ws_bridge_connections_total 1".to_string(),
        "zerodds_ws_bridge_connections_active 1".to_string(),
    ] {
        assert!(text.lines().any(|l| l == line), "{line} not in:\n{text}");
    }
    drop(client);
    handle.shutdown();
}

/// §8.2: a sample frame whose `topic` is no configured topic is counted
/// under `topic=""` and `direction=""`, like a message of no topic; the
/// name the client wrote gets no series. Its 404 error frame is one error.
#[test]
fn wsbridge_8_2_a_sample_frame_of_an_unknown_topic_counts_under_no_topic() {
    let (mut cfg, _lease) = make_test_config(0);
    cfg.metrics_enabled = true;
    cfg.metrics_addr = "127.0.0.1:0".to_string();
    cfg.ping_interval_ms = 0;
    let mut handle = server::start(cfg).expect("daemon start");
    let admin = handle.admin_addr.clone().expect("admin endpoint");
    let (mut client, head) = Frames::handshake(
        &handle.local_addr,
        "GET /topics/trade HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\n\
         Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n",
    );
    assert!(head.contains("101"), "{head}");
    let mut sent = 0;
    for n in 0..3 {
        let publish = format!(
            r#"{{"op":"publish","topic":"Client::Named{n}","data":{{"sym":"x","seq":1}}}}"#
        );
        sent += publish.len();
        client.send_text(&publish);
        let error = loop {
            let text = client.next_text(Duration::from_secs(2)).expect("error");
            if text.contains("\"op\":\"error\"") {
                break text;
            }
        };
        assert!(error.contains("404"), "{error}");
    }
    let text = scrape(&admin);
    let none = r#"{direction="",topic=""}"#;
    for line in [
        format!("zerodds_ws_bridge_frames_in_total{none} 3"),
        format!("zerodds_ws_bridge_bytes_in_total{none} {sent}"),
        "zerodds_ws_bridge_errors_total 3".to_string(),
    ] {
        assert!(text.lines().any(|l| l == line), "{line} not in:\n{text}");
    }
    assert!(!text.contains("Client::Named"), "{text}");
    drop(client);
    handle.shutdown();
}

/// `direction` means what it means in the other bridge daemons: `in`
/// carries client frames into DDS (a DataWriter), `out` carries DDS
/// samples to the clients (a DataReader). A publish or subscribe against
/// the direction a topic does not carry gets an error frame, not silence.
#[test]
fn direction_in_writes_into_dds_and_out_reads_from_it() {
    let (mut cfg, _lease) = make_test_config(0);
    cfg.topics.clear();
    for (name, direction) in [("IntoDds", "in"), ("OutOfDds", "out")] {
        cfg.topics.push(TopicConfig {
            name: name.to_string(),
            type_name: name.to_string(),
            direction: direction.to_string(),
            ws_path: format!("/topics/{name}"),
            reliability: "reliable".to_string(),
            durability: "volatile".to_string(),
            history_depth: 10,
            ..TopicConfig::default()
        });
    }
    let mut handle = server::start(cfg).expect("daemon start");
    assert!(handle.user_writers.contains_key("IntoDds"));
    assert!(!handle.user_readers.contains_key("IntoDds"));
    assert!(handle.user_readers.contains_key("OutOfDds"));
    assert!(!handle.user_writers.contains_key("OutOfDds"));

    let mut stream = ws_client_connect(&handle.local_addr, "/");
    for (op, reason) in [
        (
            "{\"op\":\"publish\",\"topic\":\"OutOfDds\",\"data\":{}}",
            "OutOfDds is not bridged into DDS",
        ),
        (
            "{\"op\":\"subscribe\",\"topics\":[\"IntoDds\"]}",
            "IntoDds is not bridged out of DDS",
        ),
    ] {
        let bytes = encode(&Frame::text(op).with_mask([1, 2, 3, 4])).expect("encode");
        stream.write_all(&bytes).expect("send");
        let frame = read_frame_from(&mut stream);
        let text = std::str::from_utf8(&frame.payload).expect("text frame");
        assert!(text.contains(reason), "{op}: {text}");
    }
    drop(stream);
    handle.shutdown();
}

/// Outbound notify frames to an idle subscriber must not wait for the
/// connection's 500 ms reader timeout to expire.
#[test]
fn subscriber_delivery_does_not_wait_for_reader_timeout() {
    let (cfg, _lease) = make_test_config(0);
    let mut handle = server::start(cfg).expect("daemon start");

    let mut sub_stream = ws_client_connect(&handle.local_addr, "/topics/trade");
    sub_stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .expect("set read-timeout");

    std::thread::sleep(Duration::from_millis(150));

    let reader_eid = *handle
        .user_readers
        .get("TradeE2E")
        .expect("daemon registered user_reader for TradeE2E");

    for seq in 0..5 {
        // Sleep 40 ms so the daemon's reader loop is parked inside its 500 ms read().
        std::thread::sleep(Duration::from_millis(40));
        let t0 = std::time::Instant::now();
        assert!(
            handle
                .runtime
                .test_inject_user_alive(reader_eid, trade("X", seq)),
            "inject sample"
        );
        let frame = read_frame_from(&mut sub_stream);
        let elapsed = t0.elapsed();
        let text = std::str::from_utf8(&frame.payload).expect("utf8");
        assert!(text.contains(&format!("\"seq\":{seq}")), "got: {text}");
        assert!(
            elapsed < Duration::from_millis(100),
            "sample {seq} took {elapsed:?} (expected < 100 ms; reader lock contention?)"
        );
    }

    handle.shutdown();
}

/// Spec §9.2 through the binary: `READY=1` to `$NOTIFY_SOCKET` once it
/// serves, SIGTERM ends the process with exit 0 after `STOPPING=1`.
/// The binary parked forever and was still running 3 s after SIGTERM.
#[cfg(unix)]
#[test]
fn wsbridge_9_2_the_binary_exits_0_on_sigterm() {
    use std::process::{Command, Stdio};
    let lease = zerodds_test_domains::lease_domain();
    // sd_notify: the packaged units are Type=notify.
    let dir = std::env::temp_dir().join(format!("zd-notify-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let sock_path = dir.join("notify.sock");
    let _ = std::fs::remove_file(&sock_path);
    let notify = std::os::unix::net::UnixDatagram::bind(&sock_path).unwrap();
    notify
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    let next_state = || {
        let mut buf = [0u8; 64];
        let n = notify.recv(&mut buf).expect("a notify datagram");
        String::from_utf8_lossy(&buf[..n]).into_owned()
    };
    let mut child = Reaped(
        Command::new(env!("CARGO_BIN_EXE_zerodds-ws-bridged"))
            .args(["--set", "listen=127.0.0.1:0", "--set"])
            .arg(format!("domain={}", lease.id()))
            .env("NOTIFY_SOCKET", &sock_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn"),
    );
    // The signal handler is installed before the daemon says `running`; a
    // SIGTERM before that ends the process by the default action.
    use std::io::BufRead;
    let mut lines = std::io::BufReader::new(child.0.stdout.take().unwrap()).lines();
    assert!(
        lines
            .by_ref()
            .map_while(Result::ok)
            .any(|l| l.contains("\"event\":\"running\"")),
        "the daemon never reported running"
    );
    std::thread::spawn(move || lines.for_each(drop));
    assert_eq!(next_state(), "READY=1");
    let killed = Command::new("kill")
        .args(["-TERM", &child.0.id().to_string()])
        .status()
        .unwrap();
    assert!(killed.success());
    let started = std::time::Instant::now();
    let status = loop {
        if let Some(st) = child.0.try_wait().unwrap() {
            break st;
        }
        if started.elapsed() > std::time::Duration::from_secs(5) {
            panic!("still running 5 s after SIGTERM");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(0));
    assert_eq!(next_state(), "STOPPING=1");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The JSON log line `line` of the daemon (Spec §8.1) when it is the
/// event `event`.
#[cfg(unix)]
fn log_line(line: &str, event: &str) -> Option<serde_json::Value> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    (v["event"] == event).then_some(v)
}

/// One plain HTTP request to `addr`, the whole response.
fn http_get(addr: &str, path: &str) -> String {
    let mut s = TcpStream::connect(addr).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(2))).ok();
    s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes())
        .expect("write");
    let mut body = String::new();
    s.read_to_string(&mut body).ok();
    body
}

/// §4.2: `/healthz` answers a plain GET on the WebSocket listener, with
/// the metrics endpoint off; an opening handshake to it is no health probe.
#[test]
fn wsbridge_4_2_healthz_answers_on_the_ws_listener_without_metrics() {
    let (cfg, _lease) = make_test_config(0);
    assert!(!cfg.metrics_enabled && cfg.metrics_addr.is_empty());
    let mut handle = server::start(cfg).expect("daemon start");
    assert!(handle.admin_addr.is_none(), "no admin endpoint");
    let resp = http_get(&handle.local_addr, "/healthz");
    assert!(resp.starts_with("HTTP/1.1 200 OK\r\n"), "{resp}");
    assert!(resp.ends_with("\r\n\r\nOK\n"), "{resp}");
    // Another path of a plain GET is still an invalid opening handshake.
    let other = http_get(&handle.local_addr, "/metrics");
    assert!(other.starts_with("HTTP/1.1 400"), "{other}");
    handle.shutdown();
}

/// §4.2 with §3 `metrics.path`: the admin endpoint serves the metrics at
/// the configured path and not at `/metrics`.
#[test]
fn wsbridge_4_2_the_metrics_path_is_the_configured_one() {
    let (mut cfg, _lease) = make_test_config(0);
    cfg.metrics_enabled = true;
    cfg.metrics_addr = "127.0.0.1:0".to_string();
    cfg.metrics_path = "/internal/metrics".to_string();
    let mut handle = server::start(cfg).expect("daemon start");
    let admin = handle.admin_addr.clone().expect("admin endpoint");
    let metrics = http_get(&admin, "/internal/metrics");
    assert!(metrics.starts_with("HTTP/1.1 200"), "{metrics}");
    assert!(
        metrics.contains("zerodds_ws_bridge_connections_total"),
        "{metrics}"
    );
    let default = http_get(&admin, "/metrics");
    assert!(default.starts_with("HTTP/1.1 404"), "{default}");
    handle.shutdown();
}

/// §4.2: a topic with `ws_path` is served there alone; its default path
/// opens a connection without the topic.
#[test]
fn wsbridge_4_2_ws_path_replaces_the_default_path() {
    let (cfg, _lease) = make_test_config(0);
    assert_eq!(cfg.topics[0].ws_path, "/topics/trade");
    let mut handle = server::start(cfg).expect("daemon start");
    let mut at_ws_path = ws_client_connect(&handle.local_addr, "/topics/trade");
    let mut at_default = ws_client_connect(&handle.local_addr, "/topics/tradee2e");
    for s in [&mut at_ws_path, &mut at_default] {
        s.set_read_timeout(Some(Duration::from_millis(50))).ok();
    }
    std::thread::sleep(Duration::from_millis(150));
    let reader = *handle.user_readers.get("TradeE2E").expect("reader");
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut acc = Vec::new();
    let mut buf = [0u8; 4096];
    let frame = loop {
        assert!(std::time::Instant::now() < deadline, "no sample at ws_path");
        let _ = handle
            .runtime
            .test_inject_user_alive(reader, trade("WS", 1));
        if let Ok(n) = at_ws_path.read(&mut buf) {
            assert!(n > 0, "eof");
            acc.extend_from_slice(&buf[..n]);
            if let Ok((f, _)) = decode(&acc) {
                break f;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(
        std::str::from_utf8(&frame.payload)
            .unwrap()
            .contains("\"sym\":\"WS\"")
    );
    // The default path subscribed to nothing.
    at_default
        .set_read_timeout(Some(Duration::from_millis(300)))
        .ok();
    match at_default.read(&mut buf) {
        Ok(n) => panic!("{n} octets at the default path"),
        Err(e) => assert!(
            matches!(
                e.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ),
            "{e}"
        ),
    }
    handle.shutdown();
}

/// §4.2 with §5.2: `/topics/__catalog__` is the catalog, over WebSocket as
/// the first frame of a connection that stays open, and to a plain GET.
#[test]
fn wsbridge_4_2_the_catalog_path_serves_the_catalog_over_ws_and_http() {
    let (cfg, _lease) = make_test_config(0);
    let mut handle = server::start(cfg).expect("daemon start");
    let (mut ws, head) = Frames::handshake(
        &handle.local_addr,
        &build_handshake_request(
            &handle.local_addr,
            "/topics/__catalog__",
            "dGhlIHNhbXBsZSBub25jZQ==",
        ),
    );
    assert!(head.contains("101 Switching Protocols"), "{head}");
    let text = ws
        .next_text(Duration::from_secs(3))
        .expect("the catalog frame");
    let text = text.as_str();
    for part in [
        "{\"op\":\"catalog\",",
        "\"name\":\"TradeE2E\"",
        "\"ws_path\":\"/topics/trade\"",
        "\"reliability\":\"reliable\"",
    ] {
        assert!(text.contains(part), "{part} in {text}");
    }
    // The connection takes control messages after the catalog.
    ws.send_text("{\"op\":\"ping\"}");
    let pong = ws.next_text(Duration::from_secs(3)).expect("pong");
    assert!(pong.contains("\"op\":\"pong\""), "{pong}");
    let http = http_get(&handle.local_addr, "/topics/__catalog__");
    assert!(http.starts_with("HTTP/1.1 200 OK\r\n"), "{http}");
    assert!(
        http.contains("Content-Type: application/json\r\n"),
        "{http}"
    );
    let body = &http[http.find("\r\n\r\n").unwrap() + 4..];
    assert_eq!(body, text, "the same catalog over HTTP and over WebSocket");
    handle.shutdown();
}

/// The binary with `args`, its exit code and its output; a daemon that has
/// not ended after 10 s is killed and the test fails.
fn run_binary(args: &[&str]) -> (Option<i32>, String, String) {
    use std::process::{Command, Stdio};
    let mut child = Reaped(
        Command::new(env!("CARGO_BIN_EXE_zerodds-ws-bridged"))
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn"),
    );
    let started = std::time::Instant::now();
    while child.0.try_wait().unwrap().is_none() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "{args:?}: still running after 10 s"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut out = String::new();
    let mut err = String::new();
    child.0.stdout.take().unwrap().read_to_string(&mut out).ok();
    child.0.stderr.take().unwrap().read_to_string(&mut err).ok();
    (child.0.wait().unwrap().code(), out, err)
}

/// §2: a configuration error is exit 1, from a flag and from a file key.
#[test]
fn wsbridge_2_a_config_error_is_exit_1() {
    for args in [
        &["--domain", "notanint"][..],
        &["--set", "limits.max_frame_size=3"][..],
        &["--topic", "Chat::Message:chat"][..],
        &["--no-such-option"][..],
    ] {
        let (code, _, err) = run_binary(args);
        assert_eq!(code, Some(1), "{args:?}: {err}");
    }
}

/// §2: a listen address in use is exit 2.
#[test]
fn wsbridge_2_a_port_in_use_is_exit_2() {
    let lease = zerodds_test_domains::lease_domain();
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = taken.local_addr().unwrap().to_string();
    let domain = lease.id().to_string();
    let (code, _, err) = run_binary(&["--listen", &addr, "--domain", &domain]);
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains("bind error"), "{err}");
}

/// §2: a DDS participant that does not start (domain 233 lies beyond the
/// UDP ports of DDSI-RTPS 2.5 §9.6.1.1) is exit 3.
#[test]
fn wsbridge_2_a_participant_that_does_not_start_is_exit_3() {
    let (code, _, err) = run_binary(&["--listen", "127.0.0.1:0", "--domain", "233"]);
    assert_eq!(code, Some(3), "{err}");
    assert!(err.contains("dds error"), "{err}");
}

/// §2: a certificate or key that does not load is exit 4.
#[test]
fn wsbridge_2_a_certificate_that_does_not_load_is_exit_4() {
    let dir = std::env::temp_dir().join(format!("zd-ws-exit4-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let garbage = dir.join("garbage.pem");
    std::fs::write(&garbage, "no PEM here\n").unwrap();
    let missing = dir.join("missing.pem");
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id().to_string();
    for file in [&garbage, &missing] {
        let file = file.to_str().unwrap();
        let (code, _, err) = run_binary(&[
            "--listen",
            "127.0.0.1:0",
            "--domain",
            &domain,
            "--tls-cert",
            file,
            "--tls-key",
            file,
        ]);
        assert_eq!(code, Some(4), "{file}: {err}");
        assert!(err.contains("tls error"), "{err}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// §2: `--check-config` prints the values with their origin and exits 0
/// for a valid configuration, 1 for an invalid one, 2 for a file that
/// does not load; the daemon does not start.
#[test]
fn wsbridge_2_check_config_prints_the_values_and_exits() {
    let (code, out, err) = run_binary(&["--check-config", "--listen", "127.0.0.1:7777"]);
    assert_eq!(code, Some(0), "{out}{err}");
    assert!(
        out.lines()
            .any(|l| l.contains("listen") && l.contains("127.0.0.1:7777")),
        "{out}"
    );
    assert!(out.ends_with("configuration valid\n"), "{out}");
    assert!(!out.contains("\"event\":\"running\""), "{out}{err}");
    let (code, out, _) = run_binary(&["--check-config", "--set", "limits.max_frame_size=3"]);
    assert_eq!(code, Some(1), "{out}");
    assert!(out.contains("configuration invalid"), "{out}");
    let (code, _, err) = run_binary(&["--check-config", "--config", "/nonexistent/ws.yaml"]);
    assert_eq!(code, Some(2), "{err}");
}

/// §2 with §7.2: `auth.mode: none` on an address that is not loopback does
/// not start (exit 1), and `--check-config` reports that configuration
/// invalid (exit 1); with a bearer token it is valid, with an empty one not.
#[test]
fn wsbridge_2_check_config_refuses_none_off_loopback() {
    for listen in ["0.0.0.0:7777", "[::]:7777"] {
        let (code, out, err) = run_binary(&["--check-config", "--listen", listen]);
        assert_eq!(code, Some(1), "{listen}: {out}{err}");
        assert!(out.contains("loopback"), "{out}");
        assert!(out.ends_with("configuration invalid\n"), "{out}");
    }
    let (code, out, err) = run_binary(&[
        "--check-config",
        "--listen",
        "0.0.0.0:7777",
        "--auth-token",
        "secret",
    ]);
    assert_eq!(code, Some(0), "{out}{err}");
    // `bearer` with an empty token (an unset `${VAR}` expands to nothing).
    let (code, out, err) = run_binary(&[
        "--check-config",
        "--listen",
        "0.0.0.0:7777",
        "--set",
        "auth.mode=bearer",
        "--set",
        "auth.bearer_token=",
    ]);
    assert_eq!(code, Some(1), "{out}{err}");
    assert!(out.contains("bearer_token"), "{out}");
    // The start refuses `none` off loopback the same way.
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id().to_string();
    let (code, _, err) = run_binary(&["--listen", "0.0.0.0:0", "--domain", &domain]);
    assert_eq!(code, Some(1), "{err}");
    assert!(err.contains("loopback"), "{err}");
}

/// §2: `--help` names the options and the exit codes, `--version` the
/// program and the version of its crate; both exit 0 without starting.
#[test]
fn wsbridge_2_help_and_version() {
    let (code, out, _) = run_binary(&["--help"]);
    assert_eq!(code, Some(0));
    for part in [
        "--topic <NAME[:WS_PATH]>",
        "--check-config",
        "(default 127.0.0.1:8080)",
        "4   TLS error",
    ] {
        assert!(out.contains(part), "{part} in {out}");
    }
    for flag in ["--version", "-V"] {
        let (code, out, err) = run_binary(&[flag]);
        assert_eq!(code, Some(0));
        assert_eq!(
            out,
            format!("zerodds-ws-bridged {}\n", env!("CARGO_PKG_VERSION"))
        );
        assert!(err.is_empty(), "{err}");
    }
}

/// §2: the flags win over the file, and `--topic NAME:WS_PATH` serves the
/// topic at its path; the catalog of the running binary shows both.
#[cfg(unix)]
#[test]
fn wsbridge_2_the_flags_override_the_file_and_the_topic_flag_names_its_path() {
    use std::io::BufRead;
    use std::process::{Command, Stdio};
    let lease = zerodds_test_domains::lease_domain();
    let dir = std::env::temp_dir().join(format!("zd-ws-flags-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ws-bridged.yaml");
    // The file's listen address is one the test holds: started there, the
    // daemon would fail with exit 2.
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    std::fs::write(
        &file,
        format!(
            "listen: \"{}\"\ndomain: 0\ntopics:\n  - name: FromFile\n",
            taken.local_addr().unwrap()
        ),
    )
    .unwrap();
    let mut child = Reaped(
        Command::new(env!("CARGO_BIN_EXE_zerodds-ws-bridged"))
            .args(["--config", file.to_str().unwrap()])
            .args(["--listen", "127.0.0.1:0", "--domain"])
            .arg(lease.id().to_string())
            .args(["--topic", "Chat::Message:/chat"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn"),
    );
    let mut lines = std::io::BufReader::new(child.0.stdout.take().unwrap()).lines();
    let mut seen = Vec::new();
    let addr = lines
        .by_ref()
        .map_while(Result::ok)
        .inspect(|l| seen.push(l.clone()))
        .find_map(|l| log_line(&l, "running").map(|v| v["peer"].as_str().unwrap().to_string()))
        .unwrap_or_else(|| panic!("never running: {seen:?}"));
    std::thread::spawn(move || lines.for_each(drop));
    assert!(
        seen.iter()
            .filter_map(|l| log_line(l, "start"))
            .any(|v| { v["domain"] == i64::from(lease.id()) && v["topics"] == 2 }),
        "{seen:?}"
    );
    let catalog = http_get(&addr, "/topics/__catalog__");
    assert!(catalog.starts_with("HTTP/1.1 200"), "{catalog}");
    for part in [
        "\"name\":\"FromFile\",\"type\":\"FromFile\",\"direction\":\"bidir\",\"ws_path\":\"/topics/fromfile\"",
        "\"name\":\"Chat::Message\",\"type\":\"Chat::Message\",\"direction\":\"bidir\",\"ws_path\":\"/chat\"",
    ] {
        assert!(catalog.contains(part), "{part} in {catalog}");
    }
    drop(child);
    let _ = std::fs::remove_dir_all(&dir);
}

/// §4.3: the samples a DDS writer writes inside a coherent set reach a
/// binary-mode client with flag bit 1, a sample outside the set without it;
/// a client's binary frame with bit 1 is refused with an error frame.
#[test]
fn wsbridge_4_3_a_coherent_set_reaches_a_binary_client_with_bit_1() {
    use zerodds_dcps::runtime::UserWriterConfig;
    let (cfg, _lease) = make_test_config(0);
    let mut handle = server::start(cfg).expect("daemon start");
    // A binary-mode client: no `+json` subprotocol.
    let (mut client, head) = Frames::handshake(
        &handle.local_addr,
        "GET /topics/trade HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\n\
         Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n",
    );
    assert!(head.contains("101"), "{head}");
    // A writer of its own in the daemon's participant, matched with the
    // daemon's reader on the same-runtime route.
    let writer = handle
        .runtime
        .register_user_writer_kind(
            UserWriterConfig {
                topic_name: "TradeE2E".into(),
                type_name: "TradeE2E".into(),
                reliable: true,
                ..Default::default()
            },
            false,
        )
        .expect("writer");
    std::thread::sleep(Duration::from_millis(300));
    handle
        .runtime
        .begin_user_writer_coherent_set(writer)
        .unwrap();
    for seq in [1, 2] {
        handle
            .runtime
            .write_user_sample(writer, trade("SET", seq))
            .unwrap();
    }
    handle.runtime.end_user_writer_coherent_set(writer).unwrap();
    handle
        .runtime
        .write_user_sample(writer, trade("ONE", 3))
        .unwrap();
    let mut flags = Vec::new();
    for _ in 0..3 {
        let frame = client
            .next_within(Duration::from_secs(3))
            .expect("a sample frame");
        assert_eq!(&frame.payload[..4], b"ZDB1", "a binary sample frame");
        flags.push(frame.payload[4]);
    }
    assert_eq!(flags, [0x02, 0x02, 0x00]);

    let mut sample = b"ZDB1\x02\x00\x01\x00\x00".to_vec();
    sample.extend_from_slice(&trade("X", 9));
    let frame = Frame::binary(sample).with_mask([3, 1, 4, 1]);
    client.stream.write_all(&encode(&frame).unwrap()).unwrap();
    let text = client
        .next_text(Duration::from_secs(3))
        .expect("an error frame");
    assert!(
        text.contains("\"op\":\"error\"") && text.contains("400"),
        "{text}"
    );
    handle.shutdown();
}

/// A JSON-mode connection to `path` after its 101: the frames the daemon
/// sends right behind the response (a burst, the catalog) are kept.
fn json_client(addr: &str, path: &str) -> Frames {
    let key = "dGhlIHNhbXBsZSBub25jZQ==";
    let (frames, head) = Frames::handshake(addr, &build_handshake_request(addr, path, key));
    assert!(head.contains("101 Switching Protocols"), "{head}");
    assert!(head.contains(&compute_accept(key)), "{head}");
    frames
}

/// The frames of a connection one after the other; octets of a second
/// frame in the same read are kept for the next call.
struct Frames {
    stream: TcpStream,
    acc: Vec<u8>,
}

impl Frames {
    /// Sends `request` to `addr` and reads the response head; the octets
    /// behind it are the first of the frames. Returns the head as well.
    fn handshake(addr: &str, request: &str) -> (Self, String) {
        let mut stream = TcpStream::connect(addr).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut acc = Vec::new();
        let mut buf = [0u8; 4096];
        let end = loop {
            if let Some(i) = acc.windows(4).position(|w| w == b"\r\n\r\n") {
                break i + 4;
            }
            let n = stream.read(&mut buf).expect("the response");
            assert!(n > 0, "the connection ended in the response");
            acc.extend_from_slice(&buf[..n]);
        };
        let head = String::from_utf8_lossy(&acc[..end]).into_owned();
        let mut frames = Self::new(stream);
        frames.acc = acc[end..].to_vec();
        (frames, head)
    }

    fn new(stream: TcpStream) -> Self {
        stream
            .set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        Self {
            stream,
            acc: Vec::new(),
        }
    }

    /// The next frame within `within`, `None` after it.
    fn next_within(&mut self, within: Duration) -> Option<Frame> {
        let deadline = std::time::Instant::now() + within;
        let mut buf = [0u8; 4096];
        loop {
            if let Ok((frame, used)) = decode(&self.acc) {
                self.acc.drain(..used);
                return Some(frame);
            }
            if std::time::Instant::now() >= deadline {
                return None;
            }
            match self.stream.read(&mut buf) {
                Ok(0) => return None,
                Ok(n) => self.acc.extend_from_slice(&buf[..n]),
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) => panic!("read: {e}"),
            }
        }
    }

    /// The next text frame within `within`, answering the daemon's Pings.
    fn next_text(&mut self, within: Duration) -> Option<String> {
        let deadline = std::time::Instant::now() + within;
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            let frame = self.next_within(left)?;
            match frame.opcode {
                zerodds_websocket_bridge::frame::Opcode::Text => {
                    return Some(String::from_utf8(frame.payload).unwrap());
                }
                zerodds_websocket_bridge::frame::Opcode::Ping => self.pong(&frame.payload),
                other => panic!("unexpected {other:?}"),
            }
        }
    }

    fn pong(&mut self, data: &[u8]) {
        let mut pong = Frame::binary(data.to_vec()).with_mask([7, 7, 7, 7]);
        pong.opcode = zerodds_websocket_bridge::frame::Opcode::Pong;
        self.stream.write_all(&encode(&pong).unwrap()).unwrap();
    }

    fn send_text(&mut self, text: &str) {
        let frame = Frame::text(text).with_mask([1, 3, 5, 7]);
        self.stream.write_all(&encode(&frame).unwrap()).unwrap();
    }
}

/// §6: an instance of a topic with `qos.deadline_ms` that gets no sample
/// for the period is a `deadline_missed` frame to its subscribers, once
/// per period; before its first sample nothing is due.
#[test]
fn wsbridge_6_a_missed_deadline_is_a_deadline_missed_frame() {
    let (mut cfg, _lease) = make_test_config(0);
    cfg.topics[0].deadline_ms = Some(200);
    let mut handle = server::start(cfg).expect("daemon start");
    let mut client = json_client(&handle.local_addr, "/topics/trade");
    assert_eq!(client.next_text(Duration::from_millis(400)), None);
    let reader = *handle.user_readers.get("TradeE2E").unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert!(handle.runtime.test_inject_user_alive(reader, trade("D", 1)));
    let started = std::time::Instant::now();
    let sample = client
        .next_text(Duration::from_secs(2))
        .expect("the sample");
    assert!(sample.contains("\"op\":\"publish\""), "{sample}");
    let missed = client
        .next_text(Duration::from_secs(2))
        .expect("deadline_missed");
    let elapsed = started.elapsed();
    assert_eq!(
        missed,
        "{\"op\":\"deadline_missed\",\"topic\":\"TradeE2E\"}"
    );
    assert!(
        elapsed >= Duration::from_millis(150) && elapsed < Duration::from_millis(500),
        "{elapsed:?}"
    );
    let again = client.next_text(Duration::from_secs(2)).expect("the next");
    assert!(again.contains("deadline_missed"), "{again}");
    handle.shutdown();
}

/// §6 liveliness: the daemon Pings every `ping_interval_ms`; a client that
/// does not answer is closed with 1011 at the next Ping, one that answers
/// stays connected.
#[test]
fn wsbridge_6_a_silent_client_is_closed_after_a_missed_pong() {
    let (mut cfg, _lease) = make_test_config(0);
    cfg.ping_interval_ms = 200;
    let mut handle = server::start(cfg).expect("daemon start");
    // The silent client.
    let mut silent = json_client(&handle.local_addr, "/");
    let started = std::time::Instant::now();
    let ping = silent.next_within(Duration::from_secs(1)).expect("a Ping");
    assert_eq!(ping.opcode, zerodds_websocket_bridge::frame::Opcode::Ping);
    let close = silent.next_within(Duration::from_secs(1)).expect("a Close");
    assert_eq!(close.opcode, zerodds_websocket_bridge::frame::Opcode::Close);
    assert_eq!(
        u16::from_be_bytes([close.payload[0], close.payload[1]]),
        1011
    );
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(350) && elapsed < Duration::from_millis(900),
        "{elapsed:?}"
    );
    // The client that answers.
    let mut alive = json_client(&handle.local_addr, "/");
    let until = std::time::Instant::now() + Duration::from_millis(1200);
    let mut pings = 0;
    while std::time::Instant::now() < until {
        if let Some(frame) = alive.next_within(Duration::from_millis(50)) {
            assert_eq!(frame.opcode, zerodds_websocket_bridge::frame::Opcode::Ping);
            pings += 1;
            alive.pong(&frame.payload);
        }
    }
    assert!(pings >= 4, "{pings} Pings in 1.2 s");
    alive.send_text("{\"op\":\"ping\"}");
    assert_eq!(
        alive.next_text(Duration::from_secs(1)).as_deref(),
        Some("{\"op\":\"pong\"}")
    );
    handle.shutdown();
}

/// §6: a TRANSIENT_LOCAL topic keeps the last `history.depth` samples and
/// sends them to a client that connects later, before live samples; a
/// client of a VOLATILE topic gets live samples only.
#[test]
fn wsbridge_6_transient_local_bursts_the_last_n_on_connect() {
    let (mut cfg, _lease) = make_test_config(0);
    cfg.topics[0].durability = "transient_local".into();
    cfg.topics[0].history_depth = 2;
    let mut handle = server::start(cfg).expect("daemon start");
    let reader = *handle.user_readers.get("TradeE2E").unwrap();
    for seq in 1..=3 {
        assert!(
            handle
                .runtime
                .test_inject_user_alive(reader, trade("T", seq))
        );
    }
    std::thread::sleep(Duration::from_millis(300));
    let mut client = json_client(&handle.local_addr, "/topics/trade");
    for seq in [2, 3] {
        let text = client
            .next_text(Duration::from_secs(2))
            .expect("a kept sample");
        assert!(text.contains(&format!("\"seq\":{seq}")), "{seq}: {text}");
    }
    assert!(handle.runtime.test_inject_user_alive(reader, trade("T", 4)));
    let live = client
        .next_text(Duration::from_secs(2))
        .expect("the live sample");
    assert!(live.contains("\"seq\":4"), "{live}");
    // By `subscribe` on a connection without a topic: `subscribed`, then
    // the kept samples.
    let mut late = json_client(&handle.local_addr, "/");
    late.send_text("{\"op\":\"subscribe\",\"topics\":[\"TradeE2E\"]}");
    let first = late.next_text(Duration::from_secs(2)).unwrap();
    assert!(first.contains("\"op\":\"subscribed\""), "{first}");
    for seq in [3, 4] {
        let text = late
            .next_text(Duration::from_secs(2))
            .expect("a kept sample");
        assert!(text.contains(&format!("\"seq\":{seq}")), "{seq}: {text}");
    }
    handle.shutdown();

    let (cfg, _lease) = make_test_config(0);
    let mut handle = server::start(cfg).expect("daemon start");
    let reader = *handle.user_readers.get("TradeE2E").unwrap();
    assert!(handle.runtime.test_inject_user_alive(reader, trade("V", 1)));
    std::thread::sleep(Duration::from_millis(300));
    let mut client = json_client(&handle.local_addr, "/topics/trade");
    assert_eq!(client.next_text(Duration::from_millis(400)), None);
    handle.shutdown();
}

/// §6: the topic's `qos.partition` is the partition of the daemon's
/// DataReader: a writer of another partition does not reach the client,
/// one of the same partition does.
#[test]
fn wsbridge_6_a_sample_of_another_partition_does_not_reach_the_client() {
    use zerodds_dcps::runtime::UserWriterConfig;
    let (mut cfg, _lease) = make_test_config(0);
    cfg.topics[0].partition = vec!["plant-a".into()];
    cfg.topics[0].direction = "out".into();
    let mut handle = server::start(cfg).expect("daemon start");
    let mut client = json_client(&handle.local_addr, "/topics/trade");
    let writer = |partition: &str| {
        handle
            .runtime
            .register_user_writer_kind(
                UserWriterConfig {
                    topic_name: "TradeE2E".into(),
                    type_name: "TradeE2E".into(),
                    reliable: true,
                    partition: vec![partition.to_string()],
                    ..Default::default()
                },
                false,
            )
            .expect("writer")
    };
    let other = writer("plant-b");
    let same = writer("plant-a");
    std::thread::sleep(Duration::from_millis(300));
    handle
        .runtime
        .write_user_sample(other, trade("B", 1))
        .unwrap();
    handle
        .runtime
        .write_user_sample(same, trade("A", 2))
        .unwrap();
    let text = client.next_text(Duration::from_secs(2)).expect("a sample");
    assert!(text.contains("\"sym\":\"A\""), "{text}");
    assert_eq!(client.next_text(Duration::from_millis(400)), None);
    handle.shutdown();
}

/// One plain HTTP GET of `path` with the fields `extra` (each ending in
/// CRLF), the whole response.
fn http_get_with(addr: &str, path: &str, extra: &str) -> String {
    let mut s = TcpStream::connect(addr).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(2))).ok();
    s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n{extra}\r\n").as_bytes())
        .expect("write");
    let mut body = String::new();
    s.read_to_string(&mut body).ok();
    body
}

/// §4.2 with RFC 6455 §10.2: the catalog over plain HTTP checks the origin
/// as a topic path does: a foreign `Origin` gets 403, a request without
/// one under `origin.required` gets 403, the expected origin gets 200.
#[test]
fn wsbridge_4_2_the_catalog_over_http_checks_the_origin() {
    let (mut cfg, _lease) = make_test_config(0);
    cfg.origin_allowed = vec!["https://app.example.org".into()];
    cfg.origin_required = true;
    let mut handle = server::start(cfg).expect("daemon start");
    let addr = handle.local_addr.clone();
    let foreign = http_get_with(
        &addr,
        "/topics/__catalog__",
        "Origin: https://evil.example\r\n",
    );
    assert!(foreign.starts_with("HTTP/1.1 403"), "{foreign}");
    assert!(!foreign.contains("TradeE2E"), "{foreign}");
    let without = http_get_with(&addr, "/topics/__catalog__", "");
    assert!(without.starts_with("HTTP/1.1 403"), "{without}");
    assert!(!without.contains("TradeE2E"), "{without}");
    let expected = http_get_with(
        &addr,
        "/topics/__catalog__",
        "Origin: https://app.example.org\r\n",
    );
    assert!(expected.starts_with("HTTP/1.1 200 OK\r\n"), "{expected}");
    assert!(expected.contains("\"name\":\"TradeE2E\""), "{expected}");
    handle.shutdown();
}

/// §6 partition, writer side: the samples of the clients go out in the
/// topic's `qos.partition`: a reader of the same partition gets a client's
/// publish, a reader of another partition does not.
#[test]
fn wsbridge_6_the_samples_of_a_client_go_out_in_the_partition() {
    use zerodds_dcps::runtime::UserReaderConfig;
    let (mut cfg, _lease) = make_test_config(0);
    cfg.topics[0].partition = vec!["plant-a".into()];
    cfg.topics[0].direction = "in".into();
    let mut handle = server::start(cfg).expect("daemon start");
    let reader = |partition: &str| {
        handle
            .runtime
            .register_user_reader_kind(
                UserReaderConfig {
                    topic_name: "TradeE2E".into(),
                    type_name: "TradeE2E".into(),
                    reliable: true,
                    partition: vec![partition.to_string()],
                    ..Default::default()
                },
                false,
            )
            .expect("reader")
    };
    let (_, same) = reader("plant-a");
    let (_, other) = reader("plant-b");
    std::thread::sleep(Duration::from_millis(300));
    let mut client = json_client(&handle.local_addr, "/topics/trade");
    client.send_text(
        "{\"op\":\"publish\",\"topic\":\"TradeE2E\",\"data\":{\"sym\":\"P\",\"seq\":7}}",
    );
    let got = same
        .recv_timeout(Duration::from_secs(3))
        .expect("the reader of plant-a gets the sample");
    assert!(
        matches!(got, zerodds_dcps::runtime::UserSample::Alive { .. }),
        "{got:?}"
    );
    assert!(
        other.recv_timeout(Duration::from_millis(500)).is_err(),
        "the reader of plant-b gets nothing"
    );
    handle.shutdown();
}

/// §6 deadline and liveliness: `qos.deadline_ms` and
/// `qos.liveliness_lease_ms` are the offered QoS of the daemon's DataWriter
/// and the requested QoS of its DataReader (DDS 1.4 §2.2.3, offered at
/// most what is requested). The daemon's writer matches a reader that
/// requests 5 s, so it offers at most 5 s, and not one that requests 0.5 s,
/// so it offers more; its reader matches a writer that offers 0.5 s and not
/// one that offers 5 s. Without the keys the writer would offer an infinite
/// deadline and lease and match neither reader.
#[test]
fn wsbridge_6_the_deadline_and_the_lease_reach_the_dds_endpoints() {
    use zerodds_dcps::runtime::{UserReaderConfig, UserWriterConfig};
    use zerodds_qos::{DeadlineQosPolicy, LivelinessKind, LivelinessQosPolicy};
    use zerodds_rtps::wire_types::EntityId;
    let ms = |v: i32| zerodds_qos::Duration::from_millis(v);
    let deadline = |v| DeadlineQosPolicy { period: ms(v) };
    let lease = |v| LivelinessQosPolicy {
        kind: LivelinessKind::Automatic,
        lease_duration: ms(v),
    };
    let (mut cfg, _lease) = make_test_config(0);
    cfg.topics[0].deadline_ms = Some(1_000);
    cfg.topics[0].liveliness_lease_ms = Some(2_000);
    let mut handle = server::start(cfg).expect("daemon start");
    let rt = std::sync::Arc::clone(&handle.runtime);
    let reader = |d, l| {
        rt.register_user_reader_kind(
            UserReaderConfig {
                topic_name: "TradeE2E".into(),
                type_name: "TradeE2E".into(),
                reliable: true,
                deadline: deadline(d),
                liveliness: lease(l),
                ..Default::default()
            },
            false,
        )
        .expect("reader")
        .0
    };
    let writer = |d, l| {
        rt.register_user_writer_kind(
            UserWriterConfig {
                topic_name: "TradeE2E".into(),
                type_name: "TradeE2E".into(),
                reliable: true,
                deadline: deadline(d),
                liveliness: lease(l),
                ..Default::default()
            },
            false,
        )
        .expect("writer")
    };
    let r_ok = reader(5_000, 5_000);
    let r_deadline = reader(500, 5_000);
    let r_lease = reader(5_000, 500);
    let w_ok = writer(500, 500);
    let w_deadline = writer(5_000, 500);
    let w_lease = writer(500, 5_000);
    let daemon_writer = *handle.user_writers.get("TradeE2E").unwrap();
    let daemon_reader = *handle.user_readers.get("TradeE2E").unwrap();
    // Every endpoint here is of the daemon's participant: the GUIDs differ
    // in their entity id, the last four octets.
    let has = |guids: &[[u8; 16]], eid: EntityId| guids.iter().any(|g| g[12..] == eid.to_bytes());
    let until = std::time::Instant::now() + Duration::from_secs(3);
    while std::time::Instant::now() < until
        && !(has(&rt.user_writer_matched_reader_guids(daemon_writer), r_ok)
            && has(&rt.user_reader_matched_writer_guids(daemon_reader), w_ok))
    {
        std::thread::sleep(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_millis(200));
    let readers = rt.user_writer_matched_reader_guids(daemon_writer);
    assert!(has(&readers, r_ok), "{readers:?}");
    assert!(!has(&readers, r_deadline), "requested deadline 0.5 s");
    assert!(!has(&readers, r_lease), "requested lease 0.5 s");
    let writers = rt.user_reader_matched_writer_guids(daemon_reader);
    assert!(has(&writers, w_ok), "{writers:?}");
    assert!(!has(&writers, w_deadline), "offered deadline 5 s");
    assert!(!has(&writers, w_lease), "offered lease 5 s");
    handle.shutdown();
}

/// §6 BEST_EFFORT: a client that does not read lets its send queue fill;
/// every sample that does not reach it is counted in the drop counter, and
/// the samples it gets plus the counted drops are the samples the topic
/// got.
#[test]
fn wsbridge_6_a_best_effort_drop_is_counted() {
    const SAMPLES: u64 = 600;
    let (mut cfg, _lease) = make_test_config(0);
    cfg.topics[0].reliability = "best_effort".into();
    cfg.max_send_queue_per_connection = 1;
    cfg.ping_interval_ms = 0;
    cfg.metrics_enabled = true;
    cfg.metrics_addr = "127.0.0.1:0".to_string();
    let mut handle = server::start(cfg).expect("daemon start");
    let metrics = handle.metrics.clone().expect("metrics set");
    // A binary-mode client that reads nothing until the topic has its
    // samples.
    let (mut client, head) = Frames::handshake(
        &handle.local_addr,
        "GET /topics/trade HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\n\
         Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n",
    );
    assert!(head.contains("101"), "{head}");
    let reader = *handle.user_readers.get("TradeE2E").unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let big = "x".repeat(64 * 1024);
    for seq in 0..SAMPLES {
        assert!(
            handle
                .runtime
                .test_inject_user_alive(reader, trade(&big, i32::try_from(seq).unwrap()))
        );
    }
    let until = std::time::Instant::now() + Duration::from_secs(10);
    while metrics.total(family::DDS_RECEIVED) < SAMPLES && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(metrics.total(family::DDS_RECEIVED), SAMPLES);
    let mut got = 0u64;
    while let Some(frame) = client.next_within(Duration::from_secs(1)) {
        assert_eq!(&frame.payload[..4], b"ZDB1", "a binary sample frame");
        got += 1;
    }
    let dropped = metrics.total(family::SEND_QUEUE_DROPS);
    assert!(dropped > 0, "the queue of one sample overflowed");
    assert_eq!(got + dropped, SAMPLES, "got {got}, dropped {dropped}");
    // §8.2: the drops are counted under the daemon's number of the
    // connection, the first it accepted.
    let text = scrape(&handle.admin_addr.clone().expect("admin endpoint"));
    let drops: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("zerodds_ws_bridge_send_queue_drops_total"))
        .collect();
    assert_eq!(
        drops,
        [
            format!("zerodds_ws_bridge_send_queue_drops_total{{connection_id=\"1\"}} {dropped}")
                .as_str()
        ],
        "{text}"
    );
    handle.shutdown();
}

/// §6 RELIABLE with the liveliness of a connection: a client that takes
/// no frame for `ping_interval_ms` while its send queue is full is closed;
/// the topic's samples then reach the other clients again, and none is
/// counted as dropped.
#[test]
fn wsbridge_6_a_client_that_takes_nothing_is_closed_and_the_topic_goes_on() {
    let (mut cfg, _lease) = make_test_config(0);
    cfg.max_send_queue_per_connection = 1;
    cfg.ping_interval_ms = 300;
    cfg.metrics_enabled = true;
    cfg.metrics_addr = "127.0.0.1:0".to_string();
    let mut handle = server::start(cfg).expect("daemon start");
    let metrics = handle.metrics.clone().expect("metrics set");
    let request = "GET /topics/trade HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\n\
         Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n";
    let (stalled, head) = Frames::handshake(&handle.local_addr, request);
    assert!(head.contains("101"), "{head}");
    let reader = *handle.user_readers.get("TradeE2E").unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let big = "x".repeat(64 * 1024);
    for seq in 0..600 {
        assert!(
            handle
                .runtime
                .test_inject_user_alive(reader, trade(&big, seq))
        );
    }
    // The stalled connection is closed within a few ping intervals.
    let until = std::time::Instant::now() + Duration::from_secs(5);
    while metrics.connections_active.get() > 0 && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        metrics.connections_active.get(),
        0,
        "the stalled client is closed"
    );
    drop(stalled);
    // The pump goes on: the remaining samples are taken, and a new client
    // gets the next one.
    let until = std::time::Instant::now() + Duration::from_secs(10);
    while metrics.total(family::DDS_RECEIVED) < 600 && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(metrics.total(family::DDS_RECEIVED), 600);
    let (mut fresh, head) = Frames::handshake(&handle.local_addr, request);
    assert!(head.contains("101"), "{head}");
    std::thread::sleep(Duration::from_millis(100));
    assert!(handle.runtime.test_inject_user_alive(reader, trade("N", 1)));
    let frame = fresh
        .next_within(Duration::from_secs(2))
        .expect("the next sample");
    assert_eq!(&frame.payload[..4], b"ZDB1");
    assert_eq!(metrics.total(family::SEND_QUEUE_DROPS), 0);
    handle.shutdown();
}

/// `zerodds-ws-bridged` as a subprocess with `yaml` as its configuration
/// file: its stdout line by line on the channel; the lines that are no JSON
/// object make the test fail where they are read.
#[cfg(unix)]
fn spawn_daemon(name: &str, yaml: &str) -> (Reaped, std::sync::mpsc::Receiver<String>) {
    spawn_daemon_with_env(name, yaml, &[])
}

/// [`spawn_daemon`] with the variables `env` in the daemon's environment.
#[cfg(unix)]
fn spawn_daemon_with_env(
    name: &str,
    yaml: &str,
    env: &[(&str, &str)],
) -> (Reaped, std::sync::mpsc::Receiver<String>) {
    use std::io::BufRead;
    use std::process::{Command, Stdio};
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ws-bridged.yaml");
    std::fs::write(&file, yaml).unwrap();
    let mut child = Reaped(
        Command::new(env!("CARGO_BIN_EXE_zerodds-ws-bridged"))
            .arg("--config")
            .arg(&file)
            .envs(env.iter().copied())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn"),
    );
    let stdout = child.0.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
        {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    (child, rx)
}

/// The next log line of the event `event`; each line on the way is a JSON
/// object of Spec §8.1 (`timestamp` in RFC 3339 UTC, `level`, `event`).
#[cfg(unix)]
fn next_event(rx: &std::sync::mpsc::Receiver<String>, event: &str) -> serde_json::Value {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        let line = rx
            .recv_timeout(left)
            .unwrap_or_else(|_| panic!("no log line `{event}`"));
        let v: serde_json::Value =
            serde_json::from_str(&line).unwrap_or_else(|e| panic!("not JSON ({e}): {line}"));
        assert!(
            v["timestamp"].as_str().is_some_and(|t| t.ends_with('Z')),
            "{line}"
        );
        assert!(
            ["trace", "debug", "info", "warn", "error"].contains(&v["level"].as_str().unwrap()),
            "{line}"
        );
        // `""`: any event.
        if event.is_empty() || v["event"] == event {
            return v;
        }
    }
}

/// The YAML of a daemon on `domain` at `listen` with the topic `TradeE2E`.
#[cfg(unix)]
fn daemon_yaml(domain: i32, listen: &str, log_level: &str) -> String {
    format!(
        "listen: \"{listen}\"\ndomain: {domain}\nlog_level: {log_level}\ntopics:\n  - name: TradeE2E\n    type: TradeE2E\n    idl_file: \"{}\"\n    ws_path: /topics/trade\n    qos:\n      reliability: reliable\n",
        trade_idl_file()
    )
}

/// Spec §8.1: the binary's log is one JSON object per line on stdout; a
/// frame of a client is the line `frame_in` with `connection_id`, `topic`,
/// `bytes`, `peer` and `latency_us`, a frame to it `frame_out`; with
/// `--log-level error` (here `log_level: error`) nothing below `error` is
/// written.
#[cfg(unix)]
#[test]
fn wsbridge_8_1_the_binary_logs_json_lines_for_a_frame_exchange() {
    let lease = zerodds_test_domains::lease_domain();
    let (child, rx) = spawn_daemon(
        "ws-log-debug",
        &daemon_yaml(lease.id(), "127.0.0.1:0", "debug"),
    );
    let start = next_event(&rx, "start");
    assert_eq!(start["level"], "info");
    assert_eq!(start["domain"], i64::from(lease.id()));
    let addr = next_event(&rx, "running")["peer"]
        .as_str()
        .unwrap()
        .to_string();
    let mut client = json_client(&addr, "/topics/trade");
    let local = client.stream.local_addr().unwrap().to_string();
    let open = next_event(&rx, "connection_open");
    assert_eq!(open["peer"], local);
    assert_eq!(open["topic"], "TradeE2E");
    assert_eq!(open["mode"], "json");
    // §1 L6: no tenant names the subject; the participant of `domain`.
    assert_eq!(open["tenant"], "");
    let id = open["connection_id"].as_u64().expect("connection_id");
    let publish = "{\"op\":\"publish\",\"topic\":\"TradeE2E\",\"data\":{\"sym\":\"L\",\"seq\":1}}";
    client.send_text(publish);
    let frame_in = next_event(&rx, "frame_in");
    assert_eq!(frame_in["level"], "debug");
    assert_eq!(frame_in["connection_id"], id);
    assert_eq!(frame_in["peer"], local);
    assert_eq!(frame_in["topic"], "TradeE2E");
    assert_eq!(frame_in["bytes"], publish.len());
    assert!(frame_in["latency_us"].as_u64().is_some(), "{frame_in}");
    // A control message: `frame_in` of no topic, its answer `frame_out`.
    // The answer goes out while the message is handled, so its line comes
    // first; the daemon's reader may hand the client's own sample back
    // before both.
    client.send_text("{\"op\":\"ping\"}");
    let (mut ping_in, mut out) = (None, None);
    while ping_in.is_none() || out.is_none() {
        let v = next_event(&rx, "");
        match (v["event"].as_str().unwrap(), v["topic"].as_str()) {
            ("frame_in", Some("")) => ping_in = Some(v),
            ("frame_out", Some("")) => out = Some(v),
            ("frame_out", Some(topic)) => assert_eq!(topic, "TradeE2E", "{v}"),
            _ => {}
        }
    }
    let (ping_in, out) = (ping_in.unwrap(), out.unwrap());
    assert_eq!(ping_in["connection_id"], id);
    assert_eq!(out["connection_id"], id);
    assert_eq!(out["bytes"], "{\"op\":\"pong\"}".len());
    assert!(out["latency_us"].as_u64().is_some(), "{out}");
    // A refused message is a warning with the reason.
    client.send_text("{\"op\":\"publish\",\"topic\":\"Nope\",\"data\":{}}");
    let refused = next_event(&rx, "frame_refused");
    assert_eq!(refused["level"], "warn");
    assert_eq!(refused["topic"], "Nope");
    assert!(
        refused["reason"]
            .as_str()
            .is_some_and(|r| r.contains("Nope"))
    );
    drop(client);
    assert_eq!(next_event(&rx, "connection_close")["connection_id"], id);
    drop(child);

    // `log_level: error`: the same exchange writes no line.
    let port = zerodds_test_domains::free_tcp_port();
    let addr = format!("127.0.0.1:{port}");
    let (child, rx) = spawn_daemon("ws-log-error", &daemon_yaml(lease.id(), &addr, "error"));
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut client = loop {
        if TcpStream::connect(&addr).is_ok() {
            break json_client(&addr, "/topics/trade");
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the daemon never bound"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    client.send_text(publish);
    client.send_text("{\"op\":\"ping\"}");
    assert!(client.next_text(Duration::from_secs(3)).is_some(), "pong");
    drop(client);
    std::thread::sleep(Duration::from_millis(300));
    drop(child);
    let lines: Vec<String> = rx.try_iter().collect();
    assert!(lines.is_empty(), "{lines:?}");
}

/// Spec §8.1: a SIGHUP reload applies a changed `log_level`. The binary
/// starts at `error` and a frame exchange writes no line; the file is
/// rewritten to `debug`, and after SIGHUP the same connection's frames are
/// the lines `frame_in` and `frame_out`; rewritten to `warn` (with a
/// changed `shutdown_drain_ms`, whose `reload_needs_restart` warning marks
/// the reload), the frames write no line again.
#[cfg(unix)]
#[test]
fn wsbridge_8_1_a_sighup_reload_applies_a_changed_log_level() {
    use std::process::Command;
    let lease = zerodds_test_domains::lease_domain();
    let port = zerodds_test_domains::free_tcp_port();
    let addr = format!("127.0.0.1:{port}");
    let name = "ws-log-reload";
    let file = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(name)
        .join("ws-bridged.yaml");
    let (child, rx) = spawn_daemon(name, &daemon_yaml(lease.id(), &addr, "error"));
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut client = loop {
        if TcpStream::connect(&addr).is_ok() {
            break json_client(&addr, "/topics/trade");
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the daemon never bound"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    let ping = |client: &mut Frames| {
        client.send_text("{\"op\":\"ping\"}");
        assert_eq!(
            client.next_text(Duration::from_secs(3)).as_deref(),
            Some("{\"op\":\"pong\"}")
        );
    };
    let sighup = || {
        let sent = Command::new("kill")
            .args(["-HUP", &child.0.id().to_string()])
            .status()
            .unwrap();
        assert!(sent.success());
    };
    ping(&mut client);
    std::thread::sleep(Duration::from_millis(300));
    let lines: Vec<String> = rx.try_iter().collect();
    assert!(lines.is_empty(), "at error: {lines:?}");

    std::fs::write(&file, daemon_yaml(lease.id(), &addr, "debug")).unwrap();
    sighup();
    assert_eq!(next_event(&rx, "reload")["level"], "info");
    ping(&mut client);
    let (mut ping_in, mut pong_out) = (false, false);
    while !(ping_in && pong_out) {
        let v = next_event(&rx, "");
        assert_eq!(v["level"], "debug", "{v}");
        match v["event"].as_str().unwrap() {
            "frame_in" => ping_in = true,
            "frame_out" => pong_out = true,
            other => panic!("unexpected event {other}: {v}"),
        }
        assert_eq!(v["topic"], "", "{v}");
    }

    std::fs::write(
        &file,
        format!(
            "shutdown_drain_ms: 1234\n{}",
            daemon_yaml(lease.id(), &addr, "warn")
        ),
    )
    .unwrap();
    sighup();
    let restart = next_event(&rx, "reload_needs_restart");
    assert!(
        restart["settings"]
            .as_str()
            .is_some_and(|s| s.contains("shutdown_drain_ms")),
        "{restart}"
    );
    ping(&mut client);
    std::thread::sleep(Duration::from_millis(300));
    let lines: Vec<String> = rx.try_iter().collect();
    assert!(lines.is_empty(), "at warn: {lines:?}");
    drop(client);
    drop(child);
}

/// Spec §12.2: `zerodds-ws-bridged` as a subprocess; a binary frame of a
/// WebSocket client reaches a DDS reader of the test process with the same
/// CDR bytes and the encapsulation header the frame named, and a sample of
/// a DDS writer of the test process reaches the client as a binary frame
/// with the same bytes and header (DDS discovery between the two
/// processes on the leased domain).
#[cfg(unix)]
#[test]
fn wsbridge_12_2_a_ws_publish_reaches_a_dds_reader_byte_exact() {
    use zerodds_dcps::runtime::{
        DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
    };
    use zerodds_rtps::publication_data::data_representation::XCDR2;
    use zerodds_rtps::wire_types::GuidPrefix;
    let lease = zerodds_test_domains::lease_domain();
    let (_child, rx) = spawn_daemon("ws-12-2", &daemon_yaml(lease.id(), "127.0.0.1:0", "info"));
    let addr = next_event(&rx, "running")["peer"]
        .as_str()
        .unwrap()
        .to_string();
    // The DDS participant of the test.
    let mut prefix = [0u8; 12];
    prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
    prefix[4..8].copy_from_slice(&lease.id().to_be_bytes());
    prefix[11] = 0x12;
    let dds = DcpsRuntime::start(
        lease.id(),
        GuidPrefix::from_bytes(prefix),
        RuntimeConfig::default(),
    )
    .expect("the test's participant");
    let (reader, samples) = dds
        .register_user_reader_kind(
            UserReaderConfig {
                topic_name: "TradeE2E".into(),
                type_name: "TradeE2E".into(),
                reliable: true,
                ..Default::default()
            },
            false,
        )
        .expect("reader");
    let writer = dds
        .register_user_writer_kind(
            UserWriterConfig {
                topic_name: "TradeE2E".into(),
                type_name: "TradeE2E".into(),
                reliable: true,
                data_representation_offer: Some(vec![XCDR2]),
                ..Default::default()
            },
            false,
        )
        .expect("writer");
    dds.set_user_writer_representation_identifier(writer, [0x00, 0x07])
        .expect("XCDR2 little endian");
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while dds.user_reader_matched_count(reader) == 0 || dds.user_writer_matched_count(writer) == 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "the daemon's endpoints never matched the test's"
        );
        dds.wait_match_event(Duration::from_millis(200));
    }
    // A binary-mode client (no `+json`): WS → DDS.
    let (mut client, head) = Frames::handshake(
        &addr,
        "GET /topics/trade HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\n\
         Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n",
    );
    assert!(head.contains("101"), "{head}");
    // Discovery is matched from the test's side; the daemon's writer may
    // learn of the test's reader a moment later, and a volatile sample
    // written before that reaches nobody. The frame is sent again until the
    // reader has it; each copy is the same sample.
    let body = trade("BYTES", 12);
    let mut sent = b"ZDB1\x00\x00\x07\x00\x00".to_vec();
    sent.extend_from_slice(&body);
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let got = loop {
        assert!(
            std::time::Instant::now() < deadline,
            "the client's sample never reached the test's reader"
        );
        client
            .stream
            .write_all(&encode(&Frame::binary(sent.clone()).with_mask([9, 8, 7, 6])).unwrap())
            .unwrap();
        if let Ok(sample) = samples.recv_timeout(Duration::from_millis(300)) {
            break sample;
        }
    };
    match got {
        UserSample::Alive {
            payload,
            representation_identifier,
            ..
        } => {
            assert_eq!(payload.to_vec(), body, "the CDR bytes");
            assert_eq!(representation_identifier, [0x00, 0x07], "the header");
        }
        other => panic!("expected the client's sample, got {other:?}"),
    }
    // DDS → WS: the client gets the writer's sample byte for byte; written
    // again until it arrives, for the same reason.
    let body = trade("BACK", 34);
    let mut want = b"ZDB1\x00\x00\x07\x00\x00".to_vec();
    want.extend_from_slice(&body);
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let frame = 'wait: loop {
        assert!(
            std::time::Instant::now() < deadline,
            "the writer's sample never reached the client"
        );
        dds.write_user_sample(writer, body.clone()).unwrap();
        while let Some(f) = client.next_within(Duration::from_millis(300)) {
            if f.opcode == zerodds_websocket_bridge::frame::Opcode::Ping {
                client.pong(&f.payload);
                continue;
            }
            // The daemon's reader may hand the client's own samples back.
            if f.payload == sent {
                continue;
            }
            break 'wait f;
        }
    };
    assert_eq!(frame.payload, want);
    dds.shutdown();
}

/// Spec §5.2: `/schema/<slug>` on the WebSocket listener is the IDL of the
/// topic (`text/plain`), `/schema/<slug>.xml` its XML type representation;
/// a slug of no topic and a topic without `idl_file` are 404; the catalog
/// names the schema path.
#[test]
fn wsbridge_5_2_the_schema_endpoint_returns_the_idl_of_the_topic() {
    let (mut cfg, _lease) = make_test_config(0);
    cfg.topics.push(TopicConfig {
        name: "Untyped::Thing".to_string(),
        direction: "out".to_string(),
        ..TopicConfig::default()
    });
    let mut handle = server::start(cfg).expect("daemon start");
    let body = |resp: &str| resp[resp.find("\r\n\r\n").unwrap() + 4..].to_string();
    for path in ["/schema/tradee2e.idl", "/schema/tradee2e"] {
        let resp = http_get(&handle.local_addr, path);
        assert!(resp.starts_with("HTTP/1.1 200 OK\r\n"), "{resp}");
        assert!(
            resp.contains("Content-Type: text/plain; charset=utf-8\r\n"),
            "{resp}"
        );
        assert_eq!(body(&resp), TRADE_IDL);
    }
    let xml = http_get(&handle.local_addr, "/schema/tradee2e.xml");
    assert!(xml.starts_with("HTTP/1.1 200 OK\r\n"), "{xml}");
    assert!(xml.contains("Content-Type: application/xml"), "{xml}");
    assert!(
        body(&xml).contains(
            "<struct name=\"TradeE2E\" extensibility=\"final\">\n    <member name=\"sym\" type=\"string\"/>\n    <member name=\"seq\" type=\"int32\"/>\n  </struct>"
        ),
        "{xml}"
    );
    let none = http_get(&handle.local_addr, "/schema/nothing.idl");
    assert!(none.starts_with("HTTP/1.1 404"), "{none}");
    let untyped = http_get(&handle.local_addr, "/schema/untyped/thing");
    assert!(untyped.starts_with("HTTP/1.1 404"), "{untyped}");
    assert!(body(&untyped).contains("has no idl_file"), "{untyped}");
    // Only GET: another method is no plain GET and no opening handshake.
    let mut s = TcpStream::connect(&handle.local_addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(2))).ok();
    s.write_all(b"POST /schema/tradee2e HTTP/1.1\r\nHost: x\r\n\r\n")
        .unwrap();
    let mut resp = String::new();
    s.read_to_string(&mut resp).ok();
    assert!(resp.starts_with("HTTP/1.1 405"), "{resp}");
    let catalog = http_get(&handle.local_addr, "/topics/__catalog__");
    assert!(
        catalog.contains("\"schema_url\":\"/schema/tradee2e.idl\""),
        "{catalog}"
    );
    handle.shutdown();
}

/// Spec §5.2: with `catalog_on_connect` every connection gets the catalog
/// as its first frame, on a topic path too, and on the path of a topic
/// without a type before the 415 error frame; without it, only the catalog
/// path does.
#[test]
fn wsbridge_5_2_a_connection_gets_the_catalog_frame_when_configured() {
    let (mut cfg, _lease) = make_test_config(0);
    cfg.catalog_on_connect = true;
    cfg.topics.push(TopicConfig {
        name: "Untyped::Thing".to_string(),
        direction: "out".to_string(),
        ws_path: "/topics/thing".to_string(),
        ..TopicConfig::default()
    });
    let mut handle = server::start(cfg).expect("daemon start");
    for path in ["/topics/trade", "/", "/topics/thing"] {
        let mut ws = json_client(&handle.local_addr, path);
        let first = ws.next_text(Duration::from_secs(3)).expect("the catalog");
        let v: serde_json::Value = serde_json::from_str(&first).unwrap();
        assert_eq!(v["op"], "catalog", "{path}: {first}");
        assert_eq!(v["topics"][0]["name"], "TradeE2E");
        assert_eq!(v["topics"][0]["schema_url"], "/schema/tradee2e.idl");
        if path == "/topics/thing" {
            let second = ws.next_text(Duration::from_secs(3)).expect("the 415");
            let e: serde_json::Value = serde_json::from_str(&second).unwrap();
            assert_eq!(e["op"], "error", "{second}");
            assert_eq!(e["code"], 415, "{second}");
        }
    }
    handle.shutdown();

    let (cfg, _lease) = make_test_config(0);
    let mut handle = server::start(cfg).expect("daemon start");
    let mut ws = json_client(&handle.local_addr, "/topics/trade");
    ws.send_text("{\"op\":\"ping\"}");
    let first = ws.next_text(Duration::from_secs(3)).expect("pong");
    assert_eq!(first, "{\"op\":\"pong\"}");
    handle.shutdown();
}

/// Sends a Close 1000 and reads the frames of the daemon up to its Close.
#[cfg(unix)]
fn close_cleanly(client: &mut Frames) {
    use zerodds_websocket_bridge::frame::Opcode;
    let mut close = Frame::binary(1000u16.to_be_bytes().to_vec()).with_mask([5, 6, 7, 8]);
    close.opcode = Opcode::Close;
    client.stream.write_all(&encode(&close).unwrap()).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        let frame = client.next_within(left).expect("the daemon's Close");
        if frame.opcode == Opcode::Close {
            return;
        }
    }
}

/// Spec §8.3: with `OTEL_EXPORTER_OTLP_ENDPOINT` the binary sends the spans
/// of a connection to the collector: the root `ws.connection` with the
/// peer, the path, the subprotocol and the status 101; a client's sample
/// as `ws.frame.in`, a child of the root, with the child `dds.write`; the
/// pong of the daemon as `ws.frame.out`, a child of the root; an opening
/// handshake the daemon refused (426) as a `ws.connection` with status
/// error.
#[cfg(unix)]
#[test]
fn wsbridge_8_3_the_spans_of_a_connection_and_its_frames_reach_a_collector() {
    use std::process::Command;
    let lease = zerodds_test_domains::lease_domain();
    let (endpoint, spans) = otlp_collector();
    let (mut child, rx) = spawn_daemon_with_env(
        "ws-otlp",
        &daemon_yaml(lease.id(), "127.0.0.1:0", "info"),
        &[("OTEL_EXPORTER_OTLP_ENDPOINT", endpoint.as_str())],
    );
    next_event(&rx, "otlp_active");
    let addr = next_event(&rx, "running")["peer"]
        .as_str()
        .unwrap()
        .to_string();
    let mut client = json_client(&addr, "/topics/trade");
    let local = client.stream.local_addr().unwrap().to_string();
    let publish = "{\"op\":\"publish\",\"topic\":\"TradeE2E\",\"data\":{\"sym\":\"O\",\"seq\":3}}";
    client.send_text(publish);
    client.send_text("{\"op\":\"ping\"}");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        let text = client.next_text(left).expect("the pong");
        if text.contains("\"pong\"") {
            break;
        }
    }
    // The closing handshake (RFC 6455 §7.1.2): the frames before the
    // daemon's Close (its own sample may come back) are read, so the
    // connection ends without a reset and without an error.
    close_cleanly(&mut client);
    drop(client);
    next_event(&rx, "connection_close");
    // RFC 6455 §4.2.2: a version the daemon does not speak is a 426.
    let mut refused = TcpStream::connect(&addr).unwrap();
    refused
        .write_all(
            build_handshake_request(&addr, "/topics/trade", "dGhlIHNhbXBsZSBub25jZQ==")
                .replace("Sec-WebSocket-Version: 13", "Sec-WebSocket-Version: 8")
                .as_bytes(),
        )
        .unwrap();
    refused
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    assert!(read_until_double_crlf(&mut refused).contains(" 426 "));
    assert_eq!(next_event(&rx, "handshake_reject")["code"], 426);
    // RFC 6455 §10.7: octets that are no UTF-8 are no request, a 400.
    let mut garbled = TcpStream::connect(&addr).unwrap();
    garbled
        .write_all(b"GET /topics/\xff HTTP/1.1\r\nHost: x\r\n\r\n")
        .unwrap();
    garbled
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    assert!(read_until_double_crlf(&mut garbled).contains(" 400 "));
    let garbled_reject = next_event(&rx, "handshake_reject");
    assert_eq!(garbled_reject["code"], 400);
    assert_eq!(garbled_reject["reason"], "the request is not UTF-8");
    // A client that closes with a frame of the daemon unread resets the
    // TCP connection; the daemon's read fails.
    let mut reset = json_client(&addr, "/topics/trade");
    let reset_local = reset.stream.local_addr().unwrap().to_string();
    reset.send_text("{\"op\":\"ping\"}");
    std::thread::sleep(Duration::from_millis(300));
    drop(reset);
    assert_eq!(next_event(&rx, "connection_close")["peer"], reset_local);
    let killed = Command::new("kill")
        .args(["-TERM", &child.0.id().to_string()])
        .status()
        .unwrap();
    assert!(killed.success());
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while child.0.try_wait().unwrap().is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "still running 10 s after SIGTERM"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let spans = spans.lock().unwrap().clone();
    let named = |name: &str| -> Vec<&serde_json::Value> {
        spans.iter().filter(|s| s["name"] == name).collect()
    };
    let connections = named("ws.connection");
    let of_peer = |peer: &str| {
        connections
            .iter()
            .find(|s| span_attribute(s, "net.peer") == Some(peer))
            .copied()
            .unwrap_or_else(|| panic!("no ws.connection of {peer}: {spans:?}"))
    };
    let open = of_peer(&local);
    assert_eq!(span_attribute(open, "http.status_code"), Some("101"));
    assert_eq!(open["kind"], 2, "{open}");
    assert_eq!(open["status"]["code"], 1, "{open}");
    assert!(open.get("parentSpanId").is_none(), "{open}");
    // §1 L6: no tenant, no `ws.tenant`.
    assert_eq!(span_attribute(open, "ws.tenant"), None, "{open}");
    assert_eq!(span_attribute(open, "ws.path"), Some("/topics/trade"));
    assert_eq!(
        span_attribute(open, "ws.subprotocol"),
        Some("zerodds-ws-bridge-1.0+json")
    );
    assert_eq!(span_attribute(open, "dds.topic"), Some("TradeE2E"));
    let trace = open["traceId"].as_str().unwrap();
    let root = open["spanId"].as_str().unwrap();
    let frame_in = named("ws.frame.in")
        .into_iter()
        .find(|s| span_attribute(s, "dds.topic") == Some("TradeE2E"))
        .unwrap_or_else(|| panic!("no ws.frame.in of the sample: {spans:?}"));
    assert_eq!(frame_in["traceId"], trace);
    assert_eq!(frame_in["parentSpanId"], root);
    assert_eq!(
        span_attribute(frame_in, "ws.message.bytes"),
        Some(publish.len().to_string().as_str())
    );
    let write = named("dds.write")
        .into_iter()
        .find(|s| s["parentSpanId"] == frame_in["spanId"])
        .unwrap_or_else(|| panic!("no dds.write under the frame: {spans:?}"));
    assert_eq!(write["traceId"], trace);
    assert_eq!(write["kind"], 3);
    assert_eq!(span_attribute(write, "dds.topic"), Some("TradeE2E"));
    let pong = named("ws.frame.out")
        .into_iter()
        .find(|s| {
            s["parentSpanId"] == root
                && span_attribute(s, "ws.message.bytes")
                    == Some("{\"op\":\"pong\"}".len().to_string().as_str())
        })
        .unwrap_or_else(|| panic!("no ws.frame.out of the pong: {spans:?}"));
    assert_eq!(pong["traceId"], trace);
    let rejected = connections
        .iter()
        .find(|s| span_attribute(s, "http.status_code") == Some("426"))
        .unwrap_or_else(|| panic!("no ws.connection with 426: {spans:?}"));
    assert_eq!(rejected["status"]["code"], 2, "{rejected}");
    assert_ne!(rejected["traceId"], trace);
    let garbled = connections
        .iter()
        .find(|s| span_attribute(s, "http.status_code") == Some("400"))
        .unwrap_or_else(|| panic!("no ws.connection with 400: {spans:?}"));
    assert_eq!(garbled["status"]["code"], 2, "{garbled}");
    assert_eq!(
        garbled["status"]["message"], "the request is not UTF-8",
        "{garbled}"
    );
    // §8.3: a connection that failed has status error with the reason.
    let reset = of_peer(&reset_local);
    assert_eq!(span_attribute(reset, "http.status_code"), Some("101"));
    assert_eq!(reset["status"]["code"], 2, "{reset}");
    assert!(
        reset["status"]["message"]
            .as_str()
            .is_some_and(|m| m.starts_with("read: ")),
        "{reset}"
    );
}

/// SIGTERM to the binary; it exits within 10 s.
#[cfg(unix)]
fn terminate(child: &mut Reaped) {
    let killed = std::process::Command::new("kill")
        .args(["-TERM", &child.0.id().to_string()])
        .status()
        .unwrap();
    assert!(killed.success());
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while child.0.try_wait().unwrap().is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "still running 10 s after SIGTERM"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Spec §8.3 with §6: a connection whose client takes no octet of a frame
/// within `ping_interval_ms` fails on the write timeout of its socket; its
/// `ws.connection` has status error with that reason. The client of
/// `/topics/trade` reads nothing; a second client publishes large samples
/// into the topic, which the daemon's reader hands to the first.
#[cfg(unix)]
#[test]
fn wsbridge_8_3_a_connection_that_fails_on_a_write_timeout_is_an_error_span() {
    let lease = zerodds_test_domains::lease_domain();
    let (endpoint, spans) = otlp_collector();
    let yaml = format!(
        "{}ping_interval_ms: 1000\nmax_send_queue_per_connection: 1\n",
        daemon_yaml(lease.id(), "127.0.0.1:0", "info")
    );
    let (mut child, rx) = spawn_daemon_with_env(
        "ws-otlp-write-timeout",
        &yaml,
        &[("OTEL_EXPORTER_OTLP_ENDPOINT", endpoint.as_str())],
    );
    next_event(&rx, "otlp_active");
    let addr = next_event(&rx, "running")["peer"]
        .as_str()
        .unwrap()
        .to_string();
    let (stalled, head) = Frames::handshake(
        &addr,
        "GET /topics/trade HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\n\
         Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n",
    );
    assert!(head.contains("101"), "{head}");
    let stalled_local = stalled.stream.local_addr().unwrap().to_string();
    // A client of no topic path: it gets none of the samples, and it
    // answers the daemon's Pings.
    let mut publisher = json_client(&addr, "/");
    let publisher_local = publisher.stream.local_addr().unwrap().to_string();
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let done_p = std::sync::Arc::clone(&done);
    let publishing = std::thread::spawn(move || {
        let big = "x".repeat(64 * 1024);
        for seq in 0..200 {
            publisher.send_text(&format!(
                "{{\"op\":\"publish\",\"topic\":\"TradeE2E\",\"data\":{{\"sym\":\"{big}\",\"seq\":{seq}}}}}"
            ));
        }
        while !done_p.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = publisher.next_text(Duration::from_millis(100));
        }
    });
    // SO_SNDTIMEO bounds each send that takes no octet; the client's
    // kernel takes a few octets now and then, so the failure can come a
    // few intervals later.
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        let line = rx
            .recv_timeout(left)
            .expect("the stalled connection is closed within 20 s");
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        if v["event"] == "connection_close" {
            assert_eq!(
                v["peer"], stalled_local,
                "the publisher {publisher_local} closed first"
            );
            break;
        }
    }
    done.store(true, std::sync::atomic::Ordering::SeqCst);
    publishing.join().unwrap();
    drop(stalled);
    terminate(&mut child);
    let spans = spans.lock().unwrap().clone();
    let failed = spans
        .iter()
        .find(|s| {
            s["name"] == "ws.connection"
                && span_attribute(s, "net.peer") == Some(stalled_local.as_str())
        })
        .unwrap_or_else(|| panic!("no ws.connection of {stalled_local}: {spans:?}"));
    assert_eq!(span_attribute(failed, "http.status_code"), Some("101"));
    assert_eq!(failed["status"]["code"], 2, "{failed}");
    assert_eq!(
        failed["status"]["message"],
        "write timeout: the client took no frame within ping_interval_ms",
        "{failed}"
    );
}

/// Spec §1 L6 with §8.1 and §8.3: the log line `connection_open` of a
/// connection bound to a tenant names the tenant in `tenant`, and its
/// `ws.connection` carries `ws.tenant`. (A connection of the participant of
/// `domain` has `tenant: ""` and no `ws.tenant`:
/// `wsbridge_8_1_the_binary_logs_json_lines_for_a_frame_exchange`,
/// `wsbridge_8_3_the_spans_of_a_connection_and_its_frames_reach_a_collector`.)
#[cfg(unix)]
#[test]
fn wsbridge_1_l6_the_log_line_and_the_span_name_the_tenant_of_a_connection() {
    let base = zerodds_test_domains::lease_domain();
    let other = zerodds_test_domains::lease_domain();
    let (endpoint, spans) = otlp_collector();
    let yaml = format!(
        "{}auth:\n  mode: bearer\n  bearer_token: \"s3cret\"\n  bearer_subject: \"alice\"\ntenants:\n  - name: plant-a\n    domain: {}\n    subjects: [alice]\n    topics:\n      - name: TradeE2E\n        type: TradeE2E\n        idl_file: \"{}\"\n        ws_path: /topics/trade\n        acl:\n          read: [alice]\n",
        daemon_yaml(base.id(), "127.0.0.1:0", "info"),
        other.id(),
        trade_idl_file()
    );
    let (mut child, rx) = spawn_daemon_with_env(
        "ws-tenant-log-span",
        &yaml,
        &[("OTEL_EXPORTER_OTLP_ENDPOINT", endpoint.as_str())],
    );
    next_event(&rx, "otlp_active");
    let addr = next_event(&rx, "running")["peer"]
        .as_str()
        .unwrap()
        .to_string();
    let key = "dGhlIHNhbXBsZSBub25jZQ==";
    let request = build_handshake_request(&addr, "/topics/trade", key).replacen(
        "\r\n\r\n",
        "\r\nAuthorization: Bearer s3cret\r\n\r\n",
        1,
    );
    let (mut alice, head) = Frames::handshake(&addr, &request);
    assert!(head.contains("101 Switching Protocols"), "{head}");
    let local = alice.stream.local_addr().unwrap().to_string();
    let open = next_event(&rx, "connection_open");
    assert_eq!(open["peer"], local);
    assert_eq!(open["subject"], "alice");
    assert_eq!(open["topic"], "TradeE2E");
    assert_eq!(open["tenant"], "plant-a");
    close_cleanly(&mut alice);
    drop(alice);
    next_event(&rx, "connection_close");
    terminate(&mut child);
    let spans = spans.lock().unwrap().clone();
    let root = spans
        .iter()
        .find(|s| {
            s["name"] == "ws.connection" && span_attribute(s, "net.peer") == Some(local.as_str())
        })
        .unwrap_or_else(|| panic!("no ws.connection of {local}: {spans:?}"));
    assert_eq!(span_attribute(root, "http.status_code"), Some("101"));
    assert_eq!(span_attribute(root, "auth.subject"), Some("alice"));
    assert_eq!(span_attribute(root, "ws.tenant"), Some("plant-a"));
    assert_eq!(root["status"]["code"], 1, "{root}");
}

/// A binary-mode connection (no `+json` subprotocol) to `/topics/trade`.
fn binary_client(addr: &str) -> Frames {
    let (frames, head) = Frames::handshake(
        addr,
        "GET /topics/trade HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\n\
         Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n",
    );
    assert!(head.contains("101"), "{head}");
    frames
}

/// A binary frame of a client: `ZDB1`, flags 0, the header `identifier`
/// and `body`, masked.
fn client_sample(identifier: [u8; 2], body: &[u8]) -> Vec<u8> {
    let mut payload = b"ZDB1\x00".to_vec();
    payload.extend_from_slice(&identifier);
    payload.extend_from_slice(&[0, 0]);
    payload.extend_from_slice(body);
    encode(&Frame::binary(payload).with_mask([3, 1, 4, 1])).unwrap()
}

/// [`trade`] in big endian (`CDR2_BE`, `PLAIN_CDR2` of a `@final` type).
fn trade_be(sym: &str, seq: i32) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&u32::try_from(sym.len() + 1).unwrap().to_be_bytes());
    out.extend_from_slice(sym.as_bytes());
    out.push(0);
    while out.len() % 4 != 0 {
        out.push(0);
    }
    out.extend_from_slice(&seq.to_be_bytes());
    out
}

/// §4.4: a sample whose DDS write fails is answered with the error frame
/// 500, a binary frame as well as a JSON sample frame, and counted in
/// `errors_total` (§8.2).
#[test]
fn wsbridge_4_4_a_dds_write_that_fails_is_an_error_frame_500() {
    let (cfg, _lease) = make_test_config(0);
    let mut handle = server::start(cfg).expect("daemon start");
    let metrics = handle.metrics.clone().expect("metrics");
    let mut binary = binary_client(&handle.local_addr);
    let mut json = json_client(&handle.local_addr, "/topics/trade");
    // The daemon's DataWriter of the topic is gone: each write fails.
    handle
        .runtime
        .remove_user_writer(handle.user_writers["TradeE2E"]);
    let errors = metrics.errors_total.get();
    binary
        .stream
        .write_all(&client_sample([0x00, 0x07], &trade("X", 1)))
        .unwrap();
    let text = binary
        .next_text(Duration::from_secs(3))
        .expect("an error frame");
    assert!(
        text.contains("\"op\":\"error\"") && text.contains("\"code\":500"),
        "{text}"
    );
    json.send_text(r#"{"topic":"TradeE2E","op":"publish","data":{"sym":"Y","seq":2}}"#);
    let text = json
        .next_text(Duration::from_secs(3))
        .expect("an error frame");
    assert!(
        text.contains("\"op\":\"error\"") && text.contains("\"code\":500"),
        "{text}"
    );
    assert_eq!(metrics.errors_total.get() - errors, 2);
    handle.shutdown();
}

/// §4.4: the control messages are text frames. A binary message without
/// the magic of §4.3 is refused with 400, also when it holds a control
/// message as UTF-8 JSON; the same message as a text frame is answered.
#[test]
fn wsbridge_4_4_a_control_message_in_a_binary_frame_is_refused() {
    let (cfg, _lease) = make_test_config(0);
    let mut handle = server::start(cfg).expect("daemon start");
    let mut client = json_client(&handle.local_addr, "/");
    for message in [
        r#"{"op":"ping"}"#,
        r#"{"op":"subscribe","topics":["TradeE2E"]}"#,
    ] {
        let frame = Frame::binary(message.as_bytes().to_vec()).with_mask([2, 7, 1, 8]);
        client.stream.write_all(&encode(&frame).unwrap()).unwrap();
        let text = client
            .next_text(Duration::from_secs(3))
            .expect("an error frame");
        assert!(
            text.contains("\"op\":\"error\"") && text.contains("\"code\":400"),
            "{message}: {text}"
        );
    }
    // Nothing else came: the next frame is the answer to a text ping.
    client.send_text(r#"{"op":"ping"}"#);
    assert_eq!(
        client.next_text(Duration::from_secs(3)).as_deref(),
        Some("{\"op\":\"pong\"}")
    );
    handle.shutdown();
}

/// §4.3: the writer of a topic with `idl_file` offers XCDR2 alone, in the
/// format of the type's extensibility. A binary frame under XCDR1 or under
/// another XCDR2 format is refused with 400 and writes nothing; a frame in
/// the type's format goes out under the byte order it names.
#[test]
fn wsbridge_4_3_a_binary_frame_in_a_format_the_typed_writer_does_not_offer_is_refused() {
    let (mut cfg, _lease) = make_test_config(0);
    cfg.ping_interval_ms = 0;
    let mut handle = server::start(cfg).expect("daemon start");
    let metrics = handle.metrics.clone().expect("metrics");
    let mut client = binary_client(&handle.local_addr);
    for identifier in [
        [0x00, 0x01],
        [0x00, 0x00],
        [0x00, 0x03],
        [0x00, 0x09],
        [0x00, 0x0b],
    ] {
        client
            .stream
            .write_all(&client_sample(identifier, &trade("X", 1)))
            .unwrap();
        let text = client
            .next_text(Duration::from_secs(3))
            .expect("an error frame");
        assert!(
            text.contains("\"op\":\"error\"") && text.contains("\"code\":400"),
            "{identifier:02x?}: {text}"
        );
    }
    assert_eq!(metrics.total(family::DDS_PUBLISHED), 0);
    // `PLAIN_CDR2` big endian is the type's format: the client's own
    // sample comes back through the daemon's DataReader under its header.
    let body = trade_be("BE", 7);
    client
        .stream
        .write_all(&client_sample([0x00, 0x06], &body))
        .unwrap();
    let frame = client
        .next_within(Duration::from_secs(3))
        .expect("the sample back");
    let mut want = b"ZDB1\x00\x00\x06\x00\x00".to_vec();
    want.extend_from_slice(&body);
    assert_eq!(frame.payload, want);
    handle.shutdown();
}

/// §4.3: all connections of a topic write through its one DataWriter. A
/// binary client writing big endian and a JSON client (little endian)
/// writing at the same time each get their samples out under their own
/// header: a third client sees every sample under the header its body
/// was encoded for.
#[test]
fn wsbridge_4_3_samples_of_concurrent_clients_go_out_under_their_own_header() {
    const EACH: i32 = 200;
    let (mut cfg, _lease) = make_test_config(0);
    cfg.ping_interval_ms = 0;
    let mut handle = server::start(cfg).expect("daemon start");
    let mut observer = binary_client(&handle.local_addr);
    let big = binary_client(&handle.local_addr);
    let little = json_client(&handle.local_addr, "/topics/trade");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let writers = [
        {
            let barrier = std::sync::Arc::clone(&barrier);
            let mut stream = big.stream.try_clone().unwrap();
            std::thread::spawn(move || {
                barrier.wait();
                for seq in 0..EACH {
                    stream
                        .write_all(&client_sample([0x00, 0x06], &trade_be("BE", seq)))
                        .unwrap();
                }
            })
        },
        {
            let barrier = std::sync::Arc::clone(&barrier);
            let mut stream = little.stream.try_clone().unwrap();
            std::thread::spawn(move || {
                barrier.wait();
                for seq in 0..EACH {
                    let text = format!(
                        r#"{{"topic":"TradeE2E","op":"publish","data":{{"sym":"LE","seq":{seq}}}}}"#
                    );
                    let frame = Frame::text(&text).with_mask([1, 3, 5, 7]);
                    stream.write_all(&encode(&frame).unwrap()).unwrap();
                }
            })
        },
    ];
    for w in writers {
        w.join().unwrap();
    }
    let (mut seen_be, mut seen_le) = (0, 0);
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while seen_be + seen_le < 2 * EACH && std::time::Instant::now() < deadline {
        let Some(frame) = observer.next_within(Duration::from_millis(500)) else {
            continue;
        };
        let p = &frame.payload;
        assert_eq!(&p[..5], b"ZDB1\x00", "a sample frame");
        let body = &p[9..];
        match [p[5], p[6]] {
            [0x00, 0x06] => {
                assert_eq!(&body[..7], b"\x00\x00\x00\x03BE\x00", "{p:02x?}");
                seen_be += 1;
            }
            [0x00, 0x07] => {
                assert_eq!(&body[..7], b"\x03\x00\x00\x00LE\x00", "{p:02x?}");
                seen_le += 1;
            }
            other => panic!("header {other:02x?}: {p:02x?}"),
        }
    }
    assert_eq!((seen_be, seen_le), (EACH, EACH));
    drop((big, little));
    handle.shutdown();
}
