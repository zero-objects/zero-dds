"""IDL topic path (§6.1 vendor spec).

Pure-Python wrappers that combine ``BytesTopic`` / ``BytesWriter`` /
``BytesReader`` with the ``@idl_struct`` encoder/decoder,
so that users can publish/subscribe typed dataclasses directly —
without dealing with byte encoding.

Example::

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
            f"IdlTopic requires an @idl_struct-decorated @dataclass; "
            f"{cls.__name__} is not one.",
        )


class IdlTopic(Generic[T]):
    """Topic handle that encapsulates an ``@idl_struct`` dataclass type.

    Internally the topic holds a ``BytesTopic`` value. The type name on
    the wire comes from ``cls.TYPE_NAME``, which is set by the
    ``@idl_struct`` decorator (§3.2 vendor spec).
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
    """Writer that calls `cls.encode(self)` before each ``write``."""

    def __init__(self, inner: Any, cls: Type[T]) -> None:
        self._inner = inner
        self._cls: Type[T] = cls

    def write(self, value: T) -> None:
        if not isinstance(value, self._cls):
            raise TypeError(
                f"IdlWriter[{self._cls.__name__}].write expects {self._cls.__name__}, "
                f"got {type(value).__name__}",
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
    """Reader that deserializes each ``take`` sample via ``cls.decode(bytes)``."""

    def __init__(self, inner: Any, cls: Type[T]) -> None:
        self._inner = inner
        self._cls: Type[T] = cls

    def take(self) -> list[T]:
        # Endian-aware path: thread the wire byte order from the encapsulation
        # header into the generated decoder so a big-endian peer's sample is
        # read correctly. Falls back to the little-endian `take()` for an inner
        # reader that predates `take_endian`.
        take_endian = getattr(self._inner, "take_endian", None)
        if take_endian is None:
            raw_samples = self._inner.take()
            return [self._cls.decode(b) for b in raw_samples]  # type: ignore[attr-defined]
        out: list[T] = []
        while True:
            got = take_endian()
            if got is None:
                break
            data, big_endian, representation = got
            out.append(
                self._cls.decode(  # type: ignore[attr-defined]
                    data, "be" if big_endian else "le", representation
                )
            )
        return out

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
