// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! E2E for the experimental DTLS 1.2 CoAP transport (feature `dtls`).
//! A DTLS client and server complete a real handshake (self-signed cert) and
//! exchange a CoAP GET → 2.05 Content over the encrypted channel.
//!
//! The crate doc sits before `#![cfg]` so the (empty) crate still carries
//! documentation when `dtls` is off — otherwise `missing_docs` + `-D warnings`
//! fails the build.
#![cfg(feature = "dtls")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::time::Duration;

use zerodds_coap_bridge::dtls_transport::{DtlsCoapClient, DtlsCoapServer};
use zerodds_coap_bridge::message::{CoapCode, CoapMessage, MessageType};

#[tokio::test]
async fn dtls_coap_get_content_roundtrip() {
    // Server binds an ephemeral port + generates a self-signed cert.
    let server = DtlsCoapServer::bind("127.0.0.1:0".parse().unwrap())
        .await
        .expect("bind dtls server");
    let addr = server.local_addr().await.expect("local addr");

    // Server task: accept one DTLS session, expect a GET, answer 2.05 Content.
    let srv = tokio::spawn(async move {
        let session = server.accept().await.expect("accept dtls");
        let req = session.recv_message().await.expect("recv request");
        assert_eq!(req.code, CoapCode::GET, "server should see a GET");
        let mut resp = CoapMessage::new(
            MessageType::Acknowledgement,
            CoapCode::CONTENT,
            req.message_id,
        );
        resp.token = req.token.clone();
        resp.payload = b"secure-pong".to_vec();
        session.send_message(&resp).await.expect("send response");
        // Hold the session briefly so the response record is flushed.
        tokio::time::sleep(Duration::from_millis(100)).await;
    });

    // Client: dial, accept the self-signed cert (verify = false), send a GET.
    let client = DtlsCoapClient::connect(addr, "localhost", false)
        .await
        .expect("dtls handshake");

    let mut req = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 0x1234);
    req.token = vec![0xAA, 0xBB];
    client.send_message(&req).await.expect("send request");

    let resp = client.recv_message().await.expect("recv response");
    assert_eq!(
        resp.code,
        CoapCode::CONTENT,
        "client should see 2.05 Content"
    );
    assert_eq!(resp.payload, b"secure-pong", "payload through DTLS");
    assert_eq!(resp.token, vec![0xAA, 0xBB], "token echoed");

    let _ = client.close().await;
    srv.await.expect("server task");
}

/// Bridge-Spec §7.1 PSK mode: `TLS_PSK_WITH_AES_128_CCM_8` with the
/// `coap.dtls.psk` identities, no certificate anywhere in the handshake.
#[tokio::test]
async fn dtls_psk_handshake_carries_coap() {
    use zerodds_coap_bridge::dtls::{DtlsCredentials, PskStore, parse_secret_hex};

    let secret = parse_secret_hex("000102030405060708090a0b0c0d0e0f").expect("hex secret");
    let mut psk = PskStore::new();
    psk.add("device-001", secret.clone()).expect("psk entry");
    let credentials = DtlsCredentials {
        psk,
        ..DtlsCredentials::default()
    };
    assert!(credentials.uses_psk());

    let server = DtlsCoapServer::bind_with("127.0.0.1:0".parse().unwrap(), &credentials)
        .await
        .expect("bind psk dtls server");
    let addr = server.local_addr().await.expect("local addr");

    let srv = tokio::spawn(async move {
        let session = server.accept().await.expect("accept psk dtls");
        let req = session.recv_message().await.expect("recv request");
        let mut resp = CoapMessage::new(
            MessageType::Acknowledgement,
            CoapCode::CONTENT,
            req.message_id,
        );
        resp.token = req.token.clone();
        resp.payload = b"psk-pong".to_vec();
        session.send_message(&resp).await.expect("send response");
        tokio::time::sleep(Duration::from_millis(100)).await;
    });

    let client = DtlsCoapClient::connect_psk(addr, "device-001", secret)
        .await
        .expect("psk handshake");
    let mut req = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 0x2345);
    req.token = vec![0x01];
    client.send_message(&req).await.expect("send request");
    let resp = client.recv_message().await.expect("recv response");
    assert_eq!(resp.code, CoapCode::CONTENT);
    assert_eq!(resp.payload, b"psk-pong");
    let _ = client.close().await;
    srv.await.expect("server task");
}

/// An identity the `coap.dtls.psk` list does not hold cannot complete the
/// handshake — the PSK callback has no secret to answer with.
#[tokio::test]
async fn dtls_psk_unknown_identity_is_refused() {
    use zerodds_coap_bridge::dtls::{DtlsCredentials, PskStore};

    let mut psk = PskStore::new();
    psk.add("device-001", vec![7u8; 16]).expect("psk entry");
    let credentials = DtlsCredentials {
        psk,
        ..DtlsCredentials::default()
    };
    let server = DtlsCoapServer::bind_with("127.0.0.1:0".parse().unwrap(), &credentials)
        .await
        .expect("bind psk dtls server");
    let addr = server.local_addr().await.expect("local addr");
    let srv = tokio::spawn(async move {
        // The handshake fails inside `accept`; the loop must not panic.
        let _ = tokio::time::timeout(Duration::from_secs(5), server.accept()).await;
    });

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        DtlsCoapClient::connect_psk(addr, "intruder", vec![9u8; 16]),
    )
    .await;
    assert!(
        matches!(outcome, Ok(Err(_)) | Err(_)),
        "an unknown PSK identity must not establish a session"
    );
    srv.abort();
}

/// Bridge-Spec §7.1 cert mode from `cert_file`/`key_file`, and the SIGHUP
/// rotation: `rotate` swaps the credentials the next handshake uses while the
/// session established before it keeps working ("Sessions halten").
#[tokio::test]
async fn dtls_certificate_files_and_rotation_keep_sessions_alive() {
    use std::io::Write;
    use zerodds_coap_bridge::dtls::DtlsCredentials;

    // Two independent self-signed certificate/key pairs on disk.
    let dir = std::env::temp_dir().join(format!("zerodds-dtls-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let mut written = Vec::new();
    for name in ["first", "second"] {
        let cert =
            rcgen::generate_simple_self_signed(vec!["localhost".to_owned()]).expect("self-signed");
        let cert_path = dir.join(format!("{name}-cert.pem"));
        let key_path = dir.join(format!("{name}-key.pem"));
        std::fs::File::create(&cert_path)
            .expect("cert file")
            .write_all(cert.cert.pem().as_bytes())
            .expect("write cert");
        std::fs::File::create(&key_path)
            .expect("key file")
            .write_all(cert.key_pair.serialize_pem().as_bytes())
            .expect("write key");
        written.push((cert_path, key_path));
    }
    let credentials_for = |i: usize| DtlsCredentials {
        cert_file: Some(written[i].0.to_string_lossy().into_owned()),
        key_file: Some(written[i].1.to_string_lossy().into_owned()),
        ..DtlsCredentials::default()
    };

    let first = credentials_for(0);
    let server = std::sync::Arc::new(
        DtlsCoapServer::bind_with("127.0.0.1:0".parse().unwrap(), &first)
            .await
            .expect("bind cert dtls server"),
    );
    let addr = server.local_addr().await.expect("local addr");

    let accepting = std::sync::Arc::clone(&server);
    let srv = tokio::spawn(async move {
        let mut sessions = Vec::new();
        for _ in 0..2 {
            let session = accepting.accept().await.expect("accept");
            let req = session.recv_message().await.expect("recv");
            let mut resp = CoapMessage::new(
                MessageType::Acknowledgement,
                CoapCode::CONTENT,
                req.message_id,
            );
            resp.token = req.token.clone();
            resp.payload = b"cert-pong".to_vec();
            session.send_message(&resp).await.expect("send");
            sessions.push(session);
        }
        // Answer once more on the session opened before the rotation.
        let req = sessions[0].recv_message().await.expect("recv after rotate");
        let mut resp = CoapMessage::new(
            MessageType::Acknowledgement,
            CoapCode::CONTENT,
            req.message_id,
        );
        resp.token = req.token.clone();
        resp.payload = b"still-alive".to_vec();
        sessions[0].send_message(&resp).await.expect("send");
        tokio::time::sleep(Duration::from_millis(100)).await;
    });

    let old = DtlsCoapClient::connect(addr, "localhost", false)
        .await
        .expect("first handshake");
    let mut req = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 1);
    req.token = vec![1];
    old.send_message(&req).await.expect("send");
    assert_eq!(
        old.recv_message().await.expect("recv").payload,
        b"cert-pong"
    );

    // SIGHUP: the daemon re-reads cert/key. Same socket, new credentials.
    server.rotate(&credentials_for(1)).expect("rotate");

    // A new client handshakes against the rotated certificate...
    let fresh = DtlsCoapClient::connect(addr, "localhost", false)
        .await
        .expect("handshake after rotation");
    let mut req2 = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 2);
    req2.token = vec![2];
    fresh.send_message(&req2).await.expect("send");
    assert_eq!(
        fresh.recv_message().await.expect("recv").payload,
        b"cert-pong"
    );

    // ...and the session established before the rotation still carries CoAP.
    let mut req3 = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 3);
    req3.token = vec![3];
    old.send_message(&req3).await.expect("send on old session");
    assert_eq!(
        old.recv_message().await.expect("recv").payload,
        b"still-alive",
        "the session opened before the rotation has to survive it"
    );

    let _ = old.close().await;
    let _ = fresh.close().await;
    srv.await.expect("server task");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A key file that is not PKCS#8 is refused by name instead of failing inside
/// the handshake.
#[test]
fn dtls_rejects_a_non_pkcs8_key_file() {
    use std::io::Write;
    let dir = std::env::temp_dir().join(format!("zerodds-dtls-bad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");
    let cert =
        rcgen::generate_simple_self_signed(vec!["localhost".to_owned()]).expect("self-signed");
    std::fs::File::create(&cert_path)
        .expect("cert file")
        .write_all(cert.cert.pem().as_bytes())
        .expect("write");
    std::fs::File::create(&key_path)
        .expect("key file")
        .write_all(b"-----BEGIN EC PRIVATE KEY-----\nAAAA\n-----END EC PRIVATE KEY-----\n")
        .expect("write");
    let err = zerodds_coap_bridge::dtls_transport::load_certificate(
        &cert_path.to_string_lossy(),
        &key_path.to_string_lossy(),
    )
    .expect_err("a SEC1 key must be refused");
    assert!(
        format!("{err}").contains("PKCS#8"),
        "the error must name the conversion, got: {err}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
