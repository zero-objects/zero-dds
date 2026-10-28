# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Byte order.

The counterpart of `crates/wire/src/endianness.rs`. A separate module
because the writer and the reader both need it and neither should own it.
"""

from __future__ import annotations

import enum

__all__ = ["Endianness"]


class Endianness(enum.Enum):
    """Which end of a multi-byte value goes out first."""

    LITTLE = "le"
    BIG = "be"

    def is_little(self) -> bool:
        """Whether the low byte goes out first."""
        return self is Endianness.LITTLE

    @property
    def struct_prefix(self) -> str:
        """The `struct` module's byte-order character.

        Explicit rather than `=` or `@`: the native order of the host is
        not the order the encapsulation identifier announced, and a probe
        that agreed with itself on one machine and not another would be
        useless as a second opinion.
        """
        return "<" if self is Endianness.LITTLE else ">"

    @staticmethod
    def parse(name: str) -> Endianness | None:
        """The command line's spelling, or `None` for anything else.

        `None` and not a default: a typo in a wire-affecting flag has to
        be loud, because the bytes it produces look plausible.
        """
        if name in ("le", "little"):
            return Endianness.LITTLE
        if name in ("be", "big"):
            return Endianness.BIG
        return None
