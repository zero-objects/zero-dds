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
//! The foreign measurement lives in `tests/websockets_py_interop_e2e.rs`
//! (Python `websockets` against the library half) and, for the daemon, in
//! `python_websockets_client_handshakes_and_pings_the_daemon` below. The
//! `websocat` test is a second foreign client. It is `#[ignore]`: run with
//! `--ignored` it fails where the binary is absent, which is every host here
//! so far, rather than passing without having measured anything.
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
use std::process::{Command, Stdio};
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

/// Gate for the Python `websockets` test below. Same convention as
/// `tests/websockets_py_interop_e2e.rs`.
fn python_ws_enabled() -> bool {
    std::env::var("WS_PYTHON").is_ok()
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
    // Run with `--ignored` means measure: a missing binary fails instead
    // of reporting interop that never happened.
    assert!(
        websocat_available(),
        "websocat is not installed (cargo install websocat)"
    );
    let cfg = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let port = handle.local_addr.rsplit(':').next().unwrap();
    let url = format!("ws://127.0.0.1:{port}/dds/Trade");
    // `-E`: exit on EOF of stdin (here at once), after the handshake and
    // the Close handshake. It was `-1 -n`: `--no-close` kept websocat
    // waiting for a message the daemon never sends, and the test hung
    // (measured on the Linux test host, websocat 1.x) instead of measuring anything.
    let mut child = Command::new("websocat")
        .args(["-E", &url])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("websocat");
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            panic!("websocat did not finish within 10 s");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let out = child.wait_with_output().expect("websocat output");
    // This was `let _ = out;`: websocat's result was discarded, so a
    // rejected handshake, a refused connection and a missing binary were
    // all indistinguishable from success. The exit status is the whole
    // point of running a foreign client.
    assert!(
        out.status.success(),
        "websocat must complete the RFC 6455 handshake against the daemon; \
         status={:?} stderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    handle.shutdown();
}

/// The daemon's accept loop against a foreign client that is actually
/// installed.
///
/// `websocat_can_connect_to_zerodds_daemon` above returns early when the
/// binary is absent and the runner then prints `ok` — on this host websocat
/// is not installed, so that test has never put a foreign octet on the
/// daemon's socket. Python's `websockets` 15.0.1 is installed, computes its
/// own `Sec-WebSocket-Key`, validates the `Sec-WebSocket-Accept` against
/// RFC 6455 §4.1, masks every frame it sends and drives the §5.5.2
/// ping/pong and §5.5.1 close handshakes.
///
/// `tests/websockets_py_interop_e2e.rs` drives the same client against the
/// library half; this one drives it against `daemon::server`, which is the
/// path behind `zerodds-dcps` and therefore the part that could not be
/// compiled — let alone run — before this crate became a workspace member.
#[test]
#[ignore = "needs python3 + the `websockets` module; set WS_PYTHON=1"]
fn python_websockets_client_handshakes_and_pings_the_daemon() {
    if !python_ws_enabled() {
        eprintln!("[skip] WS_PYTHON not set");
        return;
    }
    let cfg = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let port = handle.local_addr.rsplit(':').next().expect("port");
    let script = format!(
        r#"
from websockets.sync.client import connect
with connect("ws://127.0.0.1:{port}/dds/Trade") as ws:
    ws.ping(b"zerodds").wait(5)
    print("pong-ok")
"#
    );
    let out = Command::new("python3")
        .arg("-c")
        .arg(&script)
        .output()
        .expect("spawn python3");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "the `websockets` reference client must complete the RFC 6455 \
         handshake against the daemon; status={:?} stderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        stdout.trim(),
        "pong-ok",
        "the daemon must answer a masked Ping with a Pong carrying the same \
         application data (RFC 6455 §5.5.3); stdout={stdout}"
    );
    handle.shutdown();
}
