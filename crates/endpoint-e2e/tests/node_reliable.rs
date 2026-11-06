// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Node reliable-stream endpoint: the pure-Node `AsyncReliableWriter`
//! (`endpoints/node/reliable.js`) is the reliable **sender** -- an async
//! producer path enqueues into a bounded queue (Promise-based backpressure),
//! a single drain loop owns the `ReliableSender` state, frames + sends
//! `WRITE_DATA` over a real `dgram` socket, fires `HEARTBEAT` on a timer, and
//! retransmits on `ACKNACK` until the send window drains. The shared Rust
//! reliable peer (`bind_reliable_peer`/`reliable_receive`) injects loss and
//! drives recovery. The samples are the `Ping` of the codec the Node
//! generator writes from the IDL, so the run checks IDL, generated codec and
//! runtime together. The unit and byte-golden self-test runs in
//! the endpoints-gate column of Node. Gated on `node`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::codegen::{Language, runtime_dir, scratch_dir, source};
use zerodds_endpoint_e2e::{
    RELIABLE_SAMPLES, assert_reliable_pings, bind_reliable_peer, reliable_receive,
};

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

/// The reliable app: the codec `codegen::source(Language::Node)` writes from
/// the IDL (`pingpong_gen.js`, calling `runtime/node/zdwire.js`) encodes
/// `{ seq: i, msg: "reliable" }`, and the SDK's async-decoupled
/// `AsyncReliableWriter` (`endpoints/node/reliable.js`) sends it.
const NODE_RELIABLE_APP: &str = r#"'use strict';
const zdwire = require(process.env.ZD_NODE_RUNTIME);
const { zd_enc_Ping } = require('./pingpong_gen.js');
const { AsyncReliableWriter } = require(process.env.ZD_NODE_ENDPOINT + '/reliable.js');

function encodePing(i) {
  const w = new zdwire.Writer(zdwire.Endianness.Little, 4);
  zd_enc_Ping(w, { seq: i, msg: 'reliable' });
  return w.finish();
}

async function main() {
  const port = parseInt(process.argv[2], 10);
  const n = parseInt(process.argv[3], 10);
  const writer = new AsyncReliableWriter('127.0.0.1', port, 0x0015);
  writer.start();
  for (let i = 0; i < n; i++) {
    await writer.submit(encodePing(i));
  }
  const ok = await writer.finish(20000);
  writer.close();
  console.log(ok ? `SENT ${n}` : `INCOMPLETE ${writer.drainError() || ''}`);
  process.exit(ok ? 0 : 1);
}

main();
"#;

/// The generated module as a CommonJS file: the runtime it calls by the name
/// `zdwire`, then the generated text, then the encoder exported.
fn gen_module() -> String {
    format!(
        "'use strict';\nconst zdwire = require(process.env.ZD_NODE_RUNTIME);\n\n{}\n\
         module.exports = {{ zd_enc_Ping }};\n",
        source(Language::Node)
    )
}

fn spawn_reliable(port: u16, n: usize) -> Child {
    let dir = scratch_dir(&format!("node_reliable_{port}"));
    std::fs::write(dir.join("pingpong_gen.js"), gen_module()).expect("write gen");
    std::fs::write(dir.join("app.js"), NODE_RELIABLE_APP).expect("write app");
    Command::new("node")
        .arg(dir.join("app.js"))
        .arg(port.to_string())
        .arg(n.to_string())
        .env("ZD_NODE_RUNTIME", runtime_dir("node").join("zdwire.js"))
        .env(
            "ZD_NODE_ENDPOINT",
            endpoints_node().canonicalize().expect("endpoints/node"),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn node reliable app")
}

fn run_loss(drop_every: Option<usize>, label: &str) {
    if !node_available() {
        eprintln!("SKIP {label}: `node` not on PATH");
        return;
    }
    let n = RELIABLE_SAMPLES;
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child = spawn_reliable(peer.port, n);
    let delivered = reliable_receive(&peer, child, label, n);
    assert_reliable_pings(label, &delivered, n);
}

/// reliable-endpoint 1.0 §5 point 3: the Node app sends its samples on the
/// reliable stream (DDS-XRCE 1.0 §8.4.10, stream id 0x80); the Rust peer
/// drops datagrams and answers with ACKNACK (§8.3.5.11), and every sample
/// arrives once and in order.
#[test]
#[ignore = "needs the Node toolchain; runs in integration-bindings"]
fn reliable_5_3_node_samples_arrive_in_order_despite_dropped_datagrams() {
    run_loss(Some(3), "node/loss");
}

/// reliable-endpoint 1.0 §5 point 4 and §2: the Node producer's enqueue into
/// the async-decoupled writer costs less than the inline send it replaces,
/// which enters the kernel with `sendto`; both values are printed.
#[test]
#[ignore = "needs the Node toolchain; runs in integration-bindings"]
fn reliable_5_4_node_the_producer_enqueue_costs_less_than_the_inline_send() {
    if !node_available() {
        eprintln!("SKIP node reliable bench: `node` not on PATH");
        return;
    }
    let peer = bind_reliable_peer(None).expect("bind reliable peer");
    let out = Command::new("node")
        .current_dir(endpoints_node())
        .arg("example_reliable.js")
        .arg(peer.port.to_string())
        .arg("0")
        .arg("bench")
        .output()
        .expect("run node example_reliable.js bench");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("BENCH"),
        "bench output missing:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("node reliable {}", stdout.trim());
    zerodds_endpoint_e2e::assert_enqueue_below_inline("node", &stdout);
}
