"""Minimal XCDR2 little-endian codec for Python.

OMG XTypes 1.3 §7.4. Used by the `@idl_struct` decorator so that
users can pump `@dataclass` types directly over bytes topics,
without writing their own encoder.

**Current scope:**

* Primitives: `bool`, `int8`, `uint8`, `int16`, `uint16`, `int32`,
  `uint32`, `int64`, `uint64`, `float32`, `float64`.
* `str` as `string<N>` with CDR null terminator + length prefix +
  alignment padding.
* `bytes` as `sequence<octet>` (u32 length + bytes).
* CDR alignment rules (natural alignment on 1/2/4/8).

**Not in the current variant:**

* Nested structs (may come via an additional decorator —
  v1.4).
* ``sequence<T>`` for arbitrary T (except octet).
* Fixed-length arrays.
* Optional / discriminated unions.

The codec is byte-exactly compatible with the Rust-side encoder
in `dds_cdr::buffer::BufferWriter` / `BufferReader`.
"""

from __future__ import annotations

import struct


# XCDR2 caps the maximum alignment at 4 (XTypes 1.3 §7.4.1.1.1 / §7.4.3.2.3
# INIT MAXALIGN(2)=4): a 64-bit primitive (double / int64 / uint64) aligns to
# 4, NOT 8. This mirrors the cdr-core reference `BufferWriter::align`
# (crates/cdr/src/buffer.rs:161 — `alignment.min(self.max_alignment)` with
# `XCDR2_MAX_ALIGNMENT == 4`). The over-align-to-8 form is XCDR1.
XCDR2_MAX_ALIGNMENT = 4
XCDR1_MAX_ALIGNMENT = 8


class CdrWriter:
    """XCDR2-LE Encoder. Alignment relative to the write position.

    ``max_alignment`` caps the natural alignment (XCDR2 = 4, §7.4.1.1.1); a
    64-bit primitive therefore aligns to 4, never 8. The IDL codec
    (``zerodds.idl``) always uses XCDR2, matching the cross-vendor-validated
    ``zerodds-cdr`` core. The ``align_origin`` is the absolute stream offset of
    this buffer's byte 0, so a DHEADER body (encoded into a fresh sub-buffer)
    still aligns as if it sat at its true position in the parent stream.
    """

    __slots__ = ("buf", "max_alignment", "align_origin", "_bo")

    def __init__(
        self,
        max_alignment: int = XCDR2_MAX_ALIGNMENT,
        align_origin: int = 0,
        endian: str = "le",
    ) -> None:
        self.buf = bytearray()
        self.max_alignment = max_alignment
        self.align_origin = align_origin
        # struct byte-order prefix: "<" little-endian (canonical XCDR2 wire),
        # ">" big-endian. Threaded into every pack so a big-endian stream
        # carries big-endian primitives, length prefixes and UTF-16 units.
        self._bo = ">" if endian == "be" else "<"

    def _align(self, n: int) -> None:
        # §7.4.1.1.1: effective alignment is min(natural, max_alignment).
        eff = n if n < self.max_alignment else self.max_alignment
        pos = len(self.buf) + self.align_origin
        pad = (-pos) % eff
        if pad:
            self.buf.extend(b"\x00" * pad)

    def write_bool(self, v: bool) -> None:
        self.buf.append(1 if v else 0)

    def write_u8(self, v: int) -> None:
        self.buf.append(v & 0xFF)

    def write_i8(self, v: int) -> None:
        self.buf.append(v & 0xFF)

    def write_u16(self, v: int) -> None:
        self._align(2)
        self.buf.extend(struct.pack(self._bo + "H", v & 0xFFFF))

    def write_i16(self, v: int) -> None:
        self._align(2)
        self.buf.extend(struct.pack(self._bo + "h", v))

    def write_u32(self, v: int) -> None:
        self._align(4)
        self.buf.extend(struct.pack(self._bo + "I", v & 0xFFFFFFFF))

    def write_i32(self, v: int) -> None:
        self._align(4)
        self.buf.extend(struct.pack(self._bo + "i", v))

    def write_u64(self, v: int) -> None:
        self._align(8)
        self.buf.extend(struct.pack(self._bo + "Q", v & 0xFFFFFFFFFFFFFFFF))

    def write_i64(self, v: int) -> None:
        self._align(8)
        self.buf.extend(struct.pack(self._bo + "q", v))

    def write_f32(self, v: float) -> None:
        self._align(4)
        self.buf.extend(struct.pack(self._bo + "f", v))

    def write_f64(self, v: float) -> None:
        self._align(8)
        self.buf.extend(struct.pack(self._bo + "d", v))

    def write_string(self, s: str) -> None:
        """CDR string: u32 length incl. null terminator + UTF-8 bytes + 0x00."""
        encoded = s.encode("utf-8")
        self.write_u32(len(encoded) + 1)
        self.buf.extend(encoded)
        self.buf.append(0)

    def write_bytes(self, b: bytes) -> None:
        """sequence<octet>: u32 length + raw bytes."""
        self.write_u32(len(b))
        self.buf.extend(b)

    def write_fixed_bcd(self, decimal: str, p: int, s: int) -> None:
        """IDL ``fixed<P,S>`` (CORBA/GIOP §9.3.2.7 packed BCD).

        ``decimal`` is the value as a string (e.g. ``"123.45"``, ``"-1.5"``).
        Writes ``(P+2)//2`` octets: P digit nibbles MSB-first + a sign nibble
        (0xC pos, 0xD neg), a leading 0x0 pad when P+1 is odd, packed 2
        nibbles/byte high-first. No length prefix, no alignment,
        endian-independent. Ported from ``crates/cdr/src/fixed.rs``.
        """
        text = decimal
        positive = True
        if text.startswith("-"):
            positive, text = False, text[1:]
        elif text.startswith("+"):
            text = text[1:]
        int_part, _, frac_part = text.partition(".")
        int_needed = p - s
        if len(int_part) > int_needed:
            raise ValueError(f"fixed: integer part '{int_part}' exceeds P-S={int_needed}")
        if len(frac_part) > s:
            raise ValueError(f"fixed: fractional part '{frac_part}' exceeds S={s}")
        digits = int_part.rjust(int_needed, "0") + frac_part.ljust(s, "0")
        nibbles: list[int] = []
        if (p + 1) % 2 == 1:
            nibbles.append(0)
        for c in digits:
            if not c.isdigit():
                raise ValueError(f"fixed: non-digit '{c}'")
            nibbles.append(ord(c) - 48)
        nibbles.append(0x0C if positive else 0x0D)
        for i in range(0, len(nibbles), 2):
            self.buf.append((nibbles[i] << 4) | nibbles[i + 1])

    def write_char(self, c: str) -> None:
        """IDL ``char`` — a single 8-bit code unit (§7.4.1.4.1)."""
        if not isinstance(c, str) or len(c) != 1:
            raise ValueError(f"char must be a single-character str, got {c!r}")
        self.buf.append(ord(c) & 0xFF)

    def write_wstring(self, s: str) -> None:
        """CDR ``wstring`` — u32 byte-length (NO terminator) + UTF-16LE units.

        XCDR2 represents a wide character as a 16-bit (UTF-16) code unit. The
        length prefix is the count of *bytes* that follow, matching the Rust /
        C++ reference encoders; there is no trailing null.
        """
        encoded = s.encode("utf-16-be" if self._bo == ">" else "utf-16-le")
        self.write_u32(len(encoded))
        self.buf.extend(encoded)

    def position(self) -> int:
        """Absolute stream offset of the next byte (incl. ``align_origin``)."""
        return len(self.buf) + self.align_origin

    def write_dheader(self, body: bytes) -> None:
        """Write a DHEADER-framed body: u32 byte length + body (XTypes 1.3
        §7.4.3.4.2 / §7.4.3.5.3 rule(12)/(15)/(30)).

        The DHEADER is itself 4-byte aligned (u32); the body bytes were already
        produced by a sub-writer whose ``align_origin`` was set just past the
        header, so member alignment inside the body is stream-correct. Mirrors
        the cdr-core ``struct_enc::encode_appendable``
        (crates/cdr/src/struct_enc.rs:42).
        """
        self.write_u32(len(body))
        self.buf.extend(body)

    def write_emheader_lc(
        self,
        member_id: int,
        lc: int,
        body: bytes,
        must_understand: bool = False,
    ) -> None:
        """Write one ``@mutable`` member in PL_CDR2 framing with a COMPACT
        length-code (XTypes 1.3 §7.4.3.4.2; cdr-core
        ``struct_enc::encode_mutable_member_lc``).

        EMHEADER = (M_FLAG<<31) | (LC<<28) | member_id (bits27-0). Then:

        * LC0..LC3 — fixed 1/2/4/8-byte body, NO NEXTINT (the body length is
          implied by the LC).
        * LC4      — variable body, a separate NEXTINT u32 = body byte length.
        * LC5/6/7  — variable body whose own leading 4-byte length word (a
          string-length / DHEADER) is REUSED as the NEXTINT — NO separate
          NEXTINT is serialized (this is what CycloneDDS/RTI/FastDDS emit).

        The body bytes were produced by a *fresh* sub-writer starting at stream
        position 0 (cdr-core builds ``BufferWriter::new(...)`` per member — its
        alignment is relative to the member start), so we emit them verbatim
        after the 4-aligned EMHEADER (+ NEXTINT for LC4 only).
        """
        self._align(4)  # EMHEADER is a u32 → 4-aligned.
        m_bit = (1 if must_understand else 0) << 31
        lc_bits = (lc & 0b111) << 28
        emheader = m_bit + lc_bits + (member_id & 0x0FFF_FFFF)
        self.write_u32(emheader)
        if lc <= 3:
            want = (1, 2, 4, 8)[lc]
            if len(body) != want:
                raise ValueError(
                    f"LC{lc} requires exactly {want}-byte body, got {len(body)}",
                )
        elif lc == 4:
            self.write_u32(len(body))  # separate NEXTINT = body byte length.
        else:  # LC5/6/7 — leading word reused as NEXTINT, none serialized here.
            if len(body) < 4:
                raise ValueError(
                    f"LC{lc} member body must start with a 4-byte length word",
                )
        self.buf.extend(body)

    def write_pl_cdr1_member(self, member_id: int, body: bytes) -> None:
        """Write one PL_CDR1 (``@mutable`` XCDR1) member: a 4-byte aligned
        ``[u16 PID][u16 length]`` header (or the PID_EXTENDED long form for ids
        >= 0x3F00 / bodies > 0xFFFF), the member ``body`` bytes, then zero-pad to
        the next 4-byte boundary (the pad is NOT counted in ``length``). The body
        was built by a fresh sub-writer (member-relative alignment). Mirrors
        cdr-core ``xcdr1::encode_pl_cdr1_member``."""
        self._align(4)
        body_len = len(body)
        if member_id >= 0x3F00 or body_len > 0xFFFF:
            self.write_u16(0x3F01)  # PID_EXTENDED
            self.write_u16(8)
            self.write_u32(member_id)
            self.write_u32(body_len)
        else:
            self.write_u16(member_id)
            self.write_u16(body_len)
        self.buf.extend(body)
        pad = (4 - (body_len % 4)) % 4
        for _ in range(pad):
            self.buf.append(0)

    def write_pl_cdr1_sentinel(self) -> None:
        """Write the PID_LIST_END (0x3F02) terminator of a PL_CDR1 member list."""
        self._align(4)
        self.write_u16(0x3F02)
        self.write_u16(0)

    def into_bytes(self) -> bytes:
        return bytes(self.buf)


class CdrReader:
    """XCDR2-LE Decoder.

    Symmetric to :class:`CdrWriter`: ``max_alignment`` caps 64-bit alignment at
    4 (XCDR2, §7.4.1.1.1) and ``align_origin`` lets a DHEADER body be decoded by
    a sub-reader at its true stream offset.
    """

    __slots__ = ("buf", "pos", "max_alignment", "align_origin", "_bo")

    def __init__(
        self,
        data: bytes,
        max_alignment: int = XCDR2_MAX_ALIGNMENT,
        align_origin: int = 0,
        endian: str = "le",
    ) -> None:
        self.buf = data
        self.pos = 0
        self.max_alignment = max_alignment
        self.align_origin = align_origin
        # See Writer._bo — the stream byte order, threaded into every unpack so
        # a big-endian payload is read big-endian (mirrors the writer).
        self._bo = ">" if endian == "be" else "<"

    def _align(self, n: int) -> None:
        eff = n if n < self.max_alignment else self.max_alignment
        abs_pos = self.pos + self.align_origin
        self.pos += (-abs_pos) % eff

    def position(self) -> int:
        """Absolute stream offset of the next byte to read."""
        return self.pos + self.align_origin

    def read_dheader(self) -> "CdrReader":
        """Read a DHEADER (u32 byte length) and return a sub-reader scoped to
        exactly that many body bytes, with ``align_origin`` set so inner member
        alignment matches the parent stream. Mirrors the cdr-core
        ``struct_enc::decode_appendable`` (crates/cdr/src/struct_enc.rs:68).

        XCDR1 / classic CDR (``max_alignment == 8``) has NO DHEADER frame — an
        ``@appendable``/``@final`` aggregate and every collection continue in the
        same stream (the cdr-core writer only emits the DHEADER when
        ``max_alignment() == 4``). For an XCDR1 reader this is a no-op that
        returns ``self`` so every DHEADER call site decodes the classic wire."""
        if self.max_alignment != XCDR2_MAX_ALIGNMENT:
            return self
        length = self.read_u32()
        body = self._take(length)
        return CdrReader(
            bytes(body),
            max_alignment=self.max_alignment,
            align_origin=self.position() - length,
            endian="be" if self._bo == ">" else "le",
        )

    def _take(self, n: int) -> bytes:
        if self.pos + n > len(self.buf):
            raise ValueError(
                f"CDR underrun: need {n} bytes at pos {self.pos}, have {len(self.buf)}",
            )
        out = self.buf[self.pos : self.pos + n]
        self.pos += n
        return out

    def read_mutable_member(self) -> "tuple[int, bool, CdrReader] | None":
        """Read one ``@mutable`` member entry (EMHEADER [+ NEXTINT] + body) and
        return ``(member_id, must_understand, body_reader)`` or ``None`` at the
        end of the enclosing DHEADER frame. Mirrors cdr-core
        ``struct_enc::read_mutable_member`` (crates/cdr/src/struct_enc.rs:389).

        The body sub-reader starts at stream offset 0 (cdr-core decodes each
        member body via a fresh ``BufferReader::new(member.body, ...)``), so a
        64-bit member inside aligns relative to the member start. LC0-3 are
        fixed-size with no NEXTINT; LC4 carries a separate NEXTINT = body byte
        length; LC5/6/7 REUSE the body's own leading 4-byte length word as the
        NEXTINT (the word stays in the body), so the body length is `4 + word`
        (LC5) / `4 + 4*N` (LC6) / `4 + 8*N` (LC7).
        """
        if self.pos >= len(self.buf):
            return None
        emheader = self.read_u32()
        must_understand = (emheader >> 31) & 1 == 1
        lc = (emheader >> 28) & 0b0111
        member_id = emheader & 0x0FFF_FFFF
        if lc <= 3:
            length = (1, 2, 4, 8)[lc]
            body = self._take(length)
        elif lc == 4:
            length = self.read_u32()  # separate NEXTINT = body byte length.
            body = self._take(length)
        else:
            # LC5/6/7: the leading 4-byte length word is part of the body and
            # is reused as the NEXTINT — it is NOT consumed separately. Peek it
            # (without advancing) to size the body, then take word + content.
            self._align(4)
            word = struct.unpack(self._bo + "I", self.buf[self.pos : self.pos + 4])[0]
            if lc == 5:
                content = word
            elif lc == 6:
                content = 4 * word
            else:  # lc == 7
                content = 8 * word
            body = self._take(4 + content)
        return (member_id, must_understand, CdrReader(
            bytes(body),
            max_alignment=self.max_alignment,
            align_origin=0,
            endian="be" if self._bo == ">" else "le",
        ))

    def read_pl_cdr1_member(self) -> "tuple[int, CdrReader] | None":
        """Read one PL_CDR1 (`@mutable` XCDR1) member: a 4-byte aligned header
        ``[u16 PID][u16 length]`` then ``length`` body bytes, padded to the next
        4-byte boundary (the pad is NOT counted in ``length``). Returns
        ``(member_id, body_reader)`` or ``None`` at the ``PID_LIST_END``
        sentinel. ``PID_EXTENDED`` (length=8) carries a 32-bit member id + 32-bit
        body length for ids >= 0x3F00 or bodies > 0xFFFF. Mirrors cdr-core
        ``xcdr1::read_pl_cdr1_member`` (crates/cdr/src/xcdr1.rs:117)."""
        _PID_LIST_END = 0x3F02
        _PID_EXTENDED = 0x3F01
        self._align(4)
        if self.pos + 4 > len(self.buf):
            return None
        pid = self.read_u16()
        len_u16 = self.read_u16()
        if pid == _PID_LIST_END:
            return None
        if pid == _PID_EXTENDED:
            member_id = self.read_u32()
            body_len = self.read_u32()
        else:
            member_id, body_len = pid, len_u16
        body = self._take(body_len)
        # Skip trailing pad to the next 4-byte boundary (tolerate truncation of
        # the final pad at EOF, like the Rust reference).
        pad = (4 - (body_len % 4)) % 4
        for _ in range(pad):
            if self.pos >= len(self.buf):
                break
            self.pos += 1
        return (member_id, CdrReader(
            bytes(body),
            max_alignment=self.max_alignment,
            align_origin=0,
            endian="be" if self._bo == ">" else "le",
        ))

    def read_bool(self) -> bool:
        return self._take(1)[0] != 0

    def read_u8(self) -> int:
        return self._take(1)[0]

    def read_i8(self) -> int:
        return struct.unpack(self._bo + "b", self._take(1))[0]

    def read_u16(self) -> int:
        self._align(2)
        return struct.unpack(self._bo + "H", self._take(2))[0]

    def read_i16(self) -> int:
        self._align(2)
        return struct.unpack(self._bo + "h", self._take(2))[0]

    def read_u32(self) -> int:
        self._align(4)
        return struct.unpack(self._bo + "I", self._take(4))[0]

    def read_i32(self) -> int:
        self._align(4)
        return struct.unpack(self._bo + "i", self._take(4))[0]

    def read_u64(self) -> int:
        self._align(8)
        return struct.unpack(self._bo + "Q", self._take(8))[0]

    def read_i64(self) -> int:
        self._align(8)
        return struct.unpack(self._bo + "q", self._take(8))[0]

    def read_f32(self) -> float:
        self._align(4)
        return struct.unpack(self._bo + "f", self._take(4))[0]

    def read_f64(self) -> float:
        self._align(8)
        return struct.unpack(self._bo + "d", self._take(8))[0]

    def read_string(self) -> str:
        length = self.read_u32()
        if length == 0:
            raise ValueError("CDR string length 0 (missing null terminator)")
        raw = self._take(length)
        if raw[-1] != 0:
            raise ValueError("CDR string missing null terminator")
        return raw[:-1].decode("utf-8")

    def read_bytes(self) -> bytes:
        n = self.read_u32()
        return bytes(self._take(n))

    def read_fixed_bcd(self, p: int, s: int) -> str:
        """IDL ``fixed<P,S>``: read ``(P+2)//2`` packed-BCD octets back into a
        decimal string. Inverse of ``CdrWriter.write_fixed_bcd``."""
        n = (p + 2) // 2
        raw = self._take(n)
        chars: list[str] = []
        sign = "+"
        for i in range(n):
            hi = (raw[i] >> 4) & 0x0F
            lo = raw[i] & 0x0F
            chars.append(chr(48 + (hi % 10)))
            if i == n - 1:
                sign = "-" if lo == 0x0D else "+"
            else:
                chars.append(chr(48 + (lo % 10)))
        while len(chars) > s + 1 and chars[0] == "0":
            chars.pop(0)
        out = "-" if sign == "-" else ""
        if s > 0:
            dot = max(len(chars) - s, 0)
            for i, c in enumerate(chars):
                if i == dot:
                    out += "."
                out += c
        else:
            out += "".join(chars)
        return out

    def read_char(self) -> str:
        """IDL ``char`` — a single 8-bit code unit."""
        return chr(self._take(1)[0])

    def read_wstring(self) -> str:
        """CDR ``wstring`` — u32 byte-length + UTF-16 units (see write_wstring).

        Tolerates an optional leading byte-order mark (some ORBs prepend one);
        defaults to little-endian, matching ``write_wstring``.
        """
        octets = self.read_u32()
        if octets == 0:
            return ""
        if octets % 2 != 0:
            raise ValueError(f"wstring octet length {octets} is not even")
        raw = self._take(octets)
        if raw[:2] == b"\xff\xfe":
            return raw[2:].decode("utf-16-le")
        if raw[:2] == b"\xfe\xff":
            return raw[2:].decode("utf-16-be")
        # No BOM: units are in the message byte order (mirrors write_wstring).
        return raw.decode("utf-16-be" if self._bo == ">" else "utf-16-le")
