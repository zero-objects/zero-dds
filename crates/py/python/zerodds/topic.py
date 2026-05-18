"""IDL-Topic-Pfad (§6.1 Vendor-Spec).

Pure-Python-Wrapper, die ``BytesTopic`` / ``BytesWriter`` /
``BytesReader`` mit dem ``@idl_struct``-Encoder/Decoder kombinieren,
sodass Anwender direkt typisierte Dataclasses publishen/subscriben
koennen — ohne sich mit Byte-Encoding zu beschaeftigen.

Beispiel::

    from dataclasses import dataclass
    import zerodds
    from zerodds.idl import idl_struct, Int32, String

    @idl_struct(typename="sensor_msgs::msg::Temperature")
    @dataclass
    class Temperature:
        celsius: Int32
        sensor_id: String

    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_fast(0)
    topic = zerodds.IdlTopic(p, "Temp", Temperature)
    writer = topic.create_writer(p.create_publisher())
    reader = topic.create_reader(p.create_subscriber())

    writer.wait_for_matched_subscription(1, 5.0)
    reader.wait_for_matched_publication(1, 5.0)
    writer.write(Temperature(celsius=23, sensor_id="A7"))
    reader.wait_for_data(3.0)
    for msg in reader.take():
        assert isinstance(msg, Temperature)
"""

from __future__ import annotations

from typing import Any, Generic, Type, TypeVar

from .idl import is_idl_struct, type_name_of

T = TypeVar("T")


def _ensure_idl_struct(cls: Type[T]) -> None:
    if not is_idl_struct(cls):
        raise TypeError(
            f"IdlTopic verlangt eine mit @idl_struct dekorierte @dataclass; "
            f"{cls.__name__} ist keine.",
        )


class IdlTopic(Generic[T]):
    """Topic-Handle, der einen ``@idl_struct``-Dataclass-Type kapselt.

    Intern haelt der Topic einen ``BytesTopic``-Wert. Der Type-Name auf
    dem Wire kommt aus ``cls.TYPE_NAME``, das vom ``@idl_struct``-
    Decorator gesetzt wird (§3.2 Vendor-Spec).
    """

    def __init__(self, participant: Any, name: str, cls: Type[T]) -> None:
        _ensure_idl_struct(cls)
        self._participant = participant
        self._cls: Type[T] = cls
        self._bytes_topic = participant.create_bytes_topic(name)
        self._name = name

    @property
    def name(self) -> str:
        return self._name

    @property
    def type_name(self) -> str:
        return type_name_of(self._cls)

    @property
    def cls(self) -> Type[T]:
        return self._cls

    def create_writer(self, publisher: Any) -> IdlWriter[T]:
        return IdlWriter(publisher.create_bytes_writer(self._bytes_topic), self._cls)

    def create_reader(self, subscriber: Any) -> IdlReader[T]:
        return IdlReader(subscriber.create_bytes_reader(self._bytes_topic), self._cls)


class IdlWriter(Generic[T]):
    """Writer, der `cls.encode(self)` vor jedem ``write`` aufruft."""

    def __init__(self, inner: Any, cls: Type[T]) -> None:
        self._inner = inner
        self._cls: Type[T] = cls

    def write(self, value: T) -> None:
        if not isinstance(value, self._cls):
            raise TypeError(
                f"IdlWriter[{self._cls.__name__}].write erwartet {self._cls.__name__}, "
                f"bekommen {type(value).__name__}",
            )
        encoded = value.encode()  # type: ignore[attr-defined]
        self._inner.write(encoded)

    def wait_for_matched_subscription(self, count: int, timeout_secs: float) -> None:
        self._inner.wait_for_matched_subscription(count, timeout_secs)

    def matched_subscription_count(self) -> int:
        return self._inner.matched_subscription_count()

    def publication_matched_status(self) -> tuple:
        return self._inner.publication_matched_status()

    def liveliness_lost_status(self) -> tuple:
        return self._inner.liveliness_lost_status()

    def offered_deadline_missed_status(self) -> tuple:
        return self._inner.offered_deadline_missed_status()


class IdlReader(Generic[T]):
    """Reader, der jedes ``take``-Sample via ``cls.decode(bytes)`` deserialisiert."""

    def __init__(self, inner: Any, cls: Type[T]) -> None:
        self._inner = inner
        self._cls: Type[T] = cls

    def take(self) -> list[T]:
        raw_samples = self._inner.take()
        return [self._cls.decode(b) for b in raw_samples]  # type: ignore[attr-defined]

    def wait_for_data(self, timeout_secs: float) -> None:
        self._inner.wait_for_data(timeout_secs)

    def wait_for_matched_publication(self, count: int, timeout_secs: float) -> None:
        self._inner.wait_for_matched_publication(count, timeout_secs)

    def matched_publication_count(self) -> int:
        return self._inner.matched_publication_count()

    def subscription_matched_status(self) -> tuple:
        return self._inner.subscription_matched_status()

    def sample_lost_status(self) -> tuple:
        return self._inner.sample_lost_status()

    def requested_deadline_missed_status(self) -> tuple:
        return self._inner.requested_deadline_missed_status()


__all__ = [
    "IdlReader",
    "IdlTopic",
    "IdlWriter",
]
