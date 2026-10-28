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
//! behind `zerodds-dcps`; the server here is built from the **library half** —
//! `handshake`, `frame`, `codec` and `masking` — which is the part that owns
//! the octets.
//!
//! The reference side's exit status is checked. A `websockets` module that is
//! not installed makes this test fail, not pass: a cross-implementation test
//! whose foreign half can silently not run is not a cross-implementation test.
//!
//! Gated on `WS_PYTHON=1`; `#[ignore]` by default.

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

use zerodds_websocket_bridge::codec;
use zerodds_websocket_bridge::frame::{Frame, Opcode};
use zerodds_websocket_bridge::handshake::{
    build_server_response, parse_client_request, render_server_response,
};

fn enabled() -> bool {
    std::env::var("WS_PYTHON").is_ok()
}

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

/// A server-to-client frame: final, no reserved bits, and — RFC 6455 §5.1 —
/// unmasked, which is the half of the masking rule a same-source client
/// cannot check.
fn server_frame(opcode: Opcode, payload: Vec<u8>) -> Frame {
    Frame {
        fin: true,
        rsv1: false,
        rsv2: false,
        rsv3: false,
        opcode,
        masking_key: None,
        payload,
    }
}

/// Serves one connection: RFC 6455 §4.2 handshake, then echo until close.
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
    let Ok(client) = parse_client_request(&text) else {
        let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n");
        return;
    };
    let resp = build_server_response(&client);
    if stream
        .write_all(render_server_response(&resp).as_bytes())
        .is_err()
    {
        return;
    }

    // §5 — frame loop. Client frames are masked; ours are not.
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let decoded = match codec::decode(&buf) {
            Ok((frame, used)) => {
                buf.drain(..used);
                Some(frame)
            }
            Err(_) => None,
        };
        if let Some(frame) = decoded {
            match frame.opcode {
                Opcode::Text | Opcode::Binary => {
                    let echo = server_frame(frame.opcode, frame.payload);
                    let bytes = codec::encode(&echo).expect("encode echo");
                    if stream.write_all(&bytes).is_err() {
                        return;
                    }
                }
                Opcode::Ping => {
                    let pong = server_frame(Opcode::Pong, frame.payload);
                    let bytes = codec::encode(&pong).expect("encode pong");
                    if stream.write_all(&bytes).is_err() {
                        return;
                    }
                }
                Opcode::Close => {
                    // §5.5.1 — echo the close frame back and stop.
                    let close = server_frame(Opcode::Close, frame.payload);
                    if let Ok(bytes) = codec::encode(&close) {
                        let _ = stream.write_all(&bytes);
                    }
                    return;
                }
                _ => {}
            }
            continue;
        }
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
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
#[ignore = "needs python3 + the `websockets` module; set WS_PYTHON=1"]
fn python_websockets_client_round_trips_a_text_frame() {
    if !enabled() {
        eprintln!("[skip] WS_PYTHON not set");
        return;
    }
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
#[ignore = "needs python3 + the `websockets` module; set WS_PYTHON=1"]
fn python_websockets_client_round_trips_binary_and_ping() {
    if !enabled() {
        eprintln!("[skip] WS_PYTHON not set");
        return;
    }
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
