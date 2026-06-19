# zerodds (Python)

Python bindings for ZeroDDS, the native Rust DDS implementation.

## Installation (Dev-Setup)

```bash
# Maturin in a virtualenv
python3 -m venv .venv
source .venv/bin/activate
pip install maturin pytest

# Build + install in develop mode (rebuild on code change)
cd crates/py
maturin develop --features extension-module
```

## Schnellstart

```python
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
```

## Tests

```bash
pytest crates/py/python/tests/
```

## Scope (current version)

- `DomainParticipantFactory`, `DomainParticipant` with `assert_liveliness`,
  `ignore_*`, `contains_entity`, `get_discovered_*`
- `BytesTopic` / `BytesWriter` / `BytesReader` for opaque payload
- `ShapeTopic` / `ShapeWriter` / `ShapeReader` + `Shape` dataclass
  for cross-vendor interop against Cyclone/Fast-DDS ShapesDemo
- Status getters: `publication_matched_status`, `liveliness_lost_status`,
  `subscription_matched_status`, `sample_lost_status`, …
- `GuardCondition` + `WaitSet`
- Sync primitives: `wait_for_matched_*`, `wait_for_data`
- GIL release during all blocking calls

## Roadmap

- IDL→Python dataclass generator (`@dataclass` from IDL); then
  arbitrary `DdsType`s work natively from Python.
- ROS2 pytest integration + multi-process live tests + sphinx docs.
- Load QoS profiles from XML/YAML (analogous to the Fast-DDS QoS Profiles
  Manager).

## Architecture

The Rust crate `zerodds-py` builds as a `cdylib` (`zerodds._core`). The
Python wrapper `python/zerodds/__init__.py` re-exports and adds
pythonic sugar where needed.

The Rust core is **identical** to the Rust API — no Python-
specific business logic, only type conversions + GIL release.
This keeps the binding thin and keeps the DDS semantics in one place.
