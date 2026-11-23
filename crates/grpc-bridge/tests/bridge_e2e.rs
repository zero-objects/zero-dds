// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// E2E test for `zerodds-grpc-bridged` via an HTTP/2-over-TCP roundtrip.
//
// Spec: `docs/specs/zerodds-grpc-bridge-1.0.md` §12.2.

//! The daemon as a subprocess between a client and a DDS participant of the
//! test (spec §12.2). Two tests drive it with a hand-written HTTP/2 client;
//! the `grpcbridge_12_2_*` tests use `grpcurl` as the client, with the
//! descriptors from the reflection service alone, and a DDS DataReader or
//! DataWriter in the test process on a leased domain. Those are `#[ignore]`
//! in the crate tier and run in the integration pillar's job
//! `integration-bridges-vendor-grpc`, which installs `grpcurl`
//! (`tests/integration/provision.sh grpcurl`); a missing `grpcurl` fails
//! them:
//! ```bash
//! cargo nextest run -p zerodds-grpc-bridge --test bridge_e2e --run-ignored all
//! ```

#![cfg(feature = "std")]
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

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use zerodds_dcps::runtime::{DcpsRuntime, UserReaderConfig, UserSample, UserWriterConfig};

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
        terminate_child_gracefully(&mut self.child);
    }
}

/// Graceful SIGTERM termination with a 3 s wait, otherwise a SIGKILL fallback.
/// Lets the daemon run its atexit handlers — in particular the
/// LLVM coverage `__llvm_profile_write_file`. A hard
/// `child.kill()` (SIGKILL) leaves corrupt `.profraw` headers under
/// `cargo llvm-cov` and makes `llvm-profdata merge` fail.
fn terminate_child_gracefully(child: &mut Child) {
    #[cfg(unix)]
    {
        let pid = child.id() as i32;
        // SAFETY: `child.id()` is valid as long as we have not already
        // waited; Drop is exclusive. `libc::kill` with SIGTERM
        // is always call-safe.
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => {
                    if std::time::Instant::now() >= deadline {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(_) => break,
            }
        }
        let _ = child.kill();
        let _ = child.wait();
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
        let _ = child.wait();
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
fn grpcbridge_2_http2_roundtrip_publish_topic() {
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
        Flags::END_HEADERS,
        1,
        &h_payload,
    )
    .expect("write headers");
    // The Sample: `cdr_payload` is the encapsulation header (XCDR1, little
    // endian) and the CDR bytes (spec §4.4), as one Length-Prefixed-Message.
    write_frame(
        &mut stream,
        FrameType::Data,
        Flags::END_STREAM,
        1,
        &[0, 0, 0, 0, 8, 0x0a, 6, 0, 1, 0, 0, b'h', b'i'],
    )
    .expect("write data");

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
fn grpcbridge_2_http2_unknown_service_yields_status_5() {
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
        HeaderField {
            name: "content-type".into(),
            value: "application/grpc".into(),
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

// ---- §12.2 with grpcurl and a DDS participant of the test ------------------

/// The topic the `grpcbridge_12_2_*` daemons map, and its service.
const TOPIC: &str = "Chat::Message";
const SERVICE: &str = "zerodds.chat.v1.ChatMessageStream";

/// Fails the test when `grpcurl` is not on the PATH.
fn require_grpcurl() {
    assert!(
        Command::new("grpcurl").arg("-version").output().is_ok(),
        "`grpcurl` is not on the PATH (tests/integration/provision.sh grpcurl)"
    );
}

/// A daemon of this build on `domain` and a port of its own, `Chat::Message`
/// mapped both ways, with `extra` arguments; ready once it logged its
/// listener (a probing connect would reach it as a connection without a
/// preface).
fn spawn_on(domain: i32, extra: &[&str]) -> (DaemonGuard, String) {
    let port = pick_port();
    let mut child = Command::new(daemon_binary())
        .args([
            "--bind",
            &format!("127.0.0.1:{port}"),
            "--domain",
            &domain.to_string(),
            "--topic",
            &format!("{TOPIC}=ChatMessageStream"),
            "--idle-timeout-ms",
            "60000",
        ])
        .args(extra)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn zerodds-grpc-bridged");
    // Spec §8.1: the JSON log is on stdout.
    let stdout = child.stdout.take().expect("a piped stdout");
    let mut lines = BufReader::new(stdout).lines();
    let listening = lines
        .by_ref()
        .take(40)
        .map_while(Result::ok)
        .find(|line| line.contains("\"event\":\"listening\""));
    assert!(
        listening.is_some(),
        "the daemon did not report its listener on 127.0.0.1:{port}"
    );
    // Keep draining, or the daemon blocks once the pipe fills.
    std::thread::spawn(move || lines.for_each(drop));
    (DaemonGuard { child }, format!("127.0.0.1:{port}"))
}

/// A DDS participant of the test on `domain`.
fn dds_peer(domain: i32) -> Arc<DcpsRuntime> {
    let mut prefix = [0u8; 12];
    prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
    prefix[4..8].copy_from_slice(&domain.to_be_bytes());
    prefix[8] = 0x12;
    prefix[11] = 0x2e;
    DcpsRuntime::start(
        domain,
        zerodds_rtps::wire_types::GuidPrefix::from_bytes(prefix),
        zerodds_dcps::runtime::RuntimeConfig::default(),
    )
    .unwrap()
}

/// The reliable DataReader of `TOPIC` on `rt` with the bridge's type name,
/// once it matched the daemon's DataWriter.
fn dds_reader(rt: &DcpsRuntime) -> mpsc::Receiver<UserSample> {
    let (reader, rx) = rt
        .register_user_reader(UserReaderConfig {
            topic_name: TOPIC.into(),
            type_name: "ZeroDdsBridgeBytes".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while rt.user_reader_matched_count(reader) < 1 {
        assert!(
            Instant::now() < deadline,
            "the daemon's DataWriter did not match"
        );
        rt.wait_match_event(Duration::from_millis(200));
    }
    rx
}

/// The next Alive sample of `rx`: its representation identifier and its
/// CDR bytes.
fn next_alive(rx: &mpsc::Receiver<UserSample>) -> ([u8; 2], Vec<u8>) {
    loop {
        match rx
            .recv_timeout(Duration::from_secs(10))
            .expect("a sample from the daemon")
        {
            UserSample::Alive {
                payload,
                representation_identifier,
                ..
            } => return (representation_identifier, payload.to_vec()),
            _ => continue,
        }
    }
}

/// CDR bytes with every octet value, so that a byte the bridge changed,
/// dropped or added shows.
fn every_octet(seed: u8) -> Vec<u8> {
    (0..=255u8).map(|b| b.wrapping_add(seed)).collect()
}

/// Runs `grpcurl` and returns its stdout, failing on a non-zero exit.
fn grpcurl(args: &[&str]) -> String {
    let out = Command::new("grpcurl")
        .args(args)
        .output()
        .expect("spawn grpcurl");
    assert!(
        out.status.success(),
        "grpcurl {args:?}: status={:?} stderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// §12.2: grpcurl's client-streaming `Publish` of two Samples, encoded from
/// the descriptors the reflection service serves, reaches the DDS reader of
/// the test octet for octet: the encapsulation header of `cdr_payload` is
/// the representation identifier of the DDS sample (here `CDR2_LE`), the
/// rest its CDR bytes; the PublishAck names the sequence number of the
/// second sample.
#[test]
#[ignore = "needs grpcurl; runs in integration-bridges-vendor-grpc"]
fn grpcbridge_12_2_grpcurl_publish_reaches_the_dds_reader_byte_exact() {
    require_grpcurl();
    let lease = zerodds_test_domains::lease_domain();
    let (_daemon, addr) = spawn_on(lease.id(), &["--reflection"]);
    let dds = dds_peer(lease.id());
    let rx = dds_reader(&dds);
    let (first, second) = (every_octet(0), every_octet(0x80));
    let cdr = |body: &[u8]| {
        let mut c = vec![0x00, 0x07, 0x00, 0x00];
        c.extend_from_slice(body);
        B64.encode(c)
    };
    let request = format!(
        "{{\"cdrPayload\":\"{}\"}} {{\"cdrPayload\":\"{}\"}}",
        cdr(&first),
        cdr(&second)
    );
    let ack = grpcurl(&[
        "-plaintext",
        "-d",
        &request,
        &addr,
        &format!("{SERVICE}/Publish"),
    ]);
    assert!(ack.contains("\"sequenceNumber\": \"2\""), "{ack}");
    assert_eq!(next_alive(&rx), ([0x00, 0x07], first));
    assert_eq!(next_alive(&rx), ([0x00, 0x07], second));
    dds.shutdown();
}

/// §12.2: a sample a DataWriter of the test writes (XCDR2, big endian)
/// reaches grpcurl's server-streaming `Subscribe` octet for octet:
/// `cdr_payload` is the encapsulation header `CDR2_BE` and the CDR bytes,
/// with the topic and the type name.
#[test]
#[ignore = "needs grpcurl; runs in integration-bridges-vendor-grpc"]
fn grpcbridge_12_2_a_dds_sample_reaches_grpcurl_subscribe_byte_exact() {
    require_grpcurl();
    let lease = zerodds_test_domains::lease_domain();
    let (_daemon, addr) = spawn_on(lease.id(), &["--reflection"]);
    let dds = dds_peer(lease.id());
    let w = dds
        .register_user_writer(UserWriterConfig {
            topic_name: TOPIC.into(),
            type_name: "ZeroDdsBridgeBytes".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    let mut client = Command::new("grpcurl")
        .args([
            "-plaintext",
            "-max-time",
            "20",
            "-d",
            "{}",
            &addr,
            &format!("{SERVICE}/Subscribe"),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn grpcurl");
    let (tx, lines) = mpsc::channel();
    let out = client.stdout.take().expect("a piped stdout");
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    // The stream's DataReader exists once grpcurl's call is open.
    let deadline = Instant::now() + Duration::from_secs(10);
    while dds.user_writer_matched_count(w) < 1 {
        assert!(
            Instant::now() < deadline,
            "the DataReader of grpcurl's Subscribe did not match"
        );
        dds.wait_match_event(Duration::from_millis(200));
    }
    // The representation of the samples to come, set after the match as
    // the daemon sets it per Publish.
    dds.set_user_writer_data_rep_override(
        w,
        Some(vec![
            zerodds_rtps::publication_data::data_representation::XCDR2,
        ]),
    )
    .unwrap();
    dds.set_user_writer_byte_order_override(w, true).unwrap();
    let body = every_octet(0x33);
    dds.write_user_sample_borrowed(w, &body).unwrap();
    let mut message = Vec::new();
    while let Ok(line) = lines.recv_timeout(Duration::from_secs(10)) {
        let end = line.trim() == "}";
        message.push(line);
        if end {
            break;
        }
    }
    let _ = client.kill();
    let _ = client.wait();
    let field = |name: &str| {
        message
            .iter()
            .find_map(|l| {
                l.trim()
                    .strip_prefix(&format!("\"{name}\": \""))
                    .map(|v| v.trim_end_matches(',').trim_end_matches('"').to_string())
            })
            .unwrap_or_else(|| panic!("no {name} in {message:?}"))
    };
    let mut expected = vec![0x00, 0x06, 0x00, 0x00];
    expected.extend_from_slice(&body);
    assert_eq!(B64.decode(field("cdrPayload")).unwrap(), expected);
    assert_eq!(field("topicName"), TOPIC);
    assert_eq!(field("typeName"), "ZeroDdsBridgeBytes");
    dds.shutdown();
}

/// §12.2 reflection round trip: grpcurl saves the descriptors the
/// reflection service serves for the service (`-protoset-out`), and a call
/// built from that file alone reaches a daemon without reflection, whose
/// `list` grpcurl cannot read; the DDS reader receives the Sample octet for
/// octet.
#[test]
#[ignore = "needs grpcurl; runs in integration-bridges-vendor-grpc"]
fn grpcbridge_12_2_grpcurl_describe_round_trips_the_service_through_reflection() {
    require_grpcurl();
    let lease = zerodds_test_domains::lease_domain();
    let dir = std::env::temp_dir().join(format!(
        "zerodds-grpc-12-2-{}-{}",
        std::process::id(),
        lease.id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let protoset = dir.join("chat.protoset");
    let protoset = protoset.to_str().unwrap();
    {
        let (_with_reflection, addr) = spawn_on(lease.id(), &["--reflection"]);
        let described = grpcurl(&[
            "-plaintext",
            "-protoset-out",
            protoset,
            &addr,
            "describe",
            SERVICE,
        ]);
        assert!(
            described.contains(
                "rpc PublishOne ( .zerodds.chat.v1.Sample ) returns ( .zerodds.chat.v1.PublishAck );"
            ),
            "{described}"
        );
    }
    let (_without_reflection, addr) = spawn_on(lease.id(), &[]);
    let listed = Command::new("grpcurl")
        .args(["-plaintext", &addr, "list"])
        .output()
        .expect("spawn grpcurl");
    assert!(
        !listed.status.success(),
        "the daemon without reflection answered list: {}",
        String::from_utf8_lossy(&listed.stdout)
    );
    let dds = dds_peer(lease.id());
    let rx = dds_reader(&dds);
    let body = every_octet(0x5a);
    let mut cdr = vec![0x00, 0x01, 0x00, 0x00];
    cdr.extend_from_slice(&body);
    let ack = grpcurl(&[
        "-plaintext",
        "-protoset",
        protoset,
        "-d",
        &format!("{{\"cdrPayload\":\"{}\"}}", B64.encode(&cdr)),
        &addr,
        &format!("{SERVICE}/PublishOne"),
    ]);
    assert!(ack.contains("\"sequenceNumber\": \"1\""), "{ack}");
    assert_eq!(next_alive(&rx), ([0x00, 0x01], body));
    dds.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}
