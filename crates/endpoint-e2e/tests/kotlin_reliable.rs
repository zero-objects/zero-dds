// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Kotlin reliable-stream endpoint: `endpoints/kotlin`'s `AsyncReliableWriter`
//! (package `zerodds`) is the reliable sender -- a producer path enqueues into
//! an `ArrayBlockingQueue`, a drain `Thread` owns the `ReliableSender` state,
//! frames + sends `WRITE_DATA` over a real `DatagramSocket`, fires `HEARTBEAT`
//! on a timer, and retransmits on `ACKNACK` until the send window drains. The
//! shared Rust reliable peer (`bind_reliable_peer`/`reliable_receive`) injects
//! loss and drives recovery. Also runs the unit/byte-golden suite. Gated on
//! `kotlinc`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::{bind_reliable_peer, reliable_receive};

const N: usize = 12;

fn kotlinc_available() -> bool {
    Command::new("kotlinc")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn endpoints_kotlin() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../endpoints/kotlin")
}

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pp_kt_rel_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("mkdir tmp");
    d
}

/// Compiles the reliable core (`src/Reliable.kt`) plus the given driver file
/// (resolved under `endpoints/kotlin/`) into a self-contained `app.jar`
/// (`-include-runtime` bundles the Kotlin stdlib so plain `java -cp` runs it).
/// Returns the jar path.
fn kotlinc_build(out_dir: &Path, driver: &str) -> PathBuf {
    let kt = endpoints_kotlin();
    let jar = out_dir.join("app.jar");
    let o = Command::new("kotlinc")
        .arg(kt.join("src/Reliable.kt"))
        .arg(kt.join(driver))
        .arg("-include-runtime")
        .arg("-d")
        .arg(&jar)
        .output()
        .expect("run kotlinc");
    assert!(
        o.status.success(),
        "kotlinc failed for {driver}:\n{}",
        String::from_utf8_lossy(&o.stderr)
    );
    jar
}

fn run_loss(drop_every: Option<usize>, label: &str) {
    if !kotlinc_available() {
        eprintln!("SKIP {label}: kotlinc not on PATH");
        return;
    }
    let dir = tmp_dir(label.replace('/', "_").as_str());
    let jar = kotlinc_build(&dir, "ReliableExample.kt");

    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child: Child = Command::new("java")
        .arg("-cp")
        .arg(&jar)
        .arg("ReliableExampleKt")
        .arg(peer.port.to_string())
        .arg(N.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn kotlin ReliableExample");

    let delivered = reliable_receive(&peer, child, label, N);
    assert_eq!(delivered.len(), N, "{label}: delivered count");
    for (i, payload) in delivered.iter().enumerate() {
        assert!(payload.len() >= 4, "{label}: sample {i} too short");
        let v = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
        assert_eq!(v as usize, i, "{label}: sample {i} value (gap-free order)");
    }
}

#[test]
fn kotlin_reliable_loss_recovery() {
    run_loss(Some(3), "kotlin/loss");
}

#[test]
fn kotlin_reliable_no_loss_baseline() {
    run_loss(None, "kotlin/baseline");
}

#[test]
fn kotlin_reliable_unit_and_golden() {
    if !kotlinc_available() {
        eprintln!("SKIP kotlin_reliable_unit_and_golden: kotlinc not on PATH");
        return;
    }
    let dir = tmp_dir("unit");
    let jar = kotlinc_build(&dir, "ReliableSelfTest.kt");

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
    cmd.arg("-cp").arg(&jar).arg("ReliableSelfTestKt");
    if gen_res.map(|o| o.status.success()).unwrap_or(false) && hb.exists() && ak.exists() {
        cmd.arg(&hb).arg(&ak);
    }
    let o = cmd.output().expect("run ReliableSelfTest");
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        o.status.success() && out.contains("ALL OK"),
        "kotlin unit/golden failed:\nstdout: {out}\nstderr: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}
