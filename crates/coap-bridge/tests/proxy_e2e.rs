// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The daemon as a CoAP-to-CoAP proxy (RFC 7252 §5.7), against a CoAP
//! origin server in the test.

#![cfg(feature = "daemon")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::field_reassign_with_default,
    missing_docs
)]

use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use zerodds_coap_bridge::codec::{decode, encode};
use zerodds_coap_bridge::daemon::config::{DaemonConfig, TopicConfig};
use zerodds_coap_bridge::daemon::server;
use zerodds_coap_bridge::message::{CoapCode, CoapMessage, MessageType};
use zerodds_coap_bridge::option::{CoapOption, OptionValue, numbers, uint_from_bytes};

type Answer = Box<dyn Fn(&CoapMessage) -> Option<CoapMessage> + Send>;

/// A CoAP origin server: answers every request with what `answer` returns,
/// piggybacked on the ACK of a Confirmable request; `None` is no answer.
struct Origin {
    addr: SocketAddr,
    seen: Arc<Mutex<Vec<CoapMessage>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Origin {
    fn start(answer: Answer) -> Self {
        let socket = UdpSocket::bind("127.0.0.1:0").expect("origin bind");
        socket
            .set_read_timeout(Some(Duration::from_millis(100)))
            .expect("timeout");
        let addr = socket.local_addr().expect("addr");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (seen_c, stop_c) = (Arc::clone(&seen), Arc::clone(&stop));
        let thread = std::thread::spawn(move || {
            let mut buf = [0u8; 65535];
            while !stop_c.load(Ordering::SeqCst) {
                let Ok((n, from)) = socket.recv_from(&mut buf) else {
                    continue;
                };
                let Ok(req) = decode(&buf[..n]) else {
                    continue;
                };
                if !req.code.is_request() {
                    continue;
                }
                seen_c.lock().expect("seen").push(req.clone());
                if let Some(mut resp) = answer(&req) {
                    resp.token = req.token.clone();
                    if matches!(req.message_type, MessageType::Confirmable) {
                        resp.message_type = MessageType::Acknowledgement;
                        resp.message_id = req.message_id;
                    } else {
                        resp.message_type = MessageType::NonConfirmable;
                        resp.message_id = req.message_id.wrapping_add(1000);
                    }
                    let _ = socket.send_to(&encode(&resp).expect("encode"), from);
                }
            }
        });
        Self {
            addr,
            seen,
            stop,
            thread: Some(thread),
        }
    }

    fn requests(&self) -> Vec<CoapMessage> {
        self.seen.lock().expect("seen").clone()
    }
}

impl Drop for Origin {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn response(code: CoapCode, options: Vec<CoapOption>, payload: &[u8]) -> CoapMessage {
    let mut m = CoapMessage::new(MessageType::Acknowledgement, code, 0);
    m.options = options;
    m.payload = payload.to_vec();
    m
}

fn proxy_config() -> (DaemonConfig, zerodds_test_domains::DomainLease) {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.bind = "127.0.0.1:0".to_string();
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

fn client() -> UdpSocket {
    let s = UdpSocket::bind("127.0.0.1:0").expect("bind client");
    s.set_read_timeout(Some(Duration::from_secs(4)))
        .expect("timeout");
    s
}

fn recv(socket: &UdpSocket) -> CoapMessage {
    let mut buf = [0u8; 65535];
    let (n, _) = socket.recv_from(&mut buf).expect("recv");
    decode(&buf[..n]).expect("decode")
}

fn exchange(socket: &UdpSocket, to: &str, req: &CoapMessage) -> CoapMessage {
    socket
        .send_to(&encode(req).expect("encode"), to)
        .expect("send");
    recv(socket)
}

fn request(code: CoapCode, mid: u16, options: Vec<CoapOption>) -> CoapMessage {
    let mut m = CoapMessage::new(MessageType::Confirmable, code, mid);
    m.token = vec![0xc0, mid as u8];
    m.options = options;
    m
}

fn proxy_uri(uri: &str) -> CoapOption {
    CoapOption::new(numbers::PROXY_URI, OptionValue::String(uri.to_string()))
}

fn values(msg: &CoapMessage, number: u16) -> Vec<Vec<u8>> {
    msg.options
        .iter()
        .filter(|o| o.number == number)
        .map(|o| o.value.to_wire_bytes())
        .collect()
}

fn uint(msg: &CoapMessage, number: u16) -> Option<u64> {
    values(msg, number).first().and_then(|v| uint_from_bytes(v))
}

fn strings(msg: &CoapMessage, number: u16) -> Vec<String> {
    values(msg, number)
        .into_iter()
        .map(|v| String::from_utf8(v).expect("utf8"))
        .collect()
}

/// RFC 7252 §5.7.2: the forward proxy translates a Proxy-Uri request into
/// the Uri-* options, with its own token, a Hop-Limit (RFC 8768 §3) and no
/// Proxy-Uri; §5.6, §5.7.1: a fresh response comes from the cache with the
/// Max-Age reduced, a change makes it stale, and a stale one is validated
/// with its ETag; §4.5: a retransmitted request is carried out once.
#[test]
fn coap_5_7_2_a_forward_proxy_carries_out_the_request_and_serves_the_cache() {
    let origin = Origin::start(Box::new(|req: &CoapMessage| {
        let etag = CoapOption::new(numbers::ETAG, OptionValue::Opaque(b"e1".to_vec()));
        if req.code == CoapCode::POST {
            return Some(response(CoapCode::CHANGED, Vec::new(), b""));
        }
        if req.options.iter().any(|o| o.number == numbers::ETAG) {
            return Some(response(
                CoapCode::VALID,
                vec![
                    etag,
                    CoapOption::new(numbers::MAX_AGE, OptionValue::Uint(30)),
                ],
                b"",
            ));
        }
        Some(response(
            CoapCode::CONTENT,
            vec![
                etag,
                CoapOption::new(numbers::MAX_AGE, OptionValue::Uint(60)),
                CoapOption::content_format(0),
            ],
            b"v1",
        ))
    }));
    let (mut cfg, _lease) = proxy_config();
    cfg.proxy.forward = true;
    let mut handle = server::start(cfg).expect("daemon start");
    let daemon = handle.local_addr.clone();
    let c = client();
    let target = format!("coap://127.0.0.1:{}/res", origin.addr.port());

    let first = request(CoapCode::GET, 1, vec![proxy_uri(&target)]);
    let resp = exchange(&c, &daemon, &first);
    assert_eq!(resp.code, CoapCode::CONTENT);
    assert!(matches!(resp.message_type, MessageType::Acknowledgement));
    assert_eq!((resp.message_id, &resp.token), (1, &first.token));
    assert_eq!(resp.payload, b"v1");
    let seen = origin.requests();
    assert_eq!(seen.len(), 1);
    assert_eq!(
        strings(&seen[0], numbers::URI_PATH),
        vec!["res".to_string()]
    );
    assert!(values(&seen[0], numbers::URI_HOST).is_empty());
    assert!(values(&seen[0], numbers::PROXY_URI).is_empty());
    assert_eq!(uint(&seen[0], numbers::HOP_LIMIT), Some(16));
    assert_ne!(seen[0].token, first.token, "the proxy's own token");

    // §4.5: the same request again gets the same acknowledgement.
    let again = exchange(&c, &daemon, &first);
    assert_eq!(again, resp);
    // A new GET: from the cache, its Max-Age not beyond 60.
    let cached = exchange(
        &c,
        &daemon,
        &request(CoapCode::GET, 2, vec![proxy_uri(&target)]),
    );
    assert_eq!(cached.payload, b"v1");
    assert!(uint(&cached, numbers::MAX_AGE).is_some_and(|s| s <= 60));
    assert_eq!(origin.requests().len(), 1, "served from the cache");

    // §5.9.1.4: 2.04 Changed makes the stored response stale.
    let changed = exchange(
        &c,
        &daemon,
        &request(CoapCode::POST, 3, vec![proxy_uri(&target)]),
    );
    assert_eq!(changed.code, CoapCode::CHANGED);
    // §5.6.2: the stale response is validated with its ETag; 2.03 Valid
    // refreshes it with the new Max-Age.
    let validated = exchange(
        &c,
        &daemon,
        &request(CoapCode::GET, 4, vec![proxy_uri(&target)]),
    );
    assert_eq!(validated.code, CoapCode::CONTENT);
    assert_eq!(validated.payload, b"v1");
    assert!(uint(&validated, numbers::MAX_AGE).is_some_and(|s| s <= 30));
    let seen = origin.requests();
    assert_eq!(seen.len(), 3);
    assert_eq!(values(&seen[2], numbers::ETAG), vec![b"e1".to_vec()]);
    handle.shutdown();
}

/// RFC 7252 §5.7.1: "If the request to the destination times out, then a
/// 5.04 (Gateway Timeout) response MUST be returned"; §5.2.2: the request
/// is acknowledged empty first and the response is a separate Confirmable
/// message the client acknowledges.
#[test]
fn coap_5_7_1_a_silent_destination_is_5_04_after_an_empty_ack() {
    let origin = Origin::start(Box::new(|_req: &CoapMessage| None));
    let (mut cfg, _lease) = proxy_config();
    cfg.proxy.forward = true;
    cfg.proxy.timeout_ms = 3500;
    let mut handle = server::start(cfg).expect("daemon start");
    let daemon = handle.local_addr.clone();
    let c = client();
    let target = format!("coap://127.0.0.1:{}/quiet", origin.addr.port());
    let req = request(CoapCode::GET, 21, vec![proxy_uri(&target)]);
    let started = Instant::now();
    let ack = exchange(&c, &daemon, &req);
    assert!(matches!(ack.message_type, MessageType::Acknowledgement));
    assert_eq!((ack.code, ack.message_id), (CoapCode::EMPTY, 21));
    let late = recv(&c);
    assert!(started.elapsed() >= Duration::from_millis(3400));
    assert_eq!(late.code, CoapCode::new(5, 4));
    assert!(matches!(late.message_type, MessageType::Confirmable));
    assert_eq!(late.token, req.token);
    let confirm = CoapMessage::new(
        MessageType::Acknowledgement,
        CoapCode::EMPTY,
        late.message_id,
    );
    c.send_to(&encode(&confirm).expect("encode"), &daemon)
        .expect("send");
    // Acknowledged: no retransmission follows.
    c.set_read_timeout(Some(Duration::from_millis(3500)))
        .expect("timeout");
    let mut buf = [0u8; 2048];
    assert!(
        c.recv_from(&mut buf).is_err(),
        "retransmitted after the ACK"
    );
    assert!(
        origin.requests().len() >= 2,
        "the proxy retransmitted upstream"
    );
    handle.shutdown();
}

/// RFC 7252 §5.7.1: "Unsafe options in a response that are not recognized
/// by the CoAP-to-CoAP proxy server MUST lead to a 5.02 (Bad Gateway)
/// response"; a recognized one passes.
#[test]
fn coap_5_7_1_an_unsafe_option_in_the_response_is_5_02() {
    let origin = Origin::start(Box::new(|req: &CoapMessage| {
        let unsafe_opt = req
            .options
            .iter()
            .any(|o| o.number == numbers::URI_PATH && o.value.to_wire_bytes() == b"odd");
        let options = if unsafe_opt {
            vec![CoapOption::new(2050, OptionValue::Opaque(vec![1]))]
        } else {
            vec![CoapOption::new(2048, OptionValue::Opaque(vec![1]))]
        };
        Some(response(CoapCode::CONTENT, options, b"x"))
    }));
    let (mut cfg, _lease) = proxy_config();
    cfg.proxy.forward = true;
    let mut handle = server::start(cfg).expect("daemon start");
    let daemon = handle.local_addr.clone();
    let c = client();
    let base = format!("coap://127.0.0.1:{}", origin.addr.port());
    let odd = exchange(
        &c,
        &daemon,
        &request(CoapCode::GET, 31, vec![proxy_uri(&format!("{base}/odd"))]),
    );
    assert_eq!(odd.code, CoapCode::new(5, 2));
    let fine = exchange(
        &c,
        &daemon,
        &request(CoapCode::GET, 32, vec![proxy_uri(&format!("{base}/fine"))]),
    );
    assert_eq!(fine.code, CoapCode::CONTENT);
    assert_eq!(
        values(&fine, 2048),
        vec![vec![1]],
        "a safe option is forwarded"
    );
    handle.shutdown();
}

/// RFC 7252 §5.7.3: a reverse route serves the resources of its origin
/// under its own path, the Location-Path mapped back; the daemon's own
/// resources stay its own. §5.7.2: without `proxy.forward` a proxy request
/// is 5.05, except for a URI that names the daemon itself, which is a
/// local request.
#[test]
fn coap_5_7_3_a_reverse_route_reaches_its_origin_and_maps_the_location() {
    let origin = Origin::start(Box::new(|_req: &CoapMessage| {
        Some(response(
            CoapCode::new(2, 1),
            vec![
                CoapOption::new(numbers::LOCATION_PATH, OptionValue::String("base".into())),
                CoapOption::new(numbers::LOCATION_PATH, OptionValue::String("new".into())),
            ],
            b"",
        ))
    }));
    let (mut cfg, _lease) = proxy_config();
    cfg.proxy.reverse = vec![(
        "far".to_string(),
        format!("coap://127.0.0.1:{}/base", origin.addr.port()),
    )];
    let mut handle = server::start(cfg).expect("daemon start");
    let daemon = handle.local_addr.clone();
    let c = client();
    let mut post = request(
        CoapCode::POST,
        41,
        vec![CoapOption::uri_path("far"), CoapOption::uri_path("x")],
    );
    post.payload = b"body".to_vec();
    let created = exchange(&c, &daemon, &post);
    assert_eq!(created.code, CoapCode::new(2, 1));
    assert_eq!(
        strings(&created, numbers::LOCATION_PATH),
        vec!["far".to_string(), "new".into()]
    );
    let seen = origin.requests();
    assert_eq!(
        strings(&seen[0], numbers::URI_PATH),
        vec!["base".to_string(), "x".into()]
    );
    assert_eq!(seen[0].payload, b"body");

    let local = exchange(
        &c,
        &daemon,
        &request(CoapCode::GET, 42, vec![CoapOption::uri_path("trade")]),
    );
    assert_eq!(local.code, CoapCode::CONTENT);
    let own = format!("coap://{daemon}/trade");
    let named = exchange(
        &c,
        &daemon,
        &request(CoapCode::GET, 43, vec![proxy_uri(&own)]),
    );
    assert_eq!(named.code, CoapCode::CONTENT, "the daemon itself is local");
    let elsewhere = exchange(
        &c,
        &daemon,
        &request(CoapCode::GET, 44, vec![proxy_uri("coap://192.0.2.1/x")]),
    );
    assert_eq!(elsewhere.code, CoapCode::new(5, 5));
    handle.shutdown();
}

/// RFC 7959 §2.10 with RFC 7252 §11.3: the proxy reassembles the Block2
/// blocks of its destination and gives a client that is not authenticated
/// blocks of at most eight times its request, the later ones from the
/// cache.
#[test]
fn rfc7959_2_10_the_proxy_reassembles_and_cuts_blocks() {
    let body: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
    let served = body.clone();
    let origin = Origin::start(Box::new(move |req: &CoapMessage| {
        let num = req
            .options
            .iter()
            .find(|o| o.number == numbers::BLOCK2)
            .and_then(|o| uint_from_bytes(&o.value.to_wire_bytes()))
            .map_or(0, |v| (v >> 4) as usize);
        let start = num * 1024;
        let end = (start + 1024).min(served.len());
        let more = end < served.len();
        Some(response(
            CoapCode::CONTENT,
            vec![
                CoapOption::new(numbers::ETAG, OptionValue::Opaque(b"big".to_vec())),
                CoapOption::block2(num as u32, more, 6),
            ],
            &served[start..end],
        ))
    }));
    let (mut cfg, _lease) = proxy_config();
    cfg.proxy.forward = true;
    let mut handle = server::start(cfg).expect("daemon start");
    let daemon = handle.local_addr.clone();
    let c = client();
    let target = format!("coap://127.0.0.1:{}/big", origin.addr.port());
    let first = exchange(
        &c,
        &daemon,
        &request(CoapCode::GET, 51, vec![proxy_uri(&target)]),
    );
    assert_eq!(first.code, CoapCode::CONTENT);
    assert_eq!(uint(&first, numbers::SIZE2), Some(3000));
    let block = uint(&first, numbers::BLOCK2).expect("Block2");
    let szx = (block & 7) as u8;
    assert!(block & 8 != 0, "more blocks");
    assert!(szx <= 4, "at most 8 times a request of about 40 octets");
    let size = 16usize << szx;
    assert_eq!(first.payload, body[..size]);
    assert_eq!(origin.requests().len(), 3, "three blocks from the origin");

    let next = exchange(
        &c,
        &daemon,
        &request(
            CoapCode::GET,
            52,
            vec![proxy_uri(&target), CoapOption::block2(1, false, szx)],
        ),
    );
    assert_eq!(next.code, CoapCode::CONTENT);
    assert_eq!(next.payload, body[size..2 * size]);
    assert_eq!(
        origin.requests().len(),
        3,
        "the second block from the cache"
    );
    handle.shutdown();
}

/// RFC 7252 §5.7, §9 with §11.2: a `coaps` destination is reached over DTLS
/// (here with a pre-shared key) and, by default, only for a client that
/// itself comes over DTLS.
#[cfg(feature = "dtls")]
#[test]
fn coap_5_7_a_coaps_destination_is_reached_over_dtls() {
    let key = vec![0x5a; 16];
    let (mut origin_cfg, _origin_lease) = proxy_config();
    origin_cfg.dtls_enabled = true;
    origin_cfg.bind_dtls = Some("127.0.0.1:0".to_string());
    origin_cfg.dtls.psk.add("proxy", key.clone()).expect("psk");
    let mut origin = server::start(origin_cfg).expect("origin start");
    let mut dtls = None;
    for _ in 0..100 {
        dtls = origin.dtls_local_addr();
        if dtls.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let dtls = dtls.expect("origin DTLS listener");
    let target = format!("coaps://127.0.0.1:{}/trade", dtls.port());

    for require in [true, false] {
        let (mut cfg, _lease) = proxy_config();
        cfg.proxy.forward = true;
        cfg.proxy.require_dtls_for_coaps = require;
        cfg.proxy.dtls_psk = Some(("proxy".to_string(), key.clone()));
        let mut handle = server::start(cfg).expect("daemon start");
        let daemon = handle.local_addr.clone();
        let c = client();
        let resp = exchange(
            &c,
            &daemon,
            &request(CoapCode::GET, 61, vec![proxy_uri(&target)]),
        );
        if require {
            assert_eq!(resp.code, CoapCode::new(4, 1), "a plain client leg");
        } else {
            let resp = if resp.code == CoapCode::EMPTY {
                recv(&c)
            } else {
                resp
            };
            assert_eq!(resp.code, CoapCode::CONTENT, "over DTLS to the origin");
        }
        handle.shutdown();
    }
    origin.shutdown();
}

/// RFC 7967 §2: the proxy forwards No-Response (it is Unsafe-to-Forward, an
/// intermediary interprets it) and applies it to its own response: a
/// suppressed 2.05 does not reach the client, its Confirmable request gets
/// an empty acknowledgement; without the bit the response comes.
#[test]
fn rfc7967_2_the_proxy_forwards_no_response_and_suppresses_its_response() {
    let origin = Origin::start(Box::new(|_req: &CoapMessage| {
        Some(response(CoapCode::CONTENT, Vec::new(), b"v"))
    }));
    let (mut cfg, _lease) = proxy_config();
    cfg.proxy.forward = true;
    let mut handle = server::start(cfg).expect("daemon start");
    let daemon = handle.local_addr.clone();
    let c = client();
    let target = format!("coap://127.0.0.1:{}/res", origin.addr.port());
    let quiet = request(
        CoapCode::GET,
        41,
        vec![
            proxy_uri(&target),
            CoapOption::new(numbers::NO_RESPONSE, OptionValue::Uint(2)),
        ],
    );
    let ack = exchange(&c, &daemon, &quiet);
    assert_eq!(ack.code, CoapCode::EMPTY);
    assert!(matches!(ack.message_type, MessageType::Acknowledgement));
    assert_eq!(ack.message_id, 41);
    let seen = origin.requests();
    assert_eq!(uint(&seen[0], numbers::NO_RESPONSE), Some(2), "forwarded");
    // Not suppressed: 4.xx only.
    let told = request(
        CoapCode::GET,
        42,
        vec![
            proxy_uri(&format!("coap://127.0.0.1:{}/other", origin.addr.port())),
            CoapOption::new(numbers::NO_RESPONSE, OptionValue::Uint(8)),
        ],
    );
    let resp = exchange(&c, &daemon, &told);
    assert_eq!(resp.code, CoapCode::CONTENT);
    assert_eq!(resp.payload, b"v");
    handle.shutdown();
}
