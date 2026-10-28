# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Generated typed DDS contract, independent of any native DDS binding.

Callbacks come from the shared IDL codec planner. This module validates their
boundary contract but contains no IDL parser, key layout, or serializer.
"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable

from .endianness import Endianness
from .reader import Reader
from .typesupport import Status, TryConstruct
from .writer import Writer


@dataclass(frozen=True)
class TopicDescriptor:
    """One generated class and its exact DDS wire/identity contract.

    Representation uses DDS IDs: 0 is XCDR1 and 2 is XCDR2. Extensibility
    is 0 final, 1 appendable, 2 mutable. Metadata unavailable to the AST
    prevents typed registration, without preventing wire-only import.
    """

    cls: type
    type_name: str
    _type_object: bytes | None
    keyed: bool
    extensibility: int
    representation: int
    _encode: Callable[[Writer, Any], None]
    _decode: Callable[[Reader, TryConstruct], Any]
    _key_hash: Callable[[Any], bytes] | None
    _metadata_error: str = "CompleteTypeObject unavailable"
    _key_holder: Callable[[Any], bytes] | None = None
    _decode_key_holder: Callable[[Reader, TryConstruct], Any] | None = None
    _key_decode_error: str = "this descriptor has no generated inverse key holder"
    _type_dependencies: tuple[bytes, ...] = ()

    def __post_init__(self) -> None:
        if not isinstance(self._type_dependencies, tuple) or any(
            not isinstance(value, bytes) or not value for value in self._type_dependencies
        ):
            raise TypeError("type dependencies must be an immutable tuple of nonempty TypeObject bytes")
        if not isinstance(self.cls, type) or not self.type_name:
            raise ValueError("a descriptor requires a class and IDL type name")
        if type(self.representation) is not int or self.representation not in (0, 2):
            raise ValueError("representation must be DDS XCDR1=0 or XCDR2=2")
        if type(self.extensibility) is not int or self.extensibility not in (0, 1, 2):
            raise ValueError("extensibility must be final=0, appendable=1 or mutable=2")
        if type(self.keyed) is not bool or self.keyed != (self._key_hash is not None):
            raise ValueError("keyed flag and key encoder disagree")
        if self._key_hash is not None and not callable(self._key_hash):
            raise TypeError("generated key callback is not callable")
        if self._key_holder is not None:
            if not self.keyed:
                raise ValueError("a keyless descriptor cannot supply a key holder")
            if not callable(self._key_holder):
                raise TypeError("generated key holder callback is not callable")
        if self._decode_key_holder is not None:
            if not self.keyed:
                raise ValueError("a keyless descriptor cannot supply an inverse key holder")
            if not callable(self._decode_key_holder):
                raise TypeError("generated inverse key holder callback is not callable")
        if self._type_object is not None and (
            not isinstance(self._type_object, bytes) or not self._type_object
        ):
            raise ValueError("CompleteTypeObject must be nonempty bytes or unavailable")
        if not callable(self._encode) or not callable(self._decode):
            raise TypeError("generated encode/decode callbacks are required")

    @property
    def type_dependencies(self) -> tuple[bytes, ...]:
        """Complete dependency closure, encoded by the shared metadata writer."""
        return self._type_dependencies

    @property
    def type_object(self) -> bytes:
        """Serialized CompleteTypeObject, or explicit refusal to register."""
        if self._type_object is None:
            raise NotImplementedError(self._metadata_error)
        return self._type_object

    def _order(self, endian: str, representation: int | None) -> Endianness:
        if representation is not None and (
            type(representation) is not int or representation != self.representation
        ):
            raise ValueError("the requested representation is not implemented by this codec")
        if endian == "little":
            return Endianness.LITTLE
        if endian == "big":
            return Endianness.BIG
        raise ValueError("endian must be 'little' or 'big'")

    def _sample(self, sample: Any) -> None:
        if not isinstance(sample, self.cls):
            raise TypeError(f"expected {self.type_name} ({self.cls.__name__})")

    def encode(self, sample: Any, *, endian: str = "little", representation: int | None = None) -> bytes:
        """Encode a body without encapsulation using the generated codec."""
        order = self._order(endian, representation)
        self._sample(sample)
        writer = Writer(order, max_align=8 if self.representation == 0 else 4)
        self._encode(writer, sample)
        return writer.finish()

    def decode(self, payload: bytes, *, endian: str = "little", representation: int | None = None) -> Any:
        """Decode completely or fail. Discard and trailing bytes never pass."""
        order = self._order(endian, representation)
        if not isinstance(payload, bytes):
            raise TypeError("payload must be bytes")
        reader = Reader(payload, order, max_align=8 if self.representation == 0 else 4)
        sample = self._decode(reader, TryConstruct.DISCARD)
        if reader.reader_status() is not Status.OK or reader.remaining() != 0:
            raise ValueError("generated decoder refused the body or left trailing bytes")
        self._sample(sample)
        return sample

    def key_hash(self, sample: Any) -> bytes:
        """The generated 16-byte KeyHash, never a hash of the whole sample."""
        self._sample(sample)
        key = bytes(16) if self._key_hash is None else self._key_hash(sample)
        if not isinstance(key, bytes) or len(key) != 16:
            raise ValueError("generated KeyHash must be exactly 16 bytes")
        return key

    def key_holder(self, sample: Any) -> bytes | None:
        """Actual PLAIN_CDR2-BE holder bytes, or None for a keyless type.

        None is not an empty holder. Older keyed descriptors without the
        generated callback explicitly refuse.
        """
        self._sample(sample)
        if not self.keyed:
            return None
        if self._key_holder is None:
            raise NotImplementedError("this descriptor has no generated key holder writer")
        holder = self._key_holder(sample)
        if not isinstance(holder, bytes):
            raise TypeError("generated key holder must be bytes")
        return holder

    def decode_key_holder(self, holder: bytes) -> Any:
        """Read actual PLAIN_CDR2-BE key bytes with generated non-key defaults.

        Only the shared generator's supported inverse is callable. Unavailable
        nested key contexts and keyless descriptors explicitly refuse.
        Callers must supply holder bytes, not a hash. Format and full consumption
        are checked; arbitrary digest bytes that happen to form a valid holder
        cannot be recognized as a digest from the bytes alone.
        """
        if not isinstance(holder, bytes):
            raise TypeError("key holder must be bytes")
        if self._decode_key_holder is None:
            raise NotImplementedError(self._key_decode_error)
        reader = Reader(holder, Endianness.BIG, max_align=4)
        sample = self._decode_key_holder(reader, TryConstruct.DISCARD)
        if reader.reader_status() is not Status.OK or reader.remaining() != 0:
            raise ValueError("generated inverse refused the holder or left trailing bytes")
        self._sample(sample)
        return sample
