// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! E2E for the DTLS 1.2 CoAP transport (feature `dtls`, RFC 7252 §9): a
//! DTLS client and server complete a real handshake in each of the three
//! modes and exchange a CoAP GET → 2.05 Content over the encrypted channel.
//!
//! The crate doc sits before `#![cfg]` so the (empty) crate still carries
//! documentation when `dtls` is off — otherwise `missing_docs` + `-D warnings`
//! fails the build.
#![cfg(feature = "dtls")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;
use std::time::Duration;

use zerodds_coap_bridge::dtls::{DtlsCredentials, PskStore, parse_secret_hex};
use zerodds_coap_bridge::dtls_transport::{
    DtlsCoapClient, DtlsCoapServer, DtlsError, DtlsPeerIdentity, KeyName,
};
use zerodds_coap_bridge::message::{CoapCode, CoapMessage, MessageType};

const WAIT: Duration = Duration::from_secs(10);

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "zerodds-coap-dtls-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).expect("temp dir");
    d
}

/// A self-signed certificate for `localhost` with the CN `name` and its
/// key, as PEM files. Each certificate has a subject of its own: as trust
/// anchors in one file, two with the same subject would be taken for each
/// other's issuer.
fn self_signed(dir: &std::path::Path, name: &str) -> (PathBuf, PathBuf) {
    let mut params = rcgen::CertificateParams::new(vec!["localhost".to_owned()]).expect("params");
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, format!("coap test {name}"));
    let key = rcgen::KeyPair::generate().expect("key");
    let cert = params.self_signed(&key).expect("cert");
    let cert_path = dir.join(format!("{name}-cert.pem"));
    let key_path = dir.join(format!("{name}-key.pem"));
    std::fs::write(&cert_path, cert.pem()).expect("write cert");
    std::fs::write(&key_path, key.serialize_pem()).expect("write key");
    (cert_path, key_path)
}

/// Accepts `sessions` sessions on `server` and answers each request with a
/// 2.05 carrying `payload`, until each session got `per_session` requests.
fn serve(
    server: DtlsCoapServer,
    sessions: usize,
    payload: &'static [u8],
) -> std::thread::JoinHandle<Vec<Option<DtlsPeerIdentity>>> {
    std::thread::spawn(move || {
        let mut ids = Vec::new();
        let mut open = Vec::new();
        for _ in 0..sessions {
            let mut s = server.accept(WAIT).expect("accept");
            ids.push(s.peer_identity());
            let req = s.recv_message(WAIT).expect("recv").expect("a request");
            assert_eq!(req.code, CoapCode::GET);
            let mut resp = CoapMessage::new(
                MessageType::Acknowledgement,
                CoapCode::CONTENT,
                req.message_id,
            );
            resp.token = req.token.clone();
            resp.payload = payload.to_vec();
            s.send_message(&resp).expect("send");
            open.push(s);
        }
        std::thread::sleep(Duration::from_millis(200));
        ids
    })
}

fn get(client: &mut zerodds_coap_bridge::dtls_transport::DtlsCoapSession, mid: u16) -> CoapMessage {
    let mut req = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, mid);
    req.token = mid.to_be_bytes().to_vec();
    client.send_message(&req).expect("send request");
    let resp = client
        .recv_message(WAIT)
        .expect("recv")
        .expect("a response");
    assert_eq!(resp.token, req.token, "token echoed");
    resp
}

/// Certificate mode (RFC 7252 §9.1.3.3): the server's certificate is checked
/// against the client's trust anchor and the name `localhost`.
#[test]
fn coap_9_dtls_coap_get_content_roundtrip() {
    let dir = scratch("cert");
    let (cert, key) = self_signed(&dir, "server");
    let credentials = DtlsCredentials {
        cert_file: Some(cert.to_string_lossy().into_owned()),
        key_file: Some(key.to_string_lossy().into_owned()),
        ..DtlsCredentials::default()
    };
    let server = DtlsCoapServer::bind_with("127.0.0.1:0".parse().unwrap(), &credentials)
        .expect("bind dtls server");
    let addr = server.local_addr().expect("local addr");
    let srv = serve(server, 1, b"secure-pong");
    let mut client =
        DtlsCoapClient::connect_verified(addr, "localhost", &cert.to_string_lossy(), None, WAIT)
            .expect("dtls handshake");
    assert_eq!(client.cipher(), Some("ECDHE-ECDSA-AES128-CCM8"));
    let resp = get(&mut client, 0x1234);
    assert_eq!(resp.code, CoapCode::CONTENT);
    assert_eq!(resp.payload, b"secure-pong", "payload through DTLS");
    client.close();
    srv.join().expect("server thread");
    let _ = std::fs::remove_dir_all(dir);
}

/// Bridge-Spec §7.1 PSK mode, RFC 7252 §9.1.3.1: `TLS_PSK_WITH_AES_128_CCM_8`
/// with the `coap.dtls.psk` identities, no certificate in the handshake.
#[test]
fn coap_9_dtls_psk_handshake_carries_coap() {
    let secret = parse_secret_hex("000102030405060708090a0b0c0d0e0f").expect("hex secret");
    let mut psk = PskStore::new();
    psk.add("device-001", secret.clone()).expect("psk entry");
    let credentials = DtlsCredentials {
        psk,
        ..DtlsCredentials::default()
    };
    let server = DtlsCoapServer::bind_with("127.0.0.1:0".parse().unwrap(), &credentials)
        .expect("bind psk dtls server");
    let addr = server.local_addr().expect("local addr");
    let srv = serve(server, 1, b"psk-pong");
    let mut client =
        DtlsCoapClient::connect_psk(addr, "device-001", secret, WAIT).expect("psk handshake");
    assert_eq!(client.cipher(), Some("PSK-AES128-CCM8"));
    assert_eq!(get(&mut client, 0x2345).payload, b"psk-pong");
    client.close();
    assert_eq!(
        srv.join().expect("server thread"),
        vec![Some(DtlsPeerIdentity::Psk(b"device-001".to_vec()))]
    );
}

/// An identity the `coap.dtls.psk` list does not hold cannot complete the
/// handshake — the PSK callback has no secret to answer with.
#[test]
fn coap_9_dtls_psk_unknown_identity_is_refused() {
    let mut psk = PskStore::new();
    psk.add("device-001", vec![7u8; 16]).expect("psk entry");
    let credentials = DtlsCredentials {
        psk,
        ..DtlsCredentials::default()
    };
    let server = DtlsCoapServer::bind_with("127.0.0.1:0".parse().unwrap(), &credentials)
        .expect("bind psk dtls server");
    let addr = server.local_addr().expect("local addr");
    let outcome =
        DtlsCoapClient::connect_psk(addr, "intruder", vec![9u8; 16], Duration::from_secs(5));
    assert!(
        outcome.is_err(),
        "an unknown PSK identity must not establish a session"
    );
    assert!(server.accept(Duration::from_millis(500)).is_none());
}

/// RawPublicKey mode (RFC 7252 §9.1.3.2, RFC 7250): both sides show a P-256
/// key without a certificate and accept the other's by its RFC 6920 name
/// (`coap.dtls.rpk_trusted`); TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8. A key that
/// is not trusted gets no session.
#[test]
fn coap_9_1_3_2_raw_public_keys_carry_coap() {
    let dir = scratch("rpk");
    let server_key = rcgen::KeyPair::generate().expect("key");
    let client_key = rcgen::KeyPair::generate().expect("key");
    let stranger = rcgen::KeyPair::generate().expect("key");
    let write = |name: &str, k: &rcgen::KeyPair| {
        let p = dir.join(name);
        std::fs::write(&p, k.serialize_pem()).expect("write key");
        p.to_string_lossy().into_owned()
    };
    let (server_file, client_file, stranger_file) = (
        write("server.pem", &server_key),
        write("client.pem", &client_key),
        write("stranger.pem", &stranger),
    );
    let name_of = |k: &rcgen::KeyPair| KeyName::of_key(&k.public_key_der(), 3).expect("name");
    let credentials = DtlsCredentials {
        rpk_key_file: Some(server_file),
        rpk_trusted: vec![name_of(&client_key).to_nih_uri()],
        ..DtlsCredentials::default()
    };
    let server = DtlsCoapServer::bind_with("127.0.0.1:0".parse().unwrap(), &credentials)
        .expect("bind rpk dtls server");
    let addr = server.local_addr().expect("local addr");
    let srv = serve(server, 1, b"rpk-pong");
    let mut client =
        DtlsCoapClient::connect_rpk(addr, &client_file, vec![name_of(&server_key)], WAIT)
            .expect("rpk handshake");
    assert_eq!(client.cipher(), Some("ECDHE-ECDSA-AES128-CCM8"));
    assert_eq!(
        client.peer_identity(),
        Some(DtlsPeerIdentity::RawPublicKey(server_key.public_key_der()))
    );
    assert_eq!(get(&mut client, 0x3456).payload, b"rpk-pong");
    client.close();
    assert_eq!(
        srv.join().expect("server thread"),
        vec![Some(DtlsPeerIdentity::RawPublicKey(
            client_key.public_key_der()
        ))]
    );

    let server = DtlsCoapServer::bind_with("127.0.0.1:0".parse().unwrap(), &credentials)
        .expect("bind rpk dtls server");
    let addr = server.local_addr().expect("local addr");
    let untrusted = DtlsCoapClient::connect_rpk(
        addr,
        &stranger_file,
        vec![name_of(&server_key)],
        Duration::from_secs(5),
    );
    assert!(untrusted.is_err(), "a client key the server does not trust");
    let _ = std::fs::remove_dir_all(dir);
}

/// Bridge-Spec §7.1 cert mode from `cert_file`/`key_file`, and the SIGHUP
/// rotation: `rotate` swaps the credentials the next handshake uses while the
/// session established before it keeps working ("Sessions halten").
#[test]
fn coap_9_dtls_certificate_files_and_rotation_keep_sessions_alive() {
    let dir = scratch("rotate");
    let first = self_signed(&dir, "first");
    let second = self_signed(&dir, "second");
    // The client trusts both certificates.
    let anchors = dir.join("anchors.pem");
    let mut both = std::fs::read(&first.0).expect("read");
    both.extend(std::fs::read(&second.0).expect("read"));
    std::fs::write(&anchors, both).expect("write anchors");
    let anchors = anchors.to_string_lossy().into_owned();
    let credentials_for = |pair: &(PathBuf, PathBuf)| DtlsCredentials {
        cert_file: Some(pair.0.to_string_lossy().into_owned()),
        key_file: Some(pair.1.to_string_lossy().into_owned()),
        ..DtlsCredentials::default()
    };

    let server = std::sync::Arc::new(
        DtlsCoapServer::bind_with("127.0.0.1:0".parse().unwrap(), &credentials_for(&first))
            .expect("bind cert dtls server"),
    );
    let addr = server.local_addr().expect("local addr");
    let accepting = std::sync::Arc::clone(&server);
    let srv = std::thread::spawn(move || {
        let mut sessions = Vec::new();
        for _ in 0..2 {
            let mut s = accepting.accept(WAIT).expect("accept");
            let req = s.recv_message(WAIT).expect("recv").expect("request");
            let mut resp = CoapMessage::new(
                MessageType::Acknowledgement,
                CoapCode::CONTENT,
                req.message_id,
            );
            resp.token = req.token.clone();
            resp.payload = b"cert-pong".to_vec();
            s.send_message(&resp).expect("send");
            sessions.push(s);
        }
        // Answer once more on the session opened before the rotation.
        let req = sessions[0]
            .recv_message(WAIT)
            .expect("recv after rotate")
            .expect("request");
        let mut resp = CoapMessage::new(
            MessageType::Acknowledgement,
            CoapCode::CONTENT,
            req.message_id,
        );
        resp.token = req.token.clone();
        resp.payload = b"still-alive".to_vec();
        sessions[0].send_message(&resp).expect("send");
        std::thread::sleep(Duration::from_millis(200));
    });

    let mut old = DtlsCoapClient::connect_verified(addr, "localhost", &anchors, None, WAIT)
        .expect("first handshake");
    let first_der = match old.peer_identity() {
        Some(DtlsPeerIdentity::Certificate(der)) => der,
        other => panic!("{other:?}"),
    };
    assert_eq!(get(&mut old, 1).payload, b"cert-pong");

    // SIGHUP: the daemon re-reads cert/key. Same socket, new credentials.
    server.rotate(&credentials_for(&second)).expect("rotate");

    // A new client handshakes against the rotated certificate...
    let mut fresh = DtlsCoapClient::connect_verified(addr, "localhost", &anchors, None, WAIT)
        .expect("handshake after rotation");
    match fresh.peer_identity() {
        Some(DtlsPeerIdentity::Certificate(der)) => assert_ne!(der, first_der),
        other => panic!("{other:?}"),
    }
    assert_eq!(get(&mut fresh, 2).payload, b"cert-pong");

    // ...and the session established before the rotation still carries CoAP.
    assert_eq!(
        get(&mut old, 3).payload,
        b"still-alive",
        "the session opened before the rotation has to survive it"
    );
    old.close();
    fresh.close();
    srv.join().expect("server thread");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Bridge-Spec §7.1: the three suites it recommends are negotiated —
/// `TLS_PSK_WITH_AES_128_CCM_8`, `TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8` and the
/// hybrid `TLS_ECDHE_PSK_WITH_AES_128_CBC_SHA256` — by a server configured
/// with all three and a client that offers one each.
#[test]
fn coapbridge_7_1_the_three_suites_of_section_7_1_are_negotiated() {
    use zerodds_coap_bridge::dtls::DtlsCipherSuite;
    use zerodds_dtls_openssl::ClientConfig;
    let dir = scratch("suites");
    let (cert, key) = self_signed(&dir, "suites");
    let mut psk = PskStore::new();
    psk.add("hybrid", vec![5u8; 16]).expect("psk");
    let credentials = DtlsCredentials {
        psk,
        cert_file: Some(cert.to_string_lossy().into_owned()),
        key_file: Some(key.to_string_lossy().into_owned()),
        cipher_suites: vec![
            DtlsCipherSuite::PskWithAes128Ccm8,
            DtlsCipherSuite::EcdheEcdsaWithAes128Ccm8,
            DtlsCipherSuite::EcdhePskWithAes128CbcSha256,
        ],
        ..DtlsCredentials::default()
    };
    let server =
        DtlsCoapServer::bind_with("127.0.0.1:0".parse().unwrap(), &credentials).expect("bind");
    let addr = server.local_addr().expect("local addr");
    let srv = serve(server, 3, b"suite");
    for (suite, openssl) in [
        ("TLS_PSK_WITH_AES_128_CCM_8", "PSK-AES128-CCM8"),
        (
            "TLS_ECDHE_PSK_WITH_AES_128_CBC_SHA256",
            "ECDHE-PSK-AES128-CBC-SHA256",
        ),
        (
            "TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8",
            "ECDHE-ECDSA-AES128-CCM8",
        ),
    ] {
        let config = if suite.contains("ECDSA") {
            ClientConfig {
                ca_file: Some(cert.clone()),
                server_name: Some("localhost".into()),
                cipher_suites: vec![suite.into()],
                ..ClientConfig::default()
            }
        } else {
            ClientConfig {
                psk: Some(("hybrid".into(), vec![5u8; 16])),
                cipher_suites: vec![suite.into()],
                ..ClientConfig::default()
            }
        };
        let mut client = DtlsCoapClient::connect(addr, &config, WAIT).expect(suite);
        assert_eq!(client.cipher(), Some(openssl), "{suite}");
        assert_eq!(get(&mut client, 0x4000).payload, b"suite");
        client.close();
    }
    srv.join().expect("server thread");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A key file that cannot be read as a private key stops the listener with
/// a configuration error instead of failing inside a handshake.
#[test]
fn coapbridge_7_1_dtls_refuses_a_key_file_it_cannot_read() {
    let dir = scratch("bad");
    let (cert, _) = self_signed(&dir, "c");
    let key = dir.join("key.pem");
    std::fs::write(
        &key,
        b"-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----\n",
    )
    .expect("write");
    let credentials = DtlsCredentials {
        cert_file: Some(cert.to_string_lossy().into_owned()),
        key_file: Some(key.to_string_lossy().into_owned()),
        ..DtlsCredentials::default()
    };
    match DtlsCoapServer::bind_with("127.0.0.1:0".parse().unwrap(), &credentials) {
        Err(DtlsError::Config(_)) => {}
        Err(other) => panic!("expected a configuration error, got {other}"),
        Ok(_) => panic!("a broken key must be refused"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}
