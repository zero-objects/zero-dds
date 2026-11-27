// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-grpc-bridged` §7 on the wire: the binary started with a
//! config file, a client in this file speaking HTTP/2 over TCP or TLS.
//!
//! The daemon used to read `grpc.tls` and `auth.mode` and then serve
//! plaintext without checking anything. Covered here: bearer and JWT in
//! the `authorization` metadata, the topic ACL, TLS with ALPN `h2`, mTLS,
//! and the SIGHUP reload of §9.2.

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
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[path = "support/metrics.rs"]
mod metrics;
#[path = "support/writer_match.rs"]
mod writer_match;

use metrics::{metric, scrape};
use zerodds_hpack::{Decoder, Encoder, HeaderField};
use zerodds_http2::{Flags, FrameHeader, FrameType, encode_frame};

const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

fn temp_dir(name: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "zd-grpcd-sec-{name}-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Daemon {
    child: Child,
    port: u16,
    /// The port of `grpc.bind` in the file, kept by a rewrite: 0, or the
    /// port of [`Daemon::start_quiet`].
    file_port: u16,
    log: PathBuf,
    /// The daemon's DDS domain, leased: the daemons of the tests run side
    /// by side, and on one domain each reader of `T::Open` received what
    /// every other test's daemon wrote (measured: ten Subscribe messages
    /// for the one sample this test's Publish wrote).
    domain: zerodds_test_domains::DomainLease,
}

/// `config` with the leased DDS domain in front and `port` for `PORT`: 0,
/// and the daemon binds a port the system picks and names it in its
/// `listening` line. A rewrite for a reload keeps the port, the
/// `grpc.bind` the daemon started with.
fn with_domain(config: &str, domain: i32, port: u16) -> String {
    format!(
        "domain: {domain}\n{}",
        config.replace("PORT", &port.to_string())
    )
}

impl Daemon {
    /// Starts the binary on `config` and waits for the listener.
    fn start(dir: &Path, config: &str) -> Self {
        Self::start_with(dir, config, &[])
    }

    /// [`Self::start`] with further command-line arguments.
    fn start_with(dir: &Path, config: &str, args: &[&str]) -> Self {
        let mut d = Self::spawn(dir, config, args, 0);
        d.port = d.logged_port("listening");
        d
    }

    /// [`Self::start_with`] for a log level that hides the `listening`
    /// line: on a port of [`zerodds_test_domains::free_tcp_port`], ready
    /// once a TCP connection is accepted.
    fn start_quiet(dir: &Path, config: &str, args: &[&str]) -> Self {
        let port = zerodds_test_domains::free_tcp_port();
        let mut d = Self::spawn(dir, config, args, port);
        d.port = port;
        let deadline = Instant::now() + Duration::from_secs(10);
        while TcpStream::connect(("127.0.0.1", port)).is_err() {
            assert!(Instant::now() < deadline, "no listener: {}", d.log_text());
            std::thread::sleep(Duration::from_millis(50));
        }
        d
    }

    /// The binary on `config` with `file_port` for `PORT`, on a leased
    /// domain; `port` is still to be set.
    fn spawn(dir: &Path, config: &str, args: &[&str], file_port: u16) -> Self {
        let domain = zerodds_test_domains::lease_domain();
        let path = dir.join("grpc-bridged.yaml");
        std::fs::write(&path, with_domain(config, domain.id(), file_port)).unwrap();
        let log = dir.join("daemon.log");
        let out = std::fs::File::create(&log).unwrap();
        let child = Command::new(env!("CARGO_BIN_EXE_zerodds-grpc-bridged"))
            .args(["--config", path.to_str().unwrap()])
            .args(args)
            // Spec §8.1: the JSON log is on stdout; the exit error on
            // stderr goes to the same file (one offset, as `>log 2>&1`).
            .stdout(out.try_clone().unwrap())
            .stderr(out)
            .spawn()
            .unwrap();
        Self {
            child,
            port: 0,
            file_port,
            log,
            domain,
        }
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

    fn log_text(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    fn rewrite(&self, dir: &Path, config: &str) {
        std::fs::write(
            dir.join("grpc-bridged.yaml"),
            with_domain(config, self.domain.id(), self.file_port),
        )
        .unwrap();
    }

    /// SIGHUP, then the log line that says the reload ran.
    fn sighup(&self, expect: &str) {
        let before = self.log_text().matches(expect).count();
        // SAFETY: the pid is our live child.
        unsafe { libc::kill(self.child.id() as i32, libc::SIGHUP) };
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.log_text().matches(expect).count() == before {
            assert!(
                Instant::now() < deadline,
                "no {expect}: {}",
                self.log_text()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn tcp(&self) -> TcpStream {
        let s = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        s
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

/// The client side of an HTTP/2 connection: the stream and the HPACK
/// state, which lives as long as the connection (RFC 7541 §2.3.2).
struct H2<S> {
    s: S,
    enc: Encoder,
    dec: Decoder,
}

/// Preface and SETTINGS.
fn open<S: Read + Write>(mut s: S) -> H2<S> {
    s.write_all(PREFACE).unwrap();
    frame(&mut s, FrameType::Settings, 0, 0, &[]);
    H2 {
        s,
        enc: Encoder::new(),
        dec: Decoder::new(),
    }
}

/// One call of `method` on `service` on stream `sid`: the `grpc-status`
/// of the trailers and the DATA bytes of the reply.
fn call<S: Read + Write>(
    c: &mut H2<S>,
    sid: u32,
    service: &str,
    method: &str,
    authorization: Option<&str>,
) -> (u32, usize) {
    let (status, data) = call_body(c, sid, service, method, authorization);
    (status, data.len())
}

/// [`call`] with the DATA octets of the reply.
fn call_body<S: Read + Write>(
    c: &mut H2<S>,
    sid: u32,
    service: &str,
    method: &str,
    authorization: Option<&str>,
) -> (u32, Vec<u8>) {
    let mut headers = vec![
        (":method", "POST".to_string()),
        (":scheme", "http".to_string()),
        (":path", format!("/zerodds.t.v1.{service}/{method}")),
        ("content-type", "application/grpc+proto".to_string()),
    ];
    if let Some(a) = authorization {
        headers.push(("authorization", a.to_string()));
    }
    let fields: Vec<HeaderField> = headers
        .into_iter()
        .map(|(n, v)| HeaderField {
            name: n.into(),
            value: v,
        })
        .collect();
    let block = c.enc.encode(&fields);
    let s = &mut c.s;
    frame(s, FrameType::Headers, Flags::END_HEADERS, sid, &block);
    // Sample { cdr_payload = 00 01 00 00 "hi" } as one Length-Prefixed-
    // Message: the encapsulation header (XCDR1, little endian) and the CDR
    // bytes (spec §4.4).
    frame(
        s,
        FrameType::Data,
        Flags::END_STREAM,
        sid,
        &[0, 0, 0, 0, 8, 0x0a, 6, 0, 1, 0, 0, b'h', b'i'],
    );
    let dec = &mut c.dec;
    let mut data = Vec::new();
    loop {
        let mut h = [0u8; 9];
        s.read_exact(&mut h).expect("a response frame");
        let len = (usize::from(h[0]) << 16) | (usize::from(h[1]) << 8) | usize::from(h[2]);
        let mut p = vec![0u8; len];
        s.read_exact(&mut p).unwrap();
        if h[3] == FrameType::Data as u8 {
            data.extend_from_slice(&p);
        }
        if h[3] == FrameType::Headers as u8 {
            let fields = dec.decode(&p).unwrap();
            if let Some(st) = fields.iter().find(|f| f.name == "grpc-status") {
                return (st.value.parse().unwrap(), data);
            }
        }
    }
}

/// Opens a `Subscribe` stream on `service` (spec §4.3, a server stream) and
/// returns once the daemon answered with the response HEADERS: from then on
/// the stream's own DataReader exists.
fn subscribe_open<S: Read + Write>(
    c: &mut H2<S>,
    sid: u32,
    service: &str,
    authorization: Option<&str>,
) {
    let mut headers = vec![
        (":method", "POST".to_string()),
        (":scheme", "http".to_string()),
        (":path", format!("/zerodds.t.v1.{service}/Subscribe")),
        ("content-type", "application/grpc+proto".to_string()),
    ];
    if let Some(a) = authorization {
        headers.push(("authorization", a.to_string()));
    }
    let fields: Vec<HeaderField> = headers
        .into_iter()
        .map(|(n, v)| HeaderField {
            name: n.into(),
            value: v,
        })
        .collect();
    let block = c.enc.encode(&fields);
    frame(
        &mut c.s,
        FrameType::Headers,
        Flags::END_HEADERS | Flags::END_STREAM,
        sid,
        &block,
    );
    loop {
        let (typ, p) = read_frame(&mut c.s);
        if typ == FrameType::Headers as u8 {
            let fields = c.dec.decode(&p).unwrap();
            if let Some(st) = fields.iter().find(|f| f.name == "grpc-status") {
                panic!(
                    "Subscribe answered grpc-status {} instead of a stream",
                    st.value
                );
            }
            return;
        }
    }
}

/// The length of the first message frame of the open stream `sid`; then
/// the client cancels the stream with RST_STREAM(CANCEL).
fn first_message<S: Read + Write>(c: &mut H2<S>, sid: u32) -> usize {
    loop {
        let (typ, p) = read_frame(&mut c.s);
        if typ == FrameType::Data as u8 {
            frame(&mut c.s, FrameType::RstStream, 0, sid, &8u32.to_be_bytes());
            return p.len();
        }
        if typ == FrameType::Headers as u8 {
            let fields = c.dec.decode(&p).unwrap();
            if let Some(st) = fields.iter().find(|f| f.name == "grpc-status") {
                panic!(
                    "Subscribe ended with grpc-status {} before a message",
                    st.value
                );
            }
        }
    }
}

fn read_frame(s: &mut impl Read) -> (u8, Vec<u8>) {
    let mut h = [0u8; 9];
    s.read_exact(&mut h).expect("a response frame");
    let len = (usize::from(h[0]) << 16) | (usize::from(h[1]) << 8) | usize::from(h[2]);
    let mut p = vec![0u8; len];
    s.read_exact(&mut p).unwrap();
    (h[3], p)
}

/// One unary call `Publish` on `service`: the `grpc-status` of the
/// trailers, with the `authorization` metadata when given.
fn publish(s: impl Read + Write, service: &str, authorization: Option<&str>) -> u32 {
    call(&mut open(s), 1, service, "Publish", authorization).0
}

fn tcp_publish(d: &Daemon, service: &str, authorization: Option<&str>) -> u32 {
    publish(d.tcp(), service, authorization)
}

const OK: u32 = 0;
const NOT_FOUND: u32 = 5;
const PERMISSION_DENIED: u32 = 7;
const UNAUTHENTICATED: u32 = 16;

fn bearer_config(token: &str) -> String {
    [
        "grpc:",
        "  bind: \"127.0.0.1:PORT\"",
        "auth:",
        "  mode: bearer",
        &format!("  bearer_token: \"{token}\""),
        "  bearer_subject: alice",
        "topics:",
        "  - dds_name: \"T::Open\"",
        "    grpc_service: OpenStream",
        "    acl:",
        "      publish: [alice]",
        "      subscribe: [alice]",
        "  - dds_name: \"T::Closed\"",
        "    grpc_service: ClosedStream",
        "    acl:",
        "      publish: [bob]",
        "      subscribe: [bob]",
    ]
    .join("\n")
}

/// §7.2 bearer and §7.3 ACL: no token is UNAUTHENTICATED, a wrong one too,
/// the right one passes on the topic alice may publish and is
/// PERMISSION_DENIED on the one only bob may.
#[test]
fn grpcbridge_7_2_bearer_and_acl_are_checked_per_call() {
    let dir = temp_dir("bearer");
    let d = Daemon::start(&dir, &bearer_config("tk-1"));
    assert_eq!(tcp_publish(&d, "OpenStream", None), UNAUTHENTICATED);
    assert_eq!(
        tcp_publish(&d, "OpenStream", Some("Bearer wrong")),
        UNAUTHENTICATED
    );
    assert_eq!(tcp_publish(&d, "OpenStream", Some("Bearer tk-1")), OK);
    assert_eq!(
        tcp_publish(&d, "ClosedStream", Some("Bearer tk-1")),
        PERMISSION_DENIED
    );
}

/// §2 `--log-level`: the threshold of the lines on stdout. Under `warn`
/// the refused call is logged and the accepted connection is not; under
/// `debug` both are.
#[test]
fn grpcbridge_2_log_level_filters_the_lines() {
    for (level, accept_logged) in [("warn", false), ("debug", true)] {
        let dir = temp_dir("log-level");
        let d = Daemon::start_quiet(&dir, &bearer_config("tk-1"), &["--log-level", level]);
        assert_eq!(tcp_publish(&d, "OpenStream", None), UNAUTHENTICATED);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !d.log_text().contains("\"event\":\"auth_failed\"") {
            assert!(Instant::now() < deadline, "{level}: {}", d.log_text());
            std::thread::sleep(Duration::from_millis(20));
        }
        let log = d.log_text();
        assert_eq!(
            log.contains("\"event\":\"accept\""),
            accept_logged,
            "{level}: {log}"
        );
        assert_eq!(
            log.contains("\"event\":\"listening\""),
            accept_logged,
            "{level}: {log}"
        );
    }
}

/// §9.2: SIGHUP applies a new token; a file that does not load keeps it.
#[test]
fn grpcbridge_9_2_a_sighup_applies_the_new_token_and_a_broken_file_keeps_it() {
    let dir = temp_dir("reload");
    let d = Daemon::start(&dir, &bearer_config("tk-1"));
    assert_eq!(tcp_publish(&d, "OpenStream", Some("Bearer tk-1")), OK);
    d.rewrite(&dir, &bearer_config("tk-2"));
    d.sighup("\"event\":\"reloaded\"");
    assert_eq!(
        tcp_publish(&d, "OpenStream", Some("Bearer tk-1")),
        UNAUTHENTICATED
    );
    assert_eq!(tcp_publish(&d, "OpenStream", Some("Bearer tk-2")), OK);
    d.rewrite(&dir, "auth:\n  mode: sometimes\n");
    d.sighup("reload_refused");
    assert_eq!(tcp_publish(&d, "OpenStream", Some("Bearer tk-2")), OK);
}

/// §7.2, §9.2: SIGHUP checks `auth.mode: none` against the address the
/// listener serves. Started on 0.0.0.0 with TLS and `bearer`, a file that
/// moves `grpc.bind` to 127.0.0.1 and sets `none` is refused: a call
/// without a token is still UNAUTHENTICATED on the bound 0.0.0.0, the token
/// still passes, and nothing of the file is applied.
#[test]
fn grpcbridge_7_2_a_sighup_does_not_switch_none_on_for_the_served_public_bind() {
    let dir = temp_dir("reload-none");
    let pki = Pki::new();
    let (_, server_pem, server_key, _) = pki.leaf("localhost", &["localhost"]);
    std::fs::write(dir.join("server.pem"), &server_pem).unwrap();
    std::fs::write(dir.join("server.key"), &server_key).unwrap();
    let config = |bind: &str, auth: &str| {
        format!(
            "grpc:\n  bind: \"{bind}:PORT\"\n  tls:\n    enabled: true\n    cert_file: \"{0}/server.pem\"\n    key_file: \"{0}/server.key\"\n\
auth:\n{auth}\ntopics:\n  - dds_name: \"T::Open\"\n    grpc_service: OpenStream\n    acl:\n      publish: [alice]\n",
            dir.display()
        )
    };
    let bearer = "  mode: bearer\n  bearer_token: \"tk-p\"\n  bearer_subject: alice";
    let d = Daemon::start(&dir, &config("0.0.0.0", bearer));
    let call = |authorization: Option<&str>| {
        publish(
            &mut tls_client(&d, &pki.ca_pem, None),
            "OpenStream",
            authorization,
        )
    };
    assert_eq!(call(None), UNAUTHENTICATED);
    assert_eq!(call(Some("Bearer tk-p")), OK);
    d.rewrite(&dir, &config("127.0.0.1", "  mode: none"));
    d.sighup("reload_refused");
    let log = d.log_text();
    assert!(
        log.contains("auth.mode: none") && log.contains("grpc.bind 0.0.0.0:"),
        "{log}"
    );
    assert!(!log.contains("\"event\":\"reloaded\""), "{log}");
    assert_eq!(call(None), UNAUTHENTICATED);
    assert_eq!(call(Some("Bearer tk-p")), OK);
}

/// A setting that cannot be served stops the start instead of being
/// ignored.
#[test]
fn grpcbridge_7_2_jwt_without_a_key_and_mtls_without_tls_do_not_start() {
    for (config, expect) in [
        ("auth:\n  mode: jwt\n", "auth.jwt.public_key"),
        ("auth:\n  mode: mtls\n", "grpc.tls.client_ca_file"),
        (
            "grpc:\n  tls:\n    enabled: true\n",
            "cert_file and key_file",
        ),
    ] {
        let dir = temp_dir("refuse");
        let path = dir.join("grpc-bridged.yaml");
        std::fs::write(&path, config).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_zerodds-grpc-bridged"))
            .args(["--config", path.to_str().unwrap(), "--bind", "127.0.0.1:0"])
            .output()
            .unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "{config}: {err}");
        assert!(err.contains(expect), "{config}: {err}");
    }
}

// ---- TLS ------------------------------------------------------------------

struct Pki {
    ca_pem: String,
    ca: rcgen::Certificate,
    ca_key: rcgen::KeyPair,
}

impl Pki {
    fn new() -> Self {
        let mut p = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        p.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        p.distinguished_name
            .push(rcgen::DnType::CommonName, "zerodds test CA");
        let ca_key = rcgen::KeyPair::generate().unwrap();
        let ca = p.self_signed(&ca_key).unwrap();
        Self {
            ca_pem: ca.pem(),
            ca,
            ca_key,
        }
    }

    /// A leaf signed by the CA: (cert DER, cert PEM, key PEM, key DER).
    fn leaf(&self, cn: &str, sans: &[&str]) -> (Vec<u8>, String, String, Vec<u8>) {
        let mut p =
            rcgen::CertificateParams::new(sans.iter().map(|s| s.to_string()).collect::<Vec<_>>())
                .unwrap();
        p.distinguished_name.push(rcgen::DnType::CommonName, cn);
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = p.signed_by(&key, &self.ca, &self.ca_key).unwrap();
        (
            cert.der().to_vec(),
            cert.pem(),
            key.serialize_pem(),
            key.serialize_der(),
        )
    }
}

fn tls_client(
    d: &Daemon,
    ca_pem: &str,
    client: Option<(&[u8], &[u8])>,
) -> rustls::StreamOwned<rustls::ClientConnection, TcpStream> {
    tls_client_offering(d, ca_pem, client, vec![b"h2".to_vec()])
}

/// [`tls_client`] that offers the ALPN protocols `alpn`.
fn tls_client_offering(
    d: &Daemon,
    ca_pem: &str,
    client: Option<(&[u8], &[u8])>,
    alpn: Vec<Vec<u8>>,
) -> rustls::StreamOwned<rustls::ClientConnection, TcpStream> {
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName};
    let mut roots = rustls::RootCertStore::empty();
    let der = rustls_pemfile::certs(&mut ca_pem.as_bytes())
        .next()
        .unwrap()
        .unwrap();
    roots.add(der).unwrap();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots);
    let mut cfg = match client {
        Some((cert, key)) => builder
            .with_client_auth_cert(
                vec![CertificateDer::from(cert.to_vec())],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.to_vec())),
            )
            .unwrap(),
        None => builder.with_no_client_auth(),
    };
    cfg.alpn_protocols = alpn;
    let conn =
        rustls::ClientConnection::new(Arc::new(cfg), ServerName::try_from("localhost").unwrap())
            .unwrap();
    rustls::StreamOwned::new(conn, d.tcp())
}

/// §7.1 + §7.2 mtls: TLS with ALPN `h2`, the client certificate's subject
/// DN (`CN=alice`, RFC 4514) is the subject, and the ACL names it. A second certificate from the same CA
/// is authenticated and PERMISSION_DENIED; without a certificate the
/// handshake fails (`require_client_cert: true`).
#[test]
fn grpcbridge_7_1_mtls_serves_h2_and_the_certificate_is_the_subject() {
    let dir = temp_dir("mtls");
    let pki = Pki::new();
    let (_, server_pem, server_key, _) = pki.leaf("localhost", &["localhost"]);
    let (alice_der, _, _, alice_key) = pki.leaf("alice", &[]);
    let (bob_der, _, _, bob_key) = pki.leaf("bob", &[]);
    for (name, body) in [
        ("server.pem", &server_pem),
        ("server.key", &server_key),
        ("ca.pem", &pki.ca_pem),
    ] {
        std::fs::write(dir.join(name), body).unwrap();
    }
    let config = format!(
        "grpc:\n  bind: \"127.0.0.1:PORT\"\n  tls:\n    enabled: true\n    cert_file: \"{0}/server.pem\"\n    key_file: \"{0}/server.key\"\n    client_ca_file: \"{0}/ca.pem\"\n    require_client_cert: true\n\
auth:\n  mode: mtls\ntopics:\n  - dds_name: \"T::Open\"\n    grpc_service: OpenStream\n    acl:\n      publish: [\"CN=alice\"]\n",
        dir.display()
    );
    let d = Daemon::start(&dir, &config);

    let mut alice = tls_client(&d, &pki.ca_pem, Some((&alice_der, &alice_key)));
    assert_eq!(publish(&mut alice, "OpenStream", None), OK);
    assert_eq!(alice.conn.alpn_protocol(), Some(&b"h2"[..]));

    let mut bob = tls_client(&d, &pki.ca_pem, Some((&bob_der, &bob_key)));
    assert_eq!(publish(&mut bob, "OpenStream", None), PERMISSION_DENIED);

    let mut anonymous = tls_client(&d, &pki.ca_pem, None);
    let mut s = [0u8; 1];
    let refused = anonymous.write_all(PREFACE).is_err() || anonymous.read(&mut s).is_err();
    assert!(refused, "a client without a certificate got through");

    // Plaintext against the TLS port does not reach the dispatcher.
    let mut plain = d.tcp();
    plain.write_all(PREFACE).unwrap();
    let mut buf = [0u8; 9];
    assert!(plain.read_exact(&mut buf).is_err() || buf[3] != FrameType::Settings as u8);
}

/// §4.1, RFC 9113 §3.2: HTTP/2 over TLS is negotiated with ALPN `h2`. A
/// client that offers no ALPN and sends the HTTP/2 preface gets no
/// SETTINGS; one that offers only `http/1.1` gets the HTTP/1.1 endpoint of
/// §5.2, not HTTP/2; one that offers `h2` is served. With `grpc.tls.alpn:
/// ["h2"]` a client that offers only `http/1.1` fails the handshake.
#[test]
fn grpcbridge_4_1_tls_without_alpn_h2_gets_no_http2() {
    let (d, pki, _dir) = tls_daemon("alpn", "");
    let served = |d: &Daemon, alpn: Vec<Vec<u8>>| {
        let mut c = tls_client_offering(d, &pki.ca_pem, None, alpn);
        if c.write_all(PREFACE).is_err() {
            return None;
        }
        // An empty SETTINGS; the write fails when the server has closed.
        if c.write_all(&[0, 0, 0, 4, 0, 0, 0, 0, 0]).is_err() {
            return None;
        }
        let mut h = [0u8; 9];
        c.read_exact(&mut h).ok()?;
        Some(h)
    };
    let settings = |h: Option<[u8; 9]>| h.is_some_and(|h| h[3] == FrameType::Settings as u8);
    assert!(!settings(served(&d, Vec::new())), "no ALPN");
    // http/1.1 only: the preface is an HTTP/1.1 request of the method PRI.
    let mut c = tls_client_offering(&d, &pki.ca_pem, None, vec![b"http/1.1".to_vec()]);
    c.write_all(PREFACE).unwrap();
    let mut out = Vec::new();
    let _ = c.read_to_end(&mut out);
    let text = String::from_utf8_lossy(&out);
    assert!(text.starts_with("HTTP/1.1 405 "), "http/1.1 only: {text}");
    assert!(
        settings(served(&d, vec![b"http/1.1".to_vec(), b"h2".to_vec()])),
        "h2"
    );
    assert!(
        d.log_text().contains("tls_without_alpn_h2"),
        "{}",
        d.log_text()
    );
    drop(d);
    let (d, pki, _dir) = tls_daemon("alpn-h2", "    alpn: [\"h2\"]\n");
    let mut c = tls_client_offering(&d, &pki.ca_pem, None, vec![b"http/1.1".to_vec()]);
    let mut s = [0u8; 1];
    let refused = c.write_all(b"GET /idl/x HTTP/1.1\r\n\r\n").is_err() || c.read(&mut s).is_err();
    assert!(
        refused,
        "an http/1.1 client got through a listener of h2 alone"
    );
}

/// §5.2 over TLS with HTTP/1.1: a client that negotiates ALPN `http/1.1`
/// (curl --http1.1, a script's HTTP library), or none, gets `GET
/// /idl/<dds_name>` answered as on the cleartext listener, under the
/// authentication of §7.2.
#[test]
fn grpcbridge_5_2_idl_over_tls_with_http_1_1() {
    let dir = temp_dir("idl-tls-h1");
    let idl = "module Chat { struct Message { string text; }; };\n";
    std::fs::write(dir.join("chat.idl"), idl).unwrap();
    let pki = Pki::new();
    let (_, server_pem, server_key, _) = pki.leaf("localhost", &["localhost"]);
    std::fs::write(dir.join("server.pem"), &server_pem).unwrap();
    std::fs::write(dir.join("server.key"), &server_key).unwrap();
    let config = format!(
        "grpc:\n  bind: \"127.0.0.1:PORT\"\n  tls:\n    enabled: true\n    cert_file: \"{0}/server.pem\"\n    key_file: \"{0}/server.key\"\n\
auth:\n  mode: bearer\n  bearer_token: tk\n\
topics:\n  - dds_name: \"Chat::Message\"\n    idl_file: \"{0}/chat.idl\"\n",
        dir.display()
    );
    let d = Daemon::start(&dir, &config);
    let get = |alpn: Vec<Vec<u8>>, authorization: &str| {
        let mut c = tls_client_offering(&d, &pki.ca_pem, None, alpn);
        c.write_all(
            format!("GET /idl/Chat::Message HTTP/1.1\r\nHost: localhost\r\n{authorization}\r\n")
                .as_bytes(),
        )
        .unwrap();
        let mut out = Vec::new();
        let _ = c.read_to_end(&mut out);
        (
            c.conn.alpn_protocol().map(<[u8]>::to_vec),
            String::from_utf8_lossy(&out).into_owned(),
        )
    };
    let (alpn, r) = get(vec![b"http/1.1".to_vec()], "Authorization: Bearer tk\r\n");
    assert_eq!(alpn.as_deref(), Some(&b"http/1.1"[..]));
    assert!(r.starts_with("HTTP/1.1 200 OK\r\n"), "{r}");
    assert!(r.contains("content-type: text/x-omg-idl\r\n"), "{r}");
    assert!(r.ends_with(idl), "{r}");
    let (alpn, r) = get(Vec::new(), "Authorization: Bearer tk\r\n");
    assert_eq!(alpn, None);
    assert!(r.starts_with("HTTP/1.1 200 OK\r\n"), "no ALPN: {r}");
    let (_, r) = get(vec![b"http/1.1".to_vec()], "");
    assert!(r.starts_with("HTTP/1.1 401 "), "without the token: {r}");
}

/// The protocol version and cipher suite a TLS handshake with the daemon
/// settles on, for a client that offers `versions` and `suites` (IANA names
/// as rustls spells them, `TLS13_…` for TLS 1.3); `None` when the handshake
/// fails.
fn negotiated(
    d: &Daemon,
    ca_pem: &str,
    versions: &[&'static rustls::SupportedProtocolVersion],
    suites: &[&str],
) -> Option<(rustls::ProtocolVersion, String)> {
    use rustls::pki_types::ServerName;
    let mut roots = rustls::RootCertStore::empty();
    let der = rustls_pemfile::certs(&mut ca_pem.as_bytes())
        .next()
        .unwrap()
        .unwrap();
    roots.add(der).unwrap();
    let mut provider = rustls::crypto::ring::default_provider();
    if !suites.is_empty() {
        provider
            .cipher_suites
            .retain(|s| suites.contains(&format!("{:?}", s.suite()).as_str()));
    }
    let mut cfg = rustls::ClientConfig::builder_with_provider(Arc::new(provider))
        .with_protocol_versions(versions)
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    cfg.alpn_protocols = vec![b"h2".to_vec()];
    let mut conn =
        rustls::ClientConnection::new(Arc::new(cfg), ServerName::try_from("localhost").unwrap())
            .unwrap();
    let mut sock = d.tcp();
    while conn.is_handshaking() {
        conn.complete_io(&mut sock).ok()?;
    }
    Some((
        conn.protocol_version()?,
        format!("{:?}", conn.negotiated_cipher_suite()?.suite()),
    ))
}

/// A TLS daemon whose `grpc.tls` section carries `extra`.
fn tls_daemon(name: &str, extra: &str) -> (Daemon, Pki, PathBuf) {
    let dir = temp_dir(name);
    let pki = Pki::new();
    let (_, server_pem, server_key, _) = pki.leaf("localhost", &["localhost"]);
    std::fs::write(dir.join("server.pem"), &server_pem).unwrap();
    std::fs::write(dir.join("server.key"), &server_key).unwrap();
    let config = format!(
        "grpc:\n  bind: \"127.0.0.1:PORT\"\n  tls:\n    enabled: true\n    cert_file: \"{0}/server.pem\"\n    key_file: \"{0}/server.key\"\n{extra}\
topics:\n  - \"T::Open=OpenStream\"\n",
        dir.display()
    );
    let d = Daemon::start(&dir, &config);
    (d, pki, dir)
}

/// §7.1 `grpc.tls.cipher_suites`: the listener negotiates only the named
/// suites, in the order named; a client without one of them fails the
/// handshake. Without the key a client gets TLS 1.3 with the provider's
/// suites.
#[test]
fn grpcbridge_7_1_the_configured_cipher_suites_are_the_ones_offered() {
    use rustls::ProtocolVersion::{TLSv1_2, TLSv1_3};
    use rustls::version::{TLS12, TLS13};
    let (d, pki, _dir) = tls_daemon(
        "suites",
        "    cipher_suites: [\"TLS_CHACHA20_POLY1305_SHA256\", \"TLS_AES_128_GCM_SHA256\", \"TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384\"]\n",
    );
    // The client prefers AES-256 and AES-128; the server's order wins.
    assert_eq!(
        negotiated(&d, &pki.ca_pem, &[&TLS13], &[]),
        Some((TLSv1_3, "TLS13_CHACHA20_POLY1305_SHA256".into()))
    );
    assert_eq!(
        negotiated(
            &d,
            &pki.ca_pem,
            &[&TLS13],
            &["TLS13_AES_256_GCM_SHA384", "TLS13_AES_128_GCM_SHA256"]
        ),
        Some((TLSv1_3, "TLS13_AES_128_GCM_SHA256".into()))
    );
    assert_eq!(
        negotiated(&d, &pki.ca_pem, &[&TLS13], &["TLS13_AES_256_GCM_SHA384"]),
        None,
        "a suite outside the list was negotiated"
    );
    assert_eq!(
        negotiated(&d, &pki.ca_pem, &[&TLS12], &[]),
        Some((TLSv1_2, "TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384".into()))
    );
    drop(d);
    let (d, pki, _dir) = tls_daemon("suites-default", "");
    let (version, _) = negotiated(&d, &pki.ca_pem, &[&TLS12, &TLS13], &[]).expect("default");
    assert_eq!(version, TLSv1_3, "TLS 1.3 is the default");
}

/// §7.1 `grpc.tls.min_version`: with `1.3` a TLS 1.2 client fails the
/// handshake; with the default `1.2` it is served with TLS 1.2.
#[test]
fn grpcbridge_7_1_min_version_1_3_refuses_a_tls_1_2_client() {
    use rustls::version::{TLS12, TLS13};
    let (d, pki, _dir) = tls_daemon("min13", "    min_version: \"1.3\"\n");
    assert_eq!(negotiated(&d, &pki.ca_pem, &[&TLS12], &[]), None);
    assert!(negotiated(&d, &pki.ca_pem, &[&TLS13], &[]).is_some());
    drop(d);
    let (d, pki, _dir) = tls_daemon("min12", "");
    assert_eq!(
        negotiated(&d, &pki.ca_pem, &[&TLS12], &[]).map(|(v, _)| v),
        Some(rustls::ProtocolVersion::TLSv1_2)
    );
}

/// §7.1: a suite this build does not offer, and an empty list, stop the
/// start with a configuration error, and `--check-config` refuses them.
#[test]
fn grpcbridge_7_1_an_unknown_suite_stops_the_start() {
    let dir = temp_dir("badsuite");
    let path = dir.join("grpc-bridged.yaml");
    for (suites, needle) in [
        ("[\"TLS_RSA_WITH_RC4_128_SHA\"]", "TLS_RSA_WITH_RC4_128_SHA"),
        ("[]", "grpc.tls.cipher_suites"),
    ] {
        std::fs::write(
            &path,
            format!("grpc:\n  bind: \"127.0.0.1:0\"\n  tls:\n    cipher_suites: {suites}\n"),
        )
        .unwrap();
        for extra in [&[][..], &["--check-config"][..]] {
            let out = Command::new(env!("CARGO_BIN_EXE_zerodds-grpc-bridged"))
                .args(["--config", path.to_str().unwrap()])
                .args(extra)
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(1), "{suites} {extra:?}");
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            assert!(text.contains(needle), "{text}");
        }
    }
}

/// A GET of `path` on stream `sid` with the optional `authorization`: the
/// `:status`, `content-type` and the body.
fn http2_get<S: Read + Write>(
    c: &mut H2<S>,
    sid: u32,
    path: &str,
    authorization: Option<&str>,
) -> (String, String, Vec<u8>) {
    let mut fields = vec![
        HeaderField {
            name: ":method".into(),
            value: "GET".into(),
        },
        HeaderField {
            name: ":scheme".into(),
            value: "https".into(),
        },
        HeaderField {
            name: ":path".into(),
            value: path.into(),
        },
    ];
    if let Some(a) = authorization {
        fields.push(HeaderField {
            name: "authorization".into(),
            value: a.into(),
        });
    }
    let block = c.enc.encode(&fields);
    frame(
        &mut c.s,
        FrameType::Headers,
        Flags::END_HEADERS | Flags::END_STREAM,
        sid,
        &block,
    );
    let (mut status, mut ctype, mut body) = (String::new(), String::new(), Vec::new());
    loop {
        let mut h = [0u8; 9];
        c.s.read_exact(&mut h).expect("a response frame");
        let len = (usize::from(h[0]) << 16) | (usize::from(h[1]) << 8) | usize::from(h[2]);
        let mut p = vec![0u8; len];
        c.s.read_exact(&mut p).unwrap();
        let end = h[4] & Flags::END_STREAM != 0;
        if h[3] == FrameType::Headers as u8 {
            for f in c.dec.decode(&p).unwrap() {
                match f.name.as_str() {
                    ":status" => status = f.value,
                    "content-type" => ctype = f.value,
                    _ => {}
                }
            }
        } else if h[3] == FrameType::Data as u8 {
            body.extend_from_slice(&p);
        } else {
            continue;
        }
        if end {
            return (status, ctype, body);
        }
    }
}

/// §5.2 over TLS: `GET /idl/<dds_name>` on the HTTP/2 connection (ALPN
/// `h2`) returns the IDL as `text/x-omg-idl`, under the authentication of
/// §7.2: without the token 401; a topic without `idl_file` 404.
#[test]
fn grpcbridge_5_2_idl_over_tls_on_the_h2_connection() {
    let dir = temp_dir("idl-tls");
    let idl = "module Chat { struct Message { string text; }; };\n";
    std::fs::write(dir.join("chat.idl"), idl).unwrap();
    let pki = Pki::new();
    let (_, server_pem, server_key, _) = pki.leaf("localhost", &["localhost"]);
    std::fs::write(dir.join("server.pem"), &server_pem).unwrap();
    std::fs::write(dir.join("server.key"), &server_key).unwrap();
    let config = format!(
        "grpc:\n  bind: \"127.0.0.1:PORT\"\n  tls:\n    enabled: true\n    cert_file: \"{0}/server.pem\"\n    key_file: \"{0}/server.key\"\n\
auth:\n  mode: bearer\n  bearer_token: tk\n\
topics:\n  - dds_name: \"Chat::Message\"\n    idl_file: \"{0}/chat.idl\"\n  - dds_name: \"Other\"\n",
        dir.display()
    );
    let d = Daemon::start(&dir, &config);
    let mut c = open(tls_client(&d, &pki.ca_pem, None));
    let (status, ctype, body) = http2_get(&mut c, 1, "/idl/Chat::Message", Some("Bearer tk"));
    assert_eq!((status.as_str(), ctype.as_str()), ("200", "text/x-omg-idl"));
    assert_eq!(body, idl.as_bytes());
    let (status, _, _) = http2_get(&mut c, 3, "/idl/Chat%3A%3AMessage", Some("Bearer tk"));
    assert_eq!(status, "200", "a percent-encoded name");
    assert_eq!(http2_get(&mut c, 5, "/idl/Chat::Message", None).0, "401");
    assert_eq!(
        http2_get(&mut c, 7, "/idl/Other", Some("Bearer tk")).0,
        "404"
    );
    assert_eq!(
        http2_get(&mut c, 9, "/idl/Nothing", Some("Bearer tk")).0,
        "404"
    );
}

/// §7.1 SIGHUP: a new server certificate serves the next connection.
#[test]
fn grpcbridge_7_1_a_sighup_serves_the_next_connection_with_the_new_certificate() {
    let dir = temp_dir("rotate");
    let old = Pki::new();
    let new = Pki::new();
    let write = |pki: &Pki| {
        let (_, pem, key, _) = pki.leaf("localhost", &["localhost"]);
        std::fs::write(dir.join("server.pem"), pem).unwrap();
        std::fs::write(dir.join("server.key"), key).unwrap();
    };
    write(&old);
    let config = format!(
        "grpc:\n  bind: \"127.0.0.1:PORT\"\n  tls:\n    enabled: true\n    cert_file: \"{0}/server.pem\"\n    key_file: \"{0}/server.key\"\ntopics:\n  - \"T::Open=OpenStream\"\n",
        dir.display()
    );
    let d = Daemon::start(&dir, &config);
    assert_eq!(
        publish(&mut tls_client(&d, &old.ca_pem, None), "OpenStream", None),
        OK
    );
    write(&new);
    d.sighup("\"event\":\"reloaded\"");
    assert_eq!(
        publish(&mut tls_client(&d, &new.ca_pem, None), "OpenStream", None),
        OK
    );
    let mut stale = tls_client(&d, &old.ca_pem, None);
    assert!(
        stale.write_all(PREFACE).is_err() || stale.flush().is_err() || {
            let mut b = [0u8; 1];
            stale.read(&mut b).is_err()
        }
    );
}

// ---- JWT ------------------------------------------------------------------

/// An RSA key from `openssl genpkey`: PKCS#8 DER and the SPKI PEM.
fn rsa_key(dir: &Path) -> (Vec<u8>, String) {
    let key = dir.join("jwt.key");
    let run = |args: &[&str]| {
        let out = Command::new("openssl")
            .args(args)
            .output()
            .expect("openssl on PATH: the JWT test needs a real RSA key");
        assert!(out.status.success(), "openssl {args:?}: {out:?}");
        out.stdout
    };
    let k = key.to_str().unwrap();
    run(&[
        "genpkey",
        "-algorithm",
        "RSA",
        "-pkeyopt",
        "rsa_keygen_bits:2048",
        "-out",
        k,
    ]);
    let der = run(&["pkcs8", "-topk8", "-nocrypt", "-outform", "DER", "-in", k]);
    let pem = run(&["pkey", "-pubout", "-in", k]);
    (der, String::from_utf8(pem).unwrap())
}

fn jwt(pkcs8: &[u8], payload: &str) -> String {
    use base64::Engine as _;
    let b64 = |b: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b);
    let signed = format!(
        "{}.{}",
        b64(br#"{"alg":"RS256","typ":"JWT"}"#),
        b64(payload.as_bytes())
    );
    let pair = ring::signature::RsaKeyPair::from_pkcs8(pkcs8).unwrap();
    let mut sig = vec![0u8; pair.public().modulus_len()];
    pair.sign(
        &ring::signature::RSA_PKCS1_SHA256,
        &ring::rand::SystemRandom::new(),
        signed.as_bytes(),
        &mut sig,
    )
    .unwrap();
    format!("Bearer {signed}.{}", b64(&sig))
}

/// §7.2 jwt with Spec §3's `audience` and `required_claim`: the `sub`
/// claim is the subject for the ACL; an expired token, a foreign audience
/// or a missing scope is UNAUTHENTICATED.
#[test]
fn grpcbridge_7_2_jwt_checks_signature_time_audience_and_scope() {
    let dir = temp_dir("jwt");
    let (pkcs8, pem) = rsa_key(&dir);
    std::fs::write(dir.join("jwt.pub"), pem).unwrap();
    let config = format!(
        "grpc:\n  bind: \"127.0.0.1:PORT\"\nauth:\n  mode: jwt\n  jwt:\n    public_key: \"{}/jwt.pub\"\n    audience: zerodds-grpc\n    required_claim: \"scope=dds.write\"\n\
topics:\n  - dds_name: \"T::Open\"\n    grpc_service: OpenStream\n    acl:\n      publish: [alice]\n",
        dir.display()
    );
    let d = Daemon::start(&dir, &config);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let token = |sub: &str, exp: u64, aud: &str, scope: &str| {
        jwt(
            &pkcs8,
            &format!(r#"{{"sub":"{sub}","aud":"{aud}","scope":"{scope}","exp":{exp}}}"#),
        )
    };
    let good = token("alice", now + 600, "zerodds-grpc", "dds.read dds.write");
    assert_eq!(tcp_publish(&d, "OpenStream", Some(&good)), OK);
    let carol = token("carol", now + 600, "zerodds-grpc", "dds.write");
    assert_eq!(
        tcp_publish(&d, "OpenStream", Some(&carol)),
        PERMISSION_DENIED
    );
    for bad in [
        token("alice", now - 3600, "zerodds-grpc", "dds.write"),
        token("alice", now + 600, "elsewhere", "dds.write"),
        token("alice", now + 600, "zerodds-grpc", "dds.read"),
    ] {
        assert_eq!(tcp_publish(&d, "OpenStream", Some(&bad)), UNAUTHENTICATED);
    }
}

/// An EC P-256 key from `openssl genpkey` at `dir/jwt-ec.key`, and its SPKI
/// PEM.
fn ec_key(dir: &Path) -> (PathBuf, String) {
    let key = dir.join("jwt-ec.key");
    let k = key.to_str().unwrap();
    let run = |args: &[&str]| {
        let out = Command::new("openssl")
            .args(args)
            .output()
            .expect("openssl on PATH: the ES256 test needs a real EC key");
        assert!(out.status.success(), "openssl {args:?}: {out:?}");
        out.stdout
    };
    run(&[
        "genpkey",
        "-algorithm",
        "EC",
        "-pkeyopt",
        "ec_paramgen_curve:P-256",
        "-out",
        k,
    ]);
    let pem = run(&["pkey", "-pubout", "-in", k]);
    (key, String::from_utf8(pem).unwrap())
}

/// The DER of one TLV: its contents and what follows (short and long
/// lengths up to two octets, as an ECDSA-Sig-Value of P-256 has them).
fn der(input: &[u8]) -> (&[u8], &[u8]) {
    let (len, at) = match input[1] {
        n if n < 0x80 => (usize::from(n), 2),
        0x81 => (usize::from(input[2]), 3),
        _ => (usize::from(input[2]) << 8 | usize::from(input[3]), 4),
    };
    (&input[at..at + len], &input[at + len..])
}

/// `Bearer <JWS>` with `header`, signed by `openssl dgst -sha256 -sign`
/// with the EC key at `key`: the DER `ECDSA-Sig-Value` as the 64-octet
/// `R || S` of RFC 7518 §3.4.
fn jwt_es256(key: &Path, header: &str, payload: &str) -> String {
    use base64::Engine as _;
    let b64 = |b: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b);
    let signed = format!("{}.{}", b64(header.as_bytes()), b64(payload.as_bytes()));
    let mut child = Command::new("openssl")
        .args(["dgst", "-sha256", "-sign", key.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("openssl dgst");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(signed.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "openssl dgst: {out:?}");
    let (seq, _) = der(&out.stdout);
    let (r, rest) = der(seq);
    let (s, _) = der(rest);
    let mut fixed = [0u8; 64];
    for (int, at) in [(r, 0), (s, 32)] {
        let int = &int[int.len().saturating_sub(32)..];
        fixed[at + 32 - int.len()..at + 32].copy_from_slice(int);
    }
    format!("Bearer {signed}.{}", b64(&fixed))
}

/// §7.2 jwt with ES256: a token openssl signed with the P-256 key whose
/// SPKI PEM is `auth.jwt.public_key` passes, its `sub` is the subject of
/// the ACL; an expired one, one signed by another key and one whose header
/// says RS256 are UNAUTHENTICATED.
#[test]
fn grpcbridge_7_2_jwt_es256_is_accepted() {
    let dir = temp_dir("jwt-es256");
    let (key, pem) = ec_key(&dir);
    std::fs::write(dir.join("jwt-ec.pub"), pem).unwrap();
    let other_dir = temp_dir("jwt-es256-other");
    let (other, _) = ec_key(&other_dir);
    let config = format!(
        "grpc:\n  bind: \"127.0.0.1:PORT\"\nauth:\n  mode: jwt\n  jwt:\n    public_key: \"{}/jwt-ec.pub\"\n    audience: zerodds-grpc\n\
topics:\n  - dds_name: \"T::Open\"\n    grpc_service: OpenStream\n    acl:\n      publish: [alice]\n",
        dir.display()
    );
    let d = Daemon::start(&dir, &config);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let es = r#"{"alg":"ES256","typ":"JWT"}"#;
    let payload =
        |sub: &str, exp: u64| format!(r#"{{"sub":"{sub}","aud":"zerodds-grpc","exp":{exp}}}"#);
    let good = jwt_es256(&key, es, &payload("alice", now + 600));
    assert_eq!(tcp_publish(&d, "OpenStream", Some(&good)), OK);
    let carol = jwt_es256(&key, es, &payload("carol", now + 600));
    assert_eq!(
        tcp_publish(&d, "OpenStream", Some(&carol)),
        PERMISSION_DENIED
    );
    for bad in [
        jwt_es256(&key, es, &payload("alice", now - 3600)),
        jwt_es256(&other, es, &payload("alice", now + 600)),
        jwt_es256(&key, r#"{"alg":"RS256"}"#, &payload("alice", now + 600)),
    ] {
        assert_eq!(tcp_publish(&d, "OpenStream", Some(&bad)), UNAUTHENTICATED);
    }
}

/// §7.2: `auth.mode: none` is the dev mode of a loopback bind. With TLS on
/// a public address the start stops with exit 1 and names §7.2, and
/// `--check-config` refuses the file; on 127.0.0.1 the same file starts.
#[test]
fn grpcbridge_7_2_mode_none_on_a_public_bind_does_not_start() {
    let dir = temp_dir("none-public");
    let path = dir.join("grpc-bridged.yaml");
    std::fs::write(
        &path,
        "grpc:\n  bind: \"0.0.0.0:0\"\n  tls:\n    enabled: true\n    cert_file: /nonexistent/c.pem\n    key_file: /nonexistent/k.pem\nauth:\n  mode: none\n",
    )
    .unwrap();
    let bin = env!("CARGO_BIN_EXE_zerodds-grpc-bridged");
    let out = Command::new(bin)
        .args(["--config", path.to_str().unwrap()])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(
        err.contains("auth.mode: none") && err.contains("§7.2"),
        "{err}"
    );
    let out = Command::new(bin)
        .args(["--config", path.to_str().unwrap(), "--check-config"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.contains("auth.mode: none"), "{text}");
    // On a loopback bind the mode is allowed; the start then stops at the
    // missing certificate (exit 4), past the auth check.
    let out = Command::new(bin)
        .args([
            "--config",
            path.to_str().unwrap(),
            "--set",
            "grpc.bind=127.0.0.1:0",
        ])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(4), "{err}");
    assert!(!err.contains("auth.mode: none"), "{err}");
}

/// Spec §8.2: the ten `zerodds_grpc_bridge_*` families count what the
/// calls did — status per call, auth failures by reason, a RST_STREAM by
/// its error code, and (with the DDS side) the samples of a topic.
#[test]
fn grpcbridge_8_2_the_metrics_of_section_8_2_count_calls_failures_and_resets() {
    let dir = temp_dir("metrics");
    let config = format!(
        "{}\nmetrics:\n  enabled: true\n  listen: \"127.0.0.1:0\"\n",
        bearer_config("tk-m")
    );
    let d = Daemon::start(&dir, &config);
    let admin = d.logged_port("admin_endpoint");
    // A Subscribe stream (spec §4.3), open before anything is published: its
    // own DataReader receives what the Publish below writes.
    let mut sub = open(d.tcp());
    subscribe_open(&mut sub, 1, "OpenStream", Some("Bearer tk-m"));
    let mut s = open(d.tcp());
    assert_eq!(
        call(&mut s, 1, "OpenStream", "Publish", Some("Bearer tk-m")).0,
        OK
    );
    assert_eq!(
        call(&mut s, 3, "ClosedStream", "Publish", Some("Bearer tk-m")).0,
        PERMISSION_DENIED
    );
    assert_eq!(
        call(&mut s, 5, "OpenStream", "Publish", None).0,
        UNAUTHENTICATED
    );
    assert_eq!(
        call(&mut s, 7, "OpenStream", "Publish", Some("Bearer no")).0,
        UNAUTHENTICATED
    );
    // A Publish stream opened and cancelled with RST_STREAM(CANCEL) (a
    // reset of a stream never opened is a connection error, RFC 9113
    // §6.4), then a call behind it on the same connection: its answer
    // means the reset has been read.
    let fields: Vec<HeaderField> = [
        (":method", "POST"),
        (":scheme", "http"),
        (":path", "/zerodds.t.v1.OpenStream/Publish"),
        ("content-type", "application/grpc+proto"),
        ("authorization", "Bearer tk-m"),
    ]
    .into_iter()
    .map(|(n, v)| HeaderField {
        name: n.into(),
        value: v.into(),
    })
    .collect();
    let block = s.enc.encode(&fields);
    frame(&mut s.s, FrameType::Headers, Flags::END_HEADERS, 9, &block);
    frame(&mut s.s, FrameType::RstStream, 0, 9, &8u32.to_be_bytes());
    assert_eq!(
        call(&mut s, 11, "NoSuchStream", "Publish", Some("Bearer tk-m")).0,
        5
    );
    // The Subscribe stream carries what Publish wrote; the client cancels it
    // after the first message, and a call behind the reset on the same
    // connection answers once the reset has been read.
    let received = first_message(&mut sub, 1);
    assert!(received > 0, "no sample reached Subscribe");
    assert_eq!(
        call(&mut sub, 3, "NoSuchStream", "Publish", Some("Bearer tk-m")).0,
        5
    );
    let body = scrape(admin);
    let v = |series: &str| metric(&body, series).unwrap_or_else(|| panic!("{series}\n{body}"));
    assert_eq!(
        v(r#"zerodds_grpc_bridge_rpcs_total{method="Publish",service="OpenStream",status="OK"}"#),
        1.0
    );
    assert_eq!(
        v(
            r#"zerodds_grpc_bridge_rpcs_total{method="Publish",service="ClosedStream",status="PERMISSION_DENIED"}"#
        ),
        1.0
    );
    assert_eq!(
        v(
            r#"zerodds_grpc_bridge_rpcs_total{method="Publish",service="OpenStream",status="UNAUTHENTICATED"}"#
        ),
        2.0
    );
    assert_eq!(
        v(
            r#"zerodds_grpc_bridge_rpcs_total{method="Publish",service="unknown",status="NOT_FOUND"}"#
        ),
        2.0
    );
    assert_eq!(
        v(r#"zerodds_grpc_bridge_auth_failures_total{reason="missing"}"#),
        1.0
    );
    assert_eq!(
        v(r#"zerodds_grpc_bridge_auth_failures_total{reason="rejected"}"#),
        1.0
    );
    // The reset above and the cancel of the Subscribe stream; each call
    // ended CANCELLED (spec §8.1 `grpc_status`).
    assert_eq!(
        v(r#"zerodds_grpc_bridge_h2_resets_total{error_code="CANCEL"}"#),
        2.0
    );
    assert_eq!(
        v(
            r#"zerodds_grpc_bridge_rpcs_total{method="Publish",service="OpenStream",status="CANCELLED"}"#
        ),
        1.0
    );
    assert_eq!(
        v(
            r#"zerodds_grpc_bridge_rpcs_total{method="Subscribe",service="OpenStream",status="CANCELLED"}"#
        ),
        1.0
    );
    assert_eq!(
        v(r#"zerodds_grpc_bridge_messages_in_total{dds_topic="T::Open"}"#),
        1.0
    );
    assert_eq!(
        v(r#"zerodds_grpc_bridge_bytes_in_total{dds_topic="T::Open"}"#),
        13.0
    );
    assert_eq!(
        v(r#"zerodds_grpc_bridge_streams_active{service="OpenStream"}"#),
        0.0
    );
    assert_eq!(
        v(r#"zerodds_grpc_bridge_messages_in_total{dds_topic="T::Closed"}"#),
        0.0
    );
    assert_eq!(
        v(r#"zerodds_grpc_bridge_dds_samples_published{dds_topic="T::Open"}"#),
        1.0
    );
    assert!(v(r#"zerodds_grpc_bridge_dds_samples_received{dds_topic="T::Open"}"#) >= 1.0);
    assert_eq!(
        v(r#"zerodds_grpc_bridge_bytes_out_total{dds_topic="T::Open"}"#),
        received as f64
    );
    assert!(v(r#"zerodds_grpc_bridge_messages_out_total{dds_topic="T::Open"}"#) >= 1.0);
}

/// §9.2: SIGTERM with a client connected — GOAWAY with the last stream the
/// daemon answered and NO_ERROR, then the connection closes and the daemon
/// exits 0. Before, the connection thread sat in a 5 s read and the client
/// saw a plain EOF.
#[test]
fn grpcbridge_9_2_sigterm_sends_goaway_with_the_last_stream_and_exits_0() {
    let dir = temp_dir("goaway");
    let mut d = Daemon::start(&dir, &bearer_config("tk-g"));
    let mut c = open(d.tcp());
    assert_eq!(
        call(&mut c, 1, "OpenStream", "Publish", Some("Bearer tk-g")).0,
        OK
    );
    assert_eq!(
        call(&mut c, 3, "OpenStream", "Publish", Some("Bearer tk-g")).0,
        OK
    );
    let started = Instant::now();
    // SAFETY: the pid is our live child.
    unsafe { libc::kill(d.child.id() as i32, libc::SIGTERM) };
    let mut goaway = None;
    loop {
        let mut h = [0u8; 9];
        if c.s.read_exact(&mut h).is_err() {
            break;
        }
        let len = (usize::from(h[0]) << 16) | (usize::from(h[1]) << 8) | usize::from(h[2]);
        let mut p = vec![0u8; len];
        c.s.read_exact(&mut p).unwrap();
        if h[3] == FrameType::GoAway as u8 {
            goaway = Some((
                u32::from_be_bytes([p[0], p[1], p[2], p[3]]),
                u32::from_be_bytes([p[4], p[5], p[6], p[7]]),
            ));
        }
    }
    assert_eq!(goaway, Some((3, 0)), "GOAWAY(last_stream_id, NO_ERROR)");
    let status = loop {
        if let Some(st) = d.child.try_wait().unwrap() {
            break st;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{}",
            d.log_text()
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(0), "{}", d.log_text());
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
}

/// One frame: its type, stream id and payload.
fn read_frame_of(s: &mut impl Read) -> (u8, u32, Vec<u8>) {
    let mut h = [0u8; 9];
    s.read_exact(&mut h).expect("a response frame");
    let len = (usize::from(h[0]) << 16) | (usize::from(h[1]) << 8) | usize::from(h[2]);
    let sid = u32::from_be_bytes([h[5], h[6], h[7], h[8]]) & 0x7fff_ffff;
    let mut p = vec![0u8; len];
    s.read_exact(&mut p).unwrap();
    (h[3], sid, p)
}

/// Reads frames up to the GOAWAY: its last-stream-id and error code.
fn until_goaway<S: Read + Write>(c: &mut H2<S>) -> (u32, u32) {
    loop {
        let (typ, _, p) = read_frame_of(&mut c.s);
        if typ == FrameType::GoAway as u8 {
            return (
                u32::from_be_bytes([p[0], p[1], p[2], p[3]]),
                u32::from_be_bytes([p[4], p[5], p[6], p[7]]),
            );
        }
    }
}

/// Waits for the daemon's exit within `limit` of `since`; its exit code.
fn exit_within(d: &mut Daemon, since: Instant, limit: Duration) -> Option<i32> {
    loop {
        if let Some(st) = d.child.try_wait().unwrap() {
            return st.code();
        }
        assert!(since.elapsed() < limit, "no exit: {}", d.log_text());
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The opaque-bytes DataWriter of `topic` on `rt`, matched with the
/// daemon's DataReaders of it.
fn peer_writer(
    rt: &zerodds_dcps::runtime::DcpsRuntime,
    topic: &str,
    readers: usize,
) -> zerodds_rtps::wire_types::EntityId {
    let w = rt
        .register_user_writer(zerodds_dcps::runtime::UserWriterConfig {
            topic_name: topic.into(),
            type_name: "ZeroDdsBridgeBytes".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while rt.user_writer_matched_count(w) < readers {
        assert!(
            Instant::now() < deadline,
            "the daemon's reader did not match"
        );
        rt.wait_match_event(Duration::from_millis(200));
    }
    w
}

fn sigterm(d: &Daemon) -> Instant {
    // SAFETY: the pid is our live child.
    unsafe { libc::kill(d.child.id() as i32, libc::SIGTERM) };
    Instant::now()
}

/// §9.2 "let running RPCs end naturally": a Subscribe stream open at
/// SIGTERM gets the GOAWAY with its stream id, then still the DDS sample
/// written after it; when the client cancels the stream the connection
/// ends and the daemon exits 0, long before the 30 s of the drain.
#[test]
fn grpcbridge_9_2_a_stream_open_at_sigterm_keeps_delivering_until_the_client_ends_it() {
    let dir = temp_dir("drain-stream");
    let mut d = Daemon::start(&dir, &bearer_config("tk-d"));
    let mut c = open(d.tcp());
    subscribe_open(&mut c, 1, "OpenStream", Some("Bearer tk-d"));
    let dds = dds_peer(d.domain.id());
    let w = peer_writer(&dds, "T::Open", 1);
    let started = sigterm(&d);
    assert_eq!(until_goaway(&mut c), (1, 0), "GOAWAY(1, NO_ERROR)");
    dds.write_user_sample_borrowed(w, b"after-goaway").unwrap();
    let data = loop {
        let (typ, sid, p) = read_frame_of(&mut c.s);
        assert!(
            typ != FrameType::Headers as u8,
            "the stream ended before the sample"
        );
        if typ == FrameType::Data as u8 && sid == 1 {
            break p;
        }
    };
    assert!(data.windows(12).any(|w| w == b"after-goaway"), "{data:?}");
    frame(&mut c.s, FrameType::RstStream, 0, 1, &8u32.to_be_bytes());
    assert_eq!(
        exit_within(&mut d, started, Duration::from_secs(10)),
        Some(0)
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    dds.shutdown();
}

/// §9.2 "graceful drain (max 30 s)", §4.5 daemon drain: with
/// `grpc.drain_secs: 1` a Subscribe stream still open after the GOAWAY ends
/// with UNAVAILABLE after about a second, then the daemon exits 0.
#[test]
fn grpcbridge_9_2_after_the_drain_limit_open_streams_end_unavailable() {
    let dir = temp_dir("drain-limit");
    let config = bearer_config("tk-l").replace(
        "  bind: \"127.0.0.1:PORT\"",
        "  bind: \"127.0.0.1:PORT\"\n  drain_secs: 1",
    );
    let mut d = Daemon::start(&dir, &config);
    let mut c = open(d.tcp());
    subscribe_open(&mut c, 1, "OpenStream", Some("Bearer tk-l"));
    let started = sigterm(&d);
    assert_eq!(until_goaway(&mut c), (1, 0));
    let status = loop {
        let (typ, sid, p) = read_frame_of(&mut c.s);
        if typ == FrameType::Headers as u8 && sid == 1 {
            let fields = c.dec.decode(&p).unwrap();
            if let Some(st) = fields.iter().find(|f| f.name == "grpc-status") {
                break st.value.clone();
            }
        }
    };
    let at = started.elapsed();
    assert_eq!(status, "14", "UNAVAILABLE");
    assert!(
        at >= Duration::from_millis(900) && at < Duration::from_secs(4),
        "the stream ended after {at:?}"
    );
    assert_eq!(
        exit_within(&mut d, started, Duration::from_secs(8)),
        Some(0)
    );
    assert!(d.log_text().contains("\"event\":\"drain_limit\""));
}

/// §9.2, RFC 9113 §6.8: after the GOAWAY a stream the client opens is
/// refused with RST_STREAM(REFUSED_STREAM), and a new connection is
/// refused by the closed listening socket; the stream the GOAWAY names is
/// still served.
#[test]
fn grpcbridge_9_2_a_new_stream_after_goaway_is_refused() {
    let dir = temp_dir("drain-refuse");
    let mut d = Daemon::start(&dir, &bearer_config("tk-r"));
    let mut c = open(d.tcp());
    subscribe_open(&mut c, 1, "OpenStream", Some("Bearer tk-r"));
    let started = sigterm(&d);
    assert_eq!(until_goaway(&mut c), (1, 0));
    let refused_by = Instant::now() + Duration::from_secs(2);
    loop {
        match TcpStream::connect(("127.0.0.1", d.port)) {
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => break,
            other => assert!(
                Instant::now() < refused_by,
                "a connection during the drain: {other:?}"
            ),
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let fields: Vec<HeaderField> = [
        (":method", "POST".to_string()),
        (":scheme", "http".to_string()),
        (":path", "/zerodds.t.v1.OpenStream/Publish".to_string()),
        ("content-type", "application/grpc+proto".to_string()),
        ("authorization", "Bearer tk-r".to_string()),
    ]
    .into_iter()
    .map(|(n, v)| HeaderField {
        name: n.into(),
        value: v,
    })
    .collect();
    let block = c.enc.encode(&fields);
    frame(&mut c.s, FrameType::Headers, Flags::END_HEADERS, 3, &block);
    frame(
        &mut c.s,
        FrameType::Data,
        Flags::END_STREAM,
        3,
        &[0, 0, 0, 0, 8, 0x0a, 6, 0, 1, 0, 0, b'h', b'i'],
    );
    let code = loop {
        let (typ, sid, p) = read_frame_of(&mut c.s);
        assert!(
            !(typ == FrameType::Headers as u8 && sid == 3),
            "stream 3 was answered"
        );
        if typ == FrameType::RstStream as u8 && sid == 3 {
            break u32::from_be_bytes([p[0], p[1], p[2], p[3]]);
        }
    };
    assert_eq!(code, 7, "REFUSED_STREAM");
    // The stream the GOAWAY names is open: the client ends it.
    frame(&mut c.s, FrameType::RstStream, 0, 1, &8u32.to_be_bytes());
    assert_eq!(
        exit_within(&mut d, started, Duration::from_secs(10)),
        Some(0)
    );
}

/// §9.2 "drain pending DDS writes": a sample a Publish wrote right before
/// SIGTERM reaches the reliable DDS reader of the test; the daemon logs
/// that its writers were acknowledged, then exits 0.
#[test]
fn grpcbridge_9_2_a_sample_published_before_sigterm_reaches_the_dds_reader() {
    use zerodds_dcps::runtime::{UserReaderConfig, UserSample};
    let dir = temp_dir("drain-dds");
    let mut d = Daemon::start(&dir, &bearer_config("tk-w"));
    let dds = dds_peer(d.domain.id());
    let (_reader, rx) = dds
        .register_user_reader(UserReaderConfig {
            topic_name: "T::Open".into(),
            type_name: "ZeroDdsBridgeBytes".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    writer_match::publish_until_received(&rx, || {
        assert_eq!(tcp_publish(&d, "OpenStream", Some("Bearer tk-w")), OK);
    });
    assert_eq!(tcp_publish(&d, "OpenStream", Some("Bearer tk-w")), OK);
    let started = sigterm(&d);
    let payload = loop {
        match rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the sample written before SIGTERM")
        {
            UserSample::Alive { payload, .. } => break payload.to_vec(),
            _ => continue,
        }
    };
    assert_eq!(payload, b"hi");
    assert_eq!(
        exit_within(&mut d, started, Duration::from_secs(10)),
        Some(0)
    );
    assert!(
        d.log_text()
            .contains("\"event\":\"dds_writes_drained\",\"acknowledged\":true"),
        "{}",
        d.log_text()
    );
    dds.shutdown();
}

/// §9.2 "drain pending DDS writes" with an acknowledgement outstanding at
/// SIGTERM: the daemon's writer matches the test's reliable reader (a
/// sample reached it), then the reader's participant stops without notice
/// (no SEDP dispose, no SPDP departure), so the reader stays matched for
/// its lease and never acknowledges. A Publish writes a sample, SIGTERM
/// follows: the exit is held until `grpc.drain_secs` (2 s) and the daemon
/// logs that its writers were not acknowledged, then exits 0.
#[test]
fn grpcbridge_9_2_an_acknowledgement_outstanding_at_sigterm_holds_the_exit_to_the_drain_limit() {
    use zerodds_dcps::runtime::UserReaderConfig;
    let dir = temp_dir("drain-unacked");
    let config = bearer_config("tk-u").replace(
        "  bind: \"127.0.0.1:PORT\"",
        "  bind: \"127.0.0.1:PORT\"\n  drain_secs: 2",
    );
    let mut d = Daemon::start(&dir, &config);
    let dds = dds_peer(d.domain.id());
    let (_reader, rx) = dds
        .register_user_reader(UserReaderConfig {
            topic_name: "T::Open".into(),
            type_name: "ZeroDdsBridgeBytes".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    writer_match::publish_until_received(&rx, || {
        assert_eq!(tcp_publish(&d, "OpenStream", Some("Bearer tk-u")), OK);
    });
    dds.__vanish();
    assert_eq!(tcp_publish(&d, "OpenStream", Some("Bearer tk-u")), OK);
    let started = sigterm(&d);
    assert_eq!(
        exit_within(&mut d, started, Duration::from_secs(10)),
        Some(0)
    );
    let held = started.elapsed();
    assert!(
        held >= Duration::from_millis(1900),
        "the daemon exited {held:?} after SIGTERM, before the drain limit: {}",
        d.log_text()
    );
    assert!(
        d.log_text()
            .contains("\"event\":\"dds_writes_drained\",\"acknowledged\":false"),
        "{}",
        d.log_text()
    );
    dds.shutdown();
}

// ---- §1 L6 tenants --------------------------------------------------------

/// A DDS participant of the test on `domain`.
fn dds_peer(domain: i32) -> Arc<zerodds_dcps::runtime::DcpsRuntime> {
    let mut prefix = [0u8; 12];
    prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
    prefix[4..8].copy_from_slice(&domain.to_be_bytes());
    prefix[8] = 0x61;
    prefix[11] = 0xc1;
    zerodds_dcps::runtime::DcpsRuntime::start(
        domain,
        zerodds_rtps::wire_types::GuidPrefix::from_bytes(prefix),
        zerodds_dcps::runtime::RuntimeConfig::default(),
    )
    .unwrap()
}

/// §1 L6: a TLS connection whose client certificate a tenant names is
/// bound to that tenant: its Publish reaches a DDS reader on the tenant's
/// domain, its Catalog lists only the tenant's topics, and the topics of
/// `domain` are NOT_FOUND to it. A connection without a certificate and one
/// with a certificate no tenant names see the topics of `domain` only.
#[test]
fn grpcbridge_1_l6_a_tls_connection_is_bound_to_the_tenant_of_its_certificate() {
    use zerodds_dcps::runtime::UserReaderConfig;
    let dir = temp_dir("tenant");
    let pki = Pki::new();
    let tenant_domain = zerodds_test_domains::lease_domain();
    let (_, server_pem, server_key, _) = pki.leaf("localhost", &["localhost"]);
    let (plant_der, _, _, plant_key) = pki.leaf("plant-a", &[]);
    let (bob_der, _, _, bob_key) = pki.leaf("bob", &[]);
    for (name, body) in [
        ("server.pem", &server_pem),
        ("server.key", &server_key),
        ("ca.pem", &pki.ca_pem),
    ] {
        std::fs::write(dir.join(name), body).unwrap();
    }
    let config = format!(
        "grpc:\n  bind: \"127.0.0.1:PORT\"\n  tls:\n    enabled: true\n    cert_file: \"{0}/server.pem\"\n    key_file: \"{0}/server.key\"\n    client_ca_file: \"{0}/ca.pem\"\n    require_client_cert: false\n\
topics:\n  - \"T::Open=OpenStream\"\n\
tenants:\n  - name: plant-a\n    domain: {1}\n    subjects: [\"CN=plant-a\"]\n    topics:\n      - dds_name: \"Plant::Status\"\n        grpc_service: zerodds.t.v1.PlantStream\n",
        dir.display(),
        tenant_domain.id()
    );
    let d = Daemon::start(&dir, &config);
    assert!(
        d.log_text().contains("\"tenant\":\"plant-a\""),
        "the tenant's participant: {}",
        d.log_text()
    );

    let dds = dds_peer(tenant_domain.id());
    let (_reader, rx) = dds
        .register_user_reader(UserReaderConfig {
            topic_name: "Plant::Status".into(),
            type_name: "ZeroDdsBridgeBytes".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();

    let mut plant = open(tls_client(&d, &pki.ca_pem, Some((&plant_der, &plant_key))));
    let mut stream = 1;
    let payload = writer_match::publish_until_received(&rx, || {
        assert_eq!(
            call(&mut plant, stream, "PlantStream", "Publish", None).0,
            OK
        );
        stream += 2;
    })
    .payload;
    assert_eq!(payload, b"hi");
    assert_eq!(
        call(&mut plant, stream, "OpenStream", "Publish", None).0,
        NOT_FOUND
    );
    let (status, catalog) = call_body(&mut plant, stream + 2, "PlantStream", "Catalog", None);
    assert_eq!(status, OK);
    let catalog = String::from_utf8_lossy(&catalog);
    assert!(catalog.contains("Plant::Status"), "{catalog}");
    assert!(!catalog.contains("T::Open"), "{catalog}");

    for client in [
        tls_client(&d, &pki.ca_pem, None),
        tls_client(&d, &pki.ca_pem, Some((&bob_der, &bob_key))),
    ] {
        let mut c = open(client);
        assert_eq!(call(&mut c, 1, "PlantStream", "Publish", None).0, NOT_FOUND);
        assert_eq!(call(&mut c, 3, "OpenStream", "Publish", None).0, OK);
    }
    assert!(
        rx.recv_timeout(Duration::from_millis(300)).is_err(),
        "a connection of `domain` wrote to the tenant's domain"
    );
    assert!(d.log_text().contains("\"event\":\"tenant_bound\""));
    dds.shutdown();
}
