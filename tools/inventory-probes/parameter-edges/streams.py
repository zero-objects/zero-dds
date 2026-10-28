#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Hand-built PL_CDR1 parameter lists for the four header edges of
`docs/wire-feature-inventory.md` §I that no writer will produce.

I9  `FLAG_IMPL_EXTENSION` — bit 15 of a short parameter id, `FLAG_1` of
    the extended `eMemberHeader`. XTypes 1.3 §7.4.1.2.2 says a
    user-defined member "will never set" it, so the only way to put one
    in front of a vendor's reader is to write the octets here.
I10 `FLAG_UNSPECIFIED1`/`FLAG_UNSPECIFIED2` — `FLAG_3` and `FLAG_4` of
    the extended header, which §7.4.1.2.1 says "should be set to zero".
    No measured implementation sets either, so the same applies.
I14 The reserved short-id range 0x3F01–0x3FFF (§7.4.1.2.2).
I17 `PID_IGNORE = 0x3F03` — "All consumers of this Data Representation
    shall ignore parameters with this ID", with the must-understand bit
    set, which is the case that decides whether the clause outranks
    §7.2.2.4.4.4.6.

Every stream is little-endian XCDR1 payload with no encapsulation
header, which is what `wireprobe-<vendor> --in` takes.

## Why each stream is written twice

A parameter list's framing is the writing implementation's, and the four
DDS products do not share one: Cyclone writes `PID_EXTENDED` for every
member, RTI and Fast DDS write the short header, and OpenDDS writes the
short header with `FLAG_IMPL_EXTENSION | FLAG_MUST_UNDERSTAND` set on
every member id. A stream in one vendor's framing handed to another
measures the framing and not the edge, so each edge is built once per
framing and each vendor is handed the one it writes itself. The framings
are read off that vendor's own output for `member-id-private-parameter`,
recorded in the module below.
"""
import struct
import sys

PID_EXTENDED = 0x3F01
PID_LIST_END = 0x3F02
PID_IGNORE = 0x3F03
PID_MUST_UNDERSTAND = 0x4000
PID_IMPL_EXTENSION = 0x8000
# The top four bits of the extended `eMemberHeader`, XTypes 1.3 §7.4.1.2.1.
E_FLAG_1 = 0x8000_0000  # FLAG_IMPL_EXTENSION
E_FLAG_2 = 0x4000_0000  # FLAG_MUST_UNDERSTAND
E_FLAG_3 = 0x2000_0000  # FLAG_UNSPECIFIED1
E_FLAG_4 = 0x1000_0000  # FLAG_UNSPECIFIED2

# What each vendor writes on a member id and on the sentinel, measured
# from its own `member-id-private-parameter` XCDR1 LE payload:
#
#   zerodds  01000400 07000000 02000400 09000000 027f0000
#   cyclone  017f0800 01000000 04000000 07000000 ... 027f0000
#   rti      01000400 07000000 02000400 09000000 027f0000
#   fastdds  01000400 07000000 02000400 09000000 023f0000
#   opendds  01c00400 07000000 02c00400 09000000 023f0000
#
# The `PID_EXTENDED` marker carries flags of its own and they are not the
# member's: Cyclone, RTI and this project write `017f`, i.e. `0x3F01`
# with the must-understand bit, and Fast DDS writes `013f`. A stream that
# gets the marker wrong is refused for the framing and never reaches the
# edge it was built to ask about, which is what the first run of this
# probe measured before the marker was a per-vendor value.
FRAMING = {
    #  vendor       member    sentinel              marker
    #               flags     flags                 flags
    "zerodds": (0x0000, PID_MUST_UNDERSTAND, PID_MUST_UNDERSTAND, False),
    "cyclone": (0x0000, PID_MUST_UNDERSTAND, PID_MUST_UNDERSTAND, True),
    "rti": (0x0000, PID_MUST_UNDERSTAND, PID_MUST_UNDERSTAND, False),
    "fastdds": (0x0000, 0x0000, 0x0000, False),
    "opendds": (PID_IMPL_EXTENSION | PID_MUST_UNDERSTAND, 0x0000, 0x0000, False),
}


def short(pid, payload):
    return struct.pack("<HH", pid, len(payload)) + payload


def extended(vendor, member_header, payload):
    """`PID_EXTENDED` with an eight-octet header: the 32-bit
    `eMemberHeader` and the 32-bit `nextInt` length. XTypes 1.3
    §7.4.1.2.1."""
    _, _, marker, _ = FRAMING[vendor]
    return (
        struct.pack("<HH", marker | PID_EXTENDED, 8)
        + struct.pack("<II", member_header, len(payload))
        + payload
    )


def member(vendor, member_id, payload):
    flags, _, _, always_extended = FRAMING[vendor]
    if always_extended or member_id > 0x3F00:
        e = 0
        if flags & PID_IMPL_EXTENSION:
            e |= E_FLAG_1
        if flags & PID_MUST_UNDERSTAND:
            e |= E_FLAG_2
        return extended(vendor, e | member_id, payload)
    return short(flags | member_id, payload)


def sentinel(vendor):
    _, flags, _, _ = FRAMING[vendor]
    return short(flags | PID_LIST_END, b"")


def i32(v):
    return struct.pack("<i", v)


def streams(vendor):
    """name -> (type, octets, what a conforming reader must see)."""
    flags, _, _, _ = FRAMING[vendor]
    e_member = (E_FLAG_1 if flags & PID_IMPL_EXTENSION else 0) | (
        E_FLAG_2 if flags & PID_MUST_UNDERSTAND else 0
    )
    a = member(vendor, 1, i32(7))
    b = member(vendor, 2, i32(9))
    end = sentinel(vendor)
    out = {}

    # I9, short form. A vendor-private parameter whose low fourteen bits
    # are a real member's id, placed *after* that member so a reader that
    # masks bit 15 off overwrites the value it already had.
    out["i9-short"] = (
        "member-id-private-parameter",
        a + b + short(PID_IMPL_EXTENSION | 2, struct.pack("<I", 0xDEADBEEF)) + end,
        "a=7 b=9 — the private parameter is no member of this type",
    )
    # I9, extended form: the same property as `FLAG_1`.
    out["i9-extended"] = (
        "member-id-private-parameter",
        a + b + extended(vendor, E_FLAG_1 | e_member | 2, struct.pack("<I", 0xDEADBEEF)) + end,
        "a=7 b=9",
    )
    # I10. FLAG_3 and FLAG_4 sit outside the 28-bit id, so a tolerant
    # reader takes the parameter as member 2 and the value arrives.
    out["i10-flag3"] = (
        "member-id-private-parameter",
        a + extended(vendor, E_FLAG_3 | e_member | 2, i32(9)) + end,
        "a=7 b=9 — the flag is outside the id and cannot move a member",
    )
    out["i10-flag4"] = (
        "member-id-private-parameter",
        a + extended(vendor, E_FLAG_4 | e_member | 2, i32(9)) + end,
        "a=7 b=9",
    )
    out["i10-both"] = (
        "member-id-private-parameter",
        a + extended(vendor, E_FLAG_3 | E_FLAG_4 | e_member | 2, i32(9)) + end,
        "a=7 b=9",
    )
    # I14. A short id inside the reserved range beside a member declared
    # with that same number, which the extended header can carry legally.
    ra = member(vendor, 1, i32(7))
    rz = member(vendor, 16132, i32(9))
    out["i14-reserved"] = (
        "member-id-reserved-range",
        ra + rz + short(flags | 0x3F04, struct.pack("<I", 0xDEADBEEF)) + sentinel(vendor),
        "a=7 z=9 — a reserved id is no member of this type",
    )
    # I17. `PID_IGNORE` with the must-understand bit set: the clause
    # states no exception, so it outranks §7.2.2.4.4.4.6.
    out["i17-ignore"] = (
        "member-id-private-parameter",
        a + short(flags | PID_IGNORE, struct.pack("<I", 0xDEADBEEF)) + b + end,
        "a=7 b=9 — skipped, sample delivered",
    )
    out["i17-ignore-mustunderstand"] = (
        "member-id-private-parameter",
        a
        + short(PID_MUST_UNDERSTAND | PID_IGNORE, struct.pack("<I", 0xDEADBEEF))
        + b
        + end,
        "a=7 b=9 — ignored despite the must-understand bit",
    )
    # L4. `O.ssize` is constrained below 4 GiB, so the largest a 32-bit
    # length field holds is a length no stream can satisfy.
    out["l4-length-4gib"] = (
        "sequence-length-overflow",
        struct.pack("<I", 0xFFFFFFFF) + i32(42),
        "refused — 0xFFFFFFFF elements is 16 GiB of body",
    )
    out["l4-length-just-over"] = (
        "sequence-length-overflow",
        struct.pack("<I", 0x4000_0000) + i32(42),
        "refused — 2^30 elements is 4 GiB of body",
    )
    return out


if __name__ == "__main__":
    vendor = sys.argv[1]
    for name, (case, octets, expected) in streams(vendor).items():
        print(f"{name}\t{case}\t{octets.hex()}\t{expected}")
