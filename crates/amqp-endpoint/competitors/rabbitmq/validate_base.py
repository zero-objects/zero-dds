#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Validates the RabbitMQ interop base with reference clients for BOTH protocols.

Proves the broker (provisioned by setup_rabbitmq.sh) backs:
  * AMQP 0.9.1 via `pika`        — classic class/method protocol.
  * AMQP 1.0   via `qpid-proton` — RabbitMQ 4.0 native, v2 address format.

Both publish + consume a roundtrip on their own queue. Exit 0 on success.
This is the reference baseline the ZeroDDS AMQP clients (Path A = 1.0,
Path B = 0.9.1) are tested against.
"""
import sys

HOST = "localhost"
USER = "zerodds"
PASS = "zerodds"


def check_091() -> str:
    import pika

    conn = pika.BlockingConnection(
        pika.ConnectionParameters(HOST, credentials=pika.PlainCredentials(USER, PASS))
    )
    ch = conn.channel()
    ch.queue_declare(queue="zd091")
    ch.basic_publish(exchange="", routing_key="zd091", body=b"hello-0.9.1")
    _, _, body = ch.basic_get(queue="zd091", auto_ack=True)
    conn.close()
    return body.decode() if body else "<none>"


def check_10() -> str:
    import pika  # declare the queue via 0.9.1 first (shared broker entity)
    from proton import Message
    from proton.utils import BlockingConnection

    c = pika.BlockingConnection(
        pika.ConnectionParameters(HOST, credentials=pika.PlainCredentials(USER, PASS))
    )
    c.channel().queue_declare(queue="zd10", durable=True)
    c.close()

    # RabbitMQ 4.0 AMQP 1.0 v2 address format: /queues/<name>.
    conn = BlockingConnection(f"amqp://{USER}:{PASS}@{HOST}:5672")
    snd = conn.create_sender("/queues/zd10")
    snd.send(Message(body="hello-1.0"))
    rcv = conn.create_receiver("/queues/zd10")
    msg = rcv.receive(timeout=5)
    body = msg.body
    rcv.accept()
    conn.close()
    return body


def main() -> int:
    ok = True
    r091 = check_091()
    print(f"AMQP 0.9.1 (pika)       roundtrip: {r091!r}")
    ok &= r091 == "hello-0.9.1"

    r10 = check_10()
    print(f"AMQP 1.0   (qpid-proton) roundtrip: {r10!r}")
    ok &= r10 == "hello-1.0"

    print("BASE OK" if ok else "BASE FAILED")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
