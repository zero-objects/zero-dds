// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Go reliable-stream endpoint: the pure-Go reliable state machine + frame
//! codec (`endpoints/go/reliable.go`) driven by an async-decoupled writer
//! (`endpoints/go/reliable_app`) against the shared Rust reliable peer, which
//! injects loss. Proves loss recovery (peer drops → app retransmits on
//! ACKNACK → all samples delivered gap-free), the unit + byte-golden suite,
//! the in-process runnable example, and the producer-latency decoupling.
//! Gated on `go`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::{bind_reliable_peer, reliable_receive};

fn go_available() -> bool {
    Command::new("go").arg("version").output().is_ok()
}

fn endpoints_go() -> PathBuf {
    PathBuf::from(format!("{}/../../endpoints/go", env!("CARGO_MANIFEST_DIR")))
}

/// Builds `./<pkg>` (a `main` package under `endpoints/go`) with `go build`
/// into a uniquely-tagged temp binary. The module (`zeroddsendpoint`) already
/// lives in place, so no scaffolding/copying is needed — just build in-tree.
fn go_build(pkg: &str, tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("zd_go_reliable_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let bin = dir.join(pkg.trim_start_matches("./"));
    let out = Command::new("go")
        .arg("build")
        .arg("-o")
        .arg(&bin)
        .arg(format!("./{pkg}"))
        .current_dir(endpoints_go())
        .output()
        .expect("spawn go build");
    assert!(
        out.status.success(),
        "go build ./{pkg} failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    bin
}

fn spawn_reliable_app(bin: &std::path::Path, port: u16, n: usize) -> Child {
    Command::new(bin)
        .arg(port.to_string())
        .arg(n.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn go reliable_app")
}

fn decode_u32(b: &[u8]) -> u32 {
    assert_eq!(b.len(), 4, "sample must be 4 bytes (u32 LE)");
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn run_loss(drop_every: Option<usize>, tag: &str, label: &str) {
    if !go_available() {
        eprintln!("SKIP {label}: `go` not on PATH");
        return;
    }
    let n = 12usize;
    let bin = go_build("reliable_app", tag);
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child = spawn_reliable_app(&bin, peer.port, n);
    let delivered = reliable_receive(&peer, child, label, n);
    assert_eq!(delivered.len(), n, "{label}: delivered count");
    for (i, payload) in delivered.iter().enumerate() {
        assert_eq!(
            decode_u32(payload),
            i as u32,
            "{label}: gap/misorder at sample {i}"
        );
    }
}

#[test]
fn go_reliable_loss_recovery() {
    // Peer drops every 3rd distinct sample once → the app must retransmit.
    run_loss(Some(3), "loss", "go/loss");
}

#[test]
fn go_reliable_no_loss() {
    run_loss(None, "noloss", "go/baseline");
}

#[test]
fn go_reliable_unit_and_golden() {
    if !go_available() {
        eprintln!("SKIP go_reliable_unit_and_golden: `go` not on PATH");
        return;
    }
    // Best-effort: generate the Rust goldens so the byte-golden test can also
    // diff against the cargo-produced files (`endpoints/go/reliable_test.go`
    // reads `$GOLDEN_DIR`, the same convention `wire_test.go`'s
    // TestByteIdentity uses); it also asserts hardcoded golden bytes
    // unconditionally, so this generation step is not required for the test
    // itself to be meaningful.
    let gold = std::env::temp_dir().join(format!("zd_go_reliable_gold_{}", std::process::id()));
    std::fs::create_dir_all(&gold).expect("mkdir gold");
    let gen_ok = Command::new("cargo")
        .args(["run", "-q", "-p", "zerodds-endpoint-golden", "--"])
        .arg(&gold)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    // Scoped to the reliable-stream tests this task owns (`-run`): the root
    // `zerodds` package also holds pre-existing tests (`wire_test.go`'s
    // TestByteIdentity, `async_test.go`, `sync_test.go`) with their own,
    // separately-maintained fixture setup that is out of scope here.
    let mut cmd = Command::new("go");
    cmd.arg("test")
        .arg("-run")
        .arg("^Test(Submit|PendingHeartbeat|PendingAcknack|RecvAcknack|RecvData|Reset|EndToEnd|Config|SeqLtGt|ByteGolden|FrameRoundTrip|ReliableAsyncWriter)")
        .arg("-v")
        .arg(".")
        .current_dir(endpoints_go());
    if gen_ok
        && gold.join("golden_heartbeat_le.bin").exists()
        && gold.join("golden_acknack_le.bin").exists()
    {
        cmd.env("GOLDEN_DIR", &gold);
    }
    let out = cmd.output().expect("go test");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "go test failed:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    for name in [
        "TestByteGoldenHeartbeat",
        "TestByteGoldenAckNack",
        "TestEndToEndSenderReceiverWithLossRecovery",
        "TestReliableAsyncWriterLossRecovery",
    ] {
        assert!(
            stdout.contains(&format!("--- PASS: {name}")),
            "{name} did not run/pass:\nstdout: {stdout}"
        );
    }
}

#[test]
fn go_reliable_example() {
    if !go_available() {
        eprintln!("SKIP go_reliable_example: `go` not on PATH");
        return;
    }
    let out = Command::new("go")
        .arg("run")
        .arg("./example_reliable")
        .current_dir(endpoints_go())
        .output()
        .expect("go run example_reliable");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("RELIABLE OK"),
        "go example_reliable did not report RELIABLE OK:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("sequence 0..11 verified in order"),
        "go example_reliable did not verify the recovered sequence:\nstdout: {stdout}"
    );
}

#[test]
fn go_reliable_latency_bench() {
    if !go_available() {
        eprintln!("SKIP go_reliable_latency_bench: `go` not on PATH");
        return;
    }
    let bin = go_build("reliable_bench", "bench");
    // No live peer needed (an arbitrary loopback port): only local dispatch
    // cost (channel enqueue vs. inline syscall) is under measurement.
    let out = Command::new(&bin).arg("9").output().expect("run bench");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout
        .lines()
        .find(|l| l.starts_with("BENCH "))
        .unwrap_or_else(|| {
            panic!(
                "no BENCH line:\nstdout: {stdout}\nstderr: {}",
                String::from_utf8_lossy(&out.stderr)
            )
        });
    let decoupled = parse_kv(line, "decoupled_ns=");
    let inline_send = parse_kv(line, "inline_ns=");
    eprintln!(
        "go async-writer producer latency: decoupled={decoupled} ns, inline(send)={inline_send} ns"
    );
}

fn parse_kv(line: &str, key: &str) -> u64 {
    line.split_whitespace()
        .find_map(|tok| tok.strip_prefix(key))
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| panic!("missing {key} in `{line}`"))
}
