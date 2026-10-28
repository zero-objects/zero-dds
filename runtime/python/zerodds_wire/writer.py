# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Putting bytes on the wire.

The Python counterpart of `crates/wire/src/writer.rs` — a second
implementation of the same clauses, not a binding to the first. Where the
two disagree, one of them has misread DDS-XTypes 1.3, and that is the
point of having both.

Alignment is relative to the start of the stream, not to the start of the
value: XCDR aligns each primitive to its own size counted from the
encapsulation, which is why a `long` after a `char` sits at 4 and not at
1. XTypes 1.3 §7.4.1.1.

Three things Python makes easy to get wrong, and which are therefore
checked here rather than assumed:

- an `int` is unbounded and the wire is not, so every integer write
  refuses a value its field cannot hold instead of wrapping it;
- `len()` on a `str` counts characters and the wire counts bytes, so
  every length is taken from the encoded form;
- `float` is a double, so a `float` field is rounded once, explicitly.
"""

from __future__ import annotations

import hashlib
import struct
from dataclasses import dataclass
from enum import Enum

from .endianness import Endianness
from .error import OutOfRange
from .float128 import Float128

__all__ = [
    "Writer",
    "DHeader",
    "EmHeader",
    "Pid",
    "PID_EXTENDED",
    "PID_LIST_END",
    "PID_SHORT_MAX_ID",
    "PID_IMPL_EXTENSION",
    "PID_IGNORE",
    "PID_MUST_UNDERSTAND",
    "PlCdr1Framing",
    "pid_flags_at",
    "no_dheader",
    "bit_pattern_int",
    "bit_pattern_bool",
    "bit_pattern_f32",
    "bit_pattern_f64",
    "KEY_HASH_LEN",
]

#: The parameter id that introduces the extended form. XTypes 1.3 §7.4.1.2.2.
PID_EXTENDED = 0x3F01

#: The parameter id that ends a parameter list. XTypes 1.3 §7.4.1.2.4.
PID_LIST_END = 0x3F02

#: The highest member id a short parameter header can carry. §7.4.1.2.2
#: gives the range as 0 to 0x3F00 *inclusive*, and PID_EXTENDED is one
#: above it. RTI Connext and Fast DDS both write the short form for
#: 0x3F00 (case `pid_3f00`, 2026-08-10).
PID_SHORT_MAX_ID = 0x3F00

#: FLAG_IMPL_EXTENSION, bit 15 of a short parameter id. §7.4.1.2.2: a
#: parameter that sets it is the writing implementation's own, and
#: user-defined members "will never set the FLAG_IMPL_EXTENSION bit".
PID_IMPL_EXTENSION = 0x8000

#: PID_IGNORE. §7.4.1.2.2: "All consumers of this Data Representation
#: shall ignore parameters with this ID" — including a consumer that
#: would otherwise discard the sample over the must-understand bit.
PID_IGNORE = 0x3F03

#: The longest value a short parameter header can state the length of.
#: §7.4.1.2.2 gives `ssize` sixteen bits; `Writer.finish_pid` rewrites
#: the header rather than truncating a longer length into two bytes.
PID_SHORT_MAX_LEN = 0xFFFF

#: The must-understand bit of a parameter id. DDSI-RTPS 2.5 §9.4.2.11.2.
PID_MUST_UNDERSTAND = 0x4000

#: `FLAG_2` of the extended `eMemberHeader`, which is the **member's**
#: must-understand, and the mask of the 28-bit id beside it. §7.4.1.2.1.
E_FLAG_MUST_UNDERSTAND = 0x4000_0000

#: `FLAG_1` of the extended `eMemberHeader`, which is
#: FLAG_IMPL_EXTENSION — the same property as :data:`PID_IMPL_EXTENSION`
#: in the short form. XTypes 1.3 §7.4.1.2.1.
E_FLAG_IMPL_EXTENSION = 0x8000_0000
E_MEMBER_ID_MASK = 0x0FFF_FFFF

#: A KeyHash is sixteen bytes. DDSI-RTPS 2.5 §9.6.3.8.
KEY_HASH_LEN = 16


class ParameterRole(Enum):
    """Which of §7.4.1.2's two roles a parameter header stands in.

    The header is the same header. The role decides what the length
    field counts, and whether a member that took no value writes one at
    all: a `@mutable` member that is absent leaves no trace in the
    parameter list, an `@optional` member of a non-mutable type writes
    its header with a declared length of zero.
    """

    #: A member of a `@mutable` type. Rules (24) and (25).
    MEMBER = "member"
    #: An `@optional` member of a type that is not `@mutable`. Rule (19).
    OPTIONAL = "optional"


def pid_flags_at(policy: str, position: str) -> int:
    """What flags a PL_CDR1 parameter id carries at `position`.

    XTypes 1.3 §7.4.1.2 splits the sixteen-bit field: the low fourteen
    bits are the id, bit 14 is `PID_MUST_UNDERSTAND` and bit 15 is
    `PID_IMPL_EXTENSION`. The clause says nothing about what flags
    `PID_LIST_END` carries, and the four measured implementations do not
    agree — `struct-mutable` and `evolution-mutable-mustunderstand`
    under XCDR1:

        Cyclone   member as declared    sentinel 7f02
        RTI       member as declared    sentinel 7f02
        Fast DDS  member never          sentinel 3f02
        OpenDDS   member c0xx, always   sentinel 3f02

    The sentinel's answer cannot be read off the member's, which is what
    makes `"sentinel"` a position on this axis rather than a property of
    the list. `zerodds_ast_layout::PidFlagPolicy` carries the
    measurement; the positions are `zerodds_ast_layout::WirePosition`.

    `"extended-marker"` is the third position, and it moved here off the
    framing form for the same reason. `annotation-hashid-implicit` and
    `annotation-id-sparse`, XCDR1:

        Cyclone   always-extended    marker 7f01
        RTI       specification      marker 7f01
        Fast DDS  short-true-length  marker 3f01
        OpenDDS   short-true-length  marker ff01

    Two forms answer alike and two forms answer differently, so the
    marker's flags are not a property of the form.

    A member's *own* `must_understand` is not on this axis; it is
    `PlCdr1Framing.member_must_understand`, and the writer ORs the two.

    An unknown policy raises rather than falling back to a default: a
    typo in a wire-affecting setting must be loud.
    """
    if policy == "sentinel-must-understand":
        if position in ("sentinel", "extended-marker"):
            return PID_MUST_UNDERSTAND
        return 0
    if policy == "bare":
        return 0
    if policy == "impl-extension-members":
        if position in ("member", "extended-marker"):
            return PID_IMPL_EXTENSION | PID_MUST_UNDERSTAND
        return 0
    raise ValueError(f"`{policy}` is not a PID-flag policy")


@dataclass(frozen=True)
class PlCdr1Framing:
    """How a PL_CDR1 parameter list is framed.

    The three ways the measured implementations differ inside an XCDR1
    sample. Which header *form* each member takes is the fourth, and
    that one is in the layout plan because it changes the member's
    extent. The defaults are the clause's — `Dialect::Specification`,
    which is this project's default dialect. See
    `zerodds_ast_layout::Dialect`; they were Cyclone's while
    `Dialect::Cyclone` was the default, and the two that moved are
    `pad_declared_optional_length` and the header form.
    """

    #: What flags a parameter id carries, asked per position — a
    #: member's id, `PID_LIST_END` and the `PID_EXTENDED` marker. One
    #: setting for all three, because no position's answer can be read
    #: off another's: OpenDDS sets both flags on every member and closes
    #: with a bare `3f02`, while Cyclone and RTI leave the member bare
    #: and close with `7f02`; and at the marker Cyclone and RTI agree
    #: across two different framing forms while Fast DDS and OpenDDS
    #: share a form and disagree.
    #: `zerodds_ast_layout::PidFlagPolicy::as_str` names them.
    pid_flags: str = "sentinel-must-understand"
    #: The highest member id a short parameter header carries.
    #: §7.4.1.2.2 stops at 0x3F00 inclusive and reserves the rest of the
    #: low fourteen bits; one implementation carries all fourteen, and a
    #: reader holding the clause's threshold as a constant reports
    #: `@id(16383)` as reserved and drops the member. Both sides:
    #: `Writer.write_pid` refuses an id above it rather than putting a
    #: reserved parameter on the wire.
    highest_short_id: int = PID_SHORT_MAX_ID
    #: Whether a `@mutable` member's declared length covers its padding.
    pad_declared_length: bool = False
    #: The same question for an `@optional` member of a type that is not
    #: `@mutable`. A separate axis because Cyclone answers the two
    #: differently: `04000000` for the two-byte `short c` of
    #: `ext_mutable`, `01000000` for the one-byte `@optional octet b` of
    #: `opt_final_present`. RTI pads both, Fast DDS pads neither, and
    #: `PlCdr1HeaderPolicy::Specification` — the default — pads neither
    #: (XTypes 1.3 §7.4.1.2: the exact length); `ShortRoundedLength` pads both.
    pad_declared_optional_length: bool = False
    #: Whether a member's own must-understand bit goes out. §7.4.1.2.2
    #: makes it unconditional and two of the three write it: Cyclone as
    #: `FLAG_2` of the extended `eMemberHeader`, RTI as bit 14 of the
    #: short id, Fast DDS not at all.
    member_must_understand: bool = True
    #: Whether the parameter that just ended writes its own trailing
    #: padding to the next four-byte boundary. Every header writer
    #: aligns itself, so the two readings only part at the end of the
    #: stream: RTI pads after a final `@optional` parameter, Fast DDS
    #: does not (`annotation-must-understand-unannotated`, 24 against
    #: 22 octets).
    pad_after_value: bool = True

    def pads_declared_length(self, role: "ParameterRole") -> bool:
        """Whether a parameter in this role declares a padded length."""
        if role is ParameterRole.OPTIONAL:
            return self.pad_declared_optional_length
        return self.pad_declared_length


@dataclass(frozen=True)
class DHeader:
    """A DHEADER waiting for its length.

    `at` below zero is the delimiter a `@final` aggregate does not
    write; see `no_dheader`.
    """

    at: int


def no_dheader() -> DHeader:
    """The DHEADER a value that takes no delimiter does not write.

    §7.4.3.5.3 gives the delimiter to `@appendable` and `@mutable` and
    not to `@final`, and the shared walk declares one `DHeader` per
    value and finishes it either way. So the absence is a value of this
    type rather than `None`: the alternative is `DHeader | None` in
    every generated signature and a check at every finish, for a case
    the layout plan has already decided. `Writer.finish_dheader` ignores
    a header with `at` below zero, which is what lets a generated codec
    write the same two lines for both extensibilities.

    A module-level function and not a method, because the walk emits it
    without a receiver — `T::runtime_static("no_dheader", …)` in
    `crates/lang-rules/src/emit/encode.rs`.
    """
    return DHeader(at=-1)


@dataclass(frozen=True)
class EmHeader:
    """An EMHEADER waiting for its NEXTINT, when it has one."""

    next_int: int | None
    #: Where the header word itself sits, so a short length code can be
    #: patched into it once the value's length is known.
    word: int = 0
    #: The code the header was written with, so that `finish_emheader`
    #: can hold the member's value to the length the code declares. Codes
    #: 0 to 3 are lengths; anything else declares nothing of its own.
    length_code: int = 8


@dataclass(frozen=True)
class Pid:
    """A parameter header waiting for its length."""

    at: int
    extended: bool
    saved_origin: int
    #: Which length convention `finish_pid` patches by.
    role: ParameterRole = ParameterRole.MEMBER
    #: The member id and its must-understand, kept so that a short
    #: header can be rewritten as an extended one when the value turns
    #: out not to fit sixteen bits. The bit is in a different word in
    #: the two forms.
    pid: int = 0
    must_understand: bool = False


def _f32_bits(value: float) -> int:
    """The IEEE-754 single-precision pattern of a double.

    `struct.pack` raises for a magnitude no `float` can hold, where the
    Rust cast `value as f32` yields an infinity. The infinity is the
    wire-correct answer — it is what a C `(float)` cast produces and what
    every peer will read back — so the overflow is turned into one rather
    than reported as a Python error the caller cannot act on.
    """
    try:
        return int.from_bytes(struct.pack("<f", value), "little")
    except OverflowError:
        return 0xFF800000 if value < 0 else 0x7F800000


def _f64_bits(value: float) -> int:
    return int.from_bytes(struct.pack("<d", value), "little")


def _utf16_units(text: str) -> list[int]:
    """The UTF-16 code units of `text`, without a BOM.

    Done by hand rather than through `str.encode("utf-16-le")` because
    the rule this implements is about code *units* and the encoder hides
    them. XTypes 1.3 §7.4.1.1.2: "The serialized length of an object of
    type String<Char16> shall be the number of bytes in the CDR buffer
    taken by the String<Char16> characters. This is twice the number of
    characters in the string" — with the clause's own rationale being
    that a code point outside the Basic Multilingual Plane takes two
    units and a byte count still reaches the end of the string. So `"𝄞"`
    is a length of four.

    A lone surrogate is refused. Python permits one in a `str` and
    `encode` would either raise or smuggle it through with
    `surrogatepass`; neither is a code point, and writing one would put a
    sequence on the wire that is not UTF-16 at all.
    """
    units: list[int] = []
    for character in text:
        point = ord(character)
        if 0xD800 <= point <= 0xDFFF:
            raise OutOfRange(len(units) * 2, "a Unicode code point, not a lone surrogate")
        if point < 0x10000:
            units.append(point)
        else:
            offset = point - 0x10000
            units.append(0xD800 + (offset >> 10))
            units.append(0xDC00 + (offset & 0x3FF))
    return units


#: Every `wstring` length convention, as `WstringLengthPolicy::as_str`
#: spells it. Named once so that the writer and the reader cannot each
#: accept a slightly different set.
#: All eight cells of C1 x C2 x C3, five of them named for an
#: implementation and three by coordinate. The coordinate form
#: `counts/terminator/width` reaches every one of them and is what the
#: three unnamed cells are spelled as.
_WSTRING_POLICIES = (
    "specification",
    "character-count",
    "character-count-terminated",
    "specification-wide-units",
    "ros-type-support",
    "octets/terminated/2",
    "octets/terminated/4",
    "characters/terminated/2",
)

#: What each cell answers: C1 counts characters, C2 writes a terminator,
#: C3 is the unit width. One table rather than a predicate per question,
#: so a cell cannot answer one of the three and be forgotten by another.
_WSTRING_CELLS = {
    "specification": (False, False, 2),
    "character-count": (True, False, 2),
    "character-count-terminated": (True, True, 4),
    "specification-wide-units": (False, False, 4),
    "ros-type-support": (True, False, 4),
    "octets/terminated/2": (False, True, 2),
    "octets/terminated/4": (False, True, 4),
    "characters/terminated/2": (True, True, 2),
}

# Every cell is reachable by its coordinates as well as by its name,
# exactly as `WstringLengthPolicy::parse` accepts both. A cell that can
# only be named cannot be asked for by someone who has measured it and
# not yet named it, which is how the ROS form stayed unreachable.
_WSTRING_CELLS.update(
    {
        f"{'characters' if counts else 'octets'}/"
        f"{'terminated' if terminated else 'unterminated'}/{width}": (
            counts,
            terminated,
            width,
        )
        for counts, terminated, width in _WSTRING_CELLS.values()
    }
)

#: The names and the coordinate spellings together.
_WSTRING_POLICIES = _WSTRING_POLICIES + tuple(
    name for name in _WSTRING_CELLS if name not in _WSTRING_POLICIES
)

#: Every `wstring` unit order, as `WstringUnitOrder::as_str` spells it.
#: `host-unswapped` arrived as a value of the length policy and is a
#: byte order, so it is on this axis.
_WSTRING_UNIT_ORDERS = (
    "stream",
    "big-endian-always",
    "byte-order-mark",
    "host-unswapped",
)

#: The UTF-16 byte-order mark, and its byte-swapped form.
#:
#: `U+FFFE` is not a character: a mark that reads as one says the units
#: behind it are the other way round from the order they were read in.
_BYTE_ORDER_MARK = 0xFEFF
_SWAPPED_BYTE_ORDER_MARK = 0xFFFE


def wstring_unit_width(policy: str) -> int:
    """How many bytes one of a `wstring`'s code units takes.

    C3 of the product. Four under RTI Connext's XCDR1 form, under TAO's
    and under both ROS type supports'. The measurement is on
    `WstringLengthPolicy` in `crates/ast-layout/src/key.rs`; the names
    here are that type's `as_str`.

    **Not** the width of a scalar `wchar`, which is an argument of its
    own: `rmw_cyclonedds_cpp` 1.3.4 writes two octets for a `wchar`
    member and four for a `wstring`'s units, and this function answered
    both until that was fitted.
    """
    cell = _WSTRING_CELLS.get(policy)
    if cell is None:
        raise OutOfRange(0, f"a wstring length policy, not {policy!r}")
    return cell[2]


def fixed_nibbles(
    text: str, digits: int, scale: int, count: str = "declared"
) -> list[int]:
    """The BCD nibbles of `text` under `fixed<digits, scale>`.

    [CDR] §9.3.2.8, which is the only wire definition an IDL `fixed` has:
    digit nibbles most-significant first, then a sign nibble — `0xc` for
    positive or zero, `0xd` for negative — with a leading zero nibble
    prepended when the nibble count is odd so the nibbles pair up.

    Neither half is padded in the direction that would lose a digit: a
    value too long for its integer part or its fraction is one this type
    cannot represent, and is refused.

    ## How many digits go out

    The clause gives the packing and says nothing about the count, and
    the four implementations that write a `fixed` split two and two.
    `count` is `"declared"` — `digits` nibbles whatever the value is,
    which omniORB 4.3.3 and this project write — `"value-precision"`,
    as many digits as the value has and never fewer than one, which
    JacORB 3.9 writes (`BigDecimal("0.00").precision()` is 1 in Java,
    which is where the "never fewer than one" comes from), or
    `"value-string-digits"`, the integer digits of `text` as written,
    leading zeros included, plus the `scale` fraction digits, made odd by
    one leading zero, which ACE/TAO 2.5.24's `ACE_CDR::Fixed` and so
    OpenDDS write for a decimal with `scale` fraction digits (`0.12345`
    under `fixed<5,5>` is `0012345c`). Counting the fraction as written
    instead would cut `7` under `fixed<5,2>` to its last nibble, `0c`.
    That count can exceed `digits`; the field is then filled with leading
    zeros, and one past 31 digits is refused.
    """
    if digits == 0:
        raise OutOfRange(0, "a fixed with at least one digit")
    # OMG IDL 4.2 rule (42) caps the BCD rule at 31 digits, and
    # `fixed<31, S>` is the widest declaration IDL admits. A wider one has
    # no packing defined for it, so it is refused rather than packed into
    # octets no reader can be told the width of.
    if digits > 31:
        raise OutOfRange(0, f"a fixed of at most 31 digits, not {digits}")
    if scale > digits:
        raise OutOfRange(0, "a scale no larger than the digit count")

    if text.startswith("-"):
        negative, magnitude = True, text[1:]
    elif text.startswith("+"):
        negative, magnitude = False, text[1:]
    else:
        negative, magnitude = False, text

    whole, _, fraction = magnitude.partition(".")
    written = len(whole) + scale
    if not whole and not fraction:
        raise OutOfRange(0, f"a decimal that fits fixed<{digits}, {scale}>, not `{text}`")
    if not all(character.isdigit() and character.isascii() for character in whole + fraction):
        raise OutOfRange(0, f"a decimal that fits fixed<{digits}, {scale}>, not `{text}`")

    integral_width = digits - scale
    whole = whole.lstrip("0")
    if len(whole) > integral_width:
        raise OutOfRange(
            0,
            f"a decimal whose integer part fits {integral_width} digits, not `{text}`",
        )
    if len(fraction) > scale:
        raise OutOfRange(0, f"a decimal whose fraction fits {scale} digits, not `{text}`")

    if count not in ("declared", "value-precision", "value-string-digits"):
        raise OutOfRange(0, f"a fixed digit count, not `{count}`")

    # The full `digits`-wide string first, whatever the count: the
    # value's precision is that string with its leading zeros removed,
    # so both counts are read off one construction.
    every: list[int] = []
    every.extend([0] * (integral_width - len(whole)))
    every.extend(ord(character) - 0x30 for character in whole)
    every.extend(ord(character) - 0x30 for character in fraction)
    every.extend([0] * (scale - len(fraction)))
    significant = len(_after_leading_zeros(every))
    if count == "value-precision":
        wire = max(significant, 1)
    elif count == "value-string-digits":
        wire = max(written + (1 - written % 2), 1)
        if wire > 31:
            raise OutOfRange(0, f"a fixed of at most 31 digits on the wire, not {wire} for `{text}`")
    else:
        wire = len(every)

    nibbles: list[int] = []
    if (wire + 1) % 2 == 1:
        nibbles.append(0)
    kept = every[max(len(every) - wire, 0) :]
    nibbles.extend([0] * (wire - len(kept)))
    nibbles.extend(kept)
    # A negative zero is written positive, because there is one zero.
    nibbles.append(0x0D if negative and any(nibble != 0 for nibble in every) else 0x0C)
    return nibbles


def _after_leading_zeros(nibbles: list[int]) -> list[int]:
    """`nibbles` with its leading zeros removed."""
    first = 0
    while first < len(nibbles) and nibbles[first] == 0:
        first += 1
    return nibbles[first:]


class Writer:
    """Accumulates the bytes of one sample."""

    __slots__ = (
        "_out",
        "_endianness",
        "_origin",
        "_max_align",
        "_pl_cdr1",
        "short_length_codes",
    )

    def __init__(
        self,
        endianness: Endianness,
        max_align: int = 8,
        pl_cdr1: "PlCdr1Framing | None" = None,
        short_length_codes: bool = False,
    ) -> None:
        """A writer in `endianness` whose alignment is capped at `max_align`.

        Four for XCDR2, eight for XCDR1. XTypes 1.3 §7.4.1: `ALIGN(N)`
        aligns to `MIN(N, XCDR.maxalign)`. A writer that did not know this
        padded an eight-byte primitive to eight under XCDR2 and put four
        bytes on the wire that no peer expects.
        """
        self._out = bytearray()
        self._endianness = endianness
        # Where alignment is counted from. Zero for a sample; a PL_CDR1
        # parameter value moves it, because a parameter is its own
        # encapsulated block. XTypes 1.3 §7.4.1.2.
        self._origin = 0
        self._max_align = max(1, max_align)
        self._pl_cdr1 = pl_cdr1 if pl_cdr1 is not None else PlCdr1Framing()
        #: Whether an LC4 header whose member turns out to be 1, 2, 4 or
        #: 8 bytes is rewritten with a short length code. Set it from
        #: `LengthCodePolicy::short_code_where_it_fits`; the layout plan
        #: already answers for every member whose type fixes the size,
        #: and this is the other half.
        self.short_length_codes = short_length_codes

    # -- state -----------------------------------------------------------

    @property
    def endianness(self) -> Endianness:
        """The byte order in force."""
        return self._endianness

    @property
    def max_align(self) -> int:
        """The alignment cap in force."""
        return self._max_align

    def position(self) -> int:
        """How many bytes have been written."""
        return len(self._out)

    def as_bytes(self) -> bytes:
        """The bytes written so far."""
        return bytes(self._out)

    def finish(self) -> bytes:
        """The bytes, as an immutable copy."""
        return bytes(self._out)

    @staticmethod
    def big_endian() -> Writer:
        """A big-endian writer, which is what a KeyHash holder is."""
        return Writer(Endianness.BIG)

    @staticmethod
    def little_endian() -> Writer:
        """A little-endian writer."""
        return Writer(Endianness.LITTLE)

    # -- alignment -------------------------------------------------------

    def align_to(self, align: int) -> None:
        """Pad to `align`, counted from the stream's origin."""
        align = min(align, self._max_align)
        if align <= 1:
            return
        offset = len(self._out) - self._origin
        padding = (align - (offset % align)) % align
        if padding:
            self._out.extend(b"\x00" * padding)

    def begin_packed_element(self) -> int:
        """Move the alignment origin to the current position.

        `ElementPadding::Packed` is HDDS's measured departure from
        §7.4.1, which counts every element from the encapsulation: each
        collection element and each aggregate value counts its own
        alignment from its own first octet instead. The
        `array-1d-struct-final` and `typedef-of-struct` cases record it.

        Returns the origin displaced, for `finish_packed_element`.
        """
        saved = self._origin
        self._origin = len(self._out)
        return saved

    def finish_packed_element(self, saved: int) -> None:
        """Restore the origin `begin_packed_element` displaced."""
        self._origin = saved

    # -- primitives ------------------------------------------------------

    def _check(self, value: int, low: int, high: int, expected: str) -> None:
        """Refuse an integer the field cannot hold.

        The check the Rust runtime does not need and Python does: there
        an `i16` is an `i16`, here `70000` is as much an `int` as `7` is,
        and `& 0xffff` would put `4464` on the wire and report success.
        """
        if value < low or value > high:
            raise OutOfRange(len(self._out), f"{expected} in [{low}, {high}], not {value}")

    def write_bool(self, value: bool) -> None:
        """`boolean`, one byte, 0 or 1."""
        self._out.append(1 if value else 0)

    def write_u8(self, value: int) -> None:
        """`octet` and `uint8`."""
        self._check(value, 0, 0xFF, "an octet")
        self._out.append(value)

    def write_i8(self, value: int) -> None:
        """`int8`."""
        self._check(value, -0x80, 0x7F, "an int8")
        self._out.append(value & 0xFF)

    def write_char(self, value: str) -> None:
        """`char`, one byte.

        A code point above 255 is refused rather than truncated. The Rust
        runtime casts (`value as u32 as u8`) and documents the loss as a
        caller error; here it is a raise, because a `char` field holding
        `'ä'` in a UTF-8 source file is a mistake worth stopping on and
        `0xe4` is a different character.
        """
        if len(value) != 1:
            raise OutOfRange(len(self._out), f"exactly one character, not `{value}`")
        point = ord(value)
        if point > 0xFF:
            raise OutOfRange(len(self._out), f"a character below U+0100, not U+{point:04X}")
        self._out.append(point)

    def write_wchar(
        self,
        value: str,
        width: int = 2,
        framing: str = "fixed",
        order: str = "stream",
    ) -> None:
        """`wchar`, at `width` octets and in the shape `framing` does.

        Two octets under XTypes 1.3 Table 31 and for seven of the nine
        measured implementations; four under RTI Connext's XCDR1 —
        measured 2026-08-09, `wchar c = U+0061` gives `61000000` under
        XCDR1 and `6100` under XCDR2 — under TAO, and under
        `rosidl_typesupport_fastrtps_c`. It is an argument of its own
        and not a reading of the `wstring` policy, because
        `rmw_cyclonedds_cpp` writes two here and four inside a
        `wstring`.

        The BMP restriction of §7.2.2.2.1.2.5 is a property of the type
        and not of the width: the four-byte form zero-extends a code
        unit, it does not carry a code point.
        """
        if len(value) != 1:
            raise OutOfRange(len(self._out), f"exactly one character, not `{value}`")
        point = ord(value)
        if point > 0xFFFF:
            raise OutOfRange(len(self._out), f"a BMP character, not U+{point:04X}")
        if width not in (2, 4):
            raise OutOfRange(len(self._out), f"a wchar width of 2 or 4, not {width!r}")
        if framing == "giop-length-prefixed":
            # CORBA 3.3 part 2 §9.3.1.6's GIOP 1.2 form: the octet count
            # first, then the code unit's octets in the stream's order,
            # all aligned to one. Measured 2026-08-10: omniORB writes
            # `02 00e4` for U+00E4 in either stream order and TAO
            # `04 e4000000`. The count is the transmission code set's
            # width and not the value's, so it is the same octet for
            # every character.
            # The octets follow the same axis a `wstring`'s code units
            # do, with one difference the clause makes: a scalar cannot
            # carry a byte-order mark, so CORBA's default applies and
            # `byte-order-mark` means big-endian here. Measured
            # 2026-08-10 in a little-endian stream: omniORB writes
            # `02 00e4` and JacORB `02 00e4`, both big-endian; TAO
            # writes `04 e4000000`, the stream's order.
            self._out.append(width)
            if order != "stream":
                byte_order = "big"
            else:
                byte_order = "little" if self._endianness.is_little() else "big"
            self._out.extend(point.to_bytes(width, byte_order))
            return
        if framing != "fixed":
            raise OutOfRange(len(self._out), f"a wchar framing, not {framing!r}")
        if width == 4:
            self.write_u32(point)
        else:
            self.write_u16(point)

    def write_u16_little(self, value: int) -> None:
        """A 16-bit value little-endian whatever the stream's order is.

        Two callers: the code units of the `unswapped-units` `wstring`
        form, and the `byte-order-mark` unit order. Aligned to two like
        any other 16-bit value.
        """
        if not 0 <= value <= 0xFFFF:
            raise OutOfRange(len(self._out), f"an unsigned 16-bit value, not {value}")
        self.align_to(2)
        self._out.extend(value.to_bytes(2, "little"))

    def write_u16_big(self, value: int) -> None:
        """A 16-bit value big-endian whatever the stream's order is.

        The other side of the same axis: what omniORB and JacORB decode
        a `wstring`'s code units as, whatever the stream says.
        """
        if not 0 <= value <= 0xFFFF:
            raise OutOfRange(len(self._out), f"an unsigned 16-bit value, not {value}")
        self.align_to(2)
        self._out.extend(value.to_bytes(2, "big"))

    def write_u32_little(self, value: int) -> None:
        """A 32-bit value little-endian whatever the stream's order is.

        A four-octet transmission code set crossed with a fixed unit
        order. No dialect selects the combination; it exists so that two
        orthogonal settings have an answer for every pairing rather than
        a hole one of them falls into.
        """
        if not 0 <= value <= 0xFFFFFFFF:
            raise OutOfRange(len(self._out), f"an unsigned 32-bit value, not {value}")
        self.align_to(4)
        self._out.extend(value.to_bytes(4, "little"))

    def write_u32_big(self, value: int) -> None:
        """A 32-bit value big-endian whatever the stream's order is."""
        if not 0 <= value <= 0xFFFFFFFF:
            raise OutOfRange(len(self._out), f"an unsigned 32-bit value, not {value}")
        self.align_to(4)
        self._out.extend(value.to_bytes(4, "big"))

    def write_i16(self, value: int) -> None:
        """`short`."""
        self._check(value, -0x8000, 0x7FFF, "a short")
        self._write_int(value & 0xFFFF, 2)

    def write_u16(self, value: int) -> None:
        """`unsigned short`."""
        self._check(value, 0, 0xFFFF, "an unsigned short")
        self._write_int(value, 2)

    def write_i32(self, value: int) -> None:
        """`long`."""
        self._check(value, -0x8000_0000, 0x7FFF_FFFF, "a long")
        self._write_int(value & 0xFFFF_FFFF, 4)

    def write_u32(self, value: int) -> None:
        """`unsigned long`."""
        self._check(value, 0, 0xFFFF_FFFF, "an unsigned long")
        self._write_int(value, 4)

    def write_i64(self, value: int) -> None:
        """`long long`."""
        self._check(value, -0x8000_0000_0000_0000, 0x7FFF_FFFF_FFFF_FFFF, "a long long")
        self._write_int(value & 0xFFFF_FFFF_FFFF_FFFF, 8)

    def write_u64(self, value: int) -> None:
        """`unsigned long long`."""
        self._check(value, 0, 0xFFFF_FFFF_FFFF_FFFF, "an unsigned long long")
        self._write_int(value, 8)

    def write_f32(self, value: float) -> None:
        """`float`, rounded to single precision once, here."""
        self._write_int(_f32_bits(value), 4)

    def write_f64(self, value: float) -> None:
        """`double`."""
        self._write_int(_f64_bits(value), 8)

    def write_f128(self, value: Float128 | float) -> None:
        """`long double`: sixteen bytes of IEEE 754 binary128.

        CORBA 3.3 part 2 §9.3.1.3 with Figure 9.2 gives the layout and
        shows the two byte orders as the whole sixteen bytes reversed,
        not two eight-byte halves swapped in place. XTypes 1.3 Table 31
        gives the alignment: eight, capped to four by `MAXALIGN` under
        XCDR2, which `align_to` already does.

        This used to take a `float` and write it followed by eight zero
        bytes, because Python has no quadruple-precision type. That was
        the right length and the wrong value and it reported success.

        A plain `float` is still accepted and widened, which is exact —
        every double is a binary128. A value that needs more than 53
        significand bits has to arrive as a `Float128`, because there is
        no `float` that holds it to hand to this method.
        """
        wide = value if isinstance(value, Float128) else Float128.from_float(value)
        self.align_to(8)
        self._out.extend(
            wide.to_bytes_ordered("little" if self._endianness.is_little() else "big")
        )

    def _write_int(self, unsigned: int, width: int) -> None:
        """Align to the value's own width, then write it."""
        self.align_to(width)
        self._write_raw(unsigned, width)

    def _write_raw(self, unsigned: int, width: int) -> None:
        self._out.extend(
            unsigned.to_bytes(width, "little" if self._endianness.is_little() else "big")
        )

    # -- strings ---------------------------------------------------------

    def write_string(self, value: str) -> None:
        """A string: a four-byte length including the terminator, the
        bytes, then a NUL. XTypes 1.3 §7.4.1.1.

        The length counts the terminator, which is the one place XCDR
        differs from every other length-prefixed encoding. It counts
        *bytes* of UTF-8, not characters — `len(value)` would be right
        only for an ASCII sample and every corpus string is deliberately
        not one.
        """
        encoded = value.encode("utf-8")
        self.write_u32(len(encoded) + 1)
        self._out.extend(encoded)
        self._out.append(0)

    def write_string_bounded(self, value: str, bound: int) -> None:
        """A bounded string.

        The same bytes as an unbounded one: the bound is a promise about
        the source, not a difference on the wire — so this is the only
        place it can be enforced. An over-long value is refused rather
        than cut at a character boundary: XTypes 1.3 §7.2.2.7's TRIM is a
        *read*-side behaviour that belongs to the receiver's own bound,
        and a writer that cut the value would put a sample on the wire
        nobody wrote.
        """
        encoded = value.encode("utf-8")
        if len(encoded) > bound:
            raise OutOfRange(
                len(self._out),
                f"at most {bound} octets in a bounded string, and this one has "
                f"{len(encoded)}",
            )
        self.write_string(value)

    def write_wstring_bounded(
        self,
        value: str,
        bound: int,
        policy: str = "specification",
        order: str = "stream",
    ) -> None:
        """A bounded `wstring`, as `write_string_bounded` is to
        `write_string`.

        The bound counts UTF-16 code units, not characters and not
        octets, whatever `policy` puts on the wire.
        """
        units = len(_utf16_units(value))
        if units > bound:
            raise OutOfRange(
                len(self._out),
                f"at most {bound} UTF-16 code units in a bounded wstring, and this one "
                f"has {units}",
            )
        self.write_wstring_utf8(value, policy, order)

    def check_sequence_bound(self, actual: int, bound: int) -> None:
        """The count check every bounded `sequence` makes before a byte
        of it is written.

        Table 17 makes `O2.length <= T1.length` the condition for
        constructing a `sequence<T, N>`, and the wire carries no bound
        to compare against: a sender whose count already exceeds its
        own declaration has written a value of a type nobody declared.
        """
        if actual > bound:
            raise OutOfRange(
                len(self._out),
                f"at most {bound} elements in a bounded sequence, and this one has {actual}",
            )

    def check_map_bound(self, actual: int, bound: int) -> None:
        """The count check every bounded `map` makes before a byte of it
        is written.

        Its own message and not `check_sequence_bound`'s: the unit is an
        entry and not an element, per XTypes 1.3 §7.4.1.1.4's
        map-as-a-sequence-of-pairs equivalence.
        """
        if actual > bound:
            raise OutOfRange(
                len(self._out),
                f"at most {bound} entries in a bounded map, and this one has {actual}",
            )

    def write_wstring(
        self,
        value: str,
        policy: str = "specification",
        order: str = "stream",
    ) -> None:
        """A `wstring`, in the form `policy` and `order` name.

        `specification` is a four-byte byte count and then UTF-16 code
        units. No BOM and no terminator — XTypes 1.3 §7.4.1.1.2 forbids
        both outright: "The UTF-16 representation ... shall not include a
        Byte Order Mark (BOM). The representation shall also not include
        any terminating NUL character(s)." The order the units go out in
        is the stream's, which the encapsulation identifier already
        announced.

        The three questions are asked separately because they are three:
        which code units go out, what the length field counts them in,
        and in what byte order each one lands. The measurement and the
        argument are on `WstringLengthPolicy` and `WstringUnitOrder` in
        `crates/ast-layout/src/key.rs`; the names here are those enums'
        `as_str`.
        """
        if policy in _WSTRING_CELLS:
            counts_characters, _terminated, unit_width = _WSTRING_CELLS[policy]
            if counts_characters and unit_width == 2:
                # Fast CDR counts characters and writes each four-byte
                # `wchar_t` as two bytes, so a code point outside the BMP
                # goes out as its low sixteen bits and arrives as a
                # different character. A vendor's length convention is a
                # policy; its data loss is not, so the value is refused.
                #
                # The refusal is on the pair (characters, two octets) and
                # not on a named form: a character count over four-octet
                # units carries the surrogate pair intact.
                for index, character in enumerate(value):
                    if ord(character) > 0xFFFF:
                        raise OutOfRange(
                            index,
                            "a code point inside the BMP: a character count over two-octet "
                            "units has no way to carry one outside it",
                        )
        if policy not in _WSTRING_POLICIES:
            raise OutOfRange(len(self._out), f"a wstring length policy, not {policy!r}")
        if order not in _WSTRING_UNIT_ORDERS:
            raise OutOfRange(len(self._out), f"a wstring unit order, not {order!r}")

        counts_characters, terminated, width = _WSTRING_CELLS[policy]
        units = _utf16_units(value)
        if terminated:
            # CORBA §9.3.1.6's GIOP 1.1 rule, and RTI's XCDR1 form: a NUL
            # code unit at the end, inside whatever the length counts.
            # Nothing is lost above the BMP — both units are written.
            units = units + [0]
        # An empty `wstring` carries no mark: omniORB writes `00000000`
        # for one, measured 2026-08-10, byte-identical to every other
        # implementation. A mark on an empty string would be a two-octet
        # string that decodes to nothing, and no peer writes it.
        if order == "byte-order-mark" and units:
            units = [_BYTE_ORDER_MARK] + units

        # C1, over units that already include the terminator and the
        # mark. An octet count multiplies by C3; a character count is the
        # run's length. One expression rather than an arm per named form,
        # so a cell nobody has measured computes its length the same way.
        self.write_u32(len(units) if counts_characters else len(units) * width)
        for unit in units:
            self._write_wstring_unit(unit, width, policy, order)

    def write_wstring_utf8(
        self,
        value: str,
        policy: str = "specification",
        order: str = "stream",
    ) -> None:
        """A `wstring`, under the name the shared walk calls it by.

        The same operation as `write_wstring`. `runtime/c` converts from
        the UTF-8 its `wstring` arrives in as a `const char *`, and its
        name carries the conversion; a Python `str` holds code points
        already, so the two names name one method.
        """
        self.write_wstring(value, policy, order)

    def write_wstring_utf8_octets(self, value: str, policy: str = "specification") -> None:
        """A `wstring` whose characters go out as their own UTF-8 octets.

        `WstringCodeUnits::Utf8` is the measured departure of one
        product's generated serialiser: XTypes 1.3 Table 31 gives a
        `wstring`'s code unit as `Char16` and §7.4.1.1.2 makes the length
        "twice the number of characters", so the clause has no UTF-8
        form. The declared length is the UTF-8 octet count, and only C2 —
        the terminator — still reaches the wire. `wstring-member` is
        `04 00 00 00 61 62 63 00` for `"abc"`.
        """
        cell = _WSTRING_CELLS.get(policy)
        if cell is None:
            raise OutOfRange(len(self._out), f"a wstring length policy, not {policy!r}")
        data = value.encode("utf-8")
        length = len(data) + (1 if cell[1] else 0)
        if length > 0xFFFFFFFF:
            raise OutOfRange(len(self._out), "a wstring length the UInt32 field can hold")
        self.write_u32(length)
        self._out.extend(data)
        if cell[1]:
            self._out.append(0)

    def write_wstring_utf8_octets_bounded(
        self, value: str, bound: int, policy: str = "specification"
    ) -> None:
        """A bounded UTF-8-octet `wstring`.

        The bound still counts UTF-16 code units, which is what a
        `wstring<N>` bound means to every peer, while the wire carries the
        UTF-8 octets.
        """
        units = len(_utf16_units(value))
        if units > bound:
            raise OutOfRange(
                len(self._out),
                f"at most {bound} UTF-16 code units in a bounded wstring, and this one "
                f"holds {units}",
            )
        self.write_wstring_utf8_octets(value, policy)

    def _write_wstring_unit(self, unit: int, width: int, policy: str, order: str) -> None:
        """One `wstring` code unit, at its width and in its order.

        C3 picks the writer and C4 picks its endianness. There is no
        longer an arm for a length policy that decides a byte order:
        `host-unswapped` is a point of the order axis.
        """
        del policy
        little = order in ("byte-order-mark", "host-unswapped")
        if width == 4:
            if order == "big-endian-always":
                self.write_u32_big(unit)
            elif little:
                self.write_u32_little(unit)
            else:
                self.write_u32(unit)
        elif order == "big-endian-always":
            self.write_u16_big(unit)
        elif little:
            self.write_u16_little(unit)
        else:
            self.write_u16(unit)

    def write_fixed(
        self, value: str, digits: int, scale: int, count: str = "declared"
    ) -> None:
        """`fixed<digits, scale>` as packed BCD. [CDR] §9.3.2.8.

        DDS-XTypes 1.3 does not carry the type at all — §7.2.2 has no
        fixed-point kind and §7.4.3.5.3 has no rule whose left-hand side
        matches one — so the form is CORBA's, which is where an IDL
        `fixed` came from and the only wire definition it has.

        The value arrives as a decimal string because a JSON number is a
        double and `fixed<31, 2>` is exactly the case a double cannot
        carry.
        """
        nibbles = fixed_nibbles(value, digits, scale, count)
        for index in range(0, len(nibbles), 2):
            self._out.append((nibbles[index] << 4) | nibbles[index + 1])

    # -- the bit-bounded kinds -------------------------------------------

    def write_enum(self, value: int, bit_bound: int) -> None:
        """An enumerator, as the integer its bit bound calls for.

        XTypes 1.3 §7.4.3.5.3 rule (5) is `XCDR << {O : ENUM_TYPE} =
        XCDR << {O.value : O.holder_type}`, and the ENUM_TYPE row of the
        same clause's table says "O.holder_type is either Int8, Int16 or
        Int32 depending on the value of the @bit_bound annotation". A
        **signed** holder: `@value(-1)` is the two's complement of one,
        so at the default bound it goes out as `ffffffff`.

        What fits is everything expressible in the bound's bits, signed
        or unsigned — `-2^(n-1) ..= 2^n - 1`. RTI Connext 7.7 accepts
        `@value(-1)` at the default bound and Cyclone accepts
        `@value(200)` under `@bit_bound(8)`; a signed-only range would
        reject the second and a wider one would accept values neither can
        express.

        §7.3.1.2.1.5 caps the bound itself at 1 to 32 for an enumerated
        type, so a bound outside that has no holder and is refused rather
        than widened.
        """
        if bit_bound == 0 or bit_bound > 32:
            raise OutOfRange(len(self._out), f"an @bit_bound between 1 and 32, not {bit_bound}")
        if value < -(1 << (bit_bound - 1)) or value > (1 << bit_bound) - 1:
            raise OutOfRange(
                len(self._out), f"an enumerator a {bit_bound}-bit holder can hold, not {value}"
            )
        pattern = value & ((1 << bit_bound) - 1)
        if bit_bound <= 8:
            self.write_u8(pattern)
        elif bit_bound <= 16:
            self.write_u16(pattern)
        else:
            self.write_u32(pattern)

    def write_bitmask(self, value: int, bit_bound: int) -> None:
        """A bitmask, as the unsigned integer its bit bound calls for.

        XTypes 1.3 §7.4.3.5.3 rule (6) writes `O.value` as
        `O.holder_type`, and Table 33 of §7.4.1.1.3.2 gives the holder:
        `UInt8` for a bound of 1 to 8, `UInt16` to 16, `UInt32` to 32 and
        `UInt64` to 64. The same clause fixes the direction — "Bit
        indexes are counted from zero starting at the least-significant
        bit of the full byte size of the bitmask" — so bit `n` is
        `1 << n` whatever the holder's width.
        """
        if bit_bound == 0 or bit_bound > 64:
            raise OutOfRange(len(self._out), f"an @bit_bound between 1 and 64, not {bit_bound}")
        if value < 0 or value >= 1 << bit_bound:
            raise OutOfRange(len(self._out), f"a bitmask below 2^{bit_bound}, not {value}")
        if bit_bound <= 8:
            self.write_u8(value)
        elif bit_bound <= 16:
            self.write_u16(value)
        elif bit_bound <= 32:
            self.write_u32(value)
        else:
            self.write_u64(value)

    def bitfield(self, value: int, width: int, position: int) -> int:
        """One bitfield, shifted into the accumulator a bitset packs into.

        A method of the writer and not a free function, because the
        shared walk emits it as a call on the writer —
        `T::runtime("w", "bitfield", …)` in
        `crates/lang-rules/src/emit/encode.rs` — and a free function of
        that name would be a second `bitfield` the generated code never
        reaches.

        A value wider than the field is refused rather than masked:
        masking would put a different number on the wire than the caller
        asked for, and the caller would never find out. The counterpart
        of `zdw_bitfield` and of Java's and TypeScript's `bitfield`,
        which refuse the same two things — a field that does not fit
        inside the anchor's sixty-four bits, and a value that does not
        fit inside the field.
        """
        if width <= 0 or width > 64 or position < 0 or position + width > 64:
            raise OutOfRange(
                len(self._out), f"a bitfield of {width} bits at {position}, inside 64"
            )
        limit = (1 << width) - 1
        if value < 0 or value > limit:
            raise OutOfRange(len(self._out), f"a value inside {width} bits, not {value}")
        return value << position

    def write_bitset(self, value: int, anchor: int) -> None:
        """A bitset, as the anchor integer its bitfields pack into.

        DDS-XTypes 1.3 has no serialisation rule for a bitset — §7.4.3.5.3
        runs from rule (1) to rule (30) and no left-hand side matches one.
        How wide the anchor is comes from the layout planner and arrives
        here as a number, the same as every other framing decision.
        """
        if anchor == 1:
            self.write_u8(value)
        elif anchor == 2:
            self.write_u16(value)
        elif anchor == 4:
            self.write_u32(value)
        elif anchor == 8:
            self.write_u64(value)
        else:
            raise OutOfRange(len(self._out), f"a bitset anchor of 1, 2, 4 or 8 bytes, not {anchor}")

    def write_union_overlay(self, pattern: int, octets: int) -> None:
        """The Cyclone union overlay: the low `octets` of `pattern`.

        Written as one unsigned integer of that width in the stream's
        byte order. A discriminator carrying a label the `union-labels`
        dialect axis says selects nothing makes the peer fall through
        to `default:` and serialise its one C union storage *as that
        arm* — the selected member's octets, reinterpreted. The pattern
        is the integer whose little-endian bytes are the selected
        member's little-endian storage octets, zero-extended; the
        `bit_pattern_*` functions produce it.
        """
        if octets == 1:
            self.write_u8(pattern & 0xFF)
        elif octets == 2:
            self.write_u16(pattern & 0xFFFF)
        elif octets == 4:
            self.write_u32(pattern & 0xFFFF_FFFF)
        elif octets == 8:
            self.write_u64(pattern)
        else:
            raise OutOfRange(len(self._out), f"a union arm of 1, 2, 4 or 8 octets, not {octets}")

    # -- framing ---------------------------------------------------------

    def write_dheader(self) -> DHeader:
        """A DHEADER: a four-byte length the members that follow occupy.

        Written as a placeholder and patched when the members are done,
        because the length is not known until then.
        """
        self.align_to(4)
        at = len(self._out)
        self._out.extend(b"\x00\x00\x00\x00")
        return DHeader(at=at)

    def finish_dheader(self, header: DHeader) -> None:
        """Patch a DHEADER with the length of what followed it.

        A header from `no_dheader` is not there and is ignored: the walk
        finishes a delimiter for every value, including the ones
        §7.4.3.5.3 gives none.
        """
        if header.at < 0:
            return
        length = len(self._out) - header.at - 4
        self._patch(header.at, length, 4)

    def write_emheader(self, member_id: int, length_code: int, must_understand: bool) -> EmHeader:
        """An XCDR2 EMHEADER: the member id, its length code and the
        must-understand flag in one 32-bit word. XTypes 1.3 §7.4.3.4.2.

        The length code is the layout plan's decision and arrives here as
        a number; this runtime does not choose one.

        LC0 through LC3 carry the length in the code itself. LC4 is
        followed by a NEXTINT holding the member's length. LC5 through
        LC7 also have a NEXTINT, but §7.4.3.4.2 says it "shall be reused
        also as part of the serialized member ... because the
        serialization of certain members also starts with an integer
        length, which would take exactly the same value as NEXTINT" — so
        no placeholder is reserved for them: the member's own leading
        word lands in that position and *is* the NEXTINT. Reserving one
        would write the length twice.

        Raises `OutOfRange` for a member id above `E_MEMBER_ID_MASK`:
        §7.2.2.4.4.4.4 runs the ids from 0 to 0x0FFFFFFF and §7.4.3.4.2
        leaves them the low 28 bits of the word, so masking a larger one
        would write a well-formed header for `id & 0x0FFFFFFF` — a member
        the caller did not name, read back with nothing said at either
        end.
        """
        if not 0 <= member_id <= E_MEMBER_ID_MASK:
            raise OutOfRange(
                len(self._out),
                f"a member id inside the 28 bits of an EMHEADER, not {member_id}",
            )
        self.align_to(4)
        word = (
            ((1 if must_understand else 0) << 31)
            | ((length_code & 0x7) << 28)
            | (member_id & 0x0FFF_FFFF)
        )
        self.write_u32(word)
        word_at = len(self._out) - 4
        if length_code == 4:
            at = len(self._out)
            self._out.extend(b"\x00\x00\x00\x00")
            return EmHeader(next_int=at, word=word_at, length_code=length_code)
        return EmHeader(next_int=None, word=word_at, length_code=length_code)

    def finish_emheader(self, header: EmHeader) -> None:
        """Patch an EMHEADER's NEXTINT with the length of what followed.

        Under `short_length_codes` an LC4 header whose member turned out
        to be 1, 2, 4 or 8 bytes long instead loses its NEXTINT and
        states the length in the code, which is what Fast DDS does. See
        `Writer.short_length_codes`.
        """
        if header.next_int is None:
            # Under codes 0 to 3 the code *is* the length — 1, 2, 4 or 8
            # octets, §7.4.3.4.2. A value of another size misdeclares it
            # and the reader's skip lands inside data, so the mismatch is
            # refused here rather than written. Codes 5 to 7 declare
            # nothing of their own: the member's leading length word is
            # the NEXTINT.
            if 0 <= header.length_code <= 3 and header.word + 4 <= len(self._out):
                declared = 1 << header.length_code
                body = len(self._out) - header.word - 4
                if body != declared:
                    raise OutOfRange(
                        len(self._out),
                        f"a member of {declared} octets under length code "
                        f"{header.length_code}, not {body}",
                    )
            return
        length = len(self._out) - header.next_int - 4
        if self.short_length_codes and length in (1, 2, 4, 8):
            self._shorten_emheader(header, length)
            return
        self._patch(header.next_int, length, 4)

    def _shorten_emheader(self, header: EmHeader, length: int) -> None:
        """Remove an LC4 header's NEXTINT and raise its length code.

        Every byte of the member's value is unchanged and only its
        position moves, which is safe for one reason and only under
        XCDR2: MAXALIGN is four there, so the value began at a
        four-aligned offset with the NEXTINT in front of it and begins
        at a four-aligned offset without it.
        """
        at = header.next_int
        assert at is not None
        code = {1: 0, 2: 1, 4: 2, 8: 3}[length]
        del self._out[at : at + 4]
        order = "little" if self._endianness.is_little() else "big"
        previous = int.from_bytes(bytes(self._out[header.word : header.word + 4]), order)
        self._patch(header.word, (previous & ~(0x7 << 28)) | (code << 28), 4)

    def write_pid(
        self,
        pid: int,
        role: ParameterRole = ParameterRole.MEMBER,
        must_understand: bool = False,
    ) -> Pid:
        """A PL_CDR1 parameter header, short form: `[id:u16][length:u16]`.

        `must_understand` is bit 14 of the id. §7.4.1.2.2: the flag
        "shall be set if and only if the must_understand property of the
        member is set to true".

        Raises `OutOfRange` for an id above
        `PlCdr1Framing.highest_short_id`: §7.4.1.2.2 reserves the range
        above it for the protocol's own parameters — PID_EXTENDED is
        0x3F01 itself — so such an id takes the extended form or none.
        """
        if pid > self._pl_cdr1.highest_short_id:
            raise OutOfRange(
                len(self._out),
                f"a parameter id the short header carries, not {pid}"
                " — the extended header takes it",
            )
        self.align_to(4)
        # Two claimants on the same word, deliberately: the member's own
        # declaration, and whatever the framing puts on every id
        # regardless. OpenDDS's `c0010004` is the second alone.
        if must_understand and self._pl_cdr1.member_must_understand:
            pid |= PID_MUST_UNDERSTAND
        pid |= pid_flags_at(self._pl_cdr1.pid_flags, "member")
        bare = pid & ~(PID_MUST_UNDERSTAND | PID_IMPL_EXTENSION)
        self.write_u16(pid)
        at = len(self._out)
        self._out.extend(b"\x00\x00")
        return Pid(
            at=at,
            extended=False,
            saved_origin=self._begin_parameter_value(),
            role=role,
            pid=bare,
            must_understand=must_understand,
        )

    def write_pid_extended(
        self,
        pid: int,
        role: ParameterRole = ParameterRole.MEMBER,
        must_understand: bool = False,
    ) -> Pid:
        """A PL_CDR1 parameter header, extended form.

        `[PID_EXTENDED][8][id:u32][length:u32]`, required when the id
        reaches the reserved range or the value is longer than `0xFFFF`,
        and used throughout under Cyclone DDS's framing.
        XTypes 1.3 §7.4.1.2.

        Raises `OutOfRange` for a member id above `E_MEMBER_ID_MASK`,
        for the reason `write_emheader` does: the `eMemberHeader` of
        §7.4.1.2.1 has the same 28 bits, and masking aliases the
        parameter onto another member.
        """
        if not 0 <= pid <= E_MEMBER_ID_MASK:
            raise OutOfRange(
                len(self._out),
                f"a member id inside the 28 bits of an eMemberHeader, not {pid}",
            )
        self.align_to(4)
        marker = PID_EXTENDED | pid_flags_at(
            self._pl_cdr1.pid_flags, "extended-marker"
        )
        self.write_u16(marker)
        self.write_u16(8)
        # The member's must-understand is `FLAG_2` of the
        # `eMemberHeader` (§7.4.1.2.1) and not the marker's bit above
        # it: Cyclone sets the marker's on every parameter and `FLAG_2`
        # only where the member declares it.
        member_header = pid & E_MEMBER_ID_MASK
        if must_understand and self._pl_cdr1.member_must_understand:
            member_header |= E_FLAG_MUST_UNDERSTAND
        self.write_u32(member_header)
        at = len(self._out)
        self._out.extend(b"\x00\x00\x00\x00")
        return Pid(
            at=at,
            extended=True,
            saved_origin=self._begin_parameter_value(),
            role=role,
        )

    def _begin_parameter_value(self) -> int:
        """Start a parameter's value, returning the origin it replaces.

        "Each element, or parameter, within a parameter list data
        structure is simply a CDR-encapsulated block of data" —
        XTypes 1.3 §7.4.1.2. A block counts alignment from its own start,
        so an eight-byte member at the head of one is already aligned and
        takes no padding inside its own length.
        """
        saved = self._origin
        self._origin = len(self._out)
        return saved

    def finish_pid(self, header: Pid) -> None:
        """Patch a parameter header with its value's length.

        The declared length covers the value's trailing padding or
        stops at the value, depending on the framing — Cyclone and RTI
        round it up to four, Fast DDS does not. The padding itself
        starts the next parameter at a four-byte offset measured from
        the enclosing stream, so the origin goes back before it is
        written. XTypes 1.3 §7.4.1.2.
        """
        width = 4 if header.extended else 2
        length = len(self._out) - header.at - width
        if self._pl_cdr1.pads_declared_length(header.role):
            past = (len(self._out) - header.saved_origin) % 4
            if past:
                length += 4 - past
        if not header.extended and length > PID_SHORT_MAX_LEN:
            self._promote_pid(header, length)
        else:
            self._patch(header.at, length, width)
        self._origin = header.saved_origin
        # Under Fast DDS's framing the parameter writes no padding of
        # its own - the next header aligns itself, and a final
        # parameter ends the stream where its value does.
        if self._pl_cdr1.pad_after_value:
            self.align_to(4)

    def _promote_pid(self, header: Pid, length: int) -> None:
        """Rewrite a short parameter header as the extended form.

        §7.4.1.2.2's `ssize` is sixteen bits, and which values overrun
        it is a property of the sample rather than of the type — a
        `string` member is short in almost every sample and over 64 KiB
        in some. So the plan writes the short form for an unbounded
        member and the fallback lives here, where the length is known.

        Eight bytes are inserted between the header and the value. The
        value's own bytes do not change: XCDR1's MAXALIGN is eight and
        the insertion is eight, so every offset inside it keeps its
        remainder.
        """
        start = header.at - 2
        self._out[header.at + 2 : header.at + 2] = b"\x00" * 8
        marker = PID_EXTENDED | pid_flags_at(
            self._pl_cdr1.pid_flags, "extended-marker"
        )
        self._patch(start, marker, 2)
        self._patch(start + 2, 8, 2)
        member_header = header.pid & E_MEMBER_ID_MASK
        if header.must_understand and self._pl_cdr1.member_must_understand:
            member_header |= E_FLAG_MUST_UNDERSTAND
        self._patch(start + 4, member_header, 4)
        self._patch(start + 8, length, 4)

    def write_sentinel(self) -> None:
        """The `PID_LIST_END` sentinel a PL_CDR1 parameter list ends with.

        A reader that stops at the last member it recognises instead of
        at the sentinel misreads whatever follows.

        The flags come from the framing asked at the `"sentinel"`
        position, which is the only thing that decides them:
        `PID_LIST_END` is not a member and has no declaration of its own.
        """
        self.align_to(4)
        sentinel = PID_LIST_END | pid_flags_at(self._pl_cdr1.pid_flags, "sentinel")
        self.write_u16(sentinel)
        self.write_u16(0)

    def _patch(self, at: int, value: int, width: int) -> None:
        if value < 0 or value >= 1 << (width * 8):
            raise OutOfRange(at, f"a {width}-byte length, not {value}")
        self._out[at : at + width] = value.to_bytes(
            width, "little" if self._endianness.is_little() else "big"
        )

    # -- the KeyHash -----------------------------------------------------

    def zero_padded(self) -> bytes:
        """The holder as a KeyHash, zero-padded to sixteen bytes.

        For a holder of sixteen bytes or fewer. DDSI-RTPS 2.5 §9.6.3.8.
        """
        return bytes(self._out[:KEY_HASH_LEN]).ljust(KEY_HASH_LEN, b"\x00")

    def md5(self) -> bytes:
        """The MD5 of the holder, for one longer than sixteen bytes.

        MD5 because DDSI-RTPS 2.5 §9.6.3.8 names it, not as a security
        claim: the KeyHash identifies an instance and is not a signature.
        """
        return hashlib.md5(bytes(self._out), usedforsecurity=False).digest()


def bit_pattern_int(value: int, octets: int) -> int:
    """The low `octets` of `value`'s two's-complement bits, zero-extended.

    `Writer.write_union_overlay`'s pattern for an integer, character,
    enumeration or bitmask arm. The mask is the function: `-2` in four
    octets has to come out `0x00000000FFFFFFFE`.
    """
    return value & ((1 << (octets * 8)) - 1) if octets < 8 else value & 0xFFFF_FFFF_FFFF_FFFF


def bit_pattern_bool(value: bool) -> int:
    """One octet, 0 or 1 — the boolean overlay pattern."""
    return 1 if value else 0


def bit_pattern_f32(value: float) -> int:
    """The IEEE 754 binary32 bits, zero-extended."""
    return _f32_bits(value)


def bit_pattern_f64(value: float) -> int:
    """The IEEE 754 binary64 bits."""
    return _f64_bits(value)
