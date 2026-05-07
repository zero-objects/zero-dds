// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// E2E-Tests fuer `zerodds-amqp-bridged`. Wir spawnen einen
// Mock-AMQP-Broker (TCP-Listener), starten den Daemon im --once-Mode,
// und verifizieren dass der Bridge-Pump den Wire-Spec ausgibt:
// AMQP-Protocol-Header → OPEN → BEGIN → ATTACH → TRANSFER/FLOW.
//
// Spec: `docs/specs/zerodds-amqp-bridge-daemon-1.0.md` §12.2.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use zerodds_amqp_bridge::frame::{decode_frame_header, encode_frame_header};
use zerodds_amqp_bridge::performatives::{decode_performative, descriptor};

const AMQP_PROTOCOL_HEADER: [u8; 8] = [b'A', b'M', b'Q', b'P', 0x00, 0x01, 0x00, 0x00];

fn daemon_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_zerodds-amqp-bridged"))
}

struct DaemonGuard {
    child: Child,
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn spawn_daemon_with_topic(broker_addr: &str, topic_arg: &str) -> DaemonGuard {
    let child = Command::new(daemon_binary())
        .args([
            "--broker",
            &format!("amqp://{broker_addr}"),
            "--container-id",
            "zerodds-test-bridge",
            "--topic",
            topic_arg,
            "--once",
            "--connect-timeout-ms",
            "3000",
        ])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn daemon");
    DaemonGuard { child }
}

/// Mock-AMQP-Broker: akzeptiert eine Verbindung und sammelt alle
/// empfangenen Wire-Bytes bis EOF; spielt zurueck:
/// 1) Protocol-Header
/// 2) OPEN (echo descriptor)
/// 3) BEGIN
/// 4) ATTACH per geschickter ATTACH
fn run_mock_broker(listener: TcpListener) -> Vec<u8> {
    let (mut stream, _peer) = listener.accept().expect("accept");
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    // 1) Read client protocol header.
    let mut hdr = [0u8; 8];
    if stream.read_exact(&mut hdr).is_err() {
        return Vec::new();
    }
    let mut all = Vec::new();
    all.extend_from_slice(&hdr);

    // 2) Echo back protocol header.
    stream.write_all(&AMQP_PROTOCOL_HEADER).expect("write hdr");

    // 3) Read frames until EOF or error. After OPEN/BEGIN, send back
    //    minimally compliant OPEN/BEGIN/ATTACH responses so the daemon
    //    can proceed.
    let mut peer_open_sent = false;
    let mut peer_begin_sent = false;
    loop {
        let mut h = [0u8; 8];
        match stream.read_exact(&mut h) {
            Ok(_) => {}
            Err(_) => break,
        }
        let parsed = match decode_frame_header(&h) {
            Ok(p) => p,
            Err(_) => break,
        };
        all.extend_from_slice(&h);
        let body_len = (parsed.size as usize).saturating_sub(8);
        let mut body = vec![0u8; body_len];
        if body_len > 0 && stream.read_exact(&mut body).is_err() {
            break;
        }
        all.extend_from_slice(&body);

        // Best-effort echo response after OPEN/BEGIN/ATTACH.
        if !peer_open_sent {
            // Mock OPEN reply.
            send_dummy_performative(&mut stream, descriptor::OPEN);
            peer_open_sent = true;
        } else if !peer_begin_sent {
            send_dummy_performative(&mut stream, descriptor::BEGIN);
            peer_begin_sent = true;
        } else {
            send_dummy_performative(&mut stream, descriptor::ATTACH);
        }
    }

    all
}

fn send_dummy_performative(stream: &mut TcpStream, desc: u64) {
    use zerodds_amqp_bridge::extended_types::AmqpExtValue;
    use zerodds_amqp_bridge::performatives::encode_performative;
    let body = AmqpExtValue::List(Vec::new());
    let Ok(perf) = encode_performative(desc, &body) else {
        return;
    };
    let total = 8u32 + perf.len() as u32;
    let h = zerodds_amqp_bridge::frame::FrameHeader::new_amqp(total, 2, 0);
    let hdr = encode_frame_header(h);
    let _ = stream.write_all(&hdr);
    let _ = stream.write_all(&perf);
}

fn pick_port_listener() -> (TcpListener, String) {
    let l = TcpListener::bind("127.0.0.1:0").expect("ephemeral bind");
    let p = l.local_addr().expect("addr").port();
    (l, format!("127.0.0.1:{p}"))
}

#[test]
fn daemon_opens_and_attaches_link() {
    let (listener, addr) = pick_port_listener();

    // Spawn mock-broker thread before daemon starts.
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let captured = run_mock_broker(listener);
        let _ = tx.send(captured);
    });

    let _guard = spawn_daemon_with_topic(&addr, "Chat::Message=topic://chat/msg");

    // Wait for broker thread.
    let captured = rx
        .recv_timeout(Duration::from_secs(8))
        .expect("broker recv");
    assert!(
        captured.len() >= 8,
        "expected at least protocol header, got {} bytes",
        captured.len()
    );
    assert_eq!(
        &captured[..8],
        b"AMQP\x00\x01\x00\x00",
        "protocol header mismatch"
    );

    // Walk frames after the 8-byte protocol header and confirm we see
    // at least one of: OPEN/BEGIN/ATTACH.
    let mut idx = 8;
    let mut seen_descriptors = Vec::new();
    while idx + 8 <= captured.len() {
        let h = match decode_frame_header(&captured[idx..idx + 8]) {
            Ok(h) => h,
            Err(_) => break,
        };
        let body_start = idx + h.body_offset();
        let frame_end = idx + h.size as usize;
        if frame_end > captured.len() || body_start > frame_end {
            break;
        }
        let body = &captured[body_start..frame_end];
        if let Ok((desc, _, _)) = decode_performative(body) {
            seen_descriptors.push(desc);
        }
        idx = frame_end;
    }

    assert!(
        seen_descriptors.contains(&descriptor::OPEN),
        "expected OPEN performative, got {seen_descriptors:?}"
    );
    assert!(
        seen_descriptors.contains(&descriptor::BEGIN),
        "expected BEGIN performative, got {seen_descriptors:?}"
    );
    assert!(
        seen_descriptors.contains(&descriptor::ATTACH),
        "expected ATTACH performative, got {seen_descriptors:?}"
    );
}

#[test]
fn daemon_with_no_broker_fails_with_exit_2() {
    // Connect to a port that's almost certainly closed.
    let (listener, addr) = pick_port_listener();
    drop(listener); // free the port.
    let mut child = Command::new(daemon_binary())
        .args([
            "--broker",
            &format!("amqp://{addr}"),
            "--once",
            "--connect-timeout-ms",
            "300",
        ])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn daemon");
    let status = child.wait().expect("wait");
    // exit 2 = broker connect failed (Spec §2 exit code).
    assert_eq!(status.code(), Some(2), "expected exit 2 for connect-fail");
}

#[test]
fn version_flag_emits_one_line_and_exits_zero() {
    let out = Command::new(daemon_binary())
        .arg("--version")
        .output()
        .expect("spawn");
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("zerodds-amqp-bridged"));
    assert!(s.contains("1.0"));
}

#[test]
fn missing_broker_url_fails_with_exit_1() {
    let mut child = Command::new(daemon_binary())
        .args(["--once"])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn");
    let status = child.wait().expect("wait");
    assert_eq!(status.code(), Some(1), "expected exit 1 for missing broker");
}
