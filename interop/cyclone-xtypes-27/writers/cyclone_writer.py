#!/usr/bin/env python3
"""CycloneDDS writer for the #27 interop matrix.

Usage: cyclone_writer.py <final|appendable> <xcdr1|xcdr2> [seconds]

Extensibility is set on the type (default = final, matching Cyclone's own
generator default); representation is set via the DataRepresentation QoS.
Writes topic `robot` on domain 100.
"""
import os
import sys
import time
from dataclasses import dataclass

from cyclonedds.core import Policy, Qos
from cyclonedds.domain import DomainParticipant
from cyclonedds.idl import IdlStruct
from cyclonedds.idl.annotations import appendable
from cyclonedds.idl.types import uint32
from cyclonedds.pub import DataWriter, Publisher
from cyclonedds.topic import Topic

ext = sys.argv[1] if len(sys.argv) > 1 else "final"
rep = sys.argv[2] if len(sys.argv) > 2 else "xcdr1"
secs = float(sys.argv[3]) if len(sys.argv) > 3 else 30.0

if ext == "appendable":
    @appendable
    @dataclass
    class RobotType(IdlStruct, typename="Robot"):
        id: uint32 = 0
        label: uint32 = 0
else:
    @dataclass
    class RobotType(IdlStruct, typename="Robot"):
        id: uint32 = 0
        label: uint32 = 0

qos = Qos(Policy.DataRepresentation(
    use_cdrv0_representation=(rep == "xcdr1"),
    use_xcdrv2_representation=(rep == "xcdr2"),
))
dp = DomainParticipant(domain_id=int(os.environ.get("ZERODDS_DOMAIN", "100")))
tp = Topic(dp, "robot", RobotType)
w = DataWriter(Publisher(dp), tp, qos=qos)
print(f"[cyclone writer] ext={ext} rep={rep}", flush=True)
t0 = time.time()
c = 0
while time.time() - t0 < secs:
    w.write(RobotType(id=1, label=c % 1000))
    c += 1
    time.sleep(0.3)
