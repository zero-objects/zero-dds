// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AMQP 0-9-1 against RabbitMQ: the ZeroDDS client `Amqp091Client` on one
//! side, a RabbitMQ node of each test alone on the other, and pika (AMQP
//! 0-9-1) or qpid-proton (AMQP 1.0) as the reference clients where a test
//! reads or writes without ZeroDDS code.
//!
//! The tests need `rabbitmq-server`, python3 with pika and qpid-proton; they
//! run in the integration pillar's job `integration-bridges-vendor-amqp`, where a
//! missing program fails them:
//!
//! ```bash
//! cargo nextest run -p zerodds-amqp-0-9-1 --test rabbitmq_amqp091_e2e --run-ignored all
//! ```

#![cfg(all(feature = "std", unix))]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::uninlined_format_args
)]

#[path = "support/rabbitmq.rs"]
mod rabbitmq;

use std::process::Command;

use rabbitmq::RabbitNode;
use zerodds_amqp_0_9_1::client::Amqp091Client;
use zerodds_amqp_0_9_1::method::ContentProperties;
use zerodds_amqp_0_9_1::types::FieldValue;

fn connect(node: &RabbitNode) -> Amqp091Client {
    Amqp091Client::connect(node.addr(), "guest", "guest", "/").expect("AMQP 0-9-1 handshake")
}

fn text(body: Option<Vec<u8>>) -> Option<String> {
    body.map(|b| String::from_utf8_lossy(&b).to_string())
}

/// §1.4 connection handshake (start-ok with PLAIN, tune-ok, open), then
/// queue.declare, basic.publish with content and basic.get.
#[test]
#[ignore = "needs RabbitMQ, pika and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqp091_1_4_the_client_handshakes_declares_publishes_and_gets() {
    let node = RabbitNode::start();
    let mut c = connect(&node);
    assert_eq!(
        c.queue_declare("zd_self", true).expect("queue.declare"),
        "zd_self"
    );
    c.publish("", "zd_self", b"zerodds-0.9.1-self")
        .expect("basic.publish");
    assert_eq!(
        text(c.get("zd_self").expect("basic.get")).as_deref(),
        Some("zerodds-0.9.1-self")
    );
    c.close();
}

/// §1.8: a message the client publishes is read by an AMQP 1.0 client
/// (qpid-proton) from the same queue.
#[test]
#[ignore = "needs RabbitMQ, pika and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqp091_1_8_a_published_message_is_read_by_an_amqp_1_0_client() {
    let node = RabbitNode::start();
    let mut c = connect(&node);
    c.queue_declare("zd_to10", true).expect("declare");
    c.publish("", "zd_to10", b"zerodds-0.9.1->1.0")
        .expect("publish");
    c.close();
    let out = Command::new(rabbitmq::python())
        .arg("-c")
        .arg(format!(
            r#"
from proton.utils import BlockingConnection
conn = BlockingConnection("{url}")
rcv = conn.create_receiver("/queues/zd_to10")
m = rcv.receive(timeout=10)
b = m.body
print(b.decode() if isinstance(b, (bytes, bytearray)) else b)
rcv.accept(); conn.close()
"#,
            url = node.url()
        ))
        .output()
        .expect("python3");
    assert!(
        out.status.success(),
        "qpid-proton failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "zerodds-0.9.1->1.0"
    );
}

/// §1.8 basic.get: the client reads what pika published.
#[test]
#[ignore = "needs RabbitMQ, pika and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqp091_1_8_get_reads_what_pika_published() {
    let node = RabbitNode::start();
    let _ = node.pika(
        "ch.queue_declare(queue='zd_rx', durable=True)\n\
         ch.basic_publish(exchange='', routing_key='zd_rx', body=b'pika->zerodds-0.9.1')",
    );
    let mut c = connect(&node);
    assert_eq!(
        text(c.get("zd_rx").expect("basic.get")).as_deref(),
        Some("pika->zerodds-0.9.1")
    );
    c.close();
}

/// §1.6 and §1.7: a topic exchange routes a message to the queue bound
/// with its key; unbind, purge, delete and exchange.delete are answered.
#[test]
#[ignore = "needs RabbitMQ, pika and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqp091_1_6_a_topic_exchange_routes_to_the_bound_queue() {
    let node = RabbitNode::start();
    let mut c = connect(&node);
    c.exchange_declare("zd_ex", "topic", true)
        .expect("exchange.declare");
    c.queue_declare("zd_ex_q", true).expect("queue.declare");
    c.queue_bind("zd_ex_q", "zd_ex", "zd.key")
        .expect("queue.bind");
    c.publish("zd_ex", "zd.key", b"routed-via-exchange")
        .expect("publish");
    assert_eq!(
        text(c.get("zd_ex_q").expect("basic.get")).as_deref(),
        Some("routed-via-exchange")
    );
    c.queue_unbind("zd_ex_q", "zd_ex", "zd.key")
        .expect("queue.unbind");
    c.publish("zd_ex", "zd.key", b"after-unbind")
        .expect("publish");
    assert_eq!(
        c.queue_purge("zd_ex_q").expect("queue.purge"),
        0,
        "unbound: nothing routed"
    );
    assert_eq!(c.queue_delete("zd_ex_q").expect("queue.delete"), 0);
    c.exchange_delete("zd_ex", false).expect("exchange.delete");
    c.close();
}

/// §1.8 basic.qos and basic.consume: the broker pushes basic.deliver.
#[test]
#[ignore = "needs RabbitMQ, pika and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqp091_1_8_consume_delivers_asynchronously() {
    let node = RabbitNode::start();
    let mut c = connect(&node);
    c.queue_declare("zd_consume", true).expect("declare");
    c.qos(10).expect("basic.qos");
    c.publish("", "zd_consume", b"delivered-async")
        .expect("publish");
    let body = c.consume_one("zd_consume").expect("consume_one");
    c.close();
    assert_eq!(String::from_utf8_lossy(&body), "delivered-async");
}

/// RabbitMQ's confirm class: after confirm.select the broker acks a
/// publish with its delivery-tag, the first being 1.
#[test]
#[ignore = "needs RabbitMQ, pika and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqp091_confirm_the_broker_acks_a_publish_after_select() {
    let node = RabbitNode::start();
    let mut c = connect(&node);
    c.queue_declare("zd_confirm", true).expect("declare");
    c.confirm_select().expect("confirm.select");
    let tag = c
        .publish_confirmed("", "zd_confirm", b"confirmed-msg")
        .expect("publish_confirmed");
    assert_eq!(tag, 1, "the first confirm carries delivery-tag 1");
    assert_eq!(
        text(c.get("zd_confirm").expect("get")).as_deref(),
        Some("confirmed-msg")
    );
    c.close();
}

/// §1.9: a publish rolled back is not queued, a committed one is.
#[test]
#[ignore = "needs RabbitMQ, pika and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqp091_1_9_a_rolled_back_publish_is_not_queued_a_committed_one_is() {
    let node = RabbitNode::start();
    let mut c = connect(&node);
    c.queue_declare("zd_tx", true).expect("declare");
    c.tx_select().expect("tx.select");
    c.publish("", "zd_tx", b"rolled-back").expect("publish");
    c.tx_rollback().expect("tx.rollback");
    assert!(c.get("zd_tx").expect("get after rollback").is_none());
    c.publish("", "zd_tx", b"committed").expect("publish");
    c.tx_commit().expect("tx.commit");
    assert_eq!(
        text(c.get("zd_tx").expect("get after commit")).as_deref(),
        Some("committed")
    );
    c.close();
}

/// §1.8 basic.reject with requeue: the message is delivered again.
#[test]
#[ignore = "needs RabbitMQ, pika and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqp091_1_8_a_message_rejected_with_requeue_is_delivered_again() {
    let node = RabbitNode::start();
    let mut c = connect(&node);
    c.queue_declare("zd_reject", true).expect("declare");
    c.publish("", "zd_reject", b"reject-me").expect("publish");
    assert_eq!(
        text(c.get_reject("zd_reject", true).expect("get_reject")).as_deref(),
        Some("reject-me")
    );
    assert_eq!(
        text(c.get("zd_reject").expect("get after requeue")).as_deref(),
        Some("reject-me")
    );
    c.close();
}

/// §1.5: channel.flow is answered with flow-ok, channel.close with
/// close-ok, and the connection stays open to close itself.
#[test]
#[ignore = "needs RabbitMQ, pika and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqp091_1_5_flow_and_close_are_answered_and_the_connection_stays() {
    let node = RabbitNode::start();
    let mut c = connect(&node);
    c.channel_flow(true).expect("channel.flow");
    c.channel_close().expect("channel.close");
    c.close();
}

/// §4.2.6.1 and the basic class's properties: pika reads the properties
/// the client published, and the client reads the ones pika published.
#[test]
#[ignore = "needs RabbitMQ, pika and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqp091_4_2_6_1_pika_and_the_client_read_each_others_properties() {
    let node = RabbitNode::start();
    let mut c = connect(&node);
    c.queue_declare("zd_props", true).expect("declare");
    let props = ContentProperties {
        content_type: Some("application/json".into()),
        delivery_mode: Some(2),
        priority: Some(4),
        app_id: Some("zerodds".into()),
        headers: Some(vec![
            ("x-trace".into(), FieldValue::str("t-123")),
            ("x-retry".into(), FieldValue::I32(2)),
            ("x-flag".into(), FieldValue::Bool(true)),
        ]),
        ..ContentProperties::default()
    };
    c.publish_with_props("", "zd_props", br#"{"k":"v"}"#, &props)
        .expect("publish_with_props");
    let seen = node.pika(
        "m, p, b = ch.basic_get(queue='zd_props', auto_ack=True)\n\
         h = p.headers\n\
         print(p.content_type, p.delivery_mode, p.priority, p.app_id, \
         h['x-trace'].decode() if isinstance(h['x-trace'], bytes) else h['x-trace'], \
         h['x-retry'], h['x-flag'], b.decode())",
    );
    assert_eq!(
        seen,
        r#"application/json 2 4 zerodds t-123 2 True {"k":"v"}"#
    );

    let _ = node.pika(
        "ch.basic_publish(exchange='', routing_key='zd_props', body=b'from-pika', \
         properties=pika.BasicProperties(content_type='text/plain', delivery_mode=2, \
         correlation_id='c-7', headers={'x-n': 5}))",
    );
    let (body, got) = c
        .get_with_props("zd_props")
        .expect("get_with_props")
        .expect("a message");
    c.close();
    assert_eq!(body, b"from-pika");
    assert_eq!(got.content_type.as_deref(), Some("text/plain"));
    assert_eq!(got.delivery_mode, Some(2));
    assert_eq!(got.correlation_id.as_deref(), Some("c-7"));
    let headers = got.headers.expect("headers");
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0].0, "x-n");
    // pika writes a Python int that fits 32 bits as `I`.
    assert_eq!(headers[0].1, FieldValue::I32(5));
}
