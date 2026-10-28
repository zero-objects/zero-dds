#!/usr/bin/env python3
"""CycloneDDS reader for the reverse leg of the #27 interop matrix
(ZeroDDS writer -> CycloneDDS reader).

Usage: cyclone_reader.py <final|appendable|appendable-extended> [seconds]

Counts successfully received samples on topic `robot`, domain 100, and prints
`CYCLONE_RESULT samples=<n>`.
"""
import sys
import time
from dataclasses import dataclass

from cyclonedds.domain import DomainParticipant
from cyclonedds.idl import IdlStruct
from cyclonedds.idl.annotations import appendable
from cyclonedds.idl.types import uint32
from cyclonedds.sub import DataReader, Subscriber
from cyclonedds.topic import Topic


# CPD-OFF  (standalone interop scripts share this CLI helper by design)
def resolve_domain(argv, idx):
    """Domain from CLI arg `idx`: omitted -> 100; present + valid 0..=232 ->
    that value; malformed / out-of-range -> stderr + exit 2."""
    if len(argv) <= idx:
        return 100
    try:
        d = int(argv[idx])
    except ValueError:
        sys.stderr.write(f"invalid domain {argv[idx]!r}: expected an integer in 0..=232\n")
        sys.exit(2)
    if not (0 <= d <= 232):
        sys.stderr.write(f"domain {d} out of range 0..=232\n")
        sys.exit(2)
    return d

# CPD-ON

ext = sys.argv[1] if len(sys.argv) > 1 else "appendable"
secs = float(sys.argv[2]) if len(sys.argv) > 2 else 12.0

if ext == "appendable":
    @appendable
    @dataclass
    class RobotType(IdlStruct, typename="Robot"):
        id: uint32 = 0
        label: uint32 = 0
elif ext == "appendable-extended":
    # A member appended: the reader has to fetch the writer's type with a
    # TypeLookup getTypes request before it can match.
    @appendable
    @dataclass
    class RobotType(IdlStruct, typename="Robot"):
        id: uint32 = 0
        label: uint32 = 0
        extra: uint32 = 0
else:
    @dataclass
    class RobotType(IdlStruct, typename="Robot"):
        id: uint32 = 0
        label: uint32 = 0

domain = resolve_domain(sys.argv, 3)
dp = DomainParticipant(domain_id=domain)
tp = Topic(dp, "robot", RobotType)
r = DataReader(Subscriber(dp), tp)
print(f"[cyclone reader] ext={ext} domain={domain}", file=sys.stderr, flush=True)
n = 0
t0 = time.time()
while time.time() - t0 < secs:
    for s in r.take(N=20):
        if s.sample_info.valid_data:
            n += 1
    time.sleep(0.1)
print(f"CYCLONE_RESULT samples={n}", flush=True)
