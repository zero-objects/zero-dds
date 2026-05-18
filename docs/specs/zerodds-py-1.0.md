# `zerodds-py` v1.0 — Vendor-Spec

ZeroDDS Vendor-Spec. In `crates/py/` implementiert.

**Status:** Draft 2026-05-15.

## Motivation

OMG hat **keine** normative Python-PSM für DDS publiziert. Existierende
Python-Bindings (cyclonedds-python, rti-dds-python, fastdds-python)
gehen jeweils einen anderen Weg:

1. **ctypes-Wrapper** um eine C-Library (Eclipse Cyclone DDS Python).
2. **C-API über pybind11** mit autogenerierten Stubs (FastDDS Python).
3. **C-API über Cython** plus C-Side-Codegen für Topic-Types (RTI Connext-DDS Python).

Alle drei Vendoren teilen einen Pain-Point: der Topic-Type-Pfad
verlangt entweder einen separaten IDL-Compiler-Schritt (FastDDS,
Cyclone) oder ein proprietäres Datacenter-Tooling (RTI). Anwender,
die "nur ein paar Dataclasses publishen wollen", müssen erst eine
Toolchain installieren.

ZeroDDS wählt einen Hybrid-Pfad:

- **`crates/py/src/`** — PyO3-Bindings über `cdylib`, der Binding-
  Module `zerodds._core` liefert; das ist das Performance-Default für
  Anwender, die `maturin develop` ausführen können oder von PyPI
  installieren.
- **`crates/py/python/zerodds/`** — Pure-Python-Layer auf
  `zerodds._core`. Enthält drei eigenständige Module:
  - `cdr.py` — pure-Python XCDR2-Little-Endian-Encoder/Decoder.
  - `idl.py` — `@idl_struct`-Dataclass-Annotations, baut Encoder/Decoder
    aus `@dataclass`-Felder ohne Codegen-Schritt.
  - `loader.py` — pure-`ctypes`-Loader gegen `libzerodds.{so,dylib,dll}`
    aus `crates/zerodds-c-api`. Bedient den Distro-Package-Pfad
    (System-libzerodds installiert, kein Rust-Build).

Diese Vendor-Spec definiert die Python-Surface normativ. Sie ist
parallel zu `zerodds-c-api-1.0` und `zerodds-java-omgdds-1.0` zu
lesen — alle drei realisieren das gleiche DCPS-Modell unter den
jeweiligen Sprach-Idiomen.

## Ziele

- **DCPS-Surface:** Eine Python-API über DDS 1.4 §2.2.2 mit
  PyClass-Mapping je Entity (`DomainParticipantFactory`,
  `DomainParticipant`, `Publisher`, `Subscriber`, `Topic`, `DataWriter`,
  `DataReader`).
- **Codegen-frei für einfache Typen:** Anwender annotieren eine
  `@dataclass` mit `@idl_struct(typename=…)` und Field-Types
  (`Int32`, `String`, `Bytes`, `Sequence`, …). Der Encoder/Decoder
  entsteht zur Decorator-Zeit, keine externe Toolchain.
- **Byte-Identität zum Rust-Pfad:** der pure-Python-Codec in
  `cdr.py` produziert byte-genau dieselben XCDR2-LE-Bytes wie
  `crates/cdr/` (verifiziert durch `test_pyshape_byte_roundtrip`,
  `test_cdr_primitive_roundtrip`).
- **Zwei-Pfad-Architektur:** PyO3 (Performance, RTPS-native) +
  pure-`ctypes` (Distro-Package, Zero-Build). Beide Pfade sind
  unabhängig benutzbar; Anwender wählen pro Use-Case.
- **GIL-Release auf Blocking-Calls:** Alle Wait-Funktionen (`write`,
  `wait_for_data`, `wait_for_matched_*`, `WaitSet.wait`) geben den
  GIL frei (PyO3 `py.allow_threads`), damit Python-Threads konkurrent
  laufen können.
- **abi3-py38:** PyO3-Binding ist für die stabile Python-ABI 3.8+
  gebaut (`pyo3 = { features = ["abi3-py38"] }`). Ein einzelnes Wheel
  läuft auf CPython 3.8 … 3.13.

## Nicht-Ziele

- **OMG DDS-Python-PSM-Compliance**: existiert nicht. Diese Spec
  definiert die Mapping-Konvention selbst; ist gegen DDS 1.4 §2.2.2
  (Sprach-neutral) normalisiert.
- **OMG-DDS-Python-PSM-Compliance**: existiert nicht. Diese Spec
  definiert die Mapping-Konvention selbst.
- **Direkter RTPS-Wire in pure-Python**: pure-Python-Pfad geht über
  die ctypes-Loader-Brücke zur Rust-Library. Es gibt kein
  rein-Python-RTPS-Datagram in v1.0 oder v2.0.

## §1 Architektur

### §1.1 Module-Layout

```
crates/py/
  Cargo.toml           # crate-type = ["cdylib", "rlib"]
  pyproject.toml       # maturin-Build-Konfig
  build.rs             # version.h für extension-module
  src/
    lib.rs             # Modul-Root, Doc, kein Code
    ffi.rs             # PyO3-Bindings: 13 PyClasses
  python/
    zerodds/
      __init__.py      # re-exportiert _core + idl + cdr
      idl.py           # @idl_struct, Type-Markers
      cdr.py           # CdrWriter / CdrReader (XCDR2-LE)
      loader.py        # pure-ctypes-Loader gegen libzerodds.so
    tests/
      test_smoke.py    # 7 Smoke-Tests
      test_idl.py      # 21 IDL/CDR-Tests
  examples/
    01_bytes_pubsub.py
    02_shape_pubsub.py
    03_idl_struct_cdr.py
  docs/                # sphinx
    conf.py, index.rst, api.rst, quickstart.rst, examples.rst
```

### §1.2 PyO3-Modul `zerodds._core`

Der Rust-Crate wird mit `--features extension-module` als `cdylib`
gebaut und liefert das Modul `zerodds_py` (von Python aus als
`zerodds._core` importiert). Es exportiert 13 PyClasses und die
Konstante `__version__`:

| PyClass | Ort | Mapped DCPS-Type |
|---|---|---|
| `DomainParticipantFactory` | `ffi.rs:53` | `DomainParticipantFactory` (DDS 1.4 §2.2.2.2.1) |
| `DomainParticipant` | `ffi.rs:97` | `DomainParticipant` (§2.2.2.2.2) |
| `BytesTopic` | `ffi.rs:219` | `Topic<RawBytes>` (§2.2.2.3.1) |
| `ShapeTopic` | `ffi.rs:236` | `Topic<ShapeType>` (Vendor-Interop-Type) |
| `Publisher` | `ffi.rs:257` | `Publisher` (§2.2.2.4.1) |
| `Subscriber` | `ffi.rs:281` | `Subscriber` (§2.2.2.5.1) |
| `BytesWriter` | `ffi.rs:309` | `DataWriter<RawBytes>` (§2.2.2.4.2) |
| `BytesReader` | `ffi.rs:357` | `DataReader<RawBytes>` (§2.2.2.5.3) |
| `Shape` | `ffi.rs:420` | ShapeType-Dataclass (Cross-Vendor-Interop) |
| `ShapeWriter` | `ffi.rs:475` | `DataWriter<ShapeType>` |
| `ShapeReader` | `ffi.rs:539` | `DataReader<ShapeType>` |
| `GuardCondition` | `ffi.rs:585` | `GuardCondition` (§2.2.2.6.3) |
| `WaitSet` | `ffi.rs:611` | `WaitSet` (§2.2.2.6.1) |

Alle PyClass-Methoden konvertieren `DdsError` über
`dds_err_to_py` (Zeile 41) in `RuntimeError`. Blocking-Calls geben
den GIL via `py.allow_threads` frei.

### §1.3 Python-Wrapper `python/zerodds/`

Das äussere Package `zerodds` re-exportiert `_core` plus drei
pure-Python-Module (`__init__.py:1-32`):

```python
from . import cdr, idl
from ._core import (
    DomainParticipantFactory, DomainParticipant,
    BytesTopic, ShapeTopic, Publisher, Subscriber,
    BytesWriter, BytesReader, Shape, ShapeWriter, ShapeReader,
    GuardCondition, WaitSet,
    __version__,
)
```

Die drei Python-Module fügen Funktionalität hinzu, die nicht über
PyO3 läuft:

- `cdr.py` — pure-Python `CdrWriter` / `CdrReader`. Alignment-Rules,
  null-terminated strings, length-prefixed sequences. Byte-genau
  zu `crates/cdr/src/buffer.rs`.
- `idl.py` — `@idl_struct(typename=…)` decorator. Liest Field-Types
  aus `@dataclass`-Annotations und baut zur Decorator-Zeit
  Encoder/Decoder mit `CdrWriter`/`CdrReader`. Unterstützte Types:
  `Bool`, `Int8…64`, `UInt8…64`, `Float32/64`, `String`, `Bytes`,
  `Sequence[T]`, `Array[T, N]`, `Optional[T]`, `idl_enum`,
  `idl_union`, nested `idl_struct`.
- `loader.py` — `Runtime`, `Writer`, `Reader` als pure-`ctypes`-
  Loader gegen `libzerodds.{so,dylib,dll}`. Folgt
  `zerodds-ffi-loader-1.0` §3.1. Header-Signaturen kommen aus
  `crates/zerodds-c-api/include/zerodds.h`.

### §1.4 Zwei-Pfad-Wahl

Anwender wählen pro Anwendung einen Pfad:

| Pfad | Wann | Build-Aufwand |
|---|---|---|
| PyO3 (`zerodds.DomainParticipantFactory.instance()`) | Performance, Topic-Type-API, `maturin`-fähige Umgebung | `pip install zerodds` (Wheel von PyPI) oder `maturin develop --features extension-module` |
| `ctypes` (`zerodds.loader.Runtime.create_participant()`) | Distro-Package, kein Rust-Toolchain im Build, embedded-Python | nur `libzerodds.so` muss installiert sein |

Die zwei Pfade teilen sich nichts ausser dem Wire-Format (XCDR2-LE)
und der Domain-ID. Sie können in der gleichen Process koexistieren,
sind aber nicht durch Cross-Calls verbunden — das ist Phase-2.

### §1.5 Schichten-Position

Layer 6 — PSMs / Bindings.

Direkte Abhängigkeiten: `zerodds-dcps` (`crates/dcps/`,
Layer 4 Core Services) für die PyO3-Bindings;
`zerodds-c-api` (`crates/zerodds-c-api/`, Layer 6 C-FFI)
für den ctypes-Loader-Pfad.

## §2 OMG-API-Coverage

Mapping DDS 1.4 §2.2.2 → Python-Pendant.

### §2.1 DomainParticipantFactory (DDS 1.4 §2.2.2.2.1)

| Spec-Operation | Python-Äquivalent |
|---|---|
| `get_instance()` | `DomainParticipantFactory.instance()` (Singleton, classmethod) |
| `create_participant(domain_id, qos, listener, mask)` | `factory.create_participant(domain_id)` — QoS=Default, Listener=None |
| `create_participant_offline(domain_id)` | `factory.create_participant_offline(domain_id)` — kein Discovery, für Tests |
| `delete_participant(p)` | implizit über Python-GC; `__del__` deleted die Rust-Side |

### §2.2 DomainParticipant (DDS 1.4 §2.2.2.2.2)

| Spec | Python |
|---|---|
| `get_domain_id()` | `p.domain_id` (Property) |
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
| `wait_for_acknowledgments(timeout)` | implizit über `wait_for_matched_subscription` + ACK-Loop in `crates/dcps` |
| `get_matched_subscriptions()` | `writer.matched_subscription_count() -> int` |
| `get_publication_matched_status()` | `writer.publication_matched_status() -> (total_count, total_count_change, current_count, current_count_change, last_subscription_handle)` |
| `get_liveliness_lost_status()` | `writer.liveliness_lost_status() -> (total_count, total_count_change)` |
| `get_offered_deadline_missed_status()` | `writer.offered_deadline_missed_status() -> (total_count, total_count_change)` |

Zusätzlich: `wait_for_matched_subscription(count, timeout_secs)` als
sync-helper.

### §2.5 DataReader (DDS 1.4 §2.2.2.5.3)

| Spec | Python (`BytesReader` / `ShapeReader`) |
|---|---|
| `take(max_samples, sample_states, view_states, instance_states)` | `reader.take() -> list[bytes]` / `list[Shape]` |
| `read(...)` | bewusst weggelassen in v1.0; `take` ist der Default |
| `wait_for_historical_data(timeout)` | bewusst weggelassen; im RTPS-Pfad nicht benötigt |
| `get_matched_publications()` | `reader.matched_publication_count() -> int` |
| `get_subscription_matched_status()` | `reader.subscription_matched_status() -> tuple` |
| `get_sample_lost_status()` | `reader.sample_lost_status() -> (total_count, total_count_change)` |
| `get_requested_deadline_missed_status()` | `reader.requested_deadline_missed_status() -> tuple` |

Zusätzlich: `wait_for_data(timeout_secs)` und
`wait_for_matched_publication(count, timeout_secs)`.

### §2.6 WaitSet / Conditions (DDS 1.4 §2.2.2.6)

| Spec | Python |
|---|---|
| `WaitSet()` | `zerodds._core.WaitSet()` |
| `WaitSet.attach_condition(cond)` | `ws.attach_guard_condition(gc)` (v1.0: nur GuardCondition) |
| `WaitSet.wait(active_conditions, timeout)` | `ws.wait(timeout_secs: float) -> int` (Anzahl getriggert; wirft `TimeoutError` bei Timeout) |
| `GuardCondition()` | `zerodds._core.GuardCondition()` |
| `GuardCondition.set_trigger_value(v)` | `gc.set_trigger_value(v: bool)` |
| `GuardCondition.get_trigger_value()` | `gc.get_trigger_value() -> bool` |

`GuardCondition` und `WaitSet` sind in v1.0 nur über
`zerodds._core` direkt erreichbar; das aeussere `zerodds`-Namespace
(`__init__.py`) re-exportiert sie absichtlich noch nicht, weil die
WaitSet-Surface in v1.1 zusammen mit `ReadCondition`/`QueryCondition`
fertig wird (siehe §6 Phase-2-Plan). `ReadCondition` und
`QueryCondition` sind Phase-2.

### §2.7 ShapeType — Cross-Vendor-Interop-Type

`Shape` (PyClass) und `ShapeType` (`crates/dcps/src/interop.rs`) sind
byte-identisch. Felder: `color: str`, `x: i32`, `y: i32`,
`shapesize: i32` (= 30 default). Type-Name auf dem Wire: `"ShapeType"`
(`crates/dcps/src/interop.rs:91`: `TYPE_NAME: &'static str = "ShapeType"`),
kompatibel zu Cyclone-/Fast-DDS-ShapesDemo, deren Default-Topic-Type
ebenfalls `ShapeType` (ohne Modul-Prefix) ist.

## §3 IDL-Mapping — `@idl_struct` + XCDR2-LE-Codec

OMG XTypes 1.3 §7.4 XCDR2-Little-Endian. Implementiert in
`python/zerodds/cdr.py` und `python/zerodds/idl.py`.

### §3.1 `CdrWriter` / `CdrReader` (`cdr.py`)

Pure-Python XCDR2-LE-Codec mit Alignment-Rules (1/2/4/8 natural).

| Operation | Wire-Format |
|---|---|
| `write_bool(v) / read_bool()` | 1 Byte, 0/1 |
| `write_iN(v) / read_iN()` für `N ∈ {8, 16, 32, 64}` | signed Little-Endian, N/8 Bytes, padded auf N/8-Alignment |
| `write_uN(v) / read_uN()` für `N ∈ {8, 16, 32, 64}` | unsigned Little-Endian |
| `write_f32 / write_f64` | IEEE-754, Little-Endian, padded |
| `write_string(s) / read_string()` | u32 length-prefix (inkl. \0) + UTF-8 + \0-terminator + Alignment-Padding |
| `write_bytes(b) / read_bytes()` | u32 length-prefix + raw bytes (= `sequence<octet>`) |

`CdrReader` rejected truncated input mit `ValueError` (test:
`test_cdr_reader_rejects_truncated_string`).

### §3.2 `@idl_struct(typename=…)` (`idl.py`)

Decorator über `@dataclass`. Liest Annotations zur Decorator-Zeit,
generiert eine `_encode(value, writer)`-Methode und eine
`_decode(reader) -> instance`-Methode.

Unterstützte Field-Types (alle in v1.0):

| Annotation | XCDR2 |
|---|---|
| `Bool, Int8/16/32/64, UInt8/16/32/64, Float32/64, String, Bytes` | siehe §3.1 |
| `bool, int, float, str, bytes` (Python-Primitives) | automatisch auf XCDR2-Default-Width gemapped (test: `test_auto_map_python_primitives`) |
| `Sequence[T]` | u32 length-prefix + N×encode(T) |
| `Array[T, N]` | N×encode(T) ohne Length-Prefix; Wrong-Count beim Encode rejected |
| `Optional[T]` | bool-flag + (T-encode wenn True) |
| `@idl_enum` über `IntEnum` | u32-Wert; unbekannter Wert beim Decode → `ValueError` |
| `@idl_union` mit `disc: IntEnum`, `cases: dict[int, T]`, optional default | disc + branch-encode; unknown disc → default oder ValueError |
| Nested `@idl_struct` | rekursiv |

Type-Name auf dem Wire kommt aus `typename=…`-Argument am Decorator.
Wird im RTPS-Discovery-Pfad als Topic-Type-Name verglichen
(Cross-Vendor-Match).

### §3.3 Codegen-Free-Pfad

Ein Anwender, der eine neue Message publishen will, schreibt:

```python
from dataclasses import dataclass
from zerodds.idl import idl_struct, Int32, String

@idl_struct(typename="sensor_msgs::msg::Temperature")
@dataclass
class Temperature:
    celsius: Int32
    sensor_id: String

# nutzbar direkt: writer.write_idl(Temperature(23, "A7"))
```

Keine externe IDL-Datei, kein Build-Schritt, kein generierter
Source-Tree. Byte-identisch zum Rust-Pfad, der die gleiche
ShapeType-`@dataclass` über `dds_dcps::interop::ShapeType` codiert.

## §4 Test-Pflicht

| Test-File | Anzahl | Schwerpunkt |
|---|---|---|
| `crates/py/python/tests/test_smoke.py` | 7 | PyClass-Smoke: Factory-Singleton, Topic, Reader/Writer-Offline, Shape-Defaults, Live-Roundtrip |
| `crates/py/python/tests/test_idl.py` | 21 | CDR-Primitive-Roundtrip, ShapeType-Byte-Identität, `@idl_struct`-API für alle 9 Annotation-Klassen (Primitives, Sequence, Array, Optional, Enum, Union, Nested) |

Test-Lauf:

```bash
cd crates/py
maturin develop --features extension-module
pytest crates/py/python/tests/
```

Total: **28 Python-Tests**.

Rust-seitig hat das Crate **kein** Test-Inventar — ohne
`--features extension-module` ist `crates/py/` ein Platzhalter-Lib
ohne API. Die Spec-Compliance wird ausschliesslich pytest-seitig
verifiziert.

## §5 Multi-Process / Cross-Vendor

| Szenario | RC1-Status | Mechanismus |
|---|---|---|
| Single-Process Python ↔ Python | ✅ done | über `zerodds._core` und `crates/dcps` |
| Multi-Process Python ↔ Python (lokal, gleiches Domain) | ✅ done | `crates/dcps` spawnt SPDP/SEDP-Endpoints, RTPS über UDP |
| Cross-Vendor Python ↔ C++/Rust/Java/C# (ShapeType) | ✅ done | ShapeType-Wire byte-identisch, gleiche Topic-Type-Name `org::omg::dds::demo::ShapeType` |
| Cross-Vendor Python ↔ Cyclone-/Fast-DDS-ShapesDemo | ✅ done | gleiche XCDR2-LE-Konvention + gleicher Type-Name + RTPS-2.5-Wire |
| `@idl_struct`-Custom-Type über `BytesTopic` ↔ Rust-Codegen | ✅ done | byte-identisch verifiziert in `test_pyshape_byte_roundtrip` und `test_sensor_mixed_fields_roundtrip` |
| AsyncIO-API | ✅ done | `zerodds.aio` Wrapper (siehe §6.3) |
| Vollständige QoS-Surface (22 Policies) | ✅ done | `DataWriterQos` / `DataReaderQos` (siehe §6.2) |
| ReadCondition / QueryCondition | ✅ done | siehe §6.6 |
| Status-Listener-Callbacks | ✅ done | siehe §6.5 |
| IDL-Topic mit Codegen-Loop | ✅ done | `IdlTopic[T]` (siehe §6.1) |

## §6 Erweiterte Surface (alle in v1.0 enthalten)

Diese Punkte waren ursprünglich als Phase-2 geplant und sind im
Verlauf des Audits in v1.0 gezogen worden. Sie nutzen Code-Pfade, die
parallel zur Default-QoS-Surface stehen, und sind opt-in.

- **§6.1 `IdlTopic` mit Codegen-Loop**: `IdlTopic[T]` mit
  `IdlWriter`/`IdlReader` (pure-Python-Wrapper ueber
  `BytesTopic`/`BytesWriter`/`BytesReader`, die den
  `@idl_struct`-Encoder pro Sample ausfuehren). Implementation:
  `crates/py/python/zerodds/topic.py`.
- **§6.2 QoS-Builder**: PyClass `DataWriterQos` /
  `DataReaderQos` mit Settern fuer alle 22 Policies aus DDS 1.4
  §2.2.3, plus `Publisher.create_*_writer_with_qos` /
  `Subscriber.create_*_reader_with_qos`. Implementation:
  `crates/py/src/qos.rs`.
- **§6.3 AsyncIO-API**: `zerodds.aio.AsyncBytesWriter`,
  `AsyncBytesReader`, `AsyncShapeWriter`, `AsyncShapeReader`,
  `AsyncWaitSet`. Blocking-Calls laufen ueber
  `asyncio.to_thread`-Brueke. Implementation:
  `crates/py/python/zerodds/aio.py`.
- **§6.4 ROS-2-pytest-Integration**: Test-Tree
  `crates/py/python/tests/ros2/` mit `conftest.py` (Skip wenn
  `ROS_DISTRO + rclpy + RMW_IMPLEMENTATION=rmw_zerodds_shim`
  fehlt) und `test_rmw_zerodds_interop.py` fuer Standard-rclpy
  Publish/Subscribe ueber das `rmw_zerodds_shim`.
- **§6.5 Status-Listener-Callbacks**: PyClasses
  `DataWriterListener` (3 Slots) und `DataReaderListener` (6 Slots),
  registriert ueber `writer.set_listener(listener, mask)` /
  `reader.set_listener(listener, mask)`. Implementation:
  `crates/py/src/listener.rs`.
- **§6.6 ReadCondition / QueryCondition**: PyClasses `ReadCondition`
  und `QueryCondition` ueber die Rust-Conditions aus
  `crates/dcps/src/condition.rs`. SampleState/ViewState/InstanceState-
  Bitmask-Konstanten als pure-Python-Module
  `zerodds.{sample_state,view_state,instance_state}`. SQL92-Filter
  wird im Konstruktor validiert. `WaitSet.attach_read_condition` /
  `attach_query_condition` zum Anhaengen. Implementation:
  `crates/py/src/conditions.rs`.
- **§6.7 sphinx-Doc-Pfad**: `crates/py/docs/conf.py` mit autodoc/
  napoleon/viewcode/intersphinx, `autodoc_mock_imports` fuer
  RTD-Build-ohne-maturin. `crates/py/docs/api.rst` listet alle
  13 PyClasses + 3 pure-Python-Module (cdr, idl, loader).

## §7 Stabilität

Vendor-Spec, semver:

- v1.0 = aktuelle Surface (PyO3-13-PyClasses + `@idl_struct` +
  pure-`ctypes`-Loader). RC1-Surface.
- v1.1 = + AsyncIO + QoS-Surface + Listener-Callbacks.
- v1.2 = + ReadCondition / QueryCondition + sphinx-Docs.
- v2.0 = + `create_idl_topic` mit Codegen-Loop.

Breaking-Changes erfordern Major-Version-Bump. Default-Annotations
(`Int32`, `String`, etc.) sind ab v1.0 stabil — werden nicht
umbenannt.

## §8 Lizenz

Apache-2.0 (Workspace-Default).

## §9 Referenzen

- OMG DDS 1.4 (formal/2015-04-10) §2.2.2 DCPS Module
- OMG DDS-XTypes 1.3 (formal/2019-02-01) §7.4 XCDR2
- OMG DDS-PSM-Cxx 1.0 (formal/2013-12-04) §7.5 — Sprach-PSM-
  Konventionen als Vergleichs-Referenz
- ZeroDDS Vendor-Spec `zerodds-c-api-1.0` (gemeinsame Wire-Layer
  über libzerodds für den ctypes-Pfad)
- ZeroDDS Vendor-Spec `zerodds-ffi-loader-1.0` §3.1 (Loader-
  Konvention für `loader.py`)
- ZeroDDS Vendor-Spec `zerodds-java-omgdds-1.0` (paralleler Pure-
  Java-Pfad als Vergleichs-Referenz)
- PyO3 0.22 Documentation (https://pyo3.rs)
- PEP 384: Limited API (abi3-py38)
