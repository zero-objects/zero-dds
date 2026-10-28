// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Capstone interop: ZeroDDS's own **AMQP 1.0** and **AMQP 0.9.1** stacks
//! exchange messages through RabbitMQ 4.0 — both producer and consumer are
//! ZeroDDS, on different protocols, via the same broker queue.

#![cfg(feature = "std")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

use zerodds_amqp_0_9_1::client::Amqp091Client;
use zerodds_amqp_endpoint::client::AmqpClient;

fn rabbit() -> bool {
    std::env::var("AMQP_RABBITMQ").is_ok()
}

/// Read `queue` until a message is there, or give up.
///
/// `AmqpClient::send_to` writes the transfer pre-settled and detaches, so
/// AMQP 1.0 hands the sender nothing to wait on: when it returns, the
/// broker may not yet hold the message. Reading once straight afterwards
/// assumed a latency, and on any link slower than loopback the assumption
/// was wrong — the message arrived on the next read, 50 ms later, and the
/// test called the stack broken. Waiting on the condition states what the
/// single read left implicit. `AmqpClient::send_to_accepted` is the send
/// that does wait, and the last test here reads once after it.
fn get_until(client: &mut Amqp091Client, queue: &str) -> Option<Vec<u8>> {
    for _ in 0..100 {
        if let Some(body) = client.get(queue).unwrap() {
            return Some(body);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    None
}

#[test]
#[ignore = "needs live RabbitMQ 4.0 on localhost:5672 (Linux test host); set AMQP_RABBITMQ=1"]
fn zerodds_10_publishes_zerodds_091_consumes() {
    if !rabbit() {
        eprintln!("AMQP_RABBITMQ not set — skipping cross-stack e2e");
        return;
    }
    let queue = "zd_x_10to091";
    // Declare the queue via the 0.9.1 client (topology), then publish via 1.0.
    let mut b = Amqp091Client::connect("127.0.0.1:5672", "zerodds", "zerodds", "/").unwrap();
    b.queue_declare(queue, true).unwrap();

    let mut a =
        AmqpClient::connect_plain("127.0.0.1:5672", "zerodds", "zerodds", "zd-x-10").unwrap();
    a.send_to(&format!("/queues/{queue}"), b"zerodds10->zerodds091")
        .unwrap();
    a.close();

    let got = get_until(&mut b, queue);
    b.close();
    assert_eq!(
        got.as_deref()
            .map(|x| String::from_utf8_lossy(x).to_string()),
        Some("zerodds10->zerodds091".to_string()),
        "ZeroDDS-0.9.1 must consume what ZeroDDS-1.0 published"
    );
    eprintln!("cross-stack OK: ZeroDDS-1.0 → RabbitMQ → ZeroDDS-0.9.1");
}

#[test]
#[ignore = "needs live RabbitMQ 4.0 on localhost:5672 (Linux test host); set AMQP_RABBITMQ=1"]
fn zerodds_091_publishes_zerodds_10_consumes() {
    if !rabbit() {
        return;
    }
    let queue = "zd_x_091to10";
    let mut b = Amqp091Client::connect("127.0.0.1:5672", "zerodds", "zerodds", "/").unwrap();
    b.queue_declare(queue, true).unwrap();
    b.publish("", queue, b"zerodds091->zerodds10").unwrap();
    b.close();

    let mut a =
        AmqpClient::connect_plain("127.0.0.1:5672", "zerodds", "zerodds", "zd-x-091").unwrap();
    let got = a.recv_from(&format!("/queues/{queue}")).unwrap();
    a.close();
    assert_eq!(
        got.as_deref()
            .map(|x| String::from_utf8_lossy(x).to_string()),
        Some("zerodds091->zerodds10".to_string()),
        "ZeroDDS-1.0 must consume what ZeroDDS-0.9.1 published"
    );
    eprintln!("cross-stack OK: ZeroDDS-0.9.1 → RabbitMQ → ZeroDDS-1.0");
}

/// `send_to_accepted` returns once RabbitMQ settled the delivery as
/// `accepted` (AMQP 1.0 §3.4.2), so the queue holds the message by then:
/// one read straight afterwards finds it, where the pre-settled `send_to`
/// needed `get_until`.
#[test]
#[ignore = "needs live RabbitMQ 4.0 on localhost:5672 (Linux test host); set AMQP_RABBITMQ=1"]
fn an_accepted_send_is_in_the_queue_when_it_returns() {
    if !rabbit() {
        return;
    }
    let queue = "zd_x_accepted";
    let mut b = Amqp091Client::connect("127.0.0.1:5672", "zerodds", "zerodds", "/").unwrap();
    b.queue_declare(queue, true).unwrap();
    while b.get(queue).unwrap().is_some() {}

    let mut a =
        AmqpClient::connect_plain("127.0.0.1:5672", "zerodds", "zerodds", "zd-x-acc").unwrap();
    for n in 0..20 {
        let body = format!("accepted-{n}");
        a.send_to_accepted(&format!("/queues/{queue}"), body.as_bytes())
            .unwrap();
        assert_eq!(
            b.get(queue).unwrap().as_deref(),
            Some(body.as_bytes()),
            "send {n} returned before the queue held the message"
        );
    }
    a.close();
    b.close();
}
