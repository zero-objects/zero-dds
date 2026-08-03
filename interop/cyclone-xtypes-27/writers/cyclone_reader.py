#!/usr/bin/env python3
"""CycloneDDS reader for the reverse leg of the #27 interop matrix
(ZeroDDS writer -> CycloneDDS reader).

Usage: cyclone_reader.py <final|appendable> [seconds]

Counts successfully received samples on topic `robot`, domain 100, and prints
`CYCLONE_RESULT samples=<n>`.
"""
import os
import sys
import time
from dataclasses import dataclass

from cyclonedds.domain import DomainParticipant
from cyclonedds.idl import IdlStruct
from cyclonedds.idl.annotations import appendable
from cyclonedds.idl.types import uint32
from cyclonedds.sub import DataReader, Subscriber
from cyclonedds.topic import Topic

ext = sys.argv[1] if len(sys.argv) > 1 else "appendable"
secs = float(sys.argv[2]) if len(sys.argv) > 2 else 12.0

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

dp = DomainParticipant(domain_id=int(os.environ.get("ZERODDS_DOMAIN", "100")))
tp = Topic(dp, "robot", RobotType)
r = DataReader(Subscriber(dp), tp)
print(f"[cyclone reader] ext={ext}", flush=True)
n = 0
t0 = time.time()
while time.time() - t0 < secs:
    for s in r.take(N=20):
        if s.sample_info.valid_data:
            n += 1
    time.sleep(0.1)
print(f"CYCLONE_RESULT samples={n}", flush=True)
