// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Live AMQP 0.9.1 interop: ZeroDDS `Amqp091Client` ↔ RabbitMQ 4.0 (+ refs).

#![cfg(feature = "std")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::uninlined_format_args
)]

use std::process::Command;

use zerodds_amqp_0_9_1::client::Amqp091Client;
use zerodds_amqp_0_9_1::method::ContentProperties;
use zerodds_amqp_0_9_1::types::FieldValue;

fn connect() -> Amqp091Client {
    Amqp091Client::connect("127.0.0.1:5672", "zerodds", "zerodds", "/")
        .expect("AMQP 0.9.1 handshake to RabbitMQ")
}

#[test]
#[ignore = "needs live RabbitMQ 4.0 on localhost:5672 (codepit); set AMQP_RABBITMQ=1"]
fn zerodds_amqp091_self_roundtrip() {
    if std::env::var("AMQP_RABBITMQ").is_err() {
        eprintln!("AMQP_RABBITMQ not set — skipping AMQP 0.9.1 e2e");
        return;
    }
    let queue = "zd_b_self";
    let mut c = connect();
    let name = c.queue_declare(queue, true).expect("queue.declare");
    assert_eq!(name, queue, "broker echoes the declared queue name");
    c.publish("", queue, b"zerodds-0.9.1-self")
        .expect("basic.publish");
    let got = c.get(queue).expect("basic.get");
    c.close();
    assert_eq!(
        got.as_deref()
            .map(|b| String::from_utf8_lossy(b).to_string()),
        Some("zerodds-0.9.1-self".to_string()),
        "ZeroDDS 0.9.1 publish→get self roundtrip"
    );
    eprintln!("AMQP 0.9.1 self roundtrip OK (ZeroDDS ↔ RabbitMQ)");
}

#[test]
#[ignore = "needs live RabbitMQ 4.0 on localhost:5672 (codepit); set AMQP_RABBITMQ=1"]
fn zerodds_amqp091_publish_consumed_by_proton_10() {
    if std::env::var("AMQP_RABBITMQ").is_err() {
        return;
    }
    let queue = "zd_b_to10";
    let mut c = connect();
    c.queue_declare(queue, true).expect("declare");
    c.publish("", queue, b"zerodds-0.9.1->1.0")
        .expect("publish");
    c.close();

    // Cross-protocol: consume via qpid-proton (AMQP 1.0).
    let py = format!(
        r#"
from proton.utils import BlockingConnection
conn = BlockingConnection("amqp://zerodds:zerodds@localhost:5672")
rcv = conn.create_receiver("/queues/{q}")
m = rcv.receive(timeout=5)
b = m.body
print(b.decode() if isinstance(b, (bytes, bytearray)) else b)
rcv.accept(); conn.close()
"#,
        q = queue
    );
    let out = Command::new("python3")
        .arg("-c")
        .arg(&py)
        .output()
        .expect("proton get");
    let got = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert_eq!(
        got, "zerodds-0.9.1->1.0",
        "a 1.0 consumer must read the 0.9.1-published message"
    );
    eprintln!("AMQP cross-protocol OK: ZeroDDS-0.9.1 → RabbitMQ → proton-1.0");
}

#[test]
#[ignore = "needs live RabbitMQ 4.0 on localhost:5672 (codepit); set AMQP_RABBITMQ=1"]
fn zerodds_amqp091_consumes_pika_published() {
    if std::env::var("AMQP_RABBITMQ").is_err() {
        return;
    }
    let queue = "zd_b_rx";
    // Reference publisher (pika 0.9.1) puts a message on the queue.
    let py = format!(
        r#"
import pika
p = pika.PlainCredentials("zerodds","zerodds")
c = pika.BlockingConnection(pika.ConnectionParameters("localhost", credentials=p))
ch = c.channel()
ch.queue_declare(queue="{q}", durable=True)
ch.basic_publish(exchange="", routing_key="{q}", body=b"pika->zerodds-0.9.1")
c.close()
"#,
        q = queue
    );
    Command::new("python3")
        .arg("-c")
        .arg(&py)
        .output()
        .expect("pika publish");

    let mut c = connect();
    let got = c.get(queue).expect("basic.get");
    c.close();
    assert_eq!(
        got.as_deref()
            .map(|b| String::from_utf8_lossy(b).to_string()),
        Some("pika->zerodds-0.9.1".to_string()),
        "ZeroDDS 0.9.1 must consume a pika-published message"
    );
    eprintln!("AMQP 0.9.1 OK: pika → RabbitMQ → ZeroDDS-0.9.1");
}

#[test]
#[ignore = "needs live RabbitMQ 4.0 on localhost:5672 (codepit); set AMQP_RABBITMQ=1"]
fn zerodds_amqp091_exchange_routing() {
    if std::env::var("AMQP_RABBITMQ").is_err() {
        return;
    }
    // Declare a topic exchange, bind a queue to it, publish via the exchange,
    // and read the routed message back.
    let exchange = "zd_b_ex";
    let queue = "zd_b_ex_q";
    let key = "zd.key";
    let mut c = connect();
    c.exchange_declare(exchange, "topic", true)
        .expect("exchange.declare");
    c.queue_declare(queue, true).expect("queue.declare");
    c.queue_bind(queue, exchange, key).expect("queue.bind");
    c.publish(exchange, key, b"routed-via-exchange")
        .expect("publish to exchange");
    let got = c.get(queue).expect("basic.get");
    assert_eq!(
        got.as_deref()
            .map(|b| String::from_utf8_lossy(b).to_string()),
        Some("routed-via-exchange".to_string()),
        "topic-exchange routing must deliver to the bound queue"
    );
    // Tidy up: unbind, purge, delete, drop exchange.
    c.queue_unbind(queue, exchange, key).expect("queue.unbind");
    let _ = c.queue_purge(queue).expect("queue.purge");
    let _ = c.queue_delete(queue).expect("queue.delete");
    c.exchange_delete(exchange, false).expect("exchange.delete");
    c.close();
    eprintln!("AMQP 0.9.1 OK: exchange.declare/bind/unbind + queue.purge/delete");
}

#[test]
#[ignore = "needs live RabbitMQ 4.0 on localhost:5672 (codepit); set AMQP_RABBITMQ=1"]
fn zerodds_amqp091_async_consume() {
    if std::env::var("AMQP_RABBITMQ").is_err() {
        return;
    }
    // Exercise the async push path: basic.qos + basic.consume → basic.deliver
    // → ack → basic.cancel, distinct from synchronous basic.get.
    let queue = "zd_b_consume";
    let mut c = connect();
    c.queue_declare(queue, true).expect("declare");
    let _ = c.queue_purge(queue).expect("purge");
    c.qos(10).expect("basic.qos");
    c.publish("", queue, b"delivered-async").expect("publish");
    let body = c.consume_one(queue).expect("consume_one");
    c.close();
    assert_eq!(
        String::from_utf8_lossy(&body),
        "delivered-async",
        "basic.consume must asynchronously deliver the message"
    );
    eprintln!("AMQP 0.9.1 OK: basic.qos/consume/deliver/cancel");
}

#[test]
#[ignore = "needs live RabbitMQ 4.0 on localhost:5672 (codepit); set AMQP_RABBITMQ=1"]
fn zerodds_amqp091_publisher_confirms() {
    if std::env::var("AMQP_RABBITMQ").is_err() {
        return;
    }
    // confirm.select then publish_confirmed must block until the broker acks.
    let queue = "zd_b_confirm";
    let mut c = connect();
    c.queue_declare(queue, true).expect("declare");
    let _ = c.queue_purge(queue).expect("purge");
    c.confirm_select().expect("confirm.select");
    let tag = c
        .publish_confirmed("", queue, b"confirmed-msg")
        .expect("publish_confirmed");
    assert!(tag >= 1, "first confirm delivery-tag is >= 1, got {tag}");
    let got = c.get(queue).expect("get");
    c.close();
    assert_eq!(
        got.as_deref()
            .map(|b| String::from_utf8_lossy(b).to_string()),
        Some("confirmed-msg".to_string()),
    );
    eprintln!("AMQP 0.9.1 OK: confirm.select + publisher-confirm (basic.ack tag={tag})");
}

#[test]
#[ignore = "needs live RabbitMQ 4.0 on localhost:5672 (codepit); set AMQP_RABBITMQ=1"]
fn zerodds_amqp091_transactions() {
    if std::env::var("AMQP_RABBITMQ").is_err() {
        return;
    }
    // A rolled-back tx must NOT deliver; a committed tx must.
    let queue = "zd_b_tx";
    let mut c = connect();
    c.queue_declare(queue, true).expect("declare");
    let _ = c.queue_purge(queue).expect("purge");

    c.tx_select().expect("tx.select");
    c.publish("", queue, b"rolled-back").expect("publish");
    c.tx_rollback().expect("tx.rollback");
    assert!(
        c.get(queue).expect("get after rollback").is_none(),
        "a rolled-back publish must not be queued"
    );

    c.publish("", queue, b"committed").expect("publish");
    c.tx_commit().expect("tx.commit");
    let got = c.get(queue).expect("get after commit");
    c.close();
    assert_eq!(
        got.as_deref()
            .map(|b| String::from_utf8_lossy(b).to_string()),
        Some("committed".to_string()),
        "a committed publish must be queued"
    );
    eprintln!("AMQP 0.9.1 OK: tx.select/commit/rollback");
}

#[test]
#[ignore = "needs live RabbitMQ 4.0 on localhost:5672 (codepit); set AMQP_RABBITMQ=1"]
fn zerodds_amqp091_reject_requeue() {
    if std::env::var("AMQP_RABBITMQ").is_err() {
        return;
    }
    // basic.reject with requeue=true must put the message back for re-delivery.
    let queue = "zd_b_reject";
    let mut c = connect();
    c.queue_declare(queue, true).expect("declare");
    let _ = c.queue_purge(queue).expect("purge");
    c.publish("", queue, b"reject-me").expect("publish");

    let first = c.get_reject(queue, true).expect("get_reject(requeue)");
    assert_eq!(
        first
            .as_deref()
            .map(|b| String::from_utf8_lossy(b).to_string()),
        Some("reject-me".to_string()),
    );
    // After requeue the message is available again.
    let second = c.get(queue).expect("get after requeue");
    c.close();
    assert_eq!(
        second
            .as_deref()
            .map(|b| String::from_utf8_lossy(b).to_string()),
        Some("reject-me".to_string()),
        "a requeued reject must be redelivered"
    );
    eprintln!("AMQP 0.9.1 OK: basic.reject + requeue");
}

#[test]
#[ignore = "needs live RabbitMQ 4.0 on localhost:5672 (codepit); set AMQP_RABBITMQ=1"]
fn zerodds_amqp091_channel_flow_and_close() {
    if std::env::var("AMQP_RABBITMQ").is_err() {
        return;
    }
    // channel.flow(true) is a no-op resume the broker echoes; channel.close
    // gracefully tears down the channel while the connection stays open.
    let mut c = connect();
    c.channel_flow(true).expect("channel.flow");
    c.channel_close().expect("channel.close");
    c.close();
    eprintln!("AMQP 0.9.1 OK: channel.flow + channel.close");
}

#[test]
#[ignore = "needs live RabbitMQ 4.0 on localhost:5672 (codepit); set AMQP_RABBITMQ=1"]
fn zerodds_amqp091_content_properties() {
    if std::env::var("AMQP_RABBITMQ").is_err() {
        return;
    }
    // Publish with content-type + persistent delivery-mode + typed headers,
    // and verify the broker round-trips every property back.
    let queue = "zd_b_props";
    let mut c = connect();
    c.queue_declare(queue, true).expect("declare");
    let _ = c.queue_purge(queue).expect("purge");

    let props = ContentProperties {
        content_type: Some("application/json".into()),
        delivery_mode: Some(2), // persistent
        priority: Some(4),
        app_id: Some("zerodds".into()),
        headers: Some(vec![
            ("x-trace".into(), FieldValue::str("t-123")),
            ("x-retry".into(), FieldValue::I32(2)),
            ("x-flag".into(), FieldValue::Bool(true)),
        ]),
        ..ContentProperties::default()
    };
    c.publish_with_props("", queue, br#"{"k":"v"}"#, &props)
        .expect("publish_with_props");

    let (body, got) = c
        .get_with_props(queue)
        .expect("get_with_props")
        .expect("a message must be present");
    c.close();
    assert_eq!(String::from_utf8_lossy(&body), r#"{"k":"v"}"#);
    assert_eq!(got.content_type.as_deref(), Some("application/json"));
    assert_eq!(got.delivery_mode, Some(2));
    assert_eq!(got.priority, Some(4));
    assert_eq!(got.app_id.as_deref(), Some("zerodds"));
    let headers = got.headers.expect("headers must be present");
    assert!(headers.contains(&("x-trace".to_string(), FieldValue::str("t-123"))));
    assert!(headers.contains(&("x-retry".to_string(), FieldValue::I32(2))));
    assert!(headers.contains(&("x-flag".to_string(), FieldValue::Bool(true))));
    eprintln!("AMQP 0.9.1 OK: content-properties + typed headers round-trip");
}
