"""IDL topic path (§6.1 vendor spec).

Pure-Python wrappers that combine ``BytesTopic`` / ``BytesWriter`` /
``BytesReader`` with a generated ``TopicDescriptor``
(``zerodds_wire.topic_descriptor.TopicDescriptor``, from
``zerodds-idlc generate --lang python``), so that users can publish/
subscribe typed dataclasses directly — without dealing with byte
encoding.

Example::

    import zerodds
    from my_generated_module import ZD_TOPIC_DESCRIPTORS

    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_fast(0)
    topic = zerodds.IdlTopic(p, "Temp", ZD_TOPIC_DESCRIPTORS["sensor_msgs::msg::Temperature"])
    writer = topic.create_writer(p.create_publisher())
    reader = topic.create_reader(p.create_subscriber())

    writer.wait_for_matched_subscription(1, 5.0)
    reader.wait_for_matched_publication(1, 5.0)
    writer.write(topic.cls(celsius=23, sensor_id="A7"))
    reader.wait_for_data(3.0)
    for msg in reader.take():
        assert isinstance(msg, topic.cls)
"""

from __future__ import annotations

from typing import Any, Generic, Type, TypeVar

T = TypeVar("T")


class IdlTopic(Generic[T]):
    """Topic from a generated ``TopicDescriptor``.

    Pass ``generated.ZD_TOPIC_DESCRIPTORS["module::Type"]`` for native
    DDS identity, keys and lifecycle. The descriptor's CompleteTypeObject
    establishes an immutable Rust codec, shared by ordinary native
    entities — the same TypeSupport bridge C#'s ``<Type>.TypeSupport.cs``
    and Java's ``ZdTypeSupports.Support_<tag>`` provide.
    """

    def __init__(self, participant: Any, name: str, descriptor: Any) -> None:
        from zerodds_wire.topic_descriptor import TopicDescriptor

        if not isinstance(descriptor, TopicDescriptor):
            raise TypeError(
                f"IdlTopic requires a zerodds_wire.topic_descriptor.TopicDescriptor "
                f"(ZD_TOPIC_DESCRIPTORS[...] from generated code), got "
                f"{type(descriptor).__name__}",
            )
        self._descriptor = descriptor
        self._participant = participant
        self._cls = descriptor.cls
        self._bytes_topic = participant.create_descriptor_topic(
            name,
            descriptor.type_name,
            [descriptor.type_object, *descriptor.type_dependencies],
            descriptor.keyed,
            descriptor.extensibility,
            descriptor.representation,
        )
        self._name = name

    @property
    def name(self) -> str:
        return self._name

    @property
    def type_name(self) -> str:
        return self._bytes_topic.type_name

    @property
    def cls(self) -> Type[T]:
        return self._cls

    @property
    def descriptor(self) -> Any:
        return self._descriptor

    def create_writer(self, publisher: Any) -> IdlWriter[T]:
        return IdlWriter(publisher.create_descriptor_writer(self._bytes_topic), self._descriptor)

    def create_reader(self, subscriber: Any) -> IdlReader[T]:
        return IdlReader(subscriber.create_descriptor_reader(self._bytes_topic), self._descriptor)


class IdlWriter(Generic[T]):
    """Writer that encodes each ``write`` through the generated descriptor."""

    def __init__(self, inner: Any, descriptor: Any) -> None:
        self._inner = inner
        self._descriptor = descriptor

    def write(self, value: T) -> None:
        self._inner.write(self._descriptor.encode(value))

    def register_instance(self, value: T) -> int:
        return self._inner.register_instance(self._descriptor.encode(value))

    def dispose(self, value: T) -> None:
        self._inner.dispose(self._descriptor.encode(value))

    def unregister_instance(self, value: T) -> None:
        self._inner.unregister_instance(self._descriptor.encode(value))

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
    """Reader that decodes each taken sample through the generated descriptor."""

    def __init__(self, inner: Any, descriptor: Any) -> None:
        self._inner = inner
        self._descriptor = descriptor

    def take_with_info(self) -> list[tuple[T, Any]]:
        return [(self._descriptor.decode(body), info) for body, info in self._inner.take_with_info()]

    def take(self) -> list[T]:
        return [value for value, info in self.take_with_info() if info.valid_data]

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
