// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Lua reliable-stream endpoint: the pure-Lua reliable state machine + frame
//! codec (`endpoints/lua/reliable.lua`) driven by a cooperative submit/drain
//! sender app (`endpoints/lua/reliable_app.lua`) against the shared Rust
//! reliable peer. Proves loss recovery (peer drops -> app retransmits on
//! ACKNACK -> all samples delivered gap-free), the unit + byte-golden suite,
//! the runnable example, and the honest single-OS-thread producer-latency
//! note. Gated on `lua5.4` + luasocket.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::{bind_reliable_peer, reliable_receive};

/// `lua5.4` on PATH.
fn lua_available() -> bool {
    Command::new("lua5.4").arg("-v").output().is_ok()
}

/// luasocket (the `socket` module) loadable by this `lua5.4` -- the app
/// needs real UDP; stock Lua has none. Loud skip if missing (no false green).
fn luasocket_available() -> bool {
    Command::new("lua5.4")
        .arg("-e")
        .arg("require('socket')")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn endpoints_lua() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../endpoints/lua")
        .canonicalize()
        .expect("endpoints/lua path")
}

/// Runs a script in place under `endpoints/lua` (so its own
/// `package.path = "./?.lua;" .. package.path` resolves `require("reliable")`).
fn run_lua(script: &str, args: &[String]) -> Child {
    let mut cmd = Command::new("lua5.4");
    cmd.arg(script);
    for a in args {
        cmd.arg(a);
    }
    cmd.current_dir(endpoints_lua())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn lua5.4 {script}: {e}"))
}

fn run_loss(drop_every: Option<usize>, label: &str) {
    if !lua_available() {
        eprintln!("SKIP {label}: `lua5.4` not on PATH");
        return;
    }
    if !luasocket_available() {
        eprintln!("SKIP {label}: luasocket (`socket` module) not installed");
        return;
    }
    let n = 12usize;
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child = run_lua(
        "reliable_app.lua",
        &[peer.port.to_string(), n.to_string(), "run".to_string()],
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
fn lua_reliable_loss_recovery() {
    // Peer drops every 3rd distinct sample once -> the app must retransmit.
    run_loss(Some(3), "lua/loss");
}

#[test]
fn lua_reliable_no_loss() {
    run_loss(None, "lua/noloss");
}

/// Builds the two control-frame goldens the way `endpoints/golden-gen` does
/// (via `zerodds-xrce`, so this tracks any wire change) and runs the Lua
/// unit + byte-golden suite.
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
fn lua_reliable_unit_and_golden() {
    if !lua_available() {
        eprintln!("SKIP lua_reliable_unit_and_golden: `lua5.4` not on PATH");
        return;
    }
    let dir = std::env::temp_dir().join(format!("rel_lua_gold_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    write_goldens(&dir);
    let dir_str = dir.to_string_lossy().to_string();
    let out = Command::new("lua5.4")
        .arg("reliable_test.lua")
        .arg(&dir_str)
        .current_dir(endpoints_lua())
        .output()
        .expect("run reliable_test.lua");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("ALL OK"),
        "lua reliable unit/golden failed:\n{stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("ok   byte_golden_heartbeat")
            && stdout.contains("ok   byte_golden_acknack"),
        "byte-golden did not run:\n{stdout}"
    );
}

#[test]
fn lua_reliable_example() {
    if !lua_available() {
        eprintln!("SKIP lua_reliable_example: `lua5.4` not on PATH");
        return;
    }
    let out = Command::new("lua5.4")
        .arg("example_reliable.lua")
        .current_dir(endpoints_lua())
        .output()
        .expect("run example_reliable.lua");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("RELIABLE OK"),
        "lua example failed:\n{stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn lua_reliable_producer_latency() {
    if !lua_available() {
        eprintln!("SKIP lua_reliable_producer_latency: `lua5.4` not on PATH");
        return;
    }
    if !luasocket_available() {
        eprintln!("SKIP lua_reliable_producer_latency: luasocket not installed");
        return;
    }
    let peer = bind_reliable_peer(None).expect("bind reliable peer");
    let child = run_lua(
        "reliable_app.lua",
        &[peer.port.to_string(), "0".to_string(), "bench".to_string()],
    );
    let out = child.wait_with_output().expect("bench wait");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("BENCH"),
        "bench output missing:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // No strict `enqueue < inline` assertion -- honest note: stock lua5.4
    // has no OS threads, so this is a cooperative, single-thread figure, not
    // a proof of concurrent decoupling.
    eprintln!(
        "lua reliable (cooperative, single OS thread): {}",
        stdout.trim()
    );
}
