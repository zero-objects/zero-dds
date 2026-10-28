#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

//! TLS termination through `run_server` (DDS-AMQP 1.0 §10.1): the server
//! used to run every connection in plaintext, whatever `<tls>` said, while
//! the handler was told TLS was active. Here a TLS client finishes the
//! handshake, is offered PLAIN (§10.2.1 allows it on TLS) and authenticates
//! against the credential store.

use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use amqp_dds_endpoint::auth::{CredentialStore, SaslPolicy};
use amqp_dds_endpoint::frame_io::{
    AmqpProtocol, read_frame, read_protocol_header, write_frame, write_protocol_header,
};
use amqp_dds_endpoint::tls::{
    ClientTlsConfig, ServerTlsConfig, build_client_config, build_server_config, connect_client,
};
use amqp_dds_endpoint::{ServerConfig, run_server};
use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_bridge::frame::FrameType;
use zerodds_amqp_bridge::performatives::{self, decode_performative, encode_performative};
use zerodds_amqp_endpoint::MetricsHub;
use zerodds_amqp_endpoint::sasl::SaslMechanism;

#[test]
fn a_tls_client_is_offered_plain_and_authenticated_against_the_store() {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let cert_pem = cert.cert.pem().into_bytes();
    let server_tls = build_server_config(&ServerTlsConfig {
        cert_pem: cert_pem.clone(),
        key_pem: cert.key_pair.serialize_pem().into_bytes(),
        client_ca_pem: None,
        require_client_cert: false,
    })
    .unwrap();
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let mut cfg = ServerConfig::default_listen();
    cfg.listen_addr = format!("127.0.0.1:{port}");
    cfg.tls = Some(server_tls);
    cfg.sasl = SaslPolicy {
        enabled: vec![SaslMechanism::Plain],
        store: Some(
            CredentialStore::parse(&CredentialStore::entry_line(
                "alice", "wonder", b"salt", 4096,
            ))
            .unwrap(),
        ),
    };
    let shutdown = Arc::new(AtomicBool::new(false));
    let server = {
        let shutdown = Arc::clone(&shutdown);
        std::thread::spawn(move || run_server(cfg, Arc::new(MetricsHub::new()), shutdown))
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    let tcp = loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(s) => break s,
            Err(e) => {
                assert!(Instant::now() < deadline, "server not up: {e}");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    };
    tcp.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let client_tls = build_client_config(&ClientTlsConfig {
        trust_anchors_pem: cert_pem,
        client_auth: None,
    })
    .unwrap();
    let mut s = connect_client(client_tls, "localhost", tcp).unwrap();

    write_protocol_header(&mut s, AmqpProtocol::Sasl).unwrap();
    assert_eq!(
        read_protocol_header(&mut s).unwrap().protocol,
        AmqpProtocol::Sasl
    );
    let f = read_frame(&mut s, 65_536).unwrap();
    let (_, body, _) = decode_performative(&f.body).unwrap();
    assert_eq!(performatives::sasl_mechanisms_from_body(&body), ["PLAIN"]);
    let init = encode_performative(
        0x41,
        &AmqpExtValue::List(vec![
            AmqpExtValue::Symbol("PLAIN".into()),
            AmqpExtValue::Binary(performatives::sasl_plain_response("alice", "wonder")),
        ]),
    )
    .unwrap();
    write_frame(&mut s, FrameType::Sasl, 0, &init).unwrap();
    let f = read_frame(&mut s, 65_536).unwrap();
    let (d, body, _) = decode_performative(&f.body).unwrap();
    assert_eq!(d, 0x44);
    assert_eq!(performatives::sasl_outcome_code(&body), Some(0));

    shutdown.store(true, Ordering::SeqCst);
    server.join().unwrap().unwrap();
}

/// EXTERNAL (RFC 4422 §7) over mTLS: a CA signs the server's and the
/// client's certificate, the server requires client certificates, offers
/// EXTERNAL and takes the client certificate as the identity.
#[test]
fn an_mtls_client_authenticates_with_external() {
    use rcgen::{CertificateParams, IsCa, KeyPair};
    let mut ca_p = CertificateParams::new(vec!["endpoint test CA".to_string()]).unwrap();
    ca_p.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca_key = KeyPair::generate().unwrap();
    let ca = ca_p.self_signed(&ca_key).unwrap();
    let leaf = |name: &str| {
        let mut p = CertificateParams::new(vec![name.to_string()]).unwrap();
        p.is_ca = IsCa::NoCa;
        let k = KeyPair::generate().unwrap();
        let c = p.signed_by(&k, &ca, &ca_key).unwrap();
        (c.pem().into_bytes(), k.serialize_pem().into_bytes())
    };
    let (server_cert, server_key) = leaf("localhost");
    let (client_cert, client_key) = leaf("amqp-client");
    let ca_pem = ca.pem().into_bytes();
    let server_tls = build_server_config(&ServerTlsConfig {
        cert_pem: server_cert,
        key_pem: server_key,
        client_ca_pem: Some(ca_pem.clone()),
        require_client_cert: true,
    })
    .unwrap();
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let mut cfg = ServerConfig::default_listen();
    cfg.listen_addr = format!("127.0.0.1:{port}");
    cfg.tls = Some(server_tls);
    cfg.sasl = SaslPolicy {
        enabled: vec![SaslMechanism::External],
        store: None,
    };
    let shutdown = Arc::new(AtomicBool::new(false));
    let server = {
        let shutdown = Arc::clone(&shutdown);
        std::thread::spawn(move || run_server(cfg, Arc::new(MetricsHub::new()), shutdown))
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    let tcp = loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(s) => break s,
            Err(e) => {
                assert!(Instant::now() < deadline, "server not up: {e}");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    };
    tcp.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let client_tls = build_client_config(&ClientTlsConfig {
        trust_anchors_pem: ca_pem,
        client_auth: Some((client_cert, client_key)),
    })
    .unwrap();
    let mut s = connect_client(client_tls, "localhost", tcp).unwrap();
    write_protocol_header(&mut s, AmqpProtocol::Sasl).unwrap();
    read_protocol_header(&mut s).unwrap();
    let f = read_frame(&mut s, 65_536).unwrap();
    let (_, body, _) = decode_performative(&f.body).unwrap();
    assert_eq!(
        performatives::sasl_mechanisms_from_body(&body),
        ["EXTERNAL"]
    );
    let init = encode_performative(
        0x41,
        &AmqpExtValue::List(vec![AmqpExtValue::Symbol("EXTERNAL".into())]),
    )
    .unwrap();
    write_frame(&mut s, FrameType::Sasl, 0, &init).unwrap();
    let f = read_frame(&mut s, 65_536).unwrap();
    let (d, body, _) = decode_performative(&f.body).unwrap();
    assert_eq!(d, 0x44);
    assert_eq!(performatives::sasl_outcome_code(&body), Some(0));
    shutdown.store(true, Ordering::SeqCst);
    server.join().unwrap().unwrap();
}
