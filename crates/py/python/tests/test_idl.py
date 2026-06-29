"""Current variant — IDL dataclass tests.

Central test: the Python serialization of a ShapeType-equivalent
``@dataclass`` must produce **byte-exactly** the same XCDR2-LE bytes
as the Rust side in `crates/dcps/src/interop.rs` (= `dds_dcps::
interop::ShapeType`).

This ensures that a Python publisher and a Rust subscriber can talk
bidirectionally over a ``BytesTopic`` with the correct ``typename`` —
without Rust codegen.
"""

import importlib.util
import pathlib
from dataclasses import dataclass

import pytest

from zerodds.cdr import CdrReader, CdrWriter
from zerodds.idl import Bool, Bytes, Float32, Int32, String, idl_struct


# =============================================================================
# CDR primitive roundtrip
# =============================================================================


def test_cdr_primitive_roundtrip() -> None:
    w = CdrWriter()
    w.write_bool(True)
    w.write_u8(0x7F)
    w.write_i16(-1234)
    w.write_u32(0xDEADBEEF)
    w.write_i64(-1_000_000_000_000)
    w.write_f32(3.14)
    w.write_f64(-2.7182818)
    w.write_string("hello")
    w.write_bytes(b"\x01\x02\x03")
    data = w.into_bytes()

    r = CdrReader(data)
    assert r.read_bool() is True
    assert r.read_u8() == 0x7F
    assert r.read_i16() == -1234
    assert r.read_u32() == 0xDEADBEEF
    assert r.read_i64() == -1_000_000_000_000
    assert abs(r.read_f32() - 3.14) < 1e-5
    assert abs(r.read_f64() - (-2.7182818)) < 1e-12
    assert r.read_string() == "hello"
    assert r.read_bytes() == b"\x01\x02\x03"


def test_cdr_string_alignment_padding() -> None:
    # "AB\0" is 3 raw bytes, then 1 byte of padding to 4-aligned, then x int32.
    # Identical layout to the Rust ShapeType "AB":
    #   04 00 00 00  length=3
    #   41 42 00     "AB\0"
    #   00           pad
    #   01 00 00 00  int32=1
    w = CdrWriter()
    w.write_string("AB")
    w.write_i32(1)
    expected = bytes(
        [
            0x03,
            0x00,
            0x00,
            0x00,  # length = 3 (incl. null)
            0x41,
            0x42,
            0x00,  # "AB\0"
            0x00,  # pad
            0x01,
            0x00,
            0x00,
            0x00,  # int32 = 1
        ],
    )
    assert w.into_bytes() == expected


def test_cdr_reader_rejects_truncated_string() -> None:
    # Length says 3, but only 2 bytes follow.
    data = bytes([0x03, 0x00, 0x00, 0x00, 0x41, 0x42])
    r = CdrReader(data)
    with pytest.raises(ValueError):
        r.read_string()


# =============================================================================
# @idl_struct
# =============================================================================


@idl_struct(typename="ShapeType")
@dataclass
class PyShape:
    color: String  # type: ignore[valid-type]
    x: Int32  # type: ignore[valid-type]
    y: Int32  # type: ignore[valid-type]
    shapesize: Int32  # type: ignore[valid-type]


def test_pyshape_byte_roundtrip() -> None:
    s = PyShape(color="RED", x=42, y=77, shapesize=30)
    encoded = s.encode()
    # Reference exactly as in crates/dcps/tests/shapes_type_wire.rs.
    expected = bytes(
        [
            0x04,
            0x00,
            0x00,
            0x00,  # color.length
            0x52,
            0x45,
            0x44,
            0x00,  # "RED\0"
            0x2A,
            0x00,
            0x00,
            0x00,  # x = 42
            0x4D,
            0x00,
            0x00,
            0x00,  # y = 77
            0x1E,
            0x00,
            0x00,
            0x00,  # shapesize = 30
        ],
    )
    assert encoded == expected, (
        f"Python CDR encoder deviates from the Rust reference.\n"
        f"  got: {encoded.hex(' ')}\n"
        f"  exp: {expected.hex(' ')}"
    )
    # Return path.
    back = PyShape.decode(encoded)
    assert back == s


def test_pyshape_type_name_set_by_decorator() -> None:
    assert PyShape.TYPE_NAME == "ShapeType"


def test_idl_struct_requires_dataclass() -> None:
    # Plain class → error.
    with pytest.raises(TypeError):

        @idl_struct(typename="x")
        class NotAClass:
            x: Int32  # type: ignore[valid-type]

        _ = NotAClass


@idl_struct(typename="zerodds::Sensor")
@dataclass
class Sensor:
    active: Bool  # type: ignore[valid-type]
    reading: Float32  # type: ignore[valid-type]
    label: String  # type: ignore[valid-type]
    raw: Bytes  # type: ignore[valid-type]


def test_sensor_mixed_fields_roundtrip() -> None:
    s = Sensor(active=True, reading=1.5, label="sonar", raw=b"\xAA\xBB\xCC")
    back = Sensor.decode(s.encode())
    assert back.active is True
    assert abs(back.reading - 1.5) < 1e-6
    assert back.label == "sonar"
    assert back.raw == b"\xAA\xBB\xCC"


def test_auto_map_python_primitives() -> None:
    # Without an explicit IDL annotation: `int` → Int32, `str` → String, etc.
    @idl_struct(typename="auto::Test")
    @dataclass
    class Auto:
        n: int
        name: str

    a = Auto(n=7, name="x")
    back = Auto.decode(a.encode())
    assert back == a


# =============================================================================
# Composite extension — composite types: nested struct, sequence, array, optional
# =============================================================================


@idl_struct(typename="geom::Vec3")
@dataclass
class Vec3:
    x: Float32  # type: ignore[valid-type]
    y: Float32  # type: ignore[valid-type]
    z: Float32  # type: ignore[valid-type]


@idl_struct(typename="geom::Pose")
@dataclass
class Pose:
    position: Vec3
    label: String  # type: ignore[valid-type]


def test_nested_struct_roundtrip() -> None:
    p = Pose(position=Vec3(x=1.0, y=2.0, z=3.0), label="origin")
    back = Pose.decode(p.encode())
    assert back == p


from zerodds.idl import Array, Optional, Sequence  # noqa: E402


@idl_struct(typename="container::Grid")
@dataclass
class Grid:
    values: Sequence[Int32]  # type: ignore[valid-type]


def test_sequence_of_primitives_roundtrip() -> None:
    g = Grid(values=[1, 2, 3, 42, -7])
    back = Grid.decode(g.encode())
    assert back.values == g.values


@idl_struct(typename="container::Mesh")
@dataclass
class Mesh:
    points: Sequence[Vec3]  # type: ignore[valid-type]


def test_sequence_of_structs_roundtrip() -> None:
    m = Mesh(points=[Vec3(1.0, 0.0, 0.0), Vec3(0.0, 1.0, 0.0)])
    back = Mesh.decode(m.encode())
    assert back.points == m.points


@idl_struct(typename="container::Fixed")
@dataclass
class Fixed:
    raw: Array[Int32, 4]  # type: ignore[valid-type]


def test_array_fixed_count_roundtrip() -> None:
    f = Fixed(raw=[10, 20, 30, 40])
    back = Fixed.decode(f.encode())
    assert back.raw == f.raw


def test_array_wrong_count_rejected() -> None:
    f = Fixed(raw=[1, 2])
    with pytest.raises(ValueError):
        f.encode()


@idl_struct(typename="container::Maybe")
@dataclass
class Maybe:
    tag: String  # type: ignore[valid-type]
    maybe_num: Optional[Int32]  # type: ignore[valid-type]


def test_optional_present_and_absent() -> None:
    with_value = Maybe(tag="hit", maybe_num=42)
    without = Maybe(tag="miss", maybe_num=None)
    assert Maybe.decode(with_value.encode()) == with_value
    assert Maybe.decode(without.encode()) == without


# =============================================================================
# IntEnum extension — IntEnum as an IDL field type
# =============================================================================


from enum import IntEnum  # noqa: E402


class Severity(IntEnum):
    OK = 0
    WARN = 1
    ERROR = 2


@idl_struct(typename="diag::Event")
@dataclass
class Event:
    code: Int32  # type: ignore[valid-type]
    severity: Severity
    message: String  # type: ignore[valid-type]


def test_enum_roundtrip() -> None:
    e = Event(code=42, severity=Severity.WARN, message="voltage drop")
    back = Event.decode(e.encode())
    assert back == e
    assert back.severity is Severity.WARN


# =============================================================================
# Union extension — discriminated unions
# =============================================================================


from zerodds.idl import Float64, idl_union  # noqa: E402


# Union: disc=0 → Int32 'n', disc=1 → String 's', default → Float64 'f'.
MyUnion = idl_union(
    typename="u::MyUnion",
    discriminator=Int32,
    cases={0: ("n", Int32), 1: ("s", String)},
    default=("f", Float64),
)


def test_union_case_int_roundtrip() -> None:
    v = MyUnion.make(0, 42)
    back = MyUnion.decode(MyUnion.encode(v))
    assert back == v
    assert back.value == 42


def test_union_case_string_roundtrip() -> None:
    v = MyUnion.make(1, "hello")
    back = MyUnion.decode(MyUnion.encode(v))
    assert back.value == "hello"


def test_union_default_branch_used_for_unknown_disc() -> None:
    # Discriminator 99 matches no case → the default (Float64) is taken.
    v = MyUnion.make(99, 3.14)
    back = MyUnion.decode(MyUnion.encode(v))
    assert abs(back.value - 3.14) < 1e-9


def test_union_without_default_rejects_unknown_disc() -> None:
    strict = idl_union(
        typename="u::Strict",
        discriminator=Int32,
        cases={0: ("n", Int32)},
    )
    with pytest.raises(ValueError):
        strict.encode(strict.make(1, 0))


def test_enum_unknown_value_raises() -> None:
    # Hand-built bytes with Severity=99 (not in the enum).
    # code=0 | severity=99 | message="x" → simulated via the _idl_fields-
    # internal encoder with a raw_int instead of an enum variant.
    from zerodds.cdr import CdrWriter

    w = CdrWriter()
    w.write_i32(0)   # code
    w.write_i32(99)  # severity raw
    w.write_string("x")  # message
    raw = w.into_bytes()
    with pytest.raises(ValueError):
        Event.decode(raw)


def test_idl_struct_resolves_pep563_stringified_annotations() -> None:
    # With `from __future__ import annotations`, all field types are
    # strings at runtime. The decorator must resolve them in the module
    # namespace — regression test for the multi-process tests example bug.
    import textwrap
    import types

    import sys

    mod = types.ModuleType("_pep563_probe")
    sys.modules["_pep563_probe"] = mod
    mod.__dict__["__name__"] = "_pep563_probe"
    src = textwrap.dedent(
        """
        from __future__ import annotations
        from dataclasses import dataclass
        from zerodds.idl import idl_struct, Int32, String

        @idl_struct(typename="probe::T")
        @dataclass
        class Probe:
            n: Int32
            label: String
        """,
    )
    exec(src, mod.__dict__)  # noqa: S102
    p = mod.Probe(n=7, label="abc")
    assert mod.Probe.decode(p.encode()) == p


# =============================================================================
# Bug K / Bug P — map<K,V> + typing-generic field annotations roundtrip
# =============================================================================


def test_map_field_roundtrips() -> None:
    """``map<string,long>`` resolves from a ``Dict[String, Int32]`` annotation
    and round-trips (Bug K + Bug P)."""
    from typing import Dict, List

    @idl_struct(typename="conf::Scores")
    @dataclass
    class Scores:
        ids: List[Int32]
        table: Dict[String, Int32]

    v = Scores(ids=[1, 2, 3], table={"b": 20, "a": 10})
    back = Scores.decode(Scores.encode(v))
    assert back.ids == [1, 2, 3]
    assert back.table == {"a": 10, "b": 20}


def test_map_is_key_sorted_on_the_wire() -> None:
    """Map entries serialise in ascending-key order regardless of insertion
    order (matches the Rust/C++ reference encoders, §7.4.4.6)."""
    from typing import Dict

    @idl_struct(typename="conf::M")
    @dataclass
    class M:
        table: Dict[Int32, Int32]

    a = M.encode(M(table={3: 30, 1: 10, 2: 20}))
    b = M.encode(M(table={1: 10, 2: 20, 3: 30}))
    assert a == b


# =============================================================================
# Bug R / Bug Q — ForwardRef (PEP 649) nested refs + octet brand roundtrip
# =============================================================================


def test_nested_struct_forwardref_roundtrips() -> None:
    """A field referencing another @idl_struct resolves even when Python 3.14
    delivers the annotation as a ForwardRef (Bug R)."""

    @idl_struct(typename="conf::Inner")
    @dataclass
    class Inner:
        x: Int32

    @idl_struct(typename="conf::Outer")
    @dataclass
    class Outer:
        a: Inner
        b: Int32

    v = Outer(a=Inner(x=42), b=7)
    back = Outer.decode(Outer.encode(v))
    assert back.a.x == 42
    assert back.b == 7


def test_octet_brand_roundtrips() -> None:
    """`octet` is exported and wire-identical to uint8 (Bug Q)."""
    from zerodds.idl import Octet

    @idl_struct(typename="conf::Raw")
    @dataclass
    class Raw:
        b: Octet

    assert Raw.decode(Raw.encode(Raw(b=0xAB))).b == 0xAB


# =============================================================================
# Bug Q-cluster (#60) — runtime brand additions: Char, WChar, WString.
# =============================================================================


def test_char_brand_roundtrips() -> None:
    from zerodds.idl import Char

    @idl_struct(typename="conf::C")
    @dataclass
    class C:
        c: Char

    assert C.decode(C.encode(C(c="Q"))).c == "Q"


def test_wstring_brand_roundtrips_utf16() -> None:
    from zerodds.idl import WString

    @idl_struct(typename="conf::W")
    @dataclass
    class W:
        w: WString

    # Include a non-Latin code point to prove UTF-16 (not Latin-1) encoding.
    sample = "héllo-ä€"
    assert W.decode(W.encode(W(w=sample))).w == sample


def test_wstring_empty_roundtrips() -> None:
    from zerodds.idl import WString

    @idl_struct(typename="conf::W")
    @dataclass
    class W:
        w: WString

    assert W.decode(W.encode(W(w=""))).w == ""


def test_union_as_struct_member_resolves_and_roundtrips() -> None:
    """A union (idl_union facade) used AS a struct member resolves through
    `_kind_from_annotation` and roundtrips (Bug Q-cluster union-as-member)."""
    from zerodds.idl import Int32, idl_union

    Reading = idl_union(
        typename="conf::Reading",
        discriminator=Int32,
        cases={0: ("idleTicks", Int32), 1: ("activeRate", Int32)},
        default=None,
    )

    @idl_struct(typename="conf::Telemetry")
    @dataclass
    class Telemetry:
        seq: Int32
        reading: Reading

    v = Telemetry(seq=5, reading=Reading.make(1, 99))
    back = Telemetry.decode(Telemetry.encode(v))
    assert back.seq == 5
    assert back.reading.discriminator == 1
    assert back.reading.value == 99


def test_fixed_array_brand_omits_length_prefix() -> None:
    """Array[T, N] writes N elements with NO length prefix; List[T] writes a
    u32 count. The byte counts must differ — proves the codegen choosing Array
    over List is observable on the wire (Bug Q-cluster fixed array)."""
    from zerodds.idl import Array, Int32

    @idl_struct(typename="conf::Arr")
    @dataclass
    class Arr:
        v: Array[Int32, 3]

    enc = Arr.encode(Arr(v=[1, 2, 3]))
    # 3 * 4 bytes, no 4-byte length prefix.
    assert len(enc) == 12
    assert Arr.decode(enc).v == [1, 2, 3]


# =============================================================================
# Generated-module roundtrips — import the ACTUAL codegen output and run real
# encode→decode roundtrips against the live runtime. The modules are emitted by
# the `gen_for_pytest` Rust test (run `cargo test -p zerodds-idl-python` first).
# =============================================================================

_GEN_DIR = pathlib.Path(__file__).parent / "_generated"


def _load_generated(name: str):
    path = _GEN_DIR / f"{name}.py"
    if not path.exists():
        pytest.skip(
            f"{path} not generated yet — run `cargo test -p zerodds-idl-python` first",
        )
    spec = importlib.util.spec_from_file_location(f"_gen_{name}", path)
    assert spec is not None and spec.loader is not None
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def test_generated_optionals_roundtrip() -> None:
    """Bug R5 (#64): generated @optional members roundtrip present + absent."""
    mod = _load_generated("optionals_gen")
    Optionals = mod.conf_Optionals
    # Present value.
    v = Optionals(required=1, maybe=7, note="hi")
    back = Optionals.decode(Optionals.encode(v))
    assert (back.required, back.maybe, back.note) == (1, 7, "hi")
    # Absent (None) value — exercises the u8 presence flag.
    v2 = Optionals(required=2, maybe=None, note=None)
    back2 = Optionals.decode(Optionals.encode(v2))
    assert (back2.required, back2.maybe, back2.note) == (2, None, None)


def test_generated_consts_are_module_constants() -> None:
    """Bug M (#56): const decls become importable module constants."""
    mod = _load_generated("consts_gen")
    assert mod.MAX_ITEMS == 10
    assert mod.RATE == 2.5
    assert mod.ENABLED is True
    assert mod.conf_N == 4


def test_generated_combo_module_imports_and_roundtrips() -> None:
    """Bug Q-cluster (#60): the module-wrapped combo type imports cleanly under
    py3.14 deferred annotations and roundtrips, exercising flattened references,
    a typedef member, a union member, an @optional member and a fixed array."""
    mod = _load_generated("combo_gen")
    Telemetry = mod.combo_Telemetry
    Mode = mod.combo_Mode
    Reading = mod.combo_Reading
    Sample = mod.combo_Sample

    v = Telemetry(
        unitId=11,
        mode=Mode.MODE_ACTIVE,
        batteryCurrent=2.5,
        history=[Sample(seq=1, value=1.5), Sample(seq=2, value=2.5)],
        reading=Reading.make(Mode.MODE_IDLE, 42),
        calibration=None,
        window=[10, 20, 30, 40],
    )
    back = Telemetry.decode(Telemetry.encode(v))
    assert back.unitId == 11
    assert back.mode == Mode.MODE_ACTIVE
    assert back.batteryCurrent == 2.5
    assert [s.seq for s in back.history] == [1, 2]
    assert back.reading.discriminator == Mode.MODE_IDLE
    assert back.reading.value == 42
    assert back.calibration is None
    assert back.window == [10, 20, 30, 40]


def test_generated_chars_roundtrip() -> None:
    """Bug Q-cluster: generated char + wstring members roundtrip."""
    mod = _load_generated("chars_gen")
    Chars = mod.Chars
    v = Chars(c="Z", w="wide-ä")
    back = Chars.decode(Chars.encode(v))
    assert back.c == "Z"
    assert back.w == "wide-ä"


def test_generated_arrays_roundtrip() -> None:
    """Bug Q-cluster: generated multidim + array-of-struct members roundtrip."""
    mod = _load_generated("arrays_gen")
    Arrays = mod.conf_Arrays
    Point = mod.conf_Point
    v = Arrays(
        grid=[[1, 2], [3, 4]],
        shape=[Point(x=1, y=2), Point(x=3, y=4)],
    )
    back = Arrays.decode(Arrays.encode(v))
    assert back.grid == [[1, 2], [3, 4]]
    assert [(p.x, p.y) for p in back.shape] == [(1, 2), (3, 4)]


# =============================================================================
# Adversarial edge sweep — the cases that break adapters past hello-world.
# Hand-written runtime tests (the brand semantics) + generated-module
# roundtrips (the codegen emits those brands).
# =============================================================================


def test_union_as_sequence_element_roundtrips() -> None:
    """Checklist 5: a union used AS a sequence element. Previously
    `_write_any`/`_read_any` only knew `_IdlKind`/dataclass and crashed with
    `unsupported kind <_UnionFacade>` for `sequence<union>`."""
    from zerodds.idl import Sequence

    u = idl_union(
        typename="u::Elem",
        discriminator=Int32,
        cases={0: ("n", Int32), 1: ("s", String), 2: ("seq", Sequence[Int32])},
        default=("f", Float64),
    )

    @idl_struct(typename="e::UnionSeq")
    @dataclass
    class UnionSeq:
        items: Sequence[u]

    v = UnionSeq(items=[u.make(0, 1), u.make(1, "z"), u.make(2, [5, 6]), u.make(9, 2.5)])
    back = UnionSeq.decode(UnionSeq.encode(v))
    assert [it.value for it in back.items] == [1, "z", [5, 6], 2.5]
    assert [it.discriminator for it in back.items] == [0, 1, 2, 9]


def test_union_as_map_value_roundtrips() -> None:
    """Checklist 5: a union used AS a map value (same `_resolve_inner` path)."""
    from zerodds.idl import Map

    u = idl_union(
        typename="u::MapVal",
        discriminator=Int32,
        cases={0: ("n", Int32), 1: ("s", String)},
        default=("f", Float64),
    )

    @idl_struct(typename="e::UnionMap")
    @dataclass
    class UnionMap:
        m: Map[String, u]

    v = UnionMap(m={"a": u.make(0, 5), "b": u.make(1, "q")})
    back = UnionMap.decode(UnionMap.encode(v))
    assert back.m["a"].value == 5
    assert back.m["b"].value == "q"


def test_enum_as_sequence_element_roundtrips() -> None:
    """Regression for the `_resolve_inner` path: an IntEnum element (not an
    `_IdlKind` instance) must resolve inside a sequence."""
    from zerodds.idl import Sequence

    class Color(IntEnum):
        RED = 0
        GREEN = 1
        BLUE = 2

    @idl_struct(typename="e::EnumSeq")
    @dataclass
    class EnumSeq:
        cs: Sequence[Color]

    v = EnumSeq(cs=[Color.RED, Color.BLUE, Color.GREEN])
    back = EnumSeq.decode(EnumSeq.encode(v))
    assert back.cs == [Color.RED, Color.BLUE, Color.GREEN]
    assert all(isinstance(c, Color) for c in back.cs)


def test_bounded_sequence_exact_bound_and_overflow() -> None:
    """Checklist 2: bounded sequence<T, N> filled exactly to N (ok), empty
    (ok), and over N (must raise — never silently corrupt)."""
    from zerodds.idl import Sequence

    @idl_struct(typename="e::BSeq")
    @dataclass
    class BSeq:
        s: Sequence[Int32, 3]

    assert BSeq.decode(BSeq.encode(BSeq(s=[1, 2, 3]))).s == [1, 2, 3]
    assert BSeq.decode(BSeq.encode(BSeq(s=[]))).s == []
    with pytest.raises(ValueError):
        BSeq(s=[1, 2, 3, 4]).encode()


def test_bounded_sequence_rejects_overbound_wire() -> None:
    """A malformed wire frame whose count exceeds the bound must be rejected on
    decode, not over-allocated."""
    from zerodds.cdr import CdrWriter
    from zerodds.idl import Sequence

    @idl_struct(typename="e::BSeqW")
    @dataclass
    class BSeqW:
        s: Sequence[Int32, 3]

    w = CdrWriter()
    w.write_u32(4)  # 4 > bound 3
    for x in (1, 2, 3, 4):
        w.write_i32(x)
    with pytest.raises(ValueError):
        BSeqW.decode(w.into_bytes())


def test_bounded_string_exact_and_overflow() -> None:
    """Checklist 2: string<N> at exactly N and empty roundtrip; over N raises."""
    from zerodds.idl import BoundedString

    @idl_struct(typename="e::BStr")
    @dataclass
    class BStr:
        s: BoundedString[5]

    assert BStr.decode(BStr.encode(BStr(s="hello"))).s == "hello"
    assert BStr.decode(BStr.encode(BStr(s=""))).s == ""
    with pytest.raises(ValueError):
        BStr(s="toolong").encode()


def test_bounded_wstring_exact_and_overflow_unicode() -> None:
    """Checklist 2 + 6: bounded wstring<N> counts code units; CJK at exactly N
    roundtrips, over N raises."""
    from zerodds.idl import BoundedWString

    @idl_struct(typename="e::BWStr")
    @dataclass
    class BWStr:
        s: BoundedWString[3]

    assert BWStr.decode(BWStr.encode(BWStr(s="日本語"))).s == "日本語"
    with pytest.raises(ValueError):
        BWStr(s="日本語X").encode()


def test_bounded_map_exact_and_overflow() -> None:
    """Checklist 2: map<K, V, N> at exactly N and empty roundtrip; over N raises."""
    from zerodds.idl import Map

    @idl_struct(typename="e::BMap")
    @dataclass
    class BMap:
        m: Map[String, Int32, 2]

    assert BMap.decode(BMap.encode(BMap(m={"a": 1, "b": 2}))).m == {"a": 1, "b": 2}
    assert BMap.decode(BMap.encode(BMap(m={}))).m == {}
    with pytest.raises(ValueError):
        BMap(m={"a": 1, "b": 2, "c": 3}).encode()


def test_empty_collections_all_roundtrip() -> None:
    """Checklist 1: empty sequence, empty string, empty wstring, empty map all
    survive count=0 without crashing."""
    from zerodds.idl import Map, Sequence, WString

    @idl_struct(typename="e::Empties")
    @dataclass
    class Empties:
        seq: Sequence[Int32]
        s: String
        ws: WString
        m: Map[String, Int32]

    v = Empties(seq=[], s="", ws="", m={})
    back = Empties.decode(Empties.encode(v))
    assert back.seq == [] and back.s == "" and back.ws == "" and back.m == {}


def test_generated_bounds_roundtrip_and_enforced() -> None:
    """Checklist 2 (codegen): generated string<N>/sequence<T,N>/map<K,V,N>/
    wstring<N> roundtrip at bound and reject over-bound."""
    mod = _load_generated("bounds_gen")
    Bounded = mod.b_Bounded
    v = Bounded(
        nums=[1, 2, 3],
        name="hello",
        wname="日本語",
        counters={"a": 1, "b": 2},
        unbounded=[9, 8, 7, 6, 5],
        fullstr="anything at all",
    )
    back = Bounded.decode(Bounded.encode(v))
    assert back.nums == [1, 2, 3]
    assert back.name == "hello"
    assert back.wname == "日本語"
    assert back.counters == {"a": 1, "b": 2}
    assert back.unbounded == [9, 8, 7, 6, 5]
    assert back.fullstr == "anything at all"

    def _over(**kw):
        base = {
            "nums": [1],
            "name": "x",
            "wname": "y",
            "counters": {"a": 1},
            "unbounded": [],
            "fullstr": "",
        }
        base.update(kw)
        return Bounded(**base)

    for kw in (
        {"nums": [1, 2, 3, 4]},
        {"name": "toolong"},
        {"wname": "日本語X"},
        {"counters": {"a": 1, "b": 2, "c": 3}},
    ):
        with pytest.raises(ValueError):
            _over(**kw).encode()


def test_generated_deep_nesting_roundtrips() -> None:
    """Checklist 3 (codegen): struct->struct->struct, sequence<sequence<struct>>,
    map<string, struct-with-a-sequence>."""
    mod = _load_generated("deepnest_gen")
    Deep = mod.d_Deep
    L1 = mod.d_L1
    L2 = mod.d_L2
    L3 = mod.d_L3
    HasSeq = mod.d_HasSeq

    v = Deep(
        chain=L1(inner=L2(inner=L3(v=7))),
        grid=[[L3(v=1), L3(v=2)], [], [L3(v=3)]],
        table={"a": HasSeq(xs=[1, 2]), "b": HasSeq(xs=[])},
    )
    back = Deep.decode(Deep.encode(v))
    assert back.chain.inner.inner.v == 7
    assert [[e.v for e in row] for row in back.grid] == [[1, 2], [], [3]]
    assert back.table["a"].xs == [1, 2]
    assert back.table["b"].xs == []


def test_generated_optional_aggregate_present_and_absent() -> None:
    """Checklist 4 (codegen): @optional of nested struct / sequence / map /
    string — present AND absent both roundtrip; absent stays None."""
    mod = _load_generated("optagg_gen")
    OptAgg = mod.o_OptAgg
    Inner = mod.o_Inner

    present = OptAgg(nested=Inner(v=9), nums=[1, 2], table={"k": 1}, note="hi")
    back_p = OptAgg.decode(OptAgg.encode(present))
    assert back_p.nested.v == 9
    assert back_p.nums == [1, 2]
    assert back_p.table == {"k": 1}
    assert back_p.note == "hi"

    absent = OptAgg(nested=None, nums=None, table=None, note=None)
    back_a = OptAgg.decode(OptAgg.encode(absent))
    assert back_a.nested is None
    assert back_a.nums is None
    assert back_a.table is None
    assert back_a.note is None


def test_generated_unicode_codepoints_survive() -> None:
    """Checklist 6 (codegen): multi-byte UTF-8 in string + UTF-16 in wstring;
    exact code points survive (CJK + emoji)."""
    mod = _load_generated("unicode_gen")
    Uni = mod.Uni
    text = "日本語🚀café"
    v = Uni(s=text, w=text)
    back = Uni.decode(Uni.encode(v))
    assert back.s == text
    assert back.w == text
    assert [ord(c) for c in back.s] == [ord(c) for c in text]


def test_generated_extreme_primitives_roundtrip() -> None:
    """Checklist 8 (codegen): integer min/max/0/-1 across all widths + a normal
    float/double."""
    mod = _load_generated("extremes_gen")
    Extremes = mod.Extremes
    v = Extremes(
        i8=-128,
        u8=255,
        i16=-32768,
        u16=65535,
        i32=-(2**31),
        u32=2**32 - 1,
        i64=-(2**63),
        u64=2**64 - 1,
        f=1.5,
        dbl=3.141592653589793,
    )
    back = Extremes.decode(Extremes.encode(v))
    assert back.i8 == -128 and back.u8 == 255
    assert back.i16 == -32768 and back.u16 == 65535
    assert back.i32 == -(2**31) and back.u32 == 2**32 - 1
    assert back.i64 == -(2**63) and back.u64 == 2**64 - 1
    assert back.f == 1.5
    assert back.dbl == 3.141592653589793

    # zero / -1 edge.
    z = Extremes(i8=0, u8=0, i16=0, u16=0, i32=0, u32=0, i64=-1, u64=0, f=0.0, dbl=-1.0)
    backz = Extremes.decode(Extremes.encode(z))
    assert backz.i64 == -1 and backz.dbl == -1.0 and backz.u32 == 0


def test_generated_keyed_same_key_different_payload() -> None:
    """Checklist 9 (codegen): two samples with the same @key, different payload
    — the key fields roundtrip identically."""
    mod = _load_generated("keyed_gen")
    Keyed = mod.Keyed
    a = Keyed(id=7, label="dev", payload=100)
    b = Keyed(id=7, label="dev", payload=999)
    back_a = Keyed.decode(Keyed.encode(a))
    back_b = Keyed.decode(Keyed.encode(b))
    assert back_a.id == back_b.id == 7
    assert back_a.label == back_b.label == "dev"
    assert back_a.payload == 100
    assert back_b.payload == 999


# =============================================================================
# XCDR2 canonical-layout regression (Bug XW cross-PSM convergence).
# The exact bytes below come from the cross-vendor-validated rust reference
# golden (internal/idl-codegen/xcdr2-canonical-layout.md), the conform target
# for all 7 backends. Pins the spec framing so the runtime can't regress.
# =============================================================================


def test_xcdr2_max_align_caps_double_at_four() -> None:
    """XTypes 1.3 §7.4.1.1.1: XCDR2 MAXALIGN = 4 — a double aligns to 4, never
    8. After a u8 + u32 (5 bytes) the double pads only to offset 8, not 16."""
    from zerodds.idl import Float64, UInt8, UInt32

    @idl_struct(typename="x::AlignProbe")
    @dataclass
    class AlignProbe:
        a: UInt8  # type: ignore[valid-type]
        b: UInt32  # type: ignore[valid-type]
        c: Float64  # type: ignore[valid-type]

    enc = AlignProbe(a=1, b=2, c=1.0).encode()
    # off 0: a=01 ; pad 3 to 4 ; off 4: b=02000000 ; off 8: double (NOT off 16).
    assert enc[0] == 0x01
    assert enc[1:4] == b"\x00\x00\x00"
    assert enc[4:8] == b"\x02\x00\x00\x00"
    assert len(enc) == 16, f"double over-aligned to 8: {enc.hex(' ')}"
    assert AlignProbe.decode(enc).c == 1.0


def test_xcdr2_appendable_emits_single_dheader() -> None:
    """XTypes 1.3 §7.4.3.5.3 rule(30): a top-level @appendable aggregate carries
    exactly ONE DHEADER whose body length excludes itself."""
    from zerodds.idl import Int32

    @idl_struct(typename="x::App", extensibility="appendable")
    @dataclass
    class App:
        a: Int32  # type: ignore[valid-type]
        b: Int32  # type: ignore[valid-type]

    enc = App(a=0x11, b=0x22).encode()
    # DHEADER(=8) + body(8 bytes).
    assert enc[0:4] == b"\x08\x00\x00\x00"
    assert enc[4:8] == b"\x11\x00\x00\x00"
    assert enc[8:12] == b"\x22\x00\x00\x00"
    assert len(enc) == 12
    assert App.decode(enc) == App(a=0x11, b=0x22)


def test_xcdr2_final_struct_has_no_dheader() -> None:
    """rule(17)/(18): a @final aggregate is tight-packed, NO DHEADER (default)."""
    from zerodds.idl import Int32

    @idl_struct(typename="x::Fin")
    @dataclass
    class Fin:
        a: Int32  # type: ignore[valid-type]

    enc = Fin(a=0x33).encode()
    assert enc == b"\x33\x00\x00\x00"


def test_xcdr2_sequence_of_struct_has_dheader_primitive_array_does_not() -> None:
    """rule(12): sequence<@final struct> → DHEADER + count + tight elements;
    rule(8): primitive array long[N] → plain elements, NO DHEADER."""
    from typing import List

    from zerodds.idl import Array, Float64, Int32

    @idl_struct(typename="x::Pair")
    @dataclass
    class Pair:
        seq: Int32  # type: ignore[valid-type]
        value: Float64  # type: ignore[valid-type]

    @idl_struct(typename="x::Holder", extensibility="appendable")
    @dataclass
    class Holder:
        hist: List[Pair]  # type: ignore[valid-type]
        win: Array[Int32, 3]  # type: ignore[valid-type]

    v = Holder(hist=[Pair(seq=1, value=0.5)], win=[10, 20, 30])
    enc = Holder.encode(v)
    back = Holder.decode(enc)
    assert back == v
    # The sequence carries its own DHEADER (non-primitive element); a
    # `Pair` element has NO per-element DHEADER (it is @final). The primitive
    # array `win` has NO DHEADER. We assert the structural roundtrip plus the
    # sequence DHEADER presence: locate count=1 right after the seq DHEADER.
    # After top DHEADER(4) the first field is the seq DHEADER (u32) then count.
    seq_dheader = int.from_bytes(enc[4:8], "little")
    seq_count = int.from_bytes(enc[8:12], "little")
    assert seq_count == 1
    # seq body = count(4) + seq(4) + pad(0, off12%8->4) + double(8) = 16.
    assert seq_dheader == 16, enc.hex(" ")


def test_xcdr2_optional_present_flag_then_align() -> None:
    """rule(20): @optional → 1-byte present flag, then align(4) before the
    value when present."""
    from zerodds.idl import Float64, Optional, UInt8

    @idl_struct(typename="x::Opt")
    @dataclass
    class Opt:
        tag: UInt8  # type: ignore[valid-type]
        cal: Optional[Float64]  # type: ignore[valid-type]

    present = Opt(tag=0xAA, cal=0.001)
    enc = Opt.encode(present)
    # off0: tag=AA ; off1: present=01 ; pad to 4 ; off4..? wait double aligns 4.
    assert enc[0] == 0xAA
    assert enc[1] == 0x01
    # present flag at off1, then the double aligns to 4 → pad off2,3, double off4.
    assert enc[2:4] == b"\x00\x00"
    import struct as _s

    assert _s.unpack("<d", enc[4:12])[0] == 0.001
    assert Opt.decode(enc) == present
    # absent → just the flag byte 0.
    absent = Opt(tag=0xBB, cal=None)
    enc2 = Opt.encode(absent)
    assert enc2 == b"\xbb\x00"
    assert Opt.decode(enc2) == absent


# =============================================================================
# fixed<P,S> — CORBA/GIOP §9.3.2.7 packed BCD (oracle: JacORB 3.9 / omniORB 4.3)
# =============================================================================


def test_fixed_bcd_oracle_vectors_roundtrip() -> None:
    from zerodds.cdr import CdrReader, CdrWriter

    for val, p, s, hx in [
        ("123.45", 5, 2, "12345c"),
        ("1234", 4, 0, "01234c"),
        ("-1.50", 6, 2, "0000150d"),
    ]:
        w = CdrWriter("le")
        w.write_fixed_bcd(val, p, s)
        assert bytes(w.buf).hex() == hx, f"{val}: {bytes(w.buf).hex()} != {hx}"
        r = CdrReader(bytes(w.buf), "le")
        assert r.read_fixed_bcd(p, s) == val


def test_fixed_member_struct_byte_identical_to_rust_cpp() -> None:
    from zerodds.idl import Fixed, Int32, idl_struct

    @idl_struct(typename="m::S", extensibility="appendable")
    @dataclass
    class S:
        id: Int32
        price: Fixed[5, 2]

    s = S(id=7, price="123.45")
    enc = S.encode(s)
    # @appendable: DHEADER(7) + id(4) + bcd(3) — same bytes rust/cpp emit + the
    # CORBA oracle vectors.
    assert enc.hex() == "070000000700000012345c"
    back = S.decode(enc)
    assert back.id == 7 and back.price == "123.45"
