# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Encodings, and the four bytes that announce one.

DDSI-RTPS 2.5 §10.5 and XTypes 1.3 §7.4.1.1. Kept out of `writer.py`
because it frames a sample rather than being part of one: a KeyHash
holder is written by the same writer and carries no header at all.
"""

from __future__ import annotations

import enum

from .endianness import Endianness

__all__ = ["Encoding", "encapsulation_header", "encapsulation_padding"]


class Encoding(enum.Enum):
    """Which encoding a plan and a stream are in.

    The five of `zerodds_ast_layout::Encoding`. XCDR1 and XCDR2 differ in
    their framing: XCDR1 has no delimiter header, and `@mutable` uses PID
    headers there and EMHEADERs here.
    """

    PLAIN_CDR1 = "plain-cdr1"
    PL_CDR1 = "pl-cdr1"
    PLAIN_CDR2 = "plain-cdr2"
    DELIMITED_CDR = "delimited-cdr"
    PL_CDR2 = "pl-cdr2"

    def is_v1(self) -> bool:
        """Whether this is an XCDR version 1 encoding."""
        return self in (Encoding.PLAIN_CDR1, Encoding.PL_CDR1)

    def is_v2(self) -> bool:
        """Whether this is an XCDR version 2 encoding."""
        return not self.is_v1()

    @property
    def max_alignment(self) -> int:
        """The largest alignment any primitive takes.

        XTypes 1.3 §7.4.1: `MAXALIGN(VERSION2) = 4`,
        `MAXALIGN(VERSION1) = 8`, and `ALIGN(N)` aligns to
        `MIN(N, XCDR.maxalign)`. §7.6.8 repeats it for the key holder:
        "the maximum alignment in PLAIN_CDR2 (i.e., 4)".
        """
        return 8 if self.is_v1() else 4

    @staticmethod
    def parse(name: str) -> Encoding | None:
        """The command line's spelling, including the two shorthands.

        `xcdr1` and `xcdr2` name the plain form, because that is what a
        caller who has not thought about extensibility means. An unknown
        name is `None` and never a default.
        """
        if name in ("xcdr1", "plain-cdr1"):
            return Encoding.PLAIN_CDR1
        if name in ("xcdr2", "plain-cdr2"):
            return Encoding.PLAIN_CDR2
        for encoding in Encoding:
            if encoding.value == name:
                return encoding
        return None


_IDENTIFIERS: dict[tuple[Encoding, Endianness], int] = {
    (Encoding.PLAIN_CDR1, Endianness.BIG): 0x0000,
    (Encoding.PLAIN_CDR1, Endianness.LITTLE): 0x0001,
    (Encoding.PL_CDR1, Endianness.BIG): 0x0002,
    (Encoding.PL_CDR1, Endianness.LITTLE): 0x0003,
    (Encoding.PLAIN_CDR2, Endianness.BIG): 0x0006,
    (Encoding.PLAIN_CDR2, Endianness.LITTLE): 0x0007,
    (Encoding.DELIMITED_CDR, Endianness.BIG): 0x0008,
    (Encoding.DELIMITED_CDR, Endianness.LITTLE): 0x0009,
    (Encoding.PL_CDR2, Endianness.BIG): 0x000A,
    (Encoding.PL_CDR2, Endianness.LITTLE): 0x000B,
}


def encapsulation_padding(length: int) -> int:
    """How many zero bytes bring `length` up to the next multiple of four.

    XTypes 1.3 §7.4.1.1 pads the serialized data to a four-byte
    boundary; §7.6.3.1.2 states the count in the encapsulation options.
    """
    return (4 - (length & 3)) & 3


def encapsulation_header(
    encoding: Encoding, endianness: Endianness, padding: int = 0
) -> bytes:
    """The four-byte encapsulation header.

    The identifier itself is always big-endian on the wire, whatever the
    payload behind it is — it is what tells a reader which order to
    switch to.

    The two option bytes carry the payload's trailing padding count in
    the low two bits of the second one. XTypes 1.3 §7.6.3.1.2: those
    bits "shall be interpreted by the receiver to determine where the
    serialized data exactly ended". Measured on the loopback wire on
    2026-08-09: Cyclone, RTI and Fast DDS all pad the payload to four
    and all three state the count — `@final struct One { octet a; }`
    goes out as `0001 0003 ab000000` from every one of them.
    """
    identifier = _IDENTIFIERS[(encoding, endianness)]
    return bytes([identifier >> 8, identifier & 0xFF, 0, padding & 0x3])
