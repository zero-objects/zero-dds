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


class CdrWriter:
    """XCDR2-LE Encoder. Alignment zur Schreib-Position."""

    __slots__ = ("buf",)

    def __init__(self) -> None:
        self.buf = bytearray()

    def _align(self, n: int) -> None:
        pad = (-len(self.buf)) % n
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
        self.buf.extend(struct.pack("<H", v & 0xFFFF))

    def write_i16(self, v: int) -> None:
        self._align(2)
        self.buf.extend(struct.pack("<h", v))

    def write_u32(self, v: int) -> None:
        self._align(4)
        self.buf.extend(struct.pack("<I", v & 0xFFFFFFFF))

    def write_i32(self, v: int) -> None:
        self._align(4)
        self.buf.extend(struct.pack("<i", v))

    def write_u64(self, v: int) -> None:
        self._align(8)
        self.buf.extend(struct.pack("<Q", v & 0xFFFFFFFFFFFFFFFF))

    def write_i64(self, v: int) -> None:
        self._align(8)
        self.buf.extend(struct.pack("<q", v))

    def write_f32(self, v: float) -> None:
        self._align(4)
        self.buf.extend(struct.pack("<f", v))

    def write_f64(self, v: float) -> None:
        self._align(8)
        self.buf.extend(struct.pack("<d", v))

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

    def into_bytes(self) -> bytes:
        return bytes(self.buf)


class CdrReader:
    """XCDR2-LE Decoder."""

    __slots__ = ("buf", "pos")

    def __init__(self, data: bytes) -> None:
        self.buf = data
        self.pos = 0

    def _align(self, n: int) -> None:
        self.pos += (-self.pos) % n

    def _take(self, n: int) -> bytes:
        if self.pos + n > len(self.buf):
            raise ValueError(
                f"CDR underrun: need {n} bytes at pos {self.pos}, have {len(self.buf)}",
            )
        out = self.buf[self.pos : self.pos + n]
        self.pos += n
        return out

    def read_bool(self) -> bool:
        return self._take(1)[0] != 0

    def read_u8(self) -> int:
        return self._take(1)[0]

    def read_i8(self) -> int:
        return struct.unpack("<b", self._take(1))[0]

    def read_u16(self) -> int:
        self._align(2)
        return struct.unpack("<H", self._take(2))[0]

    def read_i16(self) -> int:
        self._align(2)
        return struct.unpack("<h", self._take(2))[0]

    def read_u32(self) -> int:
        self._align(4)
        return struct.unpack("<I", self._take(4))[0]

    def read_i32(self) -> int:
        self._align(4)
        return struct.unpack("<i", self._take(4))[0]

    def read_u64(self) -> int:
        self._align(8)
        return struct.unpack("<Q", self._take(8))[0]

    def read_i64(self) -> int:
        self._align(8)
        return struct.unpack("<q", self._take(8))[0]

    def read_f32(self) -> float:
        self._align(4)
        return struct.unpack("<f", self._take(4))[0]

    def read_f64(self) -> float:
        self._align(8)
        return struct.unpack("<d", self._take(8))[0]

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
