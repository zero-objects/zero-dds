"""Generated-IDL tests — the one Python CDR codec, C2(7).

`zerodds.idl`/`zerodds.cdr` (the hand-written `@idl_struct` decorator and its
own CDR codec) are removed. Every type below is real IDL, run through
`zerodds-idlc --lang python` (the modules in `_generated/` are its committed
output) and consumed through `runtime/python`'s `zerodds_wire` — the same
path C#/Java's tests take through their generated TypeSupport companions.

Central test: the generated Python serialization of the OMG `shape.idl`
ShapeType must produce byte-exactly the same XCDR2-LE bytes as the Rust side
in `crates/dcps/src/interop.rs` (= `dds_dcps::interop::ShapeType`). This
ensures that a Python publisher and a Rust subscriber can talk
bidirectionally over a `BytesTopic` with the correct `typename` — without
hand-written Python codec.
"""

import struct

import pytest

import zerodds
from zerodds_wire import Endianness, Reader, Writer, WireError

from generated_modules import load_generated

_CORE_AVAILABLE = getattr(zerodds, "_CORE_AVAILABLE", False)
_NO_CORE_REASON = "zerodds._core not compiled — maturin develop needed"


# =============================================================================
# `zerodds_wire` primitive roundtrip — the runtime the generated codec calls,
# exercised directly (no struct involved).
# =============================================================================


def test_wire_primitive_roundtrip() -> None:
    w = Writer(Endianness.LITTLE, max_align=8)
    w.write_bool(True)
    w.write_u8(0x7F)
    w.write_i16(-1234)
    w.write_u32(0xDEADBEEF)
    w.write_i64(-1_000_000_000_000)
    w.write_f32(3.14)
    w.write_f64(-2.7182818)
    w.write_string("hello")
    data = w.finish()

    r = Reader(data, Endianness.LITTLE, max_align=8)
    assert r.read_bool() is True
    assert r.read_u8() == 0x7F
    assert r.read_i16() == -1234
    assert r.read_u32() == 0xDEADBEEF
    assert r.read_i64() == -1_000_000_000_000
    assert abs(r.read_f32() - 3.14) < 1e-5
    assert abs(r.read_f64() - (-2.7182818)) < 1e-12
    assert r.read_string() == "hello"


def test_wire_string_alignment_padding() -> None:
    # "AB\0" is 3 raw bytes, then 1 byte of padding to 4-aligned, then x int32.
    # Identical layout to the Rust ShapeType "AB":
    #   03 00 00 00  length=3 (incl. terminator)
    #   41 42 00     "AB\0"
    #   00           pad
    #   01 00 00 00  int32=1
    w = Writer(Endianness.LITTLE, max_align=8)
    w.write_string("AB")
    w.write_i32(1)
    expected = bytes(
        [
            0x03, 0x00, 0x00, 0x00,  # length = 3 (incl. terminator)
            0x41, 0x42, 0x00,  # "AB\0"
            0x00,  # pad
            0x01, 0x00, 0x00, 0x00,  # int32 = 1
        ],
    )
    assert w.finish() == expected


def test_wire_reader_rejects_truncated_string() -> None:
    # Length says 3, but only 2 bytes follow.
    data = bytes([0x03, 0x00, 0x00, 0x00, 0x41, 0x42])
    r = Reader(data, Endianness.LITTLE, max_align=8)
    with pytest.raises(WireError):
        r.read_string()


# =============================================================================
# ShapeType — the OMG shape.idl struct, byte-identical to
# `crates/dcps/src/interop.rs`. `shape.idl` / `shape_gen.py`.
# =============================================================================


def test_shapetype_byte_roundtrip() -> None:
    mod = load_generated("shape_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["ShapeType"]
    ShapeType = descriptor.cls
    s = ShapeType(color="RED", x=42, y=77, shapesize=30, additional_payload_size=b"")
    encoded = descriptor.encode(s)
    # Hand-derived from OMG XCDR2 §7.4 for the @appendable shape.idl:
    # DHEADER, then the five members.
    expected = bytes(
        [
            0x18, 0x00, 0x00, 0x00,  # DHEADER = 24 (byte length of the members)
            0x04, 0x00, 0x00, 0x00,  # color.length
            0x52, 0x45, 0x44, 0x00,  # "RED\0"
            0x2A, 0x00, 0x00, 0x00,  # x = 42
            0x4D, 0x00, 0x00, 0x00,  # y = 77
            0x1E, 0x00, 0x00, 0x00,  # shapesize = 30
            0x00, 0x00, 0x00, 0x00,  # additional_payload_size.length = 0
        ],
    )
    assert encoded == expected, (
        f"generated Python XCDR2 encoder deviates from the reference.\n"
        f"  got: {encoded.hex(' ')}\n"
        f"  exp: {expected.hex(' ')}"
    )
    # Return path. `sequence<octet>` decodes as `list[int]`, not `bytes` — a
    # measured shape difference from the removed `zerodds.idl` codec's own
    # `Bytes` type, not a byte-level one.
    back = descriptor.decode(encoded)
    assert (back.color, back.x, back.y, back.shapesize) == ("RED", 42, 77, 30)
    assert bytes(back.additional_payload_size) == b""


@pytest.mark.skipif(not _CORE_AVAILABLE, reason=_NO_CORE_REASON)
@pytest.mark.parametrize(
    ("color", "x", "y", "shapesize"),
    [
        ("RED", 42, 77, 30),
        ("BLUE", 0, 0, 30),
        ("MAGENTA", -50, -75, 45),
        ("", 0, 0, 0),
        ("A", 1, 2, 3),
        ("AB", 1, 2, 3),
        ("ABCDEFG", 2147483647, -2147483648, 255),
    ],
)
def test_shapetype_matches_the_rust_codec_octet_for_octet(
    color: str,
    x: int,
    y: int,
    shapesize: int,
) -> None:
    """§5.5 — the generated Python codec against the Rust one compiled into
    this very module, not against a literal copied out of a Rust test.
    `shape_reference_octets` returns what
    `zerodds_dcps::interop::ShapeType::encode` writes."""
    from zerodds import _core

    mod = load_generated("shape_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["ShapeType"]
    reference = _core.shape_reference_octets(_core.Shape(color, x, y, shapesize))
    mine = descriptor.encode(
        descriptor.cls(
            color=color, x=x, y=y, shapesize=shapesize, additional_payload_size=b"",
        ),
    )
    assert mine == reference, (
        f"Python and Rust ShapeType codecs disagree for {color!r}.\n"
        f"  python: {mine.hex(' ')}\n"
        f"  rust:   {reference.hex(' ')}"
    )
    back = descriptor.decode(reference)
    assert (back.color, back.x, back.y, back.shapesize) == (color, x, y, shapesize)


def test_shapetype_descriptor_reports_appendable() -> None:
    mod = load_generated("shape_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["ShapeType"]
    assert descriptor.type_name == "ShapeType"
    assert descriptor.extensibility == 1  # appendable


# =============================================================================
# Sensor — mixed primitive fields (boolean, float, string, sequence<octet>).
# `sensor.idl` / `sensor_gen.py`.
# =============================================================================


def test_sensor_mixed_fields_roundtrip() -> None:
    mod = load_generated("sensor_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["zerodds::Sensor"]
    Sensor = descriptor.cls
    s = Sensor(active=True, reading=1.5, label="sonar", raw=b"\xAA\xBB\xCC")
    back = descriptor.decode(descriptor.encode(s))
    assert back.active is True
    assert abs(back.reading - 1.5) < 1e-6
    assert back.label == "sonar"
    assert bytes(back.raw) == b"\xAA\xBB\xCC"


# =============================================================================
# Composite shapes — nested struct, sequence<primitive>, sequence<struct>,
# fixed array, @optional scalar. `geometry.idl` / `geometry_gen.py`.
# =============================================================================


def test_nested_struct_roundtrip() -> None:
    mod = load_generated("geometry_gen")
    Vec3 = mod.geom.Vec3
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["geom::Pose"]
    p = descriptor.cls(position=Vec3(x=1.0, y=2.0, z=3.0), label="origin")
    back = descriptor.decode(descriptor.encode(p))
    assert (back.position.x, back.position.y, back.position.z) == (1.0, 2.0, 3.0)
    assert back.label == "origin"


def test_sequence_of_primitives_roundtrip() -> None:
    mod = load_generated("geometry_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["container::Grid"]
    g = descriptor.cls(values=[1, 2, 3, 42, -7])
    back = descriptor.decode(descriptor.encode(g))
    assert back.values == [1, 2, 3, 42, -7]


def test_sequence_of_structs_roundtrip() -> None:
    mod = load_generated("geometry_gen")
    Vec3 = mod.geom.Vec3
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["container::Mesh"]
    m = descriptor.cls(points=[Vec3(x=1.0, y=0.0, z=0.0), Vec3(x=0.0, y=1.0, z=0.0)])
    back = descriptor.decode(descriptor.encode(m))
    assert [(p.x, p.y, p.z) for p in back.points] == [(1.0, 0.0, 0.0), (0.0, 1.0, 0.0)]


def test_fixed_array_roundtrip() -> None:
    """`long raw[4]` writes N elements with NO length prefix (Bug Q-cluster
    fixed array), matching `arrays_gen`'s finding for a multidim array."""
    mod = load_generated("geometry_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["container::FixedArr"]
    f = descriptor.cls(raw=(10, 20, 30, 40))
    enc = descriptor.encode(f)
    assert len(enc) == 16, enc.hex()  # 4 * 4 bytes, no length prefix
    assert descriptor.decode(enc).raw == (10, 20, 30, 40)


@pytest.mark.xfail(
    strict=True,
    reason="open defect: the generated array encoder checks no element count "
    "(crates/lang-rules/src/emit/encode.rs); an overcount is cut silently",
)
def test_fixed_array_count_is_refused_on_encode() -> None:
    """A fixed array `long raw[4]` holds exactly four elements, and a fixed
    array has no count on the wire for a decoder to check: the encoder is the
    only place a wrong count can be refused. Today too few elements raise
    `IndexError` from inside the generated `zd_enc_arr_long_4`, and too many
    are cut to four without an error — measured: `(1, ..., 7)` decodes as
    `[1, 2, 3, 4]`. Both must raise `OutOfRange`, as a bounded sequence does.
    """
    from zerodds_wire import OutOfRange

    mod = load_generated("geometry_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["container::FixedArr"]
    with pytest.raises(OutOfRange):
        descriptor.encode(descriptor.cls(raw=(1, 2)))
    with pytest.raises(OutOfRange):
        descriptor.encode(descriptor.cls(raw=(1, 2, 3, 4, 5, 6, 7)))


def test_optional_present_and_absent() -> None:
    mod = load_generated("geometry_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["container::Maybe"]
    with_value = descriptor.cls(tag="hit", maybe_num=42)
    without = descriptor.cls(tag="miss", maybe_num=None)
    back_with = descriptor.decode(descriptor.encode(with_value))
    back_without = descriptor.decode(descriptor.encode(without))
    assert (back_with.tag, back_with.maybe_num) == ("hit", 42)
    assert (back_without.tag, back_without.maybe_num) == ("miss", None)


# =============================================================================
# IntEnum member + the unknown-value refusal path.
# `severity.idl` / `severity_gen.py`.
# =============================================================================


def test_enum_roundtrip() -> None:
    mod = load_generated("severity_gen")
    SeverityKind = mod.diag.SeverityKind
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["diag::Event"]
    e = descriptor.cls(code=42, severity=SeverityKind.WARN, message="voltage drop")
    back = descriptor.decode(descriptor.encode(e))
    assert back.code == 42
    assert back.severity is SeverityKind.WARN
    assert back.message == "voltage drop"


def test_enum_unknown_value_raises() -> None:
    """Hand-built bytes with severity=99 (not in the enum). The generated
    decoder does not raise inline — it refuses the stream
    (`@try_construct` DISCARD, XTypes 1.3 §7.2.2.4.4.4.9) and carries on;
    `TopicDescriptor.decode()` is what turns that refusal into the
    `ValueError` a caller sees, exactly like `test_generated_bounds_
    roundtrip_and_enforced`'s over-bound wire."""
    mod = load_generated("severity_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["diag::Event"]
    w = Writer(Endianness.LITTLE, max_align=4)
    w.write_i32(0)  # code
    w.write_i32(99)  # severity raw — not OK(0)/WARN(1)/ERROR(2)
    w.write_string("x")  # message
    raw = w.finish()
    with pytest.raises(ValueError):
        descriptor.decode(raw)


# =============================================================================
# Discriminated unions — case selection, the default branch, and an
# unmatched discriminator with NO default (measured, not the removed
# `zerodds.idl` decorator's own stricter validation).
# `unionelem.idl` / `unionelem_gen.py`.
# =============================================================================


def test_union_case_int_roundtrip() -> None:
    mod = load_generated("unionelem_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["u::Elem"]
    v = descriptor.cls(_d=0, n=42)
    back = descriptor.decode(descriptor.encode(v))
    assert (back._d, back.n) == (0, 42)


def test_union_case_string_roundtrip() -> None:
    mod = load_generated("unionelem_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["u::Elem"]
    v = descriptor.cls(_d=1, s="hello")
    back = descriptor.decode(descriptor.encode(v))
    assert (back._d, back.s) == (1, "hello")


def test_union_default_branch_used_for_unknown_disc() -> None:
    # Discriminator 9 matches no explicit case → the default (double f) is taken.
    mod = load_generated("unionelem_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["u::Elem"]
    v = descriptor.cls(_d=9, f=3.14)
    back = descriptor.decode(descriptor.encode(v))
    assert back._d == 9
    assert abs(back.f - 3.14) < 1e-9


def test_union_without_default_writes_only_the_discriminator() -> None:
    """`u::Strict` has one case (0) and no `default:`. For an unmatched
    discriminator the shared codegen writes the discriminator and NOTHING
    else (`crates/lang-rules/src/emit/encode.rs`: "a union without
    `default:` writes nothing after the discriminator") — it does not raise.
    This is a measured behaviour difference from the removed `zerodds.idl`
    decorator's `_IdlUnion._write_body`, which raised `ValueError` for
    exactly this case; the generated codec has no equivalent check, and
    that is by design (it mirrors the suppressed-label overlay every other
    binding's codec uses), not a gap this migration introduces.
    """
    mod = load_generated("unionelem_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["u::Strict"]
    v = descriptor.cls(_d=1, n=None)  # 1 matches no case
    enc = descriptor.encode(v)
    assert enc == (1).to_bytes(4, "little")  # discriminator alone, 4 bytes
    back = descriptor.decode(enc)
    assert back._d == 1
    # `_d = 1` selects no case, so no case field is live: the decoder
    # clears the one Table 9's default had set (lang-rules
    # `CodecTarget::clear_union_cases`).
    assert back.n is None


# =============================================================================
# Adversarial edge sweep: union as a sequence element, union as a map value,
# enum as a sequence element, an aggregate of only empty/default collections.
# `unionelem.idl` / `unionelem_gen.py`.
# =============================================================================


def test_union_as_sequence_element_roundtrips() -> None:
    mod = load_generated("unionelem_gen")
    Elem = mod.u.Elem
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["e::UnionSeq"]
    v = descriptor.cls(
        items=[
            Elem(_d=0, n=1),
            Elem(_d=1, s="z"),
            Elem(_d=2, seq=[5, 6]),
            Elem(_d=9, f=2.5),
        ],
    )
    back = descriptor.decode(descriptor.encode(v))
    assert [it._d for it in back.items] == [0, 1, 2, 9]
    assert back.items[0].n == 1
    assert back.items[1].s == "z"
    assert back.items[2].seq == [5, 6]
    assert back.items[3].f == 2.5


def test_union_as_map_value_roundtrips() -> None:
    mod = load_generated("unionelem_gen")
    MapVal = mod.u.MapVal
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["e::UnionMap"]
    v = descriptor.cls(m=[("a", MapVal(_d=0, n=5)), ("b", MapVal(_d=1, s="q"))])
    back = descriptor.decode(descriptor.encode(v))
    table = dict(back.m)
    assert table["a"].n == 5
    assert table["b"].s == "q"


def test_enum_as_sequence_element_roundtrips() -> None:
    mod = load_generated("unionelem_gen")
    Color = mod.e.Color
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["e::EnumSeq"]
    v = descriptor.cls(cs=[Color.RED, Color.BLUE, Color.GREEN])
    back = descriptor.decode(descriptor.encode(v))
    assert back.cs == [Color.RED, Color.BLUE, Color.GREEN]
    assert all(isinstance(c, Color) for c in back.cs)


def test_empty_collections_all_roundtrip() -> None:
    """An empty sequence, empty string, empty wstring and empty map all
    survive count=0 without crashing. `map<K,V>` is `list[tuple[K,V]]`, not
    a `dict` — order-preserving, and not requiring `V` to be hashable."""
    mod = load_generated("unionelem_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["e::Empties"]
    v = descriptor.cls(seq=[], s="", ws="", m=[])
    back = descriptor.decode(descriptor.encode(v))
    assert back.seq == [] and back.s == "" and back.ws == "" and back.m == []


# =============================================================================
# Generated-module roundtrips carried over unchanged from the previous
# round — import codegen output over `runtime/python`'s `zerodds_wire`, and
# run real encode/decode roundtrips against the live runtime.
# =============================================================================


def test_generated_optionals_roundtrip() -> None:
    """Bug R5 (#64): generated @optional members roundtrip present + absent."""
    mod = load_generated("optionals_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["conf::Optionals"]
    Optionals = descriptor.cls
    v = Optionals(required=1, maybe=7, note="hi")
    back = descriptor.decode(descriptor.encode(v))
    assert (back.required, back.maybe, back.note) == (1, 7, "hi")
    v2 = Optionals(required=2, maybe=None, note=None)
    back2 = descriptor.decode(descriptor.encode(v2))
    assert (back2.required, back2.maybe, back2.note) == (2, None, None)


def test_generated_consts_are_module_constants() -> None:
    """Bug M (#56): const decls become importable module constants."""
    mod = load_generated("consts_gen")
    assert mod.MAX_ITEMS == 10
    assert mod.RATE == 2.5
    assert mod.ENABLED is True
    assert mod.conf.N == 4


def test_generated_combo_module_imports_and_roundtrips() -> None:
    """Bug Q-cluster (#60): the module-wrapped combo type imports cleanly
    under py3.14 deferred annotations and roundtrips, exercising flattened
    references, a typedef member, a union member, an @optional member and
    a fixed array.

    The generated union is a dataclass with a `_d` discriminator field and
    one Optional field per case, not the removed `idl_union` helper's
    `.make()`/`.discriminator`/`.value`. A fixed array (`window`) is a
    `tuple`, not a `list`.
    """
    mod = load_generated("combo_gen")
    Telemetry = mod.combo.Telemetry
    Mode = mod.combo.ModeKind
    Reading = mod.combo.ReadingUnion
    Sample = mod.combo.Sample
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["combo::Telemetry"]

    v = Telemetry(
        unitId=11,
        mode=Mode.MODE_ACTIVE,
        batteryCurrent=2.5,
        history=[Sample(seq=1, value=1.5), Sample(seq=2, value=2.5)],
        reading=Reading(_d=Mode.MODE_IDLE, idleTicks=42),
        calibration=None,
        window=(10, 20, 30, 40),
    )
    back = descriptor.decode(descriptor.encode(v))
    assert back.unitId == 11
    assert back.mode == Mode.MODE_ACTIVE
    assert back.batteryCurrent == 2.5
    assert [s.seq for s in back.history] == [1, 2]
    assert back.reading._d == Mode.MODE_IDLE
    assert back.reading.idleTicks == 42
    assert back.calibration is None
    assert back.window == (10, 20, 30, 40)
    assert back == v


def test_generated_chars_roundtrip() -> None:
    """Bug Q-cluster: generated char + wstring members roundtrip, including
    the empty wstring."""
    mod = load_generated("chars_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["Chars"]
    Chars = descriptor.cls
    v = Chars(c="Z", w="wide-ä")
    back = descriptor.decode(descriptor.encode(v))
    assert back.c == "Z"
    assert back.w == "wide-ä"
    empty = descriptor.decode(descriptor.encode(Chars(c="Q", w="")))
    assert empty.w == ""


def test_generated_arrays_roundtrip() -> None:
    """Bug Q-cluster: generated multidim + array-of-struct members roundtrip.

    Fields are typed `tuple[...]` for a fixed array, and the decoder
    answers with tuples, one per dimension: a sample read back is `==` to
    the one written.
    """
    mod = load_generated("arrays_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["conf::Arrays"]
    Arrays = descriptor.cls
    Point = mod.conf.Point
    v = Arrays(
        grid=((1, 2), (3, 4)),
        shape=(Point(x=1, y=2), Point(x=3, y=4)),
    )
    back = descriptor.decode(descriptor.encode(v))
    assert back.grid == ((1, 2), (3, 4))
    assert isinstance(back.shape, tuple)
    assert [(p.x, p.y) for p in back.shape] == [(1, 2), (3, 4)]
    assert back == v
    assert Arrays(grid=((0, 0), (0, 0)), shape=(Point(x=0, y=0), Point(x=0, y=0))) == (
        mod.zd_def_conf_Arrays()
    )


def test_generated_bounds_roundtrip_and_enforced() -> None:
    """Checklist 2 (codegen): generated string<N>/sequence<T,N>/map<K,V,N>/
    wstring<N> roundtrip at bound, and both ENCODE and DECODE enforce it.

    `map<K,V,N>` is a `list[tuple[K,V]]` in the regenerated module, not a
    `dict` — order-preserving, and not requiring V to be hashable.

    Fixed: the regenerated ENCODER now enforces the declared bound before
    writing a byte (`zd_enc_seq_b3_long` et al. call
    `Writer.check_sequence_bound`/`check_map_bound`/`write_string_bounded`/
    `write_wstring_bounded`, which raise `zerodds_wire.OutOfRange`), the
    same refusal `runtime/csharp`'s `Writer.CheckSequenceBound` and
    `WriteStringBounded` raise, and the one the removed hand-written
    `zerodds.idl` codec's `_IdlBoundedString._check` raised as `ValueError`.
    Nothing is written on an over-bound value — `crates/lang-rules`'s
    shared codec walk checks the count or the octet/UTF-16 length before
    the DHEADER or the length word goes out, so a caller that ignores the
    exception has an unfinished `Writer` and not a wire value nobody
    declared. The DECODE side keeps enforcing it exactly as before
    (XTypes 1.3 §7.2.2.4.4.4.9's @try_construct — DISCARD by default),
    verified below with an over-bound wire value assembled by hand (the
    unbounded `write_sequence`-style calls a decoder still has to read from
    a foreign peer that made no such promise).
    """
    from zerodds_wire import OutOfRange

    mod = load_generated("bounds_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["b::Bounded"]
    Bounded = descriptor.cls
    v = Bounded(
        nums=[1, 2, 3],
        name="hello",
        wname="日本語",
        counters=[("a", 1), ("b", 2)],
        unbounded=[9, 8, 7, 6, 5],
        fullstr="anything at all",
    )
    back = descriptor.decode(descriptor.encode(v))
    assert back.nums == [1, 2, 3]
    assert back.name == "hello"
    assert back.wname == "日本語"
    assert back.counters == [("a", 1), ("b", 2)]
    assert back.unbounded == [9, 8, 7, 6, 5]
    assert back.fullstr == "anything at all"

    # The encoder refuses an over-bound value before writing anything.
    over_bound_nums = [1, 2, 3, 4]  # bound is 3
    w = mod.Writer(mod.Endianness.LITTLE, max_align=4)
    with pytest.raises(OutOfRange):
        mod.zd_enc_seq_b3_long(w, over_bound_nums)

    # Exactly at the bound still encodes, and roundtrips.
    w = mod.Writer(mod.Endianness.LITTLE, max_align=4)
    mod.zd_enc_seq_b3_long(w, [1, 2, 3])
    r = mod.Reader(w.finish(), mod.Endianness.LITTLE, max_align=4)
    assert mod.zd_dec_seq_b3_long(r, mod.TryConstruct.DISCARD) == [1, 2, 3]
    assert r.reader_status() is mod.Status.OK

    # Same for string<5>, wstring<3> and map<string,long,2>.
    w = mod.Writer(mod.Endianness.LITTLE, max_align=4)
    with pytest.raises(OutOfRange):
        mod.zd_enc_string_b5(w, "toolong")
    w = mod.Writer(mod.Endianness.LITTLE, max_align=4)
    mod.zd_enc_string_b5(w, "hello")

    w = mod.Writer(mod.Endianness.LITTLE, max_align=4)
    with pytest.raises(OutOfRange):
        mod.zd_enc_wstring_b3(w, "日本語X")
    w = mod.Writer(mod.Endianness.LITTLE, max_align=4)
    mod.zd_enc_wstring_b3(w, "日本語")

    w = mod.Writer(mod.Endianness.LITTLE, max_align=4)
    with pytest.raises(OutOfRange):
        mod.zd_enc_map_b2_string_long(w, [("a", 1), ("b", 2), ("c", 3)])
    w = mod.Writer(mod.Endianness.LITTLE, max_align=4)
    mod.zd_enc_map_b2_string_long(w, [("a", 1), ("b", 2)])

    # The decoder still enforces its own bound against a wire value a
    # foreign peer wrote — the encoder's refusal cannot reach that case.
    # Assembled by hand from the same primitives `zd_enc_seq_b3_long`
    # calls, minus its `check_sequence_bound`.
    w = mod.Writer(mod.Endianness.LITTLE, max_align=4)
    d = mod.no_dheader()
    w.write_u32(len(over_bound_nums))
    for n in over_bound_nums:
        mod.zd_enc_p_long(w, n)
    w.finish_dheader(d)
    r = mod.Reader(w.finish(), mod.Endianness.LITTLE, max_align=4)
    result = mod.zd_dec_seq_b3_long(r, mod.TryConstruct.DISCARD)
    assert r.reader_status() is mod.Status.DISCARDED
    assert result == over_bound_nums  # decoded before the bound check fires


def test_generated_deep_nesting_roundtrips() -> None:
    """Checklist 3 (codegen): struct->struct->struct, sequence<sequence<struct>>,
    map<string, struct-with-a-sequence>. `map<K,V>` is a `list[tuple[K,V]]`."""
    mod = load_generated("deepnest_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["d::Deep"]
    L1 = mod.d.L1
    L2 = mod.d.L2
    L3 = mod.d.L3
    HasSeq = mod.d.HasSeq

    v = mod.d.Deep(
        chain=L1(inner=L2(inner=L3(v=7))),
        grid=[[L3(v=1), L3(v=2)], [], [L3(v=3)]],
        table=[("a", HasSeq(xs=[1, 2])), ("b", HasSeq(xs=[]))],
    )
    back = descriptor.decode(descriptor.encode(v))
    assert back.chain.inner.inner.v == 7
    assert [[e.v for e in row] for row in back.grid] == [[1, 2], [], [3]]
    table = dict(back.table)
    assert table["a"].xs == [1, 2]
    assert table["b"].xs == []


def test_generated_optional_aggregate_present_and_absent() -> None:
    """Checklist 4 (codegen): @optional of nested struct / sequence / map /
    string — present AND absent both roundtrip; absent stays None.
    `map<K,V>` is a `list[tuple[K,V]]`."""
    mod = load_generated("optagg_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["o::OptAgg"]
    OptAgg = descriptor.cls
    Inner = mod.o.Inner

    present = OptAgg(nested=Inner(v=9), nums=[1, 2], table=[("k", 1)], note="hi")
    back_p = descriptor.decode(descriptor.encode(present))
    assert back_p.nested.v == 9
    assert back_p.nums == [1, 2]
    assert back_p.table == [("k", 1)]
    assert back_p.note == "hi"

    absent = OptAgg(nested=None, nums=None, table=None, note=None)
    back_a = descriptor.decode(descriptor.encode(absent))
    assert back_a.nested is None
    assert back_a.nums is None
    assert back_a.table is None
    assert back_a.note is None


def test_generated_unicode_codepoints_survive() -> None:
    """Checklist 6 (codegen): multi-byte UTF-8 in string + UTF-16 in wstring;
    exact code points survive (CJK + emoji)."""
    mod = load_generated("unicode_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["Uni"]
    Uni = descriptor.cls
    text = "日本語🚀café"
    v = Uni(s=text, w=text)
    back = descriptor.decode(descriptor.encode(v))
    assert back.s == text
    assert back.w == text
    assert [ord(c) for c in back.s] == [ord(c) for c in text]


def test_generated_extreme_primitives_roundtrip() -> None:
    """Checklist 8 (codegen): integer min/max/0/-1 across all widths + a normal
    float/double."""
    mod = load_generated("extremes_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["Extremes"]
    Extremes = descriptor.cls
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
    back = descriptor.decode(descriptor.encode(v))
    assert back.i8 == -128 and back.u8 == 255
    assert back.i16 == -32768 and back.u16 == 65535
    assert back.i32 == -(2**31) and back.u32 == 2**32 - 1
    assert back.i64 == -(2**63) and back.u64 == 2**64 - 1
    assert back.f == 1.5
    assert back.dbl == 3.141592653589793

    z = Extremes(i8=0, u8=0, i16=0, u16=0, i32=0, u32=0, i64=-1, u64=0, f=0.0, dbl=-1.0)
    backz = descriptor.decode(descriptor.encode(z))
    assert backz.i64 == -1 and backz.dbl == -1.0 and backz.u32 == 0


def test_generated_keyed_same_key_different_payload() -> None:
    """Checklist 9 (codegen): two samples with the same @key, different payload
    — the key fields roundtrip identically, and the same-key samples also
    produce the same KeyHash."""
    mod = load_generated("keyed_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["Keyed"]
    Keyed = descriptor.cls
    assert descriptor.keyed
    a = Keyed(id=7, label="dev", payload=100)
    b = Keyed(id=7, label="dev", payload=999)
    back_a = descriptor.decode(descriptor.encode(a))
    back_b = descriptor.decode(descriptor.encode(b))
    assert back_a.id == back_b.id == 7
    assert back_a.label == back_b.label == "dev"
    assert back_a.payload == 100
    assert back_b.payload == 999
    assert descriptor.key_hash(a) == descriptor.key_hash(b)
    c = Keyed(id=8, label="dev", payload=100)
    assert descriptor.key_hash(a) != descriptor.key_hash(c)


# =============================================================================
# XCDR2 canonical-layout regression (Bug XW cross-PSM convergence).
# Bytes measured against the actual generated module — not hand-derived —
# and cross-checked against the cross-vendor-validated Rust reference golden
# (internal/idl-codegen/xcdr2-canonical-layout.md), the conform target for
# all 7 backends. Pins the spec framing so the runtime can't regress.
# `layout.idl` / `layout_gen.py`.
# =============================================================================


def test_xcdr2_max_align_caps_double_at_four() -> None:
    """XTypes 1.3 §7.4.1.1.1: XCDR2 MAXALIGN = 4 — a double aligns to 4, never
    8. After a u8 + u32 (5 bytes) the double pads only to offset 8, not 16."""
    mod = load_generated("layout_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["x::AlignProbe"]
    enc = descriptor.encode(descriptor.cls(a=1, b=2, c=1.0))
    # off 0: a=01 ; pad 3 to 4 ; off 4: b=02000000 ; off 8: double (NOT off 16).
    assert enc[0] == 0x01
    assert enc[1:4] == b"\x00\x00\x00"
    assert enc[4:8] == b"\x02\x00\x00\x00"
    assert len(enc) == 16, f"double over-aligned to 8: {enc.hex(' ')}"
    assert descriptor.decode(enc).c == 1.0


def test_xcdr2_appendable_emits_single_dheader() -> None:
    """XTypes 1.3 §7.4.3.5.3 rule(30): a top-level @appendable aggregate carries
    exactly ONE DHEADER whose body length excludes itself."""
    mod = load_generated("layout_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["x::App"]
    enc = descriptor.encode(descriptor.cls(a=0x11, b=0x22))
    # DHEADER(=8) + body(8 bytes).
    assert enc[0:4] == b"\x08\x00\x00\x00"
    assert enc[4:8] == b"\x11\x00\x00\x00"
    assert enc[8:12] == b"\x22\x00\x00\x00"
    assert len(enc) == 12
    back = descriptor.decode(enc)
    assert (back.a, back.b) == (0x11, 0x22)


def test_xcdr2_final_struct_has_no_dheader() -> None:
    """rule(17)/(18): a @final aggregate is tight-packed, NO DHEADER."""
    mod = load_generated("layout_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["x::Fin"]
    enc = descriptor.encode(descriptor.cls(a=0x33))
    assert enc == b"\x33\x00\x00\x00"


def test_xcdr2_sequence_of_struct_has_dheader_primitive_array_does_not() -> None:
    """rule(12): sequence<@final struct> → DHEADER + count + tight elements;
    rule(8): primitive array long[N] → plain elements, NO DHEADER."""
    mod = load_generated("layout_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["x::Holder"]
    Pair = mod.x.Pair
    v = descriptor.cls(hist=[Pair(seq=1, value=0.5)], win=(10, 20, 30))
    enc = descriptor.encode(v)
    back = descriptor.decode(enc)
    assert back.hist[0].seq == 1 and back.hist[0].value == 0.5
    assert back.win == (10, 20, 30)
    # The sequence carries its own DHEADER (non-primitive element); a
    # `Pair` element has NO per-element DHEADER (it is @final). The primitive
    # array `win` has NO DHEADER. Structural roundtrip plus the sequence
    # DHEADER presence: locate count=1 right after the seq DHEADER. After
    # the top DHEADER(4) the first field is the seq DHEADER (u32) then count.
    seq_dheader = int.from_bytes(enc[4:8], "little")
    seq_count = int.from_bytes(enc[8:12], "little")
    assert seq_count == 1
    # seq body = count(4) + seq(4) + double(8), already 4-aligned at offset 8
    # under the generated codec's XCDR2 MAXALIGN=4 (no further padding) = 16.
    assert seq_dheader == 16, enc.hex(" ")


def test_xcdr2_optional_present_flag_then_align() -> None:
    """rule(20): @optional → 1-byte present flag, then align(4) before the
    value when present."""
    mod = load_generated("layout_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["x::Opt"]
    present = descriptor.cls(tag=0xAA, cal=0.001)
    enc = descriptor.encode(present)
    # off0: tag=AA ; off1: present=01 ; pad to 4 ; off4: double aligns to 4.
    assert enc[0] == 0xAA
    assert enc[1] == 0x01
    assert enc[2:4] == b"\x00\x00"
    assert struct.unpack("<d", enc[4:12])[0] == 0.001
    back = descriptor.decode(enc)
    assert (back.tag, back.cal) == (0xAA, 0.001)
    # absent → just the flag byte 0.
    absent = descriptor.cls(tag=0xBB, cal=None)
    enc2 = descriptor.encode(absent)
    assert enc2 == b"\xbb\x00"
    back2 = descriptor.decode(enc2)
    assert (back2.tag, back2.cal) == (0xBB, None)


# =============================================================================
# fixed<P,S> — CORBA/GIOP §9.3.2.7 packed BCD (oracle: JacORB 3.9 / omniORB 4.3)
# =============================================================================


def test_fixed_bcd_oracle_vectors_roundtrip() -> None:
    for val, p, s, hx in [
        ("123.45", 5, 2, "12345c"),
        ("1234", 4, 0, "01234c"),
        ("-1.50", 6, 2, "0000150d"),
    ]:
        w = Writer(Endianness.LITTLE, max_align=8)
        w.write_fixed(val, p, s)
        encoded = w.finish()
        assert encoded.hex() == hx, f"{val}: {encoded.hex()} != {hx}"
        r = Reader(encoded, Endianness.LITTLE, max_align=8)
        assert r.read_fixed(p, s) == val


def test_fixed_member_struct_byte_identical_to_rust_cpp() -> None:
    mod = load_generated("fixedbcd_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["m::S"]
    s = descriptor.cls(id=7, price="123.45")
    enc = descriptor.encode(s)
    # @appendable: DHEADER(7) + id(4) + bcd(3) — same bytes rust/cpp emit + the
    # CORBA oracle vectors.
    assert enc.hex() == "070000000700000012345c"
    back = descriptor.decode(enc)
    assert back.id == 7 and back.price == "123.45"


# =============================================================================
# XCDR1 wire path (F.1 item 13). `zerodds-idlc --lang python` fixes one
# representation per generation (`TopicDescriptor.representation`, checked
# and refused if a caller's `representation=` argument disagrees) —
# `xcdr1.idl` is regenerated twice, once per `--encoding`, into
# `xcdr1_xcdr1_gen.py` and `xcdr1_xcdr2_gen.py`, and a test compares the two
# modules' descriptors for the same type instead of one descriptor under
# two representations.
# =============================================================================


def test_generated_xcdr1_final_alignment_and_roundtrip() -> None:
    """@final under XCDR1 caps alignment at 8: the `double` aligns to offset 8
    (XCDR2 would cap at 4). Asserts byte layout + roundtrip, and that the two
    representations produce different wire bytes."""
    mod1 = load_generated("xcdr1_xcdr1_gen")
    mod2 = load_generated("xcdr1_xcdr2_gen")
    d1 = mod1.ZD_TOPIC_DESCRIPTORS["X1Final"]
    d2 = mod2.ZD_TOPIC_DESCRIPTORS["X1Final"]

    x1 = d1.encode(d1.cls(a=1, d=1.0))  # XCDR1 / classic CDR
    # octet at 0, 7 bytes pad, f64 aligned to 8 -> total 16 bytes.
    assert len(x1) == 16, x1.hex()
    assert x1[0] == 1
    assert x1[1:8] == b"\x00" * 7, x1.hex()
    assert x1[8:16] == struct.pack("<d", 1.0), x1.hex()

    # XCDR2 caps alignment at 4: octet + 3 pad + f64 -> 12 bytes.
    x2 = d2.encode(d2.cls(a=1, d=1.0))
    assert len(x2) == 12, x2.hex()
    assert x1 != x2

    back = d1.decode(x1)
    assert (back.a, back.d) == (1, 1.0)


def test_generated_xcdr1_appendable_has_no_dheader() -> None:
    """@appendable under XCDR1 emits NO DHEADER (XCDR2 emits one, rule(30)).
    The wire therefore starts with the first member's value, not a length."""
    mod1 = load_generated("xcdr1_xcdr1_gen")
    mod2 = load_generated("xcdr1_xcdr2_gen")
    d1 = mod1.ZD_TOPIC_DESCRIPTORS["X1App"]
    d2 = mod2.ZD_TOPIC_DESCRIPTORS["X1App"]

    x1 = d1.encode(d1.cls(a=7, d=2.5))
    # No DHEADER: the wire starts with the long `a` (7) directly.
    assert x1[0:4] == (7).to_bytes(4, "little"), x1.hex()
    # long(4) + 4 pad + f64 aligned to 8 -> 16 bytes.
    assert len(x1) == 16, x1.hex()

    # XCDR2 appendable prepends a DHEADER: the first word is the body length
    # (12), NOT the value `a`. This is the observable XCDR1-vs-XCDR2 difference.
    x2 = d2.encode(d2.cls(a=7, d=2.5))
    assert x2[0:4] == (12).to_bytes(4, "little"), x2.hex()
    assert x1 != x2

    back = d1.decode(x1)
    assert (back.a, back.d) == (7, 2.5)


def test_generated_xcdr1_mutable_pl_cdr1_roundtrip() -> None:
    """@mutable under XCDR1 frames members as PL_CDR1 ([PID][len] + sentinel),
    NOT XCDR2 PL_CDR2 EMHEADERs. Asserts the PL_CDR1 wire roundtrips and that
    the two representations differ on the wire."""
    mod1 = load_generated("xcdr1_xcdr1_gen")
    mod2 = load_generated("xcdr1_xcdr2_gen")
    d1 = mod1.ZD_TOPIC_DESCRIPTORS["X1Mut"]
    d2 = mod2.ZD_TOPIC_DESCRIPTORS["X1Mut"]

    x1 = d1.encode(d1.cls(a=9, d=-1.5))
    back = d1.decode(x1)
    assert (back.a, back.d) == (9, -1.5)

    # PL_CDR1 and PL_CDR2 are different framings -> different bytes.
    x2 = d2.encode(d2.cls(a=9, d=-1.5))
    assert x1 != x2
    # And each representation decodes its OWN wire only.
    back2 = d2.decode(x2)
    assert (back2.a, back2.d) == (9, -1.5)
