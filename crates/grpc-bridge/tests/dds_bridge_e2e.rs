// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! E2E test for the bridge daemon's **real DDS side** (bridge spec §4.2,
//! feature `dds-runtime`). Proves the `FUTURE (L2)` stub is gone:
//!
//!   gRPC `Publish(Sample{payload})`  →  DDS DataWriter  →  (loopback)
//!   →  DDS DataReader  →  gRPC `Subscribe` returns the same `Sample`.
//!
//! This is the Rust-side anchor for the F2b java-omgdds multi-process
//! bridge: the daemon really moves opaque bytes through the DCPS runtime,
//! not an echo. The Java client (`crates/java-omgdds/java`) speaks the
//! identical gRPC wire against this daemon.
//!
//! The crate doc sits before `#![cfg]` so the (empty) crate still carries
//! documentation when `dds-runtime` is off — otherwise `missing_docs` +
//! `-D warnings` fails the build.
#![cfg(feature = "dds-runtime")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::uninlined_format_args
)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use zerodds_grpc_bridge::{decode_message, encode_message};
use zerodds_hpack::{Decoder as HpackDecoder, Encoder as HpackEncoder, HeaderField};
use zerodds_http2::{Flags, FrameHeader, FrameType, encode_frame};

const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
const SERVICE: &str = "/zerodds.bridge.v1.DemoStream";

fn daemon_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_zerodds-grpc-bridged"))
}

struct DaemonGuard {
    child: Child,
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let pid = self.child.id() as i32;
            // SAFETY: child still owned (Drop is exclusive); SIGTERM is call-safe.
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                match self.child.try_wait() {
                    Ok(Some(_)) => return,
                    Ok(None) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    _ => break,
                }
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn pick_port() -> u16 {
    let s = std::net::TcpListener::bind("127.0.0.1:0").expect("ephemeral bind");
    let p = s.local_addr().expect("addr").port();
    drop(s);
    p
}

fn spawn_daemon(port: u16, domain: i32) -> DaemonGuard {
    let bind = format!("127.0.0.1:{port}");
    let child = Command::new(daemon_binary())
        .args([
            "--bind",
            &bind,
            "--domain",
            &domain.to_string(),
            "--topic",
            "Demo::Bytes=DemoStream",
            "--idle-timeout-ms",
            "20000",
        ])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn daemon");
    // DDS participant startup (UDP sockets + writer/reader registration).
    std::thread::sleep(Duration::from_millis(600));
    DaemonGuard { child }
}

fn connect(port: u16) -> TcpStream {
    for _ in 0..40 {
        if let Ok(s) = TcpStream::connect(("127.0.0.1", port)) {
            let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
            let _ = s.set_write_timeout(Some(Duration::from_secs(3)));
            return s;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("connect failed after retries");
}

fn write_frame(stream: &mut TcpStream, typ: FrameType, flags: u8, sid: u32, payload: &[u8]) {
    let h = FrameHeader {
        length: payload.len() as u32,
        frame_type: typ,
        flags: Flags(flags),
        stream_id: sid,
    };
    let mut buf = vec![0u8; 9 + payload.len()];
    encode_frame(&h, payload, &mut buf, 16384).expect("encode frame");
    stream.write_all(&buf).expect("write frame");
}

/// HTTP/2 client preface + SETTINGS handshake (drains the server SETTINGS).
fn handshake(stream: &mut TcpStream) {
    stream.write_all(PREFACE).expect("preface");
    write_frame(stream, FrameType::Settings, 0, 0, &[]);
    let mut hdr = [0u8; 9];
    stream.read_exact(&mut hdr).expect("server settings hdr");
    let len = ((hdr[0] as u32) << 16) | ((hdr[1] as u32) << 8) | hdr[2] as u32;
    let mut payload = vec![0u8; len as usize];
    if !payload.is_empty() {
        stream.read_exact(&mut payload).expect("settings payload");
    }
}

/// Encodes `Sample { bytes payload = 1; }` protobuf.
fn proto_sample(payload: &[u8]) -> Vec<u8> {
    let mut out = vec![0x0A];
    let mut v = payload.len() as u64;
    loop {
        let mut b = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            break;
        }
    }
    out.extend_from_slice(payload);
    out
}

/// Reads `Sample.payload` (field 1, wire-type 2) from a protobuf message.
fn proto_sample_payload(msg: &[u8]) -> Option<Vec<u8>> {
    if msg.first() != Some(&0x0A) {
        return None;
    }
    let mut i = 1usize;
    let mut len = 0u64;
    let mut shift = 0u32;
    while i < msg.len() {
        let b = msg[i];
        i += 1;
        len |= u64::from(b & 0x7F) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    let end = (i + len as usize).min(msg.len());
    Some(msg[i..end].to_vec())
}

/// Reads response frames on `stream`, returning the first DATA-frame body
/// (LPM-decoded protobuf message) and whether a `grpc-status:0` trailer was
/// seen.
fn read_response(stream: &mut TcpStream) -> (Option<Vec<u8>>, bool) {
    let mut dec = HpackDecoder::new();
    let mut data: Option<Vec<u8>> = None;
    let mut status_ok = false;
    let mut saw_headers = false;
    for _ in 0..8 {
        let mut h = [0u8; 9];
        if stream.read_exact(&mut h).is_err() {
            break;
        }
        let len = ((h[0] as u32) << 16) | ((h[1] as u32) << 8) | h[2] as u32;
        let typ = h[3];
        let flags = h[4];
        let mut p = vec![0u8; len as usize];
        if !p.is_empty() && stream.read_exact(&mut p).is_err() {
            break;
        }
        if typ == FrameType::Data as u8 && !p.is_empty() {
            if let Ok((_, msg, _)) = decode_message(&p) {
                data = Some(msg);
            }
        } else if typ == FrameType::Headers as u8 {
            if let Ok(fields) = dec.decode(&p) {
                if saw_headers
                    && fields
                        .iter()
                        .any(|f| f.name == "grpc-status" && f.value == "0")
                {
                    status_ok = true;
                }
                saw_headers = true;
            }
        }
        if (flags & Flags::END_STREAM) != 0 && typ == FrameType::Headers as u8 && saw_headers {
            break;
        }
    }
    (data, status_ok)
}

/// gRPC `Publish(Sample{payload})`. Returns `PublishAck.accepted`.
fn publish(port: u16, payload: &[u8]) -> u64 {
    let mut stream = connect(port);
    handshake(&mut stream);

    // HEADERS (no END_STREAM) — the DATA frame carries the Sample.
    let mut enc = HpackEncoder::new();
    let h = enc.encode(&[
        HeaderField {
            name: ":method".into(),
            value: "POST".into(),
        },
        HeaderField {
            name: ":scheme".into(),
            value: "http".into(),
        },
        HeaderField {
            name: ":path".into(),
            value: format!("{SERVICE}/Publish"),
        },
        HeaderField {
            name: "content-type".into(),
            value: "application/grpc+proto".into(),
        },
    ]);
    write_frame(&mut stream, FrameType::Headers, Flags::END_HEADERS, 1, &h);

    // DATA: LPM(Sample) + END_STREAM.
    let lpm = encode_message(&proto_sample(payload), false).expect("lpm");
    write_frame(&mut stream, FrameType::Data, Flags::END_STREAM, 1, &lpm);

    let (data, status_ok) = read_response(&mut stream);
    assert!(status_ok, "Publish: no grpc-status:0 trailer");
    // PublishAck { uint64 accepted = 1; } → tag 0x08 + varint.
    match data {
        Some(msg) if msg.first() == Some(&0x08) => {
            let mut v = 0u64;
            let mut shift = 0u32;
            for &b in &msg[1..] {
                v |= u64::from(b & 0x7F) << shift;
                if b & 0x80 == 0 {
                    break;
                }
                shift += 7;
            }
            v
        }
        _ => 0,
    }
}

/// gRPC `Subscribe` — returns the next `Sample.payload`, if any.
fn subscribe_once(port: u16) -> Option<Vec<u8>> {
    let mut stream = connect(port);
    handshake(&mut stream);
    let mut enc = HpackEncoder::new();
    let h = enc.encode(&[
        HeaderField {
            name: ":method".into(),
            value: "POST".into(),
        },
        HeaderField {
            name: ":scheme".into(),
            value: "http".into(),
        },
        HeaderField {
            name: ":path".into(),
            value: format!("{SERVICE}/Subscribe"),
        },
        HeaderField {
            name: "content-type".into(),
            value: "application/grpc+proto".into(),
        },
    ]);
    write_frame(
        &mut stream,
        FrameType::Headers,
        Flags::END_HEADERS | Flags::END_STREAM,
        1,
        &h,
    );
    let (data, _ok) = read_response(&mut stream);
    data.and_then(|msg| proto_sample_payload(&msg))
}

#[test]
fn dds_publish_then_subscribe_roundtrip() {
    let port = pick_port();
    // Domain derived from port → low collision risk on a shared host.
    let domain = ((port % 200) as i32).max(1);
    let _guard = spawn_daemon(port, domain);

    // Ingress: gRPC Publish → DDS DataWriter.
    let accepted = publish(port, b"hello-zerodds");
    assert_eq!(
        accepted, 1,
        "PublishAck.accepted should be 1 (DDS write ok)"
    );

    // Egress: DDS DataReader (loopback) → gRPC Subscribe. Retry until the
    // sample has propagated through the runtime to the reader channel.
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut got = None;
    while Instant::now() < deadline {
        if let Some(p) = subscribe_once(port) {
            got = Some(p);
            break;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    assert_eq!(
        got.as_deref(),
        Some(&b"hello-zerodds"[..]),
        "Subscribe should return the published payload through real DDS"
    );
}
