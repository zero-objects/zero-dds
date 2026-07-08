// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! E2E for the `zerodds-coap-bridged` daemon serving `coaps://` over DTLS
//! (features `daemon` + `dtls`, RFC 7252 §9, ADR 0011).
//!
//! A real DTLS client handshakes with the daemon and drives the full request
//! path through the shared `dispatch` core — POST → 2.04 Changed, GET-Observe →
//! initial 2.05, and a subsequent DDS sample is pushed back as a CoAP Observe
//! notification *over the encrypted DTLS session* (proving the sync-pump →
//! async-session notify bridge).
//!
//! The crate doc sits before `#![cfg]` so the (empty) crate still carries docs
//! when the features are off — otherwise `missing_docs` + `-D warnings` fails.
#![cfg(all(feature = "daemon", feature = "dtls"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::field_reassign_with_default,
    missing_docs
)]

use std::time::Duration;

use zerodds_coap_bridge::daemon::config::{DaemonConfig, TopicConfig};
use zerodds_coap_bridge::daemon::server;
use zerodds_coap_bridge::dtls_transport::DtlsCoapClient;
use zerodds_coap_bridge::message::{CoapCode, CoapMessage, MessageType};
use zerodds_coap_bridge::option::CoapOption;

fn dtls_test_config() -> DaemonConfig {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.bind = "127.0.0.1:0".to_string(); // plaintext coap:// (required)
    cfg.domain = 71;
    cfg.dtls_enabled = true;
    cfg.bind_dtls = Some("127.0.0.1:0".to_string()); // ephemeral coaps://
    cfg.topics.push(TopicConfig {
        dds_name: "Trade".to_string(),
        dds_type: "Trade".to_string(),
        coap_uri_path: "trade".to_string(),
        direction: "bidir".to_string(),
        reliability: "best_effort".to_string(),
        durability: "volatile".to_string(),
        history_depth: 10,
    });
    cfg
}

#[tokio::test]
async fn daemon_serves_coaps_over_dtls_request_response_and_observe() {
    let mut handle = server::start(dtls_test_config()).expect("daemon start");

    // Wait for the DTLS listener to publish its bound address.
    let mut dtls_addr = None;
    for _ in 0..100 {
        if let Some(a) = handle.dtls_local_addr() {
            dtls_addr = Some(a);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let dtls_addr = dtls_addr.expect("dtls listener never came up");

    // Real DTLS handshake (accept the daemon's self-signed cert in test).
    let client = DtlsCoapClient::connect(dtls_addr, "localhost", false)
        .await
        .expect("dtls handshake with daemon");

    // 1) POST → DDS write → 2.04 Changed, over DTLS.
    let mut post = CoapMessage::new(MessageType::Confirmable, CoapCode::POST, 0x4001);
    post.token = b"tdtls".to_vec();
    post.options.push(CoapOption::uri_path("trade"));
    post.options.push(CoapOption::content_format(65000));
    post.payload = b"AAPL@200".to_vec();
    client.send_message(&post).await.expect("send POST");
    let resp = recv(&client).await.expect("POST response");
    assert_eq!(
        resp.code,
        CoapCode::CHANGED,
        "POST → 2.04 Changed over DTLS"
    );
    assert_eq!(resp.token, post.token, "token echoed");

    // 2) GET with Observe:0 → initial 2.05 Content with the Observe option.
    let mut obs = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 0x4002);
    obs.token = b"obs01".to_vec();
    obs.options.push(CoapOption::uri_path("trade"));
    obs.options.push(CoapOption::observe(0));
    client.send_message(&obs).await.expect("send observe GET");
    let initial = recv(&client).await.expect("observe initial");
    assert_eq!(initial.code, CoapCode::CONTENT, "observe initial → 2.05");
    assert!(
        initial.options.iter().any(|o| o.number == 6), // Observe option number
        "initial observe response must carry the Observe option"
    );

    // 3) A new sample → pushed back as an Observe notification over DTLS.
    let mut post2 = CoapMessage::new(MessageType::Confirmable, CoapCode::POST, 0x4003);
    post2.token = b"tdtl2".to_vec();
    post2.options.push(CoapOption::uri_path("trade"));
    post2.options.push(CoapOption::content_format(65000));
    post2.payload = b"MSFT@300".to_vec();
    client.send_message(&post2).await.expect("send POST 2");

    // Collect a few messages; one of them must be the NON-confirmable Observe
    // notification carrying the new sample's payload (the DDS sample loops back
    // through the daemon's own reader → pump → DTLS session).
    let mut got_notify = false;
    for _ in 0..4 {
        match recv(&client).await {
            Some(m)
                if matches!(m.message_type, MessageType::NonConfirmable)
                    && m.payload == b"MSFT@300" =>
            {
                got_notify = true;
                break;
            }
            Some(_) => continue, // the 2.04 ack for post2, etc.
            None => break,
        }
    }
    assert!(
        got_notify,
        "expected an Observe notification with the new sample over DTLS"
    );

    let _ = client.close().await;
    handle.shutdown();
}

/// Receive one CoAP message over DTLS with a generous timeout.
async fn recv(
    client: &zerodds_coap_bridge::dtls_transport::DtlsCoapSession,
) -> Option<CoapMessage> {
    tokio::time::timeout(Duration::from_secs(3), client.recv_message())
        .await
        .ok()
        .and_then(|r| r.ok())
}
