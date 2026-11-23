// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! E2E tests for `zerodds-mqtt-bridged` §7.x security wireup.
//!
//! Covers:
//! * §7.1 TLS — bridge-client connect via `mqtts://` with a rustls wrap.
//! * §7.2 Auth — `password` (also under its names `bearer` and
//!   `sasl_plain`) as CONNECT username/password; `mtls` with the client
//!   certificate and neither.
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
use zerodds_mqtt_bridge::daemon::auth::{AuthKind, Credentials};
use zerodds_mqtt_bridge::daemon::config::DaemonConfig;

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
    /// The ALPN protocol the TLS handshake agreed on; `None` on plain TCP.
    alpn: Option<Vec<u8>>,
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
                if let Some(mut connect) = read_mqtt_connect(&mut tls) {
                    connect.alpn = tls.conn.alpn_protocol().map(<[u8]>::to_vec);
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
    // The will (spec §6 Liveliness): its properties, topic and message.
    if flags & 0x04 != 0 {
        let mut sub = std::io::Cursor::new(&body[i..]);
        let props_len = read_vbi(&mut sub)? as usize;
        i += sub.position() as usize + props_len;
        for _ in 0..2 {
            let len = u16::from_be_bytes([*body.get(i)?, *body.get(i + 1)?]) as usize;
            i += 2 + len;
        }
    }
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
        alpn: None,
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
fn mqttbridge_7_1_tls_connect_to_broker_succeeds_with_valid_cert() {
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

/// Spec §7.1: `mqtts://` offers the ALPN protocol `mqtt` (RFC 7301). A
/// broker that speaks ALPN agrees on it; the mock broker reads back what
/// the handshake agreed on.
#[test]
fn mqttbridge_7_1_tls_connect_offers_the_mqtt_alpn_protocol() {
    use zerodds_mqtt_bridge::daemon::client::MqttClient;
    let (cert, key) = gen_self_signed_for("localhost");
    let mut server_cfg = (*server_tls_cfg(&cert, &key)).clone();
    server_cfg.alpn_protocols = vec![b"mqtt".to_vec()];
    let broker = MockBroker::start_tls(Arc::new(server_cfg));
    let (host, port) = broker.addr.rsplit_once(':').unwrap();
    let port: u16 = port.parse().unwrap();
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.client_id = "alpn-cli".into();
    cfg.broker_tls_server_name = "localhost".into();
    let _client =
        MqttClient::connect_secure(host, port, &cfg, Some(build_test_client_config(&cert)))
            .expect("tls connect");
    let captured = broker
        .captured
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    assert_eq!(captured.alpn.as_deref(), Some(&b"mqtt"[..]));
    broker.shutdown();

    // Gegenprobe: a broker that allows only another protocol refuses the
    // handshake, so the offer is on the wire.
    let mut other_cfg = (*server_tls_cfg(&cert, &key)).clone();
    other_cfg.alpn_protocols = vec![b"x-other".to_vec()];
    let broker = MockBroker::start_tls(Arc::new(other_cfg));
    let (host, port) = broker.addr.rsplit_once(':').unwrap();
    let port: u16 = port.parse().unwrap();
    let refused =
        MqttClient::connect_secure(host, port, &cfg, Some(build_test_client_config(&cert)));
    assert!(
        refused.is_err(),
        "a broker without mqtt in its ALPN list accepted"
    );
    broker.shutdown();
}

/// Spec §3 `mqtt.tls.alpn`: the TLS handshake offers the protocol names
/// of the configuration instead of `mqtt` (a broker that serves MQTT on
/// another ALPN name, such as `x-amzn-mqtt-ca`); a broker that allows only
/// `mqtt` refuses it.
#[test]
fn mqttbridge_3_the_alpn_list_of_the_configuration_is_offered() {
    use zerodds_mqtt_bridge::daemon::client::MqttClient;
    let (cert, key) = gen_self_signed_for("localhost");
    let mut server_cfg = (*server_tls_cfg(&cert, &key)).clone();
    server_cfg.alpn_protocols = vec![b"x-amzn-mqtt-ca".to_vec()];
    let broker = MockBroker::start_tls(Arc::new(server_cfg));
    let (host, port) = broker.addr.rsplit_once(':').unwrap();
    let port: u16 = port.parse().unwrap();
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.client_id = "alpn-list".into();
    cfg.broker_tls_server_name = "localhost".into();
    cfg.broker_tls_alpn = Some(vec!["x-amzn-mqtt-ca".into()]);
    let _client =
        MqttClient::connect_secure(host, port, &cfg, Some(build_test_client_config(&cert)))
            .expect("tls connect");
    let captured = broker
        .captured
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    assert_eq!(captured.alpn.as_deref(), Some(&b"x-amzn-mqtt-ca"[..]));
    broker.shutdown();

    let mut mqtt_only = (*server_tls_cfg(&cert, &key)).clone();
    mqtt_only.alpn_protocols = vec![b"mqtt".to_vec()];
    let broker = MockBroker::start_tls(Arc::new(mqtt_only));
    let (host, port) = broker.addr.rsplit_once(':').unwrap();
    let port: u16 = port.parse().unwrap();
    let refused =
        MqttClient::connect_secure(host, port, &cfg, Some(build_test_client_config(&cert)));
    assert!(
        refused.is_err(),
        "a broker of only `mqtt` accepted the offer"
    );
    broker.shutdown();
}

#[test]
fn mqttbridge_7_2_auth_bearer_emits_password_in_connect() {
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
    cfg.auth_mode = AuthKind::parse("bearer");
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
fn mqttbridge_7_2_auth_sasl_plain_emits_user_and_password() {
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
    cfg.auth_mode = AuthKind::parse("sasl_plain");
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
fn mqttbridge_7_2_the_bearer_token_is_the_password_without_a_user_name() {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.auth_mode = AuthKind::parse("bearer");
    cfg.auth_bearer_token = Some("tok42".into());
    let c = Credentials::of(&cfg);
    assert_eq!(c.kind(), AuthKind::Password);
    assert!(c.connect_user_name().is_none());
    assert_eq!(c.connect_password().as_deref(), Some(b"tok42".as_ref()));
}

#[test]
fn mqttbridge_7_2_auth_username_and_password_win_over_the_mqtt_keys() {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.auth_mode = AuthKind::parse("sasl_plain");
    cfg.username = Some("mqtt-user".into());
    cfg.password = Some("mqtt-pass".into());
    cfg.outbound_username = Some("u".into());
    cfg.outbound_password = Some("p".into());
    let c = Credentials::of(&cfg);
    assert_eq!(c.connect_user_name().as_deref(), Some("u"));
    assert_eq!(c.connect_password().as_deref(), Some(b"p".as_ref()));
}

// ---------- §7.1: the certificate of the next connection ----------

/// A client certificate verifier that takes any certificate and keeps
/// each, so the test reads which one the daemon presented.
#[derive(Debug)]
struct RecordingVerifier {
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl rustls::server::danger::ClientCertVerifier for RecordingVerifier {
    fn root_hint_subjects(&self) -> &[rustls::DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::server::danger::ClientCertVerified, rustls::Error> {
        Ok(rustls::server::danger::ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// The server name and the ALPN protocol names a ClientHello offered.
type Hello = (Option<String>, Vec<Vec<u8>>);

/// The server certificate of [`CertBroker`]: it keeps the server name and
/// the ALPN offer of each ClientHello and serves the certificate in force,
/// which [`CertBroker::serve`] replaces.
#[derive(Debug)]
struct HelloRecorder {
    key: std::sync::Mutex<Arc<rustls::sign::CertifiedKey>>,
    hellos: std::sync::Mutex<Vec<Hello>>,
}

impl HelloRecorder {
    fn certified(cert_pem: &str, key_pem: &str) -> Arc<rustls::sign::CertifiedKey> {
        let key = rustls::crypto::ring::sign::any_supported_type(&load_key(key_pem).into())
            .expect("a signing key");
        Arc::new(rustls::sign::CertifiedKey::new(load_certs(cert_pem), key))
    }
}

impl rustls::server::ResolvesServerCert for HelloRecorder {
    fn resolve(
        &self,
        hello: rustls::server::ClientHello<'_>,
    ) -> Option<Arc<rustls::sign::CertifiedKey>> {
        let alpn = hello
            .alpn()
            .map(|names| names.map(<[u8]>::to_vec).collect())
            .unwrap_or_default();
        self.hellos
            .lock()
            .unwrap()
            .push((hello.server_name().map(str::to_string), alpn));
        Some(Arc::clone(&self.key.lock().unwrap()))
    }
}

/// A TLS broker that asks for a client certificate and keeps the one of
/// each connection, and the server name and ALPN offer of each ClientHello;
/// it answers CONNECT, SUBSCRIBE, a QoS 1 PUBLISH and PINGREQ, and closes
/// the connection that stands on [`Self::close`].
struct CertBroker {
    port: u16,
    certs: Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
    /// The Connect Flags of each CONNECT (MQTT 5.0 §3.1.2.3).
    connect_flags: Arc<std::sync::Mutex<Vec<u8>>>,
    current: Arc<std::sync::Mutex<Option<TcpStream>>>,
    resolver: Arc<HelloRecorder>,
}

impl CertBroker {
    fn start(cert_pem: &str, key_pem: &str) -> Self {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let resolver = Arc::new(HelloRecorder {
            key: std::sync::Mutex::new(HelloRecorder::certified(cert_pem, key_pem)),
            hellos: std::sync::Mutex::default(),
        });
        let mut server_cfg = ServerConfig::builder_with_provider(Arc::clone(&provider))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_client_cert_verifier(Arc::new(RecordingVerifier {
                provider: Arc::clone(&provider),
            }))
            .with_cert_resolver(
                Arc::clone(&resolver) as Arc<dyn rustls::server::ResolvesServerCert>
            );
        // Every connection is a full handshake: a resumed session (RFC 8446
        // §2.2) shows no certificate, so the client's CA and certificate of
        // the moment would not be what the test reads.
        server_cfg.send_tls13_tickets = 0;
        server_cfg.session_storage = Arc::new(rustls::server::NoServerSessionStorage {});
        let server_cfg = Arc::new(server_cfg);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let certs: Arc<std::sync::Mutex<Vec<Vec<u8>>>> = Arc::default();
        let connect_flags: Arc<std::sync::Mutex<Vec<u8>>> = Arc::default();
        let current: Arc<std::sync::Mutex<Option<TcpStream>>> = Arc::default();
        let (certs_t, current_t, flags_t) = (
            Arc::clone(&certs),
            Arc::clone(&current),
            Arc::clone(&connect_flags),
        );
        thread::spawn(move || {
            for sock in listener.incoming() {
                let Ok(sock) = sock else { break };
                let Ok(conn) = ServerConnection::new(Arc::clone(&server_cfg)) else {
                    continue;
                };
                *current_t.lock().unwrap() = sock.try_clone().ok();
                let certs = Arc::clone(&certs_t);
                let flags = Arc::clone(&flags_t);
                thread::spawn(move || {
                    let mut tls = StreamOwned::new(conn, sock);
                    while tls.conn.is_handshaking() {
                        if tls.conn.complete_io(&mut tls.sock).is_err() {
                            return;
                        }
                    }
                    if let Some(chain) = tls.conn.peer_certificates() {
                        certs.lock().unwrap().push(chain[0].to_vec());
                    }
                    serve_mqtt(&mut tls, &flags);
                });
            }
        });
        Self {
            port,
            certs,
            connect_flags,
            current,
            resolver,
        }
    }

    /// The connections from now on get this server certificate.
    fn serve(&self, cert_pem: &str, key_pem: &str) {
        *self.resolver.key.lock().unwrap() = HelloRecorder::certified(cert_pem, key_pem);
    }

    /// The server name and the ALPN offer of each ClientHello, completed
    /// handshake or not.
    fn hellos(&self) -> Vec<Hello> {
        self.resolver.hellos.lock().unwrap().clone()
    }

    /// Closes the connection that stands, without DISCONNECT.
    fn close(&self) {
        if let Some(s) = self.current.lock().unwrap().take() {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
    }

    fn presented(&self) -> Vec<Vec<u8>> {
        self.certs.lock().unwrap().clone()
    }
}

/// The broker side of one MQTT connection: CONNACK, SUBACK, PUBACK,
/// PINGRESP, until the connection ends; the Connect Flags of the CONNECT go
/// into `flags`.
fn serve_mqtt<S: Read + Write>(s: &mut S, flags: &std::sync::Mutex<Vec<u8>>) {
    use zerodds_mqtt_bridge::net::{frame_packet, read_packet};
    while let Ok((byte0, body)) = read_packet(s) {
        let answer = match byte0 >> 4 {
            1 => {
                // Protocol Name (2 + 4 octets), Protocol Level, Connect Flags.
                flags.lock().unwrap().push(body[7]);
                Some(frame_packet(0x20, &[0, 0, 0]).unwrap())
            }
            8 => {
                // SUBACK: the identifier, no property, QoS 0 granted.
                let mut b = body[..2].to_vec();
                b.extend([0, 0]);
                Some(frame_packet(0x90, &b).unwrap())
            }
            3 if (byte0 >> 1) & 0x03 == 1 => {
                let topic_len = usize::from(u16::from_be_bytes([body[0], body[1]]));
                let mut b = body[2 + topic_len..4 + topic_len].to_vec();
                b.push(0);
                Some(frame_packet(0x40, &b).unwrap())
            }
            12 => Some(vec![0xD0, 0x00]),
            14 => return,
            _ => None,
        };
        if let Some(a) = answer {
            if s.write_all(&a).is_err() || s.flush().is_err() {
                return;
            }
        }
    }
}

/// The DER of the first certificate of `pem`.
fn der_of(pem: &str) -> Vec<u8> {
    load_certs(pem)[0].to_vec()
}

/// The daemon towards `broker` with the client certificate in `dir`
/// (`client.pem`, `client.key`), reloading with `load`.
fn rotating_daemon(
    broker: &CertBroker,
    dir: &std::path::Path,
    ca_pem: &str,
    next: &Arc<std::sync::Mutex<DaemonConfig>>,
) -> (
    zerodds_mqtt_bridge::daemon::server::DaemonHandle,
    zerodds_test_domains::DomainLease,
) {
    std::fs::write(dir.join("ca.pem"), ca_pem).unwrap();
    let lease = zerodds_test_domains::lease_domain();
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.domain = lease.id();
    cfg.broker_url = format!("mqtts://127.0.0.1:{}", broker.port);
    cfg.client_id = "rotating-cli".into();
    cfg.session_dir = dir.join("sessions").display().to_string();
    cfg.broker_tls_enabled = true;
    cfg.broker_tls_ca_file = dir.join("ca.pem").display().to_string();
    cfg.broker_tls_client_cert_file = dir.join("client.pem").display().to_string();
    cfg.broker_tls_client_key_file = dir.join("client.key").display().to_string();
    cfg.broker_tls_server_name = "localhost".into();
    cfg.reconnect.initial_delay_ms = 100;
    cfg.reconnect.max_delay_ms = 400;
    *next.lock().unwrap() = cfg.clone();
    let n = Arc::clone(next);
    let d = zerodds_mqtt_bridge::daemon::server::start_with_reload(
        cfg,
        zerodds_dcps::runtime::RuntimeConfig::default(),
        Some(Box::new(move || Ok(n.lock().unwrap().clone()))),
    )
    .expect("daemon start");
    (d, lease)
}

fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !cond() {
        assert!(std::time::Instant::now() < deadline, "{what} within 10 s");
        thread::sleep(Duration::from_millis(20));
    }
}

/// Spec §7.2 `mtls`: the daemon presents the client certificate of
/// `mqtt.tls.cert_file`, and its CONNECT carries neither User Name nor
/// Password (Connect Flags bits 7 and 6 clear); without `auth.mode` a
/// client certificate and no credentials are `mtls`.
#[test]
fn mqttbridge_7_2_mtls_sends_no_user_name_and_no_password() {
    let (srv_cert, srv_key) = gen_self_signed_for("localhost");
    let (a_cert, a_key) = gen_self_signed_for("client-a");
    let dir = write_temp("mtls-marker", b"")
        .parent()
        .unwrap()
        .to_path_buf();
    std::fs::write(dir.join("client.pem"), &a_cert).unwrap();
    std::fs::write(dir.join("client.key"), &a_key).unwrap();
    let broker = CertBroker::start(&srv_cert, &srv_key);
    let next = Arc::new(std::sync::Mutex::new(DaemonConfig::default_for_dev()));
    let (mut d, _lease) = rotating_daemon(&broker, &dir, &srv_cert, &next);
    assert_eq!(
        Credentials::of(&next.lock().unwrap()).kind(),
        AuthKind::Mtls
    );
    wait_for("the CONNECT", || {
        !broker.connect_flags.lock().unwrap().is_empty()
    });
    assert_eq!(broker.presented()[0], der_of(&a_cert));
    let flags = broker.connect_flags.lock().unwrap()[0];
    assert_eq!(flags & 0xC0, 0, "Connect Flags {flags:#04x}");
    d.shutdown();
}

/// Spec §7.1, §9.2: after SIGHUP the daemon reads the client certificate
/// and key again; the connection that stands keeps the old one, the next
/// connection (§9.3, after the broker closed the first) presents the new
/// one.
#[test]
fn mqttbridge_7_1_a_sighup_puts_the_new_client_certificate_into_the_next_connection() {
    let (srv_cert, srv_key) = gen_self_signed_for("localhost");
    let (a_cert, a_key) = gen_self_signed_for("client-a");
    let (b_cert, b_key) = gen_self_signed_for("client-b");
    let dir = write_temp("rotate-marker", b"")
        .parent()
        .unwrap()
        .to_path_buf();
    std::fs::write(dir.join("client.pem"), &a_cert).unwrap();
    std::fs::write(dir.join("client.key"), &a_key).unwrap();
    let broker = CertBroker::start(&srv_cert, &srv_key);
    let next = Arc::new(std::sync::Mutex::new(DaemonConfig::default_for_dev()));
    let (mut d, _lease) = rotating_daemon(&broker, &dir, &srv_cert, &next);
    wait_for("the first connection", || broker.presented().len() == 1);
    assert_eq!(broker.presented()[0], der_of(&a_cert));
    std::fs::write(dir.join("client.pem"), &b_cert).unwrap();
    std::fs::write(dir.join("client.key"), &b_key).unwrap();
    d.reload_flag.store(true, Ordering::SeqCst);
    thread::sleep(Duration::from_millis(600));
    assert_eq!(broker.presented().len(), 1, "the connection stands");
    broker.close();
    wait_for("the next connection", || broker.presented().len() == 2);
    assert_eq!(broker.presented()[1], der_of(&b_cert));
    d.shutdown();
}

/// Spec §7.1: a reload whose TLS material does not load changes nothing;
/// the next connection presents the certificate of before.
#[test]
fn mqttbridge_7_1_a_failed_reload_keeps_the_old_certificate() {
    let (srv_cert, srv_key) = gen_self_signed_for("localhost");
    let (a_cert, a_key) = gen_self_signed_for("client-a");
    let dir = write_temp("rotate-fail-marker", b"")
        .parent()
        .unwrap()
        .to_path_buf();
    std::fs::write(dir.join("client.pem"), &a_cert).unwrap();
    std::fs::write(dir.join("client.key"), &a_key).unwrap();
    let broker = CertBroker::start(&srv_cert, &srv_key);
    let next = Arc::new(std::sync::Mutex::new(DaemonConfig::default_for_dev()));
    let (mut d, _lease) = rotating_daemon(&broker, &dir, &srv_cert, &next);
    wait_for("the first connection", || broker.presented().len() == 1);
    next.lock().unwrap().broker_tls_client_key_file = dir.join("missing.key").display().to_string();
    d.reload_flag.store(true, Ordering::SeqCst);
    thread::sleep(Duration::from_millis(600));
    broker.close();
    wait_for("the next connection", || broker.presented().len() == 2);
    assert_eq!(broker.presented()[1], der_of(&a_cert));
    d.shutdown();
}

/// Spec §7.1, §9.2: a SIGHUP takes `mqtt.tls.server_name` and
/// `mqtt.tls.alpn`; the connection that stands keeps what it offered, the
/// next connection (after the broker closed the first) offers the new
/// server name in its SNI and the new ALPN list.
#[test]
fn mqttbridge_7_1_a_sighup_puts_the_new_server_name_and_alpn_into_the_next_connection() {
    let ck = rcgen::generate_simple_self_signed(vec![
        "localhost".to_string(),
        "broker.example".to_string(),
    ])
    .unwrap();
    let (srv_cert, srv_key) = (ck.cert.pem(), ck.key_pair.serialize_pem());
    let (a_cert, a_key) = gen_self_signed_for("client-a");
    let dir = write_temp("rotate-name-marker", b"")
        .parent()
        .unwrap()
        .to_path_buf();
    std::fs::write(dir.join("client.pem"), &a_cert).unwrap();
    std::fs::write(dir.join("client.key"), &a_key).unwrap();
    let broker = CertBroker::start(&srv_cert, &srv_key);
    let next = Arc::new(std::sync::Mutex::new(DaemonConfig::default_for_dev()));
    let (mut d, _lease) = rotating_daemon(&broker, &dir, &srv_cert, &next);
    wait_for("the first connection", || broker.presented().len() == 1);
    assert_eq!(
        broker.hellos(),
        [(Some("localhost".to_string()), vec![b"mqtt".to_vec()])]
    );
    {
        let mut n = next.lock().unwrap();
        n.broker_tls_server_name = "broker.example".into();
        n.broker_tls_alpn = Some(vec!["x-amzn-mqtt-ca".into(), "mqtt".into()]);
    }
    d.reload_flag.store(true, Ordering::SeqCst);
    thread::sleep(Duration::from_millis(600));
    assert_eq!(broker.presented().len(), 1, "the connection stands");
    broker.close();
    wait_for("the next connection", || broker.presented().len() == 2);
    assert_eq!(
        broker.hellos().last().cloned(),
        Some((
            Some("broker.example".to_string()),
            vec![b"x-amzn-mqtt-ca".to_vec(), b"mqtt".to_vec()]
        ))
    );
    d.shutdown();
}

/// Spec §7.1: a SIGHUP reads `mqtt.tls.ca_file` again. The broker moves
/// to a certificate the CA of the start does not trust: the reconnects
/// fail their handshake and are tried again (§9.3); after the CA file
/// names the new certificate and a SIGHUP, the next attempt stands.
#[test]
fn mqttbridge_7_1_a_sighup_reads_the_ca_file_again() {
    let (old_cert, old_key) = gen_self_signed_for("localhost");
    let (new_cert, new_key) = gen_self_signed_for("localhost");
    let (a_cert, a_key) = gen_self_signed_for("client-a");
    let dir = write_temp("rotate-ca-marker", b"")
        .parent()
        .unwrap()
        .to_path_buf();
    std::fs::write(dir.join("client.pem"), &a_cert).unwrap();
    std::fs::write(dir.join("client.key"), &a_key).unwrap();
    let broker = CertBroker::start(&old_cert, &old_key);
    let next = Arc::new(std::sync::Mutex::new(DaemonConfig::default_for_dev()));
    let (mut d, _lease) = rotating_daemon(&broker, &dir, &old_cert, &next);
    wait_for("the first connection", || broker.presented().len() == 1);
    broker.serve(&new_cert, &new_key);
    broker.close();
    wait_for("three refused handshakes", || broker.hellos().len() >= 4);
    assert_eq!(broker.presented().len(), 1, "an untrusted broker accepted");
    std::fs::write(dir.join("ca.pem"), &new_cert).unwrap();
    d.reload_flag.store(true, Ordering::SeqCst);
    wait_for("the connection under the new CA", || {
        broker.presented().len() == 2
    });
    d.shutdown();
}
