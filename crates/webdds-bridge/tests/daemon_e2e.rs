// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! End-to-end test for `zerodds-webdds-bridged`.
//!
//! Strategy, matching `crates/websocket-bridge/tests/daemon_e2e.rs`:
//! the daemon runs in-process (`daemon::server::start`), driven over a
//! real `TcpStream` with hand-written HTTP/1.1 text, on a real leased
//! DDS domain. The write path is verified by a **second, independent**
//! `DomainParticipant` — not the gateway's own `Gateway::read_samples`
//! — that resolves the same topic type from the same IDL file through
//! the public `zerodds-dcps` typed API
//! (`SerializedTypeSupport`/`Topic`/`DataReader`) and receives the
//! sample over real RTPS/UDP: SEDP discovery, wire encode by the
//! daemon, wire decode by this test. That is what actually exercises
//! the daemon's write path end to end, rather than only its own
//! in-process bookkeeping.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use zerodds_dcps::dds_type::DdsType;
use zerodds_dcps::serialized_type::SerializedTypeSupport;
use zerodds_dcps::{DataReaderQos, DomainParticipantFactory, DomainParticipantQos, SubscriberQos};
use zerodds_webdds_bridge::daemon::config::{DaemonConfig, TypeEntry};
use zerodds_webdds_bridge::daemon::server;

fn write_idl(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("reading.idl");
    std::fs::write(
        &path,
        "module Demo { @final struct Reading { @key long sensor_id; double value; }; };",
    )
    .expect("write idl");
    path
}

fn http_request(addr: &str, raw: &str) -> String {
    let mut s = TcpStream::connect(addr).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(5))).ok();
    s.write_all(raw.as_bytes()).expect("write request");
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    // One response is enough for these tests: read until the peer either
    // closes (Connection: close) or the buffer plausibly holds a full
    // small response.
    loop {
        match s.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                out.extend_from_slice(&buf[..n]);
                if out.windows(4).any(|w| w == b"\r\n\r\n") && raw.contains("Connection: close") {
                    // headers-only response (e.g. 204/201) with a `close`
                    // request: one more read attempt then stop.
                }
            }
            Err(_) => break,
        }
        if out.len() > 64 && !raw.contains("Connection: close") {
            break;
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

fn location_of(response: &str) -> String {
    response
        .lines()
        .find_map(|l| l.strip_prefix("Location: "))
        .expect("Location header")
        .trim()
        .to_string()
}

fn last_segment(path: &str) -> String {
    path.rsplit('/').next().expect("segment").to_string()
}

#[test]
fn write_over_http_arrives_at_an_independent_participant() {
    let dir = std::env::temp_dir().join(format!("zd-webdds-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let idl_path = write_idl(&dir);

    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id();

    let mut cfg = DaemonConfig::default_for_dev();
    cfg.listen = "127.0.0.1:0".to_string();
    cfg.types.push(TypeEntry {
        type_name: "Demo::Reading".to_string(),
        idl_file: Some(idl_path.to_string_lossy().to_string()),
        xml_file: None,
    });
    let mut handle = server::start(cfg).expect("start daemon");
    let addr = handle.local_addr.clone();

    // 1. Create application.
    let body = "<application><name>app1</name></application>";
    let req = format!(
        "POST /dds/rest1/applications HTTP/1.1\r\nAccept: application/dds-web+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let resp = http_request(&addr, &req);
    assert!(resp.starts_with("HTTP/1.1 201"), "{resp}");

    // 2. Create participant on the leased domain.
    let body = format!("<domain_participant><domain_id>{domain}</domain_id></domain_participant>");
    let req = format!(
        "POST /dds/rest1/applications/app1/domain_participants HTTP/1.1\r\nAccept: application/dds-web+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let resp = http_request(&addr, &req);
    assert!(resp.starts_with("HTTP/1.1 201"), "{resp}");
    let participant = last_segment(&location_of(&resp));

    // 3. Create topic.
    let body = "<topic><name>Reading</name><type_name>Demo::Reading</type_name></topic>";
    let req = format!(
        "POST /dds/rest1/applications/app1/domain_participants/{participant}/topics HTTP/1.1\r\nAccept: application/dds-web+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let resp = http_request(&addr, &req);
    assert!(resp.starts_with("HTTP/1.1 201"), "{resp}");

    // 4. Create publisher.
    let req = format!(
        "POST /dds/rest1/applications/app1/domain_participants/{participant}/publishers HTTP/1.1\r\nAccept: application/dds-web+xml\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    let resp = http_request(&addr, &req);
    assert!(resp.starts_with("HTTP/1.1 201"), "{resp}");
    let publisher = last_segment(&location_of(&resp));

    // 5. Create data writer.
    let body = "<data_writer><topic_name>Reading</topic_name></data_writer>";
    let req = format!(
        "POST /dds/rest1/applications/app1/domain_participants/{participant}/publishers/{publisher}/data_writers HTTP/1.1\r\nAccept: application/dds-web+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let resp = http_request(&addr, &req);
    assert!(resp.starts_with("HTTP/1.1 201"), "{resp}");
    let writer = last_segment(&location_of(&resp));

    // The independent verification participant, on the same leased
    // domain, resolving the same IDL through the public typed API.
    let factory = DomainParticipantFactory::instance();
    let participant2 = factory
        .create_participant(domain, DomainParticipantQos::default())
        .expect("second participant");
    let registry = zerodds_webdds_bridge::types_registry::TypeRegistry::new();
    let resolved = registry
        .load_idl_file(&idl_path, "Demo::Reading")
        .expect("resolve type");
    let support: std::sync::Arc<SerializedTypeSupport> = resolved.support;
    let topic2 = participant2
        .create_serialized_topic("Reading", zerodds_dcps::TopicQos::default(), support)
        .expect("topic2");
    let subscriber2 = participant2.create_subscriber(SubscriberQos::default());
    let reader2 = subscriber2
        .create_datareader(&topic2, DataReaderQos::default())
        .expect("reader2");

    // 6. Write the sample over HTTP.
    let body = "<data><sensor_id>7</sensor_id><value>3.5</value></data>";
    let req = format!(
        "POST /dds/rest1/applications/app1/domain_participants/{participant}/publishers/{publisher}/data_writers/{writer} HTTP/1.1\r\nAccept: application/dds-web+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let resp = http_request(&addr, &req);
    assert!(resp.starts_with("HTTP/1.1 200"), "{resp}");

    // The independent participant receives it over real RTPS.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut samples = Vec::new();
    while samples.is_empty() && std::time::Instant::now() < deadline {
        samples = reader2.read().expect("read");
        if samples.is_empty() {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    assert_eq!(samples.len(), 1, "expected exactly one sample to arrive");
    let sample = &samples[0];
    assert_eq!(
        sample.field_value("sensor_id"),
        Some(zerodds_dcps::FilterValue::Int(7))
    );
    assert_eq!(
        sample.field_value("value"),
        Some(zerodds_dcps::FilterValue::Float(3.5))
    );

    handle.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn wsdl_endpoint_lists_object_model_operations() {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.listen = "127.0.0.1:0".to_string();
    let mut handle = server::start(cfg).expect("start daemon");
    let addr = handle.local_addr.clone();
    let resp = http_request(&addr, "GET /wsdl HTTP/1.1\r\nConnection: close\r\n\r\n");
    assert!(resp.contains("<definitions"), "{resp}");
    assert!(resp.contains("CreateApplication"));
    assert!(resp.contains("WriteSample"));
    handle.shutdown();
}

/// `PUT` on a `DataWriter` changes its live QoS over real HTTP — Spec
/// §7.4.5.2 `update_datawriter`. DEADLINE is Changeable=YES post-enable
/// (unlike DURABILITY/RELIABILITY/HISTORY/RESOURCE_LIMITS/OWNERSHIP/
/// LIVELINESS, which `DataWriter::set_qos` correctly refuses to change
/// once the writer is enabled — this daemon enables every entity at
/// creation, per Spec §7.4.4.7/§7.4.5.1).
#[test]
fn update_data_writer_over_http_changes_its_qos() {
    let dir = std::env::temp_dir().join(format!("zd-webdds-e2e-update-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let idl_path = write_idl(&dir);

    let mut cfg = DaemonConfig::default_for_dev();
    cfg.listen = "127.0.0.1:0".to_string();
    cfg.types.push(TypeEntry {
        type_name: "Demo::Reading".to_string(),
        idl_file: Some(idl_path.to_string_lossy().to_string()),
        xml_file: None,
    });
    let mut handle = server::start(cfg).expect("start daemon");
    let addr = handle.local_addr.clone();

    let body = "<application><name>app1</name></application>";
    let req = format!(
        "POST /dds/rest1/applications HTTP/1.1\r\nAccept: application/dds-web+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    assert!(http_request(&addr, &req).starts_with("HTTP/1.1 201"));

    let body = "<domain_participant><domain_id>0</domain_id></domain_participant>";
    let req = format!(
        "POST /dds/rest1/applications/app1/domain_participants HTTP/1.1\r\nAccept: application/dds-web+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let resp = http_request(&addr, &req);
    assert!(resp.starts_with("HTTP/1.1 201"));
    let participant = last_segment(&location_of(&resp));

    let body = "<topic><name>Reading</name><type_name>Demo::Reading</type_name></topic>";
    let req = format!(
        "POST /dds/rest1/applications/app1/domain_participants/{participant}/topics HTTP/1.1\r\nAccept: application/dds-web+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    assert!(http_request(&addr, &req).starts_with("HTTP/1.1 201"));

    let req = "POST /dds/rest1/applications/app1/domain_participants/p1/publishers HTTP/1.1\r\nAccept: application/dds-web+xml\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string();
    let resp = http_request(&addr, &req);
    assert!(resp.starts_with("HTTP/1.1 201"));
    let publisher = last_segment(&location_of(&resp));

    let body = "<data_writer><topic_name>Reading</topic_name></data_writer>";
    let req = format!(
        "POST /dds/rest1/applications/app1/domain_participants/{participant}/publishers/{publisher}/data_writers HTTP/1.1\r\nAccept: application/dds-web+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let resp = http_request(&addr, &req);
    assert!(resp.starts_with("HTTP/1.1 201"));
    let writer = last_segment(&location_of(&resp));

    let body = "<data_writer><qos><deadline><period><sec>2</sec><nanosec>0</nanosec></period></deadline></qos></data_writer>";
    let req = format!(
        "PUT /dds/rest1/applications/app1/domain_participants/{participant}/publishers/{publisher}/data_writers/{writer} HTTP/1.1\r\nAccept: application/dds-web+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let resp = http_request(&addr, &req);
    assert!(resp.starts_with("HTTP/1.1 204"), "{resp}");

    handle.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn soap_create_application_round_trips() {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.listen = "127.0.0.1:0".to_string();
    let mut handle = server::start(cfg).expect("start daemon");
    let addr = handle.local_addr.clone();
    let body = "<soap:Envelope xmlns:soap=\"http://www.w3.org/2003/05/soap-envelope\"><soap:Body><CreateApplication><name>app1</name></CreateApplication></soap:Body></soap:Envelope>";
    let req = format!(
        "POST /soap HTTP/1.1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let resp = http_request(&addr, &req);
    assert!(resp.contains("CreateApplicationResponse"), "{resp}");
    assert!(resp.contains("applications/app1"), "{resp}");
    handle.shutdown();
}

/// One SOAP operation; the created resource's last path segment, if the
/// reply names one.
fn soap(addr: &str, operation: &str) -> (String, Option<String>) {
    let body = format!(
        "<soap:Envelope xmlns:soap=\"http://www.w3.org/2003/05/soap-envelope\"><soap:Body>{operation}</soap:Body></soap:Envelope>"
    );
    let req = format!(
        "POST /soap HTTP/1.1\r\nContent-Type: application/soap+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let resp = http_request(addr, &req);
    let created = resp
        .split("<path>")
        .nth(1)
        .and_then(|rest| rest.split("</path>").next())
        .map(last_segment);
    (resp, created)
}

/// A `WriteSample` whose `<data>` holds the sample as elements, the REST
/// representation — measured on the Linux test host before the fix: `env:Receiver`
/// "malformed XML body: no root element", because the field reader took
/// `<data>`'s children for a list and the sample arrived empty.
#[test]
fn soap_write_sample_arrives_at_an_independent_participant() {
    let dir = std::env::temp_dir().join(format!("zd-webdds-soap-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let idl_path = write_idl(&dir);
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id();

    let mut cfg = DaemonConfig::default_for_dev();
    cfg.listen = "127.0.0.1:0".to_string();
    cfg.types.push(TypeEntry {
        type_name: "Demo::Reading".to_string(),
        idl_file: Some(idl_path.to_string_lossy().to_string()),
        xml_file: None,
    });
    let mut handle = server::start(cfg).expect("start daemon");
    let addr = handle.local_addr.clone();

    let (resp, _) = soap(
        &addr,
        "<CreateApplication><name>app1</name></CreateApplication>",
    );
    assert!(resp.contains("CreateApplicationResponse"), "{resp}");
    let (resp, participant) = soap(
        &addr,
        &format!(
            "<CreateParticipant><app>app1</app><domain_id>{domain}</domain_id></CreateParticipant>"
        ),
    );
    let participant = participant.unwrap_or_else(|| panic!("{resp}"));
    let (resp, _) = soap(
        &addr,
        &format!(
            "<CreateTopic><app>app1</app><participant>{participant}</participant><name>Reading</name><type_name>Demo::Reading</type_name></CreateTopic>"
        ),
    );
    assert!(resp.contains("CreateTopicResponse"), "{resp}");
    let (resp, publisher) = soap(
        &addr,
        &format!(
            "<CreatePublisher><app>app1</app><participant>{participant}</participant></CreatePublisher>"
        ),
    );
    let publisher = publisher.unwrap_or_else(|| panic!("{resp}"));
    let (resp, writer) = soap(
        &addr,
        &format!(
            "<CreateDataWriter><app>app1</app><participant>{participant}</participant><publisher>{publisher}</publisher><topic_name>Reading</topic_name></CreateDataWriter>"
        ),
    );
    let writer = writer.unwrap_or_else(|| panic!("{resp}"));

    let participant2 = DomainParticipantFactory::instance()
        .create_participant(domain, DomainParticipantQos::default())
        .expect("second participant");
    let resolved = zerodds_webdds_bridge::types_registry::TypeRegistry::new()
        .load_idl_file(&idl_path, "Demo::Reading")
        .expect("resolve type");
    let topic2 = participant2
        .create_serialized_topic(
            "Reading",
            zerodds_dcps::TopicQos::default(),
            resolved.support,
        )
        .expect("topic2");
    let reader2 = participant2
        .create_subscriber(SubscriberQos::default())
        .create_datareader(&topic2, DataReaderQos::default())
        .expect("reader2");

    let (resp, _) = soap(
        &addr,
        &format!(
            "<WriteSample><app>app1</app><participant>{participant}</participant><publisher>{publisher}</publisher><writer>{writer}</writer><data><sensor_id>6</sensor_id><value>6.5</value></data></WriteSample>"
        ),
    );
    assert!(resp.contains("WriteSampleResponse"), "{resp}");

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut samples = Vec::new();
    while samples.is_empty() && std::time::Instant::now() < deadline {
        samples = reader2.read().expect("read");
        if samples.is_empty() {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    assert_eq!(samples.len(), 1, "expected exactly one sample to arrive");
    assert_eq!(
        samples[0].field_value("sensor_id"),
        Some(zerodds_dcps::FilterValue::Int(6))
    );
    assert_eq!(
        samples[0].field_value("value"),
        Some(zerodds_dcps::FilterValue::Float(6.5))
    );

    handle.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

/// A waitset long-poll held the gateway, and every other request waited
/// for it: measured on the Linux test host, a list of applications took 3.7 s during a
/// 4 s long-poll. A timeout the gateway cannot hold (`-1`) ended the
/// connection's thread without a response.
#[test]
fn a_long_poll_leaves_other_requests_alone_and_a_bad_timeout_is_refused() {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.listen = "127.0.0.1:0".to_string();
    let mut handle = server::start(cfg).expect("start daemon");
    let addr = handle.local_addr.clone();
    let post = |path: &str, body: &str| {
        http_request(
            &addr,
            &format!(
                "POST /dds/rest1/{path} HTTP/1.1\r\nAccept: application/dds-web+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ),
        )
    };
    let get = |addr: &str, path: &str| {
        http_request(
            addr,
            &format!(
                "GET /dds/rest1/{path} HTTP/1.1\r\nAccept: application/dds-web+xml\r\nConnection: close\r\n\r\n"
            ),
        )
    };
    let resp = post(
        "applications",
        "<application><name>app1</name></application>",
    );
    assert!(resp.starts_with("HTTP/1.1 201"), "{resp}");
    let resp = post(
        "applications/app1/waitsets",
        "<waitset><name>ws1</name></waitset>",
    );
    assert!(resp.starts_with("HTTP/1.1 201"), "{resp}");

    let poll_addr = addr.clone();
    let poll = std::thread::spawn(move || {
        let started = std::time::Instant::now();
        let resp = get(&poll_addr, "applications/app1/waitsets/ws1?timeout=2");
        (resp, started.elapsed())
    });
    std::thread::sleep(Duration::from_millis(300));
    let started = std::time::Instant::now();
    let resp = get(&addr, "applications");
    let listed_in = started.elapsed();
    assert!(resp.starts_with("HTTP/1.1 200"), "{resp}");
    assert!(
        listed_in < Duration::from_millis(1000),
        "the list waited {listed_in:?} for the long-poll"
    );
    let (resp, polled_in) = poll.join().expect("poll thread");
    assert!(resp.starts_with("HTTP/1.1 200"), "{resp}");
    assert!(polled_in >= Duration::from_millis(1900), "{polled_in:?}");

    for bad in ["-1", "nan", "abc"] {
        let resp = get(
            &addr,
            &format!("applications/app1/waitsets/ws1?timeout={bad}"),
        );
        assert!(resp.starts_with("HTTP/1.1 422"), "timeout={bad}: {resp}");
    }
    let resp = post(
        "applications/app1/domain_participants",
        "<domain_participant><domain_id>abc</domain_id></domain_participant>",
    );
    assert!(resp.starts_with("HTTP/1.1 422"), "{resp}");
    handle.shutdown();
}
