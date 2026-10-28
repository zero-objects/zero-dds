#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Cyclone DDS (Python binding) publishes `probe::Sample` with its own
TypeInformation, waits for a reader, writes the samples and their expected
values as JSON lines to `--expect`."""

import argparse
import json
import time
from dataclasses import dataclass

from cyclonedds.core import Policy, Qos
from cyclonedds.domain import DomainParticipant
from cyclonedds.idl import IdlEnum, IdlStruct
from cyclonedds.idl.annotations import appendable, key
from cyclonedds.idl.types import array, float64, int16, int32, sequence
from cyclonedds.pub import DataWriter
from cyclonedds.topic import Topic


class Color(IdlEnum, typename="probe::Color"):
    RED = 0
    GREEN = 1
    BLUE = 2


@dataclass
@appendable
class Inner(IdlStruct, typename="probe::Inner"):
    x: int32
    y: float64


@dataclass
class Sample(IdlStruct, typename="probe::Sample"):
    id: int32
    key("id")
    label: str
    values: sequence[int16]
    inner: Inner
    color: Color
    triple: array[int32, 3]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--domain", type=int, required=True)
    parser.add_argument("--topic", default="ProbeTopic")
    parser.add_argument("--count", type=int, default=3)
    parser.add_argument("--expect", required=True)
    args = parser.parse_args()

    participant = DomainParticipant(args.domain)
    topic = Topic(participant, args.topic, Sample)
    qos = Qos(Policy.Reliability.Reliable(max_blocking_time=1_000_000_000),
              Policy.History.KeepAll)
    writer = DataWriter(participant, topic, qos=qos)

    deadline = time.time() + 30
    while not writer.get_matched_subscriptions() and time.time() < deadline:
        time.sleep(0.05)
    time.sleep(0.5)

    samples = [
        Sample(id=i, label=f"sample-{i} ä", values=[i, -i, 300 + i],
               inner=Inner(x=10 * i, y=i + 0.25), color=Color(i % 3),
               triple=[i, i + 1, i + 2])
        for i in range(1, args.count + 1)
    ]
    with open(args.expect, "w", encoding="utf-8") as out:
        for s in samples:
            writer.write(s)
            out.write(json.dumps({
                "id": s.id, "label": s.label, "values": list(s.values),
                "inner": {"x": s.inner.x, "y": s.inner.y},
                "color": s.color.name, "triple": list(s.triple),
            }) + "\n")
    time.sleep(3)


if __name__ == "__main__":
    main()
