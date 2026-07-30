// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Node endpoint ping-pong: the native Node wire-core (`endpoints/node`,
//! `zerodds.js`) marshals a `@final` `Ping` (inline XCDR2, byte-identical to
//! the Rust core), the endpoint SDK does the XRCE `WRITE_DATA` framing, and a
//! real `dgram` UDP socket carries the bytes to the Rust peer. `sync` = the
//! caller owns the poll loop (`Client.poll()`); `async` = the async iterator
//! drains the socket (`AsyncReader.stream()`) -- the idiomatic Node model.
//! Gated on `node`.
//!
//! Unlike the Java/C# harnesses, `Ping`/`Pong` need no generated TypeSupport:
//! the fields (`long seq; string msg;`) are written directly with the Node
//! `Writer`, whose bytes match the Rust `zerodds_cdr` XCDR2 codec the peer
//! decodes with.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::{Peer, bind_peer, ping_pong};

fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn endpoints_node() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../endpoints/node")
}

fn spawn_app(mode: &str, port: u16) -> Child {
    Command::new("node")
        .current_dir(endpoints_node())
        .arg("example_ping_pong.js")
        .arg(mode)
        .arg(port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn node example_ping_pong.js")
}

fn endpoint_case(mode: &str) {
    if !node_available() {
        eprintln!("SKIP node endpoint {mode}: `node` not on PATH");
        return;
    }
    let peer: Peer = bind_peer().expect("bind peer");
    let child = spawn_app(mode, peer.port);
    ping_pong(&peer, child, &format!("node/{mode}"));
}

#[test]
fn node_endpoint_sync() {
    endpoint_case("sync");
}

#[test]
fn node_endpoint_async() {
    endpoint_case("async");
}
