// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Live WebSocket interop: the Python `websockets` reference client against a
//! server built from this crate's own handshake and framing.
//!
//! `tests/cross_vendor.rs` states plainly what it is — "our codec stack
//! simulates the browser client side" — and a client written from the same
//! source tree as the server cannot contradict a shared assumption. This test
//! puts a foreign implementation on the other end: `websockets` computes its
//! own `Sec-WebSocket-Key`, validates our `Sec-WebSocket-Accept` against
//! RFC 6455 §4.1, masks every client frame with its own key, validates the
//! UTF-8 of every text frame it receives and drives the §5.5.1 close
//! handshake. Every one of those is a place where the two sides can disagree.
//!
//! The daemon's accept loop sits behind the `daemon` feature and therefore
//! behind `zerodds-dcps`; the echo server here is built from the **library
//! half** — `handshake`, `permessage_deflate` and `connection` — which is
//! the part that owns the octets. The tests of the module `daemon` put the
//! same client in front of `zerodds-ws-bridged`.
//!
//! The reference side's exit status is checked. A `websockets` module that is
//! not installed makes this test fail, not pass: a cross-implementation test
//! whose foreign half can silently not run is not a cross-implementation test.
//!
//! `#[ignore]`d; running them (`--run-ignored`, the weekly job
//! `integration-bridges-vendor`) measures.

#![cfg(feature = "std")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::uninlined_format_args
)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use zerodds_websocket_bridge::connection::{Event, Limits, Receiver, Sender};
use zerodds_websocket_bridge::handshake::{
    build_server_response, parse_client_request, render_server_response,
};
use zerodds_websocket_bridge::masking::Role;
use zerodds_websocket_bridge::permessage_deflate::{accept_offers, render_accept};

/// A one-connection RFC 6455 echo server over this crate's library half.
struct EchoServer {
    port: u16,
    join: Option<JoinHandle<()>>,
}

impl EchoServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind tcp");
        let port = listener.local_addr().expect("local_addr").port();
        let join = thread::spawn(move || {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            serve(stream);
        });
        Self {
            port,
            join: Some(join),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("ws://127.0.0.1:{}{}", self.port, path)
    }
}

impl Drop for EchoServer {
    fn drop(&mut self) {
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

/// Serves one connection: RFC 6455 §4.2 handshake with the
/// `permessage-deflate` the client offers (RFC 7692 §7.1), then echo until
/// close. The frames go through the connection layer: the client's are
/// validated, ours leave unmasked (§5.1), which is the half of the masking
/// rule a same-source client cannot check.
fn serve(mut stream: TcpStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("read timeout");

    // §4.1 — read the client's opening handshake up to the blank line.
    let mut req = Vec::new();
    let mut byte = [0u8; 1];
    while !req.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(0) | Err(_) => return,
            Ok(_) => req.push(byte[0]),
        }
    }
    let text = String::from_utf8_lossy(&req).into_owned();
    let client = match parse_client_request(&text) {
        Ok(client) => client,
        Err(e) => {
            let _ = stream.write_all(e.render_error_response().as_bytes());
            return;
        }
    };
    let mut resp = build_server_response(&client);
    let mut receiver = Receiver::new(Role::Server, Limits::default());
    let mut sender = Sender::server();
    if let Some(params) = accept_offers(&client.extensions) {
        resp.extensions.push(render_accept(&params));
        receiver = receiver.with_permessage_deflate(&params);
        sender = sender.with_permessage_deflate(&params);
    }
    if stream
        .write_all(render_server_response(&resp).as_bytes())
        .is_err()
    {
        return;
    }

    // §5 — frame loop.
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let wire = match receiver.feed(&mut buf) {
            Ok(Some(Event::Message(m))) => sender.message(m.is_text, &m.payload).ok(),
            Ok(Some(Event::Ping(data))) => sender.pong(&data).ok(),
            Ok(Some(Event::Pong(_))) => continue,
            Ok(Some(Event::Close(body))) => {
                // §5.5.1 — answer the Close with its status and stop.
                if let Ok(Some(wire)) = sender.close_received(body.as_ref()) {
                    let _ = stream.write_all(&wire);
                }
                return;
            }
            Ok(None) => match stream.read(&mut chunk) {
                Ok(0) | Err(_) => return,
                Ok(n) => {
                    buf.extend_from_slice(&chunk[..n]);
                    continue;
                }
            },
            Err(failure) => {
                if let Some(wire) = sender.fail(&failure) {
                    let _ = stream.write_all(&wire);
                }
                return;
            }
        };
        match wire {
            Some(wire) if stream.write_all(&wire).is_ok() => {}
            _ => return,
        }
    }
}

/// Runs a `websockets` script and returns its stdout, failing on a
/// non-zero exit.
fn run_python(script: &str) -> String {
    let out = Command::new("python3")
        .arg("-c")
        .arg(script)
        .output()
        .expect("spawn python3");
    assert!(
        out.status.success(),
        "the `websockets` reference client failed: status={:?} stderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
#[ignore = "needs python3 with the `websockets` module; runs in integration-bridges-vendor"]
fn python_websockets_client_round_trips_a_text_frame() {
    let srv = EchoServer::start();
    let script = format!(
        r#"
from websockets.sync.client import connect
with connect("{url}") as ws:
    ws.send("hallo-von-python")
    print(ws.recv())
"#,
        url = srv.url("/dds/Trade")
    );
    let got = run_python(&script);
    assert_eq!(
        got, "hallo-von-python",
        "a foreign client's masked text frame must survive unmask + re-encode"
    );
    eprintln!("OK: python-websockets round-tripped a text frame through the library codec");
}

#[test]
#[ignore = "needs python3 with the `websockets` module; runs in integration-bridges-vendor"]
fn python_websockets_client_round_trips_binary_and_ping() {
    let srv = EchoServer::start();
    // A payload of 200 bytes crosses RFC 6455 §5.2's 7-bit length boundary
    // (126 selects the 16-bit extended form), so the length encoding is
    // exercised rather than assumed.
    let script = format!(
        r#"
from websockets.sync.client import connect
payload = bytes(range(256)) * 2
with connect("{url}") as ws:
    ws.ping().wait(5)
    ws.send(payload)
    got = ws.recv()
    print("len", len(got), "eq", got == payload)
"#,
        url = srv.url("/dds/Trade")
    );
    let got = run_python(&script);
    assert_eq!(
        got, "len 512 eq True",
        "a 512-byte binary frame and a ping must both survive; got: {got:?}"
    );
    eprintln!("OK: python-websockets round-tripped a 512-byte binary frame + ping/pong");
}

#[test]
#[ignore = "needs python3 with the `websockets` module; runs in integration-bridges-vendor"]
fn python_websockets_client_negotiates_permessage_deflate_and_round_trips() {
    // `websockets` offers permessage-deflate by default. What it names as
    // the extension in use is what the two sides agreed on (RFC 7692
    // §7.1); its zlib inflates what this crate compressed and this crate
    // inflates what its zlib compressed, three messages over one context
    // (§7.2), the last one in fragments.
    let srv = EchoServer::start();
    let script = format!(
        r#"
from websockets.sync.client import connect
text = "{{\"topic\":\"Chat::Message\",\"op\":\"publish\",\"data\":{{\"n\":%d}}}}" * 200
with connect("{url}") as ws:
    print([type(e).__name__ for e in ws.protocol.extensions])
    for n in range(3):
        ws.send(text % ((n,) * 200))
        print(ws.recv() == text % ((n,) * 200))
    ws.send([text[:1000], text[1000:5000], text[5000:]])
    print(ws.recv() == text)
    ws.close(code=4000, reason="done")
    print(ws.protocol.close_code)
"#,
        url = srv.url("/dds/Trade")
    );
    let got = run_python(&script);
    assert_eq!(
        got, "['PerMessageDeflate']\nTrue\nTrue\nTrue\nTrue\n4000",
        "the extension in use, four echoes, the close code this side answered with"
    );
}

/// The same foreign client in front of the daemon.
#[cfg(feature = "daemon")]
mod daemon {
    use super::run_python;
    use zerodds_websocket_bridge::daemon::config::DaemonConfig;
    use zerodds_websocket_bridge::daemon::server;

    #[test]
    #[ignore = "needs python3 with the `websockets` module; runs in integration-bridges-vendor"]
    fn python_websockets_client_talks_compressed_to_the_daemon_and_closes() {
        // The daemon agrees to the client's permessage-deflate (RFC 7692
        // §7.1), answers the control messages of the bridge over the
        // compressed connection, and answers the client's Close with a
        // Close of the same status (RFC 6455 §5.5.1), which `websockets`
        // reports as the close code it received.
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.listen = "127.0.0.1:0".to_string();
        let lease = zerodds_test_domains::lease_domain();
        cfg.domain = lease.id();
        let mut handle = server::start(cfg).expect("daemon start");
        let script = format!(
            r#"
from websockets.sync.client import connect
with connect("ws://{addr}/", subprotocols=["zerodds-ws-bridge-1.0+json"]) as ws:
    print([type(e).__name__ for e in ws.protocol.extensions])
    for n in range(3):
        ws.send('{{"op":"ping"}}')
        print(ws.recv())
    ws.ping().wait(5)
    ws.close(code=4001, reason="done")
    print(ws.protocol.close_code)
"#,
            addr = handle.local_addr
        );
        let got = run_python(&script);
        assert_eq!(
            got,
            "['PerMessageDeflate']\n{\"op\":\"pong\"}\n{\"op\":\"pong\"}\n{\"op\":\"pong\"}\n4001"
        );
        handle.shutdown();
    }

    #[test]
    #[ignore = "needs python3 with the `websockets` module; runs in integration-bridges-vendor"]
    fn ws_10_2_the_daemon_checks_the_origin_python_websockets_sends() {
        // `websockets` writes the `Origin` field of RFC 6455 §4.1 item 8
        // itself; the daemon expects one origin and requires the field.
        // The client reports the HTTP status of a refused handshake.
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.listen = "127.0.0.1:0".to_string();
        let lease = zerodds_test_domains::lease_domain();
        cfg.domain = lease.id();
        cfg.origin_allowed = vec!["https://app.example.org".to_string()];
        cfg.origin_required = true;
        let mut handle = server::start(cfg).expect("daemon start");
        let script = format!(
            r#"
from websockets.sync.client import connect
from websockets.exceptions import InvalidStatus
url = "ws://{addr}/"
proto = ["zerodds-ws-bridge-1.0+json"]
with connect(url, subprotocols=proto, origin="https://app.example.org") as ws:
    ws.send('{{"op":"ping"}}')
    print(ws.recv())
for origin in ("https://evil.example", "http://app.example.org", None):
    try:
        with connect(url, subprotocols=proto, origin=origin) as ws:
            print("connected")
    except InvalidStatus as e:
        print(e.response.status_code)
"#,
            addr = handle.local_addr
        );
        let got = run_python(&script);
        assert_eq!(got, "{\"op\":\"pong\"}\n403\n403\n403");
        handle.shutdown();
    }
}
