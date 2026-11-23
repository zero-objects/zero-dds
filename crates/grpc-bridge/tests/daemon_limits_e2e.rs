// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-grpc-bridged` §3 on the wire: the `grpc.*` limits of the
//! configuration file as a client sees them — the server's SETTINGS,
//! `REFUSED_STREAM`, `RESOURCE_EXHAUSTED` for a message above
//! `max_recv_message_size` or `max_send_message_size`, the keepalive PINGs
//! and their GOAWAY, `ENHANCE_YOUR_CALM` for PINGs without a call, and
//! `metrics.path`.
//!
//! The reader of the spec's example used to refuse every one of these keys
//! as unknown, so a file that named one did not start.

#![cfg(all(unix, feature = "std"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use zerodds_hpack::{Decoder, Encoder, HeaderField};
use zerodds_http2::{Flags, FrameHeader, FrameType, encode_frame};

const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

fn temp_dir(name: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "zd-grpcd-limits-{name}-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn pick_port() -> u16 {
    let s = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    s.local_addr().unwrap().port()
}

struct Daemon {
    child: Child,
    port: u16,
    log: PathBuf,
    _domain: zerodds_test_domains::DomainLease,
}

impl Daemon {
    /// Starts the binary on `config` (`PORT` is the listener port) on a
    /// leased DDS domain and waits for the listener.
    fn start(dir: &Path, config: &str) -> Self {
        let port = pick_port();
        let domain = zerodds_test_domains::lease_domain();
        let path = dir.join("grpc-bridged.yaml");
        std::fs::write(
            &path,
            format!(
                "domain: {}\ngrpc:\n  bind: \"127.0.0.1:{port}\"\n{config}",
                domain.id()
            ),
        )
        .unwrap();
        let log = dir.join("daemon.log");
        let out = std::fs::File::create(&log).unwrap();
        let child = Command::new(env!("CARGO_BIN_EXE_zerodds-grpc-bridged"))
            .args(["--config", path.to_str().unwrap()])
            // Spec §8.1: the JSON log is on stdout; the exit error on
            // stderr goes to the same file (one offset, as `>log 2>&1`).
            .stdout(out.try_clone().unwrap())
            .stderr(out)
            .spawn()
            .unwrap();
        let d = Self {
            child,
            port,
            log,
            _domain: domain,
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while !d.log_text().contains("\"event\":\"listening\"") {
            assert!(Instant::now() < deadline, "no listener: {}", d.log_text());
            std::thread::sleep(Duration::from_millis(50));
        }
        d
    }

    fn log_text(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    /// A connection after the preface and the client's SETTINGS.
    fn connect(&self) -> H2 {
        let mut s = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        s.write_all(PREFACE).unwrap();
        let mut c = H2 {
            s,
            enc: Encoder::new(),
            dec: Decoder::new(),
        };
        c.frame(FrameType::Settings, 0, 0, &[]);
        c
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        // SAFETY: the pid is our child; SIGTERM lets it flush coverage data.
        unsafe { libc::kill(self.child.id() as i32, libc::SIGTERM) };
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One frame: type, flags, stream id, payload.
type Frame = (u8, u8, u32, Vec<u8>);

/// The client side of an HTTP/2 connection with its HPACK state.
struct H2 {
    s: TcpStream,
    enc: Encoder,
    dec: Decoder,
}

impl H2 {
    fn frame(&mut self, typ: FrameType, flags: u8, stream_id: u32, payload: &[u8]) {
        let h = FrameHeader {
            length: payload.len() as u32,
            frame_type: typ,
            flags: Flags(flags),
            stream_id,
        };
        let mut buf = vec![0u8; 9 + payload.len()];
        encode_frame(&h, payload, &mut buf, 16_777_215).unwrap();
        self.s.write_all(&buf).unwrap();
    }

    /// The next frame, or `None` at the end of the connection or after the
    /// read timeout.
    fn next(&mut self) -> Option<Frame> {
        let mut h = [0u8; 9];
        self.s.read_exact(&mut h).ok()?;
        let len = (usize::from(h[0]) << 16) | (usize::from(h[1]) << 8) | usize::from(h[2]);
        let mut p = vec![0u8; len];
        self.s.read_exact(&mut p).ok()?;
        let sid = u32::from_be_bytes([h[5], h[6], h[7], h[8]]) & 0x7fff_ffff;
        if h[3] == FrameType::Headers as u8 {
            // Every header block goes through the decoder (RFC 7541
            // §2.3.2); the decoded block replaces the payload.
            let fields = self.dec.decode(&p).unwrap();
            p = fields
                .iter()
                .map(|f| format!("{}={}\n", f.name, f.value))
                .collect::<String>()
                .into_bytes();
        }
        Some((h[3], h[4], sid, p))
    }

    /// HEADERS of a call of `method` on `service`.
    fn open(&mut self, sid: u32, service: &str, method: &str, end_stream: bool) {
        let fields: Vec<HeaderField> = [
            (":method", "POST".to_string()),
            (":scheme", "http".to_string()),
            (":path", format!("/zerodds.t.v1.{service}/{method}")),
            ("content-type", "application/grpc+proto".to_string()),
        ]
        .into_iter()
        .map(|(n, v)| HeaderField {
            name: n.into(),
            value: v,
        })
        .collect();
        let block = self.enc.encode(&fields);
        let flags = Flags::END_HEADERS | if end_stream { Flags::END_STREAM } else { 0 };
        self.frame(FrameType::Headers, flags, sid, &block);
    }

    /// The `grpc-status` that ends stream `sid`, skipping other frames.
    fn status_of(&mut self, sid: u32) -> u32 {
        loop {
            let (typ, _, id, p) = self.next().expect("a frame before the status");
            if typ == FrameType::Headers as u8 && id == sid {
                let text = String::from_utf8(p).unwrap();
                if let Some(line) = text.lines().find(|l| l.starts_with("grpc-status=")) {
                    return line["grpc-status=".len()..].parse().unwrap();
                }
            }
        }
    }

    /// Waits until the response HEADERS of the server stream `sid`.
    fn stream_opened(&mut self, sid: u32) {
        loop {
            let (typ, _, id, p) = self.next().expect("the response HEADERS");
            if typ == FrameType::Headers as u8 && id == sid {
                let text = String::from_utf8(p).unwrap();
                assert!(!text.contains("grpc-status"), "the call ended: {text}");
                return;
            }
        }
    }
}

/// A Length-Prefixed `Sample { cdr_payload }` with an XCDR1 little-endian
/// encapsulation header in front of `body` (spec §4.4).
fn sample(body: &[u8]) -> Vec<u8> {
    let mut cdr = vec![0, 1, 0, 0];
    cdr.extend_from_slice(body);
    let mut msg = vec![0x0a];
    let mut n = cdr.len();
    while n >= 0x80 {
        msg.push((n as u8 & 0x7f) | 0x80);
        n >>= 7;
    }
    msg.push(n as u8);
    msg.extend_from_slice(&cdr);
    let mut lpm = vec![0];
    lpm.extend_from_slice(&(msg.len() as u32).to_be_bytes());
    lpm.extend_from_slice(&msg);
    lpm
}

const TOPIC: &str = "topics:\n  - \"T::Limits=LimitStream\"\n";

const RESOURCE_EXHAUSTED: u32 = 8;

/// Spec §3, §4.1: the server's SETTINGS carry `max_concurrent_streams`,
/// `initial_window_size` and `max_frame_size` of the file, a WINDOW_UPDATE
/// opens the connection window to `initial_window_size`, and the
/// Prometheus page is at `metrics.path`.
#[test]
fn grpcbridge_3_the_limits_of_the_file_reach_the_wire() {
    let dir = temp_dir("settings");
    let admin = pick_port();
    let d = Daemon::start(
        &dir,
        &format!(
            "  max_concurrent_streams: 5\n  initial_window_size: 100000\n  max_frame_size: 32768\nmetrics:\n  enabled: true\n  listen: \"127.0.0.1:{admin}\"\n  path: /prom\n{TOPIC}"
        ),
    );
    let mut c = d.connect();
    let (typ, flags, _, settings) = c.next().unwrap();
    assert_eq!((typ, flags), (FrameType::Settings as u8, 0));
    let values: Vec<(u16, u32)> = settings
        .chunks(6)
        .map(|e| {
            (
                u16::from_be_bytes([e[0], e[1]]),
                u32::from_be_bytes([e[2], e[3], e[4], e[5]]),
            )
        })
        .collect();
    assert!(values.contains(&(0x3, 5)), "{values:?}");
    assert!(values.contains(&(0x4, 100_000)), "{values:?}");
    assert!(values.contains(&(0x5, 32_768)), "{values:?}");
    let (typ, _, sid, increment) = c.next().unwrap();
    assert_eq!((typ, sid), (FrameType::WindowUpdate as u8, 0));
    assert_eq!(
        u32::from_be_bytes(increment[..4].try_into().unwrap()),
        100_000 - 65_535
    );
    // A DATA frame above the protocol's 16 384, within the 32 768 of the
    // file, is taken.
    c.open(1, "LimitStream", "Publish", false);
    let big = sample(&[7; 32_000]);
    assert!(big.len() > 16_384 && big.len() <= 32_768);
    c.frame(FrameType::Data, Flags::END_STREAM, 1, &big);
    assert_eq!(c.status_of(1), 0);

    let mut s = TcpStream::connect(("127.0.0.1", admin)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    s.write_all(b"GET /prom HTTP/1.1\r\nHost: x\r\n\r\n")
        .unwrap();
    let mut body = String::new();
    let _ = s.read_to_string(&mut body);
    assert!(body.starts_with("HTTP/1.1 200"), "{body}");
    assert!(body.contains("zerodds_grpc_bridge_rpcs_total"), "{body}");
}

/// Spec §3 `max_recv_message_size`: a Publish whose message is larger is
/// RESOURCE_EXHAUSTED, the stream is reset after the trailers, and the
/// connection serves the next call.
#[test]
fn grpcbridge_3_max_recv_message_size_answers_resource_exhausted() {
    let dir = temp_dir("recv");
    let d = Daemon::start(&dir, &format!("  max_recv_message_size: 16\n{TOPIC}"));
    let mut c = d.connect();
    c.open(1, "LimitStream", "Publish", false);
    c.frame(FrameType::Data, 0, 1, &sample(&[1; 20]));
    assert_eq!(c.status_of(1), RESOURCE_EXHAUSTED);
    loop {
        let (typ, _, sid, p) = c.next().expect("RST_STREAM after the trailers");
        if typ == FrameType::RstStream as u8 {
            assert_eq!((sid, p.as_slice()), (1, &[0u8, 0, 0, 0][..]), "NO_ERROR");
            break;
        }
    }
    c.open(3, "LimitStream", "Publish", false);
    c.frame(FrameType::Data, Flags::END_STREAM, 3, &sample(b"hi"));
    assert_eq!(c.status_of(3), 0);
    assert!(d.log_text().contains("exceeds max_recv_message_size 16"));
}

/// Spec §3 `max_concurrent_streams`: with one stream open (a Subscribe),
/// the next is refused with RST_STREAM(REFUSED_STREAM).
#[test]
fn grpcbridge_3_max_concurrent_streams_refuses_the_stream_above() {
    let dir = temp_dir("streams");
    let d = Daemon::start(&dir, &format!("  max_concurrent_streams: 1\n{TOPIC}"));
    let mut c = d.connect();
    c.open(1, "LimitStream", "Subscribe", true);
    c.stream_opened(1);
    c.open(3, "LimitStream", "Publish", false);
    loop {
        let (typ, _, sid, p) = c.next().expect("RST_STREAM(REFUSED_STREAM)");
        if typ == FrameType::RstStream as u8 {
            assert_eq!(sid, 3);
            assert_eq!(p, 7u32.to_be_bytes(), "REFUSED_STREAM");
            break;
        }
    }
}

/// Spec §3 `max_send_message_size`: a sample larger than it ends the
/// Subscribe stream with RESOURCE_EXHAUSTED instead of going out.
#[test]
fn grpcbridge_3_max_send_message_size_ends_the_stream_with_resource_exhausted() {
    let dir = temp_dir("send");
    let d = Daemon::start(&dir, &format!("  max_send_message_size: 32\n{TOPIC}"));
    let mut sub = d.connect();
    sub.open(1, "LimitStream", "Subscribe", true);
    sub.stream_opened(1);
    sub.s
        .set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    let mut publisher = d.connect();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut sid = 1;
    loop {
        assert!(Instant::now() < deadline, "the stream did not end");
        publisher.open(sid, "LimitStream", "Publish", false);
        publisher.frame(FrameType::Data, Flags::END_STREAM, sid, &sample(&[3; 40]));
        assert_eq!(publisher.status_of(sid), 0);
        sid += 2;
        // A sample written before the stream's reader matched is not
        // delivered (VOLATILE); the next one is.
        while let Some((typ, _, id, p)) = sub.next() {
            assert_ne!(typ, FrameType::Data as u8, "the large message went out");
            if typ == FrameType::Headers as u8 && id == 1 {
                let text = String::from_utf8(p).unwrap();
                assert!(text.contains("grpc-status=8"), "{text}");
                assert!(text.contains("max_send_message_size 32"), "{text}");
                return;
            }
        }
    }
}

/// Spec §3 `keepalive`, §6 Liveliness: with a call open the server PINGs
/// after `time_secs` without a frame; without the ACK it sends GOAWAY
/// after `timeout_secs` and closes the connection.
#[test]
fn grpcbridge_3_keepalive_pings_and_closes_without_an_ack() {
    let dir = temp_dir("keepalive");
    let d = Daemon::start(
        &dir,
        &format!("  keepalive:\n    time_secs: 1\n    timeout_secs: 1\n{TOPIC}"),
    );
    let mut c = d.connect();
    c.open(1, "LimitStream", "Subscribe", true);
    c.stream_opened(1);
    let started = Instant::now();
    let mut pinged = None;
    loop {
        let Some((typ, flags, _, p)) = c.next() else {
            panic!("the connection ended before GOAWAY");
        };
        if typ == FrameType::Ping as u8 && flags & Flags::ACK == 0 {
            pinged.get_or_insert(started.elapsed());
        }
        if typ == FrameType::GoAway as u8 {
            assert!(pinged.is_some(), "GOAWAY without a PING");
            assert_eq!(p[4..8], [0, 0, 0, 0], "NO_ERROR");
            break;
        }
    }
    let at = pinged.unwrap();
    assert!(at >= Duration::from_millis(800), "PING after {at:?}");
    let mut rest = [0u8; 1];
    assert!(
        matches!(c.s.read(&mut rest), Ok(0)),
        "the connection stays open after GOAWAY"
    );
}

/// Spec §3 `keepalive`: a client that acknowledges the PINGs keeps its
/// connection.
#[test]
fn grpcbridge_3_keepalive_with_acks_keeps_the_connection() {
    let dir = temp_dir("keepalive-ack");
    let d = Daemon::start(
        &dir,
        &format!("  keepalive:\n    time_secs: 1\n    timeout_secs: 1\n{TOPIC}"),
    );
    let mut c = d.connect();
    c.open(1, "LimitStream", "Subscribe", true);
    c.stream_opened(1);
    let until = Instant::now() + Duration::from_millis(3500);
    let mut pings = 0;
    while Instant::now() < until {
        let (typ, flags, _, p) = c.next().expect("the connection stays");
        assert_ne!(typ, FrameType::GoAway as u8, "GOAWAY although acknowledged");
        if typ == FrameType::Ping as u8 && flags & Flags::ACK == 0 {
            pings += 1;
            c.frame(FrameType::Ping, Flags::ACK, 0, &p);
        }
    }
    assert!(pings >= 2, "{pings} PINGs in 3.5 s");
}

/// Spec §3 `keepalive.permit_without_calls: false` (the default): the
/// fourth PING without a call ends the connection with
/// GOAWAY(ENHANCE_YOUR_CALM, "too_many_pings"); with `true` every PING is
/// answered and the server PINGs the idle connection itself.
#[test]
fn grpcbridge_3_pings_without_a_call_follow_permit_without_calls() {
    let dir = temp_dir("pings");
    let d = Daemon::start(&dir, TOPIC);
    let mut c = d.connect();
    for _ in 0..4 {
        c.frame(FrameType::Ping, 0, 0, &[1; 8]);
    }
    let mut acks = 0;
    loop {
        let (typ, flags, _, p) = c.next().expect("GOAWAY");
        if typ == FrameType::Ping as u8 && flags & Flags::ACK != 0 {
            acks += 1;
        }
        if typ == FrameType::GoAway as u8 {
            assert_eq!(p[4..8], 0xbu32.to_be_bytes(), "ENHANCE_YOUR_CALM");
            assert_eq!(&p[8..], b"too_many_pings");
            break;
        }
    }
    assert_eq!(acks, 3);

    let dir = temp_dir("pings-permitted");
    let d = Daemon::start(
        &dir,
        &format!("  keepalive:\n    time_secs: 1\n    permit_without_calls: true\n{TOPIC}"),
    );
    let mut c = d.connect();
    for _ in 0..6 {
        c.frame(FrameType::Ping, 0, 0, &[2; 8]);
    }
    let (mut acks, mut server_pings) = (0, 0);
    // Past the 5 s after which a connection without a call and without
    // `permit_without_calls` is closed.
    let until = Instant::now() + Duration::from_millis(6500);
    while Instant::now() < until {
        let (typ, flags, _, p) = c.next().expect("the connection stays");
        assert_ne!(typ, FrameType::GoAway as u8, "GOAWAY although permitted");
        if typ == FrameType::Ping as u8 {
            if flags & Flags::ACK != 0 {
                acks += 1;
            } else {
                server_pings += 1;
                c.frame(FrameType::Ping, Flags::ACK, 0, &p);
            }
        }
    }
    assert_eq!(acks, 6);
    assert!(
        server_pings >= 3,
        "{server_pings} keepalive PINGs on the idle connection in 6.5 s"
    );
}

/// The resident memory of process `pid` and its peak (`VmRSS`, `VmHWM` in
/// kB, Linux `/proc/<pid>/status`).
#[cfg(target_os = "linux")]
fn memory_kb(pid: u32) -> (u64, u64) {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap();
    let kb = |key: &str| -> u64 {
        status
            .lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|v| v.trim().trim_end_matches(" kB").trim().parse().ok())
            .unwrap()
    };
    (kb("VmRSS:"), kb("VmHWM:"))
}

/// Spec §4.1, RFC 9113 §6.5.2, §10.5.1: the SETTINGS announce
/// MAX_HEADER_LIST_SIZE 16 384. A request block as in the review — 64 062
/// octets: `:method`, `:path`, a literal of 4 000 octets with incremental
/// indexing, then 60 000 references to it — is RESOURCE_EXHAUSTED, the
/// daemon's peak memory grows by less than 32 MiB (before the limit it grew
/// by 475 MB on the Linux test host), and the connection serves the next
/// call, which refers to the literal: the HPACK state stayed in step.
#[test]
fn grpcbridge_4_1_a_header_list_above_16_kib_is_resource_exhausted() {
    let dir = temp_dir("header-list");
    let d = Daemon::start(&dir, TOPIC);
    let mut c = d.connect();
    let (typ, _, _, settings) = c.next().unwrap();
    assert_eq!(typ, FrameType::Settings as u8);
    assert!(
        settings
            .chunks(6)
            .any(|e| e[..2] == [0, 6] && e[2..] == 16_384u32.to_be_bytes()),
        "{settings:?}"
    );
    let path = |b: &mut Vec<u8>| {
        b.extend_from_slice(&[0x83, 0x86, 0x04]);
        b.extend_from_slice(&zerodds_hpack::encode_string(
            "/zerodds.t.v1.LimitStream/Publish",
            false,
        ));
        // content-type, literal without indexing, static name 31.
        b.extend_from_slice(&[0x0f, 0x10]);
        b.extend_from_slice(&zerodds_hpack::encode_string("application/grpc", false));
    };
    let mut block = Vec::new();
    path(&mut block);
    block.push(0x40);
    block.extend_from_slice(&zerodds_hpack::encode_string("x", false));
    block.extend_from_slice(&zerodds_hpack::encode_string(&"v".repeat(4000), false));
    block.extend(std::iter::repeat_n(0xbe, 60_000));
    #[cfg(target_os = "linux")]
    let (rss_before, peak_before) = memory_kb(d.child.id());
    let mut chunks = block.chunks(16_384).enumerate().peekable();
    while let Some((i, chunk)) = chunks.next() {
        let last = chunks.peek().is_none();
        let flags = if last { Flags::END_HEADERS } else { 0 };
        if i == 0 {
            c.frame(FrameType::Headers, flags, 1, chunk);
        } else {
            c.frame(FrameType::Continuation, flags, 1, chunk);
        }
    }
    c.frame(FrameType::Data, Flags::END_STREAM, 1, &sample(b"no"));
    assert_eq!(c.status_of(1), RESOURCE_EXHAUSTED);
    #[cfg(target_os = "linux")]
    {
        let (rss_after, peak_after) = memory_kb(d.child.id());
        eprintln!("VmRSS {rss_before} -> {rss_after} kB, VmHWM {peak_before} -> {peak_after} kB");
        assert!(
            peak_after < peak_before + 32 * 1024,
            "VmHWM {peak_before} -> {peak_after} kB"
        );
    }
    // The next call refers to the literal of the refused block (0xbe).
    let mut next = Vec::new();
    path(&mut next);
    next.push(0xbe);
    c.frame(FrameType::Headers, Flags::END_HEADERS, 3, &next);
    c.frame(FrameType::Data, Flags::END_STREAM, 3, &sample(b"hi"));
    assert_eq!(c.status_of(3), 0);
    let log = d.log_text();
    assert!(log.contains("header list exceeds 16384 octets"), "{log}");
}
