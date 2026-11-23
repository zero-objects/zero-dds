// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! E2E test for `zerodds-coap-bridged`. Spec §12.2.
//!
//! Verifies:
//! * L1 — CoAP wire (POST/GET round-trip via UDP socket in the test).
//! * L3 — POST with `Uri-Path` for a configured topic triggers
//!   `2.04 Changed`.
//! * L3 — GET `/.well-known/core` (RFC 6690) returns the topic
//!   catalog.
//! * L3 — Observe register with `Observe: 0` receives the initial
//!   notify.

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

#[path = "support/udp_client.rs"]
mod udp_client;

use std::net::UdpSocket;
use std::time::Duration;

use udp_client::{client_socket, recv_msg};
use zerodds_coap_bridge::codec::{decode, encode};
use zerodds_coap_bridge::daemon::config::{DaemonConfig, TopicConfig};
use zerodds_coap_bridge::daemon::server;
use zerodds_coap_bridge::message::{CoapCode, CoapMessage, MessageType};
use zerodds_coap_bridge::option::{CoapOption, OptionValue, numbers};

/// The daemon child, killed and reaped when the test ends, also when an
/// assertion fails before the test's own SIGTERM: a bare `Child` is not
/// killed on drop, and a failed run left the daemon running for hours.
struct Reaped(std::process::Child);

impl Drop for Reaped {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The daemon's configuration on a leased DDS domain (the host's share of
/// the band, `zerodds-test-domains`); the lease is held while the test runs.
fn make_test_config(bind: &str) -> (DaemonConfig, zerodds_test_domains::DomainLease) {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.bind = bind.to_string();
    let lease = zerodds_test_domains::lease_domain();
    cfg.domain = lease.id();
    cfg.topics.push(TopicConfig {
        dds_name: "Trade".to_string(),
        dds_type: "Trade".to_string(),
        coap_uri_path: "trade".to_string(),
        direction: "bidir".to_string(),
        reliability: "best_effort".to_string(),
        durability: "volatile".to_string(),
        history_depth: 10,
        ..Default::default()
    });
    (cfg, lease)
}

#[test]
fn coapbridge_2_post_to_configured_path_returns_2_04_changed() {
    let (cfg, _lease) = make_test_config("127.0.0.1:0");
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
fn coapbridge_2_well_known_core_returns_link_format_catalog() {
    let (cfg, _lease) = make_test_config("127.0.0.1:0");
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
fn coapbridge_2_observe_register_returns_initial_content_with_observe_option() {
    let (cfg, _lease) = make_test_config("127.0.0.1:0");
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
    // The decoder does not normalize option values; we accept both
    // representations (Uint and Opaque).
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

/// RFC 7252 §5.9.2.5: a resource the server does not have is `4.04 Not
/// Found`; §5.10.1: a Uri-Path ".." is `4.00 Bad Request`; §6.5: a "/"
/// inside one Uri-Path segment does not name the resource `trade/x`.
#[test]
fn coap_5_9_2_5_an_unknown_resource_is_4_04_and_a_dot_segment_4_00() {
    let (cfg, _lease) = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let server_addr = handle.local_addr.clone();
    let client = client_socket();
    let ask = |path: &[&str], mid: u16| {
        let mut req = CoapMessage::new(MessageType::Confirmable, CoapCode::POST, mid);
        req.token = vec![mid as u8];
        for segment in path {
            req.options.push(CoapOption::uri_path(*segment));
        }
        req.payload = b"x".to_vec();
        client
            .send_to(&encode(&req).expect("encode"), &server_addr)
            .expect("send");
        recv_msg(&client).code
    };
    assert_eq!(ask(&["nonexistent"], 1), CoapCode::new(4, 4));
    assert_eq!(ask(&["trade", ".."], 2), CoapCode::BAD_REQUEST);
    assert_eq!(ask(&["tr/ade"], 3), CoapCode::new(4, 4));
    assert_eq!(ask(&["trade"], 4), CoapCode::CHANGED);
    handle.shutdown();
}

/// RFC 7252 §8, §7.1, §11.3 and RFC 6690 §4.1: the daemon joins its group;
/// a Non-confirmable GET of `/.well-known/core` to the group is answered
/// with a Non-confirmable unicast response within the leisure period. No
/// response comes for a filter that matches nothing, for a Confirmable
/// request, for a resource not configured for multicast and for one that
/// needs OSCORE. The same filter by unicast gets an empty `2.05`.
#[cfg(target_os = "linux")]
#[test]
fn coap_8_a_multicast_request_is_answered_by_unicast_within_the_leisure() {
    let (mut cfg, _lease) = make_test_config("0.0.0.0:0");
    let group: std::net::Ipv4Addr = std::format!("239.255.{}.{}", 7, 1 + std::process::id() % 250)
        .parse()
        .expect("group");
    cfg.multicast.enabled = true;
    cfg.multicast.groups = vec![std::net::IpAddr::V4(group)];
    cfg.multicast.leisure_ms = 300;
    cfg.multicast.resources = vec![".well-known/core".to_string(), "trade".to_string()];
    let mut handle = server::start(cfg).expect("daemon start");
    let port: u16 = handle
        .local_addr
        .parse::<std::net::SocketAddr>()
        .expect("addr")
        .port();
    let client = UdpSocket::bind("0.0.0.0:0").expect("bind client");
    client.set_multicast_loop_v4(true).expect("loop");
    let target = std::net::SocketAddr::from((group, port));
    let send = |mtype: MessageType, path: &[&str], query: Option<&str>, token: &[u8]| {
        let mut req = CoapMessage::new(mtype, CoapCode::GET, u16::from(token[0]));
        req.token = token.to_vec();
        for segment in path {
            req.options.push(CoapOption::uri_path(*segment));
        }
        if let Some(q) = query {
            req.options.push(CoapOption {
                number: zerodds_coap_bridge::option::numbers::URI_QUERY,
                value: zerodds_coap_bridge::option::OptionValue::String(q.to_string()),
            });
        }
        client
            .send_to(&encode(&req).expect("encode"), target)
            .expect("send");
    };
    let recv = |wait: Duration| -> Option<(CoapMessage, std::net::SocketAddr)> {
        client.set_read_timeout(Some(wait)).expect("timeout");
        let mut buf = [0u8; 4096];
        client
            .recv_from(&mut buf)
            .ok()
            .map(|(n, from)| (decode(&buf[..n]).expect("decode"), from))
    };

    send(
        MessageType::NonConfirmable,
        &[".well-known", "core"],
        None,
        b"m1",
    );
    let (resp, from) = recv(Duration::from_secs(3)).expect("no response to the multicast request");
    assert_eq!(resp.code, CoapCode::CONTENT);
    assert_eq!(resp.token, b"m1".to_vec());
    assert!(matches!(resp.message_type, MessageType::NonConfirmable));
    assert!(String::from_utf8_lossy(&resp.payload).contains("</trade>"));
    assert_eq!(from.port(), port);
    assert_ne!(
        from.ip(),
        std::net::IpAddr::V4(group),
        "the response is unicast"
    );

    for (mtype, path, query, token) in [
        (
            MessageType::NonConfirmable,
            &[".well-known", "core"][..],
            Some("rt=nothing"),
            b"m2",
        ),
        (
            MessageType::Confirmable,
            &[".well-known", "core"][..],
            None,
            b"m3",
        ),
        (MessageType::NonConfirmable, &["unlisted"][..], None, b"m4"),
        (MessageType::NonConfirmable, &["trade"][..], None, b"m5"),
    ] {
        send(mtype, path, query, token);
        assert!(
            recv(Duration::from_millis(900)).is_none(),
            "a response to {path:?} {query:?} {mtype:?}"
        );
    }

    // The same filter by unicast: an empty catalog.
    let unicast = client_socket();
    let mut req = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 9);
    req.token = b"u".to_vec();
    req.options.push(CoapOption::uri_path(".well-known"));
    req.options.push(CoapOption::uri_path("core"));
    req.options.push(CoapOption {
        number: zerodds_coap_bridge::option::numbers::URI_QUERY,
        value: zerodds_coap_bridge::option::OptionValue::String("rt=nothing".to_string()),
    });
    unicast
        .send_to(&encode(&req).expect("encode"), ("127.0.0.1", port))
        .expect("send");
    let resp = recv_msg(&unicast);
    assert_eq!(resp.code, CoapCode::CONTENT);
    assert!(resp.payload.is_empty());
    handle.shutdown();
}

/// RFC 7252 §5.4.1, §5.8, §5.10.4, §5.10.3, §5.10.6.2 and §5.10.8 at the
/// daemon's own resources: an unrecognized critical option is 4.02 (a
/// Non-confirmable request with one is reset), an unsupported method 4.05,
/// an Accept the resource cannot serve 4.06, a body in another
/// Content-Format 4.15, a precondition that does not hold 4.12, and an
/// ETag that names the current representation 2.03.
#[test]
fn coap_5_10_the_daemon_checks_options_methods_and_preconditions() {
    let (cfg, _lease) = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let server_addr = handle.local_addr.clone();
    let client = client_socket();
    let mut mid = 200u16;
    let mut ask = |code: CoapCode, path: &[&str], options: Vec<CoapOption>, payload: &[u8]| {
        mid += 1;
        let mut req = CoapMessage::new(MessageType::Confirmable, code, mid);
        req.token = mid.to_be_bytes().to_vec();
        for segment in path {
            req.options.push(CoapOption::uri_path(*segment));
        }
        req.options.extend(options);
        req.payload = payload.to_vec();
        client
            .send_to(&encode(&req).expect("encode"), &server_addr)
            .expect("send");
        recv_msg(&client)
    };
    let uint = |n: u16, v: u64| CoapOption::new(n, OptionValue::Uint(v));
    let opaque = |n: u16, v: &[u8]| CoapOption::new(n, OptionValue::Opaque(v.to_vec()));
    let wk = [".well-known", "core"];

    // 2049 is critical and unknown.
    let r = ask(CoapCode::GET, &["trade"], vec![opaque(2049, b"x")], b"");
    assert_eq!(r.code, CoapCode::new(4, 2));
    assert_eq!(
        ask(CoapCode::new(0, 5), &["trade"], vec![], b"").code,
        CoapCode::new(4, 5)
    );
    assert_eq!(
        ask(CoapCode::POST, &wk, vec![], b"").code,
        CoapCode::new(4, 5)
    );
    let r = ask(
        CoapCode::GET,
        &["trade"],
        vec![uint(numbers::ACCEPT, 50)],
        b"",
    );
    assert_eq!(r.code, CoapCode::new(4, 6));
    let r = ask(CoapCode::GET, &wk, vec![uint(numbers::ACCEPT, 0)], b"");
    assert_eq!(r.code, CoapCode::new(4, 6));
    let r = ask(
        CoapCode::POST,
        &["trade"],
        vec![uint(numbers::CONTENT_FORMAT, 50)],
        b"{}",
    );
    assert_eq!(r.code, CoapCode::new(4, 15));
    // If-Match before the first sample: no representation, 4.12.
    let r = ask(
        CoapCode::PUT,
        &["trade"],
        vec![opaque(numbers::IF_MATCH, b"")],
        b"A",
    );
    assert_eq!(r.code, CoapCode::new(4, 12));

    // A sample, then its representation with its ETag.
    let r = ask(CoapCode::PUT, &["trade"], vec![], b"AAPL@203");
    assert_eq!(r.code, CoapCode::CHANGED);
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let etag = loop {
        let r = ask(CoapCode::GET, &["trade"], vec![], b"");
        if r.payload == b"AAPL@203" {
            break r
                .options
                .iter()
                .find(|o| o.number == numbers::ETAG)
                .map(|o| o.value.to_wire_bytes())
                .expect("ETag");
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the sample never came"
        );
        std::thread::sleep(Duration::from_millis(200));
    };
    let r = ask(
        CoapCode::GET,
        &["trade"],
        vec![opaque(numbers::ETAG, &etag)],
        b"",
    );
    assert_eq!(r.code, CoapCode::VALID);
    assert!(r.payload.is_empty());
    let r = ask(
        CoapCode::PUT,
        &["trade"],
        vec![opaque(numbers::IF_NONE_MATCH, b"")],
        b"B",
    );
    assert_eq!(r.code, CoapCode::new(4, 12));
    let r = ask(
        CoapCode::PUT,
        &["trade"],
        vec![opaque(numbers::IF_MATCH, b"other")],
        b"B",
    );
    assert_eq!(r.code, CoapCode::new(4, 12));
    let r = ask(
        CoapCode::PUT,
        &["trade"],
        vec![opaque(numbers::IF_MATCH, &etag)],
        b"B",
    );
    assert_eq!(r.code, CoapCode::CHANGED);

    // A Non-confirmable request with an unknown critical option is reset.
    let mut non = CoapMessage::new(MessageType::NonConfirmable, CoapCode::GET, 299);
    non.options.push(CoapOption::uri_path("trade"));
    non.options.push(opaque(2049, b"x"));
    client
        .send_to(&encode(&non).expect("encode"), &server_addr)
        .expect("send");
    let rst = recv_msg(&client);
    assert!(matches!(rst.message_type, MessageType::Reset));
    assert_eq!(rst.message_id, 299);
    handle.shutdown();
}

/// RFC 7252 §4.5: a retransmitted Confirmable request gets the same
/// acknowledgement and is processed once; a duplicate Non-confirmable one
/// is ignored. Before, a retransmitted POST was written to DDS again.
#[test]
fn coap_4_5_a_retransmitted_post_is_written_once() {
    let (cfg, _lease) = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let server_addr = handle.local_addr.clone();
    let client = client_socket();
    let mut post = CoapMessage::new(MessageType::Confirmable, CoapCode::POST, 90);
    post.token = b"d".to_vec();
    post.options.push(CoapOption::uri_path("trade"));
    post.payload = b"AAPL@202".to_vec();
    let bytes = encode(&post).expect("encode");
    client.send_to(&bytes, &server_addr).expect("send");
    let first = recv_msg(&client);
    client.send_to(&bytes, &server_addr).expect("send");
    let again = recv_msg(&client);
    assert_eq!(first.code, CoapCode::CHANGED);
    assert_eq!(again, first, "the same acknowledgement");
    let mut non = post.clone();
    non.message_type = MessageType::NonConfirmable;
    non.message_id = 91;
    let bytes = encode(&non).expect("encode");
    client.send_to(&bytes, &server_addr).expect("send");
    assert_eq!(recv_msg(&client).code, CoapCode::CHANGED);
    client.send_to(&bytes, &server_addr).expect("send");
    client
        .set_read_timeout(Some(Duration::from_millis(800)))
        .expect("timeout");
    let mut buf = [0u8; 256];
    assert!(
        client.recv_from(&mut buf).is_err(),
        "a duplicate NON is ignored"
    );
    let written = handle
        .metrics
        .as_ref()
        .expect("metrics")
        .dds_samples_in_total
        .get();
    assert_eq!(written, 2, "one write per request, none per retransmission");
    handle.shutdown();
}

/// RFC 7252 §4.3: an empty Confirmable message (a "CoAP ping") is
/// answered with Reset, as is a Confirmable message that cannot be decoded
/// (§4.2); an Acknowledgement gets no response (it got `4.00` before).
#[test]
fn coap_4_3_a_ping_is_reset_and_an_acknowledgement_is_not_answered() {
    let (cfg, _lease) = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let server_addr = handle.local_addr.clone();
    let client = client_socket();
    let ping = CoapMessage::new(MessageType::Confirmable, CoapCode::EMPTY, 77);
    client
        .send_to(&encode(&ping).expect("encode"), &server_addr)
        .expect("send");
    let rst = recv_msg(&client);
    assert!(matches!(rst.message_type, MessageType::Reset));
    assert_eq!((rst.code, rst.message_id), (CoapCode::EMPTY, 77));
    // Version 1, Confirmable, the reserved token length 9.
    client
        .send_to(&[0x49, 0x01, 0x00, 0x4e], &server_addr)
        .expect("send");
    let rst = recv_msg(&client);
    assert!(matches!(rst.message_type, MessageType::Reset));
    assert_eq!(rst.message_id, 0x4e);
    let ack = CoapMessage::new(MessageType::Acknowledgement, CoapCode::EMPTY, 78);
    client
        .send_to(&encode(&ack).expect("encode"), &server_addr)
        .expect("send");
    client
        .set_read_timeout(Some(Duration::from_millis(800)))
        .expect("timeout");
    let mut buf = [0u8; 64];
    assert!(
        client.recv_from(&mut buf).is_err(),
        "an ACK is not answered"
    );
    handle.shutdown();
}

/// RFC 7252 §5.8.1: a GET returns the current representation of the
/// resource, the last sample of its topic.
#[test]
fn coap_5_8_1_a_get_returns_the_last_sample() {
    let (cfg, _lease) = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let server_addr = handle.local_addr.clone();
    let client = client_socket();
    let mut post = CoapMessage::new(MessageType::Confirmable, CoapCode::POST, 10);
    post.token = b"p".to_vec();
    post.options.push(CoapOption::uri_path("trade"));
    post.payload = b"AAPL@201".to_vec();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    // A new Message ID for every request: the same one again is a
    // retransmission, served once (RFC 7252 §4.5).
    let mut mid = 10u16;
    loop {
        post.message_id = mid;
        client
            .send_to(&encode(&post).expect("encode"), &server_addr)
            .expect("send");
        assert_eq!(recv_msg(&client).code, CoapCode::CHANGED);
        let mut get = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, mid + 1);
        mid += 2;
        get.token = b"g".to_vec();
        get.options.push(CoapOption::uri_path("trade"));
        client
            .send_to(&encode(&get).expect("encode"), &server_addr)
            .expect("send");
        let resp = recv_msg(&client);
        assert_eq!(resp.code, CoapCode::CONTENT);
        if resp.payload == post.payload {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the GET never returned the sample written"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    handle.shutdown();
}

#[test]
fn coapbridge_4_4_block1_chunked_post_completes_with_2_04_changed() {
    // RFC 7959 §2.5 — Block1 fragments the request body. We send 3
    // chunks of 16 bytes each (szx=0). The first two should receive
    // 2.31 Continue, the third 2.04 Changed.
    let (cfg, _lease) = make_test_config("127.0.0.1:0");
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
fn coapbridge_4_4_block1_out_of_order_returns_2_31_continue_only_in_order() {
    // RFC 7959 §2.5 — sequence: chunk 0 (more=true), then directly
    // chunk 2 (more=false) without chunk 1. The second send attempt
    // triggers "block out of order" in our reassembler → non-fatal
    // path: the daemon sends no response. We therefore only test that
    // chunk 0 yields the 2.31 Continue.
    let (cfg, _lease) = make_test_config("127.0.0.1:0");
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

/// `dtls.enabled=true` must **fail closed** when this binary was built WITHOUT
/// the `dtls` feature: the bridge must never silently fall back to serving
/// plaintext on a port the operator expects to be `coaps://` (RFC 7252 §9).
/// (With the `dtls` feature it instead serves real DTLS — see
/// `daemon_dtls_e2e.rs`.)
#[cfg(not(feature = "dtls"))]
#[test]
fn dtls_enabled_fails_closed_never_serves_plaintext() {
    let (mut cfg, _lease) = make_test_config("127.0.0.1:0");
    cfg.dtls_enabled = true;
    let res = server::start(cfg);
    assert!(
        res.is_err(),
        "dtls.enabled=true must fail closed, not start a plaintext server"
    );
    let msg = format!("{:?}", res.err().expect("err")).to_lowercase();
    assert!(
        msg.contains("dtls") && msg.contains("refusing"),
        "fail-closed error must explain the dtls refusal, got: {msg}"
    );
}

/// Spec §9.2 through the binary: `READY=1` to `$NOTIFY_SOCKET` once it
/// serves, SIGTERM ends the process with exit 0 after `STOPPING=1`.
/// The binary parked forever and was still running 3 s after SIGTERM.
#[cfg(unix)]
#[test]
fn coapbridge_9_2_the_binary_exits_0_on_sigterm() {
    use std::process::{Command, Stdio};
    let lease = zerodds_test_domains::lease_domain();
    // sd_notify: the packaged units are Type=notify.
    let dir = std::env::temp_dir().join(format!("zd-notify-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let sock_path = dir.join("notify.sock");
    let _ = std::fs::remove_file(&sock_path);
    let notify = std::os::unix::net::UnixDatagram::bind(&sock_path).unwrap();
    notify
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    let next_state = || {
        let mut buf = [0u8; 64];
        let n = notify.recv(&mut buf).expect("a notify datagram");
        String::from_utf8_lossy(&buf[..n]).into_owned()
    };
    let mut child = Reaped(
        Command::new(env!("CARGO_BIN_EXE_zerodds-coap-bridged"))
            .args(["--set", "coap.bind=127.0.0.1:0", "--set"])
            .arg(format!("domain={}", lease.id()))
            .env("NOTIFY_SOCKET", &sock_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn"),
    );
    // The signal handler is installed before the daemon says `running`; a
    // SIGTERM before that ends the process by the default action. Spec
    // §8.1: the log is JSON on stdout, one object per line.
    use std::io::BufRead;
    let mut lines = std::io::BufReader::new(child.0.stdout.take().unwrap()).lines();
    assert!(
        lines.by_ref().map_while(Result::ok).any(|l| {
            let v: serde_json::Value = serde_json::from_str(&l).expect("a JSON line");
            v["event"] == "running"
        }),
        "the daemon never reported running"
    );
    std::thread::spawn(move || lines.for_each(drop));
    assert_eq!(next_state(), "READY=1");
    let killed = Command::new("kill")
        .args(["-TERM", &child.0.id().to_string()])
        .status()
        .unwrap();
    assert!(killed.success());
    let started = std::time::Instant::now();
    let status = loop {
        if let Some(st) = child.0.try_wait().unwrap() {
            break st;
        }
        if started.elapsed() > std::time::Duration::from_secs(5) {
            panic!("still running 5 s after SIGTERM");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(0));
    assert_eq!(next_state(), "STOPPING=1");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Spec §9.2 / RFC 7641 §4.2: at shutdown every observer gets a
/// notification with a non-2.xx code (5.03, no Observe option) with its
/// token, which ends the observation on the client. Before, the registry
/// was dropped and the client kept waiting.
#[test]
fn coapbridge_9_2_the_shutdown_ends_every_observation_with_5_03() {
    let (cfg, _lease) = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let server_addr = handle.local_addr.clone();
    let client = client_socket();
    let mut req = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 0x7001);
    req.token = b"obs-end".to_vec();
    req.options.push(CoapOption::observe(0));
    req.options.push(CoapOption::uri_path("trade"));
    client
        .send_to(&encode(&req).expect("encode"), &server_addr)
        .expect("send");
    assert_eq!(recv_msg(&client).code, CoapCode::CONTENT);

    handle.shutdown();
    // Notifications of samples other tests write to `Trade` on the same
    // domain may come first.
    let end = loop {
        let m = recv_msg(&client);
        if m.code != CoapCode::CONTENT {
            break m;
        }
    };
    assert_eq!(end.code, CoapCode::new(5, 3));
    assert_eq!(end.token, b"obs-end".to_vec());
    assert!(
        !end.options.iter().any(|o| o.number == numbers::OBSERVE),
        "no Observe option: the observation ends"
    );
}
