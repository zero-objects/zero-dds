// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-grpc-bridged` §8.1 and §8.3 on the wire: the binary started with
//! a config file, a client in this file speaking HTTP/2 over TCP. §8.1: the
//! JSON log on stdout, one `rpc` line per call (a reset call and one whose
//! connection closed CANCELLED), one `http` line per request to the IDL
//! endpoint. §8.3: with `OTEL_EXPORTER_OTLP_ENDPOINT` the spans of the calls
//! reach the endpoint, a call with a W3C `traceparent` in the caller's
//! trace; a failed export is a `warn` line; a value the exporter cannot
//! serve stops the start; without the variable nothing is sent. The
//! endpoint here is a listener of the test; `cross_vendor.rs` has the
//! OpenTelemetry Collector on the other side.

#![cfg(all(unix, feature = "std"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[path = "support/metrics.rs"]
mod metrics;

use metrics::{metric, scrape};
use zerodds_hpack::{Decoder, Encoder, HeaderField};
use zerodds_http2::{Flags, FrameHeader, FrameType, encode_frame};

const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

/// The caller's span of the W3C Trace Context §3.2.3 example, sampled.
const TRACEPARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
/// A caller that did not sample its request.
const UNSAMPLED: &str = "00-11111111111111111111111111111111-2222222222222222-00";

fn temp_dir(name: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "zd-grpcd-obs-{name}-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn pick_port() -> u16 {
    let s = TcpListener::bind("127.0.0.1:0").unwrap();
    s.local_addr().unwrap().port()
}

/// The daemon with its stdout and stderr read into memory, line by line.
struct Daemon {
    child: Child,
    port: u16,
    stdout: Arc<Mutex<Vec<String>>>,
    stderr: Arc<Mutex<Vec<String>>>,
    _domain: zerodds_test_domains::DomainLease,
}

fn collect(from: impl Read + Send + 'static) -> Arc<Mutex<Vec<String>>> {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&lines);
    std::thread::spawn(move || {
        for line in BufReader::new(from).lines().map_while(Result::ok) {
            sink.lock().unwrap().push(line);
        }
    });
    lines
}

impl Daemon {
    /// The daemon on `config` (the listener port and the leased domain put
    /// in front), with `args` and `env`, once its listener accepts.
    fn start(name: &str, config: &str, args: &[&str], env: &[(&str, String)]) -> Self {
        let dir = temp_dir(name);
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
        let mut command = Command::new(env!("CARGO_BIN_EXE_zerodds-grpc-bridged"));
        command
            .args(["--config", path.to_str().unwrap()])
            .args(args)
            .env_remove("OTEL_EXPORTER_OTLP_ENDPOINT")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in env {
            command.env(k, v);
        }
        let mut child = command.spawn().unwrap();
        let stdout = collect(child.stdout.take().unwrap());
        let stderr = collect(child.stderr.take().unwrap());
        let d = Self {
            child,
            port,
            stdout,
            stderr,
            _domain: domain,
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while !d
            .out()
            .iter()
            .any(|l| l.contains("\"event\":\"listening\""))
        {
            assert!(Instant::now() < deadline, "no listener: {:?}", d.out());
            std::thread::sleep(Duration::from_millis(20));
        }
        d
    }

    fn out(&self) -> Vec<String> {
        self.stdout.lock().unwrap().clone()
    }

    fn err(&self) -> Vec<String> {
        self.stderr.lock().unwrap().clone()
    }

    /// The `rpc` lines, once there are `n` of them.
    fn rpc_lines(&self, n: usize) -> Vec<String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let lines: Vec<String> = self
                .out()
                .into_iter()
                .filter(|l| l.contains("\"event\":\"rpc\""))
                .collect();
            if lines.len() >= n {
                return lines;
            }
            assert!(
                Instant::now() < deadline,
                "{} of {n} rpc lines: {:?}",
                lines.len(),
                self.out()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// SIGTERM, then the exit (spec §9.2), within 10 s.
    fn stop(&mut self) {
        // SAFETY: the pid is our live child.
        unsafe { libc::kill(self.child.id() as i32, libc::SIGTERM) };
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.child.try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline, "no exit: {:?}", self.out());
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

const TOPIC: &str = "topics:\n  - dds_name: \"T::Open\"\n    grpc_service: OpenStream\n";

fn frame(s: &mut impl Write, typ: FrameType, flags: u8, stream_id: u32, payload: &[u8]) {
    let h = FrameHeader {
        length: payload.len() as u32,
        frame_type: typ,
        flags: Flags(flags),
        stream_id,
    };
    let mut buf = vec![0u8; 9 + payload.len()];
    encode_frame(&h, payload, &mut buf, 16384).unwrap();
    s.write_all(&buf).unwrap();
}

/// The client side of one HTTP/2 connection.
struct H2 {
    s: TcpStream,
    enc: Encoder,
    dec: Decoder,
}

impl H2 {
    fn open(port: u16) -> Self {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        s.write_all(PREFACE).unwrap();
        frame(&mut s, FrameType::Settings, 0, 0, &[]);
        Self {
            s,
            enc: Encoder::new(),
            dec: Decoder::new(),
        }
    }

    /// The client's address as the daemon sees it.
    fn local(&self) -> String {
        self.s.local_addr().unwrap().to_string()
    }

    /// The request HEADERS of stream `sid`: `method` on `path`, with
    /// END_STREAM when `end`.
    fn headers(&mut self, sid: u32, method: &str, path: &str, end: bool) {
        let mut fields = vec![
            HeaderField {
                name: ":method".into(),
                value: method.into(),
            },
            HeaderField {
                name: ":scheme".into(),
                value: "http".into(),
            },
            HeaderField {
                name: ":path".into(),
                value: path.into(),
            },
        ];
        if method == "POST" {
            fields.push(HeaderField {
                name: "content-type".into(),
                value: "application/grpc+proto".into(),
            });
        }
        let block = self.enc.encode(&fields);
        let flags = if end {
            Flags::END_HEADERS | Flags::END_STREAM
        } else {
            Flags::END_HEADERS
        };
        frame(&mut self.s, FrameType::Headers, flags, sid, &block);
    }

    /// The next frame: its header, its stream, its payload and, for a
    /// HEADERS frame, the decoded fields. Every header block goes through
    /// the decoder, whatever its stream: it may change the dynamic table
    /// (RFC 7541 §2.3.2).
    fn next_frame(&mut self) -> ([u8; 9], u32, Vec<u8>, Vec<HeaderField>) {
        let mut h = [0u8; 9];
        self.s.read_exact(&mut h).expect("a response frame");
        let len = (usize::from(h[0]) << 16) | (usize::from(h[1]) << 8) | usize::from(h[2]);
        let mut p = vec![0u8; len];
        self.s.read_exact(&mut p).unwrap();
        let stream = u32::from_be_bytes([h[5], h[6], h[7], h[8]]) & 0x7fff_ffff;
        let fields = if h[3] == FrameType::Headers as u8 {
            self.dec.decode(&p).unwrap()
        } else {
            Vec::new()
        };
        (h, stream, p, fields)
    }

    /// A GET of `path` on stream `sid` (an HTTP/2 request outside gRPC,
    /// spec §5.2): the `:status` and the body.
    fn get(&mut self, sid: u32, path: &str) -> (String, Vec<u8>) {
        self.headers(sid, "GET", path, true);
        let (mut status, mut body) = (String::new(), Vec::new());
        loop {
            let (h, stream, p, fields) = self.next_frame();
            if stream != sid {
                continue;
            }
            for f in fields {
                if f.name == ":status" {
                    status = f.value;
                }
            }
            if h[3] == FrameType::Data as u8 {
                body.extend_from_slice(&p);
            }
            if h[4] & Flags::END_STREAM != 0 {
                return (status, body);
            }
        }
    }

    /// One `Publish` on `service` on stream `sid` with one `Sample` and
    /// the metadata `extra`: the `grpc-status` and the DATA octets of the
    /// answer (the Length-Prefixed-Messages with their prefix).
    fn publish(&mut self, sid: u32, service: &str, extra: &[(&str, &str)]) -> (u32, Vec<u8>) {
        let mut headers = vec![
            (":method", "POST".to_string()),
            (":scheme", "http".to_string()),
            (":path", format!("/zerodds.t.v1.{service}/Publish")),
            ("content-type", "application/grpc+proto".to_string()),
        ];
        headers.extend(extra.iter().map(|(n, v)| (*n, (*v).to_string())));
        let fields: Vec<HeaderField> = headers
            .into_iter()
            .map(|(n, v)| HeaderField {
                name: n.into(),
                value: v,
            })
            .collect();
        let block = self.enc.encode(&fields);
        frame(
            &mut self.s,
            FrameType::Headers,
            Flags::END_HEADERS,
            sid,
            &block,
        );
        // Sample { cdr_payload = 00 01 00 00 "hi" } (spec §4.2, §4.4): 13
        // octets with the prefix.
        frame(
            &mut self.s,
            FrameType::Data,
            Flags::END_STREAM,
            sid,
            &[0, 0, 0, 0, 8, 0x0a, 6, 0, 1, 0, 0, b'h', b'i'],
        );
        let mut data = Vec::new();
        loop {
            let (h, stream, p, fields) = self.next_frame();
            if stream != sid {
                continue;
            }
            if h[3] == FrameType::Data as u8 {
                data.extend_from_slice(&p);
            }
            if let Some(st) = fields.iter().find(|f| f.name == "grpc-status") {
                return (st.value.parse().unwrap(), data);
            }
        }
    }
}

/// The digits of a number field of a JSON line.
fn number(line: &str, key: &str) -> u64 {
    let start = line
        .find(&format!("\"{key}\":"))
        .unwrap_or_else(|| panic!("{key} in {line}"))
        + key.len()
        + 3;
    line[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or_else(|_| panic!("{key} is a number in {line}"))
}

/// Spec §8.1: every line on stdout is a JSON object that begins with
/// `timestamp` (RFC 3339, UTC, milliseconds), `level` and `event`.
fn assert_json_line(line: &str) {
    let ts = line
        .strip_prefix("{\"timestamp\":\"")
        .unwrap_or_else(|| panic!("no timestamp first: {line}"));
    let b = ts.as_bytes();
    let shape = b.len() > 25
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && b[19] == b'.'
        && &b[23..25] == b"Z\""
        && [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18, 20, 21, 22]
            .iter()
            .all(|i| b[*i].is_ascii_digit());
    assert!(shape, "timestamp is not RFC 3339 UTC with ms: {line}");
    assert!(
        ts[24..].starts_with("\",\"level\":\""),
        "level after timestamp: {line}"
    );
    assert!(line.contains("\",\"event\":\""), "{line}");
    assert!(line.ends_with('}'), "{line}");
}

/// Spec §8.1: a call is one `rpc` line on stdout with `peer`, `service`,
/// `method`, `dds_topic`, `bytes`, `grpc_status`, `latency_us` and
/// `stream_id`; an OK call at `info`, a refused one at `warn` with its
/// `grpc_message`. Nothing of the log goes to stderr.
#[test]
fn grpcbridge_8_1_every_call_is_one_line_on_stdout_with_the_fields_of_the_spec() {
    let d = Daemon::start("log", &format!("log_level: info\n{TOPIC}"), &[], &[]);
    let mut c = H2::open(d.port);
    let peer = c.local();
    let (status, answer) = c.publish(1, "OpenStream", &[]);
    assert_eq!(status, 0);
    assert!(!answer.is_empty(), "a PublishAck");
    assert_eq!(c.publish(3, "NoSuchStream", &[]).0, 5);
    let lines = d.rpc_lines(2);
    assert_eq!(lines.len(), 2, "{lines:?}");
    for line in d.out() {
        assert_json_line(&line);
    }
    let ok = &lines[0];
    for field in [
        "\"level\":\"info\",\"event\":\"rpc\"".to_string(),
        format!("\"peer\":\"{peer}\""),
        "\"service\":\"zerodds.t.v1.OpenStream\"".to_string(),
        "\"method\":\"Publish\"".to_string(),
        "\"dds_topic\":\"T::Open\"".to_string(),
        "\"grpc_status\":0,".to_string(),
        "\"stream_id\":1".to_string(),
    ] {
        assert!(ok.contains(&field), "{field} in {ok}");
    }
    // The request message (13 octets) and the PublishAck, each with its
    // prefix.
    assert_eq!(number(ok, "bytes"), 13 + answer.len() as u64, "{ok}");
    assert!(number(ok, "latency_us") < 5_000_000, "{ok}");
    assert!(!ok.contains("grpc_message"), "{ok}");
    let refused = &lines[1];
    for field in [
        "\"level\":\"warn\",\"event\":\"rpc\"",
        "\"service\":\"zerodds.t.v1.NoSuchStream\"",
        "\"dds_topic\":\"\"",
        "\"grpc_status\":5,",
        "\"stream_id\":3",
        "\"grpc_message\":\"",
    ] {
        assert!(refused.contains(field), "{field} in {refused}");
    }
    assert!(
        !d.err().iter().any(|l| l.contains("\"event\"")),
        "log lines on stderr: {:?}",
        d.err()
    );
}

/// Spec §2, §3, §8.1: `--log-level warn` wins over the file's `log_level:
/// debug`; an OK call (`info`) is not logged, a refused one (`warn`) is.
#[test]
fn grpcbridge_8_1_log_level_warn_hides_ok_calls_and_the_flag_wins_over_the_file() {
    // The listener is up once a connection is served: no `listening` line
    // under `warn`.
    let dir_config = format!("log_level: debug\n{TOPIC}");
    let d = Daemon::start_quiet("warn", &dir_config, &["--log-level", "warn"]);
    let mut c = H2::open(d.port);
    assert_eq!(c.publish(1, "OpenStream", &[]).0, 0);
    assert_eq!(c.publish(3, "NoSuchStream", &[]).0, 5);
    let lines = d.rpc_lines(1);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("\"stream_id\":3"), "{lines:?}");
    let out = d.out();
    for line in &out {
        assert_json_line(line);
        assert!(
            line.contains("\"level\":\"warn\"") || line.contains("\"level\":\"error\""),
            "below warn: {line}"
        );
    }
    assert!(
        !out.iter().any(|l| l.contains("\"grpc_status\":0,")),
        "{out:?}"
    );
}

impl Daemon {
    /// [`Self::start`] for a daemon that writes no `listening` line: ready
    /// once a TCP connection is accepted and served.
    fn start_quiet(name: &str, config: &str, args: &[&str]) -> Self {
        let dir = temp_dir(name);
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
        let mut child = Command::new(env!("CARGO_BIN_EXE_zerodds-grpc-bridged"))
            .args(["--config", path.to_str().unwrap()])
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = collect(child.stdout.take().unwrap());
        let stderr = collect(child.stderr.take().unwrap());
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            // A connection with the preface and SETTINGS: the daemon answers
            // with its SETTINGS once it serves.
            if let Ok(mut s) = TcpStream::connect(("127.0.0.1", port)) {
                s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                let _ = s.write_all(PREFACE);
                let _ = s.write_all(&[0, 0, 0, FrameType::Settings as u8, 0, 0, 0, 0, 0]);
                let mut h = [0u8; 9];
                if s.read_exact(&mut h).is_ok() && h[3] == FrameType::Settings as u8 {
                    break;
                }
            }
            assert!(Instant::now() < deadline, "no listener");
            std::thread::sleep(Duration::from_millis(50));
        }
        Self {
            child,
            port,
            stdout,
            stderr,
            _domain: domain,
        }
    }
}

/// The path and the body of each request an endpoint received.
type Received = Arc<Mutex<Vec<(String, String)>>>;

/// An OTLP/HTTP endpoint of the test: every request's path and body.
fn collector() -> (u16, Received) {
    collector_on("127.0.0.1:0")
}

/// [`collector`] on the listener address `bind`.
fn collector_on(bind: &str) -> (u16, Received) {
    let listener = TcpListener::bind(bind).unwrap();
    let port = listener.local_addr().unwrap().port();
    let received = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&received);
    std::thread::spawn(move || {
        for mut s in listener.incoming().map_while(Result::ok) {
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut req = Vec::new();
            let mut buf = [0u8; 4096];
            let body_at = loop {
                let n = s.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break None;
                }
                req.extend_from_slice(&buf[..n]);
                if let Some(i) = req.windows(4).position(|w| w == b"\r\n\r\n") {
                    break Some(i + 4);
                }
            };
            let Some(body_at) = body_at else { continue };
            let head = String::from_utf8_lossy(&req[..body_at]).to_string();
            let length: usize = head
                .lines()
                .find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.eq_ignore_ascii_case("content-length")
                        .then(|| v.trim().parse().ok())?
                })
                .unwrap_or(0);
            while req.len() < body_at + length {
                let n = s.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break;
                }
                req.extend_from_slice(&buf[..n]);
            }
            let path = head.split(' ').nth(1).unwrap_or("").to_string();
            let body = String::from_utf8_lossy(&req[body_at..]).to_string();
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
            sink.lock().unwrap().push((path, body));
        }
    });
    (port, received)
}

/// Spec §8.3: with `OTEL_EXPORTER_OTLP_ENDPOINT` the daemon posts the spans
/// of its calls to `/v1/traces`, at the latest when it stops (§9.2). A call
/// with a sampled `traceparent` is in the caller's trace, its server span a
/// child of the caller's span (W3C Trace Context §3.2); a call without one
/// starts a trace; a call whose caller did not sample it records nothing
/// (§3.2.2.5.1). The `dds.publish` child hangs below the server span.
#[test]
fn grpcbridge_8_3_the_daemon_posts_the_spans_of_a_call_to_the_otlp_endpoint() {
    let (cport, received) = collector();
    let mut d = Daemon::start(
        "otlp",
        TOPIC,
        &[],
        &[(
            "OTEL_EXPORTER_OTLP_ENDPOINT",
            format!("http://127.0.0.1:{cport}"),
        )],
    );
    let mut c = H2::open(d.port);
    assert_eq!(
        c.publish(1, "OpenStream", &[("traceparent", TRACEPARENT)])
            .0,
        0
    );
    assert_eq!(c.publish(3, "OpenStream", &[]).0, 0);
    assert_eq!(
        c.publish(5, "OpenStream", &[("traceparent", UNSAMPLED)]).0,
        0
    );
    drop(c);
    d.rpc_lines(3);
    d.stop();
    let deadline = Instant::now() + Duration::from_secs(5);
    let text = loop {
        let got = received.lock().unwrap().clone();
        let text: String = got.iter().map(|(_, body)| body.as_str()).collect();
        if text
            .matches("\"name\":\"zerodds.t.v1.OpenStream/Publish\"")
            .count()
            >= 2
        {
            assert!(got.iter().all(|(path, _)| path == "/v1/traces"), "{got:?}");
            break text;
        }
        assert!(
            Instant::now() < deadline,
            "the spans did not reach the endpoint: {got:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(
        text.matches("\"name\":\"zerodds.t.v1.OpenStream/Publish\"")
            .count(),
        2,
        "{text}"
    );
    assert_eq!(
        text.matches("\"name\":\"dds.publish\"").count(),
        2,
        "{text}"
    );
    assert!(
        text.contains("\"traceId\":\"4bf92f3577b34da6a3ce929d0e0e4736\""),
        "{text}"
    );
    assert!(
        text.contains("\"parentSpanId\":\"00f067aa0ba902b7\""),
        "{text}"
    );
    assert_eq!(
        text.matches("\"traceId\":\"4bf92f3577b34da6a3ce929d0e0e4736\"")
            .count(),
        2,
        "the server span and its dds.publish child: {text}"
    );
    assert!(!text.contains("11111111111111111111111111111111"), "{text}");
    assert!(text.contains("\"key\":\"rpc.system\""), "{text}");
    assert!(text.contains("\"key\":\"dds.topic\""), "{text}");
    // The OpenTelemetry semantic conventions define the status code and the
    // peer port as int: an `AnyValue` with `intValue`.
    assert_eq!(
        text.matches(r#"{"key":"rpc.grpc.status_code","value":{"intValue":"0"}}"#)
            .count(),
        2,
        "{text}"
    );
    assert_eq!(
        text.matches(r#"{"key":"network.peer.port","value":{"intValue":""#)
            .count(),
        2,
        "{text}"
    );
    assert_eq!(
        text.matches(r#"{"key":"dds.samples","value":{"intValue":"1"}}"#)
            .count(),
        2,
        "{text}"
    );
    assert!(!text.contains(r#""rpc.grpc.status_code","value":{"stringValue""#));
}

/// Spec §8.3: `host` of `OTEL_EXPORTER_OTLP_ENDPOINT` may be an IPv6
/// address in brackets: with `http://[::1]:<port>` the span of a call
/// reaches the endpoint listening on the IPv6 loopback when the daemon
/// stops, and no export fails.
#[test]
fn grpcbridge_8_3_an_ipv6_endpoint_receives_the_spans() {
    let (cport, received) = collector_on("[::1]:0");
    let mut d = Daemon::start(
        "otlp-ipv6",
        TOPIC,
        &[],
        &[(
            "OTEL_EXPORTER_OTLP_ENDPOINT",
            format!("http://[::1]:{cport}"),
        )],
    );
    let mut c = H2::open(d.port);
    assert_eq!(c.publish(1, "OpenStream", &[]).0, 0);
    drop(c);
    d.rpc_lines(1);
    d.stop();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let got = received.lock().unwrap().clone();
        let text: String = got.iter().map(|(_, body)| body.as_str()).collect();
        if text.contains("\"name\":\"zerodds.t.v1.OpenStream/Publish\"") {
            assert!(got.iter().all(|(path, _)| path == "/v1/traces"), "{got:?}");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the span did not reach the IPv6 endpoint: {got:?} {:?}",
            d.out()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !d.out().iter().any(|l| l.contains("otlp_export_failed")),
        "{:?}",
        d.out()
    );
}

/// One GET of `path` over HTTP/1.1 on a new connection: the client's
/// address and the whole response.
fn http1_get(port: u16, path: &str) -> (String, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let peer = s.local_addr().unwrap().to_string();
    s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes())
        .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    (peer, out)
}

/// The Prometheus text of the admin endpoint on `port`.
impl Daemon {
    /// The first line of `event` that contains each of `fields`, once
    /// there is one.
    fn line_with(&self, event: &str, fields: &[&str]) -> String {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(line) = self.out().into_iter().find(|l| {
                l.contains(&format!("\"event\":\"{event}\""))
                    && fields.iter().all(|f| l.contains(f))
            }) {
                return line;
            }
            assert!(
                Instant::now() < deadline,
                "no {event} line with {fields:?}: {:?}",
                self.out()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// Spec §8.1 `grpc_status`: a call the client reset with RST_STREAM, and a
/// `Subscribe` whose connection closed before its status, each end
/// CANCELLED (1): one `rpc` line at `warn` with its `grpc_message`, counted
/// in `zerodds_grpc_bridge_rpcs_total` with `status="CANCELLED"`.
#[test]
fn grpcbridge_8_1_a_reset_call_and_a_closed_connection_are_cancelled() {
    let admin = pick_port();
    let d = Daemon::start(
        "cancel",
        &format!("{TOPIC}metrics:\n  enabled: true\n  listen: \"127.0.0.1:{admin}\"\n"),
        &[],
        &[],
    );
    // A Publish opened and reset with RST_STREAM(CANCEL), then a call behind
    // it on the same connection: its answer means the reset has been read.
    let mut c = H2::open(d.port);
    let peer = c.local();
    c.headers(1, "POST", "/zerodds.t.v1.OpenStream/Publish", false);
    frame(&mut c.s, FrameType::RstStream, 0, 1, &8u32.to_be_bytes());
    assert_eq!(c.publish(3, "NoSuchStream", &[]).0, 5);
    let reset = d.line_with("rpc", &["\"method\":\"Publish\"", "\"stream_id\":1"]);
    assert_json_line(&reset);
    for field in [
        "\"level\":\"warn\",\"event\":\"rpc\"".to_string(),
        format!("\"peer\":\"{peer}\""),
        "\"service\":\"zerodds.t.v1.OpenStream\"".to_string(),
        "\"dds_topic\":\"T::Open\"".to_string(),
        "\"grpc_status\":1,".to_string(),
        "RST_STREAM".to_string(),
    ] {
        assert!(reset.contains(&field), "{field} in {reset}");
    }
    // A Subscribe stream open, a call behind it answered, then the client
    // closes the connection without a reset.
    let mut s = H2::open(d.port);
    let sub_peer = s.local();
    s.headers(1, "POST", "/zerodds.t.v1.OpenStream/Subscribe", false);
    frame(
        &mut s.s,
        FrameType::Data,
        Flags::END_STREAM,
        1,
        &[0, 0, 0, 0, 0],
    );
    assert_eq!(s.publish(3, "NoSuchStream", &[]).0, 5);
    s.s.shutdown(std::net::Shutdown::Both).unwrap();
    drop(s);
    let closed = d.line_with("rpc", &["\"method\":\"Subscribe\""]);
    assert_json_line(&closed);
    for field in [
        "\"level\":\"warn\",\"event\":\"rpc\"".to_string(),
        format!("\"peer\":\"{sub_peer}\""),
        "\"dds_topic\":\"T::Open\"".to_string(),
        "\"grpc_status\":1,".to_string(),
        "\"stream_id\":1".to_string(),
        "\"grpc_message\":\"the connection closed\"".to_string(),
    ] {
        assert!(closed.contains(&field), "{field} in {closed}");
    }
    // The counter is raised before the line is written.
    let body = scrape(admin);
    for (series, value) in [
        (
            r#"zerodds_grpc_bridge_rpcs_total{method="Publish",service="OpenStream",status="CANCELLED"}"#,
            1.0,
        ),
        (
            r#"zerodds_grpc_bridge_rpcs_total{method="Subscribe",service="OpenStream",status="CANCELLED"}"#,
            1.0,
        ),
        (
            r#"zerodds_grpc_bridge_rpcs_total{method="Publish",service="unknown",status="NOT_FOUND"}"#,
            2.0,
        ),
    ] {
        assert_eq!(metric(&body, series), Some(value), "{series}\n{body}");
    }
}

/// Spec §8.1: a request to the IDL endpoint (§5.2) is one `http` line with
/// `peer`, `protocol` (`http/1.1` or `h2`), `method`, `path`,
/// `http_status`, `bytes` (the body) and `latency_us`; `info` for a 2xx
/// answer, `warn` for another.
#[test]
fn grpcbridge_8_1_a_request_to_the_idl_endpoint_is_an_http_line() {
    let dir = temp_dir("idl");
    let idl = "module T {\n  struct Open { @key long id; };\n};\n";
    let file = dir.join("t.idl");
    std::fs::write(&file, idl).unwrap();
    let d = Daemon::start(
        "http",
        &format!(
            "topics:\n  - dds_name: \"T::Open\"\n    dds_type: \"T::Open\"\n    grpc_service: OpenStream\n    idl_file: \"{}\"\n",
            file.display()
        ),
        &[],
        &[],
    );
    // HTTP/1.1 on the cleartext listener.
    let (peer, response) = http1_get(d.port, "/idl/T::Open");
    assert!(response.starts_with("HTTP/1.1 200 "), "{response}");
    let line = d.line_with("http", &[format!("\"peer\":\"{peer}\"").as_str()]);
    assert_json_line(&line);
    for field in [
        "\"level\":\"info\",\"event\":\"http\"".to_string(),
        "\"protocol\":\"http/1.1\"".to_string(),
        "\"method\":\"GET\"".to_string(),
        "\"path\":\"/idl/T::Open\"".to_string(),
        "\"http_status\":200,".to_string(),
        format!("\"bytes\":{},", idl.len()),
    ] {
        assert!(line.contains(&field), "{field} in {line}");
    }
    assert!(number(&line, "latency_us") < 5_000_000, "{line}");
    let (peer404, response) = http1_get(d.port, "/idl/Nothing");
    assert!(response.starts_with("HTTP/1.1 404 "), "{response}");
    let line = d.line_with("http", &[format!("\"peer\":\"{peer404}\"").as_str()]);
    for field in [
        "\"level\":\"warn\",\"event\":\"http\"",
        "\"path\":\"/idl/Nothing\"",
        "\"http_status\":404,",
    ] {
        assert!(line.contains(field), "{field} in {line}");
    }
    // An HTTP/2 GET on a gRPC connection.
    let mut c = H2::open(d.port);
    let h2_peer = c.local();
    let (status, body) = c.get(1, "/idl/T::Open");
    assert_eq!(status, "200");
    assert_eq!(body, idl.as_bytes());
    let line = d.line_with("http", &[format!("\"peer\":\"{h2_peer}\"").as_str()]);
    assert_json_line(&line);
    for field in [
        "\"level\":\"info\",\"event\":\"http\"".to_string(),
        "\"protocol\":\"h2\"".to_string(),
        "\"method\":\"GET\"".to_string(),
        "\"path\":\"/idl/T::Open\"".to_string(),
        "\"http_status\":200,".to_string(),
        format!("\"bytes\":{},", idl.len()),
    ] {
        assert!(line.contains(&field), "{field} in {line}");
    }
    assert!(number(&line, "latency_us") < 5_000_000, "{line}");
    // A request to the IDL endpoint is no call: no `rpc` line.
    assert!(
        !d.out().iter().any(|l| l.contains("\"event\":\"rpc\"")),
        "{:?}",
        d.out()
    );
}

/// The exit code and stderr of a daemon that ends before it serves.
fn start_fails(name: &str, args: &[&str], env: &[(&str, &str)]) -> (Option<i32>, String, String) {
    let dir = temp_dir(name);
    let domain = zerodds_test_domains::lease_domain();
    let path = dir.join("grpc-bridged.yaml");
    std::fs::write(
        &path,
        format!(
            "domain: {}\ngrpc:\n  bind: \"127.0.0.1:{}\"\n{TOPIC}",
            domain.id(),
            pick_port()
        ),
    )
    .unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_zerodds-grpc-bridged"));
    command
        .args(["--config", path.to_str().unwrap()])
        .args(args)
        .env_remove("OTEL_EXPORTER_OTLP_ENDPOINT");
    for (k, v) in env {
        command.env(k, v);
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            panic!("the daemon started");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let out = child.wait_with_output().unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

/// Spec §8.3, §2: an `OTEL_EXPORTER_OTLP_ENDPOINT` the exporter cannot
/// serve stops the start with exit 1, the value named on stderr: `https`
/// (the exporter speaks no TLS), a path (it posts to `/v1/traces` at the
/// root), a port that is no number. `--check-config` refuses it too.
#[test]
fn grpcbridge_8_3_an_otlp_endpoint_the_exporter_cannot_serve_stops_the_start() {
    for value in [
        "https://127.0.0.1:4318",
        "http://127.0.0.1:4318/otlp",
        "http://127.0.0.1:43x8",
    ] {
        let (code, _, stderr) = start_fails(
            "otlp-refused",
            &[],
            &[("OTEL_EXPORTER_OTLP_ENDPOINT", value)],
        );
        assert_eq!(code, Some(1), "{value}: {stderr}");
        assert!(
            stderr.contains(&format!("OTEL_EXPORTER_OTLP_ENDPOINT \"{value}\"")),
            "{value}: {stderr}"
        );
    }
    let (code, stdout, _) = start_fails(
        "otlp-check",
        &["--check-config"],
        &[("OTEL_EXPORTER_OTLP_ENDPOINT", "https://127.0.0.1:4318")],
    );
    assert_eq!(code, Some(1), "{stdout}");
    assert!(stdout.contains("configuration invalid"), "{stdout}");
    assert!(stdout.contains("no TLS"), "{stdout}");
}

/// Spec §8.3: without `OTEL_EXPORTER_OTLP_ENDPOINT` nothing is recorded
/// or sent: a call and the stop write no `otlp` line (a failed export
/// would) and send no request to 127.0.0.1:4318, the default of an
/// OTLP/HTTP exporter, where a listener of the test waits. Where another
/// process holds that port (a collector of the host; the CI jobs and the
/// test host run in a network namespace of their own, where it is free),
/// the connection is not looked for and the test says so on stderr.
#[test]
fn grpcbridge_8_3_without_an_endpoint_nothing_is_recorded() {
    let listener = match TcpListener::bind("127.0.0.1:4318") {
        Ok(l) => Some(l),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            eprintln!(
                "127.0.0.1:4318 is held by another process: no connection to it is looked for"
            );
            None
        }
        Err(e) => panic!("127.0.0.1:4318: {e}"),
    };
    if let Some(l) = &listener {
        l.set_nonblocking(true).unwrap();
    }
    let mut d = Daemon::start("no-otlp", TOPIC, &[], &[]);
    let mut c = H2::open(d.port);
    assert_eq!(
        c.publish(1, "OpenStream", &[("traceparent", TRACEPARENT)])
            .0,
        0
    );
    drop(c);
    d.rpc_lines(1);
    // The stop is when the last spans would go out (§9.2).
    d.stop();
    assert_eq!(d.child.try_wait().unwrap().and_then(|s| s.code()), Some(0));
    if let Some(l) = &listener {
        match l.accept() {
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            other => panic!("a connection to the OTLP default port: {other:?}"),
        }
    }
    assert!(!d.out().iter().any(|l| l.contains("otlp")), "{:?}", d.out());
}

/// Spec §8.3: the spans of a failed export are lost, and the failure is a
/// `warn` line `otlp_export_failed` with `err` and `failures`; the export
/// at the end of the shutdown writes its line in any case.
#[test]
fn grpcbridge_8_3_a_failed_export_is_a_warn_line() {
    // A port nothing listens on: the export is refused.
    let closed = pick_port();
    let mut d = Daemon::start(
        "otlp-down",
        TOPIC,
        &[],
        &[(
            "OTEL_EXPORTER_OTLP_ENDPOINT",
            format!("http://127.0.0.1:{closed}"),
        )],
    );
    let mut c = H2::open(d.port);
    assert_eq!(c.publish(1, "OpenStream", &[]).0, 0);
    drop(c);
    d.rpc_lines(1);
    d.stop();
    assert_eq!(d.child.try_wait().unwrap().and_then(|s| s.code()), Some(0));
    let line = d.line_with("otlp_export_failed", &[]);
    assert_json_line(&line);
    assert!(line.contains("\"level\":\"warn\""), "{line}");
    assert!(line.contains("\"err\":\"io: "), "{line}");
    assert!(number(&line, "failures") >= 1, "{line}");
}
