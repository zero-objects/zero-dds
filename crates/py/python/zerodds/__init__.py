"""ZeroDDS Python API.

Rust-native DDS implementation, exposed via PyO3. The API shape
deliberately follows OMG DDS 1.4 §2.2.2 — anyone coming from
cyclonedds-python or rti-dds finds their way around immediately.

Example::

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

Typed topics come from `zerodds-idlc generate --lang python`: pass a
generated `ZD_TOPIC_DESCRIPTORS[...]` entry to `IdlTopic`. The generated
code depends on `zerodds_wire` (`runtime/python`), the one Python CDR
codec — there is no second, hand-written one to import here.
"""

# Optional import of the Rust extension module. If not built, we set
# all DDS classes to `None` and the user gets a clear `ImportError` on
# access (not the cryptic `attribute` not found).
try:
    from ._core import (  # noqa: F401
        __version__,
        BytesContentFilteredTopic,
        BytesReader,
        BytesTopic,
        BytesWriter,
        DataReaderListener,
        DataReaderQos,
        DataWriterListener,
        DataWriterQos,
        DomainParticipant,
        DomainParticipantFactory,
        GuardCondition,
        KeyedReader,
        KeyedReading,
        KeyedTopic,
        KeyedWriter,
        Publisher,
        QueryCondition,
        ReadCondition,
        SampleInfo,
        Shape,
        ShapeReader,
        ShapeTopic,
        ShapeWriter,
        Subscriber,
        WaitSet,
    )
    from . import sample_state, view_state, instance_state  # noqa: F401

    _CORE_AVAILABLE = True
except ImportError as _core_err:  # pragma: no cover - only in dev-without-maturin
    _CORE_AVAILABLE = False
    _CORE_IMPORT_ERROR = _core_err
    __version__ = "0.0.0+nocore"

    def _core_not_available(*_args: object, **_kwargs: object) -> None:
        raise ImportError(
            "zerodds._core is not compiled. Run "
            "`maturin develop --features extension-module` in crates/py/.",
        ) from _CORE_IMPORT_ERROR

    # Placeholder objects so that tests can at least import the classes
    # without ImportError; instantiation then throws the clear
    # error.
    class _CoreStub:
        def __init_subclass__(cls, **_kw: object) -> None:
            _core_not_available()

        def __init__(self, *_a: object, **_kw: object) -> None:
            _core_not_available()

        @staticmethod
        def instance() -> None:
            _core_not_available()

    BytesReader = BytesTopic = BytesWriter = _CoreStub  # type: ignore[assignment,misc]
    BytesContentFilteredTopic = SampleInfo = _CoreStub  # type: ignore[assignment,misc]
    KeyedReader = KeyedReading = KeyedTopic = KeyedWriter = _CoreStub  # type: ignore[assignment,misc]
    DomainParticipant = DomainParticipantFactory = _CoreStub  # type: ignore[assignment,misc]
    Publisher = Subscriber = _CoreStub  # type: ignore[assignment,misc]
    Shape = ShapeReader = ShapeTopic = ShapeWriter = _CoreStub  # type: ignore[assignment,misc]
    GuardCondition = WaitSet = _CoreStub  # type: ignore[assignment,misc]
    DataWriterQos = DataReaderQos = _CoreStub  # type: ignore[assignment,misc]
    DataWriterListener = DataReaderListener = _CoreStub  # type: ignore[assignment,misc]
    ReadCondition = QueryCondition = _CoreStub  # type: ignore[assignment,misc]
    sample_state = view_state = instance_state = None  # type: ignore[assignment]
from .topic import IdlReader, IdlTopic, IdlWriter  # noqa: F401

__all__ = [
    "__version__",
    "BytesContentFilteredTopic",
    "BytesReader",
    "BytesTopic",
    "BytesWriter",
    "KeyedReader",
    "KeyedReading",
    "KeyedTopic",
    "KeyedWriter",
    "SampleInfo",
    "DataReaderListener",
    "DataReaderQos",
    "DataWriterListener",
    "DataWriterQos",
    "DomainParticipant",
    "DomainParticipantFactory",
    "GuardCondition",
    "QueryCondition",
    "ReadCondition",
    "Publisher",
    "Shape",
    "ShapeReader",
    "ShapeTopic",
    "ShapeWriter",
    "Subscriber",
    "WaitSet",
    "IdlTopic",
    "IdlWriter",
    "IdlReader",
    "sample_state",
    "view_state",
    "instance_state",
]
