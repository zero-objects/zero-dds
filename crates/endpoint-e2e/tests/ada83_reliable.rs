// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! pre-Object **Ada 83** reliable endpoint: the strict-`-gnat83` reliability
//! state machine (unit + byte-golden) and a live loss-recovery run — the Ada-83
//! app is the reliable SENDER (procedural poll loop, no tasking/protected
//! objects), the shared Rust [`ReliablePeer`](zerodds_endpoint_e2e) drops
//! datagrams and replies `ACKNACK` so the app retransmits until its window
//! drains. Gated on `gprbuild`.
//!
//! Mirrors `ada_reliable.rs` but for the oldest-legacy variant. The reliability
//! CORE (`zerodds_ada83_reliable`) + its unit test are compiled with `-gnat83`;
//! the UDP driver app (`example_ada83_reliable`, which needs GNAT.Sockets) is
//! compiled with a relaxed standard — the same split endpoints/c uses for
//! `reliable_udp_app.c`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;

use zerodds_endpoint_e2e::{bind_reliable_peer, reliable_receive};

fn ada_available() -> bool {
    Command::new("gprbuild").arg("--version").output().is_ok()
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

/// Builds the Ada-83 mains once with gprbuild (strict `-gnat83` core + unit
/// test, relaxed-standard UDP sender app); returns the exec dir. `None` on build
/// failure.
fn built_dir() -> Option<&'static PathBuf> {
    static BUILD: OnceLock<Option<PathBuf>> = OnceLock::new();
    BUILD
        .get_or_init(|| {
            let root = root();
            let dir = std::env::temp_dir().join(format!("ada83_reliable_{}", std::process::id()));
            std::fs::create_dir_all(dir.join("obj")).expect("mkdir");
            // Strict `-gnat83` by default; the socket driver app gets a relaxed
            // standard (GNAT.Sockets / Command_Line / Real_Time are not Ada 83),
            // exactly the split the endpoints/ada-83 project file itself uses.
            let gpr = format!(
                "project Rel83 is\n\
                 \x20  for Languages use (\"Ada\");\n\
                 \x20  for Source_Dirs use (\"{src}\", \"{test}\");\n\
                 \x20  for Object_Dir use \"obj\";\n\
                 \x20  for Exec_Dir use \".\";\n\
                 \x20  for Main use (\"example_ada83_reliable.adb\", \"test_ada83_reliable.adb\");\n\
                 \x20  package Compiler is\n\
                 \x20     for Default_Switches (\"Ada\") use (\"-gnat83\");\n\
                 \x20     for Switches (\"example_ada83_reliable.adb\") use (\"-gnat2012\");\n\
                 \x20  end Compiler;\n\
                 end Rel83;\n",
                src = root.join("endpoints/ada-83/src").display(),
                test = root.join("endpoints/ada-83/test").display(),
            );
            std::fs::write(dir.join("rel83.gpr"), gpr).expect("write gpr");
            let ok = Command::new("gprbuild")
                .arg("-P")
                .arg(dir.join("rel83.gpr"))
                .arg("-p")
                .arg("-q")
                .current_dir(&dir)
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok { Some(dir) } else { None }
        })
        .as_ref()
}

fn spawn_sender(exec_dir: &Path, port: u16) -> Child {
    Command::new(exec_dir.join("example_ada83_reliable"))
        .arg(port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ada-83 sender")
}

#[test]
fn ada83_reliable_unit() {
    if !ada_available() {
        eprintln!("SKIP ada83_reliable_unit: gprbuild not on PATH");
        return;
    }
    let dir = built_dir().expect("ada-83 build");
    // Strict Ada 83 has no Command_Line, so the unit test carries the reference
    // HEARTBEAT/ACKNACK goldens inline and needs no arguments.
    let out = Command::new(dir.join("test_ada83_reliable"))
        .output()
        .expect("run unit");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ALL OK"),
        "ada-83 reliable unit tests failed\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn ada83_reliable_loss_recovery() {
    if !ada_available() {
        eprintln!("SKIP ada83_reliable_loss_recovery: gprbuild not on PATH");
        return;
    }
    let dir = built_dir().expect("ada-83 build");
    let peer = bind_reliable_peer(Some(3)).expect("bind reliable peer");
    let child = spawn_sender(dir, peer.port);
    let delivered = reliable_receive(&peer, child, "ada-83/loss", 12);
    assert_eq!(
        delivered.len(),
        12,
        "ada-83: all 12 samples gap-free despite drops"
    );
}

#[test]
fn ada83_reliable_baseline() {
    if !ada_available() {
        eprintln!("SKIP ada83_reliable_baseline: gprbuild not on PATH");
        return;
    }
    let dir = built_dir().expect("ada-83 build");
    let peer = bind_reliable_peer(None).expect("bind reliable peer");
    let child = spawn_sender(dir, peer.port);
    let delivered = reliable_receive(&peer, child, "ada-83/baseline", 12);
    assert_eq!(
        delivered.len(),
        12,
        "ada-83: all 12 samples delivered (lossless)"
    );
}
