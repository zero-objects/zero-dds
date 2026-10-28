"""Custom IDL type: generated code + XCDR2-LE roundtrip.

Shows how to define a project-specific type without hand-written Python
as a DDS payload. `03_idl_topic.idl` is the source; `03_idl_topic_gen.py`
(committed next to it) is `zerodds-idlc generate --lang python`'s output —
regenerate with::

    zerodds-idlc generate --lang python --out . 03_idl_topic.idl
    mv 03_idl_topic.py 03_idl_topic_gen.py

The generated `TopicDescriptor`'s `encode()`/`decode()` produce
byte-exactly the same data the Rust-side encoder produces, guaranteeing
cross-language interop — there is no second, hand-written Python codec to
keep in step with it.
"""

from __future__ import annotations

import importlib.util
import pathlib
import sys


def _load_generated():
    path = pathlib.Path(__file__).parent / "03_idl_topic_gen.py"
    spec = importlib.util.spec_from_file_location("_idl_topic_gen", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    # Registered before exec: the generated module uses
    # `from __future__ import annotations` (deferred annotations), and
    # dataclasses.dataclass on 3.10+ resolves those strings via
    # `sys.modules[cls.__module__]` — a module never entered into
    # sys.modules has no entry to resolve against.
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def main() -> None:
    generated = _load_generated()
    descriptor = generated.ZD_TOPIC_DESCRIPTORS["sensors::TemperatureReading"]
    TemperatureReading = descriptor.cls

    msg = TemperatureReading(
        sensor_id="boiler-01",
        timestamp_us=1_698_765_432,
        celsius=87.25,
        raw=b"\x00\x01\x02\x03",
    )
    print(f"original:  {msg}")
    print(f"type-name: {descriptor.type_name}")

    wire = descriptor.encode(msg)
    print(f"encoded:   {len(wire)} bytes = {wire.hex()}")

    decoded = descriptor.decode(wire)
    print(f"decoded:   {decoded}")
    # `sequence<octet>` decodes as `list[int]`, not `bytes` — compared as
    # bytes here since that is what the caller wrote.
    assert msg.sensor_id == decoded.sensor_id
    assert msg.timestamp_us == decoded.timestamp_us
    assert msg.celsius == decoded.celsius
    assert bytes(decoded.raw) == msg.raw
    print("roundtrip ok")


if __name__ == "__main__":
    main()
