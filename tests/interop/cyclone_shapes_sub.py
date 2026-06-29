#!/usr/bin/env python3
"""Cyclone-DDS Python-basierter ShapesDemo-Subscriber.

Gegenstueck zu `cyclone_shapes_pub.py`. Empfaengt ShapeType-Samples
auf demselben Topic-/Type-Namen wie ZeroDDS und prueft damit den
Cross-Vendor-Wire-Pfad.

CLI:
    python3 cyclone_shapes_sub.py [topic=Square] [domain=0]
"""


import sys
import time
from dataclasses import dataclass

from cyclonedds.domain import DomainParticipant  # type: ignore[import-not-found]
from cyclonedds.idl import IdlStruct  # type: ignore[import-not-found]
from cyclonedds.idl.annotations import key  # type: ignore[import-not-found]
from cyclonedds.idl.types import bounded_str, int32  # type: ignore[import-not-found]
from cyclonedds.sub import DataReader, Subscriber  # type: ignore[import-not-found]
from cyclonedds.topic import Topic  # type: ignore[import-not-found]


@dataclass
class ShapeType(IdlStruct, typename="ShapeType"):
    color: bounded_str[128] = ""
    x: int32 = 0
    y: int32 = 0
    shapesize: int32 = 0
    key("color")


def main() -> int:
    topic_name = sys.argv[1] if len(sys.argv) > 1 else "Square"
    domain_id = int(sys.argv[2]) if len(sys.argv) > 2 else 0

    participant = DomainParticipant(domain_id=domain_id)
    topic = Topic(participant, topic_name, ShapeType)
    subscriber = Subscriber(participant)
    reader = DataReader(subscriber, topic)

    print(
        f"[cyclone-sub] Topic={topic_name} Domain={domain_id} — Ctrl-C to stop",
        flush=True,
    )

    try:
        while True:
            samples = reader.take(N=100)
            for s in samples:
                # `s.sample_info` ist cyclonedds-python-spezifisch;
                # auf einigen Versionen ist es direkt auf der Instance.
                data = s if not hasattr(s, "data") else s.data
                print(
                    f"  <- color={data.color:>8} x={data.x:4} y={data.y:4} size={data.shapesize}",
                    flush=True,
                )
            time.sleep(0.1)
    except KeyboardInterrupt:
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
