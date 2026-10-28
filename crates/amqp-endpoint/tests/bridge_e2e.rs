// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// E2E tests of the `zerodds-amqp-bridged` command line: exit codes, the
// refusals before any traffic flows. The traffic itself is measured against
// RabbitMQ in `bridge_rabbitmq_e2e.rs`.
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
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const AMQP_PROTOCOL_HEADER: [u8; 8] = [b'A', b'M', b'Q', b'P', 0x00, 0x01, 0x00, 0x00];

fn daemon_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_zerodds-amqp-bridged"))
}

fn pick_port_listener() -> (TcpListener, String) {
    let l = TcpListener::bind("127.0.0.1:0").expect("ephemeral bind");
    let p = l.local_addr().expect("addr").port();
    (l, format!("127.0.0.1:{p}"))
}

/// A peer that answers the SASL header with the bare AMQP header: an
/// AMQP 1.0 peer without the SASL layer. The daemon must not continue
/// unauthenticated; it stops with the reason.
#[test]
fn a_broker_without_the_sasl_layer_is_refused_by_name() {
    let (listener, addr) = pick_port_listener();
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let (mut s, _) = listener.accept().expect("accept");
        let mut hdr = [0u8; 8];
        let _ = s.read_exact(&mut hdr);
        let _ = s.write_all(&AMQP_PROTOCOL_HEADER);
        let _ = tx.send(hdr.to_vec());
        std::thread::sleep(Duration::from_millis(500));
    });
    let out = Command::new(daemon_binary())
        .args([
            "--broker",
            &format!("amqp://{addr}"),
            "--sasl-mechanism",
            "ANONYMOUS",
            "--once",
            "--connect-timeout-ms",
            "3000",
        ])
        .output()
        .expect("spawn daemon");
    let first = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the daemon connected");
    assert_eq!(&first[..], b"AMQP\x03\x01\x00\x00", "SASL header first");
    assert_eq!(out.status.code(), Some(5), "SASL failure (spec §2)");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("does not offer SASL"), "{stderr}");
}

/// PLAIN sends the password in the clear; towards a remote broker the
/// daemon refuses it without TLS (spec §7.2).
#[test]
fn plain_without_tls_is_refused_towards_a_remote_broker() {
    let out = Command::new(daemon_binary())
        .args([
            "--broker",
            "amqp://broker.example.com",
            "--sasl-mechanism",
            "PLAIN",
            "--user",
            "u",
            "--password",
            "p",
            "--once",
        ])
        .output()
        .expect("spawn daemon");
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("amqps://"));
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
