// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Elixir reliable-stream endpoint: the pure-Elixir reliable state machine
//! (`endpoints/elixir/lib/reliable.ex`, module `ZeroDDS.Reliable`) driven by
//! a decoupled `ZeroDDS.Reliable.Drain` GenServer sender app
//! (`reliable_app.exs`) against the shared Rust reliable peer. Proves loss
//! recovery (peer drops -> app retransmits on ACKNACK -> all samples
//! delivered gap-free), the unit + byte-golden suite, the runnable example,
//! and the producer-latency decoupling. Gated on `elixir` on PATH.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};

use zerodds_endpoint_e2e::{bind_reliable_peer, reliable_receive};

fn elixir_available() -> bool {
    Command::new("elixir").arg("--version").output().is_ok()
}

fn endpoints_elixir() -> PathBuf {
    PathBuf::from(format!(
        "{}/../../endpoints/elixir",
        env!("CARGO_MANIFEST_DIR")
    ))
}

/// Absolute path to the reliable-stream SDK module (`ZeroDDS.Reliable` +
/// `ZeroDDS.Reliable.Drain`).
fn reliable_lib() -> PathBuf {
    endpoints_elixir()
        .join("lib/reliable.ex")
        .canonicalize()
        .expect("endpoints/elixir/lib/reliable.ex")
}

fn elixir_cmd(script: &str, args: &[&str]) -> Command {
    let mut cmd = Command::new("elixir");
    cmd.current_dir(endpoints_elixir());
    cmd.arg("-r").arg(reliable_lib()).arg(script);
    for a in args {
        cmd.arg(a);
    }
    cmd
}

/// Spawns `elixir -r lib/reliable.ex <script> <args...>` (for the live E2E
/// cases, where the caller drives a peer against the running child).
fn spawn_elixir(script: &str, args: &[&str]) -> Child {
    elixir_cmd(script, args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn elixir")
}

/// Runs `elixir -r lib/reliable.ex <script> <args...>` to completion (unit,
/// golden, example -- no peer needed).
fn run_elixir(script: &str, args: &[&str]) -> Output {
    elixir_cmd(script, args).output().expect("run elixir")
}

fn run_loss(drop_every: Option<usize>, label: &str) {
    if !elixir_available() {
        eprintln!("SKIP {label}: `elixir` not on PATH");
        return;
    }
    let n = 12usize;
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child = spawn_elixir(
        "reliable_app.exs",
        &[&peer.port.to_string(), &n.to_string()],
    );
    let delivered = reliable_receive(&peer, child, label, n);
    assert_eq!(delivered.len(), n, "{label}: delivered count");
    for (i, s) in delivered.iter().enumerate() {
        assert_eq!(s.len(), 4, "{label}: sample {i} length");
        let v = u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
        assert_eq!(v as usize, i, "{label}: sample {i} value (gap-free order)");
    }
}

#[test]
fn elixir_reliable_loss_recovery() {
    run_loss(Some(3), "elixir/loss");
}

#[test]
fn elixir_reliable_no_loss() {
    run_loss(None, "elixir/noloss");
}

/// Builds the two goldens the way `endpoints/golden-gen` does (via
/// `zerodds-xrce`, so this tracks any wire change) and runs the Elixir unit +
/// byte-golden suite.
#[test]
fn elixir_reliable_unit_and_golden() {
    if !elixir_available() {
        eprintln!("SKIP elixir_reliable_unit_and_golden: `elixir` not on PATH");
        return;
    }
    let dir = std::env::temp_dir().join(format!("rel_elixir_gold_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");

    // Best-effort: generate the Rust goldens and pass the dir for a
    // byte-identical check; the unit suite runs regardless.
    let gen_res = Command::new("cargo")
        .args(["run", "-q", "-p", "zerodds-endpoint-golden", "--"])
        .arg(&dir)
        .output();
    let hb = dir.join("golden_heartbeat_le.bin");
    let ak = dir.join("golden_acknack_le.bin");
    let goldens_ready =
        gen_res.map(|o| o.status.success()).unwrap_or(false) && hb.exists() && ak.exists();

    let dir_arg = dir.display().to_string();
    let args: &[&str] = if goldens_ready { &[&dir_arg] } else { &[] };
    let out = run_elixir("reliable_test.exs", args);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("ALL OK"),
        "elixir reliable unit/golden failed:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    if goldens_ready {
        assert!(
            stdout.contains("ok   byte_golden_heartbeat")
                && stdout.contains("ok   byte_golden_acknack"),
            "byte-golden did not run:\n{stdout}"
        );
    }
}

#[test]
fn elixir_reliable_example() {
    if !elixir_available() {
        eprintln!("SKIP elixir_reliable_example: `elixir` not on PATH");
        return;
    }
    let out = run_elixir("example_reliable.exs", &[]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("RELIABLE OK"),
        "elixir example failed:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn elixir_reliable_producer_latency() {
    if !elixir_available() {
        eprintln!("SKIP elixir_reliable_producer_latency: `elixir` not on PATH");
        return;
    }
    let peer = bind_reliable_peer(None).expect("bind reliable peer");
    let child = spawn_elixir("reliable_app.exs", &[&peer.port.to_string(), "0", "bench"]);
    let out = child.wait_with_output().expect("wait bench");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("BENCH"),
        "bench output missing:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("elixir reliable {}", stdout.trim());
}
