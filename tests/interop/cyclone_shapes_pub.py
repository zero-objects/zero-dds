#!/usr/bin/env python3
"""Cyclone-DDS Python-basierter ShapesDemo-Publisher.

Setzt denselben Topic-Namen + Type-Namen wie ZeroDDS's
`hello_dds::interop::ShapeType`. Damit kann ein ZeroDDS-Subscriber
die Samples dieses Cyclone-Publishers empfangen — als Cross-Vendor-
Wire-Beweis fuer WP 2.1 Stufe 1.

Benoetigt: `pip install cyclonedds` (mitgelieferte C-Lib). Linux mit
funktionierendem IP-Multicast auf loopback oder lokalem Subnet.

CLI:
    python3 cyclone_shapes_pub.py [topic=Square] [color=RED] [domain=0]
"""

from __future__ import annotations

import math
import sys
import time
from dataclasses import dataclass

from cyclonedds.domain import DomainParticipant  # type: ignore[import-not-found]
from cyclonedds.idl import IdlStruct  # type: ignore[import-not-found]
from cyclonedds.idl.annotations import key  # type: ignore[import-not-found]
from cyclonedds.idl.types import bounded_str, int32  # type: ignore[import-not-found]
from cyclonedds.pub import DataWriter, Publisher  # type: ignore[import-not-found]
from cyclonedds.topic import Topic  # type: ignore[import-not-found]


@dataclass
class ShapeType(IdlStruct, typename="ShapeType"):
    """Spec-kompatibel mit RTI/Cyclone/Fast-DDS ShapesDemo.

    IDL:
        struct ShapeType {
            @key string<128> color;
            int32 x, y, shapesize;
        };
    """

    color: key[bounded_str[128]] = ""
    x: int32 = 0
    y: int32 = 0
    shapesize: int32 = 0


def main() -> int:
    topic_name = sys.argv[1] if len(sys.argv) > 1 else "Square"
    color = sys.argv[2] if len(sys.argv) > 2 else "RED"
    domain_id = int(sys.argv[3]) if len(sys.argv) > 3 else 0

    participant = DomainParticipant(domain_id=domain_id)
    topic = Topic(participant, topic_name, ShapeType)
    publisher = Publisher(participant)
    writer = DataWriter(publisher, topic)

    print(
        f"[cyclone-pub] Topic={topic_name} Color={color} Domain={domain_id} — Ctrl-C to stop",
        flush=True,
    )

    t = 0.0
    try:
        while True:
            x = int(120 + 80 * math.sin(t))
            y = int(135 + 90 * math.cos(t * 1.3))
            sample = ShapeType(color=color, x=x, y=y, shapesize=30)
            writer.write(sample)
            print(f"  -> color={color} x={x} y={y} size=30", flush=True)
            t += 0.15
            time.sleep(0.1)
    except KeyboardInterrupt:
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
