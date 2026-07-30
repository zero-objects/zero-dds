// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Java reliable-stream endpoint: `endpoints/java`'s `AsyncReliableWriter`
//! (package `org.zerodds.endpoint`) is the reliable sender -- a producer
//! path enqueues into a `BlockingQueue`, a drain `Thread` owns the
//! `ReliableSender` state, frames + sends `WRITE_DATA` over a real
//! `DatagramSocket`, fires `HEARTBEAT` on a timer, and retransmits on
//! `ACKNACK` until the send window drains. The shared Rust reliable peer
//! (`bind_reliable_peer`/`reliable_receive`) injects loss and drives
//! recovery. Also runs the unit/byte-golden suite and the producer-latency
//! bench. Gated on `javac`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::{bind_reliable_peer, reliable_receive};

const N: usize = 12;

fn javac_available() -> bool {
    Command::new("javac")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn endpoints_java() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../endpoints/java")
}

/// The `org.zerodds.endpoint` state-machine + wire + async-writer classes,
/// laid out at their package path so `javac -d` derives the package
/// directory automatically.
fn core_sources() -> Vec<PathBuf> {
    let base = endpoints_java().join("org/zerodds/endpoint");
    [
        "ReliableWire.java",
        "ReliableSender.java",
        "ReliableReceiver.java",
        "AsyncReliableWriter.java",
    ]
    .iter()
    .map(|f| base.join(f))
    .collect()
}

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pp_java_rel_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("mkdir tmp");
    d
}

/// Compiles [`core_sources`] plus the given default-package driver file(s)
/// (resolved under `endpoints/java/`) into `out_dir`.
fn javac_build(out_dir: &Path, extra: &[&str]) {
    let java_dir = endpoints_java();
    let mut srcs = core_sources();
    for f in extra {
        srcs.push(java_dir.join(f));
    }
    let o = Command::new("javac")
        .arg("-nowarn")
        .arg("-d")
        .arg(out_dir)
        .args(&srcs)
        .output()
        .expect("run javac");
    assert!(
        o.status.success(),
        "javac failed for {extra:?}:\n{}",
        String::from_utf8_lossy(&o.stderr)
    );
}

fn run_loss(drop_every: Option<usize>, label: &str) {
    if !javac_available() {
        eprintln!("SKIP {label}: javac not on PATH");
        return;
    }
    let dir = tmp_dir(label.replace('/', "_").as_str());
    javac_build(&dir, &["ExampleReliable.java"]);

    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child: Child = Command::new("java")
        .arg("-cp")
        .arg(&dir)
        .arg("ExampleReliable")
        .arg(peer.port.to_string())
        .arg(N.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn java ExampleReliable");

    let delivered = reliable_receive(&peer, child, label, N);
    assert_eq!(delivered.len(), N, "{label}: delivered count");
    for (i, payload) in delivered.iter().enumerate() {
        assert!(payload.len() >= 4, "{label}: sample {i} too short");
        let v = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
        assert_eq!(v as usize, i, "{label}: sample {i} value (gap-free order)");
    }
}

#[test]
fn java_reliable_loss_recovery() {
    run_loss(Some(3), "java/loss");
}

#[test]
fn java_reliable_no_loss_baseline() {
    run_loss(None, "java/baseline");
}

#[test]
fn java_reliable_unit_and_golden() {
    if !javac_available() {
        eprintln!("SKIP java_reliable_unit_and_golden: javac not on PATH");
        return;
    }
    let dir = tmp_dir("unit");
    javac_build(&dir, &["ReliableSelfTest.java"]);

    // Best-effort: generate the Rust goldens and pass them for a byte-identical
    // check; the test also asserts the hardcoded golden bytes unconditionally.
    let gold = dir.join("gold");
    std::fs::create_dir_all(&gold).expect("mkdir gold");
    let gen_res = Command::new("cargo")
        .args(["run", "-q", "-p", "zerodds-endpoint-golden", "--"])
        .arg(&gold)
        .output();
    let hb = gold.join("golden_heartbeat_le.bin");
    let ak = gold.join("golden_acknack_le.bin");
    let mut cmd = Command::new("java");
    cmd.arg("-cp").arg(&dir).arg("ReliableSelfTest");
    if gen_res.map(|o| o.status.success()).unwrap_or(false) && hb.exists() && ak.exists() {
        cmd.arg(&hb).arg(&ak);
    }
    let o = cmd.output().expect("run ReliableSelfTest");
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        o.status.success() && out.contains("ALL OK"),
        "java unit/golden failed:\nstdout: {out}\nstderr: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn java_reliable_latency_bench() {
    if !javac_available() {
        eprintln!("SKIP java_reliable_latency_bench: javac not on PATH");
        return;
    }
    let dir = tmp_dir("bench");
    javac_build(&dir, &["ReliableBench.java"]);
    let o = Command::new("java")
        .arg("-cp")
        .arg(&dir)
        .arg("ReliableBench")
        .output()
        .expect("run ReliableBench");
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        out.contains("producer latency"),
        "java bench produced no figure:\nstdout: {out}\nstderr: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    eprintln!("java bench: {}", out.trim());
}
