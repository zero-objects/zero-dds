#!/usr/bin/env python3
"""CycloneDDS writer for the #27 interop matrix.

Usage: cyclone_writer.py <final|appendable|appendable-extended|appendable-retyped>
                         <xcdr1|xcdr2> [seconds] [domain]

Extensibility is set on the type (default = final, matching Cyclone's own
generator default); representation is set via the DataRepresentation QoS.
Writes topic `robot` on domain 100.

The two `appendable-*` variants give the writer a type whose hash differs
from the ZeroDDS reader's, so the reader has to fetch it over the TypeLookup
service before it can decide: `appendable-extended` appends a member (the
reader's type is assignable from it, XTypes 1.3 §7.2.4.4.8), and
`appendable-retyped` declares `label` a string (not assignable).
"""
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

ext = sys.argv[1] if len(sys.argv) > 1 else "final"
rep = sys.argv[2] if len(sys.argv) > 2 else "xcdr1"
secs = float(sys.argv[3]) if len(sys.argv) > 3 else 30.0

if ext == "appendable":
    @appendable
    @dataclass
    class RobotType(IdlStruct, typename="Robot"):
        id: uint32 = 0
        label: uint32 = 0
elif ext == "appendable-extended":
    @appendable
    @dataclass
    class RobotType(IdlStruct, typename="Robot"):
        id: uint32 = 0
        label: uint32 = 0
        extra: uint32 = 0
elif ext == "appendable-retyped":
    @appendable
    @dataclass
    class RobotType(IdlStruct, typename="Robot"):
        id: uint32 = 0
        label: str = ""
elif ext == "final":
    @dataclass
    class RobotType(IdlStruct, typename="Robot"):
        id: uint32 = 0
        label: uint32 = 0
else:
    sys.stderr.write(f"unknown extensibility {ext!r}\n")
    sys.exit(2)

qos = Qos(Policy.DataRepresentation(
    use_cdrv0_representation=(rep == "xcdr1"),
    use_xcdrv2_representation=(rep == "xcdr2"),
))
domain = resolve_domain(sys.argv, 4)
dp = DomainParticipant(domain_id=domain)
tp = Topic(dp, "robot", RobotType)
print(f"[cyclone writer] domain={domain}", file=sys.stderr, flush=True)
w = DataWriter(Publisher(dp), tp, qos=qos)
print(f"[cyclone writer] ext={ext} rep={rep}", flush=True)
t0 = time.time()
c = 0
while time.time() - t0 < secs:
    w.write(RobotType(id=1, label=str(c) if ext == "appendable-retyped" else c % 1000))
    c += 1
    time.sleep(0.3)
