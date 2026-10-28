// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The bridge daemon against RabbitMQ 4.0 (`zerodds-amqp-bridge-daemon-1.0`
//! §12.2): a DDS participant in this process on one side, RabbitMQ on the
//! other, and `pika` (AMQP 0.9.1) as the reference client that reads and
//! writes the queues without any ZeroDDS code.
//!
//! * DDS → AMQP: samples written in DDS arrive in the queue as §4.3 messages:
//!   the Pass-Through body with its encapsulation header, the content type.
//! * AMQP → DDS: a message published by `pika` reaches a DDS reader.
//! * Gegenprobe: a body that is no DDS sample is rejected, and no sample
//!   reaches DDS.
//!
//! Needs RabbitMQ on 127.0.0.1:5672 with the user `zerodds`/`zerodds`
//! (Linux test host) and python3 with `pika`. A missing piece fails the test.
//!
//! ```bash
//! cargo test -p zerodds-amqp-endpoint --test bridge_rabbitmq_e2e -- --ignored --test-threads=1
//! ```

#![cfg(feature = "std")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use zerodds_amqp_endpoint::bridge_daemon::{self, DaemonConfig};
use zerodds_amqp_endpoint::daemon_runtime::BridgeMetrics;
use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_monitor::Registry;
use zerodds_rtps::wire_types::GuidPrefix;

thread_local! {
    /// The domain of the running test, leased host-wide and held until the
    /// test's thread ends (libtest runs every test on a thread of its own).
    /// One domain per process let the tests of this file, run in parallel,
    /// see each other's writers.
    static DOMAIN: zerodds_test_domains::DomainLease = zerodds_test_vendor_reader::lease_domain();
}

fn domain() -> i32 {
    DOMAIN.with(zerodds_test_domains::DomainLease::id)
}

fn pika(script: &str) -> String {
    let out = Command::new("python3")
        .arg("-c")
        .arg(format!(
            "import pika, sys\n\
             c = pika.BlockingConnection(pika.ConnectionParameters('localhost', credentials=pika.PlainCredentials('zerodds','zerodds')))\n\
             ch = c.channel()\n{script}\nc.close()\n"
        ))
        .output()
        .expect("python3");
    assert!(
        out.status.success(),
        "pika failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn fresh_queue(q: &str) {
    pika(&format!(
        "ch.queue_declare(queue='{q}', durable=True)\nch.queue_purge(queue='{q}')"
    ));
}

fn config(yaml: &str) -> DaemonConfig {
    let mut c = zerodds_config::Config::default();
    c.push(
        zerodds_config::parse_str(yaml, "<test>").unwrap(),
        "<test>".into(),
    )
    .unwrap();
    DaemonConfig::from_config(Some(&c)).unwrap()
}

/// Runs the daemon in a thread until the returned flag is set.
fn start_bridge(cfg: DaemonConfig) -> (Arc<AtomicBool>, std::thread::JoinHandle<()>) {
    let stop = Arc::new(AtomicBool::new(false));
    let s = Arc::clone(&stop);
    let join = std::thread::spawn(move || {
        let registry = Registry::new();
        let metrics = BridgeMetrics::register(&registry);
        let r = bridge_daemon::run(
            &cfg,
            RuntimeConfig::default(),
            &s,
            &metrics,
            false,
            Duration::from_secs(5),
        );
        assert!(r.is_ok(), "bridge stopped: {r:?}");
    });
    (stop, join)
}

fn participant() -> Arc<DcpsRuntime> {
    // A prefix of its own per participant: two participants with one
    // prefix give their first writers one GUID, and a reader takes DATA
    // from either as from the writer it matched.
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let mut prefix = [0u8; 12];
    prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
    prefix[4..8].copy_from_slice(&NEXT.fetch_add(1, Ordering::Relaxed).to_be_bytes());
    prefix[11] = 0x5a;
    DcpsRuntime::start(
        domain(),
        GuidPrefix::from_bytes(prefix),
        RuntimeConfig::default(),
    )
    .unwrap()
}

fn writer_cfg(topic: &str) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic.into(),
        type_name: format!("{topic}Type"),
        ..Default::default()
    }
}

fn bridge_yaml(topic: &str, queue: &str, direction: &str) -> String {
    format!(
        "domain: {d}\namqp:\n  broker_url: \"amqp://127.0.0.1:5672\"\n  container_id: \"zd-e2e-{topic}-{pid}\"\n  sasl:\n    mechanism: PLAIN\n    username: zerodds\n    password: zerodds\ntopics:\n  - dds_name: {topic}\n    dds_type: {topic}Type\n    amqp_address: /queues/{queue}\n    direction: {direction}\n    qos:\n      history: {{ kind: keep_all }}\n",
        d = domain(),
        pid = std::process::id()
    )
}

#[test]
#[ignore = "needs RabbitMQ 4.0 on 127.0.0.1:5672 and python3-pika (Linux test host)"]
fn dds_samples_arrive_in_the_queue_as_pass_through_messages() {
    let queue = "zd_bridge_out";
    fresh_queue(queue);
    let (stop, join) = start_bridge(config(&bridge_yaml("BridgeOut", queue, "out")));
    let dds = participant();
    let eid = dds
        .register_user_writer_kind(writer_cfg("BridgeOut"), false)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while dds.user_writer_matched_count(eid) == 0 {
        assert!(
            Instant::now() < deadline,
            "the bridge's reader did not match"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    for n in 1u8..=3 {
        dds.write_user_sample(eid, vec![n, n, n, n]).unwrap();
    }
    // pika reads the three messages: body and content type.
    let got = pika(&format!(
        "import time\n\
         out = []\n\
         end = time.time() + 10\n\
         while len(out) < 3 and time.time() < end:\n\
         \x20   m, p, b = ch.basic_get(queue='{queue}', auto_ack=True)\n\
         \x20   if m is None:\n\
         \x20       time.sleep(0.1)\n\
         \x20       continue\n\
         \x20   out.append(p.content_type + ' ' + b.hex())\n\
         print('\\n'.join(out))"
    ));
    stop.store(true, Ordering::SeqCst);
    join.join().unwrap();
    let lines: Vec<&str> = got.lines().collect();
    assert_eq!(lines.len(), 3, "{got}");
    for (i, line) in lines.iter().enumerate() {
        let (ctype, hex) = line.split_once(' ').unwrap();
        let n = format!("{:02x}", i + 1).repeat(4);
        // Header 0x0001 (CDR_LE) or 0x0007 (PLAIN_CDR2_LE), then the payload.
        assert!(hex.ends_with(&n), "{line}");
        match &hex[..4] {
            "0001" => assert_eq!(ctype, "application/vnd.dds.xcdr1", "{line}"),
            "0007" => assert_eq!(ctype, "application/vnd.dds.xcdr2", "{line}"),
            other => panic!("encapsulation {other}: {line}"),
        }
    }
}

#[test]
#[ignore = "needs RabbitMQ 4.0 on 127.0.0.1:5672 and python3-pika (Linux test host)"]
fn a_message_published_by_pika_reaches_a_dds_reader_and_a_bad_body_does_not() {
    let queue = "zd_bridge_in";
    fresh_queue(queue);
    let dds = participant();
    let (_eid, rx) = dds
        .register_user_reader_kind(
            UserReaderConfig {
                topic_name: "BridgeIn".into(),
                type_name: "BridgeInType".into(),
                reliable: true,
                durability: zerodds_qos::DurabilityKind::Volatile,
                ..Default::default()
            },
            false,
        )
        .unwrap();
    let (stop, join) = start_bridge(config(&bridge_yaml("BridgeIn", queue, "in")));
    // Let the bridge's writer match before anything is published.
    std::thread::sleep(Duration::from_secs(2));
    pika(&format!(
        "props = pika.BasicProperties(content_type='application/vnd.dds.xcdr2')\n\
         ch.basic_publish(exchange='', routing_key='{queue}', body=b'\\x00\\x07\\x00\\x00' + b'\\x2a\\x00\\x00\\x00', properties=props)\n\
         ch.basic_publish(exchange='', routing_key='{queue}', body=b'xy', properties=props)"
    ));
    let mut payloads = Vec::new();
    let end = Instant::now() + Duration::from_secs(5);
    while Instant::now() < end {
        if let Ok(UserSample::Alive { payload, .. }) = rx.recv_timeout(Duration::from_millis(200)) {
            payloads.push(payload.as_ref().to_vec());
        }
    }
    // The queue is empty: the good message accepted, the bad one rejected.
    let left = pika(&format!(
        "q = ch.queue_declare(queue='{queue}', durable=True, passive=True)\nprint(q.method.message_count)"
    ));
    stop.store(true, Ordering::SeqCst);
    join.join().unwrap();
    assert_eq!(
        payloads,
        vec![vec![0x2a, 0, 0, 0]],
        "exactly the one DDS sample"
    );
    assert_eq!(left, "0", "both messages settled");
}

fn bridge_yaml_with(topic: &str, queue: &str, direction: &str, extra_amqp: &str) -> String {
    bridge_yaml(topic, queue, direction).replace("  sasl:\n", &format!("{extra_amqp}  sasl:\n"))
}

/// Messages larger than the frame size travel as multi-frame transfers in
/// both directions (AMQP Part 2 §2.6.14): a 20 000-byte sample at a
/// 4096-byte frame size, out to the queue and back in from it.
#[test]
#[ignore = "needs RabbitMQ 4.0 on 127.0.0.1:5672 and python3-pika (Linux test host)"]
fn a_sample_larger_than_the_frame_size_crosses_in_both_directions() {
    let (qout, qin) = ("zd_bridge_big_out", "zd_bridge_big_in");
    fresh_queue(qout);
    fresh_queue(qin);
    let payload: Vec<u8> = (0..20_000u32).map(|i| (i % 251) as u8).collect();
    let dds = participant();
    let (_r, rx) = dds
        .register_user_reader_kind(
            UserReaderConfig {
                topic_name: "BigIn".into(),
                type_name: "BigInType".into(),
                reliable: true,
                durability: zerodds_qos::DurabilityKind::Volatile,
                ..Default::default()
            },
            false,
        )
        .unwrap();
    let w = dds
        .register_user_writer_kind(writer_cfg("BigOut"), false)
        .unwrap();
    let yaml = format!(
        "{}  - dds_name: BigIn\n    dds_type: BigInType\n    amqp_address: /queues/{qin}\n    direction: in\n",
        bridge_yaml_with("BigOut", qout, "out", "  max_frame_size: 4096\n")
    );
    let (stop, join) = start_bridge(config(&yaml));
    let deadline = Instant::now() + Duration::from_secs(10);
    while dds.user_writer_matched_count(w) == 0 {
        assert!(
            Instant::now() < deadline,
            "the bridge's reader did not match"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_secs(1));
    dds.write_user_sample(w, payload.clone()).unwrap();
    // Out: pika sees one message of 4 + 20 000 bytes and puts it into the
    // inbound queue twice: once as it is, carrying this bridge's
    // dds:bridge-id (the loop DDS-AMQP 1.0 §8.4 stops), once with the
    // content type alone.
    let moved = pika(&format!(
        "import time\n\
         end = time.time() + 10\n\
         while time.time() < end:\n\
         \x20   m, p, b = ch.basic_get(queue='{qout}', auto_ack=True)\n\
         \x20   if m is not None:\n\
         \x20       ch.basic_publish(exchange='', routing_key='{qin}', body=b, properties=p)\n\
         \x20       time.sleep(2)\n\
         \x20       bare = pika.BasicProperties(content_type=p.content_type)\n\
         \x20       ch.basic_publish(exchange='', routing_key='{qin}', body=b, properties=bare)\n\
         \x20       print(len(b))\n\
         \x20       break\n\
         \x20   time.sleep(0.1)"
    ));
    assert_eq!(moved, "20004", "header + payload in one message");
    let mut back = Vec::new();
    let end = Instant::now() + Duration::from_secs(6);
    while Instant::now() < end {
        if let Ok(UserSample::Alive { payload, .. }) = rx.recv_timeout(Duration::from_millis(200)) {
            back.push(payload.as_ref().to_vec());
        }
    }
    stop.store(true, Ordering::SeqCst);
    join.join().unwrap();
    assert_eq!(
        back.len(),
        1,
        "the stamped copy is dropped, the bare one written"
    );
    assert_eq!(back[0], payload, "the same 20 000 bytes back in DDS");
}

/// The broker drops the connection; the bridge reconnects, attaches again
/// and delivers what was written meanwhile (spec §9.3).
#[test]
#[ignore = "needs RabbitMQ 4.0 on 127.0.0.1:5672, rabbitmqctl and python3-pika (Linux test host)"]
fn after_the_broker_closes_the_connection_the_bridge_reconnects() {
    let queue = "zd_bridge_reconnect";
    fresh_queue(queue);
    let dds = participant();
    let w = dds
        .register_user_writer_kind(writer_cfg("Reconnect"), false)
        .unwrap();
    let yaml = bridge_yaml_with(
        "Reconnect",
        queue,
        "out",
        "  reconnect:\n    initial_delay_ms: 200\n    max_delay_ms: 1000\n    factor: 2.0\n",
    );
    let (stop, join) = start_bridge(config(&yaml));
    let deadline = Instant::now() + Duration::from_secs(10);
    while dds.user_writer_matched_count(w) == 0 {
        assert!(
            Instant::now() < deadline,
            "the bridge's reader did not match"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_secs(1));
    dds.write_user_sample(w, vec![1, 1, 1, 1]).unwrap();
    std::thread::sleep(Duration::from_millis(500));
    let closed = Command::new("rabbitmqctl")
        .args(["close_all_connections", "zerodds-e2e"])
        .output()
        .expect("rabbitmqctl");
    assert!(
        closed.status.success(),
        "{}",
        String::from_utf8_lossy(&closed.stderr)
    );
    // Written while the connection is down or coming back.
    std::thread::sleep(Duration::from_millis(100));
    dds.write_user_sample(w, vec![2, 2, 2, 2]).unwrap();
    let got = pika(&format!(
        "import time\n\
         out = []\n\
         end = time.time() + 15\n\
         while len(out) < 2 and time.time() < end:\n\
         \x20   m, p, b = ch.basic_get(queue='{queue}', auto_ack=True)\n\
         \x20   if m is None:\n\
         \x20       time.sleep(0.1)\n\
         \x20       continue\n\
         \x20   out.append(b[4:].hex())\n\
         print(' '.join(out))"
    ));
    stop.store(true, Ordering::SeqCst);
    join.join().unwrap();
    assert_eq!(
        got, "01010101 02020202",
        "both samples, in order, after the reconnect"
    );
}

/// A topic whose address the broker refuses does not take the others down
/// (the broker detaches that one link), the catalog goes to its address
/// (§5.2), and the writer's LIFESPAN is the message ttl (§6).
#[test]
#[ignore = "needs RabbitMQ 4.0 on 127.0.0.1:5672 and python3-pika (Linux test host)"]
fn a_refused_link_leaves_the_others_running_and_catalog_and_ttl_arrive() {
    let (queue, catalog) = ("zd_bridge_ok", "zd_bridge_catalog");
    fresh_queue(queue);
    fresh_queue(catalog);
    let dds = participant();
    let mut wcfg = writer_cfg("Alive");
    wcfg.lifespan = zerodds_qos::LifespanQosPolicy {
        duration: zerodds_qos::Duration::from_millis(60_000),
    };
    let w = dds.register_user_writer_kind(wcfg, false).unwrap();
    let yaml = format!(
        "{}  - dds_name: Missing\n    amqp_address: /queues/zd_bridge_does_not_exist\n    direction: out\ncatalog:\n  address: /queues/{catalog}\n",
        bridge_yaml("Alive", queue, "out")
    );
    let (stop, join) = start_bridge(config(&yaml));
    let deadline = Instant::now() + Duration::from_secs(10);
    while dds.user_writer_matched_count(w) == 0 {
        assert!(
            Instant::now() < deadline,
            "the bridge's reader did not match"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_secs(1));
    dds.write_user_sample(w, vec![7, 7, 7, 7]).unwrap();
    let got = pika(&format!(
        "import time\n\
         out = {{}}\n\
         end = time.time() + 10\n\
         while len(out) < 2 and time.time() < end:\n\
         \x20   for q in ('{queue}', '{catalog}'):\n\
         \x20       if q in out:\n\
         \x20           continue\n\
         \x20       m, p, b = ch.basic_get(queue=q, auto_ack=True)\n\
         \x20       if m is not None:\n\
         \x20           out[q] = (p.expiration, p.content_type, b)\n\
         \x20   time.sleep(0.1)\n\
         e, ct, b = out['{queue}']\n\
         print(e)\n\
         print(b[4:].hex())\n\
         e, ct, b = out['{catalog}']\n\
         print(ct)\n\
         print(b.decode())"
    ));
    stop.store(true, Ordering::SeqCst);
    join.join().unwrap();
    let lines: Vec<&str> = got.lines().collect();
    assert_eq!(lines[0], "60000", "LIFESPAN as the ttl: {got}");
    assert_eq!(lines[1], "07070707", "{got}");
    assert_eq!(lines[2], "application/json", "{got}");
    assert!(
        lines[3].contains("\"dds_name\":\"Alive\"")
            && lines[3].contains("\"dds_name\":\"Missing\""),
        "{got}"
    );
}

/// Spec §8.2 against RabbitMQ: three DDS samples out, one good and one bad
/// message in; the `zerodds_amqp_bridge_*` families count them per topic,
/// with the dispositions of both directions.
#[test]
#[ignore = "needs RabbitMQ 4.0 on 127.0.0.1:5672 and python3-pika (Linux test host)"]
fn the_metrics_of_section_8_2_count_transfers_dispositions_and_samples() {
    let (qout, qin) = ("zd_bridge_metrics_out", "zd_bridge_metrics_in");
    fresh_queue(qout);
    fresh_queue(qin);
    let dds = participant();
    let (_r, rx) = dds
        .register_user_reader_kind(
            UserReaderConfig {
                topic_name: "MetricsIn".into(),
                type_name: "MetricsInType".into(),
                reliable: true,
                durability: zerodds_qos::DurabilityKind::Volatile,
                ..Default::default()
            },
            false,
        )
        .unwrap();
    let eid = dds
        .register_user_writer_kind(writer_cfg("MetricsOut"), false)
        .unwrap();
    let yaml = bridge_yaml("MetricsOut", qout, "out")
        + "  - dds_name: MetricsIn\n    dds_type: MetricsInType\n    amqp_address: /queues/zd_bridge_metrics_in\n    direction: in\n";
    let cfg = config(&yaml);
    let registry = Arc::new(Registry::new());
    let stop = Arc::new(AtomicBool::new(false));
    let join = {
        let (s, r) = (Arc::clone(&stop), Arc::clone(&registry));
        std::thread::spawn(move || {
            let metrics = BridgeMetrics::register_shared(&r);
            let res = bridge_daemon::run(
                &cfg,
                RuntimeConfig::default(),
                &s,
                &metrics,
                false,
                Duration::from_secs(5),
            );
            assert!(res.is_ok(), "bridge stopped: {res:?}");
        })
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    while dds.user_writer_matched_count(eid) == 0 {
        assert!(
            Instant::now() < deadline,
            "the bridge's reader did not match"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_secs(1));
    for n in 1u8..=3 {
        dds.write_user_sample(eid, vec![n, n, n, n]).unwrap();
    }
    pika(&format!(
        "props = pika.BasicProperties(content_type='application/vnd.dds.xcdr2')\n\
         ch.basic_publish(exchange='', routing_key='{qin}', body=b'\\x00\\x07\\x00\\x00' + b'\\x2a\\x00\\x00\\x00', properties=props)\n\
         ch.basic_publish(exchange='', routing_key='{qin}', body=b'xy', properties=props)"
    ));
    let got = pika(&format!(
        "import time\n\
         n = 0\n\
         end = time.time() + 10\n\
         while n < 3 and time.time() < end:\n\
         \x20   m, p, b = ch.basic_get(queue='{qout}', auto_ack=True)\n\
         \x20   if m is None:\n\
         \x20       time.sleep(0.1)\n\
         \x20       continue\n\
         \x20   n += 1\n\
         print(n)"
    ));
    assert_eq!(got, "3");
    let end = Instant::now() + Duration::from_secs(5);
    let mut received = 0;
    while Instant::now() < end && received < 1 {
        if let Ok(UserSample::Alive { .. }) = rx.recv_timeout(Duration::from_millis(200)) {
            received += 1;
        }
    }
    std::thread::sleep(Duration::from_millis(500));
    let text = zerodds_monitor::render_prometheus(&registry.snapshot());
    stop.store(true, Ordering::SeqCst);
    join.join().unwrap();
    let value = |line_start: &str| -> Option<u64> {
        text.lines()
            .find(|l| l.starts_with(line_start))
            .and_then(|l| l.rsplit(' ').next())
            .and_then(|v| v.parse().ok())
    };
    let expect = [
        (
            "zerodds_amqp_bridge_connections_total{state=\"established\"}",
            1,
        ),
        (
            "zerodds_amqp_bridge_transfers_out_total{dds_topic=\"MetricsOut\"}",
            3,
        ),
        (
            "zerodds_amqp_bridge_dds_samples_received{dds_topic=\"MetricsOut\"}",
            3,
        ),
        (
            "zerodds_amqp_bridge_transfers_in_total{dds_topic=\"MetricsIn\"}",
            2,
        ),
        (
            "zerodds_amqp_bridge_dds_samples_published{dds_topic=\"MetricsIn\"}",
            1,
        ),
        (
            "zerodds_amqp_bridge_dispositions_rejected_total{reason=\"amqp:decode-error\"}",
            1,
        ),
        (
            "zerodds_amqp_bridge_sasl_failures_total{mechanism=\"PLAIN\"}",
            0,
        ),
    ];
    for (line, want) in expect {
        assert_eq!(value(line), Some(want), "{line} in\n{text}");
    }
    // The broker accepts the three sent, the bridge accepts the good one.
    assert!(
        value("zerodds_amqp_bridge_dispositions_accepted_total").unwrap_or(0) >= 1,
        "{text}"
    );
    assert!(
        value("zerodds_amqp_bridge_credit_topup_total").unwrap_or(0) >= 1,
        "{text}"
    );
    assert_eq!(
        value("zerodds_amqp_bridge_links_active{role=\"receiver\"}"),
        Some(1),
        "{text}"
    );
}

/// §9.2 SIGHUP: a reload that withdraws the read right stops the topic's
/// samples at once; a configuration that does not load changes nothing.
#[test]
#[ignore = "needs RabbitMQ 4.0 on 127.0.0.1:5672 and python3-pika (Linux test host)"]
fn a_reload_that_withdraws_the_read_right_stops_the_topic() {
    let queue = "zd_bridge_reload";
    fresh_queue(queue);
    let open_yaml = bridge_yaml("ReloadOut", queue, "out");
    let closed_yaml = format!("{open_yaml}acl:\n  default_deny: true\n");
    let cfg = config(&open_yaml);
    let current = Arc::new(std::sync::Mutex::new(open_yaml.clone()));
    let flag = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));
    let join = {
        let (s, f, c) = (Arc::clone(&stop), Arc::clone(&flag), Arc::clone(&current));
        std::thread::spawn(move || {
            let registry = Registry::new();
            let metrics = BridgeMetrics::register(&registry);
            let load = move || -> Result<DaemonConfig, String> {
                let yaml = c.lock().unwrap().clone();
                let mut cc = zerodds_config::Config::default();
                cc.push(
                    zerodds_config::parse_str(&yaml, "<reload>").map_err(|e| e.to_string())?,
                    "<reload>".into(),
                )
                .map_err(|e| e.to_string())?;
                DaemonConfig::from_config(Some(&cc)).map_err(|e| e.to_string())
            };
            let r = bridge_daemon::run_with_reload(
                &cfg,
                RuntimeConfig::default(),
                &s,
                &metrics,
                false,
                Duration::from_secs(5),
                Some(bridge_daemon::Reload {
                    flag: f,
                    load: Box::new(load),
                }),
            );
            assert!(r.is_ok(), "bridge stopped: {r:?}");
        })
    };
    let dds = participant();
    let eid = dds
        .register_user_writer_kind(writer_cfg("ReloadOut"), false)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while dds.user_writer_matched_count(eid) == 0 {
        assert!(
            Instant::now() < deadline,
            "the bridge's reader did not match"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_secs(1));
    let count = || {
        pika(&format!(
            "import time\ntime.sleep(1.0)\nq = ch.queue_declare(queue='{queue}', durable=True, passive=True)\nprint(q.method.message_count)"
        ))
    };
    dds.write_user_sample(eid, vec![1, 1, 1, 1]).unwrap();
    assert_eq!(count(), "1", "before the reload the sample arrives");
    // A file that does not load: nothing changes.
    *current.lock().unwrap() = format!("{open_yaml}acl:\n  default_deny: maybe\n");
    flag.store(true, Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(500));
    dds.write_user_sample(eid, vec![2, 2, 2, 2]).unwrap();
    assert_eq!(count(), "2", "a broken file keeps the old ACL");
    // The read right withdrawn.
    *current.lock().unwrap() = closed_yaml;
    flag.store(true, Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(500));
    dds.write_user_sample(eid, vec![3, 3, 3, 3]).unwrap();
    assert_eq!(count(), "2", "after the reload the topic is closed");
    stop.store(true, Ordering::SeqCst);
    join.join().unwrap();
}

/// The instance states a reader of another vendor (`which`: `cyclone`, or
/// the one `ZERODDS_DISPOSE_READER` names) reports after `pika` published
/// instances 7 and 8 of `KeyedPointA` and a dispose of 7 that names it by
/// `dds:instance-handle` alone — the only identity the DDS-AMQP 1.0 dispose
/// message has. `type_file` gives the endpoint's writer the type.
fn amqp_dispose_states(
    which: &str,
    with_type_file: bool,
) -> std::collections::BTreeMap<String, String> {
    use zerodds_test_vendor_reader::{
        VendorReader, keyed_point_a, keyed_point_a_key_hash, keyed_point_a_type_object,
        reader_command,
    };
    let queue = if with_type_file {
        "zd_bridge_dispose_typed"
    } else {
        "zd_bridge_dispose_hash"
    };
    fresh_queue(queue);
    let type_path = std::env::temp_dir().join(format!(
        "zerodds-amqp-keyed-point-a-{}.typeobject",
        std::process::id()
    ));
    std::fs::write(&type_path, keyed_point_a_type_object()).unwrap();
    let mut yaml = bridge_yaml("AmqpPoint", queue, "in").replace(
        "dds_type: AmqpPointType",
        "dds_type: KeyedPointA\n    keyed: true",
    );
    if with_type_file {
        yaml = yaml.replace(
            "    keyed: true\n",
            &format!(
                "    keyed: true\n    type_file: \"{}\"\n",
                type_path.display()
            ),
        );
    }
    let (command, label) = reader_command(which);
    let mut reader = VendorReader::start(command, &label, domain(), "AmqpPoint");
    // A container id of its own per run: two runs of the endpoint with one
    // id have one GUID prefix.
    let yaml = yaml.replace(
        "container_id: \"zd-e2e-AmqpPoint-",
        &format!("container_id: \"zd-e2e-AmqpPoint-{queue}-"),
    );
    let (stop, join) = start_bridge(config(&yaml));
    reader.wait_matched();
    let hex = |b: &[u8]| b.iter().map(|x| format!("\\x{x:02x}")).collect::<String>();
    let mut publish =
        String::from("props = pika.BasicProperties(content_type='application/vnd.dds.xcdr1')\n");
    for id in [7, 8] {
        publish.push_str(&format!(
            "ch.basic_publish(exchange='', routing_key='{queue}', body=b'\\x00\\x01\\x00\\x00{}', properties=props)\n",
            hex(&keyed_point_a(id, 100 + id))
        ));
    }
    // Published until the reader has both instances: a sample the
    // endpoint's writer sends before it knows the reader is lost
    // (VOLATILE).
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        pika(&publish);
        if reader.wait_seen(&[7, 8], Duration::from_secs(2)) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the bridged samples did not reach the reader"
        );
    }
    // The dispose goes out as AMQP 1.0 (qpid-proton): `dds:instance-handle`
    // is a binary application property, which pika's AMQP 0.9.1 headers
    // cannot carry.
    proton_dispose(queue, &hex(&keyed_point_a_key_hash(7)));
    std::thread::sleep(Duration::from_millis(1500));
    let states = reader.snapshot();
    stop.store(true, Ordering::SeqCst);
    join.join().unwrap();
    let _ = reader.finish();
    let _ = std::fs::remove_file(&type_path);
    println!(
        "{label}, {} type_file: {states:?}",
        if with_type_file { "with" } else { "without" }
    );
    states
}

/// A DDS-AMQP 1.0 dispose message, `dds:instance-handle` = the escaped
/// octets `handle`, into `queue`, through python3's qpid-proton.
fn proton_dispose(queue: &str, handle: &str) {
    let out = Command::new("python3")
        .arg("-c")
        .arg(format!(
            "from proton import Message\n\
             from proton.utils import BlockingConnection\n\
             c = BlockingConnection('amqp://127.0.0.1:5672', user='zerodds', password='zerodds', allowed_mechs='PLAIN')\n\
             s = c.create_sender('/queues/{queue}')\n\
             s.send(Message(body=b'\\x00\\x01\\x00\\x00', inferred=True, content_type='application/vnd.dds.xcdr1', \
             properties={{'dds:operation': 'dispose', 'dds:instance-handle': b'{handle}'}}))\n\
             c.close()\n"
        ))
        .output()
        .expect("python3");
    assert!(
        out.status.success(),
        "qpid-proton failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn assert_amqp_dispose_states(label: &str, states: &std::collections::BTreeMap<String, String>) {
    assert_eq!(
        states.get("7").map(String::as_str),
        Some("NOT_ALIVE_DISPOSED"),
        "instance 7 at the {label} reader (all: {states:?})"
    );
    assert_eq!(
        states.get("8").map(String::as_str),
        Some("ALIVE"),
        "instance 8 at the {label} reader (all: {states:?})"
    );
}

/// A DDS-AMQP dispose message reaches a Cyclone reader as a dispose (D10).
#[test]
#[ignore = "needs RabbitMQ 4.0 on 127.0.0.1:5672, python3-pika, python3-qpid-proton and python3 with the cyclonedds binding (Linux test host)"]
fn an_amqp_dispose_reaches_a_cyclone_reader() {
    let _ = amqp_dispose_states("cyclone", false);
    let states = amqp_dispose_states("cyclone", true);
    assert_amqp_dispose_states("cyclone", &states);
}

/// The same against the reader `ZERODDS_DISPOSE_READER` names
/// (`tools/dispose-reader/build.sh`: Fast DDS, OpenDDS, OpenSplice).
#[test]
#[ignore = "needs RabbitMQ 4.0 on 127.0.0.1:5672, python3-pika, python3-qpid-proton and ZERODDS_DISPOSE_READER (Linux test host)"]
fn an_amqp_dispose_reaches_a_foreign_reader() {
    let _ = amqp_dispose_states("env", false);
    let states = amqp_dispose_states("env", true);
    assert_amqp_dispose_states("foreign", &states);
}
