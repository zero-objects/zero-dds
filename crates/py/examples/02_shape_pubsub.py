"""ShapeType cross-vendor interop.

Counterpart to OMG ShapesDemo (Cyclone/Fast-DDS/RTI). Start in one
terminal::

    python examples/02_shape_pubsub.py publisher

and in another — whether zerodds, cyclonedds-python or
rticonnextdds::

    python examples/02_shape_pubsub.py subscriber
"""

from __future__ import annotations

import argparse
import time

import zerodds


def run_publisher(
    domain_id: int,
    topic_name: str,
    color: str,
    count: int,
) -> None:
    factory = zerodds.DomainParticipantFactory.instance()
    participant = factory.create_participant(domain_id)
    topic = participant.create_shape_topic(topic_name)
    publisher = participant.create_publisher()
    writer = publisher.create_shape_writer(topic)

    print(f"[pub] '{topic_name}' warte auf Subscriber ...")
    writer.wait_for_matched_subscription(1, timeout_secs=30.0)
    print("[pub] matched, sende")

    x = 50
    y = 50
    direction = 1
    for i in range(count):
        shape = zerodds.Shape(color=color, x=x, y=y, shapesize=30)
        writer.write(shape)
        print(f"[pub] {i:03d}: {shape}")
        x += 5 * direction
        if x >= 240 or x <= 10:
            direction *= -1
        time.sleep(0.1)


def run_subscriber(domain_id: int, topic_name: str, count: int) -> None:
    factory = zerodds.DomainParticipantFactory.instance()
    participant = factory.create_participant(domain_id)
    topic = participant.create_shape_topic(topic_name)
    subscriber = participant.create_subscriber()
    reader = subscriber.create_shape_reader(topic)

    print(f"[sub] '{topic_name}' warte auf Publisher ...")
    reader.wait_for_matched_publication(1, timeout_secs=30.0)
    print("[sub] matched, empfange")

    received = 0
    while received < count:
        reader.wait_for_data(timeout_secs=10.0)
        for shape in reader.take():
            print(f"[sub] {received:03d}: {shape}")
            received += 1
            if received >= count:
                break


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("role", choices=["publisher", "subscriber"])
    parser.add_argument("--domain", type=int, default=0)
    parser.add_argument(
        "--topic",
        type=str,
        default="Square",
        choices=["Square", "Circle", "Triangle"],
    )
    parser.add_argument("--color", type=str, default="BLUE")
    parser.add_argument("--count", type=int, default=50)
    args = parser.parse_args()

    if args.role == "publisher":
        run_publisher(args.domain, args.topic, args.color, args.count)
    else:
        run_subscriber(args.domain, args.topic, args.count)


if __name__ == "__main__":
    main()
