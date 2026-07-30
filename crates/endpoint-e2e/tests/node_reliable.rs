// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Node reliable-stream endpoint: the pure-Node `AsyncReliableWriter`
//! (`endpoints/node/reliable.js`) is the reliable **sender** -- an async
//! producer path enqueues into a bounded queue (Promise-based backpressure),
//! a single drain loop owns the `ReliableSender` state, frames + sends
//! `WRITE_DATA` over a real `dgram` socket, fires `HEARTBEAT` on a timer, and
//! retransmits on `ACKNACK` until the send window drains. The shared Rust
//! reliable peer (`bind_reliable_peer`/`reliable_receive`) injects loss and
//! drives recovery. Also runs the unit/byte-golden self-test, cross-checked
//! byte-identical against the Rust golden files. Gated on `node`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::{bind_reliable_peer, reliable_receive};

const N: usize = 12;

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

fn spawn_reliable(port: u16, n: usize) -> Child {
    Command::new("node")
        .current_dir(endpoints_node())
        .arg("example_reliable.js")
        .arg(port.to_string())
        .arg(n.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn node example_reliable.js")
}

fn run_loss(drop_every: Option<usize>, label: &str) {
    if !node_available() {
        eprintln!("SKIP {label}: `node` not on PATH");
        return;
    }
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child = spawn_reliable(peer.port, N);
    let delivered = reliable_receive(&peer, child, label, N);
    assert_eq!(delivered.len(), N, "{label}: delivered count");
    for (i, payload) in delivered.iter().enumerate() {
        assert!(payload.len() >= 4, "{label}: sample {i} too short");
        let v = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
        assert_eq!(v as usize, i, "{label}: sample {i} value (gap-free order)");
    }
}

#[test]
fn node_reliable_loss_recovery() {
    run_loss(Some(3), "node/loss");
}

#[test]
fn node_reliable_no_loss_baseline() {
    run_loss(None, "node/baseline");
}

#[test]
fn node_reliable_unit_and_golden() {
    if !node_available() {
        eprintln!("SKIP node_reliable_unit_and_golden: `node` not on PATH");
        return;
    }
    // Best-effort: generate the Rust goldens and pass them for a byte-identical
    // cross-check; the self-test also asserts the hardcoded golden bytes
    // unconditionally.
    let gold = std::env::temp_dir().join(format!("zd_node_rel_gold_{}", std::process::id()));
    std::fs::create_dir_all(&gold).expect("mkdir gold");
    let gen_res = Command::new("cargo")
        .args(["run", "-q", "-p", "zerodds-endpoint-golden", "--"])
        .arg(&gold)
        .output();
    let hb = gold.join("golden_heartbeat_le.bin");
    let ak = gold.join("golden_acknack_le.bin");

    let mut cmd = Command::new("node");
    cmd.current_dir(endpoints_node())
        .arg("reliable_selftest.js");
    if gen_res.map(|o| o.status.success()).unwrap_or(false) && hb.exists() && ak.exists() {
        cmd.arg(&hb).arg(&ak);
    }
    let o = cmd.output().expect("run reliable_selftest.js");
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        o.status.success() && out.contains("ALL OK"),
        "node unit/golden failed:\nstdout: {out}\nstderr: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}
