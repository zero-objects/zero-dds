# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Tests the descriptor boundary itself, not DDS or a generated codec."""
import unittest
from dataclasses import replace

from zerodds_wire.topic_descriptor import TopicDescriptor


class Sample:
    pass


def descriptor(**changes):
    # Metadata intentionally unavailable, not a fabricated TypeObject.
    base = TopicDescriptor(Sample, "Test::Sample", None, False, 0, 2,
                           lambda writer, sample: None,
                           lambda reader, policy: Sample(), None)
    return replace(base, **changes)


class DescriptorBoundary(unittest.TestCase):
    def test_mode_validation_precedes_codec_call(self):
        calls = []
        support = descriptor(_encode=lambda writer, sample: calls.append(sample))
        for representation in (0, 1, 3, True, "2"):
            with self.assertRaises(ValueError):
                support.encode(Sample(), representation=representation)
        with self.assertRaises(ValueError):
            support.encode(Sample(), endian="BE")
        self.assertEqual(calls, [])
        self.assertEqual(support.encode(Sample(), representation=2), b"")
        self.assertEqual(len(calls), 1)

    def test_metadata_unavailable_is_not_an_empty_success(self):
        with self.assertRaises(NotImplementedError):
            _ = descriptor().type_object
        with self.assertRaises(ValueError):
            descriptor(_type_object=b"")

    def test_bad_key_lengths_and_callback_contract_refuse(self):
        for length in (0, 15, 17):
            support = descriptor(keyed=True, _key_hash=lambda sample, n=length: bytes(n))
            with self.assertRaises(ValueError):
                support.key_hash(Sample())
        with self.assertRaises(ValueError):
            descriptor(keyed=True)
        with self.assertRaises(TypeError):
            descriptor(keyed=True, _key_hash=42)
        self.assertEqual(descriptor().key_hash(Sample()), bytes(16))

    def test_decoder_must_consume_body_and_return_declared_class(self):
        with self.assertRaises(ValueError):
            descriptor().decode(b"\x00")
        with self.assertRaises(TypeError):
            descriptor(_decode=lambda reader, policy: None).decode(b"")
        self.assertIsInstance(descriptor().decode(b""), Sample)

    def test_holder_contract_preserves_unknown_empty_and_zero(self):
        self.assertIsNone(descriptor().key_holder(Sample()))
        keyed = descriptor(keyed=True, _key_hash=lambda sample: bytes(16))
        with self.assertRaises(NotImplementedError):
            keyed.key_holder(Sample())
        for holder in (b"", bytes(4), b"bounded\x00"):
            support = replace(keyed, _key_holder=lambda sample, value=holder: value)
            self.assertEqual(support.key_holder(Sample()), holder)
            with self.assertRaises(TypeError):
                support.key_holder(None)
        with self.assertRaises(TypeError):
            replace(keyed, _key_holder=lambda sample: bytearray()).key_holder(Sample())
        with self.assertRaises(TypeError):
            replace(keyed, _key_holder=42)
        with self.assertRaises(ValueError):
            descriptor(_key_holder=lambda sample: b"")

    def test_inverse_holder_refusal_and_strict_consumption(self):
        with self.assertRaises(NotImplementedError):
            descriptor().decode_key_holder(b"")
        keyed = descriptor(keyed=True, _key_hash=lambda sample: bytes(16),
                           _decode_key_holder=lambda reader, policy: Sample())
        self.assertIsInstance(keyed.decode_key_holder(b""), Sample)
        with self.assertRaises(ValueError):
            keyed.decode_key_holder(bytes(16))
        with self.assertRaises(TypeError):
            keyed.decode_key_holder(bytearray())
        with self.assertRaises(TypeError):
            replace(keyed, _decode_key_holder=lambda reader, policy: None).decode_key_holder(b"")
        with self.assertRaises(TypeError):
            replace(keyed, _decode_key_holder=42)


if __name__ == "__main__":
    unittest.main()
