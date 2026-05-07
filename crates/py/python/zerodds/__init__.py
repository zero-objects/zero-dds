"""ZeroDDS Python API.

Rust-native DDS-Implementation, exponiert ueber PyO3. API-Shape folgt
bewusst OMG DDS 1.4 §2.2.2 — wer von cyclonedds-python oder rti-dds
kommt, findet sich direkt zurecht.

Beispiel::

    import zerodds

    factory = zerodds.DomainParticipantFactory.instance()
    participant = factory.create_participant(0)
    topic = participant.create_bytes_topic("Chatter")
    publisher = participant.create_publisher()
    writer = publisher.create_bytes_writer(topic)
    subscriber = participant.create_subscriber()
    reader = subscriber.create_bytes_reader(topic)

    writer.wait_for_matched_subscription(1, timeout_secs=5.0)
    reader.wait_for_matched_publication(1, timeout_secs=5.0)

    writer.write(b"hello world")
    reader.wait_for_data(timeout_secs=3.0)
    for payload in reader.take():
        print(payload)

Typisierte DdsType-Bindings (`@dataclass`-Mapping via IDL-Gen)
kommen aus `crates/idl-rust`-Codegen ueber Python-Generator.
"""

# Reine Python-Module ohne Abhaengigkeit zum kompilierten Extension-
# Module. Damit `zerodds.cdr` und `zerodds.idl` auch ohne
# `maturin develop --features extension-module` nutzbar bleiben
# (pytest in CI braucht kein Rust-Build).
from . import cdr, idl  # noqa: F401

# Optional-Import des Rust-Extension-Moduls. Wenn nicht gebaut, setzen
# wir alle DDS-Klassen auf `None` und der Nutzer bekommt beim Zugriff
# einen klaren `ImportError` (nicht den kryptischen `attribute` not found).
try:
    from ._core import (  # noqa: F401
        __version__,
        BytesReader,
        BytesTopic,
        BytesWriter,
        DomainParticipant,
        DomainParticipantFactory,
        Publisher,
        Shape,
        ShapeReader,
        ShapeTopic,
        ShapeWriter,
        Subscriber,
    )

    _CORE_AVAILABLE = True
except ImportError as _core_err:  # pragma: no cover - nur in dev-ohne-maturin
    _CORE_AVAILABLE = False
    _CORE_IMPORT_ERROR = _core_err
    __version__ = "0.0.0+nocore"

    def _core_not_available(*_args: object, **_kwargs: object) -> None:
        raise ImportError(
            "zerodds._core ist nicht kompiliert. Fuehre "
            "`maturin develop --features extension-module` im crates/py/ aus.",
        ) from _CORE_IMPORT_ERROR

    # Platzhalter-Objekte, damit Tests die Klassen zumindest importieren
    # koennen ohne ImportError; Instanziierung wirft dann den klaren
    # Fehler.
    class _CoreStub:
        def __init_subclass__(cls, **_kw: object) -> None:
            _core_not_available()

        def __init__(self, *_a: object, **_kw: object) -> None:
            _core_not_available()

        @staticmethod
        def instance() -> None:
            _core_not_available()

    BytesReader = BytesTopic = BytesWriter = _CoreStub  # type: ignore[assignment,misc]
    DomainParticipant = DomainParticipantFactory = _CoreStub  # type: ignore[assignment,misc]
    Publisher = Subscriber = _CoreStub  # type: ignore[assignment,misc]
    Shape = ShapeReader = ShapeTopic = ShapeWriter = _CoreStub  # type: ignore[assignment,misc]
from .idl import (  # noqa: F401
    Array,
    Bool,
    Bytes,
    Float32,
    Float64,
    Int8,
    Int16,
    Int32,
    Int64,
    Optional,
    Sequence,
    String,
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    idl_struct,
    idl_union,
    is_idl_struct,
    type_name_of,
)

__all__ = [
    "__version__",
    "BytesReader",
    "BytesTopic",
    "BytesWriter",
    "DomainParticipant",
    "DomainParticipantFactory",
    "Publisher",
    "Shape",
    "ShapeReader",
    "ShapeTopic",
    "ShapeWriter",
    "Subscriber",
    "cdr",
    "idl",
    "idl_struct",
    "is_idl_struct",
    "type_name_of",
    "Bool",
    "Bytes",
    "Float32",
    "Float64",
    "Int8",
    "Int16",
    "Int32",
    "Int64",
    "String",
    "UInt8",
    "UInt16",
    "UInt32",
    "UInt64",
    "Array",
    "Optional",
    "Sequence",
    "idl_union",
]
