// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// E2E-Test fuer `zerodds-grpc-bridged` per HTTP/2-TCP-Roundtrip.
//
// Spec: `docs/specs/zerodds-grpc-bridge-1.0.md` §12.2.

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

use zerodds_hpack::{Encoder as HpackEncoder, HeaderField};
use zerodds_http2::{Flags, FrameHeader, FrameType, encode_frame};

const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

fn daemon_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_zerodds-grpc-bridged"))
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

fn spawn_daemon(port: u16) -> DaemonGuard {
    let bind = format!("127.0.0.1:{port}");
    let child = Command::new(daemon_binary())
        .args([
            "--bind",
            &bind,
            "--reflection",
            "--topic",
            "Chat::Message=ChatMessageStream",
            "--idle-timeout-ms",
            "5000",
        ])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn daemon");
    // Wait for "listening" log.
    std::thread::sleep(Duration::from_millis(300));
    DaemonGuard { child }
}

fn pick_port() -> u16 {
    // Bind ephemeral once to find a free port, then immediately drop.
    let s = std::net::TcpListener::bind("127.0.0.1:0").expect("ephemeral bind");
    let p = s.local_addr().expect("addr").port();
    drop(s);
    p
}

fn connect(port: u16) -> TcpStream {
    let mut last_err = None;
    for _ in 0..30 {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(s) => {
                let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
                let _ = s.set_write_timeout(Some(Duration::from_secs(3)));
                return s;
            }
            Err(e) => {
                last_err = Some(e);
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
    panic!("connect failed after retries: {last_err:?}");
}

fn write_frame(
    stream: &mut TcpStream,
    typ: FrameType,
    flags: u8,
    stream_id: u32,
    payload: &[u8],
) -> std::io::Result<()> {
    let h = FrameHeader {
        length: payload.len() as u32,
        frame_type: typ,
        flags: Flags(flags),
        stream_id,
    };
    let mut buf = vec![0u8; 9 + payload.len()];
    encode_frame(&h, payload, &mut buf, 16384)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "encode"))?;
    stream.write_all(&buf)
}

#[test]
fn http2_roundtrip_publish_topic() {
    let port = pick_port();
    let _guard = spawn_daemon(port);
    let mut stream = connect(port);

    // 1) Send preface + empty SETTINGS (client-side).
    stream.write_all(PREFACE).expect("write preface");
    write_frame(&mut stream, FrameType::Settings, 0, 0, &[]).expect("client settings");

    // 2) Read server SETTINGS frame (read 9-byte hdr + payload).
    let mut hdr = [0u8; 9];
    stream
        .read_exact(&mut hdr)
        .expect("read server settings hdr");
    let len = ((hdr[0] as u32) << 16) | ((hdr[1] as u32) << 8) | hdr[2] as u32;
    let mut payload = vec![0u8; len as usize];
    if !payload.is_empty() {
        stream.read_exact(&mut payload).expect("settings payload");
    }
    assert_eq!(hdr[3], FrameType::Settings as u8);

    // 3) Send HEADERS with :path /pkg.ChatMessageStream/Publish + END_STREAM (no DATA).
    let mut enc = HpackEncoder::new();
    let headers = vec![
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
            value: "/zerodds.chat.v1.ChatMessageStream/Publish".into(),
        },
        HeaderField {
            name: "content-type".into(),
            value: "application/grpc+proto".into(),
        },
    ];
    let h_payload = enc.encode(&headers);
    write_frame(
        &mut stream,
        FrameType::Headers,
        Flags::END_HEADERS | Flags::END_STREAM,
        1,
        &h_payload,
    )
    .expect("write headers");

    // 4) Read response: server should send HEADERS + DATA(?) + Trailer-HEADERS.
    let mut got_response_hdr = false;
    let mut got_trailer_status_ok = false;
    let mut dec = zerodds_hpack::Decoder::new();
    for _ in 0..6 {
        let mut h = [0u8; 9];
        if stream.read_exact(&mut h).is_err() {
            break;
        }
        let len = ((h[0] as u32) << 16) | ((h[1] as u32) << 8) | h[2] as u32;
        let typ = h[3];
        let flags = h[4];
        let mut p = vec![0u8; len as usize];
        if !p.is_empty() {
            stream.read_exact(&mut p).expect("read payload");
        }
        if typ == FrameType::Headers as u8 {
            // Decode once with shared decoder so dynamic table stays consistent.
            let fields = match dec.decode(&p) {
                Ok(f) => f,
                Err(_) => continue,
            };
            if !got_response_hdr {
                got_response_hdr = true;
            } else if fields
                .iter()
                .any(|f| f.name == "grpc-status" && f.value == "0")
            {
                got_trailer_status_ok = true;
            }
            if (flags & Flags::END_STREAM) != 0 {
                break;
            }
        }
    }
    assert!(got_response_hdr, "no HEADERS response");
    assert!(got_trailer_status_ok, "no grpc-status:0 trailer");
}

#[test]
fn http2_unknown_service_yields_status_5() {
    let port = pick_port();
    let _guard = spawn_daemon(port);
    let mut stream = connect(port);

    stream.write_all(PREFACE).expect("preface");
    write_frame(&mut stream, FrameType::Settings, 0, 0, &[]).expect("settings");
    let mut hdr = [0u8; 9];
    stream.read_exact(&mut hdr).expect("server settings");
    let len = ((hdr[0] as u32) << 16) | ((hdr[1] as u32) << 8) | hdr[2] as u32;
    let mut payload = vec![0u8; len as usize];
    if !payload.is_empty() {
        stream.read_exact(&mut payload).expect("settings payload");
    }

    let mut enc = HpackEncoder::new();
    let headers = vec![
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
            value: "/no.such.Svc/X".into(),
        },
    ];
    let payload2 = enc.encode(&headers);
    write_frame(
        &mut stream,
        FrameType::Headers,
        Flags::END_HEADERS | Flags::END_STREAM,
        1,
        &payload2,
    )
    .expect("write");

    let mut got_status_5 = false;
    let mut dec = zerodds_hpack::Decoder::new();
    for _ in 0..6 {
        let mut h = [0u8; 9];
        if stream.read_exact(&mut h).is_err() {
            break;
        }
        let len = ((h[0] as u32) << 16) | ((h[1] as u32) << 8) | h[2] as u32;
        let typ = h[3];
        let mut p = vec![0u8; len as usize];
        if !p.is_empty() {
            stream.read_exact(&mut p).expect("payload");
        }
        if typ == FrameType::Headers as u8 {
            if let Ok(fields) = dec.decode(&p) {
                if fields
                    .iter()
                    .any(|f| f.name == "grpc-status" && f.value == "5")
                {
                    got_status_5 = true;
                    break;
                }
            }
        }
    }
    assert!(got_status_5, "expected NotFound (5) for unknown service");
}
