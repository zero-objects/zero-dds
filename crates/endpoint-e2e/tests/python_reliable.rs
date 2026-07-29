// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Python reliable endpoint: the pure-Python reliable state machine + frame
//! codec (`endpoints/python/zerodds_reliable.py`) driven by a decoupled
//! `queue.Queue` + drain-`threading.Thread` sender app
//! (`example_reliable.py`) against the shared Rust reliable peer. Proves loss
//! recovery (peer drops -> app retransmits on ACKNACK -> all samples
//! delivered gap-free), the unit + byte-golden suite, and the producer-
//! latency decoupling. Gated on `python3`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use zerodds_endpoint_e2e::{bind_reliable_peer, reliable_receive};

fn python_available() -> bool {
    Command::new("python3").arg("--version").output().is_ok()
}

fn endpoints_python() -> PathBuf {
    Path::new(&format!(
        "{}/../../endpoints/python",
        env!("CARGO_MANIFEST_DIR")
    ))
    .canonicalize()
    .expect("endpoints/python path")
}

fn run_loss(drop_every: Option<usize>, label: &str) {
    if !python_available() {
        eprintln!("SKIP python_reliable {label}: `python3` not on PATH");
        return;
    }
    let n = 12usize;
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child = Command::new("python3")
        .arg(endpoints_python().join("example_reliable.py"))
        .arg(peer.port.to_string())
        .arg(n.to_string())
        .arg("run")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn python3 reliable app");
    let delivered = reliable_receive(&peer, child, label, n);
    assert_eq!(delivered.len(), n, "{label}: delivered count");
    for (i, s) in delivered.iter().enumerate() {
        assert_eq!(s.len(), 4, "{label}: sample {i} length");
        let v = u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
        assert_eq!(v as usize, i, "{label}: sample {i} value (gap-free order)");
    }
}

#[test]
fn python_reliable_loss_recovery() {
    run_loss(Some(3), "python/loss");
}

#[test]
fn python_reliable_no_loss() {
    run_loss(None, "python/noloss");
}

/// Builds the two goldens the way `endpoints/golden-gen` does (via
/// `zerodds-xrce`, so this tracks any wire change) and runs the Python unit +
/// byte-golden suite.
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
fn python_reliable_unit_and_golden() {
    if !python_available() {
        eprintln!("SKIP python_reliable unit/golden: `python3` not on PATH");
        return;
    }
    let dir = std::env::temp_dir().join(format!("rel_py_gold_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    write_goldens(&dir);
    let out = Command::new("python3")
        .arg(endpoints_python().join("reliable_test.py"))
        .arg(&dir)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .expect("run python reliable_test.py");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("ALL OK"),
        "python reliable unit/golden failed:\n{stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("ok   byte_golden_heartbeat")
            && stdout.contains("ok   byte_golden_acknack"),
        "byte-golden did not run:\n{stdout}"
    );
}

#[test]
fn python_reliable_producer_latency() {
    if !python_available() {
        eprintln!("SKIP python_reliable bench: `python3` not on PATH");
        return;
    }
    let peer = bind_reliable_peer(None).expect("bind reliable peer");
    let out = Command::new("python3")
        .arg(endpoints_python().join("example_reliable.py"))
        .arg(peer.port.to_string())
        .arg("0")
        .arg("bench")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .expect("run python reliable bench");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("BENCH"),
        "bench output missing:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("python reliable {}", stdout.trim());
}
