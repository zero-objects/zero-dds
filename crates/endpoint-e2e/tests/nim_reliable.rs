// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Nim reliable endpoint: the pure-Nim reliable state machine + frame codec
//! (`endpoints/nim/reliable.nim`) driven by a decoupled ring+drain-thread sender
//! app (`reliable_app.nim`) against the shared Rust reliable peer. Proves loss
//! recovery (peer drops → app retransmits on ACKNACK → all samples delivered
//! gap-free), the unit + byte-golden suite, and the producer-latency decoupling.
//! Gated on `nim`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use zerodds_endpoint_e2e::{bind_reliable_peer, reliable_receive};

fn nim_available() -> bool {
    Command::new("nim").arg("--version").output().is_ok()
}

fn endpoints_nim() -> PathBuf {
    PathBuf::from(format!(
        "{}/../../endpoints/nim",
        env!("CARGO_MANIFEST_DIR")
    ))
}

/// Compiles a Nim source under `endpoints/nim/` (its `import ./reliable` resolves
/// in place) to a temp binary. `tag` keeps parallel tests from sharing a dir.
fn compile_nim(src: &str, tag: &str, threads: bool) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rel_nim_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let bin = dir.join("app");
    let mut cmd = Command::new("nim");
    cmd.args(["c", "--hints:off", "--warnings:off", "-d:release"]);
    if threads {
        cmd.arg("--threads:on");
    }
    let out = cmd
        .arg(format!("--nimcache:{}", dir.join("nimc").display()))
        .arg(format!("-o:{}", bin.display()))
        .arg(endpoints_nim().join(src))
        .output()
        .expect("nim c");
    assert!(
        out.status.success(),
        "nim compile {src} failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    bin
}

fn run_loss(drop_every: Option<usize>, tag: &str, label: &str) {
    if !nim_available() {
        eprintln!("SKIP nim_reliable {label}: `nim` not on PATH");
        return;
    }
    let n = 12usize;
    let bin = compile_nim("reliable_app.nim", tag, true);
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child = Command::new(&bin)
        .arg(peer.port.to_string())
        .arg(n.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn nim reliable app");
    let delivered = reliable_receive(&peer, child, label, n);
    assert_eq!(delivered.len(), n, "{label}: delivered count");
    for (i, s) in delivered.iter().enumerate() {
        assert_eq!(s.len(), 4, "{label}: sample {i} length");
        let v = u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
        assert_eq!(v as usize, i, "{label}: sample {i} value (gap-free order)");
    }
}

#[test]
fn nim_reliable_loss_recovery() {
    run_loss(Some(3), "loss", "nim/loss");
}

#[test]
fn nim_reliable_no_loss() {
    run_loss(None, "noloss", "nim/noloss");
}

/// Builds the two goldens the way `endpoints/golden-gen` does (via `zerodds-xrce`,
/// so this tracks any wire change) and runs the Nim unit + byte-golden suite.
fn write_goldens(dir: &Path) {
    use zerodds_xrce::SerialNumber16;
    use zerodds_xrce::header::{MessageHeader, SessionId, StreamId};
    use zerodds_xrce::submessages::{AckNackPayload, HeartbeatPayload, Message};

    let hb_hdr =
        MessageHeader::without_client_key(SessionId(0x80), StreamId::NONE, SerialNumber16(1))
            .expect("hb header");
    let hb = HeartbeatPayload {
        first_unacked_seq_nr: 1,
        last_unacked_seq_nr: 3,
        stream_id: 0x80,
    }
    .into_submessage()
    .expect("hb submessage");
    let hb_bytes = Message::new(hb_hdr, vec![hb])
        .expect("hb msg")
        .encode()
        .expect("hb encode");
    std::fs::write(dir.join("golden_heartbeat_le.bin"), hb_bytes).expect("write hb golden");

    let an_hdr =
        MessageHeader::without_client_key(SessionId(0x80), StreamId::NONE, SerialNumber16(1))
            .expect("an header");
    let an = AckNackPayload {
        first_unacked_seq_num: 1,
        nack_bitmap: [0x00, 0x00],
        stream_id: 0x80,
    }
    .into_submessage()
    .expect("an submessage");
    let an_bytes = Message::new(an_hdr, vec![an])
        .expect("an msg")
        .encode()
        .expect("an encode");
    std::fs::write(dir.join("golden_acknack_le.bin"), an_bytes).expect("write an golden");
}

#[test]
fn nim_reliable_unit_and_golden() {
    if !nim_available() {
        eprintln!("SKIP nim_reliable unit/golden: `nim` not on PATH");
        return;
    }
    let dir = std::env::temp_dir().join(format!("rel_nim_gold_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    write_goldens(&dir);
    let bin = compile_nim("reliable_test.nim", "test", false);
    let out = Command::new(&bin).arg(&dir).output().expect("run nim test");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("ALL OK"),
        "nim reliable unit/golden failed:\n{stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("ok   byte_golden_heartbeat")
            && stdout.contains("ok   byte_golden_acknack"),
        "byte-golden did not run:\n{stdout}"
    );
}

#[test]
fn nim_reliable_producer_latency() {
    if !nim_available() {
        eprintln!("SKIP nim_reliable bench: `nim` not on PATH");
        return;
    }
    let bin = compile_nim("reliable_app.nim", "bench", true);
    let peer = bind_reliable_peer(None).expect("bind reliable peer");
    let out = Command::new(&bin)
        .arg(peer.port.to_string())
        .arg("0")
        .arg("bench")
        .output()
        .expect("run nim bench");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("BENCH"),
        "bench output missing:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("nim reliable {}", stdout.trim());
}
