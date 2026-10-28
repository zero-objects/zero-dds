# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""What a generated module exposes.

A generated module holds one dataclass and one set of functions per IDL
type, and a `TYPES` table naming them. The table is how a caller that
knows only a scoped name — a probe, a topic registration — reaches the
generated code; it is not how the generated code reaches itself, which
is by direct call, so nothing on the encode path is looked up at runtime.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import TYPE_CHECKING, Any, Callable

from .values import Mismatch, SampleError

if TYPE_CHECKING:
    # Only the annotations of `TypeEntry` need these, and `from
    # __future__ import annotations` leaves those as strings. Imported
    # for real they would be a cycle: `reader.py` takes `Status` from
    # here, so this module cannot be reached through `reader.py` and
    # reach back into it while it is still executing.
    from .reader import Reader
    from .writer import Writer

__all__ = [
    "Discarded",
    "Status",
    "TryConstruct",
    "TypeEntry",
    "check_bound",
    "check_dimension",
    "trim_utf16",
    "trim_utf8",
    "utf16_units",
    "utf8_octets",
]


class TryConstruct(Enum):
    """What a receiving type does with a value it cannot hold.

    DDS-XTypes 1.3 §7.2.2.7, Table 11. Passed to every generated
    decoder, because the behaviour belongs to the *position* — this
    member, this collection's element, this map's key, this union's
    discriminator — and not to the type, which is shared between
    positions that may each want a different answer.
    """

    #: The failure propagates and the sample is discarded. The default
    #: where no annotation was written.
    DISCARD = "DISCARD"
    #: The member or element takes the default value of its type
    #: (Table 9) and the container is constructed.
    USE_DEFAULT = "USE_DEFAULT"
    #: A string, wstring, sequence or map is cut to the bound, order
    #: preserved. Undefined on anything else, which the IDL frontend
    #: refuses rather than passing on.
    TRIM = "TRIM"


class Status(Enum):
    """What a reader answers about the *type*, without raising.

    Two kinds of failure, and they are not one. A malformed **stream**
    raises: a length that runs past the end of the buffer is not a state
    a caller can carry on from. A refusal about the **type** sets this
    and the generated decoder walks on to the end of the sample, which
    is what lets one `if` at the top of the caller decide what to do
    with it. The runtime owns the wire and the generated decoder owns
    the type, so the reader cannot see an integer that names no
    enumerator, a bitmask bit the declaration does not have, or a member
    that did not arrive; and §7.2.2.7's DISCARD is a sample the receiver
    drops and carries on from, which is not a failure of the peer.

    The values are `Status` in `runtime/typescript/src/policy.ts`
    spelled the same, because the shared codec walk emits one constant
    into both columns and a column that renamed a member would compare
    against a name no other column knows.
    """

    #: Nothing has been refused.
    OK = "ok"
    #: The stream ended inside a value.
    TRUNCATED = "truncated"
    #: A length field states more than the stream holds. Kept apart from
    #: TRUNCATED: the first says the stream is short, this says the
    #: stream is lying, and a reader that allocated for the claimed
    #: length before checking it is how a bad message becomes an
    #: out-of-memory.
    IMPOSSIBLE_LENGTH = "impossible-length"
    #: A value the reading type cannot hold — an enumerator it does not
    #: declare, a `wstring` count its unit width does not divide.
    OUT_OF_RANGE = "out-of-range"
    #: §7.2.2.7's DISCARD. An **absence**, not an error: a DDS
    #: subscriber's `take()` returns nothing and no listener fires.
    DISCARDED = "discarded"


class Discarded(SampleError):
    """The sample is well formed and this type cannot hold it.

    §7.2.4.1: "the default behavior is to discard the O2 object to avoid
    misinterpretation", and §7.2.3: the sample "can be safely ignored
    (filtered out) by the receiver". A discarded sample is an
    **absence**, not an error — a DDS subscriber's `take()` returns
    nothing and no listener fires.

    Its own class, and not a `Mismatch`, for the same reason the Rust
    and C runtimes keep the two apart: `Mismatch` and the wire errors
    under it say the *stream* is wrong and the peer needs fixing, while
    this says one sample does not fit a type that is otherwise perfectly
    compatible, and the next sample may well fit.
    """

    def __init__(self, path: str, expected: str, found: str) -> None:
        super().__init__(
            f"the sample is discarded: at {path} this type holds {expected} "
            f"and the sample carries {found}, and `@try_construct` there is DISCARD"
        )
        self.path = path


def utf8_octets(text: str) -> int:
    """How many octets `text` is, which is what a `string<N>` bound
    counts: `string<5>` is `char[6]` in the C mapping of every measured
    DDS implementation, so a five-scalar value of ten octets is one no
    peer can hold."""
    return len(text.encode("utf-8"))


def utf16_units(text: str) -> int:
    """How many UTF-16 code units `text` is, which is what a
    `wstring<N>` bound counts. A scalar above the basic plane is two."""
    return sum(2 if ord(character) > 0xFFFF else 1 for character in text)


def _trim(text: str, limit: int, wide: bool) -> str:
    """A prefix of `text` costing at most `limit` units.

    Table 11: "The order of the characters in the string ... is
    preserved", so this is a prefix and never a reordering. A character
    that would straddle the bound is left out whole — a peer holding
    `char[limit + 1]` cannot store half of one either.
    """
    used = 0
    out = []
    for character in text:
        if wide:
            cost = 2 if ord(character) > 0xFFFF else 1
        else:
            cost = len(character.encode("utf-8"))
        if used + cost > limit:
            break
        used += cost
        out.append(character)
    return "".join(out)


def trim_utf8(text: str, octets: int) -> str:
    """`text` cut to at most `octets` bytes, never inside a character."""
    return _trim(text, octets, False)


def trim_utf16(text: str, units: int) -> str:
    """`text` cut to at most `units` UTF-16 code units."""
    return _trim(text, units, True)


@dataclass(frozen=True)
class TypeEntry:
    """One IDL type's generated support."""

    #: The scoped name, without the leading `::`.
    name: str
    #: Read one sample out of parsed JSON.
    from_json: Callable[[Any, str], Any]
    #: Put one sample back into JSON, in the same dialect `from_json`
    #: reads. The inverse and not a debug printer: what it produces is
    #: canonicalised by `crates/wire-probe/src/canonical.rs` and
    #: compared against the sample that went in, which is how the read
    #: direction of this column is measured in the same currency as the
    #: other two. Without it the only available check re-encoded what
    #: the decoder built and compared bytes, which passes any value the
    #: decoder invents that the encoder writes the same way — the shape
    #: of the `fixed` defect, where the leading zeros of the BCD field
    #: came back and both encodings held the same nibbles.
    to_json: Callable[[Any, str], Any]
    #: Write one sample.
    encode: Callable[[Any, Writer], None]
    #: Read one sample back.
    decode: Callable[[Reader], Any]
    #: The KeyHash holder, for a type with `@key` members; `None` for one
    #: without, because a KeyHash over nothing would give every instance
    #: of that type the same handle.
    key_hash: Callable[[Any], bytes] | None


def check_bound(count: int, bound: int, path: str) -> None:
    """Refuse a collection longer than its bound.

    A bounded sequence writes the same bytes as an unbounded one of the
    same length — the bound is a promise about the source, not a
    difference on the wire — so this is the only place it is enforced.
    """
    if count > bound:
        raise Mismatch(path, f"at most {bound} elements", str(count))


def check_dimension(count: int, expected: int, path: str) -> None:
    """Refuse an array that is not exactly its declared length.

    An array's element count is in the type and not on the wire, so a
    short one would be written short and read as whatever followed it.
    """
    if count != expected:
        raise Mismatch(path, f"exactly {expected} elements", str(count))
