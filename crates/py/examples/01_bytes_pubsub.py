"""Minimales Bytes-Pub/Sub.

Start im einem Terminal::

    python examples/01_bytes_pubsub.py publisher

und in einem zweiten::

    python examples/01_bytes_pubsub.py subscriber
"""

from __future__ import annotations

import argparse
import time

import zerodds


def build_participant(domain_id: int) -> zerodds.DomainParticipant:
    factory = zerodds.DomainParticipantFactory.instance()
    return factory.create_participant(domain_id)


def run_publisher(domain_id: int, topic_name: str, count: int) -> None:
    participant = build_participant(domain_id)
    topic = participant.create_bytes_topic(topic_name)
    publisher = participant.create_publisher()
    writer = publisher.create_bytes_writer(topic)

    print(f"[pub] warte auf Subscriber auf Topic '{topic_name}' ...")
    writer.wait_for_matched_subscription(1, timeout_secs=30.0)
    print("[pub] matched, sende")

    for i in range(count):
        payload = f"message-{i:04d}".encode()
        writer.write(payload)
        print(f"[pub] -> {payload!r}")
        time.sleep(0.2)


def run_subscriber(domain_id: int, topic_name: str, count: int) -> None:
    participant = build_participant(domain_id)
    topic = participant.create_bytes_topic(topic_name)
    subscriber = participant.create_subscriber()
    reader = subscriber.create_bytes_reader(topic)

    print(f"[sub] warte auf Publisher auf Topic '{topic_name}' ...")
    reader.wait_for_matched_publication(1, timeout_secs=30.0)
    print("[sub] matched, empfange")

    received = 0
    while received < count:
        reader.wait_for_data(timeout_secs=10.0)
        for payload in reader.take():
            print(f"[sub] <- {payload!r}")
            received += 1
            if received >= count:
                break


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("role", choices=["publisher", "subscriber"])
    parser.add_argument("--domain", type=int, default=0)
    parser.add_argument("--topic", type=str, default="Chatter")
    parser.add_argument("--count", type=int, default=10)
    args = parser.parse_args()

    if args.role == "publisher":
        run_publisher(args.domain, args.topic, args.count)
    else:
        run_subscriber(args.domain, args.topic, args.count)


if __name__ == "__main__":
    main()
