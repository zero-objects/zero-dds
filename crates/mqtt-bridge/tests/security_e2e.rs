// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! E2E tests for `zerodds-mqtt-bridged` §7.x security wireup.
//!
//! Covers:
//! * §7.1 TLS — bridge-client connect via `mqtts://` with a rustls wrap.
//! * §7.2 Auth — bearer token + SASL-PLAIN as CONNECT username/password.
//! * §7.3 topic ACL — subscribe-skip + publish-drop on deny.
//!
//! Mock broker: a rustls server on `127.0.0.1:0` that reads a single
//! MQTT CONNECT frame, pushes the CONNECT body into a channel
//! and answers with CONNACK reason 0x00. The test side then reads the
//! user/password field.

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
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use rustls::{ClientConfig, ServerConfig, ServerConnection, StreamOwned};
use rustls_pemfile::Item;
use zerodds_mqtt_bridge::daemon::config::DaemonConfig;
use zerodds_mqtt_bridge::daemon::security::{
    AclEntry, AclOp, AuthSubject, SecurityConfig, authorize, build_ctx, ctx_from_daemon_config,
    outbound_credentials,
};

// ---------- Cert-Helper ----------

#[allow(dead_code)]
fn write_temp(name: &str, body: &[u8]) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering as O};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, O::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "zd-mqtt-sec-e2e-{}-{}-{}",
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

fn load_certs(pem: &str) -> Vec<CertificateDer<'static>> {
    let mut br = std::io::BufReader::new(pem.as_bytes());
    let mut out = Vec::new();
    for it in rustls_pemfile::read_all(&mut br) {
        if let Ok(Item::X509Certificate(d)) = it {
            out.push(d);
        }
    }
    out
}

fn load_key(pem: &str) -> PrivatePkcs8KeyDer<'static> {
    let mut br = std::io::BufReader::new(pem.as_bytes());
    for it in rustls_pemfile::read_all(&mut br) {
        if let Ok(Item::Pkcs8Key(k)) = it {
            return k;
        }
    }
    panic!("no PKCS8 key in PEM");
}

fn server_tls_cfg(cert_pem: &str, key_pem: &str) -> Arc<ServerConfig> {
    let certs = load_certs(cert_pem);
    let key = load_key(key_pem);
    let provider = rustls::crypto::ring::default_provider();
    let cfg = ServerConfig::builder_with_provider(Arc::new(provider))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(certs, key.into())
        .unwrap();
    Arc::new(cfg)
}

// ---------- Mock-Broker ----------

struct MockBroker {
    addr: String,
    captured: mpsc::Receiver<MqttConnect>,
    stop: Arc<AtomicBool>,
    _join: thread::JoinHandle<()>,
}

#[derive(Debug, Clone)]
struct MqttConnect {
    username: Option<String>,
    password: Option<Vec<u8>>,
    client_id: String,
}

impl MockBroker {
    fn start_tls(server_cfg: Arc<ServerConfig>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().unwrap().to_string();
        let (tx, rx) = mpsc::channel::<MqttConnect>();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_c = Arc::clone(&stop);
        let join = thread::spawn(move || {
            listener.set_nonblocking(false).ok();
            while !stop_c.load(Ordering::SeqCst) {
                let (sock, _peer) = match listener.accept() {
                    Ok(p) => p,
                    Err(_) => break,
                };
                if stop_c.load(Ordering::SeqCst) {
                    break;
                }
                let conn = match ServerConnection::new(Arc::clone(&server_cfg)) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let mut tls = StreamOwned::new(conn, sock);
                let _ = tls.sock.set_read_timeout(Some(Duration::from_secs(3)));
                if let Some(connect) = read_mqtt_connect(&mut tls) {
                    let _ = tx.send(connect);
                    // CONNACK with reason 0x00: 0x20 0x03 flags=0 reason=0 props_len=0
                    let connack = [0x20u8, 0x03, 0x00, 0x00, 0x00];
                    let _ = tls.write_all(&connack);
                    let _ = tls.flush();
                }
            }
        });
        Self {
            addr,
            captured: rx,
            stop,
            _join: join,
        }
    }

    #[allow(dead_code)]
    fn start_plain() -> (Self, ()) {
        // No-op: not used in current tests, but exposed for symmetry.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().unwrap().to_string();
        let (tx, rx) = mpsc::channel::<MqttConnect>();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_c = Arc::clone(&stop);
        let join = thread::spawn(move || {
            while !stop_c.load(Ordering::SeqCst) {
                let (mut sock, _) = match listener.accept() {
                    Ok(p) => p,
                    Err(_) => break,
                };
                if stop_c.load(Ordering::SeqCst) {
                    break;
                }
                let _ = sock.set_read_timeout(Some(Duration::from_secs(3)));
                if let Some(connect) = read_mqtt_connect(&mut sock) {
                    let _ = tx.send(connect);
                    let connack = [0x20u8, 0x03, 0x00, 0x00, 0x00];
                    let _ = sock.write_all(&connack);
                }
            }
        });
        (
            Self {
                addr,
                captured: rx,
                stop,
                _join: join,
            },
            (),
        )
    }

    fn shutdown(&self) {
        self.stop.store(true, Ordering::SeqCst);
        // Self-connect to wake.
        let _ = TcpStream::connect(&self.addr);
    }
}

fn read_vbi<R: Read>(r: &mut R) -> Option<u32> {
    let mut acc: u32 = 0;
    let mut mult: u32 = 1;
    for _ in 0..4 {
        let mut b = [0u8; 1];
        r.read_exact(&mut b).ok()?;
        acc += (b[0] as u32 & 0x7f) * mult;
        if b[0] & 0x80 == 0 {
            return Some(acc);
        }
        mult = mult.checked_mul(128)?;
    }
    None
}

fn read_mqtt_connect<R: Read>(r: &mut R) -> Option<MqttConnect> {
    let mut hdr = [0u8; 1];
    r.read_exact(&mut hdr).ok()?;
    if hdr[0] >> 4 != 1 {
        return None;
    }
    let len = read_vbi(r)? as usize;
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).ok()?;
    let mut i = 0usize;
    // Protocol-Name: u16 length + "MQTT".
    if body.len() < 2 {
        return None;
    }
    let pn_len = u16::from_be_bytes([body[i], body[i + 1]]) as usize;
    i += 2 + pn_len;
    if i >= body.len() {
        return None;
    }
    // Protocol-Version (u8).
    let _ver = body[i];
    i += 1;
    // Connect-Flags (u8).
    let flags = body[i];
    i += 1;
    let user_flag = (flags & 0x80) != 0;
    let pass_flag = (flags & 0x40) != 0;
    // Keep-Alive (u16).
    i += 2;
    // Properties: vbi length + bytes.
    let mut sub = std::io::Cursor::new(&body[i..]);
    let props_len = read_vbi(&mut sub)? as usize;
    let consumed = sub.position() as usize;
    i += consumed + props_len;
    if i >= body.len() {
        return None;
    }
    // Client-Id (u16 length + utf8).
    let cid_len = u16::from_be_bytes([body[i], body[i + 1]]) as usize;
    i += 2;
    let client_id = String::from_utf8_lossy(&body[i..i + cid_len]).to_string();
    i += cid_len;
    let mut username: Option<String> = None;
    let mut password: Option<Vec<u8>> = None;
    if user_flag && i + 2 <= body.len() {
        let ul = u16::from_be_bytes([body[i], body[i + 1]]) as usize;
        i += 2;
        if i + ul <= body.len() {
            username = Some(String::from_utf8_lossy(&body[i..i + ul]).to_string());
            i += ul;
        }
    }
    if pass_flag && i + 2 <= body.len() {
        let pl = u16::from_be_bytes([body[i], body[i + 1]]) as usize;
        i += 2;
        if i + pl <= body.len() {
            password = Some(body[i..i + pl].to_vec());
        }
    }
    Some(MqttConnect {
        username,
        password,
        client_id,
    })
}

fn build_test_client_config(server_cert_pem: &str) -> Arc<ClientConfig> {
    let mut roots = rustls::RootCertStore::empty();
    for c in load_certs(server_cert_pem) {
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

// ---------- TESTS ----------

#[test]
fn tls_connect_to_broker_succeeds_with_valid_cert() {
    use zerodds_mqtt_bridge::daemon::client::MqttClient;
    let (cert, key) = gen_self_signed_for("localhost");
    let server_cfg = server_tls_cfg(&cert, &key);
    let broker = MockBroker::start_tls(server_cfg);
    // Strip host:port.
    let parts: Vec<&str> = broker.addr.rsplitn(2, ':').collect();
    let port: u16 = parts[0].parse().unwrap();
    let host = parts[1];

    let client_cfg = build_test_client_config(&cert);
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.client_id = "test-cli".into();
    cfg.broker_tls_enabled = true;
    cfg.broker_tls_server_name = "localhost".into();

    let _client =
        MqttClient::connect_secure(host, port, &cfg, Some(client_cfg)).expect("tls connect");
    let captured = broker
        .captured
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    assert_eq!(captured.client_id, "test-cli");
    broker.shutdown();
}

#[test]
fn auth_bearer_emits_password_in_connect() {
    use zerodds_mqtt_bridge::daemon::client::MqttClient;
    let (cert, key) = gen_self_signed_for("localhost");
    let server_cfg = server_tls_cfg(&cert, &key);
    let broker = MockBroker::start_tls(server_cfg);
    let parts: Vec<&str> = broker.addr.rsplitn(2, ':').collect();
    let port: u16 = parts[0].parse().unwrap();
    let host = parts[1];

    let client_cfg = build_test_client_config(&cert);
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.client_id = "auth-cli".into();
    cfg.broker_tls_enabled = true;
    cfg.broker_tls_server_name = "localhost".into();
    cfg.auth_mode = "bearer".into();
    cfg.auth_bearer_token = Some("super-secret-token".into());

    let _client = MqttClient::connect_secure(host, port, &cfg, Some(client_cfg)).expect("connect");
    let captured = broker
        .captured
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    assert_eq!(
        captured.password.as_deref(),
        Some(b"super-secret-token".as_ref())
    );
    broker.shutdown();
}

#[test]
fn auth_sasl_plain_emits_user_and_password() {
    use zerodds_mqtt_bridge::daemon::client::MqttClient;
    let (cert, key) = gen_self_signed_for("localhost");
    let server_cfg = server_tls_cfg(&cert, &key);
    let broker = MockBroker::start_tls(server_cfg);
    let parts: Vec<&str> = broker.addr.rsplitn(2, ':').collect();
    let port: u16 = parts[0].parse().unwrap();
    let host = parts[1];

    let client_cfg = build_test_client_config(&cert);
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.client_id = "sasl-cli".into();
    cfg.broker_tls_enabled = true;
    cfg.broker_tls_server_name = "localhost".into();
    cfg.auth_mode = "sasl_plain".into();
    cfg.outbound_username = Some("alice".into());
    cfg.outbound_password = Some("wonderland".into());

    let _client = MqttClient::connect_secure(host, port, &cfg, Some(client_cfg)).expect("connect");
    let captured = broker
        .captured
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    assert_eq!(captured.username.as_deref(), Some("alice"));
    assert_eq!(captured.password.as_deref(), Some(b"wonderland".as_ref()));
    broker.shutdown();
}

#[test]
fn ctx_from_config_builds_security_context_with_acl() {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.auth_mode = "bearer".into();
    cfg.auth_bearer_token = Some("tk".into());
    cfg.auth_bearer_subject = Some("alice".into());
    cfg.topic_acl
        .insert("Trade".into(), (vec!["alice".into()], vec!["alice".into()]));
    cfg.topic_acl
        .insert("Forbidden".into(), (vec!["bob".into()], vec!["bob".into()]));

    let (ctx, tls) = ctx_from_daemon_config(&cfg).expect("ctx");
    assert!(tls.is_none(), "tls disabled => no client cfg");
    let alice = AuthSubject::new("alice");
    let bob = AuthSubject::new("bob");
    assert!(authorize(&ctx.acl, &alice, AclOp::Read, "Trade"));
    assert!(authorize(&ctx.acl, &alice, AclOp::Write, "Trade"));
    assert!(!authorize(&ctx.acl, &alice, AclOp::Read, "Forbidden"));
    assert!(authorize(&ctx.acl, &bob, AclOp::Read, "Forbidden"));
}

#[test]
fn outbound_credentials_bearer_yields_password_only() {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.auth_mode = "bearer".into();
    cfg.auth_bearer_token = Some("tok42".into());
    let (u, p) = outbound_credentials(&cfg);
    assert!(u.is_none());
    assert_eq!(p.as_deref(), Some(b"tok42".as_ref()));
}

#[test]
fn outbound_credentials_sasl_yields_user_and_password() {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.auth_mode = "sasl_plain".into();
    cfg.outbound_username = Some("u".into());
    cfg.outbound_password = Some("p".into());
    let (u, p) = outbound_credentials(&cfg);
    assert_eq!(u.as_deref(), Some("u"));
    assert_eq!(p.as_deref(), Some(b"p".as_ref()));
}

#[test]
fn acl_skip_subscribe_when_subject_not_in_read_list() {
    // Synthetic ACL setup; covers the behavior used in
    // server.rs for SUBSCRIBE filters.
    let mut sec = SecurityConfig::default();
    sec.topic_acl.insert(
        "Allowed".into(),
        AclEntry {
            read: vec!["zerodds-mqtt-bridge".into()],
            write: vec!["zerodds-mqtt-bridge".into()],
        },
    );
    sec.topic_acl.insert(
        "Forbidden".into(),
        AclEntry {
            read: vec!["someone-else".into()],
            write: vec!["someone-else".into()],
        },
    );
    let ctx = build_ctx(&sec).unwrap();
    let bridge = AuthSubject::new("zerodds-mqtt-bridge");
    assert!(authorize(&ctx.acl, &bridge, AclOp::Read, "Allowed"));
    assert!(!authorize(&ctx.acl, &bridge, AclOp::Read, "Forbidden"));
}
