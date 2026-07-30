// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! D reliable-stream endpoint: the D app is the reliable sender (submit +
//! WRITE_DATA + on-ACKNACK retransmit); the shared Rust reliable peer injects
//! loss and drives recovery. Also runs D's unit/byte-golden suite, the
//! in-process example, and the producer-latency bench. Gated on `gdc`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use zerodds_endpoint_e2e::{bind_reliable_peer, reliable_receive};

fn gdc_available() -> bool {
    Command::new("gdc").arg("--version").output().is_ok()
}

fn endpoints_d() -> String {
    format!("{}/../../endpoints/d", env!("CARGO_MANIFEST_DIR"))
}

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pp_d_rel_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("mkdir");
    d
}

/// Compile the given `endpoints/d` sources into one binary with gdc.
fn gdc_build(bin: &Path, srcs: &[&str]) {
    let d = endpoints_d();
    let mut c = Command::new("gdc");
    for s in srcs {
        c.arg(format!("{d}/{s}"));
    }
    c.arg("-o").arg(bin);
    let o = c.output().expect("run gdc");
    assert!(
        o.status.success(),
        "gdc failed for {srcs:?}:\n{}",
        String::from_utf8_lossy(&o.stderr)
    );
}

fn run_loss(drop_every: Option<usize>, label: &str) {
    if !gdc_available() {
        eprintln!("SKIP {label}: gdc not on PATH");
        return;
    }
    let dir = tmp_dir(label.replace('/', "_").as_str());
    let app = dir.join("reliable_app");
    gdc_build(&app, &["reliable.d", "reliable_app.d"]);

    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let n = 12usize;
    let child = Command::new(&app)
        .arg(peer.port.to_string())
        .arg(n.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn d app");

    let samples = reliable_receive(&peer, child, label, n);
    assert_eq!(samples.len(), n, "{label}: sample count");
    for (i, s) in samples.iter().enumerate() {
        assert!(s.len() >= 4, "{label}: sample {i} too short");
        let v = u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
        assert_eq!(v as usize, i, "{label}: sample {i} value");
    }
}

#[test]
fn d_reliable_loss_recovery() {
    run_loss(Some(3), "d/loss");
}

#[test]
fn d_reliable_no_loss() {
    run_loss(None, "d/baseline");
}

#[test]
fn d_reliable_unit_and_golden() {
    if !gdc_available() {
        eprintln!("SKIP d_reliable_unit_and_golden: gdc not on PATH");
        return;
    }
    let dir = tmp_dir("unit");
    let bin = dir.join("reliable_test");
    gdc_build(&bin, &["reliable.d", "reliable_test.d"]);

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
    let mut cmd = Command::new(&bin);
    if gen_res.map(|o| o.status.success()).unwrap_or(false) && hb.exists() && ak.exists() {
        cmd.arg(&hb).arg(&ak);
    }
    let o = cmd.output().expect("run reliable_test");
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        o.status.success() && out.contains("ALL OK"),
        "d unit/golden failed:\nstdout: {out}\nstderr: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn d_reliable_example() {
    if !gdc_available() {
        eprintln!("SKIP d_reliable_example: gdc not on PATH");
        return;
    }
    let dir = tmp_dir("example");
    let bin = dir.join("example_reliable");
    gdc_build(&bin, &["reliable.d", "example_reliable.d"]);
    let o = Command::new(&bin).output().expect("run example");
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        out.contains("RELIABLE OK"),
        "d example failed:\nstdout: {out}\nstderr: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn d_reliable_latency_bench() {
    if !gdc_available() {
        eprintln!("SKIP d_reliable_latency_bench: gdc not on PATH");
        return;
    }
    let dir = tmp_dir("bench");
    let bin = dir.join("reliable_bench");
    gdc_build(&bin, &["reliable.d", "reliable_bench.d"]);
    let o = Command::new(&bin).output().expect("run bench");
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        out.contains("producer latency"),
        "d bench produced no figure:\nstdout: {out}\nstderr: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    eprintln!("d bench: {}", out.trim());
}
