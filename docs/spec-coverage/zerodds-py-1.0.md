# `zerodds-py` v1.0 — Spec-Coverage

Audit der Vendor-Spec `docs/specs/zerodds-py-1.0.md` gegen den heutigen
Baum.

**Source:** `docs/specs/zerodds-py-1.0.md` (Vendor-Spec).

`crates/py/` ist eine PyO3-Brücke: die DCPS-Entitäten aus `crates/dcps/`
als Erweiterungsmodul `zerodds._core`, mit einem Python-Package darüber,
das `IdlTopic` über generiertem Code und den `ctypes`-Loader gegen
`libzerodds` mitbringt. Topic-Typen haben einen Codec: den Code, den
`zerodds-idlc generate --lang python` erzeugt (das Python-Backend von
`crates/lang-rules`), über der Wire-Laufzeit in
`runtime/python/zerodds_wire/`. Das Crate trägt ohne `--features
extension-module` nur die Platzhalter-Lib; das Modul baut maturin.

Daneben steht das XRCE-Endpoint-SDK in
`endpoints/python/zerodds_endpoint.py`, das diese Spec nicht beschreibt.

Implementation:

- `crates/py/` — PyO3-Brücke (`src/`) + Python-Package (`python/zerodds/`)
  + Test-Baum (`python/tests/`) + Beispiele + sphinx-Baum.

---

## §1 Architektur

### §1.1 Module-Layout

**Spec:** §1.1 — Crate-Layout mit `Cargo.toml` (crate-type `cdylib`+`rlib`),
`pyproject.toml` für maturin, `src/lib.rs`+`ffi.rs`,
`python/zerodds/{__init__,topic,aio,loader}.py`, `python/tests/` mit
`_generated/`, `examples/`, `docs/` sphinx und `runtime/python/`.

**Repo:** `crates/py/Cargo.toml` (`crate-type = ["cdylib", "rlib"]`,
Feature `extension-module`), `crates/py/pyproject.toml`
(`build-backend = "maturin"`, `python-source = "python"`,
`module-name = "zerodds._core"`), `crates/py/build.rs`
(`-undefined dynamic_lookup` auf Darwin), `crates/py/src/lib.rs`,
`crates/py/src/ffi.rs`, `crates/py/python/zerodds/__init__.py`,
`crates/py/python/zerodds/topic.py`, `crates/py/python/zerodds/aio.py`,
`crates/py/python/zerodds/loader.py`, `crates/py/python/tests/`
(Fixtures: `tests/_generated/*.idl` und ihr `zerodds-idlc`-Output),
`crates/py/examples/`, `crates/py/docs/conf.py`,
`runtime/python/zerodds_wire/`.

`maturin build` baut daraus ein abi3-Wheel mit `_core.abi3.so` und den
Python-Modulen.

**Tests:** `crates/py/python/tests/test_docs.py::test_examples_parse`,
`crates/py/python/tests/test_docs.py::test_literalincludes_point_at_existing_examples`,
`crates/py/python/tests/test_smoke.py::test_version_exposed`

**Status:** done

### §1.2 PyO3-Modul `zerodds._core` — PyClasses

Pro PyClass ein Item. Mapping aus DDS 1.4 §2.2.2.

#### §1.2.1 `DomainParticipantFactory` (PyClass)

**Spec:** §1.2 + §2.1 — PyClass `DomainParticipantFactory` mit
`instance()`, `create_participant(domain_id)`,
`create_participant_offline(domain_id)`,
`create_participant_fast(domain_id)`.

**Repo:** `crates/py/src/ffi.rs` (`PyFactory`) — `instance` als
`#[staticmethod]`, die drei Konstruktoren über
`zerodds_dcps::DomainParticipantFactory`; `create_participant_fast`
setzt `RuntimeConfig { tick_period: 20 ms, spdp_period: 100 ms }`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_factory_singleton_and_offline_participant`,
`crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`,
`crates/py/python/tests/test_loader_smoke.py::test_loader_factory_singleton`

**Status:** done

#### §1.2.2 `DomainParticipant` (PyClass)

**Spec:** §1.2 + §2.2 — PyClass `DomainParticipant` mit `domain_id`,
`topics_len`, `discovered_participants_count`,
`create_{bytes,shape}_topic`, `create_{publisher,subscriber}`,
`assert_liveliness`, `ignore_{participant,topic,publication,subscription}`,
`contains_entity`, `get_discovered_{topics,participants}`.

**Repo:** `crates/py/src/ffi.rs` (`PyParticipant`) — zusätzlich
`create_keyed_topic` und `create_bytes_contentfilteredtopic`;
die vier `ignore_*` und `contains_entity` nehmen den `u64` einer
`InstanceHandle`.

**Tests:** `crates/py/python/tests/test_participant_ignore.py::test_ignore_participant_unknown_handle_is_silent`,
`crates/py/python/tests/test_participant_ignore.py::test_contains_entity_false_for_unknown_handle`,
`crates/py/python/tests/test_participant_ignore.py::test_get_discovered_participants_offline_is_empty`,
`crates/py/python/tests/test_participant_ignore.py::test_assert_liveliness_offline_does_not_raise`

**Status:** done

#### §1.2.3 `BytesTopic` (PyClass)

**Spec:** §1.2 — PyClass `BytesTopic` mit `name`, `type_name`.

**Repo:** `crates/py/src/ffi.rs` (`PyBytesTopic`) über
`Topic<RawBytes>`; `type_name` liest
`<RawBytes as DdsType>::TYPE_NAME` und ist damit an die Rust-Seite
gebunden statt neu geschrieben.

**Tests:** `crates/py/python/tests/test_smoke.py::test_bytes_topic_and_reader_are_creatable_offline`

**Status:** done

#### §1.2.4 `ShapeTopic` (PyClass)

**Spec:** §1.2 + §2.7 — PyClass `ShapeTopic` mit `name`, `type_name`,
Type-Name = `"ShapeType"`.

**Repo:** `crates/py/src/ffi.rs` (`PyShapeTopic`) über
`Topic<ShapeType>`; `type_name` liest
`<ShapeType as DdsType>::TYPE_NAME` aus `crates/dcps/src/interop.rs`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_shape_topic_matches_vendor_interop_type_name`,
`crates/py/python/tests/test_shapesdemo_interop.py::test_shape_type_name_matches_shapesdemo_convention`

**Status:** done

#### §1.2.5 `Publisher` (PyClass)

**Spec:** §1.2 + §2.3 — PyClass `Publisher` mit
`create_bytes_writer(topic)`, `create_shape_writer(topic)`.

**Repo:** `crates/py/src/ffi.rs` (`PyPublisher`) — dazu die
`*_with_qos`-Varianten aus §6.2 und `create_keyed_writer`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`,
`crates/py/python/tests/test_qos.py::test_create_bytes_writer_with_qos_offline`,
`crates/py/python/tests/test_qos.py::test_create_shape_writer_with_qos_offline`

**Status:** done

#### §1.2.6 `Subscriber` (PyClass)

**Spec:** §1.2 + §2.3 — PyClass `Subscriber` mit
`create_bytes_reader(topic)`, `create_shape_reader(topic)`.

**Repo:** `crates/py/src/ffi.rs` (`PySubscriber`) — dazu die
`*_with_qos`-Varianten, `create_keyed_reader` und
`create_bytes_reader_cft` für den Content-Filter aus §2.2.2.5.4.

**Tests:** `crates/py/python/tests/test_smoke.py::test_bytes_topic_and_reader_are_creatable_offline`,
`crates/py/python/tests/test_qos.py::test_create_bytes_reader_with_qos_offline`,
`crates/py/python/tests/test_qos_lifecycle_surface.py::test_content_filtered_topic_filters_samples`

**Status:** done

#### §1.2.7 `BytesWriter` (PyClass)

**Spec:** §1.2 + §2.4 — PyClass `BytesWriter` mit `write`,
`wait_for_matched_subscription`, `matched_subscription_count`,
`publication_matched_status`, `liveliness_lost_status`,
`offered_deadline_missed_status`.

**Repo:** `crates/py/src/ffi.rs` (`PyBytesWriter`) — dazu
`register_instance`, `lookup_instance`, `dispose`,
`unregister_instance` und `set_listener`/`clear_listener`. `write` und
die drei Wartefunktionen geben den GIL über `py.detach` frei.

**Tests:** `crates/py/python/tests/test_status_getters.py::test_publication_matched_status_shape`,
`crates/py/python/tests/test_status_getters.py::test_liveliness_lost_status_shape`,
`crates/py/python/tests/test_status_getters.py::test_offered_deadline_missed_status_shape`,
`crates/py/python/tests/test_qos_lifecycle_surface.py::test_bytes_writer_lifecycle_methods_present`

**Status:** done

#### §1.2.8 `BytesReader` (PyClass)

**Spec:** §1.2 + §2.5 — PyClass `BytesReader` mit `take`,
`wait_for_data`, `wait_for_matched_publication`,
`matched_publication_count`, `subscription_matched_status`,
`sample_lost_status`, `requested_deadline_missed_status`.

**Repo:** `crates/py/src/ffi.rs` (`PyBytesReader`) — dazu
`take_with_info` / `read_with_info`, die `(bytes, SampleInfo)`-Paare
liefern, `liveliness_changed_status` und
`set_listener`/`clear_listener`.

**Tests:** `crates/py/python/tests/test_status_getters.py::test_subscription_matched_status_shape`,
`crates/py/python/tests/test_status_getters.py::test_sample_lost_status_shape`,
`crates/py/python/tests/test_status_getters.py::test_requested_deadline_missed_status_shape`,
`crates/py/python/tests/test_qos_lifecycle_surface.py::test_bytes_take_with_info_returns_sample_info`

**Status:** done

#### §1.2.9 `Shape` (PyClass / Dataclass)

**Spec:** §1.2 + §2.7 — PyClass `Shape` mit Feldern `color`, `x`, `y`,
`shapesize` (default 30), `__repr__`, Konvertierungen `From<&PyShape>`
und `From<ShapeType>` in beide Richtungen.

**Repo:** `crates/py/src/ffi.rs` (`PyShape`) mit
`#[pyo3(signature = (color, x, y, shapesize=30))]` und den beiden
`From`-Implementierungen gegen `zerodds_dcps::interop::ShapeType`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_shape_constructor_and_repr`,
`crates/py/python/tests/test_smoke.py::test_shape_default_shapesize_is_30`

**Status:** done

#### §1.2.10 `ShapeWriter` (PyClass)

**Spec:** §1.2 + §2.4 — PyClass `ShapeWriter` mit `write(shape)`,
`register_instance(shape)`, `dispose(shape)`,
`unregister_instance(shape)`, `wait_for_matched_subscription`.

**Repo:** `crates/py/src/ffi.rs` (`PyShapeWriter`) — `dispose` und
`unregister_instance` registrieren die Instanz nach, wenn
`lookup_instance` nil liefert, damit immer ein gültiges Handle
vorliegt (Spec §2.2.2.4.2.4).

**Tests:** `crates/py/python/tests/test_writer_lifecycle.py::test_register_instance_returns_nonzero_handle`,
`crates/py/python/tests/test_writer_lifecycle.py::test_register_instance_is_idempotent_for_same_key`,
`crates/py/python/tests/test_writer_lifecycle.py::test_lifecycle_full_sequence`

**Status:** done

#### §1.2.11 `ShapeReader` (PyClass)

**Spec:** §1.2 + §2.5 — PyClass `ShapeReader` mit `take`,
`wait_for_data`, `wait_for_matched_publication`.

**Repo:** `crates/py/src/ffi.rs` (`PyShapeReader`) — dazu
`take_with_info` / `read_with_info` und
`liveliness_changed_status`.

**Tests:** `crates/py/python/tests/test_status_getters.py::test_matched_publication_count_zero_offline`,
`crates/py/python/tests/test_listener.py::test_set_listener_on_shape_reader_offline`

**Status:** done

#### §1.2.12 `GuardCondition` (PyClass)

**Spec:** §1.2 + §2.6 — PyClass `GuardCondition` mit Konstruktor,
`set_trigger_value(bool)`, `get_trigger_value() -> bool`.

**Repo:** `crates/py/src/ffi.rs` (`PyGuardCondition`) über
`zerodds_dcps::condition::GuardCondition`.

**Tests:** `crates/py/python/tests/test_conditions.py::test_guard_condition_default_trigger_is_false`,
`crates/py/python/tests/test_conditions.py::test_guard_condition_set_trigger_roundtrip`,
`crates/py/python/tests/test_conditions.py::test_guard_condition_via_outer_namespace`

**Status:** done

#### §1.2.13 `WaitSet` (PyClass)

**Spec:** §1.2 + §2.6 — PyClass `WaitSet` mit Konstruktor,
`attach_guard_condition(gc)`, `wait(timeout_secs) -> int`.

**Repo:** `crates/py/src/ffi.rs` (`PyWaitSet`) über
`zerodds_dcps::condition::WaitSet`; `wait` gibt den GIL frei und wirft
`TimeoutError`, wenn nichts triggert. `attach_read_condition` und
`attach_query_condition` kommen aus §6.6 dazu.

**Tests:** `crates/py/python/tests/test_conditions.py::test_waitset_attach_and_wait_raises_timeout`,
`crates/py/python/tests/test_conditions.py::test_waitset_wakes_when_guard_condition_fires`,
`crates/py/python/tests/test_conditions.py::test_waitset_via_outer_namespace`

**Status:** done

### §1.3 Python-Wrapper `python/zerodds/`

#### §1.3.1 `__init__.py` — Re-Export

**Spec:** §1.3 — `zerodds.__init__` re-exportiert die `_core`-PyClasses
und `IdlTopic`/`IdlWriter`/`IdlReader` aus `topic.py`.

**Repo:** `crates/py/python/zerodds/__init__.py` — re-exportiert die
PyClasses samt `__version__`, die Module `sample_state`, `view_state`,
`instance_state` und `IdlTopic`/`IdlWriter`/`IdlReader` aus `topic.py`.
Fehlt
das Erweiterungsmodul, setzt `_CORE_AVAILABLE = False` und die
PyClasses werden zu Platzhaltern, die beim Instanziieren einen
`ImportError` mit der maturin-Zeile werfen — die pure-Python-Module
bleiben ohne Rust-Build benutzbar.

**Tests:** `crates/py/python/tests/test_smoke.py::test_version_exposed`,
`crates/py/python/tests/test_conditions.py::test_guard_condition_via_outer_namespace`,
`crates/py/python/tests/test_docs.py::test_api_rst_documents_every_extension_class`

**Status:** done

#### §1.3.2 `topic.py` — `IdlTopic` über einem generierten `TopicDescriptor`

**Spec:** §1.3 + §6.1 — `IdlTopic(participant, name, descriptor)` nimmt
nur einen `zerodds_wire.topic_descriptor.TopicDescriptor` an und legt das
Topic mit CompleteTypeObject, Schlüssel und Extensibility des Typs an;
`IdlWriter`/`IdlReader` kodieren und dekodieren mit dem generierten Code.

**Repo:** `crates/py/python/zerodds/topic.py` (`IdlTopic`, `IdlWriter`,
`IdlReader`) über `DomainParticipant.create_descriptor_topic` und die
PyClasses `DescriptorTopic`/`DescriptorWriter`/`DescriptorReader` in
`crates/py/src/ffi.rs`.

**Tests:** `crates/py/python/tests/test_idl_topic.py::test_idl_topic_rejects_a_non_topic_descriptor`,
`::test_idl_topic_exposes_type_name_from_the_descriptor`,
`::test_idl_writer_encodes_and_reader_decodes_roundtrip`,
`::test_idl_writer_rejects_wrong_type`,
`::test_idl_writer_passthrough_status`

**Status:** done

#### §1.3.3 `runtime/python` — `zerodds_wire`

**Spec:** §1 + §3 — die Wire-Laufzeit, auf die der generierte Code
aufsetzt.

**Repo:** `runtime/python/zerodds_wire/` (`Writer`, `Reader`,
`TopicDescriptor`, `OutOfRange`, Encapsulation und Byte-Ordnung) —
Alignment auf `min(N, MAXALIGN)` mit MAXALIGN vier unter XCDR2 und acht
unter XCDR1, DHEADER- und EMHEADER-Framing, PL_CDR1-Parameter.

**Tests:** `runtime/python/tests/test_wire.py`,
`crates/py/python/tests/test_idl.py::test_wire_primitive_roundtrip`,
`::test_wire_string_alignment_padding`,
`::test_wire_reader_rejects_truncated_string`,
`::test_xcdr2_max_align_caps_double_at_four`

**Status:** done

#### §1.3.4 `loader.py` — pure-`ctypes`-Loader

**Spec:** §1.3 + §1.4 — Pure-`ctypes`-Loader gegen
`libzerodds.{so,dylib,dll}` aus `crates/zerodds-c-api/`. Folgt
`zerodds-ffi-loader-1.0` §3.1. Signaturen aus
`crates/zerodds-c-api/include/zerodds.h`.

**Repo:** `crates/py/python/zerodds/loader.py` (`load_library`,
`Runtime`, `Writer`, `Reader`, `DomainParticipantFactory`,
`ZeroDdsError`) — Auflösung über `ZERODDS_LIB`, das
Wheel-interne `_lib/`, `target/{debug,release}` im Entwicklungsbaum
und zuletzt den System-Linker.

**Tests:** `crates/py/python/tests/test_loader_smoke.py::test_loader_resolves_library`,
`crates/py/python/tests/test_loader_smoke.py::test_loader_runtime_create_participant_offline`,
`crates/py/python/tests/test_loader_smoke.py::test_loader_zerodds_error_is_runtime_error_subclass`

Die vier Tests überspringen, wenn keine `libzerodds` gefunden wird
(`cargo build -p zerodds-c-api` oder `ZERODDS_LIB`).

**Status:** done

### §1.4 Zwei-Pfad-Wahl

**Spec:** §1.4 — Zwei unabhängige Pfade: PyO3 (Performance,
RTPS-native) und pure-`ctypes` (Distro-Package, Zero-Build).

**Repo:** `crates/py/src/ffi.rs` für den PyO3-Pfad,
`crates/py/python/zerodds/loader.py` für den `ctypes`-Pfad. Die beiden
teilen nur Wire-Format und Domain-ID: `loader.py` importiert
`_core` nicht und läuft ohne Rust-Toolchain, `ffi.rs` kennt
`libzerodds` nicht.

**Tests:** `crates/py/python/tests/test_loader_smoke.py::test_loader_factory_singleton`,
`crates/py/python/tests/test_smoke.py::test_factory_singleton_and_offline_participant`

**Status:** done

### §1.5 Schichten-Position

**Spec:** §1.5 — Layer 6 (PSMs / Bindings). Direkte Abhängigkeiten:
`zerodds-dcps` (Layer 4), `zerodds-c-api` (Layer 6 C-FFI).

**Repo:** `crates/py/Cargo.toml` — die Rust-Abhängigkeiten sind
`zerodds-dcps`, `zerodds-qos` und `zerodds-wire`; auf
`crates/zerodds-c-api/` greift der `ctypes`-Pfad zur Laufzeit über den
Loader zu, ohne Manifest-Kante. `crates/py/src/lib.rs` nennt die
Schicht im Crate-Doc.

**Tests:** `crates/py/src/lib.rs::dcps_re_export_compiles`

**Status:** done

## §2 OMG-API-Coverage (DDS 1.4 §2.2.2)

### §2.1 DomainParticipantFactory (DDS 1.4 §2.2.2.2.1)

**Spec:** §2.1 — Mapping-Tabelle: `get_instance()`,
`create_participant`, `create_participant_offline`, implizites
`delete_participant` via Python-GC.

**Repo:** `crates/py/src/ffi.rs` (`PyFactory`) —
`DomainParticipantFactory.instance()` als classmethod-artige
`#[staticmethod]`, `create_participant` und
`create_participant_offline` je auf die gleichnamige Rust-Operation.
Das Löschen hängt am Python-GC: `PyParticipant` hält den
`DomainParticipant`, dessen `Drop` die Rust-Seite abbaut.

**Tests:** `crates/py/python/tests/test_smoke.py::test_factory_singleton_and_offline_participant`,
`crates/py/python/tests/test_multiproc.py::test_multiproc_python_python_roundtrip`

**Status:** done

### §2.2 DomainParticipant (DDS 1.4 §2.2.2.2.2)

**Spec:** §2.2 — 12 Operations: `get_domain_id`, `create_topic`,
`create_publisher`, `create_subscriber`, `assert_liveliness`,
`ignore_{participant,topic,publication,subscription}`,
`contains_entity`, `get_discovered_{topics,participants}`.

**Repo:** `crates/py/src/ffi.rs` (`PyParticipant`) — alle zwölf.
`get_domain_id` ist das Property `domain_id`, `create_topic` sind die
drei getypten Konstruktoren `create_bytes_topic`,
`create_shape_topic` und `create_keyed_topic`.

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

**Repo:** `crates/py/src/ffi.rs` (`PyPublisher`, `PySubscriber`) —
je Topic-Typ ein Konstruktor, jeweils mit `*_with_qos`-Variante.

**Tests:** `crates/py/python/tests/test_qos.py::test_create_shape_reader_with_qos_offline`,
`crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`

**Status:** done

### §2.4 DataWriter (DDS 1.4 §2.2.2.4.2)

**Spec:** §2.4 — Mapping-Tabelle: `write`, `register_instance`,
`unregister_instance`, `dispose`, `get_matched_subscriptions`,
`get_publication_matched_status`, `get_liveliness_lost_status`,
`get_offered_deadline_missed_status` plus
`wait_for_matched_subscription` sync-helper.

**Repo:** `crates/py/src/ffi.rs` (`PyBytesWriter`, `PyShapeWriter`,
`PyKeyedWriter`) — `get_matched_subscriptions` ist
`matched_subscription_count`, die drei Status-Getter liefern die
Tupel der Spec-Struktur.

**Tests:** `crates/py/python/tests/test_writer_lifecycle.py::test_dispose_does_not_raise`,
`crates/py/python/tests/test_writer_lifecycle.py::test_unregister_instance_does_not_raise`,
`crates/py/python/tests/test_status_getters.py::test_matched_subscription_count_zero_offline`,
`crates/py/python/tests/test_qos_lifecycle_surface.py::test_keyed_register_instance_returns_real_handle`

**Status:** done

### §2.5 DataReader (DDS 1.4 §2.2.2.5.3)

**Spec:** §2.5 — `take`, `wait_for_data`, `wait_for_matched_publication`,
`matched_publication_count`, `subscription_matched_status`,
`sample_lost_status`, `requested_deadline_missed_status`. `read`
und `wait_for_historical_data` bewusst weggelassen.

**Repo:** `crates/py/src/ffi.rs` (`PyBytesReader`, `PyShapeReader`,
`PyKeyedReader`). Über die Spec-Liste hinaus liegen
`read_with_info` und `take_with_info` an, die je Sample die
`SampleInfo` mitliefern — `read` ohne `SampleInfo` bleibt weg, wie die
Spec es sagt.

**Tests:** `crates/py/python/tests/test_qos_lifecycle_surface.py::test_bytes_take_with_info_returns_sample_info`,
`crates/py/python/tests/test_status_getters.py::test_subscription_matched_status_shape`,
`crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`

**Status:** done

### §2.6 WaitSet / Conditions (DDS 1.4 §2.2.2.6)

**Spec:** §2.6 — `WaitSet`, `GuardCondition` mit
`attach_guard_condition`, `wait`, `set/get_trigger_value`.
`ReadCondition`/`QueryCondition` sind über §6.6 verfügbar.

**Repo:** `crates/py/src/ffi.rs` (`PyGuardCondition`, `PyWaitSet`),
`crates/py/src/conditions.rs` für die beiden Reader-Conditions.

Die Spec sagt in §2.6, das äussere `zerodds`-Namespace re-exportiere
`WaitSet` und `GuardCondition` noch nicht, und listet beide in §1.3 im
Re-Export-Block. `crates/py/python/zerodds/__init__.py` folgt §1.3:
beide sind über `zerodds.` und über `zerodds._core.` erreichbar, was
den §2.6-Zugang einschliesst.

**Tests:** `crates/py/python/tests/test_conditions.py::test_waitset_wakes_when_guard_condition_fires`,
`crates/py/python/tests/test_conditions.py::test_waitset_via_outer_namespace`,
`crates/py/python/tests/test_read_conditions.py::test_read_condition_attaches_to_waitset`

**Status:** done

### §2.7 ShapeType — Cross-Vendor-Interop-Type

**Spec:** §2.7 — `Shape`-PyClass byte-identisch zu `ShapeType` in
`crates/dcps/src/interop.rs`. Felder `color: String, x: i32, y: i32,
shapesize: i32` (default 30). Type-Name auf dem Wire = `"ShapeType"`.

**Repo:** `crates/py/src/ffi.rs` (`PyShape`, `shape_reference_octets`)
— die Konvertierung geht über `zerodds_dcps::interop::ShapeType`
selbst, es gibt keinen zweiten Codec auf der Rust-Seite.
`shape_reference_octets` gibt die Oktette dieses Codecs nach Python
zurück, damit der generierte Python-Code gegen ihn gemessen werden kann.

**Tests:** `crates/py/python/tests/test_idl.py::test_shapetype_matches_the_rust_codec_octet_for_octet`,
`crates/py/python/tests/test_smoke.py::test_shape_topic_matches_vendor_interop_type_name`,
`crates/py/python/tests/test_shapesdemo_interop.py::test_shape_type_name_matches_shapesdemo_convention`

**Status:** done

## §3 IDL-Mapping — generierter Code über `zerodds_wire`

### §3.1 Mapping

**Spec:** §3.1 — `zerodds-idlc generate --lang python` erzeugt je Typ
eine Dataclass, ihre Kodierfunktionen und `ZD_TOPIC_DESCRIPTORS`; die
Mapping-Tabelle (Ganzzahlen, Gleitkomma, Strings, `sequence`, `T[N]`,
`map`, `@optional`, `enum`, `union`, `struct`); ein beschränkter Wert
über seiner Schranke wird mit `OutOfRange` abgewiesen, bevor ein Byte
geschrieben ist; der Decoder wendet `@try_construct` an (XTypes 1.3
§7.2.2.4.4.4.9).

**Repo:** `crates/lang-rules/src/adapter/python.rs` (das Python-Mapping),
`crates/lang-rules/src/emit/encode.rs` (der gemeinsame Codec-Walk samt
Schrankenprüfung), `tools/idlc` (`generate --lang python`),
`runtime/python/zerodds_wire/`.

**Tests:** `crates/py/python/tests/test_idl.py::test_sensor_mixed_fields_roundtrip`,
`::test_optional_present_and_absent`, `::test_enum_roundtrip`,
`::test_enum_unknown_value_raises`, `::test_union_case_int_roundtrip`,
`::test_union_default_branch_used_for_unknown_disc`,
`::test_nested_struct_roundtrip`, `::test_fixed_array_roundtrip`,
`::test_generated_bounds_roundtrip_and_enforced`

**Status:** partial — die Elementzahl eines festen Arrays prüft der
Encoder nicht: zu viele Elemente werden gekürzt, zu wenige werfen
`IndexError` (`test_fixed_array_count_is_refused_on_encode`,
`xfail(strict=True)`).

### §3.2 Beispiel

**Spec:** §3.2 — IDL, generiertes Modul, `IdlTopic` über seinem
Deskriptor.

**Repo:** `crates/py/examples/03_idl_topic.idl`,
`crates/py/examples/03_idl_topic_gen.py` (der `zerodds-idlc`-Output),
`crates/py/examples/03_idl_topic.py`.

**Tests:** `crates/py/python/tests/test_docs.py::test_examples_parse`,
`crates/py/python/tests/test_idl.py::test_shapetype_matches_the_rust_codec_octet_for_octet`

**Status:** done

## §4 Test-Pflicht

### §4.1 Test-Inventar

**Spec:** §4 — pytest über `crates/py/python/tests/`, Fixtures in
`tests/_generated/` als IDL plus ihr unveränderter `zerodds-idlc`-Output,
die eigenen Tests der Laufzeit in `runtime/python/tests/`.

**Repo:** siebzehn Dateien in `crates/py/python/tests/` — darunter
`test_smoke.py` (sieben Tests), `test_idl.py` (vierundvierzig
Testfunktionen über den generierten Fixtures), `test_idl_topic.py`,
`test_qos.py`, `test_qos_lifecycle_surface.py`, `test_status_getters.py`,
`test_writer_lifecycle.py`, `test_participant_ignore.py`,
`test_conditions.py`, `test_read_conditions.py`, `test_listener.py`,
`test_aio.py`, `test_loader_smoke.py`,
`test_loader_declares_extensibility.py`, `test_multiproc.py`,
`test_shapesdemo_interop.py`, `test_docs.py` — dazu der ROS-2-Baum aus
§6.4 und `runtime/python/tests/test_wire.py`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_version_exposed`,
`::test_factory_singleton_and_offline_participant`,
`::test_bytes_topic_and_reader_are_creatable_offline`,
`::test_shape_topic_matches_vendor_interop_type_name`,
`::test_shape_constructor_and_repr`, `::test_shape_default_shapesize_is_30`,
`::test_pub_sub_roundtrip_live`; die Testfunktionen von `test_idl.py`.

**Status:** done

### §4.2 Rust-seitiges Test-Inventar

**Spec:** §4 — kein Rust-Test-Inventar, weil das Crate ohne
`--features extension-module` eine Platzhalter-Lib ohne API ist; die
Spec-Compliance wird pytest-seitig verifiziert.

**Repo:** `crates/py/src/lib.rs` — die vier Module `ffi`, `qos`,
`listener` und `conditions` hängen an `#[cfg(feature =
"extension-module")]`. Es gibt kein `tests/`-Verzeichnis im Crate; der
einzige Rust-Test ist `dcps_re_export_compiles`, der prüft, dass
`zerodds-dcps` als Abhängigkeit erreichbar ist. Jede Aussage dieser
Coverage über Verhalten steht auf einem pytest-Fall.

**Tests:** `crates/py/src/lib.rs::dcps_re_export_compiles`

**Status:** done

## §5 Multi-Process / Cross-Vendor

### §5.1 Single-Process Python ↔ Python

**Spec:** §5 Tabelle Row 1 — Single-Process Python ↔ Python über
`zerodds._core` und `crates/dcps`.

**Repo:** `crates/py/src/ffi.rs` (`PyFactory::create_participant_fast`)
— zwei Participants derselben Domain in einem Prozess; das Matching
läuft über den In-Prozess-Discovery-Fastpath aus `crates/dcps`.

**Tests:** `crates/py/python/tests/test_smoke.py::test_pub_sub_roundtrip_live`,
`crates/py/python/tests/test_qos_lifecycle_surface.py::test_bytes_take_with_info_returns_sample_info`,
`crates/py/python/tests/test_qos_lifecycle_surface.py::test_content_filtered_topic_filters_samples`

**Status:** done

### §5.2 Multi-Process Python ↔ Python

**Spec:** §5 Tabelle Row 2 — Multi-Process lokal über RTPS/UDP +
SPDP/SEDP.

**Repo:** `crates/py/python/tests/test_multiproc.py` — zwei
`python -c`-Subprozesse, ein `BytesTopic`, eine Nutzlast; der
Subscriber beendet sich mit 0 und `OK`, sobald er sie hat.

**Tests:** `crates/py/python/tests/test_multiproc.py::test_multiproc_python_python_roundtrip`

**Status:** done

### §5.3 Cross-Vendor Python ↔ andere Sprachen

**Spec:** §5 Tabelle Row 3 — ShapeType-Wire byte-identisch zu
C++/Rust/Java/C#-Peers über gemeinsamen Topic-Type `"ShapeType"`.

**Repo:** `crates/py/python/tests/_generated/shape.idl` und das daraus
generierte Modul `shape_gen.py` +
`crates/py/src/ffi.rs::shape_reference_octets` (gibt zurück, was
`zerodds_dcps::interop::ShapeType::encode` schreibt) — die Referenz, gegen
die der generierte Python-Codec Oktett für Oktett gehalten wird.

**Tests:** `crates/py/python/tests/test_idl.py::test_shapetype_matches_the_rust_codec_octet_for_octet`
(7 Parameter-Sätze, jeder Fall vergleicht die geschriebenen Oktetten mit
der Rust-Referenz aus demselben Prozess),
`crates/py/python/tests/test_idl.py::test_shapetype_byte_roundtrip`,
`crates/py/python/tests/test_shapesdemo_interop.py::test_shape_type_name_matches_shapesdemo_convention`
(Topic-Type-Name `"ShapeType"`). Kein Gate; alle drei laufen im
Default-Lauf von pytest.

**Status:** partial — die Byte-Identität Python ↔ Rust ist im Baum
gemessen. Für C++, Java und C# gibt es keine Messung im Repo: die
Interop-Matrix, die diese Spalten trägt, liegt auf dem Bench-Host und ist
nicht Teil dieses Baums. Rest als §5.3b geführt.

### §5.3b Byte-Identität Python ↔ C++/Java/C# im Baum

**Spec:** §5 Tabelle Row 3 — ShapeType-Wire byte-identisch zu
C++/Java/C#-Peers.

**Repo:** —

**Tests:** —

**Status:** open — es gibt keinen Testfall im Repo, der die vom
Python-Codec geschriebenen ShapeType-Oktetten gegen die der C++-, Java-
oder C#-Bindung hält. `shape_reference_octets` liefert diese Referenz
bisher nur für Rust. Plan: einen Golden-Vektor je Bindung wie in
`endpoints/python/test_byte_identity.py` (dort für `SensorReading`) auch
für `ShapeType` erzeugen und die vier Codecs gegen denselben Vektor
halten.

### §5.4 Cross-Vendor Python ↔ Cyclone-/Fast-DDS-ShapesDemo

**Spec:** §5 Tabelle Row 4 — gleiche XCDR2-LE-Konvention +
gleicher Type-Name + RTPS-2.5-Wire.

**Repo:** `matrix/zerodds-python/cyclone-*` und `matrix/zerodds-python/fastdds-*` — dieselbe Messung gegen Cyclone und Fast DDS

**Tests:** die Zellen dieser Optionssätze; dazu
`crates/py/python/tests/test_shapesdemo_interop.py::test_cross_vendor_participant_discovery_against_ddsperf`,
das die SPDP-Entdeckung gegen Cyclones `ddsperf` misst und
überspringt, wo `ddsperf` nicht im Pfad liegt

**Status:** done

### §5.5 Generierter Typ über `IdlTopic` ↔ Rust-Codegen

**Spec:** §5 Tabelle Row 5 — Oktetten gegen die Rust-Referenz gemessen.

**Repo:** `crates/py/src/ffi.rs` (`shape_reference_octets`) reicht die
Oktetten von `zerodds_dcps::interop::ShapeType::encode` an Python; das
Modul, das `zerodds-idlc` aus `tests/_generated/shape.idl` erzeugt,
schreibt sie auf der Python-Seite. Verglichen wird mit dem Codec, der im
selben Modul kompiliert ist, nicht mit einem aus einem Rust-Test
kopierten Literal. `wireprobe-python-matrix` misst die ganze
Python-Spalte gegen die Rust-Spalte.

**Tests:** `crates/py/python/tests/test_idl.py::test_shapetype_matches_the_rust_codec_octet_for_octet`
(sieben Shapes, darunter eine leere und eine einbuchstabige Farbe sowie
`i32::MAX`/`i32::MIN`),
`crates/py/python/tests/test_idl.py::test_fixed_member_struct_byte_identical_to_rust_cpp`,
`crates/py/python/tests/test_idl.py::test_sensor_mixed_fields_roundtrip`

**Status:** done

## §6 IDL-Topic, QoS, Async + erweiterte API

### §6.1 `IdlTopic` / `IdlWriter` / `IdlReader`

**Spec:** §6.1 — `IdlTopic(participant, name, descriptor)` über einem
generierten `TopicDescriptor`, liefert `IdlWriter[T]`/`IdlReader[T]`, die
mit dem generierten Code kodieren und dekodieren.

**Repo:** `crates/py/python/zerodds/topic.py` (`IdlTopic`, `IdlWriter`,
`IdlReader`) über `DescriptorTopic`/`DescriptorWriter`/`DescriptorReader`;
der Konstruktor weist alles ausser einem `TopicDescriptor` ab, und
`IdlTopic.type_name` kommt aus dem Deskriptor.

**Tests:** `crates/py/python/tests/test_idl_topic.py::test_idl_topic_rejects_a_non_topic_descriptor`,
`::test_idl_topic_exposes_type_name_from_the_descriptor`,
`::test_idl_writer_encodes_and_reader_decodes_roundtrip`,
`::test_idl_writer_rejects_wrong_type`, `::test_idl_writer_passthrough_status`

**Status:** done

### §6.2 QoS-Builder (alle 22 Policies)

**Spec:** §6.2 — `DataWriterQos`/`DataReaderQos`-PyClass mit Settern
für alle 22 Policies aus DDS 1.4 §2.2.3 plus
`Publisher.create_*_writer_with_qos` /
`Subscriber.create_*_reader_with_qos`.

**Repo:** `crates/py/src/qos.rs` (`PyDataWriterQos`,
`PyDataReaderQos`) — Setter für Reliability, Durability,
DurabilityService, Deadline, LatencyBudget, Liveliness,
DestinationOrder, Lifespan, Ownership, OwnershipStrength, Partition,
Presentation, History, ResourceLimits, TransportPriority,
WriterDataLifecycle, TimeBasedFilter, ReaderDataLifecycle, UserData,
TopicData, GroupData, je auf der Seite, auf der DDS 1.4 §2.2.3 sie
führt. Ein unbekannter Kind-String wird als `ValueError` abgewiesen
statt still auf einen Default zu fallen. Die
`create_*_with_qos`-Konstruktoren liegen in `crates/py/src/ffi.rs`.

**Tests:** `crates/py/python/tests/test_qos.py::test_writer_qos_defaults`,
`crates/py/python/tests/test_qos.py::test_writer_qos_setters_with_durations`,
`crates/py/python/tests/test_qos.py::test_reader_qos_full_setter_chain`,
`crates/py/python/tests/test_qos.py::test_writer_qos_rejects_unknown_reliability_kind`,
`crates/py/python/tests/test_qos.py::test_create_bytes_writer_with_qos_offline`

**Status:** done

### §6.3 AsyncIO-API

**Spec:** §6.3 — `zerodds.aio` mit `AsyncBytesWriter`,
`AsyncBytesReader`, `AsyncShapeWriter`, `AsyncShapeReader`,
`AsyncWaitSet`; blockierende Aufrufe über eine
`asyncio.to_thread`-Brücke.

**Repo:** `crates/py/python/zerodds/aio.py` — die fünf Wrapper; jede
blockierende Operation läuft über `asyncio.to_thread`, die
nicht-blockierenden Getter gehen direkt durch.

**Tests:** `crates/py/python/tests/test_aio.py::test_async_wrappers_are_constructible`,
`crates/py/python/tests/test_aio.py::test_async_wait_for_data_uses_event_loop`,
`crates/py/python/tests/test_aio.py::test_async_take_returns_list_passthrough`,
`crates/py/python/tests/test_aio.py::test_async_waitset_wait_raises_timeout`

**Status:** done

### §6.4 ROS-2-pytest-Integration

**Spec:** §6.4 — Test-Baum `crates/py/python/tests/ros2/` mit
`conftest.py` (Skip, wenn `ROS_DISTRO` + `rclpy` +
`RMW_IMPLEMENTATION` fehlen) und `test_rmw_zerodds_interop.py` für
Standard-rclpy-Publish/Subscribe über das ZeroDDS-rmw.

**Repo:** `crates/py/python/tests/ros2/conftest.py` — markiert
ausschliesslich die Fälle unterhalb `ros2/` als übersprungen, wenn
`ROS_DISTRO`, `ros2` im Pfad oder `rclpy` fehlen, und getrennt davon,
wenn `RMW_IMPLEMENTATION` nicht auf dem ZeroDDS-rmw steht;
`crates/py/python/tests/ros2/test_rmw_zerodds_interop.py` — sechs
Fälle über rclpy;
`crates/rmw-zerodds-shim/rmw_c/run_ros2_pytest.sh` — der
reproduzierbare Lauf, der die ABI-Schicht baut, sie in eine
ROS-2-Installation registriert und diesen Baum darauf ausführt.

Die Spec nennt den RMW-Namen `rmw_zerodds_shim`; der
Implementierungs-Identifier nach REP-2007 ist
`rmw_zerodds_cpp` (`crates/rmw-zerodds-shim/ament/package.xml`), und
danach prüft die `conftest.py`.

**Tests:** `crates/py/python/tests/ros2/test_rmw_zerodds_interop.py::test_rclpy_init_succeeds_with_zerodds_rmw`,
`crates/py/python/tests/ros2/test_rmw_zerodds_interop.py::test_rclpy_publish_subscribe_string_roundtrip`,
`crates/py/python/tests/ros2/test_rmw_zerodds_interop.py::test_rclpy_service_call_roundtrip`,
`crates/py/python/tests/ros2/test_rmw_zerodds_interop.py::test_rclpy_topic_graph_introspection`,
`crates/py/python/tests/ros2/test_rmw_zerodds_interop.py::test_rclpy_node_names_graph`,
`crates/py/python/tests/ros2/test_rmw_zerodds_interop.py::test_rclpy_endpoint_info_by_topic`

Auf einem Host ohne ROS 2 überspringen alle sechs mit der Begründung
aus der `conftest.py`; gemessen werden sie über
`run_ros2_pytest.sh` auf einer ROS-2-Installation, siehe
`docs/spec-coverage/ros2-rmw.md`.

**Status:** done

### §6.5 Status-Listener-Callbacks

**Spec:** §6.5 — PyClasses `DataWriterListener` (3 Slots) und
`DataReaderListener` (6 Slots), registriert über
`writer.set_listener(listener, mask)` /
`reader.set_listener(listener, mask)`.

**Repo:** `crates/py/src/listener.rs` (`PyDataWriterListener`,
`PyDataReaderListener`, `PyDataWriterListenerBridge`,
`PyDataReaderListenerBridge`) — drei Writer-Slots
(`on_offered_deadline_missed`, `on_liveliness_lost`,
`on_publication_matched`) und sechs Reader-Slots
(`on_data_available`, `on_sample_lost`, `on_sample_rejected`,
`on_requested_deadline_missed`, `on_liveliness_changed`,
`on_subscription_matched`). Die Brücke nimmt den GIL vor jedem
Aufruf; eine Exception im Callback wird gedruckt und nicht nach Rust
propagiert. `set_listener`/`clear_listener` liegen an den vier
Writer-/Reader-PyClasses in `crates/py/src/ffi.rs`.

**Tests:** `crates/py/python/tests/test_listener.py::test_writer_listener_constructible`,
`crates/py/python/tests/test_listener.py::test_reader_listener_constructible`,
`crates/py/python/tests/test_listener.py::test_set_listener_on_bytes_writer_offline`,
`crates/py/python/tests/test_listener.py::test_set_listener_on_bytes_reader_offline`,
`crates/py/python/tests/test_listener.py::test_set_listener_on_shape_writer_offline`,
`crates/py/python/tests/test_listener.py::test_set_listener_on_shape_reader_offline`

**Status:** done

### §6.6 ReadCondition / QueryCondition

**Spec:** §6.6 — PyClasses `ReadCondition` und `QueryCondition` über
die Rust-Conditions, Bitmask-Konstanten als
`zerodds.{sample_state,view_state,instance_state}`, SQL92-Filter im
Konstruktor validiert, `WaitSet.attach_read_condition` /
`attach_query_condition`.

**Repo:** `crates/py/src/conditions.rs` (`PyReadCondition`,
`PyQueryCondition`) über `zerodds_dcps::condition`;
`crates/py/python/zerodds/sample_state.py`,
`crates/py/python/zerodds/view_state.py`,
`crates/py/python/zerodds/instance_state.py` für die Bitmasken, die
`crates/py/src/ffi.rs` als Modul-Attribute aus
`zerodds_dcps::sample_info` setzt. Ein ungültiger SQL-Ausdruck wird im
Konstruktor als `RuntimeError` abgewiesen.

**Tests:** `crates/py/python/tests/test_read_conditions.py::test_sample_state_constants`,
`crates/py/python/tests/test_read_conditions.py::test_read_condition_modes`,
`crates/py/python/tests/test_read_conditions.py::test_query_condition_with_parameters`,
`crates/py/python/tests/test_read_conditions.py::test_query_condition_invalid_sql_raises`,
`crates/py/python/tests/test_read_conditions.py::test_query_condition_attaches_to_waitset`

**Status:** done

### §6.7 sphinx-Doc-Pfad

**Spec:** §6.7 — `crates/py/docs/conf.py` mit autodoc/napoleon/
viewcode/intersphinx, `autodoc_mock_imports` für den Build ohne
maturin, `crates/py/docs/api.rst` listet die PyClasses und die Module
`loader`, `topic`, `aio`, `sample_state`, `view_state`,
`instance_state`.

**Repo:** `crates/py/docs/conf.py` — die vier Extensions,
`autodoc_mock_imports = ["zerodds._core"]` und die Version aus dem
Workspace-Manifest statt als Literal; `crates/py/docs/api.rst` — jede
Klasse, die das Erweiterungsmodul exportiert, und die Module `loader`,
`topic`, `aio` und die drei State-Module;
`crates/py/docs/index.rst`, `crates/py/docs/quickstart.rst`,
`crates/py/docs/examples.rst`.

**Tests:** `crates/py/python/tests/test_docs.py::test_conf_py_declares_the_four_extensions_the_spec_names`,
`crates/py/python/tests/test_docs.py::test_api_rst_documents_every_spec_pyclass`,
`crates/py/python/tests/test_docs.py::test_api_rst_documents_every_pure_python_module`,
`crates/py/python/tests/test_docs.py::test_api_rst_documents_every_extension_class`,
`crates/py/python/tests/test_docs.py::test_toctree_entries_exist`,
`crates/py/python/tests/test_docs.py::test_conf_py_is_importable_and_names_the_workspace_version`

Die Tests prüfen den Bestand des Baums, nicht den `sphinx-build`-Lauf.

**Status:** done

## §7 Stabilität

**Spec:** §7 — semver-Politik, v1.0 stabil, Major-Bump für
Breaking-Changes.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Policy-Statement, nicht
test-pflichtig.

## §8 Lizenz

**Spec:** §8 — Apache-2.0 (Workspace-Default).

**Repo:** —

**Tests:** —

**Status:** n/a (informative).

## §9 Referenzen

**Spec:** §9 — Liste von externen Specs (OMG DDS 1.4, XTypes 1.3,
PSM-Cxx 1.0) und internen Vendor-Specs (`zerodds-c-api-1.0`,
`zerodds-ffi-loader-1.0`, `zerodds-java-omgdds-1.0`) plus PyO3 und
PEP 384.

**Repo:** —

**Tests:** —

**Status:** n/a (informative).

---

## Audit-Status

41 done / 2 partial / 1 open / 3 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `PYTHONPATH=runtime/python:crates/py/python python3 -m pytest
crates/py/python/tests` ohne `test_qos_lifecycle_surface.py` (startet
Participants auf den Domains 41–46) — 161 grün, 0 failed, 1 xfailed
(`test_fixed_array_count_is_refused_on_encode`, §3.1), 17 bedingt
übersprungen (ROS-2 ohne `ROS_DISTRO`/`rclpy`, ShapesDemo ohne `ddsperf`,
Loader ohne `libzerodds`).
`cd runtime/python && python3 -m unittest discover -s tests` — 125
Tests grün, 0 failed.

Offene Punkte: siehe `zerodds-py-1.0.open.md`. Decision-Records: keine.
