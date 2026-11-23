// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-mqtt-bridge` with `zerodds-bridge-security`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-bridge-security`: `build_client_tls_connector`, `SecurityError`, `mtls_subject_from_der`, `rustls_pemfile`, `ScramClient`, `oauthbearer_initial_response`, `OAUTHBEARER_ERROR_ACK`, `jwt_subject_unverified`, `SCRAM_SHA_256`, `OAUTHBEARER` and `tls_provider` (`src/daemon/security.rs`, `acl.rs`, `auth.rs`, feature `daemon`); `serve_tls_handshake` (`src/server/tls.rs`, feature `tls`, the TLS listeners of the broker server): the handshake with the caller's `rustls::ServerConfig` on the accepted socket, each read of it bounded by the timeout, the same socket and the finished connection returned.
//!
//! Oracles: RFC 7677 §3 (the SCRAM-SHA-256 exchange of user `user` with password `pencil`, octet for octet); RFC 7628 §3.1 and the example of §4.1 (the OAUTHBEARER initial client response) and §3.2.3 (`%x01` answers an error); RFC 7519 §4.1.2 (`sub`); RFC 4514 §2 (the DN as a string, the RDNs in reverse order); RFC 8446 §9.1 (TLS_AES_128_GCM_SHA256 must be implemented); RFC 8446 §2 and RFC 7301 §3.2 (the handshake, then application data; the server selects one ALPN protocol both offer); MQTT 5.0 §4.2 (TLS below MQTT).
#![allow(clippy::unwrap_used, clippy::panic)]

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use zerodds_bridge_security::{
    OAUTHBEARER, OAUTHBEARER_ERROR_ACK, SCRAM_SHA_256, ScramClient, build_client_tls_connector,
    jwt_subject_unverified, mtls_subject_from_der, oauthbearer_initial_response, rustls,
    serve_tls_handshake, tls_provider,
};

#[test]
fn mqttbridge_7_1_the_client_tls_connector_loads_the_ca_and_refuses_a_missing_file() {
    let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let dir = std::env::temp_dir().join(format!("zerodds-mqtt-contract-bs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ca = dir.join("ca.pem");
    std::fs::write(&ca, ck.cert.pem()).unwrap();
    let cfg = build_client_tls_connector(Some(&ca), None, None).unwrap();
    assert!(!cfg.client_auth_cert_resolver.has_certs());
    assert!(build_client_tls_connector(Some(&dir.join("missing.pem")), None, None).is_err());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn mqttbridge_7_3_the_subject_of_a_certificate_is_its_rfc_4514_dn_and_its_fingerprint() {
    let mut params = rcgen::CertificateParams::new(vec!["bridge".to_string()]).unwrap();
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "bridge-01");
    params
        .distinguished_name
        .push(rcgen::DnType::OrganizationName, "Plant");
    let key = rcgen::KeyPair::generate().unwrap();
    let cert = params.self_signed(&key).unwrap();
    let s = mtls_subject_from_der(cert.der());
    assert_eq!(s.name, "O=Plant,CN=bridge-01");
    let print = s.fingerprint.unwrap();
    assert!(
        print.starts_with("mtls:") && print.len() == 5 + 64,
        "{print}"
    );
}

#[test]
fn mqttbridge_7_2_the_scram_client_runs_the_exchange_of_rfc_7677() {
    assert_eq!(SCRAM_SHA_256, "SCRAM-SHA-256");
    let mut c = ScramClient::with_nonce("user", "pencil", "rOprNGfwEbeRWgbNEkqO").unwrap();
    assert_eq!(c.client_first(), "n,,n=user,r=rOprNGfwEbeRWgbNEkqO");
    assert_eq!(
        c.client_final(
            "r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096"
        )
        .unwrap(),
        "c=biws,r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,p=dHzbZapWIk4jUhN+Ute9ytag9zjfMHgsqmmiz7AndVQ="
    );
    c.verify_server_final("v=6rriTRBi23WpRR/wtup+mMhUZUn/dB5nLTJRsjl95G4=")
        .unwrap();
    assert!(c.is_done());
}

#[test]
fn mqttbridge_7_2_the_oauthbearer_response_is_the_one_of_rfc_7628() {
    assert_eq!(OAUTHBEARER, "OAUTHBEARER");
    assert_eq!(
        oauthbearer_initial_response(
            "vF9dft4qmTc2Nvb3RlckBhbHRhdmlzdGEuY29tCg==",
            Some("server.example.com"),
            Some(143)
        ),
        b"n,,\x01host=server.example.com\x01port=143\x01auth=Bearer vF9dft4qmTc2Nvb3RlckBhbHRhdmlzdGEuY29tCg==\x01\x01".to_vec()
    );
    assert_eq!(OAUTHBEARER_ERROR_ACK, b"\x01");
}

#[test]
fn mqttbridge_7_3_the_sub_of_a_jwt_is_its_subject() {
    // {"alg":"none"} . {"sub":"svc-7"}
    assert_eq!(
        jwt_subject_unverified("eyJhbGciOiJub25lIn0.eyJzdWIiOiJzdmMtNyJ9.").as_deref(),
        Some("svc-7")
    );
    assert_eq!(jwt_subject_unverified("opaque-token"), None);
}

#[test]
fn mqttbridge_7_1_the_tls_provider_implements_tls_aes_128_gcm_sha256() {
    let provider = tls_provider();
    assert!(
        provider
            .cipher_suites
            .iter()
            .any(|s| s.suite() == rustls::CipherSuite::TLS13_AES_128_GCM_SHA256)
    );
}

#[test]
fn mqtt5_4_2_serve_tls_handshake_finishes_the_handshake_or_fails_within_its_timeout() {
    let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let mut server = rustls::ServerConfig::builder_with_provider(Arc::new(tls_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![ck.cert.der().clone()],
            rustls::pki_types::PrivatePkcs8KeyDer::from(ck.key_pair.serialize_der()).into(),
        )
        .unwrap();
    server.alpn_protocols = vec![b"mqtt".to_vec()];
    let server = Arc::new(server);
    let mut roots = rustls::RootCertStore::empty();
    roots.add(ck.cert.der().clone()).unwrap();
    let mut client = rustls::ClientConfig::builder_with_provider(Arc::new(tls_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    client.alpn_protocols = vec![b"x-other".to_vec(), b"mqtt".to_vec()];
    let client = Arc::new(client);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    // A client that finishes the handshake and sends application data.
    let peer = thread::spawn(move || {
        let tcp = TcpStream::connect(addr).unwrap();
        let name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
        let conn = rustls::ClientConnection::new(client, name).unwrap();
        let mut tls = rustls::StreamOwned::new(conn, tcp);
        tls.write_all(b"ping").unwrap();
        tls.flush().unwrap();
        let mut answer = [0u8; 4];
        tls.read_exact(&mut answer).unwrap();
        answer
    });
    let (tcp, _) = listener.accept().unwrap();
    let peer_addr = tcp.peer_addr().unwrap();
    let (tcp, conn, subject) =
        serve_tls_handshake(Arc::clone(&server), tcp, Duration::from_secs(5)).unwrap();
    assert!(!conn.is_handshaking());
    assert_eq!(
        tcp.peer_addr().unwrap(),
        peer_addr,
        "the same socket comes back"
    );
    assert_eq!(conn.alpn_protocol(), Some(&b"mqtt"[..]));
    assert!(subject.is_none(), "no client certificate was asked for");
    let mut tls = rustls::StreamOwned::new(conn, tcp);
    let mut got = [0u8; 4];
    tls.read_exact(&mut got).unwrap();
    assert_eq!(&got, b"ping");
    tls.write_all(b"pong").unwrap();
    tls.flush().unwrap();
    assert_eq!(&peer.join().unwrap(), b"pong");

    // A client that sends nothing: the handshake fails within the timeout.
    let silent = TcpStream::connect(addr).unwrap();
    let (tcp, _) = listener.accept().unwrap();
    let t = Instant::now();
    let e = serve_tls_handshake(Arc::clone(&server), tcp, Duration::from_millis(300)).unwrap_err();
    assert!(
        matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut),
        "{e}"
    );
    assert!(t.elapsed() < Duration::from_secs(3), "{:?}", t.elapsed());
    drop(silent);

    // A client that closes before its ClientHello: the handshake fails at
    // once.
    drop(TcpStream::connect(addr).unwrap());
    let (tcp, _) = listener.accept().unwrap();
    let e = serve_tls_handshake(server, tcp, Duration::from_secs(5)).unwrap_err();
    assert_eq!(e.kind(), ErrorKind::UnexpectedEof, "{e}");
}
