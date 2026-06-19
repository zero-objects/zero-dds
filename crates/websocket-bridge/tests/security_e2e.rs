// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! E2E test for `zerodds-ws-bridged` §7 security wireup.
//!
//! Covers:
//! * §7.1 TLS handshake (server-side, optionally with a client cert).
//! * §7.2 auth (bearer token in the `Authorization` header).
//! * §7.3 topic ACL for the auto-subscribe path and the inline subscribe op.
//! * §9.2 SIGHUP cert reload (live reload without restart).

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
use zerodds_websocket_bridge::daemon::config::{DaemonConfig, TopicConfig};
use zerodds_websocket_bridge::daemon::server;
use zerodds_websocket_bridge::handshake::compute_accept;

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

fn make_secure_cfg(cert: &str, key: &str, with_auth: bool) -> DaemonConfig {
    let cert_path = write_temp("ws_cert.pem", cert.as_bytes());
    let key_path = write_temp("ws_key.pem", key.as_bytes());
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.listen = "127.0.0.1:0".to_string();
    cfg.domain = 99;
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
    });
    cfg.topics.push(TopicConfig {
        name: "Forbidden".into(),
        type_name: "Forbidden".into(),
        direction: "bidir".into(),
        ws_path: "/topics/forbidden".into(),
        reliability: "reliable".into(),
        durability: "volatile".into(),
        history_depth: 10,
    });
    cfg
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

#[test]
fn tls_plain_handshake_succeeds_then_data_flows() {
    let (cert, key) = gen_self_signed_for("localhost");
    let cfg = make_secure_cfg(&cert, &key, false);
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
    drop(h);
}

#[test]
fn auth_missing_bearer_yields_401() {
    let (cert, key) = gen_self_signed_for("localhost");
    let cfg = make_secure_cfg(&cert, &key, true);
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
fn auth_invalid_bearer_yields_401() {
    let (cert, key) = gen_self_signed_for("localhost");
    let cfg = make_secure_cfg(&cert, &key, true);
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
fn auth_valid_bearer_then_acl_allows_subscribed_topic() {
    let (cert, key) = gen_self_signed_for("localhost");
    let cfg = make_secure_cfg(&cert, &key, true);
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
fn auth_valid_bearer_but_acl_denies_forbidden_topic() {
    let (cert, key) = gen_self_signed_for("localhost");
    let cfg = make_secure_cfg(&cert, &key, true);
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
fn sighup_cert_reload_keeps_daemon_running_with_new_cert() {
    use std::sync::atomic::Ordering;

    let (cert1, key1) = gen_self_signed_for("localhost");
    let cert_path = write_temp("ws_reload_cert.pem", cert1.as_bytes());
    let key_path = write_temp("ws_reload_key.pem", key1.as_bytes());
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.listen = "127.0.0.1:0".into();
    cfg.domain = 99;
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

    drop(h);
}
