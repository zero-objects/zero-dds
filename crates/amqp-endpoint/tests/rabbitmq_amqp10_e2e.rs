// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Live AMQP 1.0 interop: ZeroDDS `AmqpClient` → RabbitMQ 4.0 → reference consumer.

#![cfg(feature = "std")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::uninlined_format_args
)]

use std::process::Command;

use zerodds_amqp_endpoint::client::AmqpClient;

/// Declares a queue (RabbitMQ 1.0 does not declare topology) + drains+returns
/// the next body via the `pika` 0.9.1 reference client — a cross-protocol read
/// of what the ZeroDDS 1.0 client published. Returns the consumed payload.
fn pika_declare_then_get(queue: &str) -> String {
    let py = format!(
        r#"
import pika
p = pika.PlainCredentials("zerodds","zerodds")
c = pika.BlockingConnection(pika.ConnectionParameters("localhost", credentials=p))
ch = c.channel()
ch.queue_declare(queue="{q}", durable=True)
import sys
mode = sys.argv[1] if len(sys.argv)>1 else "declare"
if mode == "get":
    _,_,body = ch.basic_get(queue="{q}", auto_ack=True)
    print(body.decode() if body else "<none>")
c.close()
"#,
        q = queue
    );
    // declare first
    Command::new("python3")
        .arg("-c")
        .arg(&py)
        .arg("declare")
        .output()
        .expect("pika declare");
    // (re)used below for get
    let out = Command::new("python3")
        .arg("-c")
        .arg(&py)
        .arg("get")
        .output()
        .expect("pika get");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
#[ignore = "needs live RabbitMQ 4.0 on localhost:5672 (codepit); set AMQP_RABBITMQ=1"]
fn zerodds_amqp10_publish_consumed_by_pika() {
    if std::env::var("AMQP_RABBITMQ").is_err() {
        eprintln!("AMQP_RABBITMQ not set — skipping live RabbitMQ 1.0 e2e");
        return;
    }
    let queue = "zd_a10";
    // Declare the queue first (drains nothing yet — just ensures it exists).
    let _ = pika_declare_then_get(queue);

    // ZeroDDS AMQP 1.0 client: SASL-PLAIN + open + attach(sender) + transfer.
    let mut client =
        AmqpClient::connect_plain("127.0.0.1:5672", "zerodds", "zerodds", "zerodds-a10")
            .expect("connect+SASL+open to RabbitMQ 1.0");
    client
        .send_to(&format!("/queues/{queue}"), b"zerodds-1.0->rmq")
        .expect("publish over AMQP 1.0");
    client.close();

    // Cross-protocol verify: consume via pika (AMQP 0.9.1).
    let got = pika_declare_then_get(queue);
    assert_eq!(
        got, "zerodds-1.0->rmq",
        "RabbitMQ should deliver the ZeroDDS-1.0 message to a 0.9.1 consumer"
    );
    eprintln!("AMQP 1.0 cross-protocol OK: ZeroDDS-1.0 → RabbitMQ → pika-0.9.1");
}

/// Publishes one message to `queue` via the pika 0.9.1 reference client.
fn pika_publish(queue: &str, payload: &str) {
    let py = format!(
        r#"
import pika
p = pika.PlainCredentials("zerodds","zerodds")
c = pika.BlockingConnection(pika.ConnectionParameters("localhost", credentials=p))
ch = c.channel()
ch.queue_declare(queue="{q}", durable=True)
ch.basic_publish(exchange="", routing_key="{q}", body=b"{body}")
c.close()
"#,
        q = queue,
        body = payload
    );
    Command::new("python3")
        .arg("-c")
        .arg(&py)
        .output()
        .expect("pika publish");
}

#[test]
#[ignore = "needs live RabbitMQ 4.0 on localhost:5672 (codepit); set AMQP_RABBITMQ=1"]
fn zerodds_amqp10_consumes_pika_published() {
    if std::env::var("AMQP_RABBITMQ").is_err() {
        eprintln!("AMQP_RABBITMQ not set — skipping live RabbitMQ 1.0 consume e2e");
        return;
    }
    let queue = "zd_a10_rx";
    // Reference publisher (AMQP 0.9.1) puts a message on the queue.
    pika_publish(queue, "pika-0.9.1->zerodds");

    // ZeroDDS AMQP 1.0 client consumes it.
    let mut client =
        AmqpClient::connect_plain("127.0.0.1:5672", "zerodds", "zerodds", "zerodds-a10rx")
            .expect("connect+SASL+open to RabbitMQ 1.0");
    let got = client
        .recv_from(&format!("/queues/{queue}"))
        .expect("receive over AMQP 1.0");
    client.close();

    assert_eq!(
        got.as_deref()
            .map(|b| String::from_utf8_lossy(b).to_string()),
        Some("pika-0.9.1->zerodds".to_string()),
        "ZeroDDS-1.0 must consume the message a 0.9.1 producer published"
    );
    eprintln!("AMQP 1.0 cross-protocol OK: pika-0.9.1 → RabbitMQ → ZeroDDS-1.0");
}
