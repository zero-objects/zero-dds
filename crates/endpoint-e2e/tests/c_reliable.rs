// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! C reliable-stream endpoint: the C SDK app plays the reliable **sender**
//! (submit + WRITE_DATA + HEARTBEAT + ACKNACK-driven retransmit) against the
//! shared Rust [`ReliablePeer`](zerodds_endpoint_e2e), which injects loss. Every
//! sample must arrive gap-free in order despite drops. Plus a producer-latency
//! micro-bench (decoupled enqueue vs. inline send). Gated on `gcc`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::{bind_reliable_peer, reliable_receive};

fn gcc_available() -> bool {
    Command::new("gcc").arg("--version").output().is_ok()
}

fn c_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../endpoints/c")
}

/// Compiles the reliable UDP sender app (POSIX sockets, default gcc std). `tag`
/// keeps concurrently-running tests from colliding on the output path.
fn build_app(tag: &str) -> PathBuf {
    let c = c_dir();
    let out = std::env::temp_dir().join(format!("zd_c_reliable_app_{}_{tag}", std::process::id()));
    let status = Command::new("gcc")
        .arg("-O2")
        .arg("-I")
        .arg(c.join("include"))
        .arg(c.join("test/reliable_udp_app.c"))
        .arg(c.join("src/zerodds_endpoint.c"))
        .arg("-o")
        .arg(&out)
        .status()
        .expect("spawn gcc");
    assert!(status.success(), "gcc failed to build reliable_udp_app.c");
    out
}

fn spawn_app(bin: &Path, port: u16, count: usize) -> Child {
    Command::new(bin)
        .arg(port.to_string())
        .arg(count.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn app")
}

fn assert_contiguous(got: &[Vec<u8>], n: usize, label: &str) {
    assert_eq!(
        got.len(),
        n,
        "{label}: expected {n} gap-free samples, got {}",
        got.len()
    );
    for (i, p) in got.iter().enumerate() {
        assert_eq!(
            p.as_slice(),
            &(i as u32).to_le_bytes(),
            "{label}: sample {i} content mismatch"
        );
    }
}

#[test]
fn c_reliable_loss_recovery() {
    if !gcc_available() {
        eprintln!("SKIP c_reliable_loss_recovery: `gcc` not on PATH");
        return;
    }
    let bin = build_app("loss");
    let n = 12usize;
    let peer = bind_reliable_peer(Some(3)).expect("bind reliable peer");
    let child = spawn_app(&bin, peer.port, n);
    let got = reliable_receive(&peer, child, "c/loss", n);
    assert_contiguous(&got, n, "c/loss");
}

#[test]
fn c_reliable_no_loss() {
    if !gcc_available() {
        eprintln!("SKIP c_reliable_no_loss: `gcc` not on PATH");
        return;
    }
    let bin = build_app("noloss");
    let n = 12usize;
    let peer = bind_reliable_peer(None).expect("bind reliable peer");
    let child = spawn_app(&bin, peer.port, n);
    let got = reliable_receive(&peer, child, "c/no-loss", n);
    assert_contiguous(&got, n, "c/no-loss");
}

#[test]
fn c_reliable_latency_bench() {
    if !gcc_available() {
        eprintln!("SKIP c_reliable_latency_bench: `gcc` not on PATH");
        return;
    }
    let c = c_dir();
    let out = std::env::temp_dir().join(format!("zd_c_reliable_bench_{}", std::process::id()));
    let status = Command::new("gcc")
        .arg("-O2")
        .arg("-std=c11")
        .arg("-I")
        .arg(c.join("include"))
        .arg(c.join("test/bench_reliable_async.c"))
        .arg(c.join("src/zerodds_reliable_async.c"))
        .arg(c.join("src/zerodds_endpoint.c"))
        .arg("-o")
        .arg(&out)
        .arg("-lpthread")
        .status()
        .expect("spawn gcc");
    assert!(
        status.success(),
        "gcc failed to build bench_reliable_async.c"
    );
    let res = Command::new(&out).output().expect("run bench");
    let stdout = String::from_utf8_lossy(&res.stdout);
    eprintln!("c reliable producer latency: {}", stdout.trim());
    assert!(
        stdout.contains("decoupled_enqueue_ns_per_op="),
        "bench output missing latency figure: {stdout}"
    );
    assert!(
        res.status.success(),
        "bench: decoupled enqueue was not <= inline send\n{stdout}\nstderr: {}",
        String::from_utf8_lossy(&res.stderr)
    );
}
