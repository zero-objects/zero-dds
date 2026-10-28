# zerodds (Python)

Python bindings for ZeroDDS, the native Rust DDS implementation.

## Installation (Dev-Setup)

```bash
# Maturin in a virtualenv
python3 -m venv .venv
source .venv/bin/activate
pip install maturin pytest
pip install ./runtime/python

# Build + install in develop mode (rebuild on code change)
cd crates/py
maturin develop --features extension-module
```

## Quickstart

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

## Scope

- `DomainParticipantFactory`, `DomainParticipant` with `assert_liveliness`,
  `ignore_*`, `contains_entity`, `get_discovered_*`
- `BytesTopic` / `BytesWriter` / `BytesReader` for opaque payload
- `ShapeTopic` / `ShapeWriter` / `ShapeReader` + `Shape` dataclass
  for cross-vendor interop against Cyclone/Fast-DDS ShapesDemo
- `KeyedTopic` / `KeyedWriter` / `KeyedReader` + `SampleInfo` for the keyed
  lifecycle: `register_instance`, `dispose`, `unregister_instance`, and the
  instance state a reader observes for each
- `BytesContentFilteredTopic` — a Python predicate over the payload
- Status getters: `publication_matched_status`, `liveliness_lost_status`,
  `subscription_matched_status`, `sample_lost_status`, …
- `GuardCondition`, `ReadCondition`, `QueryCondition`, `WaitSet`
- `DataWriterQos` / `DataReaderQos` — all twenty-two DDS 1.4 §2.2.3 policies
- `DataWriterListener` / `DataReaderListener` — Python callbacks per status
- `zerodds.topic` — `IdlTopic` / `IdlWriter` / `IdlReader`, a pure-Python
  wrapper turning a `zerodds-idlc generate --lang python` descriptor
  (`ZD_TOPIC_DESCRIPTORS[...]`) into native typed entities. One CDR codec
  (`runtime/python`'s `zerodds_wire`), the same TypeSupport bridge C#/Java
  connect their generated companion through — no separate hand-written
  Python codec
- `zerodds.aio` — the same calls as coroutines
- `zerodds.loader` — the `ctypes` path over `libzerodds`, for a tree with no
  Rust toolchain
- Sync primitives: `wait_for_matched_*`, `wait_for_data`
- GIL release during all blocking calls

## Generated Python and native topics

The current [shared IDL CLI](../../tools/idlc/README.md) generates Python classes,
wire codecs and `ZD_TOPIC_DESCRIPTORS` in one module. Each descriptor carries
the exact IDL name, class, key contract, extensibility, representation and
available CompleteTypeObject. Its encode/decode methods handle payload bodies,
not native DDS entities. See the [fresh-output consumer](../../tools/idlc/tests/python-support/README.md)
for concrete usage and the boundary it checks.

Pass a generated descriptor to `IdlTopic` to establish a native typed topic:

```python
import support as generated  # freshly generated from the support.idl fixture
descriptor = generated.ZD_TOPIC_DESCRIPTORS["bridge::Point"]
topic = zerodds.IdlTopic(participant, "Points", descriptor)
writer = topic.create_writer(participant.create_publisher())
writer.write(descriptor.cls(id=177, reading=1234))
```

The immutable CompleteTypeObject supplies native discovery identity, keyedness
and extensibility. Rust reconstructs its common AST and uses the existing codec
and DDS writer/reader policy paths. Python codecs run only at the binding
boundary, never from DDS cache locks. `take_with_info()` returns generated
values and native lifecycle metadata. Keyless types and scalar/string/enum,
alias and nested-structure keys are supported. Generated descriptors carry the
complete TypeObject dependency closure, registered before endpoint discovery.
Missing TypeObject dependencies and unsupported
inverse key contexts fail registration explicitly. The descriptor's generated
representation is selected for both endpoints.

The [two-process generated consumer](tests/generated_descriptor_live.py)
checks data and dispose reconstruction. This is not a cross-vendor or ROS 2
interoperability claim. Fixed `ShapeTopic` and `KeyedTopic` routes remain
separate. Collection, union, optional and external inverse key contexts still
refuse native registration. The pure descriptor's Python inverse callback is
more limited than this Rust native inverse and is not used by DDS lifecycle.
Generated modules require the matching `zerodds-wire` distribution, declared
as an exact-version binding dependency. Its package sources live in
`runtime/python`. The installed-wheel path does not require a repository
`PYTHONPATH`. Python 3.9 or later is required by the shared key-hash runtime.
The [installed-wheel consumer](tests/installed_wheel_consumer.sh) installs
both local wheels offline into a fresh environment before running that
generated native exchange.

## Documentation

The binding handbook starts at [docs/index.rst](docs/index.rst), with
[examples](docs/examples.rst) and their current transport boundaries.

```bash
pip install sphinx
python3 -m sphinx -b html crates/py/docs crates/py/docs/_build/html
```

## Architecture

The Rust crate `zerodds-py` builds as a `cdylib` (`zerodds._core`). The
Python wrapper `python/zerodds/__init__.py` re-exports and adds
pythonic sugar where needed.

The extension uses the Rust DDS core. Generated per-topic support now connects
to its existing typed entity paths through a Rust-only shared codec. The
Python facade does not expose every Rust API contract, and typed registration
explicitly refuses metadata or key contexts outside the bounded support above.
