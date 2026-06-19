// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! E2E test for `zerodds-coap-bridged` §7 security wireup.
//!
//! Spec status §7.1 (DTLS): rejected (next phase) — so only Auth
//! (§7.2) + topic ACL (§7.3) are covered here.
//!
//! Tests:
//! * §7.2 Bearer token via CoAP option 65000.
//! * §7.2 Reject without token: 4.01 Unauthorized.
//! * §7.3 ACL write deny: POST on a disallowed topic → 4.03 Forbidden.
//! * §7.3 ACL read deny: GET-Observe on a disallowed topic → 4.03.

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
use zerodds_coap_bridge::option::{CoapOption, OptionValue};

const COAP_OPTION_AUTH_TOKEN: u16 = 65000;

fn auth_token_option(token: &[u8]) -> CoapOption {
    CoapOption::new(COAP_OPTION_AUTH_TOKEN, OptionValue::Opaque(token.to_vec()))
}

fn make_secure_cfg(with_acl: bool) -> DaemonConfig {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.bind = "127.0.0.1:0".to_string();
    cfg.domain = 99;
    cfg.auth_mode = "bearer".into();
    cfg.auth_bearer_token = Some("secret-coap-token".into());
    cfg.auth_bearer_subject = Some("alice".into());
    cfg.topics.push(TopicConfig {
        dds_name: "Allowed".to_string(),
        dds_type: "Allowed".to_string(),
        coap_uri_path: "allowed".to_string(),
        direction: "bidir".to_string(),
        reliability: "best_effort".to_string(),
        durability: "volatile".to_string(),
        history_depth: 10,
    });
    cfg.topics.push(TopicConfig {
        dds_name: "Forbidden".to_string(),
        dds_type: "Forbidden".to_string(),
        coap_uri_path: "forbidden".to_string(),
        direction: "bidir".to_string(),
        reliability: "best_effort".to_string(),
        durability: "volatile".to_string(),
        history_depth: 10,
    });
    if with_acl {
        cfg.topic_acl.insert(
            "Allowed".into(),
            (vec!["alice".into()], vec!["alice".into()]),
        );
        cfg.topic_acl
            .insert("Forbidden".into(), (vec!["bob".into()], vec!["bob".into()]));
    }
    cfg
}

fn make_open_cfg() -> DaemonConfig {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.bind = "127.0.0.1:0".to_string();
    cfg.domain = 99;
    cfg.auth_mode = "none".into();
    cfg.topics.push(TopicConfig {
        dds_name: "T".to_string(),
        dds_type: "T".to_string(),
        coap_uri_path: "t".to_string(),
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
fn auth_bearer_token_in_option_is_accepted() {
    let cfg = make_secure_cfg(true);
    let h = server::start(cfg).expect("start");
    let addr = h.local_addr.clone();
    let client = client_socket();

    let mut req = CoapMessage::new(MessageType::Confirmable, CoapCode::POST, 0x1001);
    req.token = b"tk1".to_vec();
    req.options.push(CoapOption::uri_path("allowed"));
    req.options
        .push(auth_token_option(b"Bearer secret-coap-token"));
    req.payload = b"payload".to_vec();

    let bytes = encode(&req).expect("encode");
    client.send_to(&bytes, &addr).expect("send");
    let resp = recv_msg(&client);
    assert_eq!(resp.code, CoapCode::CHANGED, "expected 2.04 Changed");
    drop(h);
}

#[test]
fn auth_missing_bearer_token_yields_4_01_unauthorized() {
    let cfg = make_secure_cfg(false);
    let h = server::start(cfg).expect("start");
    let addr = h.local_addr.clone();
    let client = client_socket();

    let mut req = CoapMessage::new(MessageType::Confirmable, CoapCode::POST, 0x1002);
    req.token = b"tk2".to_vec();
    req.options.push(CoapOption::uri_path("allowed"));
    // No auth token.
    req.payload = b"payload".to_vec();

    let bytes = encode(&req).expect("encode");
    client.send_to(&bytes, &addr).expect("send");
    let resp = recv_msg(&client);
    assert_eq!(resp.code, CoapCode::new(4, 1), "expected 4.01 Unauthorized");
    drop(h);
}

#[test]
fn auth_invalid_bearer_token_yields_4_01() {
    let cfg = make_secure_cfg(false);
    let h = server::start(cfg).expect("start");
    let addr = h.local_addr.clone();
    let client = client_socket();

    let mut req = CoapMessage::new(MessageType::Confirmable, CoapCode::POST, 0x1003);
    req.token = b"tk3".to_vec();
    req.options.push(CoapOption::uri_path("allowed"));
    req.options.push(auth_token_option(b"Bearer wrong-token"));
    req.payload = b"payload".to_vec();

    let bytes = encode(&req).expect("encode");
    client.send_to(&bytes, &addr).expect("send");
    let resp = recv_msg(&client);
    assert_eq!(resp.code, CoapCode::new(4, 1), "expected 4.01");
    drop(h);
}

#[test]
fn acl_write_deny_yields_4_03_forbidden() {
    let cfg = make_secure_cfg(true);
    let h = server::start(cfg).expect("start");
    let addr = h.local_addr.clone();
    let client = client_socket();

    let mut req = CoapMessage::new(MessageType::Confirmable, CoapCode::POST, 0x1004);
    req.token = b"tk4".to_vec();
    req.options.push(CoapOption::uri_path("forbidden"));
    req.options
        .push(auth_token_option(b"Bearer secret-coap-token"));
    req.payload = b"x".to_vec();

    let bytes = encode(&req).expect("encode");
    client.send_to(&bytes, &addr).expect("send");
    let resp = recv_msg(&client);
    assert_eq!(resp.code, CoapCode::new(4, 3), "expected 4.03 Forbidden");
    drop(h);
}

#[test]
fn acl_observe_register_deny_yields_4_03() {
    let cfg = make_secure_cfg(true);
    let h = server::start(cfg).expect("start");
    let addr = h.local_addr.clone();
    let client = client_socket();

    let mut req = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 0x1005);
    req.token = b"tk5".to_vec();
    req.options.push(CoapOption::observe(0));
    req.options.push(CoapOption::uri_path("forbidden"));
    req.options
        .push(auth_token_option(b"Bearer secret-coap-token"));

    let bytes = encode(&req).expect("encode");
    client.send_to(&bytes, &addr).expect("send");
    let resp = recv_msg(&client);
    assert_eq!(resp.code, CoapCode::new(4, 3), "expected 4.03");
    drop(h);
}

#[test]
fn auth_none_open_topic_works_unauthenticated() {
    // Sanity: with auth.mode=none the daemon must keep the open path
    // unbroken (no regression for non-authenticated deployments).
    let cfg = make_open_cfg();
    let h = server::start(cfg).expect("start");
    let addr = h.local_addr.clone();
    let client = client_socket();

    let mut req = CoapMessage::new(MessageType::Confirmable, CoapCode::POST, 0x1006);
    req.token = b"tk6".to_vec();
    req.options.push(CoapOption::uri_path("t"));
    req.payload = b"y".to_vec();
    let bytes = encode(&req).expect("encode");
    client.send_to(&bytes, &addr).expect("send");
    let resp = recv_msg(&client);
    assert_eq!(resp.code, CoapCode::CHANGED);
    drop(h);
}
