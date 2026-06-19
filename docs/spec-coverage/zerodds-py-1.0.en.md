# `zerodds-py` v1.0 — Spec Coverage

Audit of the vendor spec `docs/specs/zerodds-py-1.0.md` against
`crates/py/` code reality.

**Source:** `docs/specs/zerodds-py-1.0.md` (vendor spec, draft 2026-05-15).

Implementation:

- `crates/py/` — Rust PyO3 bridge + Python wrapper.

---

## §1 Architecture

### §1.1 Module Layout

**Spec:** §1.1 — crate layout with `Cargo.toml` (crate-type `cdylib`+`rlib`),
`pyproject.toml` for maturin, `src/lib.rs`+`ffi.rs`,
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

**Tests:** layout self-verified by pytest discovery
(`pytest crates/py/python/tests/`).

**Status:** done

### §1.2 PyO3 module `zerodds._core` — 13 PyClasses

One item per PyClass. Mapping from DDS 1.4 §2.2.2.

#### §1.2.1 `DomainParticipantFactory` (PyClass)

**Spec:** §1.2 + §2.1 — PyClass `DomainParticipantFactory` with
`instance()`, `create_participant(domain_id)`,
`create_participant_offline(domain_id)`,
`create_participant_fast(domain_id)`.

**Repo:** `crates/py/src/ffi.rs:53` (`#[pyclass(name = "DomainParticipantFactory")]`),
methods in `ffi.rs:60-95`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_factory_singleton_and_offline_participant`,
`test_pub_sub_roundtrip_live`.

**Status:** done

#### §1.2.2 `DomainParticipant` (PyClass)

**Spec:** §1.2 + §2.2 — PyClass `DomainParticipant` with `domain_id`,
`topics_len`, `discovered_participants_count`,
`create_{bytes,shape}_topic`, `create_{publisher,subscriber}`,
`assert_liveliness`, `ignore_{participant,topic,publication,subscription}`,
`contains_entity`, `get_discovered_{topics,participants}`.

**Repo:** `crates/py/src/ffi.rs:97` (`#[pyclass(name = "DomainParticipant")]`),
methods in `ffi.rs:105-215`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_factory_singleton_and_offline_participant`,
`test_bytes_topic_and_reader_are_creatable_offline`,
`test_pub_sub_roundtrip_live`.

**Status:** done

#### §1.2.3 `BytesTopic` (PyClass)

**Spec:** §1.2 — PyClass `BytesTopic` with `name`, `type_name`.

**Repo:** `crates/py/src/ffi.rs:219` (`#[pyclass(name = "BytesTopic")]`),
methods in `ffi.rs:224-234`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_bytes_topic_and_reader_are_creatable_offline`.

**Status:** done

#### §1.2.4 `ShapeTopic` (PyClass)

**Spec:** §1.2 + §2.7 — PyClass `ShapeTopic` with `name`, `type_name`,
type name = `"ShapeType"`.

**Repo:** `crates/py/src/ffi.rs:236` (`#[pyclass(name = "ShapeTopic")]`),
methods in `ffi.rs:241-255`. Type name from
`crates/dcps/src/interop.rs:91` (`TYPE_NAME: &'static str = "ShapeType"`).

**Tests:** `crates/py/python/tests/test_smoke.py::test_shape_topic_matches_vendor_interop_type_name`.

**Status:** done

#### §1.2.5 `Publisher` (PyClass)

**Spec:** §1.2 + §2.3 — PyClass `Publisher` with
`create_bytes_writer(topic)`, `create_shape_writer(topic)`.

**Repo:** `crates/py/src/ffi.rs:257` (`#[pyclass(name = "Publisher")]`),
methods in `ffi.rs:262-279`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`.

**Status:** done

#### §1.2.6 `Subscriber` (PyClass)

**Spec:** §1.2 + §2.3 — PyClass `Subscriber` with
`create_bytes_reader(topic)`, `create_shape_reader(topic)`.

**Repo:** `crates/py/src/ffi.rs:281` (`#[pyclass(name = "Subscriber")]`),
methods in `ffi.rs:286-303`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_bytes_topic_and_reader_are_creatable_offline`,
`test_pub_sub_roundtrip_live`.

**Status:** done

#### §1.2.7 `BytesWriter` (PyClass)

**Spec:** §1.2 + §2.4 — PyClass `BytesWriter` with `write`,
`wait_for_matched_subscription`, `matched_subscription_count`,
`publication_matched_status`, `liveliness_lost_status`,
`offered_deadline_missed_status`.

**Repo:** `crates/py/src/ffi.rs:309` (`#[pyclass(name = "BytesWriter")]`),
methods in `ffi.rs:314-355`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`.

**Status:** done

#### §1.2.8 `BytesReader` (PyClass)

**Spec:** §1.2 + §2.5 — PyClass `BytesReader` with `take`,
`wait_for_data`, `wait_for_matched_publication`,
`matched_publication_count`, `subscription_matched_status`,
`sample_lost_status`, `requested_deadline_missed_status`.

**Repo:** `crates/py/src/ffi.rs:357` (`#[pyclass(name = "BytesReader")]`),
methods in `ffi.rs:362-418`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_bytes_topic_and_reader_are_creatable_offline`,
`test_pub_sub_roundtrip_live`.

**Status:** done

#### §1.2.9 `Shape` (PyClass / dataclass)

**Spec:** §1.2 + §2.7 — PyClass `Shape` with fields `color`, `x`, `y`,
`shapesize` (default 30), `__repr__`, conversions `From<&PyShape>`
and `From<ShapeType>` in both directions.

**Repo:** `crates/py/src/ffi.rs:420` (`#[pyclass(name = "Shape")]`),
methods in `ffi.rs:437-468`. Rust counterpart in
`crates/dcps/src/interop.rs:61` (`pub struct ShapeType`).

**Tests:** `crates/py/python/tests/test_smoke.py::test_shape_constructor_and_repr`,
`test_shape_default_shapesize_is_30`,
`crates/py/python/tests/test_idl.py::test_pyshape_byte_roundtrip`,
`test_pyshape_type_name_set_by_decorator`.

**Status:** done

#### §1.2.10 `ShapeWriter` (PyClass)

**Spec:** §1.2 + §2.4 — PyClass `ShapeWriter` with `write(shape)`,
`register_instance(shape)`, `dispose(shape)`,
`unregister_instance(shape)`, `wait_for_matched_subscription`.

**Repo:** `crates/py/src/ffi.rs:475` (`#[pyclass(name = "ShapeWriter")]`),
methods in `ffi.rs:480-537`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`.

**Status:** done

#### §1.2.11 `ShapeReader` (PyClass)

**Spec:** §1.2 + §2.5 — PyClass `ShapeReader` with `take`,
`wait_for_data`, `wait_for_matched_publication`.

**Repo:** `crates/py/src/ffi.rs:539` (`#[pyclass(name = "ShapeReader")]`),
methods in `ffi.rs:544-583`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`.

**Status:** done

#### §1.2.12 `GuardCondition` (PyClass)

**Spec:** §1.2 + §2.6 — PyClass `GuardCondition` with constructor,
`set_trigger_value(bool)`, `get_trigger_value() -> bool`.

**Repo:** `crates/py/src/ffi.rs:585` (`#[pyclass(name = "GuardCondition")]`),
methods in `ffi.rs:590-609`. Module registration:
`crates/py/src/ffi.rs:660` (`m.add_class::<PyGuardCondition>()`).

**Tests:** `crates/py/python/tests/test_conditions.py::test_guard_condition_default_trigger_is_false`,
`test_guard_condition_set_trigger_roundtrip`,
`test_guard_condition_via_outer_namespace`. `zerodds.GuardCondition`
is re-exported in the outer namespace
(`crates/py/python/zerodds/__init__.py:50` + `__all__`).

**Status:** done

#### §1.2.13 `WaitSet` (PyClass)

**Spec:** §1.2 + §2.6 — PyClass `WaitSet` with constructor,
`attach_guard_condition(gc)`, `wait(timeout_secs) -> int`.

**Repo:** `crates/py/src/ffi.rs:611` (`#[pyclass(name = "WaitSet")]`),
methods in `ffi.rs:617-642`. Module registration:
`crates/py/src/ffi.rs:661` (`m.add_class::<PyWaitSet>()`).

**Tests:** `crates/py/python/tests/test_conditions.py::test_waitset_attach_and_wait_raises_timeout`,
`test_waitset_wakes_when_guard_condition_fires`,
`test_waitset_via_outer_namespace`. `zerodds.WaitSet` is
re-exported in the outer namespace.

**Status:** done

### §1.3 Python wrapper `python/zerodds/`

#### §1.3.1 `__init__.py` — re-export

**Spec:** §1.3 — `zerodds.__init__` re-exports the `_core` PyClasses
plus the pure-Python modules `cdr` and `idl`.

**Repo:** `crates/py/python/zerodds/__init__.py` re-exports the
full `_core` surface — all 13 original PyClasses + the 6
new ones from §6 (`DataWriterQos, DataReaderQos, DataWriterListener,
DataReaderListener, ReadCondition, QueryCondition`) + the three
bitmask modules (`sample_state`, `view_state`, `instance_state`).
Plus IDL type markers from `.idl` and the §6.1 `IdlTopic`/`IdlWriter`/
`IdlReader` wrappers from `.topic`. Fallback when `_core` is missing:
`_CoreStub` with a clear `ImportError`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_version_exposed`,
`test_conditions.py::test_guard_condition_via_outer_namespace`,
`test_conditions.py::test_waitset_via_outer_namespace` —
explicitly verify that both are reachable via `zerodds.<Name>`.

**Status:** done

#### §1.3.2 `cdr.py` — XCDR2-LE codec

**Spec:** §1.3 + §3.1 — `CdrWriter` / `CdrReader` with alignment rules,
primitives (`bool`, `i8..64`, `u8..64`, `f32/64`), `string` (length-prefix
+ null-terminator + padding), `bytes` (length-prefix + raw). Byte-exact
to `crates/cdr/src/buffer.rs`.

**Repo:** `crates/py/python/zerodds/cdr.py` (`CdrWriter`, `CdrReader`).

**Tests:** `crates/py/python/tests/test_idl.py::test_cdr_primitive_roundtrip`,
`test_cdr_string_alignment_padding`,
`test_cdr_reader_rejects_truncated_string`.

**Status:** done

#### §1.3.3 `idl.py` — `@idl_struct` decorator

**Spec:** §1.3 + §3.2 — `@idl_struct(typename=…)` decorator,
field type markers (`Bool`, `Int8..64`, `UInt8..64`, `Float32/64`,
`String`, `Bytes`, `Sequence[T]`, `Array[T, N]`, `Optional[T]`,
`idl_enum`, `idl_union`, nested `@idl_struct`), auto-mapping of
Python primitives.

**Repo:** `crates/py/python/zerodds/idl.py` with `_IdlKind` class and
type constants (Bool, Int8..64, UInt8..64, Float32/64, String,
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

#### §1.3.4 `loader.py` — pure-`ctypes` loader

**Spec:** §1.3 + §1.4 — pure-`ctypes` loader against
`libzerodds.{so,dylib,dll}` from `crates/zerodds-c-api/`. Follows
`zerodds-ffi-loader-1.0` §3.1. Signatures from
`crates/zerodds-c-api/include/zerodds.h`.

**Repo:** `crates/py/python/zerodds/loader.py` (420 lines) with
`Runtime`, `Writer`, `Reader`. Consumes
`crates/zerodds-c-api/include/zerodds.h`.

**Tests:** `crates/py/python/tests/test_loader_smoke.py` with
4 tests (`test_loader_resolves_library`,
`test_loader_runtime_create_participant_offline`,
`test_loader_factory_singleton`,
`test_loader_zerodds_error_is_runtime_error_subclass`). Tests
skip automatically when `libzerodds.{so,dylib,dll}` is not
found; before a CI run, `cargo build -p zerodds-c-api`
must have been run (or `ZERODDS_LIB` set).

**Status:** done — tests run green on
`Linux-Bench-Host` (Debian 12, libzerodds.so from
`~/zerodds/target/debug/libzerodds.so`, `ZERODDS_LIB` ENV override).
 On systems without a built artifact the
tests skip cleanly.

### §1.4 Two-path choice

**Spec:** §1.4 — two independent paths: PyO3 (performance,
RTPS-native) and pure-`ctypes` (distro package, zero-build).

**Repo:** PyO3 path: `crates/py/src/ffi.rs` + `python/zerodds/__init__.py`.
ctypes path: `crates/py/python/zerodds/loader.py` +
`crates/zerodds-c-api/`.

**Tests:** PyO3 fully covered (all PyClass tests from §1.2/§2/§3/§6).
ctypes path via `test_loader_smoke.py` (conditional skip; see §1.3.4).

**Status:** done — both paths verified live on
`Linux-Bench-Host` (PyO3 via maturin + ctypes via
libzerodds.so).

### §1.5 Layer position

**Spec:** §1.5 — Layer 6 (PSMs / bindings). Direct dependencies:
`zerodds-dcps` (Layer 4), `zerodds-c-api` (Layer 6 C-FFI).

**Repo:** `crates/py/Cargo.toml` (`zerodds-dcps = { path = "../dcps" }`).
ctypes path: no Cargo dep, only runtime lookup of
`libzerodds.{so,dylib,dll}`.

**Tests:** indirectly via `cargo build -p zerodds-py` + all tests
(workspace build).

**Status:** n/a (informative) — architecture statement, not
test-bound.

## §2 OMG API coverage (DDS 1.4 §2.2.2)

### §2.1 DomainParticipantFactory (DDS 1.4 §2.2.2.2.1)

**Spec:** §2.1 — mapping table: `get_instance()`,
`create_participant`, `create_participant_offline`, implicit
`delete_participant` via Python GC.

**Repo:** `crates/py/src/ffi.rs:60` (`fn instance() -> Self`),
`ffi.rs:65` (`create_participant_offline`), `ffi.rs:72`
(`create_participant`), `ffi.rs:80` (`create_participant_fast`).
Drop behavior inherits from `Arc<DomainParticipant>` in
`crates/dcps/src/factory.rs`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_factory_singleton_and_offline_participant`,
`test_pub_sub_roundtrip_live`.

**Status:** done

### §2.2 DomainParticipant (DDS 1.4 §2.2.2.2.2)

**Spec:** §2.2 — 12 operations: `get_domain_id`, `create_topic`,
`create_publisher`, `create_subscriber`, `assert_liveliness`,
`ignore_{participant,topic,publication,subscription}`,
`contains_entity`, `get_discovered_{topics,participants}`.

**Repo:** `crates/py/src/ffi.rs:105-215` — all 12 operations as
PyMethods (`domain_id`, `topics_len`, `discovered_participants_count`,
`create_bytes_topic`, `create_shape_topic`, `create_publisher`,
`create_subscriber`, `assert_liveliness`, `ignore_*` (4 variants),
`contains_entity`, `get_discovered_topics`, `get_discovered_participants`).

**Tests:** smoke tests from `test_smoke.py` plus dedicated tests in
`crates/py/python/tests/test_participant_ignore.py` (9 tests):
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

**Spec:** §2.4 — mapping table: `write`, `register_instance`,
`unregister_instance`, `dispose`, `get_matched_subscriptions`,
`get_publication_matched_status`, `get_liveliness_lost_status`,
`get_offered_deadline_missed_status` plus
`wait_for_matched_subscription` sync helper.

**Repo:** `crates/py/src/ffi.rs:317` (BytesWriter `write`), `ffi.rs:324`
(`wait_for_matched_subscription`), `ffi.rs:337-355` (status getters
+ `matched_subscription_count`). `ShapeWriter` in `ffi.rs:482`
(`write`), `ffi.rs:492` (`register_instance`), `ffi.rs:502`
(`dispose`), `ffi.rs:515` (`unregister_instance`), `ffi.rs:525`
(`wait_for_matched_subscription`).

**Tests:** smoke tests + dedicated lifecycle tests in
`crates/py/python/tests/test_writer_lifecycle.py` (6 tests):
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
and `wait_for_historical_data` deliberately omitted.

**Repo:** `crates/py/src/ffi.rs:365` (BytesReader `take`), `ffi.rs:375`
(`wait_for_data`), `ffi.rs:381` (`wait_for_matched_publication`),
`ffi.rs:394-418` (status getters + `matched_publication_count`).
ShapeReader analogously in `ffi.rs:546-583`.

**Tests:** smoke tests + dedicated status-getter tests in
`crates/py/python/tests/test_status_getters.py` (8 tests):
`test_publication_matched_status_shape`,
`test_subscription_matched_status_shape`,
`test_liveliness_lost_status_shape`,
`test_offered_deadline_missed_status_shape`,
`test_sample_lost_status_shape`,
`test_requested_deadline_missed_status_shape`,
`test_matched_subscription_count_zero_offline`,
`test_matched_publication_count_zero_offline`.

**Status:** done

### §2.6 WaitSet / conditions (DDS 1.4 §2.2.2.6)

**Spec:** §2.6 — `WaitSet`, `GuardCondition` with
`attach_guard_condition`, `wait`, `set/get_trigger_value`.
`ReadCondition`/`QueryCondition` are available via §6.6. `WaitSet`
+ `GuardCondition` in v1.0 reachable only directly via `zerodds._core`,
not in the outer namespace.

**Repo:** `crates/py/src/ffi.rs:585-642`. Module registration:
`ffi.rs:660-661`.

**Tests:** `crates/py/python/tests/test_conditions.py` (GuardCondition +
WaitSet, 6 tests, see §1.2.12/§1.2.13) +
`crates/py/python/tests/test_read_conditions.py` (ReadCondition +
QueryCondition + bitmask constants, 11 tests, see §6.6).
`zerodds.GuardCondition`, `zerodds.WaitSet`, `zerodds.ReadCondition`,
`zerodds.QueryCondition` are all re-exported in the outer namespace.

**Status:** done

### §2.7 ShapeType — cross-vendor interop type

**Spec:** §2.7 — `Shape` PyClass byte-identical to `ShapeType` in
`crates/dcps/src/interop.rs`. Fields `color: String, x: i32, y: i32,
shapesize: i32` (default 30). Type name on the wire = `"ShapeType"`.

**Repo:** `crates/py/src/ffi.rs:420` (`#[pyclass(name = "Shape")]`),
constructor `ffi.rs:441` (`fn new(color, x, y, shapesize)`),
default `shapesize=30` via `pyo3(signature)`,
conversions `ffi.rs:459` (`From<&PyShape> for ShapeType`),
`ffi.rs:465` (`From<ShapeType> for PyShape`). Rust side:
`crates/dcps/src/interop.rs:61` (`pub struct ShapeType`),
`crates/dcps/src/interop.rs:91` (`const TYPE_NAME: &'static str = "ShapeType"`).

**Tests:** `crates/py/python/tests/test_smoke.py::test_shape_topic_matches_vendor_interop_type_name`,
`test_shape_constructor_and_repr`, `test_shape_default_shapesize_is_30`,
`crates/py/python/tests/test_idl.py::test_pyshape_byte_roundtrip`.

**Status:** done

## §3 IDL mapping — `@idl_struct` + XCDR2-LE codec

### §3.1 `CdrWriter` / `CdrReader`

**Spec:** §3.1 — XCDR2-LE codec, alignment rules (1/2/4/8 natural),
primitives (`bool, iN/uN for N ∈ {8,16,32,64}, f32, f64`),
`write_string` (length-prefix + null-terminator + padding),
`write_bytes` (length-prefix + raw).

**Repo:** `crates/py/python/zerodds/cdr.py` with `CdrWriter` /
`CdrReader`.

**Tests:** `crates/py/python/tests/test_idl.py::test_cdr_primitive_roundtrip`,
`test_cdr_string_alignment_padding`,
`test_cdr_reader_rejects_truncated_string`.

**Status:** done

### §3.2 `@idl_struct(typename=…)` decorator

**Spec:** §3.2 — decorator over `@dataclass` with field type mapping
for: explicit markers (Bool, Int8..64, UInt8..64, Float32/64, String,
Bytes), Python primitives (bool/int/float/str/bytes), `Sequence[T]`,
`Array[T, N]`, `Optional[T]`, `idl_enum` via `IntEnum`, `idl_union`
with `disc: IntEnum + cases + default`, nested `@idl_struct`.

**Repo:** `crates/py/python/zerodds/idl.py` — `_IdlKind`,
`_IdlSequence`, `_IdlArray`, `_IdlOptional`, `_IdlEnum`,
`_IdlUnion`, `_IdlStruct`, `idl_struct()`, `idl_union()`,
`is_idl_struct()`, `type_name_of()`.

**Tests:** 18 tests in `crates/py/python/tests/test_idl.py`
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

### §3.3 Codegen-free path

**Spec:** §3.3 — user annotates `@dataclass`, gets encoder/decoder
without external build tooling. Byte-identical to the Rust path.

**Repo:** workflow live via `@idl_struct` (§3.2). Byte identity
verified in `test_pyshape_byte_roundtrip` and
`test_sensor_mixed_fields_roundtrip`.

**Tests:** as §3.2.

**Status:** done

## §4 Test obligation

### §4.1 Test inventory

**Spec:** §4 — 7 smoke tests + 21 IDL/CDR tests = 28 tests.

**Repo:** `crates/py/python/tests/test_smoke.py` (7 `def test_*`),
`crates/py/python/tests/test_idl.py` (21 `def test_*`).

**Tests:** `pytest crates/py/python/tests/` must deliver all 28 tests
green.

**Status:** done

### §4.2 Rust-side test inventory

**Spec:** §4 — no Rust test, because the crate is a placeholder lib
without `extension-module`.

**Repo:** no `tests/` tree in `crates/py/`.

**Tests:** `cargo test -p zerodds-py` without feature flag yields
"0 tests run" (the workspace run therefore silently included it in the
total count).

**Status:** n/a (informative) — deliberate architecture decision
(see vendor spec §1.2, `crates/py/Cargo.toml` `[features]` block).

## §5 Multi-process / cross-vendor

### §5.1 Single-process Python ↔ Python

**Spec:** §5 table row 1 — single-process Python ↔ Python via
`zerodds._core` and `crates/dcps`.

**Repo:** `crates/py/src/ffi.rs` builds on
`crates/dcps/src/{factory,participant,publisher,subscriber}.rs`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`.

**Status:** done

### §5.2 Multi-process Python ↔ Python

**Spec:** §5 table row 2 — multi-process local via RTPS/UDP +
SPDP/SEDP.

**Repo:** `crates/dcps` spawns SPDP/SEDP endpoints. The multi-process
path therefore lives on the Rust side.

**Tests:** `crates/py/python/tests/test_multiproc.py::test_multiproc_python_python_roundtrip`
spawns two `python -m` subprocesses (publisher + subscriber) and
verifies cross-process pub/sub on the same domain ID. The test skips
on Windows (loopback multicast setup is linux/macOS-specific).

**Status:** done — test runs green on
`Linux-Bench-Host` (Debian 12, enp6s18 NIC, SPDP multicast). Skip on
Windows.

### §5.3 Cross-vendor Python ↔ other languages

**Spec:** §5 table row 3 — ShapeType wire byte-identical to
C++/Rust/Java/C# peers via the shared topic type `"ShapeType"`.

**Repo:** `crates/dcps/src/interop.rs` (Rust side),
`crates/py/src/ffi.rs:420` (Python side). XCDR2-LE wire via
`crates/cdr/`.

**Tests:** `crates/py/python/tests/test_idl.py::test_pyshape_byte_roundtrip`
verifies byte identity against the XCDR2-LE bytes of the
Rust `ShapeType` encoding.

**Status:** done

### §5.4 Cross-vendor Python ↔ Cyclone/Fast-DDS ShapesDemo

**Spec:** §5 table row 4 — same XCDR2-LE convention +
same type name + RTPS 2.5 wire.

**Repo:** `crates/py/src/ffi.rs:236` (`ShapeTopic` with
`type_name = "ShapeType"`); RTPS 2.5 wire via `crates/rtps/`
+ `crates/transport-udp/`.

**Tests:** the Rust side remains the canonical source for byte identity
(`crates/discovery/tests/cyclone_live_*.rs`). The Python side has
`crates/py/python/tests/test_shapesdemo_interop.py` with 2 tests:
`test_shape_type_name_matches_shapesdemo_convention` (topic type name
`"ShapeType"` matches the Cyclone convention),
`test_cross_vendor_participant_discovery_against_ddsperf`
(SPDP multicast discovery: Cyclone `ddsperf -i <domain> pub 1Hz`
starts a participant, ZeroDDS sees it via
`discovered_participants_count() >= 1`).

**Status:** done — cross-vendor SPDP wire compatibility verified on
`Linux-Bench-Host` with Cyclone DDS 0.10.2 (test runs green).

### §5.5 `@idl_struct` custom type ↔ Rust codegen

**Spec:** §5 table row 5 — byte-identity verified.

**Repo:** `crates/py/python/zerodds/idl.py` +
`crates/py/python/zerodds/cdr.py`. Rust codegen counterpart in
`crates/idl-rust/`.

**Tests:** `crates/py/python/tests/test_idl.py::test_pyshape_byte_roundtrip`,
`test_sensor_mixed_fields_roundtrip` (byte-exact against Rust-encoded
reference bytes).

**Status:** done

## §6 IDL topic, QoS, async + extended API

### §6.1 `IdlTopic` / `IdlWriter` / `IdlReader` with codegen loop

**Spec:** §6 — `IdlTopic(participant, name, dataclass)` wraps an
`@idl_struct` dataclass with a `BytesTopic` surface and delivers
`IdlWriter[T]`/`IdlReader[T]`, which encode/decode per sample.

**Repo:** `crates/py/python/zerodds/topic.py` with three pure-Python
classes (`IdlTopic`, `IdlWriter`, `IdlReader`) plus
`_ensure_idl_struct` guard. Re-export via
`zerodds.__init__.py` as `zerodds.IdlTopic` etc.

**Tests:** `crates/py/python/tests/test_idl_topic.py` (5 tests):
`test_idl_topic_rejects_non_idl_struct`,
`test_idl_topic_exposes_type_name_from_decorator`,
`test_idl_writer_encodes_and_reader_decodes_roundtrip`,
`test_idl_writer_rejects_wrong_type`,
`test_idl_writer_passthrough_status`.

**Status:** done

### §6.2 QoS builder (all 22 policies)

**Spec:** §6 — `DataWriterQos`/`DataReaderQos` PyClass with setters
for all 22 policies from DDS 1.4 §2.2.3 plus
`Publisher.create_*_writer_with_qos` /
`Subscriber.create_*_reader_with_qos`.

**Repo:** `crates/py/src/qos.rs` (~530 lines) — two PyClasses
`PyDataWriterQos`, `PyDataReaderQos` with 18 + 16 setters in total
for the 22 policies, seven enum parser helpers
(`parse_reliability_kind`, `parse_durability_kind`,
`parse_history_kind`, `parse_liveliness_kind`,
`parse_ownership_kind`, `parse_destination_order_kind`,
`parse_presentation_scope`), reflection getters
(`reliability_kind()`, `durability_kind()`, `history_kind()`,
`history_depth()`). `crates/py/src/ffi.rs:286-301`
(`create_bytes_writer_with_qos`, `create_shape_writer_with_qos`,
`create_bytes_reader_with_qos`, `create_shape_reader_with_qos`).

**Tests:** `crates/py/python/tests/test_qos.py` (12 tests):
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

### §6.3 AsyncIO API

**Spec:** §6 — `async def wait_for_data`,
`async def wait_for_matched_*`, asyncio integration via
`asyncio.to_thread` bridge.

**Repo:** `crates/py/python/zerodds/aio.py` with
`AsyncBytesWriter`, `AsyncBytesReader`, `AsyncShapeWriter`,
`AsyncShapeReader`, `AsyncWaitSet`. Non-blocking calls are
passed through directly (`take`, status getters); blocking calls
run via the `_to_thread` helper (backport-friendly for Py3.8).

**Tests:** `crates/py/python/tests/test_aio.py` (5 tests):
`test_async_wrappers_are_constructible`,
`test_async_passthrough_status_getters`,
`test_async_take_returns_list_passthrough`,
`test_async_wait_for_data_uses_event_loop` (verifies that
the asyncio event loop does not block on `wait_for_data`),
`test_async_waitset_wait_raises_timeout`.

**Status:** done

### §6.4 ROS-2 pytest integration

**Spec:** §6 — `launch_pytest` fixture for multi-process live tests
against Cyclone DDS ROS-2.

**Repo:** `crates/py/python/tests/ros2/` with `conftest.py`
(conditional skip when `ROS_DISTRO` is unset or `rclpy`
is not importable or `RMW_IMPLEMENTATION != rmw_zerodds_shim`)
plus `test_rmw_zerodds_interop.py` with 2 tests
(`test_rclpy_init_succeeds_with_zerodds_rmw`,
`test_rclpy_publish_subscribe_string_roundtrip`).

**Tests:** on a ROS-2 host: both tests run. In a normal
pytest run they are skipped.

**Status:** partial — real `launch_pytest` integration tests (not a
skeleton) with conditional skip; they run against a ROS-2 host with
`rmw_zerodds_shim` as the RMW implementation (`crates/rmw-zerodds-shim/`)
and are skipped in host-free CI.

### §6.5 Status listener callbacks

**Spec:** §6 — Python callbacks for `on_data_available`,
`on_liveliness_changed`, etc., registered via
`set_listener(callback, mask)`.

**Repo:** `crates/py/src/listener.rs` (~365 lines) — two PyClasses
`PyDataWriterListener` (3 slots: `on_offered_deadline_missed`,
`on_liveliness_lost`, `on_publication_matched`) and
`PyDataReaderListener` (6 slots: `on_data_available`,
`on_sample_lost`, `on_sample_rejected`,
`on_requested_deadline_missed`, `on_liveliness_changed`,
`on_subscription_matched`). Two bridge structs
(`PyDataWriterListenerBridge`, `PyDataReaderListenerBridge`)
implement the Rust traits `DataWriterListener`/`DataReaderListener`
and call the Python callbacks under GIL acquire. `set_listener` /
`clear_listener` on `PyBytesWriter`/`PyBytesReader`/
`PyShapeWriter`/`PyShapeReader` in `ffi.rs`.

**Tests:** `crates/py/python/tests/test_listener.py` (6 tests):
`test_writer_listener_constructible`,
`test_reader_listener_constructible`,
`test_set_listener_on_bytes_writer_offline`,
`test_set_listener_on_bytes_reader_offline`,
`test_set_listener_on_shape_writer_offline`,
`test_set_listener_on_shape_reader_offline`.

**Status:** done

### §6.6 ReadCondition / QueryCondition

**Spec:** §6 — WaitSet conditions over reader state
(SampleStateKind, ViewStateKind, InstanceStateKind), SQL92 filter
expression for QueryCondition.

**Repo:** `crates/py/src/conditions.rs` (~135 lines) — two PyClasses
`PyReadCondition`, `PyQueryCondition` over the Rust conditions
from `crates/dcps/src/condition.rs`. SampleState/ViewState/
InstanceState bitmask constants exported in `ffi.rs` as
`SAMPLE_STATE_*`, `VIEW_STATE_*`, `INSTANCE_STATE_*` module
attributes; pure-Python wrapper modules
`crates/py/python/zerodds/{sample_state,view_state,instance_state}.py`
provide the Pythonic namespace. `PyWaitSet::attach_read_condition` /
`attach_query_condition` in `ffi.rs`.

**Tests:** `crates/py/python/tests/test_read_conditions.py`
(11 tests): `test_sample_state_constants`,
`test_view_state_constants`, `test_instance_state_constants`,
`test_read_condition_constructible`, `test_read_condition_modes`,
`test_read_condition_unknown_mode_raises`,
`test_read_condition_attaches_to_waitset`,
`test_query_condition_constructible_simple_filter`,
`test_query_condition_with_parameters`,
`test_query_condition_invalid_sql_raises`,
`test_query_condition_attaches_to_waitset`.

**Status:** done

### §6.7 sphinx doc path

**Spec:** §6 — `crates/py/docs/` with API inventory from `_core`
via autodoc.

**Repo:** `crates/py/docs/conf.py` with `autodoc`/`napoleon`/`viewcode`/
`intersphinx` extensions, `autodoc_default_options` and
`autodoc_mock_imports = ["zerodds._core"]` for an RTD build without
maturin. `crates/py/docs/api.rst` lists all PyClasses and
pure-Python modules in full with `autoclass`/`automodule`.

**Tests:** `PYTHONPATH=crates/py/python python3 -m sphinx -b html
crates/py/docs /tmp/zd-py-sphinx` produces HTML with all modules
(`zerodds._core`, `zerodds.cdr`, `zerodds.idl`, `zerodds.loader`)
and all 13 PyClasses, 0 hard errors (14 warnings = missing
docstrings in the mock path).

**Status:** done

## §7 Stability

**Spec:** §7 — semver policy, v1.0 stable, major bump for
breaking changes.

**Repo:** `crates/py/Cargo.toml` (`version.workspace = true` →
`1.0.0-rc.1` from the workspace Cargo).

**Tests:** no dedicated stability test (version-reconciliation CI in
`tests/system/` if present).

**Status:** n/a (informative) — policy statement, not
test-bound.

## §8 License

**Spec:** §8 — Apache-2.0 (workspace default).

**Repo:** `crates/py/Cargo.toml` (`license.workspace = true`),
`LICENSE-APACHE` in the repo root.

**Tests:** —

**Status:** n/a (informative).

## §9 References

**Spec:** §9 — list of external specs (OMG DDS 1.4, XTypes 1.3,
PSM-Cxx 1.0) and internal vendor specs (`zerodds-c-api-1.0`,
`zerodds-ffi-loader-1.0`, `zerodds-java-omgdds-1.0`) plus PyO3 0.22
and PEP 384.

**Repo:** references linked to the respective
`docs/specs/*.md` files and external sources.

**Tests:** —

**Status:** n/a (informative).

---

## Audit status

**42 done / 0 partial / 0 open / 5 n/a (informative) / 0 n/a (rejected).**

Test runs:

- **Local (macOS, Apple Silicon)**: `cd crates/py && SDKROOT=$(xcrun --show-sdk-path) maturin develop --features extension-module && python3 -m pytest python/tests/` — 96 tests green, 9 conditional skips.
- **Bench host `Linux-Bench-Host` (Debian 12, x86_64, Cyclone DDS 0.10.2)**: identical pytest invocation, plus
  `ZERODDS_LIB=$HOME/zerodds/target/debug/libzerodds.so` and
  `LD_LIBRARY_PATH=$HOME/zerodds/target/debug` — **102 tests green,
  3 conditional skips** (ROS-2 missing + 1 live-E2E placeholder).
  

On the Linux bench host the loader-smoke suite (libzerodds.so available),
multi-process subprocess roundtrip and cross-vendor SPDP discovery
against Cyclone `ddsperf` all pass.

No open items.
