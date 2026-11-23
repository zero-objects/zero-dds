// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! E2E test for `zerodds-ws-bridged` §7 security wireup.
//!
//! Covers:
//! * §7.1 TLS handshake (server-side, optionally with a client cert).
//! * §7.2 auth (bearer token in the `Authorization` header).
//! * §7.3 topic ACL for the auto-subscribe path and the inline subscribe op.
//! * §9.2 SIGHUP cert reload (live reload without restart).
//! * RFC 6455 §7.1 over TLS: the daemon's Close frame, then the end of the
//!   TLS session with `close_notify`.

#![cfg(feature = "daemon")]
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
use std::sync::Arc;
use std::time::Duration;

use rustls::ClientConfig;
use rustls::pki_types::ServerName;
use rustls_pemfile::Item;
use zerodds_websocket_bridge::codec::{decode, encode};
use zerodds_websocket_bridge::connection::{Event, Limits, Receiver};
use zerodds_websocket_bridge::daemon::config::{DaemonConfig, TopicConfig};
use zerodds_websocket_bridge::daemon::server;
use zerodds_websocket_bridge::frame::Frame;
use zerodds_websocket_bridge::handshake::compute_accept;
use zerodds_websocket_bridge::masking::Role;

// ---------- Self-signed Cert-Helper ----------

fn write_temp(name: &str, body: &[u8]) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "zd-ws-sec-e2e-{}-{}-{}",
        name,
        std::process::id(),
        seq
    ));
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    p
}

fn gen_self_signed_for(host: &str) -> (String, String) {
    let ck = rcgen::generate_simple_self_signed(vec![host.to_string()]).unwrap();
    (ck.cert.pem(), ck.key_pair.serialize_pem())
}

fn load_certs_for_root(pem: &str) -> Vec<rustls::pki_types::CertificateDer<'static>> {
    let mut br = std::io::BufReader::new(pem.as_bytes());
    let mut out = Vec::new();
    for item in rustls_pemfile::read_all(&mut br) {
        if let Ok(Item::X509Certificate(d)) = item {
            out.push(d);
        }
    }
    out
}

fn build_test_client_config(server_cert_pem: &str) -> Arc<ClientConfig> {
    let mut roots = rustls::RootCertStore::empty();
    for c in load_certs_for_root(server_cert_pem) {
        roots.add(c).unwrap();
    }
    let provider = rustls::crypto::ring::default_provider();
    let cfg = ClientConfig::builder_with_provider(Arc::new(provider))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    Arc::new(cfg)
}

// ---------- TLS stream helper for the test client ----------

struct TlsTestStream {
    conn: rustls::ClientConnection,
    sock: TcpStream,
}

impl TlsTestStream {
    fn connect(server_cfg: Arc<ClientConfig>, addr: &str) -> Self {
        let server_name: ServerName<'static> =
            ServerName::try_from("localhost".to_string()).expect("server name");
        let conn = rustls::ClientConnection::new(server_cfg, server_name).expect("client conn");
        let sock = TcpStream::connect(addr).expect("tcp");
        sock.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        sock.set_write_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut me = Self { conn, sock };
        me.drive_handshake();
        me
    }

    fn drive_handshake(&mut self) {
        // Drive until handshake completes.
        while self.conn.is_handshaking() {
            if self.conn.wants_write() {
                self.conn.write_tls(&mut self.sock).expect("tls write");
            }
            if self.conn.wants_read() {
                let n = self.conn.read_tls(&mut self.sock).expect("tls read");
                if n == 0 {
                    panic!("eof during handshake");
                }
                self.conn.process_new_packets().expect("process");
            }
        }
        // Drain any pending output.
        while self.conn.wants_write() {
            self.conn.write_tls(&mut self.sock).expect("post hs write");
        }
    }
}

impl Read for TlsTestStream {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        // Drive read until plaintext available.
        loop {
            match self.conn.reader().read(b) {
                Ok(n) if n > 0 => return Ok(n),
                Ok(_) | Err(_) => {}
            }
            if self.conn.wants_read() {
                self.conn.read_tls(&mut self.sock)?;
                self.conn.process_new_packets().map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e}"))
                })?;
            } else {
                return Ok(0);
            }
        }
    }
}

impl Write for TlsTestStream {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        let n = self.conn.writer().write(b)?;
        while self.conn.wants_write() {
            self.conn.write_tls(&mut self.sock)?;
        }
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        while self.conn.wants_write() {
            self.conn.write_tls(&mut self.sock)?;
        }
        Ok(())
    }
}

// ---------- Daemon-Helpers ----------

/// The configuration on a leased DDS domain (the host's share of the band,
/// `zerodds-test-domains`); the lease is held while the test runs.
fn make_secure_cfg(
    cert: &str,
    key: &str,
    with_auth: bool,
) -> (DaemonConfig, zerodds_test_domains::DomainLease) {
    let cert_path = write_temp("ws_cert.pem", cert.as_bytes());
    let key_path = write_temp("ws_key.pem", key.as_bytes());
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.listen = "127.0.0.1:0".to_string();
    let lease = zerodds_test_domains::lease_domain();
    cfg.domain = lease.id();
    cfg.tls_enabled = true;
    cfg.tls_cert_file = cert_path.to_string_lossy().into();
    cfg.tls_key_file = key_path.to_string_lossy().into();
    if with_auth {
        cfg.auth_mode = "bearer".into();
        cfg.auth_bearer_token = Some("secret-tk".into());
        cfg.auth_bearer_subject = Some("alice".into());
        // Topic "Allowed" to alice; topic "Forbidden" to nobody.
        cfg.topic_acl.insert(
            "Allowed".into(),
            (vec!["alice".into()], vec!["alice".into()]),
        );
        cfg.topic_acl
            .insert("Forbidden".into(), (vec!["bob".into()], vec!["bob".into()]));
    }
    cfg.topics.push(TopicConfig {
        name: "Allowed".into(),
        type_name: "Allowed".into(),
        direction: "bidir".into(),
        ws_path: "/topics/allowed".into(),
        reliability: "reliable".into(),
        durability: "volatile".into(),
        history_depth: 10,
        ..TopicConfig::default()
    });
    cfg.topics.push(TopicConfig {
        name: "Forbidden".into(),
        type_name: "Forbidden".into(),
        direction: "bidir".into(),
        ws_path: "/topics/forbidden".into(),
        reliability: "reliable".into(),
        durability: "volatile".into(),
        history_depth: 10,
        ..TopicConfig::default()
    });
    (cfg, lease)
}

fn ws_handshake_request(host: &str, path: &str, auth: Option<&str>) -> String {
    let mut s = format!(
        "GET {path} HTTP/1.1\r\n\
         Host: {host}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         Sec-WebSocket-Version: 13\r\n"
    );
    if let Some(a) = auth {
        s.push_str(&format!("Authorization: {a}\r\n"));
    }
    s.push_str("\r\n");
    s
}

fn read_until_double_crlf<S: Read>(s: &mut S) -> String {
    let mut buf = [0u8; 4096];
    let mut acc = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        if std::time::Instant::now() > deadline {
            break;
        }
        match s.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => acc.extend_from_slice(&buf[..n]),
            Err(_) => break,
        }
        if acc.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8_lossy(&acc).to_string()
}

// ---------- TESTS ----------

/// The next frame the daemon sends on `s`.
fn read_frame<S: Read>(s: &mut S) -> Frame {
    read_frame_after(s, &mut Vec::new())
}

/// The response head of a handshake on `s`, and the octets read behind it
/// (the first frames).
fn read_head<S: Read>(s: &mut S) -> (String, Vec<u8>) {
    let mut acc = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        if let Some(i) = acc.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&acc[..i + 4]).into_owned();
            return (head, acc[i + 4..].to_vec());
        }
        let n = s.read(&mut buf).expect("the response");
        assert!(n > 0, "the connection ended in the response");
        acc.extend_from_slice(&buf[..n]);
    }
}

/// The next frame on `s`, beginning with the octets in `acc`; what follows
/// the frame stays in `acc`.
fn read_frame_after<S: Read>(s: &mut S, acc: &mut Vec<u8>) -> Frame {
    let mut buf = [0u8; 4096];
    loop {
        if let Ok((frame, used)) = decode(acc) {
            acc.drain(..used);
            return frame;
        }
        let n = s.read(&mut buf).expect("a frame");
        assert!(n > 0, "the connection ended before a frame");
        acc.extend_from_slice(&buf[..n]);
    }
}

/// §4.4 over `s`: a `ping` control message is answered with `pong`.
fn ping_pong<S: Read + Write>(s: &mut S) {
    let ping = encode(&Frame::text("{\"op\":\"ping\"}").with_mask([5, 6, 7, 8])).unwrap();
    s.write_all(&ping).unwrap();
    let pong = read_frame(s);
    let text = std::str::from_utf8(&pong.payload).expect("a text frame");
    assert!(text.contains("\"op\":\"pong\""), "{text}");
}

/// The TLS handshake alone, without a panic: `Err` with the reason when
/// the client does not accept the server's certificate.
fn tls_handshake(client_cfg: Arc<ClientConfig>, addr: &str) -> Result<(), String> {
    let name: ServerName<'static> = ServerName::try_from("localhost".to_string()).unwrap();
    let mut conn = rustls::ClientConnection::new(client_cfg, name).unwrap();
    let mut sock = TcpStream::connect(addr).map_err(|e| e.to_string())?;
    sock.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    while conn.is_handshaking() {
        conn.complete_io(&mut sock).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// §7.1: `wss://` carries the handshake and the frames behind it.
#[test]
fn ws_10_6_tls_plain_handshake_succeeds_then_data_flows() {
    let (cert, key) = gen_self_signed_for("localhost");
    let (cfg, _lease) = make_secure_cfg(&cert, &key, false);
    let h = server::start(cfg).expect("start daemon");
    let addr = h.local_addr.clone();

    let client_cfg = build_test_client_config(&cert);
    let mut s = TlsTestStream::connect(client_cfg, &addr);
    s.write_all(ws_handshake_request("localhost", "/topics/allowed", None).as_bytes())
        .unwrap();
    let resp = read_until_double_crlf(&mut s);
    assert!(resp.contains("101 Switching Protocols"), "got: {resp}");
    let expected = compute_accept("dGhlIHNhbXBsZSBub25jZQ==");
    assert!(
        resp.contains(&expected),
        "expected accept hash in response, got: {resp}"
    );
    // Frames cross the TLS session in both directions, more than once.
    for _ in 0..3 {
        ping_pong(&mut s);
    }
    drop(h);
}

/// A wss connection after the 101.
fn tls_open(client_cfg: &Arc<ClientConfig>, addr: &str) -> TlsTestStream {
    let mut s = TlsTestStream::connect(Arc::clone(client_cfg), addr);
    s.write_all(ws_handshake_request("localhost", "/topics/allowed", None).as_bytes())
        .unwrap();
    let resp = read_until_double_crlf(&mut s);
    assert!(resp.contains("101 Switching Protocols"), "got: {resp}");
    s
}

/// Reads a wss connection to its end. Returns the status code of the Close
/// frame, which has to be the only frame the daemon still sent; panics
/// unless the TLS session ended with `close_notify` and the TCP connection
/// with a FIN behind it.
fn tls_close_then_close_notify(s: &mut TlsTestStream) -> Option<u16> {
    let (texts, status) = tls_messages_close_then_close_notify(s);
    assert!(
        texts.is_empty(),
        "messages before the Close frame: {texts:?}"
    );
    status
}

/// [`tls_close_then_close_notify`], with the text messages the daemon sent
/// before its Close frame.
fn tls_messages_close_then_close_notify(s: &mut TlsTestStream) -> (Vec<String>, Option<u16>) {
    let mut plain = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        // 0: the TCP connection ended, or the `close_notify` has arrived.
        if s.conn.read_tls(&mut s.sock).expect("TLS records") == 0 {
            break;
        }
        s.conn.process_new_packets().expect("valid TLS records");
        loop {
            match s.conn.reader().read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => plain.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => panic!("the TLS session broke: {e}"),
            }
        }
    }
    // rustls answers a read behind the peer's `close_notify` with 0 octets,
    // and one behind a TCP end without it with `UnexpectedEof`.
    match s.conn.reader().read(&mut chunk) {
        Ok(0) => {}
        other => panic!("the TLS session ended without close_notify: {other:?}"),
    }
    // Behind the `close_notify` the daemon ends its sending direction.
    match s.sock.read(&mut chunk) {
        Ok(0) => {}
        other => panic!("expected the FIN behind the close_notify, got {other:?}"),
    }
    let mut receiver = Receiver::new(Role::Client, Limits::default());
    let mut texts = Vec::new();
    loop {
        let event = receiver
            .feed(&mut plain)
            .expect("the daemon's frame is valid");
        match event {
            Some(Event::Message(m)) if m.is_text => {
                texts.push(String::from_utf8(m.payload).expect("UTF-8"));
            }
            Some(Event::Close(body)) => {
                assert!(plain.is_empty(), "octets behind the Close frame: {plain:?}");
                return (texts, body.map(|(code, _)| code));
            }
            other => panic!("expected a Close frame, got {other:?}"),
        }
    }
}

#[test]
fn ws_7_1_the_daemon_ends_a_tls_session_with_close_notify_behind_its_close_frame() {
    // RFC 6455 §7.1.1 over TLS: the Close frame, then the TLS session ends
    // with `close_notify` and the TCP connection with a FIN. A session that
    // ended with the TCP connection alone would read as truncated (RFC 8446
    // §6.1).
    let (cert, key) = gen_self_signed_for("localhost");
    let (cfg, _lease) = make_secure_cfg(&cert, &key, false);
    let mut h = server::start(cfg).expect("start daemon");
    let addr = h.local_addr.clone();
    let client_cfg = build_test_client_config(&cert);
    let masked = |frame: Frame| encode(&frame.with_mask([0x37, 0xfa, 0x21, 0x3d])).expect("encode");
    let mut rsv1_ping = masked(Frame::ping(Vec::new()));
    rsv1_ping[0] |= 0x40;
    let cases: [(&str, Vec<u8>, u16); 3] = [
        // §5.1: an unmasked frame fails the connection.
        (
            "an unmasked frame",
            encode(&Frame::text("{\"op\":\"ping\"}")).expect("encode"),
            1002,
        ),
        // §5.2: a reserved bit no extension defines.
        ("RSV1 on a Ping", rsv1_ping, 1002),
        // §5.5.1: the client's Close is answered with its status code.
        ("a Close", masked(Frame::close(1000, "")), 1000),
    ];
    for (what, wire, status) in cases {
        let mut s = tls_open(&client_cfg, &addr);
        s.write_all(&wire).unwrap();
        assert_eq!(tls_close_then_close_notify(&mut s), Some(status), "{what}");
    }
    // The shutdown: 1001 on an open connection, ended the same way.
    let mut s = tls_open(&client_cfg, &addr);
    let shutdown = std::thread::spawn(move || h.shutdown());
    // §9.2: the `shutdown` message comes before the Close.
    assert_eq!(
        tls_messages_close_then_close_notify(&mut s),
        (vec![r#"{"op":"shutdown"}"#.to_string()], Some(1001))
    );
    drop(s);
    shutdown.join().expect("the shutdown");
}

#[test]
fn wsbridge_7_2_auth_missing_bearer_yields_401() {
    let (cert, key) = gen_self_signed_for("localhost");
    let (cfg, _lease) = make_secure_cfg(&cert, &key, true);
    let h = server::start(cfg).expect("start daemon");
    let addr = h.local_addr.clone();

    let client_cfg = build_test_client_config(&cert);
    let mut s = TlsTestStream::connect(client_cfg, &addr);
    s.write_all(ws_handshake_request("localhost", "/topics/allowed", None).as_bytes())
        .unwrap();
    let resp = read_until_double_crlf(&mut s);
    assert!(resp.contains("401"), "expected 401, got: {resp}");
    drop(h);
}

#[test]
fn ws_10_7_auth_invalid_bearer_yields_401() {
    let (cert, key) = gen_self_signed_for("localhost");
    let (cfg, _lease) = make_secure_cfg(&cert, &key, true);
    let h = server::start(cfg).expect("start daemon");
    let addr = h.local_addr.clone();

    let client_cfg = build_test_client_config(&cert);
    let mut s = TlsTestStream::connect(client_cfg, &addr);
    s.write_all(
        ws_handshake_request("localhost", "/topics/allowed", Some("Bearer wrong")).as_bytes(),
    )
    .unwrap();
    let resp = read_until_double_crlf(&mut s);
    assert!(resp.contains("401"), "expected 401, got: {resp}");
    drop(h);
}

#[test]
fn wsbridge_7_2_auth_valid_bearer_then_acl_allows_subscribed_topic() {
    let (cert, key) = gen_self_signed_for("localhost");
    let (cfg, _lease) = make_secure_cfg(&cert, &key, true);
    let h = server::start(cfg).expect("start daemon");
    let addr = h.local_addr.clone();

    let client_cfg = build_test_client_config(&cert);
    let mut s = TlsTestStream::connect(client_cfg, &addr);
    s.write_all(
        ws_handshake_request("localhost", "/topics/allowed", Some("Bearer secret-tk")).as_bytes(),
    )
    .unwrap();
    let resp = read_until_double_crlf(&mut s);
    assert!(
        resp.contains("101 Switching Protocols"),
        "expected 101, got: {resp}"
    );
    drop(h);
}

#[test]
fn wsbridge_7_3_auth_valid_bearer_but_acl_denies_forbidden_topic() {
    let (cert, key) = gen_self_signed_for("localhost");
    let (cfg, _lease) = make_secure_cfg(&cert, &key, true);
    let h = server::start(cfg).expect("start daemon");
    let addr = h.local_addr.clone();

    let client_cfg = build_test_client_config(&cert);
    let mut s = TlsTestStream::connect(client_cfg, &addr);
    s.write_all(
        ws_handshake_request("localhost", "/topics/forbidden", Some("Bearer secret-tk")).as_bytes(),
    )
    .unwrap();
    let resp = read_until_double_crlf(&mut s);
    assert!(
        resp.contains("403"),
        "expected 403 for ACL deny, got: {resp}"
    );
    drop(h);
}

#[test]
fn wsbridge_7_1_sighup_cert_reload_keeps_daemon_running_with_new_cert() {
    use std::sync::atomic::Ordering;

    let (cert1, key1) = gen_self_signed_for("localhost");
    let cert_path = write_temp("ws_reload_cert.pem", cert1.as_bytes());
    let key_path = write_temp("ws_reload_key.pem", key1.as_bytes());
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.listen = "127.0.0.1:0".into();
    let lease = zerodds_test_domains::lease_domain();
    cfg.domain = lease.id();
    cfg.tls_enabled = true;
    cfg.tls_cert_file = cert_path.to_string_lossy().into();
    cfg.tls_key_file = key_path.to_string_lossy().into();
    cfg.topics.push(TopicConfig {
        name: "T".into(),
        type_name: "T".into(),
        direction: "bidir".into(),
        ws_path: "/topics/t".into(),
        reliability: "reliable".into(),
        durability: "volatile".into(),
        history_depth: 5,
        ..TopicConfig::default()
    });
    let h = server::start(cfg).expect("start");
    let addr = h.local_addr.clone();

    // Pre-Reload: connect with cert1 succeeds.
    let client_cfg1 = build_test_client_config(&cert1);
    let mut s1 = TlsTestStream::connect(client_cfg1, &addr);
    s1.write_all(ws_handshake_request("localhost", "/topics/t", None).as_bytes())
        .unwrap();
    let resp1 = read_until_double_crlf(&mut s1);
    assert!(
        resp1.contains("101"),
        "pre-reload 101 expected, got: {resp1}"
    );

    // Replace cert + key on disk, then trigger reload via reload_flag.
    let (cert2, key2) = gen_self_signed_for("localhost");
    std::fs::write(&cert_path, cert2.as_bytes()).unwrap();
    std::fs::write(&key_path, key2.as_bytes()).unwrap();
    h.reload_flag.store(true, Ordering::SeqCst);
    // Reload-Watcher polls every 250ms; give it generous time.
    std::thread::sleep(Duration::from_millis(800));

    // Post-Reload: only cert2-trusting client succeeds. cert1-trusting
    // client must fail the TLS-handshake.
    let client_cfg2 = build_test_client_config(&cert2);
    let mut s2 = TlsTestStream::connect(client_cfg2, &addr);
    s2.write_all(ws_handshake_request("localhost", "/topics/t", None).as_bytes())
        .unwrap();
    let resp2 = read_until_double_crlf(&mut s2);
    assert!(
        resp2.contains("101"),
        "post-reload 101 expected, got: {resp2}"
    );
    ping_pong(&mut s2);
    // The connection of before the reload keeps its session.
    ping_pong(&mut s1);
    // A client that trusts the old certificate alone fails the handshake.
    let refused = tls_handshake(build_test_client_config(&cert1), &addr);
    assert!(refused.is_err(), "cert1 still served: {refused:?}");

    drop(h);
}

/// §7.1 through the binary: SIGHUP makes the daemon read its certificate
/// files again; new sessions get the new certificate, a session of before
/// keeps its own, the daemon keeps running.
#[cfg(unix)]
#[test]
fn wsbridge_7_1_the_binary_reloads_the_certificate_on_sighup() {
    use std::io::BufRead;
    use std::process::{Command, Stdio};
    struct Reaped(std::process::Child);
    impl Drop for Reaped {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let lease = zerodds_test_domains::lease_domain();
    let (cert1, key1) = gen_self_signed_for("localhost");
    let cert_path = write_temp("ws_bin_cert.pem", cert1.as_bytes());
    let key_path = write_temp("ws_bin_key.pem", key1.as_bytes());
    let mut child = Reaped(
        Command::new(env!("CARGO_BIN_EXE_zerodds-ws-bridged"))
            .args(["--listen", "127.0.0.1:0", "--domain"])
            .arg(lease.id().to_string())
            .arg("--tls-cert")
            .arg(&cert_path)
            .arg("--tls-key")
            .arg(&key_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn"),
    );
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let stdout = child.0.stdout.take().unwrap();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
        {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let wait_for = |what: &str| -> String {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            let line = rx
                .recv_timeout(left)
                .unwrap_or_else(|_| panic!("no line with `{what}`"));
            if line.contains(what) {
                return line;
            }
        }
    };
    // The JSON log of Spec §8.1.
    let running = wait_for("\"event\":\"running\"");
    let running: serde_json::Value = serde_json::from_str(&running).unwrap();
    let addr = running["peer"].as_str().unwrap().to_string();
    let open = |cert: &str| {
        let mut s = TlsTestStream::connect(build_test_client_config(cert), &addr);
        s.write_all(ws_handshake_request("localhost", "/", None).as_bytes())
            .unwrap();
        let resp = read_until_double_crlf(&mut s);
        assert!(resp.contains("101"), "{resp}");
        s
    };
    let mut before = open(&cert1);
    ping_pong(&mut before);

    let (cert2, key2) = gen_self_signed_for("localhost");
    std::fs::write(&cert_path, cert2.as_bytes()).unwrap();
    std::fs::write(&key_path, key2.as_bytes()).unwrap();
    let sent = Command::new("kill")
        .args(["-HUP", &child.0.id().to_string()])
        .status()
        .unwrap();
    assert!(sent.success());
    wait_for("\"event\":\"reload\"");
    let mut after = open(&cert2);
    ping_pong(&mut after);
    ping_pong(&mut before);
    assert!(tls_handshake(build_test_client_config(&cert1), &addr).is_err());
    assert!(child.0.try_wait().unwrap().is_none(), "the daemon ended");
}

/// §4.2 with §7.3: the catalog names the topics its client may read or
/// write, over HTTP and over WebSocket; it needs the credentials a topic
/// path needs.
#[test]
fn wsbridge_4_2_the_catalog_names_the_topics_its_client_may_use() {
    let (cert, key) = gen_self_signed_for("localhost");
    let (cfg, _lease) = make_secure_cfg(&cert, &key, true);
    let h = server::start(cfg).expect("start daemon");
    let addr = h.local_addr.clone();
    let get = |auth: Option<&str>| tls_get(&cert, &addr, "/topics/__catalog__", auth);
    let http = get(Some("Bearer secret-tk"));
    assert!(http.starts_with("HTTP/1.1 200 OK\r\n"), "{http}");
    assert!(http.contains("\"name\":\"Allowed\""), "{http}");
    assert!(!http.contains("Forbidden"), "{http}");
    let refused = get(None);
    assert!(refused.starts_with("HTTP/1.1 401"), "{refused}");
    assert!(!refused.contains("Allowed"), "{refused}");

    let mut ws = TlsTestStream::connect(build_test_client_config(&cert), &addr);
    ws.write_all(
        ws_handshake_request("localhost", "/topics/__catalog__", Some("Bearer secret-tk"))
            .as_bytes(),
    )
    .unwrap();
    let (head, mut rest) = read_head(&mut ws);
    assert!(head.contains("101"), "{head}");
    let frame = read_frame_after(&mut ws, &mut rest);
    let text = std::str::from_utf8(&frame.payload).unwrap();
    assert_eq!(text, &http[http.find("\r\n\r\n").unwrap() + 4..]);
    drop(h);
}

/// §9.2 SIGHUP with a configuration loader: the reloaded ACL decides the
/// next handshake — alice is refused `/topics/forbidden` (403) until the
/// reloaded configuration grants it (101).
#[test]
fn wsbridge_9_2_sighup_applies_the_reloaded_acl() {
    use std::sync::atomic::Ordering;
    let (cert, key) = gen_self_signed_for("localhost");
    let (cfg, _lease) = make_secure_cfg(&cert, &key, true);
    let mut granted = cfg.clone();
    granted.topic_acl.insert(
        "Forbidden".into(),
        (vec!["alice".into()], vec!["alice".into()]),
    );
    let next = Arc::new(std::sync::Mutex::new(granted));
    let n = Arc::clone(&next);
    let h = server::start_with_reload(
        cfg,
        zerodds_dcps::runtime::RuntimeConfig::default(),
        Some(Box::new(move || Ok(n.lock().unwrap().clone()))),
    )
    .expect("start daemon");
    let addr = h.local_addr.clone();
    let handshake = || {
        let mut s = TlsTestStream::connect(build_test_client_config(&cert), &addr);
        s.write_all(
            ws_handshake_request("localhost", "/topics/forbidden", Some("Bearer secret-tk"))
                .as_bytes(),
        )
        .unwrap();
        read_until_double_crlf(&mut s)
    };
    assert!(handshake().contains("403"), "before the reload");
    h.reload_flag.store(true, Ordering::SeqCst);
    std::thread::sleep(std::time::Duration::from_millis(400));
    let resp = handshake();
    assert!(resp.contains("101"), "after the reload: {resp}");
    drop(h);
}

/// An RSA key from `openssl genpkey`: PKCS#8 DER and the SPKI PEM.
fn rsa_key() -> (Vec<u8>, String) {
    let key = write_temp("jwt.key", b"");
    let k = key.to_str().unwrap();
    let run = |args: &[&str]| {
        let out = std::process::Command::new("openssl")
            .args(args)
            .output()
            .expect("openssl on PATH: the JWT test needs a real RSA key");
        assert!(out.status.success(), "openssl {args:?}: {out:?}");
        out.stdout
    };
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

/// A daemon over TLS with `auth.mode: jwt`, audience `zerodds-ws` and
/// the required claim `scope=dds.read`, its public key in the temporary
/// file `pub_name`. Returns the certificate, the PKCS#8 key that signs the
/// tokens, the lease of the daemon's domain and the daemon.
fn jwt_daemon(
    pub_name: &str,
) -> (
    String,
    Vec<u8>,
    zerodds_test_domains::DomainLease,
    server::DaemonHandle,
) {
    let (cert, key) = gen_self_signed_for("localhost");
    let (pkcs8, pem) = rsa_key();
    let pub_path = write_temp(pub_name, pem.as_bytes());
    let (mut cfg, lease) = make_secure_cfg(&cert, &key, true);
    cfg.auth_mode = "jwt".into();
    cfg.auth_jwt = zerodds_bridge_security::JwtSettings {
        public_key: pub_path.to_string_lossy().into(),
        issuer: None,
        audience: Some("zerodds-ws".into()),
        required_claim: Some("scope=dds.read".into()),
    };
    let h = server::start_with_reload(cfg, zerodds_dcps::runtime::RuntimeConfig::default(), None)
        .expect("start daemon");
    (cert, pkcs8, lease, h)
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

/// §7.2 `jwt` with Spec §3's `auth.jwt` block: the `sub` claim is the ACL
/// subject; an expired token is 401, a missing scope 403. `auth.mode: jwt`
/// could be chosen before but had no key setting, and the start failed.
#[test]
fn wsbridge_7_2_a_jwt_is_checked_and_its_subject_meets_the_acl() {
    let (cert, pkcs8, _lease, h) = jwt_daemon("jwt.pub");
    let addr = h.local_addr.clone();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let token = |exp: u64, scope: &str| {
        jwt(
            &pkcs8,
            &format!(r#"{{"sub":"alice","aud":"zerodds-ws","scope":"{scope}","exp":{exp}}}"#),
        )
    };
    let status = |path: &str, auth: &str| {
        let mut s = TlsTestStream::connect(build_test_client_config(&cert), &addr);
        s.write_all(ws_handshake_request("localhost", path, Some(auth)).as_bytes())
            .unwrap();
        let resp = read_until_double_crlf(&mut s);
        resp.split(' ').nth(1).unwrap_or("").to_string()
    };
    let good = token(now + 600, "dds.read");
    assert_eq!(status("/topics/allowed", &good), "101");
    assert_eq!(status("/topics/forbidden", &good), "403");
    assert_eq!(
        status("/topics/allowed", &token(now - 3600, "dds.read")),
        "401"
    );
    // §4.1: a token that verifies and lacks the scope is 403.
    assert_eq!(
        status("/topics/allowed", &token(now + 600, "dds.write")),
        "403"
    );
    drop(h);
}

/// §4.1: 401 on failed auth, 403 on missing scope. A token that verifies
/// (signature, time, audience) and lacks `auth.jwt.required_claim` is 403
/// with `insufficient_scope` (RFC 6750 §3.1), on a topic path and on any
/// other; a token that does not verify is 401 whatever scope it names.
#[test]
fn wsbridge_4_1_a_token_without_the_required_claim_is_403() {
    let (cert, pkcs8, _lease, h) = jwt_daemon("jwt-scope.pub");
    let addr = h.local_addr.clone();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let token = |exp: u64, claims: &str| {
        jwt(
            &pkcs8,
            &format!(r#"{{"sub":"alice","aud":"zerodds-ws",{claims}"exp":{exp}}}"#),
        )
    };
    let response = |path: &str, auth: Option<&str>| {
        let mut s = TlsTestStream::connect(build_test_client_config(&cert), &addr);
        s.write_all(ws_handshake_request("localhost", path, auth).as_bytes())
            .unwrap();
        read_until_double_crlf(&mut s)
    };
    let status = |resp: &str| resp.split(' ').nth(1).unwrap_or("").to_string();

    for path in ["/topics/allowed", "/"] {
        // Another scope, and no scope claim at all.
        for claims in [r#""scope":"dds.write","#, ""] {
            let resp = response(path, Some(&token(now + 600, claims)));
            assert_eq!(status(&resp), "403", "{path} {claims}: {resp}");
            assert!(
                resp.contains("error=\"insufficient_scope\"")
                    && resp.contains("scope=\"dds.read\""),
                "{resp}"
            );
            assert!(!resp.contains("Sec-WebSocket-Accept"), "{resp}");
        }
        // The scope alone, and in a list.
        for claims in [r#""scope":"dds.read","#, r#""scope":"dds.write dds.read","#] {
            let resp = response(path, Some(&token(now + 600, claims)));
            assert_eq!(status(&resp), "101", "{path} {claims}: {resp}");
        }
    }
    // Not authenticated: expired with the right scope, no token.
    let expired = response(
        "/topics/allowed",
        Some(&token(now - 3600, r#""scope":"dds.read","#)),
    );
    assert_eq!(status(&expired), "401", "{expired}");
    assert!(expired.contains("error=\"invalid_token\""), "{expired}");
    let none = response("/topics/allowed", None);
    assert_eq!(status(&none), "401", "{none}");
    assert!(none.contains("WWW-Authenticate: Bearer"), "{none}");
    // §8.2: the four refusals for the claim count as `scope`.
    let metrics = h.metrics.clone().expect("metrics");
    let count = |reason: &str| {
        metrics.value(
            zerodds_websocket_bridge::daemon::runtime_common::family::AUTH_FAILURES,
            &[("reason", reason)],
        )
    };
    assert_eq!(count("scope"), 4);
    assert_eq!(count("rejected"), 1);
    assert_eq!(count("missing"), 1);
    drop(h);
}

/// An EC key on P-256 from `openssl genpkey`: PKCS#8 DER and the SPKI PEM.
fn ec_key() -> (Vec<u8>, String) {
    let key = write_temp("jwt-ec.key", b"");
    let k = key.to_str().unwrap();
    let run = |args: &[&str]| {
        let out = std::process::Command::new("openssl")
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
    let der = run(&["pkcs8", "-topk8", "-nocrypt", "-outform", "DER", "-in", k]);
    let pem = run(&["pkey", "-pubout", "-in", k]);
    (der, String::from_utf8(pem).unwrap())
}

/// An ES256 token (RFC 7518 §3.4: the signature is `R || S`, 64 octets).
fn jwt_es256(pkcs8: &[u8], payload: &str) -> String {
    use base64::Engine as _;
    let b64 = |b: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b);
    let signed = format!(
        "{}.{}",
        b64(br#"{"alg":"ES256","typ":"JWT"}"#),
        b64(payload.as_bytes())
    );
    let rng = ring::rand::SystemRandom::new();
    let pair = ring::signature::EcdsaKeyPair::from_pkcs8(
        &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
        pkcs8,
        &rng,
    )
    .unwrap();
    let sig = pair.sign(&rng, signed.as_bytes()).unwrap();
    format!("Bearer {signed}.{}", b64(sig.as_ref()))
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// The status of a handshake on `path` over TLS, without a panic: the
/// status code of the response, or `refused: <why>` when the TLS session
/// or the connection ended first.
fn tls_status(client_cfg: Arc<ClientConfig>, addr: &str, path: &str, auth: Option<&str>) -> String {
    let name: ServerName<'static> = ServerName::try_from("localhost".to_string()).unwrap();
    let mut conn = rustls::ClientConnection::new(client_cfg, name).unwrap();
    let Ok(mut sock) = TcpStream::connect(addr) else {
        return "refused: connect".into();
    };
    sock.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let mut tls = rustls::Stream::new(&mut conn, &mut sock);
    if let Err(e) = tls.write_all(ws_handshake_request("localhost", path, auth).as_bytes()) {
        return format!("refused: {e}");
    }
    let mut acc = Vec::new();
    let mut buf = [0u8; 4096];
    while !acc.windows(4).any(|w| w == b"\r\n\r\n") {
        match tls.read(&mut buf) {
            Ok(0) => return "refused: eof".into(),
            Ok(n) => acc.extend_from_slice(&buf[..n]),
            Err(e) => return format!("refused: {e}"),
        }
    }
    String::from_utf8_lossy(&acc)
        .split(' ')
        .nth(1)
        .unwrap_or("")
        .to_string()
}

/// §7.2 `jwt` with an ES256 key (ECDSA P-256, RFC 7518 §3.4): the key file
/// is a P-256 SPKI PEM, a token it signed opens the topic its `sub` may
/// read, an expired one and an RS256 token are 401.
#[test]
fn wsbridge_7_2_an_es256_jwt_is_checked() {
    let (cert, key) = gen_self_signed_for("localhost");
    let (pkcs8, pem) = ec_key();
    let pub_path = write_temp("jwt-es256.pub", pem.as_bytes());
    let (mut cfg, _lease) = make_secure_cfg(&cert, &key, true);
    cfg.auth_mode = "jwt".into();
    cfg.auth_jwt = zerodds_bridge_security::JwtSettings {
        public_key: pub_path.to_string_lossy().into(),
        issuer: Some("https://idp.example.org".into()),
        audience: Some("zerodds-ws".into()),
        required_claim: None,
    };
    let h = server::start_with_reload(cfg, zerodds_dcps::runtime::RuntimeConfig::default(), None)
        .expect("start daemon");
    let addr = h.local_addr.clone();
    let now = unix_now();
    let claims = |exp: u64| {
        format!(
            r#"{{"sub":"alice","iss":"https://idp.example.org","aud":"zerodds-ws","exp":{exp}}}"#
        )
    };
    let status = |path: &str, auth: &str| {
        tls_status(build_test_client_config(&cert), &addr, path, Some(auth))
    };
    let good = jwt_es256(&pkcs8, &claims(now + 600));
    assert_eq!(status("/topics/allowed", &good), "101");
    assert_eq!(status("/topics/forbidden", &good), "403");
    assert_eq!(
        status("/topics/allowed", &jwt_es256(&pkcs8, &claims(now - 3600))),
        "401"
    );
    let (other, _) = ec_key();
    assert_eq!(
        status("/topics/allowed", &jwt_es256(&other, &claims(now + 600))),
        "401"
    );
    // An RS256 token does not fit the ES256 key.
    let (rsa, _) = rsa_key();
    assert_eq!(
        status("/topics/allowed", &jwt(&rsa, &claims(now + 600))),
        "401"
    );
    drop(h);
}

/// §7.2: `auth.mode: none` is served on a loopback address only. A
/// configuration with `none` on `0.0.0.0` does not start (exit 1, Spec §2),
/// and a SIGHUP reload to `none` on such an address changes nothing: the
/// daemon keeps asking for the bearer token.
#[test]
fn wsbridge_7_2_none_is_refused_off_loopback() {
    use std::sync::atomic::Ordering;
    let lease = zerodds_test_domains::lease_domain();
    let mut open = DaemonConfig::default_for_dev();
    open.domain = lease.id();
    open.auth_mode = "none".into();
    for listen in ["0.0.0.0:0", "[::]:0"] {
        open.listen = listen.into();
        match server::start_with_reload(
            open.clone(),
            zerodds_dcps::runtime::RuntimeConfig::default(),
            None,
        ) {
            Err(server::ServerError::Config(m)) => {
                assert!(m.contains("loopback"), "{listen}: {m}");
            }
            Err(e) => panic!("{listen}: {e}"),
            Ok(_) => panic!("{listen}: started with auth.mode none"),
        }
    }
    // A reload that asks for `none` on the wildcard address is refused.
    let (cert, key) = gen_self_signed_for("localhost");
    let (mut cfg, _lease) = make_secure_cfg(&cert, &key, true);
    cfg.listen = "0.0.0.0:0".into();
    let mut anonymous = cfg.clone();
    anonymous.auth_mode = "none".into();
    // The second reload also moves `listen` to loopback; `listen` waits for
    // a restart, the daemon still serves 0.0.0.0, so `none` stays refused.
    let mut moved = anonymous.clone();
    moved.listen = "127.0.0.1:0".into();
    let next = Arc::new(std::sync::Mutex::new(anonymous));
    let n = Arc::clone(&next);
    let h = server::start_with_reload(
        cfg,
        zerodds_dcps::runtime::RuntimeConfig::default(),
        Some(Box::new(move || Ok(n.lock().unwrap().clone()))),
    )
    .expect("start daemon with bearer on 0.0.0.0");
    let port = h.local_addr.rsplit_once(':').unwrap().1.to_string();
    let addr = format!("127.0.0.1:{port}");
    let status = || {
        tls_status(
            build_test_client_config(&cert),
            &addr,
            "/topics/allowed",
            None,
        )
    };
    assert_eq!(status(), "401", "before the reload");
    h.reload_flag.store(true, Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(status(), "401", "after the refused reload");
    *next.lock().unwrap() = moved;
    h.reload_flag.store(true, Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(
        status(),
        "401",
        "after a reload that moves listen to loopback"
    );
    drop(h);
}

/// §7.2 `mtls` and §7.3: the subject DN of the client certificate (RFC 4514)
/// is the identity the ACL names. A certificate of the configured CA with
/// the DN in the ACL opens the topic (101), one with another DN is 403, a
/// client without a certificate gets no session.
#[test]
fn wsbridge_7_2_a_client_certificate_subject_meets_the_acl() {
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
    let ca_key = rcgen::KeyPair::generate().unwrap();
    let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    ca_params.distinguished_name = rcgen::DistinguishedName::new();
    ca_params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "zerodds test clients");
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let client = |cn: &str| {
        let k = rcgen::KeyPair::generate().unwrap();
        let mut p = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        p.distinguished_name = rcgen::DistinguishedName::new();
        p.distinguished_name.push(rcgen::DnType::CommonName, cn);
        p.distinguished_name
            .push(rcgen::DnType::OrganizationName, "Example");
        p.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ClientAuth];
        let c = p.signed_by(&k, &ca, &ca_key).unwrap();
        (c.der().clone(), k.serialize_der())
    };
    let (cert, key) = gen_self_signed_for("localhost");
    let (mut cfg, _lease) = make_secure_cfg(&cert, &key, false);
    cfg.tls_client_ca_file = write_temp("clients-ca.pem", ca.pem().as_bytes())
        .to_string_lossy()
        .into();
    cfg.auth_mode = "mtls".into();
    // RFC 4514 §2.1: the last RDN of the encoding comes first.
    cfg.topic_acl.insert(
        "Allowed".into(),
        (vec!["O=Example,CN=alice".into()], vec![]),
    );
    let h = server::start_with_reload(cfg, zerodds_dcps::runtime::RuntimeConfig::default(), None)
        .expect("start daemon");
    let addr = h.local_addr.clone();
    let with_cert = |cn: &str| {
        let (der, pkcs8): (CertificateDer<'static>, Vec<u8>) = client(cn);
        let mut roots = rustls::RootCertStore::empty();
        for c in load_certs_for_root(&cert) {
            roots.add(c).unwrap();
        }
        let cfg =
            ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_root_certificates(roots)
                .with_client_auth_cert(
                    vec![der],
                    PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(pkcs8)),
                )
                .unwrap();
        tls_status(Arc::new(cfg), &addr, "/topics/allowed", None)
    };
    assert_eq!(with_cert("alice"), "101");
    assert_eq!(with_cert("mallory"), "403");
    let without = tls_status(
        build_test_client_config(&cert),
        &addr,
        "/topics/allowed",
        None,
    );
    assert!(without.starts_with("refused"), "{without}");
    drop(h);
}

/// §7.3 `*group:<n>*`: a JWT whose `groups` claim names the group meets
/// the rule (101), a token of the same subject without it does not (403).
#[test]
fn wsbridge_7_3_a_jwt_group_meets_a_group_rule() {
    let (cert, key) = gen_self_signed_for("localhost");
    let (pkcs8, pem) = rsa_key();
    let pub_path = write_temp("jwt-groups.pub", pem.as_bytes());
    let (mut cfg, _lease) = make_secure_cfg(&cert, &key, true);
    cfg.auth_mode = "jwt".into();
    cfg.auth_jwt = zerodds_bridge_security::JwtSettings {
        public_key: pub_path.to_string_lossy().into(),
        ..Default::default()
    };
    cfg.topic_acl.insert(
        "Forbidden".into(),
        (vec!["*group:engineers*".into()], vec![]),
    );
    let h = server::start_with_reload(cfg, zerodds_dcps::runtime::RuntimeConfig::default(), None)
        .expect("start daemon");
    let addr = h.local_addr.clone();
    let exp = unix_now() + 600;
    let status = |groups: &str| {
        let token = jwt(
            &pkcs8,
            &format!(r#"{{"sub":"carol","groups":{groups},"exp":{exp}}}"#),
        );
        tls_status(
            build_test_client_config(&cert),
            &addr,
            "/topics/forbidden",
            Some(&token),
        )
    };
    assert_eq!(status(r#"["ops","engineers"]"#), "101");
    assert_eq!(status(r#"["ops"]"#), "403");
    assert_eq!(status("[]"), "403");
    drop(h);
}

/// §8.2 `auth_failures_total{reason}`: a handshake without a token is
/// `missing`, an unknown token `rejected`, a topic the ACL does not grant
/// `acl`, and so is a `subscribe`, a JSON sample frame and a binary sample
/// frame the ACL refuses on an open connection. An ACL refusal is no error
/// of `errors_total`.
#[test]
fn wsbridge_8_2_a_refused_token_counts_as_auth_failure() {
    use zerodds_websocket_bridge::daemon::runtime_common::{auth_reason, family};
    let (cert, key) = gen_self_signed_for("localhost");
    let (mut cfg, _lease) = make_secure_cfg(&cert, &key, true);
    // alice reads `Allowed` but does not write it.
    cfg.topic_acl
        .insert("Allowed".into(), (vec!["alice".into()], vec!["bob".into()]));
    let h = server::start(cfg).expect("start daemon");
    let addr = h.local_addr.clone();
    let metrics = h.metrics.clone().expect("metrics");
    let status = |path: &str, auth: Option<&str>| {
        tls_status(build_test_client_config(&cert), &addr, path, auth)
    };
    let count = |reason: &str| metrics.value(family::AUTH_FAILURES, &[("reason", reason)]);
    assert_eq!(status("/topics/allowed", None), "401");
    assert_eq!(status("/topics/allowed", Some("Bearer wrong")), "401");
    assert_eq!(status("/topics/allowed", Some("Bearer wrong")), "401");
    assert_eq!(status("/topics/forbidden", Some("Bearer secret-tk")), "403");
    assert_eq!(count(auth_reason::MISSING), 1);
    assert_eq!(count(auth_reason::REJECTED), 2);
    assert_eq!(count(auth_reason::ACL), 1);
    // A subscribe the ACL refuses: an error frame 403, counted as `acl`.
    let mut s = TlsTestStream::connect(build_test_client_config(&cert), &addr);
    s.write_all(
        ws_handshake_request("localhost", "/topics/allowed", Some("Bearer secret-tk")).as_bytes(),
    )
    .unwrap();
    let (head, mut acc) = read_head(&mut s);
    assert!(head.contains("101"), "{head}");
    let sub = encode(
        &Frame::text(r#"{"op":"subscribe","topics":["Forbidden"]}"#).with_mask([1, 2, 3, 4]),
    )
    .unwrap();
    s.write_all(&sub).unwrap();
    let error = read_frame_after(&mut s, &mut acc);
    let text = String::from_utf8(error.payload).unwrap();
    assert!(text.contains("403"), "{text}");
    assert_eq!(count(auth_reason::ACL), 2);
    // A JSON sample frame of a topic alice does not write.
    let publish = encode(
        &Frame::text(r#"{"op":"publish","topic":"Allowed","data":{}}"#).with_mask([1, 2, 3, 4]),
    )
    .unwrap();
    s.write_all(&publish).unwrap();
    // The `subscribed` of the refused subscribe comes first.
    let error_after = |s: &mut TlsTestStream, acc: &mut Vec<u8>| loop {
        let frame = read_frame_after(s, acc);
        let text = String::from_utf8(frame.payload).unwrap();
        if text.contains("\"op\":\"error\"") {
            break text;
        }
    };
    let text = error_after(&mut s, &mut acc);
    assert!(text.contains("403"), "{text}");
    assert_eq!(count(auth_reason::ACL), 3);
    // A binary sample frame on the topic of the path, XCDR2 little endian.
    let mut sample = b"ZDB1".to_vec();
    sample.extend_from_slice(&[0, 0x00, 0x07, 0, 0, 1, 0, 0, 0]);
    let binary = encode(&Frame::binary(sample).with_mask([1, 2, 3, 4])).unwrap();
    s.write_all(&binary).unwrap();
    let text = error_after(&mut s, &mut acc);
    assert!(text.contains("403"), "{text}");
    assert_eq!(count(auth_reason::ACL), 4);
    assert_eq!(count(auth_reason::SCOPE), 0);
    assert_eq!(metrics.total(family::AUTH_FAILURES), 7);
    assert_eq!(metrics.errors_total.get(), 0, "an ACL refusal is no error");
    drop(h);
}

/// §8.2 `connections_total` counts the accepted TCP connections: one whose
/// TLS handshake fails is counted too (and in `errors_total`), and it is
/// never an open connection.
#[test]
fn wsbridge_8_2_a_failed_tls_handshake_counts_as_an_accepted_connection() {
    let (cert, key) = gen_self_signed_for("localhost");
    let (cfg, _lease) = make_secure_cfg(&cert, &key, true);
    let h = server::start(cfg).expect("start daemon");
    let metrics = h.metrics.clone().expect("metrics");
    // Plain HTTP on the TLS listener: the TLS handshake fails.
    let mut plain = TcpStream::connect(&h.local_addr).unwrap();
    plain
        .write_all(b"GET /topics/allowed HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();
    plain
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut rest = Vec::new();
    let _ = plain.read_to_end(&mut rest);
    let until = std::time::Instant::now() + Duration::from_secs(5);
    while metrics.errors_total.get() == 0 && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(metrics.errors_total.get(), 1);
    assert_eq!(metrics.connections_total.get(), 1);
    assert_eq!(metrics.connections_active.get(), 0);
    // A connection that completes TLS and the handshake is the second.
    let status = tls_status(
        build_test_client_config(&cert),
        &h.local_addr,
        "/topics/allowed",
        Some("Bearer secret-tk"),
    );
    assert_eq!(status, "101");
    assert_eq!(metrics.connections_total.get(), 2);
    drop(h);
}

/// A mode that cannot be served stops the start as a configuration error
/// (Spec §2 exit 1), not as a bind error; `bearer` without a token too.
#[test]
fn wsbridge_7_2_jwt_without_a_key_and_mtls_without_a_client_ca_are_config_errors() {
    let (cert, key) = gen_self_signed_for("localhost");
    for mode in ["jwt", "mtls", "bearer"] {
        let (mut cfg, _lease) = make_secure_cfg(&cert, &key, false);
        cfg.auth_mode = mode.into();
        let r =
            server::start_with_reload(cfg, zerodds_dcps::runtime::RuntimeConfig::default(), None);
        match r {
            Err(server::ServerError::Config(_)) => {}
            Err(e) => panic!("{mode}: {e}"),
            Ok(_) => panic!("{mode}: started"),
        }
    }
}

/// The next frame on `s` within `within`, beginning with the octets in
/// `acc`; `None` when none came.
fn frame_within(s: &mut TlsTestStream, acc: &mut Vec<u8>, within: Duration) -> Option<Frame> {
    let deadline = std::time::Instant::now() + within;
    s.sock
        .set_read_timeout(Some(Duration::from_millis(50)))
        .unwrap();
    let mut buf = [0u8; 4096];
    loop {
        if let Ok((frame, used)) = decode(acc) {
            acc.drain(..used);
            return Some(frame);
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        match s.read(&mut buf) {
            Ok(0) => return None,
            Ok(n) => acc.extend_from_slice(&buf[..n]),
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => panic!("read: {e}"),
        }
    }
}

/// §6 with §7.3: `deadline_missed` names its topic and an instance, so it
/// reaches a client only while the read ACL grants the topic. After a
/// SIGHUP reload that withdraws alice's read right, her connection gets no
/// more `deadline_missed` frames, though the deadline keeps passing.
#[test]
fn wsbridge_6_deadline_missed_passes_the_read_acl() {
    use std::sync::atomic::Ordering;
    let (cert, key) = gen_self_signed_for("localhost");
    let (mut cfg, _lease) = make_secure_cfg(&cert, &key, true);
    cfg.topics[0].deadline_ms = Some(200);
    let mut revoked = cfg.clone();
    revoked
        .topic_acl
        .insert("Allowed".into(), (vec!["bob".into()], vec!["alice".into()]));
    let next = Arc::new(std::sync::Mutex::new(revoked));
    let n = Arc::clone(&next);
    let h = server::start_with_reload(
        cfg,
        zerodds_dcps::runtime::RuntimeConfig::default(),
        Some(Box::new(move || Ok(n.lock().unwrap().clone()))),
    )
    .expect("start daemon");
    let mut s = TlsTestStream::connect(build_test_client_config(&cert), &h.local_addr);
    s.write_all(
        ws_handshake_request("localhost", "/topics/allowed", Some("Bearer secret-tk")).as_bytes(),
    )
    .unwrap();
    let (head, mut acc) = read_head(&mut s);
    assert!(head.contains("101"), "{head}");
    let reader = *h.user_readers.get("Allowed").unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert!(h.runtime.test_inject_user_alive(reader, vec![1, 0, 0, 0]));
    let mut texts = Vec::new();
    while texts.is_empty() {
        let frame = frame_within(&mut s, &mut acc, Duration::from_secs(2))
            .expect("the sample, then deadline_missed");
        if let Ok(text) = String::from_utf8(frame.payload) {
            if text.contains("deadline_missed") {
                texts.push(text);
            }
        }
    }
    assert!(texts[0].contains("\"topic\":\"Allowed\""), "{texts:?}");
    h.reload_flag.store(true, Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(400));
    // What was queued before the reload took effect.
    while frame_within(&mut s, &mut acc, Duration::from_millis(100)).is_some() {}
    // The deadline passes every 200 ms; none of it reaches alice.
    let after = frame_within(&mut s, &mut acc, Duration::from_millis(1_000));
    assert!(after.is_none(), "{after:?}");
    drop(h);
}

/// A plain GET of `path` over TLS with `auth`: the whole response, read by
/// its `Content-Length` (the reader of the test stream does not end at the
/// end of the session).
fn tls_get(cert: &str, addr: &str, path: &str, auth: Option<&str>) -> String {
    let mut s = TlsTestStream::connect(build_test_client_config(cert), addr);
    let mut req = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n");
    if let Some(a) = auth {
        req.push_str(&format!("Authorization: {a}\r\n"));
    }
    req.push_str("\r\n");
    s.write_all(req.as_bytes()).unwrap();
    let mut out = read_until_double_crlf(&mut s).into_bytes();
    let head_len = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
    let head = String::from_utf8_lossy(&out[..head_len]).into_owned();
    let body_len: usize = head
        .lines()
        .find_map(|l| l.strip_prefix("Content-Length: "))
        .expect("Content-Length")
        .trim()
        .parse()
        .unwrap();
    let mut buf = [0u8; 4096];
    while out.len() < head_len + body_len {
        let n = s.read(&mut buf).expect("the body");
        assert!(n > 0, "the body ended early");
        out.extend_from_slice(&buf[..n]);
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// §5.2 with §7.2, §7.3: `/schema/<slug>` needs the credentials a topic
/// path needs (401 without) and a read or write grant of the topic (403
/// without); with both it is the topic's IDL.
#[test]
fn wsbridge_5_2_the_schema_needs_the_credentials_and_a_grant() {
    let (cert, key) = gen_self_signed_for("localhost");
    let (mut cfg, _lease) = make_secure_cfg(&cert, &key, true);
    let idl = write_temp(
        "ws_schema_acl.idl",
        b"struct Allowed { long x; };\nstruct Forbidden { long y; };\n",
    );
    for t in &mut cfg.topics {
        t.idl_file = idl.to_string_lossy().into();
    }
    let h = server::start(cfg).expect("start daemon");
    let addr = h.local_addr.clone();
    let ok = tls_get(
        &cert,
        &addr,
        "/schema/allowed.idl",
        Some("Bearer secret-tk"),
    );
    assert!(ok.starts_with("HTTP/1.1 200 OK\r\n"), "{ok}");
    assert!(
        ok.ends_with("struct Allowed { long x; };\nstruct Forbidden { long y; };\n"),
        "{ok}"
    );
    let missing = tls_get(&cert, &addr, "/schema/allowed.idl", None);
    assert!(missing.starts_with("HTTP/1.1 401"), "{missing}");
    assert!(!missing.contains("struct"), "{missing}");
    let forbidden = tls_get(&cert, &addr, "/schema/forbidden", Some("Bearer secret-tk"));
    assert!(forbidden.starts_with("HTTP/1.1 403"), "{forbidden}");
    assert!(!forbidden.contains("struct"), "{forbidden}");
    drop(h);
}

/// A `Chat` sample as XCDR2 little endian: the string's length with its
/// NUL, the characters and the NUL, padding to four, the long.
fn chat_sample(text: &str, seq: i32) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&u32::try_from(text.len() + 1).unwrap().to_le_bytes());
    out.extend_from_slice(text.as_bytes());
    out.push(0);
    while out.len() % 4 != 0 {
        out.push(0);
    }
    out.extend_from_slice(&seq.to_le_bytes());
    out
}

/// A DDS participant of the test on `domain` with a reliable DataReader
/// and DataWriter of the topic `Chat`, matched with the daemon's.
struct ChatPeer {
    dds: std::sync::Arc<zerodds_dcps::runtime::DcpsRuntime>,
    writer: zerodds_rtps::wire_types::EntityId,
    samples: std::sync::mpsc::Receiver<zerodds_dcps::runtime::UserSample>,
}

impl ChatPeer {
    fn start(domain: i32, tag: u8) -> Self {
        use zerodds_dcps::runtime::{
            DcpsRuntime, RuntimeConfig, UserReaderConfig, UserWriterConfig,
        };
        use zerodds_rtps::publication_data::data_representation::XCDR2;
        let mut prefix = [0u8; 12];
        prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
        prefix[4..8].copy_from_slice(&domain.to_be_bytes());
        prefix[11] = tag;
        let dds = DcpsRuntime::start(
            domain,
            zerodds_rtps::wire_types::GuidPrefix::from_bytes(prefix),
            RuntimeConfig::default(),
        )
        .expect("the test's participant");
        let (reader, samples) = dds
            .register_user_reader_kind(
                UserReaderConfig {
                    topic_name: "Chat".into(),
                    type_name: "Chat".into(),
                    reliable: true,
                    ..Default::default()
                },
                false,
            )
            .expect("reader");
        let writer = dds
            .register_user_writer_kind(
                UserWriterConfig {
                    topic_name: "Chat".into(),
                    type_name: "Chat".into(),
                    reliable: true,
                    data_representation_offer: Some(vec![XCDR2]),
                    ..Default::default()
                },
                false,
            )
            .expect("writer");
        dds.set_user_writer_representation_identifier(writer, [0x00, 0x07])
            .expect("XCDR2 little endian");
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while dds.user_reader_matched_count(reader) == 0
            || dds.user_writer_matched_count(writer) == 0
        {
            assert!(
                std::time::Instant::now() < deadline,
                "the daemon's endpoints on domain {domain} never matched the test's"
            );
            dds.wait_match_event(Duration::from_millis(200));
        }
        Self {
            dds,
            writer,
            samples,
        }
    }

    /// The payloads this participant's reader received within `within`.
    fn received(&self, within: Duration) -> Vec<Vec<u8>> {
        let deadline = std::time::Instant::now() + within;
        let mut out = Vec::new();
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match self.samples.recv_timeout(left) {
                Ok(zerodds_dcps::runtime::UserSample::Alive { payload, .. }) => {
                    out.push(payload.to_vec());
                }
                Ok(_) => {}
                Err(_) => return out,
            }
        }
    }
}

/// A binary-mode client (§4.3) of `path` with the token `auth`: the
/// stream, the response head and the octets behind it.
fn binary_client(addr: &str, path: &str, auth: &str) -> (TcpStream, String, Vec<u8>) {
    let mut s = TcpStream::connect(addr).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    s.write_all(ws_handshake_request("localhost", path, Some(auth)).as_bytes())
        .unwrap();
    let (head, acc) = read_head(&mut s);
    (s, head, acc)
}

/// The payloads of the binary frames the daemon sends on `s` within
/// `within`; a Ping is answered.
fn binary_frames_within(s: &mut TcpStream, acc: &mut Vec<u8>, within: Duration) -> Vec<Vec<u8>> {
    use zerodds_websocket_bridge::frame::Opcode;
    let deadline = std::time::Instant::now() + within;
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    s.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
    loop {
        while let Ok((frame, used)) = decode(acc) {
            acc.drain(..used);
            match frame.opcode {
                Opcode::Binary => out.push(frame.payload),
                Opcode::Ping => {
                    let mut pong = Frame::binary(frame.payload).with_mask([3, 3, 3, 3]);
                    pong.opcode = Opcode::Pong;
                    s.write_all(&encode(&pong).unwrap()).unwrap();
                }
                _ => {}
            }
        }
        if std::time::Instant::now() >= deadline {
            return out;
        }
        match s.read(&mut buf) {
            Ok(0) => return out,
            Ok(n) => acc.extend_from_slice(&buf[..n]),
            Err(_) => {}
        }
    }
}

/// Spec §1 L6, §3 `tenants`: a connection whose JWT subject a tenant names
/// is bound to the tenant's participant on its own domain for its life.
/// The top-level topic and the tenant's topic are both `Chat`, on two
/// domains, each with its own ACL: alice (tenant `plant-a`) publishes into
/// the tenant's domain only and gets its samples only; carol, whom no
/// tenant names, uses the participant of `domain`; alice's catalog names
/// the tenant's topics; the metrics count alice's sample under
/// `plant-a/Chat`. Alice's 101 on `/topics/chat` is the tenant's ACL:
/// the top-level `Chat` grants carol alone, and bob, whom no tenant names,
/// gets 403 there.
#[test]
fn wsbridge_1_l6_a_connection_is_bound_to_the_tenant_of_its_subject() {
    use zerodds_websocket_bridge::daemon::runtime_common::family;
    let (pkcs8, pem) = rsa_key();
    let pub_path = write_temp("jwt-tenants.pub", pem.as_bytes());
    // `@final`: the samples of the test are PLAIN_CDR2 (`00 07`), the one
    // XCDR2 format the daemon's writer of a final type offers (§4.3).
    let idl = write_temp(
        "chat.idl",
        b"@final struct Chat { string text; long seq; };\nstruct Status { long code; };\n",
    );
    let base = zerodds_test_domains::lease_domain();
    let other = zerodds_test_domains::lease_domain();
    let yaml = format!(
        "listen: \"127.0.0.1:0\"\ndomain: {base}\nauth:\n  mode: jwt\n  jwt:\n    public_key: \"{key}\"\ntopics:\n  - name: Chat\n    idl_file: \"{idl}\"\n    qos:\n      reliability: reliable\n    acl:\n      read: [carol]\n      write: [carol]\ntenants:\n  - name: plant-a\n    domain: {other}\n    subjects: [alice]\n    topics:\n      - name: Chat\n        idl_file: \"{idl}\"\n        qos:\n          reliability: reliable\n        acl:\n          read: [alice]\n          write: [alice]\n      - name: Plant::Status\n        type: Status\n        idl_file: \"{idl}\"\n        direction: out\n        acl:\n          read: [alice]\n",
        base = base.id(),
        other = other.id(),
        key = pub_path.display(),
        idl = idl.display(),
    );
    let cfg = DaemonConfig::load_from_str(&yaml).expect("the configuration");
    let h = server::start_with_reload(cfg, zerodds_dcps::runtime::RuntimeConfig::default(), None)
        .expect("start daemon");
    assert!(h.tenant_runtimes.contains_key("plant-a"));
    let addr = h.local_addr.clone();
    let exp = unix_now() + 600;
    let token = |sub: &str| jwt(&pkcs8, &format!(r#"{{"sub":"{sub}","exp":{exp}}}"#));
    let peer_base = ChatPeer::start(base.id(), 0x31);
    let peer_other = ChatPeer::start(other.id(), 0x32);

    let (mut alice, head, mut alice_acc) = binary_client(&addr, "/topics/chat", &token("alice"));
    assert!(head.contains(" 101 "), "{head}");
    let (mut carol, head, mut carol_acc) = binary_client(&addr, "/topics/chat", &token("carol"));
    assert!(head.contains(" 101 "), "{head}");
    let (_bob, head, _) = binary_client(&addr, "/topics/chat", &token("bob"));
    assert!(head.contains(" 403 "), "{head}");

    // WS → DDS: each client's sample reaches the domain of its participant.
    let frame_of = |body: &[u8], mask: [u8; 4]| {
        let mut payload = b"ZDB1\x00\x00\x07\x00\x00".to_vec();
        payload.extend_from_slice(body);
        encode(&Frame::binary(payload).with_mask(mask)).unwrap()
    };
    let from_alice = chat_sample("alice", 1);
    let from_carol = chat_sample("carol", 2);
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut other_got = Vec::new();
    while !other_got.contains(&from_alice) {
        assert!(
            std::time::Instant::now() < deadline,
            "alice's sample never reached the tenant's domain"
        );
        alice
            .write_all(&frame_of(&from_alice, [1, 2, 3, 4]))
            .unwrap();
        other_got.extend(peer_other.received(Duration::from_millis(300)));
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut base_got = Vec::new();
    while !base_got.contains(&from_carol) {
        assert!(
            std::time::Instant::now() < deadline,
            "carol's sample never reached the domain of the daemon"
        );
        carol
            .write_all(&frame_of(&from_carol, [4, 3, 2, 1]))
            .unwrap();
        base_got.extend(peer_base.received(Duration::from_millis(300)));
    }
    other_got.extend(peer_other.received(Duration::from_millis(500)));
    base_got.extend(peer_base.received(Duration::from_millis(500)));
    assert!(
        !base_got.contains(&from_alice),
        "alice's sample on the domain of the daemon"
    );
    assert!(
        !other_got.contains(&from_carol),
        "carol's sample on the tenant's domain"
    );

    // DDS → WS: each client gets the samples of its participant's domain.
    let to_alice = chat_sample("tenant", 3);
    let to_carol = chat_sample("daemon", 4);
    let ends_with = |frames: &[Vec<u8>], body: &[u8]| frames.iter().any(|f| f.ends_with(body));
    let mut alice_frames = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !ends_with(&alice_frames, &to_alice) {
        assert!(
            std::time::Instant::now() < deadline,
            "the tenant's sample never reached alice"
        );
        peer_other
            .dds
            .write_user_sample(peer_other.writer, to_alice.clone())
            .unwrap();
        alice_frames.extend(binary_frames_within(
            &mut alice,
            &mut alice_acc,
            Duration::from_millis(300),
        ));
    }
    let mut carol_frames = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !ends_with(&carol_frames, &to_carol) {
        assert!(
            std::time::Instant::now() < deadline,
            "the daemon's domain's sample never reached carol"
        );
        peer_base
            .dds
            .write_user_sample(peer_base.writer, to_carol.clone())
            .unwrap();
        carol_frames.extend(binary_frames_within(
            &mut carol,
            &mut carol_acc,
            Duration::from_millis(300),
        ));
    }
    alice_frames.extend(binary_frames_within(
        &mut alice,
        &mut alice_acc,
        Duration::from_millis(500),
    ));
    carol_frames.extend(binary_frames_within(
        &mut carol,
        &mut carol_acc,
        Duration::from_millis(500),
    ));
    for body in [&to_carol, &from_carol] {
        assert!(
            !ends_with(&alice_frames, body),
            "a sample of the daemon's domain reached alice"
        );
    }
    for body in [&to_alice, &from_alice] {
        assert!(
            !ends_with(&carol_frames, body),
            "a sample of the tenant reached carol"
        );
    }

    // The catalog of each connection names its participant's topics.
    let catalog = |sub: &str| {
        let mut s = TcpStream::connect(&addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        s.write_all(
            format!(
                "GET /topics/__catalog__ HTTP/1.1\r\nHost: x\r\nAuthorization: {}\r\nConnection: close\r\n\r\n",
                token(sub)
            )
            .as_bytes(),
        )
        .unwrap();
        let mut text = String::new();
        let _ = s.read_to_string(&mut text);
        text
    };
    let alice_catalog = catalog("alice");
    assert!(alice_catalog.contains("200 OK"), "{alice_catalog}");
    assert!(alice_catalog.contains("Plant::Status"), "{alice_catalog}");
    let carol_catalog = catalog("carol");
    assert!(carol_catalog.contains("\"Chat\""), "{carol_catalog}");
    assert!(!carol_catalog.contains("Plant::Status"), "{carol_catalog}");

    // §8.2: the tenant's topic is counted under `plant-a/Chat`.
    let metrics = h.metrics.clone().expect("metrics");
    assert!(metrics.value(family::DDS_PUBLISHED, &[("topic", "plant-a/Chat")]) >= 1);
    assert!(metrics.value(family::DDS_PUBLISHED, &[("topic", "Chat")]) >= 1);
    assert!(metrics.value(family::DDS_RECEIVED, &[("topic", "plant-a/Chat")]) >= 1);
    peer_base.dds.shutdown();
    peer_other.dds.shutdown();
    drop(h);
}
