# `zerodds-py` v1.0 — vendor spec

ZeroDDS vendor spec. Implemented in `crates/py/`.

## Motivation

The OMG has published **no** normative Python PSM for DDS. Existing
Python bindings (cyclonedds-python, rti-dds-python, fastdds-python)
each take a different path:

1. **ctypes wrapper** around a C library (Eclipse Cyclone DDS Python).
2. **C API via pybind11** with auto-generated stubs (FastDDS Python).
3. **C API via Cython** plus C-side codegen for topic types (RTI Connext DDS Python).

ZeroDDS has exactly one codec per language for topic types: the code
that `zerodds-idlc generate --lang python` generates from the IDL, on top of the
runtime `runtime/python` (package `zerodds_wire`). The same IDL generates
the codecs of the other languages; their bytes are measured against the
Rust reference.

ZeroDDS splits the Python surface into three parts:

- **`crates/py/src/`** — PyO3 bindings via `cdylib`, which provide the module
  `zerodds._core`; the path for users who can run `maturin develop`
  or install from PyPI.
- **`crates/py/python/zerodds/`** — pure Python layer on
  `zerodds._core`:
  - `topic.py` — `IdlTopic`/`IdlWriter`/`IdlReader`, which write and read typed
    dataclasses via a generated `TopicDescriptor`.
  - `loader.py` — pure `ctypes` loader against `libzerodds.{so,dylib,dll}`
    from `crates/zerodds-c-api`. Serves the distro package path
    (system libzerodds installed, no Rust build).
- **`runtime/python/`** — `zerodds_wire`: `Writer`/`Reader` (XCDR1,
  XCDR2, PL-CDR, both byte orders), `TopicDescriptor`, the
  bound and value checks that the generated code relies on.

This vendor spec defines the Python surface normatively. It should be read
in parallel with `zerodds-c-api-1.0` and `zerodds-java-omgdds-1.0`
— all three realize the same DCPS model under the
respective language idioms.

## Goals

- **DCPS surface:** a Python API over DDS 1.4 §2.2.2 with a
  PyClass mapping per entity (`DomainParticipantFactory`,
  `DomainParticipant`, `Publisher`, `Subscriber`, `Topic`, `DataWriter`,
  `DataReader`).
- **One codec from the IDL:** `zerodds-idlc generate --lang python`
  generates one dataclass and one `TopicDescriptor` per type
  (`ZD_TOPIC_DESCRIPTORS["modul::Typ"]`); `IdlTopic` only accepts such
  a descriptor.
- **Byte identity with the Rust path:** the Python column of the
  serialization measurement (`wireprobe-python-matrix`) compares the
  bytes of the generated code with the Rust reference.
- **Two-path architecture:** PyO3 (performance, RTPS-native) +
  pure `ctypes` (distro package, zero build). Both paths can be used
  independently; users choose per use case.
- **GIL release on blocking calls:** all wait functions (`write`,
  `wait_for_data`, `wait_for_matched_*`, `WaitSet.wait`) release the
  GIL (PyO3 `py.allow_threads`), so that Python threads can run
  concurrently.
- **abi3-py38:** the PyO3 binding is built for the stable Python ABI 3.8+
  (`pyo3 = { features = ["abi3-py38"] }`). A single wheel
  runs on CPython 3.8 … 3.13.

## Non-goals

- **OMG DDS Python PSM compliance**: does not exist. This spec
  defines the mapping convention itself; it is normalized against DDS 1.4 §2.2.2
  (language-neutral).
- **OMG DDS Python PSM compliance**: does not exist. This spec
  defines the mapping convention itself.
- **Direct RTPS wire in pure Python**: the pure Python path goes via
  the ctypes loader bridge to the Rust library. There is no
  pure-Python RTPS datagram in v1.0 or v2.0.

## §1 Architecture

### §1.1 Module layout

```
crates/py/
  Cargo.toml           # crate-type = ["cdylib", "rlib"]
  pyproject.toml       # maturin build config
  build.rs             # version.h for extension-module
  src/
    lib.rs             # module root, doc, no code
    ffi.rs             # PyO3 bindings: 13 PyClasses
  python/
    zerodds/
      __init__.py      # re-exports _core, topic, aio, loader
      topic.py         # IdlTopic / IdlWriter / IdlReader
      aio.py           # asyncio wrapper
      loader.py        # pure ctypes loader against libzerodds.so
    tests/
      _generated/      # IDL fixtures + their zerodds-idlc output
      test_*.py        # pytest
  examples/
    01_bytes_pubsub.py
    02_shape_pubsub.py
    03_idl_topic.py    # + 03_idl_topic.idl and the generated code
  docs/                # sphinx
runtime/python/
  zerodds_wire/        # Writer / Reader / TopicDescriptor
    conf.py, index.rst, api.rst, quickstart.rst, examples.rst
```

### §1.2 PyO3 module `zerodds._core`

The Rust crate is built with `--features extension-module` as a `cdylib`
and provides the module `zerodds_py` (imported from Python as
`zerodds._core`). PyClasses in `src/ffi.rs`:

| PyClass | Mapped DCPS type |
|---|---|
| `DomainParticipantFactory` | `DomainParticipantFactory` (DDS 1.4 §2.2.2.2.1) |
| `DomainParticipant` | `DomainParticipant` (§2.2.2.2.2) |
| `BytesTopic`, `BytesContentFilteredTopic` | `Topic<RawBytes>` (§2.2.2.3.1), `ContentFilteredTopic` |
| `DescriptorTopic`, `DescriptorWriter`, `DescriptorReader` | Topic, DataWriter and DataReader of a type from a generated `TopicDescriptor` (CompleteTypeObject, key, extensibility) |
| `KeyedTopic`, `KeyedWriter`, `KeyedReader`, `KeyedReading` | Topic with a key over bytes |
| `ShapeTopic`, `ShapeWriter`, `ShapeReader`, `Shape` | `ShapeType` (cross-vendor interop) |
| `Publisher`, `Subscriber` | §2.2.2.4.1, §2.2.2.5.1 |
| `BytesWriter`, `BytesReader` | `DataWriter<RawBytes>`, `DataReader<RawBytes>` |
| `SampleInfo` | §2.2.2.5.5 |
| `GuardCondition`, `WaitSet` | §2.2.2.6.3, §2.2.2.6.1 |

In addition, the QoS, listener and condition classes from `src/qos.rs`,
`src/listener.rs` and `src/conditions.rs` (§6).

All PyClass methods convert `DdsError` via
`dds_err_to_py` (line 41) into `RuntimeError`. Blocking calls release
the GIL via `py.allow_threads`.

### §1.3 Python wrapper `python/zerodds/`

The outer package `zerodds` re-exports the classes from `_core`
and `IdlTopic`/`IdlWriter`/`IdlReader` from `topic.py`. The
Python modules add functionality that does not go through PyO3:

- `topic.py` — `IdlTopic(participant, name, descriptor)` requires a
  `zerodds_wire.topic_descriptor.TopicDescriptor` and, via
  `DomainParticipant.create_descriptor_topic`, creates a topic with the
  type's CompleteTypeObject, key specification and extensibility;
  `IdlWriter`/`IdlReader` encode and decode with the
  generated code.
- `loader.py` — `Runtime`, `Writer`, `Reader` as a pure `ctypes`
  loader against `libzerodds.{so,dylib,dll}`. Follows
  `zerodds-ffi-loader-1.0` §3.1. Header signatures come from
  `crates/zerodds-c-api/include/zerodds.h`.

### §1.4 Choosing between the two paths

Users choose one path per application:

| Path | When | Build effort |
|---|---|---|
| PyO3 (`zerodds.DomainParticipantFactory.instance()`) | Performance, topic type API, environment that can run `maturin` | `pip install zerodds` (wheel from PyPI) or `maturin develop --features extension-module` |
| `ctypes` (`zerodds.loader.Runtime.create_participant()`) | Distro package, no Rust toolchain in the build, embedded Python | only `libzerodds.so` must be installed |

The two paths share nothing except the wire format (XCDR2-LE)
and the domain ID. They can coexist in the same process,
but are not connected by cross calls — that is v1.1 (multi-process bridge).

### §1.5 Layer position

Layer 6 — PSMs / bindings.

Direct dependencies: `zerodds-dcps` (`crates/dcps/`,
layer 4 core services) for the PyO3 bindings;
`zerodds-c-api` (`crates/zerodds-c-api/`, layer 6 C-FFI)
for the ctypes loader path.

## §2 OMG API coverage

Mapping DDS 1.4 §2.2.2 → Python counterpart.

### §2.1 DomainParticipantFactory (DDS 1.4 §2.2.2.2.1)

| Spec operation | Python equivalent |
|---|---|
| `get_instance()` | `DomainParticipantFactory.instance()` (singleton, classmethod) |
| `create_participant(domain_id, qos, listener, mask)` | `factory.create_participant(domain_id)` — QoS=default, listener=None |
| `create_participant_offline(domain_id)` | `factory.create_participant_offline(domain_id)` — no discovery, for tests |
| `delete_participant(p)` | implicitly via the Python GC; `__del__` deletes the Rust side |

### §2.2 DomainParticipant (DDS 1.4 §2.2.2.2.2)

| Spec | Python |
|---|---|
| `get_domain_id()` | `p.domain_id` (property) |
| `create_topic(name, type, qos, listener, mask)` | `p.create_bytes_topic(name)` / `p.create_shape_topic(name)` |
| `create_publisher(qos, listener, mask)` | `p.create_publisher()` |
| `create_subscriber(qos, listener, mask)` | `p.create_subscriber()` |
| `assert_liveliness()` | `p.assert_liveliness()` |
| `ignore_participant(handle)` | `p.ignore_participant(handle: int)` |
| `ignore_topic(handle)` | `p.ignore_topic(handle: int)` |
| `ignore_publication(handle)` | `p.ignore_publication(handle: int)` |
| `ignore_subscription(handle)` | `p.ignore_subscription(handle: int)` |
| `contains_entity(handle)` | `p.contains_entity(handle: int) -> bool` |
| `get_discovered_topics()` | `p.get_discovered_topics() -> list[int]` |
| `get_discovered_participants()` | `p.get_discovered_participants() -> list[int]` |

### §2.3 Publisher / Subscriber

| Spec | Python |
|---|---|
| `Publisher.create_datawriter(topic, qos, listener, mask)` | `pub.create_bytes_writer(topic)` / `pub.create_shape_writer(topic)` |
| `Subscriber.create_datareader(topic, qos, listener, mask)` | `sub.create_bytes_reader(topic)` / `sub.create_shape_reader(topic)` |

### §2.4 DataWriter (DDS 1.4 §2.2.2.4.2)

| Spec | Python (`BytesWriter` / `ShapeWriter`) |
|---|---|
| `write(data, handle, source_timestamp)` | `writer.write(data: bytes)` / `writer.write(shape: Shape)` |
| `register_instance(data)` | `writer.register_instance(shape: Shape) -> int` (`ShapeWriter` only) |
| `unregister_instance(handle)` | `writer.unregister_instance(shape: Shape)` (`ShapeWriter` only) |
| `dispose(handle)` | `writer.dispose(shape: Shape)` (`ShapeWriter` only) |
| `wait_for_acknowledgments(timeout)` | implicitly via `wait_for_matched_subscription` + ACK loop in `crates/dcps` |
| `get_matched_subscriptions()` | `writer.matched_subscription_count() -> int` |
| `get_publication_matched_status()` | `writer.publication_matched_status() -> (total_count, total_count_change, current_count, current_count_change, last_subscription_handle)` |
| `get_liveliness_lost_status()` | `writer.liveliness_lost_status() -> (total_count, total_count_change)` |
| `get_offered_deadline_missed_status()` | `writer.offered_deadline_missed_status() -> (total_count, total_count_change)` |

Additionally: `wait_for_matched_subscription(count, timeout_secs)` as a
sync helper.

### §2.5 DataReader (DDS 1.4 §2.2.2.5.3)

| Spec | Python (`BytesReader` / `ShapeReader`) |
|---|---|
| `take(max_samples, sample_states, view_states, instance_states)` | `reader.take() -> list[bytes]` / `list[Shape]` |
| `read(...)` | deliberately omitted in v1.0; `take` is the default |
| `wait_for_historical_data(timeout)` | deliberately omitted; not needed in the RTPS path |
| `get_matched_publications()` | `reader.matched_publication_count() -> int` |
| `get_subscription_matched_status()` | `reader.subscription_matched_status() -> tuple` |
| `get_sample_lost_status()` | `reader.sample_lost_status() -> (total_count, total_count_change)` |
| `get_requested_deadline_missed_status()` | `reader.requested_deadline_missed_status() -> tuple` |

Additionally: `wait_for_data(timeout_secs)` and
`wait_for_matched_publication(count, timeout_secs)`.

### §2.6 WaitSet / conditions (DDS 1.4 §2.2.2.6)

| Spec | Python |
|---|---|
| `WaitSet()` | `zerodds._core.WaitSet()` |
| `WaitSet.attach_condition(cond)` | `ws.attach_guard_condition(gc)` (v1.0: GuardCondition only) |
| `WaitSet.wait(active_conditions, timeout)` | `ws.wait(timeout_secs: float) -> int` (number triggered; raises `TimeoutError` on timeout) |
| `GuardCondition()` | `zerodds._core.GuardCondition()` |
| `GuardCondition.set_trigger_value(v)` | `gc.set_trigger_value(v: bool)` |
| `GuardCondition.get_trigger_value()` | `gc.get_trigger_value() -> bool` |

In v1.0, `GuardCondition` and `WaitSet` are only reachable directly via
`zerodds._core`; the outer `zerodds` namespace
(`__init__.py`) intentionally does not yet re-export them in the outer
namespace; `ReadCondition`/`QueryCondition` are available via §6.6
(via `zerodds._core`), the re-export in the outer WaitSet namespace follows in v1.1.

### §2.7 ShapeType — cross-vendor interop type

`Shape` (PyClass) and `ShapeType` (`crates/dcps/src/interop.rs`) are
byte-identical. Fields: `color: str`, `x: i32`, `y: i32`,
`shapesize: i32` (= 30 default). Type name on the wire: `"ShapeType"`
(`crates/dcps/src/interop.rs:91`: `TYPE_NAME: &'static str = "ShapeType"`),
compatible with the Cyclone/Fast-DDS ShapesDemo, whose default topic type
is also `ShapeType` (without a module prefix).

## §3 IDL mapping — generated code over `zerodds_wire`

From the IDL, `zerodds-idlc generate --lang python` generates a module with
one dataclass per type, the encoding functions per type and
`ZD_TOPIC_DESCRIPTORS`. The encoding (XTypes 1.3 §7.4, XCDR1, XCDR2 and
PL-CDR, both byte orders) is in `runtime/python/zerodds_wire`;
the mapping of the IDL types to Python is governed by `crates/lang-rules`
(`adapter/python.rs`), the same planning for all languages.

### §3.1 Mapping

| IDL | Python |
|---|---|
| Integer and floating-point types, `boolean`, `char`, `string`, `wstring` | `int`, `float`, `bool`, `str` |
| `sequence<T>` | `list[T]` |
| `T[N]` | type annotation `tuple[T, …]`; the encoder takes a sequence, the decoder returns `list` |
| `map<K, V>` | `list[tuple[K, V]]`, encoded in the caller's order |
| `@optional T` | `typing.Optional[T]`, default `None` |
| `enum` | `enum.IntEnum` |
| `union` | dataclass with discriminator `_d` |
| `struct`, inheritance | dataclass |

`string<N>`, `wstring<N>`, `sequence<T, N>` and `map<K, V, N>`: the
encoder rejects a value above the bound with
`zerodds_wire.OutOfRange` before it writes a byte; the decoder
applies `@try_construct` (XTypes 1.3 §7.2.2.4.4.4.9, default DISCARD).
The encoder does not yet check the element count of a fixed array (open,
`test_fixed_array_count_is_refused_on_encode`).

### §3.2 Example

```idl
module sensor_msgs { module msg {
  struct Temperature { long celsius; string sensor_id; };
}; };
```

```python
import zerodds
from temperature import ZD_TOPIC_DESCRIPTORS  # zerodds-idlc generate --lang python

p = zerodds.DomainParticipantFactory.instance().create_participant(0)
topic = zerodds.IdlTopic(p, "Temp", ZD_TOPIC_DESCRIPTORS["sensor_msgs::msg::Temperature"])
writer = topic.create_writer(p.create_publisher())
writer.write(topic.cls(celsius=23, sensor_id="A7"))
```

## §4 Test requirement

The tests are in `crates/py/python/tests/` (pytest); the fixtures in
`tests/_generated/` are IDL files and their unchanged
`zerodds-idlc` output. The runtime has its own tests in
`runtime/python/tests/`.

```bash
cd crates/py
maturin develop --features extension-module
PYTHONPATH=../../runtime/python:python pytest python/tests/
```

On the Rust side the crate has **no** test inventory — without
`--features extension-module`, `crates/py/` is a placeholder lib
without an API. Spec compliance is verified on the pytest side.

## §5 Multi-process / cross-vendor

| Scenario | v1.0 status | Mechanism |
|---|---|---|
| Single-process Python ↔ Python | ✅ done | via `zerodds._core` and `crates/dcps` |
| Multi-process Python ↔ Python (local, same domain) | ✅ done | `crates/dcps` spawns SPDP/SEDP endpoints, RTPS over UDP |
| Cross-vendor Python ↔ C++/Rust/Java/C# (ShapeType) | ✅ done | ShapeType wire byte-identical, same topic type name `org::omg::dds::demo::ShapeType` |
| Cross-vendor Python ↔ Cyclone/Fast-DDS ShapesDemo | ✅ done | same XCDR2-LE convention + same type name + RTPS 2.5 wire |
| Generated type via `IdlTopic` ↔ Rust codegen | ✅ done | bytes measured against the Rust reference (`wireprobe-python-matrix`) |
| AsyncIO API | ✅ done | `zerodds.aio` wrapper (see §6.3) |
| Complete QoS surface (22 policies) | ✅ done | `DataWriterQos` / `DataReaderQos` (see §6.2) |
| ReadCondition / QueryCondition | ✅ done | see §6.6 |
| Status listener callbacks | ✅ done | see §6.5 |
| IDL topic with codegen loop | ✅ done | `IdlTopic[T]` (see §6.1) |

## §6 Extended surface (included in v1.0)

These opt-in items use code paths that stand in parallel to the
default QoS surface.

- **§6.1 `IdlTopic`**: `IdlTopic[T]` with `IdlWriter`/`IdlReader` over
  `DescriptorTopic`/`DescriptorWriter`/`DescriptorReader`, encoded with
  the generated code (§3). Implementation:
  `crates/py/python/zerodds/topic.py`.
- **§6.2 QoS builder**: PyClass `DataWriterQos` /
  `DataReaderQos` with setters for all 22 policies from DDS 1.4
  §2.2.3, plus `Publisher.create_*_writer_with_qos` /
  `Subscriber.create_*_reader_with_qos`. Implementation:
  `crates/py/src/qos.rs`.
- **§6.3 AsyncIO API**: `zerodds.aio.AsyncBytesWriter`,
  `AsyncBytesReader`, `AsyncShapeWriter`, `AsyncShapeReader`,
  `AsyncWaitSet`. Blocking calls run via the
  `asyncio.to_thread` bridge. Implementation:
  `crates/py/python/zerodds/aio.py`.
- **§6.4 ROS-2 pytest integration**: test tree
  `crates/py/python/tests/ros2/` with `conftest.py` (skip if
  `ROS_DISTRO + rclpy + RMW_IMPLEMENTATION=rmw_zerodds_shim`
  is missing) and `test_rmw_zerodds_interop.py` for standard rclpy
  publish/subscribe via the `rmw_zerodds_shim`.
- **§6.5 Status listener callbacks**: PyClasses
  `DataWriterListener` (3 slots) and `DataReaderListener` (6 slots),
  registered via `writer.set_listener(listener, mask)` /
  `reader.set_listener(listener, mask)`. Implementation:
  `crates/py/src/listener.rs`.
- **§6.6 ReadCondition / QueryCondition**: PyClasses `ReadCondition`
  and `QueryCondition` over the Rust conditions from
  `crates/dcps/src/condition.rs`. SampleState/ViewState/InstanceState
  bitmask constants as pure Python modules
  `zerodds.{sample_state,view_state,instance_state}`. The SQL92 filter
  is validated in the constructor. `WaitSet.attach_read_condition` /
  `attach_query_condition` for attaching. Implementation:
  `crates/py/src/conditions.rs`.
- **§6.7 sphinx doc path**: `crates/py/docs/conf.py` with autodoc/
  napoleon/viewcode/intersphinx, `autodoc_mock_imports` for an
  RTD build without maturin. `crates/py/docs/api.rst` lists the
  PyClasses and the modules `loader`, `topic`, `aio`, `sample_state`,
  `view_state`, `instance_state`.

## §7 Stability

Vendor spec, semver:

- v1.0 = the surface of this spec: PyO3 classes, `IdlTopic` over
  generated code, pure `ctypes` loader, AsyncIO, QoS, listeners,
  conditions.

Breaking changes require a major version bump.

## §8 License

Apache-2.0 (workspace default).

## §9 References

- OMG DDS 1.4 (formal/2015-04-10) §2.2.2 DCPS Module
- OMG DDS-XTypes 1.3 (formal/2019-02-01) §7.4 XCDR1/XCDR2, §7.2.2.4.4.4.9 `@try_construct`
- OMG DDS-PSM-Cxx 1.0 (formal/2013-12-04) §7.5 — language PSM
  conventions as a comparison reference
- ZeroDDS vendor spec `zerodds-c-api-1.0` (shared wire layer
  via libzerodds for the ctypes path)
- ZeroDDS vendor spec `zerodds-ffi-loader-1.0` §3.1 (loader
  convention for `loader.py`)
- ZeroDDS vendor spec `zerodds-java-omgdds-1.0` (parallel pure
  Java path as a comparison reference)
- PyO3 0.22 Documentation (https://pyo3.rs)
- PEP 384: Limited API (abi3-py38)
