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

fn pick_port() -> u16 {
    let s = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    s.local_addr().unwrap().port()
}

struct Daemon {
    child: Child,
    port: u16,
    log: PathBuf,
}

impl Daemon {
    /// Starts the binary on `config` and waits for the listener.
    fn start(dir: &Path, config: &str) -> Self {
        let port = pick_port();
        let path = dir.join("grpc-bridged.yaml");
        std::fs::write(&path, config.replace("PORT", &port.to_string())).unwrap();
        let log = dir.join("daemon.log");
        let child = Command::new(env!("CARGO_BIN_EXE_zerodds-grpc-bridged"))
            .args(["--config", path.to_str().unwrap()])
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(&log).unwrap())
            .spawn()
            .unwrap();
        let d = Self { child, port, log };
        let deadline = Instant::now() + Duration::from_secs(10);
        while TcpStream::connect(("127.0.0.1", port)).is_err() {
            assert!(Instant::now() < deadline, "no listener: {}", d.log_text());
            std::thread::sleep(Duration::from_millis(50));
        }
        d
    }

    fn log_text(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    fn rewrite(&self, dir: &Path, config: &str) {
        std::fs::write(
            dir.join("grpc-bridged.yaml"),
            config.replace("PORT", &self.port.to_string()),
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
    // Sample { payload = "hi" } as one Length-Prefixed-Message.
    frame(
        s,
        FrameType::Data,
        Flags::END_STREAM,
        sid,
        &[0, 0, 0, 0, 4, 0x0a, 2, b'h', b'i'],
    );
    let dec = &mut c.dec;
    let mut data = 0usize;
    loop {
        let mut h = [0u8; 9];
        s.read_exact(&mut h).expect("a response frame");
        let len = (usize::from(h[0]) << 16) | (usize::from(h[1]) << 8) | usize::from(h[2]);
        let mut p = vec![0u8; len];
        s.read_exact(&mut p).unwrap();
        if h[3] == FrameType::Data as u8 {
            data += len;
        }
        if h[3] == FrameType::Headers as u8 {
            let fields = dec.decode(&p).unwrap();
            if let Some(st) = fields.iter().find(|f| f.name == "grpc-status") {
                return (st.value.parse().unwrap(), data);
            }
        }
    }
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
fn bearer_and_acl_are_checked_per_call() {
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

/// §9.2: SIGHUP applies a new token; a file that does not load keeps it.
#[test]
fn a_sighup_applies_the_new_token_and_a_broken_file_keeps_it() {
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

/// A setting that cannot be served stops the start instead of being
/// ignored.
#[test]
fn jwt_without_a_key_and_mtls_without_tls_do_not_start() {
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
    cfg.alpn_protocols = vec![b"h2".to_vec()];
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
fn mtls_serves_h2_and_the_certificate_is_the_subject() {
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

/// §7.1 SIGHUP: a new server certificate serves the next connection.
#[test]
fn a_sighup_serves_the_next_connection_with_the_new_certificate() {
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
fn jwt_checks_signature_time_audience_and_scope() {
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

fn scrape(port: u16) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(b"GET /metrics HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut body = String::new();
    let _ = s.read_to_string(&mut body);
    body
}

/// The value of the sample line that starts with `series`.
fn metric(body: &str, series: &str) -> Option<f64> {
    body.lines()
        .find(|l| l.starts_with(series))
        .and_then(|l| l.rsplit(' ').next())
        .and_then(|v| v.parse().ok())
}

/// Spec §8.2: the ten `zerodds_grpc_bridge_*` families count what the
/// calls did — status per call, auth failures by reason, a RST_STREAM by
/// its error code, and (with the DDS side) the samples of a topic.
#[test]
fn the_metrics_of_section_8_2_count_calls_failures_and_resets() {
    let dir = temp_dir("metrics");
    let admin = pick_port();
    let config = format!(
        "{}\nmetrics:\n  enabled: true\n  listen: \"127.0.0.1:{admin}\"\n",
        bearer_config("tk-m")
    );
    let d = Daemon::start(&dir, &config);
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
    // RST_STREAM(CANCEL) on a fresh stream, then a call behind it on the
    // same connection: its answer means the reset has been read.
    frame(&mut s.s, FrameType::RstStream, 0, 9, &8u32.to_be_bytes());
    assert_eq!(
        call(&mut s, 11, "NoSuchStream", "Publish", Some("Bearer tk-m")).0,
        5
    );
    // With the DDS side, a Subscribe takes what Publish wrote.
    let mut received = 0usize;
    let mut sid = 13;
    let deadline = Instant::now() + Duration::from_secs(5);
    while received == 0 && Instant::now() < deadline {
        received = call(&mut s, sid, "OpenStream", "Subscribe", Some("Bearer tk-m")).1;
        sid += 2;
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(received > 0, "no sample reached Subscribe");
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
        1.0
    );
    assert_eq!(
        v(r#"zerodds_grpc_bridge_auth_failures_total{reason="missing"}"#),
        1.0
    );
    assert_eq!(
        v(r#"zerodds_grpc_bridge_auth_failures_total{reason="rejected"}"#),
        1.0
    );
    assert_eq!(
        v(r#"zerodds_grpc_bridge_h2_resets_total{error_code="CANCEL"}"#),
        1.0
    );
    assert_eq!(
        v(r#"zerodds_grpc_bridge_messages_in_total{dds_topic="T::Open"}"#),
        1.0
    );
    assert_eq!(
        v(r#"zerodds_grpc_bridge_bytes_in_total{dds_topic="T::Open"}"#),
        9.0
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
fn sigterm_sends_goaway_with_the_last_stream_and_exits_0() {
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
