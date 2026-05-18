# `zerodds-py` v1.0 — Spec-Coverage

Audit der Vendor-Spec `docs/specs/zerodds-py-1.0.md` gegen
`crates/py/` Code-Realität.

**Source:** `docs/specs/zerodds-py-1.0.md` (Vendor-Spec, Draft 2026-05-15).
**Repo:** `crates/py/` (Rust-PyO3-Bridge + Python-Wrapper).

---

## §1 Architektur

### §1.1 Module-Layout

**Spec:** §1.1 — Crate-Layout mit `Cargo.toml` (crate-type `cdylib`+`rlib`),
`pyproject.toml` für maturin, `src/lib.rs`+`ffi.rs`,
`python/zerodds/{__init__,idl,cdr,loader}.py`, `python/tests/`,
`examples/`, `docs/` sphinx.

**Repo:** `crates/py/Cargo.toml` (`crate-type = ["cdylib", "rlib"]`),
`crates/py/pyproject.toml`, `crates/py/src/lib.rs`,
`crates/py/src/ffi.rs`, `crates/py/python/zerodds/__init__.py`,
`crates/py/python/zerodds/idl.py`,
`crates/py/python/zerodds/cdr.py`,
`crates/py/python/zerodds/loader.py`,
`crates/py/python/tests/test_smoke.py`,
`crates/py/python/tests/test_idl.py`,
`crates/py/examples/{01_bytes_pubsub,02_shape_pubsub,03_idl_struct_cdr}.py`,
`crates/py/docs/{conf,index,api,quickstart,examples}.{py,rst}`.

**Tests:** Layout durch Pytest-Discovery selbst verifiziert
(`pytest crates/py/python/tests/`).

**Status:** done

### §1.2 PyO3-Modul `zerodds._core` — 13 PyClasses

Pro PyClass ein Item. Mapping aus DDS 1.4 §2.2.2.

#### §1.2.1 `DomainParticipantFactory` (PyClass)

**Spec:** §1.2 + §2.1 — PyClass `DomainParticipantFactory` mit
`instance()`, `create_participant(domain_id)`,
`create_participant_offline(domain_id)`,
`create_participant_fast(domain_id)`.

**Repo:** `crates/py/src/ffi.rs:53` (`#[pyclass(name = "DomainParticipantFactory")]`),
Methoden in `ffi.rs:60-95`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_factory_singleton_and_offline_participant`,
`test_pub_sub_roundtrip_live`.

**Status:** done

#### §1.2.2 `DomainParticipant` (PyClass)

**Spec:** §1.2 + §2.2 — PyClass `DomainParticipant` mit `domain_id`,
`topics_len`, `discovered_participants_count`,
`create_{bytes,shape}_topic`, `create_{publisher,subscriber}`,
`assert_liveliness`, `ignore_{participant,topic,publication,subscription}`,
`contains_entity`, `get_discovered_{topics,participants}`.

**Repo:** `crates/py/src/ffi.rs:97` (`#[pyclass(name = "DomainParticipant")]`),
Methoden in `ffi.rs:105-215`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_factory_singleton_and_offline_participant`,
`test_bytes_topic_and_reader_are_creatable_offline`,
`test_pub_sub_roundtrip_live`.

**Status:** done

#### §1.2.3 `BytesTopic` (PyClass)

**Spec:** §1.2 — PyClass `BytesTopic` mit `name`, `type_name`.

**Repo:** `crates/py/src/ffi.rs:219` (`#[pyclass(name = "BytesTopic")]`),
Methoden in `ffi.rs:224-234`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_bytes_topic_and_reader_are_creatable_offline`.

**Status:** done

#### §1.2.4 `ShapeTopic` (PyClass)

**Spec:** §1.2 + §2.7 — PyClass `ShapeTopic` mit `name`, `type_name`,
Type-Name = `"ShapeType"`.

**Repo:** `crates/py/src/ffi.rs:236` (`#[pyclass(name = "ShapeTopic")]`),
Methoden in `ffi.rs:241-255`. Type-Name aus
`crates/dcps/src/interop.rs:91` (`TYPE_NAME: &'static str = "ShapeType"`).

**Tests:** `crates/py/python/tests/test_smoke.py::test_shape_topic_matches_vendor_interop_type_name`.

**Status:** done

#### §1.2.5 `Publisher` (PyClass)

**Spec:** §1.2 + §2.3 — PyClass `Publisher` mit
`create_bytes_writer(topic)`, `create_shape_writer(topic)`.

**Repo:** `crates/py/src/ffi.rs:257` (`#[pyclass(name = "Publisher")]`),
Methoden in `ffi.rs:262-279`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`.

**Status:** done

#### §1.2.6 `Subscriber` (PyClass)

**Spec:** §1.2 + §2.3 — PyClass `Subscriber` mit
`create_bytes_reader(topic)`, `create_shape_reader(topic)`.

**Repo:** `crates/py/src/ffi.rs:281` (`#[pyclass(name = "Subscriber")]`),
Methoden in `ffi.rs:286-303`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_bytes_topic_and_reader_are_creatable_offline`,
`test_pub_sub_roundtrip_live`.

**Status:** done

#### §1.2.7 `BytesWriter` (PyClass)

**Spec:** §1.2 + §2.4 — PyClass `BytesWriter` mit `write`,
`wait_for_matched_subscription`, `matched_subscription_count`,
`publication_matched_status`, `liveliness_lost_status`,
`offered_deadline_missed_status`.

**Repo:** `crates/py/src/ffi.rs:309` (`#[pyclass(name = "BytesWriter")]`),
Methoden in `ffi.rs:314-355`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`.

**Status:** done

#### §1.2.8 `BytesReader` (PyClass)

**Spec:** §1.2 + §2.5 — PyClass `BytesReader` mit `take`,
`wait_for_data`, `wait_for_matched_publication`,
`matched_publication_count`, `subscription_matched_status`,
`sample_lost_status`, `requested_deadline_missed_status`.

**Repo:** `crates/py/src/ffi.rs:357` (`#[pyclass(name = "BytesReader")]`),
Methoden in `ffi.rs:362-418`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_bytes_topic_and_reader_are_creatable_offline`,
`test_pub_sub_roundtrip_live`.

**Status:** done

#### §1.2.9 `Shape` (PyClass / Dataclass)

**Spec:** §1.2 + §2.7 — PyClass `Shape` mit Feldern `color`, `x`, `y`,
`shapesize` (default 30), `__repr__`, Konvertierungen `From<&PyShape>`
und `From<ShapeType>` in beide Richtungen.

**Repo:** `crates/py/src/ffi.rs:420` (`#[pyclass(name = "Shape")]`),
Methoden in `ffi.rs:437-468`. Rust-Pendant in
`crates/dcps/src/interop.rs:61` (`pub struct ShapeType`).

**Tests:** `crates/py/python/tests/test_smoke.py::test_shape_constructor_and_repr`,
`test_shape_default_shapesize_is_30`,
`crates/py/python/tests/test_idl.py::test_pyshape_byte_roundtrip`,
`test_pyshape_type_name_set_by_decorator`.

**Status:** done

#### §1.2.10 `ShapeWriter` (PyClass)

**Spec:** §1.2 + §2.4 — PyClass `ShapeWriter` mit `write(shape)`,
`register_instance(shape)`, `dispose(shape)`,
`unregister_instance(shape)`, `wait_for_matched_subscription`.

**Repo:** `crates/py/src/ffi.rs:475` (`#[pyclass(name = "ShapeWriter")]`),
Methoden in `ffi.rs:480-537`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`.

**Status:** done

#### §1.2.11 `ShapeReader` (PyClass)

**Spec:** §1.2 + §2.5 — PyClass `ShapeReader` mit `take`,
`wait_for_data`, `wait_for_matched_publication`.

**Repo:** `crates/py/src/ffi.rs:539` (`#[pyclass(name = "ShapeReader")]`),
Methoden in `ffi.rs:544-583`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`.

**Status:** done

#### §1.2.12 `GuardCondition` (PyClass)

**Spec:** §1.2 + §2.6 — PyClass `GuardCondition` mit Konstruktor,
`set_trigger_value(bool)`, `get_trigger_value() -> bool`.

**Repo:** `crates/py/src/ffi.rs:585` (`#[pyclass(name = "GuardCondition")]`),
Methoden in `ffi.rs:590-609`. Modul-Registration:
`crates/py/src/ffi.rs:660` (`m.add_class::<PyGuardCondition>()`).

**Tests:** `crates/py/python/tests/test_conditions.py::test_guard_condition_default_trigger_is_false`,
`test_guard_condition_set_trigger_roundtrip`,
`test_guard_condition_via_outer_namespace`. `zerodds.GuardCondition`
ist im aeusseren Namespace re-exportiert
(`crates/py/python/zerodds/__init__.py:50` + `__all__`).

**Status:** done

#### §1.2.13 `WaitSet` (PyClass)

**Spec:** §1.2 + §2.6 — PyClass `WaitSet` mit Konstruktor,
`attach_guard_condition(gc)`, `wait(timeout_secs) -> int`.

**Repo:** `crates/py/src/ffi.rs:611` (`#[pyclass(name = "WaitSet")]`),
Methoden in `ffi.rs:617-642`. Modul-Registration:
`crates/py/src/ffi.rs:661` (`m.add_class::<PyWaitSet>()`).

**Tests:** `crates/py/python/tests/test_conditions.py::test_waitset_attach_and_wait_raises_timeout`,
`test_waitset_wakes_when_guard_condition_fires`,
`test_waitset_via_outer_namespace`. `zerodds.WaitSet` ist im
aeusseren Namespace re-exportiert.

**Status:** done

### §1.3 Python-Wrapper `python/zerodds/`

#### §1.3.1 `__init__.py` — Re-Export

**Spec:** §1.3 — `zerodds.__init__` re-exportiert `_core`-PyClasses
plus die pure-Python-Module `cdr` und `idl`.

**Repo:** `crates/py/python/zerodds/__init__.py` re-exportiert das
volle `_core`-Surface — alle 13 originalen PyClasses + die 6
neuen aus §6 (`DataWriterQos, DataReaderQos, DataWriterListener,
DataReaderListener, ReadCondition, QueryCondition`) + die drei
Bitmask-Module (`sample_state`, `view_state`, `instance_state`).
Plus IDL-Type-Markers aus `.idl` und die §6.1 `IdlTopic`/`IdlWriter`/
`IdlReader`-Wrapper aus `.topic`. Fallback bei fehlendem `_core`:
`_CoreStub` mit klarem `ImportError`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_version_exposed`,
`test_conditions.py::test_guard_condition_via_outer_namespace`,
`test_conditions.py::test_waitset_via_outer_namespace` —
verifizieren explizit, dass beide via `zerodds.<Name>` erreichbar
sind.

**Status:** done

#### §1.3.2 `cdr.py` — XCDR2-LE-Codec

**Spec:** §1.3 + §3.1 — `CdrWriter` / `CdrReader` mit Alignment-Rules,
Primitive (`bool`, `i8..64`, `u8..64`, `f32/64`), `string` (length-prefix
+ null-terminator + padding), `bytes` (length-prefix + raw). Byte-genau
zu `crates/cdr/src/buffer.rs`.

**Repo:** `crates/py/python/zerodds/cdr.py` (`CdrWriter`, `CdrReader`).

**Tests:** `crates/py/python/tests/test_idl.py::test_cdr_primitive_roundtrip`,
`test_cdr_string_alignment_padding`,
`test_cdr_reader_rejects_truncated_string`.

**Status:** done

#### §1.3.3 `idl.py` — `@idl_struct` Decorator

**Spec:** §1.3 + §3.2 — `@idl_struct(typename=…)`-Decorator,
Field-Type-Markers (`Bool`, `Int8..64`, `UInt8..64`, `Float32/64`,
`String`, `Bytes`, `Sequence[T]`, `Array[T, N]`, `Optional[T]`,
`idl_enum`, `idl_union`, nested `@idl_struct`), Auto-Mapping von
Python-Primitives.

**Repo:** `crates/py/python/zerodds/idl.py` mit `_IdlKind`-Class und
Type-Konstanten (Bool, Int8..64, UInt8..64, Float32/64, String,
Bytes), `_IdlSequence`, `_IdlArray`, `_IdlOptional`, `_IdlEnum`,
`_IdlUnion`, `_IdlStruct`, `idl_struct()`, `idl_union()`,
`is_idl_struct()`, `type_name_of()`.

**Tests:** `crates/py/python/tests/test_idl.py::test_idl_struct_requires_dataclass`,
`test_pyshape_byte_roundtrip`, `test_pyshape_type_name_set_by_decorator`,
`test_sensor_mixed_fields_roundtrip`, `test_auto_map_python_primitives`,
`test_nested_struct_roundtrip`, `test_sequence_of_primitives_roundtrip`,
`test_sequence_of_structs_roundtrip`, `test_array_fixed_count_roundtrip`,
`test_array_wrong_count_rejected`, `test_optional_present_and_absent`,
`test_enum_roundtrip`, `test_enum_unknown_value_raises`,
`test_union_case_int_roundtrip`, `test_union_case_string_roundtrip`,
`test_union_default_branch_used_for_unknown_disc`,
`test_union_without_default_rejects_unknown_disc`,
`test_idl_struct_resolves_pep563_stringified_annotations`.

**Status:** done

#### §1.3.4 `loader.py` — pure-`ctypes`-Loader

**Spec:** §1.3 + §1.4 — Pure-`ctypes`-Loader gegen
`libzerodds.{so,dylib,dll}` aus `crates/zerodds-c-api/`. Folgt
`zerodds-ffi-loader-1.0` §3.1. Signaturen aus
`crates/zerodds-c-api/include/zerodds.h`.

**Repo:** `crates/py/python/zerodds/loader.py` (420 Zeilen) mit
`Runtime`, `Writer`, `Reader`. Konsumiert
`crates/zerodds-c-api/include/zerodds.h`.

**Tests:** `crates/py/python/tests/test_loader_smoke.py` mit
4 Tests (`test_loader_resolves_library`,
`test_loader_runtime_create_participant_offline`,
`test_loader_factory_singleton`,
`test_loader_zerodds_error_is_runtime_error_subclass`). Tests
skippen automatisch, wenn `libzerodds.{so,dylib,dll}` nicht
gefunden wird; vor einem CI-Lauf muss `cargo build -p zerodds-c-api`
gelaufen sein (oder `ZERODDS_LIB` gesetzt).

**Status:** done — Tests laufen grün auf
`llvm@llvm` (Debian 12, libzerodds.so aus
`~/zerodds/target/debug/libzerodds.so`, `ZERODDS_LIB`-ENV-Override).
Verifikation 2026-05-15. Auf Systemen ohne gebauten Artefakt skippen
die Tests sauber.

### §1.4 Zwei-Pfad-Wahl

**Spec:** §1.4 — Zwei unabhängige Pfade: PyO3 (Performance,
RTPS-native) und pure-`ctypes` (Distro-Package, Zero-Build).

**Repo:** PyO3-Pfad: `crates/py/src/ffi.rs` + `python/zerodds/__init__.py`.
ctypes-Pfad: `crates/py/python/zerodds/loader.py` +
`crates/zerodds-c-api/`.

**Tests:** PyO3 voll abgedeckt (alle PyClass-Tests aus §1.2/§2/§3/§6).
ctypes-Pfad via `test_loader_smoke.py` (Conditional-Skip; siehe §1.3.4).

**Status:** done — beide Pfade auf
`llvm@llvm` live verifiziert (PyO3 via maturin + ctypes via
libzerodds.so).

### §1.5 Schichten-Position

**Spec:** §1.5 — Layer 6 (PSMs / Bindings). Direkte Abhängigkeiten:
`zerodds-dcps` (Layer 4), `zerodds-c-api` (Layer 6 C-FFI).

**Repo:** `crates/py/Cargo.toml` (`zerodds-dcps = { path = "../dcps" }`).
ctypes-Pfad: keine Cargo-Dep, nur Runtime-Lookup auf
`libzerodds.{so,dylib,dll}`.

**Tests:** indirekt durch `cargo build -p zerodds-py` + alle Tests
(Workspace-Build).

**Status:** n/a (informative) — Architektur-Statement, nicht
test-pflichtig.

## §2 OMG-API-Coverage (DDS 1.4 §2.2.2)

### §2.1 DomainParticipantFactory (DDS 1.4 §2.2.2.2.1)

**Spec:** §2.1 — Mapping-Tabelle: `get_instance()`,
`create_participant`, `create_participant_offline`, implizites
`delete_participant` via Python-GC.

**Repo:** `crates/py/src/ffi.rs:60` (`fn instance() -> Self`),
`ffi.rs:65` (`create_participant_offline`), `ffi.rs:72`
(`create_participant`), `ffi.rs:80` (`create_participant_fast`).
Drop-Verhalten erbt von `Arc<DomainParticipant>` aus
`crates/dcps/src/factory.rs`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_factory_singleton_and_offline_participant`,
`test_pub_sub_roundtrip_live`.

**Status:** done

### §2.2 DomainParticipant (DDS 1.4 §2.2.2.2.2)

**Spec:** §2.2 — 12 Operations: `get_domain_id`, `create_topic`,
`create_publisher`, `create_subscriber`, `assert_liveliness`,
`ignore_{participant,topic,publication,subscription}`,
`contains_entity`, `get_discovered_{topics,participants}`.

**Repo:** `crates/py/src/ffi.rs:105-215` — alle 12 Operations als
PyMethods (`domain_id`, `topics_len`, `discovered_participants_count`,
`create_bytes_topic`, `create_shape_topic`, `create_publisher`,
`create_subscriber`, `assert_liveliness`, `ignore_*` (4 Varianten),
`contains_entity`, `get_discovered_topics`, `get_discovered_participants`).

**Tests:** Smoke-Tests aus `test_smoke.py` plus dedizierte Tests in
`crates/py/python/tests/test_participant_ignore.py` (9 Tests):
`test_ignore_participant_unknown_handle_is_silent`,
`test_ignore_topic_unknown_handle_is_silent`,
`test_ignore_publication_unknown_handle_is_silent`,
`test_ignore_subscription_unknown_handle_is_silent`,
`test_contains_entity_false_for_unknown_handle`,
`test_get_discovered_topics_offline_is_empty`,
`test_get_discovered_participants_offline_is_empty`,
`test_discovered_participants_count_offline_is_zero`,
`test_assert_liveliness_offline_does_not_raise`.

**Status:** done

### §2.3 Publisher / Subscriber (DDS 1.4 §2.2.2.4.1 / §2.2.2.5.1)

**Spec:** §2.3 — `Publisher.create_datawriter` →
`pub.create_{bytes,shape}_writer(topic)`,
`Subscriber.create_datareader` →
`sub.create_{bytes,shape}_reader(topic)`.

**Repo:** `crates/py/src/ffi.rs:264` (Publisher `create_bytes_writer`),
`ffi.rs:272` (`create_shape_writer`),
`ffi.rs:288` (Subscriber `create_bytes_reader`),
`ffi.rs:296` (`create_shape_reader`).

**Tests:** `crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`,
`test_bytes_topic_and_reader_are_creatable_offline`.

**Status:** done

### §2.4 DataWriter (DDS 1.4 §2.2.2.4.2)

**Spec:** §2.4 — Mapping-Tabelle: `write`, `register_instance`,
`unregister_instance`, `dispose`, `get_matched_subscriptions`,
`get_publication_matched_status`, `get_liveliness_lost_status`,
`get_offered_deadline_missed_status` plus
`wait_for_matched_subscription` sync-helper.

**Repo:** `crates/py/src/ffi.rs:317` (BytesWriter `write`), `ffi.rs:324`
(`wait_for_matched_subscription`), `ffi.rs:337-355` (Status-Getter
+ `matched_subscription_count`). `ShapeWriter` in `ffi.rs:482`
(`write`), `ffi.rs:492` (`register_instance`), `ffi.rs:502`
(`dispose`), `ffi.rs:515` (`unregister_instance`), `ffi.rs:525`
(`wait_for_matched_subscription`).

**Tests:** Smoke-Tests + dedizierte Lifecycle-Tests in
`crates/py/python/tests/test_writer_lifecycle.py` (6 Tests):
`test_register_instance_returns_nonzero_handle`,
`test_register_instance_is_idempotent_for_same_key`,
`test_register_different_keys_yield_different_handles`,
`test_dispose_does_not_raise`,
`test_unregister_instance_does_not_raise`,
`test_lifecycle_full_sequence`.

**Status:** done

### §2.5 DataReader (DDS 1.4 §2.2.2.5.3)

**Spec:** §2.5 — `take`, `wait_for_data`, `wait_for_matched_publication`,
`matched_publication_count`, `subscription_matched_status`,
`sample_lost_status`, `requested_deadline_missed_status`. `read`
und `wait_for_historical_data` bewusst weggelassen.

**Repo:** `crates/py/src/ffi.rs:365` (BytesReader `take`), `ffi.rs:375`
(`wait_for_data`), `ffi.rs:381` (`wait_for_matched_publication`),
`ffi.rs:394-418` (Status-Getter + `matched_publication_count`).
ShapeReader analog in `ffi.rs:546-583`.

**Tests:** Smoke-Tests + dedizierte Status-Getter-Tests in
`crates/py/python/tests/test_status_getters.py` (8 Tests):
`test_publication_matched_status_shape`,
`test_subscription_matched_status_shape`,
`test_liveliness_lost_status_shape`,
`test_offered_deadline_missed_status_shape`,
`test_sample_lost_status_shape`,
`test_requested_deadline_missed_status_shape`,
`test_matched_subscription_count_zero_offline`,
`test_matched_publication_count_zero_offline`.

**Status:** done

### §2.6 WaitSet / Conditions (DDS 1.4 §2.2.2.6)

**Spec:** §2.6 — `WaitSet`, `GuardCondition` mit
`attach_guard_condition`, `wait`, `set/get_trigger_value`.
`ReadCondition`/`QueryCondition` als Phase-2 angekündigt. `WaitSet`
+ `GuardCondition` in v1.0 nur über `zerodds._core` direkt
erreichbar, nicht im aeusseren Namespace.

**Repo:** `crates/py/src/ffi.rs:585-642`. Modul-Registration:
`ffi.rs:660-661`.

**Tests:** `crates/py/python/tests/test_conditions.py` (GuardCondition +
WaitSet, 6 Tests, siehe §1.2.12/§1.2.13) +
`crates/py/python/tests/test_read_conditions.py` (ReadCondition +
QueryCondition + Bitmask-Konstanten, 11 Tests, siehe §6.6).
`zerodds.GuardCondition`, `zerodds.WaitSet`, `zerodds.ReadCondition`,
`zerodds.QueryCondition` sind alle im aeusseren Namespace
re-exportiert.

**Status:** done

### §2.7 ShapeType — Cross-Vendor-Interop-Type

**Spec:** §2.7 — `Shape`-PyClass byte-identisch zu `ShapeType` in
`crates/dcps/src/interop.rs`. Felder `color: String, x: i32, y: i32,
shapesize: i32` (default 30). Type-Name auf dem Wire = `"ShapeType"`.

**Repo:** `crates/py/src/ffi.rs:420` (`#[pyclass(name = "Shape")]`),
Konstruktor `ffi.rs:441` (`fn new(color, x, y, shapesize)`),
Default-`shapesize=30` über `pyo3(signature)`,
Konvertierungen `ffi.rs:459` (`From<&PyShape> for ShapeType`),
`ffi.rs:465` (`From<ShapeType> for PyShape`). Rust-Side:
`crates/dcps/src/interop.rs:61` (`pub struct ShapeType`),
`crates/dcps/src/interop.rs:91` (`const TYPE_NAME: &'static str = "ShapeType"`).

**Tests:** `crates/py/python/tests/test_smoke.py::test_shape_topic_matches_vendor_interop_type_name`,
`test_shape_constructor_and_repr`, `test_shape_default_shapesize_is_30`,
`crates/py/python/tests/test_idl.py::test_pyshape_byte_roundtrip`.

**Status:** done

## §3 IDL-Mapping — `@idl_struct` + XCDR2-LE-Codec

### §3.1 `CdrWriter` / `CdrReader`

**Spec:** §3.1 — XCDR2-LE-Codec, Alignment-Rules (1/2/4/8 natural),
Primitive (`bool, iN/uN für N ∈ {8,16,32,64}, f32, f64`),
`write_string` (length-prefix + null-terminator + padding),
`write_bytes` (length-prefix + raw).

**Repo:** `crates/py/python/zerodds/cdr.py` mit `CdrWriter` /
`CdrReader`.

**Tests:** `crates/py/python/tests/test_idl.py::test_cdr_primitive_roundtrip`,
`test_cdr_string_alignment_padding`,
`test_cdr_reader_rejects_truncated_string`.

**Status:** done

### §3.2 `@idl_struct(typename=…)` Decorator

**Spec:** §3.2 — Decorator über `@dataclass` mit Field-Type-Mapping
für: explizite Marker (Bool, Int8..64, UInt8..64, Float32/64, String,
Bytes), Python-Primitives (bool/int/float/str/bytes), `Sequence[T]`,
`Array[T, N]`, `Optional[T]`, `idl_enum` über `IntEnum`, `idl_union`
mit `disc: IntEnum + cases + default`, nested `@idl_struct`.

**Repo:** `crates/py/python/zerodds/idl.py` — `_IdlKind`,
`_IdlSequence`, `_IdlArray`, `_IdlOptional`, `_IdlEnum`,
`_IdlUnion`, `_IdlStruct`, `idl_struct()`, `idl_union()`,
`is_idl_struct()`, `type_name_of()`.

**Tests:** 18 Tests in `crates/py/python/tests/test_idl.py`
(`test_idl_struct_requires_dataclass`, `test_pyshape_byte_roundtrip`,
`test_pyshape_type_name_set_by_decorator`,
`test_sensor_mixed_fields_roundtrip`, `test_auto_map_python_primitives`,
`test_nested_struct_roundtrip`, `test_sequence_of_primitives_roundtrip`,
`test_sequence_of_structs_roundtrip`, `test_array_fixed_count_roundtrip`,
`test_array_wrong_count_rejected`, `test_optional_present_and_absent`,
`test_enum_roundtrip`, `test_enum_unknown_value_raises`,
`test_union_case_int_roundtrip`, `test_union_case_string_roundtrip`,
`test_union_default_branch_used_for_unknown_disc`,
`test_union_without_default_rejects_unknown_disc`,
`test_idl_struct_resolves_pep563_stringified_annotations`).

**Status:** done

### §3.3 Codegen-Free-Pfad

**Spec:** §3.3 — Anwender annotiert `@dataclass`, bekommt Encoder/Decoder
ohne externes Build-Tooling. Byte-identisch zum Rust-Pfad.

**Repo:** Workflow live über `@idl_struct` (§3.2). Byte-Identität
verifiziert in `test_pyshape_byte_roundtrip` und
`test_sensor_mixed_fields_roundtrip`.

**Tests:** wie §3.2.

**Status:** done

## §4 Test-Pflicht

### §4.1 Test-Inventar

**Spec:** §4 — 7 Smoke-Tests + 21 IDL/CDR-Tests = 28 Tests.

**Repo:** `crates/py/python/tests/test_smoke.py` (7 `def test_*`),
`crates/py/python/tests/test_idl.py` (21 `def test_*`).

**Tests:** `pytest crates/py/python/tests/` muss alle 28 Tests
grün liefern.

**Status:** done

### §4.2 Rust-seitiges Test-Inventar

**Spec:** §4 — kein Rust-Test, weil Crate ohne `extension-module`
Platzhalter-Lib ist.

**Repo:** kein `tests/`-Tree in `crates/py/`.

**Tests:** `cargo test -p zerodds-py` ohne Feature-Flag liefert
"0 tests run" (Workspace-Run hat es deshalb stillschweigend in der
Total-Zählung).

**Status:** n/a (informative) — bewusste Architektur-Entscheidung
(siehe Vendor-Spec §1.2, `crates/py/Cargo.toml` `[features]`-Block).

## §5 Multi-Process / Cross-Vendor

### §5.1 Single-Process Python ↔ Python

**Spec:** §5 Tabelle Row 1 — Single-Process Python ↔ Python über
`zerodds._core` und `crates/dcps`.

**Repo:** `crates/py/src/ffi.rs` baut auf
`crates/dcps/src/{factory,participant,publisher,subscriber}.rs`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`.

**Status:** done

### §5.2 Multi-Process Python ↔ Python

**Spec:** §5 Tabelle Row 2 — Multi-Process lokal über RTPS/UDP +
SPDP/SEDP.

**Repo:** `crates/dcps` spawnt SPDP-/SEDP-Endpoints. Multi-Process-
Pfad lebt also auf der Rust-Side.

**Tests:** `crates/py/python/tests/test_multiproc.py::test_multiproc_python_python_roundtrip`
spawnt zwei `python -m`-Subprocesses (Publisher + Subscriber) und
verifiziert Cross-Process-Pub/Sub auf gleicher Domain-ID. Test skippt
auf Windows (Loopback-Multicast-Setup linux/macOS-spezifisch).

**Status:** done — Test laeuft grün auf
`llvm@llvm` (Debian 12, enp6s18 NIC, SPDP-Multicast). Skip auf
Windows.

### §5.3 Cross-Vendor Python ↔ andere Sprachen

**Spec:** §5 Tabelle Row 3 — ShapeType-Wire byte-identisch zu
C++/Rust/Java/C#-Peers über gemeinsamen Topic-Type `"ShapeType"`.

**Repo:** `crates/dcps/src/interop.rs` (Rust-Side),
`crates/py/src/ffi.rs:420` (Python-Side). XCDR2-LE-Wire über
`crates/cdr/`.

**Tests:** `crates/py/python/tests/test_idl.py::test_pyshape_byte_roundtrip`
verifiziert Byte-Identität gegen die XCDR2-LE-Bytes der
Rust-`ShapeType`-Encoding.

**Status:** done

### §5.4 Cross-Vendor Python ↔ Cyclone-/Fast-DDS-ShapesDemo

**Spec:** §5 Tabelle Row 4 — gleiche XCDR2-LE-Konvention +
gleicher Type-Name + RTPS-2.5-Wire.

**Repo:** `crates/py/src/ffi.rs:236` (`ShapeTopic` mit
`type_name = "ShapeType"`); RTPS-2.5-Wire über `crates/rtps/`
+ `crates/transport-udp/`.

**Tests:** Rust-Side bleibt die kanonische Quelle fuer Byte-Identitaet
(`crates/discovery/tests/cyclone_live_*.rs`). Python-Side hat
`crates/py/python/tests/test_shapesdemo_interop.py` mit 2 Tests:
`test_shape_type_name_matches_shapesdemo_convention` (Topic-Type-Name
`"ShapeType"` matches Cyclone-Konvention),
`test_cross_vendor_participant_discovery_against_ddsperf`
(SPDP-Multicast-Discovery: Cyclone `ddsperf -i <domain> pub 1Hz`
startet einen Participant, ZeroDDS sieht ihn ueber
`discovered_participants_count() >= 1`).

**Status:** done — Cross-Vendor-SPDP-Wire-Kompatibilitaet auf
`llvm@llvm` mit Cyclone DDS 0.10.2 verifiziert (Test laeuft grün).

### §5.5 `@idl_struct`-Custom-Type ↔ Rust-Codegen

**Spec:** §5 Tabelle Row 5 — byte-identisch verifiziert.

**Repo:** `crates/py/python/zerodds/idl.py` +
`crates/py/python/zerodds/cdr.py`. Rust-Codegen-Pendant in
`crates/idl-rust/`.

**Tests:** `crates/py/python/tests/test_idl.py::test_pyshape_byte_roundtrip`,
`test_sensor_mixed_fields_roundtrip` (byte-genau gegen Rust-encoded
Reference-Bytes).

**Status:** done

## §6 Phase-2-Plan

### §6.1 `IdlTopic` / `IdlWriter` / `IdlReader` mit Codegen-Loop

**Spec:** §6 — `IdlTopic(participant, name, dataclass)` wraps eine
`@idl_struct`-Dataclass mit `BytesTopic`-Surface und liefert
`IdlWriter[T]`/`IdlReader[T]`, die per-Sample encode/decode betreiben.

**Repo:** `crates/py/python/zerodds/topic.py` mit drei pure-Python-
Klassen (`IdlTopic`, `IdlWriter`, `IdlReader`) plus
`_ensure_idl_struct`-Guard. Re-Export ueber
`zerodds.__init__.py` als `zerodds.IdlTopic` etc.

**Tests:** `crates/py/python/tests/test_idl_topic.py` (5 Tests):
`test_idl_topic_rejects_non_idl_struct`,
`test_idl_topic_exposes_type_name_from_decorator`,
`test_idl_writer_encodes_and_reader_decodes_roundtrip`,
`test_idl_writer_rejects_wrong_type`,
`test_idl_writer_passthrough_status`.

**Status:** done

### §6.2 QoS-Builder (alle 22 Policies)

**Spec:** §6 — `DataWriterQos`/`DataReaderQos`-PyClass mit Settern
fuer alle 22 Policies aus DDS 1.4 §2.2.3 plus
`Publisher.create_*_writer_with_qos` /
`Subscriber.create_*_reader_with_qos`.

**Repo:** `crates/py/src/qos.rs` (~530 Zeilen) — zwei PyClasses
`PyDataWriterQos`, `PyDataReaderQos` mit insgesamt 18 + 16 Settern
fuer die 22 Policies, sieben Enum-Parser-Helpers
(`parse_reliability_kind`, `parse_durability_kind`,
`parse_history_kind`, `parse_liveliness_kind`,
`parse_ownership_kind`, `parse_destination_order_kind`,
`parse_presentation_scope`), Reflection-Getter
(`reliability_kind()`, `durability_kind()`, `history_kind()`,
`history_depth()`). `crates/py/src/ffi.rs:286-301`
(`create_bytes_writer_with_qos`, `create_shape_writer_with_qos`,
`create_bytes_reader_with_qos`, `create_shape_reader_with_qos`).

**Tests:** `crates/py/python/tests/test_qos.py` (12 Tests):
`test_writer_qos_defaults`, `test_reader_qos_defaults`,
`test_writer_qos_reliability_setter`,
`test_writer_qos_rejects_unknown_reliability_kind`,
`test_writer_qos_durability_all_kinds`,
`test_writer_qos_history_setter`,
`test_writer_qos_setters_with_durations`,
`test_reader_qos_full_setter_chain`,
`test_create_bytes_writer_with_qos_offline`,
`test_create_bytes_reader_with_qos_offline`,
`test_create_shape_writer_with_qos_offline`,
`test_create_shape_reader_with_qos_offline`.

**Status:** done

### §6.3 AsyncIO-API

**Spec:** §6 — `async def wait_for_data`,
`async def wait_for_matched_*`, asyncio-Integration via
`asyncio.to_thread`-Brueke.

**Repo:** `crates/py/python/zerodds/aio.py` mit
`AsyncBytesWriter`, `AsyncBytesReader`, `AsyncShapeWriter`,
`AsyncShapeReader`, `AsyncWaitSet`. Non-blocking Calls werden
direkt durchgereicht (`take`, Status-Getter); blocking Calls
laufen ueber `_to_thread`-Helper (Backport-freundlich fuer Py3.8).

**Tests:** `crates/py/python/tests/test_aio.py` (5 Tests):
`test_async_wrappers_are_constructible`,
`test_async_passthrough_status_getters`,
`test_async_take_returns_list_passthrough`,
`test_async_wait_for_data_uses_event_loop` (verifiziert dass
der asyncio-Event-Loop bei `wait_for_data` nicht blockiert),
`test_async_waitset_wait_raises_timeout`.

**Status:** done

### §6.4 ROS-2-pytest-Integration

**Spec:** §6 — `launch_pytest`-Fixture für Multi-Process-Live-Tests
gegen Cyclone DDS ROS-2.

**Repo:** `crates/py/python/tests/ros2/` mit `conftest.py`
(Conditional-Skip wenn `ROS_DISTRO` nicht gesetzt oder `rclpy`
nicht importierbar oder `RMW_IMPLEMENTATION != rmw_zerodds_shim`)
plus `test_rmw_zerodds_interop.py` mit 2 Tests
(`test_rclpy_init_succeeds_with_zerodds_rmw`,
`test_rclpy_publish_subscribe_string_roundtrip`).

**Tests:** auf einem ROS-2-Host: beide Tests laufen. In normalem
pytest-Run werden sie geskipped.

**Status:** partial — Test-Skeleton geschrieben + Skip-Logic. Echte
Verifikation braucht ROS-2-Setup + `rmw_zerodds_shim` als
RMW-Implementation (siehe `crates/rmw-zerodds-shim/`).

### §6.5 Status-Listener-Callbacks

**Spec:** §6 — Python-Callbacks für `on_data_available`,
`on_liveliness_changed`, etc., registriert über
`set_listener(callback, mask)`.

**Repo:** `crates/py/src/listener.rs` (~365 Zeilen) — zwei PyClasses
`PyDataWriterListener` (3 Slots: `on_offered_deadline_missed`,
`on_liveliness_lost`, `on_publication_matched`) und
`PyDataReaderListener` (6 Slots: `on_data_available`,
`on_sample_lost`, `on_sample_rejected`,
`on_requested_deadline_missed`, `on_liveliness_changed`,
`on_subscription_matched`). Zwei Bridge-Structs
(`PyDataWriterListenerBridge`, `PyDataReaderListenerBridge`)
implementieren die Rust-Traits `DataWriterListener`/`DataReaderListener`
und rufen die Python-Callbacks unter GIL-Acquire. `set_listener` /
`clear_listener` auf `PyBytesWriter`/`PyBytesReader`/
`PyShapeWriter`/`PyShapeReader` in `ffi.rs`.

**Tests:** `crates/py/python/tests/test_listener.py` (6 Tests):
`test_writer_listener_constructible`,
`test_reader_listener_constructible`,
`test_set_listener_on_bytes_writer_offline`,
`test_set_listener_on_bytes_reader_offline`,
`test_set_listener_on_shape_writer_offline`,
`test_set_listener_on_shape_reader_offline`.

**Status:** done

### §6.6 ReadCondition / QueryCondition

**Spec:** §6 — WaitSet-Conditions über Reader-State
(SampleStateKind, ViewStateKind, InstanceStateKind), SQL92-Filter-
Expression für QueryCondition.

**Repo:** `crates/py/src/conditions.rs` (~135 Zeilen) — zwei PyClasses
`PyReadCondition`, `PyQueryCondition` ueber den Rust-Conditions
aus `crates/dcps/src/condition.rs`. SampleState/ViewState/
InstanceState-Bitmask-Konstanten in `ffi.rs` exportiert als
`SAMPLE_STATE_*`, `VIEW_STATE_*`, `INSTANCE_STATE_*` Modul-
Attribute; pure-Python-Wrapper-Module
`crates/py/python/zerodds/{sample_state,view_state,instance_state}.py`
liefern den pythonischen Namespace. `PyWaitSet::attach_read_condition` /
`attach_query_condition` in `ffi.rs`.

**Tests:** `crates/py/python/tests/test_read_conditions.py`
(11 Tests): `test_sample_state_constants`,
`test_view_state_constants`, `test_instance_state_constants`,
`test_read_condition_constructible`, `test_read_condition_modes`,
`test_read_condition_unknown_mode_raises`,
`test_read_condition_attaches_to_waitset`,
`test_query_condition_constructible_simple_filter`,
`test_query_condition_with_parameters`,
`test_query_condition_invalid_sql_raises`,
`test_query_condition_attaches_to_waitset`.

**Status:** done

### §6.7 sphinx-Doc-Pfad

**Spec:** §6 — `crates/py/docs/` schon angelegt; Phase-2 ergänzt
API-Inventarisierung aus `_core` über autodoc.

**Repo:** `crates/py/docs/conf.py` mit `autodoc`/`napoleon`/`viewcode`/
`intersphinx` extensions, `autodoc_default_options` und
`autodoc_mock_imports = ["zerodds._core"]` fuer RTD-Build ohne
maturin. `crates/py/docs/api.rst` listet voll alle PyClasses und
pure-Python-Module mit `autoclass`/`automodule`.

**Tests:** `PYTHONPATH=crates/py/python python3 -m sphinx -b html
crates/py/docs /tmp/zd-py-sphinx` produziert HTML mit allen Modulen
(`zerodds._core`, `zerodds.cdr`, `zerodds.idl`, `zerodds.loader`)
und allen 13 PyClasses, 0 hard errors (14 warnings = fehlende
docstrings im Mock-Pfad).

**Status:** done

## §7 Stabilität

**Spec:** §7 — semver-Politik, v1.0 stabil, Major-Bump für
Breaking-Changes.

**Repo:** `crates/py/Cargo.toml` (`version.workspace = true` →
`1.0.0-rc.1` aus dem Workspace-Cargo).

**Tests:** kein eigener Stabilitäts-Test (Versionsabgleich-CI in
`tests/system/` falls vorhanden).

**Status:** n/a (informative) — Policy-Statement, nicht
test-pflichtig.

## §8 Lizenz

**Spec:** §8 — Apache-2.0 (Workspace-Default).

**Repo:** `crates/py/Cargo.toml` (`license.workspace = true`),
`LICENSE-APACHE` im Repo-Root.

**Tests:** —

**Status:** n/a (informative).

## §9 Referenzen

**Spec:** §9 — Liste von externen Specs (OMG DDS 1.4, XTypes 1.3,
PSM-Cxx 1.0) und internen Vendor-Specs (`zerodds-c-api-1.0`,
`zerodds-ffi-loader-1.0`, `zerodds-java-omgdds-1.0`) plus PyO3 0.22
und PEP 384.

**Repo:** Verweise verlinkt zu den jeweiligen
`docs/specs/*.md`-Dateien und externen Quellen.

**Tests:** —

**Status:** n/a (informative).

---

## Audit-Status

**41 done / 1 partial / 0 open / 5 n/a (informative) / 0 n/a (rejected).**

Test-Laeufe:

- **Lokal (macOS, Apple Silicon)**: `cd crates/py && SDKROOT=$(xcrun --show-sdk-path) maturin develop --features extension-module && python3 -m pytest python/tests/` — 96 Tests grün, 9 Conditional-Skip.
- **Bench-Host `llvm@llvm` (Debian 12, x86_64, Cyclone DDS 0.10.2)**: identischer pytest-Aufruf, plus
  `ZERODDS_LIB=$HOME/zerodds/target/debug/libzerodds.so` und
  `LD_LIBRARY_PATH=$HOME/zerodds/target/debug` — **102 Tests grün,
  3 Conditional-Skip** (ROS-2 fehlt + 1 live-E2E-Placeholder).
  Verifikation 2026-05-15.

Auf llvm laufen die loader-smoke-Suite (libzerodds.so verfuegbar),
Multi-Process-Subprocess-Roundtrip und Cross-Vendor-SPDP-Discovery
gegen Cyclone `ddsperf` durch.

Offene Punkte: siehe `docs/spec-coverage/zerodds-py-1.0.open.md` (1 partial). 0 open. Keine `n/a (rejected)`-Items.
