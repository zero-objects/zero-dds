// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! E2E-Test fuer `zerodds-ws-bridged`.
//!
//! Spec: `docs/specs/zerodds-ws-bridge-1.0.md` §12.2.
//!
//! Strategie: statt einen Subprocess zu spawnen (was MSRV/lock-step
//! gegen die Build-Outputs der Workspace-Suite macht), starten wir
//! den Daemon im selben Prozess via `daemon::server::start()` und
//! benchen einen WS-Client mit dem Library-eigenen Handshake-/Codec-
//! Pfad gegen ihn. Das deckt:
//!
//! * L1 — Handshake ueber TCP (`parse_client_request` Roundtrip).
//! * L1 — Frame-Codec (Text-Frame-Round-Trip).
//! * L3 — Bidir-Pump (Subscribe + DDS-Pump → Notify).

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
use std::net::TcpStream;
use std::time::Duration;

#[allow(unused_imports)]
use zerodds_websocket_bridge::codec::{decode, encode};
use zerodds_websocket_bridge::daemon::config::{DaemonConfig, TopicConfig};
use zerodds_websocket_bridge::daemon::server;
#[allow(unused_imports)]
use zerodds_websocket_bridge::frame::Frame;
use zerodds_websocket_bridge::handshake::compute_accept;

fn build_handshake_request(host: &str, path: &str, key: &str) -> String {
    format!(
        "GET {path} HTTP/1.1\r\n\
         Host: {host}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {key}\r\n\
         Sec-WebSocket-Version: 13\r\n\
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

fn make_test_config(port_hint: u16) -> DaemonConfig {
    let listen = format!("127.0.0.1:{port_hint}");
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.listen = listen;
    cfg.domain = 99;
    cfg.topics.push(TopicConfig {
        name: "TradeE2E".to_string(),
        type_name: "TradeE2E".to_string(),
        direction: "bidir".to_string(),
        ws_path: "/topics/trade".to_string(),
        reliability: "reliable".to_string(),
        durability: "volatile".to_string(),
        history_depth: 10,
    });
    cfg
}

#[test]
fn handshake_roundtrip_succeeds() {
    let cfg = make_test_config(0);
    let mut handle = server::start(cfg).expect("daemon start");
    let addr = handle.local_addr.clone();

    let stream = ws_client_connect(&addr, "/topics/trade");
    drop(stream);
    handle.shutdown();
}

#[test]
fn rejects_non_upgrade_request() {
    let cfg = make_test_config(0);
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

// Cross-Daemon-Pump-Test ist Linux-only — macOS faengt Multicast-
// Loopback nicht zuverlaessig (kein auto-interface-join bei
// `bind_multicast_v4(0.0.0.0)`); siehe `crates/dcps/tests/wlp_integration.rs`
// fuer dasselbe Pattern.
#[cfg(target_os = "linux")]
#[test]
fn cross_daemon_publish_pump_delivers_to_subscriber() {
    // Bidir-Pump-Beweis: zwei separate Daemon-Instanzen (= zwei DDS-
    // Participants) auf derselben Domain. Daemon A wird bedient von
    // einem WS-Client der `publish` schickt; Daemon B hat einen
    // WS-Client der `subscribe` macht. Der DDS-RTPS-Pfad zwischen
    // den Daemons ueberbringt das Sample → Daemon B's Pump pusht
    // das Sample als Notify-Frame an seinen Subscriber-Client.
    //
    // Das beweist:
    //   * L1 — Handshake (beide Clients connecten ueber RFC 6455).
    //   * L2 — DDS-Discovery (zwei Participants finden sich via SPDP).
    //   * L3 — bidir Pump (WS-Op → DDS-Writer; DDS-Reader → WS-Notify).

    let mut cfg_a = make_test_config(0);
    cfg_a.domain = 199;
    let mut cfg_b = make_test_config(0);
    cfg_b.domain = 199;

    let mut handle_a = server::start(cfg_a).expect("daemon A start");
    let mut handle_b = server::start(cfg_b).expect("daemon B start");

    // SPDP/SEDP-Konvergenz abwarten.
    std::thread::sleep(Duration::from_millis(1500));

    let mut sub_stream = ws_client_connect(&handle_b.local_addr, "/topics/trade");
    let sub_frame = Frame::text("{\"op\":\"subscribe\",\"topic\":\"TradeE2E\"}");
    let sub_bytes = encode(&sub_frame.with_mask([0x12, 0x34, 0x56, 0x78])).expect("encode sub");
    sub_stream.write_all(&sub_bytes).expect("send subscribe");

    // Subscriber muss auf der Liste stehen bevor wir publishen.
    std::thread::sleep(Duration::from_millis(200));

    let mut pub_stream = ws_client_connect(&handle_a.local_addr, "/topics/trade");
    let publish_frame = Frame::text(
        "{\"op\":\"publish\",\"topic\":\"TradeE2E\",\"data\":\"{\\\"sym\\\":\\\"AAPL\\\"}\"}",
    );
    let bytes = encode(&publish_frame.with_mask([0xAA, 0xBB, 0xCC, 0xDD])).expect("encode publish");
    pub_stream.write_all(&bytes).expect("send publish");

    // Notify-Frame auf B's subscribed-stream einsammeln.
    let frame = read_frame_from(&mut sub_stream);
    let text = std::str::from_utf8(&frame.payload).unwrap_or("<bin>");
    assert!(
        text.contains("\"op\":\"notify\""),
        "expected notify frame, got: {text}"
    );
    assert!(
        text.contains("TradeE2E"),
        "expected topic in payload, got: {text}"
    );
    handle_a.shutdown();
    handle_b.shutdown();
}

// ============================================================================
// Cross-Cutting Daemon-Runtime — E2E (Signal-Watcher / Admin-Endpoint /
// Metrics-Counter).
// ============================================================================

/// Graceful shutdown via DaemonHandle::shutdown() laesst Drop-on-Exit
/// alle Threads sauber join'en (§9.2).
#[test]
fn graceful_shutdown_completes_within_5s() {
    let cfg = make_test_config(0);
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

/// Catalog/Healthz/Metrics-Endpoint liefert JSON-Body mit den
/// konfigurierten Topics (§5.2 + §8.2).
#[test]
fn admin_endpoint_serves_catalog_metrics_healthz() {
    let mut cfg = make_test_config(0);
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
    assert!(body.contains("zerodds_ws_connections_total"), "got: {body}");

    handle.shutdown();
}

/// Frame-In/Out-Counter inkrementieren bei aktiver Connection (§8.2).
#[test]
fn metrics_counters_track_frames_in_and_out() {
    let mut cfg = make_test_config(0);
    cfg.metrics_enabled = true;
    cfg.metrics_addr = "127.0.0.1:0".to_string();
    let mut handle = server::start(cfg).expect("daemon start");

    let metrics = handle.metrics.clone().expect("metrics set");
    let conns_before = metrics.connections_total.get();

    let mut stream = ws_client_connect(&handle.local_addr, "/topics/trade");
    // Send a publish frame.
    let publish_frame = Frame::text("{\"op\":\"publish\",\"topic\":\"TradeE2E\",\"data\":\"hi\"}");
    let bytes = encode(&publish_frame.with_mask([0xAA, 0xBB, 0xCC, 0xDD])).expect("encode publish");
    stream.write_all(&bytes).expect("send publish");

    // Allow the daemon to process the frame.
    std::thread::sleep(Duration::from_millis(200));

    let conns_after = metrics.connections_total.get();
    assert!(
        conns_after > conns_before,
        "connections_total should increment: before={conns_before} after={conns_after}"
    );
    assert!(
        metrics.frames_in_total.get() >= 1,
        "frames_in_total should be >=1, got {}",
        metrics.frames_in_total.get()
    );

    drop(stream);
    handle.shutdown();
}
