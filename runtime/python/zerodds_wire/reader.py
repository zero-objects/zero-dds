# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Taking bytes off the wire.

The Python counterpart of `crates/wire/src/reader.rs`, and the mirror of
`writer.py` clause for clause. Every read that cannot be satisfied raises
a `WireError` naming what was expected and where: a reader that returned
a zero for a truncated stream would hand the caller a value that was
never sent.
"""

from __future__ import annotations

import dataclasses
import struct
from dataclasses import dataclass

from .endianness import Endianness
from .error import ImpossibleLength, NotUtf8, OutOfRange, Truncated
from .float128 import Float128
from .typesupport import Status, trim_utf16, trim_utf8
from .writer import (
    E_FLAG_IMPL_EXTENSION,
    E_FLAG_MUST_UNDERSTAND,
    E_MEMBER_ID_MASK,
    PID_EXTENDED,
    PID_IGNORE,
    PID_IMPL_EXTENSION,
    PID_LIST_END,
    PID_MUST_UNDERSTAND,
    PID_SHORT_MAX_ID,
    _BYTE_ORDER_MARK,
    _WSTRING_CELLS,
    _SWAPPED_BYTE_ORDER_MARK,
    _WSTRING_UNIT_ORDERS,
    ParameterRole,
    pid_flags_at,
    wstring_unit_width,
)


def _strip_byte_order_mark(units: list[int]) -> list[int]:
    """Take the byte-order mark off the front and apply what it said.

    The units arrived little-endian, because that is what omniORB writes
    on this host and what the `byte-order-mark` order therefore reads.
    Three cases, and the third is the one that matters:

    - `FEFF` — the mark is the right way up, the units are as read.
    - `FFFE` — the mark is byte-swapped, so every unit behind it is too,
      and each one is swapped back. This is what a conforming big-endian
      omniORB peer produces and what a reader that ignored the mark
      would return as different characters, well formed and the right
      length.
    - no mark — CORBA's rule for a GIOP 1.2 `wstring` without one is
      UTF-16 big-endian, so every unit read little-endian is swapped.

    The mark is never a character of the returned string in any of the
    three, which is the half §7.4.1.1.2 cares about. The stream's own
    byte order is not an argument and does not need to be: reading the
    units little-endian and then obeying the mark is correct in all four
    combinations of writer host and stream order, because the mark is
    written in the same order as the content it introduces. That is what
    a byte-order mark is for.
    """
    if not units:
        return units
    if units[0] == _BYTE_ORDER_MARK:
        return units[1:]
    rest = units[1:] if units[0] == _SWAPPED_BYTE_ORDER_MARK else units
    return [((unit & 0xFF) << 8) | (unit >> 8) for unit in rest]

#: The low fourteen bits of a short parameter id, which are the id.
PID_ID_MASK = 0x3FFF

__all__ = ["Reader", "EmHeaderRead", "PidRead", "ReaderMark"]


@dataclass(frozen=True)
class ReaderMark:
    """Everything a two-pass read has to put back.

    One caller: a `map` under `rule-15-omits`, where a delimited map
    states no entry count and the entries are read once to count them
    and again into storage sized from that count. Python owns its own
    values, so there is no heap mark to carry — the cursor, the stream
    origin and the status are the whole of it.

    The status is part of it deliberately. A refusal made during the
    counting pass is a refusal about bytes that are about to be read
    again, and carrying it forward would refuse the sample twice.
    """

    at: int
    origin: int
    status: Status


@dataclass(frozen=True)
class EmHeaderRead:
    """An EMHEADER as read."""

    member_id: int
    length_code: int
    must_understand: bool
    next_int: int | None

    @property
    def extent(self) -> int:
        """How many octets the member occupies after the header word,
        the NEXTINT included where there is one.

        What a reader skipping an unknown member advances by, and what a
        reader of a known one checks itself against. XTypes 1.3
        §7.4.3.4.2: codes 0 to 3 state a size of one, two, four or eight
        octets outright; code 4 is followed by a NEXTINT holding the
        member's length; and codes 5, 6 and 7 have a NEXTINT that is
        *also* the member's own leading word, so the member spans that
        word plus the length it states, times one, four or eight.

        The same arithmetic `skip_member` does, and a property rather
        than a second copy of it: the two answered the same question and
        only one of them existed, so a reader that had already read the
        member and wanted to know where it should have ended had nothing
        to ask. `runtime/typescript`'s `EmHeaderRead` carries it as a
        field for the same caller.

        Zero where a code of 5 or above arrived without its NEXTINT,
        which `read_emheader` never produces: it peeks the word and puts
        the cursor back. A member of no extent then ends where it began,
        which is what the reader's own bounds check will report.
        """
        if self.length_code <= 3:
            return 1 << self.length_code
        if self.next_int is None:
            return 0
        if self.length_code == 4:
            return self.next_int
        return 4 + self.next_int * {5: 1, 6: 4, 7: 8}[self.length_code]


@dataclass(frozen=True)
class PidRead:
    """A parameter header as read."""

    pid: int
    length: int
    end: bool
    #: Whether the writer set the member's must-understand flag.
    must_understand: bool = False
    #: Whether the writer set FLAG_IMPL_EXTENSION.
    #:
    #: §7.4.1.2.2: the parameter is the writing implementation's own,
    #: and a user-defined member "will never set the
    #: FLAG_IMPL_EXTENSION bit". Masking it off with the rest made a
    #: vendor-private parameter indistinguishable from the member with
    #: the same low fourteen bits.
    impl_extension: bool = False
    #: Whether the short-form id is inside §7.4.1.2.2's reserved range
    #: 0x3F01-0x3FFF, which no member id occupies.
    reserved: bool = False
    #: PID_IGNORE, which §7.4.1.2.2 says every consumer shall ignore —
    #: with no exception for the must-understand bit.
    ignore: bool = False
    #: Which of §7.4.1.2's two roles the caller read it in.
    role: ParameterRole = ParameterRole.MEMBER
    #: The stream origin in force before this parameter's value block
    #: opened, for `finish_pid_read` to put back. Zero on a header from
    #: `read_pid`, which opens no block — see `read_pid_as`.
    saved_origin: int = 0
    #: Where the value starts, which is the first octet after the header.
    value_at: int = 0

    def is_user_member(self) -> bool:
        """Whether this parameter can be a member of the reading type.

        False for a vendor-private parameter and for a reserved id.
        Both are legal for a peer to send and neither is a member, so
        both take the unknown-parameter path — skipped, or the sample
        discarded when the writer set must-understand. §7.4.1.2.2,
        §7.2.2.4.4.4.6.
        """
        return not self.impl_extension and not self.reserved and not self.ignore


class Reader:
    """Reads one sample's bytes."""

    __slots__ = (
        "_bytes",
        "_at",
        "_endianness",
        "_max_align",
        "_origin",
        "_pid_flags",
        "_highest_short_id",
        "_status",
    )

    def __init__(
        self,
        data: bytes,
        endianness: Endianness,
        max_align: int = 8,
        pid_flags: str = "sentinel-must-understand",
        highest_short_id: int = PID_SHORT_MAX_ID,
    ) -> None:
        """A reader over `data`, capped at `max_align`.

        Four for XCDR2, eight for XCDR1 — the writer's counterpart. A
        reader that skipped padding the writer never wrote reads the
        wrong bytes from the second member on.

        `pid_flags` is the writer's, and the reader needs it for one
        thing: under `"impl-extension-members"` every member id carries
        `FLAG_IMPL_EXTENSION`, so the flag no longer tells a
        vendor-private parameter from a user member. See
        `read_pid`.

        `highest_short_id` is the other thing it needs: §7.4.1.2.2 stops
        the short form's ids at 0x3F00 inclusive and one implementation
        carries the whole low fourteen bits, so a reader holding the
        clause's threshold as a constant reports `@id(16383)` as a
        reserved parameter and drops the member.
        """
        self._bytes = data
        self._at = 0
        self._endianness = endianness
        self._max_align = max(1, max_align)
        self._origin = 0
        self._pid_flags = pid_flags
        self._highest_short_id = highest_short_id
        # Nothing is refused until something refuses it. §7.2.2.7's
        # DISCARD and the type-level refusals below it are not raised,
        # because the generated decoder has to reach the end of the type
        # to know how much of the stream the sample occupied.
        self._status = Status.OK

    # -- the status a generated decoder reads ----------------------------

    def reader_status(self) -> Status:
        """How the read is going."""
        return self._status

    def reader_refuse(self, refusal: Status) -> None:
        """Refuse the stream from outside the runtime. The first refusal
        sticks.

        A later refusal does not replace an earlier one. The generated
        decoder carries on to the end of the type rather than returning
        early — that is what lets one `if` at the top of the caller
        decide what to do with the sample — so everything read after the
        first refusal is read at a position no writer need have put
        anything at, and a second refusal would describe that rather
        than the fault. `Status.OK` refuses nothing and is ignored.
        """
        if refusal is not Status.OK and self._status is Status.OK:
            self._status = refusal

    def reader_clear_discard(self) -> bool:
        """Take back a `Status.DISCARDED`, and only that one.

        A member whose own `@try_construct` is USE_DEFAULT but which
        also carries an explicit `@default` cannot be answered inside
        the type's decoder: the decoder knows Table 9's value for the
        type and not the literal the member's declaration gave it, and
        §7.2.2.4.4.4.9 puts the literal first. So the call site asks for
        DISCARD — §7.2.2.7 — and where it gets one it clears it here and
        assigns the literal.

        Every other status stays: a truncated stream is not a narrowing.
        """
        if self._status is Status.DISCARDED:
            self._status = Status.OK
            return True
        return False

    def reader_position(self) -> int:
        """Where it is. The generated decoder's name for `position`."""
        return self.position()

    def reader_remaining(self) -> int:
        """How many bytes are left. The decoder's name for `remaining`."""
        return self.remaining()

    def reader_skip_to(self, target: int) -> None:
        """Move the cursor forward to `target`, which is never backwards.

        A framing word pointing behind the cursor means the reader has
        already read past what it framed, and seeking back would paper
        over it: the member would be read twice and the sample would
        come back plausible. So that is a refusal about the stream's
        shape and not a seek.

        Past the end is `Truncated` and raises, because there is no
        position there to carry on from.
        """
        if self._status is not Status.OK:
            return
        if target < self._at:
            self.reader_refuse(Status.OUT_OF_RANGE)
            return
        if target > len(self._bytes):
            raise Truncated(self._at, target - self._at, self.remaining())
        self._at = target

    def reader_save(self) -> ReaderMark:
        """The cursor, the origin and the status, to be put back later."""
        return ReaderMark(at=self._at, origin=self._origin, status=self._status)

    def reader_restore(self, mark: ReaderMark) -> None:
        """Put back what `reader_save` took."""
        self._at = mark.at
        self._origin = mark.origin
        self._status = mark.status

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
        """Where it is."""
        return self._at

    def remaining(self) -> int:
        """How many bytes are left."""
        return max(0, len(self._bytes) - self._at)

    def align_to(self, align: int) -> None:
        """Skip padding to `align`, counted from the stream's origin."""
        align = min(align, self._max_align)
        if align <= 1:
            return
        offset = self._at - self._origin
        self._at += (align - (offset % align)) % align

    def reader_begin_packed_element(self) -> int:
        """Move the alignment origin to the current position.

        The read-side half of `Writer.begin_packed_element`, for
        `ElementPadding::Packed`. Returns the origin displaced.
        """
        saved = self._origin
        self._origin = self._at
        return saved

    def reader_finish_packed_element(self, saved: int) -> None:
        """Restore the origin `reader_begin_packed_element` displaced."""
        self._origin = saved

    def _take(self, count: int) -> bytes:
        end = self._at + count
        if end > len(self._bytes):
            raise Truncated(self._at, count, self.remaining())
        chunk = self._bytes[self._at : end]
        self._at = end
        return chunk

    def _read_int(self, width: int, signed: bool) -> int:
        self.align_to(width)
        raw = self._take(width)
        return int.from_bytes(raw, "little" if self._endianness.is_little() else "big", signed=signed)

    # -- primitives ------------------------------------------------------

    def read_bool(self) -> bool:
        """`boolean`.

        A byte that is neither 0 nor 1 is a stream that disagrees with
        its own type, and is refused rather than coerced.
        """
        at = self._at
        value = self.read_u8()
        if value == 0:
            return False
        if value == 1:
            return True
        raise OutOfRange(at, "a boolean of 0 or 1")

    def read_u8(self) -> int:
        """`octet` and `uint8`."""
        return self._take(1)[0]

    def read_i8(self) -> int:
        """`int8`."""
        value = self._take(1)[0]
        return value - 0x100 if value >= 0x80 else value

    def read_char(self) -> str:
        """`char`, one byte, as a one-character string."""
        return chr(self._take(1)[0])

    def read_wchar(
        self,
        width: int = 2,
        framing: str = "fixed",
        order: str = "stream",
    ) -> str:
        """`wchar`, at `width` octets and in `framing`'s shape.

        The four-byte form is RTI Connext's under XCDR1, where the high
        sixteen bits are padding around one UTF-16 code unit. A value
        that uses them is refused rather than narrowed: narrowing would
        return a character the peer never sent.

        `giop-length-prefixed` is CORBA 3.3 part 2 §9.3.1.6's GIOP 1.2
        form, which all three ORBs write. The octet count is checked
        against the configured code-set width rather than believed: a
        stream whose first octet says 0xE4 — which is what a bare
        two-octet `wchar` looks like read this way — would otherwise
        consume 228 octets of whatever followed and return a character
        assembled out of the next four members.
        """
        at = self._at
        if width not in (2, 4):
            raise OutOfRange(at, f"a wchar width of 2 or 4, not {width!r}")
        if framing == "giop-length-prefixed":
            declared = self.read_u8()
            if declared != width:
                raise OutOfRange(
                    at,
                    f"a GIOP 1.2 wchar of {width} octets, the configured transmission "
                    f"code set, and not {declared}",
                )
            # A scalar cannot carry a byte-order mark, so
            # `byte-order-mark` means CORBA's default here — big-endian
            # — exactly as `big-endian-always` does.
            if order != "stream":
                byte_order = "big"
            else:
                byte_order = "little" if self._endianness.is_little() else "big"
            value = int.from_bytes(self._take(width), byte_order)
        elif framing != "fixed":
            raise OutOfRange(at, f"a wchar framing, not {framing!r}")
        elif width == 4:
            value = self.read_u32()
            if value > 0xFFFF:
                raise OutOfRange(at, "a four-byte wchar holding one UTF-16 code unit")
        else:
            value = self.read_u16()
        if 0xD800 <= value <= 0xDFFF:
            raise OutOfRange(at, "a Unicode code point")
        return chr(value)

    def read_i16(self) -> int:
        """`short`."""
        return self._read_int(2, signed=True)

    def read_u16(self) -> int:
        """`unsigned short`."""
        return self._read_int(2, signed=False)

    def read_i32(self) -> int:
        """`long`."""
        return self._read_int(4, signed=True)

    def read_u32(self) -> int:
        """`unsigned long`."""
        return self._read_int(4, signed=False)

    def read_i64(self) -> int:
        """`long long`."""
        return self._read_int(8, signed=True)

    def read_u64(self) -> int:
        """`unsigned long long`."""
        return self._read_int(8, signed=False)

    def read_f32(self) -> float:
        """`float`."""
        bits = self._read_int(4, signed=False)
        return float(struct.unpack("<f", bits.to_bytes(4, "little"))[0])

    def read_f64(self) -> float:
        """`double`."""
        bits = self._read_int(8, signed=False)
        return float(struct.unpack("<d", bits.to_bytes(8, "little"))[0])

    def read_f128(self) -> Float128:
        """`long double`: all sixteen bytes, as IEEE 754 binary128.

        The counterpart of `Writer.write_f128`. This used to read the
        first eight bytes as a double and skip the rest, so every value a
        double cannot hold decoded as a different number and said
        nothing.
        """
        self.align_to(8)
        return Float128.from_bytes(
            self._take(16), "little" if self._endianness.is_little() else "big"
        )

    # -- strings ---------------------------------------------------------

    def read_string(self) -> str:
        """A string: a length including the terminator, the bytes, the NUL."""
        at = self._at
        length = self.read_u32()
        if length == 0 or length > self.remaining() + 1:
            raise ImpossibleLength(at, length, self.remaining())
        text = self._take(length - 1)
        try:
            out = text.decode("utf-8")
        except UnicodeDecodeError as error:
            raise NotUtf8(at) from error
        self.read_u8()
        return out

    def read_string_alloc(self) -> str:
        """A string the reader materialises itself.

        The same operation as `read_string` under the name the shared
        walk calls it by. `runtime/c` has two: a `string` read into
        storage the caller set and one taken out of a heap, because an
        embedded target decides where memory comes from. A Python `str`
        owns its storage and there is nothing for the caller to decide,
        so the two names are one method.
        """
        return self.read_string()

    def read_u16_little(self) -> int:
        """A 16-bit value little-endian whatever the stream's order is.

        One caller: the code units under `host-unswapped` and under
        `byte-order-mark`. The counterpart of `Writer.write_u16_little`.
        """
        self.align_to(2)
        return int.from_bytes(self._take(2), "little")

    def read_u16_big(self) -> int:
        """A 16-bit value big-endian whatever the stream's order is.

        The counterpart of `Writer.write_u16_big`: what omniORB and
        JacORB decode a `wstring`'s code units as.
        """
        self.align_to(2)
        return int.from_bytes(self._take(2), "big")

    def read_u32_little(self) -> int:
        """A 32-bit value little-endian whatever the stream's order is."""
        self.align_to(4)
        return int.from_bytes(self._take(4), "little")

    def read_u32_big(self) -> int:
        """A 32-bit value big-endian whatever the stream's order is."""
        self.align_to(4)
        return int.from_bytes(self._take(4), "big")

    def read_wstring_utf8(self, policy: str, order: str) -> str:
        """A `wstring`, under the name the shared walk calls it by.

        The same operation as `read_wstring`. `runtime/c` converts the
        code units to UTF-8 because that is what its `const char *`
        holds, and its name carries the conversion; a Python `str` holds
        code points already, so there is no second form here and the two
        names name one method.
        """
        return self.read_wstring(policy, order)

    def read_wstring_utf8_octets(self, policy: str = "specification") -> str:
        """The inverse of `Writer.write_wstring_utf8_octets`.

        `WstringCodeUnits::Utf8`: the declared length is the UTF-8 octet
        count, and the terminator `policy` asks for is the last of those
        octets. A count the stream cannot hold, a missing or an interior
        NUL, or an ill-formed UTF-8 sequence is refused.
        """
        at = self._at
        declared = self.read_u32()
        if declared > self.remaining():
            raise ImpossibleLength(at, declared, self.remaining())
        cell = _WSTRING_CELLS.get(policy)
        if cell is None:
            raise OutOfRange(at, f"a wstring length policy, not {policy!r}")
        data = self._take(declared)
        if cell[1]:
            if declared == 0 or data[-1] != 0:
                raise OutOfRange(at, "a NUL octet ending the wstring, as its count includes one")
            data = data[:-1]
            if 0 in data:
                raise OutOfRange(at, "no NUL inside a wstring a C peer would stop at")
        try:
            return data.decode("utf-8")
        except UnicodeDecodeError as error:
            raise NotUtf8(at) from error

    def read_wstring(self, policy: str = "specification", order: str = "stream") -> str:
        """A `wstring`, in the form `policy` and `order` name.

        Under `specification` the count is bytes and not characters —
        §7.4.1.1.2 — so a count the code-unit width does not divide is a
        stream no encoder produced and is refused rather than rounded
        down: rounding would leave the remainder in the stream and put
        every later member at the wrong offset.

        The form is never inferred from the stream. The three are not
        separable: a Fast DDS `"ab"` followed by a `long` is
        byte-identical to a specification `"a"` followed by the same
        `long`, differing only in padding no clause fixes. Choosing
        between them would mean letting the length field — the one field
        a hostile peer controls — decide how much is read. The argument
        and the measured bytes are on `WstringLengthPolicy` and
        `WstringUnitOrder` in `crates/ast-layout/src/key.rs`.
        """
        at = self._at
        declared = self.read_u32()
        if order not in _WSTRING_UNIT_ORDERS:
            raise OutOfRange(at, f"a wstring unit order, not {order!r}")
        # The three coordinates, not a list of named forms. C1 says
        # whether the count is of units or of the octets they occupy, C3
        # says what an octet count divides by, and C2 is handled after
        # the run because a terminator occupies a unit on the wire
        # whichever way it is counted.
        cell = _WSTRING_CELLS.get(policy)
        if cell is None:
            raise OutOfRange(at, f"a wstring length policy, not {policy!r}")
        counts_characters, terminated, width = cell
        if counts_characters:
            units = self._wstring_units(at, declared, width, order)
        else:
            if declared % width != 0:
                raise ImpossibleLength(at, declared, self.remaining())
            units = self._wstring_units(at, declared // width, width, order)
        if terminated:
            # The count includes the NUL, so a count of zero is a frame
            # that does not hold its own terminator.
            if declared < 1:
                raise ImpossibleLength(at, declared, self.remaining())
            if units.pop() != 0:
                raise OutOfRange(at, "a NUL character ending the wstring, as its count includes one")
            # A NUL inside the text is the narrow string's problem in
            # wide form: every C peer stops at it. Only where a
            # terminator is expected — a cell without one is entitled to
            # carry U+0000 as a character.
            if 0 in units:
                raise OutOfRange(at, "no NUL inside a wstring a C peer would stop at")
        if order == "byte-order-mark":
            units = _strip_byte_order_mark(units)
        out: list[str] = []
        index = 0
        while index < len(units):
            unit = units[index]
            if 0xD800 <= unit <= 0xDBFF and index + 1 < len(units):
                low = units[index + 1]
                if 0xDC00 <= low <= 0xDFFF:
                    out.append(chr(0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00)))
                    index += 2
                    continue
            if 0xD800 <= unit <= 0xDFFF:
                raise OutOfRange(at, "a UTF-16 sequence with no lone surrogate")
            out.append(chr(unit))
            index += 1
        return "".join(out)

    def _wstring_units(
        self,
        at: int,
        count: int,
        width: int,
        order: str = "stream",
    ) -> list[int]:
        """`count` wstring characters of `width` bytes, as UTF-16 units.

        The four-byte width is RTI's XCDR1 form, where a character is a
        code unit zero-extended. A value above 0xFFFF there is not a form
        this reader knows how to narrow, so it is refused rather than
        truncated — truncating is the defect this whole policy exists
        because of.
        """
        if count * width > self.remaining():
            raise ImpossibleLength(at, count * width, self.remaining())
        # C3 picks the reader and C4 picks its endianness.
        little = order in ("byte-order-mark", "host-unswapped")
        units: list[int] = []
        for _ in range(count):
            if width == 4:
                if order == "big-endian-always":
                    wide = self.read_u32_big()
                elif little:
                    wide = self.read_u32_little()
                else:
                    wide = self.read_u32()
                if wide > 0xFFFF:
                    raise OutOfRange(at, "a four-byte wstring character holding one UTF-16 unit")
                units.append(wide)
            elif order == "big-endian-always":
                units.append(self.read_u16_big())
            elif little:
                units.append(self.read_u16_little())
            else:
                units.append(self.read_u16())
        return units

    def read_fixed(self, digits: int, scale: int, count: str = "declared") -> str:
        """`fixed<digits, scale>`, as the decimal string the writer took.

        The mirror of `Writer.write_fixed`: `ceil((digits + 1) / 2)` bytes
        of BCD, digits most-significant first, sign nibble last.

        ## Why the leading zeros go

        The BCD field is `digits` wide whatever the value, so the nibbles
        for `0.01` in a `fixed<5, 2>` spell `00001` and the whole part
        comes out of them as `000`. Returning `"000.01"` puts a decimal
        on the read side that the write side would never produce from
        the same value: `"0.01"` is what the sample said and what
        `crates/wire-probe/src/canonical.rs` calls the value.

        Both other columns already stripped them, so the Python column
        was the only one whose `fixed` did not read back as what it
        wrote. It was invisible because nothing compared the two: the
        corpus runner re-encoded what it read and compared the *bytes*,
        and `"000.01"` and `"0.01"` are the same five nibbles.

        A negative zero loses its sign for the same reason: `-0.00` and
        `0.00` are one value, and the sign nibble is the only place they
        differ.

        ## The two counts

        `count` is `"declared"` — the field is `digits` nibbles wide and
        the octet count is known before a byte is read — or
        `"value-precision"`, where the value decides and the field's end
        is read out of the octets: [CDR] §9.3.2.8 makes a digit `0` to
        `9` and a sign `0xa` upwards, and the nibble count is even by
        construction, so the field ends at the first octet whose **low**
        nibble is a sign. A field that never presents one within the
        sixteen octets of the widest `fixed` IDL admits is refused.
        """
        if count not in ("declared", "value-precision", "value-string-digits"):
            raise OutOfRange(self._at, f"a fixed digit count, not `{count}`")
        # The same bound as the writer's, on this side too: OMG IDL 4.2
        # rule (42) caps the BCD rule at 31 digits, and a reader asked for
        # a wider field would take octets no declaration can name.
        if digits <= 0 or digits > 31 or scale > digits:
            raise OutOfRange(
                self._at,
                "a fixed<P, S> with S <= P and 1 <= P <= 31, "
                f"not fixed<{digits}, {scale}>",
            )
        nibbles: list[int] = []
        if count in ("value-precision", "value-string-digits"):
            while True:
                if len(nibbles) >= 32:
                    raise OutOfRange(self._at, "a sign nibble within sixteen octets")
                byte = self._take(1)[0]
                nibbles.append(byte >> 4)
                nibbles.append(byte & 0x0F)
                if byte & 0x0F >= 0x0A:
                    break
            sign = nibbles.pop()
        else:
            raw = self._take((digits + 2) // 2)
            for byte in raw:
                nibbles.append(byte >> 4)
                nibbles.append(byte & 0x0F)
            sign = nibbles.pop()
            if (digits + 1) % 2 == 1:
                nibbles.pop(0)
        if any(nibble > 9 for nibble in nibbles):
            raise OutOfRange(self._at, "BCD digits")
        # The point sits `scale` digits from the right and not
        # `digits - scale` from the left: under the value's count the
        # field carries fewer than `digits`, and one that carries fewer
        # than the scale — `1c` under `fixed<5,2>` — needs its integer
        # part written as a zero.
        text = "".join(chr(0x30 + nibble) for nibble in nibbles)
        text = text.rjust(scale + 1, "0")
        split = len(text) - scale
        whole = text[:split].lstrip("0") or "0"
        fraction = text[split:]
        out = whole if not scale else f"{whole}.{fraction}"
        zero = whole == "0" and fraction.strip("0") == ""
        return f"-{out}" if sign == 0x0D and not zero else out

    # -- the trims a bound reaches for -----------------------------------

    def trim_utf8(self, text: str, octets: int) -> str:
        """`text` cut to at most `octets` bytes, never inside a character.

        `zerodds_wire.typesupport.trim_utf8`, reached as a method
        because the shared walk emits the trim as a call on the reader —
        `T::runtime("r", "trim_utf8", …)` in
        `crates/lang-rules/src/emit/decode.rs`. One implementation with
        two ways in, so a caller that reached the wrong one still gets
        Table 11's answer.
        """
        return trim_utf8(text, octets)

    def trim_utf16(self, text: str, units: int) -> str:
        """`text` cut to at most `units` UTF-16 code units.

        The wide half of `trim_utf8`, and a method for the same reason.
        A scalar above the basic plane costs two units and is left out
        whole rather than half-written: half a surrogate pair is not a
        character. §7.2.2.7, Table 11.
        """
        return trim_utf16(text, units)

    # -- the bit-bounded kinds -------------------------------------------

    def read_enum(self, bit_bound: int) -> int:
        """An enumerator's holder, as the signed integer §7.4.3.5.3 makes
        it.

        Rule (5)'s ENUM_TYPE row: "O.holder_type is either Int8, Int16 or
        Int32 depending on the value of the @bit_bound annotation" — all
        three signed, which is why `Writer.write_enum` sends `@value(-1)`
        at the default bound as `ffffffff`. The reader takes the same
        reading, so the two directions of this runtime agree about what
        the holder is.

        **This used to answer the unsigned pattern**, on the argument
        that `@value(-1)` and `@value(255)` under a bound of eight are
        the same byte and that which one was meant is a question about
        the type. Both halves of that are still true and neither needs an
        unsigned holder: no information is lost either way — the two
        readings differ by exactly `1 << bit_bound` — and the decoder
        still decides against its own declared enumerators. What the
        unsigned form did cost was agreement: the five sibling runtimes
        sign-extend here, and the shared codec walk's
        `emit/decode.rs::enumerator_pattern` sign-extends the declared
        value before comparing, so a decoder built from that walk matched
        no arm at all for `@value(-1)` and discarded the sample.
        Measured on `enum-negative-value` and `enum-bitbound-8-value`,
        which the Rust column reads back unchanged.
        """
        if bit_bound == 0 or bit_bound > 32:
            raise OutOfRange(self._at, f"an @bit_bound between 1 and 32, not {bit_bound}")
        if bit_bound <= 8:
            width, pattern = 8, self.read_u8()
        elif bit_bound <= 16:
            width, pattern = 16, self.read_u16()
        else:
            width, pattern = 32, self.read_u32()
        # The holder's width and not the bound's: `@bit_bound(3)` is
        # carried in an Int8, so the sign bit is bit 7 and not bit 2.
        sign = 1 << (width - 1)
        return pattern if pattern < sign else pattern - (1 << width)

    def read_bitmask(self, bit_bound: int) -> int:
        """A bitmask's holder. Table 33 of XTypes 1.3 §7.4.1.1.3.2."""
        if bit_bound == 0 or bit_bound > 64:
            raise OutOfRange(self._at, f"an @bit_bound between 1 and 64, not {bit_bound}")
        if bit_bound <= 8:
            return self.read_u8()
        if bit_bound <= 16:
            return self.read_u16()
        if bit_bound <= 32:
            return self.read_u32()
        return self.read_u64()

    def read_bitset(self, anchor: int) -> int:
        """A bitset's anchor integer, of the width the planner sized."""
        if anchor == 1:
            return self.read_u8()
        if anchor == 2:
            return self.read_u16()
        if anchor == 4:
            return self.read_u32()
        if anchor == 8:
            return self.read_u64()
        raise OutOfRange(self._at, f"a bitset anchor of 1, 2, 4 or 8 bytes, not {anchor}")

    # -- framing ---------------------------------------------------------

    def read_sequence_length(self) -> int:
        """A sequence's element count, refusing one the stream cannot hold.

        A count is one element or more, so a stream with fewer bytes left
        than elements claimed cannot be honest about it. Allocating for it
        is how a reader turns a bad message into an out-of-memory.
        """
        at = self._at
        count = self.read_u32()
        if count > self.remaining():
            raise ImpossibleLength(at, count, self.remaining())
        return count

    def read_dheader(self) -> int:
        """A DHEADER: the length the members that follow occupy."""
        at = self._at
        length = self.read_u32()
        if length > self.remaining():
            raise ImpossibleLength(at, length, self.remaining())
        return length

    def read_emheader(self) -> EmHeaderRead:
        """An EMHEADER, unpacked into its parts and the NEXTINT it carries.

        Codes 5 to 7 report the NEXTINT they read, but the reader is put
        back before it: that word is the member's own leading length and
        the member's decoder has to see it. XTypes 1.3 §7.4.3.4.2.
        """
        word = self.read_u32()
        length_code = (word >> 28) & 0x7
        header = EmHeaderRead(
            member_id=word & 0x0FFF_FFFF,
            length_code=length_code,
            must_understand=(word >> 31) != 0,
            next_int=None,
        )
        if length_code == 4:
            return EmHeaderRead(
                member_id=header.member_id,
                length_code=length_code,
                must_understand=header.must_understand,
                next_int=self.read_u32(),
            )
        if length_code >= 5:
            at = self._at
            peeked = self.read_u32()
            self._at = at
            return EmHeaderRead(
                member_id=header.member_id,
                length_code=length_code,
                must_understand=header.must_understand,
                next_int=peeked,
            )
        return header

    def read_pid(
        self, role: ParameterRole = ParameterRole.MEMBER
    ) -> PidRead:
        """A PL_CDR1 parameter header, in whichever form it was written.

        `role` is the caller's, not the stream's: the bytes are the same
        bytes and only what the declared length means differs. §7.4.1.2.
        """
        self.align_to(4)
        pid = self.read_u16()
        length = self.read_u16()
        if pid & PID_ID_MASK == PID_EXTENDED & PID_ID_MASK:
            # The four flag bits are not part of the id: §7.4.1.2.1
            # splits the `eMemberHeader` into `FLAG_1` to `FLAG_4` and a
            # 28-bit member id, and `FLAG_2` is the member's
            # must-understand. Taking the whole word gave a Cyclone
            # member declared `@must_understand` an id of `0x40000001`,
            # which matches nothing, and the member was dropped.
            member_header = self.read_u32()
            wide_length = self.read_u32()
            return PidRead(
                pid=member_header & E_MEMBER_ID_MASK,
                length=wide_length,
                end=False,
                must_understand=bool(member_header & E_FLAG_MUST_UNDERSTAND),
                # FLAG_1 is FLAG_IMPL_EXTENSION here exactly as bit 15
                # is in the short header. The extended id is 28 bits and
                # has no reserved range.
                impl_extension=bool(member_header & E_FLAG_IMPL_EXTENSION),
                role=role,
            )
        identifier = pid & PID_ID_MASK
        end = identifier == PID_LIST_END & PID_ID_MASK
        return PidRead(
            pid=identifier,
            length=length,
            end=end,
            must_understand=bool(pid & PID_MUST_UNDERSTAND),
            # Under a framing that puts `FLAG_IMPL_EXTENSION` on every
            # member id the bit no longer tells a vendor-private
            # parameter from a user member — OpenDDS writes `c0010004`
            # for `@id(1) long a`. It is reported absent there because
            # under that framing it carries no information, and
            # reporting it would send every member down the
            # unknown-parameter path.
            impl_extension=bool(pid & PID_IMPL_EXTENSION)
            and not pid_flags_at(self._pid_flags, "member") & PID_IMPL_EXTENSION,
            reserved=not end and identifier > self._highest_short_id,
            ignore=identifier == PID_IGNORE,
            role=role,
        )

    def read_pid_as(self, role: ParameterRole) -> PidRead:
        """A parameter header, and the value block it opens.

        **Not the same operation as `read_pid`**, and the difference is a
        wire question rather than a name. §7.4.1.2: a parameter's value
        is its own stream block, so a member's internal alignment is
        counted from the first octet after the header and not from the
        start of the message. `read_pid` leaves the origin alone because
        its caller opens and closes the block itself with
        `begin_parameter_value` and `finish_parameter_value`. The shared
        codec walk has no such pair: it reads a header and later calls
        `finish_pid_read`, so the block has to be opened here.

        Measured on `struct-mutable`, `struct-inheritance-mutable`,
        `identifier-keyword-member-ids` and `evolution-mutable-defaults`
        under PL_CDR1: without the origin move, a `double` at offset 0 of
        its parameter was aligned against the message and the reader
        skipped padding no writer had written — `at 28: out-of-range` for
        the first, and a `double` read four octets late for the second,
        which came back as 5.56e-309 where 3.141592653589793 went in.

        The previous origin travels on the header, so `finish_pid_read`
        can put it back without a stack: parameters do not nest.
        """
        header = self.read_pid(role)
        saved = self._origin
        self._origin = self._at
        return dataclasses.replace(header, saved_origin=saved, value_at=self._at)

    def finish_pid_read(self, header: PidRead) -> None:
        """End a parameter's value and step to the next header.

        The read-side counterpart of `Writer.finish_pid`, and the same
        thing `finish_parameter_value` does to the padding: §7.4.1.2
        puts the padding to four after the value and outside the
        declared length, so the next parameter is found by alignment and
        not by adding the length. That is what lets one reader take
        RTI's rounded-up length and Fast DDS's exact one for the same
        member.

        Nothing follows `PID_LIST_END`, so the sentinel's header steps
        over nothing; and a reader that has already refused does not
        move, because a cursor advanced after a refusal sits where no
        writer put anything and the trailing-octet check would report
        that as a second, invented, fault.

        The block `read_pid_as` opened is closed here: the origin goes
        back to what it was before the header, and only then is the
        padding stepped — aligning to four against the *parameter's* own
        origin would count from the wrong place and land the next header
        at an offset no writer used.

        A header that came from `read_pid` carries a `saved_origin` of
        zero and never moved the origin, so putting it back is what it
        already was — the pair a caller that manages its own block with
        `begin_parameter_value` uses instead of this one.
        """
        self._origin = header.saved_origin
        if self._status is not Status.OK or header.end:
            return
        self.align_to(4)

    def skip_member(self, header: EmHeaderRead) -> None:
        """Step over a member this reader does not know.

        The whole point of the length code. XTypes 1.3 §7.4.3.4.2: codes
        0 to 3 state a size of one, two, four or eight bytes outright;
        code 4 is followed by a NEXTINT holding the member's length; and
        codes 5, 6 and 7 have a NEXTINT that is *also* the member's own
        leading word, so the member spans that word plus the length it
        states, times one, four or eight.
        """
        if header.length_code <= 3:
            self.skip(1 << header.length_code)
            return
        if header.next_int is None:
            raise OutOfRange(self._at, "a NEXTINT for a length code of 4 or above")
        if header.length_code == 4:
            self.skip(header.next_int)
            return
        scale = {5: 1, 6: 4, 7: 8}[header.length_code]
        self.skip(4 + header.next_int * scale)

    def begin_parameter_value(self) -> int:
        """Start reading a parameter's value, returning the origin it
        replaces. The reader's half of `Writer._begin_parameter_value`."""
        saved = self._origin
        self._origin = self._at
        return saved

    def finish_parameter_value(self, saved_origin: int) -> None:
        """End a parameter's value and step over its trailing padding."""
        self._origin = saved_origin
        self.align_to(4)

    def skip(self, count: int) -> None:
        """Step over `count` bytes, refusing to step past the end."""
        self._take(count)

    def seek(self, at: int) -> None:
        """Move to an absolute offset inside the buffer."""
        if at < 0 or at > len(self._bytes):
            raise Truncated(self._at, at, len(self._bytes))
        self._at = at
