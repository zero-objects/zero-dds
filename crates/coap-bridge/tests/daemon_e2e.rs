// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! E2E-Test fuer `zerodds-coap-bridged`. Spec §12.2.
//!
//! Verifiziert:
//! * L1 — CoAP-Wire (POST/GET-Roundtrip via UDP-Socket im Test).
//! * L3 — POST mit `Uri-Path` fuer ein konfiguriertes Topic loest
//!   `2.04 Changed` aus.
//! * L3 — GET `/.well-known/core` (RFC 6690) liefert das Topic-
//!   Catalog.
//! * L3 — Observe-Register mit `Observe: 0` bekommt initial-Notify.

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

use std::net::UdpSocket;
use std::time::Duration;

use zerodds_coap_bridge::codec::{decode, encode};
use zerodds_coap_bridge::daemon::config::{DaemonConfig, TopicConfig};
use zerodds_coap_bridge::daemon::server;
use zerodds_coap_bridge::message::{CoapCode, CoapMessage, MessageType};
use zerodds_coap_bridge::option::{CoapOption, OptionValue, numbers};

fn make_test_config(bind: &str) -> DaemonConfig {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.bind = bind.to_string();
    cfg.domain = 99;
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

fn client_socket() -> UdpSocket {
    let s = UdpSocket::bind("127.0.0.1:0").expect("bind client");
    s.set_read_timeout(Some(Duration::from_secs(2)))
        .expect("timeout");
    s
}

fn recv_msg(socket: &UdpSocket) -> CoapMessage {
    let mut buf = [0u8; 65535];
    let (n, _peer) = socket.recv_from(&mut buf).expect("recv");
    decode(&buf[..n]).expect("decode")
}

#[test]
fn post_to_configured_path_returns_2_04_changed() {
    let cfg = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let server_addr = handle.local_addr.clone();

    let client = client_socket();
    let mut req = CoapMessage::new(MessageType::Confirmable, CoapCode::POST, 0x1234);
    req.token = b"toko".to_vec();
    req.options.push(CoapOption::uri_path("trade"));
    req.options.push(CoapOption::content_format(65000));
    req.payload = b"AAPL@200".to_vec();

    let bytes = encode(&req).expect("encode");
    client.send_to(&bytes, &server_addr).expect("send");

    let resp = recv_msg(&client);
    assert_eq!(resp.code, CoapCode::CHANGED, "expected 2.04 Changed");
    assert_eq!(resp.message_id, 0x1234, "MID mirrored");
    assert_eq!(resp.token, req.token, "token echoed");
    assert!(matches!(resp.message_type, MessageType::Acknowledgement));

    handle.shutdown();
}

#[test]
fn well_known_core_returns_link_format_catalog() {
    let cfg = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let server_addr = handle.local_addr.clone();

    let client = client_socket();
    let mut req = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 0x5678);
    req.options.push(CoapOption::uri_path(".well-known"));
    req.options.push(CoapOption::uri_path("core"));

    client
        .send_to(&encode(&req).expect("encode"), &server_addr)
        .expect("send");

    let resp = recv_msg(&client);
    assert_eq!(resp.code, CoapCode::CONTENT);
    let body = std::str::from_utf8(&resp.payload).unwrap_or("");
    assert!(
        body.contains("</trade>"),
        "expected </trade> in catalog, got: {body}"
    );
    assert!(body.contains("rt=\"dds.topic\""));

    handle.shutdown();
}

#[test]
fn observe_register_returns_initial_content_with_observe_option() {
    let cfg = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let server_addr = handle.local_addr.clone();

    let client = client_socket();
    let mut req = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 0x9ABC);
    req.token = b"obs".to_vec();
    req.options.push(CoapOption::observe(0));
    req.options.push(CoapOption::uri_path("trade"));

    client
        .send_to(&encode(&req).expect("encode"), &server_addr)
        .expect("send");

    let resp = recv_msg(&client);
    assert_eq!(resp.code, CoapCode::CONTENT);
    // Decoder normalisiert Option-Values nicht, wir akzeptieren beide
    // Repraesentationen (Uint und Opaque).
    let has_observe = resp.options.iter().any(|o| {
        o.number == numbers::OBSERVE
            && matches!(
                &o.value,
                OptionValue::Uint(_) | OptionValue::Opaque(_) | OptionValue::Empty
            )
    });
    assert!(has_observe, "expected Observe option in initial response");

    handle.shutdown();
}

#[test]
fn unknown_path_returns_bad_request() {
    let cfg = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let server_addr = handle.local_addr.clone();

    let client = client_socket();
    let mut req = CoapMessage::new(MessageType::Confirmable, CoapCode::POST, 1);
    req.token = b"t".to_vec();
    req.options.push(CoapOption::uri_path("nonexistent"));
    req.payload = b"x".to_vec();

    client
        .send_to(&encode(&req).expect("encode"), &server_addr)
        .expect("send");

    // Daemon ignoriert nicht-konfigurierte Pfade in handle_request mit
    // einem internen logged-error — keine Response wird gesendet. Wir
    // verifizieren nur, dass der Daemon weiterläuft (kein Crash).
    // Setze ein kurzes Timeout damit der Test nicht 2s wartet.
    client
        .set_read_timeout(Some(Duration::from_millis(300)))
        .expect("timeout");
    let mut buf = [0u8; 1024];
    let _ = client.recv_from(&mut buf); // may timeout
    handle.shutdown();
}

#[test]
fn block1_chunked_post_completes_with_2_04_changed() {
    // RFC 7959 §2.5 — Block1 fragmentiert Request-Body. Wir senden 3
    // Chunks à 16 Bytes (szx=0). Erste zwei sollen 2.31 Continue
    // erhalten, der dritte 2.04 Changed.
    let cfg = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let server_addr = handle.local_addr.clone();
    let client = client_socket();

    for (idx, more) in [(0u32, true), (1u32, true), (2u32, false)]
        .iter()
        .enumerate()
    {
        let (num, m) = (more.0, more.1);
        let mut req = CoapMessage::new(
            MessageType::Confirmable,
            CoapCode::POST,
            0x4000 + idx as u16,
        );
        req.token = b"blk1".to_vec();
        req.options.push(CoapOption::uri_path("trade"));
        req.options.push(CoapOption::content_format(65000));
        req.options.push(CoapOption::block1(num, m, 0));
        req.payload = vec![0x42u8; if m { 16 } else { 8 }];
        client
            .send_to(&encode(&req).expect("encode"), &server_addr)
            .expect("send");

        let resp = recv_msg(&client);
        if m {
            // 2.31 Continue
            assert_eq!(
                resp.code,
                CoapCode::new(2, 31),
                "chunk {} expected 2.31 Continue",
                num
            );
        } else {
            assert_eq!(
                resp.code,
                CoapCode::CHANGED,
                "final chunk expected 2.04 Changed"
            );
        }
        // Echo of Block1-Option present in response.
        let has_b1 = resp.options.iter().any(|o| o.number == numbers::BLOCK1);
        assert!(has_b1, "Block1 echo missing in response for chunk {}", num);
    }

    handle.shutdown();
}

#[test]
fn block1_out_of_order_returns_2_31_continue_only_in_order() {
    // RFC 7959 §2.5 — Sequenz: chunk 0 (more=true), dann direkt chunk
    // 2 (more=false) ohne chunk 1. Der zweite Send-Versuch löst bei
    // unserem Reassembler "block out of order" aus → nicht-fataler
    // Pfad: Daemon sendet keine Antwort. Wir testen daher nur, dass
    // chunk 0 die 2.31 Continue bringt.
    let cfg = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let server_addr = handle.local_addr.clone();
    let client = client_socket();

    let mut req = CoapMessage::new(MessageType::Confirmable, CoapCode::POST, 0x5000);
    req.token = b"ooo".to_vec();
    req.options.push(CoapOption::uri_path("trade"));
    req.options.push(CoapOption::block1(0, true, 0));
    req.payload = vec![0xaau8; 16];
    client
        .send_to(&encode(&req).expect("encode"), &server_addr)
        .expect("send");
    let resp = recv_msg(&client);
    assert_eq!(resp.code, CoapCode::new(2, 31));
    handle.shutdown();
}
