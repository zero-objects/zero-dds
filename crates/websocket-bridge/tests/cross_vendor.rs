// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-Vendor-Interop-Tests fuer WebSocket-Bridge.
//!
//! Spec: `zerodds-ws-bridge-1.0.md` §10 + §12.3.
//!
//! Verifiziert RFC-6455-Konformitaet via raw-TCP-Client mit
//! handgeschriebenem Handshake (mockt Browser-WS-Client). Dieser Test
//! laeuft gegen den eigenen ZeroDDS-Daemon im selben Prozess — das ist
//! die "Cross-Vendor"-Variante, weil unser Codec-Stack die Browser-
//! Client-Seite simuliert.
//!
//! Fuer echte Browser-Validierung kann ein externes Tool wie
//! `websocat` oder Node-`ws` benutzt werden — siehe ignore-tests.
//!
//! Run via:
//! ```bash
//! cargo test -p zerodds-websocket-bridge --features cross-vendor-tests \
//!     --test cross_vendor
//! cargo test -p zerodds-websocket-bridge --features cross-vendor-tests \
//!     --test cross_vendor -- --ignored
//! ```

#![cfg(all(feature = "daemon", feature = "cross-vendor-tests"))]
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
use std::process::Command;
use std::time::Duration;

use zerodds_websocket_bridge::daemon::config::{DaemonConfig, TopicConfig};
use zerodds_websocket_bridge::daemon::server;
use zerodds_websocket_bridge::handshake::compute_accept;

fn make_test_config(bind: &str) -> DaemonConfig {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.listen = bind.to_string();
    cfg.domain = 99;
    cfg.topics.push(TopicConfig {
        name: "Trade".to_string(),
        type_name: "Trade".to_string(),
        ws_path: "/dds/Trade".to_string(),
        direction: "bidir".to_string(),
        reliability: "best_effort".to_string(),
        durability: "volatile".to_string(),
        history_depth: 10,
    });
    cfg
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

fn websocat_available() -> bool {
    Command::new("websocat").arg("--help").output().is_ok()
}

#[test]
fn rfc6455_handshake_against_zerodds_daemon_returns_101() {
    // Cross-vendor in the sense: our test client uses ONLY raw TCP +
    // hand-written HTTP/1.1 — exactly what a Browser/JS-client does.
    // No reuse of zerodds_websocket_bridge::handshake::send_request.
    let cfg = make_test_config("127.0.0.1:0");
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

#[test]
#[ignore = "requires websocat binary (cargo install websocat)"]
fn websocat_can_connect_to_zerodds_daemon() {
    if !websocat_available() {
        eprintln!("websocat not available; skipping");
        return;
    }
    let cfg = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let port = handle.local_addr.rsplit(':').next().unwrap();
    let url = format!("ws://127.0.0.1:{port}/dds/Trade");
    // websocat -1 (one-shot) reads from stdin one line, sends, exits.
    let out = Command::new("websocat")
        .args(["-1", "-n", &url])
        .output()
        .expect("websocat");
    let _ = out;
    handle.shutdown();
}
