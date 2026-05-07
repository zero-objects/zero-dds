# zerodds (Python)

Python-Bindings für ZeroDDS, die native Rust-DDS-Implementation.

## Installation (Dev-Setup)

```bash
# Maturin in einem virtualenv
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

## Scope (aktuelle Version)

- `DomainParticipantFactory`, `DomainParticipant` mit `assert_liveliness`,
  `ignore_*`, `contains_entity`, `get_discovered_*`
- `BytesTopic` / `BytesWriter` / `BytesReader` für opaken Payload
- `ShapeTopic` / `ShapeWriter` / `ShapeReader` + `Shape`-Dataclass
  für Cross-Vendor-Interop gegen Cyclone-/Fast-DDS-ShapesDemo
- Status-Getter: `publication_matched_status`, `liveliness_lost_status`,
  `subscription_matched_status`, `sample_lost_status`, …
- `GuardCondition` + `WaitSet`
- Sync-Primitives: `wait_for_matched_*`, `wait_for_data`
- GIL-Release während allen blocking Calls

## Roadmap

- IDL→Python-Dataclass-Generator (`@dataclass` aus IDL); dann
  funktionieren beliebige `DdsType` native aus Python.
- ROS2-pytest-Integration + Multi-Process-Live-Tests + sphinx-Docs.
- QoS-Profile aus XML/YAML laden (analog Fast-DDS QoS Profiles
  Manager).

## Architektur

Das Rust-Crate `zerodds-py` baut als `cdylib` (`zerodds._core`). Der
Python-Wrapper `python/zerodds/__init__.py` re-exportiert und fügt
bei Bedarf pythonischen Zucker hinzu.

Der Rust-Kern ist **identisch** zum Rust-API — keine Python-
spezifische Business-Logik, nur Type-Conversions + GIL-Release.
Das hält das Binding dünn und hält die DDS-Semantik auf einer Stelle.
