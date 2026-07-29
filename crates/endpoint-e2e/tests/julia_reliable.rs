// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Julia reliable endpoint: the pure-Julia reliable state machine + frame codec
//! (`endpoints/julia/reliable.jl`) driven by a decoupled Channel+drain-Task
//! sender app (`reliable_app.jl`) against the shared Rust reliable peer. Proves
//! loss recovery (peer drops → app retransmits on ACKNACK → all samples
//! delivered gap-free), the unit + byte-golden suite, and the producer-latency
//! decoupling. Gated on `julia`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use zerodds_endpoint_e2e::{bind_reliable_peer, reliable_receive};

fn julia_available() -> bool {
    Command::new("julia").arg("--version").output().is_ok()
}

fn endpoints_julia() -> PathBuf {
    PathBuf::from(format!(
        "{}/../../endpoints/julia",
        env!("CARGO_MANIFEST_DIR")
    ))
}

fn run_loss(drop_every: Option<usize>, label: &str) {
    if !julia_available() {
        eprintln!("SKIP julia_reliable {label}: `julia` not on PATH");
        return;
    }
    let n = 12usize;
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child = Command::new("julia")
        .arg(endpoints_julia().join("reliable_app.jl"))
        .arg(peer.port.to_string())
        .arg(n.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn julia reliable app");
    let delivered = reliable_receive(&peer, child, label, n);
    assert_eq!(delivered.len(), n, "{label}: delivered count");
    for (i, s) in delivered.iter().enumerate() {
        assert_eq!(s.len(), 4, "{label}: sample {i} length");
        let v = u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
        assert_eq!(v as usize, i, "{label}: sample {i} value (gap-free order)");
    }
}

#[test]
fn julia_reliable_loss_recovery() {
    run_loss(Some(3), "julia/loss");
}

#[test]
fn julia_reliable_no_loss() {
    run_loss(None, "julia/noloss");
}

/// Builds the two goldens the way `endpoints/golden-gen` does (via `zerodds-xrce`,
/// so this tracks any wire change) and runs the Julia unit + byte-golden suite.
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
fn julia_reliable_unit_and_golden() {
    if !julia_available() {
        eprintln!("SKIP julia_reliable unit/golden: `julia` not on PATH");
        return;
    }
    let dir = std::env::temp_dir().join(format!("rel_julia_gold_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    write_goldens(&dir);
    let out = Command::new("julia")
        .arg(endpoints_julia().join("reliable_test.jl"))
        .arg(&dir)
        .output()
        .expect("run julia test");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("ALL OK"),
        "julia reliable unit/golden failed:\n{stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("ok   byte_golden_heartbeat")
            && stdout.contains("ok   byte_golden_acknack"),
        "byte-golden did not run:\n{stdout}"
    );
}

#[test]
fn julia_reliable_producer_latency() {
    if !julia_available() {
        eprintln!("SKIP julia_reliable bench: `julia` not on PATH");
        return;
    }
    let peer = bind_reliable_peer(None).expect("bind reliable peer");
    let out = Command::new("julia")
        .arg(endpoints_julia().join("reliable_app.jl"))
        .arg(peer.port.to_string())
        .arg("0")
        .arg("bench")
        .output()
        .expect("run julia bench");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("BENCH"),
        "bench output missing:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("julia reliable {}", stdout.trim());
}
