// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! E2E test for the `zerodds-amqp-bridged` §7 security wireup.
//!
//! Covers:
//! * §7.1 TLS — outbound connector to a rustls mock broker (`amqps://`).
//! * §7.2 SASL-PLAIN init-response render from config.
//! * §7.2 bearer auth via `application-properties[zerodds:auth-token]`.
//! * §7.3 topic-ACL build from the config HashMap.

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

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer, ServerName};
use rustls::{ClientConfig, ClientConnection, ServerConfig, ServerConnection, StreamOwned};
use rustls_pemfile::Item;
use zerodds_amqp_endpoint::bridge_security::{
    AclEntry, AclOp, AmqpSecurityConfig, AuthMode, AuthSubject, SecurityConfig,
    authenticate_amqp_bearer, authenticate_amqp_sasl, authorize, build_ctx, ctx_from_amqp_config,
    parse_server_name, sasl_plain_init_response,
};

// ---------- Cert/PEM-Helper ----------

fn write_temp(name: &str, body: &[u8]) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering as O};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, O::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "zd-amqp-sec-e2e-{}-{}-{}",
        name,
        std::process::id(),
        seq
    ));
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    p
}

fn gen_self_signed() -> (String, String) {
    let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
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

// ---------- Mock-Broker: AMQP-Protocol-Header ----------

const AMQP_PROTOCOL_HEADER: [u8; 8] = [b'A', b'M', b'Q', b'P', 0x00, 0x01, 0x00, 0x00];

struct MockTlsBroker {
    addr: String,
    received: mpsc::Receiver<[u8; 8]>,
    stop: Arc<AtomicBool>,
    _join: thread::JoinHandle<()>,
}

impl MockTlsBroker {
    fn start(server_cfg: Arc<ServerConfig>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().unwrap().to_string();
        let (tx, rx) = mpsc::channel::<[u8; 8]>();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_c = Arc::clone(&stop);
        let join = thread::spawn(move || {
            while !stop_c.load(Ordering::SeqCst) {
                let (sock, _) = match listener.accept() {
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
                let _ = tls.sock.set_read_timeout(Some(Duration::from_secs(2)));
                // Read 8-byte AMQP protocol header.
                let mut hdr = [0u8; 8];
                if tls.read_exact(&mut hdr).is_ok() {
                    let _ = tx.send(hdr);
                    // Echo back AMQP-protocol-header.
                    let _ = tls.write_all(&AMQP_PROTOCOL_HEADER);
                    let _ = tls.flush();
                }
            }
        });
        Self {
            addr,
            received: rx,
            stop,
            _join: join,
        }
    }

    fn shutdown(&self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(&self.addr);
    }
}

#[allow(dead_code)]
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

// ---------- Helper to write 8-byte protocol header through a rustls
// ----------  client wrapper around a TCP stream.

fn drive_amqp_proto_handshake(addr: &str, client_cfg: Arc<ClientConfig>) {
    let server_name: ServerName<'static> = ServerName::try_from("localhost".to_string()).unwrap();
    let conn = ClientConnection::new(client_cfg, server_name).unwrap();
    let sock = TcpStream::connect(addr).unwrap();
    sock.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    sock.set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut tls = StreamOwned::new(conn, sock);
    // Drive handshake synchronously.
    while tls.conn.is_handshaking() {
        if tls.conn.wants_write() {
            tls.conn.write_tls(&mut tls.sock).unwrap();
        }
        if tls.conn.wants_read() {
            let n = tls.conn.read_tls(&mut tls.sock).unwrap();
            if n == 0 {
                break;
            }
            tls.conn.process_new_packets().unwrap();
        }
    }
    while tls.conn.wants_write() {
        tls.conn.write_tls(&mut tls.sock).unwrap();
    }
    // Send AMQP protocol header through the established TLS stream.
    tls.write_all(&AMQP_PROTOCOL_HEADER).unwrap();
    tls.flush().unwrap();
    // Read echoed header back.
    let mut peer_hdr = [0u8; 8];
    let _ = tls.read_exact(&mut peer_hdr);
}

// ---------- TESTS ----------

#[test]
fn tls_client_connector_handshakes_with_mock_broker() {
    let (cert, key) = gen_self_signed();
    let server_cfg = server_tls_cfg(&cert, &key);
    let broker = MockTlsBroker::start(server_cfg);

    // Build amqp-bridge security config that mirrors a daemon-config
    // form: tls.enabled=true with a CA-pem written to disk.
    let ca_path = write_temp("amqp_ca.pem", cert.as_bytes());
    let mut sec = AmqpSecurityConfig::default();
    sec.tls_enabled = true;
    sec.tls_ca_file = ca_path.to_string_lossy().into();
    sec.tls_server_name = "localhost".into();
    sec.auth_mode = "none".into();
    let (_ctx, client_cfg) = ctx_from_amqp_config(&sec).expect("ctx");
    let client_cfg = client_cfg.expect("client cfg present");

    // Drive handshake + protocol header roundtrip.
    drive_amqp_proto_handshake(&broker.addr, client_cfg);
    let received = broker
        .received
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    assert_eq!(&received, &AMQP_PROTOCOL_HEADER);
    broker.shutdown();
}

#[test]
fn sasl_plain_init_response_round_trip_with_authenticator() {
    let mut sec = AmqpSecurityConfig::default();
    sec.auth_mode = "sasl_plain".into();
    sec.sasl_username = Some("alice".into());
    sec.sasl_password = Some("wonderland".into());
    let blob = sasl_plain_init_response(&sec).expect("sasl blob");
    // Now confirm the bridge_security side accepts this payload.
    let mut sasl_users = std::collections::HashMap::new();
    sasl_users.insert("alice".to_string(), "wonderland".to_string());
    let auth = AuthMode::SaslPlain { users: sasl_users };
    let s = authenticate_amqp_sasl(&auth, Some(&blob), None).unwrap();
    assert_eq!(s.name, "alice");
}

#[test]
fn ctx_from_amqp_config_with_bearer_token_and_acl() {
    let mut sec = AmqpSecurityConfig::default();
    sec.auth_mode = "bearer".into();
    sec.bearer_token = Some("tk1".into());
    sec.bearer_subject = Some("alice".into());
    sec.topic_acl.insert(
        "queue/orders".into(),
        (vec!["alice".into()], vec!["alice".into()]),
    );
    sec.topic_acl.insert(
        "queue/forbidden".into(),
        (vec!["bob".into()], vec!["bob".into()]),
    );
    let (ctx, tls) = ctx_from_amqp_config(&sec).expect("ctx");
    assert!(tls.is_none(), "tls disabled => no client cfg");
    let alice = AuthSubject::new("alice");
    let bob = AuthSubject::new("bob");
    assert!(authorize(&ctx.acl, &alice, AclOp::Read, "queue/orders"));
    assert!(!authorize(&ctx.acl, &bob, AclOp::Read, "queue/orders"));
    assert!(authorize(&ctx.acl, &bob, AclOp::Write, "queue/forbidden"));
    // And the Bearer-Auth flow ends up with `alice`.
    let s = authenticate_amqp_bearer(&ctx.auth, Some("tk1")).unwrap();
    assert_eq!(s.name, "alice");
}

#[test]
fn authentication_rejects_unknown_bearer() {
    let mut sec = AmqpSecurityConfig::default();
    sec.auth_mode = "bearer".into();
    sec.bearer_token = Some("tk1".into());
    sec.bearer_subject = Some("alice".into());
    let (ctx, _) = ctx_from_amqp_config(&sec).unwrap();
    let err = authenticate_amqp_bearer(&ctx.auth, Some("unknown")).unwrap_err();
    assert!(matches!(
        err,
        zerodds_amqp_endpoint::bridge_security::AuthError::Rejected(_)
    ));
}

#[test]
fn build_security_ctx_with_explicit_acl_default_denies_unlisted() {
    let mut sec = SecurityConfig::default();
    sec.topic_acl.insert(
        "queue/X".into(),
        AclEntry {
            read: vec!["alice".into()],
            write: vec![],
        },
    );
    let ctx = build_ctx(&sec).unwrap();
    let alice = AuthSubject::new("alice");
    assert!(authorize(&ctx.acl, &alice, AclOp::Read, "queue/X"));
    assert!(!authorize(&ctx.acl, &alice, AclOp::Read, "queue/Other"));
}

#[test]
fn parse_server_name_accepts_dns_and_ip() {
    let _ = parse_server_name("example.com").expect("dns");
    let _ = parse_server_name("127.0.0.1").expect("ip");
}
