// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// E2E-Tests fuer `zerodds-corba-bridged`. Wir spawnen den Daemon,
// senden eine GIOP-Request, und verifizieren dass eine GIOP-Reply
// mit `NoException` zurueckkommt.
//
// Spec: `docs/specs/zerodds-corba-bridge-1.0.md` §12.2.

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
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use zerodds_cdr::Endianness;
use zerodds_corba_giop::version::Version;
use zerodds_corba_giop::{
    Message, Request, ResponseFlags, ServiceContextList, TargetAddress, decode_message,
    encode_message,
};

fn daemon_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_zerodds-corba-bridged"))
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

fn pick_port() -> u16 {
    let s = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    let p = s.local_addr().expect("addr").port();
    drop(s);
    p
}

fn spawn_daemon(port: u16) -> DaemonGuard {
    let bind = format!("127.0.0.1:{port}");
    let child = Command::new(daemon_binary())
        .args([
            "--iiop-bind",
            &bind,
            "--orb-id",
            "zerodds-test",
            "--topic",
            "IDL:Foo:1.0=Foo::Topic",
            "--idle-timeout-ms",
            "5000",
        ])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn daemon");
    std::thread::sleep(Duration::from_millis(300));
    DaemonGuard { child }
}

fn connect(port: u16) -> TcpStream {
    for _ in 0..30 {
        if let Ok(s) = TcpStream::connect(("127.0.0.1", port)) {
            let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
            let _ = s.set_write_timeout(Some(Duration::from_secs(3)));
            return s;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("connect failed after retries");
}

#[test]
fn giop_request_yields_no_exception_reply() {
    let port = pick_port();
    let _guard = spawn_daemon(port);
    let mut stream = connect(port);

    // Build a minimal GIOP 1.2 Request.
    let req = Request {
        request_id: 42,
        response_flags: ResponseFlags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(vec![0u8; 16]),
        operation: "request_quote".into(),
        requesting_principal: None,
        service_context: ServiceContextList::default(),
        body: vec![0xCA, 0xFE, 0xBA, 0xBE],
    };
    let bytes = encode_message(
        Version::V1_2,
        Endianness::Little,
        false,
        &Message::Request(req),
    )
    .expect("encode");
    stream.write_all(&bytes).expect("write request");

    // Read 12-byte header + body.
    let mut hdr = [0u8; 12];
    stream.read_exact(&mut hdr).expect("read header");
    assert_eq!(&hdr[..4], b"GIOP", "magic");
    let endian_le = hdr[6] & 0x01 != 0;
    let size = if endian_le {
        u32::from_le_bytes([hdr[8], hdr[9], hdr[10], hdr[11]])
    } else {
        u32::from_be_bytes([hdr[8], hdr[9], hdr[10], hdr[11]])
    };
    let mut body = vec![0u8; size as usize];
    stream.read_exact(&mut body).expect("read body");
    let mut full = hdr.to_vec();
    full.extend_from_slice(&body);
    let (msg, _) = decode_message(&full).expect("decode reply");
    match msg {
        Message::Reply(r) => {
            assert_eq!(r.request_id, 42);
            // Reply::reply_status NoException → 0.
            assert_eq!(
                r.reply_status,
                zerodds_corba_giop::ReplyStatusType::NoException
            );
        }
        other => panic!("expected Reply, got {:?}", other.message_type()),
    }
}

#[test]
fn dump_iors_writes_stringified_ior() {
    let out = Command::new(daemon_binary())
        .args([
            "--dump-iors",
            "--iiop-bind",
            "127.0.0.1:6833",
            "--topic",
            "IDL:Foo/Bar:1.0=Foo::Bar::Req",
        ])
        .output()
        .expect("spawn");
    assert!(out.status.success(), "exit: {:?}", out.status);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.contains("IOR:"),
        "stdout should contain stringified IOR, got: {s}"
    );
    assert!(s.contains("IDL:Foo/Bar:1.0"));
}

#[test]
fn version_flag_emits_one_line() {
    let out = Command::new(daemon_binary())
        .arg("--version")
        .output()
        .expect("spawn");
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("zerodds-corba-bridged"));
    assert!(s.contains("1.0"));
}

#[test]
fn unknown_arg_yields_exit_1() {
    let mut child = Command::new(daemon_binary())
        .args(["--no-such-arg", "x"])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn");
    let status = child.wait().expect("wait");
    assert_eq!(status.code(), Some(1));
}
