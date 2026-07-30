// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OCaml reliable-stream endpoint: the OCaml app is the reliable sender --
//! an async-decoupled `Writer` (Mutex/Condition `Mailbox` + a drain `Thread`
//! that owns the `Sender` state and a UDP socket: submit -> history ->
//! WRITE_DATA, HEARTBEAT tick, on-ACKNACK retransmit). The shared Rust
//! reliable peer injects loss and drives recovery. Also runs OCaml's
//! unit/byte-golden suite, the in-process example, and the producer-latency
//! bench. Gated on `ocamlfind`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use zerodds_endpoint_e2e::{bind_reliable_peer, reliable_receive};

fn ocamlfind_available() -> bool {
    Command::new("ocamlfind").arg("printconf").output().is_ok()
}

fn endpoints_ocaml() -> String {
    format!("{}/../../endpoints/ocaml", env!("CARGO_MANIFEST_DIR"))
}

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pp_ocaml_rel_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("mkdir");
    d
}

/// Copies the given `endpoints/ocaml` sources into `dir` (deps-first order)
/// and compiles them into one binary with `ocamlfind ocamlopt`.
fn ocaml_build(dir: &Path, bin_name: &str, srcs: &[&str]) -> PathBuf {
    let src_dir = endpoints_ocaml();
    for s in srcs {
        let content = std::fs::read_to_string(format!("{src_dir}/{s}"))
            .unwrap_or_else(|e| panic!("read endpoints/ocaml/{s}: {e}"));
        std::fs::write(dir.join(s), content).unwrap_or_else(|e| panic!("write {s}: {e}"));
    }
    let mut c = Command::new("ocamlfind");
    c.args([
        "ocamlopt",
        "-thread",
        "-package",
        "unix,threads.posix",
        "-linkpkg",
    ]);
    for s in srcs {
        c.arg(s);
    }
    c.arg("-o").arg(bin_name);
    let o = c.current_dir(dir).output().expect("run ocamlfind ocamlopt");
    assert!(
        o.status.success(),
        "ocamlfind ocamlopt failed for {srcs:?}:\n{}",
        String::from_utf8_lossy(&o.stderr)
    );
    dir.join(bin_name)
}

fn run_loss(drop_every: Option<usize>, label: &str) {
    if !ocamlfind_available() {
        eprintln!("SKIP {label}: ocamlfind not on PATH");
        return;
    }
    let dir = tmp_dir(label.replace('/', "_").as_str());
    let bin = ocaml_build(&dir, "reliable_app", &["reliable.ml", "reliable_app.ml"]);

    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let n = 12usize;
    let child = Command::new(&bin)
        .arg(peer.port.to_string())
        .arg(n.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ocaml app");

    let samples = reliable_receive(&peer, child, label, n);
    assert_eq!(samples.len(), n, "{label}: sample count");
    for (i, s) in samples.iter().enumerate() {
        assert!(s.len() >= 4, "{label}: sample {i} too short");
        let v = u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
        assert_eq!(v as usize, i, "{label}: sample {i} value");
    }
}

#[test]
fn ocaml_reliable_loss_recovery() {
    run_loss(Some(3), "ocaml/loss");
}

#[test]
fn ocaml_reliable_no_loss() {
    run_loss(None, "ocaml/baseline");
}

#[test]
fn ocaml_reliable_unit_and_golden() {
    if !ocamlfind_available() {
        eprintln!("SKIP ocaml_reliable_unit_and_golden: ocamlfind not on PATH");
        return;
    }
    let dir = tmp_dir("unit");
    let bin = ocaml_build(&dir, "reliable_test", &["reliable.ml", "reliable_test.ml"]);

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
        "ocaml unit/golden failed:\nstdout: {out}\nstderr: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn ocaml_reliable_example() {
    if !ocamlfind_available() {
        eprintln!("SKIP ocaml_reliable_example: ocamlfind not on PATH");
        return;
    }
    let dir = tmp_dir("example");
    let bin = ocaml_build(
        &dir,
        "example_reliable",
        &["reliable.ml", "example_reliable.ml"],
    );
    let o = Command::new(&bin).output().expect("run example");
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        out.contains("RELIABLE OK"),
        "ocaml example failed:\nstdout: {out}\nstderr: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn ocaml_reliable_latency_bench() {
    if !ocamlfind_available() {
        eprintln!("SKIP ocaml_reliable_latency_bench: ocamlfind not on PATH");
        return;
    }
    let dir = tmp_dir("bench");
    let bin = ocaml_build(
        &dir,
        "reliable_bench",
        &["reliable.ml", "reliable_bench.ml"],
    );
    let o = Command::new(&bin).output().expect("run bench");
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        out.contains("producer latency"),
        "ocaml bench produced no figure:\nstdout: {out}\nstderr: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    eprintln!("ocaml bench: {}", out.trim());
}
