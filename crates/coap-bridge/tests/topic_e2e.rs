// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The daemon against a DDS participant of its own in the test (spec
//! §12.2): what a CoAP client sends reaches a DDS reader, what a DDS writer
//! sends reaches a CoAP observer, in every form of §4.5; DELETE disposes
//! (§4.2); No-Response (RFC 7967); the observe rules of §4.3 and the QoS of
//! §6; the blocks of §4.4; the catalog and the schema of §5.2; the metrics
//! of §8.2.

#![cfg(feature = "daemon")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    missing_docs
)]

use std::net::UdpSocket;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use zerodds_coap_bridge::codec::{decode, encode};
use zerodds_coap_bridge::daemon::config::{DaemonConfig, TopicConfig};
use zerodds_coap_bridge::daemon::representation::{Form, TopicType};
use zerodds_coap_bridge::daemon::server;
use zerodds_coap_bridge::message::{CoapCode, CoapMessage, MessageType};
use zerodds_coap_bridge::option::{CoapOption, OptionValue, numbers};
use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_rtps::wire_types::GuidPrefix;

const IDL: &str = "module Sensor {\n  @appendable struct Reading {\n    @key long id;\n    double value;\n    string unit;\n  };\n};\nmodule Plain {\n  struct Note {\n    string text;\n  };\n};\n";

/// The IDL file of the test's types.
fn idl_file(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("topic-e2e-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("sensor.idl");
    std::fs::write(&path, IDL).unwrap();
    path
}

fn reading() -> TopicType {
    TopicType::from_idl(IDL, "Sensor::Reading").unwrap()
}

fn topic(dds: &str, path: &str, direction: &str, idl: Option<&PathBuf>) -> TopicConfig {
    TopicConfig {
        dds_name: dds.to_string(),
        dds_type: dds.to_string(),
        coap_uri_path: path.to_string(),
        direction: direction.to_string(),
        reliability: "reliable".to_string(),
        durability: "volatile".to_string(),
        history_depth: 10,
        history_kind: "keep_last".to_string(),
        idl_file: idl.map(|p| p.display().to_string()).unwrap_or_default(),
        extensibility: "appendable".to_string(),
        data_representation: "xcdr2".to_string(),
        ..TopicConfig::default()
    }
}

/// A configuration on a leased DDS domain with the reading topic.
fn config(name: &str) -> (DaemonConfig, zerodds_test_domains::DomainLease, PathBuf) {
    let idl = idl_file(name);
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.bind = "127.0.0.1:0".to_string();
    let lease = zerodds_test_domains::lease_domain();
    cfg.domain = lease.id();
    cfg.topics.push(topic(
        "Sensor::Reading",
        "sensors/reading",
        "bidir",
        Some(&idl),
    ));
    (cfg, lease, idl)
}

/// A DDS participant of the test's own on `domain`.
fn peer(domain: i32, tag: u8) -> Arc<DcpsRuntime> {
    let mut prefix = [0u8; 12];
    prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
    prefix[4..8].copy_from_slice(&domain.to_be_bytes());
    prefix[8] = tag;
    prefix[11] = 0xc0;
    DcpsRuntime::start(
        domain,
        GuidPrefix::from_bytes(prefix),
        RuntimeConfig::default(),
    )
    .unwrap()
}

fn until(dds: &DcpsRuntime, what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < deadline, "{what}");
        dds.wait_match_event(Duration::from_millis(200));
    }
}

/// A CoAP client of the test.
struct Client {
    socket: UdpSocket,
    server: String,
    mid: u16,
}

impl Client {
    fn new(server: &str) -> Self {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        Self {
            socket,
            server: server.to_string(),
            mid: 0x4000,
        }
    }

    fn send(&self, msg: &CoapMessage) {
        self.socket
            .send_to(&encode(msg).unwrap(), &self.server)
            .unwrap();
    }

    fn recv(&self, wait: Duration) -> Option<CoapMessage> {
        self.socket.set_read_timeout(Some(wait)).unwrap();
        let mut buf = [0u8; 65_535];
        let (n, _) = self.socket.recv_from(&mut buf).ok()?;
        Some(decode(&buf[..n]).unwrap())
    }

    fn message(
        &mut self,
        mtype: MessageType,
        code: CoapCode,
        path: &str,
        options: Vec<CoapOption>,
        payload: &[u8],
    ) -> CoapMessage {
        self.mid = self.mid.wrapping_add(1);
        let mut req = CoapMessage::new(mtype, code, self.mid);
        req.token = self.mid.to_be_bytes().to_vec();
        for segment in path.split('/').filter(|s| !s.is_empty()) {
            req.options.push(CoapOption::uri_path(segment));
        }
        req.options.extend(options);
        req.options.sort_by_key(|o| o.number);
        req.payload = payload.to_vec();
        req
    }

    /// A Confirmable request and its response (by token; notifications in
    /// between are acknowledged and skipped).
    fn request(
        &mut self,
        code: CoapCode,
        path: &str,
        options: Vec<CoapOption>,
        payload: &[u8],
    ) -> CoapMessage {
        let req = self.message(MessageType::Confirmable, code, path, options, payload);
        self.send(&req);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let resp = self.recv(left).expect("a response");
            // The piggybacked response, or an empty acknowledgement (RFC
            // 7967), carries the request's Message ID.
            if matches!(resp.message_type, MessageType::Acknowledgement)
                && resp.message_id == req.message_id
            {
                return resp;
            }
            if matches!(resp.message_type, MessageType::Confirmable) {
                self.answer(&resp, MessageType::Acknowledgement);
            }
        }
    }

    fn answer(&self, msg: &CoapMessage, mtype: MessageType) {
        self.send(&CoapMessage::new(mtype, CoapCode::EMPTY, msg.message_id));
    }

    /// A GET and every Block2 block after the first (RFC 7959 §2.4).
    fn get_all(&mut self, path: &str) -> (CoapMessage, Vec<u8>) {
        let first = self.request(CoapCode::GET, path, vec![], b"");
        let mut body = first.payload.clone();
        let mut block = option(&first, numbers::BLOCK2);
        let mut num = 1u64;
        while let Some(b) = block.filter(|b| b & 0x08 != 0) {
            let next = self.request(
                CoapCode::GET,
                path,
                vec![uint(numbers::BLOCK2, (num << 4) | (b & 0x07))],
                b"",
            );
            body.extend_from_slice(&next.payload);
            block = option(&next, numbers::BLOCK2);
            num += 1;
            assert!(num < 256, "too many blocks");
        }
        (first, body)
    }

    /// Answers pings with a Reset for `period`; any other message is a
    /// failure.
    fn idle(&self, period: Duration) {
        let end = Instant::now() + period;
        while let Some(left) = end.checked_duration_since(Instant::now()) {
            if let Some(m) = self.recv(left.max(Duration::from_millis(1))) {
                assert_eq!(m.code, CoapCode::EMPTY, "only pings while idle: {m:?}");
                self.answer(&m, MessageType::Reset);
            }
        }
    }
}

fn uint(n: u16, v: u64) -> CoapOption {
    CoapOption::new(n, OptionValue::Uint(v))
}

fn option(msg: &CoapMessage, n: u16) -> Option<u64> {
    msg.options
        .iter()
        .find(|o| o.number == n)
        .and_then(|o| zerodds_coap_bridge::option::uint_from_bytes(&o.value.to_wire_bytes()))
}

fn has(msg: &CoapMessage, n: u16) -> bool {
    msg.options.iter().any(|o| o.number == n)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A sample as [`alive`] gives it: its CDR bytes, representation
/// identifier and key hash.
type Alive = (Vec<u8>, [u8; 2], Option<[u8; 16]>);

/// `post` (one request, one sample of the daemon's DataWriter) until a
/// sample reaches `rx`, the samples of the later requests taken
/// ([`zerodds_test_domains::publish_until_received`]: the reader's match
/// count says nothing about the DataWriter's). Returns the requests and
/// the first sample as [`alive`] gives it.
fn post_until_received(
    rx: &std::sync::mpsc::Receiver<UserSample>,
    post: impl FnMut(),
) -> (i64, Alive) {
    zerodds_test_domains::publish_until_received(
        rx,
        |sample| match sample {
            UserSample::Alive {
                payload,
                representation_identifier,
                key_hash,
                source_sequence_number,
                ..
            } => Some((
                source_sequence_number,
                (payload.to_vec(), representation_identifier, key_hash),
            )),
            _ => None,
        },
        post,
    )
}

fn alive(rx: &std::sync::mpsc::Receiver<UserSample>) -> Alive {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left).expect("a DDS sample") {
            UserSample::Alive {
                payload,
                representation_identifier,
                key_hash,
                ..
            } => return (payload.to_vec(), representation_identifier, key_hash),
            _ => continue,
        }
    }
}

/// Spec §4.2, §4.5: a POST in each of the five forms reaches a DDS reader
/// as the same sample, XCDR2 little endian under `D_CDR2_LE` (the type is
/// appendable), keyed by its instance; a body that is no sample is 4.00, a
/// Content-Format of no form 4.15. A GET gives the sample in the form
/// Accept names, 65000 without Accept. The catalog names the forms, the
/// type and the schema (§5.2); the schema is the IDL. §8.2: the requests
/// are counted by method and code, the samples by topic.
#[test]
fn coapbridge_4_5_every_form_goes_to_dds_and_comes_back_in_every_form() {
    let (mut cfg, lease, _idl) = config("forms");
    cfg.metrics_enabled = true;
    cfg.metrics_addr = "127.0.0.1:0".to_string();
    let mut handle = server::start(cfg).expect("daemon start");
    let mut client = Client::new(&handle.local_addr);
    let dds = peer(lease.id(), 1);
    let (_reader, rx) = dds
        .register_user_reader(UserReaderConfig {
            topic_name: "Sensor::Reading".into(),
            type_name: "Sensor::Reading".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    let probe = json!({ "id": 0, "value": 0.0, "unit": "u" }).to_string();
    let (probes, _) = post_until_received(&rx, || {
        let resp = client.request(
            CoapCode::POST,
            "sensors/reading",
            vec![uint(numbers::CONTENT_FORMAT, 50)],
            probe.as_bytes(),
        );
        assert_eq!(resp.code, CoapCode::CHANGED);
    });
    let ty = reading();
    let numbers_of = [
        (Form::Json, 50u64),
        (Form::Cbor, 60),
        (Form::Cdr2Le, 65000),
        (Form::Cdr2Be, 65001),
        (Form::Cdr1Le, 65002),
    ];
    let mut last = Value::Null;
    for (i, (form, number)) in numbers_of.iter().enumerate() {
        let value = json!({ "id": i + 1, "value": 0.5 + i as f64, "unit": format!("u{i}") });
        let body = ty.write(*form, &value).unwrap();
        let resp = client.request(
            CoapCode::POST,
            "sensors/reading",
            vec![uint(numbers::CONTENT_FORMAT, *number)],
            &body,
        );
        assert_eq!(resp.code, CoapCode::CHANGED, "{form:?}");
        let (payload, identifier, key_hash) = alive(&rx);
        assert_eq!(identifier, [0x00, 0x09], "D_CDR2_LE");
        assert_eq!(
            ty.read_body(&payload, identifier).unwrap(),
            value,
            "{form:?}"
        );
        assert_eq!(key_hash, Some(ty.key_hash(&value).unwrap()));
        last = value;
    }
    let bad = client.request(
        CoapCode::POST,
        "sensors/reading",
        vec![uint(numbers::CONTENT_FORMAT, 50)],
        b"{\"id\": \"seven\"}",
    );
    assert_eq!(bad.code, CoapCode::BAD_REQUEST);
    let xml = client.request(
        CoapCode::POST,
        "sensors/reading",
        vec![uint(numbers::CONTENT_FORMAT, 41)],
        b"<x/>",
    );
    assert_eq!(xml.code, CoapCode::new(4, 15));

    // The daemon's own reader has the last sample.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let r = client.request(
            CoapCode::GET,
            "sensors/reading",
            vec![uint(numbers::ACCEPT, 50)],
            b"",
        );
        if !r.payload.is_empty() && serde_json::from_slice::<Value>(&r.payload).unwrap() == last {
            break;
        }
        assert!(Instant::now() < deadline, "the last sample never came");
        std::thread::sleep(Duration::from_millis(100));
    }
    for (form, number) in numbers_of {
        let r = client.request(
            CoapCode::GET,
            "sensors/reading",
            vec![uint(numbers::ACCEPT, number)],
            b"",
        );
        assert_eq!(r.code, CoapCode::CONTENT, "{form:?}");
        assert_eq!(option(&r, numbers::CONTENT_FORMAT), Some(number));
        assert_eq!(ty.read(form, &r.payload).unwrap(), last, "{form:?}");
    }
    let plain = client.request(CoapCode::GET, "sensors/reading", vec![], b"");
    assert_eq!(option(&plain, numbers::CONTENT_FORMAT), Some(65000));
    assert!(has(&plain, numbers::ETAG));
    let none = client.request(
        CoapCode::GET,
        "sensors/reading",
        vec![uint(numbers::ACCEPT, 41)],
        b"",
    );
    assert_eq!(none.code, CoapCode::new(4, 6));

    // §5.2: the catalog and the schema.
    let (_, catalog) = client.get_all(".well-known/core");
    let text = String::from_utf8(catalog).unwrap();
    assert!(
        text.contains(
            "</sensors/reading>;rt=\"dds.topic\";ct=\"65000 65001 65002 50 60\";type=\"Sensor::Reading\""
        ),
        "{text}"
    );
    assert!(
        text.contains("</schema/sensors/reading>;rt=\"dds.schema\";ct=0"),
        "{text}"
    );
    let (schema, idl) = client.get_all("schema/sensors/reading");
    assert_eq!(schema.code, CoapCode::CONTENT);
    assert_eq!(option(&schema, numbers::CONTENT_FORMAT), Some(0));
    let idl = String::from_utf8(idl).unwrap();
    assert!(idl.contains("struct Reading"), "{idl}");
    assert!(TopicType::from_idl(&idl, "Sensor::Reading").is_ok());
    assert_eq!(
        client
            .request(CoapCode::POST, "schema/sensors/reading", vec![], b"")
            .code,
        CoapCode::new(4, 5)
    );

    // §8.2.
    let admin: std::net::SocketAddr = handle.admin_addr.clone().unwrap().parse().unwrap();
    let mut s = std::net::TcpStream::connect(admin).unwrap();
    use std::io::{Read, Write};
    s.write_all(b"GET /metrics HTTP/1.1\r\nHost: x\r\n\r\n")
        .unwrap();
    let mut page = String::new();
    s.read_to_string(&mut page).ok();
    // The five forms and the POSTs before them.
    let changed = 5 + probes;
    for line in [
        format!("zerodds_coap_bridge_requests_total{{code=\"2.04\",method=\"POST\"}} {changed}"),
        "zerodds_coap_bridge_requests_total{code=\"4.00\",method=\"POST\"} 1".to_string(),
        "zerodds_coap_bridge_requests_total{code=\"4.15\",method=\"POST\"} 1".to_string(),
        format!("zerodds_coap_bridge_dds_samples_published{{topic=\"Sensor::Reading\"}} {changed}"),
    ] {
        assert!(page.contains(&line), "{line} in\n{page}");
    }
    assert!(page.contains("zerodds_coap_bridge_dds_samples_received{topic=\"Sensor::Reading\"}"));
    dds.shutdown();
    handle.shutdown();
}

/// Spec §4.3, §6, RFC 7641: a DDS sample in XCDR1 becomes a notification in
/// the form the observer asked for (JSON); on a RELIABLE topic it is
/// Confirmable; a Reset to a notification ends the observation (§3.6), no
/// notification follows. The first response to an observer of a VOLATILE
/// topic is empty.
#[test]
fn coapbridge_4_3_a_dds_sample_is_a_confirmable_notification_until_a_reset() {
    let (cfg, lease, _idl) = config("notify");
    let mut handle = server::start(cfg).expect("daemon start");
    let mut client = Client::new(&handle.local_addr);
    let dds = peer(lease.id(), 2);
    let writer = dds
        .register_user_writer(UserWriterConfig {
            topic_name: "Sensor::Reading".into(),
            type_name: "Sensor::Reading".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    until(&dds, "the daemon's reader matched", || {
        dds.user_writer_matched_count(writer) >= 1
    });
    let ty = reading();
    let first = client.request(
        CoapCode::GET,
        "sensors/reading",
        vec![uint(numbers::OBSERVE, 0), uint(numbers::ACCEPT, 50)],
        b"",
    );
    assert_eq!(first.code, CoapCode::CONTENT);
    assert!(
        first.payload.is_empty(),
        "VOLATILE: nothing in the first response"
    );
    assert!(has(&first, numbers::OBSERVE));
    let token = first.token.clone();

    let a = json!({ "id": 1, "value": 2.5, "unit": "V" });
    dds.write_user_sample(writer, ty.write(Form::Cdr1Le, &a).unwrap())
        .unwrap();
    let n = client.recv(Duration::from_secs(5)).expect("a notification");
    assert_eq!(n.token, token);
    assert!(
        matches!(n.message_type, MessageType::Confirmable),
        "RELIABLE"
    );
    assert_eq!(option(&n, numbers::CONTENT_FORMAT), Some(50));
    assert_eq!(serde_json::from_slice::<Value>(&n.payload).unwrap(), a);
    let seq_a = option(&n, numbers::OBSERVE).unwrap();
    client.answer(&n, MessageType::Acknowledgement);

    let b = json!({ "id": 1, "value": 3.5, "unit": "V" });
    dds.write_user_sample(writer, ty.write(Form::Cdr1Le, &b).unwrap())
        .unwrap();
    let n = client.recv(Duration::from_secs(5)).expect("a notification");
    assert!(option(&n, numbers::OBSERVE).unwrap() > seq_a);
    assert_eq!(serde_json::from_slice::<Value>(&n.payload).unwrap(), b);
    client.answer(&n, MessageType::Reset);
    std::thread::sleep(Duration::from_millis(300));

    dds.write_user_sample(
        writer,
        ty.write(Form::Cdr1Le, &json!({ "id": 1, "value": 4.5, "unit": "V" }))
            .unwrap(),
    )
    .unwrap();
    assert!(
        client.recv(Duration::from_millis(1500)).is_none(),
        "no notification after the Reset"
    );
    dds.shutdown();
    handle.shutdown();
}

/// Spec §4.2: DELETE disposes the instance it names, by `key_hash` in
/// Uri-Query or by a sample in the body; one that names none is 4.00. A
/// type without key has no instance (4.05); a topic without writer takes
/// no POST or DELETE, one without reader no GET (4.05).
#[test]
fn coapbridge_4_2_delete_disposes_the_instance_it_names() {
    let (mut cfg, lease, idl) = config("delete");
    cfg.topics
        .push(topic("Plain::Note", "notes", "in", Some(&idl)));
    cfg.topics.push(topic("Trade", "trade", "out", None));
    let mut handle = server::start(cfg).expect("daemon start");
    let mut client = Client::new(&handle.local_addr);
    let dds = peer(lease.id(), 3);
    let (_reader, rx) = dds
        .register_user_reader(UserReaderConfig {
            topic_name: "Sensor::Reading".into(),
            type_name: "Sensor::Reading".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    let ty = reading();
    let value = json!({ "id": 9, "value": 1.0, "unit": "A" });
    let key = ty.key_hash(&value).unwrap();
    post_until_received(&rx, || {
        let resp = client.request(
            CoapCode::POST,
            "sensors/reading",
            vec![uint(numbers::CONTENT_FORMAT, 50)],
            value.to_string().as_bytes(),
        );
        assert_eq!(resp.code, CoapCode::CHANGED);
    });

    let disposed = |rx: &std::sync::mpsc::Receiver<UserSample>| {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if let UserSample::Lifecycle { key_hash, .. } =
                rx.recv_timeout(left).expect("a dispose")
            {
                return key_hash;
            }
        }
    };
    let by_hash = client.request(
        CoapCode::DELETE,
        "sensors/reading",
        vec![CoapOption::new(
            numbers::URI_QUERY,
            OptionValue::String(format!("key_hash={}", hex(&key))),
        )],
        b"",
    );
    assert_eq!(by_hash.code, CoapCode::DELETED);
    assert_eq!(disposed(&rx), key);
    let by_body = client.request(
        CoapCode::DELETE,
        "sensors/reading",
        vec![uint(numbers::CONTENT_FORMAT, 50)],
        value.to_string().as_bytes(),
    );
    assert_eq!(by_body.code, CoapCode::DELETED);
    assert_eq!(disposed(&rx), key);
    assert_eq!(
        client
            .request(CoapCode::DELETE, "sensors/reading", vec![], b"")
            .code,
        CoapCode::BAD_REQUEST
    );
    assert_eq!(
        client.request(CoapCode::DELETE, "notes", vec![], b"").code,
        CoapCode::new(4, 5)
    );
    assert_eq!(
        client.request(CoapCode::GET, "notes", vec![], b"").code,
        CoapCode::new(4, 5)
    );
    assert_eq!(
        client.request(CoapCode::POST, "trade", vec![], b"x").code,
        CoapCode::new(4, 5)
    );
    assert_eq!(
        client.request(CoapCode::DELETE, "trade", vec![], b"").code,
        CoapCode::new(4, 5)
    );
    dds.shutdown();
    handle.shutdown();
}

/// RFC 7967 §2.1: a response of a suppressed class does not leave; a
/// Confirmable request gets an empty acknowledgement instead, a
/// Non-confirmable one nothing; an empty No-Response suppresses nothing.
#[test]
fn rfc7967_a_suppressed_response_does_not_leave() {
    let (cfg, _lease, _idl) = config("no-response");
    let mut handle = server::start(cfg).expect("daemon start");
    let mut client = Client::new(&handle.local_addr);
    let body = json!({ "id": 1, "value": 1.0, "unit": "V" }).to_string();
    let quiet = client.request(
        CoapCode::POST,
        "sensors/reading",
        vec![
            uint(numbers::CONTENT_FORMAT, 50),
            uint(numbers::NO_RESPONSE, 2),
        ],
        body.as_bytes(),
    );
    assert_eq!(quiet.code, CoapCode::EMPTY);
    assert!(matches!(quiet.message_type, MessageType::Acknowledgement));
    assert!(quiet.payload.is_empty());
    let missing = client.request(
        CoapCode::GET,
        "nowhere",
        vec![uint(numbers::NO_RESPONSE, 8)],
        b"",
    );
    assert_eq!(missing.code, CoapCode::EMPTY);
    let told = client.request(
        CoapCode::GET,
        "nowhere",
        vec![CoapOption::new(numbers::NO_RESPONSE, OptionValue::Empty)],
        b"",
    );
    assert_eq!(told.code, CoapCode::new(4, 4));
    let non = client.message(
        MessageType::NonConfirmable,
        CoapCode::POST,
        "sensors/reading",
        vec![
            uint(numbers::CONTENT_FORMAT, 50),
            uint(numbers::NO_RESPONSE, 2),
        ],
        body.as_bytes(),
    );
    client.send(&non);
    assert!(client.recv(Duration::from_millis(700)).is_none());
    handle.shutdown();
}

/// Spec §6: TRANSIENT_LOCAL with KEEP_LAST(3) gives a new observer the
/// last three samples, the first in the response, two as notifications;
/// each carries the Max-Age left of the lifespan. A topic that goes
/// longer than its deadline without a sample answers a GET with 5.03 and
/// Max-Age 0 and ends its observations with a 5.03 notification.
#[test]
fn coapbridge_6_replay_lifespan_and_deadline() {
    let (mut cfg, _lease, idl) = config("replay");
    cfg.topics[0].durability = "transient_local".to_string();
    cfg.topics[0].history_depth = 3;
    cfg.topics[0].lifespan_ms = Some(60_000);
    let mut late = topic("Sensor::Late", "sensors/late", "bidir", Some(&idl));
    late.dds_type = "Sensor::Reading".to_string();
    late.deadline_ms = Some(1_500);
    cfg.topics.push(late);
    let mut handle = server::start(cfg).expect("daemon start");
    let mut client = Client::new(&handle.local_addr);
    // The observer of `sensors/late` registers before its deadline passes.
    let mut late_client = Client::new(&handle.local_addr);
    let watch = late_client.request(
        CoapCode::GET,
        "sensors/late",
        vec![uint(numbers::OBSERVE, 0)],
        b"",
    );
    assert_eq!(watch.code, CoapCode::CONTENT);
    for id in 1..=4 {
        let body = json!({ "id": id, "value": 1.0, "unit": "V" }).to_string();
        let r = client.request(
            CoapCode::POST,
            "sensors/reading",
            vec![uint(numbers::CONTENT_FORMAT, 50)],
            body.as_bytes(),
        );
        assert_eq!(r.code, CoapCode::CHANGED);
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let r = client.request(
            CoapCode::GET,
            "sensors/reading",
            vec![uint(numbers::ACCEPT, 50)],
            b"",
        );
        if !r.payload.is_empty() && serde_json::from_slice::<Value>(&r.payload).unwrap()["id"] == 4
        {
            let age = option(&r, numbers::MAX_AGE).expect("Max-Age");
            assert!((59..=60).contains(&age), "{age}");
            break;
        }
        assert!(Instant::now() < deadline, "the samples never came");
        std::thread::sleep(Duration::from_millis(100));
    }
    let first = client.request(
        CoapCode::GET,
        "sensors/reading",
        vec![uint(numbers::OBSERVE, 0), uint(numbers::ACCEPT, 50)],
        b"",
    );
    let id = |m: &CoapMessage| serde_json::from_slice::<Value>(&m.payload).unwrap()["id"].clone();
    assert_eq!(id(&first), 2, "the oldest of the last three");
    let mut ids = Vec::new();
    for _ in 0..2 {
        let n = client
            .recv(Duration::from_secs(5))
            .expect("a replayed sample");
        assert!(has(&n, numbers::MAX_AGE));
        if matches!(n.message_type, MessageType::Confirmable) {
            client.answer(&n, MessageType::Acknowledgement);
        }
        ids.push(id(&n));
    }
    assert_eq!(ids, [json!(3), json!(4)]);

    // The deadline of `sensors/late`: no sample for 1.5 s.
    let ended = late_client
        .recv(Duration::from_secs(4))
        .expect("the deadline notification");
    assert_eq!(ended.code, CoapCode::new(5, 3));
    assert_eq!(ended.token, watch.token);
    assert_eq!(option(&ended, numbers::MAX_AGE), Some(0));
    assert!(!has(&ended, numbers::OBSERVE), "it ends the observation");
    let get = late_client.request(CoapCode::GET, "sensors/late", vec![], b"");
    assert_eq!(get.code, CoapCode::new(5, 3));
    assert_eq!(option(&get, numbers::MAX_AGE), Some(0));
    handle.shutdown();
}

/// RFC 7641 §4.5 with `observe_max_age_secs`: a notification of a
/// BEST_EFFORT topic is Non-confirmable until the observer has gone a
/// second without acknowledging one, then Confirmable. Spec §6
/// (`keep_alive_secs`): the daemon pings each observer; one that answers
/// with a Reset keeps its observation, one that does not answer loses it.
#[test]
fn coapbridge_6_the_ping_and_the_bound_of_unconfirmed_notifications() {
    let (mut cfg, lease, _idl) = config("ping");
    cfg.topics[0].reliability = "best_effort".to_string();
    cfg.observe_max_age_secs = 1;
    cfg.keep_alive_secs = 1;
    cfg.ack_timeout_ms = 200;
    cfg.max_retransmit = 1;
    let mut handle = server::start(cfg).expect("daemon start");
    let mut client = Client::new(&handle.local_addr);
    let dds = peer(lease.id(), 6);
    let writer = dds
        .register_user_writer(UserWriterConfig {
            topic_name: "Sensor::Reading".into(),
            type_name: "Sensor::Reading".into(),
            reliable: false,
            ..Default::default()
        })
        .unwrap();
    until(&dds, "the daemon's reader matched", || {
        dds.user_writer_matched_count(writer) >= 1
    });
    let ty = reading();
    let write = |v: f64| {
        dds.write_user_sample(
            writer,
            ty.write(Form::Cdr2Le, &json!({ "id": 1, "value": v, "unit": "V" }))
                .unwrap(),
        )
        .unwrap();
    };
    let watch = client.request(
        CoapCode::GET,
        "sensors/reading",
        vec![uint(numbers::OBSERVE, 0)],
        b"",
    );
    assert_eq!(watch.code, CoapCode::CONTENT);
    // A notification or a ping, whichever comes; a ping is answered with
    // a Reset (RFC 7252 §4.3). The daemon pings every second, so a wait
    // without its own deadline never ends when no notification comes: a
    // hosted runner sat here until the job's six hours were up.
    let next_notification = |client: &Client| {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            // `set_read_timeout` refuses a zero duration.
            let left = deadline
                .saturating_duration_since(Instant::now())
                .max(Duration::from_millis(1));
            let m = client
                .recv(left)
                .expect("a notification within 10 s (pings answered)");
            if m.code == CoapCode::EMPTY {
                client.answer(&m, MessageType::Reset);
                continue;
            }
            return m;
        }
    };
    write(1.0);
    let n = next_notification(&client);
    assert!(matches!(n.message_type, MessageType::NonConfirmable));
    client.idle(Duration::from_millis(1200));
    write(2.0);
    let n = next_notification(&client);
    assert!(
        matches!(n.message_type, MessageType::Confirmable),
        "after a second without an acknowledgement"
    );
    client.answer(&n, MessageType::Acknowledgement);
    // A ping comes within the period; answered, the observation stays.
    let mut pinged = false;
    let until_ping = Instant::now() + Duration::from_secs(3);
    while Instant::now() < until_ping && !pinged {
        if let Some(m) = client.recv(Duration::from_millis(500))
            && m.code == CoapCode::EMPTY
            && matches!(m.message_type, MessageType::Confirmable)
        {
            client.answer(&m, MessageType::Reset);
            pinged = true;
        }
    }
    assert!(pinged, "a ping within the period");
    write(3.0);
    let n = next_notification(&client);
    assert_eq!(n.token, watch.token);
    if matches!(n.message_type, MessageType::Confirmable) {
        client.answer(&n, MessageType::Acknowledgement);
    }
    // Silent from now on: the next ping goes unanswered, the observation
    // ends.
    std::thread::sleep(Duration::from_secs(4));
    while client.recv(Duration::from_millis(100)).is_some() {}
    write(4.0);
    assert!(
        client.recv(Duration::from_millis(1500)).is_none(),
        "no notification after an unanswered ping"
    );
    dds.shutdown();
    handle.shutdown();
}

/// RFC 7959 with spec §4.4: `block_size` 16 cuts a representation into
/// blocks of 16 octets; a Block1 block that does not continue the
/// transfer is 4.08, a transfer beyond `max_body_size` 4.13 with Size1.
#[test]
fn rfc7959_block_size_and_the_limits_of_a_transfer() {
    let (mut cfg, _lease, _idl) = config("blocks");
    cfg.block_size = 16;
    cfg.max_body_size = 64;
    let mut handle = server::start(cfg).expect("daemon start");
    let mut client = Client::new(&handle.local_addr);
    let body = json!({ "id": 1, "value": 1.0, "unit": "a unit with a long name" }).to_string();
    let r = client.request(
        CoapCode::POST,
        "sensors/reading",
        vec![uint(numbers::CONTENT_FORMAT, 50)],
        body.as_bytes(),
    );
    assert_eq!(r.code, CoapCode::CHANGED);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let r = client.request(
            CoapCode::GET,
            "sensors/reading",
            vec![uint(numbers::ACCEPT, 50)],
            b"",
        );
        if has(&r, numbers::BLOCK2) {
            assert_eq!(
                option(&r, numbers::BLOCK2).unwrap() & 0x07,
                0,
                "SZX 0: 16 octets"
            );
            assert_eq!(r.payload.len(), 16);
            assert!(has(&r, numbers::SIZE2));
            break;
        }
        assert!(Instant::now() < deadline, "the sample never came");
        std::thread::sleep(Duration::from_millis(100));
    }
    let block = |num: u64, more: bool| uint(numbers::BLOCK1, (num << 4) | (u64::from(more) << 3));
    let r = client.request(
        CoapCode::POST,
        "sensors/reading",
        vec![block(0, true)],
        &[0u8; 16],
    );
    assert_eq!(r.code, CoapCode::new(2, 31));
    let r = client.request(
        CoapCode::POST,
        "sensors/reading",
        vec![block(2, false)],
        &[0u8; 16],
    );
    assert_eq!(r.code, CoapCode::new(4, 8));
    let mut last = CoapMessage::new(MessageType::Acknowledgement, CoapCode::EMPTY, 0);
    for num in 0..5 {
        last = client.request(
            CoapCode::POST,
            "sensors/reading",
            vec![block(num, true)],
            &[0u8; 16],
        );
        if last.code != CoapCode::new(2, 31) {
            break;
        }
    }
    assert_eq!(last.code, CoapCode::new(4, 13));
    assert_eq!(option(&last, numbers::SIZE1), Some(64));
    handle.shutdown();
}

/// The daemon's process (spec §12.2): killed and reaped when the test ends.
struct Reaped(std::process::Child);

impl Drop for Reaped {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Spec §8.1 and §8.3 through the binary: every exchange is a JSON line on
/// stdout with the fields of §8.1 (at `log_level: debug`), and with
/// `OTEL_EXPORTER_OTLP_ENDPOINT` a span `coap.request` with its child
/// `dds.write` reaches the collector, at the latest at the shutdown.
#[cfg(unix)]
#[test]
fn coapbridge_8_1_8_3_the_log_lines_and_the_spans_of_an_exchange() {
    use std::io::{BufRead, Read};
    use std::process::{Command, Stdio};
    let lease = zerodds_test_domains::lease_domain();
    let idl = idl_file("otlp");
    // A collector: every request it gets, as text.
    let collector = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = collector.local_addr().unwrap().port();
    let received = Arc::new(std::sync::Mutex::new(String::new()));
    {
        let received = Arc::clone(&received);
        std::thread::spawn(move || {
            for stream in collector.incoming().flatten() {
                let mut stream = stream;
                stream
                    .set_read_timeout(Some(Duration::from_millis(500)))
                    .unwrap();
                let mut text = Vec::new();
                let _ = stream.read_to_end(&mut text);
                use std::io::Write;
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
                received
                    .lock()
                    .unwrap()
                    .push_str(&String::from_utf8_lossy(&text));
            }
        });
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("topic-e2e-otlp");
    let file = dir.join("coap-bridged.yaml");
    std::fs::write(
        &file,
        format!(
            "domain: {}\nlog_level: debug\ncoap:\n  bind: \"127.0.0.1:0\"\ntopics:\n  - dds_name: \"Sensor::Reading\"\n    coap_uri_path: \"sensors/reading\"\n    idl_file: \"{}\"\n",
            lease.id(),
            idl.display()
        ),
    )
    .unwrap();
    let mut child = Reaped(
        Command::new(env!("CARGO_BIN_EXE_zerodds-coap-bridged"))
            .arg("--config")
            .arg(&file)
            .env(
                "OTEL_EXPORTER_OTLP_ENDPOINT",
                format!("http://127.0.0.1:{port}"),
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn"),
    );
    let mut lines = std::io::BufReader::new(child.0.stdout.take().unwrap()).lines();
    let mut bound = None;
    for line in lines.by_ref().map_while(Result::ok) {
        let v: Value = serde_json::from_str(&line).expect("a JSON line");
        assert!(
            v["timestamp"].as_str().is_some_and(|t| t.ends_with('Z')),
            "{line}"
        );
        if v["event"] == "bound" {
            bound = v["peer"].as_str().map(ToString::to_string);
        }
        if v["event"] == "running" {
            break;
        }
    }
    let bound = bound.expect("the bound address in the log");
    let (tx, rx) = std::sync::mpsc::channel::<Value>();
    std::thread::spawn(move || {
        for line in lines.map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                let _ = tx.send(v);
            }
        }
    });
    let mut client = Client::new(&bound);
    let body = json!({ "id": 3, "value": 1.0, "unit": "V" }).to_string();
    let resp = client.request(
        CoapCode::POST,
        "sensors/reading",
        vec![uint(numbers::CONTENT_FORMAT, 50)],
        body.as_bytes(),
    );
    assert_eq!(resp.code, CoapCode::CHANGED);
    let line = loop {
        let v = rx.recv_timeout(Duration::from_secs(5)).expect("a log line");
        if v["event"] == "request" {
            break v;
        }
    };
    assert_eq!(line["level"], "debug");
    assert_eq!(line["method"], "POST");
    assert_eq!(line["code"], "2.04");
    assert_eq!(line["coap_uri"], "sensors/reading");
    assert_eq!(line["dds_topic"], "Sensor::Reading");
    assert_eq!(line["mid"], u64::from(resp.message_id));
    assert_eq!(line["token_hex"], hex(&resp.token));
    assert!(line["bytes"].as_u64().is_some_and(|b| b > 0));
    assert!(line["latency_us"].as_u64().is_some());
    assert!(line["peer"].as_str().is_some());
    let killed = Command::new("kill")
        .args(["-TERM", &child.0.id().to_string()])
        .status()
        .unwrap();
    assert!(killed.success());
    let deadline = Instant::now() + Duration::from_secs(10);
    while child.0.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline, "still running after SIGTERM");
        std::thread::sleep(Duration::from_millis(50));
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let text = received.lock().unwrap().clone();
        if text.contains("coap.request") && text.contains("dds.write") {
            assert!(text.contains("/v1/traces"), "{text}");
            assert!(text.contains("Sensor::Reading"), "{text}");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "no span reached the collector:\n{text}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Spec §12.2: a sample of 4 KiB goes in by Block1 in blocks of 1024
/// octets (RFC 7959 §2.5) and comes back by Block2, the same sample.
#[test]
fn coapbridge_12_2_a_4_kib_sample_by_block1_and_back_by_block2() {
    let (cfg, _lease, _idl) = config("4kib");
    let mut handle = server::start(cfg).expect("daemon start");
    let mut client = Client::new(&handle.local_addr);
    let value = json!({ "id": 4, "value": 4.0, "unit": "x".repeat(4096) });
    let body = value.to_string().into_bytes();
    assert!(body.len() > 4096);
    let blocks: Vec<&[u8]> = body.chunks(1024).collect();
    for (num, chunk) in blocks.iter().enumerate() {
        let more = num + 1 < blocks.len();
        let r = client.request(
            CoapCode::POST,
            "sensors/reading",
            vec![
                uint(numbers::CONTENT_FORMAT, 50),
                uint(
                    numbers::BLOCK1,
                    ((num as u64) << 4) | (u64::from(more) << 3) | 6,
                ),
            ],
            chunk,
        );
        let want = if more {
            CoapCode::new(2, 31)
        } else {
            CoapCode::CHANGED
        };
        assert_eq!(r.code, want, "block {num}");
    }
    // Block2 of the representation, the block size the request allows.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let first = client.request(
            CoapCode::GET,
            "sensors/reading",
            vec![uint(numbers::ACCEPT, 50), uint(numbers::BLOCK2, 6)],
            b"",
        );
        if has(&first, numbers::BLOCK2) {
            let mut got = first.payload.clone();
            let szx = option(&first, numbers::BLOCK2).unwrap() & 0x07;
            let mut num = 1u64;
            let mut more = option(&first, numbers::BLOCK2).unwrap() & 0x08 != 0;
            while more {
                let next = client.request(
                    CoapCode::GET,
                    "sensors/reading",
                    vec![
                        uint(numbers::ACCEPT, 50),
                        uint(numbers::BLOCK2, (num << 4) | szx),
                    ],
                    b"",
                );
                got.extend_from_slice(&next.payload);
                more = option(&next, numbers::BLOCK2).unwrap() & 0x08 != 0;
                num += 1;
            }
            assert_eq!(serde_json::from_slice::<Value>(&got).unwrap(), value);
            break;
        }
        assert!(Instant::now() < deadline, "the sample never came");
        std::thread::sleep(Duration::from_millis(100));
    }
    handle.shutdown();
}

/// Spec §1 L6: a tenant is a DomainParticipant of its own with its topics;
/// a DTLS session whose subject the tenant names is bound to it — its POST
/// reaches the tenant's domain, its catalog lists the tenant's topics. A
/// plaintext request and a session no tenant names see the topics of
/// `domain`; the tenant's resource is 4.04 to them.
#[cfg(feature = "dtls")]
#[test]
fn coapbridge_1_l6_a_dtls_session_is_bound_to_the_tenant_of_its_subject() {
    use zerodds_coap_bridge::daemon::config::TenantConfig;
    use zerodds_coap_bridge::dtls_transport::DtlsCoapClient;
    let (mut cfg, _lease, idl) = config("tenant");
    let tenant_lease = zerodds_test_domains::lease_domain();
    cfg.dtls_enabled = true;
    cfg.bind_dtls = Some("127.0.0.1:0".to_string());
    cfg.dtls.psk.add("plant-a-gw", vec![5u8; 16]).unwrap();
    cfg.dtls.psk.add("other", vec![6u8; 16]).unwrap();
    let mut tenant_topic = topic("Plant::Reading", "plant/reading", "bidir", Some(&idl));
    tenant_topic.dds_type = "Sensor::Reading".to_string();
    cfg.tenants.push(TenantConfig {
        name: "plant-a".to_string(),
        domain: tenant_lease.id(),
        subjects: vec!["psk:plant-a-gw".to_string()],
        topics: vec![tenant_topic],
    });
    let mut handle = server::start(cfg).expect("daemon start");
    let dtls_addr = handle.dtls_local_addr().expect("dtls listener");
    // A DDS reader on the tenant's domain.
    let dds = peer(tenant_lease.id(), 7);
    let (_reader, rx) = dds
        .register_user_reader(UserReaderConfig {
            topic_name: "Plant::Reading".into(),
            type_name: "Sensor::Reading".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    let mut client = Client::new(&handle.local_addr);
    let mut mid = 0x6000u16;
    let mut ask = |session: &mut zerodds_coap_bridge::dtls_transport::DtlsCoapSession,
                   code: CoapCode,
                   path: &str,
                   options: Vec<CoapOption>,
                   payload: &[u8]| {
        mid += 1;
        let req = client.message(MessageType::Confirmable, code, path, options, payload);
        let mut req = req;
        req.message_id = mid;
        session.send_message(&req).unwrap();
        session
            .recv_message(Duration::from_secs(5))
            .unwrap()
            .expect("a response")
    };
    let value = json!({ "id": 11, "value": 2.0, "unit": "bar" });
    let mut tenant = DtlsCoapClient::connect_psk(
        dtls_addr,
        "plant-a-gw",
        vec![5u8; 16],
        Duration::from_secs(5),
    )
    .expect("the tenant's session");
    let (_, (payload, identifier, _)) = post_until_received(&rx, || {
        let r = ask(
            &mut tenant,
            CoapCode::POST,
            "plant/reading",
            vec![uint(numbers::CONTENT_FORMAT, 50)],
            value.to_string().as_bytes(),
        );
        assert_eq!(r.code, CoapCode::CHANGED);
    });
    assert_eq!(reading().read_body(&payload, identifier).unwrap(), value);
    let catalog = ask(&mut tenant, CoapCode::GET, ".well-known/core", vec![], b"");
    let text = String::from_utf8(catalog.payload).unwrap();
    assert!(text.contains("</plant/reading>"), "{text}");
    assert!(!text.contains("</sensors/reading>"), "{text}");
    assert_eq!(
        ask(&mut tenant, CoapCode::GET, "sensors/reading", vec![], b"").code,
        CoapCode::new(4, 4)
    );
    // A session no tenant names, and plaintext: the topics of `domain`.
    let mut other =
        DtlsCoapClient::connect_psk(dtls_addr, "other", vec![6u8; 16], Duration::from_secs(5))
            .expect("another session");
    assert_eq!(
        ask(&mut other, CoapCode::GET, "plant/reading", vec![], b"").code,
        CoapCode::new(4, 4)
    );
    assert_eq!(
        ask(&mut other, CoapCode::GET, "sensors/reading", vec![], b"").code,
        CoapCode::CONTENT
    );
    let mut plain = Client::new(&handle.local_addr);
    assert_eq!(
        plain
            .request(CoapCode::GET, "plant/reading", vec![], b"")
            .code,
        CoapCode::new(4, 4)
    );
    tenant.close();
    other.close();
    dds.shutdown();
    handle.shutdown();
}
