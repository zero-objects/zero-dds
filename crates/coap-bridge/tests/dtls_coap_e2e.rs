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
