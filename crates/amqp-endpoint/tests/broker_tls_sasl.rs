// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `broker::connect` over TLS with the SASL mechanisms RabbitMQ does not
//! offer: SCRAM-SHA-256 (verified both ways: the broker checks the proof,
//! the client the server signature) and EXTERNAL (the TLS client
//! certificate). The peer is a server in this test: rustls with a required
//! client certificate and ALPN `amqp`, SASL through `ScramServerExchange`
//! (checked against the RFC 7677 vector in `scram.rs`), then open.

#![cfg(feature = "std")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_bridge::frame::{
    FrameHeader, FrameType, decode_frame_header, encode_frame_header,
};
use zerodds_amqp_bridge::performatives::{decode_performative, descriptor, encode_performative};
use zerodds_amqp_endpoint::broker::{self, ConnectParams, SaslCredentials};
use zerodds_amqp_endpoint::scram::{ScramServerExchange, ScramServerSecrets, ScramStep};

struct Pki {
    ca_pem: String,
    server_cert: CertificateDer<'static>,
    server_key: PrivatePkcs8KeyDer<'static>,
    client_cert: CertificateDer<'static>,
    client_key: PrivatePkcs8KeyDer<'static>,
}

fn pki() -> Pki {
    let ca_key = rcgen::KeyPair::generate().unwrap();
    let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let issue = |names: Vec<String>| {
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = rcgen::CertificateParams::new(names)
            .unwrap()
            .signed_by(&key, &ca, &ca_key)
            .unwrap();
        (
            cert.der().clone(),
            PrivatePkcs8KeyDer::from(key.serialize_der()),
        )
    };
    let (server_cert, server_key) = issue(vec!["localhost".into()]);
    let (client_cert, client_key) = issue(vec!["bridge.example".into()]);
    Pki {
        ca_pem: ca.pem(),
        server_cert,
        server_key,
        client_cert,
        client_key,
    }
}

fn write_frame(w: &mut impl Write, t: FrameType, body: &[u8]) {
    let h = encode_frame_header(FrameHeader {
        size: 8 + body.len() as u32,
        doff: 2,
        frame_type: t,
        channel: 0,
    });
    w.write_all(&h).unwrap();
    w.write_all(body).unwrap();
    w.flush().unwrap();
}

fn read_frame(r: &mut impl Read) -> (u64, AmqpExtValue) {
    let mut h = [0u8; 8];
    r.read_exact(&mut h).unwrap();
    let hdr = decode_frame_header(&h).unwrap();
    let mut body = vec![0u8; hdr.size as usize - 8];
    r.read_exact(&mut body).unwrap();
    let (d, v, _) = decode_performative(&body).unwrap();
    (d, v)
}

fn sasl_frame(desc: u64, fields: Vec<AmqpExtValue>) -> Vec<u8> {
    encode_performative(desc, &AmqpExtValue::List(fields)).unwrap()
}

/// What the server saw of the client.
#[derive(Debug)]
struct Seen {
    alpn: Option<Vec<u8>>,
    client_cert: bool,
    mechanism: String,
    authenticated: bool,
}

/// One connection: TLS, SASL offering `offer`, then open when the SASL
/// outcome was ok.
fn serve(pki: &Pki, offer: &'static str, password: &'static str) -> (String, mpsc::Receiver<Seen>) {
    let mut roots = rustls::RootCertStore::empty();
    for c in rustls_pemfile::certs(&mut pki.ca_pem.as_bytes()) {
        roots.add(c.unwrap()).unwrap();
    }
    let provider = Arc::new(zerodds_bridge_security::tls_provider());
    let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
        Arc::new(roots),
        Arc::clone(&provider),
    )
    .build()
    .unwrap();
    let mut cfg = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_client_cert_verifier(verifier)
        .with_single_cert(
            vec![pki.server_cert.clone()],
            pki.server_key.clone_key().into(),
        )
        .unwrap();
    cfg.alpn_protocols = vec![b"amqp".to_vec()];
    let cfg = Arc::new(cfg);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let (sock, _) = listener.accept().unwrap();
        sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let conn = rustls::ServerConnection::new(cfg).unwrap();
        let mut tls = rustls::StreamOwned::new(conn, sock);
        let mut hdr = [0u8; 8];
        if tls.read_exact(&mut hdr).is_err() {
            return;
        }
        let mut seen = Seen {
            alpn: tls.conn.alpn_protocol().map(<[u8]>::to_vec),
            client_cert: tls.conn.peer_certificates().is_some_and(|c| !c.is_empty()),
            mechanism: String::new(),
            authenticated: false,
        };
        tls.write_all(&hdr).unwrap(); // SASL header back
        write_frame(
            &mut tls,
            FrameType::Sasl,
            &sasl_frame(
                descriptor::SASL_MECHANISMS,
                vec![AmqpExtValue::Array(vec![AmqpExtValue::Symbol(
                    offer.into(),
                )])],
            ),
        );
        let (_, init) = read_frame(&mut tls);
        let AmqpExtValue::List(f) = init else {
            panic!()
        };
        seen.mechanism = match &f[0] {
            AmqpExtValue::Symbol(s) => s.clone(),
            _ => String::new(),
        };
        let initial = match f.get(1) {
            Some(AmqpExtValue::Binary(b)) => String::from_utf8(b.clone()).unwrap(),
            _ => String::new(),
        };
        let (ok, additional) = match offer {
            "EXTERNAL" => (seen.client_cert, None),
            "SCRAM-SHA-256" => {
                let salt = b"zerodds-test-salt";
                let (exchange, server_first) =
                    ScramServerExchange::start(&initial, "srvnonce42", |u| {
                        (u == "bridge")
                            .then(|| ScramServerSecrets::from_password(password, salt, 4096))
                    })
                    .unwrap();
                write_frame(
                    &mut tls,
                    FrameType::Sasl,
                    &sasl_frame(
                        descriptor::SASL_CHALLENGE,
                        vec![AmqpExtValue::Binary(server_first.into_bytes())],
                    ),
                );
                let (_, resp) = read_frame(&mut tls);
                let AmqpExtValue::List(r) = resp else {
                    panic!()
                };
                let AmqpExtValue::Binary(fin) = &r[0] else {
                    panic!()
                };
                match exchange.finish(std::str::from_utf8(fin).unwrap()) {
                    ScramStep::Success { server_final, .. } => (true, Some(server_final)),
                    ScramStep::Failure(_) => (false, None),
                }
            }
            _ => (false, None),
        };
        seen.authenticated = ok;
        let mut outcome = vec![AmqpExtValue::Ubyte(if ok { 0 } else { 1 })];
        if let Some(a) = additional {
            outcome.push(AmqpExtValue::Binary(a.into_bytes()));
        }
        write_frame(
            &mut tls,
            FrameType::Sasl,
            &sasl_frame(descriptor::SASL_OUTCOME, outcome),
        );
        if ok {
            let mut amqp = [0u8; 8];
            tls.read_exact(&mut amqp).unwrap();
            tls.write_all(&amqp).unwrap();
            let (d, _) = read_frame(&mut tls);
            assert_eq!(d, descriptor::OPEN);
            write_frame(
                &mut tls,
                FrameType::Amqp,
                &sasl_frame(
                    descriptor::OPEN,
                    vec![
                        AmqpExtValue::Str("test-broker".into()),
                        AmqpExtValue::Null,
                        AmqpExtValue::Uint(65536),
                    ],
                ),
            );
        }
        let _ = tx.send(seen);
        std::thread::sleep(Duration::from_millis(300));
    });
    (addr, rx)
}

fn params(pki: &Pki, addr: String, sasl: SaslCredentials, with_client_cert: bool) -> ConnectParams {
    let mut roots = rustls::RootCertStore::empty();
    for c in rustls_pemfile::certs(&mut pki.ca_pem.as_bytes()) {
        roots.add(c.unwrap()).unwrap();
    }
    let builder = rustls::ClientConfig::builder_with_provider(Arc::new(
        zerodds_bridge_security::tls_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_root_certificates(roots);
    let tls = if with_client_cert {
        builder
            .with_client_auth_cert(
                vec![pki.client_cert.clone()],
                pki.client_key.clone_key().into(),
            )
            .unwrap()
    } else {
        builder.with_no_client_auth()
    };
    ConnectParams {
        addr,
        host: "localhost".into(),
        tls: Some(Arc::new(tls)),
        sasl,
        container_id: "tls-sasl-test".into(),
        hostname: None,
        max_frame_size: 65536,
        channel_max: 16,
        idle_time_out_ms: 0,
        properties: vec![],
        capabilities: vec![],
        timeout: Duration::from_secs(5),
    }
}

#[test]
fn external_authenticates_with_the_tls_client_certificate_and_alpn_amqp() {
    let pki = pki();
    let (addr, seen) = serve(&pki, "EXTERNAL", "");
    let r = broker::connect(&params(&pki, addr, SaslCredentials::External, true));
    assert!(r.is_ok(), "{:?}", r.as_ref().err());
    let seen = seen.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(r.is_ok(), "{:?}", r.err());
    assert_eq!(seen.alpn.as_deref(), Some(&b"amqp"[..]));
    assert!(seen.client_cert);
    assert_eq!(seen.mechanism, "EXTERNAL");
}

#[test]
fn scram_sha_256_authenticates_and_a_wrong_password_is_refused() {
    let pki = pki();
    let (addr, seen) = serve(&pki, "SCRAM-SHA-256", "correct horse");
    let ok = broker::connect(&params(
        &pki,
        addr,
        SaslCredentials::ScramSha256 {
            username: "bridge".into(),
            password: "correct horse".into(),
        },
        true,
    ));
    assert!(ok.is_ok(), "{:?}", ok.err());
    assert!(
        seen.recv_timeout(Duration::from_secs(5))
            .unwrap()
            .authenticated
    );

    let (addr, seen) = serve(&pki, "SCRAM-SHA-256", "correct horse");
    let refused = broker::connect(&params(
        &pki,
        addr,
        SaslCredentials::ScramSha256 {
            username: "bridge".into(),
            password: "wrong".into(),
        },
        true,
    ));
    assert!(
        !seen
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .authenticated
    );
    let err = refused.err().expect("a wrong password was accepted");
    assert_eq!(err.stage, broker::ConnectStage::Sasl, "{err}");
    assert_eq!(
        err.error.kind(),
        std::io::ErrorKind::PermissionDenied,
        "{err}"
    );
}

/// The broker offers only EXTERNAL; a client configured for SCRAM stops
/// with both names instead of trying another mechanism.
#[test]
fn a_mechanism_the_broker_does_not_offer_is_refused_by_name() {
    let pki = pki();
    let (addr, _seen) = serve(&pki, "EXTERNAL", "");
    let err = broker::connect(&params(
        &pki,
        addr,
        SaslCredentials::ScramSha256 {
            username: "bridge".into(),
            password: "x".into(),
        },
        true,
    ))
    .err()
    .expect("connected without the configured mechanism");
    let text = err.to_string();
    assert!(
        text.contains("EXTERNAL") && text.contains("SCRAM-SHA-256"),
        "{text}"
    );
}
