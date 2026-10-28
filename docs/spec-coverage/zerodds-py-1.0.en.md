# `zerodds-py` v1.0 — Spec Coverage

Audit of the vendor spec `docs/specs/zerodds-py-1.0.md` against the tree
as it stands.

**Source:** `docs/specs/zerodds-py-1.0.md` (vendor spec).

`crates/py/` is a PyO3 bridge: the DCPS entities from `crates/dcps/` as
the extension module `zerodds._core`, with a Python package above it that
carries `IdlTopic` over generated code and the `ctypes` loader against
`libzerodds`. Topic types have one codec: the code `zerodds-idlc
generate --lang python` emits (the Python backend of `crates/lang-rules`)
over the wire runtime in `runtime/python/zerodds_wire/`. Without
`--features extension-module` the crate is only the placeholder lib;
maturin builds the module.

Beside it stands the XRCE endpoint SDK in
`endpoints/python/zerodds_endpoint.py`, which this spec does not
describe.

Implementation:

- `crates/py/` — PyO3 bridge (`src/`) + Python package
  (`python/zerodds/`) + test tree (`python/tests/`) + examples + sphinx
  tree.

---

## §1 Architecture

### §1.1 Module Layout

**Spec:** §1.1 — crate layout with `Cargo.toml` (crate-type `cdylib`+`rlib`),
`pyproject.toml` for maturin, `src/lib.rs`+`ffi.rs`,
`python/zerodds/{__init__,topic,aio,loader}.py`, `python/tests/` with
`_generated/`, `examples/`, `docs/` sphinx, and `runtime/python/`.

**Repo:** `crates/py/Cargo.toml` (`crate-type = ["cdylib", "rlib"]`,
feature `extension-module`), `crates/py/pyproject.toml`
(`build-backend = "maturin"`, `python-source = "python"`,
`module-name = "zerodds._core"`), `crates/py/build.rs`
(`-undefined dynamic_lookup` on Darwin), `crates/py/src/lib.rs`,
`crates/py/src/ffi.rs`, `crates/py/python/zerodds/__init__.py`,
`crates/py/python/zerodds/topic.py`, `crates/py/python/zerodds/aio.py`,
`crates/py/python/zerodds/loader.py`, `crates/py/python/tests/`
(fixtures: `tests/_generated/*.idl` and their `zerodds-idlc` output),
`crates/py/examples/`, `crates/py/docs/conf.py`,
`runtime/python/zerodds_wire/`.

`maturin build` turns that into an abi3 wheel holding `_core.abi3.so`
and the Python modules.

**Tests:** `crates/py/python/tests/test_docs.py::test_examples_parse`,
`crates/py/python/tests/test_docs.py::test_literalincludes_point_at_existing_examples`,
`crates/py/python/tests/test_smoke.py::test_version_exposed`

**Status:** done

### §1.2 PyO3 module `zerodds._core` — PyClasses

One item per PyClass. Mapping from DDS 1.4 §2.2.2.

#### §1.2.1 `DomainParticipantFactory` (PyClass)

**Spec:** §1.2 + §2.1 — PyClass `DomainParticipantFactory` with
`instance()`, `create_participant(domain_id)`,
`create_participant_offline(domain_id)`,
`create_participant_fast(domain_id)`.

**Repo:** `crates/py/src/ffi.rs` (`PyFactory`) — `instance` as a
`#[staticmethod]`, the three constructors over
`zerodds_dcps::DomainParticipantFactory`; `create_participant_fast`
sets `RuntimeConfig { tick_period: 20 ms, spdp_period: 100 ms }`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_factory_singleton_and_offline_participant`,
`crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`,
`crates/py/python/tests/test_loader_smoke.py::test_loader_factory_singleton`

**Status:** done

#### §1.2.2 `DomainParticipant` (PyClass)

**Spec:** §1.2 + §2.2 — PyClass `DomainParticipant` with `domain_id`,
`topics_len`, `discovered_participants_count`,
`create_{bytes,shape}_topic`, `create_{publisher,subscriber}`,
`assert_liveliness`, `ignore_{participant,topic,publication,subscription}`,
`contains_entity`, `get_discovered_{topics,participants}`.

**Repo:** `crates/py/src/ffi.rs` (`PyParticipant`) — plus
`create_keyed_topic` and `create_bytes_contentfilteredtopic`; the four
`ignore_*` and `contains_entity` take the `u64` of an
`InstanceHandle`.

**Tests:** `crates/py/python/tests/test_participant_ignore.py::test_ignore_participant_unknown_handle_is_silent`,
`crates/py/python/tests/test_participant_ignore.py::test_contains_entity_false_for_unknown_handle`,
`crates/py/python/tests/test_participant_ignore.py::test_get_discovered_participants_offline_is_empty`,
`crates/py/python/tests/test_participant_ignore.py::test_assert_liveliness_offline_does_not_raise`

**Status:** done

#### §1.2.3 `BytesTopic` (PyClass)

**Spec:** §1.2 — PyClass `BytesTopic` with `name`, `type_name`.

**Repo:** `crates/py/src/ffi.rs` (`PyBytesTopic`) over
`Topic<RawBytes>`; `type_name` reads
`<RawBytes as DdsType>::TYPE_NAME` and is therefore bound to the Rust
side rather than written a second time.

**Tests:** `crates/py/python/tests/test_smoke.py::test_bytes_topic_and_reader_are_creatable_offline`

**Status:** done

#### §1.2.4 `ShapeTopic` (PyClass)

**Spec:** §1.2 + §2.7 — PyClass `ShapeTopic` with `name`, `type_name`,
type name = `"ShapeType"`.

**Repo:** `crates/py/src/ffi.rs` (`PyShapeTopic`) over
`Topic<ShapeType>`; `type_name` reads
`<ShapeType as DdsType>::TYPE_NAME` from `crates/dcps/src/interop.rs`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_shape_topic_matches_vendor_interop_type_name`,
`crates/py/python/tests/test_shapesdemo_interop.py::test_shape_type_name_matches_shapesdemo_convention`

**Status:** done

#### §1.2.5 `Publisher` (PyClass)

**Spec:** §1.2 + §2.3 — PyClass `Publisher` with
`create_bytes_writer(topic)`, `create_shape_writer(topic)`.

**Repo:** `crates/py/src/ffi.rs` (`PyPublisher`) — plus the
`*_with_qos` variants from §6.2 and `create_keyed_writer`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`,
`crates/py/python/tests/test_qos.py::test_create_bytes_writer_with_qos_offline`,
`crates/py/python/tests/test_qos.py::test_create_shape_writer_with_qos_offline`

**Status:** done

#### §1.2.6 `Subscriber` (PyClass)

**Spec:** §1.2 + §2.3 — PyClass `Subscriber` with
`create_bytes_reader(topic)`, `create_shape_reader(topic)`.

**Repo:** `crates/py/src/ffi.rs` (`PySubscriber`) — plus the
`*_with_qos` variants, `create_keyed_reader` and
`create_bytes_reader_cft` for the content filter of §2.2.2.5.4.

**Tests:** `crates/py/python/tests/test_smoke.py::test_bytes_topic_and_reader_are_creatable_offline`,
`crates/py/python/tests/test_qos.py::test_create_bytes_reader_with_qos_offline`,
`crates/py/python/tests/test_qos_lifecycle_surface.py::test_content_filtered_topic_filters_samples`

**Status:** done

#### §1.2.7 `BytesWriter` (PyClass)

**Spec:** §1.2 + §2.4 — PyClass `BytesWriter` with `write`,
`wait_for_matched_subscription`, `matched_subscription_count`,
`publication_matched_status`, `liveliness_lost_status`,
`offered_deadline_missed_status`.

**Repo:** `crates/py/src/ffi.rs` (`PyBytesWriter`) — plus
`register_instance`, `lookup_instance`, `dispose`,
`unregister_instance` and `set_listener`/`clear_listener`. `write` and
the three wait functions release the GIL through `py.detach`.

**Tests:** `crates/py/python/tests/test_status_getters.py::test_publication_matched_status_shape`,
`crates/py/python/tests/test_status_getters.py::test_liveliness_lost_status_shape`,
`crates/py/python/tests/test_status_getters.py::test_offered_deadline_missed_status_shape`,
`crates/py/python/tests/test_qos_lifecycle_surface.py::test_bytes_writer_lifecycle_methods_present`

**Status:** done

#### §1.2.8 `BytesReader` (PyClass)

**Spec:** §1.2 + §2.5 — PyClass `BytesReader` with `take`,
`wait_for_data`, `wait_for_matched_publication`,
`matched_publication_count`, `subscription_matched_status`,
`sample_lost_status`, `requested_deadline_missed_status`.

**Repo:** `crates/py/src/ffi.rs` (`PyBytesReader`) — plus
`take_with_info` / `read_with_info`, which yield `(bytes, SampleInfo)`
pairs, `liveliness_changed_status` and
`set_listener`/`clear_listener`.

**Tests:** `crates/py/python/tests/test_status_getters.py::test_subscription_matched_status_shape`,
`crates/py/python/tests/test_status_getters.py::test_sample_lost_status_shape`,
`crates/py/python/tests/test_status_getters.py::test_requested_deadline_missed_status_shape`,
`crates/py/python/tests/test_qos_lifecycle_surface.py::test_bytes_take_with_info_returns_sample_info`

**Status:** done

#### §1.2.9 `Shape` (PyClass / dataclass)

**Spec:** §1.2 + §2.7 — PyClass `Shape` with fields `color`, `x`, `y`,
`shapesize` (default 30), `__repr__`, conversions `From<&PyShape>` and
`From<ShapeType>` in both directions.

**Repo:** `crates/py/src/ffi.rs` (`PyShape`) with
`#[pyo3(signature = (color, x, y, shapesize=30))]` and the two `From`
implementations against `zerodds_dcps::interop::ShapeType`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_shape_constructor_and_repr`,
`crates/py/python/tests/test_smoke.py::test_shape_default_shapesize_is_30`

**Status:** done

#### §1.2.10 `ShapeWriter` (PyClass)

**Spec:** §1.2 + §2.4 — PyClass `ShapeWriter` with `write(shape)`,
`register_instance(shape)`, `dispose(shape)`,
`unregister_instance(shape)`, `wait_for_matched_subscription`.

**Repo:** `crates/py/src/ffi.rs` (`PyShapeWriter`) — `dispose` and
`unregister_instance` register the instance when `lookup_instance`
returns nil, so a valid handle is always at hand (spec §2.2.2.4.2.4).

**Tests:** `crates/py/python/tests/test_writer_lifecycle.py::test_register_instance_returns_nonzero_handle`,
`crates/py/python/tests/test_writer_lifecycle.py::test_register_instance_is_idempotent_for_same_key`,
`crates/py/python/tests/test_writer_lifecycle.py::test_lifecycle_full_sequence`

**Status:** done

#### §1.2.11 `ShapeReader` (PyClass)

**Spec:** §1.2 + §2.5 — PyClass `ShapeReader` with `take`,
`wait_for_data`, `wait_for_matched_publication`.

**Repo:** `crates/py/src/ffi.rs` (`PyShapeReader`) — plus
`take_with_info` / `read_with_info` and
`liveliness_changed_status`.

**Tests:** `crates/py/python/tests/test_status_getters.py::test_matched_publication_count_zero_offline`,
`crates/py/python/tests/test_listener.py::test_set_listener_on_shape_reader_offline`

**Status:** done

#### §1.2.12 `GuardCondition` (PyClass)

**Spec:** §1.2 + §2.6 — PyClass `GuardCondition` with constructor,
`set_trigger_value(bool)`, `get_trigger_value() -> bool`.

**Repo:** `crates/py/src/ffi.rs` (`PyGuardCondition`) over
`zerodds_dcps::condition::GuardCondition`.

**Tests:** `crates/py/python/tests/test_conditions.py::test_guard_condition_default_trigger_is_false`,
`crates/py/python/tests/test_conditions.py::test_guard_condition_set_trigger_roundtrip`,
`crates/py/python/tests/test_conditions.py::test_guard_condition_via_outer_namespace`

**Status:** done

#### §1.2.13 `WaitSet` (PyClass)

**Spec:** §1.2 + §2.6 — PyClass `WaitSet` with constructor,
`attach_guard_condition(gc)`, `wait(timeout_secs) -> int`.

**Repo:** `crates/py/src/ffi.rs` (`PyWaitSet`) over
`zerodds_dcps::condition::WaitSet`; `wait` releases the GIL and raises
`TimeoutError` when nothing triggers. `attach_read_condition` and
`attach_query_condition` come with §6.6.

**Tests:** `crates/py/python/tests/test_conditions.py::test_waitset_attach_and_wait_raises_timeout`,
`crates/py/python/tests/test_conditions.py::test_waitset_wakes_when_guard_condition_fires`,
`crates/py/python/tests/test_conditions.py::test_waitset_via_outer_namespace`

**Status:** done

### §1.3 Python wrapper `python/zerodds/`

#### §1.3.1 `__init__.py` — re-export

**Spec:** §1.3 — `zerodds.__init__` re-exports the `_core` PyClasses
and `IdlTopic`/`IdlWriter`/`IdlReader` from `topic.py`.

**Repo:** `crates/py/python/zerodds/__init__.py` — re-exports the
PyClasses along with `__version__`, the modules `sample_state`,
`view_state`, `instance_state` and `IdlTopic`/`IdlWriter`/`IdlReader`
from `topic.py`. When
the extension module is absent, `_CORE_AVAILABLE` is `False` and the
PyClasses become placeholders that raise an `ImportError` naming the
maturin line on instantiation — the pure-Python modules stay usable
with no Rust build.

**Tests:** `crates/py/python/tests/test_smoke.py::test_version_exposed`,
`crates/py/python/tests/test_conditions.py::test_guard_condition_via_outer_namespace`,
`crates/py/python/tests/test_docs.py::test_api_rst_documents_every_extension_class`

**Status:** done

#### §1.3.2 `topic.py` — `IdlTopic` over a generated `TopicDescriptor`

**Spec:** §1.3 + §6.1 — `IdlTopic(participant, name, descriptor)`
takes only a `zerodds_wire.topic_descriptor.TopicDescriptor` and creates
the topic with the type's CompleteTypeObject, key and extensibility;
`IdlWriter`/`IdlReader` encode and decode with the generated code.

**Repo:** `crates/py/python/zerodds/topic.py` (`IdlTopic`, `IdlWriter`,
`IdlReader`) over `DomainParticipant.create_descriptor_topic` and the
`DescriptorTopic`/`DescriptorWriter`/`DescriptorReader` PyClasses in
`crates/py/src/ffi.rs`.

**Tests:** `crates/py/python/tests/test_idl_topic.py::test_idl_topic_rejects_a_non_topic_descriptor`,
`::test_idl_topic_exposes_type_name_from_the_descriptor`,
`::test_idl_writer_encodes_and_reader_decodes_roundtrip`,
`::test_idl_writer_rejects_wrong_type`,
`::test_idl_writer_passthrough_status`

**Status:** done

#### §1.3.3 `runtime/python` — `zerodds_wire`

**Spec:** §1 + §3 — the wire runtime the generated code emits onto.

**Repo:** `runtime/python/zerodds_wire/` (`Writer`, `Reader`,
`TopicDescriptor`, `OutOfRange`, encapsulation and endianness) —
alignment at `min(N, MAXALIGN)` with MAXALIGN four under XCDR2 and eight
under XCDR1, DHEADER and EMHEADER framing, PL_CDR1 parameters.

**Tests:** `runtime/python/tests/test_wire.py`,
`crates/py/python/tests/test_idl.py::test_wire_primitive_roundtrip`,
`::test_wire_string_alignment_padding`,
`::test_wire_reader_rejects_truncated_string`,
`::test_xcdr2_max_align_caps_double_at_four`

**Status:** done

#### §1.3.4 `loader.py` — pure `ctypes` loader

**Spec:** §1.3 + §1.4 — pure `ctypes` loader against
`libzerodds.{so,dylib,dll}` from `crates/zerodds-c-api/`. Follows
`zerodds-ffi-loader-1.0` §3.1. Signatures from
`crates/zerodds-c-api/include/zerodds.h`.

**Repo:** `crates/py/python/zerodds/loader.py` (`load_library`,
`Runtime`, `Writer`, `Reader`, `DomainParticipantFactory`,
`ZeroDdsError`) — resolution over `ZERODDS_LIB`, the wheel-internal
`_lib/`, `target/{debug,release}` in a development tree, and finally
the system linker.

**Tests:** `crates/py/python/tests/test_loader_smoke.py::test_loader_resolves_library`,
`crates/py/python/tests/test_loader_smoke.py::test_loader_runtime_create_participant_offline`,
`crates/py/python/tests/test_loader_smoke.py::test_loader_zerodds_error_is_runtime_error_subclass`

The four tests skip when no `libzerodds` is found
(`cargo build -p zerodds-c-api` or `ZERODDS_LIB`).

**Status:** done

### §1.4 Two-path choice

**Spec:** §1.4 — two independent paths: PyO3 (performance, RTPS-native)
and pure `ctypes` (distro package, zero build).

**Repo:** `crates/py/src/ffi.rs` for the PyO3 path,
`crates/py/python/zerodds/loader.py` for the `ctypes` path. The two
share only the wire format and the domain id: `loader.py` does not
import `_core` and runs without a Rust toolchain, and `ffi.rs` knows
nothing of `libzerodds`.

**Tests:** `crates/py/python/tests/test_loader_smoke.py::test_loader_factory_singleton`,
`crates/py/python/tests/test_smoke.py::test_factory_singleton_and_offline_participant`

**Status:** done

### §1.5 Layer position

**Spec:** §1.5 — layer 6 (PSMs / bindings). Direct dependencies:
`zerodds-dcps` (layer 4), `zerodds-c-api` (layer 6 C-FFI).

**Repo:** `crates/py/Cargo.toml` — the Rust dependencies are
`zerodds-dcps`, `zerodds-qos` and `zerodds-wire`;
`crates/zerodds-c-api/` is reached at runtime by the `ctypes` path
through the loader, with no manifest edge. `crates/py/src/lib.rs`
names the layer in the crate doc.

**Tests:** `crates/py/src/lib.rs::dcps_re_export_compiles`

**Status:** done

## §2 OMG API coverage (DDS 1.4 §2.2.2)

### §2.1 DomainParticipantFactory (DDS 1.4 §2.2.2.2.1)

**Spec:** §2.1 — mapping table: `get_instance()`, `create_participant`,
`create_participant_offline`, implicit `delete_participant` via the
Python GC.

**Repo:** `crates/py/src/ffi.rs` (`PyFactory`) —
`DomainParticipantFactory.instance()` as a `#[staticmethod]`,
`create_participant` and `create_participant_offline` each onto the
Rust operation of the same name. Deletion hangs off the Python GC:
`PyParticipant` holds the `DomainParticipant`, whose `Drop` tears the
Rust side down.

**Tests:** `crates/py/python/tests/test_smoke.py::test_factory_singleton_and_offline_participant`,
`crates/py/python/tests/test_multiproc.py::test_multiproc_python_python_roundtrip`

**Status:** done

### §2.2 DomainParticipant (DDS 1.4 §2.2.2.2.2)

**Spec:** §2.2 — twelve operations: `get_domain_id`, `create_topic`,
`create_publisher`, `create_subscriber`, `assert_liveliness`,
`ignore_{participant,topic,publication,subscription}`,
`contains_entity`, `get_discovered_{topics,participants}`.

**Repo:** `crates/py/src/ffi.rs` (`PyParticipant`) — all twelve.
`get_domain_id` is the `domain_id` property, and `create_topic` is the
three typed constructors `create_bytes_topic`, `create_shape_topic`
and `create_keyed_topic`.

**Tests:** `crates/py/python/tests/test_participant_ignore.py::test_ignore_topic_unknown_handle_is_silent`,
`crates/py/python/tests/test_participant_ignore.py::test_ignore_publication_unknown_handle_is_silent`,
`crates/py/python/tests/test_participant_ignore.py::test_ignore_subscription_unknown_handle_is_silent`,
`crates/py/python/tests/test_participant_ignore.py::test_get_discovered_topics_offline_is_empty`

**Status:** done

### §2.3 Publisher / Subscriber (DDS 1.4 §2.2.2.4.1 / §2.2.2.5.1)

**Spec:** §2.3 — `Publisher.create_datawriter` →
`pub.create_{bytes,shape}_writer(topic)`,
`Subscriber.create_datareader` →
`sub.create_{bytes,shape}_reader(topic)`.

**Repo:** `crates/py/src/ffi.rs` (`PyPublisher`, `PySubscriber`) — one
constructor per topic type, each with a `*_with_qos` variant.

**Tests:** `crates/py/python/tests/test_qos.py::test_create_shape_reader_with_qos_offline`,
`crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`

**Status:** done

### §2.4 DataWriter (DDS 1.4 §2.2.2.4.2)

**Spec:** §2.4 — mapping table: `write`, `register_instance`,
`unregister_instance`, `dispose`, `get_matched_subscriptions`,
`get_publication_matched_status`, `get_liveliness_lost_status`,
`get_offered_deadline_missed_status`, plus the
`wait_for_matched_subscription` sync helper.

**Repo:** `crates/py/src/ffi.rs` (`PyBytesWriter`, `PyShapeWriter`,
`PyKeyedWriter`) — `get_matched_subscriptions` is
`matched_subscription_count`, and the three status getters return the
tuples of the spec's structures.

**Tests:** `crates/py/python/tests/test_writer_lifecycle.py::test_dispose_does_not_raise`,
`crates/py/python/tests/test_writer_lifecycle.py::test_unregister_instance_does_not_raise`,
`crates/py/python/tests/test_status_getters.py::test_matched_subscription_count_zero_offline`,
`crates/py/python/tests/test_qos_lifecycle_surface.py::test_keyed_register_instance_returns_real_handle`

**Status:** done

### §2.5 DataReader (DDS 1.4 §2.2.2.5.3)

**Spec:** §2.5 — `take`, `wait_for_data`,
`wait_for_matched_publication`, `matched_publication_count`,
`subscription_matched_status`, `sample_lost_status`,
`requested_deadline_missed_status`. `read` and
`wait_for_historical_data` deliberately omitted.

**Repo:** `crates/py/src/ffi.rs` (`PyBytesReader`, `PyShapeReader`,
`PyKeyedReader`). Beyond the spec's list there are `read_with_info`
and `take_with_info`, which carry the `SampleInfo` per sample — `read`
without a `SampleInfo` stays out, as the spec says.

**Tests:** `crates/py/python/tests/test_qos_lifecycle_surface.py::test_bytes_take_with_info_returns_sample_info`,
`crates/py/python/tests/test_status_getters.py::test_subscription_matched_status_shape`,
`crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`

**Status:** done

### §2.6 WaitSet / conditions (DDS 1.4 §2.2.2.6)

**Spec:** §2.6 — `WaitSet`, `GuardCondition` with
`attach_guard_condition`, `wait`, `set/get_trigger_value`.
`ReadCondition`/`QueryCondition` are available through §6.6.

**Repo:** `crates/py/src/ffi.rs` (`PyGuardCondition`, `PyWaitSet`),
`crates/py/src/conditions.rs` for the two reader conditions.

The spec says in §2.6 that the outer `zerodds` namespace does not
re-export `WaitSet` and `GuardCondition` yet, and lists both in §1.3's
re-export block. `crates/py/python/zerodds/__init__.py` follows §1.3:
both are reachable through `zerodds.` and through `zerodds._core.`,
which includes the §2.6 access.

**Tests:** `crates/py/python/tests/test_conditions.py::test_waitset_wakes_when_guard_condition_fires`,
`crates/py/python/tests/test_conditions.py::test_waitset_via_outer_namespace`,
`crates/py/python/tests/test_read_conditions.py::test_read_condition_attaches_to_waitset`

**Status:** done

### §2.7 ShapeType — cross-vendor interop type

**Spec:** §2.7 — the `Shape` PyClass is octet-identical to `ShapeType`
in `crates/dcps/src/interop.rs`. Fields `color: String, x: i32,
y: i32, shapesize: i32` (default 30). Type name on the wire =
`"ShapeType"`.

**Repo:** `crates/py/src/ffi.rs` (`PyShape`, `shape_reference_octets`)
— the conversion goes through `zerodds_dcps::interop::ShapeType`
itself, so there is no second codec on the Rust side.
`shape_reference_octets` returns that codec's octets to Python, so the
generated Python code can be measured against it.

**Tests:** `crates/py/python/tests/test_idl.py::test_shapetype_matches_the_rust_codec_octet_for_octet`,
`crates/py/python/tests/test_smoke.py::test_shape_topic_matches_vendor_interop_type_name`,
`crates/py/python/tests/test_shapesdemo_interop.py::test_shape_type_name_matches_shapesdemo_convention`

**Status:** done

## §3 IDL mapping — generated code over `zerodds_wire`

### §3.1 Mapping

**Spec:** §3.1 — `zerodds-idlc generate --lang python` emits one
dataclass per type, its encode/decode functions and
`ZD_TOPIC_DESCRIPTORS`; the mapping table (integers, floats, strings,
`sequence`, `T[N]`, `map`, `@optional`, `enum`, `union`, `struct`); a
bounded value over its bound is refused with `OutOfRange` before a byte
is written; decode applies `@try_construct` (XTypes 1.3
§7.2.2.4.4.4.9).

**Repo:** `crates/lang-rules/src/adapter/python.rs` (the Python
mapping), `crates/lang-rules/src/emit/encode.rs` (the shared codec walk,
including the bound checks), `tools/idlc` (`generate --lang python`),
`runtime/python/zerodds_wire/`.

**Tests:** `crates/py/python/tests/test_idl.py::test_sensor_mixed_fields_roundtrip`,
`::test_optional_present_and_absent`, `::test_enum_roundtrip`,
`::test_enum_unknown_value_raises`, `::test_union_case_int_roundtrip`,
`::test_union_default_branch_used_for_unknown_disc`,
`::test_nested_struct_roundtrip`, `::test_fixed_array_roundtrip`,
`::test_generated_bounds_roundtrip_and_enforced`

**Status:** partial — the element count of a fixed array is not checked
on encode: too many elements are cut, too few raise `IndexError`
(`test_fixed_array_count_is_refused_on_encode`, `xfail(strict=True)`).

### §3.2 Example

**Spec:** §3.2 — IDL, generated module, `IdlTopic` over its descriptor.

**Repo:** `crates/py/examples/03_idl_topic.idl`,
`crates/py/examples/03_idl_topic_gen.py` (the `zerodds-idlc` output),
`crates/py/examples/03_idl_topic.py`.

**Tests:** `crates/py/python/tests/test_docs.py::test_examples_parse`,
`crates/py/python/tests/test_idl.py::test_shapetype_matches_the_rust_codec_octet_for_octet`

**Status:** done

## §4 Test obligation

### §4.1 Test inventory

**Spec:** §4 — pytest over `crates/py/python/tests/`, fixtures in
`tests/_generated/` as IDL plus their unchanged `zerodds-idlc` output,
the runtime's own tests in `runtime/python/tests/`.

**Repo:** seventeen files in `crates/py/python/tests/` — among them
`test_smoke.py` (seven tests), `test_idl.py` (forty-four test
functions over the generated fixtures), `test_idl_topic.py`,
`test_qos.py`, `test_qos_lifecycle_surface.py`, `test_status_getters.py`,
`test_writer_lifecycle.py`, `test_participant_ignore.py`,
`test_conditions.py`, `test_read_conditions.py`, `test_listener.py`,
`test_aio.py`, `test_loader_smoke.py`,
`test_loader_declares_extensibility.py`, `test_multiproc.py`,
`test_shapesdemo_interop.py`, `test_docs.py` — plus the ROS-2 tree of
§6.4 and `runtime/python/tests/test_wire.py`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_version_exposed`,
`::test_factory_singleton_and_offline_participant`,
`::test_bytes_topic_and_reader_are_creatable_offline`,
`::test_shape_topic_matches_vendor_interop_type_name`,
`::test_shape_constructor_and_repr`, `::test_shape_default_shapesize_is_30`,
`::test_pub_sub_roundtrip_live`; the test functions of `test_idl.py`.

**Status:** done

### §4.2 Rust-side test inventory

**Spec:** §4 — no Rust test inventory, because without
`--features extension-module` the crate is a placeholder lib with no
API; spec compliance is verified on the pytest side.

**Repo:** `crates/py/src/lib.rs` — the four modules `ffi`, `qos`,
`listener` and `conditions` hang off
`#[cfg(feature = "extension-module")]`. There is no `tests/`
directory in the crate; the only Rust test is
`dcps_re_export_compiles`, which checks that `zerodds-dcps` is
reachable as a dependency. Every statement this coverage makes about
behaviour stands on a pytest case.

**Tests:** `crates/py/src/lib.rs::dcps_re_export_compiles`

**Status:** done

## §5 Multi-process / cross-vendor

### §5.1 Single-process Python ↔ Python

**Spec:** §5 table row 1 — single-process Python ↔ Python over
`zerodds._core` and `crates/dcps`.

**Repo:** `crates/py/src/ffi.rs` (`PyFactory::create_participant_fast`)
— two participants on the same domain in one process; the matching
runs over the in-process discovery fastpath from `crates/dcps`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`,
`crates/py/python/tests/test_qos_lifecycle_surface.py::test_bytes_take_with_info_returns_sample_info`,
`crates/py/python/tests/test_qos_lifecycle_surface.py::test_content_filtered_topic_filters_samples`

**Status:** done

### §5.2 Multi-process Python ↔ Python

**Spec:** §5 table row 2 — multi-process locally over RTPS/UDP +
SPDP/SEDP.

**Repo:** `crates/py/python/tests/test_multiproc.py` — two `python -c`
subprocesses, one `BytesTopic`, one payload; the subscriber exits 0
with `OK` as soon as it has it.

**Tests:** `crates/py/python/tests/test_multiproc.py::test_multiproc_python_python_roundtrip`

**Status:** done

### §5.3 Cross-vendor Python ↔ other languages

**Spec:** §5 table row 3 — the ShapeType wire is octet-identical to
C++/Rust/Java/C# peers over the shared topic type `"ShapeType"`.

**Repo:** `crates/py/python/tests/_generated/shape.idl` and its
generated module `shape_gen.py` +
`crates/py/src/ffi.rs::shape_reference_octets` (returns what
`zerodds_dcps::interop::ShapeType::encode` writes) — the reference the
generated Python codec is held against octet for octet.

**Tests:** `crates/py/python/tests/test_idl.py::test_shapetype_matches_the_rust_codec_octet_for_octet`
(7 parameter sets; each case compares the octets written with the Rust
reference from the same process),
`crates/py/python/tests/test_idl.py::test_shapetype_byte_roundtrip`,
`crates/py/python/tests/test_shapesdemo_interop.py::test_shape_type_name_matches_shapesdemo_convention`
(the topic type name `"ShapeType"`). No gate; all three run in the default
pytest run.

**Status:** partial — the Python ↔ Rust octet identity is measured in the
tree. For C++, Java and C# there is no measurement in the repo: the
interop matrix that carries those columns lives on the bench host and is
not part of this tree. Tracked as §5.3b.

### §5.3b Octet identity Python ↔ C++/Java/C# in the tree

**Spec:** §5 table row 3 — the ShapeType wire is octet-identical to
C++/Java/C# peers.

**Repo:** —

**Tests:** —

**Status:** open — no test case in the repo holds the ShapeType octets the
generated Python codec writes against those of the C++, Java or C# binding.
`shape_reference_octets` supplies that reference for Rust only. Plan:
produce a golden vector per binding the way
`endpoints/python/test_byte_identity.py` does for `SensorReading`, for
`ShapeType` too, and hold all four codecs against the same vector.

### §5.4 Cross-vendor Python ↔ Cyclone / Fast DDS ShapesDemo

**Spec:** §5 table row 4 — same XCDR2-LE convention, same type name,
same RTPS 2.5 wire.

**Repo:** `matrix/zerodds-python/cyclone-*` and `matrix/zerodds-python/fastdds-*` — the same measurement against Cyclone and Fast DDS

**Tests:** the cells of those option sets; plus
`crates/py/python/tests/test_shapesdemo_interop.py::test_cross_vendor_participant_discovery_against_ddsperf`,
which measures SPDP discovery against Cyclone's `ddsperf` and skips
where `ddsperf` is not on the path

**Status:** done

### §5.5 Generated type over `IdlTopic` ↔ Rust codegen

**Spec:** §5 table row 5 — octets measured against the Rust reference.

**Repo:** `crates/py/src/ffi.rs` (`shape_reference_octets`) hands the
octets of `zerodds_dcps::interop::ShapeType::encode` to Python; the
module `zerodds-idlc` generates from `tests/_generated/shape.idl`
produces them on the Python side. The comparison is against the codec
compiled into the same module, not against a literal copied out of a
Rust test. `wireprobe-python-matrix` measures the whole Python column
against the Rust one.

**Tests:** `crates/py/python/tests/test_idl.py::test_shapetype_matches_the_rust_codec_octet_for_octet`
(seven shapes, among them an empty and a one-character colour string
and `i32::MAX`/`i32::MIN`),
`crates/py/python/tests/test_idl.py::test_fixed_member_struct_byte_identical_to_rust_cpp`,
`crates/py/python/tests/test_idl.py::test_sensor_mixed_fields_roundtrip`

**Status:** done

## §6 IDL topic, QoS, async + extended API

### §6.1 `IdlTopic` / `IdlWriter` / `IdlReader`

**Spec:** §6.1 — `IdlTopic(participant, name, descriptor)` over a
generated `TopicDescriptor`, yielding `IdlWriter[T]`/`IdlReader[T]`,
which encode and decode with the generated code.

**Repo:** `crates/py/python/zerodds/topic.py` (`IdlTopic`,
`IdlWriter`, `IdlReader`) over `DescriptorTopic`/`DescriptorWriter`/
`DescriptorReader`; the constructor rejects anything but a
`TopicDescriptor`, and `IdlTopic.type_name` comes from the descriptor.

**Tests:** `crates/py/python/tests/test_idl_topic.py::test_idl_topic_rejects_a_non_topic_descriptor`,
`::test_idl_topic_exposes_type_name_from_the_descriptor`,
`::test_idl_writer_encodes_and_reader_decodes_roundtrip`,
`::test_idl_writer_rejects_wrong_type`, `::test_idl_writer_passthrough_status`

**Status:** done

### §6.2 QoS builder (all 22 policies)

**Spec:** §6.2 — `DataWriterQos`/`DataReaderQos` PyClasses with setters
for all twenty-two policies of DDS 1.4 §2.2.3, plus
`Publisher.create_*_writer_with_qos` /
`Subscriber.create_*_reader_with_qos`.

**Repo:** `crates/py/src/qos.rs` (`PyDataWriterQos`,
`PyDataReaderQos`) — setters for Reliability, Durability,
DurabilityService, Deadline, LatencyBudget, Liveliness,
DestinationOrder, Lifespan, Ownership, OwnershipStrength, Partition,
Presentation, History, ResourceLimits, TransportPriority,
WriterDataLifecycle, TimeBasedFilter, ReaderDataLifecycle, UserData,
TopicData, GroupData, each on the side DDS 1.4 §2.2.3 carries it. An
unknown kind string is rejected as a `ValueError` rather than falling
silently onto a default. The `create_*_with_qos` constructors live in
`crates/py/src/ffi.rs`.

**Tests:** `crates/py/python/tests/test_qos.py::test_writer_qos_defaults`,
`crates/py/python/tests/test_qos.py::test_writer_qos_setters_with_durations`,
`crates/py/python/tests/test_qos.py::test_reader_qos_full_setter_chain`,
`crates/py/python/tests/test_qos.py::test_writer_qos_rejects_unknown_reliability_kind`,
`crates/py/python/tests/test_qos.py::test_create_bytes_writer_with_qos_offline`

**Status:** done

### §6.3 AsyncIO API

**Spec:** §6.3 — `zerodds.aio` with `AsyncBytesWriter`,
`AsyncBytesReader`, `AsyncShapeWriter`, `AsyncShapeReader`,
`AsyncWaitSet`; blocking calls over an `asyncio.to_thread` bridge.

**Repo:** `crates/py/python/zerodds/aio.py` — the five wrappers; every
blocking operation runs through `asyncio.to_thread`, and the
non-blocking getters pass straight through.

**Tests:** `crates/py/python/tests/test_aio.py::test_async_wrappers_are_constructible`,
`crates/py/python/tests/test_aio.py::test_async_wait_for_data_uses_event_loop`,
`crates/py/python/tests/test_aio.py::test_async_take_returns_list_passthrough`,
`crates/py/python/tests/test_aio.py::test_async_waitset_wait_raises_timeout`

**Status:** done

### §6.4 ROS-2 pytest integration

**Spec:** §6.4 — test tree `crates/py/python/tests/ros2/` with a
`conftest.py` (skip when `ROS_DISTRO` + `rclpy` +
`RMW_IMPLEMENTATION` are missing) and `test_rmw_zerodds_interop.py`
for standard rclpy publish/subscribe over the ZeroDDS rmw.

**Repo:** `crates/py/python/tests/ros2/conftest.py` — marks only the
cases under `ros2/` as skipped when `ROS_DISTRO`, `ros2` on the path
or `rclpy` are missing, and separately when `RMW_IMPLEMENTATION` does
not name the ZeroDDS rmw;
`crates/py/python/tests/ros2/test_rmw_zerodds_interop.py` — six cases
over rclpy; `crates/rmw-zerodds-shim/rmw_c/run_ros2_pytest.sh` — the
reproducible run that builds the ABI layer, registers it into a ROS 2
installation and runs this tree against it.

The spec names the rmw `rmw_zerodds_shim`; the implementation
identifier per REP-2007 is `rmw_zerodds_cpp`
(`crates/rmw-zerodds-shim/ament/package.xml`), and that is what the
`conftest.py` checks for.

**Tests:** `crates/py/python/tests/ros2/test_rmw_zerodds_interop.py::test_rclpy_init_succeeds_with_zerodds_rmw`,
`crates/py/python/tests/ros2/test_rmw_zerodds_interop.py::test_rclpy_publish_subscribe_string_roundtrip`,
`crates/py/python/tests/ros2/test_rmw_zerodds_interop.py::test_rclpy_service_call_roundtrip`,
`crates/py/python/tests/ros2/test_rmw_zerodds_interop.py::test_rclpy_topic_graph_introspection`,
`crates/py/python/tests/ros2/test_rmw_zerodds_interop.py::test_rclpy_node_names_graph`,
`crates/py/python/tests/ros2/test_rmw_zerodds_interop.py::test_rclpy_endpoint_info_by_topic`

On a host without ROS 2 all six skip with the reason from the
`conftest.py`; they are measured through `run_ros2_pytest.sh` on a
ROS 2 installation, see `docs/spec-coverage/ros2-rmw.en.md`.

**Status:** done

### §6.5 Status listener callbacks

**Spec:** §6.5 — PyClasses `DataWriterListener` (3 slots) and
`DataReaderListener` (6 slots), registered through
`writer.set_listener(listener, mask)` /
`reader.set_listener(listener, mask)`.

**Repo:** `crates/py/src/listener.rs` (`PyDataWriterListener`,
`PyDataReaderListener`, `PyDataWriterListenerBridge`,
`PyDataReaderListenerBridge`) — three writer slots
(`on_offered_deadline_missed`, `on_liveliness_lost`,
`on_publication_matched`) and six reader slots (`on_data_available`,
`on_sample_lost`, `on_sample_rejected`,
`on_requested_deadline_missed`, `on_liveliness_changed`,
`on_subscription_matched`). The bridge takes the GIL before each call;
an exception inside a callback is printed and not propagated into
Rust. `set_listener`/`clear_listener` sit on the four writer and
reader PyClasses in `crates/py/src/ffi.rs`.

**Tests:** `crates/py/python/tests/test_listener.py::test_writer_listener_constructible`,
`crates/py/python/tests/test_listener.py::test_reader_listener_constructible`,
`crates/py/python/tests/test_listener.py::test_set_listener_on_bytes_writer_offline`,
`crates/py/python/tests/test_listener.py::test_set_listener_on_bytes_reader_offline`,
`crates/py/python/tests/test_listener.py::test_set_listener_on_shape_writer_offline`,
`crates/py/python/tests/test_listener.py::test_set_listener_on_shape_reader_offline`

**Status:** done

### §6.6 ReadCondition / QueryCondition

**Spec:** §6.6 — PyClasses `ReadCondition` and `QueryCondition` over
the Rust conditions, bitmask constants as
`zerodds.{sample_state,view_state,instance_state}`, an SQL92 filter
validated in the constructor, `WaitSet.attach_read_condition` /
`attach_query_condition`.

**Repo:** `crates/py/src/conditions.rs` (`PyReadCondition`,
`PyQueryCondition`) over `zerodds_dcps::condition`;
`crates/py/python/zerodds/sample_state.py`,
`crates/py/python/zerodds/view_state.py`,
`crates/py/python/zerodds/instance_state.py` for the bitmasks that
`crates/py/src/ffi.rs` sets as module attributes out of
`zerodds_dcps::sample_info`. An invalid SQL expression is rejected in
the constructor as a `RuntimeError`.

**Tests:** `crates/py/python/tests/test_read_conditions.py::test_sample_state_constants`,
`crates/py/python/tests/test_read_conditions.py::test_read_condition_modes`,
`crates/py/python/tests/test_read_conditions.py::test_query_condition_with_parameters`,
`crates/py/python/tests/test_read_conditions.py::test_query_condition_invalid_sql_raises`,
`crates/py/python/tests/test_read_conditions.py::test_query_condition_attaches_to_waitset`

**Status:** done

### §6.7 sphinx doc path

**Spec:** §6.7 — `crates/py/docs/conf.py` with autodoc/napoleon/
viewcode/intersphinx, `autodoc_mock_imports` for the build without
maturin, and `crates/py/docs/api.rst` listing the PyClasses and the
modules `loader`, `topic`, `aio`, `sample_state`, `view_state`,
`instance_state`.

**Repo:** `crates/py/docs/conf.py` — the four extensions,
`autodoc_mock_imports = ["zerodds._core"]` and the version read from
the workspace manifest rather than repeated as a literal;
`crates/py/docs/api.rst` — every class the extension module exports and
the modules `loader`, `topic`, `aio` and the three state modules;
`crates/py/docs/index.rst`, `crates/py/docs/quickstart.rst`,
`crates/py/docs/examples.rst`.

**Tests:** `crates/py/python/tests/test_docs.py::test_conf_py_declares_the_four_extensions_the_spec_names`,
`crates/py/python/tests/test_docs.py::test_api_rst_documents_every_spec_pyclass`,
`crates/py/python/tests/test_docs.py::test_api_rst_documents_every_pure_python_module`,
`crates/py/python/tests/test_docs.py::test_api_rst_documents_every_extension_class`,
`crates/py/python/tests/test_docs.py::test_toctree_entries_exist`,
`crates/py/python/tests/test_docs.py::test_conf_py_is_importable_and_names_the_workspace_version`

The tests check the tree's inventory, not the `sphinx-build` run.

**Status:** done

## §7 Stability

**Spec:** §7 — semver policy, v1.0 stable, major bump for breaking
changes.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — a policy statement, not test-bearing.

## §8 License

**Spec:** §8 — Apache-2.0 (workspace default).

**Repo:** —

**Tests:** —

**Status:** n/a (informative).

## §9 References

**Spec:** §9 — a list of external specs (OMG DDS 1.4, XTypes 1.3,
PSM-Cxx 1.0) and internal vendor specs (`zerodds-c-api-1.0`,
`zerodds-ffi-loader-1.0`, `zerodds-java-omgdds-1.0`) plus PyO3 and
PEP 384.

**Repo:** —

**Tests:** —

**Status:** n/a (informative).

---

## Audit status

41 done / 2 partial / 1 open / 3 n/a (informative) / 0 n/a (rejected).

Test run: `PYTHONPATH=runtime/python:crates/py/python python3 -m pytest
crates/py/python/tests` without `test_qos_lifecycle_surface.py` (it starts
participants on domains 41–46) — 161 green, 0 failed, 1 xfailed
(`test_fixed_array_count_is_refused_on_encode`, §3.1), 17 conditionally
skipped (ROS-2 without `ROS_DISTRO`/`rclpy`, ShapesDemo without
`ddsperf`, loader without `libzerodds`).
`cd runtime/python && python3 -m unittest discover -s tests` — 125
tests green, 0 failed.

Open items: see `zerodds-py-1.0.open.en.md`. Decision records: none.
