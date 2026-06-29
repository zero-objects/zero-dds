"""IDL annotations for Python dataclasses (current variant).

The user defines a normal `@dataclass`, marks the fields
with IDL primitive types, and gets an encoder/decoder that
matches the Rust-side XCDR2-LE byte-exactly.

Example::

    from dataclasses import dataclass
    from zerodds.idl import idl_struct, Int32, String, Bytes

    @idl_struct(typename="sensor_msgs::msg::Temperature")
    @dataclass
    class Temperature:
        celsius: Int32
        sensor_id: String
        raw_blob: Bytes = b""

    topic = participant.create_idl_topic("Temp", Temperature)
    writer = publisher.create_idl_writer(topic)

    writer.write(Temperature(celsius=23, sensor_id="A7", raw_blob=b"\\x01"))

Without the decorator the class stays a normal dataclass.

Scope of this MVP implementation (current variant):

* Field types: `Bool`, `Int8/16/32/64`, `UInt8/16/32/64`,
  `Float32/64`, `String`, `Bytes` (= sequence<octet>).
* Field order: ``@dataclass`` declaration order.
* **Not included (composite extension / v1.4):** nested structs, sequence<T> for
  arbitrary T, fixed-size arrays, unions, Optional.
"""

from __future__ import annotations

from dataclasses import fields, is_dataclass
from typing import Any, Callable, ClassVar, Type, TypeVar, get_args, get_origin

from .cdr import XCDR1_MAX_ALIGNMENT, XCDR2_MAX_ALIGNMENT, CdrReader, CdrWriter

T = TypeVar("T")


# =============================================================================
# IDL primitive types as type annotations. On the Python side these are
# simple aliases for int/float/str/bytes/bool — the decorator inspects
# only the _idl_kind_ markers, not the actual type check.
# =============================================================================


class _IdlKind:
    # `is_primitive` (XTypes 1.3 §7.4.3.5): True only for primitives
    # (bool/int8..int64/uint8..uint64/float/double/char/wchar). Controls
    # DHEADER prepending for `sequence<T>` / `T[N]` — a collection with
    # NON-primitive elements gets a DHEADER under XCDR2 (rule(8)/(12)), a
    # collection of primitives does not. Mirrors cdr-core
    # `CdrEncode::IS_PRIMITIVE` (crates/cdr/src/encode.rs:43).
    __slots__ = ("name", "write", "read", "is_primitive")

    def __init__(
        self,
        name: str,
        write: Callable[[CdrWriter, Any], None],
        read: Callable[[CdrReader], Any],
        is_primitive: bool = False,
    ) -> None:
        self.name = name
        self.write = write
        self.read = read
        self.is_primitive = is_primitive

    def __repr__(self) -> str:
        return f"IdlKind({self.name})"


Bool = _IdlKind("bool", CdrWriter.write_bool, CdrReader.read_bool, True)
Int8 = _IdlKind("int8", CdrWriter.write_i8, CdrReader.read_i8, True)
UInt8 = _IdlKind("uint8", CdrWriter.write_u8, CdrReader.read_u8, True)
Int16 = _IdlKind("int16", CdrWriter.write_i16, CdrReader.read_i16, True)
UInt16 = _IdlKind("uint16", CdrWriter.write_u16, CdrReader.read_u16, True)
Int32 = _IdlKind("int32", CdrWriter.write_i32, CdrReader.read_i32, True)
UInt32 = _IdlKind("uint32", CdrWriter.write_u32, CdrReader.read_u32, True)
Int64 = _IdlKind("int64", CdrWriter.write_i64, CdrReader.read_i64, True)
UInt64 = _IdlKind("uint64", CdrWriter.write_u64, CdrReader.read_u64, True)
Float32 = _IdlKind("float32", CdrWriter.write_f32, CdrReader.read_f32, True)
Float64 = _IdlKind("float64", CdrWriter.write_f64, CdrReader.read_f64, True)
String = _IdlKind("string", CdrWriter.write_string, CdrReader.read_string)
Bytes = _IdlKind("bytes", CdrWriter.write_bytes, CdrReader.read_bytes)
# `octet` is an 8-bit byte — wire-identical to uint8 (§7.4.1.4.1).
Octet = _IdlKind("octet", CdrWriter.write_u8, CdrReader.read_u8, True)
# `char` is a single 8-bit code unit; `wchar`/`wstring` are UTF-16 (§7.4.1.4.1).
Char = _IdlKind("char", CdrWriter.write_char, CdrReader.read_char, True)
WChar = _IdlKind("wchar", CdrWriter.write_u16, CdrReader.read_u16, True)
WString = _IdlKind("wstring", CdrWriter.write_wstring, CdrReader.read_wstring)


class _IdlBoundedString(_IdlKind):
    """``string<N>`` / ``wstring<N>`` — a length-bounded string.

    The bound is the maximum number of code units (characters), matching IDL
    ``string<N>`` semantics (§7.4.1.4.1: bound counts characters, not the CDR
    length prefix or terminator). An over-bound value raises ``ValueError`` on
    encode instead of being silently sent; a decode of an over-bound wire value
    is rejected. ``""`` (empty) always round-trips."""

    __slots__ = ("wide", "bound")

    def __init__(self, bound: int, *, wide: bool = False) -> None:
        self.wide = wide
        self.bound = int(bound)
        if self.bound < 0:
            raise ValueError(f"string bound must be >= 0, got {self.bound}")
        self.name = f"{'wstring' if wide else 'string'}<{self.bound}>"
        self.write = self._write  # type: ignore[assignment]
        self.read = self._read  # type: ignore[assignment]

    def _check(self, s: str, where: str) -> None:
        if len(s) > self.bound:
            raise ValueError(
                f"bounded {self.name}: {where} length {len(s)} exceeds bound "
                f"{self.bound}",
            )

    def _write(self, w: CdrWriter, s: str) -> None:
        self._check(s, "value")
        if self.wide:
            w.write_wstring(s)
        else:
            w.write_string(s)

    def _read(self, r: CdrReader) -> str:
        s = r.read_wstring() if self.wide else r.read_string()
        self._check(s, "wire")
        return s

    def __class_getitem__(cls, bound: Any) -> "_IdlBoundedString":
        return cls(int(bound), wide=False)


class _IdlBoundedWString(_IdlBoundedString):
    """``wstring<N>`` helper so ``BoundedWString[N]`` reads naturally."""

    __slots__ = ()

    def __class_getitem__(cls, bound: Any) -> "_IdlBoundedString":
        return _IdlBoundedString(int(bound), wide=True)


class _IdlFixed(_IdlKind):
    """``fixed<P,S>`` — CORBA/GIOP §9.3.2.7 packed-BCD decimal.

    `P` total digits, `S` after the point. The Python value is the decimal as a
    `str` (e.g. ``"123.45"``), matching the other PSMs; the wire form is
    ``(P+2)//2`` BCD octets, no length prefix, endian-independent. `fixed` is a
    CORBA type (no DDS-XTypes TypeObject), carried by ZeroDDS as a differentiator.
    """

    __slots__ = ("p", "s")

    def __init__(self, p: int, s: int) -> None:
        self.p = int(p)
        self.s = int(s)
        self.name = f"fixed<{self.p},{self.s}>"
        self.write = self._write  # type: ignore[assignment]
        self.read = self._read  # type: ignore[assignment]

    def _write(self, w: CdrWriter, v: Any) -> None:
        w.write_fixed_bcd(str(v), self.p, self.s)

    def _read(self, r: CdrReader) -> str:
        return r.read_fixed_bcd(self.p, self.s)

    def __class_getitem__(cls, params: Any) -> "_IdlFixed":
        p, s = params
        return cls(int(p), int(s))


# =============================================================================
# Composite markers for the composite extension: Sequence, Array, Optional, nested struct.
# All implement `_IdlKind`-compatible write/read at the instance level.
# =============================================================================


class _IdlSequence(_IdlKind):
    """``sequence<T>`` / ``sequence<T, N>`` — u32 length + N elements. `T` is an
    ``_IdlKind`` or an ``@idl_struct``-decorated dataclass type. When ``bound``
    is given the element count is enforced on both encode and decode: a
    ``bound``-exceeding write raises ``ValueError`` (never silently truncates or
    corrupts), and a decode that reads a count past the declared bound is
    rejected as malformed input rather than over-allocating."""

    __slots__ = ("inner", "bound")

    def __init__(self, inner: Any, bound: int | None = None) -> None:
        self.inner = inner
        self.bound = int(bound) if bound is not None else None
        if self.bound is not None and self.bound < 0:
            raise ValueError(f"sequence bound must be >= 0, got {self.bound}")
        suffix = "" if self.bound is None else f", {self.bound}"
        self.name = f"sequence<{_describe(inner)}{suffix}>"
        self.write = self._write  # type: ignore[assignment]
        self.read = self._read  # type: ignore[assignment]

    def _write(self, w: CdrWriter, values: Any) -> None:
        values = list(values or [])
        if self.bound is not None and len(values) > self.bound:
            raise ValueError(
                f"bounded sequence<{_describe(self.inner)}, {self.bound}>: "
                f"got {len(values)} elements, exceeds bound {self.bound}",
            )

        def body(bw: CdrWriter) -> None:
            bw.write_u32(len(values))
            for v in values:
                _write_any(bw, self.inner, v)

        # XCDR2 rule(12): `sequence<T>` with a NON-primitive `T` is framed as
        # DHEADER(byte length) + count + elements; a primitive `T` is just
        # count + elements (cdr-core encode.rs:40-43).
        if _is_primitive_kind(self.inner):
            body(w)
        else:
            _frame_dheader_write(w, body)

    def _read(self, r: CdrReader) -> list:
        if not _is_primitive_kind(self.inner):
            r = r.read_dheader()
        n = r.read_u32()
        if self.bound is not None and n > self.bound:
            raise ValueError(
                f"bounded sequence<{_describe(self.inner)}, {self.bound}>: "
                f"wire count {n} exceeds bound {self.bound}",
            )
        return [_read_any(r, self.inner) for _ in range(n)]

    def __class_getitem__(cls, args: Any) -> "_IdlSequence":
        # `Sequence[T]` (unbounded) or `Sequence[T, N]` (bounded). A 2-tuple was
        # previously swallowed as a single composite `inner`, silently dropping
        # the bound; treat the second element as the bound.
        if isinstance(args, tuple):
            if len(args) != 2:
                raise TypeError(
                    "Sequence[T] or Sequence[T, N] requires one or two parameters",
                )
            inner, bound = args
            return cls(inner, int(bound))
        return cls(args)


class _IdlArray(_IdlKind):
    """``T[N]`` — fixed count, **no** length prefix. Spec XCDR2 §7.4.3."""

    __slots__ = ("inner", "count")

    def __init__(self, inner: Any, count: int) -> None:
        if count <= 0:
            raise ValueError(f"array count must be > 0, got {count}")
        self.inner = inner
        self.count = count
        self.name = f"array<{_describe(inner)}, {count}>"
        self.write = self._write  # type: ignore[assignment]
        self.read = self._read  # type: ignore[assignment]

    def _write(self, w: CdrWriter, values: Any) -> None:
        values = list(values or [])
        if len(values) != self.count:
            raise ValueError(
                f"Array[{self.count}]: expected exactly {self.count} elements, "
                f"got {len(values)}",
            )

        def body(bw: CdrWriter) -> None:
            for v in values:
                _write_any(bw, self.inner, v)

        # XCDR2 §7.4.3.5 r8 (PARRAY) / cdr-core `composite.rs` IS_PRIMITIVE: a
        # fixed array whose LEAF element type is a primitive is a PARRAY — it is
        # tight-packed with NO collection DHEADER, *even when multi-dimensional*
        # (`long[2][3]` = array-of-array-of-long, leaf = long → still PARRAY, no
        # DHEADER). Only an array whose leaf is NON-primitive (a struct, string,
        # …) is DHEADER-framed (no count, since N is fixed). Probing
        # `self.inner` alone would wrongly DHEADER a multi-dim primitive array,
        # because the inner array is itself "non-primitive"; recurse to the leaf.
        if _array_leaf_is_primitive(self):
            body(w)
        else:
            _frame_dheader_write(w, body)

    def _read(self, r: CdrReader) -> list:
        if not _array_leaf_is_primitive(self):
            r = r.read_dheader()
        return [_read_any(r, self.inner) for _ in range(self.count)]

    def __class_getitem__(cls, args: Any) -> "_IdlArray":
        if not isinstance(args, tuple) or len(args) != 2:
            raise TypeError("Array[T, N] requires exactly two parameters")
        inner, count = args
        return cls(inner, int(count))


class _IdlOptional(_IdlKind):
    """``Optional<T>`` — u8 present flag + (if set) value."""

    __slots__ = ("inner",)

    def __init__(self, inner: Any) -> None:
        self.inner = inner
        self.name = f"optional<{_describe(inner)}>"
        self.write = self._write  # type: ignore[assignment]
        self.read = self._read  # type: ignore[assignment]

    def _write(self, w: CdrWriter, value: Any) -> None:
        # XCDR2 rule(20) OPT_FMEMBER: a 1-byte BOOLEAN is_present flag, then —
        # when present — the value at its natural alignment. The trailing
        # align(4) before e.g. a double is produced by the value writer's own
        # `_align` (XTypes 1.3 §7.4.3.5.3; cdr-core optional framing).
        if value is None:
            w.write_u8(0)
            return
        w.write_u8(1)
        _write_any(w, self.inner, value)

    def _read(self, r: CdrReader) -> Any:
        flag = r.read_u8()
        if flag == 0:
            return None
        return _read_any(r, self.inner)

    def __class_getitem__(cls, inner: Any) -> "_IdlOptional":
        return cls(inner)


class _IdlMap(_IdlKind):
    """``map<K, V>`` — u32 entry count + N ``(key, value)`` pairs, entries in
    ascending-key order (matching the Rust/C++ reference encoders, §7.4.4.6).
    ``K``/``V`` are ``_IdlKind`` instances or ``@idl_struct`` dataclasses."""

    __slots__ = ("key", "value", "bound")

    def __init__(self, key: Any, value: Any, bound: int | None = None) -> None:
        self.key = key
        self.value = value
        self.bound = int(bound) if bound is not None else None
        if self.bound is not None and self.bound < 0:
            raise ValueError(f"map bound must be >= 0, got {self.bound}")
        suffix = "" if self.bound is None else f", {self.bound}"
        self.name = f"map<{_describe(key)}, {_describe(value)}{suffix}>"
        self.write = self._write  # type: ignore[assignment]
        self.read = self._read  # type: ignore[assignment]

    def _write(self, w: CdrWriter, mapping: Any) -> None:
        items = list((mapping or {}).items())
        if self.bound is not None and len(items) > self.bound:
            raise ValueError(
                f"bounded map<..., {self.bound}>: got {len(items)} entries, "
                f"exceeds bound {self.bound}",
            )
        # Deterministic, reference-matching order: sort by key.
        items.sort(key=lambda kv: kv[0])

        def body(bw: CdrWriter) -> None:
            bw.write_u32(len(items))
            for k, v in items:
                _write_any(bw, self.key, k)
                _write_any(bw, self.value, v)

        # XCDR2 §7.4.3.5: `map<K,V>` is framed as DHEADER(byte length) + count +
        # (key,value)* ONLY when its (key,value) element is non-primitive. A
        # `map<long,long>` (both primitive) is just count + pairs, NO DHEADER —
        # mirroring `sequence<primitive>` above and cdr-core
        # `needs_collection_dheader(.., K::IS_PRIMITIVE && V::IS_PRIMITIVE)`
        # (FastDDS/OpenDDS-confirmed).
        if _is_primitive_kind(self.key) and _is_primitive_kind(self.value):
            body(w)
        else:
            _frame_dheader_write(w, body)

    def _read(self, r: CdrReader) -> dict:
        if not (_is_primitive_kind(self.key) and _is_primitive_kind(self.value)):
            r = r.read_dheader()
        n = r.read_u32()
        if self.bound is not None and n > self.bound:
            raise ValueError(
                f"bounded map<..., {self.bound}>: wire count {n} exceeds "
                f"bound {self.bound}",
            )
        out: dict = {}
        for _ in range(n):
            k = _read_any(r, self.key)
            v = _read_any(r, self.value)
            out[k] = v
        return out

    def __class_getitem__(cls, args: Any) -> "_IdlMap":
        # `Map[K, V]` (unbounded) or `Map[K, V, N]` (bounded).
        if not isinstance(args, tuple) or len(args) not in (2, 3):
            raise TypeError("Map[K, V] or Map[K, V, N] requires two or three parameters")
        if len(args) == 3:
            key, value, bound = args
            return cls(key, value, int(bound))
        key, value = args
        return cls(key, value)


class _IdlEnum(_IdlKind):
    """Python ``IntEnum`` → XCDR2 signed integer whose width is selected by the
    enum's ``@bit_bound`` (XTypes 1.3 §7.4.5.1 / §7.3.1.2.1.2): a class attribute
    ``_idl_bit_bound`` of N≤8 → int8 (1 octet), N≤16 → int16 (2 octets), else
    int32. The codegen sets ``_idl_bit_bound``; an absent attribute defaults to
    32 (full int32). Cyclone honours this — a fixed int32 broke ``@bit_bound``
    interop.

    Encoding:
    * Write: ``int(enum_value)`` at the holder width.
    * Read: ``EnumCls(raw_int)`` (raises ``ValueError`` on an unknown
      value — this enforces forward-compatible strictness).
    """

    __slots__ = ("enum_cls", "_w", "_r")

    def __init__(self, enum_cls: type) -> None:
        self.enum_cls = enum_cls
        self.name = f"enum<{enum_cls.__name__}>"
        bound = int(getattr(enum_cls, "_idl_bit_bound", 32))
        if bound <= 8:
            self._w, self._r = CdrWriter.write_i8, CdrReader.read_i8
        elif bound <= 16:
            self._w, self._r = CdrWriter.write_i16, CdrReader.read_i16
        else:
            self._w, self._r = CdrWriter.write_i32, CdrReader.read_i32
        self.write = self._write  # type: ignore[assignment]
        self.read = self._read  # type: ignore[assignment]

    def _write(self, w: CdrWriter, value: Any) -> None:
        if value is None:
            raise ValueError(f"Enum {self.enum_cls.__name__} must not be None")
        self._w(w, int(value))

    def _read(self, r: CdrReader) -> Any:
        raw = self._r(r)
        return self.enum_cls(raw)


def _holder_writer_reader(bits: int) -> tuple[Callable, Callable]:
    """Return the unsigned ``(write, read)`` pair for a bitset/bitmask holder of
    ``bits`` total bits, mirroring cdr-core ``bitset_storage_type``
    (crates/idl-rust/src/bitset_emit.rs:171): 0-8→u8, 9-16→u16, 17-32→u32,
    else u64 (XTypes 1.3 §7.4.13 — the holder is the smallest unsigned integer
    that fits the declared/used bit_bound)."""
    if bits <= 8:
        return (CdrWriter.write_u8, CdrReader.read_u8)
    if bits <= 16:
        return (CdrWriter.write_u16, CdrReader.read_u16)
    if bits <= 32:
        return (CdrWriter.write_u32, CdrReader.read_u32)
    return (CdrWriter.write_u64, CdrReader.read_u64)


class _IdlBitmask(_IdlKind):
    """IDL ``bitmask`` (XTypes 1.3 §7.4.13) — an OR-able set of named bit
    positions serialized as an unsigned holder integer. The holder width is the
    smallest unsigned type that fits the number of declared bit values (cdr-core
    ``bitset_storage_type(values.len())``): ≤8 values → uint8, etc. The Python
    value is an ``IntFlag`` member (or any int); on the wire it is just its
    integer bits."""

    __slots__ = ("flag_cls", "_w", "_r")

    def __init__(self, flag_cls: type, bit_count: int) -> None:
        self.flag_cls = flag_cls
        self.name = f"bitmask<{flag_cls.__name__}>"
        self._w, self._r = _holder_writer_reader(bit_count)
        self.is_primitive = True  # holder is a single unsigned integer
        self.write = self._write  # type: ignore[assignment]
        self.read = self._read  # type: ignore[assignment]

    def _write(self, w: CdrWriter, value: Any) -> None:
        if value is None:
            raise ValueError(f"bitmask {self.flag_cls.__name__} must not be None")
        self._w(w, int(value))

    def _read(self, r: CdrReader) -> Any:
        return self.flag_cls(self._r(r))


class _IdlBitset(_IdlKind):
    """IDL ``bitset`` (XTypes 1.3 §7.4.13) — a packed container of bitfields
    serialized as a single unsigned holder integer. The holder width is the
    smallest unsigned type fitting the SUM of the bitfield widths (cdr-core
    ``bitset_storage_type(total_bits)``). The Python value is the already-packed
    integer holder (apps pack/unpack via the generated ``*_Bits`` SHIFT/MASK
    constants)."""

    __slots__ = ("_w", "_r")

    def __init__(self, total_bits: int) -> None:
        self.name = f"bitset<{total_bits}>"
        self._w, self._r = _holder_writer_reader(total_bits)
        self.is_primitive = True
        self.write = self._write  # type: ignore[assignment]
        self.read = self._read  # type: ignore[assignment]

    def _write(self, w: CdrWriter, value: Any) -> None:
        self._w(w, int(value))

    def _read(self, r: CdrReader) -> int:
        return self._r(r)


class _IdlUnion(_IdlKind):
    """Discriminated union (IDL ``union T switch(D)`` §7.4.1.4.4).

    Wire format: discriminator (Int32 or IntEnum) + the value of the
    variant associated with the discriminator.

    Python mapping:

    * An `@idl_union(...)` decorator builds a class with the
      attributes ``discriminator`` and ``value``.
    * The mapping ``cases = {disc_val: (field_name, inner_kind)}``
      says per discriminator value which field is serialized.
    * An optional ``default`` is taken for an unknown discriminator.
    """

    __slots__ = ("cases", "disc_kind", "default", "ext")

    def __init__(
        self,
        disc_kind: Any,
        cases: dict[int, tuple[str, Any]],
        default: Any | None = None,
        extensibility: str = "final",
    ) -> None:
        self.disc_kind = _kind_from_annotation(disc_kind)
        self.cases = {int(k): (v[0], v[1]) for k, v in cases.items()}
        self.default = default
        # @appendable/@mutable unions carry a 4-aligned DHEADER over [disc + branch]
        # (XTypes §7.4.4.5); @final does not. Applies wherever the union appears
        # (top-level OR nested as a member / sequence element).
        self.ext = extensibility
        self.name = f"union<{self.disc_kind.name}>"
        self.write = self._write  # type: ignore[assignment]
        self.read = self._read  # type: ignore[assignment]

    def _resolve_case(self, disc: Any) -> tuple[str, Any] | None:
        key = int(disc)
        if key in self.cases:
            return self.cases[key]
        return self.default

    def _write_body(self, w: CdrWriter, value: Any) -> None:
        if value is None:
            raise ValueError("union value must not be None")
        disc = value.discriminator
        self.disc_kind.write(w, disc)
        case = self._resolve_case(disc)
        if case is None:
            raise ValueError(f"no case for discriminator {disc!r} and no default")
        _fname, inner = case
        _write_any(w, inner, value.value)

    def _write(self, w: CdrWriter, value: Any) -> None:
        if self.ext in ("appendable", "mutable"):
            _frame_dheader_write(w, lambda bw: self._write_body(bw, value))
        else:
            self._write_body(w, value)

    def _read_body(self, r: CdrReader) -> Any:
        disc = self.disc_kind.read(r)
        case = self._resolve_case(disc)
        if case is None:
            raise ValueError(f"no case for discriminator {disc!r} and no default")
        _fname, inner = case
        val = _read_any(r, inner)
        return _UnionValue(discriminator=disc, value=val)

    def _read(self, r: CdrReader) -> Any:
        if self.ext in ("appendable", "mutable"):
            return self._read_body(r.read_dheader())
        return self._read_body(r)


class _UnionValue:
    """Runtime container of a union value. Provides the discriminator
    and the selected case value."""

    __slots__ = ("discriminator", "value")

    def __init__(self, *, discriminator: Any, value: Any) -> None:
        self.discriminator = discriminator
        self.value = value

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, _UnionValue):
            return NotImplemented
        return self.discriminator == other.discriminator and self.value == other.value

    def __repr__(self) -> str:
        return f"_UnionValue(discriminator={self.discriminator!r}, value={self.value!r})"


def idl_union(
    *,
    typename: str,
    discriminator: Any,
    cases: dict[int, tuple[str, Any]],
    default: tuple[str, Any] | None = None,
    extensibility: str = "final",
) -> _IdlKind:
    """Construction helper: creates an `_IdlUnion` IdlKind and additionally
    provides `TYPE_NAME` + a constructor helper for users.

    Example::

        from zerodds.idl import idl_union, Int32, String, Float64

        MyUnion = idl_union(
            typename="u::MyUnion",
            discriminator=Int32,
            cases={0: ("n", Int32), 1: ("s", String)},
            default=("f", Float64),
        )

        # User code:
        val = MyUnion.make(0, 42)       # case 0 → Int32
        encoded = MyUnion.encode(val)
        decoded = MyUnion.decode(encoded)
    """
    kind = _IdlUnion(discriminator, cases, default, extensibility)

    class _UnionFacade:
        """Wrapper around an _IdlUnion, with ``encode``/``decode``/``make``/
        ``TYPE_NAME`` for user code."""

        TYPE_NAME = typename

        @staticmethod
        def encode(v: Any, endian: str = "le") -> bytes:
            w = CdrWriter(endian=endian)
            kind.write(w, v)
            return w.into_bytes()

        @staticmethod
        def decode(b: bytes, endian: str = "le") -> Any:
            r = CdrReader(b, endian=endian)
            return kind.read(r)

        @staticmethod
        def make(disc: Any, value: Any) -> _UnionValue:
            return _UnionValue(discriminator=disc, value=value)

        # Allows use as a nested IDL kind: inner kind in @idl_struct.
        _idl_union_kind = kind

    return _UnionFacade


def _struct_is_framed(struct_cls: type) -> bool:
    """`True` if the aggregate carries a DHEADER on the wire — i.e. its
    extensibility is `appendable` or `mutable` (XTypes 1.3 §7.4.3.5.3 rule(30)).
    `final` aggregates are tight-packed with NO DHEADER (rule(17)/(18)). The
    extensibility is recorded on the class by ``@idl_struct(extensibility=...)``;
    absent / ``"final"`` defaults to unframed (the codegen default)."""
    ext = getattr(struct_cls, "_idl_extensibility", "final")
    return ext in ("appendable", "mutable")


def _write_struct_body(w: CdrWriter, struct_cls: type, value: Any) -> None:
    for fname, kind in struct_cls._idl_fields:  # type: ignore[attr-defined]
        kind.write(w, getattr(value, fname))


def _read_struct_body(r: CdrReader, struct_cls: type) -> Any:
    values = {
        fname: kind.read(r)
        for fname, kind in struct_cls._idl_fields  # type: ignore[attr-defined]
    }
    return struct_cls(**values)


class _IdlStruct(_IdlKind):
    """Nested ``@idl_struct`` — encode/decode via the inner ``encode()``/
    ``decode()`` methods."""

    __slots__ = ("cls",)

    def __init__(self, struct_cls: type) -> None:
        self.cls = struct_cls
        self.name = getattr(struct_cls, "TYPE_NAME", struct_cls.__name__)
        self.write = self._write  # type: ignore[assignment]
        self.read = self._read  # type: ignore[assignment]

    def _write(self, w: CdrWriter, value: Any) -> None:
        if value is None:
            raise ValueError(f"nested struct {self.name} must not be None")
        # We keep writing into the existing buffer (no fresh `encode()` buffer,
        # which would lose stream alignment). An `@appendable`/`@mutable` nested
        # struct gets its own DHEADER frame (rule(30)); a `@final` one is
        # tight-packed (rule(17)/(18)) — matching cdr-core, which only wraps the
        # body in `encode_appendable` for non-final aggregates. A `@mutable`
        # nested struct's framed body is PL_CDR2 — each member carries its own
        # EMHEADER — exactly like the top-level encode path; writing it
        # positionally (the old `_write_struct_body`) silently dropped the
        # nested EMHEADERs and produced a shorter, non-canonical wire form.
        ext = getattr(self.cls, "_idl_extensibility", "final")
        if ext == "mutable":
            kinds = self.cls._idl_fields  # type: ignore[attr-defined]
            mids = self.cls._idl_member_ids  # type: ignore[attr-defined]
            # XCDR1 (max_alignment 8): a nested @mutable struct is PL_CDR1 with no
            # outer DHEADER; XCDR2 wraps the PL_CDR2 EMHEADER body in a DHEADER.
            if w.max_alignment != XCDR2_MAX_ALIGNMENT:
                _write_mutable_body_pl_cdr1(w, kinds, mids, value)
            else:
                _frame_dheader_write(
                    w, lambda bw: _write_mutable_body(bw, kinds, mids, value)
                )
        elif _struct_is_framed(self.cls):
            _frame_dheader_write(w, lambda bw: _write_struct_body(bw, self.cls, value))
        else:
            _write_struct_body(w, self.cls, value)

    def _read(self, r: CdrReader) -> Any:
        # Symmetric to `_write`: a nested `@mutable` struct strips its DHEADER
        # then parses the PL_CDR2 EMHEADER member frame (NOT positional reads,
        # which would mis-parse the first inner EMHEADER as the first field).
        ext = getattr(self.cls, "_idl_extensibility", "final")
        if ext == "mutable":
            kinds = self.cls._idl_fields  # type: ignore[attr-defined]
            mids = self.cls._idl_member_ids  # type: ignore[attr-defined]
            # XCDR1 (max_alignment 8): a nested @mutable struct is PL_CDR1 with no
            # outer DHEADER; XCDR2 strips its DHEADER then parses EMHEADER members.
            if r.max_alignment != XCDR2_MAX_ALIGNMENT:
                return _read_mutable_body_pl_cdr1(r, kinds, mids, self.cls)
            return _read_mutable_body(r.read_dheader(), kinds, mids, self.cls)
        # `read_dheader()` is a no-op under XCDR1, so a framed nested struct
        # decodes the classic no-DHEADER wire.
        if _struct_is_framed(self.cls):
            return _read_struct_body(r.read_dheader(), self.cls)
        return _read_struct_body(r, self.cls)


# Public-Aliases.
Sequence = _IdlSequence
Array = _IdlArray
Optional = _IdlOptional
Map = _IdlMap
BoundedString = _IdlBoundedString
BoundedWString = _IdlBoundedWString
Fixed = _IdlFixed


class Bitset:
    """``Bitset[total_bits]`` brand for an IDL ``bitset`` member — selects the
    unsigned holder width (XTypes 1.3 §7.4.13). The codegen emits the type
    alias as ``Bitset[N]`` so the runtime knows the exact wire width."""

    def __class_getitem__(cls, total_bits: Any) -> _IdlBitset:
        return _IdlBitset(int(total_bits))


def _describe(t: Any) -> str:
    if isinstance(t, _IdlKind):
        return t.name
    if isinstance(t, type) and is_dataclass(t):
        return getattr(t, "TYPE_NAME", t.__name__)
    return repr(t)


def _resolve_inner(kind: Any) -> _IdlKind | None:
    """Resolve a composite element/value spec (the `inner`/`key`/`value` of a
    Sequence/Array/Map/Optional) to a concrete ``_IdlKind``.

    Element specs come from two sources: hand-written ``Sequence[Int32]`` (an
    ``_IdlKind`` instance) AND code-generated / nested aggregates such as a
    nested ``@idl_struct`` dataclass, an ``IntEnum`` class, or a union built by
    ``idl_union(...)`` (a ``_UnionFacade`` carrying ``_idl_union_kind``). The
    plain ``isinstance(_IdlKind)`` / ``is_dataclass`` checks miss the union and
    enum cases, which is why ``sequence<union>`` / ``map<K, union>`` used to
    crash with ``unsupported kind``. Route everything through the same
    annotation resolver used for struct members so the set of supported element
    types is identical at every nesting level.
    """
    if isinstance(kind, _IdlKind):
        return kind
    if isinstance(kind, type) and is_dataclass(kind):
        return _IdlStruct(kind)
    # Union facade, IntEnum/IntFlag class, typing generic, bare primitive — all
    # handled by _kind_from_annotation. Guard with try/except so a genuinely
    # unsupported kind still yields the precise TypeError below.
    try:
        return _kind_from_annotation(kind)
    except TypeError:
        return None


def _is_primitive_kind(kind: Any) -> bool:
    """True if ``kind`` resolves to an XCDR2 primitive (§7.4.3.5). Aggregates,
    strings, enums(holder int32 IS primitive on the wire but enum is modeled as
    a non-collection member, never a sequence element here), structs, unions and
    nested collections are non-primitive → a sequence of them carries a
    DHEADER."""
    resolved = _resolve_inner(kind)
    return bool(resolved is not None and getattr(resolved, "is_primitive", False))


def _array_leaf_is_primitive(arr: "_IdlArray") -> bool:
    """True if a fixed array's LEAF element (descending through nested arrays) is
    an XCDR2 primitive — i.e. the array is a PARRAY (XTypes 1.3 §7.4.3.5 r8) and
    must be tight-packed with NO collection DHEADER, regardless of the number of
    dimensions. `long[2][3]` nests as ``Array[Array[Int32, 3], 2]``; the leaf is
    `Int32` → PARRAY. An array of structs/strings has a non-primitive leaf and IS
    DHEADER-framed."""
    inner = arr.inner
    resolved = _resolve_inner(inner)
    if isinstance(resolved, _IdlArray):
        return _array_leaf_is_primitive(resolved)
    return _is_primitive_kind(inner)


def _frame_dheader_write(w: CdrWriter, body_fn: Callable[[CdrWriter], None]) -> None:
    """Encode ``body_fn`` into a DHEADER-framed sub-buffer (XTypes 1.3
    §7.4.3.4.2; cdr-core ``struct_enc::encode_appendable``). The sub-writer's
    ``align_origin`` is set just past the 4-byte DHEADER so inner member
    alignment is stream-correct (a u64 inside the body still aligns to 4)."""
    # XCDR1 / classic CDR (max_alignment 8) has NO DHEADER — the aggregate body
    # is written inline in the same stream (single message-relative origin), so
    # the framing is a pass-through to the same writer.
    if w.max_alignment != XCDR2_MAX_ALIGNMENT:
        body_fn(w)
        return
    w._align(4)  # DHEADER is a u32 → 4-aligned; do it now so origin is exact.
    inner = CdrWriter(
        max_alignment=w.max_alignment,
        align_origin=w.position() + 4,
        endian="be" if w._bo == ">" else "le",
    )
    body_fn(inner)
    w.write_dheader(inner.into_bytes())


def _write_any(w: CdrWriter, kind: Any, value: Any) -> None:
    resolved = _resolve_inner(kind)
    if resolved is None:
        raise TypeError(f"_write_any: unsupported kind {kind!r}")
    resolved.write(w, value)


def _read_any(r: CdrReader, kind: Any) -> Any:
    resolved = _resolve_inner(kind)
    if resolved is None:
        raise TypeError(f"_read_any: unsupported kind {kind!r}")
    return resolved.read(r)


def _kind_from_annotation(annot: Any) -> _IdlKind:
    """Allows both `field: Int32` and the raw classes."""
    import collections.abc as _collections_abc
    import enum as _enum
    import typing as _typing

    if isinstance(annot, _IdlKind):
        return annot
    # A `ForwardRef` nested inside a generic (e.g. `List[feat_Tree]` where the
    # element is the self-referential type currently being defined). PEP 563/649
    # leaves these unevaluated; the top-level `_resolve` only unwraps the
    # outermost ForwardRef, so the recursion element arrives here still wrapped.
    # Resolve it against the owning class's module globals — that is exactly
    # where the (already partially defined) recursive class lives by the time
    # `@idl_struct` runs (Tree/recursion feature, XTypes 1.3 §7.4.x).
    if isinstance(annot, _typing.ForwardRef):
        import sys as _sys

        name = annot.__forward_arg__
        owner = getattr(annot, "__owner__", None) or getattr(annot, "owner", None)
        ns: dict[str, Any] = {}
        owner_mod = getattr(owner, "__module__", None)
        mod = _sys.modules.get(owner_mod) if owner_mod else None
        if mod is not None:
            ns.update(vars(mod))
        if owner is not None:
            ns[getattr(owner, "__name__", name)] = owner
        if name in ns:
            return _kind_from_annotation(ns[name])

    # typing generics emitted by the code generator: `List[T]` / `Sequence[T]`
    # → sequence, `Dict[K, V]` → map, `Optional[T]` (Union[T, None]) → optional.
    origin = get_origin(annot)
    if origin is not None:
        args = get_args(annot)
        if origin in (list, _typing.List, _collections_abc.Sequence):
            return _IdlSequence(_kind_from_annotation(args[0]))
        if origin in (dict, _typing.Dict, _collections_abc.Mapping):
            return _IdlMap(
                _kind_from_annotation(args[0]),
                _kind_from_annotation(args[1]),
            )
        if origin is _typing.Union:
            non_none = [a for a in args if a is not type(None)]
            if len(non_none) == 1:
                return _IdlOptional(_kind_from_annotation(non_none[0]))
    # A union used AS A STRUCT MEMBER: `idl_union(...)` returns a `_UnionFacade`
    # class carrying the underlying `_IdlUnion` as `_idl_union_kind`. Unwrap it
    # so e.g. `reading: combo_Reading` resolves to the union codec (Bug
    # Q-cluster union-as-member).
    inner_union = getattr(annot, "_idl_union_kind", None)
    if isinstance(inner_union, _IdlKind):
        return inner_union
    # IDL `bitmask` → an `IntFlag` class (the codegen emits `class P(IntFlag)`).
    # It is serialized as an unsigned holder integer whose width fits the number
    # of bit values (XTypes 1.3 §7.4.13), NOT as an Int32 like a plain enum.
    # `IntFlag` is a subclass of `IntEnum`'s sibling, so check it FIRST.
    if isinstance(annot, type) and issubclass(annot, _enum.IntFlag):
        # The holder width is the bitmask's @bit_bound, NOT the flag count. The
        # XTypes 1.3 §7.3.1.2.1.1 DEFAULT bit_bound is 32 → a uint32 holder even
        # for a 3-flag bitmask (the cross-vendor reference: CycloneDDS/RTI/
        # FastDDS all emit 4 bytes). The codegen records the effective bit_bound
        # as `_idl_bit_bound` on the IntFlag class; absent it (hand-written
        # IntFlag) we apply the spec default of 32.
        bit_bound = getattr(annot, "_idl_bit_bound", 32)
        return _IdlBitmask(annot, int(bit_bound))
    # IntEnum class → serialized as Int32.
    if isinstance(annot, type) and issubclass(annot, _enum.IntEnum):
        return _IdlEnum(annot)
    # Nested dataclass → wrapped as an _IdlStruct.
    if isinstance(annot, type) and is_dataclass(annot):
        return _IdlStruct(annot)
    # Fallback: if someone uses `int` / `str` / `bytes` / `bool`,
    # we map it to the natural Rust type.
    if annot is int:
        return Int32
    if annot is bool:
        return Bool
    if annot is float:
        return Float64
    if annot is str:
        return String
    if annot is bytes:
        return Bytes
    raise TypeError(
        f"@idl_struct: field type {annot!r} not supported. "
        f"Use Bool/Int8/.../UInt64/Float32/Float64/String/Bytes, "
        f"Sequence[T], Array[T, N], Optional[T], a nested @idl_struct "
        f"dataclass or standard primitives (int/bool/float/str/bytes).",
    )


# =============================================================================
# Decorator
# =============================================================================


# Compact-EMHEADER length codes per scalar member kind (XTypes 1.3 §7.4.3.4.2;
# cdr-core `mutable_member_length_code`). A fixed-size primitive picks LC0..LC3
# from its wire size; a string/wstring picks LC5 (its uint32 length prefix is
# REUSED as the NEXTINT, so no separate NEXTINT goes on the wire). Anything else
# (aggregate, sequence, array, optional, map, union) falls back to LC4 (a
# separate NEXTINT carrying the full body byte length).
_PRIMITIVE_WIRE_SIZE = {
    "bool": 1, "int8": 1, "uint8": 1, "octet": 1, "char": 1,
    "int16": 2, "uint16": 2, "wchar": 2,
    "int32": 4, "uint32": 4, "float32": 4,
    "int64": 8, "uint64": 8, "float64": 8,
}
_SIZE_TO_LC = {1: 0, 2: 1, 4: 2, 8: 3}


def _member_body_has_leading_dheader(kind: _IdlKind) -> bool:
    """True iff this member's XCDR2 body BEGINS WITH a 4-byte length word — a
    ``string``/``wstring`` length prefix, a non-primitive ``sequence`` / ``map``
    DHEADER, or a nested ``@appendable``/``@mutable`` struct's DHEADER. Such a
    member reuses that leading word as the EMHEADER NEXTINT (LengthCode 5), with
    NO separate NEXTINT — matching CycloneDDS / RTI Connext / FastDDS (cdr-core
    ``type_map::member_body_has_leading_dheader``). A ``@final`` nested struct
    (tight-packed, no DHEADER) and a ``sequence<primitive>`` (bare element count,
    not a byte length) do NOT — they stay on the universal LC4."""
    name = getattr(kind, "name", "")
    # string / wstring (incl. bounded): uint32 octet-length prefix.
    if name.startswith("string") or name.startswith("wstring"):
        return True
    # map<K,V>: a leading DHEADER iff its (key,value) element is non-primitive.
    # A `map<primitive,primitive>` starts with a bare element count (not a byte
    # length) → LC4, exactly like `sequence<primitive>` (XCDR2 §7.4.3.5).
    if isinstance(kind, _IdlMap):
        return not (_is_primitive_kind(kind.key) and _is_primitive_kind(kind.value))
    # sequence<E>: a leading DHEADER iff E is non-primitive (sequence<primitive>
    # starts with a bare element count, which is not a byte length → LC4).
    if isinstance(kind, _IdlSequence):
        return not _is_primitive_kind(kind.inner)
    # nested struct: a leading DHEADER iff @appendable / @mutable (a @final
    # nested struct is tight-packed with no DHEADER).
    if isinstance(kind, _IdlStruct):
        return _struct_is_framed(kind.cls)
    return False


def _member_length_code(kind: _IdlKind) -> int:
    """Pick the compact EMHEADER length code for one ``@mutable`` member,
    mirroring cdr-core ``mutable_member_length_code`` so the Python EMHEADERs are
    byte-identical to the rust/cross-vendor reference. Primitives → LC0..LC3 by
    wire size; any member whose body begins with a 4-byte length word
    (``string``/``wstring``, non-primitive ``sequence``/``map``, nested
    ``@appendable``/``@mutable`` struct) → LC5 (that leading word is reused as the
    NEXTINT, none is serialized separately); a ``bitmask``/``bitset`` holder is a
    single unsigned integer (LC by its holder width); everything else (``@final``
    nested struct, ``sequence<primitive>``, array, optional, union) → LC4 (a
    separate NEXTINT carrying the full body byte length)."""
    name = getattr(kind, "name", "")
    size = _PRIMITIVE_WIRE_SIZE.get(name)
    if size is not None:
        return _SIZE_TO_LC[size]
    if _member_body_has_leading_dheader(kind):
        return 5  # LC5: the body's leading 4-byte length word is the NEXTINT.
    if isinstance(kind, (_IdlBitmask, _IdlBitset)):
        # Holder is an unsigned integer; size it from a trial encode below via
        # the generic LC4 fallback would also work, but a fixed-width holder is
        # a primitive on the wire → use a body-length-derived compact LC.
        sub = CdrWriter()
        kind.write(sub, 0)
        return _SIZE_TO_LC.get(len(sub.into_bytes()), 4)
    return 4  # LC4: separate NEXTINT = body byte length.


def _write_mutable_body(w: CdrWriter, kinds: list, member_ids: list, value: Any) -> None:
    """Write a ``@mutable`` struct body as PL_CDR2: one EMHEADER (compact LC) +
    member-body per member, in declaration order (XTypes 1.3 §7.4.3.4.2;
    cdr-core ``MutableStructEncoder``). Each member body is built in a fresh
    sub-writer starting at stream position 0 — exactly what cdr-core's
    ``encode_mutable_member_lc`` does (``BufferWriter::new(...)`` per member), so
    a 64-bit member aligns relative to its own start. The LC is chosen per member
    kind so the wire matches the cross-vendor reference (long→LC2, double→LC3,
    string→LC5 with no separate NEXTINT), NOT a universal LC4."""
    for (fname, kind), mid in zip(kinds, member_ids):
        sub = CdrWriter(
            max_alignment=w.max_alignment,
            align_origin=0,
            endian="be" if w._bo == ">" else "le",
        )
        kind.write(sub, getattr(value, fname))
        w.write_emheader_lc(mid, _member_length_code(kind), sub.into_bytes())


def _write_mutable_body_pl_cdr1(
    w: CdrWriter, kinds: list, member_ids: list, value: Any
) -> None:
    """Write a ``@mutable`` struct body as XCDR1 PL_CDR1: one
    ``[PID][length]``-framed member per field (each body built member-relative)
    then the PID_LIST_END sentinel. The XCDR1 counterpart of
    :func:`_write_mutable_body`'s PL_CDR2 EMHEADER loop; mirrors cdr-core
    ``xcdr1::encode_pl_cdr1_member`` + ``write_pl_cdr1_sentinel``."""
    for (fname, kind), mid in zip(kinds, member_ids):
        sub = CdrWriter(
            max_alignment=w.max_alignment,
            align_origin=0,
            endian="be" if w._bo == ">" else "le",
        )
        kind.write(sub, getattr(value, fname))
        w.write_pl_cdr1_member(mid, sub.into_bytes())
    w.write_pl_cdr1_sentinel()


def _read_mutable_body(r: CdrReader, kinds: list, member_ids: list, klass: type) -> Any:
    """Read a ``@mutable`` struct body: loop over EMHEADER-framed members,
    dispatch by member id, tolerate unknown non-must-understand members
    (skip), require all declared members present. Mirrors cdr-core
    ``read_mutable_member`` + the generated mutable decode loop."""
    by_id = {mid: (fname, kind) for (fname, kind), mid in zip(kinds, member_ids)}
    values: dict = {}
    while True:
        entry = r.read_mutable_member()
        if entry is None:
            break
        mid, must_understand, body = entry
        slot = by_id.get(mid)
        if slot is None:
            if must_understand:
                raise ValueError(f"unknown must-understand member id {mid}")
            continue  # unknown optional member → skip body
        fname, kind = slot
        values[fname] = kind.read(body)
    for fname, _kind in kinds:
        if fname not in values:
            raise ValueError(f"missing non-optional member {fname!r}")
    return klass(**values)


def _read_mutable_body_pl_cdr1(
    r: CdrReader, kinds: list, member_ids: list, klass: type
) -> Any:
    """Read a ``@mutable`` struct body in XCDR1 PL_CDR1 form: loop over
    ``[PID][length]``-framed members up to the sentinel, dispatch by member id,
    skip unknown members. Mirrors cdr-core ``xcdr1::read_all_pl_cdr1_members`` +
    the generated XCDR1 mutable decode loop (the XCDR1 counterpart of
    :func:`_read_mutable_body`'s EMHEADER loop)."""
    by_id = {mid: (fname, kind) for (fname, kind), mid in zip(kinds, member_ids)}
    values: dict = {}
    while True:
        entry = r.read_pl_cdr1_member()
        if entry is None:
            break
        mid, body = entry
        slot = by_id.get(mid)
        if slot is None:
            continue  # unknown member → skip body
        fname, kind = slot
        values[fname] = kind.read(body)
    for fname, _kind in kinds:
        if fname not in values:
            raise ValueError(f"missing non-optional member {fname!r}")
    return klass(**values)


def idl_struct(
    *,
    typename: str,
    extensibility: str = "final",
    member_ids: "list[int] | None" = None,
) -> Callable[[Type[T]], Type[T]]:
    """Decorator. Turns a `@dataclass` into a ZeroDDS IDL type.

    Adds:
    * ``TYPE_NAME = typename`` (class-level const).
    * ``encode(self) -> bytes`` — XCDR2-LE.
    * ``decode(cls, data: bytes) -> cls`` — classmethod.

    ``extensibility`` (``"final"`` | ``"appendable"`` | ``"mutable"``) controls
    DHEADER framing (XTypes 1.3 §7.4.3.5.3): a `final` aggregate is tight-packed
    with NO DHEADER (rule(17)/(18)); an `appendable`/`mutable` one is wrapped in
    a single DHEADER (rule(30)), both at the top level and when nested. The
    default `final` keeps the compact ZeroDDS-codegen layout (matching e.g.
    ShapeType). Mirrors cdr-core `struct_enc::encode_appendable`.

    The decorator **must be placed after `@dataclass`**, so that it can
    inspect the `__dataclass_fields__` metadata::

        @idl_struct(typename="foo::Bar", extensibility="appendable")
        @dataclass
        class Bar:
            x: Int32
    """
    if extensibility not in ("final", "appendable", "mutable"):
        raise ValueError(
            f"@idl_struct: extensibility must be 'final', 'appendable' or "
            f"'mutable', got {extensibility!r}",
        )

    def apply(cls: Type[T]) -> Type[T]:
        if not is_dataclass(cls):
            raise TypeError(
                f"@idl_struct: {cls.__name__} is not a @dataclass — "
                f"declaration order: @idl_struct(...) above @dataclass.",
            )
        # With `from __future__ import annotations` (PEP 563) or Python 3.14's
        # default deferred annotations (PEP 649), `f.type` arrives as a string
        # or a ForwardRef. We resolve it in the class's module namespace + the
        # zerodds.idl namespace.
        import sys
        import typing as _typing

        module_globals: dict[str, Any] = {}
        mod = sys.modules.get(cls.__module__)
        if mod is not None:
            module_globals.update(vars(mod))
        # Always also include the own IDL-kind constants, so that
        # the user need neither import nor re-export them,
        # as long as they work via `zerodds.<Kind>`.
        module_globals.setdefault("Bool", Bool)
        for _name, _kind in (
            ("Int8", Int8), ("UInt8", UInt8),
            ("Int16", Int16), ("UInt16", UInt16),
            ("Int32", Int32), ("UInt32", UInt32),
            ("Int64", Int64), ("UInt64", UInt64),
            ("Float32", Float32), ("Float64", Float64),
            ("String", String), ("Bytes", Bytes),
            ("Octet", Octet),
            ("Char", Char), ("WChar", WChar), ("WString", WString),
            ("Array", Array), ("Optional", Optional),
            ("Sequence", Sequence), ("Map", Map),
            ("BoundedString", BoundedString), ("BoundedWString", BoundedWString),
            ("Fixed", Fixed),
        ):
            module_globals.setdefault(_name, _kind)

        def _resolve(annot: Any) -> Any:
            # Python 3.14 (PEP 649) defers annotation evaluation by default, so
            # `f.type` arrives as a ForwardRef (e.g. a nested struct/enum/typedef
            # reference) rather than the object. Unwrap it to its source string
            # and evaluate like the explicit-string (PEP 563) case below.
            if isinstance(annot, _typing.ForwardRef):
                annot = annot.__forward_arg__
            if isinstance(annot, str):
                try:
                    return eval(annot, module_globals)  # noqa: S307
                except NameError as exc:
                    raise TypeError(
                        f"@idl_struct: annotation string {annot!r} not "
                        f"resolvable in module {cls.__module__!r}. When using "
                        f"`from __future__ import annotations`, the "
                        f"kind constants must be imported in the module.",
                    ) from exc
            return annot

        kinds: list[tuple[str, _IdlKind]] = []
        for f in fields(cls):
            kinds.append((f.name, _kind_from_annotation(_resolve(f.type))))

        framed = extensibility in ("appendable", "mutable")
        is_mutable = extensibility == "mutable"
        # A `@mutable` struct needs an explicit member id per field (the @id(N)
        # annotation). When the codegen omits them we fall back to the XTypes
        # default `1..=N` (declaration order, 1-based; §7.2.2.4.4).
        mids = (
            list(member_ids)
            if member_ids is not None
            else list(range(1, len(kinds) + 1))
        )
        if len(mids) != len(kinds):
            raise ValueError(
                f"@idl_struct {typename}: member_ids has {len(mids)} entries "
                f"but the struct has {len(kinds)} members",
            )

        def _encode(self: Any, endian: str = "le", representation: int = 1) -> bytes:
            # `representation`: 1 = XCDR2 (the canonical wire), 0 = XCDR1 / classic
            # CDR (alignment cap 8, NO DHEADER on @appendable/@final, PL_CDR1 for
            # @mutable). The writer's max_alignment drives the DHEADER/PL_CDR1
            # decisions (see `_frame_dheader_write`).
            max_align = XCDR1_MAX_ALIGNMENT if representation == 0 else XCDR2_MAX_ALIGNMENT
            w = CdrWriter(endian=endian, max_alignment=max_align)
            # Top-level `@appendable`/`@mutable` aggregate → exactly ONE DHEADER
            # at offset 0 whose body length excludes itself (XTypes 1.3
            # §7.4.3.5.3 rule(30); cdr-core encode_appendable). `@final` →
            # plain tight-packed body. A `@mutable` body is PL_CDR2: each member
            # carries its own EMHEADER inside that outer DHEADER (rule §7.4.3.4.2;
            # cdr-core wraps `MutableStructEncoder` in `encode_appendable`).
            if is_mutable:
                if representation == 0:
                    _write_mutable_body_pl_cdr1(w, kinds, mids, self)
                else:
                    _frame_dheader_write(
                        w, lambda bw: _write_mutable_body(bw, kinds, mids, self)
                    )
            elif framed:
                _frame_dheader_write(
                    w, lambda bw: _write_struct_body(bw, cls, self)
                )
            else:
                _write_struct_body(w, cls, self)
            return w.into_bytes()

        def _decode(
            klass: Type[T],
            data: bytes,
            endian: str = "le",
            representation: int = 1,
        ) -> T:
            # `representation`: 1 = XCDR2 (the canonical wire), 0 = XCDR1 / classic
            # CDR. XCDR1 caps alignment at 8 (not 4), has NO DHEADER on
            # @appendable/@final aggregates or collections, and frames @mutable
            # members as PL_CDR1 (PID/length) instead of XCDR2 EMHEADER.
            max_align = XCDR1_MAX_ALIGNMENT if representation == 0 else XCDR2_MAX_ALIGNMENT
            r = CdrReader(data, endian=endian, max_alignment=max_align)
            if is_mutable:
                if representation == 0:
                    return _read_mutable_body_pl_cdr1(r, kinds, mids, klass)
                return _read_mutable_body(r.read_dheader(), kinds, mids, klass)
            # `read_dheader()` is a no-op for an XCDR1 reader (max_alignment 8),
            # so the `framed` aggregates decode the classic no-DHEADER wire.
            body = r.read_dheader() if framed else r
            values = {fname: kind.read(body) for fname, kind in kinds}
            return klass(**values)  # type: ignore[call-arg]

        cls.TYPE_NAME = typename  # type: ignore[attr-defined]
        cls._idl_extensibility = extensibility  # type: ignore[attr-defined]
        cls._idl_fields = kinds  # type: ignore[attr-defined]
        # Persist the resolved member-id list so a NESTED @mutable struct
        # (reached through `_IdlStruct`, not the top-level encode/decode) can
        # emit/parse the same PL_CDR2 EMHEADER frame as the top level.
        cls._idl_member_ids = mids  # type: ignore[attr-defined]
        cls.encode = _encode  # type: ignore[attr-defined]
        cls.decode = classmethod(_decode)  # type: ignore[attr-defined]
        return cls

    return apply


# =============================================================================
# Runtime-Introspection
# =============================================================================


def is_idl_struct(obj: Any) -> bool:
    """True if ``obj`` (or its class) is decorated with ``@idl_struct``."""
    cls: Any = obj if isinstance(obj, type) else type(obj)
    return hasattr(cls, "TYPE_NAME") and hasattr(cls, "_idl_fields")


def type_name_of(cls_or_obj: Any) -> str:
    """Returns the IDL TYPE_NAME of a decorated dataclass type/object."""
    cls: Any = cls_or_obj if isinstance(cls_or_obj, type) else type(cls_or_obj)
    name: ClassVar[str] = getattr(cls, "TYPE_NAME", None)  # type: ignore[assignment]
    if name is None:
        raise TypeError(f"{cls.__name__} hat keinen @idl_struct(typename=...)-Decorator")
    return name  # type: ignore[return-value]
