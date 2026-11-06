// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! E2E test of the bridge daemon's **real DDS side** (bridge spec §4.2 to
//! §4.4):
//!
//!   gRPC `Subscribe` opens a server stream with a DataReader of its own
//!   (§4.3); gRPC `Publish(Sample{cdr_payload})` → DDS DataWriter → each
//!   stream's DataReader → a message of each stream with the same
//!   `cdr_payload` (§4.4: the encapsulation header and the CDR bytes).
//!
//! Two streams on one topic each receive the sample: every `Subscribe` has
//! its own reader. This is the Rust-side anchor for the java-omgdds client
//! (`crates/java-omgdds/java`), which speaks the same gRPC wire against this
//! daemon.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::uninlined_format_args
)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
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
    _domain: zerodds_test_domains::DomainLease,
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        // SIGKILL: the test is over, the daemon's shutdown path is not under
        // test here (`daemon_security_e2e.rs` tests SIGTERM).
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

/// Starts the daemon on a leased DDS domain (the host's share of the band,
/// `zerodds-test-domains`) and returns once it reports `listening`, after its
/// DDS participant started (spec §9.1).
fn spawn_daemon(port: u16) -> DaemonGuard {
    let domain = zerodds_test_domains::lease_domain();
    let bind = format!("127.0.0.1:{port}");
    let mut child = Command::new(daemon_binary())
        .args([
            "--bind",
            &bind,
            "--domain",
            &domain.id().to_string(),
            "--topic",
            "Demo::Bytes=DemoStream",
            "--idle-timeout-ms",
            "20000",
        ])
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn daemon");
    let stderr = child.stderr.take().expect("stderr");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        // Reads to the end even after the test stopped listening: a daemon
        // writing to a closed pipe would fail its `eprintln!`.
        for line in BufReader::new(stderr).lines() {
            let Ok(line) = line else { break };
            let _ = tx.send(line);
        }
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(line) if line.contains("\"event\":\"listening\"") => break,
            Ok(_) => {}
            Err(e) => panic!("the daemon did not report listening: {e:?}"),
        }
    }
    DaemonGuard {
        child,
        _domain: domain,
    }
}

fn connect(port: u16) -> TcpStream {
    let s = TcpStream::connect(("127.0.0.1", port)).expect("connect to the listening daemon");
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
    s
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
        } else if typ == FrameType::Headers as u8
            && let Ok(fields) = dec.decode(&p)
        {
            if saw_headers
                && fields
                    .iter()
                    .any(|f| f.name == "grpc-status" && f.value == "0")
            {
                status_ok = true;
            }
            saw_headers = true;
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

/// Opens a gRPC `Subscribe` stream (spec §4.3) and returns once the
/// daemon answered with the response HEADERS: from then on the stream's
/// DataReader exists.
fn open_subscribe(port: u16) -> TcpStream {
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
    let mut dec = HpackDecoder::new();
    loop {
        let (typ, flags, p) = read_frame(&mut stream);
        if typ != FrameType::Headers as u8 {
            continue;
        }
        let fields = dec.decode(&p).expect("hpack");
        if let Some(status) = fields.iter().find(|f| f.name == "grpc-status") {
            panic!(
                "Subscribe answered grpc-status {} instead of a stream",
                status.value
            );
        }
        assert_eq!(flags & Flags::END_STREAM, 0, "Subscribe ended at once");
        return stream;
    }
}

fn read_frame(stream: &mut TcpStream) -> (u8, u8, Vec<u8>) {
    let mut h = [0u8; 9];
    stream.read_exact(&mut h).expect("a frame");
    let len = ((h[0] as usize) << 16) | ((h[1] as usize) << 8) | h[2] as usize;
    let mut p = vec![0u8; len];
    stream.read_exact(&mut p).expect("the frame's payload");
    (h[3], h[4], p)
}

/// The `Sample.cdr_payload` of the next message of an open stream.
fn next_message(stream: &mut TcpStream) -> Vec<u8> {
    loop {
        let (typ, _, p) = read_frame(stream);
        if typ == FrameType::Data as u8 && !p.is_empty() {
            let (_, msg, _) = decode_message(&p).expect("a length-prefixed message");
            return proto_sample_payload(&msg).expect("Sample.cdr_payload");
        }
        assert_ne!(
            typ,
            FrameType::RstStream as u8,
            "the daemon reset the stream"
        );
    }
}

#[test]
fn grpcbridge_4_3_dds_publish_reaches_every_subscribe_stream() {
    let port = pick_port();
    let _daemon = spawn_daemon(port);
    let mut first = open_subscribe(port);
    let mut second = open_subscribe(port);

    // Ingress: gRPC Publish → DDS DataWriter. `cdr_payload` is the
    // encapsulation header (XCDR1, little endian) and the CDR bytes (§4.4).
    let cdr_payload = [&[0x00, 0x01, 0x00, 0x00][..], b"hello-zerodds"].concat();
    assert_eq!(
        publish(port, &cdr_payload),
        1,
        "PublishAck.accepted should be 1 (DDS write ok)"
    );

    // Egress: each stream's own DataReader → a message of that stream.
    assert_eq!(next_message(&mut first), cdr_payload, "first stream");
    assert_eq!(next_message(&mut second), cdr_payload, "second stream");
}

/// Spec §4.4 with DDS-XTypes 1.3 §7.6.3.1.2, Table 60: a sample goes on
/// under the encapsulation header it came under. A `Publish` with
/// `PL_CDR2_BE` (`00 0a`) in front of the vendors' octets of
/// `struct Sample { long a; double b; short c; }` (`@mutable`, XCDR2 big
/// endian, `tests/interop-corpus/expected-bytes.tsv`) reaches each stream
/// with `00 0a` again: the bridge's DataWriter frames the body as
/// `@mutable` big endian, and the stream message carries the identifier
/// its reader got. Before, both directions built the header from the
/// XCDR version and the byte order alone (`CDR2_BE`, `00 06`).
#[test]
fn xtypes_7_6_3_1_2_a_stream_message_carries_the_header_the_sample_came_under() {
    let port = pick_port();
    let _daemon = spawn_daemon(port);
    let mut stream = open_subscribe(port);
    let octets: Vec<u8> = (0..30)
        .map(|i| {
            u8::from_str_radix(
                &"0000001a20000000ffed297930000001400921fb54442d1810000002fffe"[2 * i..2 * i + 2],
                16,
            )
            .unwrap()
        })
        .collect();
    let cdr_payload = [&[0x00, 0x0a, 0x00, 0x00][..], &octets[..]].concat();
    assert_eq!(publish(port, &cdr_payload), 1, "PublishAck.accepted");
    let got = next_message(&mut stream);
    assert_eq!(got[..4], [0x00, 0x0a, 0x00, 0x00], "PL_CDR2_BE");
    assert_eq!(got[4..], octets[..]);
}
