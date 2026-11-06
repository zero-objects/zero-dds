// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-vendor interop tests for the WebSocket bridge.
//!
//! Spec: `zerodds-ws-bridge-1.0.md` §10 + §12.3.
//!
//! Verifies RFC-6455 conformance via a raw TCP client with a
//! hand-written handshake (mocks a browser WS client). This test
//! runs against the own ZeroDDS daemon in the same process.
//!
//! **It is not a cross-vendor measurement**, whatever the file is called.
//! Both ends are this source tree, so an assumption the two sides share
//! cannot be contradicted here — and one was: `build_server_response`
//! echoed the client's `Sec-WebSocket-Extensions` offer back as an
//! agreement, in a form RFC 7692 §7.1.2.2 forbids in a server reply, and
//! this file never noticed because the client it writes offers no
//! extensions. A foreign client found it on the first connection.
//!
//! The foreign measurements live in `tests/websockets_py_interop_e2e.rs`
//! (Python `websockets` against the library half) and, for the daemon, in
//! the integration pillar's `bridges::websocket_bridge_against_python_websockets`.

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

use zerodds_websocket_bridge::daemon::config::{DaemonConfig, TopicConfig};
use zerodds_websocket_bridge::daemon::server;
use zerodds_websocket_bridge::handshake::compute_accept;

/// The configuration on a leased DDS domain (the host's share of the band,
/// `zerodds-test-domains`); the lease is held while the test runs.
fn make_test_config(bind: &str) -> (DaemonConfig, zerodds_test_domains::DomainLease) {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.listen = bind.to_string();
    let lease = zerodds_test_domains::lease_domain();
    cfg.domain = lease.id();
    cfg.topics.push(TopicConfig {
        name: "Trade".to_string(),
        type_name: "Trade".to_string(),
        ws_path: "/dds/Trade".to_string(),
        direction: "bidir".to_string(),
        reliability: "best_effort".to_string(),
        durability: "volatile".to_string(),
        history_depth: 10,
        idl_file: String::new(),
    });
    (cfg, lease)
}

fn build_request(path: &str, host: &str, key: &str) -> String {
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

#[test]
fn wsbridge_12_3_rfc6455_handshake_against_zerodds_daemon_returns_101() {
    // Cross-vendor in the sense: our test client uses ONLY raw TCP +
    // hand-written HTTP/1.1 — exactly what a Browser/JS-client does.
    // No reuse of zerodds_websocket_bridge::handshake::send_request.
    let (cfg, _lease) = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let server_addr = handle.local_addr.clone();

    let mut stream = TcpStream::connect(&server_addr).expect("tcp connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let key = "dGhlIHNhbXBsZSBub25jZQ=="; // RFC 6455 §1.3 example.
    let req = build_request("/dds/Trade", "127.0.0.1", key);
    stream.write_all(req.as_bytes()).expect("write request");

    let mut buf = [0u8; 4096];
    let mut acc = Vec::new();
    while !acc.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut buf).expect("read");
        if n == 0 {
            break;
        }
        acc.extend_from_slice(&buf[..n]);
    }
    let resp = String::from_utf8_lossy(&acc);
    assert!(resp.starts_with("HTTP/1.1 101"), "expected 101: {resp}");
    let expected_accept = compute_accept(key);
    assert!(
        resp.contains(&format!("Sec-WebSocket-Accept: {expected_accept}")),
        "Sec-WebSocket-Accept hash mismatch: {resp}"
    );

    handle.shutdown();
}
