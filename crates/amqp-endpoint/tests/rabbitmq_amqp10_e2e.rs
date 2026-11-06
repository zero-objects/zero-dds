// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AMQP 1.0 against RabbitMQ: the ZeroDDS client `AmqpClient` on one side,
//! a RabbitMQ node of each test alone on the other, and pika (AMQP 0-9-1)
//! as the reference client that reads and writes the queue.
//!
//! The tests need `rabbitmq-server` and python3 with pika; they run in the
//! integration pillar's job `integration-bridges-vendor-amqp`, where a missing
//! program fails them:
//!
//! ```bash
//! cargo nextest run -p zerodds-amqp-endpoint --test rabbitmq_amqp10_e2e --run-ignored all
//! ```

#![cfg(all(feature = "std", unix))]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::uninlined_format_args
)]

#[path = "../../amqp-0-9-1/tests/support/rabbitmq.rs"]
mod rabbitmq;

use rabbitmq::RabbitNode;
use zerodds_amqp_endpoint::client::AmqpClient;

fn connect(node: &RabbitNode, container: &str) -> AmqpClient {
    AmqpClient::connect_plain(node.addr(), "guest", "guest", container)
        .expect("SASL PLAIN, open and begin with RabbitMQ")
}

/// pika takes one message of `queue` within ten seconds; `<none>` if none
/// came.
fn pika_take(node: &RabbitNode, queue: &str) -> String {
    node.pika(&format!(
        "res = []\n\
         def got(ch_, m, p, b):\n\
         \x20   res.append(b.decode())\n\
         \x20   ch.stop_consuming()\n\
         ch.basic_consume(queue='{queue}', on_message_callback=got, auto_ack=True)\n\
         c.call_later(10, ch.stop_consuming)\n\
         ch.start_consuming()\n\
         print(res[0] if res else '<none>')"
    ))
}

/// Part 1 §0.2 and Part 5 §5.3: SASL PLAIN, open, begin, a sender attach
/// and a transfer; pika (AMQP 0-9-1) reads the message from the queue.
#[test]
#[ignore = "needs RabbitMQ and pika; runs in integration-bridges-vendor-amqp"]
fn amqp10_0_2_a_message_the_client_sends_is_read_by_pika() {
    let node = RabbitNode::start();
    let _ = node.pika("ch.queue_declare(queue='zd_a10', durable=True)");
    let mut client = connect(&node, "zerodds-a10");
    client
        .send_to("/queues/zd_a10", b"zerodds-1.0->rmq")
        .expect("publish over AMQP 1.0");
    client.close();
    assert_eq!(pika_take(&node, "zd_a10"), "zerodds-1.0->rmq");
}

/// Part 1 §0.2 and Part 2 §2.6.7: a receiver attach and link credit; the
/// client reads the message pika published.
#[test]
#[ignore = "needs RabbitMQ and pika; runs in integration-bridges-vendor-amqp"]
fn amqp10_0_2_the_client_receives_what_pika_published() {
    let node = RabbitNode::start();
    let _ = node.pika(
        "ch.queue_declare(queue='zd_a10_rx', durable=True)\n\
         ch.basic_publish(exchange='', routing_key='zd_a10_rx', body=b'pika-0.9.1->zerodds')",
    );
    let mut client = connect(&node, "zerodds-a10rx");
    let got = client
        .recv_from("/queues/zd_a10_rx")
        .expect("receive over AMQP 1.0");
    client.close();
    assert_eq!(got.as_deref(), Some(&b"pika-0.9.1->zerodds"[..]));
}

/// Part 3 §3.4.2: `send_to_accepted` returns once the broker settled the
/// delivery as `accepted`, so the queue holds the message by then; pika
/// reads each one straight afterwards.
#[test]
#[ignore = "needs RabbitMQ and pika; runs in integration-bridges-vendor-amqp"]
fn amqp10_3_4_2_an_accepted_send_is_in_the_queue_when_it_returns() {
    let node = RabbitNode::start();
    let _ = node.pika("ch.queue_declare(queue='zd_accepted', durable=True)");
    let mut client = connect(&node, "zd-acc");
    for n in 0..5 {
        let body = format!("accepted-{n}");
        client
            .send_to_accepted("/queues/zd_accepted", body.as_bytes())
            .unwrap();
        let depth = node.pika(
            "q = ch.queue_declare(queue='zd_accepted', durable=True, passive=True)\n\
             print(q.method.message_count)",
        );
        assert_eq!(
            depth,
            (n + 1).to_string(),
            "send {n} returned before the queue held it"
        );
    }
    client.close();
}
