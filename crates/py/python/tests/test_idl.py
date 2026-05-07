"""aktuelle Variante — IDL-Dataclass-Tests.

Zentraler Test: die Python-Serialisierung einer ShapeType-aequivalenten
``@dataclass`` muss **byte-genau** die gleichen XCDR2-LE-Bytes produzieren
wie die Rust-Seite in `crates/dcps/src/interop.rs` (= `dds_dcps::
interop::ShapeType`).

Das stellt sicher, dass Python-Publisher und Rust-Subscriber über einen
``BytesTopic`` mit korrektem ``typename`` bidirektional reden koennen —
ohne Rust-Codegen.
"""

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
    # "AB\0" ist 3 Bytes Raw, dann 1 Byte Padding zu 4-aligned, dann x int32.
    # Identisches Layout wie Rust-ShapeType "AB":
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
            0x00,  # length = 3 (inkl null)
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
    # Length sagt 3, aber nur 2 bytes folgen.
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
    # Referenz exakt wie in crates/dcps/tests/shapes_type_wire.rs.
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
        f"Python-CDR-Encoder weicht von Rust-Referenz ab.\n"
        f"  got: {encoded.hex(' ')}\n"
        f"  exp: {expected.hex(' ')}"
    )
    # Rueckweg.
    back = PyShape.decode(encoded)
    assert back == s


def test_pyshape_type_name_set_by_decorator() -> None:
    assert PyShape.TYPE_NAME == "ShapeType"


def test_idl_struct_requires_dataclass() -> None:
    # Plain class → Fehler.
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
    # Ohne explizite Idl-Annotation: `int` → Int32, `str` → String, etc.
    @idl_struct(typename="auto::Test")
    @dataclass
    class Auto:
        n: int
        name: str

    a = Auto(n=7, name="x")
    back = Auto.decode(a.encode())
    assert back == a


# =============================================================================
# Composite-Erweiterung — Composite-Types: Nested Struct, Sequence, Array, Optional
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
# IntEnum-Erweiterung — IntEnum als IDL-Feldtyp
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
# Union-Erweiterung — Discriminated Unions
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
    # Discriminator 99 matcht keinen Case → default (Float64) wird genommen.
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
    # Handgebaute Bytes mit Severity=99 (nicht im Enum).
    # code=0 | severity=99 | message="x" → simuliert via _idl_fields-
    # internen Encoder mit einer raw_int statt Enum-Variante.
    from zerodds.cdr import CdrWriter

    w = CdrWriter()
    w.write_i32(0)   # code
    w.write_i32(99)  # severity raw
    w.write_string("x")  # message
    raw = w.into_bytes()
    with pytest.raises(ValueError):
        Event.decode(raw)


def test_idl_struct_resolves_pep563_stringified_annotations() -> None:
    # Mit `from __future__ import annotations` sind alle Felder-Types
    # Strings zur Runtime. Der Decorator muss die im Modul-Namespace
    # aufloesen — Regression-Test fuer Multi-Process-Tests Example-Bug.
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
