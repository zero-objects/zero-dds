// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! E2E test for `zerodds-coap-bridged` with OSCORE (Spec §7.2, RFC 8613).
//!
//! The daemon runs as the OSCORE server (Sender ID `01`); the test is an
//! OSCORE client with the mirrored context (client Sender ID empty — the
//! RFC 8613 C.1 constellation). Verifies over a real UDP socket:
//!
//! * a protected POST is unprotected, written to DDS, and answered with a
//!   protected `2.04 Changed` the client can verify + decrypt;
//! * a replayed request (same Partial IV) is refused with `4.01`;
//! * an unprotected request to a topic path is refused with `4.01`;
//! * `/.well-known/core` stays reachable in the clear;
//! * a protected Observe register is answered protected;
//! * a DDS sample reaches the protected observer as an OSCORE-protected
//!   notification with the daemon's own Partial IV (§4.1.3.5.2).

#![cfg(all(feature = "daemon", feature = "oscore"))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::net::UdpSocket;
use std::time::Duration;

use zerodds_coap_bridge::codec::{decode, encode};
use zerodds_coap_bridge::daemon::config::{DaemonConfig, OscoreSettings, TopicConfig};
use zerodds_coap_bridge::daemon::server;
use zerodds_coap_bridge::message::{CoapCode, CoapMessage, MessageType};
use zerodds_coap_bridge::option::CoapOption;
use zerodds_coap_bridge::oscore::message::{protect_request_message, unprotect_response_message};
use zerodds_coap_bridge::oscore::{AeadAlgorithm, SecurityContext};

const MASTER_SECRET: [u8; 16] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
];
const MASTER_SALT: [u8; 8] = [0x9e, 0x7c, 0xa9, 0x22, 0x23, 0x78, 0x63, 0x40];
const SERVER_ID: [u8; 1] = [0x01];

fn make_oscore_config(bind: &str, domain: i32) -> DaemonConfig {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.bind = bind.to_string();
    // One DDS domain per test: the tests run in parallel and same-domain
    // daemons discover each other over loopback, cross-feeding notifies.
    cfg.domain = domain;
    cfg.oscore = OscoreSettings {
        enabled: true,
        master_secret: MASTER_SECRET.to_vec(),
        master_salt: MASTER_SALT.to_vec(),
        id_context: None,
        sender_id: SERVER_ID.to_vec(),
        replay_window: 32,
    };
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

/// The client context mirroring the daemon: client Sender ID empty, Recipient
/// ID = the daemon's Sender ID.
fn client_ctx() -> SecurityContext {
    SecurityContext::derive(
        &MASTER_SECRET,
        &MASTER_SALT,
        &[],
        &SERVER_ID,
        None,
        AeadAlgorithm::AesCcm16_64_128,
    )
}

fn client_socket() -> UdpSocket {
    let s = UdpSocket::bind("127.0.0.1:0").expect("bind client");
    s.set_read_timeout(Some(Duration::from_secs(2)))
        .expect("timeout");
    s
}

fn exchange(client: &UdpSocket, server_addr: &str, msg: &CoapMessage) -> CoapMessage {
    let bytes = encode(msg).expect("encode");
    client.send_to(&bytes, server_addr).expect("send");
    let mut buf = [0u8; 65535];
    let (n, _peer) = client.recv_from(&mut buf).expect("recv");
    decode(&buf[..n]).expect("decode")
}

fn post_trade(mid: u16, token: &[u8], payload: &[u8]) -> CoapMessage {
    let mut req = CoapMessage::new(MessageType::Confirmable, CoapCode::POST, mid);
    req.token = token.to_vec();
    req.options.push(CoapOption::uri_path("trade"));
    req.options.push(CoapOption::content_format(65000));
    req.payload = payload.to_vec();
    req
}

#[test]
fn protected_post_round_trips_and_replay_is_refused() {
    let cfg = make_oscore_config("127.0.0.1:0", 91);
    let mut handle = server::start(cfg).expect("daemon start");
    let server_addr = handle.local_addr.clone();
    let client = client_socket();
    let ctx = client_ctx();

    // Protected POST with Partial IV 0x14.
    let inner = post_trade(0x1234, b"tok1", b"AAPL@200");
    let outer = protect_request_message(&ctx, &[], None, &[0x14], &inner).unwrap();
    let resp = exchange(&client, &server_addr, &outer);

    // The outer response is 2.04 Changed (RFC 8613 §4.2) and decrypts to the
    // daemon's inner 2.04.
    assert_eq!(resp.code, CoapCode::new(2, 4), "outer code");
    let recovered = unprotect_response_message(&ctx, &SERVER_ID, &[], &[0x14], &resp).unwrap();
    assert_eq!(recovered.code, CoapCode::CHANGED, "inner 2.04 Changed");
    assert_eq!(recovered.token, b"tok1".to_vec());

    // The DDS write happened.
    let m = handle.metrics.as_ref().expect("metrics");
    assert_eq!(m.dds_samples_in_total.get(), 1, "one DDS sample written");

    // Replay: the identical protected message again → 4.01, no second write.
    let replay = exchange(&client, &server_addr, &outer);
    assert_eq!(replay.code, CoapCode::new(4, 1), "replay refused with 4.01");
    assert_eq!(replay.payload, b"oscore replay".to_vec());
    assert_eq!(m.dds_samples_in_total.get(), 1, "no second DDS write");
    assert_eq!(m.oscore_replays_total.get(), 1, "replay counter (§8.2)");

    handle.shutdown();
}

#[test]
fn unprotected_request_is_refused_with_4_01() {
    let cfg = make_oscore_config("127.0.0.1:0", 92);
    let mut handle = server::start(cfg).expect("daemon start");
    let server_addr = handle.local_addr.clone();
    let client = client_socket();

    let resp = exchange(&client, &server_addr, &post_trade(0x2222, b"tok2", b"x"));
    assert_eq!(resp.code, CoapCode::new(4, 1), "4.01 Unauthorized");
    assert_eq!(resp.payload, b"oscore required".to_vec());
    let m = handle.metrics.as_ref().expect("metrics");
    assert_eq!(m.dds_samples_in_total.get(), 0, "nothing written");

    handle.shutdown();
}

#[test]
fn well_known_core_stays_reachable_in_the_clear() {
    let cfg = make_oscore_config("127.0.0.1:0", 93);
    let mut handle = server::start(cfg).expect("daemon start");
    let server_addr = handle.local_addr.clone();
    let client = client_socket();

    let mut req = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 0x3333);
    req.token = b"tok3".to_vec();
    req.options.push(CoapOption::uri_path(".well-known"));
    req.options.push(CoapOption::uri_path("core"));
    let resp = exchange(&client, &server_addr, &req);
    assert_eq!(resp.code, CoapCode::CONTENT);
    let body = String::from_utf8(resp.payload).unwrap();
    assert!(body.contains("</trade>"), "catalog lists the topic: {body}");

    handle.shutdown();
}

#[test]
fn notify_to_protected_observer_arrives_protected() {
    let cfg = make_oscore_config("127.0.0.1:0", 94);
    let mut handle = server::start(cfg).expect("daemon start");
    let server_addr = handle.local_addr.clone();
    let client = client_socket();
    let ctx = client_ctx();

    // 1. Protected Observe register on `trade`.
    let mut reg = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 0x5001);
    reg.token = b"tok5".to_vec();
    reg.options.push(CoapOption::observe(0));
    reg.options.push(CoapOption::uri_path("trade"));
    let reg_outer = protect_request_message(&ctx, &[], None, &[0x40], &reg).unwrap();
    let reg_resp = exchange(&client, &server_addr, &reg_outer);
    assert_eq!(reg_resp.code, CoapCode::new(2, 5));

    // 2. Protected POST on the same (bidir) topic — the daemon's DDS write
    // loops back into its own reader, which pushes a notify. The POST response
    // (token `tok6`) and the notify (observer token `tok5`) race on the
    // socket; collect both and classify by token.
    let post = post_trade(0x5002, b"tok6", b"MSFT@300");
    let post_outer = protect_request_message(&ctx, &[], None, &[0x41], &post).unwrap();
    client
        .send_to(&encode(&post_outer).unwrap(), &server_addr)
        .expect("send");
    let mut post_resp: Option<CoapMessage> = None;
    let mut notify: Option<CoapMessage> = None;
    let mut buf = [0u8; 65535];
    while post_resp.is_none() || notify.is_none() {
        let (n, _peer) = client.recv_from(&mut buf).expect("recv");
        let msg = decode(&buf[..n]).expect("decode");
        if msg.token == b"tok6".to_vec() {
            post_resp = Some(msg);
        } else if msg.token == b"tok5".to_vec() {
            notify = Some(msg);
        }
    }
    let post_resp = post_resp.unwrap();
    assert_eq!(post_resp.code, CoapCode::new(2, 4));

    // 3. The notify arrives protected: outer 2.05 with an OSCORE option
    // carrying the daemon's own Partial IV, decryptable by the client with
    // the register exchange's request parameters (§4.1.3.5.2 / C.8).
    let notify = notify.unwrap();
    assert_eq!(notify.code, CoapCode::new(2, 5), "outer notify code");
    assert!(
        notify.options.iter().any(|o| o.number == 9),
        "OSCORE option present"
    );
    assert_ne!(notify.payload, b"MSFT@300".to_vec(), "payload not in clear");
    let recovered = unprotect_response_message(&ctx, &SERVER_ID, &[], &[0x40], &notify).unwrap();
    assert_eq!(recovered.code, CoapCode::CONTENT);
    assert_eq!(recovered.payload, b"MSFT@300".to_vec());

    handle.shutdown();
}

#[test]
fn protected_observe_register_is_answered_protected() {
    let cfg = make_oscore_config("127.0.0.1:0", 95);
    let mut handle = server::start(cfg).expect("daemon start");
    let server_addr = handle.local_addr.clone();
    let client = client_socket();
    let ctx = client_ctx();

    let mut inner = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 0x4444);
    inner.token = b"tok4".to_vec();
    inner.options.push(CoapOption::observe(0));
    inner.options.push(CoapOption::uri_path("trade"));
    let outer = protect_request_message(&ctx, &[], None, &[0x30], &inner).unwrap();
    // Outer code 0.05 FETCH — the Observe option is on the outer message
    // (RFC 8613 §4.1.3.5).
    assert_eq!(outer.code, CoapCode::new(0, 5));

    let resp = exchange(&client, &server_addr, &outer);
    // Outer 2.05 (Observe response, §4.1.3.5); decrypts to the inner 2.05
    // with the Observe + Content-Format the dispatcher set.
    assert_eq!(resp.code, CoapCode::new(2, 5), "outer code");
    let recovered = unprotect_response_message(&ctx, &SERVER_ID, &[], &[0x30], &resp).unwrap();
    assert_eq!(recovered.code, CoapCode::CONTENT);
    assert!(recovered.options.iter().any(|o| o.number == 6), "Observe");

    handle.shutdown();
}
