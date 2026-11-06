#!/usr/bin/env python3
"""The foreign side of the AMQP bridge scenario: pika, an AMQP 0-9-1 client,
against the RabbitMQ broker the bridge speaks AMQP 1.0 to — two protocols and
two implementations this repository did not write meet in the broker's
queues.

    amqp_peer.py <port> <out-queue> <count> <in-queue> <hex>...

Declares both queues and prints `declared`; on the line `consume` from its
standard input, prints `message <content-type> <hex>` for each of the first
<count> messages of <out-queue>; then publishes each <hex> to <in-queue>
with the content type DDS-AMQP 1.0 §8.1.1 gives XCDR2 and prints
`published`; then waits until its standard input ends.
"""
import sys

import pika


def main(argv):
    port, out_queue, count, in_queue, payloads = int(argv[1]), argv[2], int(argv[3]), argv[4], argv[5:]
    params = pika.ConnectionParameters(host="127.0.0.1", port=port,
                                       credentials=pika.PlainCredentials("guest", "guest"),
                                       connection_attempts=1, blocked_connection_timeout=10)
    connection = pika.BlockingConnection(params)
    channel = connection.channel()
    for queue in (out_queue, in_queue):
        channel.queue_declare(queue=queue, durable=True)
    print("declared", flush=True)
    while sys.stdin.readline().strip() != "consume":
        pass
    got = 0
    for method, properties, body in channel.consume(out_queue, inactivity_timeout=20):
        if method is None:
            print("timeout", flush=True)
            break
        channel.basic_ack(method.delivery_tag)
        print(f"message {properties.content_type} {body.hex()}", flush=True)
        got += 1
        if got == count:
            break
    channel.cancel()
    for payload in payloads:
        channel.basic_publish(exchange="", routing_key=in_queue, body=bytes.fromhex(payload),
                              properties=pika.BasicProperties(content_type="application/vnd.dds.xcdr2",
                                                              delivery_mode=2))
    print("published", flush=True)
    sys.stdin.read()
    connection.close()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
