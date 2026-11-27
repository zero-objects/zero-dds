// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-grpc-bridged` §4.1 to §4.5 on the wire, against the daemon:
//! the client-streaming `Publish` (§4.3) with one `PublishAck` at the
//! client's half-close (§4.2), flow control in both directions with
//! messages larger than a frame (§4.1), the request headers a gRPC call
//! needs (§4.3), compressed messages (§4.4) and the status of a deadline,
//! a drain and a lost DDS liveliness (§4.5).
//!
//! The client here keeps the HPACK state of its connection, takes the
//! server's SETTINGS and WINDOW_UPDATEs, sends DATA within the server's
//! windows and frame size, and returns what it receives with WINDOW_UPDATE
//! when asked to (`grant`), as a gRPC client library does.

#![cfg(all(unix, feature = "std"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use zerodds_grpc_bridge::compression::{Encoding, compress, decompress};
use zerodds_grpc_bridge::messages::{CatalogResp, FLAG_DISPOSE, PublishAck, Sample, SubscribeReq};
use zerodds_hpack::{Decoder, Encoder, HeaderField};
use zerodds_http2::{Flags, FrameHeader, FrameType, encode_frame};

const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
const SERVICE: &str = "/zerodds.demo.v1.DemoStream";
/// The topic list of [`Daemon::demo`].
const DEMO_TOPIC: &str = "  - dds_name: \"Demo::Bytes\"\n    grpc_service: DemoStream\n";

fn temp_dir(name: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "zd-grpcd-flows-{name}-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Daemon {
    child: Child,
    port: u16,
    log: PathBuf,
    domain: zerodds_test_domains::DomainLease,
}

impl Daemon {
    /// Starts the binary with `topics` (the YAML of the list) on a leased
    /// DDS domain and waits for the listener.
    fn start(dir: &Path, topics: &str) -> Self {
        Self::start_with(dir, "", topics)
    }

    /// As [`Self::start`], with `extra` (top-level YAML keys) before the
    /// topic list.
    fn start_with(dir: &Path, extra: &str, topics: &str) -> Self {
        Self::start_at(dir, Some("debug"), extra, topics)
    }

    /// As [`Self::start_with`], with `log_level` (`None`: the default).
    fn start_at(dir: &Path, log_level: Option<&str>, extra: &str, topics: &str) -> Self {
        let domain = zerodds_test_domains::lease_domain();
        let path = dir.join("grpc-bridged.yaml");
        let level = log_level.map_or(String::new(), |l| format!("log_level: {l}\n"));
        std::fs::write(
            &path,
            format!(
                "domain: {}\n{level}grpc:\n  bind: \"127.0.0.1:0\"\n{extra}topics:\n{topics}",
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
        let mut d = Self {
            child,
            port: 0,
            log,
            domain,
        };
        d.port = d.logged_port("listening");
        d
    }

    /// The port the daemon names in the `addr` of its `event` line, once
    /// it has written that line.
    fn logged_port(&self, event: &str) -> u16 {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let text = self.log_text();
            if let Some(port) = zerodds_test_domains::logged_port(&text, event, "addr") {
                return port;
            }
            assert!(Instant::now() < deadline, "no {event} line: {text}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// The daemon with the bidirectional topic `Demo::Bytes` as
    /// `DemoStream`.
    fn demo(name: &str) -> Self {
        Self::start(&temp_dir(name), DEMO_TOPIC)
    }

    fn log_text(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    fn connect(&self) -> Client {
        Client::connect(self.port)
    }

    fn sigterm(&self) {
        // SAFETY: the pid is our live child.
        unsafe { libc::kill(self.child.id() as i32, libc::SIGTERM) };
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.sigterm();
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

/// What a client sees on a stream.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Seen {
    Headers(Vec<(String, String)>, bool),
    Data(Vec<u8>),
    Reset(u32),
}

/// An HTTP/2 client of one connection.
struct Client {
    s: TcpStream,
    enc: Encoder,
    dec: Decoder,
    /// The server's MAX_FRAME_SIZE and the windows it grants.
    max_frame: usize,
    conn_window: i64,
    initial_window: i64,
    windows: HashMap<u32, i64>,
    /// Frames of each stream not yet taken by the test.
    seen: HashMap<u32, Vec<Seen>>,
    goaway: Option<(u32, u32)>,
    /// Return the DATA received with WINDOW_UPDATE.
    grant: bool,
}

impl Client {
    fn connect(port: u16) -> Self {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        s.write_all(PREFACE).unwrap();
        let mut c = Self {
            s,
            enc: Encoder::new(),
            dec: Decoder::new(),
            max_frame: 16_384,
            conn_window: 65_535,
            initial_window: 65_535,
            windows: HashMap::new(),
            seen: HashMap::new(),
            goaway: None,
            grant: true,
        };
        c.frame(FrameType::Settings, 0, 0, &[]);
        c
    }

    fn frame(&mut self, typ: FrameType, flags: u8, sid: u32, payload: &[u8]) {
        let h = FrameHeader {
            length: payload.len() as u32,
            frame_type: typ,
            flags: Flags(flags),
            stream_id: sid,
        };
        let mut buf = vec![0u8; 9 + payload.len()];
        encode_frame(&h, payload, &mut buf, 16_777_215).unwrap();
        self.s.write_all(&buf).unwrap();
    }

    /// HEADERS of a call of `method` with the extra request headers.
    fn open(&mut self, sid: u32, method: &str, extra: &[(&str, &str)], end_stream: bool) {
        self.open_path(sid, &format!("{SERVICE}/{method}"), extra, end_stream);
    }

    /// HEADERS of a call of `:path` `path`.
    fn open_path(&mut self, sid: u32, path: &str, extra: &[(&str, &str)], end_stream: bool) {
        let mut fields = vec![
            (":method".to_string(), "POST".to_string()),
            (":scheme".to_string(), "http".to_string()),
            (":path".to_string(), path.to_string()),
        ];
        for (n, v) in extra
            .iter()
            .filter(|(n, _)| n.starts_with(':') && *n != ":method")
        {
            fields.push(((*n).to_string(), (*v).to_string()));
        }
        if !extra.iter().any(|(n, _)| *n == "content-type") {
            fields.push(("content-type".into(), "application/grpc+proto".into()));
        }
        for (n, v) in extra {
            if *n == ":method" {
                fields[0].1 = (*v).to_string();
            } else if !n.starts_with(':') {
                fields.push(((*n).to_string(), (*v).to_string()));
            }
        }
        let block = self.enc.encode(
            &fields
                .into_iter()
                .map(|(name, value)| HeaderField { name, value })
                .collect::<Vec<_>>(),
        );
        let flags = Flags::END_HEADERS | if end_stream { Flags::END_STREAM } else { 0 };
        self.frame(FrameType::Headers, flags, sid, &block);
        self.windows.insert(sid, self.initial_window);
    }

    /// A Length-Prefixed-Message in DATA frames within the server's frame
    /// size and windows.
    fn message(&mut self, sid: u32, msg: &[u8], compressed: bool, end_stream: bool) {
        let mut lpm = vec![u8::from(compressed)];
        lpm.extend_from_slice(&(msg.len() as u32).to_be_bytes());
        lpm.extend_from_slice(msg);
        let mut at = 0;
        while at < lpm.len() {
            let room = self.conn_window.min(self.windows[&sid]);
            if room <= 0 {
                self.read_one();
                continue;
            }
            let n = (lpm.len() - at).min(self.max_frame).min(room as usize);
            let last = at + n == lpm.len();
            let flags = if last && end_stream {
                Flags::END_STREAM
            } else {
                0
            };
            let chunk = lpm[at..at + n].to_vec();
            self.frame(FrameType::Data, flags, sid, &chunk);
            self.conn_window -= n as i64;
            *self.windows.get_mut(&sid).unwrap() -= n as i64;
            at += n;
        }
    }

    fn half_close(&mut self, sid: u32) {
        self.frame(FrameType::Data, Flags::END_STREAM, sid, &[]);
    }

    /// Reads one frame and files it.
    fn read_one(&mut self) {
        let mut h = [0u8; 9];
        self.s.read_exact(&mut h).expect("a frame from the daemon");
        let len = (usize::from(h[0]) << 16) | (usize::from(h[1]) << 8) | usize::from(h[2]);
        let mut p = vec![0u8; len];
        self.s.read_exact(&mut p).unwrap();
        let (typ, flags) = (h[3], h[4]);
        let sid = u32::from_be_bytes([h[5], h[6], h[7], h[8]]) & 0x7fff_ffff;
        if typ == FrameType::Settings as u8 && flags & Flags::ACK == 0 {
            for s in p.chunks(6) {
                let v = i64::from(u32::from_be_bytes([s[2], s[3], s[4], s[5]]));
                match u16::from_be_bytes([s[0], s[1]]) {
                    4 => {
                        let delta = v - self.initial_window;
                        self.initial_window = v;
                        for w in self.windows.values_mut() {
                            *w += delta;
                        }
                    }
                    5 => self.max_frame = v as usize,
                    _ => {}
                }
            }
            self.frame(FrameType::Settings, Flags::ACK, 0, &[]);
        } else if typ == FrameType::WindowUpdate as u8 {
            let inc = i64::from(u32::from_be_bytes([p[0], p[1], p[2], p[3]]) & 0x7fff_ffff);
            if sid == 0 {
                self.conn_window += inc;
            } else if let Some(w) = self.windows.get_mut(&sid) {
                *w += inc;
            }
        } else if typ == FrameType::Headers as u8 {
            let fields = self
                .dec
                .decode(&p)
                .unwrap()
                .into_iter()
                .map(|f| (f.name, f.value))
                .collect();
            let end = flags & Flags::END_STREAM != 0;
            self.seen
                .entry(sid)
                .or_default()
                .push(Seen::Headers(fields, end));
        } else if typ == FrameType::Data as u8 {
            if self.grant && !p.is_empty() {
                let inc = (p.len() as u32).to_be_bytes();
                self.frame(FrameType::WindowUpdate, 0, 0, &inc);
                self.frame(FrameType::WindowUpdate, 0, sid, &inc);
            }
            self.seen.entry(sid).or_default().push(Seen::Data(p));
        } else if typ == FrameType::RstStream as u8 {
            let code = u32::from_be_bytes([p[0], p[1], p[2], p[3]]);
            self.seen.entry(sid).or_default().push(Seen::Reset(code));
        } else if typ == FrameType::GoAway as u8 {
            self.goaway = Some((
                u32::from_be_bytes([p[0], p[1], p[2], p[3]]),
                u32::from_be_bytes([p[4], p[5], p[6], p[7]]),
            ));
        }
    }

    /// The next thing seen on `sid`.
    fn next(&mut self, sid: u32) -> Seen {
        loop {
            if let Some(v) = self.seen.get_mut(&sid)
                && !v.is_empty()
            {
                return v.remove(0);
            }
            self.read_one();
        }
    }

    /// The whole answer of a call: the response headers, the messages
    /// (with their Compressed-Flag), the trailers.
    fn answer(&mut self, sid: u32) -> Answer {
        let mut a = Answer::default();
        let mut data = Vec::new();
        loop {
            match self.next(sid) {
                Seen::Headers(fields, end) => {
                    if a.headers.is_empty() {
                        a.headers = fields.clone();
                    }
                    if end {
                        a.trailers = fields;
                        break;
                    }
                }
                Seen::Data(p) => data.extend_from_slice(&p),
                Seen::Reset(code) => {
                    a.reset = Some(code);
                    break;
                }
            }
        }
        let mut at = 0;
        while at + 5 <= data.len() {
            let len = u32::from_be_bytes([data[at + 1], data[at + 2], data[at + 3], data[at + 4]])
                as usize;
            a.messages
                .push((data[at] == 1, data[at + 5..at + 5 + len].to_vec()));
            at += 5 + len;
        }
        a
    }
}

#[derive(Debug, Default)]
struct Answer {
    headers: Vec<(String, String)>,
    messages: Vec<(bool, Vec<u8>)>,
    trailers: Vec<(String, String)>,
    reset: Option<u32>,
}

impl Answer {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn status(&self) -> u32 {
        self.trailers
            .iter()
            .find(|(n, _)| n == "grpc-status")
            .map(|(_, v)| v.parse().unwrap())
            .unwrap_or_else(|| panic!("no grpc-status: {self:?}"))
    }

    fn grpc_message(&self) -> String {
        self.trailers
            .iter()
            .find(|(n, _)| n == "grpc-message")
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    }
}

/// A `Sample` whose `cdr_payload` is the XCDR1 little-endian header and
/// `body`.
fn sample(body: &[u8]) -> Vec<u8> {
    Sample {
        cdr_payload: [&[0, 1, 0, 0][..], body].concat(),
        ..Sample::default()
    }
    .encode()
}

/// Opens a Subscribe stream with `req` and waits for the response HEADERS:
/// from then on its DataReader exists.
fn subscribe(c: &mut Client, sid: u32, req: &SubscribeReq, extra: &[(&str, &str)]) {
    c.open(sid, "Subscribe", extra, false);
    c.message(sid, &req.encode(), false, true);
    match c.next(sid) {
        Seen::Headers(fields, false) => {
            assert!(
                fields.iter().any(|(n, v)| n == ":status" && v == "200"),
                "{fields:?}"
            );
        }
        other => panic!("Subscribe answered {other:?} instead of a stream"),
    }
}

/// The `cdr_payload` of the next message of an open Subscribe stream.
fn stream_sample(c: &mut Client, sid: u32) -> Sample {
    let mut data = Vec::new();
    loop {
        match c.next(sid) {
            Seen::Data(p) => {
                data.extend_from_slice(&p);
                if data.len() >= 5 {
                    let len = u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as usize;
                    if data.len() >= 5 + len {
                        assert_eq!(data.len(), 5 + len, "one message at a time");
                        return Sample::decode(&data[5..]).unwrap();
                    }
                }
            }
            other => panic!("the stream ended: {other:?}"),
        }
    }
}

/// §4.3, §4.2: a `Publish` client stream of 100 samples writes 100 samples
/// to DDS, in order, and answers once, at the client's half-close, with the
/// sequence number of the last one.
#[test]
fn grpcbridge_4_3_a_publish_stream_of_100_samples_writes_100_and_acks_once() {
    let d = Daemon::demo("hundred");
    let mut sub = d.connect();
    subscribe(&mut sub, 1, &SubscribeReq::default(), &[]);
    let mut c = d.connect();
    c.open(1, "Publish", &[], false);
    for i in 0..100u32 {
        c.message(1, &sample(&i.to_le_bytes()), false, false);
    }
    c.half_close(1);
    let a = c.answer(1);
    assert_eq!(a.status(), 0, "{a:?}");
    assert_eq!(a.messages.len(), 1, "one PublishAck");
    let ack = PublishAck::decode(&a.messages[0].1).unwrap();
    assert_eq!(ack.sequence_number, 100);
    assert!(ack.acked_at_ns > 1_600_000_000_000_000_000);
    for i in 0..100u32 {
        let s = stream_sample(&mut sub, 1);
        assert_eq!(
            s.cdr_payload,
            [&[0, 1, 0, 0][..], &i.to_le_bytes()].concat()
        );
        assert_eq!(s.topic_name, "Demo::Bytes");
        assert_eq!(s.type_name, "ZeroDdsBridgeBytes");
        assert!(s.source_ts_ns > 0);
    }
}

/// §4.1: a megabyte of samples of 16 KiB — each message larger than a
/// frame — on one connection: the client sends within the windows the
/// server grants, the server sends within the windows the subscriber
/// grants with WINDOW_UPDATE (beyond the 65 535 of the start), and every
/// sample arrives whole.
#[test]
fn grpcbridge_4_1_a_megabyte_of_publishes_on_one_connection_all_arrive() {
    let d = Daemon::demo("megabyte");
    let mut sub = d.connect();
    subscribe(&mut sub, 1, &SubscribeReq::default(), &[]);
    let mut c = d.connect();
    c.open(1, "Publish", &[], false);
    let body = |i: u8| vec![i; 16 * 1024];
    for i in 0..64u8 {
        c.message(1, &sample(&body(i)), false, false);
    }
    c.half_close(1);
    let a = c.answer(1);
    assert_eq!(a.status(), 0, "{a:?}");
    assert_eq!(
        PublishAck::decode(&a.messages[0].1)
            .unwrap()
            .sequence_number,
        64
    );
    for i in 0..64u8 {
        let s = stream_sample(&mut sub, 1);
        assert_eq!(s.cdr_payload.len(), 4 + 16 * 1024, "sample {i}");
        assert_eq!(s.cdr_payload[4..], body(i)[..], "sample {i}");
    }
}

/// §4.1: without the subscriber's WINDOW_UPDATE the server stops at the
/// 65 535 octets of the stream window and goes on after it.
#[test]
fn grpcbridge_4_1_the_server_waits_for_the_window_of_the_client() {
    let d = Daemon::demo("window");
    let mut sub = d.connect();
    sub.grant = false;
    subscribe(&mut sub, 1, &SubscribeReq::default(), &[]);
    let mut c = d.connect();
    c.open(1, "Publish", &[], false);
    for i in 0..8u8 {
        c.message(1, &sample(&vec![i; 16 * 1024]), false, false);
    }
    c.half_close(1);
    assert_eq!(c.answer(1).status(), 0);
    let mut received = 0usize;
    sub.s
        .set_read_timeout(Some(Duration::from_millis(1500)))
        .unwrap();
    loop {
        let mut h = [0u8; 9];
        if sub.s.read_exact(&mut h).is_err() {
            break;
        }
        let len = (usize::from(h[0]) << 16) | (usize::from(h[1]) << 8) | usize::from(h[2]);
        let mut p = vec![0u8; len];
        sub.s.read_exact(&mut p).unwrap();
        if h[3] == FrameType::Data as u8 {
            received += len;
        }
    }
    assert_eq!(received, 65_535, "the stream window of the client");
    sub.s
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    sub.grant = true;
    let more = (8 * (16 * 1024 + 4 + 64)) as u32;
    sub.frame(FrameType::WindowUpdate, 0, 0, &more.to_be_bytes());
    sub.frame(FrameType::WindowUpdate, 0, 1, &more.to_be_bytes());
    let mut total = received;
    while total < 8 * (16 * 1024 + 4 + 5) {
        match sub.next(1) {
            Seen::Data(p) => total += p.len(),
            other => panic!("{other:?}"),
        }
    }
}

/// §4.3: a request that is not a gRPC request of the daemon: a
/// content-type other than application/grpc (also one with a multi-byte
/// UTF-8 character across its 16th octet) is HTTP 415 with grpc-status
/// 3, a method other than POST HTTP 405 with grpc-status 12, a message
/// format other than protobuf grpc-status 12; a `PublishOne` with two
/// messages is INTERNAL.
#[test]
fn grpcbridge_4_3_a_wrong_content_type_is_refused() {
    let d = Daemon::demo("content-type");
    let mut c = d.connect();
    c.open(1, "PublishOne", &[("content-type", "text/plain")], true);
    let a = c.answer(1);
    assert_eq!(a.header(":status"), Some("415"));
    assert_eq!(a.status(), 3);
    // A multi-byte UTF-8 character across the 16th octet of the
    // content-type is HTTP 415 as well; the connection lives on.
    c.open(
        3,
        "PublishOne",
        &[("content-type", "application/grp\u{e4}")],
        true,
    );
    let a = c.answer(3);
    assert_eq!(a.header(":status"), Some("415"), "{a:?}");
    assert_eq!(a.status(), 3);
    c.open(5, "PublishOne", &[(":method", "GET")], true);
    let a = c.answer(5);
    assert_eq!(a.header(":status"), Some("405"));
    assert_eq!(a.status(), 12);
    c.open(
        7,
        "Catalog",
        &[("content-type", "application/grpc+json")],
        true,
    );
    let a = c.answer(7);
    assert_eq!(a.header(":status"), Some("200"));
    assert_eq!(a.status(), 12);
    // `application/grpc` without a subtype is protobuf.
    c.open(9, "Catalog", &[("content-type", "application/grpc")], true);
    assert_eq!(c.answer(9).status(), 0);
}

/// §4.2, §5.2: `Catalog` answers a `CatalogResp`.
#[test]
fn grpcbridge_4_2_catalog_answers_with_a_catalog_resp() {
    let d = Daemon::demo("catalog");
    let mut c = d.connect();
    c.open(1, "Catalog", &[], false);
    c.message(1, &[], false, true);
    let a = c.answer(1);
    assert_eq!(a.status(), 0);
    let cat = CatalogResp::decode(&a.messages[0].1).unwrap();
    assert_eq!(cat.topics.len(), 1);
    assert_eq!(cat.topics[0].dds_name, "Demo::Bytes");
    assert_eq!(cat.topics[0].grpc_service, "zerodds.demo.v1.DemoStream");
}

/// §4.4: a gzip-compressed `Publish` reaches DDS byte for byte; the answer
/// is compressed with gzip and says so in `grpc-encoding`; a `Subscribe`
/// with `grpc-encoding: deflate` gets its messages compressed with
/// deflate.
#[test]
fn grpcbridge_4_4_a_gzip_publish_reaches_dds_byte_exact() {
    let d = Daemon::demo("gzip");
    let mut sub = d.connect();
    subscribe(
        &mut sub,
        1,
        &SubscribeReq::default(),
        &[("grpc-encoding", "deflate")],
    );
    let mut c = d.connect();
    c.open(
        1,
        "Publish",
        &[("grpc-encoding", "gzip"), ("grpc-accept-encoding", "gzip")],
        false,
    );
    let body: Vec<u8> = (0..5_000u32).map(|i| (i % 13) as u8).collect();
    let wire = compress(Encoding::Gzip, &sample(&body)).unwrap();
    assert!(wire.len() < body.len() / 4);
    c.message(1, &wire, true, true);
    let a = c.answer(1);
    assert_eq!(a.status(), 0, "{a:?}");
    assert_eq!(a.header("grpc-encoding"), Some("gzip"));
    assert_eq!(
        a.header("grpc-accept-encoding"),
        Some("identity,deflate,gzip")
    );
    let (compressed, ack) = &a.messages[0];
    assert!(compressed);
    let ack = PublishAck::decode(&decompress(Encoding::Gzip, ack, 1 << 20).unwrap()).unwrap();
    assert_eq!(ack.sequence_number, 1);

    let mut data = Vec::new();
    while data.len() < 5
        || data.len() < 5 + u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as usize
    {
        match sub.next(1) {
            Seen::Data(p) => data.extend_from_slice(&p),
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(data[0], 1, "the Compressed-Flag");
    let s = Sample::decode(&decompress(Encoding::Deflate, &data[5..], 1 << 20).unwrap()).unwrap();
    assert_eq!(s.cdr_payload, [&[0, 1, 0, 0][..], &body[..]].concat());
}

/// §4.4, gRPC Compression: a message compressed with an encoding the daemon
/// does not speak is UNIMPLEMENTED, and the answer names the encodings it
/// accepts; the Compressed-Flag with `grpc-encoding: identity` is
/// INTERNAL.
#[test]
fn grpcbridge_4_4_an_unknown_encoding_is_unimplemented_with_accept_encoding() {
    let d = Daemon::demo("snappy");
    let mut c = d.connect();
    c.open(1, "PublishOne", &[("grpc-encoding", "snappy")], false);
    c.message(1, &[1, 2, 3], true, true);
    let a = c.answer(1);
    assert_eq!(a.status(), 12, "{a:?}");
    assert_eq!(
        a.header("grpc-accept-encoding"),
        Some("identity,deflate,gzip")
    );
    assert!(a.grpc_message().contains("snappy"));
    // Uncompressed messages under an unknown grpc-encoding are read.
    c.open(3, "PublishOne", &[("grpc-encoding", "snappy")], false);
    c.message(3, &sample(b"x"), false, true);
    assert_eq!(c.answer(3).status(), 0);
    c.open(5, "PublishOne", &[], false);
    c.message(5, &sample(b"x"), true, true);
    assert_eq!(c.answer(5).status(), 13);
}

/// §4.5: a call past its `grpc-timeout` ends with DEADLINE_EXCEEDED — a
/// Subscribe stream and a Publish stream the client keeps open.
#[test]
fn grpcbridge_4_5_a_call_past_its_grpc_timeout_is_deadline_exceeded() {
    let d = Daemon::demo("deadline");
    let mut c = d.connect();
    let started = Instant::now();
    subscribe(
        &mut c,
        1,
        &SubscribeReq::default(),
        &[("grpc-timeout", "300m")],
    );
    c.open(3, "Publish", &[("grpc-timeout", "400m")], false);
    c.message(3, &sample(b"in time"), false, false);
    let a = c.answer(1);
    assert_eq!(a.status(), 4, "{a:?}");
    assert!(started.elapsed() >= Duration::from_millis(300));
    let a = c.answer(3);
    assert_eq!(a.status(), 4, "{a:?}");
    assert!(started.elapsed() < Duration::from_secs(3));
    // A malformed grpc-timeout is INTERNAL, as grpc-go answers it.
    c.open(5, "Catalog", &[("grpc-timeout", "1x")], true);
    assert_eq!(c.answer(5).status(), 13);
}

/// §4.5, §9.2: GOAWAY(NO_ERROR) names the last stream; with
/// `grpc.drain_secs: 0` the Subscribe streams open at SIGTERM end with
/// UNAVAILABLE at once.
#[test]
fn grpcbridge_4_5_a_stream_open_at_drain_ends_unavailable() {
    let d = Daemon::start_with(&temp_dir("drain"), "  drain_secs: 0\n", DEMO_TOPIC);
    let mut c = d.connect();
    subscribe(&mut c, 1, &SubscribeReq::default(), &[]);
    subscribe(&mut c, 3, &SubscribeReq::default(), &[]);
    d.sigterm();
    for sid in [1, 3] {
        let a = c.answer(sid);
        assert_eq!(a.status(), 14, "{a:?}");
    }
    while c.goaway.is_none() {
        c.read_one();
    }
    assert_eq!(c.goaway, Some((3, 0)));
}

/// §9.2 "let running RPCs end naturally", RFC 9113 §6.9: answers that wait
/// for the client's window at SIGTERM keep the connection. A client with
/// `SETTINGS_INITIAL_WINDOW_SIZE` 16 gets 16 octets of a `Catalog` answer
/// (the call has ended, its trailers wait behind the message) and of the
/// body of a GET `/idl/`, then the GOAWAY; after its WINDOW_UPDATE both
/// arrive whole, the `Catalog` with `grpc-status` 0, then the connection
/// closes and the daemon exits 0, long before the 30 s of the drain.
#[test]
fn grpcbridge_9_2_answers_waiting_for_the_window_at_sigterm_go_out_whole() {
    let dir = temp_dir("drain-window");
    let mut idl: String = (0..100)
        .map(|i| format!("// line {i:03} of the IDL of the drain test\n"))
        .collect();
    idl.push_str("module Demo { struct Bytes { string text; }; };\n");
    std::fs::write(dir.join("demo.idl"), &idl).unwrap();
    let topics = format!(
        "{DEMO_TOPIC}    idl_file: \"{}\"\n",
        dir.join("demo.idl").display()
    );
    let mut d = Daemon::start(&dir, &topics);
    let mut c = d.connect();
    c.grant = false;
    // SETTINGS_INITIAL_WINDOW_SIZE (0x4): 16 octets per stream.
    c.frame(FrameType::Settings, 0, 0, &[0, 4, 0, 0, 0, 16]);
    c.open(1, "Catalog", &[], false);
    c.message(1, &[], false, true);
    c.open_path(3, "/idl/Demo::Bytes", &[(":method", "GET")], true);
    let mut catalog = Vec::new();
    let mut body = Vec::new();
    for (sid, got) in [(1, &mut catalog), (3, &mut body)] {
        match c.next(sid) {
            Seen::Headers(fields, false) => assert!(
                fields.iter().any(|(n, v)| n == ":status" && v == "200"),
                "{fields:?}"
            ),
            other => panic!("stream {sid}: {other:?}"),
        }
        while got.len() < 16 {
            match c.next(sid) {
                Seen::Data(p) => got.extend_from_slice(&p),
                other => panic!("stream {sid} beyond the window: {other:?}"),
            }
        }
        assert_eq!(got.len(), 16, "stream {sid}: the client's window");
    }
    let started = Instant::now();
    d.sigterm();
    while c.goaway.is_none() {
        c.read_one();
    }
    assert_eq!(c.goaway, Some((3, 0)), "GOAWAY(3, NO_ERROR)");
    // Three polls of the connection's shutdown flag pass before the
    // client returns its window.
    std::thread::sleep(Duration::from_millis(600));
    let more = (1u32 << 20).to_be_bytes();
    for sid in [0, 1, 3] {
        c.frame(FrameType::WindowUpdate, 0, sid, &more);
    }
    let status = loop {
        match c.next(1) {
            Seen::Data(p) => catalog.extend_from_slice(&p),
            Seen::Headers(fields, true) => {
                break fields
                    .into_iter()
                    .find(|(n, _)| n == "grpc-status")
                    .map(|(_, v)| v);
            }
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(status.as_deref(), Some("0"));
    let len = u32::from_be_bytes([catalog[1], catalog[2], catalog[3], catalog[4]]) as usize;
    assert!(len + 5 > 16, "the Catalog answer fits the window: {len}");
    assert_eq!(catalog.len(), 5 + len, "one whole message");
    let cat = CatalogResp::decode(&catalog[5..]).unwrap();
    assert_eq!(cat.topics[0].dds_name, "Demo::Bytes");
    while body.len() < idl.len() {
        match c.next(3) {
            Seen::Data(p) => body.extend_from_slice(&p),
            other => panic!("the IDL ended after {} octets: {other:?}", body.len()),
        }
    }
    assert_eq!(body, idl.as_bytes());
    let mut rest = Vec::new();
    let end = c.s.read_to_end(&mut rest);
    assert!(end.is_ok(), "the connection did not close: {end:?}");
    let code = loop {
        if let Some(st) = d.child.try_wait().unwrap() {
            break st.code();
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "no exit: {}",
            d.log_text()
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(code, Some(0), "{}", d.log_text());
    assert!(!d.log_text().contains("\"event\":\"drain_limit\""));
}

/// §4.5: a Subscribe stream whose DDS side lost its liveliness — the one
/// matched DataWriter, MANUAL_BY_TOPIC with a lease of 1 s, stops
/// asserting — gets the writer's sample and then ends with UNAVAILABLE.
#[test]
fn grpcbridge_4_5_liveliness_lost_ends_the_stream_unavailable() {
    use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserWriterConfig};
    let d = Daemon::start(
        &temp_dir("liveliness"),
        "  - dds_name: \"Live::Topic\"\n    grpc_service: zerodds.demo.v1.DemoStream\n    direction: out\n",
    );
    let mut c = d.connect();
    subscribe(&mut c, 1, &SubscribeReq::default(), &[]);
    let mut prefix = [0u8; 12];
    prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
    prefix[11] = 0x7e;
    let rt = DcpsRuntime::start(
        d.domain.id(),
        zerodds_rtps::wire_types::GuidPrefix::from_bytes(prefix),
        RuntimeConfig::default(),
    )
    .unwrap();
    let writer = rt
        .register_user_writer(UserWriterConfig {
            topic_name: "Live::Topic".into(),
            type_name: "ZeroDdsBridgeBytes".into(),
            reliable: true,
            liveliness: zerodds_qos::LivelinessQosPolicy {
                kind: zerodds_qos::LivelinessKind::ManualByTopic,
                lease_duration: zerodds_qos::Duration::from_millis(1_000),
            },
            ..Default::default()
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while rt.user_writer_matched_count(writer) < 1 {
        assert!(
            Instant::now() < deadline,
            "the stream's reader did not match"
        );
        rt.wait_match_event(Duration::from_millis(100));
    }
    // A write asserts a MANUAL_BY_TOPIC writer's liveliness.
    rt.write_user_sample_borrowed(writer, b"alive").unwrap();
    let s = stream_sample(&mut c, 1);
    assert_eq!(&s.cdr_payload[4..], b"alive");
    let a = c.answer(1);
    assert_eq!(a.status(), 14, "{a:?}");
    assert!(a.grpc_message().contains("liveliness"), "{a:?}");
    rt.shutdown();
}

/// The Prometheus page of the metrics listener on `port`.
fn scrape(port: u16) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(b"GET /metrics HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut body = String::new();
    let _ = s.read_to_string(&mut body);
    body
}

/// The value of the sample line of the family `name` (one topic: one
/// series).
fn metric(body: &str, name: &str) -> f64 {
    let series = format!("{name}{{");
    body.lines()
        .find(|l| l.starts_with(&series))
        .and_then(|l| l.rsplit(' ').next())
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| panic!("no {name} in {body}"))
}

/// A DDS participant of the test on the daemon's domain.
fn participant(d: &Daemon, tag: u8) -> std::sync::Arc<zerodds_dcps::runtime::DcpsRuntime> {
    use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig};
    let mut prefix = [0u8; 12];
    prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
    prefix[11] = tag;
    DcpsRuntime::start(
        d.domain.id(),
        zerodds_rtps::wire_types::GuidPrefix::from_bytes(prefix),
        RuntimeConfig::default(),
    )
    .unwrap()
}

/// §4.1: once 256 KiB of a Subscribe stream wait for a client that grants
/// no window beyond the 65 535 octets of the start, the daemon takes no
/// further samples from the stream's DataReader (the octets it handed to
/// the stream stop at the window, the 256 KiB and one message), and what it
/// did not take arrives once the client grants the window.
#[test]
fn grpcbridge_4_1_a_stream_takes_no_sample_once_256_kib_wait_for_the_window() {
    const N: u8 = 40;
    const QUEUE_LIMIT: f64 = 256.0 * 1024.0;
    let d = Daemon::start_with(
        &temp_dir("queue"),
        "metrics:\n  enabled: true\n  listen: \"127.0.0.1:0\"\n",
        DEMO_TOPIC,
    );
    let admin = d.logged_port("admin_endpoint");
    let mut sub = d.connect();
    sub.grant = false;
    subscribe(&mut sub, 1, &SubscribeReq::default(), &[]);
    let mut c = d.connect();
    c.open(1, "Publish", &[], false);
    let body = |i: u8| vec![i; 16 * 1024];
    for i in 0..N {
        c.message(1, &sample(&body(i)), false, false);
    }
    c.half_close(1);
    assert_eq!(c.answer(1).status(), 0);
    // The stream's message count settles while the client grants nothing.
    let mut last = -1.0;
    let deadline = Instant::now() + Duration::from_secs(10);
    let page = loop {
        std::thread::sleep(Duration::from_millis(400));
        let page = scrape(admin);
        let n = metric(&page, "zerodds_grpc_bridge_messages_out_total");
        if n > 0.0 && (n - last).abs() < f64::EPSILON {
            break page;
        }
        assert!(Instant::now() < deadline, "the stream did not settle");
        last = n;
    };
    let messages = metric(&page, "zerodds_grpc_bridge_messages_out_total");
    let octets = metric(&page, "zerodds_grpc_bridge_bytes_out_total");
    let one = octets / messages;
    assert!(messages < f64::from(N), "{messages} of {N} messages taken");
    assert!(
        octets <= 65_535.0 + QUEUE_LIMIT + one,
        "{octets} octets handed to the stream"
    );
    assert!(
        octets >= 65_535.0 + QUEUE_LIMIT,
        "{octets} octets: the stream stopped below its limit"
    );
    // The client grants the window: every sample arrives, in order.
    sub.grant = true;
    let more = (4u32 << 20).to_be_bytes();
    sub.frame(FrameType::WindowUpdate, 0, 0, &more);
    sub.frame(FrameType::WindowUpdate, 0, 1, &more);
    for i in 0..N {
        let s = stream_sample(&mut sub, 1);
        assert_eq!(s.cdr_payload[4..], body(i)[..], "sample {i}");
    }
}

/// §4.2, §4.3: `SubscribeReq.partition_filter` is the partitions of the
/// stream's DataReader, comma-separated, with wildcards: a stream of
/// "x, a*" gets the sample of a DataWriter in partition "ab" and not the
/// one the daemon's own DataWriter writes in the default partition, which
/// a stream without a filter gets.
#[test]
fn grpcbridge_4_2_partition_filter_is_the_partition_of_the_streams_reader() {
    use zerodds_dcps::runtime::UserWriterConfig;
    let d = Daemon::demo("partition");
    let mut c = d.connect();
    let filtered = SubscribeReq {
        partition_filter: "x, a*".into(),
        ..SubscribeReq::default()
    };
    subscribe(&mut c, 1, &filtered, &[]);
    subscribe(&mut c, 3, &SubscribeReq::default(), &[]);
    let rt = participant(&d, 0x7d);
    let writer = rt
        .register_user_writer(UserWriterConfig {
            topic_name: "Demo::Bytes".into(),
            type_name: "ZeroDdsBridgeBytes".into(),
            reliable: true,
            partition: vec!["ab".into()],
            ..Default::default()
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while rt.user_writer_matched_count(writer) < 1 {
        assert!(
            Instant::now() < deadline,
            "the filtered reader did not match"
        );
        rt.wait_match_event(Duration::from_millis(100));
    }
    // The daemon's own DataWriter, in the default partition.
    let mut p = d.connect();
    p.open(1, "PublishOne", &[], false);
    p.message(1, &sample(b"default"), false, true);
    assert_eq!(p.answer(1).status(), 0);
    assert_eq!(&stream_sample(&mut c, 3).cdr_payload[4..], b"default");
    rt.write_user_sample_borrowed(writer, b"in-ab").unwrap();
    assert_eq!(&stream_sample(&mut c, 1).cdr_payload[4..], b"in-ab");
    assert_eq!(
        rt.user_writer_matched_count(writer),
        1,
        "the reader of the default partition does not match partition ab"
    );
    rt.shutdown();
}

/// §4.2: without `include_dispose` a dispose is not streamed; with it the
/// dispose arrives between the samples, with its flag and key and no
/// payload.
#[test]
fn grpcbridge_4_2_a_dispose_is_streamed_only_with_include_dispose() {
    let d = Daemon::demo("dispose");
    let mut c = d.connect();
    subscribe(&mut c, 1, &SubscribeReq::default(), &[]);
    let with = SubscribeReq {
        include_dispose: true,
        ..SubscribeReq::default()
    };
    subscribe(&mut c, 3, &with, &[]);
    let alive = |key: u8, body: &[u8]| {
        Sample {
            cdr_payload: [&[0, 1, 0, 0][..], body].concat(),
            key_hash: vec![key; 16],
            ..Sample::default()
        }
        .encode()
    };
    let mut p = d.connect();
    p.open(1, "Publish", &[], false);
    p.message(1, &alive(7, b"first"), false, false);
    let dispose = Sample {
        flags: FLAG_DISPOSE,
        key_hash: vec![7; 16],
        ..Sample::default()
    };
    p.message(1, &dispose.encode(), false, false);
    p.message(1, &alive(8, b"after"), false, false);
    p.half_close(1);
    assert_eq!(p.answer(1).status(), 0);
    let s = stream_sample(&mut c, 3);
    assert_eq!((&s.cdr_payload[4..], s.flags), (&b"first"[..], 0));
    let s = stream_sample(&mut c, 3);
    assert_eq!(s.flags, FLAG_DISPOSE);
    assert_eq!(s.key_hash, vec![7; 16]);
    assert!(s.cdr_payload.is_empty());
    let s = stream_sample(&mut c, 3);
    assert_eq!(&s.cdr_payload[4..], b"after");
    let s = stream_sample(&mut c, 1);
    assert_eq!((&s.cdr_payload[4..], s.flags), (&b"first"[..], 0));
    let s = stream_sample(&mut c, 1);
    assert_eq!((&s.cdr_payload[4..], s.flags), (&b"after"[..], 0));
}

/// §6 TRANSIENT_LOCAL: the daemon keeps the last N samples of the topic
/// (`qos.history.depth`) and sends the last `initial_burst` of them first;
/// the live samples follow, none twice. A burst above N sends N; a burst
/// with a `partition_filter` is INVALID_ARGUMENT; a VOLATILE topic retains
/// nothing, its burst is empty.
#[test]
fn grpcbridge_6_transient_local_sends_the_initial_burst() {
    let d = Daemon::start(
        &temp_dir("burst"),
        "  - dds_name: \"Demo::Bytes\"\n    grpc_service: DemoStream\n    qos:\n      durability: transient_local\n      history: { kind: keep_last, depth: 3 }\n  - dds_name: \"Live::Bytes\"\n    grpc_service: zerodds.demo.v1.LiveStream\n",
    );
    let mut p = d.connect();
    p.open(1, "Publish", &[], false);
    for i in 1..=5u8 {
        p.message(1, &sample(&[i]), false, false);
    }
    p.half_close(1);
    assert_eq!(p.answer(1).status(), 0);
    std::thread::sleep(Duration::from_millis(300));
    let burst = |n: u32| SubscribeReq {
        initial_burst: n,
        ..SubscribeReq::default()
    };
    let mut c = d.connect();
    subscribe(&mut c, 1, &burst(2), &[]);
    assert_eq!(stream_sample(&mut c, 1).cdr_payload[4..], [4]);
    assert_eq!(stream_sample(&mut c, 1).cdr_payload[4..], [5]);
    subscribe(&mut c, 3, &burst(10), &[]);
    for i in 3..=5u8 {
        assert_eq!(stream_sample(&mut c, 3).cdr_payload[4..], [i], "depth 3");
    }
    // A live sample follows once on each stream.
    p.open(3, "PublishOne", &[], false);
    p.message(3, &sample(&[6]), false, true);
    assert_eq!(p.answer(3).status(), 0);
    assert_eq!(stream_sample(&mut c, 1).cdr_payload[4..], [6]);
    assert_eq!(stream_sample(&mut c, 3).cdr_payload[4..], [6]);
    p.open(5, "PublishOne", &[], false);
    p.message(5, &sample(&[7]), false, true);
    assert_eq!(p.answer(5).status(), 0);
    assert_eq!(stream_sample(&mut c, 1).cdr_payload[4..], [7], "no repeat");
    // With a partition_filter.
    let mut e = d.connect();
    e.open(1, "Subscribe", &[], false);
    let req = SubscribeReq {
        initial_burst: 1,
        partition_filter: "a".into(),
        ..SubscribeReq::default()
    };
    e.message(1, &req.encode(), false, true);
    assert_eq!(e.answer(1).status(), 3);
    // A VOLATILE topic: the stream opens, the first message is live.
    let mut v = d.connect();
    v.open_path(1, "/zerodds.demo.v1.LiveStream/Subscribe", &[], false);
    v.message(1, &burst(5).encode(), false, true);
    match v.next(1) {
        Seen::Headers(f, false) => assert!(f.iter().any(|(n, x)| n == ":status" && x == "200")),
        other => panic!("{other:?}"),
    }
    let mut lp = d.connect();
    lp.open_path(1, "/zerodds.demo.v1.LiveStream/PublishOne", &[], false);
    lp.message(1, &sample(b"live"), false, true);
    assert_eq!(lp.answer(1).status(), 0);
    assert_eq!(&stream_sample(&mut v, 1).cdr_payload[4..], b"live");
}

/// A keyed `Sample` of the instance `key` with `body`.
fn keyed(key: u8, body: &[u8]) -> Vec<u8> {
    Sample {
        cdr_payload: [&[0, 1, 0, 0][..], body].concat(),
        key_hash: vec![key; 16],
        ..Sample::default()
    }
    .encode()
}

/// §6 Durability/History: the daemon retains per instance the last
/// `qos.history.depth`, and a dispose removes the instance: the burst of a
/// stream after five samples of two instances at depth 2 holds the last two
/// of each, in the order they arrived; after the dispose of one instance
/// the burst holds only the other.
#[test]
fn grpcbridge_6_the_burst_keeps_depth_per_instance_and_not_a_disposed_one() {
    let d = Daemon::start(
        &temp_dir("instances"),
        "  - dds_name: \"Demo::Bytes\"\n    grpc_service: DemoStream\n    qos:\n      durability: transient_local\n      history: { kind: keep_last, depth: 2 }\n",
    );
    let mut p = d.connect();
    p.open(1, "Publish", &[], false);
    for (key, body) in [(1, b"a1"), (2, b"b1"), (1, b"a2"), (2, b"b2"), (1, b"a3")] {
        p.message(1, &keyed(key, body), false, false);
    }
    p.half_close(1);
    assert_eq!(p.answer(1).status(), 0);
    std::thread::sleep(Duration::from_millis(300));
    let burst = SubscribeReq {
        initial_burst: 10,
        ..SubscribeReq::default()
    };
    let mut c = d.connect();
    subscribe(&mut c, 1, &burst, &[]);
    for (key, body) in [(2, b"b1"), (1, b"a2"), (2, b"b2"), (1, b"a3")] {
        let s = stream_sample(&mut c, 1);
        assert_eq!(&s.cdr_payload[4..], body, "depth 2 per instance");
        assert_eq!(s.key_hash, vec![key; 16]);
    }
    let dispose = Sample {
        flags: FLAG_DISPOSE,
        key_hash: vec![2; 16],
        ..Sample::default()
    };
    p.open(3, "PublishOne", &[], false);
    p.message(3, &dispose.encode(), false, true);
    assert_eq!(p.answer(3).status(), 0);
    std::thread::sleep(Duration::from_millis(300));
    let mut e = d.connect();
    subscribe(&mut e, 1, &burst, &[]);
    for body in [b"a2", b"a3"] {
        assert_eq!(&stream_sample(&mut e, 1).cdr_payload[4..], body);
    }
    // Nothing of instance 2 follows: the next message is a live one.
    p.open(5, "PublishOne", &[], false);
    p.message(5, &keyed(1, b"a4"), false, true);
    assert_eq!(p.answer(5).status(), 0);
    assert_eq!(&stream_sample(&mut e, 1).cdr_payload[4..], b"a4");
}

/// §6 TRANSIENT_LOCAL, §4.1: the burst waits for the client's window as
/// the live samples do: a client that grants no window gets no more than
/// the window, the 256 KiB and one message of a burst of 640 KiB; the rest
/// follows, in order, once it grants the window.
#[test]
fn grpcbridge_6_the_burst_stops_once_256_kib_wait_for_the_window() {
    const N: u8 = 40;
    const QUEUE_LIMIT: f64 = 256.0 * 1024.0;
    let d = Daemon::start_with(
        &temp_dir("burstwindow"),
        "metrics:\n  enabled: true\n  listen: \"127.0.0.1:0\"\n",
        "  - dds_name: \"Demo::Bytes\"\n    grpc_service: DemoStream\n    qos:\n      durability: transient_local\n",
    );
    let admin = d.logged_port("admin_endpoint");
    let body = |i: u8| vec![i; 16 * 1024];
    let mut c = d.connect();
    c.open(1, "Publish", &[], false);
    for i in 0..N {
        c.message(1, &sample(&body(i)), false, false);
    }
    c.half_close(1);
    assert_eq!(c.answer(1).status(), 0);
    std::thread::sleep(Duration::from_millis(500));
    let mut sub = d.connect();
    sub.grant = false;
    let burst = SubscribeReq {
        initial_burst: u32::from(N),
        ..SubscribeReq::default()
    };
    subscribe(&mut sub, 1, &burst, &[]);
    let mut last = -1.0;
    let deadline = Instant::now() + Duration::from_secs(10);
    let page = loop {
        std::thread::sleep(Duration::from_millis(400));
        let page = scrape(admin);
        let n = metric(&page, "zerodds_grpc_bridge_messages_out_total");
        if n > 0.0 && (n - last).abs() < f64::EPSILON {
            break page;
        }
        assert!(Instant::now() < deadline, "the stream did not settle");
        last = n;
    };
    let messages = metric(&page, "zerodds_grpc_bridge_messages_out_total");
    let octets = metric(&page, "zerodds_grpc_bridge_bytes_out_total");
    let one = octets / messages;
    assert!(
        messages < f64::from(N),
        "{messages} of {N} burst messages taken"
    );
    assert!(
        octets <= 65_535.0 + QUEUE_LIMIT + one,
        "{octets} octets of the burst handed to the stream"
    );
    sub.grant = true;
    let more = (4u32 << 20).to_be_bytes();
    sub.frame(FrameType::WindowUpdate, 0, 0, &more);
    sub.frame(FrameType::WindowUpdate, 0, 1, &more);
    for i in 0..N {
        let s = stream_sample(&mut sub, 1);
        assert_eq!(s.cdr_payload[4..], body(i)[..], "burst sample {i}");
    }
}

/// §6 TRANSIENT_LOCAL: the stream of a burst misses nothing between the
/// burst and its live reader. A DDS writer of a second participant writes
/// every 2 ms while a client subscribes with `initial_burst`: the samples
/// it writes before the stream's VOLATILE reader has matched it come from
/// the retained samples, the later ones from the live reader, each once
/// and in order.
#[test]
fn grpcbridge_6_the_burst_and_the_live_samples_of_another_participant_have_no_gap() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use zerodds_dcps::runtime::UserWriterConfig;
    const LAST: u32 = 1000;
    let d = Daemon::start(
        &temp_dir("burstgap"),
        "  - dds_name: \"Demo::Bytes\"\n    grpc_service: DemoStream\n    direction: out\n    qos:\n      durability: transient_local\n",
    );
    let rt = participant(&d, 0x6a);
    let writer = rt
        .register_user_writer(UserWriterConfig {
            topic_name: "Demo::Bytes".into(),
            type_name: "ZeroDdsBridgeBytes".into(),
            reliable: true,
            durability: zerodds_qos::DurabilityKind::TransientLocal,
            ..Default::default()
        })
        .unwrap();
    rt.set_user_writer_history_depth(writer, 1000).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while rt.user_writer_matched_count(writer) < 1 {
        assert!(
            Instant::now() < deadline,
            "the retained reader did not match"
        );
        rt.wait_match_event(Duration::from_millis(100));
    }
    let written = Arc::new(AtomicU32::new(0));
    let go = Arc::new(AtomicBool::new(true));
    let feeder = {
        let (rt, written, go) = (Arc::clone(&rt), Arc::clone(&written), Arc::clone(&go));
        std::thread::spawn(move || {
            for i in 0..=LAST {
                if !go.load(Ordering::SeqCst) {
                    break;
                }
                rt.write_user_sample_borrowed(writer, &i.to_le_bytes())
                    .unwrap();
                written.store(i, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(2));
            }
        })
    };
    while written.load(Ordering::SeqCst) < 50 {
        std::thread::sleep(Duration::from_millis(5));
    }
    let value = |s: &Sample| u32::from_le_bytes(s.cdr_payload[4..8].try_into().unwrap());
    let burst = |n: u32| SubscribeReq {
        initial_burst: n,
        ..SubscribeReq::default()
    };
    // The retained DataReader follows the writer: a burst of one is a
    // sample it wrote.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let mut probe = d.connect();
        subscribe(&mut probe, 1, &burst(1), &[]);
        if value(&stream_sample(&mut probe, 1)) + 20 >= written.load(Ordering::SeqCst) {
            break;
        }
        assert!(Instant::now() < deadline, "the retained samples lag");
    }
    let mut c = d.connect();
    subscribe(&mut c, 1, &burst(5), &[]);
    let first = value(&stream_sample(&mut c, 1));
    assert!(first >= 40, "the burst starts with sample {first}");
    let mut prev = first;
    while prev < LAST {
        let next = value(&stream_sample(&mut c, 1));
        assert_eq!(next, prev + 1, "after {prev} came {next}");
        prev = next;
    }
    go.store(false, Ordering::SeqCst);
    feeder.join().unwrap();
    rt.shutdown();
}

/// §6 BEST_EFFORT: a stream of a best-effort topic whose client grants no
/// window drops the samples its reader takes while 256 KiB wait for the
/// window, and counts them in `zerodds_grpc_bridge_samples_dropped_total`.
#[test]
fn grpcbridge_6_a_best_effort_stream_drops_when_the_window_is_full_and_counts_it() {
    const N: u8 = 40;
    // The default log level (`info`) shows the `samples_dropped` line.
    let d = Daemon::start_at(
        &temp_dir("besteffort"),
        None,
        "metrics:\n  enabled: true\n  listen: \"127.0.0.1:0\"\n",
        "  - dds_name: \"Demo::Bytes\"\n    grpc_service: DemoStream\n    qos:\n      reliability: best_effort\n",
    );
    let admin = d.logged_port("admin_endpoint");
    let mut sub = d.connect();
    sub.grant = false;
    subscribe(&mut sub, 1, &SubscribeReq::default(), &[]);
    let mut c = d.connect();
    c.open(1, "Publish", &[], false);
    for i in 0..N {
        c.message(1, &sample(&vec![i; 16 * 1024]), false, false);
        std::thread::sleep(Duration::from_millis(5));
    }
    c.half_close(1);
    assert_eq!(c.answer(1).status(), 0);
    let deadline = Instant::now() + Duration::from_secs(10);
    let (out, dropped) = loop {
        std::thread::sleep(Duration::from_millis(300));
        let page = scrape(admin);
        let out = metric(&page, "zerodds_grpc_bridge_messages_out_total");
        let dropped = metric(&page, "zerodds_grpc_bridge_samples_dropped_total");
        if out + dropped >= f64::from(N) {
            break (out, dropped);
        }
        assert!(
            Instant::now() < deadline,
            "{out} sent and {dropped} dropped of {N}: {page}"
        );
    };
    assert!(dropped > 0.0, "{out} sent, nothing dropped");
    assert!(out < f64::from(N));
    let log = d.log_text();
    let logged: u64 = log
        .lines()
        .filter(|l| l.contains("\"event\":\"samples_dropped\""))
        .filter_map(|l| l.rsplit("\"count\":").next())
        .filter_map(|v| v.trim_end_matches('}').parse::<u64>().ok())
        .sum();
    assert!(
        logged > 0,
        "no samples_dropped line at the default level: {log}"
    );
    // The lines count what the metric counts, the rest at the end of the
    // stream.
    drop(sub);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let log = d.log_text();
        let logged: f64 = log
            .lines()
            .filter(|l| l.contains("\"event\":\"samples_dropped\""))
            .filter_map(|l| l.rsplit("\"count\":").next())
            .filter_map(|v| v.trim_end_matches('}').parse::<f64>().ok())
            .sum();
        let dropped = metric(&scrape(admin), "zerodds_grpc_bridge_samples_dropped_total");
        if (logged - dropped).abs() < f64::EPSILON {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "{logged} logged, {dropped} counted: {log}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The lifespans in milliseconds of the DataWriters `reader` matched,
/// sorted.
fn matched_lifespans(
    rt: &zerodds_dcps::runtime::DcpsRuntime,
    reader: zerodds_rtps::EntityId,
) -> Vec<u128> {
    let mut v: Vec<u128> = rt
        .user_reader_matched_publication_handles(reader)
        .into_iter()
        .filter_map(|h| rt.user_reader_matched_publication_data(reader, h))
        .map(|p| p.lifespan.duration.to_millis())
        .collect();
    v.sort_unstable();
    v
}

/// §6 Lifespan: a Publish with `zerodds-lifespan-ms` writes with a
/// DataWriter of that lifespan (DDS readers see it in the publication's
/// LIFESPAN), one with only `grpc-timeout` with its time rounded up to
/// whole seconds, `qos.lifespan_ms` is the lifespan of the topic's own
/// DataWriter; a value that is no number of milliseconds is
/// INVALID_ARGUMENT.
#[test]
fn grpcbridge_6_lifespan_metadata_reaches_the_writer() {
    use zerodds_dcps::runtime::UserReaderConfig;
    let d = Daemon::start(
        &temp_dir("lifespan"),
        "  - dds_name: \"Demo::Bytes\"\n    grpc_service: DemoStream\n    qos:\n      lifespan_ms: 60000\n",
    );
    let rt = participant(&d, 0x6c);
    let (reader, rx) = rt
        .register_user_reader(UserReaderConfig {
            topic_name: "Demo::Bytes".into(),
            type_name: "ZeroDdsBridgeBytes".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    let lifespans = || matched_lifespans(&rt, reader);
    let deadline = Instant::now() + Duration::from_secs(10);
    while lifespans().is_empty() {
        assert!(Instant::now() < deadline, "no match");
        rt.wait_match_event(Duration::from_millis(100));
    }
    assert_eq!(lifespans(), [60_000]);
    let mut c = d.connect();
    c.open(1, "PublishOne", &[("zerodds-lifespan-ms", "1500")], false);
    c.message(1, &sample(b"short"), false, true);
    assert_eq!(c.answer(1).status(), 0);
    let got = rx.recv_timeout(Duration::from_secs(5)).expect("the sample");
    assert!(matches!(
        got,
        zerodds_dcps::runtime::UserSample::Alive { .. }
    ));
    assert_eq!(lifespans(), [1_500, 60_000]);
    // Without the metadata the grpc-timeout, rounded up to whole seconds.
    c.open(3, "PublishOne", &[("grpc-timeout", "4500m")], false);
    c.message(3, &sample(b"timed"), false, true);
    assert_eq!(c.answer(3).status(), 0);
    // The answer comes when the sample is written; the reader knows the
    // new DataWriter once its sample arrived (it was matched before).
    let got = rx.recv_timeout(Duration::from_secs(5)).expect("the sample");
    assert!(matches!(
        got,
        zerodds_dcps::runtime::UserSample::Alive { .. }
    ));
    assert_eq!(lifespans(), [1_500, 5_000, 60_000]);
    c.open(5, "PublishOne", &[("zerodds-lifespan-ms", "soon")], false);
    c.message(5, &sample(b"x"), false, true);
    assert_eq!(c.answer(5).status(), 3);
    rt.shutdown();
}

/// §6 Lifespan: a topic has at most 8 DataWriters of other lifespans.
/// On a VOLATILE topic a ninth lifespan takes the place of the one that
/// wrote least recently, once its reader acknowledged its samples: ten
/// distinct lifespans (as a client with one deadline over its calls sends
/// them) all write, and the DDS reader sees the eight newest beside the
/// topic's own DataWriter. On a TRANSIENT_LOCAL topic only a DataWriter
/// whose last sample expired makes room: a ninth lifespan within the
/// lifespans of the eight is RESOURCE_EXHAUSTED.
#[test]
fn grpcbridge_6_a_ninth_lifespan_takes_the_place_of_the_least_recently_used() {
    use zerodds_dcps::runtime::UserReaderConfig;
    let d = Daemon::start(
        &temp_dir("lifespanlru"),
        "  - dds_name: \"Demo::Bytes\"\n    grpc_service: DemoStream\n    qos:\n      lifespan_ms: 60000\n",
    );
    let rt = participant(&d, 0x6d);
    let (reader, rx) = rt
        .register_user_reader(UserReaderConfig {
            topic_name: "Demo::Bytes".into(),
            type_name: "ZeroDdsBridgeBytes".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    let lifespans = || matched_lifespans(&rt, reader);
    let deadline = Instant::now() + Duration::from_secs(10);
    while lifespans().is_empty() {
        assert!(Instant::now() < deadline, "no match");
        rt.wait_match_event(Duration::from_millis(100));
    }
    let mut c = d.connect();
    for i in 1..=10u32 {
        let sid = 2 * i - 1;
        let ms = (31 - i) * 1000;
        c.open(
            sid,
            "PublishOne",
            &[("grpc-timeout", &format!("{}S", 31 - i))],
            false,
        );
        c.message(sid, &sample(&i.to_le_bytes()), false, true);
        let a = c.answer(sid);
        assert_eq!(a.status(), 0, "lifespan {ms}: {a:?}");
        let got = rx.recv_timeout(Duration::from_secs(5)).expect("the sample");
        assert!(matches!(
            got,
            zerodds_dcps::runtime::UserSample::Alive { .. }
        ));
    }
    // 30 s and 29 s, the least recently used, made room for 22 s and 21 s.
    let expected: Vec<u128> = (21..=28).map(|s| s * 1000).chain([60_000]).collect();
    let deadline = Instant::now() + Duration::from_secs(10);
    while lifespans() != expected {
        assert!(
            Instant::now() < deadline,
            "the reader sees the lifespans {:?}",
            lifespans()
        );
        rt.wait_match_event(Duration::from_millis(100));
    }
    rt.shutdown();
    drop(d);
    // TRANSIENT_LOCAL: no DataWriter of the eight expired.
    let d = Daemon::start(
        &temp_dir("lifespanfull"),
        "  - dds_name: \"Demo::Bytes\"\n    grpc_service: DemoStream\n    direction: in\n    qos:\n      durability: transient_local\n",
    );
    let mut c = d.connect();
    for i in 1..=9u32 {
        let sid = 2 * i - 1;
        let ms = (60_000 + i).to_string();
        c.open(sid, "PublishOne", &[("zerodds-lifespan-ms", &ms)], false);
        c.message(sid, &sample(&i.to_le_bytes()), false, true);
        let a = c.answer(sid);
        let want = if i <= 8 { 0 } else { 8 };
        assert_eq!(a.status(), want, "lifespan {ms}: {a:?}");
    }
}

/// §6 Deadline: a stream of a topic with `qos.deadline_ms` whose writer
/// stops ends with the misses of its reader in the trailer
/// `zerodds-deadline-missed`, and the daemon logs them.
#[test]
fn grpcbridge_6_a_missed_deadline_is_a_trailer() {
    use zerodds_dcps::runtime::UserWriterConfig;
    let d = Daemon::start(
        &temp_dir("deadline"),
        "  - dds_name: \"Demo::Bytes\"\n    grpc_service: DemoStream\n    direction: out\n    qos:\n      deadline_ms: 100\n",
    );
    let mut c = d.connect();
    subscribe(
        &mut c,
        1,
        &SubscribeReq::default(),
        &[("grpc-timeout", "1500m")],
    );
    let rt = participant(&d, 0x64);
    let mut cfg = UserWriterConfig {
        topic_name: "Demo::Bytes".into(),
        type_name: "ZeroDdsBridgeBytes".into(),
        reliable: true,
        ..Default::default()
    };
    cfg.deadline.period = zerodds_qos::Duration::from_millis(100);
    let writer = rt.register_user_writer(cfg).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while rt.user_writer_matched_count(writer) < 1 {
        assert!(Instant::now() < deadline, "no match");
        rt.wait_match_event(Duration::from_millis(100));
    }
    rt.write_user_sample_borrowed(writer, &[0, 1, 0, 0, 9])
        .unwrap();
    let _ = stream_sample(&mut c, 1);
    let a = c.answer(1);
    assert_eq!(a.status(), 4, "{a:?}");
    let missed: i32 = a
        .trailers
        .iter()
        .find(|(n, _)| n == "zerodds-deadline-missed")
        .map(|(_, v)| v.parse().unwrap())
        .unwrap_or_else(|| panic!("no zerodds-deadline-missed: {a:?}"));
    assert!(missed >= 1, "{missed}");
    assert!(d.log_text().contains("\"event\":\"deadline_missed\""));
    rt.shutdown();
}

/// One message of an open stream, whole.
fn stream_message(c: &mut Client, sid: u32) -> Vec<u8> {
    let mut data = Vec::new();
    loop {
        match c.next(sid) {
            Seen::Data(p) => {
                data.extend_from_slice(&p);
                if data.len() >= 5 {
                    let len = u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as usize;
                    if data.len() >= 5 + len {
                        assert_eq!(data.len(), 5 + len, "one message at a time");
                        return data[5..].to_vec();
                    }
                }
            }
            Seen::Headers(f, false) => assert!(f.iter().any(|(n, _)| n == ":status"), "{f:?}"),
            other => panic!("the stream ended: {other:?}"),
        }
    }
}

/// A `ServerReflectionRequest` with `host` and the oneof member `member`.
fn reflection_request(member: u8, value: &[u8]) -> Vec<u8> {
    let mut r = vec![0x0a, 4];
    r.extend_from_slice(b"here");
    r.push((member << 3) | 2);
    r.push(value.len() as u8);
    r.extend_from_slice(value);
    r
}

/// §4.6: `ServerReflectionInfo` is a bidirectional stream — each request
/// gets its response on the open stream (the services in the package of
/// the mapping, the descriptor of the file that declares a symbol, an
/// `error_response` NOT_FOUND for an unknown symbol), the client's
/// half-close ends it with OK. Both names of the service are served.
#[test]
fn grpcbridge_4_6_the_reflection_stream_answers_each_request() {
    let d = Daemon::start_with(
        &temp_dir("reflection"),
        "  reflection:\n    enabled: true\n",
        "  - dds_name: \"Chat::Message\"\n  - dds_name: \"Trade\"\n    grpc_package: acme.v2\n",
    );
    for (sid, service) in [
        (1, "grpc.reflection.v1.ServerReflection"),
        (3, "grpc.reflection.v1alpha.ServerReflection"),
    ] {
        let mut c = d.connect();
        c.open_path(sid, &format!("/{service}/ServerReflectionInfo"), &[], false);
        c.message(sid, &reflection_request(7, b""), false, false);
        let list = stream_message(&mut c, sid);
        let text = String::from_utf8_lossy(&list);
        assert!(text.contains("zerodds.chat.v1.ChatMessageStream"), "{text}");
        assert!(text.contains("acme.v2.TradeStream"), "{text}");
        assert!(
            text.contains("grpc.reflection.v1alpha.ServerReflection"),
            "{text}"
        );
        // valid_host and original_request repeat the request.
        assert_eq!(&list[..6], b"\x0a\x04here");
        c.message(
            sid,
            &reflection_request(4, b"zerodds.chat.v1.ChatMessageStream.Subscribe"),
            false,
            false,
        );
        let fd = stream_message(&mut c, sid);
        let text = String::from_utf8_lossy(&fd);
        assert!(text.contains("zerodds/chat/v1/bridge.proto"), "{text}");
        assert!(text.contains(".zerodds.chat.v1.SubscribeReq"), "{text}");
        assert!(text.contains("cdr_payload"), "{text}");
        c.message(
            sid,
            &reflection_request(4, b"zerodds.chat.v1.Nothing"),
            false,
            false,
        );
        let err = stream_message(&mut c, sid);
        // error_response (7) { error_code (1) = 5 }.
        let at = err
            .windows(3)
            .position(|w| w[0] == 0x3a && w[2] == 0x08)
            .expect("error_response");
        assert_eq!(err[at + 3], 5, "{err:?}");
        c.half_close(sid);
        let a = c.answer(sid);
        assert_eq!(a.status(), 0, "{a:?}");
    }
    // Another method of the reflection service is UNIMPLEMENTED.
    let mut c = d.connect();
    c.open_path(1, "/grpc.reflection.v1.ServerReflection/Foo", &[], false);
    c.message(1, &reflection_request(7, b""), false, true);
    let a = c.answer(1);
    assert_eq!(a.status(), 12, "{a:?}");
    assert!(a.messages.is_empty(), "{a:?}");
    // A request that names nothing ends the stream with INVALID_ARGUMENT.
    let mut c = d.connect();
    c.open_path(
        1,
        "/grpc.reflection.v1.ServerReflection/ServerReflectionInfo",
        &[],
        false,
    );
    c.message(1, b"\x0a\x01h", false, false);
    assert_eq!(c.answer(1).status(), 3);
}

/// Whether the daemon sent something on `c` within `wait`.
fn frame_ready(c: &mut Client, wait: Duration) -> bool {
    c.s.set_read_timeout(Some(wait)).unwrap();
    let mut b = [0u8; 1];
    let ready = matches!(c.s.peek(&mut b), Ok(n) if n > 0);
    c.s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    ready
}

/// §4.6, RFC 9113 §6.9: a reflection stream whose client does not read
/// answers no further request once 256 KiB of answers wait for its window
/// (it holds the window, the 256 KiB and one answer); the requests after
/// them wait, and the daemon returns no window of the stream meanwhile, so
/// the client stops sending within the 65 535 octets of
/// `initial_window_size`.
/// Once the client reads, every request is answered and the half-close
/// behind them ends the stream with OK.
#[test]
fn grpcbridge_4_6_a_reflection_stream_answers_no_request_once_256_kib_wait_for_the_window() {
    const N: usize = 3_000;
    const QUEUE_LIMIT: usize = 256 * 1024;
    let d = Daemon::start_with(
        &temp_dir("reflection-queue"),
        "  initial_window_size: 65535\n  reflection:\n    enabled: true\nmetrics:\n  enabled: true\n  listen: \"127.0.0.1:0\"\n",
        "  - dds_name: \"Chat::Message\"\n",
    );
    let admin = d.logged_port("admin_endpoint");
    let request = reflection_request(4, b"zerodds.chat.v1.ChatMessageStream.Subscribe");
    let need = request.len() as i64 + 5;
    let mut c = d.connect();
    c.grant = false;
    c.open_path(
        1,
        "/grpc.reflection.v1.ServerReflection/ServerReflectionInfo",
        &[],
        false,
    );
    let mut sent = 0;
    'send: while sent < N {
        while c.conn_window.min(c.windows[&1]) < need {
            if !frame_ready(&mut c, Duration::from_millis(500)) {
                break 'send;
            }
            c.read_one();
        }
        c.message(1, &request, false, false);
        sent += 1;
        while frame_ready(&mut c, Duration::from_millis(1)) {
            c.read_one();
        }
    }
    let answers = |page: &str| -> usize {
        page.lines()
            .find_map(|l| l.strip_prefix("zerodds_grpc_frames_out_total "))
            .and_then(|v| v.trim().parse::<f64>().ok())
            .unwrap_or_else(|| panic!("no zerodds_grpc_frames_out_total in {page}"))
            as usize
    };
    let held_answers = answers(&scrape(admin));
    let first = stream_message(&mut c, 1);
    let one = first.len() + 5;
    eprintln!("{sent} requests sent, {held_answers} answers of {one} octets queued");
    assert!(
        sent < N,
        "the daemon returned the window for all {N} requests"
    );
    assert!(
        held_answers * one <= 65_535 + QUEUE_LIMIT + one,
        "{held_answers} answers of {one} octets"
    );
    assert!(
        held_answers * one >= QUEUE_LIMIT,
        "{held_answers} answers of {one} octets: the stream stopped below its limit"
    );
    // The client reads: the rest of the requests go out, all are answered.
    c.grant = true;
    let more = (16u32 << 20).to_be_bytes();
    c.frame(FrameType::WindowUpdate, 0, 0, &more);
    c.frame(FrameType::WindowUpdate, 0, 1, &more);
    while sent < N {
        c.message(1, &request, false, false);
        sent += 1;
    }
    c.half_close(1);
    let a = c.answer(1);
    assert_eq!(a.status(), 0, "{:?}", a.trailers);
    assert_eq!(a.messages.len() + 1, N);
    assert!(a.messages.iter().all(|(_, m)| *m == first));
}

/// A daemon with `Chat::Message` and its `idl_file`, and `Other` without
/// one; the IDL text.
fn idl_daemon(name: &str) -> (Daemon, &'static str) {
    let dir = temp_dir(name);
    let idl = "module Chat {\n  struct Message { @key long id; string text; };\n};\n";
    let file = dir.join("chat.idl");
    std::fs::write(&file, idl).unwrap();
    let topics = format!(
        "  - dds_name: \"Chat::Message\"\n    dds_type: \"Chat::Message\"\n    idl_file: \"{}\"\n  - dds_name: \"Other\"\n",
        file.display()
    );
    (Daemon::start(&dir, &topics), idl)
}

/// One HTTP/1.1 request on a new connection; the whole response.
fn http1(port: u16, request: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(request.as_bytes()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out
}

/// §5.2: `GET /idl/<dds_name>` returns the IDL source of the topic as
/// `text/x-omg-idl` — as HTTP/1.1 on the cleartext listener, and as an
/// HTTP/2 GET on a gRPC connection.
#[test]
fn grpcbridge_5_2_get_idl_returns_the_source_as_text_x_omg_idl() {
    let (d, idl) = idl_daemon("idl");
    let r = http1(d.port, "GET /idl/Chat::Message HTTP/1.1\r\nHost: x\r\n\r\n");
    assert!(r.starts_with("HTTP/1.1 200 OK\r\n"), "{r}");
    assert!(r.contains("content-type: text/x-omg-idl\r\n"), "{r}");
    assert!(
        r.contains(&format!("content-length: {}\r\n", idl.len())),
        "{r}"
    );
    assert!(r.ends_with(&format!("\r\n\r\n{idl}")), "{r}");
    // The HTTP/2 GET on a gRPC connection.
    let mut c = d.connect();
    c.open_path(1, "/idl/Chat%3A%3AMessage", &[(":method", "GET")], true);
    let mut status = String::new();
    let mut body = Vec::new();
    while body.len() < idl.len() {
        match c.next(1) {
            Seen::Headers(f, _) => {
                for (n, v) in f {
                    if n == "content-type" {
                        assert_eq!(v, "text/x-omg-idl");
                    }
                    if n == ":status" {
                        status = v;
                    }
                }
            }
            Seen::Data(p) => body.extend_from_slice(&p),
            Seen::Reset(code) => panic!("reset {code}"),
        }
    }
    assert_eq!(status, "200");
    assert_eq!(body, idl.as_bytes());
    // The connection still serves gRPC calls.
    c.open_path(3, "/zerodds.chat.v1.ChatMessageStream/Catalog", &[], true);
    assert_eq!(c.answer(3).status(), 0);
}

/// §5.2: a topic that is not mapped and one without `idl_file` are 404,
/// another path 404, another method 405.
#[test]
fn grpcbridge_5_2_get_idl_of_an_unknown_topic_is_404() {
    let (d, _) = idl_daemon("idl404");
    for (request, status) in [
        ("GET /idl/Nothing HTTP/1.1\r\n\r\n", "404"),
        ("GET /idl/Other HTTP/1.1\r\n\r\n", "404"),
        ("GET /catalog HTTP/1.1\r\n\r\n", "404"),
        (
            "POST /idl/Chat::Message HTTP/1.1\r\ncontent-length: 0\r\n\r\n",
            "405",
        ),
        ("GET /idl/%ZZ HTTP/1.1\r\n\r\n", "400"),
    ] {
        let r = http1(d.port, request);
        assert!(
            r.starts_with(&format!("HTTP/1.1 {status} ")),
            "{request}: {r}"
        );
    }
}

/// §4.2, §5.2: the `Catalog` RPC names `dds_type`, the fully-qualified
/// service and, for a topic with `idl_file`, `idl_url` — the IDL endpoint
/// at the authority the client addressed; the URL serves the IDL.
#[test]
fn grpcbridge_5_2_the_catalog_rpc_names_dds_type_service_and_idl_url() {
    let (d, idl) = idl_daemon("idlurl");
    let mut c = d.connect();
    let authority = format!("127.0.0.1:{}", d.port);
    c.open_path(
        1,
        "/zerodds.chat.v1.ChatMessageStream/Catalog",
        &[(":authority", &authority)],
        true,
    );
    let a = c.answer(1);
    assert_eq!(a.status(), 0, "{a:?}");
    let cat = CatalogResp::decode(&a.messages[0].1).unwrap();
    let chat = &cat.topics[0];
    assert_eq!(chat.dds_type, "Chat::Message");
    assert_eq!(chat.grpc_service, "zerodds.chat.v1.ChatMessageStream");
    assert_eq!(
        chat.idl_url,
        format!("http://{authority}/idl/Chat::Message")
    );
    assert_eq!(cat.topics[1].grpc_service, "zerodds.other.v1.OtherStream");
    assert_eq!(cat.topics[1].idl_url, "", "no idl_file, no URL");
    let path = chat
        .idl_url
        .strip_prefix(&format!("http://{authority}"))
        .unwrap();
    let r = http1(d.port, &format!("GET {path} HTTP/1.1\r\n\r\n"));
    assert!(r.ends_with(idl), "{r}");
}
