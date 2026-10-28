# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Fresh-module contract probe. No DDS binding, compiler or generated fixture."""
import hashlib
import importlib.util
import sys


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def refuses(exception, operation):
    try:
        operation()
    except exception:
        return
    raise AssertionError(f"expected {exception.__name__}")


if len(sys.argv) != 2:
    raise ValueError("usage: consumer.py GENERATED_MODULE_PATH")
spec = importlib.util.spec_from_file_location("fresh_generated_support", sys.argv[1])
if spec is None or spec.loader is None:
    raise ValueError("generated module cannot be loaded")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)
descriptors = module.ZD_TOPIC_DESCRIPTORS
point = descriptors["bridge::Point"]
require(point.cls is module.bridge.Point, "descriptor lost actual generated class")
require(isinstance(point.type_object, bytes) and len(point.type_object) > 0, "metadata absent")
sample = point.cls(id=42, reading=65535)
require(point.encode(sample) == bytes.fromhex("2a000000ffff"), "LE independent octets differ")
require(point.encode(sample, endian="big") == bytes.fromhex("0000002affff"), "BE octets differ")
decoded = point.decode(bytes.fromhex("0000002affff"), endian="big")
require((decoded.id, decoded.reading) == (42, 65535), "decoded values differ")
expected_key = bytes.fromhex("0000002a000000000000000000000000")
require(len(expected_key) == 16, "independent key oracle length")
require(point.key_hash(sample) == expected_key, "zero-padded key differs")
require(point.key_holder(sample) == bytes.fromhex("0000002a"), "actual holder acquired hash padding")
zero = point.cls(id=0, reading=99)
require(point.key_holder(zero) == bytes(4), "zero key lost its actual holder")
require(point.key_hash(zero) == bytes(16), "legitimate zero hash changed")
from_holder = point.decode_key_holder(bytes.fromhex("0000002a"))
require((from_holder.id, from_holder.reading) == (42, 0), "key inverse or shared non-key default differs")
require(point.decode_key_holder(bytes(4)).id == 0, "zero key failed to reconstruct")
sample.reading = 7
require(point.key_hash(sample) == expected_key, "non-key data changed key")
wrong = bytearray(expected_key)
wrong[3] ^= 1
require(point.key_hash(sample) != wrong, "mutated key oracle was not distinguished")

short = descriptors["bridge::ShortName"]
a = short.cls(name="abcdX", value=1)
b = short.cls(name="abcdY", value=1)
ka, kb = short.key_hash(a), short.key_hash(b)
require(ka == bytes.fromhex("00000006616263645800000000000000"), "short string key differs")
require(kb == bytes.fromhex("00000006616263645900000000000000"), "second string key differs")
require(ka[:8] == kb[:8] and ka != kb, "full keys must preserve common-prefix difference")
require(short.key_holder(a) == bytes.fromhex("00000006616263645800"), "bounded string holder differs")
require(short.key_holder(b) == bytes.fromhex("00000006616263645900"), "second bounded holder differs")
require(short.decode_key_holder(short.key_holder(a)).name == "abcdX", "bounded holder inverse differs")
long = descriptors["bridge::LongName"]
require(long.key_hash(long.cls(name="a", value=1)) == hashlib.md5(
    bytes.fromhex("000000026100"), usedforsecurity=False).digest(), "maximum-size MD5 choice differs")
require(long.key_holder(long.cls(name="a", value=1)) == bytes.fromhex("000000026100"),
        "MD5 digest was substituted for actual holder bytes")
require(long.decode_key_holder(bytes.fromhex("000000026100")).name == "a", "MD5-selected type lost actual holder inverse")

empty = descriptors["bridge::Empty"]
require(empty.encode(empty.cls()) == b"", "empty type acquired bytes")
require(isinstance(empty.decode(b""), empty.cls), "empty decode lost instance")
require(empty.key_holder(empty.cls()) is None, "keyless is not an empty keyed holder")
extended = descriptors["bridge::Extended"]
for calibration in (None, 1.25):
    value = extended.cls(id=13, calibration=calibration)
    got = extended.decode(extended.encode(value))
    require(got.id == 13 and got.calibration == calibration, "optional fidelity lost")
    restored = extended.decode_key_holder(extended.key_holder(value))
    require(restored.id == 13 and restored.calibration is None, "optional non-key was invented instead of defaulted")

ordered = descriptors["bridge::OrderedKeys"]
ordered_value = ordered.cls(first=4, second=1, value=99)
require(ordered.key_holder(ordered_value) == bytes.fromhex("0000000100000004"), "member-id holder order differs")
restored = ordered.decode_key_holder(bytes.fromhex("0000000100000004"))
require((restored.first, restored.second, restored.value) == (4, 1, 17), "ForcedFinal inverse or explicit default differs")
nested = descriptors["bridge::NestedKey"]
refuses(NotImplementedError, lambda: nested.decode_key_holder(b""))
refuses(NotImplementedError, lambda: empty.decode_key_holder(b""))

from zerodds_wire import Truncated
refuses(Truncated, lambda: point.decode(b"\x00"))
refuses(Truncated, lambda: point.decode_key_holder(b"\x00"))
refuses(ValueError, lambda: point.decode_key_holder(expected_key))
refuses(ValueError, lambda: point.decode(point.encode(sample) + b"\x00"))
refuses(ValueError, lambda: point.encode(sample, representation=2 if point.representation == 0 else 0))
refuses(ValueError, lambda: point.decode(point.encode(sample), endian="unknown"))
refuses(TypeError, lambda: point.encode(None))
print("PYTHON_GENERATED_DESCRIPTOR_PASS: actual classes, LE/BE, optional, exact keys, prefix collision, refusals")
