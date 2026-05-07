"""IDL-Annotations für Python-Dataclasses (aktuelle Variante).

Der Nutzer definiert eine normale `@dataclass`, markiert die Felder
mit IDL-Primitive-Types, und bekommt einen Encoder/Decoder der
byte-genau mit dem Rust-Seiten-XCDR2-LE matcht.

Beispiel::

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

Ohne den Decorator bleibt die Klasse eine normale Dataclass.

Scope dieser MVP-Implementation (aktuelle Variante):

* Felder-Types: `Bool`, `Int8/16/32/64`, `UInt8/16/32/64`,
  `Float32/64`, `String`, `Bytes` (= sequence<octet>).
* Felder-Reihenfolge: ``@dataclass``-declaration-order.
* **Nicht dabei (Composite-Erweiterung / v1.4):** Nested structs, sequence<T> für
  beliebige T, fixed-size arrays, Unions, Optional.
"""

from __future__ import annotations

from dataclasses import fields, is_dataclass
from typing import Any, Callable, ClassVar, Type, TypeVar

from .cdr import CdrReader, CdrWriter

T = TypeVar("T")


# =============================================================================
# IDL-Primitive-Typen als Type-Annotations. Python-seitig sind das
# einfache Aliase auf int/float/str/bytes/bool — der Decorator inspiziert
# nur die _idl_kind_-Marker, nicht den eigentlichen Type-Check.
# =============================================================================


class _IdlKind:
    __slots__ = ("name", "write", "read")

    def __init__(
        self,
        name: str,
        write: Callable[[CdrWriter, Any], None],
        read: Callable[[CdrReader], Any],
    ) -> None:
        self.name = name
        self.write = write
        self.read = read

    def __repr__(self) -> str:
        return f"IdlKind({self.name})"


Bool = _IdlKind("bool", CdrWriter.write_bool, CdrReader.read_bool)
Int8 = _IdlKind("int8", CdrWriter.write_i8, CdrReader.read_i8)
UInt8 = _IdlKind("uint8", CdrWriter.write_u8, CdrReader.read_u8)
Int16 = _IdlKind("int16", CdrWriter.write_i16, CdrReader.read_i16)
UInt16 = _IdlKind("uint16", CdrWriter.write_u16, CdrReader.read_u16)
Int32 = _IdlKind("int32", CdrWriter.write_i32, CdrReader.read_i32)
UInt32 = _IdlKind("uint32", CdrWriter.write_u32, CdrReader.read_u32)
Int64 = _IdlKind("int64", CdrWriter.write_i64, CdrReader.read_i64)
UInt64 = _IdlKind("uint64", CdrWriter.write_u64, CdrReader.read_u64)
Float32 = _IdlKind("float32", CdrWriter.write_f32, CdrReader.read_f32)
Float64 = _IdlKind("float64", CdrWriter.write_f64, CdrReader.read_f64)
String = _IdlKind("string", CdrWriter.write_string, CdrReader.read_string)
Bytes = _IdlKind("bytes", CdrWriter.write_bytes, CdrReader.read_bytes)


# =============================================================================
# Composite-Markers fuer Composite-Erweiterung: Sequence, Array, Optional, Nested-Struct.
# Alle implementieren `_IdlKind`-kompatibles write/read auf Instance-Level.
# =============================================================================


class _IdlSequence(_IdlKind):
    """``sequence<T>`` — u32 Laenge + N Elemente. `T` ist ein ``_IdlKind``
    oder ein ``@idl_struct``-dekorierter Dataclass-Typ."""

    __slots__ = ("inner",)

    def __init__(self, inner: Any) -> None:
        self.inner = inner
        self.name = f"sequence<{_describe(inner)}>"
        self.write = self._write  # type: ignore[assignment]
        self.read = self._read  # type: ignore[assignment]

    def _write(self, w: CdrWriter, values: Any) -> None:
        values = list(values or [])
        w.write_u32(len(values))
        for v in values:
            _write_any(w, self.inner, v)

    def _read(self, r: CdrReader) -> list:
        n = r.read_u32()
        return [_read_any(r, self.inner) for _ in range(n)]

    def __class_getitem__(cls, inner: Any) -> "_IdlSequence":
        return cls(inner)


class _IdlArray(_IdlKind):
    """``T[N]`` — fester Count, **kein** Length-Prefix. Spec XCDR2 §7.4.3."""

    __slots__ = ("inner", "count")

    def __init__(self, inner: Any, count: int) -> None:
        if count <= 0:
            raise ValueError(f"Array-Count muss > 0 sein, got {count}")
        self.inner = inner
        self.count = count
        self.name = f"array<{_describe(inner)}, {count}>"
        self.write = self._write  # type: ignore[assignment]
        self.read = self._read  # type: ignore[assignment]

    def _write(self, w: CdrWriter, values: Any) -> None:
        values = list(values or [])
        if len(values) != self.count:
            raise ValueError(
                f"Array[{self.count}]: erwartet genau {self.count} Elemente, "
                f"got {len(values)}",
            )
        for v in values:
            _write_any(w, self.inner, v)

    def _read(self, r: CdrReader) -> list:
        return [_read_any(r, self.inner) for _ in range(self.count)]

    def __class_getitem__(cls, args: Any) -> "_IdlArray":
        if not isinstance(args, tuple) or len(args) != 2:
            raise TypeError("Array[T, N] braucht genau zwei Parameter")
        inner, count = args
        return cls(inner, int(count))


class _IdlOptional(_IdlKind):
    """``Optional<T>`` — u8 present-Flag + (falls gesetzt) Wert."""

    __slots__ = ("inner",)

    def __init__(self, inner: Any) -> None:
        self.inner = inner
        self.name = f"optional<{_describe(inner)}>"
        self.write = self._write  # type: ignore[assignment]
        self.read = self._read  # type: ignore[assignment]

    def _write(self, w: CdrWriter, value: Any) -> None:
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


class _IdlEnum(_IdlKind):
    """Python-``IntEnum`` → XCDR2-Int32. Ermoeglicht typisierte Enum-
    Felder, die on-the-wire als Int32 codiert werden.

    Encoding:
    * Write: ``int(enum_value)`` als Int32.
    * Read: ``EnumCls(raw_int)`` (wirft ``ValueError`` bei unbekanntem
      Wert — das erzwingt forward-kompatibel Strenge).
    """

    __slots__ = ("enum_cls",)

    def __init__(self, enum_cls: type) -> None:
        self.enum_cls = enum_cls
        self.name = f"enum<{enum_cls.__name__}>"
        self.write = self._write  # type: ignore[assignment]
        self.read = self._read  # type: ignore[assignment]

    def _write(self, w: CdrWriter, value: Any) -> None:
        if value is None:
            raise ValueError(f"Enum {self.enum_cls.__name__} darf nicht None sein")
        w.write_i32(int(value))

    def _read(self, r: CdrReader) -> Any:
        raw = r.read_i32()
        return self.enum_cls(raw)


class _IdlUnion(_IdlKind):
    """Discriminated-Union (IDL ``union T switch(D)`` §7.4.1.4.4).

    Wire-Format: Discriminator (Int32 oder IntEnum) + Value der dem
    Discriminator zugeordneten Variante.

    Python-Mapping:

    * Ein `@idl_union(...)`-Dekorator baut eine Klasse mit den
      Attributen ``discriminator`` und ``value``.
    * Das Mapping ``cases = {disc_val: (field_name, inner_kind)}``
      sagt pro Discriminator-Wert, welches Feld serialisiert wird.
    * Ein optionaler ``default`` wird bei unbekanntem Disc genommen.
    """

    __slots__ = ("cases", "disc_kind", "default")

    def __init__(
        self,
        disc_kind: Any,
        cases: dict[int, tuple[str, Any]],
        default: Any | None = None,
    ) -> None:
        self.disc_kind = _kind_from_annotation(disc_kind)
        self.cases = {int(k): (v[0], v[1]) for k, v in cases.items()}
        self.default = default
        self.name = f"union<{self.disc_kind.name}>"
        self.write = self._write  # type: ignore[assignment]
        self.read = self._read  # type: ignore[assignment]

    def _resolve_case(self, disc: Any) -> tuple[str, Any] | None:
        key = int(disc)
        if key in self.cases:
            return self.cases[key]
        return self.default

    def _write(self, w: CdrWriter, value: Any) -> None:
        if value is None:
            raise ValueError("Union-Value darf nicht None sein")
        disc = value.discriminator
        self.disc_kind.write(w, disc)
        case = self._resolve_case(disc)
        if case is None:
            raise ValueError(f"kein Case fuer Discriminator {disc!r} und kein default")
        _fname, inner = case
        _write_any(w, inner, value.value)

    def _read(self, r: CdrReader) -> Any:
        disc = self.disc_kind.read(r)
        case = self._resolve_case(disc)
        if case is None:
            raise ValueError(f"kein Case fuer Discriminator {disc!r} und kein default")
        _fname, inner = case
        val = _read_any(r, inner)
        return _UnionValue(discriminator=disc, value=val)


class _UnionValue:
    """Runtime-Container eines Union-Wertes. Stellt den Discriminator
    und den gewaehlten Case-Wert bereit."""

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
) -> _IdlKind:
    """Construct-Helfer: erstellt einen `_IdlUnion`-IdlKind und liefert
    zusaetzlich `TYPE_NAME` + Constructor-Helper fuer Nutzer.

    Beispiel::

        from zerodds.idl import idl_union, Int32, String, Float64

        MyUnion = idl_union(
            typename="u::MyUnion",
            discriminator=Int32,
            cases={0: ("n", Int32), 1: ("s", String)},
            default=("f", Float64),
        )

        # Nutzer-Code:
        val = MyUnion.make(0, 42)       # case 0 → Int32
        encoded = MyUnion.encode(val)
        decoded = MyUnion.decode(encoded)
    """
    kind = _IdlUnion(discriminator, cases, default)

    class _UnionFacade:
        """Wrapper um einen _IdlUnion, mit ``encode``/``decode``/``make``/
        ``TYPE_NAME`` fuer Nutzer-Code."""

        TYPE_NAME = typename

        @staticmethod
        def encode(v: Any) -> bytes:
            w = CdrWriter()
            kind.write(w, v)
            return w.into_bytes()

        @staticmethod
        def decode(b: bytes) -> Any:
            r = CdrReader(b)
            return kind.read(r)

        @staticmethod
        def make(disc: Any, value: Any) -> _UnionValue:
            return _UnionValue(discriminator=disc, value=value)

        # Erlaubt Nutzung als nested IDL-Kind: inner-kind in @idl_struct.
        _idl_union_kind = kind

    return _UnionFacade


class _IdlStruct(_IdlKind):
    """Nested ``@idl_struct`` — encode/decode via innerer ``encode()``/
    ``decode()``-Methoden."""

    __slots__ = ("cls",)

    def __init__(self, struct_cls: type) -> None:
        self.cls = struct_cls
        self.name = getattr(struct_cls, "TYPE_NAME", struct_cls.__name__)
        self.write = self._write  # type: ignore[assignment]
        self.read = self._read  # type: ignore[assignment]

    def _write(self, w: CdrWriter, value: Any) -> None:
        if value is None:
            raise ValueError(f"Nested-Struct {self.name} darf nicht None sein")
        # Wir schreiben in den existierenden Buffer weiter — nutzen also
        # die internen Kind-Calls statt encode() (das wuerde einen neuen
        # Buffer bauen und Alignment verlieren).
        for fname, kind in self.cls._idl_fields:  # type: ignore[attr-defined]
            kind.write(w, getattr(value, fname))

    def _read(self, r: CdrReader) -> Any:
        values = {
            fname: kind.read(r)
            for fname, kind in self.cls._idl_fields  # type: ignore[attr-defined]
        }
        return self.cls(**values)


# Public-Aliases.
Sequence = _IdlSequence
Array = _IdlArray
Optional = _IdlOptional


def _describe(t: Any) -> str:
    if isinstance(t, _IdlKind):
        return t.name
    if isinstance(t, type) and is_dataclass(t):
        return getattr(t, "TYPE_NAME", t.__name__)
    return repr(t)


def _write_any(w: CdrWriter, kind: Any, value: Any) -> None:
    if isinstance(kind, _IdlKind):
        kind.write(w, value)
        return
    if isinstance(kind, type) and is_dataclass(kind):
        _IdlStruct(kind).write(w, value)
        return
    raise TypeError(f"_write_any: nicht unterstuetzter Kind {kind!r}")


def _read_any(r: CdrReader, kind: Any) -> Any:
    if isinstance(kind, _IdlKind):
        return kind.read(r)
    if isinstance(kind, type) and is_dataclass(kind):
        return _IdlStruct(kind).read(r)
    raise TypeError(f"_read_any: nicht unterstuetzter Kind {kind!r}")


def _kind_from_annotation(annot: Any) -> _IdlKind:
    """Erlaubt sowohl `field: Int32` als auch die Raw-Classes."""
    import enum as _enum

    if isinstance(annot, _IdlKind):
        return annot
    # IntEnum-Klasse → wird als Int32 serialisiert.
    if isinstance(annot, type) and issubclass(annot, _enum.IntEnum):
        return _IdlEnum(annot)
    # Nested Dataclass → wird als _IdlStruct eingewickelt.
    if isinstance(annot, type) and is_dataclass(annot):
        return _IdlStruct(annot)
    # Fallback: wenn jemand `int` / `str` / `bytes` / `bool` nutzt,
    # mappen wir auf den natuerlichen Rust-Typ.
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
        f"@idl_struct: feldtyp {annot!r} nicht unterstuetzt. "
        f"Nutze Bool/Int8/.../UInt64/Float32/Float64/String/Bytes, "
        f"Sequence[T], Array[T, N], Optional[T], eine nested @idl_struct-"
        f"Dataclass oder Standard-Primitives (int/bool/float/str/bytes).",
    )


# =============================================================================
# Decorator
# =============================================================================


def idl_struct(*, typename: str) -> Callable[[Type[T]], Type[T]]:
    """Dekorator. Macht eine `@dataclass` zu einem ZeroDDS-IDL-Type.

    Fuegt an:
    * ``TYPE_NAME = typename`` (class-level const).
    * ``encode(self) -> bytes`` — XCDR2-LE.
    * ``decode(cls, data: bytes) -> cls`` — classmethod.

    Der Decorator **darf nach `@dataclass` stehen**, damit er die
    `__dataclass_fields__`-Metadaten inspizieren kann::

        @idl_struct(typename="foo::Bar")
        @dataclass
        class Bar:
            x: Int32
    """

    def apply(cls: Type[T]) -> Type[T]:
        if not is_dataclass(cls):
            raise TypeError(
                f"@idl_struct: {cls.__name__} ist keine @dataclass — "
                f"deklaration-reihenfolge: @idl_struct(...) ueber @dataclass.",
            )
        # Bei `from __future__ import annotations` (PEP 563) sind die
        # `f.type`-Werte Strings. Wir loesen sie im Modul-Namespace der
        # Klasse + zerodds.idl-Namespace auf.
        import sys

        module_globals: dict[str, Any] = {}
        mod = sys.modules.get(cls.__module__)
        if mod is not None:
            module_globals.update(vars(mod))
        # Immer auch die eigenen Idl-Kind-Konstanten dazunehmen, damit
        # der Nutzer sie weder importieren noch re-exportieren muss,
        # solange er via `zerodds.<Kind>` arbeitet.
        module_globals.setdefault("Bool", Bool)
        for _name, _kind in (
            ("Int8", Int8), ("UInt8", UInt8),
            ("Int16", Int16), ("UInt16", UInt16),
            ("Int32", Int32), ("UInt32", UInt32),
            ("Int64", Int64), ("UInt64", UInt64),
            ("Float32", Float32), ("Float64", Float64),
            ("String", String), ("Bytes", Bytes),
        ):
            module_globals.setdefault(_name, _kind)

        def _resolve(annot: Any) -> Any:
            if isinstance(annot, str):
                try:
                    return eval(annot, module_globals)  # noqa: S307
                except NameError as exc:
                    raise TypeError(
                        f"@idl_struct: annotation-string {annot!r} nicht "
                        f"aufloesbar im modul {cls.__module__!r}. Beim Gebrauch "
                        f"von `from __future__ import annotations` muessen die "
                        f"Kind-Konstanten im Modul importiert sein.",
                    ) from exc
            return annot

        kinds: list[tuple[str, _IdlKind]] = []
        for f in fields(cls):
            kinds.append((f.name, _kind_from_annotation(_resolve(f.type))))

        def _encode(self: Any) -> bytes:
            w = CdrWriter()
            for fname, kind in kinds:
                kind.write(w, getattr(self, fname))
            return w.into_bytes()

        def _decode(klass: Type[T], data: bytes) -> T:
            r = CdrReader(data)
            values = {fname: kind.read(r) for fname, kind in kinds}
            return klass(**values)  # type: ignore[call-arg]

        cls.TYPE_NAME = typename  # type: ignore[attr-defined]
        cls._idl_fields = kinds  # type: ignore[attr-defined]
        cls.encode = _encode  # type: ignore[attr-defined]
        cls.decode = classmethod(_decode)  # type: ignore[attr-defined]
        return cls

    return apply


# =============================================================================
# Runtime-Introspection
# =============================================================================


def is_idl_struct(obj: Any) -> bool:
    """True wenn ``obj`` (oder dessen Klasse) mit ``@idl_struct`` dekoriert ist."""
    cls: Any = obj if isinstance(obj, type) else type(obj)
    return hasattr(cls, "TYPE_NAME") and hasattr(cls, "_idl_fields")


def type_name_of(cls_or_obj: Any) -> str:
    """Liefert den IDL-TYPE_NAME eines dekorierten Dataclass-Typs/-Objekts."""
    cls: Any = cls_or_obj if isinstance(cls_or_obj, type) else type(cls_or_obj)
    name: ClassVar[str] = getattr(cls, "TYPE_NAME", None)  # type: ignore[assignment]
    if name is None:
        raise TypeError(f"{cls.__name__} hat keinen @idl_struct(typename=...)-Decorator")
    return name  # type: ignore[return-value]
