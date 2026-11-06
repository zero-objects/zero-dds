// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The bridge daemon against RabbitMQ (`zerodds-amqp-bridge-daemon-1.0`
//! §12.2): a DDS participant in this process on one side, a RabbitMQ node
//! of each test alone on the other, and `pika` (AMQP 0-9-1) or qpid-proton
//! (AMQP 1.0) as the reference clients that read and write the queues
//! without any ZeroDDS code.
//!
//! The tests need `rabbitmq-server`, python3 with pika and qpid-proton;
//! they run in the integration pillar's job `integration-bridges-vendor-amqp`,
//! the two with a reader of another DDS vendor in the jobs
//! `vendor-live-dispose-bridges` of `ci/interop-omg-weekly.yml`. A missing program
//! fails them.
//!
//! ```bash
//! cargo nextest run -p zerodds-amqp-endpoint --test bridge_rabbitmq_e2e --run-ignored all
//! ```

#![cfg(all(feature = "std", unix))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

#[path = "../../amqp-0-9-1/tests/support/rabbitmq.rs"]
mod rabbitmq;

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use rabbitmq::RabbitNode;
use zerodds_amqp_endpoint::bridge_daemon::{self, DaemonConfig};
use zerodds_amqp_endpoint::daemon_runtime::BridgeMetrics;
use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_monitor::Registry;
use zerodds_rtps::wire_types::{EntityId, GuidPrefix};

thread_local! {
    /// The domain of the running test, leased host-wide and held until the
    /// test's thread ends.
    static DOMAIN: zerodds_test_domains::DomainLease = zerodds_test_vendor_reader::lease_domain();
}

fn domain() -> i32 {
    DOMAIN.with(zerodds_test_domains::DomainLease::id)
}

/// How long a step may take before the test fails.
const STEP: Duration = Duration::from_secs(10);

fn config(yaml: &str) -> DaemonConfig {
    let mut c = zerodds_config::Config::default();
    c.push(
        zerodds_config::parse_str(yaml, "<test>").unwrap(),
        "<test>".into(),
    )
    .unwrap();
    DaemonConfig::from_config(Some(&c)).unwrap()
}

/// Runs the daemon in a thread until the returned flag is set; its metrics
/// (spec §8.2) are in the registry this returns.
fn start_bridge_with_metrics(
    cfg: DaemonConfig,
) -> (Arc<AtomicBool>, std::thread::JoinHandle<()>, Arc<Registry>) {
    let stop = Arc::new(AtomicBool::new(false));
    let registry = Arc::new(Registry::new());
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
    (stop, join, registry)
}

/// The value of the metric line that begins with `line_start`.
fn metric(registry: &Registry, line_start: &str) -> Option<u64> {
    zerodds_monitor::render_prometheus(&registry.snapshot())
        .lines()
        .find(|l| l.starts_with(line_start))
        .and_then(|l| l.rsplit(' ').next())
        .and_then(|v| v.parse().ok())
}

/// Waits until the daemon's metric `line_start` reaches `want`; the
/// participant's match events pace the reads of the in-process registry.
fn wait_metric(dds: &DcpsRuntime, registry: &Registry, line_start: &str, want: u64) {
    let deadline = Instant::now() + STEP;
    while metric(registry, line_start) != Some(want) {
        assert!(
            Instant::now() < deadline,
            "{line_start} did not reach {want}: {:?}",
            metric(registry, line_start)
        );
        dds.wait_match_event(Duration::from_millis(20));
    }
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
    // prefix give their first writers one GUID.
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

fn reader_cfg(topic: &str) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic.into(),
        type_name: format!("{topic}Type"),
        reliable: true,
        durability: zerodds_qos::DurabilityKind::Volatile,
        ..Default::default()
    }
}

/// Waits on the participant's match events until `matched` holds.
fn wait_matched(dds: &DcpsRuntime, what: &str, matched: impl Fn() -> bool) {
    let deadline = Instant::now() + STEP;
    while !matched() {
        let left = deadline.saturating_duration_since(Instant::now());
        assert!(!left.is_zero(), "{what} did not match within {STEP:?}");
        dds.wait_match_event(left.min(Duration::from_millis(100)));
    }
}

/// The bridge's DDS reader matched the test's writer `eid`.
fn wait_writer_matched(dds: &DcpsRuntime, eid: EntityId) {
    wait_matched(dds, "the bridge's reader", || {
        dds.user_writer_matched_count(eid) > 0
    });
}

/// The bridge's DDS writer matched the test's reader `eid`.
fn wait_reader_matched(dds: &DcpsRuntime, eid: EntityId) {
    wait_matched(dds, "the bridge's writer", || {
        dds.user_reader_matched_count(eid) > 0
    });
}

fn bridge_yaml(node: &RabbitNode, topic: &str, queue: &str, direction: &str) -> String {
    format!(
        "domain: {d}\namqp:\n  broker_url: \"amqp://127.0.0.1:{port}\"\n  container_id: \"zd-e2e-{topic}-{pid}\"\n  sasl:\n    mechanism: PLAIN\n    username: guest\n    password: guest\ntopics:\n  - dds_name: {topic}\n    dds_type: {topic}Type\n    amqp_address: /queues/{queue}\n    direction: {direction}\n    qos:\n      history: {{ kind: keep_all }}\n",
        d = domain(),
        port = node.port,
        pid = std::process::id()
    )
}

fn bridge_yaml_with(
    node: &RabbitNode,
    topic: &str,
    queue: &str,
    direction: &str,
    extra_amqp: &str,
) -> String {
    bridge_yaml(node, topic, queue, direction)
        .replace("  sasl:\n", &format!("{extra_amqp}  sasl:\n"))
}

fn declare(node: &RabbitNode, queues: &[&str]) {
    let script: String = queues
        .iter()
        .map(|q| format!("ch.queue_declare(queue='{q}', durable=True)\n"))
        .collect();
    let _ = node.pika(&script);
}

/// pika consumes up to `n` messages of `queue` within `within`, one line
/// each: content type, expiration, the body in hexadecimal.
fn take(node: &RabbitNode, queue: &str, n: usize, within: Duration) -> Vec<String> {
    let out = node.pika(&format!(
        "out = []\n\
         def got(ch_, m, p, b):\n\
         \x20   out.append('%s %s %s' % (p.content_type, p.expiration, b.hex()))\n\
         \x20   if len(out) >= {n}:\n\
         \x20       ch.stop_consuming()\n\
         ch.basic_consume(queue='{queue}', on_message_callback=got, auto_ack=True)\n\
         c.call_later({secs}, ch.stop_consuming)\n\
         ch.start_consuming()\n\
         print('\\n'.join(out))",
        secs = within.as_secs_f64()
    ));
    out.lines().map(str::to_string).collect()
}

/// The body (third field) of a line of [`take`].
fn body(line: &str) -> &str {
    line.rsplit(' ').next().unwrap()
}

/// The messages ready in `queue`.
fn depth(node: &RabbitNode, queue: &str) -> u64 {
    node.pika(&format!(
        "q = ch.queue_declare(queue='{queue}', durable=True, passive=True)\n\
         print(q.method.message_count)"
    ))
    .parse()
    .unwrap()
}

/// Takes what reaches `rx` within `within`.
fn received(rx: &mpsc::Receiver<UserSample>, n: usize, within: Duration) -> Vec<Vec<u8>> {
    let deadline = Instant::now() + within;
    let mut out = Vec::new();
    while out.len() < n {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(UserSample::Alive { payload, .. }) => out.push(payload.as_ref().to_vec()),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    out
}

/// §4.3: samples written in DDS arrive in the queue as pass-through
/// messages, the body the encapsulation header and the payload, the content
/// type naming the representation.
#[test]
#[ignore = "needs RabbitMQ and pika; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_4_3_dds_samples_arrive_in_the_queue_as_pass_through_messages() {
    let node = RabbitNode::start();
    declare(&node, &["zd_bridge_out"]);
    let (stop, join) = start_bridge(config(&bridge_yaml(
        &node,
        "BridgeOut",
        "zd_bridge_out",
        "out",
    )));
    let dds = participant();
    let eid = dds
        .register_user_writer_kind(writer_cfg("BridgeOut"), false)
        .unwrap();
    wait_writer_matched(&dds, eid);
    for n in 1u8..=3 {
        dds.write_user_sample(eid, vec![n, n, n, n]).unwrap();
    }
    let got = take(&node, "zd_bridge_out", 3, STEP);
    stop.store(true, Ordering::SeqCst);
    join.join().unwrap();
    assert_eq!(got.len(), 3, "{got:?}");
    for (i, line) in got.iter().enumerate() {
        let hex = body(line);
        let n = format!("{:02x}", i + 1).repeat(4);
        assert!(hex.ends_with(&n), "{line}");
        // Header 0x0001 (CDR_LE) or 0x0007 (PLAIN_CDR2_LE), then the payload.
        match &hex[..4] {
            "0001" => assert!(line.starts_with("application/vnd.dds.xcdr1 "), "{line}"),
            "0007" => assert!(line.starts_with("application/vnd.dds.xcdr2 "), "{line}"),
            other => panic!("encapsulation {other}: {line}"),
        }
    }
}

/// §4.4: a message pika publishes reaches a DDS reader and is accepted; a
/// body that is no DDS sample is rejected and reaches no reader.
#[test]
#[ignore = "needs RabbitMQ and pika; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_4_4_a_message_from_pika_reaches_a_reader_and_a_bad_body_does_not() {
    let node = RabbitNode::start();
    declare(&node, &["zd_bridge_in"]);
    let dds = participant();
    let (rid, rx) = dds
        .register_user_reader_kind(reader_cfg("BridgeIn"), false)
        .unwrap();
    let (stop, join, registry) = start_bridge_with_metrics(config(&bridge_yaml(
        &node,
        "BridgeIn",
        "zd_bridge_in",
        "in",
    )));
    wait_reader_matched(&dds, rid);
    let _ = node.pika(
        "props = pika.BasicProperties(content_type='application/vnd.dds.xcdr2')\n\
         ch.basic_publish(exchange='', routing_key='zd_bridge_in', body=b'\\x00\\x07\\x00\\x00' + b'\\x2a\\x00\\x00\\x00', properties=props)\n\
         ch.basic_publish(exchange='', routing_key='zd_bridge_in', body=b'xy', properties=props)",
    );
    let payloads = received(&rx, 1, STEP);
    // The bad body is rejected (spec §4.4: `amqp:decode-error`).
    wait_metric(
        &dds,
        &registry,
        "zerodds_amqp_bridge_dispositions_rejected_total{reason=\"amqp:decode-error\"}",
        1,
    );
    let extra = received(&rx, 1, Duration::from_millis(500));
    stop.store(true, Ordering::SeqCst);
    join.join().unwrap();
    // With the bridge's link gone, a delivery it left unsettled would be
    // back in the queue.
    assert_eq!(depth(&node, "zd_bridge_in"), 0, "both messages settled");
    assert_eq!(
        payloads,
        vec![vec![0x2a, 0, 0, 0]],
        "exactly the one DDS sample"
    );
    assert!(extra.is_empty(), "the bad body reached DDS");
}

/// Part 2 §2.6.14 and §4.3: a 20 000-byte sample at a 4096-byte frame size
/// crosses as a multi-frame transfer, out to the queue and back in; the
/// copy that carries this bridge's `dds:bridge-id` is dropped (DDS-AMQP 1.0
/// §8.4), the bare one written.
#[test]
#[ignore = "needs RabbitMQ and pika; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_4_3_a_sample_larger_than_the_frame_size_crosses_in_both_directions() {
    let node = RabbitNode::start();
    let (qout, qin) = ("zd_bridge_big_out", "zd_bridge_big_in");
    declare(&node, &[qout, qin]);
    let payload: Vec<u8> = (0..20_000u32).map(|i| (i % 251) as u8).collect();
    let dds = participant();
    let (rid, rx) = dds
        .register_user_reader_kind(reader_cfg("BigIn"), false)
        .unwrap();
    let w = dds
        .register_user_writer_kind(writer_cfg("BigOut"), false)
        .unwrap();
    let yaml = format!(
        "{}  - dds_name: BigIn\n    dds_type: BigInType\n    amqp_address: /queues/{qin}\n    direction: in\n",
        bridge_yaml_with(&node, "BigOut", qout, "out", "  max_frame_size: 4096\n")
    );
    let (stop, join, registry) = start_bridge_with_metrics(config(&yaml));
    wait_writer_matched(&dds, w);
    wait_reader_matched(&dds, rid);
    dds.write_user_sample(w, payload.clone()).unwrap();
    // pika takes the message and puts it into the inbound queue twice:
    // first as it is (with this bridge's stamp), then with the content type
    // alone.
    let moved = node.pika(&format!(
        "res = []\n\
         def got(ch_, m, p, b):\n\
         \x20   ch.basic_publish(exchange='', routing_key='{qin}', body=b, properties=p)\n\
         \x20   bare = pika.BasicProperties(content_type=p.content_type)\n\
         \x20   ch.basic_publish(exchange='', routing_key='{qin}', body=b, properties=bare)\n\
         \x20   res.append(len(b))\n\
         \x20   ch.stop_consuming()\n\
         ch.basic_consume(queue='{qout}', on_message_callback=got, auto_ack=True)\n\
         c.call_later(10, ch.stop_consuming)\n\
         ch.start_consuming()\n\
         print(res[0] if res else 0)"
    ));
    assert_eq!(moved, "20004", "header and payload in one message");
    let back = received(&rx, 1, STEP);
    // Both copies arrived at the bridge.
    wait_metric(
        &dds,
        &registry,
        "zerodds_amqp_bridge_transfers_in_total{dds_topic=\"BigIn\"}",
        2,
    );
    let extra = received(&rx, 1, Duration::from_millis(500));
    stop.store(true, Ordering::SeqCst);
    join.join().unwrap();
    assert_eq!(back, vec![payload], "the same 20 000 bytes back in DDS");
    assert!(extra.is_empty(), "the stamped copy is dropped");
    // With the bridge's link gone, a copy it left unsettled would be back
    // in the queue: the stamped one was accepted and dropped.
    assert_eq!(depth(&node, qin), 0, "both copies settled");
}

/// §9.3: the broker closes the connection; the bridge reconnects, attaches
/// again and delivers what was written meanwhile.
#[test]
#[ignore = "needs RabbitMQ and pika; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_9_3_after_the_broker_closes_the_connection_the_bridge_reconnects() {
    let node = RabbitNode::start();
    declare(&node, &["zd_bridge_reconnect"]);
    let dds = participant();
    let w = dds
        .register_user_writer_kind(writer_cfg("Reconnect"), false)
        .unwrap();
    let yaml = bridge_yaml_with(
        &node,
        "Reconnect",
        "zd_bridge_reconnect",
        "out",
        "  reconnect:\n    initial_delay_ms: 200\n    max_delay_ms: 1000\n    factor: 2.0\n",
    );
    let (stop, join) = start_bridge(config(&yaml));
    wait_writer_matched(&dds, w);
    dds.write_user_sample(w, vec![1, 1, 1, 1]).unwrap();
    let first = take(&node, "zd_bridge_reconnect", 1, STEP);
    let _ = node.ctl(&["close_all_connections", "zerodds-e2e"]);
    dds.write_user_sample(w, vec![2, 2, 2, 2]).unwrap();
    let second = take(&node, "zd_bridge_reconnect", 1, STEP);
    stop.store(true, Ordering::SeqCst);
    join.join().unwrap();
    assert_eq!(
        first.iter().map(|l| &body(l)[8..]).collect::<Vec<_>>(),
        ["01010101"]
    );
    assert_eq!(
        second.iter().map(|l| &body(l)[8..]).collect::<Vec<_>>(),
        ["02020202"],
        "the sample written after the close, after the reconnect"
    );
}

/// §4.2, §5.2, §6: a topic whose address the broker refuses does not take
/// the others down, the catalog goes to its address, and the writer's
/// LIFESPAN is the message ttl.
#[test]
#[ignore = "needs RabbitMQ and pika; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_4_2_a_refused_link_leaves_the_others_running_and_catalog_and_ttl_arrive() {
    let node = RabbitNode::start();
    let (queue, catalog) = ("zd_bridge_ok", "zd_bridge_catalog");
    declare(&node, &[queue, catalog]);
    let dds = participant();
    let mut wcfg = writer_cfg("Alive");
    wcfg.lifespan = zerodds_qos::LifespanQosPolicy {
        duration: zerodds_qos::Duration::from_millis(60_000),
    };
    let w = dds.register_user_writer_kind(wcfg, false).unwrap();
    let yaml = format!(
        "{}  - dds_name: Missing\n    amqp_address: /queues/zd_bridge_does_not_exist\n    direction: out\ncatalog:\n  address: /queues/{catalog}\n",
        bridge_yaml(&node, "Alive", queue, "out")
    );
    let (stop, join) = start_bridge(config(&yaml));
    wait_writer_matched(&dds, w);
    dds.write_user_sample(w, vec![7, 7, 7, 7]).unwrap();
    let data = take(&node, queue, 1, STEP);
    let cat = node.pika(&format!(
        "res = []\n\
         def got(ch_, m, p, b):\n\
         \x20   res.append(p.content_type + '\\n' + b.decode())\n\
         \x20   ch.stop_consuming()\n\
         ch.basic_consume(queue='{catalog}', on_message_callback=got, auto_ack=True)\n\
         c.call_later(10, ch.stop_consuming)\n\
         ch.start_consuming()\n\
         print(res[0] if res else '')"
    ));
    stop.store(true, Ordering::SeqCst);
    join.join().unwrap();
    assert_eq!(data.len(), 1, "{data:?}");
    let fields: Vec<&str> = data[0].split(' ').collect();
    assert_eq!(fields[1], "60000", "LIFESPAN as the ttl: {data:?}");
    assert_eq!(&fields[2][8..], "07070707", "{data:?}");
    let (ct, json) = cat.split_once('\n').unwrap_or((&cat, ""));
    assert_eq!(ct, "application/json", "{cat}");
    assert!(
        json.contains("\"dds_name\":\"Alive\"") && json.contains("\"dds_name\":\"Missing\""),
        "{cat}"
    );
}

/// §8.2 against RabbitMQ: three DDS samples out, one good and one bad
/// message in; the `zerodds_amqp_bridge_*` families count them per topic,
/// with the dispositions of both directions.
#[test]
#[ignore = "needs RabbitMQ and pika; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_8_2_the_metrics_count_transfers_dispositions_and_samples() {
    let node = RabbitNode::start();
    let (qout, qin) = ("zd_bridge_metrics_out", "zd_bridge_metrics_in");
    declare(&node, &[qout, qin]);
    let dds = participant();
    let (rid, rx) = dds
        .register_user_reader_kind(reader_cfg("MetricsIn"), false)
        .unwrap();
    let eid = dds
        .register_user_writer_kind(writer_cfg("MetricsOut"), false)
        .unwrap();
    let yaml = bridge_yaml(&node, "MetricsOut", qout, "out")
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
    wait_writer_matched(&dds, eid);
    wait_reader_matched(&dds, rid);
    for n in 1u8..=3 {
        dds.write_user_sample(eid, vec![n, n, n, n]).unwrap();
    }
    let _ = node.pika(&format!(
        "props = pika.BasicProperties(content_type='application/vnd.dds.xcdr2')\n\
         ch.basic_publish(exchange='', routing_key='{qin}', body=b'\\x00\\x07\\x00\\x00' + b'\\x2a\\x00\\x00\\x00', properties=props)\n\
         ch.basic_publish(exchange='', routing_key='{qin}', body=b'xy', properties=props)"
    ));
    assert_eq!(take(&node, qout, 3, STEP).len(), 3);
    assert_eq!(received(&rx, 1, STEP).len(), 1);
    let value = |text: &str, line_start: &str| -> Option<u64> {
        text.lines()
            .find(|l| l.starts_with(line_start))
            .and_then(|l| l.rsplit(' ').next())
            .and_then(|v| v.parse().ok())
    };
    // The counters move as the pump settles; wait until the rejection of
    // the bad message is counted, the last of them.
    let deadline = Instant::now() + STEP;
    let text = loop {
        let text = zerodds_monitor::render_prometheus(&registry.snapshot());
        if value(
            &text,
            "zerodds_amqp_bridge_dispositions_rejected_total{reason=\"amqp:decode-error\"}",
        ) == Some(1)
            && value(
                &text,
                "zerodds_amqp_bridge_transfers_out_total{dds_topic=\"MetricsOut\"}",
            ) == Some(3)
        {
            break text;
        }
        assert!(
            Instant::now() < deadline,
            "the counters did not reach their values:\n{text}"
        );
        dds.wait_match_event(Duration::from_millis(50));
    };
    stop.store(true, Ordering::SeqCst);
    join.join().unwrap();
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
        assert_eq!(value(&text, line), Some(want), "{line} in\n{text}");
    }
    assert!(
        value(&text, "zerodds_amqp_bridge_dispositions_accepted_total").unwrap_or(0) >= 1,
        "{text}"
    );
    assert!(
        value(&text, "zerodds_amqp_bridge_credit_topup_total").unwrap_or(0) >= 1,
        "{text}"
    );
    assert_eq!(
        value(&text, "zerodds_amqp_bridge_links_active{role=\"receiver\"}"),
        Some(1),
        "{text}"
    );
}

/// §9.2 SIGHUP: a reload that withdraws the read right stops the topic's
/// samples at once; a configuration that does not load changes nothing.
#[test]
#[ignore = "needs RabbitMQ and pika; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_9_2_a_reload_that_withdraws_the_read_right_stops_the_topic() {
    let node = RabbitNode::start();
    let queue = "zd_bridge_reload";
    declare(&node, &[queue]);
    let open_yaml = bridge_yaml(&node, "ReloadOut", queue, "out");
    let closed_yaml = format!("{open_yaml}acl:\n  default_deny: true\n");
    let cfg = config(&open_yaml);
    let current = Arc::new(std::sync::Mutex::new(open_yaml.clone()));
    let flag = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));
    // The daemon's reload reads the configuration through `load`; each
    // call is reported here, and the new ACL applies right after it.
    let (loaded_tx, loaded) = mpsc::channel::<()>();
    let join = {
        let (s, f, c) = (Arc::clone(&stop), Arc::clone(&flag), Arc::clone(&current));
        let loaded_tx = std::sync::Mutex::new(loaded_tx);
        std::thread::spawn(move || {
            let registry = Registry::new();
            let metrics = BridgeMetrics::register(&registry);
            let load = move || -> Result<DaemonConfig, String> {
                let yaml = c.lock().unwrap().clone();
                let mut cc = zerodds_config::Config::default();
                let parsed = zerodds_config::parse_str(&yaml, "<reload>")
                    .map_err(|e| e.to_string())
                    .and_then(|p| cc.push(p, "<reload>".into()).map_err(|e| e.to_string()))
                    .and_then(|()| DaemonConfig::from_config(Some(&cc)).map_err(|e| e.to_string()));
                let _ = loaded_tx.lock().unwrap().send(());
                parsed
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
    wait_writer_matched(&dds, eid);
    let reload = |yaml: String| {
        *current.lock().unwrap() = yaml;
        flag.store(true, Ordering::SeqCst);
        loaded
            .recv_timeout(STEP)
            .expect("the daemon read the configuration");
    };
    dds.write_user_sample(eid, vec![1, 1, 1, 1]).unwrap();
    assert_eq!(
        take(&node, queue, 1, STEP).len(),
        1,
        "before the reload the sample arrives"
    );
    // A file that does not load: nothing changes.
    reload(format!("{open_yaml}acl:\n  default_deny: maybe\n"));
    dds.write_user_sample(eid, vec![2, 2, 2, 2]).unwrap();
    assert_eq!(
        take(&node, queue, 1, STEP).len(),
        1,
        "a broken file keeps the old ACL"
    );
    // The read right withdrawn: within the time the open ACL took to deliver
    // one, nothing arrives.
    reload(closed_yaml);
    dds.write_user_sample(eid, vec![3, 3, 3, 3]).unwrap();
    let after = take(&node, queue, 1, Duration::from_secs(2));
    stop.store(true, Ordering::SeqCst);
    join.join().unwrap();
    assert!(
        after.is_empty(),
        "after the reload the topic is closed: {after:?}"
    );
}

/// The instance states a reader of another vendor (`which`: `cyclone`, or
/// the one `ZERODDS_DISPOSE_READER` names) reports after `pika` published
/// instances 7 and 8 of `KeyedPointA` and a dispose of 7 that names it by
/// `dds:instance-handle` alone, the only identity the DDS-AMQP 1.0 dispose
/// message has; the endpoint's writer has the type from `type_file`.
fn amqp_dispose_states(which: &str) -> std::collections::BTreeMap<String, String> {
    use zerodds_test_vendor_reader::{
        VendorReader, keyed_point_a, keyed_point_a_key_hash, keyed_point_a_type_object,
        reader_command,
    };
    let node = RabbitNode::start();
    let queue = "zd_bridge_dispose";
    declare(&node, &[queue]);
    let type_path = node.dir().join("keyed-point-a.typeobject");
    std::fs::write(&type_path, keyed_point_a_type_object()).unwrap();
    let yaml = bridge_yaml(&node, "AmqpPoint", queue, "in").replace(
        "dds_type: AmqpPointType",
        &format!(
            "dds_type: KeyedPointA\n    keyed: true\n    type_file: \"{}\"",
            type_path.display()
        ),
    );
    let (command, label) = reader_command(which);
    let mut reader = VendorReader::start(command, &label, domain(), "AmqpPoint");
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
    // Published until the reader has both instances: the reader's match
    // does not say the endpoint's writer has matched it, and a sample that
    // writer sends before is lost to the reader (VOLATILE).
    let deadline = Instant::now() + STEP;
    loop {
        let _ = node.pika(&publish);
        if reader.wait_seen(&[7, 8], Duration::from_secs(2)) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the bridged samples did not reach the {label} reader"
        );
    }
    // The dispose goes out as AMQP 1.0 (qpid-proton): `dds:instance-handle`
    // is a binary application property, which pika's AMQP 0-9-1 headers
    // cannot carry.
    let out = Command::new(rabbitmq::python())
        .arg("-c")
        .arg(format!(
            "from proton import Message\n\
             from proton.utils import BlockingConnection\n\
             c = BlockingConnection('amqp://127.0.0.1:{port}', user='guest', password='guest', allowed_mechs='PLAIN')\n\
             s = c.create_sender('/queues/{queue}')\n\
             s.send(Message(body=b'\\x00\\x01\\x00\\x00', inferred=True, content_type='application/vnd.dds.xcdr1', \
             properties={{'dds:operation': 'dispose', 'dds:instance-handle': b'{handle}'}}))\n\
             c.close()\n",
            port = node.port,
            handle = hex(&keyed_point_a_key_hash(7))
        ))
        .output()
        .expect("python3");
    assert!(
        out.status.success(),
        "qpid-proton failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let states = reader.wait_states(&[(7, "NOT_ALIVE_DISPOSED"), (8, "ALIVE")], STEP);
    stop.store(true, Ordering::SeqCst);
    join.join().unwrap();
    let _ = reader.finish();
    states
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

/// DDS-AMQP 1.0 §7.7.2: a dispose message reaches a Cyclone reader as a
/// dispose of the instance its `dds:instance-handle` names.
#[test]
#[ignore = "needs RabbitMQ, pika, qpid-proton and Cyclone's Python binding; runs in vendor-live-dispose-bridges"]
fn ddsamqp_7_7_2_an_amqp_dispose_reaches_a_cyclone_reader() {
    let states = amqp_dispose_states("cyclone");
    assert_amqp_dispose_states("cyclone", &states);
}

/// The same against the reader `ZERODDS_DISPOSE_READER` names
/// (`tools/dispose-reader/build.sh`: Fast DDS, OpenDDS, OpenSplice).
#[test]
#[ignore = "needs RabbitMQ, pika, qpid-proton and ZERODDS_DISPOSE_READER; runs in vendor-live-dispose-bridges"]
fn ddsamqp_7_7_2_an_amqp_dispose_reaches_a_foreign_reader() {
    let states = amqp_dispose_states("env");
    assert_amqp_dispose_states("foreign", &states);
}
