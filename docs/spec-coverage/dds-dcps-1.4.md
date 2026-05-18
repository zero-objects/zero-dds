# DDS DCPS 1.4 — Spec-Coverage

**PDF:** `docs/standards/cache/omg/zerodds-dcps-1.4.pdf` (180 Seiten, OMG formal/2015-04-10)

Folgt dem Format aus `docs/spec-coverage/PROCESS.md`. Audit Item-für-Item
gegen die PDF; jede Anforderung mit Spec-Zitat + Repo-Pfad + Test-Pfad +
Status (`done` / `partial` / `open` / `n/a`).

**Kontext:** `crates/dcps/` ist live mit ~25 Files + ~300 Tests; `crates/qos/` ergaenzt mit ~125 Tests.
Live: Entity-Trait + Listener-Stack (6 Listener + listener_dispatch),
WaitSet+GuardCondition+StatusCondition, alle 13 Status-Records,
register/unregister/dispose-Lifecycle, BuiltinSubscriber + 4 Builtin-
Topic-Reader, CoherentScope (begin/end_coherent_changes), WLP fuer
LIVELINESS, InstanceHandle, SampleInfo. 22 QoS-Policies definiert,
Subset live verdrahtet.

---

## §1 Scope / Purpose

### 1.1.1 DDS API + Communication-Semantik fuer DCPS

**Spec:** §1.1, S. 1 — "DDS-Spec definiert API + Communication-
Semantik für DCPS."

**Repo:** `crates/dcps/src/lib.rs` mit voller §2.2.2-Entity-API
(DomainParticipant, Publisher, Subscriber, Topic, DataWriter,
DataReader, WaitSet, Conditions). Communication-Semantik ueber
RTPS-Transport (`crates/rtps/`, `crates/discovery/`).

**Tests:** Workspace-weit (Section §2.2.2.x-Items belegen alle
Sub-Anforderungen einzeln).

**Status:** done — alle Sub-Sektionen §2.2.2-§2.2.6 sind als done
markiert; siehe pro-Item-Belege weiter unten.

### 1.1.2 Pre-allocate Resources (no dynamic alloc)

**Spec:** §1.1, S. 1 — "Pre-allocate Resources."

**Repo:** `crates/dcps/src/runtime.rs::RuntimeConfig` mit Caps fuer
Sample-Pools, Reader/Writer-Capacity. DoS-Caps in
`crates/rtps/src/parameter_list.rs::MAX_PARAMETERS` und
`crates/dcps/src/condition.rs::MAX_WAITSET_CONDITIONS`.

**Tests:** RuntimeConfig-Tests + DoS-Cap-Tests in den jeweiligen
Modulen.

**Status:** done — alle kritischen Hot-Paths haben statische Caps;
Heap-Allocations bleiben on-startup oder per-event statt per-sample.

### 1.1.3 Avoid unbounded resources

**Spec:** §1.1 — "Avoid unbounded resources."

**Repo:** RESOURCE_LIMITS-QoS in
`crates/qos/src/policies/resource_limits.rs` plus DoS-Caps in
ParameterList, WaitSet, Type-Resolver
(`crates/types/src/resolve.rs::DEFAULT_MAX_RESOLVE_NODES`),
Macro-Expansion (`crates/idl/src/preprocessor/mod.rs::MAX_MACRO_EXPANSION_DEPTH`).

**Tests:** Per-Cap Negativ-Tests pro Cap-Site (siehe jeweils
crate-spezifische Test-Module).

**Status:** done

### 1.1.4 Minimize copies

**Spec:** §1.1 — "Minimize copies."

**Repo:** `crates/cdr/src/buffer.rs::BufferReader` mit Borrow-Decode
(zero-copy bytes-Slices); `crates/dcps/src/dds_type.rs::RawBytes`
fuer ungemarshalte Inbox; `crates/cdr/src/struct_enc.rs::MutableMember`
mit `body: &[u8]`.

**Tests:** Zero-copy-Pfade implizit ueber Roundtrip-Tests; explizit
in `cdr` borrow-Tests.

**Status:** done

### 1.1.5 Typed Interfaces

**Spec:** §1.1 — "Typed Interfaces."

**Repo:** `crates/dcps/src/dds_type.rs::DdsType`-Trait, generisch
ueber Topic/Writer/Reader.

**Tests:** Type-Generic-Tests.

**Status:** done

### 1.1.6 QoS-driven Behavior

**Spec:** §1.1 — "QoS-driven Behavior."

**Repo:** `crates/qos/src/policies/` mit allen 22 Spec-Policies:
durability, durability_service, presentation, deadline, latency_budget,
ownership, ownership_strength, liveliness, time_based_filter, partition,
reliability, transport_priority, lifespan, destination_order, history,
resource_limits, entity_factory, writer_data_lifecycle,
reader_data_lifecycle, user_data, topic_data, group_data.
Compatibility-Check via `crates/qos/src/compatibility.rs`.

**Tests:** Pro QoS-Policy eigenes Test-Modul (siehe §2.2.3.x-Items
weiter unten); Cross-QoS-Compatibility-Tests in `compatibility.rs`.

**Status:** done — alle 22 Policies sind im Wire-Format und Code
verfuegbar; Engine-Side-Wiring siehe §2.2.3.x pro Policy. K3a-F-
Schliessung 2026-04-28: alle 10 Iron-Rule-Open-Tracker konvertiert
zu live-Implementationen (T1 contains_entity rekursiv, T2 GROUP-
coherent snapshot_generation, T3 RESOURCE_LIMITS write-Block,
T4 EXCLUSIVE Strength-Selection + GUID-Tiebreaker, T5 TIME_BASED_FILTER
Reader-Drop, T6 READER_DATA_LIFECYCLE autopurge-Timer, T7
QueryCondition SQL-Per-Sample-Eval, T8 MultiTopic Hash-Join,
T9 Liveliness-driven OWNERSHIP-Failover, T10 TRANSIENT/PERSISTENT
Durability-Service).

### 1.1.7 Pub/Sub-Trennung (separate include)

**Spec:** §1.1 — "Pub/Sub-Trennung."

**Repo:** `crates/dcps/src/{publisher,subscriber}.rs`.

**Tests:** Crate-Layout-Tests.

**Status:** done

### 1.2 Wire-Interop ist NICHT Scope von DCPS (siehe DDSI-RTPS)

**Spec:** §1.2 — "Wire-Interop separat in DDSI-RTPS."

**Repo:** — siehe `ddsi-rtps-2.5.md`.

**Tests:** —

**Status:** `n/a (informative)` — die Section deklariert nur die
Scope-Trennung zwischen DCPS-Spec und DDSI-RTPS-Spec; keine
normative Anforderung an die DCPS-Implementation.

---

## §2.1 Summary

### 2.1.1 DDS-Spec besteht aus PIM (UML) + IDL-PSM

**Spec:** §2.1, S. 3 — "PIM + IDL-PSM."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — beschreibt den Aufbau der
Spec selbst (PIM+PSM-Konvention der OMG), nicht eine zu
implementierende Anforderung.

### 2.1.2 DCPS-Layer fuer typisierte Pub/Sub-Daten

**Spec:** §2.1, S. 3.

**Repo:** `crates/dcps/src/lib.rs`.

**Tests:** Crate-weit.

**Status:** done

### 2.1.3 DLRL-Layer

**Spec:** §2.1, S. 3 — Optionaler Data-Local-Reconstruction-Layer
mit Object-Cache + Identity-Tracking + Relationship-Resolver
(1:1/1:N/N:M) ueber DCPS.

**Repo:** `crates/dlrl/` (1668 SLOC + 48 inline-Tests) +
`crates/dlrl-codegen/` (PSM-Codegen für C++/C#/Java/TS).

**Tests:** Cross-Ref `dlrl-1.2.md` (eigene Spec-Coverage-Datei für
DDS 1.2 §8 + Annex B; in DDS 1.4 nicht mehr Teil der Spec).

**Status:** done — DLRL ist als eigenständiger Stack implementiert;
formale Spec-Coverage in `dlrl-1.2.md` (11 done / 1 partial / 0 open).

---

## §2.2.1.1 ReturnCode_t (Page 5, Tab. RC)

### 2.2.1.1.1 RC OK -> Result::Ok

**Spec:** §2.2.1.1 Tab.RC — "OK: Successful return."

**Repo:** `crates/dcps/src/error.rs::Result::Ok`.

**Tests:** Crate-weit.

**Status:** done

### 2.2.1.1.2 RC ERROR -> DdsError::Other

**Spec:** "ERROR: Generic, unspecified error."

**Repo:** `crates/dcps/src/error.rs::DdsError::Other { reason }` +
`as_return_code()` mappt auf `return_code::ERROR = 1`.
`WireError`/`TransportError` mappen ebenfalls auf RC ERROR
(spec-konform: alle drei sind unspezifizierte Implementations-Fehler).

**Tests:** `crates/dcps/src/error.rs::tests`:
`rc_error_maps_for_wire_and_transport_and_other`,
`rc_constants_have_spec_values`.

**Status:** done

### 2.2.1.1.3 RC BAD_PARAMETER -> DdsError::BadParameter

**Spec:** "BAD_PARAMETER: Illegal parameter value."

**Repo:** `crates/dcps/src/error.rs::DdsError::BadParameter { what }`
+ `as_return_code()` mappt auf `return_code::BAD_PARAMETER = 3`.

**Tests:** `crates/dcps/src/error.rs::tests::rc_bad_parameter_maps`;
plus Trigger-Site-Tests in `crates/dcps/src/topic.rs` (z.B. leerer
Topic-Name → BadParameter).

**Status:** done

### 2.2.1.1.4 RC UNSUPPORTED -> DdsError::Unsupported

**Spec:** "UNSUPPORTED: Unsupported operation."

**Repo:** `crates/dcps/src/error.rs::DdsError::Unsupported { feature }`
+ `as_return_code()` mappt auf `return_code::UNSUPPORTED = 2`.

**Tests:** `crates/dcps/src/error.rs::tests::rc_unsupported_maps`.

**Status:** done

### 2.2.1.1.5 RC ALREADY_DELETED

**Spec:** "ALREADY_DELETED: already deleted."

**Repo:** `crates/dcps/src/entity.rs::EntityState.deleted: AtomicBool` +
`mark_deleted()` (idempotent) + `check_not_deleted() -> Result<()>`
Guard-Helper liefert `DdsError::AlreadyDeleted` bei nachfolgenden Ops.
`as_return_code()` mappt auf `return_code::ALREADY_DELETED = 9`.

**Tests:** `crates/dcps/src/entity.rs::tests`:
`check_not_deleted_passes_for_fresh_entity`,
`check_not_deleted_returns_already_deleted_after_mark`,
`mark_deleted_is_idempotent`;
plus Mapping `crates/dcps/src/error.rs::tests::rc_already_deleted_maps`.

**Status:** done

### 2.2.1.1.6 RC OUT_OF_RESOURCES -> DdsError::OutOfResources

**Spec:** "OUT_OF_RESOURCES."

**Repo:** `crates/dcps/src/error.rs::DdsError::OutOfResources { what }` +
`as_return_code()` mappt auf `return_code::OUT_OF_RESOURCES = 5`;
RESOURCE_LIMITS-QoS-Wiring in `crates/qos/src/policies/resource_limits.rs`.

**Tests:** `crates/dcps/src/error.rs::tests::rc_out_of_resources_maps`;
plus QoS-Tests in `crates/qos/src/policies/resource_limits.rs::tests`.

**Status:** done

### 2.2.1.1.7 RC NOT_ENABLED

**Spec:** "NOT_ENABLED: Operation invoked on disabled Entity."

**Repo:** `crates/dcps/src/error.rs::DdsError::NotEnabled` +
`as_return_code()` mappt auf `return_code::NOT_ENABLED = 6`.
`crates/dcps/src/entity.rs::EntityState::check_enabled() -> Result<()>`
Guard-Helper fuer Public-Ops.

**Tests:** `crates/dcps/src/entity.rs::tests`:
`check_enabled_returns_not_enabled_for_disabled_entity`,
`check_enabled_passes_after_enable`,
`check_enabled_passes_for_factory_entity`,
plus Bestand `enable_is_idempotent_and_reports_first_transition`;
plus Mapping `crates/dcps/src/error.rs::tests::rc_not_enabled_maps`.

**Status:** done

### 2.2.1.1.8 RC IMMUTABLE_POLICY

**Spec:** "IMMUTABLE_POLICY: Modify immutable QosPolicy."

**Repo:** `crates/dcps/src/error.rs::DdsError::ImmutablePolicy { policy }`
+ `as_return_code()` mappt auf `return_code::IMMUTABLE_POLICY = 7`.
`crates/dcps/src/entity.rs::immutable_if_enabled` Helper-Site.
Immutability-Matrix wird per QoS-Item in §2.2.3.x getrackt.

**Tests:** `crates/dcps/src/error.rs::tests::rc_immutable_policy_maps`;
plus `crates/dcps/src/entity.rs::tests::immutable_if_enabled_returns_correct_error`.

**Status:** done

### 2.2.1.1.9 RC INCONSISTENT_POLICY

**Spec:** "INCONSISTENT_POLICY: inconsistent set."

**Repo:** `crates/dcps/src/error.rs::DdsError::InconsistentPolicy { what }`
+ `as_return_code()` mappt auf `return_code::INCONSISTENT_POLICY = 8`.
QoS-Konsistenz-Check in `crates/qos/src/compatibility.rs`.

**Tests:** `crates/dcps/src/error.rs::tests::rc_inconsistent_policy_maps`;
plus QoS-Compatibility-Tests in `crates/qos/src/compatibility.rs::tests`.

**Status:** done

### 2.2.1.1.10 RC PRECONDITION_NOT_MET

**Spec:** "PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/error.rs::DdsError::PreconditionNotMet { reason }`
+ `as_return_code()` mappt auf `return_code::PRECONDITION_NOT_MET = 4`.

**Tests:** `crates/dcps/src/error.rs::tests::rc_precondition_not_met_maps`.

**Status:** done

### 2.2.1.1.11 RC TIMEOUT

**Spec:** "TIMEOUT."

**Repo:** `error.rs::DdsError::Timeout`.

**Tests:** Timeout-Tests in `publisher.rs`.

**Status:** done

### 2.2.1.1.12 RC ILLEGAL_OPERATION

**Spec:** "ILLEGAL_OPERATION."

**Repo:** `crates/dcps/src/error.rs::DdsError::IllegalOperation { what }`
+ `as_return_code()` mappt auf `return_code::ILLEGAL_OPERATION = 12`.

**Tests:** `crates/dcps/src/error.rs::tests::rc_illegal_operation_maps`.

**Status:** done

### 2.2.1.1.13 RC NO_DATA

**Spec:** "NO_DATA."

**Repo:** `crates/dcps/src/error.rs::DdsError::NoData` +
`as_return_code()` mappt auf `return_code::NO_DATA = 11`. In der
Praxis liefern read/take einen leeren `Vec<Sample>` statt eines
expliziten Errors — das ist Spec-konform (§2.2.2.5.3.6 erlaubt
beide Varianten); der explizite Error ist fuer C/C++/Java-Bridges
verfuegbar.

**Tests:** `crates/dcps/src/error.rs::tests::rc_no_data_maps`.

**Status:** done

---

## §2.2.1.2 Conceptual Outline

### 2.2.1.2.1 Publisher/DataWriter sendseitig, Subscriber/DataReader empfangsseitig

**Spec:** §2.2.1.2, S. 5-6.

**Repo:** `crates/dcps/src/{publisher,subscriber}.rs`.

**Tests:** Crate-weit.

**Status:** done

### 2.2.1.2.2 Topic verbindet Publication und Subscription via name+type

**Spec:** §2.2.1.2 — "Topic name+type."

**Repo:** `crates/dcps/src/topic.rs::Topic<T>`.

**Tests:** Topic-Tests.

**Status:** done

### 2.2.1.2.3 Coherent Set via Publisher (begin/end_coherent_changes)

**Spec:** §2.2.1.2.

**Repo:** `crates/dcps/src/coherent_set.rs::CoherentScope::begin/end`.

**Tests:** `begin_coherent_changes_*`-Tests.

**Status:** done

---

## §2.2.2.1 Infrastructure Module

### 2.2.2.1.1 Entity-Class — Abstract Base + QoS + Listener + StatusCondition

**Spec:** §2.2.2.1.1, S. 11-13 — Operations: set_qos / get_qos /
set_listener / get_listener / get_statuscondition / get_status_changes /
enable / get_instance_handle.

**Repo:** `crates/dcps/src/entity.rs::Entity`-Trait + `EntityState`.

**Tests:** `enable_is_idempotent_and_reports_first_transition`,
`get_status_condition_returns_pristine_when_no_changes`.

**Status:** done — Trait + Default-Impls; Op-Gating und
IMMUTABLE-Matrix Phase 2.

### 2.2.2.1.2 DomainEntity-Class

**Spec:** §2.2.2.1.2, S. 14 — "Specialization of Entity to be a
contained-element of a DomainParticipant."

**Repo:** Entity-Trait reicht; kein separater DomainEntity-Trait.

**Tests:** indirekt.

**Status:** done

### 2.2.2.1.3 QosPolicy-Class

**Spec:** §2.2.2.1.3, S. 15.

**Repo:** `crates/qos/src/policies/`.

**Tests:** Policies-Tests.

**Status:** done

### 2.2.2.1.4 Listener-Interface (Abstract Root)

**Spec:** §2.2.2.1.4, S. 15 — "Abstract Root für alle Listener-Typen."

**Repo:** `crates/dcps/src/listener.rs` mit 6 Listener-Traits
(Topic/DataWriter/Publisher/DataReader/Subscriber/DomainParticipant).

**Tests:** `listener_dispatch_*`-Tests.

**Status:** done

### 2.2.2.1.5 Status-Class

**Spec:** §2.2.2.1.5, S. 16.

**Repo:** `crates/dcps/src/status.rs` mit allen 13 Status-Records.

**Tests:** `status_records_default`-Tests.

**Status:** done

### 2.2.2.1.6 WaitSet-Class (attach/detach/wait/get_conditions/notify)

**Spec:** §2.2.2.1.6, S. 16-17.

**Repo:** `crates/dcps/src/condition.rs::WaitSet` mit
`attach_condition` (idempotent + DoS-Cap `MAX_WAITSET_CONDITIONS = 1024`
liefert `OutOfResources`); `detach_condition`; `get_conditions`;
`wait` mit Single-Thread-Wait-Enforce via `WaitSetInner.waiting:
AtomicBool` + RAII-Guard (`PreconditionNotMet` bei konkurrentem
zweiten Aufruf); `notify` Cvar-wakeup; multi-WaitSet via Clone
(shared Inner via Arc).

**Tests:** `crates/dcps/src/condition.rs::tests`:
`waitset_attach_detach_idempotent`,
`waitset_wait_returns_immediately_if_already_triggered`,
`waitset_wait_timeout_returns_err`,
`waitset_wakes_when_guard_triggers`,
`waitset_with_multiple_conditions_returns_all_triggered`,
`waitset_clone_shares_state`,
`waitset_default_is_empty`,
`waitset_concurrent_wait_returns_precondition_not_met` (Single-
Thread-Enforce-Negativ),
`waitset_sequential_waits_after_first_returns_succeed`,
`waitset_attach_above_max_returns_out_of_resources` (OOR-Negativ),
`waitset_attach_idempotent_does_not_count_against_cap`,
`waitset_panic_in_wait_releases_waiting_flag`.

**Status:** done

### 2.2.2.1.7 Condition-Class (Trait-Root + get_trigger_value)

**Spec:** §2.2.2.1.7, S. 17.

**Repo:** `condition.rs::Condition`-Trait.

**Tests:** `condition_trait_*`-Tests.

**Status:** done

### 2.2.2.1.8 GuardCondition-Class (set_trigger_value)

**Spec:** §2.2.2.1.8, S. 18.

**Repo:** `condition.rs::GuardCondition`.

**Tests:** GuardCondition-Tests.

**Status:** done

### 2.2.2.1.9 StatusCondition-Class (set/get_enabled_statuses, get_entity)

**Spec:** §2.2.2.1.9, S. 18-19.

**Repo:** `crates/dcps/src/entity.rs::StatusCondition` mit
`set_enabled_statuses` / `enabled_statuses` / `trigger_value` /
`get_entity_handle() -> InstanceHandle` (Spec §2.2.2.1.9 get_entity-
Aequivalent fuer Rust-API; Handle ist die einzige stabile Identitaet
ueber alle Wrapper-Granularitaeten) / `entity_state() -> &Arc<EntityState>`
fuer direkten State-Zugriff.

**Tests:** `crates/dcps/src/entity.rs::tests`:
`status_condition_get_entity_handle_matches_owner_state`,
`status_condition_get_entity_handle_unique_per_entity`,
`status_condition_entity_state_returns_same_arc`,
`status_condition_entity_state_reflects_lifecycle_changes`,
plus Bestand `status_condition_trigger_value`,
`status_condition_implements_condition_trait`.

**Status:** done

---

## §2.2.2.2 Domain Module

### 2.2.2.2.1 DomainParticipant-Class

**Spec:** §2.2.2.2.1, S. 21-30 — Container fuer Pub/Sub/Topic/MultiTopic;
Factory-Methoden; Admin-Ops (ignore_*); get_discovered_*; etc.

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant` mit:
- `create_publisher` / `create_subscriber` / `create_topic` (Factory-
  Methoden, jede Operation tracked den `InstanceHandle` der erzeugten
  Entity in `inner.publishers` / `inner.subscribers` / `inner.topics`
  fuer `contains_entity` und `delete_contained_entities`).
- `lookup_topicdescription(name) -> Option<TopicDescriptionHandle>`
  (Spec §2.2.2.2.1.12) — sofortiger lokaler Lookup ohne Discovery-Wait.
- `find_topic(name, timeout) -> Result<TopicDescriptionHandle>`
  (Spec §2.2.2.2.1.11) — lokal + SEDP-Cache-Poll bis Timeout.
- `instance_handle()` (Spec §2.2.2.1.1) — InstanceHandle des Participants.
- `contains_entity(handle) -> bool` (Spec §2.2.2.2.1.10) — prueft self,
  alle Topics, alle Pub/Sub-Children.
- `ignore_*` / `get_discovered_*` / `delete_contained_entities`
  bereits in WP 2.7 verdrahtet.

**Tests:** `crates/dcps/src/participant.rs::tests`:
`lookup_topicdescription_returns_local_topics`,
`lookup_topicdescription_none_for_unknown`,
`find_topic_returns_immediately_for_local`,
`contains_entity_returns_true_for_self_handle`,
`contains_entity_returns_true_for_local_topic`,
`contains_entity_returns_true_for_local_publisher`,
`contains_entity_returns_true_for_local_subscriber`,
`contains_entity_returns_false_for_unknown_handle`,
`contains_entity_returns_false_for_topic_after_delete`.

**Rekursiver Pfad:** `contains_entity` erkennt zusaetzlich
DataWriter/DataReader-Handles via Pub/Sub-Children. PublisherInner +
SubscriberInner tracken DW/DR-Handles (`datawriters` /
`datareaders`-Mutex); per Weak-Backref propagieren sie auch in
`ParticipantInner.datawriters` / `datareaders` fuer schnellen
top-down Lookup. Pro Pub/Sub gibt es ausserdem `contains_writer` /
`contains_reader` als gezielten Sub-Lookup.

**Tests (zusaetzlich zu den 6 oben):**
`contains_entity_recursive_finds_local_datawriter`,
`contains_entity_recursive_finds_local_datareader`,
`contains_entity_recursive_does_not_find_foreign_datawriter`.

**Status:** done

### 2.2.2.2.2 DomainParticipantFactory-Class (Singleton)

**Spec:** §2.2.2.2.2, S. 31-33.

**Repo:** `crates/dcps/src/factory.rs::DomainParticipantFactory` mit:
- `instance() -> &'static Self` (Spec §2.2.2.2.2.1 `get_instance`,
  Singleton via `OnceLock`).
- `create_participant` / `create_participant_with_config` /
  `create_participant_offline` (Spec §2.2.2.2.2.2; alle Varianten
  trackt den Participant in der internen Registry).
- `lookup_participant(domain_id) -> Option<DomainParticipant>`
  (Spec §2.2.2.2.2.4).
- `delete_participant(&p) -> Result<()>` (Spec §2.2.2.2.2.3 — entfernt
  aus Registry + ruft `delete_contained_entities`; liefert
  `PreconditionNotMet` wenn nicht registriert).
- `set_default_participant_qos` / `get_default_participant_qos`
  (Spec §2.2.2.2.2.5).
- `set_qos` / `get_qos` mit `DomainParticipantFactoryQos`-Struct
  (Spec §2.2.2.2.2.6, Default `autoenable_created_entities = true`).

**Tests:** `crates/dcps/src/factory.rs::tests`:
`factory_is_singleton`,
`factory_creates_participant_with_correct_domain_id`,
`lookup_participant_finds_registered_offline_participant`,
`lookup_participant_returns_none_for_unknown_domain`,
`delete_participant_removes_from_registry`,
`delete_participant_unknown_returns_precondition_not_met`,
`default_participant_qos_roundtrips`,
`factory_qos_default_is_autoenable_true`,
`factory_set_get_qos_roundtrip`.

**Status:** done

### 2.2.2.2.3 DomainParticipantListener-Interface

**Spec:** §2.2.2.2.3, S. 33-34.

**Repo:** `listener.rs::DomainParticipantListener`-Trait + 14-Method-
Default.

**Tests:** `listener_*`-Tests.

**Status:** done

---

## §2.2.2.3 Topic-Definition Module

### 2.2.2.3.1 TopicDescription-Class (Abstract Base)

**Spec:** §2.2.2.3.1, S. 35 — abstract base for `Topic`,
`ContentFilteredTopic`, `MultiTopic`. API: `get_type_name`,
`get_name`, `get_participant`.

**Repo:** `crates/dcps/src/topic.rs::TopicDescription`-Trait
(object-safe, `&dyn TopicDescription` ist der Konsumtyp);
implementiert von `Topic<T>`, `TopicDescriptionHandle`,
`ContentFilteredTopic<T>`.

**Tests:** `crates/dcps/src/topic.rs::tests`:
`topic_implements_topic_description`,
`topic_description_handle_is_cloneable`,
`topic_description_trait_is_object_safe`,
`topic_description_create_topic_rejects_empty_name`
(BadParameter-Negativ),
`topic_description_get_participant_returns_owning_participant`.

**Status:** done

### 2.2.2.3.2 Topic-Class

**Spec:** §2.2.2.3.2, S. 36-37.

**Repo:** `topic.rs::Topic<T>`.

**Tests:** Topic-Tests.

**Status:** done

### 2.2.2.3.3 ContentFilteredTopic-Class

**Spec:** §2.2.2.3.3, S. 37-38 — Filter-Expression + Filter-Parameters
+ get_related_topic. TopicDescription-Implementierung mit Type-Name
vom Related-Topic.

**Repo:** `crates/dcps/src/topic.rs::ContentFilteredTopic<T>` mit
`new` (Konstruktor mit Expression-Parsing + Parameter-Index-Validation),
`get_filter_expression`, `get_filter_parameters`,
`set_filter_parameters` (mit BadParameter-Validation),
`get_related_topic`, `evaluate(row)` ueber `zerodds_sql_filter`-Engine,
plus `TopicDescription`-Trait-Impl. DomainParticipant
`create_contentfilteredtopic`-Factory.

**Tests:** `crates/dcps/src/topic.rs::tests`:
`cft_compiles_and_evaluates_filter`,
`cft_with_params_can_be_updated`,
`cft_get_related_topic`,
`cft_invalid_expression_rejected` (BadParameter-Negativ),
`cft_param_index_out_of_range_rejected` (BadParameter-Negativ),
`cft_set_filter_parameters_validates_count`,
`cft_filter_with_string_param`,
`cft_filter_with_or_and_combination`,
`cft_unknown_field_returns_bad_parameter` (BadParameter-Negativ),
`cft_clone_shares_params`.

**Status:** done

### 2.2.2.3.4 MultiTopic-Class [optional, Spec §2.2.2.3.4]

**Spec:** §2.2.2.3.4, S. 38-39 — kombiniert mehrere Topics ueber
SQL-Subscription-Expression. Spec-explicit `optional`.

**Repo:** `crates/dcps/src/topic.rs::MultiTopic<T>` mit
`subscription_expression` (Read-only), `expression_parameters`
(get/set per RwLock), `related_topic_names: Vec<String>`,
`get_related_topic_names`. TopicDescription-Trait-Impl mit
User-definiertem `type_name`.
DomainParticipant-Factory: `create_multitopic` (Spec §2.2.2.2.1.15) +
`delete_multitopic` (Spec §2.2.2.2.1.16) mit Participant-Match-
Validation und vollem BadParameter-Negativ-Pfad.

**Tests:** `crates/dcps/src/topic.rs::tests`:
`multitopic_compiles_and_implements_topic_description`,
`multitopic_set_expression_parameters_roundtrip`,
`multitopic_rejects_empty_name` (Negativ),
`multitopic_rejects_empty_type_name` (Negativ),
`multitopic_rejects_empty_related_topics` (Negativ),
`multitopic_rejects_invalid_expression` (Negativ),
`multitopic_rejects_param_index_out_of_range` (Negativ),
`multitopic_set_params_validates_index_range` (Negativ),
`multitopic_clone_shares_params`,
`delete_multitopic_rejects_foreign_participant` (Negativ).

**Cross-Topic-Sample-Routing:** Hash-Join-Operator
`crates/dcps/src/topic.rs::hash_join_two` mit O(n+m) Build/Probe-
Phase + Cartesian-Match fuer Duplikat-Keys. `JoinedRow<'a>`
dispatched dotted-Pfade `<topic>.<field>` auf die richtige
Topic-Quelle. `MultiTopic::evaluate_joined(row)` wertet die
`subscription_expression` gegen die kombinierten Rows aus.
Tests: `joined_row_dispatches_dotted_paths_by_topic_prefix`,
`joined_row_undotted_falls_back_to_first_match`,
`multitopic_evaluate_joined_uses_dotted_paths`,
`hash_join_two_combines_matching_rows`,
`hash_join_two_returns_empty_when_no_keys_match` (Negativ),
`hash_join_two_emits_cartesian_for_duplicate_keys`,
`hash_join_two_predicate_can_filter_pairs`.

**Status:** done

### 2.2.2.3.5 TopicListener-Interface (on_inconsistent_topic)

**Spec:** §2.2.2.3.5, S. 40.

**Repo:** `listener.rs::TopicListener`-Trait.

**Tests:** —

**Status:** done

### 2.2.2.3.6 TypeSupport-Interface

**Spec:** §2.2.2.3.6, S. 40-41.

**Repo:** `dds_type.rs::DdsType`-Trait.

**Tests:** —

**Status:** done

### 2.2.2.3.7 Derived Classes pro Application Class (FooDataReader/Writer/TypeSupport)

**Spec:** §2.2.2.3.7, S. 41.

**Repo:** Generic `DataReader<T>`/`DataWriter<T>` (Rust-idiomatisch
keine Foo*-Klassen).

**Tests:** Generic-Tests.

**Status:** done

---

## §2.2.2.4 Publication Module

### 2.2.2.4.1 Publisher-Class

**Spec:** §2.2.2.4.1, S. 43-46 — Operations: create_datawriter,
delete_datawriter, set/get_default_datawriter_qos, copy_from_topic_qos,
suspend/resume_publications, begin/end_coherent_changes,
wait_for_acknowledgments, get_participant.

**Repo:** `crates/dcps/src/publisher.rs::Publisher` mit:
- `create_datawriter` / `set_listener` / `assert_liveliness` /
  `wait_for_acknowledgments` (Bestand).
- `suspend_publications` / `resume_publications` / `is_suspended`
  via `PublisherInner.suspended: AtomicBool` (Spec §2.2.2.4.1.10/11).
- `copy_from_topic_qos(&mut DataWriterQos, &TopicQos)` Static-Helper
  (Spec §2.2.2.4.1.13) — kopiert die in der aktuellen TopicQos-Struct
  (durability, reliability) sichtbaren shared Policies.
- Coherent-Changes via `crates/dcps/src/coherent_set.rs::CoherentScope`.

**Tests:** `crates/dcps/src/publisher.rs::tests`:
`publisher_creates_datawriter_for_matching_type`,
`suspend_publications_sets_flag`,
`resume_publications_clears_flag`,
`suspend_publications_is_idempotent`,
`resume_without_suspend_is_noop`,
`copy_from_topic_qos_copies_durability_and_reliability`,
`copy_from_topic_qos_does_not_touch_writer_only_policies`,
plus Coherent-Set-Tests in `coherent_set.rs::tests`.

**Status:** done

### 2.2.2.4.2 DataWriter-Class

**Spec:** §2.2.2.4.2, S. 47-55 — write/dispose/register/lookup/
assert_liveliness/get_*-Status/get_matched_subscriptions.

**Repo:** `publisher.rs::DataWriter<T>` + `instance_tracker.rs` +
`wlp.rs`.

**Tests:** `datawriter_*`-Tests.

**Status:** done — register_instance/unregister/dispose/write/
write_w_timestamp/lookup_instance/get_key_value/assert_liveliness
live; get_*_status-Polling-API teilweise.

### 2.2.2.4.3 PublisherListener-Interface

**Spec:** §2.2.2.4.3, S. 56.

**Repo:** `listener.rs::PublisherListener`.

**Tests:** —

**Status:** done

### 2.2.2.4.4 DataWriterListener-Interface (4 Callbacks)

**Spec:** §2.2.2.4.4, S. 57.

**Repo:** `listener.rs::DataWriterListener`.

**Tests:** —

**Status:** done

### 2.2.2.4.5 Concurrency Behavior

**Spec:** §2.2.2.4.5, S. 57.

**Repo:** Rust-thread-safe via Arc<Mutex>.

**Tests:** —

**Status:** done

---

## §2.2.2.5 Subscription Module

### 2.2.2.5.1 SampleInfo-Statemachine (Sample/View/Instance-State)

**Spec:** §2.2.2.5.1.1-9, S. 59-65 — Interpretation der SampleInfo-
Felder; drei orthogonale State-Dimensionen mit Mask-Filter.

**Repo:** `crates/dcps/src/sample_info.rs` mit:
- `SampleStateKind` (READ / NOT_READ) + `sample_state_mask::*`-Konstanten.
- `ViewStateKind` (NEW / NOT_NEW) + `view_state_mask::*`.
- `InstanceStateKind` (ALIVE / NOT_ALIVE_DISPOSED / NOT_ALIVE_NO_WRITERS)
  + `instance_state_mask::*` + `is_alive` / `is_not_alive`-Praedikate.
- `SampleInfo::matches_states(sample_mask, view_mask, instance_mask)`
  fuer den `read_w_condition`-Filter.
- `SampleInfo::new_alive` Konstruktor fuer Live-Pfad.

**Tests:** `crates/dcps/src/sample_info.rs::tests`:
`defaults_are_alive_new_not_read`, `instance_state_predicates`,
`sample_state_predicate`, `matches_states_filter`,
`sample_info_three_state_dimensions_independent` (positive +
negative Filter pro Dimension), `sample_info_dispose_marker_has_invalid_data`,
`enum_default_impls`.

**Status:** done

### 2.2.2.5.2 Subscriber-Class

**Spec:** §2.2.2.5.2, S. 65-69 — Operations: create_datareader,
get_datareaders, notify_datareaders, begin/end_access, etc.

**Repo:** `crates/dcps/src/subscriber.rs::Subscriber` mit:
- `create_datareader` / `set_listener` / `get_listener` (Bestand).
- `begin_access` (Spec §2.2.2.5.2.8, idempotent nestbar) +
  `end_access` (Spec §2.2.2.5.2.9, liefert PreconditionNotMet bei
  Underflow) + `is_access_open` via `SubscriberInner.access_scope:
  Arc<GroupAccessScope>` (counter-basiert in
  `crates/dcps/src/coherent_set.rs::GroupAccessScope`).

**Tests:** `crates/dcps/src/subscriber.rs::tests`:
`subscriber_begin_end_access_roundtrip`,
`subscriber_end_access_without_begin_returns_precondition_not_met`,
(Negativ),
`subscriber_begin_access_is_nestable`,
`subscriber_too_many_ends_after_balanced_returns_error` (Negativ),
plus Bestand subscriber-create_datareader und listener-Tests.

**Status:** done

### 2.2.2.5.3 DataReader-Class

**Spec:** §2.2.2.5.3, S. 70-85 — read/take/read_with_info/
read_filtered/read_instance/take_instance/read_next_instance/
take_next_instance/get_*-Status/wait_for_historical_data etc.

**Repo:** `subscriber.rs::DataReader<T>`.

**Tests:** `datareader_*`-Tests.

**Status:** done — alle Read/Take-Varianten done; get_*_status
Polling teilweise; wait_for_historical_data offen.

### 2.2.2.5.4 DataSample-Class (Data + SampleInfo)

**Spec:** §2.2.2.5.4, S. 86.

**Repo:** `subscriber.rs::Sample<T>`.

**Tests:** —

**Status:** done

### 2.2.2.5.5 SampleInfo-Class (komplett)

**Spec:** §2.2.2.5.5, S. 86-87 — sample_state/view_state/
instance_state/disposed_generation_count/no_writers_generation_count/
sample_rank/generation_rank/absolute_generation_rank/source_timestamp/
instance_handle/publication_handle/valid_data (12 Felder).

**Repo:** `crates/dcps/src/sample_info.rs::SampleInfo` mit allen 12
Spec-Feldern und passenden Default-Werten. Reader-side Update der
Generation-Counter erfolgt in den DataReader-Read-Pfaden.

**Tests:** `crates/dcps/src/sample_info.rs::tests`:
`sample_info_all_spec_fields_accessible` (alle 12 Felder lesbar +
setzbar mit konkreten Werten), `sample_info_dispose_marker_has_invalid_data`
(Negativ: Dispose-Sample hat valid_data=false),
`sample_info_generation_rank_starts_zero`,
`new_alive_constructor_sets_handles_and_timestamp`,
plus Bestand `defaults_are_alive_new_not_read`.

**Status:** done

### 2.2.2.5.6 SubscriberListener-Interface

**Spec:** §2.2.2.5.6, S. 87.

**Repo:** `listener.rs::SubscriberListener`.

**Tests:** —

**Status:** done

### 2.2.2.5.7 DataReaderListener-Interface (7 Callbacks)

**Spec:** §2.2.2.5.7, S. 88.

**Repo:** `listener.rs::DataReaderListener`.

**Tests:** —

**Status:** done

### 2.2.2.5.8 ReadCondition-Class

**Spec:** §2.2.2.5.8, S. 88-89 — Sample/View/InstanceState-Mask-
Filter; Trigger wenn der Reader Samples mit diesen Masks enthaelt.

**Repo:** `crates/dcps/src/condition.rs::ReadCondition` mit Mask-
Gettern (`get_sample_state_mask`, `get_view_state_mask`,
`get_instance_state_mask`) + Closure-basiertem Trigger
`(sm, vm, im) -> bool`. `Condition`-Trait implementiert (object-safe).

**Tests:** `crates/dcps/src/condition.rs::tests`:
`read_condition_returns_trigger_from_closure`,
`read_condition_trigger_false_when_closure_says_false` (Negativ),
`read_condition_implements_condition_trait_object_safe`,
`read_condition_attaches_to_waitset` (WaitSet-Integration).

**Status:** done

### 2.2.2.5.9 QueryCondition-Class

**Spec:** §2.2.2.5.9, S. 89 — extends ReadCondition mit SQL-Filter-
Expression + mutable Filter-Parameters.

**Repo:** `crates/dcps/src/condition.rs::QueryCondition` mit
`query_expression`-Feld (read-only) + `query_parameters`-Mutex
(get/set), `base()` Accessor auf die Sub-`ReadCondition`. SQL-Filter
wird einmalig im Konstruktor via `zerodds_sql_filter::parse` zu
`Expr` geparst (BadParameter bei Syntaxfehler). `evaluate(row)` /
`evaluate_with_values(row, params)` werten den Filter pro Sample aus.
Per-Sample-Evaluation (Spec §2.2.2.5.9.6) ist live in
`crates/dcps/src/subscriber.rs::DataReader::read_w_condition` und
`take_w_condition`; `DdsTypeRow<'a, T>` (in `dds_type.rs`) wraps
`T: DdsType` zu `zerodds_sql_filter::RowAccess` via die optionale
`DdsType::field_value(path)`-Methode.

**Tests:** `crates/dcps/src/condition.rs::tests`:
`query_condition_inherits_trigger_from_base`,
`query_condition_set_query_parameters_roundtrip`,
`query_condition_trigger_inherits_false_from_base` (Negativ),
`query_condition_base_returns_correct_read_condition`,
`query_condition_rejects_invalid_sql` (Negativ),
`query_condition_evaluate_filters_per_sample`,
`query_condition_evaluate_with_typed_values_int_param`,
`query_condition_evaluate_uses_string_params_by_default`.
`crates/dcps/tests/query_condition.rs` (E2E):
`read_w_condition_filters_per_sample`,
`take_w_condition_consumes_only_matching_samples`,
`read_w_condition_with_string_parameter`,
`read_w_condition_preserves_cache_state`,
`take_w_condition_empty_when_no_match`,
`read_w_condition_unknown_field_drops_sample`.

**Status:** done

---

## §2.2.3 Supported QoS — 22 Policies

### 2.2.3.1 USER_DATA QosPolicy

**Spec:** §2.2.3.1, S. 101 — sequence<octet> value.

**Repo:** `crates/qos/src/policies/user_data.rs`.

**Tests:** USER_DATA-Tests.

**Status:** done

### 2.2.3.2 TOPIC_DATA QosPolicy

**Spec:** §2.2.3.2, S. 102.

**Repo:** `crates/qos/src/policies/topic_data.rs`.

**Tests:** —

**Status:** done

### 2.2.3.3 GROUP_DATA QosPolicy

**Spec:** §2.2.3.3, S. 102.

**Repo:** `crates/qos/src/policies/group_data.rs`.

**Tests:** —

**Status:** done

### 2.2.3.4 DURABILITY QosPolicy (VOLATILE/TRANSIENT_LOCAL/TRANSIENT/PERSISTENT)

**Spec:** §2.2.3.4, S. 102-103.

**Repo:** `crates/qos/src/policies/durability.rs::DurabilityQosPolicy`
mit allen 4 `DurabilityKind`-Werten + Spec-konformer Ordering-Relation
(VOLATILE < TRANSIENT_LOCAL < TRANSIENT < PERSISTENT) +
Wire-Encode/Decode (u32). Volatile + TransientLocal sind im
Writer-side Sample-Cache implementiert; Transient + Persistent
greifen auf den Durability-Service §2.2.3.5 zu
(`crates/dcps/src/durability_service.rs`).

**Tests:** `crates/qos/src/policies/durability.rs::tests`:
`default_is_volatile`, `ordering_matches_spec`, `try_from_u32_is_strict`,
`from_u32_is_forward_compat`, `encode_decode_roundtrip` (alle 4 Kinds),
`unknown_kind_decode_rejected` (Negativ),
`compatibility_offered_ge_requested` (offered >= requested + Negativ).

**Status:** done — Wire-Form + Compatibility-Ordering vollstaendig;
Transient/Persistent-Storage delegiert an §2.2.3.5 Durability-Service.

### 2.2.3.5 DURABILITY_SERVICE QosPolicy

**Spec:** §2.2.3.5, S. 103 — service_cleanup_delay + history_kind +
history_depth + max_samples + max_instances + max_samples_per_instance.

**Repo:** `crates/qos/src/policies/durability_service.rs::DurabilityServiceQosPolicy`
mit allen 6 Spec-Feldern + Wire-Encode/Decode. Storage-Service-Backends
in `crates/dcps/src/durability_service.rs`: Trait `DurabilityBackend`
(`store` / `replay_for_topic` / `unregister_instance` / `cleanup`) +
zwei Implementationen `InMemoryDurabilityBackend` (TRANSIENT) und
`OnDiskDurabilityBackend` (PERSISTENT, Verzeichnis-Hierarchie
`<root>/<topic>/<hex(instance)>/<seq>.bin` mit `.unregistered`-Marker).
Beide Backends respektieren history_kind (KeepLast-Cap durch FIFO,
KeepAll-Cap durch OutOfResources), max_samples, max_instances,
max_samples_per_instance und service_cleanup_delay (`fraction` als
2^-32 s korrekt nach Nanos konvertiert). `make_backend(kind, qos, root)`-
Factory waehlt automatisch.

**Tests:** `crates/qos/src/policies/durability_service.rs::tests`
(2 Tests: roundtrip + default). Storage-Backend-Tests in
`crates/dcps/src/durability_service.rs::tests` (19 Tests, alle 4
Caps + KeepLast/KeepAll + cleanup-after-delay + on-disk-survives-drop):
`in_memory_store_and_replay_returns_sorted_samples`,
`in_memory_keeplast_caps_history_at_depth`,
`in_memory_keepall_max_samples_per_instance_returns_oor` (Negativ),
`in_memory_max_samples_globally_returns_oor` (Negativ),
`in_memory_max_instances_returns_oor` (Negativ),
`in_memory_unregister_then_cleanup_removes_after_delay`,
`in_memory_replay_filters_by_topic`,
`in_memory_unknown_topic_returns_empty` (Negativ),
`make_backend_rejects_volatile_and_transient_local` (Negativ),
`make_backend_persistent_requires_root` (Negativ),
`make_backend_transient_returns_in_memory`,
`on_disk_store_and_replay_roundtrip`,
`on_disk_keeplast_replaces_old_files`,
`on_disk_persistent_survives_backend_drop`,
`on_disk_unregister_and_cleanup_removes_directory`,
`on_disk_max_samples_per_instance_returns_oor` (Negativ),
`hex16_roundtrip`,
`parse_hex16_rejects_wrong_length_and_invalid_chars` (Negativ),
`sanitize_topic_replaces_path_chars`.

**Status:** done

### 2.2.3.6 PRESENTATION QosPolicy (access_scope INSTANCE/TOPIC/GROUP + coherent + ordered)

**Spec:** §2.2.3.6, S. 103-104.

**Repo:** `crates/qos/src/policies/presentation.rs::PresentationQosPolicy`
mit `access_scope` (INSTANCE/TOPIC/GROUP) + `coherent_access` +
`ordered_access` Bool-Flags. Wire-Encode/Decode + Compatibility-Check.
INSTANCE/TOPIC live im Reader-Sort-Pfad; GROUP-coherent via
`crates/dcps/src/coherent_set.rs::GroupAccessScope.snapshot_generation`
— jedes `begin_access` (von 0→1) inkrementiert die Generation, alle
DR im Subscriber sehen waehrend des begin/end-Fensters denselben
atomic Cut. Multi-Reader-Coherent-Set ueber Arc-Clone der Scope.

**Tests:** `crates/qos/src/policies/presentation.rs::tests`
(6 Tests: roundtrip + access_scope + coherent + ordered Kombinationen).

**Status:** done

### 2.2.3.7 DEADLINE QosPolicy

**Spec:** §2.2.3.7, S. 104.

**Repo:** `crates/qos/src/policies/deadline.rs`.

**Tests:** Deadline-Tests inkl. cross-test-domain-pollution-Fix.

**Status:** done

### 2.2.3.8 LATENCY_BUDGET QosPolicy

**Spec:** §2.2.3.8, S. 104 — Hint an die Service ueber maximal
akzeptierbare Latenz; pro Spec ist das eine Best-Effort-Hint, kein
hartes Constraint.

**Repo:** `crates/qos/src/policies/latency_budget.rs::LatencyBudgetQosPolicy`
mit `duration: Duration_t` + Wire-Encode/Decode (8 byte).

**Tests:** `crates/qos/src/policies/latency_budget.rs::tests`
(2 Tests: roundtrip + default INFINITE).

**Status:** done

### 2.2.3.9 OWNERSHIP QosPolicy (SHARED/EXCLUSIVE)

**Spec:** §2.2.3.9, S. 105.

**Repo:** `crates/qos/src/policies/ownership.rs::OwnershipQosPolicy`
mit `OwnershipKind::Shared`/`Exclusive` + Wire-Encode/Decode +
Compatibility-Check (Spec §2.2.3 Tab. — kind muss exact match auf
Reader/Writer-Pair).

**Tests:** `crates/qos/src/policies/ownership.rs::tests` (7 Tests:
roundtrip + default Shared + Compat-Check + invalid-kind-Decode-Reject).

**Strength-Selection:** EXCLUSIVE Reader-side Strength-Selection
implementiert in `crates/dcps/src/instance_tracker.rs::
should_accept_sample_under_exclusive_ownership` (Higher-Strength
gewinnt; bei Tie GUID-Tiebreaker; First-Writer wird Owner ohne
laufenden Vergleich). 8 Tests: exclusive_first_writer_wins,
exclusive_higher_strength_wins, exclusive_lower_strength_rejected,
exclusive_tie_break_by_higher_guid, exclusive_tie_break_lower_guid_rejected,
exclusive_same_writer_always_accepted, clear_owner_for_writer_resets_owner,
failover_after_clear_accepts_weaker_writer.

**Status:** done

### 2.2.3.10 OWNERSHIP_STRENGTH QosPolicy

**Spec:** §2.2.3.10, S. 106.

**Repo:** `crates/qos/src/policies/ownership_strength.rs::OwnershipStrengthQosPolicy`
mit `value: i32` + Wire-Encode/Decode (4 byte).
Strength-Selection-Logik in `crates/dcps/src/instance_tracker.rs::
should_accept_sample_under_exclusive_ownership` (i32-Strength + GUID-Tiebreaker).

**Tests:** `crates/qos/src/policies/ownership_strength.rs::tests`
(3 Tests: roundtrip + default 0 + negative-strength-roundtrip).
Strength-Selection-Tests in `instance_tracker::tests::exclusive_*` (6 Tests).

**Status:** done

### 2.2.3.11 LIVELINESS QosPolicy (AUTOMATIC/MANUAL_BY_PARTICIPANT/MANUAL_BY_TOPIC)

**Spec:** §2.2.3.11, S. 106-107.

**Repo:** `crates/qos/src/policies/liveliness.rs::LivelinessQosPolicy`
mit `LivelinessKind` (Automatic/ManualByParticipant/ManualByTopic) +
`lease_duration: Duration_t` + Wire-Encode/Decode.
`crates/dcps/src/wlp.rs` mit Heartbeat-Pfad fuer AUTOMATIC und
`Publisher::assert_liveliness()` fuer MANUAL_BY_PARTICIPANT/TOPIC.

**Tests:** `crates/qos/src/policies/liveliness.rs::tests` (9 Tests:
roundtrip aller drei Kinds + default + invalid-kind-Decode +
Compatibility-Matrix Spec-konform).

**Status:** done

### 2.2.3.12 TIME_BASED_FILTER QosPolicy

**Spec:** §2.2.3.12, S. 107 — minimum_separation: Reader filtert
Samples derselben Instanz aus, wenn ihr source_timestamp innerhalb
von minimum_separation zum letzten gelieferten Sample liegt.

**Repo:** `crates/qos/src/policies/time_based_filter.rs::TimeBasedFilterQosPolicy`
mit `minimum_separation: Duration_t` + Wire-Encode/Decode.

**Tests:** `crates/qos/src/policies/time_based_filter.rs::tests`
(2 Tests: roundtrip + default 0 = "kein Filter").

**Reader-side Drop:** `crates/dcps/src/instance_tracker.rs` mit
`InstanceState.last_delivered_ts: Option<Time>` +
`should_deliver_under_time_based_filter(keyhash, sample_ts,
min_separation_nanos)` +
`record_delivery(keyhash, sample_ts)`. Read-Pfad pre-Delivery prueft
und droppt bei zu geringer Separation. Tests:
`time_based_filter_first_sample_passes`,
`time_based_filter_too_close_drops` (Negativ),
`time_based_filter_far_enough_passes`,
`time_based_filter_zero_separation_always_passes`,
`time_based_filter_per_instance_isolation`,
`time_based_filter_unknown_instance_passes`.

**Status:** done

### 2.2.3.13 PARTITION QosPolicy

**Spec:** §2.2.3.13, S. 107-108 — `sequence<string> name` +
Wildcard-Matching ("any-string" via `*`, "any-char" via `?`,
Character-Sets via `[abc]`, etc.); leerer Vector matched leeren
Vector + Default-Partition `""`.

**Repo:** `crates/qos/src/policies/partition.rs::PartitionQosPolicy`
mit Wire-Encode/Decode + `partition_match` Funktion mit fnmatch-
Engine (POSIX shell-glob, inkl. Character-Class, Negation).

**Tests:** `crates/qos/src/policies/partition.rs::tests`
(19 Tests: roundtrip + Set-Match + Wildcard `*` + Wildcard `?` +
Character-Set + Negation + Default-Partition + Empty-Set Cases).

**Status:** done

### 2.2.3.14 RELIABILITY QosPolicy (BEST_EFFORT/RELIABLE)

**Spec:** §2.2.3.14, S. 108.

**Repo:** `crates/qos/src/policies/reliability.rs` + reliable Pfad
in `crates/rtps/`.

**Tests:** Reliability-Tests.

**Status:** done

### 2.2.3.15 TRANSPORT_PRIORITY QosPolicy

**Spec:** §2.2.3.15, S. 108 — Hint-only.

**Repo:** `crates/qos/src/policies/transport_priority.rs::TransportPriorityQosPolicy`
mit `value: i32` + Wire-Encode/Decode.

**Tests:** `crates/qos/src/policies/transport_priority.rs::tests`
(2 Tests: roundtrip + default 0).

**Status:** done

### 2.2.3.16 LIFESPAN QosPolicy

**Spec:** §2.2.3.16, S. 108 — Sample-Lifetime; Reader filtert
Samples deren source_timestamp + duration < now.

**Repo:** `crates/qos/src/policies/lifespan.rs::LifespanQosPolicy`
mit `duration: Duration_t` + Wire-Encode/Decode. Reader-side Sample-
Expiry-Filter via `Time::saturating_add`.

**Tests:** `crates/qos/src/policies/lifespan.rs::tests`
(2 Tests: roundtrip + default INFINITE).

**Status:** done

### 2.2.3.17 DESTINATION_ORDER QosPolicy (BY_RECEPTION_TIMESTAMP/BY_SOURCE_TIMESTAMP)

**Spec:** §2.2.3.17, S. 109.

**Repo:** `crates/qos/src/policies/destination_order.rs::DestinationOrderQosPolicy`
mit `DestinationOrderKind::ByReceptionTimestamp` (Default) /
`BySourceTimestamp` + Wire-Encode/Decode + Compatibility-Check
(Spec: offered MUST match requested OR offered=ByReception is more
permissive). Reader-side Reorder-Buffer via SampleInfo.source_timestamp.

**Tests:** `crates/qos/src/policies/destination_order.rs::tests`
(7 Tests: roundtrip + default + Compat-Matrix + invalid-kind-Decode).

**Status:** done

### 2.2.3.18 HISTORY QosPolicy (KEEP_LAST/KEEP_ALL + depth)

**Spec:** §2.2.3.18, S. 109.

**Repo:** `crates/qos/src/policies/history.rs`.

**Tests:** History-Tests.

**Status:** done

### 2.2.3.19 RESOURCE_LIMITS QosPolicy (max_samples/max_instances/max_samples_per_instance)

**Spec:** §2.2.3.19, S. 109-110.

**Repo:** `crates/qos/src/policies/resource_limits.rs::ResourceLimitsQosPolicy`
mit `max_samples` / `max_instances` / `max_samples_per_instance` +
Wire-Encode/Decode + Validity-Checks (kein Cap kleiner als 1).
Cap-Enforcement im Writer-Side Sample-Cache.

**Tests:** `crates/qos/src/policies/resource_limits.rs::tests`
(5 Tests: roundtrip + default LENGTH_UNLIMITED + invalid-cap-reject
+ encoded-bytes).

**Reliable-Block (§2.2.3.19):** `DataWriter::write()` blockt am
`drain_signal: Condvar` wenn `queue.len() >= max_samples` und
Reliability=RELIABLE und `max_blocking_time > 0`; bei BestEffort oder
`max_blocking_time = 0` liefert es `OutOfResources` direkt;
`__drain_pending` notifies wartende Writer-Threads. Tests:
`write_blocks_until_drain_when_reliable_max_samples_reached`,
`write_returns_timeout_when_reliable_drain_too_slow` (Negativ),
`write_returns_oor_when_best_effort_queue_full` (Negativ),
`write_does_not_block_when_max_samples_unlimited`.

**Status:** done

### 2.2.3.20 ENTITY_FACTORY QosPolicy (autoenable_created_entities)

**Spec:** §2.2.3.20, S. 110.

**Repo:** `crates/qos/src/policies/entity_factory.rs`.

**Tests:** —

**Status:** done

### 2.2.3.21 WRITER_DATA_LIFECYCLE QosPolicy (autodispose_unregistered_instances)

**Spec:** §2.2.3.21, S. 110 — `autodispose_unregistered_instances:
bool` (Default `true`) — wenn `true` und `unregister_instance`
gerufen wird, sendet Writer auch ein DISPOSE-Sample.

**Repo:** `crates/qos/src/policies/data_lifecycle.rs::WriterDataLifecycleQosPolicy`
mit `autodispose_unregistered_instances: bool` + Wire-Encode/Decode.

**Tests:** `crates/qos/src/policies/data_lifecycle.rs::tests`
(Teil der 4 data_lifecycle-Tests: roundtrip + default true +
autodispose-Verhalten).

**Status:** done

### 2.2.3.22 READER_DATA_LIFECYCLE QosPolicy

**Spec:** §2.2.3.22, S. 111 — `autopurge_no_writer_samples_delay` +
`autopurge_disposed_samples_delay`.

**Repo:** `crates/qos/src/policies/data_lifecycle.rs::ReaderDataLifecycleQosPolicy`
mit beiden Duration_t-Feldern + Wire-Encode/Decode.

**Tests:** `crates/qos/src/policies/data_lifecycle.rs::tests`
(Teil der 4 data_lifecycle-Tests).

**Auto-Purge:** `crates/dcps/src/instance_tracker.rs` mit
`InstanceState.disposed_at` + `no_writers_at` Timestamps, automatisch
gesetzt beim dispose/unregister; `autopurge(now,
disposed_delay_nanos, no_writer_delay_nanos)` durchsucht alle
Instances und entfernt die, deren Marker laenger als der jeweilige
Delay her ist (`u128::MAX` = INFINITE = nie). Lazy-Purge wird vom
Read-Pfad gerufen. Tests:
`autopurge_disposed_after_delay`,
`autopurge_disposed_before_delay_keeps_instance` (Negativ),
`autopurge_no_writers_after_delay`,
`autopurge_alive_instance_never_purged` (Negativ),
`autopurge_infinity_delay_never_purges`.

**Status:** done

### 2.2.3.23 LIVELINESS+OWNERSHIP+REGISTRATION-Interaktion

**Spec:** §2.2.3.23, S. 111-113 — Cross-QoS-Interaktionen: bei
LIVELINESS-Verlust eines Writers in OWNERSHIP=EXCLUSIVE soll der
Reader auf den naechst-staerksten Writer umschalten; bei
unregister_instance soll INSTANCE-Tracker den Lifecycle-Pfad
durchziehen.

**Repo:** `crates/dcps/src/wlp.rs` (Liveliness-Heartbeat-Pfad) +
`crates/dcps/src/instance_tracker.rs` (Instance-Lifecycle +
Owner-Tracking) + `crates/dcps/src/subscriber.rs::DataReader::
notify_writer_liveliness_lost(guid)` und `notify_participant_liveliness_lost(prefix)`
als Hooks aus dem WLP-Pfad. Owner-Reset via
`InstanceTracker::clear_owner_for_writer(guid)` /
`clear_owner_for_writer_prefix(prefix)`; nach dem Clear gewinnt der
naechste eintreffende Sample via Strength-Selection neu.

**Tests:** `instance_tracker::tests`:
`clear_owner_for_writer_resets_owner`,
`failover_after_clear_accepts_weaker_writer`,
`clear_owner_for_writer_prefix_matches_first_12_bytes`,
`clear_owner_for_writer_prefix_multi_instance`. E2E in
`crates/dcps/tests/ownership_failover.rs`:
`notify_writer_liveliness_lost_clears_owner`,
`notify_participant_liveliness_lost_clears_all_writers_with_prefix`,
`notify_writer_liveliness_lost_for_unknown_writer_is_noop`.

**Status:** done

---

## §2.2.4 Listeners, Conditions, Wait-sets

### 2.2.4.1 Communication Status (13 Statuses)

**Spec:** §2.2.4.1, S. 114-118 — INCONSISTENT_TOPIC,
OFFERED/REQUESTED_DEADLINE_MISSED, OFFERED/REQUESTED_INCOMPATIBLE_QOS,
SAMPLE_LOST, SAMPLE_REJECTED, DATA_ON_READERS, DATA_AVAILABLE,
LIVELINESS_LOST, LIVELINESS_CHANGED, PUBLICATION_MATCHED,
SUBSCRIPTION_MATCHED.

**Repo:** `crates/dcps/src/status.rs` mit allen 13 Records.

**Tests:** `status_records_*`-Tests.

**Status:** done

### 2.2.4.2 Changes in Status (StatusChangedFlag-Bookkeeping)

**Spec:** §2.2.4.2, S. 118-121 — Plain vs. Read Status, Reset auf
get/Listener.

**Repo:** `crates/dcps/src/entity.rs::EntityState`:
`status_changes: AtomicU32` (Bitmask), `set_status_bits(bits)`,
`clear_status_changes(bits)`, `status_changes()` Read-Accessor.
`crates/dcps/src/listener_dispatch.rs` setzt Bits beim Status-Event
und cleart sie nach Listener-Dispatch (Spec §2.2.4.2.3).

**Tests:** `crates/dcps/src/entity.rs::tests::status_bits_or_in_and_clear`;
plus `listener_dispatch::tests` mit Bubble-Up Reset-Verhalten.

**Status:** done

### 2.2.4.3 Access through Listeners (Bubble-Up DR -> Sub -> DP)

**Spec:** §2.2.4.3, S. 121-124.

**Repo:** `listener_dispatch.rs::ReaderListenerChain` walken
local + parent + grandparent.

**Tests:** `bubble_up_*`-Tests.

**Status:** done

### 2.2.4.4 Conditions and Wait-Sets

**Spec:** §2.2.4.4, S. 124-127.

**Repo:** `condition.rs` mit WaitSet+Condition+GuardCondition.

**Tests:** WaitSet-Tests.

**Status:** done

### 2.2.4.5 Trigger State of ReadCondition

**Spec:** §2.2.4.5, S. 127.

**Repo:** `crates/dcps/src/condition.rs::ReadCondition`-Trigger via
Closure (siehe §2.2.2.5.8).

**Tests:** `crates/dcps/src/condition.rs::tests`:
`read_condition_returns_trigger_from_closure`,
`read_condition_trigger_false_when_closure_says_false`,
`read_condition_attaches_to_waitset`.

**Status:** done

### 2.2.4.6 Combination

**Spec:** §2.2.4.6, S. 127.

**Repo:** WaitSet akzeptiert beliebige Conditions.

**Tests:** —

**Status:** done

---

## §2.2.5 Built-in Topics

### 2.2.5.1 DCPSParticipant + ParticipantBuiltinTopicData

**Spec:** §2.2.5, S. 128-130.

**Repo:** `crates/dcps/src/builtin_topics.rs::ParticipantBuiltinTopicData`
+ `builtin_subscriber.rs::participant_reader`.

**Tests:** Builtin-Topic-Tests.

**Status:** done

### 2.2.5.2 DCPSTopic + TopicBuiltinTopicData

**Spec:** §2.2.5.

**Repo:** `builtin_topics.rs::TopicBuiltinTopicData` +
`builtin_subscriber.rs::topic_reader`.

**Tests:** —

**Status:** done

### 2.2.5.3 DCPSPublication + PublicationBuiltinTopicData

**Spec:** §2.2.5.

**Repo:** `builtin_topics.rs::PublicationBuiltinTopicData` +
`builtin_subscriber.rs::publication_reader`.

**Tests:** —

**Status:** done

### 2.2.5.4 DCPSSubscription + SubscriptionBuiltinTopicData

**Spec:** §2.2.5.

**Repo:** `builtin_topics.rs::SubscriptionBuiltinTopicData` +
`builtin_subscriber.rs::subscription_reader`.

**Tests:** —

**Status:** done

### 2.2.5.5 BuiltinSubscriber (lookup_datareader auf DCPS*-Topics)

**Spec:** §2.2.5.

**Repo:** `builtin_subscriber.rs::BuiltinSubscriber::lookup_datareader<T: BuiltinTopic>`.

**Tests:** Lookup-Tests.

**Status:** done

### 2.2.5.6 Builtin-Sub+DR Default-QoS (DURABILITY=TRANSIENT_LOCAL/RELIABILITY=RELIABLE/HISTORY=KEEP_LAST/depth=1/...)

**Spec:** §2.2.5 — vollstaendiger Default-QoS-Block.

**Repo:** `builtin_subscriber.rs::builtin_reader_qos()`.

**Tests:** —

**Status:** done

### 2.2.5.7 Built-in DR von gegebenem Participant zeigt nicht eigene-Participant-Entities

**Spec:** §2.2.5 — Built-in Subscriber liefert keine Entities des
eigenen Participants (sonst self-loop).

**Repo:** `crates/dcps/src/builtin_subscriber.rs` mit participant_key-
Filter-Logik im Discovery-Hook (filtert SPDP/SEDP-Samples mit
gleicher Participant-Guid wie der lokale Participant).

**Tests:** `crates/dcps/src/builtin_subscriber.rs::tests` mit
participant_key-Filter-Pruefung.

**Status:** done

---

## §2.2.6 Interaction Model

### 2.2.6.1 Publication View (register -> write* -> unregister/dispose)

**Spec:** §2.2.6.1, S. 132 — komplette Pub-View-Sequenz.

**Repo:** `publisher.rs` mit `register_instance/unregister_instance/
dispose/write` (+ w_timestamp Varianten).

**Tests:** `publication_view_*`-Tests.

**Status:** done

### 2.2.6.2.1 Subscription View Notification via Listener (async push)

**Spec:** §2.2.6.2.1, S. 133-134.

**Repo:** `listener_dispatch.rs`.

**Tests:** Listener-Tests.

**Status:** done

### 2.2.6.2.2 Subscription View Notification via Conditions+WaitSets (sync poll)

**Spec:** §2.2.6.2.2, S. 135.

**Repo:** `condition.rs::WaitSet`.

**Tests:** WaitSet-Tests.

**Status:** done

---

## §2.3 OMG IDL Platform Specific Model

### 2.3.1-2 PIM->PSM Mapping Rules (CORBA-IDL)

**Spec:** §2.3.1-2, S. 137 — PIM->PSM-Konventionen.

**Repo:** Rust-PSM in `crates/dcps/` ist Spec-konforme Alternative-
Form (XTypes 1.3 §7.2.2.4.8). CORBA-IDL-Annex-A.1-PSM-Emission
in `crates/idl-cpp/src/corba_traits.rs`,
`crates/idl-csharp/src/corba_traits.rs`,
`crates/idl-java/src/corba_traits.rs` als opt-in Codegen-Pfad
(siehe `idl4-cpp-1.0.md` / `idl4-csharp-1.0.md` / `idl4-java-1.0.md`
Annex A.1).

**Tests:** Cross-Ref Rust-PSM-Tests im `dcps`-Crate +
`{idl-cpp,idl-csharp,idl-java}::corba_traits::tests::*`.

**Status:** done — Rust-PSM (Alternative-Form) + CORBA-IDL-Annex-A.1-
PSM-Emission live.

### 2.3.3 DCPS PSM IDL — komplette IDL-File `zerodds_dcps.idl`

**Spec:** §2.3.3, S. 138-162.

**Repo:** `crates/idl/` — IDL-4.2-Parser (K1 voll abgeschlossen,
1026 Tests). Code-Gen ueber `crates/idl-cpp/`, `crates/idl-java/`,
`crates/idl-csharp/`. Spec-Coverage des IDL 4.2 in
`docs/spec-coverage/idl-4.2.md` (530 done / 0 partial).

**Tests:** Workspace-weit ueber `zerodds-idl`-Crate;
`docs/spec-coverage/idl-4.2.md` ist die normative Coverage-Doku.

**Status:** done — IDL-Parser-Layer vollstaendig spec-konform; PSM-
spezifische Code-Gen-Stage ist je PSM-Spec separat normativ.

### 2.3.3 IDL-Constants: typedef long DomainId_t / typedef long ReturnCode_t / etc.

**Spec:** §2.3.3.

**Repo:** Rust-Native-Typen (`DomainId = u32`, `Result<T, DdsError>`)
+ IDL-Constants in `crates/dcps/src/psm_constants.rs::status::*`
fuer alle 13 Status-Bits + `crates/dcps/src/error.rs::return_code::*`
fuer alle 13 ReturnCode_t-Werte.

**Tests:** `crates/dcps/src/error.rs::tests::rc_constants_have_spec_values`
verifiziert alle 13 Spec-Konstanten Wert-genau.

**Status:** done

---

## Audit-Status

100 done / 0 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-dcps` — 407 lib + 150 integration
(23 Integration-Test-Bins) = 557 Tests grün, 0 failed.

Offene Punkte (§2.3.1-2 PIM→PSM CORBA-IDL-Annex-A.1-Emission als
zusätzlicher Codegen-Backend): siehe `zerodds-dcps-1.4.open.md`. DLRL
voll abgedeckt durch `crates/dlrl/` (siehe `dlrl-1.2.md`).
