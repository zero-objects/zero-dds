// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Pure-Ada (ADR 0013 Stage 2) reliable endpoint: the state machine (unit +
//! byte-golden) and a live loss-recovery run. The Ada-native app is the reliable
//! SENDER (async-decoupled writer: the producer enqueues, a drain task does all
//! I/O), the shared Rust `ReliablePeer` drops datagrams and replies ACKNACK so
//! the app retransmits. Byte-identical wire to `endpoints/c` + `crates/xrce`.
//! Gated on gprbuild — no C, no FFI (unlike `ada_reliable.rs`, this builds only
//! the pure-Ada `endpoints/ada-native` sources).

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;

use zerodds_endpoint_e2e::{bind_reliable_peer, reliable_receive};

fn ada_available() -> bool {
    Command::new("gprbuild").arg("--version").output().is_ok()
}

/// The reference control-frame goldens (byte-for-byte the `zerodds-endpoint-golden`
/// output, verified against crates/xrce + endpoints/c). The Ada unit test asserts
/// its own `Heartbeat_Frame(1,3)` / `Acknack_Frame(1,0)` equal these.
const GOLDEN_HEARTBEAT: &[u8] = &[
    0x80, 0x00, 0x01, 0x00, 0x0b, 0x01, 0x05, 0x00, 0x01, 0x00, 0x03, 0x00, 0x80,
];
const GOLDEN_ACKNACK: &[u8] = &[
    0x80, 0x00, 0x01, 0x00, 0x0a, 0x01, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00, 0x80,
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

/// Builds the Ada-native mains once (pure Ada, no C) with gprbuild; returns the
/// exec dir. `None` if the build fails.
fn built_dir() -> Option<&'static PathBuf> {
    static BUILD: OnceLock<Option<PathBuf>> = OnceLock::new();
    BUILD
        .get_or_init(|| {
            let root = root();
            let dir =
                std::env::temp_dir().join(format!("ada_native_reliable_{}", std::process::id()));
            std::fs::create_dir_all(dir.join("obj")).expect("mkdir");
            let gpr = format!(
                "project Rel is\n\
                 \x20  for Languages use (\"Ada\");\n\
                 \x20  for Source_Dirs use (\"{ada_src}\", \"{ada_test}\");\n\
                 \x20  for Object_Dir use \"obj\";\n\
                 \x20  for Exec_Dir use \".\";\n\
                 \x20  for Main use (\"example_native_reliable.adb\", \"test_native_reliable_unit.adb\");\n\
                 \x20  package Compiler is\n\
                 \x20     for Default_Switches (\"Ada\") use (\"-gnat2022\");\n\
                 \x20  end Compiler;\n\
                 \x20  package Binder is\n\
                 \x20     for Default_Switches (\"Ada\") use (\"-Es\");\n\
                 \x20  end Binder;\n\
                 end Rel;\n",
                ada_src = root.join("endpoints/ada-native/src").display(),
                ada_test = root.join("endpoints/ada-native/test").display(),
            );
            std::fs::write(dir.join("rel.gpr"), gpr).expect("write gpr");
            let ok = Command::new("gprbuild")
                .arg("-P")
                .arg(dir.join("rel.gpr"))
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
    Command::new(exec_dir.join("example_native_reliable"))
        .arg(port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ada-native sender")
}

#[test]
fn ada_native_reliable_unit() {
    if !ada_available() {
        eprintln!("SKIP ada_native_reliable_unit: gprbuild not on PATH");
        return;
    }
    let dir = built_dir().expect("ada-native build");
    // Write the reference goldens next to the run so the Ada test can read them.
    let gold = std::env::temp_dir().join(format!("ada_native_gold_{}", std::process::id()));
    std::fs::create_dir_all(&gold).expect("mkdir gold");
    std::fs::write(gold.join("golden_heartbeat_le.bin"), GOLDEN_HEARTBEAT).expect("hb");
    std::fs::write(gold.join("golden_acknack_le.bin"), GOLDEN_ACKNACK).expect("an");
    let out = Command::new(dir.join("test_native_reliable_unit"))
        .arg(&gold)
        .output()
        .expect("run unit");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ALL OK"),
        "ada-native unit tests failed\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn ada_native_reliable_loss_recovery() {
    if !ada_available() {
        eprintln!("SKIP ada_native_reliable_loss_recovery: gprbuild not on PATH");
        return;
    }
    let dir = built_dir().expect("ada-native build");
    let peer = bind_reliable_peer(Some(3)).expect("bind reliable peer");
    let child = spawn_sender(dir, peer.port);
    let delivered = reliable_receive(&peer, child, "ada-native/loss", 12);
    assert_eq!(
        delivered.len(),
        12,
        "ada-native: all 12 samples gap-free despite drops"
    );
}

#[test]
fn ada_native_reliable_baseline() {
    if !ada_available() {
        eprintln!("SKIP ada_native_reliable_baseline: gprbuild not on PATH");
        return;
    }
    let dir = built_dir().expect("ada-native build");
    let peer = bind_reliable_peer(None).expect("bind reliable peer");
    let child = spawn_sender(dir, peer.port);
    let delivered = reliable_receive(&peer, child, "ada-native/baseline", 12);
    assert_eq!(
        delivered.len(),
        12,
        "ada-native: all 12 samples delivered (lossless)"
    );
}
