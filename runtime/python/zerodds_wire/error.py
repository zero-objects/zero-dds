# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""What can go wrong on the wire.

The variants of `crates/wire/src/error.rs`, as exceptions. Every one
names what was expected and where: a reader that returned a zero for a
truncated stream would hand the caller a value that was never sent, and
a writer that clamped an over-wide integer would put a different number
on the wire than the caller asked for.
"""

from __future__ import annotations

__all__ = [
    "WireError",
    "Truncated",
    "ImpossibleLength",
    "NotUtf8",
    "OutOfRange",
]


class WireError(Exception):
    """Base of every wire failure."""


class Truncated(WireError):
    """The stream ended before the value did."""

    def __init__(self, at: int, needed: int, available: int) -> None:
        super().__init__(f"at {at}: {needed} bytes needed, {available} available")
        self.at = at
        self.needed = needed
        self.available = available


class ImpossibleLength(WireError):
    """A length field claims more than the stream holds.

    Separate from `Truncated`: a length that cannot be true is a
    malformed stream, not a short one, and allocating for it is how a
    reader turns a bad message into an out-of-memory.
    """

    def __init__(self, at: int, claimed: int, available: int) -> None:
        super().__init__(f"at {at}: a length of {claimed} with {available} bytes left")
        self.at = at
        self.claimed = claimed
        self.available = available


class NotUtf8(WireError):
    """A string held bytes that are not UTF-8."""

    def __init__(self, at: int) -> None:
        super().__init__(f"at {at}: the string is not UTF-8")
        self.at = at


class OutOfRange(WireError):
    """A value outside what its type admits.

    Raised by the writer as well as the reader, which is where Python
    differs from the Rust runtime: an `int` there is `i32` or `u64` and
    the compiler has already refused the value, while here it is
    unbounded and the check has to happen at the point of the write.
    """

    def __init__(self, at: int, expected: str) -> None:
        super().__init__(f"at {at}: expected {expected}")
        self.at = at
        self.expected = expected
